//! M5 managed native Backup Create use cases.

use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use serde::{Deserialize, Serialize};

use crate::{
    AccessContext, BackupId, IdempotencyKey, M3RegistryStore, OperationId, Permission, PrincipalId,
    ProjectId, ProjectRecord, ResourceKey, ResourceLockCoordinator, Revision, StateStore,
    UnityWriterState, UnityWriterStateKind,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupCompression {
    Store,
    Fast,
    Maximum,
}

impl BackupCompression {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Store => "store",
            Self::Fast => "fast",
            Self::Maximum => "maximum",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRecord {
    pub backup_id: BackupId,
    pub source_project_id: ProjectId,
    pub archive_sha256: [u8; 32],
    pub archive_bytes: u64,
    pub format_version: u32,
    pub created_at_ms: u64,
    pub compression_mode: BackupCompression,
    pub exclude_vpm_packages: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredBackupRecord {
    pub record: BackupRecord,
    pub archive_locator: String,
    pub file_identity_key: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BackupCursor {
    pub created_at_ms: u64,
    pub backup_id: BackupId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackupPage {
    pub backups: Vec<BackupRecord>,
    pub next_cursor: Option<BackupCursor>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupCreateRequest {
    pub backup_id: BackupId,
    pub project_id: ProjectId,
    pub expected_revision: Revision,
    pub compression_mode: BackupCompression,
    pub exclude_vpm_packages: bool,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupCreateOutcome {
    pub operation_id: OperationId,
    pub backup_id: BackupId,
    pub replayed: bool,
    #[serde(skip)]
    pub schedule: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupPhase {
    Accepted,
    InventoryReady,
    Archiving,
    ArchiveReady,
    PublishIntent,
    ArchivePublished,
    StateCommitted,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupArchiveEvidence {
    pub archive_sha256: [u8; 32],
    pub archive_bytes: u64,
    pub source_project_fingerprint: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackupOperationRecord {
    pub owner: PrincipalId,
    pub request: BackupCreateRequest,
    pub phase: BackupPhase,
    pub evidence: Option<BackupArchiveEvidence>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishedBackup {
    pub archive_locator: String,
    pub file_identity_key: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum M5BackupErrorCode {
    InvalidInput,
    PermissionDenied,
    BackupNotFound,
    ProjectNotRegistered,
    RevisionConflict,
    IdempotencyConflict,
    UnityProjectRunning,
    BackupSourceUnsafe,
    BackupLimitExceeded,
    ProjectChangedDuringBackup,
    BackupIntegrityMismatch,
    RecoveryRequired,
    StoreUnavailable,
    Internal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct M5BackupError {
    code: M5BackupErrorCode,
}

impl M5BackupError {
    #[must_use]
    pub const fn new(code: M5BackupErrorCode) -> Self {
        Self { code }
    }

    #[must_use]
    pub const fn code(self) -> M5BackupErrorCode {
        self.code
    }
}

impl std::fmt::Display for M5BackupError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Backup request failed")
    }
}

impl std::error::Error for M5BackupError {}

pub trait M5BackupStore: Clone + Send + Sync + 'static {
    fn list_backups(
        &self,
        owner: PrincipalId,
        project_id: Option<ProjectId>,
        cursor: Option<BackupCursor>,
        limit: u32,
    ) -> impl Future<Output = Result<Vec<StoredBackupRecord>, M5BackupError>> + Send;
    fn get_backup(
        &self,
        owner: PrincipalId,
        backup_id: BackupId,
    ) -> impl Future<Output = Result<StoredBackupRecord, M5BackupError>> + Send;
    fn accept_backup_create(
        &self,
        owner: PrincipalId,
        request: BackupCreateRequest,
        key: IdempotencyKey,
    ) -> impl Future<Output = Result<BackupCreateOutcome, M5BackupError>> + Send;
    fn begin_backup_create(
        &self,
        operation_id: OperationId,
        updated_at_ms: u64,
    ) -> impl Future<Output = Result<BackupOperationRecord, M5BackupError>> + Send;
    fn record_backup_checkpoint(
        &self,
        operation_id: OperationId,
        phase: BackupPhase,
        evidence: Option<BackupArchiveEvidence>,
        updated_at_ms: u64,
    ) -> impl Future<Output = Result<(), M5BackupError>> + Send;
    fn complete_backup_create(
        &self,
        operation_id: OperationId,
        backup: StoredBackupRecord,
        completed_at_ms: u64,
    ) -> impl Future<Output = Result<(), M5BackupError>> + Send;
    fn fail_backup_create(
        &self,
        operation_id: OperationId,
        error_code: String,
        diagnostic_id: String,
        completed_at_ms: u64,
    ) -> impl Future<Output = Result<(), M5BackupError>> + Send;
    fn defer_backup_recovery(
        &self,
        operation_id: OperationId,
        diagnostic_id: String,
        updated_at_ms: u64,
    ) -> impl Future<Output = Result<(), M5BackupError>> + Send;
    fn recover_backup_operations(
        &self,
        recovered_at_ms: u64,
    ) -> impl Future<Output = Result<Vec<OperationId>, M5BackupError>> + Send;
}

#[derive(Clone, Default)]
pub struct BackupCancellation(Arc<AtomicU8>);

impl BackupCancellation {
    #[must_use]
    pub fn cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire) == 1
    }

    pub fn cancel(&self) {
        self.0.store(1, Ordering::Release);
    }

    fn stop(&self) {
        self.0.store(2, Ordering::Release);
    }

    fn stopped(&self) -> bool {
        self.0.load(Ordering::Acquire) == 2
    }
}

pub trait M5BackupAdapter: Clone + Send + Sync + 'static {
    type Inventory: Send + 'static;

    fn inventory(
        &self,
        project: ProjectRecord,
        request: BackupCreateRequest,
    ) -> impl Future<Output = Result<Self::Inventory, M5BackupError>> + Send;
    fn archive(
        &self,
        operation_id: OperationId,
        request: BackupCreateRequest,
        inventory: Self::Inventory,
        cancellation: BackupCancellation,
    ) -> impl Future<Output = Result<BackupArchiveEvidence, M5BackupError>> + Send;
    fn publish_or_recover(
        &self,
        operation_id: OperationId,
        request: BackupCreateRequest,
        evidence: BackupArchiveEvidence,
    ) -> impl Future<Output = Result<PublishedBackup, M5BackupError>> + Send;
    fn discard_partial(
        &self,
        operation_id: OperationId,
    ) -> impl Future<Output = Result<(), M5BackupError>> + Send;
}

pub trait M5BackupWriterGate: Clone + Send + Sync + 'static {
    fn observe_backup_source(
        &self,
        access: &AccessContext,
        project_id: ProjectId,
    ) -> impl Future<Output = Result<UnityWriterState, M5BackupError>> + Send;
}

impl<S, P> M5BackupWriterGate for crate::M5UnityApplication<S, P>
where
    S: crate::M5UnityStore + M3RegistryStore,
    P: crate::M5UnityPlatform,
{
    async fn observe_backup_source(
        &self,
        access: &AccessContext,
        project_id: ProjectId,
    ) -> Result<UnityWriterState, M5BackupError> {
        self.writer_state_unchecked(access, project_id)
            .await
            .map_err(|_| error(M5BackupErrorCode::Internal))
    }
}

#[derive(Clone)]
pub struct M5BackupApplication<S: M5BackupStore, A: M5BackupAdapter, W: M5BackupWriterGate> {
    store: S,
    adapter: A,
    writer: W,
    locks: Arc<ResourceLockCoordinator>,
}

impl<S, A, W> M5BackupApplication<S, A, W>
where
    S: M5BackupStore + M3RegistryStore + StateStore,
    A: M5BackupAdapter,
    W: M5BackupWriterGate,
{
    #[must_use]
    pub fn with_locks(
        store: S,
        adapter: A,
        writer: W,
        locks: Arc<ResourceLockCoordinator>,
    ) -> Self {
        Self {
            store,
            adapter,
            writer,
            locks,
        }
    }

    pub async fn recover(&self) -> Result<(), M5BackupError> {
        for operation_id in self.store.recover_backup_operations(now_ms()?).await? {
            self.schedule(operation_id);
        }
        Ok(())
    }

    pub async fn list(
        &self,
        access: &AccessContext,
        project_id: Option<ProjectId>,
        cursor: Option<BackupCursor>,
        limit: u32,
    ) -> Result<BackupPage, M5BackupError> {
        require(access, Permission::BackupsRead)?;
        if !(1..=1_000).contains(&limit) {
            return Err(error(M5BackupErrorCode::InvalidInput));
        }
        let mut records = self
            .store
            .list_backups(access.principal().clone(), project_id, cursor, limit + 1)
            .await?;
        let has_more = records.len() > limit as usize;
        if has_more {
            records.pop();
        }
        let backups = records
            .into_iter()
            .map(|value| value.record)
            .collect::<Vec<_>>();
        let next_cursor = has_more.then(|| {
            let last = backups.last().expect("non-empty over-limit Backup page");
            BackupCursor {
                created_at_ms: last.created_at_ms,
                backup_id: last.backup_id,
            }
        });
        Ok(BackupPage {
            backups,
            next_cursor,
        })
    }

    pub async fn get(
        &self,
        access: &AccessContext,
        backup_id: BackupId,
    ) -> Result<BackupRecord, M5BackupError> {
        require(access, Permission::BackupsRead)?;
        Ok(self
            .store
            .get_backup(access.principal().clone(), backup_id)
            .await?
            .record)
    }

    pub async fn create(
        &self,
        access: &AccessContext,
        project_id: ProjectId,
        expected_revision: Revision,
        compression_mode: BackupCompression,
        exclude_vpm_packages: bool,
        key: IdempotencyKey,
    ) -> Result<BackupCreateOutcome, M5BackupError> {
        require(access, Permission::BackupsManage)?;
        require(access, Permission::ProjectsRead)?;
        let request = BackupCreateRequest {
            backup_id: BackupId::new(),
            project_id,
            expected_revision,
            compression_mode,
            exclude_vpm_packages,
            created_at_ms: now_ms()?,
        };
        let outcome = self
            .store
            .accept_backup_create(access.principal().clone(), request, key)
            .await?;
        operation_signal(&outcome);
        if outcome.schedule {
            self.schedule(outcome.operation_id);
        }
        Ok(outcome)
    }

    fn schedule(&self, operation_id: OperationId) {
        let application = self.clone();
        tokio::spawn(async move {
            let _ = application.run(operation_id).await;
        });
    }

    async fn run(&self, operation_id: OperationId) -> Result<(), M5BackupError> {
        let result = self.run_inner(operation_id).await;
        if let Err(source) = result {
            if source.code() == M5BackupErrorCode::RecoveryRequired {
                self.store
                    .defer_backup_recovery(operation_id, OperationId::new().to_string(), now_ms()?)
                    .await?;
                return Err(source);
            }
            self.store
                .fail_backup_create(
                    operation_id,
                    error_name(source.code()).to_owned(),
                    OperationId::new().to_string(),
                    now_ms()?,
                )
                .await?;
            return Err(source);
        }
        Ok(())
    }

    async fn run_inner(&self, operation_id: OperationId) -> Result<(), M5BackupError> {
        if cancelled(&self.store, operation_id).await? {
            finish_cancelled(&self.store, operation_id).await?;
            return Ok(());
        }
        let mut operation = self
            .store
            .begin_backup_create(operation_id, now_ms()?)
            .await?;
        let access = AccessContext::new(operation.owner.clone(), []);
        let _guard = self
            .locks
            .acquire(vec![ResourceKey::Project(operation.request.project_id)])
            .await;
        let project = self
            .store
            .get_project(operation.owner.clone(), operation.request.project_id)
            .await
            .map_err(|_| error(M5BackupErrorCode::ProjectNotRegistered))?;
        if project.revision != operation.request.expected_revision {
            return Err(error(M5BackupErrorCode::RevisionConflict));
        }
        let writer = self
            .writer
            .observe_backup_source(&access, project.project_id)
            .await?;
        if writer.state == UnityWriterStateKind::RunningConfirmed {
            return Err(error(M5BackupErrorCode::UnityProjectRunning));
        }

        if matches!(
            operation.phase,
            BackupPhase::Accepted | BackupPhase::InventoryReady | BackupPhase::Archiving
        ) {
            let inventory = self
                .adapter
                .inventory(project, operation.request.clone())
                .await?;
            self.store
                .record_backup_checkpoint(
                    operation_id,
                    BackupPhase::InventoryReady,
                    None,
                    now_ms()?,
                )
                .await?;
            kill_gate("inventory_ready");
            if cancelled(&self.store, operation_id).await? {
                finish_cancelled(&self.store, operation_id).await?;
                return Ok(());
            }
            self.store
                .record_backup_checkpoint(operation_id, BackupPhase::Archiving, None, now_ms()?)
                .await?;
            let cancellation = BackupCancellation::default();
            let monitor = cancellation.clone();
            let store = self.store.clone();
            let polling = tokio::spawn(async move {
                while !monitor.stopped() {
                    if store
                        .cancellation_requested(operation_id)
                        .await
                        .unwrap_or(false)
                    {
                        monitor.cancel();
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                }
            });
            let archived = self
                .adapter
                .archive(
                    operation_id,
                    operation.request.clone(),
                    inventory,
                    cancellation.clone(),
                )
                .await;
            cancellation.stop();
            let _ = polling.await;
            if archived.is_err() && cancelled(&self.store, operation_id).await? {
                self.adapter.discard_partial(operation_id).await?;
                finish_cancelled(&self.store, operation_id).await?;
                return Ok(());
            }
            let evidence = archived?;
            self.store
                .record_backup_checkpoint(
                    operation_id,
                    BackupPhase::ArchiveReady,
                    Some(evidence.clone()),
                    now_ms()?,
                )
                .await?;
            kill_gate("archive_ready");
            operation.phase = BackupPhase::ArchiveReady;
            operation.evidence = Some(evidence);
        }
        let evidence = operation
            .evidence
            .clone()
            .ok_or_else(|| error(M5BackupErrorCode::RecoveryRequired))?;
        if operation.phase == BackupPhase::ArchiveReady {
            if cancelled(&self.store, operation_id).await? {
                self.adapter.discard_partial(operation_id).await?;
                finish_cancelled(&self.store, operation_id).await?;
                return Ok(());
            }
            self.store
                .record_backup_checkpoint(
                    operation_id,
                    BackupPhase::PublishIntent,
                    Some(evidence.clone()),
                    now_ms()?,
                )
                .await?;
            kill_gate("publish_intent");
        }
        let published = self
            .adapter
            .publish_or_recover(operation_id, operation.request.clone(), evidence.clone())
            .await?;
        self.store
            .record_backup_checkpoint(
                operation_id,
                BackupPhase::ArchivePublished,
                Some(evidence.clone()),
                now_ms()?,
            )
            .await?;
        kill_gate("archive_published");
        let backup = StoredBackupRecord {
            record: BackupRecord {
                backup_id: operation.request.backup_id,
                source_project_id: operation.request.project_id,
                archive_sha256: evidence.archive_sha256,
                archive_bytes: evidence.archive_bytes,
                format_version: 1,
                created_at_ms: operation.request.created_at_ms,
                compression_mode: operation.request.compression_mode,
                exclude_vpm_packages: operation.request.exclude_vpm_packages,
            },
            archive_locator: published.archive_locator,
            file_identity_key: published.file_identity_key,
        };
        self.store
            .complete_backup_create(operation_id, backup, now_ms()?)
            .await?;
        kill_gate("state_committed");
        Ok(())
    }
}

async fn cancelled<S: StateStore>(
    store: &S,
    operation_id: OperationId,
) -> Result<bool, M5BackupError> {
    store
        .cancellation_requested(operation_id)
        .await
        .map_err(|_| error(M5BackupErrorCode::StoreUnavailable))
}

async fn finish_cancelled<S: StateStore>(
    store: &S,
    operation_id: OperationId,
) -> Result<(), M5BackupError> {
    store
        .finish_cancelled(operation_id, now_ms()?)
        .await
        .map(|_| ())
        .map_err(|_| error(M5BackupErrorCode::StoreUnavailable))
}

fn require(access: &AccessContext, permission: Permission) -> Result<(), M5BackupError> {
    access
        .require(permission)
        .map_err(|_| error(M5BackupErrorCode::PermissionDenied))
}

fn now_ms() -> Result<u64, M5BackupError> {
    use std::time::{SystemTime, UNIX_EPOCH};
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| error(M5BackupErrorCode::Internal))?
            .as_millis(),
    )
    .map_err(|_| error(M5BackupErrorCode::Internal))
}

#[cfg(feature = "test-kill-gates")]
fn kill_gate(phase: &str) {
    if std::env::var("ALCOMD_TEST_BACKUP_KILL_GATE").as_deref() == Ok(phase) {
        let signal = std::env::var_os("ALCOMD_TEST_BACKUP_KILL_SIGNAL")
            .expect("Backup kill gate signal path");
        std::fs::write(signal, phase).expect("write Backup kill gate signal");
        loop {
            std::thread::park();
        }
    }
}

#[cfg(not(feature = "test-kill-gates"))]
fn kill_gate(_: &str) {}

#[cfg(feature = "test-kill-gates")]
fn operation_signal(outcome: &BackupCreateOutcome) {
    if let Some(path) = std::env::var_os("ALCOMD_TEST_BACKUP_OPERATION_SIGNAL") {
        std::fs::write(
            path,
            format!(
                "{{\"operationId\":\"{}\",\"backupId\":\"{}\",\"replayed\":{}}}",
                outcome.operation_id, outcome.backup_id, outcome.replayed
            ),
        )
        .expect("write Backup operation signal");
    }
}

#[cfg(not(feature = "test-kill-gates"))]
fn operation_signal(_: &BackupCreateOutcome) {}

#[must_use]
pub const fn error_name(code: M5BackupErrorCode) -> &'static str {
    match code {
        M5BackupErrorCode::InvalidInput => "invalid_request",
        M5BackupErrorCode::PermissionDenied => "permission_denied",
        M5BackupErrorCode::BackupNotFound => "backup_not_found",
        M5BackupErrorCode::ProjectNotRegistered => "project_not_registered",
        M5BackupErrorCode::RevisionConflict => "revision_conflict",
        M5BackupErrorCode::IdempotencyConflict => "idempotency_conflict",
        M5BackupErrorCode::UnityProjectRunning => "unity_project_running",
        M5BackupErrorCode::BackupSourceUnsafe => "backup_source_unsafe",
        M5BackupErrorCode::BackupLimitExceeded => "backup_archive_limit_exceeded",
        M5BackupErrorCode::ProjectChangedDuringBackup => "project_changed_during_backup",
        M5BackupErrorCode::BackupIntegrityMismatch => "backup_integrity_mismatch",
        M5BackupErrorCode::RecoveryRequired => "backup_unavailable",
        M5BackupErrorCode::StoreUnavailable => "store_unavailable",
        M5BackupErrorCode::Internal => "internal_error",
    }
}

const fn error(code: M5BackupErrorCode) -> M5BackupError {
    M5BackupError::new(code)
}
