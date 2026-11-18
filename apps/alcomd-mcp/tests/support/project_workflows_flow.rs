use super::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn frozen_sdk_http_project_and_backup_workflows_use_core() {
    let _serial = REAL_CORE_LOCK.lock().await;
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/m8-business-tests")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&root).expect("isolated root");
    let root = root.canonicalize().expect("canonical fixture root");
    let projects = root.join("projects");
    let archives = root.join("archives");
    std::fs::create_dir(&archives).expect("isolated archives");
    project(&projects.join("Source"), false);
    let (stop_core, stopped) = tokio::sync::oneshot::channel();
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        IpcConfig::default(),
        DataConfig::isolated(root.join("data")),
        async {
            let _ = stopped.await;
        },
    )
    .await
    .expect("exclusive isolated Core");
    let mut owner = AlcomdClient::connect(ClientConfig::default().without_daemon_start())
        .await
        .expect("owner");
    let settings = owner.settings_get().await.expect("settings");
    owner.settings_update(serde_json::from_value(json!({
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
            grants: Vec::new(),
        })
        .await
        .expect("zero-grant client");
    let credential = owner
        .mcp_client_issue_credential(McpClientMutationParams {
            client_id: actor.client_id.clone(),
            expected_revision: actor.revision,
            idempotency_key: "workflow-credential".into(),
        })
        .await
        .expect("independent credential");
    let http = HttpFixture::start().await;
    let sdk = sdk(&credential.credential, Some(&http.url)).await;
    let mut trace = Vec::new();
    let register = json!({
        "source":{"kind":"default","relative_path":"Source"},
        "idempotency_key":"workflow-register"
    });
    error(
        call(&sdk, &mut trace, "register_project", register.clone())
            .await
            .expect("zero grant"),
        "permission_denied",
    );
    let actor = update(
        &mut owner,
        &McpClientRecord {
            revision: credential.revision,
            ..actor
        },
        [
            "projects.manage",
            "projects.read",
            "backups.read",
            "backups.manage",
            "operations.read",
        ]
        .map(|name| grant(name, "global", "*"))
        .to_vec(),
        "workflow-allow",
    )
    .await;
    let registered = data(
        call(&sdk, &mut trace, "register_project", register)
            .await
            .expect("register tool"),
    );
    let project_id = registered["project"]["project_id"]
        .as_str()
        .expect("registered ID");
    assert!(owner.project_get(project_id.to_owned()).await.is_ok());
    assert!(registered["project"].get("root_path").is_none());
    let backup = json!({"project_id":project_id,"expected_revision":registered["project"]["revision"],"idempotency_key":"workflow-backup"});
    let accepted = data(
        call(&sdk, &mut trace, "create_backup", backup.clone())
            .await
            .expect("backup tool"),
    );
    let receipt = terminal(
        &sdk,
        &mut trace,
        accepted["operation_id"].as_str().expect("operation"),
    )
    .await;
    assert_eq!(receipt["state"], "succeeded");
    assert_eq!(receipt["result"]["backup_id"], accepted["backup_id"]);
    assert_eq!(
        receipt["result"]["archive_sha256"].as_str().map(str::len),
        Some(64)
    );
    let record = data(
        call(
            &sdk,
            &mut trace,
            "get_backup",
            json!({"backup_id":accepted["backup_id"]}),
        )
        .await
        .expect("backup details"),
    );
    assert_eq!(record, receipt["result"]);
    assert_eq!(
        std::fs::read_dir(&archives)
            .expect("archive published")
            .count(),
        1
    );
    update(&mut owner, &actor, Vec::new(), "workflow-revoke").await;
    error(
        call(&sdk, &mut trace, "create_backup", backup)
            .await
            .expect("revoked on same client"),
        "permission_denied",
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
