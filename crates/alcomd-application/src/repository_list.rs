//! Owner-selected repository-list exchange. Registration remains the M3 use case.
use std::future::Future;

use crate::{AccessContext, M3Error, Permission, PrincipalId, RepositorySource};

pub const REPOSITORY_LIST_MAX_BYTES: usize = 1024 * 1024;
pub const REPOSITORY_LIST_MAX_ENTRIES: usize = 4096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryListError(pub &'static str);

impl std::fmt::Display for RepositoryListError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}

impl std::error::Error for RepositoryListError {}

#[derive(Clone, Debug)]
pub struct RepositoryListRecord {
    pub repository_id: String,
    pub revision: u64,
    pub source: RepositorySource,
}

/// One consistent database read, independent of presentation pagination.
#[derive(Clone, Debug, Default)]
pub struct RepositoryListSnapshot {
    pub repositories: Vec<RepositoryListRecord>,
    pub project_roots: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepositoryListEntryStatus {
    Candidate,
    Duplicate,
    Invalid,
    Unsupported,
}

#[derive(Clone, Debug)]
pub struct RepositoryListEntry {
    pub line: u32,
    pub status: RepositoryListEntryStatus,
    pub source: Option<RepositorySource>,
    pub reason: Option<&'static str>,
}

#[derive(Clone, Debug)]
pub struct RepositoryListInspection {
    pub entries: Vec<RepositoryListEntry>,
    pub ignored_lines: u32,
}

#[derive(Clone, Debug)]
pub struct RepositoryListExclusion {
    pub repository_id: String,
    pub reason: &'static str,
}

#[derive(Clone, Debug)]
pub struct RepositoryListPreview {
    pub exportable_count: u32,
    pub excluded: Vec<RepositoryListExclusion>,
    pub byte_count: u32,
    pub fingerprint: String,
}

pub struct PreparedRepositoryList {
    pub preview: RepositoryListPreview,
    pub bytes: Vec<u8>,
    pub sha256: String,
}

#[derive(Clone, Debug)]
pub struct RepositoryListExport {
    pub exported_count: u32,
    pub byte_count: u32,
    pub sha256: String,
}

pub trait RepositoryListStore: Clone + Send + Sync + 'static {
    fn repository_list_snapshot(
        &self,
        owner: PrincipalId,
    ) -> impl Future<Output = Result<RepositoryListSnapshot, M3Error>> + Send;
}

/// Bounded file exchange port, separate from the read-only source inspection adapter.
pub trait RepositoryListFiles: Clone + Send + Sync + 'static {
    fn inspect(
        &self,
        path: String,
        snapshot: RepositoryListSnapshot,
    ) -> impl Future<Output = Result<RepositoryListInspection, RepositoryListError>> + Send;
    fn prepare(
        &self,
        snapshot: &RepositoryListSnapshot,
    ) -> Result<PreparedRepositoryList, RepositoryListError>;
    fn create_new(
        &self,
        path: String,
        bytes: Vec<u8>,
        project_roots: Vec<String>,
    ) -> impl Future<Output = Result<(), RepositoryListError>> + Send;
}

#[derive(Clone)]
pub struct RepositoryListApplication<S, F> {
    store: S,
    files: F,
}

impl<S: RepositoryListStore, F: RepositoryListFiles> RepositoryListApplication<S, F> {
    pub const fn new(store: S, files: F) -> Self {
        Self { store, files }
    }

    pub async fn inspect(
        &self,
        access: &AccessContext,
        path: String,
    ) -> Result<RepositoryListInspection, RepositoryListError> {
        require(access, true)?;
        let snapshot = self.snapshot(access).await?;
        self.files.inspect(path, snapshot).await
    }

    pub async fn preview(
        &self,
        access: &AccessContext,
    ) -> Result<RepositoryListPreview, RepositoryListError> {
        require(access, false)?;
        let snapshot = self.snapshot(access).await?;
        Ok(self.files.prepare(&snapshot)?.preview)
    }

    pub async fn export(
        &self,
        access: &AccessContext,
        path: String,
        expected_fingerprint: String,
    ) -> Result<RepositoryListExport, RepositoryListError> {
        require(access, true)?;
        let snapshot = self.snapshot(access).await?;
        let prepared = self.files.prepare(&snapshot)?;
        if prepared.preview.fingerprint != expected_fingerprint {
            return Err(RepositoryListError("repository_list_changed"));
        }
        let result = RepositoryListExport {
            exported_count: prepared.preview.exportable_count,
            byte_count: prepared.preview.byte_count,
            sha256: prepared.sha256,
        };
        crate::log_execution_step(
            "repositories.exportListFile",
            "write_repository_list",
            Some(crate::log_path("file", path.clone())),
            self.files
                .create_new(path, prepared.bytes, snapshot.project_roots),
            |_| None,
        )
        .await?;
        crate::flush_log_steps().await;
        Ok(result)
    }

    async fn snapshot(
        &self,
        access: &AccessContext,
    ) -> Result<RepositoryListSnapshot, RepositoryListError> {
        self.store
            .repository_list_snapshot(access.principal().clone())
            .await
            .map_err(|_| RepositoryListError("store_unavailable"))
    }
}

fn require(access: &AccessContext, owner_only: bool) -> Result<(), RepositoryListError> {
    access
        .require(Permission::RepositoriesRead)
        .map_err(|_| RepositoryListError("permission_denied"))?;
    if owner_only && access.principal().as_str() != PrincipalId::LOCAL_OWNER {
        return Err(RepositoryListError("permission_denied"));
    }
    Ok(())
}
