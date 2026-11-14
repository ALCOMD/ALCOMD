use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
#[cfg(unix)]
use std::time::{SystemTime, UNIX_EPOCH};

use alcomd_client::{AlcomdClient, ClientConfig};
use alcomd_platform::IpcConfig;
static RPC_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cli_reports_human_and_json_status_over_real_ipc() {
    let _serial = RPC_TEST_LOCK.lock().await;
    let (ipc, client, runtime) = isolated_configuration();
    let shutdown = Arc::new(AtomicBool::new(false));
    let server_shutdown = Arc::clone(&shutdown);
    let daemon = alcomd_daemon::test_support::start_until(ipc, wait_for_shutdown(server_shutdown))
        .await
        .expect("start owned test daemon");
    wait_until_ready(client).await;

    let human_runtime = runtime.clone();
    let human = tokio::task::spawn_blocking(move || run_cli(false, human_runtime))
        .await
        .expect("join human CLI");
    assert!(human.status.success());
    assert!(human.stderr.is_empty());
    assert!(
        String::from_utf8(human.stdout)
            .expect("human stdout UTF-8")
            .contains("ALCOMD daemon")
    );

    let json_runtime = runtime.clone();
    let json = tokio::task::spawn_blocking(move || run_cli(true, json_runtime))
        .await
        .expect("join JSON CLI");
    assert!(json.status.success());
    assert!(json.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&json.stdout).expect("JSON stdout");
    assert_eq!(value["type"], "result");
    assert_eq!(value["command"], "system status");
    assert_eq!(value["result"]["state"], "ready");
    assert_eq!(value["result"]["rpcVersion"], 1);

    let ndjson_runtime = runtime.clone();
    let ndjson = tokio::task::spawn_blocking(move || run_ndjson_cli(ndjson_runtime))
        .await
        .expect("join NDJSON CLI");
    assert!(ndjson.status.success());
    assert!(ndjson.stderr.is_empty());
    let record: serde_json::Value = serde_json::from_slice(&ndjson.stdout).expect("NDJSON stdout");
    assert_eq!(record["type"], "result");
    assert_eq!(record["command"], "system status");

    for (arguments, expected_command) in [
        (&["operation", "list"][..], "operation list"),
        (&["project", "list"][..], "project list"),
        (&["repository", "list"][..], "repository list"),
        (&["unity", "list"][..], "unity list"),
        (&["backup", "list"][..], "backup list"),
    ] {
        let output = run_group_cli(runtime.clone(), arguments);
        assert!(output.status.success(), "{expected_command}");
        assert!(output.stderr.is_empty(), "{expected_command}");
        let document: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("group JSON stdout");
        assert_eq!(document["command"], expected_command);
    }

    let project_parent =
        std::env::temp_dir().join(format!("alcomd-cli-create-{}", std::process::id()));
    std::fs::create_dir(&project_parent).expect("owned project fixture directory");
    let target_parent = project_parent.to_str().expect("UTF-8 fixture path");
    let creation = run_group_cli(
        runtime.clone(),
        &[
            "project",
            "create",
            target_parent,
            "CLI Project",
            "--unity-version",
            "2022.3.22f1",
            "--idempotency-key",
            "cli-create-contract",
        ],
    );
    assert!(
        creation.status.success(),
        "{}",
        String::from_utf8_lossy(&creation.stderr)
    );
    let created: serde_json::Value =
        serde_json::from_slice(&creation.stdout).expect("project create JSON stdout");
    assert_eq!(created["command"], "project create");
    assert_eq!(created["result"]["state"], "succeeded");
    assert!(created["result"]["result"]["projectId"].as_str().is_some());
    assert!(
        project_parent
            .join("CLI Project/ProjectSettings/ProjectVersion.txt")
            .is_file()
    );
    std::fs::remove_dir_all(&project_parent).expect("remove owned project fixture directory");

    let confirmation_runtime = runtime.clone();
    let confirmation =
        tokio::task::spawn_blocking(move || run_confirmation_required_cli(confirmation_runtime))
            .await
            .expect("join confirmation CLI");
    assert_eq!(confirmation.status.code(), Some(1));
    assert!(confirmation.stdout.is_empty());
    let confirmation_error: serde_json::Value =
        serde_json::from_slice(&confirmation.stderr).expect("confirmation JSON stderr");
    assert_eq!(confirmation_error["type"], "error");
    assert_eq!(confirmation_error["error"]["code"], "confirmation_required");

    shutdown.store(true, Ordering::Release);
    let daemon_result = tokio::time::timeout(Duration::from_secs(2), daemon)
        .await
        .expect("daemon stops before timeout")
        .expect("join daemon");
    assert!(
        daemon_result.is_ok(),
        "daemon stopped with error: {daemon_result:?}"
    );

    let absent_runtime = runtime.clone();
    let absent = tokio::task::spawn_blocking(move || run_cli(false, absent_runtime))
        .await
        .expect("join absent-daemon CLI");
    assert!(!absent.status.success());
    assert!(absent.stdout.is_empty());
    assert!(!absent.stderr.is_empty());

    let usage = Command::new(env!("CARGO_BIN_EXE_alcomd-cli"))
        .arg("not-a-command")
        .output()
        .expect("run invalid CLI");
    assert_eq!(usage.status.code(), Some(2));
    assert!(usage.stdout.is_empty());

    let completion = Command::new(env!("CARGO_BIN_EXE_alcomd-cli"))
        .args(["--no-start-daemon", "completion", "powershell"])
        .output()
        .expect("run static completion");
    assert!(completion.status.success());
    assert!(completion.stderr.is_empty());
    assert!(
        String::from_utf8(completion.stdout)
            .expect("completion UTF-8")
            .contains("Register-ArgumentCompleter")
    );

    if let Some(runtime) = runtime {
        let _ = std::fs::remove_dir_all(runtime);
    }
}

fn run_confirmation_required_cli(runtime: Option<PathBuf>) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_alcomd-cli"));
    command.arg("--no-start-daemon").arg("--json");
    if let Some(runtime) = runtime {
        command.arg("--runtime-dir").arg(runtime);
    }
    command.args([
        "package",
        "apply-plan",
        "00000000-0000-4000-8000-000000000001",
        "--expected-revision",
        "1",
        "--idempotency-key",
        "confirmation-contract",
    ]);
    command.output().expect("run confirmation CLI")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mcp_cli_issues_once_and_broken_stdout_never_reissues() {
    use alcomd_protocol::{McpClientsCreateParams, McpClientsListParams};
    use std::process::Stdio;
    let _serial = RPC_TEST_LOCK.lock().await;
    let (ipc, config, runtime) = isolated_configuration();
    let shutdown = Arc::new(AtomicBool::new(false));
    let daemon =
        alcomd_daemon::test_support::start_until(ipc, wait_for_shutdown(Arc::clone(&shutdown)))
            .await
            .expect("owned isolated Core");
    wait_until_ready(config.clone()).await;
    let mut owner = AlcomdClient::connect(config.clone()).await.expect("owner");
    let record = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "CLI isolated client".into(),
            idempotency_key: "cli-create".into(),
            grants: vec![],
        })
        .await
        .expect("zero grants");
    let args = [
        "mcp",
        "client",
        "issue-credential",
        record.client_id.as_str(),
        "--expected-revision",
        "1",
        "--idempotency-key",
        "cli-issue",
        "--yes",
    ];
    let denied = run_group_cli(runtime.clone(), &args);
    assert!(!denied.status.success());
    let error: serde_json::Value =
        serde_json::from_slice(&denied.stderr).expect("safe preflight error");
    assert_eq!(error["error"]["code"], "invalid_arguments");
    assert_eq!(
        owner
            .mcp_clients_list(McpClientsListParams::default())
            .await
            .expect("unchanged")
            .items[0]
            .revision,
        1
    );
    let mut allowed = args.to_vec();
    allowed.push("--secret-stdout");
    let issued = run_group_cli(runtime.clone(), &allowed);
    assert!(issued.status.success());
    let result: serde_json::Value =
        serde_json::from_slice(&issued.stdout).expect("one-time response");
    let secret = result["result"]["credential"]
        .as_str()
        .expect("credential field")
        .to_owned();
    assert_eq!(secret.len(), 64);
    assert!(!String::from_utf8_lossy(&issued.stderr).contains(&secret));
    assert!(
        AlcomdClient::connect(config.clone().with_mcp_credential(secret.clone()))
            .await
            .is_ok()
    );
    let repeated = run_group_cli(runtime.clone(), &allowed);
    assert!(!repeated.status.success());
    assert_eq!(
        owner
            .mcp_clients_list(McpClientsListParams::default())
            .await
            .expect("no repeated issue")
            .items[0]
            .revision,
        2
    );
    let mut command = Command::new(env!("CARGO_BIN_EXE_alcomd-cli"));
    command.args(["--json", "--no-start-daemon"]);
    if let Some(path) = &runtime {
        command.arg("--runtime-dir").arg(path);
    }
    command.args([
        "mcp",
        "client",
        "issue-credential",
        &record.client_id,
        "--expected-revision",
        "2",
        "--idempotency-key",
        "cli-broken-output",
        "--yes",
        "--secret-stdout",
    ]);
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let output_path =
        std::env::temp_dir().join(format!("alcomd-cli-stdout-{}-{nonce}", std::process::id()));
    let mut marker = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output_path)
        .expect("owned output marker");
    std::io::Write::write_all(&mut marker, b"preserved output sentinel").expect("sentinel");
    drop(marker);
    // A valid read-only OS handle deterministically rejects writes; a closed anonymous
    // pipe did not provide a reliable failure on the first Windows run.
    let read_only = std::fs::File::open(&output_path).expect("read-only output handle");
    let failed = command
        .stdout(Stdio::from(read_only))
        .stderr(Stdio::piped())
        .output()
        .expect("join CLI");
    assert!(!failed.status.success());
    let error = String::from_utf8_lossy(&failed.stderr);
    assert!(error.contains("credential was issued, but output failed"));
    assert!(!error.contains(&secret));
    assert_eq!(
        std::fs::read(&output_path).expect("preserved sentinel"),
        b"preserved output sentinel"
    );
    std::fs::remove_file(output_path).expect("remove only owned output marker");
    let current = owner
        .mcp_clients_list(McpClientsListParams::default())
        .await
        .expect("authoritative final record");
    assert_eq!(current.items[0].revision, 3);
    assert!(current.items[0].grants.is_empty());
    assert!(
        AlcomdClient::connect(config.with_mcp_credential(secret))
            .await
            .is_err()
    );
    shutdown.store(true, Ordering::Release);
    assert!(daemon.await.expect("shutdown isolated Core").is_ok());
    if let Some(path) = runtime {
        let _ = std::fs::remove_dir_all(path);
    }
}

async fn wait_for_shutdown(shutdown: Arc<AtomicBool>) {
    while !shutdown.load(Ordering::Acquire) {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

fn run_cli(json: bool, runtime: Option<PathBuf>) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_alcomd-cli"));
    command.arg("--no-start-daemon");
    if json {
        command.arg("--json");
    }
    if let Some(runtime) = runtime {
        command.arg("--runtime-dir").arg(runtime);
    }
    command.args(["system", "status"]);
    command.output().expect("run CLI")
}

fn run_ndjson_cli(runtime: Option<PathBuf>) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_alcomd-cli"));
    command.arg("--no-start-daemon").arg("--ndjson");
    if let Some(runtime) = runtime {
        command.arg("--runtime-dir").arg(runtime);
    }
    command.args(["system", "status"]);
    command.output().expect("run NDJSON CLI")
}

fn run_group_cli(runtime: Option<PathBuf>, arguments: &[&str]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_alcomd-cli"));
    command.arg("--no-start-daemon").arg("--json");
    if let Some(runtime) = runtime {
        command.arg("--runtime-dir").arg(runtime);
    }
    command.args(arguments);
    command.output().expect("run command group CLI")
}

async fn wait_until_ready(config: ClientConfig) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        match AlcomdClient::connect(config.clone()).await {
            Ok(_) => return,
            Err(_) if tokio::time::Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            Err(error) => panic!("daemon did not become ready: {error}"),
        }
    }
}

#[cfg(unix)]
fn isolated_configuration() -> (IpcConfig, ClientConfig, Option<PathBuf>) {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    #[cfg(target_os = "macos")]
    let base = PathBuf::from("/private/tmp");
    #[cfg(not(target_os = "macos"))]
    let base = std::env::temp_dir();
    let path = base.join(format!("acm1-cli-{}-{nonce}", std::process::id()));
    (
        IpcConfig::isolated(path.clone()),
        ClientConfig::default()
            .without_daemon_start()
            .with_runtime_directory(path.clone()),
        Some(path),
    )
}

#[cfg(windows)]
fn isolated_configuration() -> (IpcConfig, ClientConfig, Option<PathBuf>) {
    (
        IpcConfig::default(),
        ClientConfig::default().without_daemon_start(),
        None,
    )
}
