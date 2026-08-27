//! M7 durable Project Copy use case.

use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::{
    AccessContext, IdempotencyKey, M3RegistryStore, M5BackupWriterGate, OperationId, Permission,
    PlanId, PrincipalId, ProjectId, ProjectObservation, ProjectRecord, ResourceKey,
    ResourceLockCoordinator, Revision, UnityWriterState, UnityWriterStateKind,
};

pub const PROJECT_COPY_PLAN_EXPIRY_MS: u64 = 900_000;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCopyPlanDraft {
    pub plan_id: PlanId,
    pub source_project: ProjectRecord,
    pub source_root_identity: Vec<u8>,
    pub target_parent_path: String,
    pub target_parent_identity: Vec<u8>,
    pub target_parent_identity_sha256: [u8; 32],
    pub target_leaf: String,
    pub target_project_id: ProjectId,
    pub writer_evidence: UnityWriterState,
    pub profile_version: u32,
    pub plan_fingerprint: [u8; 32],
    pub plan_json: String,
    pub plan_idempotency_key: IdempotencyKey,
    pub created_at_ms: u64,
    pub expires_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCopyPlanRecord {
    #[serde(flatten)]
    pub draft: ProjectCopyPlanDraft,
    pub owner: PrincipalId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCopyPlanOutcome {
    pub plan: ProjectCopyPlanRecord,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCopyApplyOutcome {
    pub operation_id: OperationId,
    pub target_project_id: ProjectId,
    pub replayed: bool,
    #[serde(skip)]
    pub schedule: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectCopyPhase {
    Accepted,
    InventoryReady,
    Staging,
    StagingComplete,
    PublishIntent,
    TargetPublished,
    ProjectRegistryCommitIntent,
    StateCommitted,
    CleanupComplete,
    RecoveryRequired,
}

impl ProjectCopyPhase {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::InventoryReady => "inventory_ready",
            Self::Staging => "staging",
            Self::StagingComplete => "staging_complete",
            Self::PublishIntent => "publish_intent",
            Self::TargetPublished => "target_published",
            Self::ProjectRegistryCommitIntent => "project_registry_commit_intent",
            Self::StateCommitted => "state_committed",
            Self::CleanupComplete => "cleanup_complete",
            Self::RecoveryRequired => "recovery_required",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCopyInventoryEvidence {
    pub private_locator: String,
    pub sha256: [u8; 32],
    pub byte_length: u64,
    pub owner_marker: String,
    pub entry_count: u64,
    pub total_regular_file_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectCopyOperationRecord {
    pub plan: ProjectCopyPlanRecord,
    pub phase: ProjectCopyPhase,
    pub inventory: Option<ProjectCopyInventoryEvidence>,
    pub published: Option<PublishedProjectCopy>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PublishedProjectCopy {
    pub observation: ProjectObservation,
    pub target_identity: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum M7CopyErrorCode {
    InvalidInput,
    PermissionDenied,
    ProjectNotRegistered,
    RevisionConflict,
    UnityProjectRunning,
    IdempotencyConflict,
    ProjectCopyPlanNotFound,
    ProjectCopyPlanStale,
    ProjectCopyTargetExists,
    ProjectCopyTargetUnsafe,
    ProjectCopySourceUnsafe,
    ProjectCopySourceChanged,
    ProjectCopyLimitExceeded,
    ProjectCopyRecoveryRequired,
    StoreUnavailable,
    Internal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct M7CopyError(M7CopyErrorCode);

impl M7CopyError {
    #[must_use]
    pub const fn new(code: M7CopyErrorCode) -> Self {
        Self(code)
    }

    #[must_use]
    pub const fn code(self) -> M7CopyErrorCode {
        self.0
    }
}

impl std::fmt::Display for M7CopyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Project Copy request failed")
    }
}

impl std::error::Error for M7CopyError {}

pub trait M7CopyStore: Clone + Send + Sync + 'static {
    fn create_project_copy_plan(
        &self,
        owner: PrincipalId,
        draft: ProjectCopyPlanDraft,
    ) -> impl Future<Output = Result<ProjectCopyPlanOutcome, M7CopyError>> + Send;
    fn get_project_copy_plan(
        &self,
        owner: PrincipalId,
        plan_id: PlanId,
    ) -> impl Future<Output = Result<ProjectCopyPlanRecord, M7CopyError>> + Send;
    fn replay_project_copy_apply(
        &self,
        owner: PrincipalId,
        plan_id: PlanId,
        expected_revision: Revision,
        key: IdempotencyKey,
    ) -> impl Future<Output = Result<Option<ProjectCopyApplyOutcome>, M7CopyError>> + Send;
    fn accept_project_copy(
        &self,
        owner: PrincipalId,
        plan_id: PlanId,
        expected_revision: Revision,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<ProjectCopyApplyOutcome, M7CopyError>> + Send;
    fn begin_project_copy(
        &self,
        operation_id: OperationId,
        now_ms: u64,
    ) -> impl Future<Output = Result<ProjectCopyOperationRecord, M7CopyError>> + Send;
    fn record_project_copy_checkpoint(
        &self,
        operation_id: OperationId,
        phase: ProjectCopyPhase,
        inventory: Option<ProjectCopyInventoryEvidence>,
        published: Option<PublishedProjectCopy>,
        now_ms: u64,
    ) -> impl Future<Output = Result<(), M7CopyError>> + Send;
    fn complete_project_copy(
        &self,
        operation_id: OperationId,
        published: PublishedProjectCopy,
        now_ms: u64,
    ) -> impl Future<Output = Result<(), M7CopyError>> + Send;
    fn finish_project_copy_success(
        &self,
        operation_id: OperationId,
        now_ms: u64,
    ) -> impl Future<Output = Result<(), M7CopyError>> + Send;
    fn fail_project_copy(
        &self,
        operation_id: OperationId,
        code: String,
        diagnostic_id: String,
        now_ms: u64,
    ) -> impl Future<Output = Result<(), M7CopyError>> + Send;
    fn recover_project_copy_operations(
        &self,
        now_ms: u64,
    ) -> impl Future<Output = Result<Vec<OperationId>, M7CopyError>> + Send;
    fn project_copy_cancel_requested(
        &self,
        operation_id: OperationId,
    ) -> impl Future<Output = Result<bool, M7CopyError>> + Send;
    fn finish_project_copy_cancelled(
        &self,
        operation_id: OperationId,
        now_ms: u64,
    ) -> impl Future<Output = Result<(), M7CopyError>> + Send;
}

pub trait M7CopyAdapter: Clone + Send + Sync + 'static {
    fn plan(
        &self,
        source: ProjectRecord,
        target_parent: PathBuf,
        target_leaf: String,
        writer_evidence: UnityWriterState,
        plan_key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<ProjectCopyPlanDraft, M7CopyError>> + Send;
    fn resources(&self, plan: &ProjectCopyPlanRecord) -> Vec<ResourceKey>;
    fn revalidate_plan(
        &self,
        plan: &ProjectCopyPlanRecord,
    ) -> impl Future<Output = Result<(), M7CopyError>> + Send;
    fn inventory(
        &self,
        operation_id: OperationId,
        plan: ProjectCopyPlanRecord,
    ) -> impl Future<Output = Result<ProjectCopyInventoryEvidence, M7CopyError>> + Send;
    fn stage(
        &self,
        operation_id: OperationId,
        plan: ProjectCopyPlanRecord,
        inventory: ProjectCopyInventoryEvidence,
    ) -> impl Future<Output = Result<ProjectCopyInventoryEvidence, M7CopyError>> + Send;
    fn verify_source(
        &self,
        plan: ProjectCopyPlanRecord,
        inventory: ProjectCopyInventoryEvidence,
    ) -> impl Future<Output = Result<(), M7CopyError>> + Send;
    fn publish(
        &self,
        operation_id: OperationId,
        plan: ProjectCopyPlanRecord,
        inventory: ProjectCopyInventoryEvidence,
    ) -> impl Future<Output = Result<PublishedProjectCopy, M7CopyError>> + Send;
    fn validate_published(
        &self,
        operation_id: OperationId,
        plan: ProjectCopyPlanRecord,
        inventory: ProjectCopyInventoryEvidence,
        expected: Option<PublishedProjectCopy>,
    ) -> impl Future<Output = Result<PublishedProjectCopy, M7CopyError>> + Send;
    fn cleanup(
        &self,
        operation_id: OperationId,
        plan: ProjectCopyPlanRecord,
    ) -> impl Future<Output = Result<(), M7CopyError>> + Send;
    fn discard(
        &self,
        operation_id: OperationId,
        plan: ProjectCopyPlanRecord,
    ) -> impl Future<Output = Result<(), M7CopyError>> + Send;
}

#[derive(Clone)]
pub struct M7CopyApplication<S, A, W> {
    store: S,
    adapter: A,
    writer: W,
    locks: Arc<ResourceLockCoordinator>,
}

impl<S, A, W> M7CopyApplication<S, A, W>
where
    S: M7CopyStore + M3RegistryStore,
    A: M7CopyAdapter,
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

    pub async fn recover(&self) -> Result<(), M7CopyError> {
        for operation_id in self
            .store
            .recover_project_copy_operations(now_ms()?)
            .await?
        {
            self.schedule(operation_id);
        }
        Ok(())
    }

    pub async fn plan_copy(
        &self,
        access: &AccessContext,
        source_project_id: ProjectId,
        expected_revision: Revision,
        target_parent: PathBuf,
        target_leaf: String,
        key: IdempotencyKey,
    ) -> Result<ProjectCopyPlanOutcome, M7CopyError> {
        require(access, Permission::ProjectsRead)?;
        require(access, Permission::ProjectsCreate)?;
        local_owner(access)?;
        let source = self
            .store
            .get_project(access.principal().clone(), source_project_id)
            .await
            .map_err(map_m3)?;
        if source.revision != expected_revision {
            return Err(error(M7CopyErrorCode::RevisionConflict));
        }
        let writer = self
            .writer
            .observe_backup_source(access, source_project_id)
            .await
            .map_err(|_| error(M7CopyErrorCode::Internal))?;
        if writer.state == UnityWriterStateKind::RunningConfirmed {
            return Err(error(M7CopyErrorCode::UnityProjectRunning));
        }
        let draft = self
            .adapter
            .plan(source, target_parent, target_leaf, writer, key, now_ms()?)
            .await?;
        self.store
            .create_project_copy_plan(access.principal().clone(), draft)
            .await
    }

    pub async fn apply_copy(
        &self,
        access: &AccessContext,
        plan_id: PlanId,
        expected_revision: Revision,
        key: IdempotencyKey,
    ) -> Result<ProjectCopyApplyOutcome, M7CopyError> {
        require(access, Permission::ProjectsRead)?;
        require(access, Permission::ProjectsCreate)?;
        local_owner(access)?;
        if let Some(outcome) = self
            .store
            .replay_project_copy_apply(
                access.principal().clone(),
                plan_id,
                expected_revision,
                key.clone(),
            )
            .await?
        {
            return Ok(outcome);
        }
        let plan = self
            .store
            .get_project_copy_plan(access.principal().clone(), plan_id)
            .await?;
        let _guard = self.locks.acquire(self.adapter.resources(&plan)).await;
        let current = self
            .store
            .get_project(
                access.principal().clone(),
                plan.draft.source_project.project_id,
            )
            .await
            .map_err(map_m3)?;
        if current.revision != expected_revision
            || current.revision != plan.draft.source_project.revision
        {
            return Err(error(M7CopyErrorCode::RevisionConflict));
        }
        let writer = self
            .writer
            .observe_backup_source(access, current.project_id)
            .await
            .map_err(|_| error(M7CopyErrorCode::Internal))?;
        if writer.state == UnityWriterStateKind::RunningConfirmed {
            return Err(error(M7CopyErrorCode::UnityProjectRunning));
        }
        self.adapter.revalidate_plan(&plan).await?;
        let outcome = self
            .store
            .accept_project_copy(
                access.principal().clone(),
                plan_id,
                expected_revision,
                key,
                now_ms()?,
            )
            .await?;
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

    async fn run(&self, operation_id: OperationId) -> Result<(), M7CopyError> {
        let result = self.run_inner(operation_id).await;
        if let Err(source) = result {
            self.store
                .fail_project_copy(
                    operation_id,
                    project_copy_error_name(source.code()).to_owned(),
                    OperationId::new().to_string(),
                    now_ms()?,
                )
                .await?;
            return Err(source);
        }
        Ok(())
    }

    async fn run_inner(&self, operation_id: OperationId) -> Result<(), M7CopyError> {
        let mut operation = self
            .store
            .begin_project_copy(operation_id, now_ms()?)
            .await?;
        let _guard = self
            .locks
            .acquire(self.adapter.resources(&operation.plan))
            .await;
        if operation.phase == ProjectCopyPhase::CleanupComplete {
            return Ok(());
        }
        let mut inventory = match operation.inventory.clone() {
            Some(value) => value,
            None => {
                self.adapter.revalidate_plan(&operation.plan).await?;
                let value = self
                    .adapter
                    .inventory(operation_id, operation.plan.clone())
                    .await?;
                self.store
                    .record_project_copy_checkpoint(
                        operation_id,
                        ProjectCopyPhase::InventoryReady,
                        Some(value.clone()),
                        None,
                        now_ms()?,
                    )
                    .await?;
                project_copy_kill_gate(ProjectCopyPhase::InventoryReady.as_str());
                value
            }
        };
        operation.inventory = Some(inventory.clone());
        if !phase_at_or_after_intent(operation.phase)
            && self
                .store
                .project_copy_cancel_requested(operation_id)
                .await?
        {
            self.adapter.discard(operation_id, operation.plan).await?;
            return self
                .store
                .finish_project_copy_cancelled(operation_id, now_ms()?)
                .await;
        }
        if matches!(
            operation.phase,
            ProjectCopyPhase::Accepted
                | ProjectCopyPhase::InventoryReady
                | ProjectCopyPhase::Staging
        ) {
            self.store
                .record_project_copy_checkpoint(
                    operation_id,
                    ProjectCopyPhase::Staging,
                    Some(inventory.clone()),
                    None,
                    now_ms()?,
                )
                .await?;
            project_copy_kill_gate(ProjectCopyPhase::Staging.as_str());
            project_copy_pause_gate(ProjectCopyPhase::Staging.as_str());
            inventory = self
                .adapter
                .stage(operation_id, operation.plan.clone(), inventory)
                .await?;
            self.store
                .record_project_copy_checkpoint(
                    operation_id,
                    ProjectCopyPhase::StagingComplete,
                    Some(inventory.clone()),
                    None,
                    now_ms()?,
                )
                .await?;
            project_copy_kill_gate(ProjectCopyPhase::StagingComplete.as_str());
        }
        if !phase_at_or_after_intent(operation.phase)
            && self
                .store
                .project_copy_cancel_requested(operation_id)
                .await?
        {
            self.adapter.discard(operation_id, operation.plan).await?;
            return self
                .store
                .finish_project_copy_cancelled(operation_id, now_ms()?)
                .await;
        }
        if matches!(
            operation.phase,
            ProjectCopyPhase::Accepted
                | ProjectCopyPhase::InventoryReady
                | ProjectCopyPhase::Staging
                | ProjectCopyPhase::StagingComplete
        ) {
            self.adapter
                .verify_source(operation.plan.clone(), inventory.clone())
                .await?;
            self.store
                .record_project_copy_checkpoint(
                    operation_id,
                    ProjectCopyPhase::PublishIntent,
                    Some(inventory.clone()),
                    None,
                    now_ms()?,
                )
                .await?;
            project_copy_kill_gate(ProjectCopyPhase::PublishIntent.as_str());
            project_copy_pause_gate(ProjectCopyPhase::PublishIntent.as_str());
        }
        let published = if matches!(
            operation.phase,
            ProjectCopyPhase::TargetPublished
                | ProjectCopyPhase::ProjectRegistryCommitIntent
                | ProjectCopyPhase::StateCommitted
                | ProjectCopyPhase::RecoveryRequired
        ) {
            self.adapter
                .validate_published(
                    operation_id,
                    operation.plan.clone(),
                    inventory.clone(),
                    operation.published.clone(),
                )
                .await?
        } else {
            let value = self
                .adapter
                .publish(operation_id, operation.plan.clone(), inventory.clone())
                .await?;
            self.store
                .record_project_copy_checkpoint(
                    operation_id,
                    ProjectCopyPhase::TargetPublished,
                    Some(inventory.clone()),
                    Some(value.clone()),
                    now_ms()?,
                )
                .await?;
            project_copy_kill_gate(ProjectCopyPhase::TargetPublished.as_str());
            value
        };
        if !matches!(operation.phase, ProjectCopyPhase::StateCommitted) {
            self.store
                .record_project_copy_checkpoint(
                    operation_id,
                    ProjectCopyPhase::ProjectRegistryCommitIntent,
                    Some(inventory),
                    Some(published.clone()),
                    now_ms()?,
                )
                .await?;
            project_copy_kill_gate(ProjectCopyPhase::ProjectRegistryCommitIntent.as_str());
            self.store
                .complete_project_copy(operation_id, published, now_ms()?)
                .await?;
            project_copy_kill_gate(ProjectCopyPhase::StateCommitted.as_str());
        }
        self.adapter.cleanup(operation_id, operation.plan).await?;
        self.store
            .finish_project_copy_success(operation_id, now_ms()?)
            .await?;
        project_copy_kill_gate(ProjectCopyPhase::CleanupComplete.as_str());
        Ok(())
    }
}

fn local_owner(access: &AccessContext) -> Result<(), M7CopyError> {
    (access.principal() == &PrincipalId::local_owner())
        .then_some(())
        .ok_or_else(|| error(M7CopyErrorCode::PermissionDenied))
}

fn require(access: &AccessContext, permission: Permission) -> Result<(), M7CopyError> {
    access
        .require(permission)
        .map_err(|_| error(M7CopyErrorCode::PermissionDenied))
}

fn map_m3(source: crate::M3Error) -> M7CopyError {
    match source.code() {
        crate::M3ErrorCode::ProjectNotRegistered => error(M7CopyErrorCode::ProjectNotRegistered),
        crate::M3ErrorCode::StoreUnavailable => error(M7CopyErrorCode::StoreUnavailable),
        _ => error(M7CopyErrorCode::Internal),
    }
}

fn error(code: M7CopyErrorCode) -> M7CopyError {
    M7CopyError::new(code)
}

fn phase_at_or_after_intent(phase: ProjectCopyPhase) -> bool {
    matches!(
        phase,
        ProjectCopyPhase::PublishIntent
            | ProjectCopyPhase::TargetPublished
            | ProjectCopyPhase::ProjectRegistryCommitIntent
            | ProjectCopyPhase::StateCommitted
            | ProjectCopyPhase::CleanupComplete
            | ProjectCopyPhase::RecoveryRequired
    )
}

pub fn project_copy_error_name(code: M7CopyErrorCode) -> &'static str {
    match code {
        M7CopyErrorCode::InvalidInput => "invalid_request",
        M7CopyErrorCode::PermissionDenied => "permission_denied",
        M7CopyErrorCode::ProjectNotRegistered => "project_not_registered",
        M7CopyErrorCode::RevisionConflict => "revision_conflict",
        M7CopyErrorCode::UnityProjectRunning => "unity_project_running",
        M7CopyErrorCode::IdempotencyConflict => "idempotency_conflict",
        M7CopyErrorCode::ProjectCopyPlanNotFound => "project_copy_plan_not_found",
        M7CopyErrorCode::ProjectCopyPlanStale => "project_copy_plan_stale",
        M7CopyErrorCode::ProjectCopyTargetExists => "project_copy_target_exists",
        M7CopyErrorCode::ProjectCopyTargetUnsafe => "project_copy_target_unsafe",
        M7CopyErrorCode::ProjectCopySourceUnsafe => "project_copy_source_unsafe",
        M7CopyErrorCode::ProjectCopySourceChanged => "project_copy_source_changed",
        M7CopyErrorCode::ProjectCopyLimitExceeded => "project_copy_limit_exceeded",
        M7CopyErrorCode::ProjectCopyRecoveryRequired => "project_copy_recovery_required",
        M7CopyErrorCode::StoreUnavailable => "store_unavailable",
        M7CopyErrorCode::Internal => "internal_error",
    }
}

fn now_ms() -> Result<u64, M7CopyError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| u64::try_from(value.as_millis()).unwrap_or(u64::MAX))
        .map_err(|_| error(M7CopyErrorCode::Internal))
}

#[cfg(feature = "test-kill-gates")]
fn project_copy_kill_gate(phase: &str) {
    if std::env::var("ALCOMD_TEST_PROJECT_COPY_KILL_GATE").as_deref() == Ok(phase) {
        let signal = std::env::var_os("ALCOMD_TEST_PROJECT_COPY_KILL_SIGNAL")
            .expect("Project Copy kill gate signal path");
        std::fs::write(signal, phase).expect("write Project Copy kill gate signal");
        loop {
            std::thread::park();
        }
    }
}

#[cfg(not(feature = "test-kill-gates"))]
fn project_copy_kill_gate(_: &str) {}

#[cfg(feature = "test-kill-gates")]
fn project_copy_pause_gate(phase: &str) {
    if std::env::var("ALCOMD_TEST_PROJECT_COPY_PAUSE_GATE").as_deref() == Ok(phase) {
        let signal = std::env::var_os("ALCOMD_TEST_PROJECT_COPY_PAUSE_SIGNAL")
            .expect("Project Copy pause gate signal path");
        let release = std::env::var_os("ALCOMD_TEST_PROJECT_COPY_PAUSE_RELEASE")
            .expect("Project Copy pause gate release path");
        std::fs::write(signal, phase).expect("write Project Copy pause gate signal");
        while !std::path::Path::new(&release).exists() {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}

#[cfg(not(feature = "test-kill-gates"))]
fn project_copy_pause_gate(_: &str) {}
