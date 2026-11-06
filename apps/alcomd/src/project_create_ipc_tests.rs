//! Framed requests over an isolated Windows pipe and an isolated store.

use std::sync::Arc;
use std::time::Duration;

use alcomd_application::{M3RegistryStore, ProjectCreateStore, StateStore};
use alcomd_platform::IpcStream;
use alcomd_store::StateStoreHandle;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};

use super::*;

async fn exchange(stream: &mut IpcStream, id: &str, method: &str, params: Value) -> Value {
    let frame = rpc::encode_frame(
        &serde_json::to_vec(&json!({"id": id, "method": method, "params": params}))
            .expect("request JSON"),
    )
    .expect("bounded frame");
    stream.write_all(&frame).await.expect("request frame");
    stream.flush().await.expect("flush request");
    let length = stream.read_u32_le().await.expect("response prefix");
    assert!(length as usize <= rpc::MAX_FRAME_PAYLOAD_BYTES);
    let mut bytes = vec![0; length as usize];
    stream.read_exact(&mut bytes).await.expect("response body");
    let response: Value = serde_json::from_slice(&bytes).expect("response JSON");
    assert_eq!(response["id"], id);
    response
}

async fn completed(store: &StateStoreHandle, accepted: &Value) -> app::OperationRecord {
    let id = OperationId::parse(
        accepted["result"]["operationId"]
            .as_str()
            .expect("operation ID"),
    )
    .expect("valid operation ID");
    let mut commits = store.event_commits();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            commits.borrow_and_update();
            let operation = store
                .get_operation(app::PrincipalId::local_owner(), id)
                .await
                .expect("owned task");
            if operation.state.is_terminal() {
                break operation;
            }
            commits.changed().await.expect("committed change");
        }
    })
    .await
    .expect("bounded task completion")
}

fn successful_result(operation: &app::OperationRecord) -> Value {
    assert_eq!(
        operation.state,
        crate::DomainState::Succeeded,
        "{operation:?}"
    );
    serde_json::from_str(operation.result_json.as_deref().expect("task result"))
        .expect("result JSON")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn standalone_project_creation_and_template_mutations_use_the_private_rpc_pipe() {
    let root = std::env::temp_dir()
        .canonicalize()
        .expect("temp parent")
        .join(format!("alcomd-create-ipc-{}", OperationId::new()));
    std::fs::create_dir(&root).expect("owned test directory");
    let store = StateStoreHandle::open(root.join("state.db")).expect("test store");
    let engine = alcomd_vpm::ProjectCreateEngine::new(
        store.clone(),
        alcomd_vpm::VpmReader::new().expect("reader"),
        root.join("package-cache"),
    )
    .expect("project create engine");
    let application = ProjectCreateApplication::with_locks(
        store.clone(),
        engine,
        Arc::new(app::ResourceLockCoordinator::default()),
    );
    application
        .seed_builtin_templates()
        .await
        .expect("register builtins");
    let recovery = application.clone();
    let template_root = root.join("templates");
    let installed = || {
        std::fs::read_dir(&template_root)
            .expect("managed template directory")
            .map(|entry| entry.expect("managed archive").path())
            .collect::<std::collections::BTreeSet<_>>()
    };
    let builtins = installed();
    assert_eq!(builtins.len(), 4);
    assert!(!root.join("project-templates").exists());

    let pipe = format!(r"\\.\pipe\alcomd-create-test-{}", OperationId::new());
    let server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&pipe)
        .expect("private pipe");
    let client = ClientOptions::new().open(&pipe).expect("private client");
    server.connect().await.expect("accept private client");
    let task = tokio::spawn(async move {
        let mut stream = IpcStream::Server(server);
        let mut state = ConnectionState {
            connection_id: "private-create-test".to_owned(),
            client_instance_id: None,
            actor: "Local IPC".to_owned(),
            handshake_complete: false,
            capabilities: std::collections::HashSet::new(),
        };
        while let Ok(length) = stream.read_u32_le().await {
            assert!(length as usize <= rpc::MAX_FRAME_PAYLOAD_BYTES);
            let mut bytes = vec![0; length as usize];
            stream
                .read_exact(&mut bytes)
                .await
                .expect("read request body");
            let request: rpc::RequestEnvelope =
                serde_json::from_slice(&bytes).expect("request DTO");
            request.validate().expect("valid envelope");
            let action = if request.method == rpc::METHOD_SYSTEM_HELLO {
                crate::dispatch_hello(request, &state)
            } else if !state.handshake_complete {
                error_action(Some(request.id), rpc::RpcError::handshake_required(), false)
            } else {
                dispatch(request, &state, &application, &AccessContext::local_owner()).await
            };
            if let Some(capabilities) = action.complete_handshake {
                state.handshake_complete = true;
                state.capabilities = capabilities;
                state.client_instance_id = action.client_instance_id;
            }
            let response = serde_json::to_vec(&action.response).expect("response JSON");
            let frame = rpc::encode_frame(&response).expect("bounded response");
            stream.write_all(&frame).await.expect("response frame");
            stream.flush().await.expect("flush response");
        }
    });
    let mut client = IpcStream::Client(client);
    let hello = exchange(&mut client, "hello", rpc::METHOD_SYSTEM_HELLO, json!({
        "rpcVersion": 1,
        "client": {"name":"alcomd-cli","version":"test","instanceId":"isolated"},
        "capabilities": [rpc::CAPABILITY_PROJECTS_CREATE_V1,rpc::CAPABILITY_PROJECT_TEMPLATES_V2,"templates.read.v1"],
    })).await;
    assert_eq!(
        hello["result"]["capabilities"],
        json!([
            rpc::CAPABILITY_PROJECT_TEMPLATES_V2,
            rpc::CAPABILITY_PROJECTS_CREATE_V1
        ])
    );
    let old = exchange(&mut client, "old", "templates.list", json!({})).await;
    assert_eq!(old["error"]["code"], "method_not_found");

    let catalog = exchange(
        &mut client,
        "catalog",
        rpc::METHOD_PROJECT_TEMPLATES_LIST,
        json!({"limit":1}),
    )
    .await;
    let builtin = &catalog["result"]["templates"][0];
    assert_eq!(builtin["locked"], true);
    let second = exchange(
        &mut client,
        "second",
        rpc::METHOD_PROJECT_TEMPLATES_LIST,
        json!({"cursor":catalog["result"]["nextCursor"], "limit":1}),
    )
    .await;
    assert_eq!(
        second["result"]["templates"]
            .as_array()
            .expect("templates")
            .len(),
        1
    );
    let locked = exchange(
        &mut client,
        "locked",
        rpc::METHOD_PROJECT_TEMPLATES_REMOVE,
        json!({"id":builtin["id"], "expectedRevision":builtin["revision"]}),
    )
    .await;
    assert_eq!(locked["error"]["code"], "template_builtin_immutable");

    let source = json!({"kind":"manual","id":"com.example.local","name":builtin["name"],"unityVersion":"2022.3.22f1","dependencies":[]});
    let saved = exchange(
        &mut client,
        "save",
        rpc::METHOD_PROJECT_TEMPLATES_SAVE,
        json!({"source":source,"idempotencyKey":"manual"}),
    )
    .await;
    let saved_result = successful_result(&completed(&store, &saved).await);
    let custom = &saved_result["template"];
    assert_eq!(custom["locked"], false);
    let saved_archives = installed();
    let mut added = saved_archives.difference(&builtins);
    let internal_archive = added.next().expect("managed copy");
    assert!(added.next().is_none());
    let archive = root.join("fixture.alcomdtemplate");
    let exported = exchange(
        &mut client,
        "export",
        rpc::METHOD_PROJECT_TEMPLATES_EXPORT,
        json!({"id":custom["id"], "revision":custom["revision"], "destinationFile":archive, "idempotencyKey":"export"}),
    ).await;
    assert_eq!(
        successful_result(&completed(&store, &exported).await)["id"],
        custom["id"]
    );
    assert!(archive.is_file());
    let import_source = root.join("import-source.alcomdtemplate");
    std::fs::copy(&archive, &import_source).expect("independent import source");
    let duplicate = exchange(
        &mut client,
        "import",
        rpc::METHOD_PROJECT_TEMPLATES_SAVE,
        json!({"source":{"kind":"file","sourceFile":import_source},"idempotencyKey":"import"}),
    )
    .await;
    assert_eq!(
        successful_result(&completed(&store, &duplicate).await)["template"],
        *custom
    );
    std::fs::remove_file(&import_source).expect("delete only the external file");
    let from_archive = exchange(
        &mut client,
        "create-from-template",
        rpc::METHOD_PROJECTS_CREATE,
        json!({"name":"From archive","targetParent":root,"unityVersion":"2022.3.22f1","idempotencyKey":"archive-create",
            "template":{"id":custom["id"],"revision":custom["revision"]}}),
    ).await;
    assert_eq!(
        successful_result(&completed(&store, &from_archive).await)["projectId"],
        from_archive["result"]["projectId"]
    );
    let removed = exchange(
        &mut client,
        "remove",
        rpc::METHOD_PROJECT_TEMPLATES_REMOVE,
        json!({"id":custom["id"],"expectedRevision":custom["revision"]}),
    )
    .await;
    assert_eq!(removed["result"]["id"], custom["id"]);
    assert_eq!(installed(), builtins);
    assert!(archive.is_file());

    let reimport = exchange(
        &mut client,
        "reimport",
        rpc::METHOD_PROJECT_TEMPLATES_SAVE,
        json!({"source":{"kind":"file","sourceFile":archive},"idempotencyKey":"reimport"}),
    )
    .await;
    let imported = successful_result(&completed(&store, &reimport).await);
    let reference = app::ProjectTemplateRef {
        id: imported["template"]["id"]
            .as_str()
            .expect("template ID")
            .to_owned(),
        revision: imported["template"]["revision"].as_u64().expect("revision"),
    };
    let deferred_export = root.join("pending-export.alcomdtemplate");
    store
        .project_template_job_accept(
            app::PrincipalId::local_owner(),
            app::ProjectTemplateJobRequest::Export {
                reference: reference.clone(),
                destination_file: deferred_export.to_str().expect("fixture path").to_owned(),
            },
            app::IdempotencyKey::parse("deferred-export").expect("fixture key"),
            "{}".to_owned(),
            1,
        )
        .await
        .expect("persist a queued export without starting its worker");
    let removed = exchange(
        &mut client,
        "remove-pinned",
        rpc::METHOD_PROJECT_TEMPLATES_REMOVE,
        json!({"id":reference.id,"expectedRevision":reference.revision}),
    )
    .await;
    assert_eq!(removed["result"]["id"], custom["id"]);
    assert!(
        internal_archive.is_file(),
        "queued export still pins the archive"
    );
    recovery.recover().await.expect("run the pending export");
    drop(recovery);
    assert_eq!(installed(), builtins);
    assert_eq!(
        std::fs::read(&deferred_export).unwrap(),
        std::fs::read(&archive).unwrap()
    );

    let mut params = json!({"name":"Created IPC","targetParent":root.to_string_lossy(),"idempotencyKey":"isolated-create"});
    let missing = exchange(
        &mut client,
        "missing-version",
        rpc::METHOD_PROJECTS_CREATE,
        params.clone(),
    )
    .await;
    assert_eq!(missing["error"]["code"], "invalid_request");
    params["unityVersion"] = json!("2022.3.22f1");
    let mut stale = params.clone();
    stale["template"] = json!({"id":custom["id"],"revision":custom["revision"]});
    let stale = exchange(&mut client, "stale", rpc::METHOD_PROJECTS_CREATE, stale).await;
    assert_eq!(stale["error"]["code"], "revision_conflict");
    assert!(!root.join("Created IPC").exists());
    let accepted = exchange(
        &mut client,
        "create",
        rpc::METHOD_PROJECTS_CREATE,
        params.clone(),
    )
    .await;
    let operation_id = OperationId::parse(
        accepted["result"]["operationId"]
            .as_str()
            .expect("operation ID"),
    )
    .expect("operation ID");
    let result = successful_result(&completed(&store, &accepted).await);
    assert_eq!(result["projectId"], accepted["result"]["projectId"]);
    let project_id = app::ProjectId::parse(result["projectId"].as_str().expect("project ID"))
        .expect("project ID");
    let project = store
        .get_project(app::PrincipalId::local_owner(), project_id)
        .await
        .expect("registered project");
    assert_eq!(project.observation.unity_version, "2022.3.22f1");
    let source_root = root.join("Created IPC");
    std::fs::write(source_root.join("Assets/example.txt"), "fixture asset").expect("asset");
    std::fs::write(source_root.join("Assets/example.txt.meta"), "fixture meta").expect("metadata");
    std::fs::write(
        source_root.join("ProjectSettings/Fixture.asset"),
        "fixture settings",
    )
    .expect("settings");
    for (id, include_assets) in [("without", false), ("with", true)] {
        let template_id = format!("com.example.{id}-assets");
        let mut source = json!({"kind":"project","projectId":project_id.to_string(),
            "expectedProjectRevision":project.revision.get(),"id":template_id,"name":template_id});
        if include_assets {
            source["includeAssets"] = json!(true);
        }
        let response = exchange(
            &mut client,
            id,
            rpc::METHOD_PROJECT_TEMPLATES_SAVE,
            json!({"source":source,"idempotencyKey":format!("capture-{id}")}),
        )
        .await;
        let captured = successful_result(&completed(&store, &response).await);
        let path = root.join(format!("{id}.alcomdtemplate"));
        let response = exchange(
            &mut client,
            id,
            rpc::METHOD_PROJECT_TEMPLATES_EXPORT,
            json!({"id":template_id,"revision":captured["template"]["revision"],
                "destinationFile":path,"idempotencyKey":format!("export-{id}")}),
        )
        .await;
        successful_result(&completed(&store, &response).await);
        let preflight = alcomd_vpm::preflight_archive(&path).expect("valid project template");
        let has = |name: &str| {
            preflight
                .entries
                .iter()
                .any(|item| item.relative_path == std::path::Path::new(name))
        };
        assert!(has("ProjectSettings/Fixture.asset"));
        assert_eq!(has("Assets/example.txt"), include_assets);
        assert_eq!(has("Assets/example.txt.meta"), include_assets);
    }
    if let Some(destination) = std::env::var_os("ALCOMD_TEST_TEMPLATE_EXPORT") {
        let mut output = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(destination)
            .expect("unused acceptance fixture path");
        std::io::copy(
            &mut std::fs::File::open(&archive).expect("exported fixture"),
            &mut output,
        )
        .expect("copy acceptance fixture");
    }
    if let Some(source) = std::env::var_os("ALCOMD_TEST_VCC_TEMPLATE_IMPORT") {
        let response = exchange(
            &mut client,
            "official-vcc-template",
            rpc::METHOD_PROJECT_TEMPLATES_SAVE,
            json!({"source":{"kind":"file","sourceFile":source.to_str().expect("fixture path")},
                "idempotencyKey":"import-official-vcc"}),
        )
        .await;
        let result = successful_result(&completed(&store, &response).await);
        assert_eq!(result["template"]["id"], "com.vrchat.template.avatar");
    }
    let replay = exchange(
        &mut client,
        "replay",
        rpc::METHOD_PROJECTS_CREATE,
        params.clone(),
    )
    .await;
    assert_eq!(replay["result"]["operationId"], operation_id.to_string());
    assert_eq!(replay["result"]["replayed"], true);
    params["idempotencyKey"] = json!("new-target-conflict");
    let collision = exchange(
        &mut client,
        "collision",
        rpc::METHOD_PROJECTS_CREATE,
        params,
    )
    .await;
    assert_eq!(collision["error"]["code"], "project_create_target_exists");
    assert!(
        root.join("Created IPC/ProjectSettings/ProjectVersion.txt")
            .is_file()
    );

    drop(client);
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .expect("pipe closes")
        .expect("pipe task success");
    drop(store);
    assert!(
        root.file_name()
            .is_some_and(|leaf| leaf.to_string_lossy().starts_with("alcomd-create-ipc-"))
    );
    std::fs::remove_dir_all(root).expect("remove only owned test directory");
}
