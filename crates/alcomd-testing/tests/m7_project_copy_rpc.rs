use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use alcomd_application::{
    AccessContext, IdempotencyKey, M3RegistryStore, M5BackupError, M5BackupWriterGate,
    M7CopyAdapter, M7CopyApplication, M7CopyErrorCode, M7CopyStore, ManifestState, PrincipalId,
    ProjectId, ProjectObservation, ProjectType, ResourceLockCoordinator, UnityWriterState,
    UnityWriterStateKind,
};
use alcomd_client::{AlcomdClient, ClientConfig, ClientError};
use alcomd_platform::{DataConfig, IpcConfig};
use alcomd_store::StateStoreHandle;
use alcomd_vpm::ProjectCopyEngine;

static RPC_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

const CRASH_ROOT_ENV: &str = "ALCOMD_TEST_PROJECT_COPY_CRASH_ROOT";
const KILL_GATE_ENV: &str = "ALCOMD_TEST_PROJECT_COPY_KILL_GATE";
const KILL_SIGNAL_ENV: &str = "ALCOMD_TEST_PROJECT_COPY_KILL_SIGNAL";
const OPERATION_SIGNAL_ENV: &str = "ALCOMD_TEST_PROJECT_COPY_OPERATION_SIGNAL";
const PAUSE_GATE_ENV: &str = "ALCOMD_TEST_PROJECT_COPY_PAUSE_GATE";
const PAUSE_SIGNAL_ENV: &str = "ALCOMD_TEST_PROJECT_COPY_PAUSE_SIGNAL";
const PAUSE_RELEASE_ENV: &str = "ALCOMD_TEST_PROJECT_COPY_PAUSE_RELEASE";

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CopyCrashSignal {
    plan_id: String,
    operation_id: String,
    target_project_id: String,
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
async fn project_copy_plan_expiry_and_all_writer_states_are_fail_closed() {
    let fixture = TestDirectory::new();
    let source = fixture.path().join("SourceProject");
    let destination = fixture.path().join("Copies");
    create_project(&source);
    fs::create_dir(&destination).expect("destination");
    let store = StateStoreHandle::open(fixture.path().join("state.db")).expect("open store");
    let project = store
        .register_project(
            PrincipalId::local_owner(),
            project_observation(&source),
            key("m7-copy-writer-project"),
            1,
        )
        .await
        .expect("register source")
        .value;
    let access = AccessContext::local_owner();

    for (index, state) in [
        UnityWriterStateKind::RunningSuspected,
        UnityWriterStateKind::Unknown,
        UnityWriterStateKind::NotObserved,
    ]
    .into_iter()
    .enumerate()
    {
        let application = M7CopyApplication::with_locks(
            store.clone(),
            ProjectCopyEngine,
            FixedWriter(state),
            Arc::new(ResourceLockCoordinator::default()),
        );
        let outcome = application
            .plan_copy(
                &access,
                project.project_id,
                project.revision,
                destination.clone(),
                format!("WriterState{index}"),
                key(&format!("m7-copy-writer-{index}")),
            )
            .await
            .expect("advisory writer state permits bounded Plan");
        assert_eq!(outcome.plan.draft.writer_evidence.state, state);
    }

    let confirmed = M7CopyApplication::with_locks(
        store.clone(),
        ProjectCopyEngine,
        FixedWriter(UnityWriterStateKind::RunningConfirmed),
        Arc::new(ResourceLockCoordinator::default()),
    )
    .plan_copy(
        &access,
        project.project_id,
        project.revision,
        destination.clone(),
        "RunningConfirmed".to_owned(),
        key("m7-copy-writer-confirmed"),
    )
    .await
    .expect_err("confirmed Unity writer must reject Copy");
    assert_eq!(confirmed.code(), M7CopyErrorCode::UnityProjectRunning);

    let project_id = project.project_id;
    let draft = ProjectCopyEngine
        .plan(
            project,
            destination,
            "Expired".to_owned(),
            UnityWriterState {
                project_id,
                state: UnityWriterStateKind::NotObserved,
                evidence: Vec::new(),
                checked_at_ms: 1,
            },
            key("m7-copy-expired-plan"),
            1,
        )
        .await
        .expect("create expiry fixture");
    let plan_id = draft.plan_id;
    let expires_at = draft.expires_at_ms;
    store
        .create_project_copy_plan(PrincipalId::local_owner(), draft)
        .await
        .expect("persist expiry fixture");
    let expired = store
        .accept_project_copy(
            PrincipalId::local_owner(),
            plan_id,
            alcomd_application::Revision::INITIAL,
            key("m7-copy-expired-apply"),
            expires_at,
        )
        .await
        .expect_err("now equal to expiresAt must be stale");
    assert_eq!(expired.code(), M7CopyErrorCode::ProjectCopyPlanStale);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn project_copy_plan_apply_registers_an_exact_independent_copy() {
    let _serial = RPC_TEST_LOCK.lock().await;
    let fixture = TestDirectory::new();
    let runtime = fixture.path().join("runtime");
    let data = fixture.path().join("data");
    let source = fixture.path().join("SourceProject");
    let destination = fixture.path().join("Copies");
    fs::create_dir(&runtime).expect("runtime");
    fs::create_dir(&data).expect("data");
    fs::create_dir(&destination).expect("destination");
    create_project(&source);

    let (ipc, config) = isolated_ipc(runtime);
    let shutdown = Arc::new(AtomicBool::new(false));
    let server_shutdown = Arc::clone(&shutdown);
    let daemon = tokio::spawn(async move {
        alcomd_daemon::serve_with_data_until(
            ipc,
            DataConfig::isolated(data),
            wait_for_shutdown(server_shutdown),
        )
        .await
    });
    let mut client = connect_with_retry(config).await;
    let registered = client
        .project_register(
            source.to_string_lossy().into_owned(),
            "m7-copy-register".to_owned(),
        )
        .await
        .expect("register source");
    let source_id = registered.project.project_id.expect("source ID");
    let plan = client
        .project_plan_copy(alcomd_protocol::ProjectsPlanCopyParams {
            source_project_id: source_id,
            expected_revision: 1,
            target_parent_path: destination.to_string_lossy().into_owned(),
            target_leaf: "CopiedProject".to_owned(),
            idempotency_key: "m7-copy-plan".to_owned(),
        })
        .await
        .expect("plan copy");
    assert_eq!(plan.plan.profile.version, 1);
    assert_eq!(plan.plan.writer_evidence.state, "not_observed");
    let accepted = client
        .project_apply_copy(alcomd_protocol::ProjectsApplyCopyParams {
            plan_id: plan.plan.plan_id,
            expected_revision: 1,
            idempotency_key: "m7-copy-apply".to_owned(),
        })
        .await
        .expect("apply copy");
    let operation = wait_for_terminal(&mut client, accepted.operation_id).await;
    assert_eq!(operation.state, alcomd_protocol::OperationState::Succeeded);
    let target = destination.join("CopiedProject");
    assert_eq!(
        fs::read(target.join("Assets/content.txt")).expect("copied content"),
        b"copy me"
    );
    assert!(!target.join("Logs").exists());
    assert!(!target.join("Assets/.git").exists());
    assert!(!target.join(".alcomd-owner.json").exists());
    assert!(
        fs::read_dir(&destination)
            .expect("destination entries")
            .all(|entry| !entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .starts_with(".alcomd-copy-"))
    );
    let copied = client
        .project_get(accepted.target_project_id)
        .await
        .expect("copied project registered");
    assert_eq!(
        Path::new(&copied.project.root_path),
        fs::canonicalize(target).expect("canonical copied project")
    );

    shutdown.store(true, Ordering::Release);
    daemon.await.expect("join daemon").expect("daemon result");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_copy_plans_for_the_same_target_publish_exactly_once() {
    let _serial = RPC_TEST_LOCK.lock().await;
    let fixture = TestDirectory::new();
    let runtime = fixture.path().join("runtime");
    let data = fixture.path().join("data");
    let source = fixture.path().join("SourceProject");
    let destination = fixture.path().join("Copies");
    fs::create_dir(&runtime).expect("runtime");
    fs::create_dir(&data).expect("data");
    fs::create_dir(&destination).expect("destination");
    create_project(&source);

    let (ipc, config) = isolated_ipc(runtime);
    let shutdown = Arc::new(AtomicBool::new(false));
    let server_shutdown = Arc::clone(&shutdown);
    let daemon = tokio::spawn(async move {
        alcomd_daemon::serve_with_data_until(
            ipc,
            DataConfig::isolated(data),
            wait_for_shutdown(server_shutdown),
        )
        .await
    });
    let mut first = connect_with_retry(config.clone()).await;
    let mut second = connect_with_retry(config).await;
    let registered = first
        .project_register(
            source.to_string_lossy().into_owned(),
            "m7-copy-race-register".to_owned(),
        )
        .await
        .expect("register source");
    let source_project_id = registered.project.project_id.expect("source ID");
    let first_plan = first
        .project_plan_copy(alcomd_protocol::ProjectsPlanCopyParams {
            source_project_id: source_project_id.clone(),
            expected_revision: 1,
            target_parent_path: destination.to_string_lossy().into_owned(),
            target_leaf: "SameTarget".to_owned(),
            idempotency_key: "m7-copy-race-plan-a".to_owned(),
        })
        .await
        .expect("first Plan");
    let second_plan = second
        .project_plan_copy(alcomd_protocol::ProjectsPlanCopyParams {
            source_project_id,
            expected_revision: 1,
            target_parent_path: destination.to_string_lossy().into_owned(),
            target_leaf: "SameTarget".to_owned(),
            idempotency_key: "m7-copy-race-plan-b".to_owned(),
        })
        .await
        .expect("second Plan");

    let (first_apply, second_apply) = tokio::join!(
        first.project_apply_copy(alcomd_protocol::ProjectsApplyCopyParams {
            plan_id: first_plan.plan.plan_id,
            expected_revision: 1,
            idempotency_key: "m7-copy-race-apply-a".to_owned(),
        }),
        second.project_apply_copy(alcomd_protocol::ProjectsApplyCopyParams {
            plan_id: second_plan.plan.plan_id,
            expected_revision: 1,
            idempotency_key: "m7-copy-race-apply-b".to_owned(),
        })
    );
    let mut accepted = Vec::new();
    let mut rejected = 0;
    for result in [first_apply, second_apply] {
        match result {
            Ok(value) => accepted.push(value.operation_id),
            Err(ClientError::Remote(error)) if error.code == "project_copy_target_exists" => {
                rejected += 1;
            }
            Err(error) => panic!("unexpected same-target Apply result: {error}"),
        }
    }
    for operation_id in accepted {
        let operation = wait_for_terminal(&mut first, operation_id).await;
        match operation.state {
            alcomd_protocol::OperationState::Succeeded => {}
            alcomd_protocol::OperationState::Failed
                if operation.error_code.as_deref() == Some("project_copy_target_exists") =>
            {
                rejected += 1;
            }
            state => panic!("unexpected same-target terminal state: {state:?} {operation:?}"),
        }
    }
    assert_eq!(rejected, 1, "one competing Copy must fail closed");
    assert_eq!(
        fs::read(destination.join("SameTarget/Assets/content.txt")).expect("published content"),
        b"copy me"
    );
    let projects = first
        .projects_list(None, Some(100))
        .await
        .expect("list registered Projects");
    assert_eq!(
        projects
            .projects
            .iter()
            .filter(|project| project.root_path.ends_with("SameTarget"))
            .count(),
        1
    );
    assert!(
        fs::read_dir(&destination)
            .expect("destination entries")
            .all(|entry| !entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .starts_with(".alcomd-copy-"))
    );

    shutdown.store(true, Ordering::Release);
    daemon.await.expect("join daemon").expect("daemon result");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn project_copy_recovers_from_every_durable_kill_checkpoint() {
    let _serial = RPC_TEST_LOCK.lock().await;
    for checkpoint in [
        "inventory_ready",
        "staging",
        "staging_complete",
        "publish_intent",
        "target_published",
        "project_registry_commit_intent",
        "state_committed",
        "cleanup_complete",
    ] {
        eprintln!("running Project Copy kill/restart checkpoint: {checkpoint}");
        run_kill_restart_case(checkpoint).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn project_copy_cancel_is_honored_before_publish_and_rejected_after_intent() {
    let _serial = RPC_TEST_LOCK.lock().await;
    run_cancel_case("staging", true).await;
    run_cancel_case("publish_intent", false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn project_copy_preserves_externally_modified_published_target_for_manual_recovery() {
    let _serial = RPC_TEST_LOCK.lock().await;
    let fixture = TestDirectory::new();
    let kill_signal = fixture.path().join("kill.signal");
    let operation_signal = fixture.path().join("operation.signal");
    let child = Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "subprocess_runs_project_copy_until_parent_kills_it",
            "--ignored",
            "--nocapture",
        ])
        .env(CRASH_ROOT_ENV, fixture.path())
        .env(KILL_GATE_ENV, "target_published")
        .env(KILL_SIGNAL_ENV, &kill_signal)
        .env(OPERATION_SIGNAL_ENV, &operation_signal)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn Project Copy subprocess");
    let mut child = ChildGuard(Some(child));
    wait_for_file(&operation_signal, None, "target_published");
    wait_for_file(&kill_signal, Some(b"target_published"), "target_published");
    let signal: CopyCrashSignal =
        serde_json::from_slice(&fs::read(&operation_signal).expect("read operation signal"))
            .expect("parse operation signal");
    child.0.as_mut().expect("child").kill().expect("force kill");
    child.0.as_mut().expect("child").wait().expect("wait child");
    child.0 = None;
    let target = fixture.path().join("Copies/CopiedProject");
    fs::write(target.join("Assets/external.txt"), b"preserve me")
        .expect("external target mutation");

    let (ipc, config) = isolated_ipc(fixture.path().join("runtime"));
    let shutdown = Arc::new(AtomicBool::new(false));
    let server_shutdown = Arc::clone(&shutdown);
    let data = fixture.path().join("data");
    let daemon = tokio::spawn(async move {
        alcomd_daemon::serve_with_data_until(
            ipc,
            DataConfig::isolated(data),
            wait_for_shutdown(server_shutdown),
        )
        .await
    });
    let mut client = connect_with_retry(config).await;
    let operation = wait_for_recovery_required(&mut client, &signal.operation_id).await;
    assert_eq!(operation.state, alcomd_protocol::OperationState::Recovering);
    assert_eq!(
        operation.error_code.as_deref(),
        Some("project_copy_recovery_required")
    );
    assert_eq!(
        fs::read(target.join("Assets/external.txt")).expect("external bytes"),
        b"preserve me"
    );
    assert!(
        fixture
            .path()
            .join("Copies")
            .join(format!(".alcomd-copy-{}.staging", signal.operation_id))
            .is_dir(),
        "ownership evidence must remain for manual recovery"
    );
    let replay = client
        .project_apply_copy(alcomd_protocol::ProjectsApplyCopyParams {
            plan_id: signal.plan_id,
            expected_revision: 1,
            idempotency_key: "m7-copy-kill-apply".to_owned(),
        })
        .await
        .expect("replay recovery-required Apply");
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, signal.operation_id);
    shutdown.store(true, Ordering::Release);
    daemon.await.expect("join daemon").expect("daemon result");
}

#[test]
#[ignore = "subprocess fixture invoked by Project Copy kill/restart matrix"]
fn subprocess_runs_project_copy_until_parent_kills_it() {
    let fixture = PathBuf::from(std::env::var_os(CRASH_ROOT_ENV).expect("crash root"));
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    runtime.block_on(async move {
        let source = fixture.join("SourceProject");
        let destination = fixture.join("Copies");
        fs::create_dir_all(fixture.join("runtime")).expect("runtime root");
        fs::create_dir_all(fixture.join("data")).expect("data root");
        fs::create_dir(&destination).expect("destination");
        create_project(&source);
        let (ipc, config) = isolated_ipc(fixture.join("runtime"));
        let shutdown = Arc::new(AtomicBool::new(false));
        let server_shutdown = Arc::clone(&shutdown);
        let data = fixture.join("data");
        let _daemon = tokio::spawn(async move {
            alcomd_daemon::serve_with_data_until(
                ipc,
                DataConfig::isolated(data),
                wait_for_shutdown(server_shutdown),
            )
            .await
        });
        let mut client = connect_with_retry(config).await;
        let registered = client
            .project_register(
                source.to_string_lossy().into_owned(),
                "m7-copy-kill-register".to_owned(),
            )
            .await
            .expect("register source");
        let plan = client
            .project_plan_copy(alcomd_protocol::ProjectsPlanCopyParams {
                source_project_id: registered.project.project_id.expect("source ID"),
                expected_revision: 1,
                target_parent_path: destination.to_string_lossy().into_owned(),
                target_leaf: "CopiedProject".to_owned(),
                idempotency_key: "m7-copy-kill-plan".to_owned(),
            })
            .await
            .expect("plan copy");
        let accepted = client
            .project_apply_copy(alcomd_protocol::ProjectsApplyCopyParams {
                plan_id: plan.plan.plan_id.clone(),
                expected_revision: 1,
                idempotency_key: "m7-copy-kill-apply".to_owned(),
            })
            .await
            .expect("apply copy");
        let signal = CopyCrashSignal {
            plan_id: plan.plan.plan_id,
            operation_id: accepted.operation_id,
            target_project_id: accepted.target_project_id,
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
            "subprocess_runs_project_copy_until_parent_kills_it",
            "--ignored",
            "--nocapture",
        ])
        .env(CRASH_ROOT_ENV, fixture.path())
        .env(KILL_GATE_ENV, checkpoint)
        .env(KILL_SIGNAL_ENV, &kill_signal)
        .env(OPERATION_SIGNAL_ENV, &operation_signal)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn Project Copy subprocess");
    let mut child = ChildGuard(Some(child));
    wait_for_file(&operation_signal, None, checkpoint);
    wait_for_file(&kill_signal, Some(checkpoint.as_bytes()), checkpoint);
    let signal: CopyCrashSignal =
        serde_json::from_slice(&fs::read(&operation_signal).expect("read operation signal"))
            .expect("parse operation signal");
    child
        .0
        .as_mut()
        .expect("child")
        .kill()
        .expect("force-kill Project Copy subprocess");
    child.0.as_mut().expect("child").wait().expect("wait child");
    child.0 = None;

    let (ipc, config) = isolated_ipc(fixture.path().join("runtime"));
    let shutdown = Arc::new(AtomicBool::new(false));
    let server_shutdown = Arc::clone(&shutdown);
    let data = fixture.path().join("data");
    let daemon = tokio::spawn(async move {
        alcomd_daemon::serve_with_data_until(
            ipc,
            DataConfig::isolated(data),
            wait_for_shutdown(server_shutdown),
        )
        .await
    });
    let mut client = connect_with_retry(config).await;
    let replay = client
        .project_apply_copy(alcomd_protocol::ProjectsApplyCopyParams {
            plan_id: signal.plan_id.clone(),
            expected_revision: 1,
            idempotency_key: "m7-copy-kill-apply".to_owned(),
        })
        .await
        .expect("replay Project Copy Apply");
    assert!(replay.replayed, "{checkpoint}");
    assert_eq!(replay.operation_id, signal.operation_id, "{checkpoint}");
    assert_eq!(
        replay.target_project_id, signal.target_project_id,
        "{checkpoint}"
    );
    let completed = wait_for_terminal(&mut client, signal.operation_id.clone()).await;
    assert_eq!(
        completed.state,
        alcomd_protocol::OperationState::Succeeded,
        "{checkpoint}: {completed:?}"
    );
    let target = fixture.path().join("Copies/CopiedProject");
    assert_eq!(
        fs::read(target.join("Assets/content.txt")).expect("recovered content"),
        b"copy me",
        "{checkpoint}"
    );
    let copied = client
        .project_get(signal.target_project_id)
        .await
        .expect("recovered Project registration");
    assert_eq!(
        Path::new(&copied.project.root_path),
        fs::canonicalize(&target).expect("canonical recovered target"),
        "{checkpoint}"
    );
    assert!(
        fs::read_dir(fixture.path().join("Copies"))
            .expect("destination entries")
            .all(|entry| !entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .starts_with(".alcomd-copy-")),
        "{checkpoint}"
    );
    shutdown.store(true, Ordering::Release);
    daemon.await.expect("join daemon").expect("daemon result");
}

async fn run_cancel_case(checkpoint: &str, should_cancel: bool) {
    let fixture = TestDirectory::new();
    let pause_signal = fixture.path().join("pause.signal");
    let pause_release = fixture.path().join("pause.release");
    let operation_signal = fixture.path().join("operation.signal");
    let child = Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "subprocess_runs_project_copy_until_parent_kills_it",
            "--ignored",
            "--nocapture",
        ])
        .env(CRASH_ROOT_ENV, fixture.path())
        .env(PAUSE_GATE_ENV, checkpoint)
        .env(PAUSE_SIGNAL_ENV, &pause_signal)
        .env(PAUSE_RELEASE_ENV, &pause_release)
        .env(OPERATION_SIGNAL_ENV, &operation_signal)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn paused Project Copy subprocess");
    let mut child = ChildGuard(Some(child));
    wait_for_file(&operation_signal, None, checkpoint);
    wait_for_file(&pause_signal, Some(checkpoint.as_bytes()), checkpoint);
    let signal: CopyCrashSignal =
        serde_json::from_slice(&fs::read(&operation_signal).expect("read operation signal"))
            .expect("parse operation signal");
    let (_, config) = isolated_ipc(fixture.path().join("runtime"));
    let mut client = connect_with_retry(config).await;
    let operation = client
        .operation_get(signal.operation_id.clone())
        .await
        .expect("paused operation");
    let cancellation = client
        .operation_cancel(
            signal.operation_id.clone(),
            operation.revision,
            format!("m7-copy-cancel-{checkpoint}"),
        )
        .await;
    if should_cancel {
        assert_eq!(
            cancellation
                .expect("pre-publish cancellation")
                .operation
                .state,
            alcomd_protocol::OperationState::Cancelling
        );
    } else {
        assert!(matches!(
            cancellation,
            Err(ClientError::Remote(ref error)) if error.code == "operation_not_cancellable"
        ));
    }
    fs::write(&pause_release, b"continue").expect("release pause gate");
    let terminal = wait_for_terminal(&mut client, signal.operation_id).await;
    let target = fixture.path().join("Copies/CopiedProject");
    if should_cancel {
        assert_eq!(terminal.state, alcomd_protocol::OperationState::Cancelled);
        assert!(!target.exists());
    } else {
        assert_eq!(terminal.state, alcomd_protocol::OperationState::Succeeded);
        assert!(target.join("Assets/content.txt").is_file());
    }
    child.0.as_mut().expect("child").kill().expect("stop child");
    child.0.as_mut().expect("child").wait().expect("wait child");
    child.0 = None;
}

fn wait_for_file(path: &Path, expected: Option<&[u8]>, checkpoint: &str) {
    for _ in 0..10_000 {
        if let Ok(bytes) = fs::read(path)
            && expected.is_none_or(|expected| bytes == expected)
        {
            return;
        }
        thread::sleep(Duration::from_millis(5));
    }
    panic!("timed out waiting for {} at {checkpoint}", path.display());
}

struct ChildGuard(Option<std::process::Child>);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn create_project(path: &Path) {
    fs::create_dir_all(path.join("ProjectSettings")).expect("settings");
    fs::create_dir_all(path.join("Packages")).expect("packages");
    fs::create_dir_all(path.join("Assets/.git")).expect("nested git");
    fs::create_dir_all(path.join("Logs")).expect("logs");
    fs::write(
        path.join("ProjectSettings/ProjectVersion.txt"),
        "m_EditorVersion: 2022.3.22f1\nm_EditorVersionWithRevision: 2022.3.22f1 (fixture)\n",
    )
    .expect("version");
    fs::write(
        path.join("Packages/vpm-manifest.json"),
        r#"{"locked":{},"dependencies":{}}"#,
    )
    .expect("vpm manifest");
    fs::write(
        path.join("Packages/manifest.json"),
        r#"{"dependencies":{}}"#,
    )
    .expect("upm manifest");
    fs::write(path.join("Assets/content.txt"), b"copy me").expect("content");
    fs::write(path.join("Assets/.git/config"), b"excluded").expect("git marker");
    fs::write(path.join("Logs/editor.log"), b"excluded").expect("log");
}

fn project_observation(root: &Path) -> ProjectObservation {
    ProjectObservation {
        root_path: fs::canonicalize(root)
            .expect("canonical Project root")
            .to_string_lossy()
            .into_owned(),
        path_identity_key: alcomd_platform::file_identity_key(root).expect("Project identity"),
        project_type: ProjectType::Unknown,
        unity_version: "2022.3.22f1".to_owned(),
        unity_revision: None,
        vpm_manifest: ManifestState::Valid,
        upm_manifest: ManifestState::Valid,
        direct_dependencies: Vec::new(),
        locked_dependencies: Vec::new(),
        issues: Vec::new(),
        observed_at_ms: 1,
    }
}

fn key(value: &str) -> IdempotencyKey {
    IdempotencyKey::parse(value.to_owned()).expect("valid idempotency key")
}

async fn wait_for_terminal(
    client: &mut AlcomdClient,
    operation_id: String,
) -> alcomd_protocol::Operation {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
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
        assert!(tokio::time::Instant::now() < deadline, "copy timed out");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

async fn wait_for_recovery_required(
    client: &mut AlcomdClient,
    operation_id: &str,
) -> alcomd_protocol::Operation {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let operation = client
            .operation_get(operation_id.to_owned())
            .await
            .expect("get recovery-required operation");
        if operation.error_code.as_deref() == Some("project_copy_recovery_required") {
            return operation;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "recovery-required timeout"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

async fn connect_with_retry(config: ClientConfig) -> AlcomdClient {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        match AlcomdClient::connect(config.clone()).await {
            Ok(client) => return client,
            Err(_) if tokio::time::Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            Err(error) => panic!("daemon did not become ready: {error}"),
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

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        #[cfg(target_os = "macos")]
        let base = PathBuf::from("/private/tmp");
        #[cfg(not(target_os = "macos"))]
        let base = std::env::temp_dir();
        let path = base.join(format!("alcomd-m7-copy-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).expect("fixture root");
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
