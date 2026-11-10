//! Owner-selected repository-list exchange. Registration remains the M3 use case.
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::{
    AccessContext, IdempotencyKey, M3Application, M3Error, M3ErrorCode, M3ReadAdapter,
    M3RegistryStore, Permission, PrincipalId, RepositorySource,
};

pub const REPOSITORY_LIST_MAX_BYTES: usize = 1024 * 1024;
pub const REPOSITORY_LIST_MAX_ENTRIES: usize = 4096;
pub const DOWNLOAD_CONCURRENCY: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepositoryBatchItem {
    Added,
    Existing,
    Updated,
    Failed(M3ErrorCode),
    Unknown(M3ErrorCode),
    Unprocessed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryListError(pub &'static str);

impl std::fmt::Display for RepositoryListError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}

impl std::error::Error for RepositoryListError {}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryListFormat {
    Native,
    #[default]
    Legacy,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryConfiguration {
    pub display_name: Option<String>,
    pub priority: u64,
    pub enabled: bool,
    pub disabled_package_ids: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct RepositoryListMetadata {
    pub repository_id: String,
    pub expected_display_name: Option<String>,
    pub expected_priority: u64,
    pub display_name: Option<String>,
    pub priority: u64,
}

#[derive(Clone, Debug)]
pub struct RepositoryListRecord {
    pub repository_id: String,
    pub revision: u64,
    pub source: RepositorySource,
    pub credential_reference: Option<String>,
    pub configuration: RepositoryConfiguration,
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
    pub credentials_configured: bool,
    pub credentials: Option<crate::RepositoryCredentials>,
    pub configuration: Option<RepositoryConfiguration>,
}

#[derive(Clone, Debug)]
pub struct RepositoryListInspection {
    pub format: RepositoryListFormat,
    pub entries: Vec<RepositoryListEntry>,
    pub ignored_lines: u32,
    pub fingerprint: String,
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
    fn apply_repository_list_metadata(
        &self,
        owner: PrincipalId,
        changes: Vec<RepositoryListMetadata>,
        now_ms: u64,
    ) -> impl Future<Output = Result<(), M3Error>> + Send;
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
        records: Vec<RepositoryListRecord>,
    ) -> impl Future<Output = Result<PreparedRepositoryList, RepositoryListError>> + Send;
    fn prepare_configuration(
        &self,
        records: Vec<RepositoryListRecord>,
    ) -> impl Future<Output = Result<PreparedRepositoryList, RepositoryListError>> + Send;
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
    imports: Arc<Mutex<ImportControls>>,
    settings: Option<crate::M7OfficialApplication>,
}

impl<S: RepositoryListStore, F: RepositoryListFiles> RepositoryListApplication<S, F> {
    pub fn new(store: S, files: F) -> Self {
        Self {
            store,
            files,
            imports: Arc::new(Mutex::new(ImportControls::default())),
            settings: None,
        }
    }

    pub fn with_settings(mut self, settings: crate::M7OfficialApplication) -> Self {
        self.settings = Some(settings);
        self
    }

    pub async fn inspect(
        &self,
        access: &AccessContext,
        path: String,
    ) -> Result<RepositoryListInspection, RepositoryListError> {
        require(access, true)?;
        let snapshot = self.snapshot(access).await?;
        let mut result = self.files.inspect(path, snapshot).await?;
        for entry in &mut result.entries {
            entry.credentials = None;
        }
        Ok(result)
    }

    pub async fn import<R: M3ReadAdapter>(
        &self,
        access: &AccessContext,
        registry: &M3Application<S, R>,
        path: String,
        expected_fingerprint: String,
        key: IdempotencyKey,
    ) -> Result<Vec<(u32, RepositoryBatchItem)>, RepositoryListError>
    where
        S: M3RegistryStore,
    {
        self.import_with_options(access, registry, path, expected_fingerprint, key, true)
            .await
    }

    pub async fn import_with_options<R: M3ReadAdapter>(
        &self,
        access: &AccessContext,
        registry: &M3Application<S, R>,
        path: String,
        expected_fingerprint: String,
        key: IdempotencyKey,
        overwrite_existing: bool,
    ) -> Result<Vec<(u32, RepositoryBatchItem)>, RepositoryListError>
    where
        S: M3RegistryStore,
    {
        require(access, true)?;
        require_import_manage(access)?;
        let (_control, stop) = ImportGuard::start(&self.imports, key.as_str())?;
        // Classify duplicates in the file here. The registry checks currently
        // registered URLs just before admission and reports their line as existing.
        let inspection = self
            .files
            .inspect(path, RepositoryListSnapshot::default())
            .await?;
        if inspection.fingerprint != expected_fingerprint {
            return Err(RepositoryListError("repository_list_changed"));
        }
        if inspection.format == RepositoryListFormat::Native {
            return self
                .import_configuration(access, registry, inspection, key, stop, overwrite_existing)
                .await;
        }
        let (lines, urls): (Vec<_>, Vec<_>) = inspection
            .entries
            .into_iter()
            .filter_map(|entry| {
                if let (
                    RepositoryListEntryStatus::Candidate,
                    Some(RepositorySource::Remote { url }),
                ) = (entry.status, entry.source)
                {
                    Some((entry.line, (url, entry.credentials)))
                } else {
                    None
                }
            })
            .unzip();
        if lines.is_empty() {
            return Ok(Vec::new());
        }
        let items = registry
            .register_repository_list_with_credentials(access, urls, key, Some(stop))
            .await
            .map_err(|error| {
                RepositoryListError(match error.code() {
                    M3ErrorCode::PermissionDenied => "permission_denied",
                    M3ErrorCode::RepositorySourceInvalid => "repository_list_invalid",
                    M3ErrorCode::StoreUnavailable => "store_unavailable",
                    _ => "internal_error",
                })
            })?;
        Ok(lines.into_iter().zip(items).collect())
    }

    pub fn cancel_registration(
        &self,
        access: &AccessContext,
        key: &IdempotencyKey,
    ) -> Result<bool, RepositoryListError> {
        require_import_manage(access)?;
        let mut controls = self
            .imports
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(stop) = controls.active.get(key.as_str()) {
            stop.send_replace(true);
            return Ok(true);
        }
        controls.expire();
        if let Some((_, _, stopped)) = controls
            .recent
            .iter()
            .find(|(entry, _, _)| entry == key.as_str())
        {
            return Ok(*stopped);
        }
        // A stop on a second connection can overtake the start request.
        controls.remember(key.as_str().to_owned(), true);
        Ok(true)
    }

    pub async fn preview(
        &self,
        access: &AccessContext,
    ) -> Result<RepositoryListPreview, RepositoryListError> {
        require(access, false)?;
        let snapshot = self.snapshot(access).await?;
        Ok(self.files.prepare(snapshot.repositories).await?.preview)
    }

    pub async fn export(
        &self,
        access: &AccessContext,
        path: String,
        expected_fingerprint: String,
    ) -> Result<RepositoryListExport, RepositoryListError> {
        require(access, true)?;
        let snapshot = self.snapshot(access).await?;
        let prepared = self.files.prepare(snapshot.repositories).await?;
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

fn require_import_manage(access: &AccessContext) -> Result<(), RepositoryListError> {
    if access.principal() != &PrincipalId::local_owner() {
        return Err(RepositoryListError("permission_denied"));
    }
    access
        .require(Permission::RepositoriesManage)
        .map_err(|_| RepositoryListError("permission_denied"))
}

#[derive(Default)]
struct ImportControls {
    active: HashMap<String, tokio::sync::watch::Sender<bool>>,
    // The bool distinguishes an early stop from a completed import.
    recent: VecDeque<(String, Instant, bool)>,
}

impl ImportControls {
    fn expire(&mut self) {
        while self
            .recent
            .front()
            .is_some_and(|(_, time, _)| time.elapsed() >= Duration::from_secs(60))
        {
            self.recent.pop_front();
        }
    }

    fn remember(&mut self, key: String, stopped: bool) {
        self.expire();
        if self.recent.len() == 32 {
            self.recent.pop_front();
        }
        self.recent.push_back((key, Instant::now(), stopped));
    }
}

struct ImportGuard<'a> {
    controls: &'a Mutex<ImportControls>,
    key: String,
}

impl<'a> ImportGuard<'a> {
    fn start(
        controls: &'a Mutex<ImportControls>,
        key: &str,
    ) -> Result<(Self, tokio::sync::watch::Receiver<bool>), RepositoryListError> {
        let mut state = controls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.active.contains_key(key) {
            return Err(RepositoryListError("idempotency_conflict"));
        }
        if state.active.len() >= 8 {
            return Err(RepositoryListError("store_unavailable"));
        }
        state.expire();
        let stopped = state
            .recent
            .iter()
            .position(|(entry, _, _)| entry == key)
            .and_then(|index| state.recent.remove(index))
            .is_some_and(|(_, _, stopped)| stopped);
        let (sender, receiver) = tokio::sync::watch::channel(stopped);
        state.active.insert(key.to_owned(), sender);
        Ok((
            Self {
                controls,
                key: key.to_owned(),
            },
            receiver,
        ))
    }
}

impl Drop for ImportGuard<'_> {
    fn drop(&mut self) {
        let mut controls = self
            .controls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        controls.active.remove(&self.key);
        controls.remember(std::mem::take(&mut self.key), false);
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

#[path = "repository_list_config.rs"]
mod configuration;
