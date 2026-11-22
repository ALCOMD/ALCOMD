use super::*;

#[cfg(windows)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires the reviewed Claude Code executable and an exclusive Windows Core endpoint"]
async fn claude_code_http_setup_uses_isolated_profile_and_real_core() {
    use tokio::io::AsyncWriteExt;

    let executable = std::path::PathBuf::from(
        std::env::var_os("ALCOMD_TEST_CLAUDE_EXE").expect("reviewed Claude Code executable"),
    );
    assert!(executable.is_absolute() && executable.is_file());
    let _serial = REAL_CORE_LOCK.lock().await;
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/m8-business-tests")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&root).expect("isolated root");
    let root = root.canonicalize().expect("canonical fixture root");
    let profile = root.join("claude-profile");
    std::fs::create_dir_all(&profile).expect("isolated profile");
    let (stop_core, stopped) = tokio::sync::oneshot::channel();
    let core = CoreConfiguration::new();
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        core.ipc.clone(),
        DataConfig::isolated(root.join("data")),
        async {
            let _ = stopped.await;
        },
    )
    .await
    .expect("exclusive isolated Core");
    let mut owner = AlcomdClient::connect(core.client.clone())
        .await
        .expect("owner");
    let actor = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "Claude Code fixture".into(),
            idempotency_key: "claude-fixture-client".into(),
        })
        .await
        .expect("dedicated client");
    let issued = owner
        .mcp_client_issue_credential(McpClientMutationParams {
            client_id: actor.client_id,
            expected_revision: actor.revision,
            idempotency_key: "claude-fixture-credential".into(),
        })
        .await
        .expect("dedicated credential");
    let http = HttpFixture::start(&core).await;
    let configuration = json!({
        "type": "http",
        "url": http.url,
        "headers": {"Authorization": "Bearer ${ALCOMD_CLAUDE_BEARER}"}
    });
    let profile_file = profile.join(".claude.json");
    let sentinel = json!({"type": "http", "url": "http://127.0.0.1:1/mcp"});
    std::fs::write(
        &profile_file,
        json!({"mcpServers": {"unrelated-fixture": sentinel}}).to_string(),
    )
    .expect("seed unrelated fixture entry");

    let run = async |arguments: &[&str], input: Option<&serde_json::Value>| {
        let mut command = tokio::process::Command::new(&executable);
        command
            .creation_flags(0x08000000)
            .current_dir(&root)
            .env_clear()
            .env("CLAUDE_CONFIG_DIR", &profile)
            .env("DISABLE_NONESSENTIAL_TRAFFIC", "1")
            .env("DISABLE_AUTOUPDATER", "1")
            .env("ENABLE_CLAUDEAI_MCP_SERVERS", "false")
            .env("ALCOMD_CLAUDE_BEARER", &issued.credential)
            .args(arguments)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        for key in ["SystemRoot", "WINDIR", "PATH"] {
            if let Some(value) = std::env::var_os(key) {
                command.env(key, value);
            }
        }
        for key in ["USERPROFILE", "APPDATA", "LOCALAPPDATA", "TEMP", "TMP"] {
            command.env(key, &profile);
        }
        let mut child = command.spawn().expect("start isolated Claude Code");
        let mut stdin = child.stdin.take().expect("stdin");
        if let Some(input) = input {
            stdin
                .write_all(input.to_string().as_bytes())
                .await
                .expect("send config without a credential");
        }
        drop(stdin);
        tokio::time::timeout(Duration::from_secs(20), child.wait_with_output())
            .await
            .expect("Claude Code command deadline")
            .expect("Claude Code exit")
    };
    let read_config = || {
        let bytes = std::fs::read(&profile_file).expect("read isolated Claude profile");
        assert!(
            !bytes
                .windows(issued.credential.len())
                .any(|field| field == issued.credential.as_bytes()),
            "only the environment variable reference belongs in the profile"
        );
        let profile: serde_json::Value = serde_json::from_slice(&bytes).expect("parse profile");
        assert_eq!(profile["mcpServers"]["unrelated-fixture"], sentinel);
        profile
    };
    let add = ["mcp", "add-json", "alcomd", "-", "--scope", "user"];
    assert!(run(&add, Some(&configuration)).await.status.success());
    assert_eq!(read_config()["mcpServers"]["alcomd"], configuration);
    assert!(!run(&add, Some(&sentinel)).await.status.success());
    assert_eq!(read_config()["mcpServers"]["alcomd"], configuration);
    let result = run(&["mcp", "get", "alcomd"], None).await;
    let status = String::from_utf8_lossy(&result.stdout);
    assert!(
        result.status.success()
            && status
                .lines()
                .any(|line| line.trim().starts_with("Status:") && line.ends_with("Connected")),
        "Claude Code did not report a healthy direct HTTP connection"
    );
    assert!(
        run(&["mcp", "remove", "alcomd", "--scope", "user"], None)
            .await
            .status
            .success()
    );
    assert!(read_config()["mcpServers"].get("alcomd").is_none());
    http.finish().await;
    drop(owner);
    stop_core.send(()).expect("stop Core");
    daemon.await.expect("join").expect("Core shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn frozen_sdk_http_project_and_backup_workflows_use_core() {
    use rmcp::model::{ClientRequest, Request};
    use rmcp::service::PeerRequestOptions;

    let _serial = REAL_CORE_LOCK.lock().await;
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/m8-business-tests")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&root).expect("isolated root");
    let root = root.canonicalize().expect("canonical fixture root");
    let projects = root.join("projects");
    let archives = root.join("archives");
    let external = root.join("外部 路径");
    std::fs::create_dir(&projects).expect("isolated projects");
    std::fs::create_dir(&archives).expect("isolated archives");
    let source = external.join("源 工程");
    project(&source, false);
    let (stop_core, stopped) = tokio::sync::oneshot::channel();
    let core = CoreConfiguration::new();
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        core.ipc.clone(),
        DataConfig::isolated(root.join("data")),
        async {
            let _ = stopped.await;
        },
    )
    .await
    .expect("exclusive isolated Core");
    let mut owner = AlcomdClient::connect(core.client.clone())
        .await
        .expect("owner");
    let settings = owner.settings_get().await.expect("settings");
    owner.settings_update(serde_json::from_value(json!({
        "idempotencyKey": "workflow-fixture-paths",
        "expectedRevision": settings.revision,
        "update": {
            "paths": {"projectDirectory":projects.to_string_lossy(),"backupDirectory":archives.to_string_lossy()},
            "projects":{"autoDiscover":false}
        }
    })).expect("fixture settings")).await.expect("isolated storage");
    let actor = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "Project workflows fixture".into(),
            idempotency_key: "workflow-client".into(),
        })
        .await
        .expect("registered client");
    let credential = owner
        .mcp_client_issue_credential(McpClientMutationParams {
            client_id: actor.client_id.clone(),
            expected_revision: actor.revision,
            idempotency_key: "workflow-credential".into(),
        })
        .await
        .expect("independent credential");
    let http = HttpFixture::start(&core).await;
    let sdk = sdk(&credential.credential, Some(&http.url), &core).await;
    let mut trace = Vec::new();
    let register = json!({
        "source":{"kind":"absolute","path":source},
        "idempotency_key":"workflow-register"
    });
    let registered = data(
        call(&sdk, &mut trace, "register_project", register)
            .await
            .expect("register tool"),
    );
    let project_id = registered["project"]["project_id"]
        .as_str()
        .expect("registered ID");
    assert!(owner.project_get(project_id.to_owned()).await.is_ok());
    let shared = owner
        .project_get(project_id.to_owned())
        .await
        .expect("owner reads the same project")
        .project;
    assert_eq!(registered["project"]["root_path"], shared.root_path);
    assert_eq!(
        registered["project"]["favorite"],
        serde_json::json!(shared.favorite)
    );
    error(
        call(
            &sdk,
            &mut trace,
            "plan_project_copy",
            json!({
                "source_project_id": project_id,
                "expected_revision": registered["project"]["revision"],
                "target": {"kind":"absolute","parent_path":source.parent().expect("fixture project parent"),"name":source.file_name().expect("fixture project name").to_str().expect("UTF-8 fixture name")},
                "idempotency_key":"copy-existing-target"
            }),
        )
        .await
        .expect("Core copy rejection"),
        "project_copy_target_exists",
    );
    let archive = external.join("第一份 备份.zip");
    let backup = json!({
        "project_id": project_id,
        "expected_revision": registered["project"]["revision"],
        "destination_path": archive,
        "idempotency_key": "workflow-backup"
    });
    let held_backup = daemon.hold_next_mcp_backup_response().await;
    let mut pending = sdk
        .send_cancellable_request(
            ClientRequest::CallToolRequest(Request::new(
                CallToolRequestParams::new("alcomd_create_backup")
                    .with_arguments(backup.as_object().expect("backup arguments").clone()),
            )),
            PeerRequestOptions::no_options(),
        )
        .await
        .expect("actual SDK backup request");
    let original = tokio::time::timeout(Duration::from_secs(10), held_backup.accepted())
        .await
        .expect("backup accepted while its reply is held");
    assert!(!original.replayed);
    let (_unused, replacement) = tokio::sync::oneshot::channel();
    let response = std::mem::replace(&mut pending.rx, replacement);
    pending
        .cancel(Some("isolated backup request cancellation".into()))
        .await
        .expect("cancel the SDK request");
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(5), response)
            .await
            .expect("protocol request cancellation deadline")
            .expect("SDK result"),
        Err(ServiceError::Cancelled { .. })
    ));
    let visible_project = tokio::time::timeout(
        Duration::from_secs(5),
        call(
            &sdk,
            &mut trace,
            "get_project",
            json!({"project_id": project_id}),
        ),
    )
    .await
    .expect("cancelled request releases the adapter before the Core reply")
    .expect("same SDK can issue another request");
    assert_eq!(data(visible_project)["project_id"], project_id);
    held_backup.release();
    let accepted = data(
        call(&sdk, &mut trace, "create_backup", backup.clone())
            .await
            .expect("recover the accepted backup with the original key"),
    );
    assert_eq!(accepted["operation_id"], original.operation_id);
    assert_eq!(accepted["replayed"], true);
    let receipt = terminal(
        &sdk,
        &mut trace,
        accepted["operation_id"].as_str().expect("operation"),
    )
    .await;
    assert_eq!(receipt["state"], "succeeded", "{receipt}");
    assert_eq!(receipt["result"]["archive_name"], "第一份 备份.zip");
    assert_eq!(
        receipt["result"]["archive_bytes"].as_u64(),
        Some(std::fs::metadata(&archive).expect("explicit archive").len())
    );
    assert!(!receipt["result"].to_string().contains("外部 路径"));

    let restored_target = external.join("已 恢复 工程");
    let restored = data(
        call(
            &sdk,
            &mut trace,
            "restore_backup",
            json!({
                "archive_path":archive,
                "destination_path":restored_target,
                "idempotency_key":"workflow-restore",
                "confirm":true
            }),
        )
        .await
        .expect("restore from a ZIP outside the default backup directory"),
    );
    let restored_task = terminal(
        &sdk,
        &mut trace,
        restored["operation_id"]
            .as_str()
            .expect("restore operation"),
    )
    .await;
    assert_eq!(restored_task["state"], "succeeded", "{restored_task}");
    assert_eq!(restored_task["result"]["target_name"], "已 恢复 工程");
    assert!(!restored_task["result"].to_string().contains("外部 路径"));
    assert!(
        restored_target
            .join("ProjectSettings/ProjectVersion.txt")
            .is_file()
    );
    let restored_id = restored_task["result"]["project_id"]
        .as_str()
        .expect("restored project ID");
    assert!(owner.project_get(restored_id.to_owned()).await.is_ok());

    let lost_archive = external.join("第二份 备份.zip");
    let mut lost_response = backup.clone();
    lost_response["idempotency_key"] = json!("workflow-backup-lost-response");
    lost_response["destination_path"] = json!(lost_archive);
    let (fault_sdk, fault) = HttpResponseFault::start(&credential.credential, &http.url).await;
    fault.armed.store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(
        call(
            &fault_sdk,
            &mut trace,
            "create_backup",
            lost_response.clone()
        )
        .await
        .is_err(),
        "an incomplete HTTP reply must be reported by the actual SDK"
    );
    assert!(
        !fault.armed.load(std::sync::atomic::Ordering::SeqCst),
        "the injector must truncate a real response"
    );
    tokio::time::timeout(Duration::from_secs(20), async {
        while !lost_archive.is_file() {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("backup publishes despite the lost HTTP response");
    let recovered = data(
        call(&sdk, &mut trace, "create_backup", lost_response)
            .await
            .expect("explicit same-key recovery"),
    );
    assert_eq!(recovered["replayed"], true);
    let recovered_task = terminal(
        &sdk,
        &mut trace,
        recovered["operation_id"]
            .as_str()
            .expect("recovered operation"),
    )
    .await;
    assert_eq!(recovered_task["state"], "succeeded");
    assert_eq!(recovered_task["result"]["archive_name"], "第二份 备份.zip");
    assert!(archive.is_file() && lost_archive.is_file());
    assert_eq!(
        std::fs::read_dir(&external)
            .expect("no duplicate archive")
            .map(|entry| entry.expect("external fixture entry").path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "zip"))
            .count(),
        2
    );
    assert_eq!(
        std::fs::read_dir(&archives)
            .expect("default archives")
            .count(),
        0
    );
    stop(fault_sdk).await;
    drop(fault);
    owner
        .mcp_client_revoke(McpClientMutationParams {
            client_id: actor.client_id,
            expected_revision: credential.revision,
            idempotency_key: "workflow-revoke".into(),
        })
        .await
        .expect("revoke connection");
    assert!(
        call(&sdk, &mut trace, "create_backup", backup)
            .await
            .is_err()
    );
    std::fs::write(
        root.join("project-workflows-flow.json"),
        serde_json::to_vec_pretty(&trace).expect("trace"),
    )
    .expect("persist evidence");
    stop(sdk).await;
    http.finish().await;
    drop(owner);
    stop_core.send(()).expect("stop Core");
    daemon.await.expect("join").expect("Core shutdown");
}
