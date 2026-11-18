use super::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn frozen_sdk_catalog_install_and_reinstall_real_core() {
    let _serial = REAL_CORE_LOCK.lock().await;
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/m8-business-tests")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&root).expect("owned evidence root");
    let root = root.canonicalize().expect("fixture root");
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        IpcConfig::default(),
        DataConfig::isolated(root.join("data")),
        std::future::pending(),
    )
    .await
    .expect("exclusive real Core bind");
    let mut owner = AlcomdClient::connect(ClientConfig::default().without_daemon_start())
        .await
        .expect("native test owner");
    let fixture = root.join("Project");
    project(&fixture, false);
    let project_record = owner
        .project_register(
            fixture.to_string_lossy().into_owned(),
            "catalog-project".into(),
        )
        .await
        .expect("register project")
        .project;
    let project_id = project_record.project_id.expect("project ID");
    let revision = project_record.revision.expect("registered revision");
    let installed = fixture.join("Packages/com.example.catalog");
    let mut packages = Vec::new();
    for (name, version, dependencies) in [
        (
            "com.example.catalog",
            "1.0.0",
            json!({"com.example.dependency":"1.0.0"}),
        ),
        ("com.example.dependency", "1.0.0", json!({})),
    ] {
        let source = root.join(format!("{name}-{version}"));
        std::fs::create_dir(&source).expect("isolated user source");
        std::fs::write(source.join("package.json"), json!({
            "name":name, "version":version, "displayName":"Catalog fixture", "description":"Delegated package", "vpmDependencies":dependencies,
        }).to_string()).expect("source manifest");
        std::fs::write(source.join("fixture.txt"), version).expect("package content");
        packages.push(
            owner
                .user_package_enroll(UserPackageEnrollParams {
                    source_path: source.to_string_lossy().into_owned(),
                    idempotency_key: format!("enroll-{name}-{version}"),
                })
                .await
                .expect("Core enrolls package")
                .user_package,
        );
    }
    let a = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "Catalog A".into(),
            grants: Vec::new(),
            idempotency_key: "catalog-a".into(),
        })
        .await
        .expect("client A");
    let b = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "Catalog B".into(),
            grants: grants(true),
            idempotency_key: "catalog-b".into(),
        })
        .await
        .expect("client B");
    let ca = owner
        .mcp_client_issue_credential(McpClientMutationParams {
            client_id: a.client_id.clone(),
            expected_revision: a.revision,
            idempotency_key: "issue-catalog-a".into(),
        })
        .await
        .expect("credential A");
    let cb = owner
        .mcp_client_issue_credential(McpClientMutationParams {
            client_id: b.client_id,
            expected_revision: b.revision,
            idempotency_key: "issue-catalog-b".into(),
        })
        .await
        .expect("credential B");
    let sa = sdk(&ca.credential, None).await;
    let sb = sdk(&cb.credential, None).await;
    let mut trace = Vec::new();
    error(
        call(&sa, &mut trace, "search_packages", json!({}))
            .await
            .expect("zero grant result"),
        "permission_denied",
    );
    let a = update(
        &mut owner,
        &McpClientRecord {
            revision: ca.revision,
            ..a
        },
        grants(true),
        "catalog-authorize",
    )
    .await;
    let first = data(
        call(&sa, &mut trace, "search_packages", json!({"limit":1}))
            .await
            .expect("global search"),
    );
    assert_eq!(first["items"][0]["package_id"], "com.example.catalog");
    assert_eq!(first["items"][0]["version"], "1.0.0");
    let next = json!({"cursor":first["next_cursor"],"limit":1});
    let second = data(
        call(&sa, &mut trace, "search_packages", next.clone())
            .await
            .expect("second page"),
    );
    assert_eq!(second["items"][0]["package_id"], "com.example.dependency");
    error(
        call(&sb, &mut trace, "search_packages", next)
            .await
            .expect("foreign cursor"),
        "revision_conflict",
    );
    let details = data(call(&sa, &mut trace, "get_package", json!({"package_id":"com.example.catalog", "project_id":project_id, "expected_revision":revision})).await.expect("versions"));
    assert_eq!(details["items"].as_array().expect("versions").len(), 1);
    assert_eq!(
        details["items"][0]["dependencies"]["com.example.dependency"],
        "1.0.0"
    );
    assert_eq!(
        details["items"][0]["compatibility"]["eligibility"],
        "eligible"
    );
    let selector = json!({"kind":"user_package","user_package_id":packages[0].user_package_id});
    let plan = data(call(&sa, &mut trace, "plan_package_install", json!({
        "project_id":project_id, "expected_revision":revision, "package_id":"com.example.catalog",
        "source":selector, "version_selector":{"kind":"exact", "version":"1.0.0"},
    })).await.expect("exact install plan"));
    assert!(!installed.exists());
    let public = plan.to_string();
    assert!(
        !public.contains("artifact_url")
            && !public.contains("source_identity")
            && !public.contains("source_root_path")
    );
    assert_eq!(plan["plan"]["action"], "install");
    assert_eq!(
        plan["sources"]
            .as_array()
            .expect("two pinned sources")
            .len(),
        2
    );
    let apply = json!({"plan_id":plan["plan"]["plan_id"],"expected_revision":revision,"idempotency_key":"catalog-install","confirm":true});
    error(
        call(&sb, &mut trace, "apply_package_changes", apply.clone())
            .await
            .expect("foreign plan"),
        "plan_not_found",
    );
    let result = data(
        call(&sa, &mut trace, "apply_package_changes", apply.clone())
            .await
            .expect("apply original install"),
    );
    let operation = result["operation_id"].as_str().expect("operation");
    let installed_state = terminal(&sa, &mut trace, operation).await;
    assert_eq!(installed_state["state"], "succeeded", "{installed_state}");
    assert_eq!(
        std::fs::read_to_string(installed.join("fixture.txt")).expect("published fixture"),
        "1.0.0"
    );
    let replay = data(
        call(&sa, &mut trace, "apply_package_changes", apply)
            .await
            .expect("same key recovery"),
    );
    assert_eq!(replay["operation_id"], operation);
    assert_eq!(replay["replayed"], true);
    let revision = owner
        .project_get(project_id.clone())
        .await
        .expect("updated project")
        .project
        .revision
        .expect("updated revision");
    std::fs::remove_file(installed.join("fixture.txt"))
        .expect("simulate disposable missing package file");
    let plan = data(call(&sa, &mut trace, "plan_package_reinstall", json!({
        "project_id":project_id, "expected_revision":revision, "package_id":"com.example.catalog", "source":selector,
    })).await.expect("reinstall original version"));
    let result = data(call(&sa, &mut trace, "apply_package_changes", json!({
        "plan_id":plan["plan"]["plan_id"],"expected_revision":revision,"idempotency_key":"catalog-reinstall","confirm":true,
    })).await.expect("apply original reinstall"));
    let reinstalled = terminal(
        &sa,
        &mut trace,
        result["operation_id"].as_str().expect("operation"),
    )
    .await;
    assert_eq!(reinstalled["state"], "succeeded", "{reinstalled}");
    assert_eq!(
        std::fs::read_to_string(installed.join("fixture.txt")).expect("reinstalled fixture"),
        "1.0.0"
    );
    update(&mut owner, &a, Vec::new(), "catalog-revoke").await;
    error(
        call(
            &sa,
            &mut trace,
            "get_package",
            json!({"package_id":"com.example.catalog"}),
        )
        .await
        .expect("revoked while connected"),
        "permission_denied",
    );
    std::fs::write(
        root.join("catalog-flow.json"),
        serde_json::to_vec_pretty(&trace).expect("evidence"),
    )
    .expect("save trace");
    sa.cancel().await.expect("stop SDK A");
    sb.cancel().await.expect("stop SDK B");
    drop(owner);
    drop(daemon);
}
