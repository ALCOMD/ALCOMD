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
    let archive = project_parent.join("CLI archive.zip");
    let archive = archive.to_str().expect("UTF-8 archive path");
    let created_id = created["result"]["result"]["projectId"]
        .as_str()
        .expect("new project ID");
    let backup = run_group_cli(
        runtime.clone(),
        &[
            "backup",
            "create",
            created_id,
            "--expected-revision",
            "1",
            "--destination",
            archive,
            "--idempotency-key",
            "cli-backup-file",
        ],
    );
    assert!(
        backup.status.success(),
        "{}",
        String::from_utf8_lossy(&backup.stderr)
    );
    let saved: serde_json::Value =
        serde_json::from_slice(&backup.stdout).expect("backup create JSON");
    assert_eq!(saved["command"], "backup create");
    assert_eq!(saved["result"]["state"], "succeeded");
    assert_eq!(saved["result"]["result"]["archiveName"], "CLI archive.zip");

    let restored = project_parent.join("CLI Restored");
    let restore = run_group_cli(
        runtime.clone(),
        &[
            "backup",
            "restore",
            archive,
            "--destination",
            restored.to_str().expect("UTF-8 project path"),
            "--idempotency-key",
            "cli-restore-file",
            "--yes",
        ],
    );
    assert!(
        restore.status.success(),
        "{}",
        String::from_utf8_lossy(&restore.stderr)
    );
    let restore: serde_json::Value =
        serde_json::from_slice(&restore.stdout).expect("backup restore JSON");
    assert_eq!(restore["command"], "backup restore");
    assert_eq!(restore["result"]["state"], "succeeded");
    assert!(
        restored
            .join("ProjectSettings/ProjectVersion.txt")
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
    assert!(current.items[0].revoked_at_ms.is_none());
    assert!(
        AlcomdClient::connect(config.clone().with_mcp_credential(secret.clone()))
            .await
            .is_err()
    );
    // A closed pipe is a separate real OS failure from the read-only file above.
    // Ordinary CLI output may treat BrokenPipe as success; a lost one-time secret cannot.
    let (reader, writer) = std::io::pipe().expect("owned output pipe");
    drop(reader);
    let mut broken_pipe = Command::new(env!("CARGO_BIN_EXE_alcomd-cli"));
    broken_pipe.args(["--json", "--no-start-daemon"]);
    if let Some(path) = &runtime {
        broken_pipe.arg("--runtime-dir").arg(path);
    }
    let failed = broken_pipe
        .args([
            "mcp",
            "client",
            "issue-credential",
            &record.client_id,
            "--expected-revision",
            "3",
            "--idempotency-key",
            "cli-closed-output",
            "--yes",
            "--secret-stdout",
        ])
        .stdout(Stdio::from(writer))
        .stderr(Stdio::piped())
        .output()
        .expect("join closed-pipe CLI");
    assert!(!failed.status.success());
    let error = String::from_utf8_lossy(&failed.stderr);
    assert!(error.contains("credential was issued, but output failed"));
    assert!(!error.contains(&secret));
    let current = owner
        .mcp_clients_list(McpClientsListParams::default())
        .await
        .expect("one closed-pipe rotation");
    assert_eq!(current.items[0].revision, 4);
    assert!(current.items[0].revoked_at_ms.is_none());
    shutdown.store(true, Ordering::Release);
    assert!(daemon.await.expect("shutdown isolated Core").is_ok());
    if let Some(path) = runtime {
        let _ = std::fs::remove_dir_all(path);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn oauth_cli_registration_and_consent_preserve_one_time_output_rules() {
    use alcomd_protocol::*;
    use std::process::Stdio;
    let _serial = RPC_TEST_LOCK.lock().await;
    let (ipc, config, runtime) = isolated_configuration();
    let shutdown = Arc::new(AtomicBool::new(false));
    let daemon =
        alcomd_daemon::test_support::start_until(ipc, wait_for_shutdown(Arc::clone(&shutdown)))
            .await
            .expect("owned isolated OAuth CLI Core");
    let mut owner = AlcomdClient::connect(config.clone())
        .await
        .expect("native owner");
    let record = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "OAuth CLI isolated".into(),
            idempotency_key: "cli-oauth-client".into(),
        })
        .await
        .expect("explicit client grants");
    let base = [
        "mcp",
        "oauth",
        "registration",
        "set",
        record.client_id.as_str(),
        "--port",
        "5443",
        "--redirect-uri",
        "http://localhost:3456/callback",
        "--confidential",
        "--allow-refresh",
        "--expected-revision",
        "0",
        "--idempotency-key",
        "cli-oauth-create",
        "--yes",
    ];
    let rejected = run_group_cli(runtime.clone(), &base);
    assert!(!rejected.status.success());
    assert!(
        owner
            .oauth_registrations_list(McpOAuthPageParams::default())
            .await
            .expect("no mutation")
            .items
            .is_empty()
    );
    let mut allowed = base.to_vec();
    allowed.push("--secret-stdout");
    let issued = run_group_cli(runtime.clone(), &allowed);
    assert!(issued.status.success());
    let result: serde_json::Value =
        serde_json::from_slice(&issued.stdout).expect("explicit one-time JSON");
    let secret = result["result"]["clientSecret"]
        .as_str()
        .expect("secret")
        .to_string();
    assert_eq!(secret.len(), 64);
    assert!(!String::from_utf8_lossy(&issued.stderr).contains(&secret));
    let repeated = run_group_cli(runtime.clone(), &allowed);
    assert!(!repeated.status.success());
    let list = owner
        .oauth_registrations_list(McpOAuthPageParams::default())
        .await
        .expect("registration list");
    assert_eq!(list.items.len(), 1);
    assert_eq!(list.items[0].revision, 1);
    assert!(
        !serde_json::to_string(&list)
            .expect("ordinary list")
            .contains(&secret)
    );
    let (reader, writer) = std::io::pipe().expect("real closed output pipe");
    drop(reader);
    let mut command = Command::new(env!("CARGO_BIN_EXE_alcomd-cli"));
    command.args(["--json", "--no-start-daemon"]);
    if let Some(path) = &runtime {
        command.arg("--runtime-dir").arg(path);
    }
    let failed = command
        .args([
            "mcp",
            "oauth",
            "registration",
            "set",
            &record.client_id,
            "--port",
            "5443",
            "--redirect-uri",
            "http://localhost:3456/callback",
            "--confidential",
            "--allow-refresh",
            "--expected-revision",
            "1",
            "--idempotency-key",
            "cli-oauth-closed-output",
            "--yes",
            "--secret-stdout",
        ])
        .stdout(Stdio::from(writer))
        .stderr(Stdio::piped())
        .output()
        .expect("join closed-pipe CLI");
    assert!(!failed.status.success());
    let error = String::from_utf8_lossy(&failed.stderr);
    assert!(error.contains("credential was issued, but output failed"));
    assert!(!error.contains(&secret));
    let after = owner
        .oauth_registrations_list(McpOAuthPageParams::default())
        .await
        .expect("authoritative list");
    assert_eq!(after.items[0].revision, 2);
    let correction = run_group_cli(
        runtime.clone(),
        &[
            "mcp",
            "oauth",
            "registration",
            "set",
            &record.client_id,
            "--port",
            "5443",
            "--redirect-uri",
            "http://localhost:3456/callback",
            "--confidential",
            "--allow-refresh",
            "--expected-revision",
            "2",
            "--idempotency-key",
            "cli-oauth-explicit-recovery",
            "--yes",
            "--secret-stdout",
        ],
    );
    assert!(correction.status.success());
    let corrected: serde_json::Value =
        serde_json::from_slice(&correction.stdout).expect("explicit recovery response");
    let new_secret = corrected["result"]["clientSecret"]
        .as_str()
        .expect("new secret")
        .to_string();
    assert!(new_secret != secret);
    let registration = owner
        .oauth_registrations_list(McpOAuthPageParams::default())
        .await
        .expect("current registration")
        .items
        .remove(0);
    assert_eq!(registration.revision, 3);
    let mut endpoint = AlcomdClient::connect(config.clone().with_oauth_endpoint())
        .await
        .expect("restricted OAuth endpoint");
    let begin = endpoint
        .oauth_begin(McpOAuthBeginParams {
            oauth_client_id: registration.oauth_client_id.clone(),
            issuer: registration.issuer.clone(),
            resource: registration.resource.clone(),
            redirect_uri: registration.redirect_uris[0].clone(),
            code_challenge: "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM".into(),
            code_challenge_method: "S256".into(),
            scopes: vec!["mcp".into()],
            state: None,
        })
        .await
        .expect("real pending request");
    let shown = run_group_cli(
        runtime.clone(),
        &["mcp", "oauth", "request", "show", &begin.request_id],
    );
    assert!(shown.status.success());
    let shown_json: serde_json::Value = serde_json::from_slice(&shown.stdout).expect("review JSON");
    let fp = shown_json["result"]["approvalFingerprint"]
        .as_str()
        .expect("fingerprint");
    assert!(!String::from_utf8_lossy(&shown.stdout).contains(&new_secret));
    assert!(!String::from_utf8_lossy(&shown.stdout).contains(&begin.browser_secret));
    let approved = run_group_cli(
        runtime.clone(),
        &[
            "mcp",
            "oauth",
            "request",
            "approve",
            &begin.request_id,
            "--expected-fingerprint",
            fp,
            "--idempotency-key",
            "cli-oauth-approve",
            "--yes",
        ],
    );
    assert!(approved.status.success());
    let finished = endpoint
        .oauth_finish(McpOAuthFinishParams {
            request_id: begin.request_id,
            browser_secret: begin.browser_secret,
        })
        .await
        .expect("approved browser continuation");
    let url = finished.redirect_url.expect("callback");
    let code = url
        .split_once('?')
        .expect("query")
        .1
        .split('&')
        .find_map(|p| p.strip_prefix("code="))
        .expect("code");
    let token = endpoint
        .oauth_token(McpOAuthTokenParams {
            grant_type: "authorization_code".into(),
            oauth_client_id: registration.oauth_client_id.clone(),
            resource: registration.resource.clone(),
            code: Some(code.into()),
            redirect_uri: Some(registration.redirect_uris[0].clone()),
            code_verifier: Some("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".into()),
            refresh_token: None,
            scopes: None,
            client_secret: Some(new_secret),
        })
        .await
        .expect("real code exchange after CLI approval");
    assert_eq!(token.scope, "mcp");
    let mut authenticated =
        AlcomdClient::connect(config.with_oauth_access(token.access_token, registration.resource))
            .await
            .expect("OAuth identity");
    assert!(authenticated.system_status().await.is_ok());
    let revoked = run_group_cli(
        runtime.clone(),
        &[
            "mcp",
            "oauth",
            "registration",
            "revoke",
            &registration.oauth_client_id,
            "--expected-revision",
            "3",
            "--idempotency-key",
            "cli-oauth-revoke",
            "--yes",
        ],
    );
    assert!(revoked.status.success());
    assert!(
        matches!(authenticated.system_status().await,Err(alcomd_client::ClientError::Remote(ref e)) if e.code=="permission_denied" && e.data.as_ref().is_some_and(|v|v["oauthError"]=="invalid_client"))
    );
    drop(authenticated);
    drop(endpoint);
    drop(owner);
    shutdown.store(true, Ordering::Release);
    assert!(daemon.await.expect("stop isolated Core").is_ok());
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
