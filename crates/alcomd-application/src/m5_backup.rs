//! File-based Unity project backup and restore tasks.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use serde::{Deserialize, Serialize};

use crate::{
    AccessContext, IdempotencyKey, M3RegistryStore, OperationId, Permission, PrincipalId,
    ProjectId, ProjectObservation, ProjectRecord, ProjectRecoveryGuard, ResourceKey,
    ResourceLockCoordinator, Revision, StateStore, UnityWriterState, UnityWriterStateKind,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupCompression {
    Store,
    Fast,
    Level3,
    Level5,
    Level7,
    Maximum,
}

impl BackupCompression {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Store => "store",
            Self::Fast => "fast",
            Self::Level3 => "level3",
            Self::Level5 => "level5",
            Self::Level7 => "level7",
            Self::Maximum => "maximum",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupCreateRequest {
    pub project_id: ProjectId,
    pub expected_revision: Revision,
    pub compression_mode: BackupCompression,
    pub exclude_vpm_packages: bool,
    pub include_library: bool,
    pub created_at_ms: u64,
    pub destination: BackupDestination,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackupDestination {
    pub directory: String,
    pub identity: Vec<u8>,
    pub file_name: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BackupCreateOptions {
    pub compression_mode: Option<BackupCompression>,
    pub exclude_vpm_packages: Option<bool>,
    pub include_library: Option<bool>,
    pub destination_path: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackupDefaults {
    pub directory: Option<String>,
    pub project_directory: Option<String>,
    pub create_default_backup_directory: bool,
    pub compression_mode: BackupCompression,
    pub include_vpm_packages: bool,
    pub include_library: bool,
}

impl Default for BackupDefaults {
    fn default() -> Self {
        Self {
            directory: None,
            project_directory: None,
            create_default_backup_directory: true,
            compression_mode: BackupCompression::Level5,
            include_vpm_packages: true,
            include_library: false,
        }
    }
}

/// Reads backup defaults and the destination from one authoritative settings snapshot.
pub trait BackupDirectoryProvider: Send + Sync + 'static {
    fn defaults(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<BackupDefaults, M5BackupError>> + Send + '_>>;
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupCreateOutcome {
    pub operation_id: OperationId,
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

impl BackupPhase {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::InventoryReady => "inventory_ready",
            Self::Archiving => "archiving",
            Self::ArchiveReady => "archive_ready",
            Self::PublishIntent => "publish_intent",
            Self::ArchivePublished => "archive_published",
            Self::StateCommitted => "state_committed",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupArchiveEvidence {
    pub archive_sha256: [u8; 32],
    pub archive_bytes: u64,
    pub source_project_fingerprint: [u8; 32],
    pub archive_file_name: String,
    pub archive_file_identity: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackupOperationRecord {
    pub owner: PrincipalId,
    pub request: BackupCreateRequest,
    pub phase: BackupPhase,
    pub evidence: Option<BackupArchiveEvidence>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRestoreInput {
    pub archive_path: String,
    pub project_name: Option<String>,
    pub destination_path: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackupRestoreRequest {
    pub project_id: ProjectId,
    pub archive_path: PathBuf,
    pub archive_parent_identity: Vec<u8>,
    pub archive_directory_fingerprint: [u8; 32],
    pub archive_file_identity: Vec<u8>,
    pub archive_bytes: u64,
    pub archive_modified_at: std::time::SystemTime,
    pub target_parent: PathBuf,
    pub target_parent_identity: Vec<u8>,
    pub target_name: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupRestorePhase {
    Accepted,
    ArchiveVerified,
    Extracting,
    StagingComplete,
    PublishIntent,
    TargetPublished,
    ProjectRegistryCommitIntent,
    StateCommitted,
}

impl BackupRestorePhase {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::ArchiveVerified => "archive_verified",
            Self::Extracting => "extracting",
            Self::StagingComplete => "staging_complete",
            Self::PublishIntent => "publish_intent",
            Self::TargetPublished => "target_published",
            Self::ProjectRegistryCommitIntent => "project_registry_commit_intent",
            Self::StateCommitted => "state_committed",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackupRestoreOperationRecord {
    pub request: BackupRestoreRequest,
    pub phase: BackupRestorePhase,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StagedBackupRestore {
    pub request: BackupRestoreRequest,
    pub staging_root: PathBuf,
    pub project_root: PathBuf,
    pub target_root: PathBuf,
    pub owner_sidecar: PathBuf,
    pub already_published: bool,
    pub project_fingerprint: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RestoredProject {
    pub project_id: ProjectId,
    pub observation: ProjectObservation,
    pub project_fingerprint: [u8; 32],
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
    BackupInvalidArchive,
    BackupSourceChanged,
    BackupRestoreTargetExists,
    BackupRestoreTargetUnsafe,
    BackupRestoreRecoveryRequired,
    RecoveryRequired,
    StoreUnavailable,
    Internal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct M5BackupError {
    code: M5BackupErrorCode,
    context: crate::TechnicalErrorContext,
    diagnostic_id: Option<String>,
}

impl M5BackupError {
    #[must_use]
    pub fn new(code: M5BackupErrorCode) -> Self {
        Self {
            code,
            context: crate::TechnicalErrorContext::default(),
            diagnostic_id: None,
        }
    }
    #[must_use]
    pub const fn code(&self) -> M5BackupErrorCode {
        self.code
    }
    #[must_use]
    pub fn with_source(mut self, source: impl std::error::Error + Send + Sync + 'static) -> Self {
        self.context = self.context.with_source(source);
        self
    }
    #[must_use]
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.context = self.context.with_path(path);
        self
    }
    #[must_use]
    pub fn path(&self) -> Option<&str> {
        self.context.path.as_deref()
    }
    #[must_use]
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.context = self.context.with_reason(reason);
        self
    }
    #[must_use]
    pub fn with_diagnostic_id(mut self, id: String) -> Self {
        self.diagnostic_id = Some(id);
        self
    }
    #[must_use]
    pub fn diagnostic_id(&self) -> Option<&str> {
        self.diagnostic_id.as_deref()
    }
    fn ensure_diagnostic_id(mut self) -> (Self, String) {
        let id = self
            .diagnostic_id
            .clone()
            .unwrap_or_else(|| crate::OperationId::new().to_string());
        self.diagnostic_id = Some(id.clone());
        (self, id)
    }
}

impl std::fmt::Display for M5BackupError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "request failed: {:?}", self.code)?;
        if let Some(path) = self.path() {
            write!(formatter, " at {path}")?;
        }
        Ok(())
    }
}
impl std::error::Error for M5BackupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.context.source_error()
    }
}

pub trait M5BackupStore: Clone + Send + Sync + 'static {
    fn replay_backup_create(
        &self,
        owner: PrincipalId,
        project_id: ProjectId,
        expected_revision: Revision,
        options: BackupCreateOptions,
        key: IdempotencyKey,
    ) -> impl Future<Output = Result<Option<BackupCreateOutcome>, M5BackupError>> + Send;
    fn accept_backup_create(
        &self,
        owner: PrincipalId,
        request: BackupCreateRequest,
        options: BackupCreateOptions,
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
        archive_name: String,
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
    fn replay_backup_restore(
        &self,
        owner: PrincipalId,
        input: BackupRestoreInput,
        key: IdempotencyKey,
    ) -> impl Future<Output = Result<Option<BackupCreateOutcome>, M5BackupError>> + Send;
    fn accept_backup_restore(
        &self,
        owner: PrincipalId,
        input: BackupRestoreInput,
        request: BackupRestoreRequest,
        key: IdempotencyKey,
        created_at_ms: u64,
    ) -> impl Future<Output = Result<BackupCreateOutcome, M5BackupError>> + Send;
    fn begin_backup_restore(
        &self,
        operation_id: OperationId,
        updated_at_ms: u64,
    ) -> impl Future<Output = Result<BackupRestoreOperationRecord, M5BackupError>> + Send;
    fn record_backup_restore_checkpoint(
        &self,
        operation_id: OperationId,
        phase: BackupRestorePhase,
        restored: Option<RestoredProject>,
        updated_at_ms: u64,
    ) -> impl Future<Output = Result<(), M5BackupError>> + Send;
    fn complete_backup_restore(
        &self,
        operation_id: OperationId,
        restored: RestoredProject,
        completed_at_ms: u64,
    ) -> impl Future<Output = Result<(), M5BackupError>> + Send;
    fn finish_backup_restore_success(
        &self,
        operation_id: OperationId,
        completed_at_ms: u64,
    ) -> impl Future<Output = Result<(), M5BackupError>> + Send;
    fn fail_backup_restore(
        &self,
        operation_id: OperationId,
        error_code: String,
        diagnostic_id: String,
        completed_at_ms: u64,
    ) -> impl Future<Output = Result<(), M5BackupError>> + Send;
    fn recover_backup_restore_operations(
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

    fn prepare_destination(
        &self,
        directory: Option<String>,
        destination_path: Option<String>,
        create_default_directory: bool,
    ) -> impl Future<Output = Result<BackupDestination, M5BackupError>> + Send;
    fn backup_resource(&self, destination: &BackupDestination) -> Option<ResourceKey>;

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
    ) -> impl Future<Output = Result<String, M5BackupError>> + Send;
    fn discard_partial(
        &self,
        operation_id: OperationId,
        request: BackupCreateRequest,
    ) -> impl Future<Output = Result<(), M5BackupError>> + Send;
    fn prepare_restore(
        &self,
        archive_path: String,
        target: PathBuf,
    ) -> impl Future<Output = Result<BackupRestoreRequest, M5BackupError>> + Send;
    fn restore_resource(&self, request: &BackupRestoreRequest) -> ResourceKey;
    fn stage_restore(
        &self,
        operation_id: OperationId,
        request: BackupRestoreRequest,
        cancellation: BackupCancellation,
    ) -> impl Future<Output = Result<StagedBackupRestore, M5BackupError>> + Send;
    fn discard_staged_restore(
        &self,
        operation_id: OperationId,
        request: BackupRestoreRequest,
    ) -> impl Future<Output = Result<(), M5BackupError>> + Send;
    fn publish_restore(
        &self,
        staged: StagedBackupRestore,
    ) -> impl Future<Output = Result<RestoredProject, M5BackupError>> + Send;
    fn validate_published_restore(
        &self,
        operation_id: OperationId,
        request: BackupRestoreRequest,
    ) -> impl Future<Output = Result<RestoredProject, M5BackupError>> + Send;
    fn finalize_restore(
        &self,
        operation_id: OperationId,
        request: BackupRestoreRequest,
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
            .map_err(|cause| error(M5BackupErrorCode::Internal).with_source(cause))
    }
}

#[derive(Clone)]
pub struct M5BackupApplication<S: M5BackupStore, A: M5BackupAdapter, W: M5BackupWriterGate> {
    store: S,
    adapter: A,
    writer: W,
    locks: Arc<ResourceLockCoordinator>,
    directories: Option<Arc<dyn BackupDirectoryProvider>>,
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
            directories: None,
        }
    }

    #[must_use]
    pub fn with_directory_provider(
        mut self,
        directories: Arc<dyn BackupDirectoryProvider>,
    ) -> Self {
        self.directories = Some(directories);
        self
    }

    pub async fn recover(&self) -> Result<(), M5BackupError> {
        for operation_id in self.store.recover_backup_operations(now_ms()?).await? {
            self.schedule(operation_id);
        }
        for operation_id in self
            .store
            .recover_backup_restore_operations(now_ms()?)
            .await?
        {
            self.schedule_restore(operation_id, Some(self.locks.project_recovery_guard()));
        }
        Ok(())
    }

    pub async fn create(
        &self,
        access: &AccessContext,
        project_id: ProjectId,
        expected_revision: Revision,
        options: BackupCreateOptions,
        key: IdempotencyKey,
    ) -> Result<BackupCreateOutcome, M5BackupError> {
        require(access, Permission::BackupsManage)?;
        require(access, Permission::ProjectsRead)?;
        if let Some(outcome) = self
            .store
            .replay_backup_create(
                access.principal().clone(),
                project_id,
                expected_revision,
                options.clone(),
                key.clone(),
            )
            .await?
        {
            return Ok(outcome);
        }
        let defaults = match &self.directories {
            Some(provider) => provider.defaults().await?,
            None => BackupDefaults::default(),
        };
        let prepared = crate::log_execution_step(
            "backups.create",
            "prepare_destination",
            None,
            self.adapter.prepare_destination(
                defaults.directory,
                options.destination_path.clone(),
                defaults.create_default_backup_directory,
            ),
            |error| error.path().map(str::to_owned),
        )
        .await;
        let destination = match prepared {
            Ok(destination) => destination,
            Err(source) => {
                if let Some(outcome) = self
                    .store
                    .replay_backup_create(
                        access.principal().clone(),
                        project_id,
                        expected_revision,
                        options,
                        key,
                    )
                    .await?
                {
                    return Ok(outcome);
                }
                return Err(source);
            }
        };
        let request = BackupCreateRequest {
            project_id,
            expected_revision,
            compression_mode: options
                .compression_mode
                .unwrap_or(defaults.compression_mode),
            exclude_vpm_packages: options
                .exclude_vpm_packages
                .unwrap_or(!defaults.include_vpm_packages),
            include_library: options.include_library.unwrap_or(defaults.include_library),
            created_at_ms: now_ms()?,
            destination,
        };
        let outcome = self
            .store
            .accept_backup_create(access.principal().clone(), request, options, key)
            .await?;
        operation_signal(&outcome);
        if outcome.schedule {
            self.schedule(outcome.operation_id);
        }
        Ok(outcome)
    }

    pub async fn restore(
        &self,
        access: &AccessContext,
        archive_path: String,
        project_name: Option<String>,
        destination_path: Option<String>,
        key: IdempotencyKey,
    ) -> Result<BackupCreateOutcome, M5BackupError> {
        require(access, Permission::BackupsRead)?;
        require(access, Permission::BackupsManage)?;
        require(access, Permission::ProjectsCreate)?;
        if !access.is_mcp_client() && access.principal() != &PrincipalId::local_owner() {
            return Err(error(M5BackupErrorCode::PermissionDenied));
        }
        if project_name.is_some() && destination_path.is_some() {
            return Err(error(M5BackupErrorCode::InvalidInput));
        }
        let owner = access.principal();
        let input = BackupRestoreInput {
            archive_path,
            project_name,
            destination_path,
        };
        if let Some(outcome) = self
            .store
            .replay_backup_restore(owner.clone(), input.clone(), key.clone())
            .await?
        {
            return Ok(outcome);
        }
        let target = if let Some(path) = &input.destination_path {
            PathBuf::from(path)
        } else {
            let directory = self
                .directories
                .as_ref()
                .ok_or_else(|| error(M5BackupErrorCode::BackupRestoreTargetUnsafe))?
                .defaults()
                .await?
                .project_directory
                .ok_or_else(|| error(M5BackupErrorCode::BackupRestoreTargetUnsafe))?;
            let name = input
                .project_name
                .as_deref()
                .or_else(|| {
                    std::path::Path::new(&input.archive_path)
                        .file_stem()?
                        .to_str()
                })
                .ok_or_else(|| error(M5BackupErrorCode::BackupRestoreTargetUnsafe))?;
            if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\', ':']) {
                return Err(error(M5BackupErrorCode::BackupRestoreTargetUnsafe));
            }
            PathBuf::from(directory).join(name)
        };
        let prepared = crate::log_execution_step(
            "backups.restore",
            "check_backup",
            None,
            self.adapter
                .prepare_restore(input.archive_path.clone(), target),
            |error| error.path().map(str::to_owned),
        )
        .await;
        let request = match prepared {
            Ok(request) => request,
            Err(source) => {
                // Another call with the same key may have accepted and published the target.
                if let Some(outcome) = self
                    .store
                    .replay_backup_restore(owner.clone(), input, key)
                    .await?
                {
                    return Ok(outcome);
                }
                return Err(source);
            }
        };
        let outcome = self
            .store
            .accept_backup_restore(owner.clone(), input, request, key, now_ms()?)
            .await?;
        if outcome.schedule {
            self.schedule_restore(outcome.operation_id, None);
        }
        Ok(outcome)
    }

    fn schedule(&self, operation_id: OperationId) {
        let application = self.clone();
        tokio::spawn(async move {
            let _ = application.run(operation_id).await;
        });
    }

    fn schedule_restore(&self, operation_id: OperationId, recovery: Option<ProjectRecoveryGuard>) {
        let application = self.clone();
        tokio::spawn(async move {
            let _recovery = recovery;
            let _ = application.run_restore(operation_id).await;
        });
    }

    async fn run_restore(&self, operation_id: OperationId) -> Result<(), M5BackupError> {
        let result = self.run_restore_inner(operation_id).await;
        if let Err(source) = result {
            let (source, diagnostic_id) = source.ensure_diagnostic_id();
            crate::with_operation_log_failure(
                operation_id,
                crate::operation_log_error(&source, source.path()),
                self.store.fail_backup_restore(
                    operation_id,
                    error_name(source.code()).to_owned(),
                    diagnostic_id.clone(),
                    now_ms()?,
                ),
            )
            .await?;
            return Err(source);
        }
        Ok(())
    }

    async fn run_restore_inner(&self, operation_id: OperationId) -> Result<(), M5BackupError> {
        let operation = self
            .store
            .begin_backup_restore(operation_id, now_ms()?)
            .await?;
        if operation.phase == BackupRestorePhase::StateCommitted {
            self.adapter
                .finalize_restore(operation_id, operation.request)
                .await?;
            self.store
                .finish_backup_restore_success(operation_id, now_ms()?)
                .await?;
            return Ok(());
        }
        let resource = self.adapter.restore_resource(&operation.request);
        let _guard = self.locks.acquire(vec![resource]).await;
        if operation.phase == BackupRestorePhase::Accepted {
            self.store
                .record_backup_restore_checkpoint(
                    operation_id,
                    BackupRestorePhase::ArchiveVerified,
                    None,
                    now_ms()?,
                )
                .await?;
            restore_kill_gate("archive_verified");
        }
        let restored = if matches!(
            operation.phase,
            BackupRestorePhase::Accepted
                | BackupRestorePhase::ArchiveVerified
                | BackupRestorePhase::Extracting
                | BackupRestorePhase::StagingComplete
                | BackupRestorePhase::PublishIntent
        ) {
            if cancelled(&self.store, operation_id).await?
                && matches!(
                    operation.phase,
                    BackupRestorePhase::Accepted
                        | BackupRestorePhase::ArchiveVerified
                        | BackupRestorePhase::Extracting
                        | BackupRestorePhase::StagingComplete
                )
            {
                self.adapter
                    .discard_staged_restore(operation_id, operation.request.clone())
                    .await?;
                finish_cancelled(&self.store, operation_id).await?;
                return Ok(());
            }
            self.store
                .record_backup_restore_checkpoint(
                    operation_id,
                    BackupRestorePhase::Extracting,
                    None,
                    now_ms()?,
                )
                .await?;
            restore_kill_gate("extracting");
            restore_pause_gate("extracting");
            let cancellation = BackupCancellation::default();
            let polling = (operation.phase != BackupRestorePhase::PublishIntent)
                .then(|| monitor_cancellation(&self.store, operation_id, cancellation.clone()));
            let staged = self
                .adapter
                .stage_restore(
                    operation_id,
                    operation.request.clone(),
                    cancellation.clone(),
                )
                .await;
            cancellation.stop();
            if let Some(polling) = polling {
                let _ = polling.await;
            }
            let staged = match staged {
                Ok(staged) => staged,
                Err(source) => {
                    if operation.phase != BackupRestorePhase::PublishIntent
                        && cancelled(&self.store, operation_id).await?
                    {
                        self.adapter
                            .discard_staged_restore(operation_id, operation.request.clone())
                            .await?;
                        finish_cancelled(&self.store, operation_id).await?;
                        return Ok(());
                    }
                    return Err(source);
                }
            };
            self.store
                .record_backup_restore_checkpoint(
                    operation_id,
                    BackupRestorePhase::StagingComplete,
                    None,
                    now_ms()?,
                )
                .await?;
            restore_kill_gate("staging_complete");
            if operation.phase != BackupRestorePhase::PublishIntent
                && cancelled(&self.store, operation_id).await?
            {
                self.adapter
                    .discard_staged_restore(operation_id, operation.request.clone())
                    .await?;
                finish_cancelled(&self.store, operation_id).await?;
                return Ok(());
            }
            self.store
                .record_backup_restore_checkpoint(
                    operation_id,
                    BackupRestorePhase::PublishIntent,
                    None,
                    now_ms()?,
                )
                .await?;
            restore_kill_gate("publish_intent");
            restore_pause_gate("publish_intent");
            let restored = self.adapter.publish_restore(staged).await?;
            self.store
                .record_backup_restore_checkpoint(
                    operation_id,
                    BackupRestorePhase::TargetPublished,
                    Some(restored.clone()),
                    now_ms()?,
                )
                .await?;
            restore_kill_gate("target_published");
            restored
        } else {
            self.adapter
                .validate_published_restore(operation_id, operation.request.clone())
                .await?
        };
        self.store
            .record_backup_restore_checkpoint(
                operation_id,
                BackupRestorePhase::ProjectRegistryCommitIntent,
                Some(restored.clone()),
                now_ms()?,
            )
            .await?;
        restore_kill_gate("project_registry_commit_intent");
        self.store
            .complete_backup_restore(operation_id, restored, now_ms()?)
            .await?;
        restore_kill_gate("state_committed");
        self.adapter
            .finalize_restore(operation_id, operation.request)
            .await?;
        self.store
            .finish_backup_restore_success(operation_id, now_ms()?)
            .await?;
        Ok(())
    }

    async fn run(&self, operation_id: OperationId) -> Result<(), M5BackupError> {
        let result = self.run_inner(operation_id).await;
        if let Err(source) = result {
            let (source, diagnostic_id) = source.ensure_diagnostic_id();
            if source.code() == M5BackupErrorCode::RecoveryRequired {
                self.store
                    .defer_backup_recovery(operation_id, diagnostic_id.clone(), now_ms()?)
                    .await?;
                return Err(source);
            }
            crate::with_operation_log_failure(
                operation_id,
                crate::operation_log_error(&source, source.path()),
                self.store.fail_backup_create(
                    operation_id,
                    error_name(source.code()).to_owned(),
                    diagnostic_id.clone(),
                    now_ms()?,
                ),
            )
            .await?;
            return Err(source);
        }
        Ok(())
    }

    async fn run_inner(&self, operation_id: OperationId) -> Result<(), M5BackupError> {
        let mut operation = self
            .store
            .begin_backup_create(operation_id, now_ms()?)
            .await?;
        if cancelled(&self.store, operation_id).await?
            && matches!(
                operation.phase,
                BackupPhase::Accepted
                    | BackupPhase::InventoryReady
                    | BackupPhase::Archiving
                    | BackupPhase::ArchiveReady
            )
        {
            if matches!(
                operation.phase,
                BackupPhase::Archiving | BackupPhase::ArchiveReady
            ) {
                self.adapter
                    .discard_partial(operation_id, operation.request.clone())
                    .await?;
            }
            finish_cancelled(&self.store, operation_id).await?;
            return Ok(());
        }
        let resource_owner = crate::workflow_resource_owner(&operation.owner);
        let access = AccessContext::new(resource_owner.clone(), []);
        let keys = std::iter::once(ResourceKey::Project(operation.request.project_id))
            .chain(self.adapter.backup_resource(&operation.request.destination))
            .collect();
        let _guard = self.locks.acquire(keys).await;

        if matches!(
            operation.phase,
            BackupPhase::Accepted | BackupPhase::InventoryReady | BackupPhase::Archiving
        ) {
            let project = self
                .store
                .get_project(resource_owner, operation.request.project_id)
                .await
                .map_err(|cause| {
                    error(M5BackupErrorCode::ProjectNotRegistered).with_source(cause)
                })?;
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
            let polling = monitor_cancellation(&self.store, operation_id, cancellation.clone());
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
                self.adapter
                    .discard_partial(operation_id, operation.request.clone())
                    .await?;
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
                self.adapter
                    .discard_partial(operation_id, operation.request.clone())
                    .await?;
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
        self.store
            .complete_backup_create(operation_id, published, now_ms()?)
            .await?;
        kill_gate("state_committed");
        Ok(())
    }
}

fn monitor_cancellation<S: StateStore + Clone + Send + Sync + 'static>(
    store: &S,
    operation_id: OperationId,
    monitor: BackupCancellation,
) -> tokio::task::JoinHandle<()> {
    let store = store.clone();
    tokio::spawn(async move {
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
    })
}

async fn cancelled<S: StateStore>(
    store: &S,
    operation_id: OperationId,
) -> Result<bool, M5BackupError> {
    store
        .cancellation_requested(operation_id)
        .await
        .map_err(|cause| error(M5BackupErrorCode::StoreUnavailable).with_source(cause))
}

async fn finish_cancelled<S: StateStore>(
    store: &S,
    operation_id: OperationId,
) -> Result<(), M5BackupError> {
    store
        .finish_cancelled(operation_id, now_ms()?)
        .await
        .map(|_| ())
        .map_err(|cause| error(M5BackupErrorCode::StoreUnavailable).with_source(cause))
}

fn require(access: &AccessContext, permission: Permission) -> Result<(), M5BackupError> {
    access
        .require(permission)
        .map_err(|cause| error(M5BackupErrorCode::PermissionDenied).with_source(cause))
}

fn now_ms() -> Result<u64, M5BackupError> {
    use std::time::{SystemTime, UNIX_EPOCH};
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|cause| error(M5BackupErrorCode::Internal).with_source(cause))?
            .as_millis(),
    )
    .map_err(|cause| error(M5BackupErrorCode::Internal).with_source(cause))
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
fn restore_kill_gate(phase: &str) {
    if std::env::var("ALCOMD_TEST_BACKUP_RESTORE_KILL_GATE").as_deref() == Ok(phase) {
        let signal = std::env::var_os("ALCOMD_TEST_BACKUP_RESTORE_KILL_SIGNAL")
            .expect("Backup Restore kill gate signal path");
        std::fs::write(signal, phase).expect("write Backup Restore kill gate signal");
        loop {
            std::thread::park();
        }
    }
}

#[cfg(not(feature = "test-kill-gates"))]
fn restore_kill_gate(_: &str) {}

#[cfg(feature = "test-kill-gates")]
fn restore_pause_gate(phase: &str) {
    if std::env::var("ALCOMD_TEST_BACKUP_RESTORE_PAUSE_GATE").as_deref() == Ok(phase) {
        let signal = std::env::var_os("ALCOMD_TEST_BACKUP_RESTORE_PAUSE_SIGNAL")
            .expect("Backup Restore pause gate signal path");
        let release = std::env::var_os("ALCOMD_TEST_BACKUP_RESTORE_PAUSE_RELEASE")
            .expect("Backup Restore pause gate release path");
        std::fs::write(signal, phase).expect("write Backup Restore pause gate signal");
        while !std::path::Path::new(&release).exists() {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}

#[cfg(not(feature = "test-kill-gates"))]
fn restore_pause_gate(_: &str) {}

#[cfg(feature = "test-kill-gates")]
fn operation_signal(outcome: &BackupCreateOutcome) {
    if let Some(path) = std::env::var_os("ALCOMD_TEST_BACKUP_OPERATION_SIGNAL") {
        std::fs::write(
            path,
            format!(
                "{{\"operationId\":\"{}\",\"replayed\":{}}}",
                outcome.operation_id, outcome.replayed
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
        M5BackupErrorCode::BackupInvalidArchive => "backup_invalid_archive",
        M5BackupErrorCode::BackupSourceChanged => "backup_source_changed",
        M5BackupErrorCode::BackupRestoreTargetExists => "backup_target_exists",
        M5BackupErrorCode::BackupRestoreTargetUnsafe => "backup_target_invalid",
        M5BackupErrorCode::BackupRestoreRecoveryRequired => "backup_restore_recovery_required",
        M5BackupErrorCode::RecoveryRequired => "backup_unavailable",
        M5BackupErrorCode::StoreUnavailable => "store_unavailable",
        M5BackupErrorCode::Internal => "internal_error",
    }
}

fn error(code: M5BackupErrorCode) -> M5BackupError {
    M5BackupError::new(code)
}
