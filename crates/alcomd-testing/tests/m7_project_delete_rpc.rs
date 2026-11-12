use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use alcomd_application::{
    AccessContext, IdempotencyKey, M3RegistryStore, M5BackupError, M5BackupWriterGate,
    M7DeleteAdapter, M7DeleteApplication, M7DeleteStore, ManifestState, OperationState,
    PrincipalId, ProjectDeleteFailureDisposition, ProjectDeletePhase, ProjectId,
    ProjectObservation, ProjectType, ResourceLockCoordinator, StateStore, UnityWriterState,
    UnityWriterStateKind,
};
use alcomd_client::{AlcomdClient, ClientConfig, ClientError};
use alcomd_platform::{DataConfig, IpcConfig};
use alcomd_store::StateStoreHandle;
use alcomd_vpm::ProjectDeleteEngine;

static RPC_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

const CRASH_ROOT_ENV: &str = "ALCOMD_TEST_PROJECT_DELETE_CRASH_ROOT";
const KILL_GATE_ENV: &str = "ALCOMD_TEST_PROJECT_DELETE_KILL_GATE";
const KILL_SIGNAL_ENV: &str = "ALCOMD_TEST_PROJECT_DELETE_KILL_SIGNAL";
const OPERATION_SIGNAL_ENV: &str = "ALCOMD_TEST_PROJECT_DELETE_OPERATION_SIGNAL";
const PAUSE_GATE_ENV: &str = "ALCOMD_TEST_PROJECT_DELETE_PAUSE_GATE";
const PAUSE_SIGNAL_ENV: &str = "ALCOMD_TEST_PROJECT_DELETE_PAUSE_SIGNAL";
const PAUSE_RELEASE_ENV: &str = "ALCOMD_TEST_PROJECT_DELETE_PAUSE_RELEASE";
const CHECKPOINT_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Clone, Copy, PartialEq)]
enum MissingTargetCase {
    Fresh,
    Legacy,
    MovedWithoutCheckpoint,
    MovedAndLost,
}

#[tokio::test]
async fn project_delete_missing_fresh_target_ends_failed_and_never_retries_a_recreated_root() {
    missing_target_case(MissingTargetCase::Fresh).await;
}

#[tokio::test]
async fn project_delete_missing_legacy_target_ends_failed_instead_of_waiting_forever() {
    missing_target_case(MissingTargetCase::Legacy).await;
}

#[tokio::test]
async fn project_delete_missing_source_with_unrecorded_payload_still_recovers_forward() {
    missing_target_case(MissingTargetCase::MovedWithoutCheckpoint).await;
}

#[tokio::test]
async fn project_delete_missing_previously_observed_payload_remains_recoverable() {
    missing_target_case(MissingTargetCase::MovedAndLost).await;
}

async fn missing_target_case(case: MissingTargetCase) {
    let fixture = TestDirectory::new();
    let root = fixture.path().join("MissingProject");
    create_project(&root);
    let store = StateStoreHandle::open(fixture.path().join("state.db")).expect("isolated store");
    let access = AccessContext::local_owner();
    let record = store
        .register_project(
            access.principal().clone(),
            observation(&root),
            key("missing-register"),
            1,
        )
        .await
        .expect("register")
        .value;
    let app = M7DeleteApplication::with_locks(
        store.clone(),
        ProjectDeleteEngine,
        FixedWriter(UnityWriterStateKind::NotObserved),
        Arc::new(ResourceLockCoordinator::default()),
    );
    let plan = app
        .plan_delete(
            &access,
            record.project_id,
            record.revision,
            key("missing-plan"),
        )
        .await
        .expect("plan");
    let now = plan.plan.draft.created_at_ms + 1;
    let accepted = alcomd_application::with_log_caller(
        store.clone(),
        &access,
        store.accept_project_delete(
            access.principal().clone(),
            plan.plan.draft.plan_id,
            record.revision,
            key("missing-apply"),
            now,
        ),
    )
    .await
    .expect("accept without dispatch");
    store
        .begin_project_delete(accepted.operation_id, now + 1)
        .await
        .expect("begin");
    let evidence = ProjectDeleteEngine
        .preflight(accepted.operation_id, plan.plan.clone())
        .await
        .expect("owned preparation");
    for (phase, offset) in [
        (ProjectDeletePhase::PreflightComplete, 2),
        (ProjectDeletePhase::QuarantineIntent, 3),
    ] {
        store
            .record_project_delete_checkpoint(
                accepted.operation_id,
                phase,
                evidence.clone(),
                now + offset,
            )
            .await
            .expect("checkpoint");
    }
    if matches!(
        case,
        MissingTargetCase::MovedWithoutCheckpoint | MissingTargetCase::MovedAndLost
    ) {
        let moved = ProjectDeleteEngine
            .quarantine(accepted.operation_id, plan.plan.clone(), evidence.clone())
            .await
            .expect("real move");
        if case == MissingTargetCase::MovedAndLost {
            store
                .record_project_delete_checkpoint(
                    accepted.operation_id,
                    ProjectDeletePhase::RootQuarantined,
                    moved,
                    now + 4,
                )
                .await
                .expect("observed move");
            let payload = Path::new(&evidence.quarantine_locator).join("payload");
            let resolved_payload = payload
                .canonicalize()
                .expect("resolve existing fixture payload");
            let fixture_root = fixture.path().canonicalize().expect("resolve fixture root");
            assert!(resolved_payload.is_absolute() && resolved_payload.starts_with(fixture_root));
            fs::remove_dir_all(resolved_payload)
                .expect("simulate external loss of fixture payload");
        }
    } else {
        assert!(root.is_absolute() && root.starts_with(fixture.path()));
        fs::remove_dir_all(&root).expect("simulate external removal of the fixture source");
    }
    let old_diagnostic = alcomd_application::OperationId::new().to_string();
    if case != MissingTargetCase::Fresh {
        alcomd_application::with_operation_log_failure(
            accepted.operation_id,
            "old recovery error".into(),
            store.fail_project_delete(
                accepted.operation_id,
                "project_delete_recovery_required".into(),
                old_diagnostic.clone(),
                if case == MissingTargetCase::MovedAndLost {
                    ProjectDeleteFailureDisposition::SourceAndPayloadMissing
                } else {
                    ProjectDeleteFailureDisposition::Uncertain
                },
                now + 5,
            ),
        )
        .await
        .expect("persist old recovery state without overriding an observed move");
    }
    app.recover().await.expect("recover durable work");
    if case == MissingTargetCase::MovedAndLost {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let operation = store
                .get_operation(access.principal().clone(), accepted.operation_id)
                .await
                .expect("operation");
            if operation.state == OperationState::Recovering
                && operation
                    .diagnostic_id
                    .as_deref()
                    .is_some_and(|id| id != old_diagnostic)
            {
                assert!(operation.completed_at_ms.is_none());
                break;
            }
            assert!(
                Instant::now() < deadline,
                "recovery classification completed"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(
            store
                .get_project(access.principal().clone(), record.project_id)
                .await
                .is_ok()
        );
        return;
    }
    let operation = wait_terminal(&store, accepted.operation_id).await;
    if case == MissingTargetCase::MovedWithoutCheckpoint {
        assert_eq!(operation.state, OperationState::Succeeded);
        assert!(
            store
                .get_project(access.principal().clone(), record.project_id)
                .await
                .is_err()
        );
        return;
    }
    assert_eq!(operation.state, OperationState::Failed);
    assert_eq!(
        operation.error_code.as_deref(),
        Some("project_delete_source_missing")
    );
    assert!(operation.completed_at_ms.is_some());
    assert!(
        !store
            .has_unfinished_operations()
            .await
            .expect("global lifecycle query")
    );
    assert_eq!(
        store
            .get_project(access.principal().clone(), record.project_id)
            .await
            .expect("registration preserved")
            .revision,
        record.revision
    );
    let events = store
        .list_events(access.principal().clone(), 0, 100)
        .await
        .expect("events");
    assert!(
        !events
            .events
            .iter()
            .any(|event| event.kind == "project.directory_deleted")
    );
    let logs = alcomd_application::OperationLogStore::read_operation_logs(
        &store,
        access.principal().clone(),
        alcomd_application::OperationLogQuery {
            limit: 100,
            ..Default::default()
        },
    )
    .await
    .expect("logs");
    assert!(
        logs.entries
            .iter()
            .any(
                |entry| entry.operation_id.as_deref() == Some(&accepted.operation_id.to_string())
                    && entry.outcome == alcomd_application::LogOutcome::Failed
            )
    );
    create_project(&root);
    fs::write(root.join("replacement.txt"), b"keep replacement").expect("replacement marker");
    app.recover().await.expect("restart after terminal failure");
    assert_eq!(
        fs::read(root.join("replacement.txt")).expect("replacement survives"),
        b"keep replacement"
    );
    assert_eq!(
        store
            .get_operation(access.principal().clone(), accepted.operation_id)
            .await
            .expect("failed task unchanged")
            .state,
        OperationState::Failed
    );
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DeleteCrashSignal {
    plan_id: String,
    operation_id: String,
    project_id: String,
}

#[derive(Clone, Copy)]
struct FixedWriter(UnityWriterStateKind);

impl M5BackupWriterGate for FixedWriter {
    async fn observe_backup_source(
        &self,
        _: &AccessContext,
        project_id: ProjectId,
    ) -> Result<UnityWriterState, M5BackupError> {
        Ok(UnityWriterState {
            project_id,
            state: self.0,
            evidence: Vec::new(),
            checked_at_ms: 1,
        })
    }
}

#[tokio::test]
async fn project_delete_plan_apply_removes_registry_and_exact_directory_once() {
    let fixture = TestDirectory::new();
    let project_root = fixture.path().join("Project");
    create_project(&project_root);
    fs::write(project_root.join("sentinel.txt"), b"delete me").expect("sentinel");
    let store = StateStoreHandle::open(fixture.path().join("state.db")).expect("store");
    let record = store
        .register_project(
            PrincipalId::local_owner(),
            observation(&project_root),
            key("delete-register"),
            1,
        )
        .await
        .expect("register")
        .value;
    let application = M7DeleteApplication::with_locks(
        store.clone(),
        ProjectDeleteEngine,
        FixedWriter(UnityWriterStateKind::NotObserved),
        Arc::new(ResourceLockCoordinator::default()),
    );
    let access = AccessContext::local_owner();
    let plan = application
        .plan_delete(
            &access,
            record.project_id,
            record.revision,
            key("delete-plan"),
        )
        .await
        .expect("plan");
    assert_eq!(plan.plan.draft.normalized_leaf, "Project");
    let accepted = application
        .apply_delete(
            &access,
            plan.plan.draft.plan_id,
            record.revision,
            key("delete-apply"),
        )
        .await
        .expect("apply");
    let replay = application
        .apply_delete(
            &access,
            plan.plan.draft.plan_id,
            record.revision,
            key("delete-apply"),
        )
        .await
        .expect("replay");
    assert_eq!(replay.operation_id, accepted.operation_id);
    assert!(replay.replayed);
    let operation = wait_terminal(&store, accepted.operation_id).await;
    assert_eq!(operation.state, OperationState::Succeeded);
    assert!(!project_root.exists());
    assert!(
        store
            .get_project(PrincipalId::local_owner(), record.project_id)
            .await
            .is_err()
    );
    let events = store
        .list_events(PrincipalId::local_owner(), 0, 100)
        .await
        .expect("events");
    assert_eq!(
        events
            .events
            .iter()
            .filter(|event| event.kind == "project.directory_deleted")
            .count(),
        1
    );
    assert_eq!(
        events
            .events
            .iter()
            .filter(|event| event.kind == "project.unregistered")
            .count(),
        0
    );
    assert!(
        fs::read_dir(fixture.path())
            .expect("fixture entries")
            .all(|entry| !entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .starts_with(".alcomd-delete-"))
    );
}

#[cfg(windows)]
#[tokio::test]
async fn project_delete_sharing_violation_fails_without_moving_or_retrying_the_source() {
    sharing_violation_case(false, false).await;
}

#[cfg(windows)]
#[tokio::test]
async fn project_delete_old_unmoved_recovery_becomes_failed_without_retrying_after_unlock() {
    sharing_violation_case(true, false).await;
}

#[cfg(windows)]
#[tokio::test]
async fn project_delete_unmoved_failure_does_not_wait_for_empty_preparation_cleanup() {
    sharing_violation_case(true, true).await;
}

#[cfg(windows)]
async fn sharing_violation_case(legacy_recovery: bool, preparation_locked: bool) {
    use std::os::windows::fs::OpenOptionsExt;
    let fixture = TestDirectory::new();
    let root = fixture.path().join("LockedProject");
    create_project(&root);
    fs::write(root.join("sentinel.txt"), b"keep project").expect("sentinel");
    let held = fs::OpenOptions::new()
        .read(true)
        .share_mode(3)
        .custom_flags(0x0200_0000)
        .open(&root)
        .expect("hold a directory without sharing DELETE");
    let store = StateStoreHandle::open(fixture.path().join("state.db")).expect("store");
    let access = AccessContext::local_owner();
    let record = store
        .register_project(
            PrincipalId::local_owner(),
            observation(&root),
            key("locked-register"),
            1,
        )
        .await
        .expect("register")
        .value;
    let app = M7DeleteApplication::with_locks(
        store.clone(),
        ProjectDeleteEngine,
        FixedWriter(UnityWriterStateKind::NotObserved),
        Arc::new(ResourceLockCoordinator::default()),
    );
    let plan = app
        .plan_delete(
            &access,
            record.project_id,
            record.revision,
            key("locked-plan"),
        )
        .await
        .expect("plan");
    let mut held_preparation = None;
    let accepted = if legacy_recovery {
        let now = plan.plan.draft.created_at_ms + 1;
        let accepted = alcomd_application::with_log_caller(
            store.clone(),
            &access,
            store.accept_project_delete(
                access.principal().clone(),
                plan.plan.draft.plan_id,
                record.revision,
                key("legacy-accept"),
                now,
            ),
        )
        .await
        .expect("legacy accepted task");
        store
            .begin_project_delete(accepted.operation_id, now + 1)
            .await
            .expect("legacy begin");
        let evidence = ProjectDeleteEngine
            .preflight(accepted.operation_id, plan.plan.clone())
            .await
            .expect("owned preparation");
        if preparation_locked {
            held_preparation = Some(
                fs::OpenOptions::new()
                    .read(true)
                    .share_mode(3)
                    .custom_flags(0x0200_0000)
                    .open(&evidence.quarantine_locator)
                    .expect("hold empty preparation"),
            );
        }
        store
            .record_project_delete_checkpoint(
                accepted.operation_id,
                ProjectDeletePhase::PreflightComplete,
                evidence.clone(),
                now + 2,
            )
            .await
            .expect("preflight");
        store
            .record_project_delete_checkpoint(
                accepted.operation_id,
                ProjectDeletePhase::QuarantineIntent,
                evidence,
                now + 3,
            )
            .await
            .expect("intent");
        alcomd_application::with_operation_log_failure(
            accepted.operation_id,
            "old sharing failure (os error 32)".into(),
            store.fail_project_delete(
                accepted.operation_id,
                "project_delete_recovery_required".into(),
                alcomd_application::OperationId::new().to_string(),
                ProjectDeleteFailureDisposition::Uncertain,
                now + 4,
            ),
        )
        .await
        .expect("previous software leaves recovery pending");
        app.recover().await.expect("recover existing stuck task");
        accepted
    } else {
        alcomd_application::with_log_caller(
            store.clone(),
            &access,
            app.apply_delete(
                &access,
                plan.plan.draft.plan_id,
                record.revision,
                key("locked-apply"),
            ),
        )
        .await
        .expect("accepted task")
    };
    let operation = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let operation = store
                .get_operation(access.principal().clone(), accepted.operation_id)
                .await
                .expect("operation");
            if operation.state.is_terminal() {
                break operation;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("sharing failure must end rather than wait for recovery");
    assert_eq!(operation.state, OperationState::Failed);
    assert_eq!(
        operation.error_code.as_deref(),
        Some("project_delete_source_unsafe")
    );
    assert!(operation.completed_at_ms.is_some());
    assert_eq!(
        store
            .get_project(PrincipalId::local_owner(), record.project_id)
            .await
            .expect("registration survives")
            .revision,
        record.revision
    );
    assert_eq!(
        fs::read(root.join("sentinel.txt")).expect("original contents"),
        b"keep project"
    );
    let wrapper = fixture.path().join(format!(
        ".alcomd-delete-{}.quarantine",
        accepted.operation_id
    ));
    if preparation_locked {
        assert!(
            wrapper.exists(),
            "an occupied empty preparation may remain without keeping deletion active"
        );
        assert!(!wrapper.join("payload").exists());
    } else {
        assert!(
            !wrapper.exists(),
            "only the empty owned preparation directory is discarded"
        );
    }
    drop(held_preparation);
    drop(held);
    app.recover().await.expect("startup recovery");
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert!(
        root.exists(),
        "a failed delete is never retried after the lock is released"
    );
    let logs = alcomd_application::OperationLogStore::read_operation_logs(
        &store,
        access.principal().clone(),
        alcomd_application::OperationLogQuery {
            limit: 100,
            ..Default::default()
        },
    )
    .await
    .expect("logs");
    assert!(logs.entries.iter().any(|entry| {
        entry.operation_id.as_deref() == Some(&accepted.operation_id.to_string())
            && entry.outcome == alcomd_application::LogOutcome::Failed
            && entry
                .details
                .as_deref()
                .is_some_and(|details| details.contains("os error 32"))
    }));
    if preparation_locked {
        assert!(logs.entries.iter().any(|entry| {
            entry
                .details
                .as_deref()
                .is_some_and(|details| details.contains("preparation cleanup failed"))
        }));
    }
}

#[tokio::test]
async fn project_delete_already_quarantined_failure_stays_recoverable_and_preserves_recreated_root()
{
    let fixture = TestDirectory::new();
    let root = fixture.path().join("Project");
    create_project(&root);
    let store = StateStoreHandle::open(fixture.path().join("state.db")).expect("store");
    let access = AccessContext::local_owner();
    let record = store
        .register_project(
            access.principal().clone(),
            observation(&root),
            key("boundary-register"),
            1,
        )
        .await
        .expect("register")
        .value;
    let app = M7DeleteApplication::with_locks(
        store.clone(),
        ProjectDeleteEngine,
        FixedWriter(UnityWriterStateKind::NotObserved),
        Arc::new(ResourceLockCoordinator::default()),
    );
    let plan = app
        .plan_delete(
            &access,
            record.project_id,
            record.revision,
            key("boundary-plan"),
        )
        .await
        .expect("plan");
    let now = plan.plan.draft.created_at_ms + 1;
    let accepted = alcomd_application::with_log_caller(
        store.clone(),
        &access,
        store.accept_project_delete(
            access.principal().clone(),
            plan.plan.draft.plan_id,
            record.revision,
            key("boundary-accept"),
            now,
        ),
    )
    .await
    .expect("accept without dispatch");
    store
        .begin_project_delete(accepted.operation_id, now + 1)
        .await
        .expect("begin");
    let evidence = ProjectDeleteEngine
        .preflight(accepted.operation_id, plan.plan.clone())
        .await
        .expect("preflight");
    store
        .record_project_delete_checkpoint(
            accepted.operation_id,
            ProjectDeletePhase::PreflightComplete,
            evidence.clone(),
            now + 2,
        )
        .await
        .expect("preflight checkpoint");
    store
        .record_project_delete_checkpoint(
            accepted.operation_id,
            ProjectDeletePhase::QuarantineIntent,
            evidence.clone(),
            now + 3,
        )
        .await
        .expect("intent");
    let evidence = ProjectDeleteEngine
        .quarantine(accepted.operation_id, plan.plan, evidence)
        .await
        .expect("real move");
    store
        .record_project_delete_checkpoint(
            accepted.operation_id,
            ProjectDeletePhase::RootQuarantined,
            evidence,
            now + 4,
        )
        .await
        .expect("quarantine checkpoint");
    create_project(&root);
    fs::write(root.join("replacement.txt"), b"keep replacement").expect("new original-path object");
    alcomd_application::with_operation_log_failure(
        accepted.operation_id,
        "fixture cleanup interruption".into(),
        store.fail_project_delete(
            accepted.operation_id,
            "project_delete_source_unsafe".into(),
            alcomd_application::OperationId::new().to_string(),
            ProjectDeleteFailureDisposition::SourceUnmoved,
            now + 5,
        ),
    )
    .await
    .expect("Store refuses terminal failure after a recorded move");
    let operation = store
        .get_operation(access.principal().clone(), accepted.operation_id)
        .await
        .expect("operation");
    assert_eq!(operation.state, OperationState::Recovering);
    assert!(operation.completed_at_ms.is_none());
    let logs = alcomd_application::OperationLogStore::read_operation_logs(
        &store,
        access.principal().clone(),
        alcomd_application::OperationLogQuery {
            limit: 100,
            ..Default::default()
        },
    )
    .await
    .expect("logs");
    assert!(
        logs.entries
            .iter()
            .any(
                |entry| entry.operation_id.as_deref() == Some(&accepted.operation_id.to_string())
                    && entry.outcome == alcomd_application::LogOutcome::Waiting
                    && entry.details.as_deref() == Some("fixture cleanup interruption")
            )
    );
    app.recover().await.expect("forward recovery");
    assert_eq!(
        wait_terminal(&store, accepted.operation_id).await.state,
        OperationState::Succeeded
    );
    assert_eq!(
        fs::read(root.join("replacement.txt")).expect("replacement survives"),
        b"keep replacement"
    );
}

#[tokio::test]
async fn project_delete_rejects_target_linked_writers() {
    for (index, state) in [
        UnityWriterStateKind::RunningConfirmed,
        UnityWriterStateKind::RunningSuspected,
    ]
    .into_iter()
    .enumerate()
    {
        let fixture = TestDirectory::new();
        let project_root = fixture.path().join("Project");
        create_project(&project_root);
        let store = StateStoreHandle::open(fixture.path().join("state.db")).expect("store");
        let record = store
            .register_project(
                PrincipalId::local_owner(),
                observation(&project_root),
                key(&format!("writer-register-{index}")),
                1,
            )
            .await
            .expect("register")
            .value;
        let application = M7DeleteApplication::with_locks(
            store,
            ProjectDeleteEngine,
            FixedWriter(state),
            Arc::new(ResourceLockCoordinator::default()),
        );
        assert!(
            application
                .plan_delete(
                    &AccessContext::local_owner(),
                    record.project_id,
                    record.revision,
                    key(&format!("writer-plan-{index}")),
                )
                .await
                .is_err()
        );
        assert!(project_root.exists());
    }
}

#[tokio::test]
async fn project_delete_allows_unattributed_unknown_but_rechecks_target_before_quarantine() {
    use std::sync::atomic::AtomicUsize;
    #[derive(Clone)]
    struct Writer {
        calls: Arc<AtomicUsize>,
        becomes_running: bool,
    }
    impl M5BackupWriterGate for Writer {
        async fn observe_backup_source(
            &self,
            _: &AccessContext,
            project_id: ProjectId,
        ) -> Result<UnityWriterState, M5BackupError> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(UnityWriterState {
                project_id,
                state: if self.becomes_running && call >= 2 {
                    UnityWriterStateKind::RunningConfirmed
                } else {
                    UnityWriterStateKind::Unknown
                },
                evidence: Vec::new(),
                checked_at_ms: 1,
            })
        }
    }
    for becomes_running in [false, true] {
        let fixture = TestDirectory::new();
        let root = fixture.path().join("Project");
        create_project(&root);
        let store = StateStoreHandle::open(fixture.path().join("state.db")).expect("store");
        let record = store
            .register_project(
                PrincipalId::local_owner(),
                observation(&root),
                key("unknown-register"),
                1,
            )
            .await
            .expect("register")
            .value;
        let writer = Writer {
            calls: Arc::new(AtomicUsize::new(0)),
            becomes_running,
        };
        let calls = writer.calls.clone();
        let app = M7DeleteApplication::with_locks(
            store.clone(),
            ProjectDeleteEngine,
            writer,
            Arc::new(ResourceLockCoordinator::default()),
        );
        let plan = app
            .plan_delete(
                &AccessContext::local_owner(),
                record.project_id,
                record.revision,
                key("unknown-plan"),
            )
            .await
            .expect("unknown is not target occupancy");
        let accepted = app
            .apply_delete(
                &AccessContext::local_owner(),
                plan.plan.draft.plan_id,
                record.revision,
                key("unknown-apply"),
            )
            .await
            .expect("apply");
        let operation = wait_terminal(&store, accepted.operation_id).await;
        assert!(calls.load(Ordering::SeqCst) >= 3);
        if becomes_running {
            assert_eq!(operation.state, OperationState::Failed);
            assert!(root.exists());
            assert!(
                fs::read_dir(fixture.path())
                    .expect("fixture entries")
                    .all(|entry| !entry
                        .expect("entry")
                        .file_name()
                        .to_string_lossy()
                        .starts_with(".alcomd-delete-"))
            );
            assert!(
                store
                    .get_project(PrincipalId::local_owner(), record.project_id)
                    .await
                    .is_ok()
            );
        } else {
            assert_eq!(operation.state, OperationState::Succeeded);
            assert!(!root.exists());
            assert!(
                store
                    .get_project(PrincipalId::local_owner(), record.project_id)
                    .await
                    .is_err()
            );
        }
    }
}

#[tokio::test]
async fn project_delete_rejects_a_replaced_root_and_preserves_both_objects() {
    let fixture = TestDirectory::new();
    let project_root = fixture.path().join("Project");
    let original_root = fixture.path().join("OriginalProject");
    create_project(&project_root);
    fs::write(project_root.join("original-sentinel.txt"), b"original").expect("sentinel");
    let store = StateStoreHandle::open(fixture.path().join("state.db")).expect("store");
    let record = store
        .register_project(
            PrincipalId::local_owner(),
            observation(&project_root),
            key("replacement-register"),
            1,
        )
        .await
        .expect("register")
        .value;
    let application = M7DeleteApplication::with_locks(
        store,
        ProjectDeleteEngine,
        FixedWriter(UnityWriterStateKind::NotObserved),
        Arc::new(ResourceLockCoordinator::default()),
    );
    let plan = application
        .plan_delete(
            &AccessContext::local_owner(),
            record.project_id,
            record.revision,
            key("replacement-plan"),
        )
        .await
        .expect("plan");
    fs::rename(&project_root, &original_root).expect("move original object");
    create_project(&project_root);
    fs::write(
        project_root.join("replacement-sentinel.txt"),
        b"replacement",
    )
    .expect("replacement sentinel");

    let failure = application
        .apply_delete(
            &AccessContext::local_owner(),
            plan.plan.draft.plan_id,
            record.revision,
            key("replacement-apply"),
        )
        .await
        .expect_err("replaced root must fail closed");
    assert_eq!(
        failure.code(),
        alcomd_application::M7DeleteErrorCode::ProjectDeleteSourceChanged
    );
    assert_eq!(
        fs::read(original_root.join("original-sentinel.txt")).expect("original survives"),
        b"original"
    );
    assert_eq!(
        fs::read(project_root.join("replacement-sentinel.txt")).expect("replacement survives"),
        b"replacement"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn project_delete_recovers_from_every_durable_kill_checkpoint() {
    let _serial = RPC_TEST_LOCK.lock().await;
    for checkpoint in [
        "accepted",
        "preflight_complete",
        "quarantine_intent",
        "root_quarantined",
        "registry_commit_intent",
        "state_committed",
        "deleting",
        "cleanup_complete",
    ] {
        run_kill_restart_case(checkpoint).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn project_delete_cancellation_stops_before_and_is_rejected_at_quarantine_intent() {
    let _serial = RPC_TEST_LOCK.lock().await;
    run_cancel_case("preflight_complete", true).await;
    run_cancel_case("quarantine_intent", false).await;
}

#[test]
#[ignore = "subprocess fixture invoked by Project Delete kill/restart matrix"]
fn subprocess_runs_project_delete_until_parent_kills_it() {
    let fixture = PathBuf::from(std::env::var_os(CRASH_ROOT_ENV).expect("crash root"));
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    runtime.block_on(async move {
        let project = fixture.join("Project");
        fs::create_dir_all(fixture.join("runtime")).expect("runtime root");
        fs::create_dir_all(fixture.join("data")).expect("data root");
        create_project(&project);
        fs::write(project.join("sentinel.txt"), b"delete me").expect("sentinel");
        let (ipc, config) = isolated_ipc(fixture.join("runtime"));
        let shutdown = Arc::new(AtomicBool::new(false));
        let server_shutdown = Arc::clone(&shutdown);
        let data = fixture.join("data");
        let _daemon = alcomd_daemon::test_support::start_with_data_until(
            ipc,
            DataConfig::isolated(data),
            wait_for_shutdown(server_shutdown),
        )
        .await
        .expect("start owned test daemon");
        let mut client = connect_client(config).await;
        let registered = client
            .project_register(
                project.to_string_lossy().into_owned(),
                "m7-delete-kill-register".to_owned(),
            )
            .await
            .expect("register project");
        let project_id = registered.project.project_id.expect("project ID");
        let plan = client
            .project_plan_delete_directory(alcomd_protocol::ProjectsPlanDeleteDirectoryParams {
                project_id: project_id.clone(),
                expected_revision: 1,
                idempotency_key: "m7-delete-kill-plan".to_owned(),
            })
            .await
            .expect("plan delete");
        let accepted = client
            .project_apply_delete_directory(alcomd_protocol::ProjectsApplyDeleteDirectoryParams {
                plan_id: plan.plan.plan_id.clone(),
                expected_revision: 1,
                idempotency_key: "m7-delete-kill-apply".to_owned(),
            })
            .await
            .expect("apply delete");
        let signal = DeleteCrashSignal {
            plan_id: plan.plan.plan_id,
            operation_id: accepted.operation_id,
            project_id,
        };
        fs::write(
            std::env::var_os(OPERATION_SIGNAL_ENV).expect("operation signal"),
            serde_json::to_vec(&signal).expect("serialize operation signal"),
        )
        .expect("write operation signal");
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });
}

async fn run_kill_restart_case(checkpoint: &str) {
    let fixture = TestDirectory::new();
    let kill_signal = fixture.path().join("kill.signal");
    let operation_signal = fixture.path().join("operation.signal");
    let child = Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "subprocess_runs_project_delete_until_parent_kills_it",
            "--ignored",
            "--nocapture",
        ])
        .env(CRASH_ROOT_ENV, fixture.path())
        .env(KILL_GATE_ENV, checkpoint)
        .env(KILL_SIGNAL_ENV, &kill_signal)
        .env(OPERATION_SIGNAL_ENV, &operation_signal)
        .env("ALCOMD_TEST_PROJECT_DELETE_WRITER_STATE", "not_observed")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn Project Delete subprocess");
    let mut child = ChildGuard(Some(child));
    wait_for_file(&operation_signal, None, checkpoint, &mut child);
    wait_for_file(
        &kill_signal,
        Some(checkpoint.as_bytes()),
        checkpoint,
        &mut child,
    );
    let signal: DeleteCrashSignal =
        serde_json::from_slice(&fs::read(&operation_signal).expect("read operation signal"))
            .expect("parse operation signal");
    child.stop();

    let project = fixture.path().join("Project");
    let original_path_must_survive = matches!(
        checkpoint,
        "root_quarantined"
            | "registry_commit_intent"
            | "state_committed"
            | "deleting"
            | "cleanup_complete"
    );
    if original_path_must_survive {
        create_project(&project);
        fs::write(project.join("external-sentinel.txt"), b"preserve me")
            .expect("recreated path sentinel");
    }

    let (ipc, config) = isolated_ipc(fixture.path().join("runtime"));
    let mut daemon =
        TestDaemon::spawn(ipc, DataConfig::isolated(fixture.path().join("data"))).await;
    let mut client = connect_client(config).await;
    let replay = client
        .project_apply_delete_directory(alcomd_protocol::ProjectsApplyDeleteDirectoryParams {
            plan_id: signal.plan_id,
            expected_revision: 1,
            idempotency_key: "m7-delete-kill-apply".to_owned(),
        })
        .await
        .expect("replay Project Delete Apply");
    assert!(replay.replayed, "{checkpoint}");
    assert_eq!(replay.operation_id, signal.operation_id, "{checkpoint}");
    assert_eq!(replay.project_id, signal.project_id, "{checkpoint}");
    let operation = wait_for_terminal_rpc(&mut client, signal.operation_id).await;
    assert_eq!(
        operation.state,
        alcomd_protocol::OperationState::Succeeded,
        "{checkpoint}: {operation:?}"
    );
    assert!(
        client.project_get(signal.project_id).await.is_err(),
        "{checkpoint}"
    );
    if original_path_must_survive {
        assert_eq!(
            fs::read(project.join("external-sentinel.txt")).expect("recreated sentinel"),
            b"preserve me",
            "{checkpoint}"
        );
    } else {
        assert!(!project.exists(), "{checkpoint}");
    }
    assert!(
        fs::read_dir(fixture.path())
            .expect("fixture entries")
            .all(|entry| !entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .starts_with(".alcomd-delete-")),
        "{checkpoint}"
    );
    daemon.shutdown().await;
}

async fn run_cancel_case(checkpoint: &str, should_cancel: bool) {
    let fixture = TestDirectory::new();
    let pause_signal = fixture.path().join("pause.signal");
    let pause_release = fixture.path().join("pause.release");
    let operation_signal = fixture.path().join("operation.signal");
    let child = Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "subprocess_runs_project_delete_until_parent_kills_it",
            "--ignored",
            "--nocapture",
        ])
        .env(CRASH_ROOT_ENV, fixture.path())
        .env(PAUSE_GATE_ENV, checkpoint)
        .env(PAUSE_SIGNAL_ENV, &pause_signal)
        .env(PAUSE_RELEASE_ENV, &pause_release)
        .env(OPERATION_SIGNAL_ENV, &operation_signal)
        .env("ALCOMD_TEST_PROJECT_DELETE_WRITER_STATE", "not_observed")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn paused Project Delete subprocess");
    let mut child = ChildGuard(Some(child));
    wait_for_file(&operation_signal, None, checkpoint, &mut child);
    wait_for_file(
        &pause_signal,
        Some(checkpoint.as_bytes()),
        checkpoint,
        &mut child,
    );
    let signal: DeleteCrashSignal =
        serde_json::from_slice(&fs::read(&operation_signal).expect("read operation signal"))
            .expect("parse operation signal");
    let (_, config) = isolated_ipc(fixture.path().join("runtime"));
    let mut client = connect_client(config).await;
    let operation = client
        .operation_get(signal.operation_id.clone())
        .await
        .expect("paused operation");
    let cancellation = client
        .operation_cancel(
            signal.operation_id.clone(),
            operation.revision,
            format!("m7-delete-cancel-{checkpoint}"),
        )
        .await;
    if should_cancel {
        assert_eq!(
            cancellation
                .expect("pre-intent cancellation")
                .operation
                .state,
            alcomd_protocol::OperationState::Cancelling
        );
    } else {
        assert!(matches!(
            cancellation,
            Err(alcomd_client::ClientError::Remote(ref error))
                if error.code == "operation_not_cancellable"
        ));
    }
    fs::write(&pause_release, b"continue").expect("release pause gate");
    let terminal = wait_for_terminal_rpc(&mut client, signal.operation_id).await;
    if should_cancel {
        assert_eq!(terminal.state, alcomd_protocol::OperationState::Cancelled);
        assert!(fixture.path().join("Project").exists());
    } else {
        assert_eq!(terminal.state, alcomd_protocol::OperationState::Succeeded);
        assert!(!fixture.path().join("Project").exists());
    }
    child.stop();
}

async fn wait_terminal(
    store: &StateStoreHandle,
    operation_id: alcomd_application::OperationId,
) -> alcomd_application::OperationRecord {
    let started = Instant::now();
    loop {
        let operation = store
            .get_operation(PrincipalId::local_owner(), operation_id)
            .await
            .expect("operation");
        if operation.state.is_terminal() {
            return operation;
        }
        assert!(started.elapsed() < Duration::from_secs(20));
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

fn create_project(root: &Path) {
    fs::create_dir_all(root.join("ProjectSettings")).expect("project settings");
    fs::write(
        root.join("ProjectSettings").join("ProjectVersion.txt"),
        b"m_EditorVersion: 2022.3.22f1\nm_EditorVersionWithRevision: 2022.3.22f1 (abc)\n",
    )
    .expect("project version");
}

fn observation(root: &Path) -> ProjectObservation {
    let canonical = fs::canonicalize(root).expect("canonical project");
    ProjectObservation {
        root_path: canonical.to_string_lossy().into_owned(),
        path_identity_key: alcomd_platform::file_identity_key(&canonical).expect("identity"),
        project_type: ProjectType::Avatars,
        unity_version: "2022.3.22f1".to_owned(),
        unity_revision: Some("abc".to_owned()),
        vpm_manifest: ManifestState::Missing,
        upm_manifest: ManifestState::Missing,
        direct_dependencies: Vec::new(),
        locked_dependencies: Vec::new(),
        issues: Vec::new(),
    }
}

fn key(value: &str) -> IdempotencyKey {
    IdempotencyKey::parse(value).expect("key")
}

async fn wait_for_terminal_rpc(
    client: &mut AlcomdClient,
    operation_id: String,
) -> alcomd_protocol::Operation {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let operation = client
            .operation_get(operation_id.clone())
            .await
            .expect("get operation");
        if matches!(
            operation.state,
            alcomd_protocol::OperationState::Succeeded
                | alcomd_protocol::OperationState::Failed
                | alcomd_protocol::OperationState::Cancelled
        ) {
            return operation;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "Project Delete recovery timed out"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

async fn connect_client(config: ClientConfig) -> AlcomdClient {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        match AlcomdClient::connect(config.clone()).await {
            Ok(client) => return client,
            Err(ClientError::StartTimeout) if tokio::time::Instant::now() < deadline => {}
            Err(error) => panic!("client connection: {error}"),
        }
    }
}

async fn wait_for_shutdown(shutdown: Arc<AtomicBool>) {
    while !shutdown.load(Ordering::Acquire) {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[cfg(unix)]
fn isolated_ipc(runtime: PathBuf) -> (IpcConfig, ClientConfig) {
    (
        IpcConfig::isolated(runtime.clone()),
        ClientConfig::default()
            .without_daemon_start()
            .with_runtime_directory(runtime),
    )
}

#[cfg(windows)]
fn isolated_ipc(_runtime: PathBuf) -> (IpcConfig, ClientConfig) {
    (
        IpcConfig::default(),
        ClientConfig::default().without_daemon_start(),
    )
}

struct TestDaemon {
    shutdown: Arc<AtomicBool>,
    task: Option<alcomd_daemon::test_support::TestDaemon>,
}

impl TestDaemon {
    async fn spawn(ipc: IpcConfig, data: DataConfig) -> Self {
        let shutdown = Arc::new(AtomicBool::new(false));
        let server_shutdown = Arc::clone(&shutdown);
        let task = alcomd_daemon::test_support::start_with_data_until(
            ipc,
            data,
            wait_for_shutdown(server_shutdown),
        )
        .await
        .expect("start owned test daemon");
        Self {
            shutdown,
            task: Some(task),
        }
    }

    async fn shutdown(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        let task = self.task.as_mut().expect("daemon task");
        tokio::time::timeout(Duration::from_secs(30), task)
            .await
            .expect("daemon shutdown timeout")
            .expect("daemon join")
            .expect("daemon result");
        self.task = None;
    }
}

impl Drop for TestDaemon {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

struct ChildGuard(Option<Child>);

impl ChildGuard {
    fn stop(&mut self) {
        let child = self.0.as_mut().expect("child");
        child.kill().expect("kill Project Delete subprocess");
        wait_for_child(child, Duration::from_secs(30));
        self.0 = None;
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            wait_for_child(child, Duration::from_secs(5));
        }
    }
}

fn wait_for_child(child: &mut Child, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Ok(None) => panic!("Project Delete subprocess did not exit"),
            Err(error) => panic!("Project Delete subprocess status failed: {error}"),
        }
    }
}

fn wait_for_file(path: &Path, expected: Option<&[u8]>, checkpoint: &str, child: &mut ChildGuard) {
    let deadline = Instant::now() + CHECKPOINT_TIMEOUT;
    loop {
        if let Ok(bytes) = fs::read(path)
            && expected.is_none_or(|expected| bytes == expected)
        {
            return;
        }
        match child.0.as_mut().expect("child").try_wait() {
            Ok(Some(status)) => {
                child.0 = None;
                panic!("Project Delete subprocess exited before {checkpoint}: {status}");
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Ok(None) => panic!("Project Delete checkpoint timed out: {checkpoint}"),
            Err(error) => panic!("Project Delete subprocess status failed: {error}"),
        }
    }
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        #[cfg(target_os = "macos")]
        let base = PathBuf::from("/private/tmp");
        #[cfg(not(target_os = "macos"))]
        let base = std::env::temp_dir();
        let path = base.join(format!("alcomd-m7-delete-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).expect("fixture directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
