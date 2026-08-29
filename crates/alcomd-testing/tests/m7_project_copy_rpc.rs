use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

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

const RPC_STAGE_TIMEOUT: Duration = Duration::from_secs(30);
const RPC_TEST_LOCK_TIMEOUT: Duration = Duration::from_secs(90);
const CLIENT_CONNECT_TEST_TIMEOUT: Duration = Duration::from_secs(10);
const DAEMON_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);
const DAEMON_DROP_WAIT_TIMEOUT: Duration = Duration::from_secs(5);
const CHILD_WAIT_TIMEOUT: Duration = Duration::from_secs(30);
const CHILD_DROP_WAIT_TIMEOUT: Duration = Duration::from_secs(5);

const CRASH_ROOT_ENV: &str = "ALCOMD_TEST_PROJECT_COPY_CRASH_ROOT";
const KILL_GATE_ENV: &str = "ALCOMD_TEST_PROJECT_COPY_KILL_GATE";
const KILL_SIGNAL_ENV: &str = "ALCOMD_TEST_PROJECT_COPY_KILL_SIGNAL";
const OPERATION_SIGNAL_ENV: &str = "ALCOMD_TEST_PROJECT_COPY_OPERATION_SIGNAL";
const PAUSE_GATE_ENV: &str = "ALCOMD_TEST_PROJECT_COPY_PAUSE_GATE";
const PAUSE_SIGNAL_ENV: &str = "ALCOMD_TEST_PROJECT_COPY_PAUSE_SIGNAL";
const PAUSE_RELEASE_ENV: &str = "ALCOMD_TEST_PROJECT_COPY_PAUSE_RELEASE";

#[derive(Clone, Copy)]
struct TestTrace {
    name: &'static str,
    started: Instant,
}

impl TestTrace {
    fn new(name: &'static str) -> Self {
        let trace = Self {
            name,
            started: Instant::now(),
        };
        trace.stage("test-start");
        trace
    }

    fn stage(self, stage: &str) {
        eprintln!(
            "[m7-project-copy][{}][+{:.3}s] {stage}",
            self.name,
            self.started.elapsed().as_secs_f64()
        );
    }
}

async fn acquire_rpc_test_lock(trace: TestTrace) -> tokio::sync::MutexGuard<'static, ()> {
    trace.stage("waiting-rpc-test-lock");
    match tokio::time::timeout(RPC_TEST_LOCK_TIMEOUT, RPC_TEST_LOCK.lock()).await {
        Ok(guard) => {
            trace.stage("acquired-rpc-test-lock");
            guard
        }
        Err(_) => {
            trace.stage("rpc-test-lock-timeout");
            panic!("m7_project_copy_rpc timed out at stage: rpc-test-lock");
        }
    }
}

async fn await_rpc<T>(trace: TestTrace, stage: &'static str, future: impl Future<Output = T>) -> T {
    trace.stage(&format!("{stage}-start"));
    match tokio::time::timeout(RPC_STAGE_TIMEOUT, future).await {
        Ok(value) => {
            trace.stage(&format!("{stage}-complete"));
            value
        }
        Err(_) => {
            trace.stage(&format!("{stage}-timeout"));
            panic!("m7_project_copy_rpc timed out at stage: {stage}");
        }
    }
}

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
    let trace = TestTrace::new("project-copy-plan-expiry-and-writer-states");
    let fixture = TestDirectory::new(trace);
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
    let trace = TestTrace::new("project-copy-plan-apply");
    let _serial = acquire_rpc_test_lock(trace).await;
    let fixture = TestDirectory::new(trace);
    let runtime = fixture.path().join("runtime");
    let data = fixture.path().join("data");
    let source = fixture.path().join("SourceProject");
    let destination = fixture.path().join("Copies");
    fs::create_dir(&runtime).expect("runtime");
    fs::create_dir(&data).expect("data");
    fs::create_dir(&destination).expect("destination");
    create_project(&source);

    let (ipc, config) = isolated_ipc(runtime);
    let mut daemon = spawn_test_daemon(ipc, DataConfig::isolated(data), trace);
    let mut client = connect_client(config, trace).await;
    let registered = await_rpc(
        trace,
        "project-register-rpc",
        client.project_register(
            source.to_string_lossy().into_owned(),
            "m7-copy-register".to_owned(),
        ),
    )
    .await
    .expect("register source");
    let source_id = registered.project.project_id.expect("source ID");
    let plan = await_rpc(
        trace,
        "project-copy-plan-rpc",
        client.project_plan_copy(alcomd_protocol::ProjectsPlanCopyParams {
            source_project_id: source_id,
            expected_revision: 1,
            target_parent_path: destination.to_string_lossy().into_owned(),
            target_leaf: "CopiedProject".to_owned(),
            idempotency_key: "m7-copy-plan".to_owned(),
        }),
    )
    .await
    .expect("plan copy");
    assert_eq!(plan.plan.profile.version, 1);
    assert_eq!(plan.plan.writer_evidence.state, "not_observed");
    let accepted = await_rpc(
        trace,
        "project-copy-apply-rpc",
        client.project_apply_copy(alcomd_protocol::ProjectsApplyCopyParams {
            plan_id: plan.plan.plan_id,
            expected_revision: 1,
            idempotency_key: "m7-copy-apply".to_owned(),
        }),
    )
    .await
    .expect("apply copy");
    let operation = wait_for_terminal(&mut client, accepted.operation_id, trace).await;
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
    let copied = await_rpc(
        trace,
        "project-get-rpc",
        client.project_get(accepted.target_project_id),
    )
    .await
    .expect("copied project registered");
    assert_eq!(
        Path::new(&copied.project.root_path),
        fs::canonicalize(target).expect("canonical copied project")
    );
    assert_eq!(copied.project.favorite, Some(false));

    daemon.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_daemon_guard_drop_releases_endpoint_for_the_next_daemon() {
    let trace = TestTrace::new("test-daemon-guard-drop-cleanup");
    let _serial = acquire_rpc_test_lock(trace).await;
    let fixture = TestDirectory::new(trace);
    let runtime = fixture.path().join("runtime");
    let data = fixture.path().join("data");
    fs::create_dir(&runtime).expect("runtime");
    fs::create_dir(&data).expect("data");

    let (first_ipc, first_config) = isolated_ipc(runtime.clone());
    let first_daemon = spawn_test_daemon(first_ipc, DataConfig::isolated(data.clone()), trace);
    let first_client = connect_client(first_config, trace).await;
    drop(first_client);
    trace.stage("daemon-guard-fallback-triggered");
    drop(first_daemon);

    let (second_ipc, second_config) = isolated_ipc(runtime);
    let mut second_daemon = spawn_test_daemon(second_ipc, DataConfig::isolated(data), trace);
    let second_client = connect_client(second_config, trace).await;
    drop(second_client);
    second_daemon.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_copy_plans_for_the_same_target_publish_exactly_once() {
    let trace = TestTrace::new("concurrent-copy-same-target");
    let _serial = acquire_rpc_test_lock(trace).await;
    let fixture = TestDirectory::new(trace);
    let runtime = fixture.path().join("runtime");
    let data = fixture.path().join("data");
    let source = fixture.path().join("SourceProject");
    let destination = fixture.path().join("Copies");
    fs::create_dir(&runtime).expect("runtime");
    fs::create_dir(&data).expect("data");
    fs::create_dir(&destination).expect("destination");
    create_project(&source);

    let (ipc, config) = isolated_ipc(runtime);
    let mut daemon = spawn_test_daemon(ipc, DataConfig::isolated(data), trace);
    let mut first = connect_client(config.clone(), trace).await;
    let mut second = connect_client(config, trace).await;
    let registered = await_rpc(
        trace,
        "project-register-rpc",
        first.project_register(
            source.to_string_lossy().into_owned(),
            "m7-copy-race-register".to_owned(),
        ),
    )
    .await
    .expect("register source");
    let source_project_id = registered.project.project_id.expect("source ID");
    let first_plan = await_rpc(
        trace,
        "project-copy-plan-rpc",
        first.project_plan_copy(alcomd_protocol::ProjectsPlanCopyParams {
            source_project_id: source_project_id.clone(),
            expected_revision: 1,
            target_parent_path: destination.to_string_lossy().into_owned(),
            target_leaf: "SameTarget".to_owned(),
            idempotency_key: "m7-copy-race-plan-a".to_owned(),
        }),
    )
    .await
    .expect("first Plan");
    let second_plan = await_rpc(
        trace,
        "project-copy-plan-rpc",
        second.project_plan_copy(alcomd_protocol::ProjectsPlanCopyParams {
            source_project_id,
            expected_revision: 1,
            target_parent_path: destination.to_string_lossy().into_owned(),
            target_leaf: "SameTarget".to_owned(),
            idempotency_key: "m7-copy-race-plan-b".to_owned(),
        }),
    )
    .await
    .expect("second Plan");

    let (first_apply, second_apply) = await_rpc(trace, "project-copy-apply-rpc", async {
        tokio::join!(
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
        )
    })
    .await;
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
        let operation = wait_for_terminal(&mut first, operation_id, trace).await;
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
    let projects = await_rpc(
        trace,
        "projects-list-rpc",
        first.projects_list(None, Some(100)),
    )
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

    daemon.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn project_copy_recovers_from_every_durable_kill_checkpoint() {
    let trace = TestTrace::new("project-copy-kill-restart-matrix");
    let _serial = acquire_rpc_test_lock(trace).await;
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
        trace.stage(&format!("kill-checkpoint-start checkpoint={checkpoint}"));
        run_kill_restart_case(checkpoint, trace).await;
        trace.stage(&format!("kill-checkpoint-complete checkpoint={checkpoint}"));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn project_copy_cancel_is_honored_before_publish_and_rejected_after_intent() {
    let trace = TestTrace::new("project-copy-cancellation");
    let _serial = acquire_rpc_test_lock(trace).await;
    run_cancel_case("staging", true, trace).await;
    run_cancel_case("publish_intent", false, trace).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn project_copy_preserves_externally_modified_published_target_for_manual_recovery() {
    let trace = TestTrace::new("project-copy-external-modification-recovery");
    let _serial = acquire_rpc_test_lock(trace).await;
    let fixture = TestDirectory::new(trace);
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
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn Project Copy subprocess");
    let mut child = ChildGuard::new(child, trace);
    trace.stage("child-process-spawned");
    wait_for_file(&operation_signal, None, "target_published", trace);
    wait_for_file(
        &kill_signal,
        Some(b"target_published"),
        "target_published",
        trace,
    );
    let signal: CopyCrashSignal =
        serde_json::from_slice(&fs::read(&operation_signal).expect("read operation signal"))
            .expect("parse operation signal");
    child.stop_and_wait("child-kill-wait");
    let target = fixture.path().join("Copies/CopiedProject");
    fs::write(target.join("Assets/external.txt"), b"preserve me")
        .expect("external target mutation");

    let (ipc, config) = isolated_ipc(fixture.path().join("runtime"));
    let data = fixture.path().join("data");
    let mut daemon = spawn_test_daemon(ipc, DataConfig::isolated(data), trace);
    let mut client = connect_client(config, trace).await;
    let operation = wait_for_recovery_required(&mut client, &signal.operation_id, trace).await;
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
    let replay = await_rpc(
        trace,
        "project-copy-apply-rpc",
        client.project_apply_copy(alcomd_protocol::ProjectsApplyCopyParams {
            plan_id: signal.plan_id,
            expected_revision: 1,
            idempotency_key: "m7-copy-kill-apply".to_owned(),
        }),
    )
    .await
    .expect("replay recovery-required Apply");
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, signal.operation_id);
    daemon.shutdown().await;
}

#[test]
#[ignore = "subprocess fixture invoked by Project Copy kill/restart matrix"]
fn subprocess_runs_project_copy_until_parent_kills_it() {
    let trace = TestTrace::new("project-copy-subprocess-fixture");
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
        trace.stage("daemon-task-spawned");
        let mut client = connect_client(config, trace).await;
        let registered = await_rpc(
            trace,
            "project-register-rpc",
            client.project_register(
                source.to_string_lossy().into_owned(),
                "m7-copy-kill-register".to_owned(),
            ),
        )
        .await
        .expect("register source");
        let plan = await_rpc(
            trace,
            "project-copy-plan-rpc",
            client.project_plan_copy(alcomd_protocol::ProjectsPlanCopyParams {
                source_project_id: registered.project.project_id.expect("source ID"),
                expected_revision: 1,
                target_parent_path: destination.to_string_lossy().into_owned(),
                target_leaf: "CopiedProject".to_owned(),
                idempotency_key: "m7-copy-kill-plan".to_owned(),
            }),
        )
        .await
        .expect("plan copy");
        let accepted = await_rpc(
            trace,
            "project-copy-apply-rpc",
            client.project_apply_copy(alcomd_protocol::ProjectsApplyCopyParams {
                plan_id: plan.plan.plan_id.clone(),
                expected_revision: 1,
                idempotency_key: "m7-copy-kill-apply".to_owned(),
            }),
        )
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
        trace.stage("operation-signal-written");
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });
}

async fn run_kill_restart_case(checkpoint: &str, trace: TestTrace) {
    let fixture = TestDirectory::new(trace);
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
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn Project Copy subprocess");
    let mut child = ChildGuard::new(child, trace);
    trace.stage("child-process-spawned");
    wait_for_file(&operation_signal, None, checkpoint, trace);
    wait_for_file(&kill_signal, Some(checkpoint.as_bytes()), checkpoint, trace);
    let signal: CopyCrashSignal =
        serde_json::from_slice(&fs::read(&operation_signal).expect("read operation signal"))
            .expect("parse operation signal");
    child.stop_and_wait("child-kill-wait");

    let (ipc, config) = isolated_ipc(fixture.path().join("runtime"));
    let data = fixture.path().join("data");
    let mut daemon = spawn_test_daemon(ipc, DataConfig::isolated(data), trace);
    let mut client = connect_client(config, trace).await;
    let replay = await_rpc(
        trace,
        "project-copy-apply-rpc",
        client.project_apply_copy(alcomd_protocol::ProjectsApplyCopyParams {
            plan_id: signal.plan_id.clone(),
            expected_revision: 1,
            idempotency_key: "m7-copy-kill-apply".to_owned(),
        }),
    )
    .await
    .expect("replay Project Copy Apply");
    assert!(replay.replayed, "{checkpoint}");
    assert_eq!(replay.operation_id, signal.operation_id, "{checkpoint}");
    assert_eq!(
        replay.target_project_id, signal.target_project_id,
        "{checkpoint}"
    );
    let completed = wait_for_terminal(&mut client, signal.operation_id.clone(), trace).await;
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
    let copied = await_rpc(
        trace,
        "project-get-rpc",
        client.project_get(signal.target_project_id),
    )
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
    daemon.shutdown().await;
}

async fn run_cancel_case(checkpoint: &str, should_cancel: bool, trace: TestTrace) {
    trace.stage(&format!("cancel-checkpoint-start checkpoint={checkpoint}"));
    let fixture = TestDirectory::new(trace);
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
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn paused Project Copy subprocess");
    let mut child = ChildGuard::new(child, trace);
    trace.stage("child-process-spawned");
    wait_for_file(&operation_signal, None, checkpoint, trace);
    wait_for_file(
        &pause_signal,
        Some(checkpoint.as_bytes()),
        checkpoint,
        trace,
    );
    let signal: CopyCrashSignal =
        serde_json::from_slice(&fs::read(&operation_signal).expect("read operation signal"))
            .expect("parse operation signal");
    let (_, config) = isolated_ipc(fixture.path().join("runtime"));
    let mut client = connect_client(config, trace).await;
    let operation = await_rpc(
        trace,
        "operation-get-rpc",
        client.operation_get(signal.operation_id.clone()),
    )
    .await
    .expect("paused operation");
    let cancellation = await_rpc(
        trace,
        "operation-cancel-rpc",
        client.operation_cancel(
            signal.operation_id.clone(),
            operation.revision,
            format!("m7-copy-cancel-{checkpoint}"),
        ),
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
    trace.stage("pause-gate-released");
    let terminal = wait_for_terminal(&mut client, signal.operation_id, trace).await;
    let target = fixture.path().join("Copies/CopiedProject");
    if should_cancel {
        assert_eq!(terminal.state, alcomd_protocol::OperationState::Cancelled);
        assert!(!target.exists());
    } else {
        assert_eq!(terminal.state, alcomd_protocol::OperationState::Succeeded);
        assert!(target.join("Assets/content.txt").is_file());
    }
    child.stop_and_wait("child-kill-wait");
    trace.stage(&format!(
        "cancel-checkpoint-complete checkpoint={checkpoint}"
    ));
}

fn wait_for_file(path: &Path, expected: Option<&[u8]>, checkpoint: &str, trace: TestTrace) {
    trace.stage(&format!("checkpoint-wait-start checkpoint={checkpoint}"));
    for _ in 0..10_000 {
        if let Ok(bytes) = fs::read(path)
            && expected.is_none_or(|expected| bytes == expected)
        {
            trace.stage(&format!("checkpoint-wait-complete checkpoint={checkpoint}"));
            return;
        }
        thread::sleep(Duration::from_millis(5));
    }
    trace.stage(&format!("checkpoint-wait-timeout checkpoint={checkpoint}"));
    panic!("m7_project_copy_rpc timed out at stage: checkpoint-wait ({checkpoint})");
}

struct TestDaemonGuard {
    shutdown: Arc<AtomicBool>,
    daemon: Option<tokio::task::JoinHandle<Result<(), alcomd_platform::BindError>>>,
    trace: TestTrace,
    completed: bool,
}

impl TestDaemonGuard {
    fn new(
        shutdown: Arc<AtomicBool>,
        daemon: tokio::task::JoinHandle<Result<(), alcomd_platform::BindError>>,
        trace: TestTrace,
    ) -> Self {
        Self {
            shutdown,
            daemon: Some(daemon),
            trace,
            completed: false,
        }
    }

    async fn shutdown(&mut self) {
        self.trace.stage("daemon-shutdown-start");
        self.shutdown.store(true, Ordering::Release);
        let result = tokio::time::timeout(
            DAEMON_SHUTDOWN_TIMEOUT,
            self.daemon.as_mut().expect("daemon task"),
        )
        .await;
        match result {
            Ok(result) => {
                self.completed = true;
                self.daemon.take();
                result.expect("join daemon").expect("daemon result");
                self.trace.stage("daemon-shutdown-complete");
            }
            Err(_) => {
                self.trace.stage("daemon-shutdown-timeout");
                let daemon = self.daemon.as_mut().expect("daemon task");
                daemon.abort();
                let _ = tokio::time::timeout(DAEMON_DROP_WAIT_TIMEOUT, daemon).await;
                self.completed = true;
                self.daemon.take();
                panic!("m7_project_copy_rpc timed out at stage: daemon-shutdown");
            }
        }
    }
}

impl Drop for TestDaemonGuard {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        self.trace.stage("daemon-drop-cleanup-start");
        self.shutdown.store(true, Ordering::Release);
        if let Some(daemon) = self.daemon.take() {
            daemon.abort();
            let deadline = Instant::now() + DAEMON_DROP_WAIT_TIMEOUT;
            while !daemon.is_finished() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(10));
            }
            if daemon.is_finished() {
                self.trace.stage("daemon-drop-cleanup-complete");
            } else {
                self.trace.stage("daemon-drop-cleanup-timeout");
            }
        }
    }
}

struct ChildGuard {
    child: Option<Child>,
    trace: TestTrace,
}

impl ChildGuard {
    fn new(child: Child, trace: TestTrace) -> Self {
        Self {
            child: Some(child),
            trace,
        }
    }

    fn stop_and_wait(&mut self, stage: &'static str) {
        self.trace.stage(&format!("{stage}-start"));
        let child = self.child.as_mut().expect("child");
        child.kill().expect("stop Project Copy subprocess");
        if wait_for_child_exit(child, CHILD_WAIT_TIMEOUT) {
            self.child = None;
            self.trace.stage(&format!("{stage}-complete"));
        } else {
            self.trace.stage(&format!("{stage}-timeout"));
            panic!("m7_project_copy_rpc timed out at stage: {stage}");
        }
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            self.trace.stage("child-drop-cleanup-start");
            let _ = child.kill();
            if wait_for_child_exit(child, CHILD_DROP_WAIT_TIMEOUT) {
                self.trace.stage("child-drop-cleanup-complete");
            } else {
                self.trace.stage("child-drop-cleanup-timeout");
            }
        }
    }
}

fn wait_for_child_exit(child: &mut Child, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return true,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Ok(None) | Err(_) => return false,
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
    trace: TestTrace,
) -> alcomd_protocol::Operation {
    trace.stage("terminal-wait-start");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let operation = await_rpc(
            trace,
            "operation-get-rpc",
            client.operation_get(operation_id.clone()),
        )
        .await
        .expect("get operation");
        if matches!(
            operation.state,
            alcomd_protocol::OperationState::Succeeded
                | alcomd_protocol::OperationState::Failed
                | alcomd_protocol::OperationState::Cancelled
        ) {
            trace.stage("terminal-wait-complete");
            return operation;
        }
        assert!(tokio::time::Instant::now() < deadline, "copy timed out");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

async fn wait_for_recovery_required(
    client: &mut AlcomdClient,
    operation_id: &str,
    trace: TestTrace,
) -> alcomd_protocol::Operation {
    trace.stage("recovery-wait-start");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let operation = await_rpc(
            trace,
            "operation-get-rpc",
            client.operation_get(operation_id.to_owned()),
        )
        .await
        .expect("get recovery-required operation");
        if operation.error_code.as_deref() == Some("project_copy_recovery_required") {
            trace.stage("recovery-wait-complete");
            return operation;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "recovery-required timeout"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

async fn connect_client(config: ClientConfig, trace: TestTrace) -> AlcomdClient {
    trace.stage("client-connect-start");
    match tokio::time::timeout(CLIENT_CONNECT_TEST_TIMEOUT, AlcomdClient::connect(config)).await {
        Ok(Ok(client)) => {
            trace.stage("client-connected");
            client
        }
        Ok(Err(error)) => {
            trace.stage("client-connect-failed");
            panic!("client establishment failed: {error}");
        }
        Err(_) => {
            trace.stage("client-connect-timeout");
            panic!("m7_project_copy_rpc timed out at stage: client-connect");
        }
    }
}

fn spawn_test_daemon(ipc: IpcConfig, data: DataConfig, trace: TestTrace) -> TestDaemonGuard {
    let shutdown = Arc::new(AtomicBool::new(false));
    let server_shutdown = Arc::clone(&shutdown);
    let daemon = tokio::spawn(async move {
        alcomd_daemon::serve_with_data_until(ipc, data, wait_for_shutdown(server_shutdown)).await
    });
    trace.stage("daemon-task-spawned");
    TestDaemonGuard::new(shutdown, daemon, trace)
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

struct TestDirectory {
    path: PathBuf,
    trace: TestTrace,
}

impl TestDirectory {
    fn new(trace: TestTrace) -> Self {
        #[cfg(target_os = "macos")]
        let base = PathBuf::from("/private/tmp");
        #[cfg(not(target_os = "macos"))]
        let base = std::env::temp_dir();
        let path = base.join(format!("alcomd-m7-copy-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).expect("fixture root");
        trace.stage("fixture-created");
        Self { path, trace }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        self.trace.stage("fixture-cleanup-start");
        if fs::remove_dir_all(&self.path).is_ok() || !self.path.exists() {
            self.trace.stage("fixture-cleanup-complete");
        } else {
            self.trace.stage("fixture-cleanup-failed");
        }
    }
}
