use super::*;

#[cfg(windows)]
#[path = "../../../../crates/alcomd-platform/test-support/windows_unity_editor.rs"]
mod windows_editor_fixture;

fn downgrade_editor(root: &Path) -> std::path::PathBuf {
    let dir = root.join("Editor");
    let metadata = dir.join("Data/Resources/PackageManager/Editor/manifest.json");
    std::fs::create_dir_all(metadata.parent().expect("manifest parent"))
        .expect("create Editor metadata directory");
    std::fs::write(metadata, r#"{"schemaVersion":4,"packages":{}}"#)
        .expect("write Editor manifest");
    #[cfg(windows)]
    {
        let executable = dir.join("Unity.exe");
        windows_editor_fixture::write_editor_fixture(
            &executable,
            "Unity",
            "2022.3.6f1_887be4894c44",
        )
        .expect("write Windows Editor fixture");
        executable
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let executable = dir.join("Unity");
        std::fs::write(
            &executable,
            "#!/bin/sh\nprintf '2022.3.6f1 (887be4894c44)\\n'\n",
        )
        .expect("write Unix Editor fixture");
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700))
            .expect("mark Editor fixture executable");
        executable
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn project_catalog_parity() {
    let _serial = REAL_CORE_LOCK.lock().await;
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/m8-business-tests")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&root).expect("create isolated test directory");
    let root = root.canonicalize().expect("canonical test directory");
    let directory = root.join("Directory");
    project(&directory, false);
    let local = root.join("Source");
    std::fs::create_dir(&local).expect("create local package directory");
    std::fs::write(
        local.join("package.json"),
        r#"{"name":"com.example.parity","version":"1.0.0"}"#,
    )
    .expect("write local package manifest");
    let (shutdown, stopped) = tokio::sync::oneshot::channel();
    let core = CoreConfiguration::new();
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        core.ipc.clone(),
        DataConfig::isolated(root.join("data")),
        async {
            let _ = stopped.await;
        },
    )
    .await
    .expect("start isolated Core");
    let mut owner = AlcomdClient::connect(core.client.clone())
        .await
        .expect("open native owner connection");
    let actor = owner
        .mcp_client_create(McpClientsCreateParams {
            idempotency_key: "parity-client".into(),
            display_name: "Project catalog parity".into(),
        })
        .await
        .expect("create MCP client");
    let issued = owner
        .mcp_client_issue_credential(McpClientMutationParams {
            client_id: actor.client_id,
            expected_revision: actor.revision,
            idempotency_key: "parity-credential".into(),
        })
        .await
        .expect("issue MCP credential");
    let http = HttpFixture::start(&core).await;
    let connection = sdk(&issued.credential, Some(&http.url), &core).await;
    let mut trace = Vec::new();
    let tools = connection
        .list_all_tools()
        .await
        .expect("read all MCP tools");
    for tool in serde_json::from_str::<Value>(include_str!(
        "../../../../specs/mcp/project-catalog-v1.schema.json"
    ))
    .expect("read named tool schema")["x-alcomd-tools"]
        .as_array()
        .expect("array of named tools")
    {
        assert!(
            tools
                .iter()
                .any(|actual| Some(actual.name.as_ref()) == tool["name"].as_str()),
            "{}",
            tool["name"]
        );
    }
    macro_rules! success {
        ($name:literal, $params:expr) => {{
            let result = call(&connection, &mut trace, $name, $params)
                .await
                .expect("call named tool");
            assert_ne!(
                result.is_error,
                Some(true),
                "{}: {:?}",
                $name,
                result.structured_content
            );
            data(result)
        }};
    }
    let inspected = success!(
        "inspect_project",
        json!({"path":directory.join("Assets"),"discovery_mode":"search-parents"})
    );
    assert!(inspected["project"]["project_id"].is_null());
    let registered = owner
        .project_register(alcomd_protocol::ProjectRegisterParams {
            source: alcomd_protocol::ProjectSource::Absolute {
                path: directory.to_string_lossy().into_owned(),
            },
            idempotency_key: "register".into(),
        })
        .await
        .expect("register shared project")
        .project;
    let id = registered.project_id.expect("registered project ID");
    let set_favorite = json!({"project_id":id,"favorite":true,"expected_revision":registered.revision,"confirm":true,"idempotency_key":"favorite"});
    let favorite = success!("set_project_favorite", set_favorite.clone());
    assert_eq!(favorite["project"]["favorite"], true);
    assert_eq!(
        success!("set_project_favorite", set_favorite)["replayed"],
        true
    );
    let page = success!(
        "list_projects",
        json!({"view":{"sort":"name","descending":false,"search":"Directory"}})
    );
    assert_eq!(page["projects"][0]["project_id"], id);
    assert_eq!(page["projects"][0]["favorite"], true);
    assert!(page["view_sequence"].is_u64());
    let refreshed = success!(
        "refresh_project",
        json!({"project_id":id,"expected_revision":favorite["project"]["revision"],"confirm":true,"idempotency_key":"refresh"})
    );
    let revision = refreshed["project"]["revision"]
        .as_u64()
        .expect("refreshed project revision");
    let target = owner
        .unity_installation_register(UnityInstallationRegisterParams {
            executable_path: downgrade_editor(&root).to_string_lossy().into_owned(),
            idempotency_key: "register-editor".into(),
        })
        .await
        .expect("register fixture Editor")
        .installation;
    let migration = success!(
        "plan_project_unity_migration",
        json!({
            "project_id":id,"target_installation_id":target.installation_id,"expected_project_revision":revision,"idempotency_key":"migration-preview"
        })
    );
    let migration_id = migration["plan"]["plan_id"]
        .as_str()
        .expect("migration plan ID");
    assert_eq!(
        success!(
            "get_plan",
            json!({"kind":"project_unity_migration","plan_id":migration_id})
        )["plan"],
        migration["plan"]
    );
    error(
        call(
            &connection,
            &mut trace,
            "apply_project_unity_migration",
            json!({
                "plan_id":migration_id,"idempotency_key":"unsupported-downgrade","confirm":true
            }),
        )
        .await
        .expect("call unsupported migration"),
        "project_unity_migration_unsupported",
    );
    let enrolled = success!(
        "enroll_user_package",
        json!({"source_path":local,"idempotency_key":"enroll","confirm":true})
    );
    let package = &enrolled["user_package"];
    let source_id = package["user_package_id"]
        .as_str()
        .expect("enrolled package ID");
    let listed = success!("list_user_packages", json!({"limit":1}));
    assert_eq!(listed["user_packages"][0], *package);
    assert_eq!(
        success!("get_user_package", json!({"user_package_id":source_id}))["user_package"],
        *package
    );
    assert_eq!(
        owner
            .user_package_get(UserPackageGetParams {
                user_package_id: source_id.into()
            })
            .await
            .expect("read shared local package as owner")
            .user_package
            .package_id,
        "com.example.parity"
    );
    let candidates = success!(
        "query_project_candidates",
        json!({"project_id":id,"expected_revision":revision,"view":{"kind":"versions","package_id":"com.example.parity","source":{"kind":"user_package","user_package_id":source_id}}})
    );
    assert_eq!(candidates["items"][0]["version"], "1.0.0", "{candidates}");
    let package = success!(
        "refresh_user_package",
        json!({"user_package_id":source_id,"expected_revision":package["revision"],"idempotency_key":"refresh-source","confirm":true})
    );
    let removed = success!(
        "remove_user_package",
        json!({"user_package_id":source_id,"expected_revision":package["user_package"]["revision"],"idempotency_key":"remove-source","confirm":true})
    );
    assert_eq!(removed["removed"], true);
    assert!(local.is_dir());
    let delete = success!(
        "plan_project_delete_directory",
        json!({"project_id":id,"expected_revision":revision,"idempotency_key":"delete-plan"})
    );
    let plan = &delete["plan"];
    let original = success!(
        "get_plan",
        json!({"kind":"project_delete_directory","plan_id":plan["plan_id"]})
    );
    assert_eq!(original["plan"], *plan);
    assert!(directory.is_dir());
    let accepted = success!(
        "apply_project_delete_directory",
        json!({"plan_id":plan["plan_id"],"expected_revision":revision,"idempotency_key":"apply","confirm":true})
    );
    let terminal = terminal(
        &connection,
        &mut trace,
        accepted["operation_id"]
            .as_str()
            .expect("accepted deletion operation ID"),
    )
    .await;
    assert_eq!(terminal["state"], "succeeded", "{terminal}");
    assert!(!directory.exists());
    let retained = success!(
        "get_plan",
        json!({"kind":"project_unity_migration","plan_id":migration_id})
    );
    assert_eq!(retained["plan"], migration["plan"]);
    let other_dir = root.join("Keep");
    project(&other_dir, false);
    let other = owner
        .project_register(alcomd_protocol::ProjectRegisterParams {
            source: alcomd_protocol::ProjectSource::Absolute {
                path: other_dir.to_string_lossy().into_owned(),
            },
            idempotency_key: "keep-directory".into(),
        })
        .await
        .expect("register second shared project")
        .project;
    let result = success!(
        "unregister_project",
        json!({"project_id":other.project_id,"expected_revision":other.revision,"idempotency_key":"unregister","confirm":true})
    );
    assert_eq!(result["unregistered"], true);
    assert!(other_dir.is_dir());
    drop(connection);
    http.finish().await;
    drop(owner);
    shutdown.send(()).expect("signal Core shutdown");
    daemon
        .await
        .expect("join Core task")
        .expect("clean Core shutdown");
}
