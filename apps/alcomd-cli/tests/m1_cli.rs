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
    assert_eq!(value["state"], "ready");
    assert_eq!(value["rpcVersion"], 1);

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

    if let Some(runtime) = runtime {
        let _ = std::fs::remove_dir_all(runtime);
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
