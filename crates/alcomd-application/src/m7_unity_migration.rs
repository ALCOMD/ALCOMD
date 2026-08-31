//! M7 durable Project Unity migration use case.

use std::future::Future;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::{
    AccessContext, IdempotencyKey, M3RegistryStore, M5BackupWriterGate, M5UnityStore, OperationId,
    Permission, PlanId, PrincipalId, ProjectId, ProjectRecord, ProjectType, ResourceKey,
    ResourceLockCoordinator, Revision, UnityInstallationId, UnityInstallationRecord,
    UnityWriterState, UnityWriterStateKind,
};

pub const UNITY_MIGRATION_PLAN_EXPIRY_MS: u64 = 900_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnityMigrationClassificationKind {
    PatchOrMinorUpgrade,
    MajorUpgrade,
    PatchOrMinorDowngrade,
    MajorDowngrade,
    ChinaVariantChange,
}

impl UnityMigrationClassificationKind {
    #[must_use]
    pub const fn supported_for_apply(self) -> bool {
        matches!(
            self,
            Self::PatchOrMinorUpgrade | Self::MajorUpgrade | Self::ChinaVariantChange
        )
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PatchOrMinorUpgrade => "patch_or_minor_upgrade",
            Self::MajorUpgrade => "major_upgrade",
            Self::PatchOrMinorDowngrade => "patch_or_minor_downgrade",
            Self::MajorDowngrade => "major_downgrade",
            Self::ChinaVariantChange => "china_variant_change",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnityMigrationPlanDraft {
    pub plan_id: PlanId,
    pub project: ProjectRecord,
    pub source_unity_version: String,
    pub source_revision_metadata: Option<String>,
    pub project_root_identity: Vec<u8>,
    pub project_version_marker_sha256: [u8; 32],
    pub target_unity_version: String,
    pub target_revision_metadata: Option<String>,
    pub target_installation: UnityInstallationRecord,
    pub writer_evidence: UnityWriterState,
    pub classification: UnityMigrationClassificationKind,
    pub preparation_profile: Option<String>,
    pub plan_fingerprint: String,
    pub request_fingerprint: String,
    pub plan_idempotency_key: IdempotencyKey,
    pub created_at_ms: u64,
    pub expires_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnityMigrationPlanRecord {
    #[serde(flatten)]
    pub draft: UnityMigrationPlanDraft,
    pub owner: PrincipalId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnityMigrationPlanAuthority {
    pub source_unity_version: String,
    pub target_unity_version: String,
    pub classification: UnityMigrationClassificationKind,
    pub preparation_profile: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum UnityMigrationPlanOutcome {
    NoChange {
        current_version: String,
    },
    Planned {
        plan: Box<UnityMigrationPlanRecord>,
        replayed: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnityMigrationApplyOutcome {
    pub operation_id: OperationId,
    pub replayed: bool,
    #[serde(skip)]
    pub schedule: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnityMigrationPhase {
    Accepted,
    PreflightComplete,
    PreparationIntent,
    PreparationComplete,
    LaunchIntent,
    UnityStarted,
    UnityExited,
    ProjectReobserved,
    StateCommitted,
    CleanupComplete,
    RecoveryRequired,
}

impl UnityMigrationPhase {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::PreflightComplete => "preflight_complete",
            Self::PreparationIntent => "preparation_intent",
            Self::PreparationComplete => "preparation_complete",
            Self::LaunchIntent => "launch_intent",
            Self::UnityStarted => "unity_started",
            Self::UnityExited => "unity_exited",
            Self::ProjectReobserved => "project_reobserved",
            Self::StateCommitted => "state_committed",
            Self::CleanupComplete => "cleanup_complete",
            Self::RecoveryRequired => "recovery_required",
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnityMigrationEvidence {
    pub preparation_kind: String,
    pub preparation_complete: bool,
    pub preparation_operation_id: Option<OperationId>,
    pub preparation_artifact_sha256: Option<[u8; 32]>,
    pub spawn_accepted: bool,
    pub exit_observation: Option<String>,
    pub reobserved_version: Option<String>,
    pub reobserved_root_identity: Option<Vec<u8>>,
    pub reobserved_marker_sha256: Option<[u8; 32]>,
    pub writer_inactive_checked_at_ms: Option<u64>,
    pub reobserved_at_ms: Option<u64>,
    pub safe_terminal_failure: bool,
    pub safe_evidence: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnityMigrationReobservation {
    pub observation: crate::ProjectObservation,
    pub evidence: UnityMigrationEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UnityMigrationRecoveryDisposition {
    Pending(UnityMigrationEvidence),
    ProjectReady(UnityMigrationReobservation),
    SafelyUnchanged(UnityMigrationEvidence),
    RecoveryRequired(UnityMigrationEvidence),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnityMigrationOperationRecord {
    pub plan: UnityMigrationPlanRecord,
    pub phase: UnityMigrationPhase,
    pub evidence: UnityMigrationEvidence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum M7UnityMigrationErrorCode {
    InvalidInput,
    PermissionDenied,
    ProjectNotRegistered,
    InstallationNotFound,
    RevisionConflict,
    IdempotencyConflict,
    ProjectRunning,
    PlanNotFound,
    PlanStale,
    Unsupported,
    SourceChanged,
    RecoveryRequired,
    StoreUnavailable,
    Internal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct M7UnityMigrationError(M7UnityMigrationErrorCode);

impl M7UnityMigrationError {
    #[must_use]
    pub const fn new(code: M7UnityMigrationErrorCode) -> Self {
        Self(code)
    }

    #[must_use]
    pub const fn code(self) -> M7UnityMigrationErrorCode {
        self.0
    }
}

impl std::fmt::Display for M7UnityMigrationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Project Unity migration request failed")
    }
}

impl std::error::Error for M7UnityMigrationError {}

pub trait M7UnityMigrationStore: Clone + Send + Sync + 'static {
    fn create_unity_migration_plan(
        &self,
        owner: PrincipalId,
        draft: UnityMigrationPlanDraft,
    ) -> impl Future<Output = Result<UnityMigrationPlanOutcome, M7UnityMigrationError>> + Send;
    fn get_unity_migration_plan(
        &self,
        owner: PrincipalId,
        plan_id: PlanId,
    ) -> impl Future<Output = Result<UnityMigrationPlanRecord, M7UnityMigrationError>> + Send;
    fn replay_unity_migration_apply(
        &self,
        owner: PrincipalId,
        plan_id: PlanId,
        key: IdempotencyKey,
    ) -> impl Future<Output = Result<Option<UnityMigrationApplyOutcome>, M7UnityMigrationError>> + Send;
    fn accept_unity_migration(
        &self,
        owner: PrincipalId,
        plan_id: PlanId,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<UnityMigrationApplyOutcome, M7UnityMigrationError>> + Send;
    fn begin_unity_migration(
        &self,
        operation_id: OperationId,
        now_ms: u64,
    ) -> impl Future<Output = Result<UnityMigrationOperationRecord, M7UnityMigrationError>> + Send;
    fn record_unity_migration_checkpoint(
        &self,
        operation_id: OperationId,
        phase: UnityMigrationPhase,
        evidence: UnityMigrationEvidence,
        now_ms: u64,
    ) -> impl Future<Output = Result<(), M7UnityMigrationError>> + Send;
    fn commit_unity_migration_project(
        &self,
        operation_id: OperationId,
        observation: crate::ProjectObservation,
        now_ms: u64,
    ) -> impl Future<Output = Result<(), M7UnityMigrationError>> + Send;
    fn finish_unity_migration_success(
        &self,
        operation_id: OperationId,
        now_ms: u64,
    ) -> impl Future<Output = Result<(), M7UnityMigrationError>> + Send;
    fn fail_unity_migration(
        &self,
        operation_id: OperationId,
        code: String,
        diagnostic_id: String,
        now_ms: u64,
    ) -> impl Future<Output = Result<(), M7UnityMigrationError>> + Send;
    fn recover_unity_migrations(
        &self,
        now_ms: u64,
    ) -> impl Future<Output = Result<Vec<OperationId>, M7UnityMigrationError>> + Send;
    fn unity_migration_cancel_requested(
        &self,
        operation_id: OperationId,
    ) -> impl Future<Output = Result<bool, M7UnityMigrationError>> + Send;
    fn finish_unity_migration_cancelled(
        &self,
        operation_id: OperationId,
        now_ms: u64,
    ) -> impl Future<Output = Result<(), M7UnityMigrationError>> + Send;
}

pub trait M7UnityMigrationAdapter: Clone + Send + Sync + 'static {
    type Process: Send + 'static;

    fn plan(
        &self,
        project: ProjectRecord,
        target: UnityInstallationRecord,
        writer: UnityWriterState,
        authority: UnityMigrationPlanAuthority,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<Option<UnityMigrationPlanDraft>, M7UnityMigrationError>> + Send;
    fn revalidate(
        &self,
        plan: UnityMigrationPlanRecord,
        project: ProjectRecord,
        target: UnityInstallationRecord,
    ) -> impl Future<Output = Result<(), M7UnityMigrationError>> + Send;
    fn materialize_preparation(
        &self,
        operation_id: OperationId,
        plan: UnityMigrationPlanRecord,
        evidence: UnityMigrationEvidence,
    ) -> impl Future<Output = Result<UnityMigrationEvidence, M7UnityMigrationError>> + Send;
    fn apply_preparation(
        &self,
        operation_id: OperationId,
        plan: UnityMigrationPlanRecord,
        evidence: UnityMigrationEvidence,
    ) -> impl Future<Output = Result<UnityMigrationEvidence, M7UnityMigrationError>> + Send;
    fn spawn(
        &self,
        plan: UnityMigrationPlanRecord,
        evidence: UnityMigrationEvidence,
    ) -> impl Future<Output = Result<(Self::Process, UnityMigrationEvidence), M7UnityMigrationError>>
    + Send;
    fn wait(
        &self,
        process: Self::Process,
        evidence: UnityMigrationEvidence,
    ) -> impl Future<Output = Result<UnityMigrationEvidence, M7UnityMigrationError>> + Send;
    fn recover_after_launch_intent(
        &self,
        plan: UnityMigrationPlanRecord,
        writer: UnityWriterState,
        evidence: UnityMigrationEvidence,
    ) -> impl Future<Output = Result<UnityMigrationRecoveryDisposition, M7UnityMigrationError>> + Send;
    fn resume_project_reobserved(
        &self,
        plan: UnityMigrationPlanRecord,
        writer: UnityWriterState,
        evidence: UnityMigrationEvidence,
    ) -> impl Future<Output = Result<UnityMigrationReobservation, M7UnityMigrationError>> + Send;
    fn cleanup(
        &self,
        operation_id: OperationId,
        plan: UnityMigrationPlanRecord,
        evidence: UnityMigrationEvidence,
    ) -> impl Future<Output = Result<UnityMigrationEvidence, M7UnityMigrationError>> + Send;
}

#[derive(Clone)]
pub struct M7UnityMigrationApplication<S, A, W> {
    store: S,
    adapter: A,
    writer: W,
    locks: Arc<ResourceLockCoordinator>,
}

impl<S, A, W> M7UnityMigrationApplication<S, A, W>
where
    S: M7UnityMigrationStore + M3RegistryStore + M5UnityStore,
    A: M7UnityMigrationAdapter,
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

    pub async fn recover(&self) -> Result<(), M7UnityMigrationError> {
        for operation_id in self.store.recover_unity_migrations(now_ms()?).await? {
            self.schedule(operation_id);
        }
        Ok(())
    }

    pub async fn plan(
        &self,
        access: &AccessContext,
        project_id: ProjectId,
        target_installation_id: UnityInstallationId,
        expected_project_revision: Revision,
        key: IdempotencyKey,
    ) -> Result<UnityMigrationPlanOutcome, M7UnityMigrationError> {
        require(access, Permission::ProjectsRead)?;
        require(access, Permission::UnityRead)?;
        require(access, Permission::ProjectsUnityMigrate)?;
        local_owner(access)?;
        let project = self
            .store
            .get_project(access.principal().clone(), project_id)
            .await
            .map_err(|_| error(M7UnityMigrationErrorCode::ProjectNotRegistered))?;
        if project.revision != expected_project_revision {
            return Err(error(M7UnityMigrationErrorCode::RevisionConflict));
        }
        let target = self
            .store
            .get_installation(access.principal().clone(), target_installation_id)
            .await
            .map_err(|_| error(M7UnityMigrationErrorCode::InstallationNotFound))?;
        let writer = self
            .writer
            .observe_backup_source(access, project_id)
            .await
            .map_err(|_| error(M7UnityMigrationErrorCode::Internal))?;
        require_safe_writer(writer.state)?;
        let source_unity_version =
            crate::m5::canonical_unity_version(&project.observation.unity_version)
                .map_err(|_| error(M7UnityMigrationErrorCode::SourceChanged))?;
        let target_unity_version =
            crate::m5::canonical_unity_version(&target.observation.unity_version)
                .map_err(|_| error(M7UnityMigrationErrorCode::InstallationNotFound))?;
        if source_unity_version == target_unity_version {
            return Ok(UnityMigrationPlanOutcome::NoChange {
                current_version: source_unity_version,
            });
        }
        let authority = migration_authority(
            &source_unity_version,
            &target_unity_version,
            project.observation.project_type,
        )?;
        let Some(draft) = self
            .adapter
            .plan(project, target, writer, authority, key, now_ms()?)
            .await?
        else {
            return Err(error(M7UnityMigrationErrorCode::Internal));
        };
        self.store
            .create_unity_migration_plan(access.principal().clone(), draft)
            .await
    }

    pub async fn apply(
        &self,
        access: &AccessContext,
        plan_id: PlanId,
        key: IdempotencyKey,
    ) -> Result<UnityMigrationApplyOutcome, M7UnityMigrationError> {
        require(access, Permission::ProjectsUnityMigrate)?;
        local_owner(access)?;
        if let Some(outcome) = self
            .store
            .replay_unity_migration_apply(access.principal().clone(), plan_id, key.clone())
            .await?
        {
            return Ok(outcome);
        }
        let plan = self
            .store
            .get_unity_migration_plan(access.principal().clone(), plan_id)
            .await?;
        if !plan.draft.classification.supported_for_apply() {
            return Err(error(M7UnityMigrationErrorCode::Unsupported));
        }
        let _guard = self
            .locks
            .acquire(vec![ResourceKey::Project(plan.draft.project.project_id)])
            .await;
        let project = self
            .store
            .get_project(access.principal().clone(), plan.draft.project.project_id)
            .await
            .map_err(|_| error(M7UnityMigrationErrorCode::ProjectNotRegistered))?;
        let target = self
            .store
            .get_installation(
                access.principal().clone(),
                plan.draft.target_installation.installation_id,
            )
            .await
            .map_err(|_| error(M7UnityMigrationErrorCode::InstallationNotFound))?;
        let writer = self
            .writer
            .observe_backup_source(access, plan.draft.project.project_id)
            .await
            .map_err(|_| error(M7UnityMigrationErrorCode::Internal))?;
        require_safe_writer(writer.state)?;
        self.adapter.revalidate(plan, project, target).await?;
        let outcome = self
            .store
            .accept_unity_migration(access.principal().clone(), plan_id, key, now_ms()?)
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

    async fn run(&self, operation_id: OperationId) -> Result<(), M7UnityMigrationError> {
        let mut record = self
            .store
            .begin_unity_migration(operation_id, now_ms()?)
            .await?;
        if record.phase == UnityMigrationPhase::RecoveryRequired {
            return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
        }
        let _guard = self
            .locks
            .acquire(vec![ResourceKey::Project(
                record.plan.draft.project.project_id,
            )])
            .await;
        if record.phase == UnityMigrationPhase::Accepted
            && self
                .store
                .unity_migration_cancel_requested(operation_id)
                .await?
        {
            self.store
                .finish_unity_migration_cancelled(operation_id, now_ms()?)
                .await?;
            return Ok(());
        }
        if record.phase == UnityMigrationPhase::Accepted {
            let access = AccessContext::local_owner();
            let project = match self
                .store
                .get_project(
                    record.plan.owner.clone(),
                    record.plan.draft.project.project_id,
                )
                .await
            {
                Ok(project) => project,
                Err(_) => {
                    self.fail_before_intent(
                        operation_id,
                        error(M7UnityMigrationErrorCode::ProjectNotRegistered),
                    )
                    .await?;
                    return Ok(());
                }
            };
            let target = match self
                .store
                .get_installation(
                    record.plan.owner.clone(),
                    record.plan.draft.target_installation.installation_id,
                )
                .await
            {
                Ok(target) => target,
                Err(_) => {
                    self.fail_before_intent(
                        operation_id,
                        error(M7UnityMigrationErrorCode::InstallationNotFound),
                    )
                    .await?;
                    return Ok(());
                }
            };
            let writer = match self
                .writer
                .observe_backup_source(&access, record.plan.draft.project.project_id)
                .await
            {
                Ok(writer) => writer,
                Err(_) => {
                    self.fail_before_intent(
                        operation_id,
                        error(M7UnityMigrationErrorCode::Internal),
                    )
                    .await?;
                    return Ok(());
                }
            };
            if let Err(failure) = require_safe_writer(writer.state) {
                self.fail_before_intent(operation_id, failure).await?;
                return Ok(());
            }
            if let Err(failure) = self
                .adapter
                .revalidate(record.plan.clone(), project, target)
                .await
            {
                self.fail_before_intent(operation_id, failure).await?;
                return Ok(());
            }
            self.store
                .record_unity_migration_checkpoint(
                    operation_id,
                    UnityMigrationPhase::PreflightComplete,
                    record.evidence.clone(),
                    now_ms()?,
                )
                .await?;
            record.phase = UnityMigrationPhase::PreflightComplete;
        }
        if record.phase == UnityMigrationPhase::PreflightComplete
            && self
                .store
                .unity_migration_cancel_requested(operation_id)
                .await?
        {
            self.store
                .finish_unity_migration_cancelled(operation_id, now_ms()?)
                .await?;
            return Ok(());
        }
        if matches!(
            record.phase,
            UnityMigrationPhase::PreflightComplete | UnityMigrationPhase::PreparationIntent
        ) {
            let evidence = if record.phase == UnityMigrationPhase::PreflightComplete {
                let evidence = match self
                    .adapter
                    .materialize_preparation(
                        operation_id,
                        record.plan.clone(),
                        record.evidence.clone(),
                    )
                    .await
                {
                    Ok(evidence) => evidence,
                    Err(failure) => {
                        self.fail_before_intent(operation_id, failure).await?;
                        return Ok(());
                    }
                };
                self.store
                    .record_unity_migration_checkpoint(
                        operation_id,
                        UnityMigrationPhase::PreparationIntent,
                        evidence.clone(),
                        now_ms()?,
                    )
                    .await?;
                evidence
            } else {
                record.evidence.clone()
            };
            let recovery_evidence = evidence.clone();
            let evidence = match self
                .adapter
                .apply_preparation(operation_id, record.plan.clone(), evidence)
                .await
            {
                Ok(evidence) => evidence,
                Err(_) => {
                    self.mark_recovery_required(operation_id, recovery_evidence)
                        .await?;
                    return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
                }
            };
            self.store
                .record_unity_migration_checkpoint(
                    operation_id,
                    UnityMigrationPhase::PreparationComplete,
                    evidence.clone(),
                    now_ms()?,
                )
                .await?;
            record.evidence = evidence;
            record.phase = UnityMigrationPhase::PreparationComplete;
        }
        if record.phase == UnityMigrationPhase::PreparationComplete {
            if record.evidence.preparation_kind == "none"
                && self
                    .store
                    .unity_migration_cancel_requested(operation_id)
                    .await?
            {
                self.store
                    .finish_unity_migration_cancelled(operation_id, now_ms()?)
                    .await?;
                return Ok(());
            }
            self.store
                .record_unity_migration_checkpoint(
                    operation_id,
                    UnityMigrationPhase::LaunchIntent,
                    record.evidence.clone(),
                    now_ms()?,
                )
                .await?;
            record.phase = UnityMigrationPhase::LaunchIntent;
            match self
                .adapter
                .spawn(record.plan.clone(), record.evidence.clone())
                .await
            {
                Ok((process, evidence)) => {
                    self.store
                        .record_unity_migration_checkpoint(
                            operation_id,
                            UnityMigrationPhase::UnityStarted,
                            evidence.clone(),
                            now_ms()?,
                        )
                        .await?;
                    let recovery_evidence = evidence.clone();
                    let evidence = match self.adapter.wait(process, evidence).await {
                        Ok(evidence) => evidence,
                        Err(_) => {
                            self.mark_recovery_required(operation_id, recovery_evidence)
                                .await?;
                            return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
                        }
                    };
                    self.store
                        .record_unity_migration_checkpoint(
                            operation_id,
                            UnityMigrationPhase::UnityExited,
                            evidence.clone(),
                            now_ms()?,
                        )
                        .await?;
                    record.evidence = evidence;
                    record.phase = UnityMigrationPhase::UnityExited;
                }
                Err(_) => {
                    record.phase = UnityMigrationPhase::LaunchIntent;
                }
            }
        }
        if matches!(
            record.phase,
            UnityMigrationPhase::LaunchIntent
                | UnityMigrationPhase::UnityStarted
                | UnityMigrationPhase::UnityExited
        ) {
            loop {
                let access = AccessContext::local_owner();
                let writer = match self
                    .writer
                    .observe_backup_source(&access, record.plan.draft.project.project_id)
                    .await
                {
                    Ok(writer) => writer,
                    Err(_) => {
                        self.mark_recovery_required(operation_id, record.evidence.clone())
                            .await?;
                        return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
                    }
                };
                let disposition = match self
                    .adapter
                    .recover_after_launch_intent(
                        record.plan.clone(),
                        writer,
                        record.evidence.clone(),
                    )
                    .await
                {
                    Ok(disposition) => disposition,
                    Err(_) => {
                        self.mark_recovery_required(operation_id, record.evidence.clone())
                            .await?;
                        return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
                    }
                };
                match disposition {
                    UnityMigrationRecoveryDisposition::Pending(evidence) => {
                        if evidence != record.evidence {
                            self.store
                                .record_unity_migration_checkpoint(
                                    operation_id,
                                    record.phase,
                                    evidence.clone(),
                                    now_ms()?,
                                )
                                .await?;
                        }
                        record.evidence = evidence;
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    }
                    UnityMigrationRecoveryDisposition::ProjectReady(reobserved) => {
                        self.store
                            .record_unity_migration_checkpoint(
                                operation_id,
                                UnityMigrationPhase::ProjectReobserved,
                                reobserved.evidence.clone(),
                                now_ms()?,
                            )
                            .await?;
                        self.store
                            .commit_unity_migration_project(
                                operation_id,
                                reobserved.observation,
                                now_ms()?,
                            )
                            .await?;
                        record.evidence = reobserved.evidence;
                        record.phase = UnityMigrationPhase::StateCommitted;
                        break;
                    }
                    UnityMigrationRecoveryDisposition::SafelyUnchanged(evidence) => {
                        self.store
                            .record_unity_migration_checkpoint(
                                operation_id,
                                record.phase,
                                evidence,
                                now_ms()?,
                            )
                            .await?;
                        self.store
                            .fail_unity_migration(
                                operation_id,
                                error_code(M7UnityMigrationErrorCode::Internal).to_owned(),
                                operation_id.to_string(),
                                now_ms()?,
                            )
                            .await?;
                        return Ok(());
                    }
                    UnityMigrationRecoveryDisposition::RecoveryRequired(evidence) => {
                        self.mark_recovery_required(operation_id, evidence).await?;
                        return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
                    }
                }
            }
        }
        if record.phase == UnityMigrationPhase::ProjectReobserved {
            let access = AccessContext::local_owner();
            let writer = match self
                .writer
                .observe_backup_source(&access, record.plan.draft.project.project_id)
                .await
            {
                Ok(writer) => writer,
                Err(_) => {
                    self.mark_recovery_required(operation_id, record.evidence.clone())
                        .await?;
                    return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
                }
            };
            let reobserved = match self
                .adapter
                .resume_project_reobserved(record.plan.clone(), writer, record.evidence.clone())
                .await
            {
                Ok(reobserved) => reobserved,
                Err(_) => {
                    self.mark_recovery_required(operation_id, record.evidence)
                        .await?;
                    return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
                }
            };
            self.store
                .commit_unity_migration_project(operation_id, reobserved.observation, now_ms()?)
                .await?;
            record.evidence = reobserved.evidence;
            record.phase = UnityMigrationPhase::StateCommitted;
        }
        if record.phase == UnityMigrationPhase::StateCommitted {
            let recovery_evidence = record.evidence.clone();
            let evidence = match self
                .adapter
                .cleanup(operation_id, record.plan, record.evidence)
                .await
            {
                Ok(evidence) => evidence,
                Err(_) => {
                    self.mark_recovery_required(operation_id, recovery_evidence)
                        .await?;
                    return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
                }
            };
            self.store
                .record_unity_migration_checkpoint(
                    operation_id,
                    UnityMigrationPhase::CleanupComplete,
                    evidence,
                    now_ms()?,
                )
                .await?;
            record.phase = UnityMigrationPhase::CleanupComplete;
        }
        if record.phase == UnityMigrationPhase::CleanupComplete {
            return self
                .store
                .finish_unity_migration_success(operation_id, now_ms()?)
                .await;
        }
        Err(error(M7UnityMigrationErrorCode::RecoveryRequired))
    }

    async fn fail_before_intent(
        &self,
        operation_id: OperationId,
        failure: M7UnityMigrationError,
    ) -> Result<(), M7UnityMigrationError> {
        self.store
            .fail_unity_migration(
                operation_id,
                error_code(failure.code()).to_owned(),
                operation_id.to_string(),
                now_ms()?,
            )
            .await
    }

    async fn mark_recovery_required(
        &self,
        operation_id: OperationId,
        evidence: UnityMigrationEvidence,
    ) -> Result<(), M7UnityMigrationError> {
        self.store
            .record_unity_migration_checkpoint(
                operation_id,
                UnityMigrationPhase::RecoveryRequired,
                evidence,
                now_ms()?,
            )
            .await
    }
}

fn require_safe_writer(state: UnityWriterStateKind) -> Result<(), M7UnityMigrationError> {
    if state == UnityWriterStateKind::NotObserved {
        Ok(())
    } else {
        Err(error(M7UnityMigrationErrorCode::ProjectRunning))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ParsedUnityVersion {
    major: u64,
    minor: u64,
    patch: u64,
    channel: u8,
    increment: u64,
    china_increment: Option<u64>,
}

fn migration_authority(
    source: &str,
    target: &str,
    project_type: ProjectType,
) -> Result<UnityMigrationPlanAuthority, M7UnityMigrationError> {
    let source_parsed = parse_canonical_unity_version(source)?;
    let target_parsed = parse_canonical_unity_version(target)?;
    if source_parsed.channel == b'x' || target_parsed.channel == b'x' {
        return Err(error(M7UnityMigrationErrorCode::Unsupported));
    }
    let source_base = (
        source_parsed.major,
        source_parsed.minor,
        source_parsed.patch,
        source_parsed.channel,
        source_parsed.increment,
    );
    let target_base = (
        target_parsed.major,
        target_parsed.minor,
        target_parsed.patch,
        target_parsed.channel,
        target_parsed.increment,
    );
    let classification = if source_base == target_base
        && source_parsed.china_increment != target_parsed.china_increment
    {
        UnityMigrationClassificationKind::ChinaVariantChange
    } else if source_parsed.china_increment != target_parsed.china_increment {
        return Err(error(M7UnityMigrationErrorCode::Unsupported));
    } else if source_parsed.major != target_parsed.major {
        if source_parsed.major < target_parsed.major {
            UnityMigrationClassificationKind::MajorUpgrade
        } else {
            UnityMigrationClassificationKind::MajorDowngrade
        }
    } else {
        let source_order = (
            source_parsed.minor,
            source_parsed.patch,
            unity_channel_order(source_parsed.channel)?,
            source_parsed.increment,
        );
        let target_order = (
            target_parsed.minor,
            target_parsed.patch,
            unity_channel_order(target_parsed.channel)?,
            target_parsed.increment,
        );
        if source_order < target_order {
            UnityMigrationClassificationKind::PatchOrMinorUpgrade
        } else {
            UnityMigrationClassificationKind::PatchOrMinorDowngrade
        }
    };
    let preparation_profile = (source_parsed.major == 2019
        && target_parsed.major == 2022
        && matches!(
            project_type,
            ProjectType::Avatars
                | ProjectType::Worlds
                | ProjectType::LegacySdk2
                | ProjectType::LegacyWorlds
                | ProjectType::LegacyAvatars
        ))
    .then(|| "vrchat-2019-to-2022-v1".to_owned());
    Ok(UnityMigrationPlanAuthority {
        source_unity_version: source.to_owned(),
        target_unity_version: target.to_owned(),
        classification,
        preparation_profile,
    })
}

fn parse_canonical_unity_version(value: &str) -> Result<ParsedUnityVersion, M7UnityMigrationError> {
    let channel_index = value
        .bytes()
        .position(|byte| matches!(byte, b'a' | b'b' | b'f' | b'p' | b'x'))
        .ok_or_else(|| error(M7UnityMigrationErrorCode::InvalidInput))?;
    let mut release = value[..channel_index].split('.');
    let major = release.next().and_then(|part| part.parse().ok());
    let minor = release.next().and_then(|part| part.parse().ok());
    let patch = release.next().and_then(|part| part.parse().ok());
    if release.next().is_some() {
        return Err(error(M7UnityMigrationErrorCode::InvalidInput));
    }
    let suffix = &value[channel_index + 1..];
    let (increment, china_increment) = suffix.split_once('c').map_or_else(
        || (suffix.parse().ok(), None),
        |(increment, china)| (increment.parse().ok(), china.parse().ok()),
    );
    Ok(ParsedUnityVersion {
        major: major.ok_or_else(|| error(M7UnityMigrationErrorCode::InvalidInput))?,
        minor: minor.ok_or_else(|| error(M7UnityMigrationErrorCode::InvalidInput))?,
        patch: patch.ok_or_else(|| error(M7UnityMigrationErrorCode::InvalidInput))?,
        channel: value.as_bytes()[channel_index],
        increment: increment.ok_or_else(|| error(M7UnityMigrationErrorCode::InvalidInput))?,
        china_increment,
    })
}

const fn unity_channel_order(channel: u8) -> Result<u8, M7UnityMigrationError> {
    match channel {
        b'a' => Ok(0),
        b'b' => Ok(1),
        b'f' => Ok(2),
        b'p' => Ok(3),
        _ => Err(error(M7UnityMigrationErrorCode::Unsupported)),
    }
}

const fn error_code(code: M7UnityMigrationErrorCode) -> &'static str {
    match code {
        M7UnityMigrationErrorCode::InvalidInput => "invalid_request",
        M7UnityMigrationErrorCode::PermissionDenied => "permission_denied",
        M7UnityMigrationErrorCode::ProjectNotRegistered => "project_not_registered",
        M7UnityMigrationErrorCode::InstallationNotFound => "unity_installation_not_found",
        M7UnityMigrationErrorCode::RevisionConflict => "revision_conflict",
        M7UnityMigrationErrorCode::IdempotencyConflict => "idempotency_conflict",
        M7UnityMigrationErrorCode::ProjectRunning => "unity_project_running",
        M7UnityMigrationErrorCode::PlanNotFound => "project_unity_migration_plan_not_found",
        M7UnityMigrationErrorCode::PlanStale => "project_unity_migration_plan_stale",
        M7UnityMigrationErrorCode::Unsupported => "project_unity_migration_unsupported",
        M7UnityMigrationErrorCode::SourceChanged => "project_unity_migration_source_changed",
        M7UnityMigrationErrorCode::RecoveryRequired => "project_unity_migration_recovery_required",
        M7UnityMigrationErrorCode::StoreUnavailable => "store_unavailable",
        M7UnityMigrationErrorCode::Internal => "internal_error",
    }
}

fn require(access: &AccessContext, permission: Permission) -> Result<(), M7UnityMigrationError> {
    access
        .require(permission)
        .map_err(|_| error(M7UnityMigrationErrorCode::PermissionDenied))
}

fn local_owner(access: &AccessContext) -> Result<(), M7UnityMigrationError> {
    (access.principal().as_str() == PrincipalId::LOCAL_OWNER)
        .then_some(())
        .ok_or_else(|| error(M7UnityMigrationErrorCode::PermissionDenied))
}

fn now_ms() -> Result<u64, M7UnityMigrationError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| error(M7UnityMigrationErrorCode::Internal))
        .and_then(|duration| {
            u64::try_from(duration.as_millis())
                .map_err(|_| error(M7UnityMigrationErrorCode::Internal))
        })
}

const fn error(code: M7UnityMigrationErrorCode) -> M7UnityMigrationError {
    M7UnityMigrationError::new(code)
}
