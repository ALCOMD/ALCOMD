//! M5 managed native Backup Create use cases.

use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use serde::{Deserialize, Serialize};

use crate::{
    AccessContext, BackupId, IdempotencyKey, M3RegistryStore, OperationId, Permission, PlanId,
    PrincipalId, ProjectId, ProjectObservation, ProjectRecord, ResourceKey,
    ResourceLockCoordinator, Revision, StateStore, UnityWriterState, UnityWriterStateKind,
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreExcludedPackage {
    pub package_id: String,
    pub version: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRestoreTarget {
    pub parent: String,
    pub leaf: String,
    pub must_be_absent: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRestorePlanDraft {
    pub plan_id: PlanId,
    pub project_id: ProjectId,
    pub backup_id: BackupId,
    pub archive_sha256: [u8; 32],
    pub archive_file_identity: Vec<u8>,
    pub archive_bytes: u64,
    pub manifest_fingerprint: [u8; 32],
    pub exclude_vpm_packages: bool,
    pub excluded_packages: Vec<RestoreExcludedPackage>,
    pub target: BackupRestoreTarget,
    pub target_parent_identity: Vec<u8>,
    pub expected_unity_project_json: String,
    pub plan_fingerprint: [u8; 32],
    pub plan_json: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRestorePlanRecord {
    #[serde(flatten)]
    pub draft: BackupRestorePlanDraft,
    pub owner: PrincipalId,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRestorePlanOutcome {
    pub plan_id: PlanId,
    pub project_id: ProjectId,
    pub backup_id: BackupId,
    pub target: BackupRestoreTarget,
    pub archive_sha256: [u8; 32],
    pub packages_require_resolve: bool,
    pub excluded_packages: Vec<RestoreExcludedPackage>,
    pub plan_fingerprint: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRestoreApplyOutcome {
    pub operation_id: OperationId,
    pub project_id: ProjectId,
    pub replayed: bool,
    #[serde(skip)]
    pub schedule: bool,
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
    pub plan: BackupRestorePlanRecord,
    pub phase: BackupRestorePhase,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedBackupRestore {
    pub plan: BackupRestorePlanRecord,
    pub archive_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StagedBackupRestore {
    pub plan: BackupRestorePlanRecord,
    pub staging_root: PathBuf,
    pub project_root: PathBuf,
    pub target_root: PathBuf,
    pub owner_sidecar: PathBuf,
    pub already_published: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RestoredProject {
    pub project_id: ProjectId,
    pub observation: ProjectObservation,
    pub target_identity: Vec<u8>,
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
    BackupRestorePlanNotFound,
    BackupRestorePlanStale,
    BackupRestoreTargetExists,
    BackupRestoreTargetUnsafe,
    BackupRestoreRecoveryRequired,
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
    fn create_backup_restore_plan(
        &self,
        owner: PrincipalId,
        draft: BackupRestorePlanDraft,
        created_at_ms: u64,
    ) -> impl Future<Output = Result<BackupRestorePlanRecord, M5BackupError>> + Send;
    fn accept_backup_restore(
        &self,
        owner: PrincipalId,
        plan_id: PlanId,
        key: IdempotencyKey,
        created_at_ms: u64,
    ) -> impl Future<Output = Result<BackupRestoreApplyOutcome, M5BackupError>> + Send;
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
    fn completed_backup_restores(
        &self,
    ) -> impl Future<Output = Result<Vec<(OperationId, BackupRestorePlanRecord)>, M5BackupError>> + Send;
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
    fn plan_restore(
        &self,
        backup: StoredBackupRecord,
        target_parent: PathBuf,
        target_leaf: String,
    ) -> impl Future<Output = Result<BackupRestorePlanDraft, M5BackupError>> + Send;
    fn restore_resource(
        &self,
        plan: &BackupRestorePlanRecord,
    ) -> Result<ResourceKey, M5BackupError>;
    fn prepare_restore(
        &self,
        plan: BackupRestorePlanRecord,
    ) -> impl Future<Output = Result<PreparedBackupRestore, M5BackupError>> + Send;
    fn stage_restore(
        &self,
        operation_id: OperationId,
        prepared: PreparedBackupRestore,
    ) -> impl Future<Output = Result<StagedBackupRestore, M5BackupError>> + Send;
    fn discard_staged_restore(
        &self,
        staged: StagedBackupRestore,
    ) -> impl Future<Output = Result<(), M5BackupError>> + Send;
    fn publish_restore(
        &self,
        staged: StagedBackupRestore,
    ) -> impl Future<Output = Result<RestoredProject, M5BackupError>> + Send;
    fn validate_published_restore(
        &self,
        operation_id: OperationId,
        plan: BackupRestorePlanRecord,
    ) -> impl Future<Output = Result<RestoredProject, M5BackupError>> + Send;
    fn finalize_restore(
        &self,
        operation_id: OperationId,
        plan: BackupRestorePlanRecord,
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
        for operation_id in self
            .store
            .recover_backup_restore_operations(now_ms()?)
            .await?
        {
            self.schedule_restore(operation_id);
        }
        for (operation_id, plan) in self.store.completed_backup_restores().await? {
            self.adapter.finalize_restore(operation_id, plan).await?;
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

    pub async fn plan_restore(
        &self,
        access: &AccessContext,
        backup_id: BackupId,
        target_parent: PathBuf,
        target_leaf: String,
    ) -> Result<BackupRestorePlanOutcome, M5BackupError> {
        require(access, Permission::BackupsRead)?;
        require(access, Permission::ProjectsCreate)?;
        if access.principal() != &PrincipalId::local_owner() {
            return Err(error(M5BackupErrorCode::PermissionDenied));
        }
        let backup = self
            .store
            .get_backup(access.principal().clone(), backup_id)
            .await?;
        let draft = self
            .adapter
            .plan_restore(backup, target_parent, target_leaf)
            .await?;
        let plan = self
            .store
            .create_backup_restore_plan(access.principal().clone(), draft, now_ms()?)
            .await?;
        Ok(BackupRestorePlanOutcome {
            plan_id: plan.draft.plan_id,
            project_id: plan.draft.project_id,
            backup_id: plan.draft.backup_id,
            target: plan.draft.target,
            archive_sha256: plan.draft.archive_sha256,
            packages_require_resolve: plan.draft.exclude_vpm_packages,
            excluded_packages: plan.draft.excluded_packages,
            plan_fingerprint: plan.draft.plan_fingerprint,
        })
    }

    pub async fn apply_restore(
        &self,
        access: &AccessContext,
        plan_id: PlanId,
        key: IdempotencyKey,
    ) -> Result<BackupRestoreApplyOutcome, M5BackupError> {
        require(access, Permission::BackupsRead)?;
        require(access, Permission::BackupsManage)?;
        require(access, Permission::ProjectsCreate)?;
        if access.principal() != &PrincipalId::local_owner() {
            return Err(error(M5BackupErrorCode::PermissionDenied));
        }
        let outcome = self
            .store
            .accept_backup_restore(access.principal().clone(), plan_id, key, now_ms()?)
            .await?;
        if outcome.schedule {
            self.schedule_restore(outcome.operation_id);
        }
        Ok(outcome)
    }

    fn schedule(&self, operation_id: OperationId) {
        let application = self.clone();
        tokio::spawn(async move {
            let _ = application.run(operation_id).await;
        });
    }

    fn schedule_restore(&self, operation_id: OperationId) {
        let application = self.clone();
        tokio::spawn(async move {
            let _ = application.run_restore(operation_id).await;
        });
    }

    async fn run_restore(&self, operation_id: OperationId) -> Result<(), M5BackupError> {
        let result = self.run_restore_inner(operation_id).await;
        if let Err(source) = result {
            self.store
                .fail_backup_restore(
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

    async fn run_restore_inner(&self, operation_id: OperationId) -> Result<(), M5BackupError> {
        let operation = self
            .store
            .begin_backup_restore(operation_id, now_ms()?)
            .await?;
        if operation.phase == BackupRestorePhase::StateCommitted {
            self.adapter
                .validate_published_restore(operation_id, operation.plan.clone())
                .await?;
            self.adapter
                .finalize_restore(operation_id, operation.plan)
                .await?;
            self.store
                .finish_backup_restore_success(operation_id, now_ms()?)
                .await?;
            return Ok(());
        }
        let resource = self.adapter.restore_resource(&operation.plan)?;
        let prepared = self.adapter.prepare_restore(operation.plan.clone()).await?;
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
        let _guard = self.locks.acquire(vec![resource]).await;
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
                let staged = self.adapter.stage_restore(operation_id, prepared).await?;
                self.adapter.discard_staged_restore(staged).await?;
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
            let staged = self.adapter.stage_restore(operation_id, prepared).await?;
            self.store
                .record_backup_restore_checkpoint(
                    operation_id,
                    BackupRestorePhase::StagingComplete,
                    None,
                    now_ms()?,
                )
                .await?;
            restore_kill_gate("staging_complete");
            if cancelled(&self.store, operation_id).await? {
                self.adapter.discard_staged_restore(staged).await?;
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
                .validate_published_restore(operation_id, operation.plan.clone())
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
            .finalize_restore(operation_id, operation.plan)
            .await?;
        self.store
            .finish_backup_restore_success(operation_id, now_ms()?)
            .await?;
        Ok(())
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
        M5BackupErrorCode::BackupRestorePlanNotFound => "backup_restore_plan_not_found",
        M5BackupErrorCode::BackupRestorePlanStale => "backup_restore_plan_stale",
        M5BackupErrorCode::BackupRestoreTargetExists => "backup_target_exists",
        M5BackupErrorCode::BackupRestoreTargetUnsafe => "backup_target_invalid",
        M5BackupErrorCode::BackupRestoreRecoveryRequired => "backup_restore_recovery_required",
        M5BackupErrorCode::RecoveryRequired => "backup_unavailable",
        M5BackupErrorCode::StoreUnavailable => "store_unavailable",
        M5BackupErrorCode::Internal => "internal_error",
    }
}

const fn error(code: M5BackupErrorCode) -> M5BackupError {
    M5BackupError::new(code)
}
