use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
#[cfg(unix)]
use std::time::{SystemTime, UNIX_EPOCH};

use alcomd_client::{AlcomdClient, ClientConfig};
use alcomd_platform::IpcConfig;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cli_reports_human_and_json_status_over_real_ipc() {
    let (ipc, client, runtime) = isolated_configuration();
    let shutdown = Arc::new(AtomicBool::new(false));
    let server_shutdown = Arc::clone(&shutdown);
    let daemon = tokio::spawn(async move {
        alcomd_daemon::serve_until(ipc, wait_for_shutdown(server_shutdown)).await
    });
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
        (&["template", "list"][..], "template list"),
        (&["backup", "list"][..], "backup list"),
    ] {
        let output = run_group_cli(runtime.clone(), arguments);
        assert!(output.status.success(), "{expected_command}");
        assert!(output.stderr.is_empty(), "{expected_command}");
        let document: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("group JSON stdout");
        assert_eq!(document["command"], expected_command);
    }

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
        "template",
        "remove",
        "00000000-0000-4000-8000-000000000001",
        "--expected-revision",
        "1",
        "--idempotency-key",
        "confirmation-contract",
    ]);
    command.output().expect("run confirmation CLI")
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
