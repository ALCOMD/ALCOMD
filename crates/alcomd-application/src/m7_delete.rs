//! M7 durable Project Directory Delete use case.

use std::future::Future;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::{
    AccessContext, IdempotencyKey, M3RegistryStore, M5BackupWriterGate, OperationId, Permission,
    PlanId, PrincipalId, ProjectId, ProjectRecord, ResourceKey, ResourceLockCoordinator, Revision,
    UnityWriterState, UnityWriterStateKind,
};

pub const PROJECT_DELETE_PLAN_EXPIRY_MS: u64 = 900_000;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDeletePlanDraft {
    pub plan_id: PlanId,
    pub project: ProjectRecord,
    pub root_identity: Vec<u8>,
    pub canonical_parent_path: String,
    pub parent_identity: Vec<u8>,
    pub parent_identity_sha256: [u8; 32],
    pub normalized_leaf: String,
    pub project_marker_sha256: [u8; 32],
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
pub struct ProjectDeletePlanRecord {
    #[serde(flatten)]
    pub draft: ProjectDeletePlanDraft,
    pub owner: PrincipalId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDeletePlanOutcome {
    pub plan: ProjectDeletePlanRecord,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDeleteApplyOutcome {
    pub operation_id: OperationId,
    pub project_id: ProjectId,
    pub replayed: bool,
    #[serde(skip)]
    pub schedule: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectDeletePhase {
    Accepted,
    PreflightComplete,
    QuarantineIntent,
    RootQuarantined,
    RegistryCommitIntent,
    StateCommitted,
    Deleting,
    CleanupComplete,
    RecoveryRequired,
}

impl ProjectDeletePhase {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::PreflightComplete => "preflight_complete",
            Self::QuarantineIntent => "quarantine_intent",
            Self::RootQuarantined => "root_quarantined",
            Self::RegistryCommitIntent => "registry_commit_intent",
            Self::StateCommitted => "state_committed",
            Self::Deleting => "deleting",
            Self::CleanupComplete => "cleanup_complete",
            Self::RecoveryRequired => "recovery_required",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDeleteFilesystemEvidence {
    pub quarantine_locator: String,
    pub quarantine_identity: Option<Vec<u8>>,
    pub entry_count: Option<u64>,
    pub safe_evidence: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectDeleteOperationRecord {
    pub plan: ProjectDeletePlanRecord,
    pub phase: ProjectDeletePhase,
    pub evidence: ProjectDeleteFilesystemEvidence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum M7DeleteErrorCode {
    InvalidInput,
    PermissionDenied,
    ProjectNotRegistered,
    RevisionConflict,
    UnityProjectRunning,
    IdempotencyConflict,
    ProjectDeletePlanNotFound,
    ProjectDeletePlanStale,
    ProjectDeleteSourceMissing,
    ProjectDeleteSourceUnsafe,
    ProjectDeleteSourceChanged,
    ProjectDeleteRecoveryRequired,
    StoreUnavailable,
    Internal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct M7DeleteError(M7DeleteErrorCode);

impl M7DeleteError {
    #[must_use]
    pub const fn new(code: M7DeleteErrorCode) -> Self {
        Self(code)
    }

    #[must_use]
    pub const fn code(self) -> M7DeleteErrorCode {
        self.0
    }
}

impl std::fmt::Display for M7DeleteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Project Directory Delete request failed")
    }
}

impl std::error::Error for M7DeleteError {}

pub trait M7DeleteStore: Clone + Send + Sync + 'static {
    fn create_project_delete_plan(
        &self,
        owner: PrincipalId,
        draft: ProjectDeletePlanDraft,
    ) -> impl Future<Output = Result<ProjectDeletePlanOutcome, M7DeleteError>> + Send;
    fn get_project_delete_plan(
        &self,
        owner: PrincipalId,
        plan_id: PlanId,
    ) -> impl Future<Output = Result<ProjectDeletePlanRecord, M7DeleteError>> + Send;
    fn replay_project_delete_apply(
        &self,
        owner: PrincipalId,
        plan_id: PlanId,
        expected_revision: Revision,
        key: IdempotencyKey,
    ) -> impl Future<Output = Result<Option<ProjectDeleteApplyOutcome>, M7DeleteError>> + Send;
    fn accept_project_delete(
        &self,
        owner: PrincipalId,
        plan_id: PlanId,
        expected_revision: Revision,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<ProjectDeleteApplyOutcome, M7DeleteError>> + Send;
    fn begin_project_delete(
        &self,
        operation_id: OperationId,
        now_ms: u64,
    ) -> impl Future<Output = Result<ProjectDeleteOperationRecord, M7DeleteError>> + Send;
    fn record_project_delete_checkpoint(
        &self,
        operation_id: OperationId,
        phase: ProjectDeletePhase,
        evidence: ProjectDeleteFilesystemEvidence,
        now_ms: u64,
    ) -> impl Future<Output = Result<(), M7DeleteError>> + Send;
    fn commit_project_delete(
        &self,
        operation_id: OperationId,
        now_ms: u64,
    ) -> impl Future<Output = Result<(), M7DeleteError>> + Send;
    fn finish_project_delete_success(
        &self,
        operation_id: OperationId,
        now_ms: u64,
    ) -> impl Future<Output = Result<(), M7DeleteError>> + Send;
    fn fail_project_delete(
        &self,
        operation_id: OperationId,
        code: String,
        diagnostic_id: String,
        now_ms: u64,
    ) -> impl Future<Output = Result<(), M7DeleteError>> + Send;
    fn recover_project_delete_operations(
        &self,
        now_ms: u64,
    ) -> impl Future<Output = Result<Vec<OperationId>, M7DeleteError>> + Send;
    fn project_delete_cancel_requested(
        &self,
        operation_id: OperationId,
    ) -> impl Future<Output = Result<bool, M7DeleteError>> + Send;
    fn finish_project_delete_cancelled(
        &self,
        operation_id: OperationId,
        now_ms: u64,
    ) -> impl Future<Output = Result<(), M7DeleteError>> + Send;
}

pub trait M7DeleteAdapter: Clone + Send + Sync + 'static {
    fn plan(
        &self,
        project: ProjectRecord,
        writer_evidence: UnityWriterState,
        plan_key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<ProjectDeletePlanDraft, M7DeleteError>> + Send;
    fn resources(&self, plan: &ProjectDeletePlanRecord) -> Vec<ResourceKey>;
    fn revalidate_plan(
        &self,
        plan: &ProjectDeletePlanRecord,
    ) -> impl Future<Output = Result<(), M7DeleteError>> + Send;
    fn preflight(
        &self,
        operation_id: OperationId,
        plan: ProjectDeletePlanRecord,
    ) -> impl Future<Output = Result<ProjectDeleteFilesystemEvidence, M7DeleteError>> + Send;
    fn quarantine(
        &self,
        operation_id: OperationId,
        plan: ProjectDeletePlanRecord,
        evidence: ProjectDeleteFilesystemEvidence,
    ) -> impl Future<Output = Result<ProjectDeleteFilesystemEvidence, M7DeleteError>> + Send;
    fn validate_quarantine(
        &self,
        operation_id: OperationId,
        plan: ProjectDeletePlanRecord,
        evidence: ProjectDeleteFilesystemEvidence,
    ) -> impl Future<Output = Result<ProjectDeleteFilesystemEvidence, M7DeleteError>> + Send;
    fn cleanup(
        &self,
        operation_id: OperationId,
        plan: ProjectDeletePlanRecord,
        evidence: ProjectDeleteFilesystemEvidence,
    ) -> impl Future<Output = Result<ProjectDeleteFilesystemEvidence, M7DeleteError>> + Send;
    fn discard(
        &self,
        operation_id: OperationId,
        plan: ProjectDeletePlanRecord,
        evidence: ProjectDeleteFilesystemEvidence,
    ) -> impl Future<Output = Result<(), M7DeleteError>> + Send;
}

#[derive(Clone)]
pub struct M7DeleteApplication<S, A, W> {
    store: S,
    adapter: A,
    writer: W,
    locks: Arc<ResourceLockCoordinator>,
}

impl<S, A, W> M7DeleteApplication<S, A, W>
where
    S: M7DeleteStore + M3RegistryStore,
    A: M7DeleteAdapter,
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

    pub async fn recover(&self) -> Result<(), M7DeleteError> {
        for operation_id in self
            .store
            .recover_project_delete_operations(now_ms()?)
            .await?
        {
            self.schedule(operation_id);
        }
        Ok(())
    }

    pub async fn plan_delete(
        &self,
        access: &AccessContext,
        project_id: ProjectId,
        expected_revision: Revision,
        key: IdempotencyKey,
    ) -> Result<ProjectDeletePlanOutcome, M7DeleteError> {
        require(access, Permission::ProjectsRead)?;
        require(access, Permission::ProjectsDelete)?;
        local_owner(access)?;
        let project = self
            .store
            .get_project(access.principal().clone(), project_id)
            .await
            .map_err(map_m3)?;
        if project.revision != expected_revision {
            return Err(error(M7DeleteErrorCode::RevisionConflict));
        }
        let writer = match project_delete_test_writer(project_id) {
            Some(writer) => writer,
            None => self
                .writer
                .observe_backup_source(access, project_id)
                .await
                .map_err(|_| error(M7DeleteErrorCode::Internal))?,
        };
        require_safe_writer(writer.state)?;
        let draft = self.adapter.plan(project, writer, key, now_ms()?).await?;
        self.store
            .create_project_delete_plan(access.principal().clone(), draft)
            .await
    }

    pub async fn apply_delete(
        &self,
        access: &AccessContext,
        plan_id: PlanId,
        expected_revision: Revision,
        key: IdempotencyKey,
    ) -> Result<ProjectDeleteApplyOutcome, M7DeleteError> {
        require(access, Permission::ProjectsDelete)?;
        local_owner(access)?;
        if let Some(outcome) = self
            .store
            .replay_project_delete_apply(
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
            .get_project_delete_plan(access.principal().clone(), plan_id)
            .await?;
        let _guard = self.locks.acquire(self.adapter.resources(&plan)).await;
        let current = self
            .store
            .get_project(access.principal().clone(), plan.draft.project.project_id)
            .await
            .map_err(map_m3)?;
        if current.revision != expected_revision || current.revision != plan.draft.project.revision
        {
            return Err(error(M7DeleteErrorCode::RevisionConflict));
        }
        let writer = match project_delete_test_writer(current.project_id) {
            Some(writer) => writer,
            None => self
                .writer
                .observe_backup_source(access, current.project_id)
                .await
                .map_err(|_| error(M7DeleteErrorCode::Internal))?,
        };
        require_safe_writer(writer.state)?;
        self.adapter.revalidate_plan(&plan).await?;
        let outcome = self
            .store
            .accept_project_delete(
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

    async fn run(&self, operation_id: OperationId) -> Result<(), M7DeleteError> {
        let result = self.run_inner(operation_id).await;
        if let Err(source) = result {
            self.store
                .fail_project_delete(
                    operation_id,
                    project_delete_error_name(source.code()).to_owned(),
                    OperationId::new().to_string(),
                    now_ms()?,
                )
                .await?;
            return Err(source);
        }
        Ok(())
    }

    async fn run_inner(&self, operation_id: OperationId) -> Result<(), M7DeleteError> {
        let operation = self
            .store
            .begin_project_delete(operation_id, now_ms()?)
            .await?;
        project_delete_kill_gate(ProjectDeletePhase::Accepted.as_str());
        project_delete_pause_gate(ProjectDeletePhase::Accepted.as_str()).await;
        let _guard = self
            .locks
            .acquire(self.adapter.resources(&operation.plan))
            .await;
        if operation.phase == ProjectDeletePhase::CleanupComplete {
            return self
                .store
                .finish_project_delete_success(operation_id, now_ms()?)
                .await;
        }
        let mut evidence = operation.evidence;
        if operation.phase == ProjectDeletePhase::Accepted {
            self.adapter.revalidate_plan(&operation.plan).await?;
            evidence = self
                .adapter
                .preflight(operation_id, operation.plan.clone())
                .await?;
            self.store
                .record_project_delete_checkpoint(
                    operation_id,
                    ProjectDeletePhase::PreflightComplete,
                    evidence.clone(),
                    now_ms()?,
                )
                .await?;
            project_delete_kill_gate(ProjectDeletePhase::PreflightComplete.as_str());
            project_delete_pause_gate(ProjectDeletePhase::PreflightComplete.as_str()).await;
        }
        if !phase_at_or_after_intent(operation.phase)
            && self
                .store
                .project_delete_cancel_requested(operation_id)
                .await?
        {
            self.adapter
                .discard(operation_id, operation.plan, evidence)
                .await?;
            return self
                .store
                .finish_project_delete_cancelled(operation_id, now_ms()?)
                .await;
        }
        if matches!(
            operation.phase,
            ProjectDeletePhase::Accepted | ProjectDeletePhase::PreflightComplete
        ) {
            self.adapter.revalidate_plan(&operation.plan).await?;
            if self
                .store
                .project_delete_cancel_requested(operation_id)
                .await?
            {
                self.adapter
                    .discard(operation_id, operation.plan, evidence)
                    .await?;
                return self
                    .store
                    .finish_project_delete_cancelled(operation_id, now_ms()?)
                    .await;
            }
            self.store
                .record_project_delete_checkpoint(
                    operation_id,
                    ProjectDeletePhase::QuarantineIntent,
                    evidence.clone(),
                    now_ms()?,
                )
                .await?;
            project_delete_kill_gate(ProjectDeletePhase::QuarantineIntent.as_str());
            project_delete_pause_gate(ProjectDeletePhase::QuarantineIntent.as_str()).await;
            evidence = self
                .adapter
                .quarantine(operation_id, operation.plan.clone(), evidence)
                .await?;
            self.store
                .record_project_delete_checkpoint(
                    operation_id,
                    ProjectDeletePhase::RootQuarantined,
                    evidence.clone(),
                    now_ms()?,
                )
                .await?;
            project_delete_kill_gate(ProjectDeletePhase::RootQuarantined.as_str());
        } else {
            evidence = self
                .adapter
                .validate_quarantine(operation_id, operation.plan.clone(), evidence)
                .await?;
        }
        if !matches!(
            operation.phase,
            ProjectDeletePhase::StateCommitted
                | ProjectDeletePhase::Deleting
                | ProjectDeletePhase::CleanupComplete
        ) {
            self.store
                .record_project_delete_checkpoint(
                    operation_id,
                    ProjectDeletePhase::RegistryCommitIntent,
                    evidence.clone(),
                    now_ms()?,
                )
                .await?;
            project_delete_kill_gate(ProjectDeletePhase::RegistryCommitIntent.as_str());
            self.store
                .commit_project_delete(operation_id, now_ms()?)
                .await?;
            project_delete_kill_gate(ProjectDeletePhase::StateCommitted.as_str());
        }
        self.store
            .record_project_delete_checkpoint(
                operation_id,
                ProjectDeletePhase::Deleting,
                evidence.clone(),
                now_ms()?,
            )
            .await?;
        project_delete_kill_gate(ProjectDeletePhase::Deleting.as_str());
        evidence = self
            .adapter
            .cleanup(operation_id, operation.plan.clone(), evidence)
            .await?;
        self.store
            .record_project_delete_checkpoint(
                operation_id,
                ProjectDeletePhase::CleanupComplete,
                evidence,
                now_ms()?,
            )
            .await?;
        project_delete_kill_gate(ProjectDeletePhase::CleanupComplete.as_str());
        self.store
            .finish_project_delete_success(operation_id, now_ms()?)
            .await?;
        Ok(())
    }
}

fn require_safe_writer(state: UnityWriterStateKind) -> Result<(), M7DeleteError> {
    match state {
        UnityWriterStateKind::NotObserved => Ok(()),
        UnityWriterStateKind::RunningConfirmed => {
            Err(error(M7DeleteErrorCode::UnityProjectRunning))
        }
        UnityWriterStateKind::RunningSuspected | UnityWriterStateKind::Unknown => {
            Err(error(M7DeleteErrorCode::ProjectDeleteSourceUnsafe))
        }
    }
}

#[cfg(feature = "test-kill-gates")]
fn project_delete_test_writer(project_id: ProjectId) -> Option<UnityWriterState> {
    (std::env::var("ALCOMD_TEST_PROJECT_DELETE_WRITER_STATE").as_deref() == Ok("not_observed"))
        .then_some(UnityWriterState {
            project_id,
            state: UnityWriterStateKind::NotObserved,
            evidence: Vec::new(),
            checked_at_ms: 1,
        })
}

#[cfg(not(feature = "test-kill-gates"))]
fn project_delete_test_writer(_: ProjectId) -> Option<UnityWriterState> {
    None
}

fn local_owner(access: &AccessContext) -> Result<(), M7DeleteError> {
    (access.principal() == &PrincipalId::local_owner())
        .then_some(())
        .ok_or_else(|| error(M7DeleteErrorCode::PermissionDenied))
}

fn require(access: &AccessContext, permission: Permission) -> Result<(), M7DeleteError> {
    access
        .require(permission)
        .map_err(|_| error(M7DeleteErrorCode::PermissionDenied))
}

fn map_m3(source: crate::M3Error) -> M7DeleteError {
    match source.code() {
        crate::M3ErrorCode::ProjectNotRegistered => error(M7DeleteErrorCode::ProjectNotRegistered),
        crate::M3ErrorCode::StoreUnavailable => error(M7DeleteErrorCode::StoreUnavailable),
        _ => error(M7DeleteErrorCode::Internal),
    }
}

fn error(code: M7DeleteErrorCode) -> M7DeleteError {
    M7DeleteError::new(code)
}

fn phase_at_or_after_intent(phase: ProjectDeletePhase) -> bool {
    matches!(
        phase,
        ProjectDeletePhase::QuarantineIntent
            | ProjectDeletePhase::RootQuarantined
            | ProjectDeletePhase::RegistryCommitIntent
            | ProjectDeletePhase::StateCommitted
            | ProjectDeletePhase::Deleting
            | ProjectDeletePhase::CleanupComplete
            | ProjectDeletePhase::RecoveryRequired
    )
}

pub fn project_delete_error_name(code: M7DeleteErrorCode) -> &'static str {
    match code {
        M7DeleteErrorCode::InvalidInput => "invalid_request",
        M7DeleteErrorCode::PermissionDenied => "permission_denied",
        M7DeleteErrorCode::ProjectNotRegistered => "project_not_registered",
        M7DeleteErrorCode::RevisionConflict => "revision_conflict",
        M7DeleteErrorCode::UnityProjectRunning => "unity_project_running",
        M7DeleteErrorCode::IdempotencyConflict => "idempotency_conflict",
        M7DeleteErrorCode::ProjectDeletePlanNotFound => "project_delete_plan_not_found",
        M7DeleteErrorCode::ProjectDeletePlanStale => "project_delete_plan_stale",
        M7DeleteErrorCode::ProjectDeleteSourceMissing => "project_delete_source_missing",
        M7DeleteErrorCode::ProjectDeleteSourceUnsafe => "project_delete_source_unsafe",
        M7DeleteErrorCode::ProjectDeleteSourceChanged => "project_delete_source_changed",
        M7DeleteErrorCode::ProjectDeleteRecoveryRequired => "project_delete_recovery_required",
        M7DeleteErrorCode::StoreUnavailable => "store_unavailable",
        M7DeleteErrorCode::Internal => "internal_error",
    }
}

fn now_ms() -> Result<u64, M7DeleteError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| u64::try_from(value.as_millis()).unwrap_or(u64::MAX))
        .map_err(|_| error(M7DeleteErrorCode::Internal))
}

#[cfg(feature = "test-kill-gates")]
fn project_delete_kill_gate(phase: &str) {
    if std::env::var("ALCOMD_TEST_PROJECT_DELETE_KILL_GATE").as_deref() == Ok(phase) {
        let signal = std::env::var_os("ALCOMD_TEST_PROJECT_DELETE_KILL_SIGNAL")
            .expect("Project Delete kill gate signal path");
        std::fs::write(signal, phase).expect("write Project Delete kill gate signal");
        loop {
            std::thread::park();
        }
    }
}

#[cfg(not(feature = "test-kill-gates"))]
fn project_delete_kill_gate(_: &str) {}

#[cfg(feature = "test-kill-gates")]
async fn project_delete_pause_gate(phase: &str) {
    if std::env::var("ALCOMD_TEST_PROJECT_DELETE_PAUSE_GATE").as_deref() == Ok(phase) {
        let signal = std::env::var_os("ALCOMD_TEST_PROJECT_DELETE_PAUSE_SIGNAL")
            .expect("Project Delete pause gate signal path");
        let release = std::env::var_os("ALCOMD_TEST_PROJECT_DELETE_PAUSE_RELEASE")
            .expect("Project Delete pause gate release path");
        std::fs::write(signal, phase).expect("write Project Delete pause gate signal");
        while !std::path::Path::new(&release).exists() {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    }
}

#[cfg(not(feature = "test-kill-gates"))]
async fn project_delete_pause_gate(_: &str) {}
