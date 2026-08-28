use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use alcomd_application::StateStore;
use alcomd_client::{AlcomdClient, ClientConfig};
use alcomd_domain::{OperationId, PrincipalId};
use alcomd_platform::{DataConfig, IpcConfig};
use alcomd_protocol::{
    OperationState, TemplateApplyPlanParams, TemplatePlanCreateProjectParams, TemplatesListParams,
};
use alcomd_store::StateStoreHandle;

const CRASH_ROOT_ENV: &str = "ALCOMD_M5_TEMPLATE_CRASH_ROOT";
const CRASH_OPERATION_SIGNAL_ENV: &str = "ALCOMD_M5_TEMPLATE_OPERATION_SIGNAL";
const KILL_GATE_ENV: &str = "ALCOMD_TEST_M5_TEMPLATE_KILL_GATE";
const KILL_SIGNAL_ENV: &str = "ALCOMD_TEST_M5_TEMPLATE_KILL_SIGNAL";
static RPC_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn template_builtin_create_project_round_trips_over_rpc() {
    let _serial = RPC_TEST_LOCK.lock().await;
    let fixture = TestDirectory::new();
    let runtime = fixture.path().join("runtime");
    let data = fixture.path().join("data");
    let projects = fixture.path().join("projects");
    fs::create_dir(&runtime).expect("runtime");
    fs::create_dir(&data).expect("data");
    fs::create_dir(&projects).expect("projects");
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
    let capabilities = client.system_status().await.expect("status").capabilities;
    for capability in [
        alcomd_protocol::CAPABILITY_TEMPLATES_READ_V1,
        alcomd_protocol::CAPABILITY_TEMPLATES_MANAGE_V1,
        alcomd_protocol::CAPABILITY_TEMPLATES_CREATE_PROJECT_V1,
    ] {
        assert!(capabilities.iter().any(|value| value == capability));
    }
    let templates = client
        .templates_list(TemplatesListParams {
            cursor: None,
            limit: Some(10),
        })
        .await
        .expect("list builtins");
    assert_eq!(templates.templates.len(), 3);
    let blank = templates
        .templates
        .into_iter()
        .find(|value| value.display_name == "Blank")
        .expect("blank");
    let plan = client
        .template_plan_create_project(TemplatePlanCreateProjectParams {
            template_id: blank.template_id,
            expected_template_revision: blank.revision,
            target_parent: projects.to_string_lossy().into_owned(),
            target_leaf: "RPC Project".to_owned(),
        })
        .await
        .expect("plan create");
    assert_eq!(plan.action, "create_project");
    assert_eq!(
        plan.evidence
            .get("targetLeaf")
            .and_then(|value| value.as_str()),
        Some("RPC Project")
    );
    assert!(!plan.evidence.contains_key("targetPath"));
    let accepted = client
        .template_apply_create_project(TemplateApplyPlanParams {
            plan_id: plan.plan_id.clone(),
            idempotency_key: "m5-template-rpc-create".to_owned(),
        })
        .await
        .expect("apply create");
    let operation = wait_for_operation(&mut client, &accepted.operation_id).await;
    assert_eq!(operation.state, OperationState::Succeeded);
    let created_project_id = operation
        .result
        .as_ref()
        .and_then(|value| value.get("projectId"))
        .and_then(serde_json::Value::as_str)
        .expect("created project ID");
    let created_project = client
        .project_get(created_project_id.to_owned())
        .await
        .expect("created project");
    assert_eq!(created_project.project.favorite, Some(false));
    assert!(
        projects
            .join("RPC Project/Packages/vpm-manifest.json")
            .is_file()
    );
    let replay = client
        .template_apply_create_project(TemplateApplyPlanParams {
            plan_id: plan.plan_id,
            idempotency_key: "m5-template-rpc-create".to_owned(),
        })
        .await
        .expect("replay create");
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, accepted.operation_id);
    shutdown.store(true, Ordering::Release);
    daemon.await.expect("join daemon").expect("daemon result");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn template_create_project_kill_restart_matrix_reuses_frozen_authority() {
    let _serial = RPC_TEST_LOCK.lock().await;
    for checkpoint in [
        "prepared",
        "staging_complete",
        "target_publish_intent",
        "target_published",
        "project_registry_commit_intent",
        "state_committed",
    ] {
        run_kill_restart_case(checkpoint).await;
    }
}

#[test]
#[ignore = "subprocess fixture invoked by the Template kill/restart test"]
fn subprocess_runs_template_create_until_killed() {
    let root = PathBuf::from(std::env::var_os(CRASH_ROOT_ENV).expect("crash root"));
    let operation_signal =
        PathBuf::from(std::env::var_os(CRASH_OPERATION_SIGNAL_ENV).expect("operation signal"));
    let runtime = tokio::runtime::Runtime::new().expect("test runtime");
    runtime.block_on(async move {
        let runtime_root = root.join("runtime");
        let data = root.join("data");
        let projects = root.join("projects");
        fs::create_dir_all(&runtime_root).expect("runtime");
        fs::create_dir_all(&data).expect("data");
        fs::create_dir_all(&projects).expect("projects");
        let (ipc, config) = isolated_ipc(runtime_root);
        let shutdown = Arc::new(AtomicBool::new(false));
        let server_shutdown = Arc::clone(&shutdown);
        let _daemon = tokio::spawn(async move {
            alcomd_daemon::serve_with_data_until(
                ipc,
                DataConfig::isolated(data),
                wait_for_shutdown(server_shutdown),
            )
            .await
        });
        let mut client = connect_with_retry(config).await;
        let templates = client
            .templates_list(TemplatesListParams {
                cursor: None,
                limit: Some(10),
            })
            .await
            .expect("list builtins");
        let blank = templates
            .templates
            .into_iter()
            .find(|value| value.display_name == "Blank")
            .expect("blank");
        let plan = client
            .template_plan_create_project(TemplatePlanCreateProjectParams {
                template_id: blank.template_id,
                expected_template_revision: blank.revision,
                target_parent: projects.to_string_lossy().into_owned(),
                target_leaf: "Crash Project".to_owned(),
            })
            .await
            .expect("plan create");
        let idempotency_key = "m5-template-kill-restart".to_owned();
        let accepted = client
            .template_apply_create_project(TemplateApplyPlanParams {
                plan_id: plan.plan_id.clone(),
                idempotency_key: idempotency_key.clone(),
            })
            .await
            .expect("apply create");
        fs::write(
            operation_signal,
            serde_json::to_vec(&CrashSignal {
                operation_id: accepted.operation_id,
                plan_id: plan.plan_id,
                idempotency_key,
            })
            .expect("serialize operation signal"),
        )
        .expect("write operation signal");
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CrashSignal {
    operation_id: String,
    plan_id: String,
    idempotency_key: String,
}

async fn run_kill_restart_case(checkpoint: &str) {
    let fixture = TestDirectory::new();
    let operation_signal = fixture.path().join("operation.json");
    let kill_signal = fixture.path().join("kill-gate.txt");
    let child = Command::new(std::env::current_exe().expect("current test executable"))
        .args([
            "--exact",
            "subprocess_runs_template_create_until_killed",
            "--ignored",
            "--nocapture",
        ])
        .env(CRASH_ROOT_ENV, fixture.path())
        .env(CRASH_OPERATION_SIGNAL_ENV, &operation_signal)
        .env(KILL_GATE_ENV, checkpoint)
        .env(KILL_SIGNAL_ENV, &kill_signal)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn Template transaction subprocess");
    let mut child = ChildGuard(Some(child));
    wait_for_file(&kill_signal, checkpoint.as_bytes());
    let signal: CrashSignal =
        serde_json::from_slice(&fs::read(&operation_signal).expect("read operation signal"))
            .expect("parse operation signal");
    child
        .0
        .as_mut()
        .expect("child process")
        .kill()
        .expect("force-kill Template subprocess");
    child
        .0
        .as_mut()
        .expect("child process")
        .wait()
        .expect("wait for killed Template subprocess");
    child.0 = None;

    let killed_store = StateStoreHandle::open(fixture.path().join("data/state.db"))
        .expect("open killed Template state");
    let killed_operation = killed_store
        .get_operation(
            PrincipalId::local_owner(),
            OperationId::parse(&signal.operation_id).expect("operation ID"),
        )
        .await
        .expect("load killed Template operation");
    let expected_killed_state = if checkpoint == "state_committed" {
        alcomd_domain::OperationState::Succeeded
    } else {
        alcomd_domain::OperationState::Running
    };
    assert_eq!(
        killed_operation.state, expected_killed_state,
        "{checkpoint} has an invalid durable state after kill: {killed_operation:?}"
    );
    drop(killed_store);

    let target = fixture.path().join("projects/Crash Project");
    if checkpoint == "state_committed" {
        assert!(target.is_dir());
    }

    let runtime = fixture.path().join("runtime");
    let data = fixture.path().join("data");
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
    let operation = wait_for_operation(&mut client, &signal.operation_id).await;
    assert_eq!(
        operation.state,
        OperationState::Succeeded,
        "{checkpoint}: {operation:?}"
    );
    assert!(
        operation
            .result
            .as_ref()
            .and_then(|value| value.get("projectId"))
            .and_then(serde_json::Value::as_str)
            .is_some()
    );
    assert!(target.join("Packages/vpm-manifest.json").is_file());
    assert!(target.join("Packages/manifest.json").is_file());
    let replay = client
        .template_apply_create_project(TemplateApplyPlanParams {
            plan_id: signal.plan_id,
            idempotency_key: signal.idempotency_key,
        })
        .await
        .expect("replay recovered create");
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, signal.operation_id);
    shutdown.store(true, Ordering::Release);
    daemon.await.expect("join daemon").expect("daemon result");
}

fn wait_for_file(path: &Path, expected: &[u8]) {
    for _ in 0..600 {
        if fs::read(path).is_ok_and(|actual| actual == expected) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for {}", path.display());
}

struct ChildGuard(Option<std::process::Child>);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

async fn wait_for_operation(
    client: &mut AlcomdClient,
    operation_id: &str,
) -> alcomd_protocol::Operation {
    for _ in 0..400 {
        let operation = client
            .operation_get(operation_id.to_owned())
            .await
            .expect("operation");
        if matches!(
            operation.state,
            OperationState::Succeeded | OperationState::Failed | OperationState::Cancelled
        ) {
            return operation;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("Template create operation did not finish")
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
        let path = base.join(format!("alcomd-m5-template-rpc-{}", uuid::Uuid::new_v4()));
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
