use super::*;
use std::io::Read;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn frozen_sdk_project_inventory_and_template_edit_share_core() {
    let _serial = REAL_CORE_LOCK.lock().await;
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/m8-business-tests")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&root).expect("isolated project assets fixture");
    let root = root
        .canonicalize()
        .expect("isolated project assets fixture");
    let project_path = root.join("Source");
    project(&project_path, true);
    let package_manifest = project_path.join("Packages/com.example.b/package.json");
    std::fs::remove_file(&package_manifest).expect("isolated project assets fixture");
    let source = root.join("asset.txt");
    let second_source = root.join("second.txt");
    std::fs::write(&source, "frozen asset").expect("isolated project assets fixture");
    std::fs::write(&second_source, "second asset").expect("isolated project assets fixture");

    let (stop_core, stopped) = tokio::sync::oneshot::channel();
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        IpcConfig::default(),
        DataConfig::isolated(root.join("data")),
        async {
            let _ = stopped.await;
        },
    )
    .await
    .expect("isolated project assets fixture");
    let mut owner = AlcomdClient::connect(ClientConfig::default().without_daemon_start())
        .await
        .expect("isolated project assets fixture");
    let registered = owner
        .project_register(
            project_path.to_string_lossy().into_owned(),
            "assets-register".into(),
        )
        .await
        .expect("isolated project assets fixture");
    let registered = serde_json::to_value(registered).expect("isolated project assets fixture");
    let project_id = registered["project"]["projectId"]
        .as_str()
        .expect("tool or RPC identifier");
    let revision = registered["project"]["revision"]
        .as_u64()
        .expect("reported resource revision");
    let identity = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "Project assets fixture".into(),
            idempotency_key: "assets-client".into(),
        })
        .await
        .expect("isolated project assets fixture");
    let identity_id = identity.client_id.clone();
    let issued = owner
        .mcp_client_issue_credential(McpClientMutationParams {
            client_id: identity.client_id,
            expected_revision: identity.revision,
            idempotency_key: "assets-credential".into(),
        })
        .await
        .expect("isolated project assets fixture");
    let http = HttpFixture::start().await;
    let connection = sdk(&issued.credential, Some(&http.url)).await;
    let mut trace = Vec::new();
    let mut inventory_args =
        json!({"project_id":project_id,"expected_revision":revision,"limit":1});
    let native = owner
        .project_inventory(ProjectInventoryParams {
            project_id: project_id.into(),
            expected_revision: revision,
            cursor: None,
            limit: Some(1),
        })
        .await
        .expect("isolated project assets fixture");
    assert_eq!(native.entries[0].state, InstalledEvidence::Matched);
    assert_eq!(native.resolution, ProjectResolutionEvidence::Needed);
    let page = data(
        call(
            &connection,
            &mut trace,
            "list_installed_packages",
            inventory_args.clone(),
        )
        .await
        .expect("isolated project assets fixture"),
    );
    assert_eq!(
        page["entries"][0]["package_id"],
        native.entries[0].package_id
    );
    assert_eq!(page["resolution"], "needed");
    inventory_args["cursor"] = page["next_cursor"].clone();
    assert!(inventory_args["cursor"].is_string());
    let page = data(
        call(
            &connection,
            &mut trace,
            "list_installed_packages",
            inventory_args.clone(),
        )
        .await
        .expect("isolated project assets fixture"),
    );
    assert_eq!(page["entries"][0]["state"], "missing");
    std::fs::write(
        &package_manifest,
        r#"{"name":"com.example.b","version":"2.0.0"}"#,
    )
    .expect("isolated project assets fixture");
    let stale = call(
        &connection,
        &mut trace,
        "list_installed_packages",
        inventory_args.clone(),
    )
    .await
    .expect("isolated project assets fixture");
    error(stale, "package_candidate_evidence_stale");
    let new = owner
        .project_inventory(ProjectInventoryParams {
            project_id: project_id.into(),
            expected_revision: revision,
            cursor: None,
            limit: Some(50),
        })
        .await
        .expect("isolated project assets fixture");
    assert_eq!(new.entries[1].state, InstalledEvidence::VersionMismatch);
    assert_eq!(new.entries[1].observed_version.as_deref(), Some("2.0.0"));

    let blank = "com.vrclearn.alcomd.templates.blank";
    let detail = data(
        call(
            &connection,
            &mut trace,
            "get_project_template",
            json!({"id":blank}),
        )
        .await
        .expect("isolated project assets fixture"),
    );
    assert_eq!(detail["template"]["locked"], true);
    error(call(&connection, &mut trace, "edit_project_template", json!({
        "id":blank,"expected_revision":detail["template"]["revision"],"changes":{"name":"Denied"},
        "confirm":true,"idempotency_key":"assets-edit-locked"
    })).await.expect("isolated project assets fixture"), "template_builtin_immutable");
    let template_id = "com.example.assets";
    let save = owner.project_templates_save(serde_json::from_value(json!({
        "source":{"kind":"manual","id":template_id,"name":"Original template","unityVersion":"2022.3.22f1",
            "baseTemplate":{"id":blank,"revision":detail["template"]["revision"]},"dependencies":[]},
        "idempotencyKey":"assets-template"
    })).expect("isolated project assets fixture")).await.expect("isolated project assets fixture");
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let operation = serde_json::to_value(
                owner
                    .operation_get(save.operation_id.clone())
                    .await
                    .expect("isolated project assets fixture"),
            )
            .expect("isolated project assets fixture");
            match operation["state"].as_str() {
                Some("succeeded") => break,
                Some("failed" | "cancelled") => panic!("template seed failed: {operation}"),
                _ => tokio::time::sleep(Duration::from_millis(30)).await,
            }
        }
    })
    .await
    .expect("isolated project assets fixture");
    let before = owner
        .template_get(ProjectTemplateGetParams {
            id: template_id.into(),
            cursor: None,
            limit: None,
        })
        .await
        .expect("isolated project assets fixture");
    let edit = json!({"id":template_id,"expected_revision":before.template.revision,"changes":{
        "name":"Edited template","set_dependencies":[{"package_id":"com.example.fixture","version_range":"^1.0.0"}],
        "add_attachments":[{"source_file":source.to_string_lossy(),"target_path":"Assets/test.txt"},
            {"source_file":second_source.to_string_lossy(),"target_path":"Assets/second.txt"}]
    },"confirm":true,"idempotency_key":"assets-edit"});
    let accepted = data(
        call(
            &connection,
            &mut trace,
            "edit_project_template",
            edit.clone(),
        )
        .await
        .expect("isolated project assets fixture"),
    );
    std::fs::write(&source, "changed after acceptance").expect("isolated project assets fixture");
    let task = accepted["operation_id"]
        .as_str()
        .expect("tool or RPC identifier");
    let finished = terminal(&connection, &mut trace, task).await;
    assert_eq!(finished["state"], "succeeded", "{finished}");
    let replay = data(
        call(
            &connection,
            &mut trace,
            "edit_project_template",
            edit.clone(),
        )
        .await
        .expect("isolated project assets fixture"),
    );
    assert_eq!(replay["operation_id"], task);
    assert_eq!(replay["replayed"], true);
    let exported_path = root.join("edited.alcomdtemplate");
    let exported = owner
        .project_templates_export(ProjectTemplatesExportParams {
            id: template_id.into(),
            revision: finished["result"]["template"]["revision"]
                .as_u64()
                .expect("reported resource revision"),
            destination_file: exported_path.to_string_lossy().into_owned(),
            idempotency_key: "assets-export".into(),
        })
        .await
        .expect("isolated project assets fixture");
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let operation = serde_json::to_value(
                owner
                    .operation_get(exported.operation_id.clone())
                    .await
                    .expect("isolated project assets fixture"),
            )
            .expect("isolated project assets fixture");
            match operation["state"].as_str() {
                Some("succeeded") => break,
                Some("failed" | "cancelled") => panic!("template export failed: {operation}"),
                _ => tokio::time::sleep(Duration::from_millis(30)).await,
            }
        }
    })
    .await
    .expect("isolated project assets fixture");
    let mut archive = zip::ZipArchive::new(
        std::fs::File::open(exported_path).expect("isolated project assets fixture"),
    )
    .expect("isolated project assets fixture");
    let mut contents = String::new();
    archive
        .by_name("Assets/test.txt")
        .expect("isolated project assets fixture")
        .read_to_string(&mut contents)
        .expect("isolated project assets fixture");
    assert_eq!(contents, "frozen asset");
    let mut changed_key = edit;
    changed_key["idempotency_key"] = "assets-edit-stale".into();
    error(
        call(
            &connection,
            &mut trace,
            "edit_project_template",
            changed_key,
        )
        .await
        .expect("isolated project assets fixture"),
        "revision_conflict",
    );

    let first = data(
        call(
            &connection,
            &mut trace,
            "get_project_template",
            json!({"id":template_id,"limit":1}),
        )
        .await
        .expect("isolated project assets fixture"),
    );
    assert_eq!(first["template"]["name"], "Edited template");
    assert_eq!(
        first["dependencies"][0]["package_id"],
        "com.example.fixture"
    );
    assert_eq!(first["attachments"], json!(["Assets/second.txt"]));
    let more = json!({"id":template_id,"limit":1,"cursor":first["next_cursor"]});
    let second = data(
        call(
            &connection,
            &mut trace,
            "get_project_template",
            more.clone(),
        )
        .await
        .expect("isolated project assets fixture"),
    );
    assert_eq!(second["attachments"], json!(["Assets/test.txt"]));
    let directory = root.join("data");
    let private_sample = directory.join("must-not-export.txt");
    std::fs::write(&private_sample, "private fixture").expect("isolated project assets fixture");
    error(call(&connection, &mut trace, "edit_project_template", json!({
        "id":template_id,"expected_revision":first["template"]["revision"],"changes":{"add_attachments":[{
            "source_file":private_sample.to_string_lossy(),"target_path":"Assets/private.txt"
        }]},"confirm":true,"idempotency_key":"assets-edit-private"
    })).await.expect("isolated project assets fixture"), "invalid_request");

    let accepted = owner.template_edit(serde_json::from_value(json!({
        "id":template_id,"expectedRevision":first["template"]["revision"],
        "changes":{"removeAttachments":["Assets/second.txt"],"removeDependencies":["com.example.fixture"]},
        "confirm":true,"idempotencyKey":"assets-native-edit"
    })).expect("isolated project assets fixture")).await.expect("isolated project assets fixture");
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let current = owner
                .template_get(ProjectTemplateGetParams {
                    id: template_id.into(),
                    cursor: None,
                    limit: None,
                })
                .await
                .expect("isolated project assets fixture");
            if current.template.revision > before.template.revision + 1 {
                assert_eq!(current.attachments, ["Assets/test.txt"]);
                assert!(current.dependencies.is_empty());
                break;
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
    })
    .await
    .expect("isolated project assets fixture");
    error(
        call(
            &connection,
            &mut trace,
            "get_operation",
            json!({"operation_id":accepted.operation_id}),
        )
        .await
        .expect("isolated project assets fixture"),
        "operation_not_found",
    );
    error(
        call(&connection, &mut trace, "get_project_template", more)
            .await
            .expect("isolated project assets fixture"),
        "package_candidate_evidence_stale",
    );
    let owner_row = owner
        .mcp_clients_list(Default::default())
        .await
        .expect("isolated project assets fixture");
    let row = owner_row
        .items
        .into_iter()
        .find(|row| row.client_id == identity_id)
        .expect("isolated project assets fixture");
    owner
        .mcp_client_revoke(McpClientMutationParams {
            client_id: identity_id,
            expected_revision: row.revision,
            idempotency_key: "assets-revoke".into(),
        })
        .await
        .expect("isolated project assets fixture");
    assert!(
        call(
            &connection,
            &mut trace,
            "get_project_template",
            json!({"id":template_id})
        )
        .await
        .is_err()
    );
    drop(connection);
    http.finish().await;
    drop(owner);
    stop_core.send(()).expect("isolated project assets fixture");
    daemon
        .await
        .expect("isolated project assets fixture")
        .expect("isolated project assets fixture");
}
