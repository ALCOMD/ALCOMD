use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use alcomd_application::{
    AccessContext, IdempotencyKey, M3RegistryStore, M5BackupError, M5BackupWriterGate,
    M5UnityStore, M7UnityMigrationAdapter, M7UnityMigrationApplication, M7UnityMigrationError,
    M7UnityMigrationStore, ManifestState, OperationId, OperationState, PlanId, PrincipalId,
    ProjectObservation, ProjectRecord, ProjectType, ResourceLockCoordinator, StateStore,
    UnityArchitecture, UnityInstallationObservation, UnityInstallationRecord,
    UnityMigrationClassificationKind, UnityMigrationEvidence, UnityMigrationPhase,
    UnityMigrationPlanAuthority, UnityMigrationPlanDraft, UnityMigrationPlanRecord,
    UnityMigrationRecoveryDisposition, UnityMigrationReobservation, UnitySourceKind,
    UnityWriterState, UnityWriterStateKind,
};
use alcomd_store::StateStoreHandle;
use tokio::sync::Notify;
use uuid::Uuid;

#[tokio::test]
async fn worker_persists_unity_started_before_wait_and_commits_after_cleanup() {
    let fixture = TestDirectory::new();
    let database_path = fixture.0.join("state.db");
    let store = StateStoreHandle::open(database_path.clone()).expect("open state store");
    let (owner, draft) = registered_draft(&store).await;
    store
        .create_unity_migration_plan(owner, draft.clone())
        .await
        .expect("create durable migration Plan");

    let adapter = RecordingAdapter::new(store.clone());
    let release_wait = Arc::clone(&adapter.release_wait);
    let wait_entered = Arc::clone(&adapter.wait_entered);
    let call_order = Arc::clone(&adapter.call_order);
    let application = M7UnityMigrationApplication::with_locks(
        store.clone(),
        adapter,
        FixedWriter,
        Arc::new(ResourceLockCoordinator::default()),
    );
    let outcome = application
        .apply(
            &AccessContext::local_owner(),
            draft.plan_id,
            key("worker-apply"),
        )
        .await
        .expect("schedule migration worker");

    wait_until(Duration::from_secs(5), || {
        wait_entered.load(Ordering::SeqCst)
    })
    .await;
    assert_eq!(
        operation_state(&store, outcome.operation_id).await,
        OperationState::Running
    );

    release_wait.notify_one();
    wait_for_operation_state(
        &store,
        outcome.operation_id,
        OperationState::Succeeded,
        Duration::from_secs(5),
    )
    .await;

    assert_eq!(
        call_order.lock().expect("call-order lock").as_slice(),
        vec![
            "revalidate",
            "revalidate",
            "materialize_preparation",
            "apply_preparation",
            "spawn",
            "wait_after_durable_unity_started",
            "recover_after_launch_intent",
            "cleanup",
        ]
    );
    let events = store
        .list_events(PrincipalId::local_owner(), 0, 100)
        .await
        .expect("list Events");
    assert_eq!(
        events
            .events
            .iter()
            .filter(|event| event.kind == "project.unity_version_migrated")
            .count(),
        1
    );
    assert!(
        events
            .events
            .iter()
            .all(|event| !event.kind.starts_with("package."))
    );
}

#[tokio::test]
async fn restart_recovery_resumes_each_durable_forward_phase_without_replanning() {
    for start_phase in [
        UnityMigrationPhase::LaunchIntent,
        UnityMigrationPhase::UnityStarted,
        UnityMigrationPhase::ProjectReobserved,
        UnityMigrationPhase::StateCommitted,
        UnityMigrationPhase::CleanupComplete,
    ] {
        let fixture = TestDirectory::new();
        let store = StateStoreHandle::open(fixture.0.join("state.db")).expect("open state store");
        let (owner, draft) = registered_draft(&store).await;
        store
            .create_unity_migration_plan(owner.clone(), draft.clone())
            .await
            .expect("create durable migration Plan");
        let accepted = store
            .accept_unity_migration(
                owner.clone(),
                draft.plan_id,
                key(&format!("recovery-apply-{}", start_phase.as_str())),
                20,
            )
            .await
            .expect("accept migration");
        store
            .begin_unity_migration(accepted.operation_id, 21)
            .await
            .expect("begin migration");
        let evidence = recovery_evidence(&draft, accepted.operation_id);
        for phase in recovery_prefix(start_phase) {
            store
                .record_unity_migration_checkpoint(
                    accepted.operation_id,
                    phase,
                    evidence.clone(),
                    22,
                )
                .await
                .expect("persist recovery checkpoint");
        }
        if matches!(
            start_phase,
            UnityMigrationPhase::StateCommitted | UnityMigrationPhase::CleanupComplete
        ) {
            store
                .commit_unity_migration_project(
                    accepted.operation_id,
                    target_observation(&draft),
                    23,
                )
                .await
                .expect("commit Project before simulated restart");
        }
        if start_phase == UnityMigrationPhase::CleanupComplete {
            store
                .record_unity_migration_checkpoint(
                    accepted.operation_id,
                    UnityMigrationPhase::CleanupComplete,
                    evidence,
                    24,
                )
                .await
                .expect("persist cleanup before simulated restart");
        }

        let adapter = RecordingAdapter::new(store.clone());
        let call_order = Arc::clone(&adapter.call_order);
        let application = M7UnityMigrationApplication::with_locks(
            store.clone(),
            adapter,
            FixedWriter,
            Arc::new(ResourceLockCoordinator::default()),
        );
        application.recover().await.expect("schedule recovery");
        wait_for_operation_state(
            &store,
            accepted.operation_id,
            OperationState::Succeeded,
            Duration::from_secs(5),
        )
        .await;

        let expected_calls: &[&str] = match start_phase {
            UnityMigrationPhase::LaunchIntent | UnityMigrationPhase::UnityStarted => {
                &["recover_after_launch_intent", "cleanup"]
            }
            UnityMigrationPhase::ProjectReobserved => &["resume_project_reobserved", "cleanup"],
            UnityMigrationPhase::StateCommitted => &["cleanup"],
            UnityMigrationPhase::CleanupComplete => &[],
            _ => unreachable!("recovery matrix phase is fixed"),
        };
        assert_eq!(
            call_order.lock().expect("call-order lock").as_slice(),
            expected_calls,
            "recovery path for {}",
            start_phase.as_str()
        );
        let events = store
            .list_events(owner, 0, 100)
            .await
            .expect("list recovery Events");
        assert_eq!(
            events
                .events
                .iter()
                .filter(|event| event.kind == "project.unity_version_migrated")
                .count(),
            1,
            "exactly one Project Event for {}",
            start_phase.as_str()
        );
        assert!(
            events
                .events
                .iter()
                .all(|event| !event.kind.starts_with("package."))
        );
    }
}

#[tokio::test]
async fn restart_recovery_required_never_reports_success_or_commits_project() {
    let fixture = TestDirectory::new();
    let store = StateStoreHandle::open(fixture.0.join("state.db")).expect("open state store");
    let (owner, draft) = registered_draft(&store).await;
    store
        .create_unity_migration_plan(owner.clone(), draft.clone())
        .await
        .expect("create durable migration Plan");
    let accepted = store
        .accept_unity_migration(owner.clone(), draft.plan_id, key("required-apply"), 20)
        .await
        .expect("accept migration");
    store
        .begin_unity_migration(accepted.operation_id, 21)
        .await
        .expect("begin migration");
    let evidence = recovery_evidence(&draft, accepted.operation_id);
    for phase in recovery_prefix(UnityMigrationPhase::UnityStarted) {
        store
            .record_unity_migration_checkpoint(accepted.operation_id, phase, evidence.clone(), 22)
            .await
            .expect("persist recovery checkpoint");
    }

    let adapter = RecordingAdapter::recovery_required(store.clone());
    let recovery_observed = Arc::clone(&adapter.recovery_observed);
    let application = M7UnityMigrationApplication::with_locks(
        store.clone(),
        adapter,
        FixedWriter,
        Arc::new(ResourceLockCoordinator::default()),
    );
    application.recover().await.expect("schedule recovery");
    wait_until(Duration::from_secs(5), || {
        recovery_observed.load(Ordering::SeqCst)
    })
    .await;

    assert_ne!(
        operation_state(&store, accepted.operation_id).await,
        OperationState::Succeeded
    );
    let events = store.list_events(owner, 0, 100).await.expect("list Events");
    assert!(
        events
            .events
            .iter()
            .all(|event| event.kind != "project.unity_version_migrated")
    );
    assert!(
        events
            .events
            .iter()
            .all(|event| event.kind != "operation.succeeded")
    );
}

#[tokio::test]
async fn observed_exit_with_conclusively_unchanged_project_is_an_ordinary_failure() {
    let fixture = TestDirectory::new();
    let store = StateStoreHandle::open(fixture.0.join("state.db")).expect("open state store");
    let (owner, draft) = registered_draft(&store).await;
    store
        .create_unity_migration_plan(owner.clone(), draft.clone())
        .await
        .expect("create durable migration Plan");
    let accepted = store
        .accept_unity_migration(owner.clone(), draft.plan_id, key("unchanged-apply"), 20)
        .await
        .expect("accept migration");
    store
        .begin_unity_migration(accepted.operation_id, 21)
        .await
        .expect("begin migration");
    let mut evidence = recovery_evidence(&draft, accepted.operation_id);
    evidence.preparation_kind = "none".to_owned();
    for phase in [
        UnityMigrationPhase::PreflightComplete,
        UnityMigrationPhase::PreparationIntent,
        UnityMigrationPhase::PreparationComplete,
        UnityMigrationPhase::LaunchIntent,
        UnityMigrationPhase::UnityStarted,
        UnityMigrationPhase::UnityExited,
    ] {
        store
            .record_unity_migration_checkpoint(accepted.operation_id, phase, evidence.clone(), 22)
            .await
            .expect("persist recovery checkpoint");
    }

    let adapter = RecordingAdapter::safely_unchanged(store.clone());
    let application = M7UnityMigrationApplication::with_locks(
        store.clone(),
        adapter,
        FixedWriter,
        Arc::new(ResourceLockCoordinator::default()),
    );
    application.recover().await.expect("schedule recovery");
    wait_for_operation_state(
        &store,
        accepted.operation_id,
        OperationState::Failed,
        Duration::from_secs(5),
    )
    .await;

    let operation = store
        .list_operations(owner.clone(), None, 100)
        .await
        .expect("list Operations")
        .operations
        .into_iter()
        .find(|operation| operation.operation_id == accepted.operation_id)
        .expect("migration Operation");
    assert_eq!(operation.error_code.as_deref(), Some("internal_error"));
    let events = store.list_events(owner, 0, 100).await.expect("list Events");
    assert!(
        events
            .events
            .iter()
            .all(|event| event.kind != "project.unity_version_migrated")
    );
    assert!(
        events
            .events
            .iter()
            .all(|event| event.kind != "operation.succeeded")
    );
}

#[derive(Clone)]
struct FixedWriter;

impl M5BackupWriterGate for FixedWriter {
    async fn observe_backup_source(
        &self,
        _access: &AccessContext,
        project_id: alcomd_application::ProjectId,
    ) -> Result<UnityWriterState, M5BackupError> {
        Ok(UnityWriterState {
            project_id,
            state: UnityWriterStateKind::NotObserved,
            evidence: Vec::new(),
            checked_at_ms: 100,
        })
    }
}

#[derive(Clone)]
struct RecordingAdapter {
    store: StateStoreHandle,
    wait_entered: Arc<AtomicBool>,
    release_wait: Arc<Notify>,
    call_order: Arc<Mutex<Vec<&'static str>>>,
    recovery_required: bool,
    safely_unchanged: bool,
    recovery_observed: Arc<AtomicBool>,
}

impl RecordingAdapter {
    fn new(store: StateStoreHandle) -> Self {
        Self {
            store,
            wait_entered: Arc::new(AtomicBool::new(false)),
            release_wait: Arc::new(Notify::new()),
            call_order: Arc::new(Mutex::new(Vec::new())),
            recovery_required: false,
            safely_unchanged: false,
            recovery_observed: Arc::new(AtomicBool::new(false)),
        }
    }

    fn recovery_required(store: StateStoreHandle) -> Self {
        Self {
            recovery_required: true,
            ..Self::new(store)
        }
    }

    fn safely_unchanged(store: StateStoreHandle) -> Self {
        Self {
            safely_unchanged: true,
            ..Self::new(store)
        }
    }

    fn record(&self, value: &'static str) {
        self.call_order.lock().expect("call-order lock").push(value);
    }

    fn reobservation(
        plan: &UnityMigrationPlanRecord,
        mut evidence: UnityMigrationEvidence,
    ) -> UnityMigrationReobservation {
        let mut observation = plan.draft.project.observation.clone();
        observation.unity_version = plan.draft.target_unity_version.clone();
        observation.unity_revision = plan.draft.target_revision_metadata.clone();
        observation.observed_at_ms = 200;
        evidence.reobserved_version = Some(plan.draft.target_unity_version.clone());
        evidence.reobserved_root_identity = Some(plan.draft.project_root_identity.clone());
        evidence.reobserved_marker_sha256 = Some([8; 32]);
        evidence.writer_inactive_checked_at_ms = Some(200);
        evidence.reobserved_at_ms = Some(200);
        UnityMigrationReobservation {
            observation,
            evidence,
        }
    }
}

impl M7UnityMigrationAdapter for RecordingAdapter {
    type Process = Arc<Notify>;

    async fn plan(
        &self,
        _project: ProjectRecord,
        _target: UnityInstallationRecord,
        _writer: UnityWriterState,
        _authority: UnityMigrationPlanAuthority,
        _key: IdempotencyKey,
        _now_ms: u64,
    ) -> Result<Option<UnityMigrationPlanDraft>, M7UnityMigrationError> {
        Ok(None)
    }

    async fn revalidate(
        &self,
        _plan: UnityMigrationPlanRecord,
        _project: ProjectRecord,
        _target: UnityInstallationRecord,
    ) -> Result<(), M7UnityMigrationError> {
        self.record("revalidate");
        Ok(())
    }

    async fn materialize_preparation(
        &self,
        operation_id: OperationId,
        _plan: UnityMigrationPlanRecord,
        mut evidence: UnityMigrationEvidence,
    ) -> Result<UnityMigrationEvidence, M7UnityMigrationError> {
        self.record("materialize_preparation");
        evidence.preparation_kind = "vrchat-2019-to-2022-v1".to_owned();
        evidence.preparation_operation_id = Some(operation_id);
        evidence.preparation_artifact_sha256 = Some([6; 32]);
        Ok(evidence)
    }

    async fn apply_preparation(
        &self,
        _operation_id: OperationId,
        _plan: UnityMigrationPlanRecord,
        mut evidence: UnityMigrationEvidence,
    ) -> Result<UnityMigrationEvidence, M7UnityMigrationError> {
        self.record("apply_preparation");
        evidence.preparation_complete = true;
        Ok(evidence)
    }

    async fn spawn(
        &self,
        _plan: UnityMigrationPlanRecord,
        mut evidence: UnityMigrationEvidence,
    ) -> Result<(Self::Process, UnityMigrationEvidence), M7UnityMigrationError> {
        self.record("spawn");
        evidence.spawn_accepted = true;
        Ok((Arc::clone(&self.release_wait), evidence))
    }

    async fn wait(
        &self,
        process: Self::Process,
        mut evidence: UnityMigrationEvidence,
    ) -> Result<UnityMigrationEvidence, M7UnityMigrationError> {
        let events = self
            .store
            .list_events(PrincipalId::local_owner(), 0, 100)
            .await
            .expect("read durable progress Events before child wait");
        assert_eq!(
            events
                .events
                .iter()
                .filter(|event| event.kind == "operation.progress")
                .count(),
            6,
            "unity_started must commit its Operation progress before wait begins"
        );
        self.record("wait_after_durable_unity_started");
        self.wait_entered.store(true, Ordering::SeqCst);
        process.notified().await;
        evidence.exit_observation = Some("success".to_owned());
        Ok(evidence)
    }

    async fn recover_after_launch_intent(
        &self,
        plan: UnityMigrationPlanRecord,
        _writer: UnityWriterState,
        evidence: UnityMigrationEvidence,
    ) -> Result<UnityMigrationRecoveryDisposition, M7UnityMigrationError> {
        self.record("recover_after_launch_intent");
        self.recovery_observed.store(true, Ordering::SeqCst);
        if self.recovery_required {
            return Ok(UnityMigrationRecoveryDisposition::RecoveryRequired(
                evidence,
            ));
        }
        if self.safely_unchanged {
            let mut evidence = evidence;
            evidence.safe_terminal_failure = true;
            return Ok(UnityMigrationRecoveryDisposition::SafelyUnchanged(evidence));
        }
        Ok(UnityMigrationRecoveryDisposition::ProjectReady(
            Self::reobservation(&plan, evidence),
        ))
    }

    async fn resume_project_reobserved(
        &self,
        plan: UnityMigrationPlanRecord,
        _writer: UnityWriterState,
        evidence: UnityMigrationEvidence,
    ) -> Result<UnityMigrationReobservation, M7UnityMigrationError> {
        self.record("resume_project_reobserved");
        Ok(Self::reobservation(&plan, evidence))
    }

    async fn cleanup(
        &self,
        _operation_id: OperationId,
        _plan: UnityMigrationPlanRecord,
        mut evidence: UnityMigrationEvidence,
    ) -> Result<UnityMigrationEvidence, M7UnityMigrationError> {
        self.record("cleanup");
        evidence.safe_evidence.push("cleanup_complete".to_owned());
        Ok(evidence)
    }
}

async fn registered_draft(store: &StateStoreHandle) -> (PrincipalId, UnityMigrationPlanDraft) {
    let owner = PrincipalId::local_owner();
    let project = store
        .register_project(owner.clone(), project(), key("worker-project"), 1)
        .await
        .expect("register Project")
        .value;
    let (target_installation, _) = store
        .register_installation(
            owner.clone(),
            target_installation(),
            key("worker-installation"),
            2,
        )
        .await
        .expect("register target Unity installation");
    let writer_evidence = UnityWriterState {
        project_id: project.project_id,
        state: UnityWriterStateKind::NotObserved,
        evidence: Vec::new(),
        checked_at_ms: 3,
    };
    let draft = UnityMigrationPlanDraft {
        plan_id: PlanId::new(),
        source_unity_version: project.observation.unity_version.clone(),
        source_revision_metadata: project.observation.unity_revision.clone(),
        project_root_identity: project.observation.path_identity_key.clone(),
        project_version_marker_sha256: [7; 32],
        target_unity_version: target_installation.observation.unity_version.clone(),
        target_revision_metadata: Some("abcdef012345".to_owned()),
        target_installation,
        writer_evidence,
        classification: UnityMigrationClassificationKind::MajorUpgrade,
        preparation_profile: Some("vrchat-2019-to-2022-v1".to_owned()),
        plan_fingerprint: "sealed-plan-fingerprint".to_owned(),
        request_fingerprint: r#"{"version":1,"request":"worker"}"#.to_owned(),
        plan_idempotency_key: key("worker-plan"),
        created_at_ms: 10,
        expires_at_ms: u64::MAX / 2,
        project,
    };
    (owner, draft)
}

fn project() -> ProjectObservation {
    ProjectObservation {
        root_path: "C:/fixture/project".to_owned(),
        path_identity_key: vec![1; 24],
        project_type: ProjectType::Avatars,
        unity_version: "2019.4.31f1".to_owned(),
        unity_revision: Some("bd5abf232a62".to_owned()),
        vpm_manifest: ManifestState::Valid,
        upm_manifest: ManifestState::Valid,
        direct_dependencies: Vec::new(),
        locked_dependencies: Vec::new(),
        issues: Vec::new(),
        observed_at_ms: 1,
    }
}

fn target_installation() -> UnityInstallationObservation {
    UnityInstallationObservation {
        executable_path: "C:/fixture/Unity.exe".to_owned(),
        filesystem_identity: vec![2; 24],
        unity_version: "2022.3.22f1".to_owned(),
        architecture: UnityArchitecture::Unknown,
        source_kind: UnitySourceKind::Manual,
        observed_at_ms: 2,
    }
}

fn recovery_evidence(
    draft: &UnityMigrationPlanDraft,
    operation_id: OperationId,
) -> UnityMigrationEvidence {
    UnityMigrationEvidence {
        preparation_kind: "vrchat-2019-to-2022-v1".to_owned(),
        preparation_complete: true,
        preparation_operation_id: Some(operation_id),
        preparation_artifact_sha256: Some([6; 32]),
        spawn_accepted: true,
        exit_observation: Some("success".to_owned()),
        reobserved_version: Some(draft.target_unity_version.clone()),
        reobserved_root_identity: Some(draft.project_root_identity.clone()),
        reobserved_marker_sha256: Some([8; 32]),
        writer_inactive_checked_at_ms: Some(200),
        reobserved_at_ms: Some(200),
        safe_terminal_failure: false,
        safe_evidence: vec!["sealed_recovery_evidence".to_owned()],
    }
}

fn recovery_prefix(phase: UnityMigrationPhase) -> Vec<UnityMigrationPhase> {
    let ordered = [
        UnityMigrationPhase::PreflightComplete,
        UnityMigrationPhase::PreparationIntent,
        UnityMigrationPhase::PreparationComplete,
        UnityMigrationPhase::LaunchIntent,
        UnityMigrationPhase::UnityStarted,
        UnityMigrationPhase::ProjectReobserved,
    ];
    let end = ordered
        .iter()
        .position(|candidate| *candidate == phase)
        .map_or(ordered.len(), |index| index + 1);
    ordered[..end].to_vec()
}

fn target_observation(draft: &UnityMigrationPlanDraft) -> ProjectObservation {
    let mut observation = draft.project.observation.clone();
    observation.unity_version = draft.target_unity_version.clone();
    observation.unity_revision = draft.target_revision_metadata.clone();
    observation.observed_at_ms = 200;
    observation
}

fn key(value: &str) -> IdempotencyKey {
    IdempotencyKey::parse(value).expect("valid idempotency key")
}

async fn wait_until(timeout: Duration, predicate: impl Fn() -> bool) {
    tokio::time::timeout(timeout, async {
        loop {
            if predicate() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("condition reached before timeout");
}

async fn operation_state(store: &StateStoreHandle, operation_id: OperationId) -> OperationState {
    store
        .list_operations(PrincipalId::local_owner(), None, 100)
        .await
        .expect("list Operations")
        .operations
        .into_iter()
        .find(|operation| operation.operation_id == operation_id)
        .expect("migration Operation")
        .state
}

async fn wait_for_operation_state(
    store: &StateStoreHandle,
    operation_id: OperationId,
    expected: OperationState,
    timeout: Duration,
) {
    tokio::time::timeout(timeout, async {
        loop {
            if operation_state(store, operation_id).await == expected {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("Operation reached expected state before timeout");
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("alcomd-m7-unity-worker-{}", Uuid::new_v4()));
        fs::create_dir(&path).expect("create test directory");
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
