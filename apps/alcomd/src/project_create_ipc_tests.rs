//! Framed requests over an isolated Windows pipe and an isolated store.

use std::sync::Arc;
use std::time::Duration;

use alcomd_application::{M3RegistryStore, StateStore};
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
        "capabilities": [rpc::CAPABILITY_PROJECTS_CREATE_V1,rpc::CAPABILITY_PROJECT_TEMPLATES_V1,"templates.read.v1"],
    })).await;
    assert_eq!(
        hello["result"]["capabilities"],
        json!([
            rpc::CAPABILITY_PROJECT_TEMPLATES_V1,
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
        json!({"cursor":catalog["result"]["nextCursor"]}),
    )
    .await;
    assert_eq!(
        second["result"]["templates"]
            .as_array()
            .expect("templates")
            .len(),
        1
    );
    let same = exchange(
        &mut client,
        "same",
        rpc::METHOD_PROJECT_TEMPLATES_SAVE,
        json!({"declaration":builtin["declaration"]}),
    )
    .await;
    assert_eq!(same["result"]["template"], *builtin);
    let locked = exchange(
        &mut client,
        "locked",
        rpc::METHOD_PROJECT_TEMPLATES_REMOVE,
        json!({"id":builtin["declaration"]["id"], "expectedRevision":builtin["revision"]}),
    )
    .await;
    assert_eq!(locked["error"]["code"], "template_builtin_immutable");

    let declaration = json!({"id":"com.example.local","name":builtin["declaration"]["name"],"unity":{"series":[],"presets":[]},"dependencies":[],"constraints":[]});
    let saved = exchange(
        &mut client,
        "save",
        rpc::METHOD_PROJECT_TEMPLATES_SAVE,
        json!({"declaration":declaration}),
    )
    .await;
    let custom = &saved["result"]["template"];
    assert_eq!(custom["locked"], false);
    let removed = exchange(
        &mut client,
        "remove",
        rpc::METHOD_PROJECT_TEMPLATES_REMOVE,
        json!({"id":declaration["id"],"expectedRevision":custom["revision"]}),
    )
    .await;
    assert_eq!(removed["result"]["id"], declaration["id"]);

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
    stale["template"] = json!({"id":declaration["id"],"revision":custom["revision"]});
    let stale = exchange(&mut client, "stale", rpc::METHOD_PROJECTS_CREATE, stale).await;
    assert_eq!(stale["error"]["code"], "revision_conflict");
    assert!(!root.join("Created IPC").exists());
    let mut commits = store.event_commits();
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
    let operation = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            commits.borrow_and_update();
            let operation = store
                .get_operation(app::PrincipalId::local_owner(), operation_id)
                .await
                .expect("owned task");
            if operation.state.is_terminal() {
                break operation;
            }
            commits.changed().await.expect("committed change");
        }
    })
    .await
    .expect("bounded task completion");
    assert_eq!(
        operation.state,
        crate::DomainState::Succeeded,
        "{operation:?}"
    );
    let result: Value =
        serde_json::from_str(operation.result_json.as_deref().expect("task result"))
            .expect("result JSON");
    assert_eq!(result["projectId"], accepted["result"]["projectId"]);
    let project_id = app::ProjectId::parse(result["projectId"].as_str().expect("project ID"))
        .expect("project ID");
    let project = store
        .get_project(app::PrincipalId::local_owner(), project_id)
        .await
        .expect("registered project");
    assert_eq!(project.observation.unity_version, "2022.3.22f1");
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
