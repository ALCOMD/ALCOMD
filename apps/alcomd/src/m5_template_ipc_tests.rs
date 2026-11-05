//! Real framed IPC on a private test pipe, never the user's daemon endpoint.

use super::*;
use alcomd_application::{M3RegistryStore, StateStore, UnityLaunchDefaultsSource};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};

struct EmptyLaunchDefaults;
impl UnityLaunchDefaultsSource for EmptyLaunchDefaults {
    fn arguments(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Vec<String>, app::M5UnityError>> + Send + '_>,
    > {
        Box::pin(async { Ok(Vec::new()) })
    }
}

async fn exchange(
    stream: &mut alcomd_platform::IpcStream,
    id: &str,
    method: &str,
    params: Value,
) -> Value {
    let request = serde_json::to_vec(&serde_json::json!({
        "id": id, "method": method, "params": params,
    }))
    .expect("request JSON");
    stream
        .write_u32_le(request.len() as u32)
        .await
        .expect("prefix");
    stream.write_all(&request).await.expect("request");
    stream.flush().await.expect("flush");
    let length = stream.read_u32_le().await.expect("response prefix");
    assert!(length <= 4 * 1024 * 1024);
    let mut bytes = vec![0; length as usize];
    stream.read_exact(&mut bytes).await.expect("response body");
    let response: Value = serde_json::from_slice(&bytes).expect("response JSON");
    assert_eq!(response["id"], id);
    response
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_creation_version_round_trips_over_a_private_pipe_without_an_installed_editor() {
    let root = std::env::temp_dir()
        .canonicalize()
        .expect("temp parent")
        .join(format!("alcomd-create-ipc-{}", OperationId::new()));
    std::fs::create_dir(&root).expect("owned isolated directory");
    let store =
        alcomd_store::StateStoreHandle::open(root.join("state.db")).expect("isolated store");
    let unity = crate::M5UnityApplication::new(
        store.clone(),
        crate::m5_platform::PlatformUnityAdapter,
        Arc::new(EmptyLaunchDefaults),
    );
    let engine = alcomd_vpm::TemplateEngine::with_package_cache(
        root.join("template-store"),
        root.join("package-cache"),
    )
    .expect("template engine");
    let builtins = engine
        .materialize_builtins(&root.join("builtin-staging"))
        .expect("builtins");
    let application = crate::TemplateApplication::with_locks(
        store.clone(),
        engine,
        unity.clone(),
        Arc::new(app::ResourceLockCoordinator::default()),
    );
    application
        .ensure_builtins(builtins)
        .await
        .expect("register builtins");
    let pipe = format!(r"\\.\pipe\alcomd-create-test-{}", OperationId::new());
    let server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&pipe)
        .expect("private pipe");
    let client = ClientOptions::new().open(&pipe).expect("private client");
    server.connect().await.expect("accept private client");
    let server_unity = unity.clone();
    let task = tokio::spawn(async move {
        let mut stream = alcomd_platform::IpcStream::Server(server);
        let mut state = ConnectionState {
            connection_id: "private-creation-test".into(),
            client_instance_id: None,
            handshake_complete: false,
            capabilities: std::collections::HashSet::new(),
        };
        while let Ok(length) = stream.read_u32_le().await {
            assert!(length <= 4 * 1024 * 1024);
            let mut bytes = vec![0; length as usize];
            stream.read_exact(&mut bytes).await.expect("request body");
            let request: rpc::RequestEnvelope =
                serde_json::from_slice(&bytes).expect("request DTO");
            request.validate().expect("valid envelope");
            let action = if request.method == rpc::METHOD_SYSTEM_HELLO {
                crate::dispatch_hello(request, &state)
            } else if !state.handshake_complete {
                error_action(Some(request.id), rpc::RpcError::handshake_required(), false)
            } else if request.method == rpc::METHOD_UNITY_LAUNCH_OPTIONS {
                crate::m5_rpc::dispatch(
                    request,
                    &state,
                    &server_unity,
                    &AccessContext::local_owner(),
                )
                .await
            } else {
                dispatch(request, &state, &application, &AccessContext::local_owner()).await
            };
            if let Some(capabilities) = action.complete_handshake {
                state.handshake_complete = true;
                state.capabilities = capabilities;
                state.client_instance_id = action.client_instance_id;
            }
            let response = serde_json::to_vec(&action.response).expect("response");
            assert!(response.len() <= 4 * 1024 * 1024);
            stream
                .write_u32_le(response.len() as u32)
                .await
                .expect("prefix");
            stream.write_all(&response).await.expect("response body");
            stream.flush().await.expect("flush response");
        }
    });
    let mut client = alcomd_platform::IpcStream::Client(client);
    let hello = exchange(&mut client, "hello", rpc::METHOD_SYSTEM_HELLO, serde_json::json!({
        "rpcVersion": 1,
        "client": {"name":"create-ipc-test","version":"test","instanceId":"isolated"},
        "capabilities": [rpc::CAPABILITY_TEMPLATES_READ_V1,rpc::CAPABILITY_TEMPLATES_CREATE_PROJECT_V1,rpc::CAPABILITY_UNITY_READ_V1,rpc::CAPABILITY_UNITY_LAUNCH_V1],
    })).await;
    assert!(hello.get("result").is_some(), "{hello}");
    let templates = exchange(
        &mut client,
        "list",
        rpc::METHOD_TEMPLATES_LIST,
        serde_json::json!({}),
    )
    .await;
    let blank = templates["result"]["templates"]
        .as_array()
        .expect("templates")
        .iter()
        .find(|v| v["displayName"] == "Blank")
        .expect("blank");
    let mut params = serde_json::json!({
        "templateId": blank["templateId"], "expectedTemplateRevision": blank["revision"],
        "targetParent": root.to_string_lossy(), "targetLeaf": "Created IPC",
    });
    let missing = exchange(
        &mut client,
        "missing-version",
        rpc::METHOD_TEMPLATES_PLAN_CREATE_PROJECT,
        params.clone(),
    )
    .await;
    assert_eq!(missing["error"]["code"], "invalid_request");
    assert!(!root.join("Created IPC").exists());
    params["unityVersion"] = "2022.3.22f1".into();
    let plan = exchange(
        &mut client,
        "plan",
        rpc::METHOD_TEMPLATES_PLAN_CREATE_PROJECT,
        params,
    )
    .await;
    assert_eq!(plan["result"]["unityVersion"], "2022.3.22f1", "{plan}");
    assert_eq!(
        plan["result"]["projectVersionSha256"]
            .as_str()
            .expect("pinned hash")
            .len(),
        64
    );
    assert!(!root.join("Created IPC").exists());
    let apply_params = serde_json::json!({"planId":plan["result"]["planId"],"idempotencyKey":"private-create-ipc"});
    let mut commits = store.event_commits();
    let accepted = exchange(
        &mut client,
        "apply",
        rpc::METHOD_TEMPLATES_APPLY_CREATE_PROJECT,
        apply_params.clone(),
    )
    .await;
    let operation_id = OperationId::parse(
        accepted["result"]["operationId"]
            .as_str()
            .expect("accepted operation"),
    )
    .expect("ID");
    let operation = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            commits.borrow_and_update();
            let operation = store
                .get_operation(app::PrincipalId::local_owner(), operation_id)
                .await
                .expect("owned task");
            if operation.state.is_terminal() {
                break operation;
            }
            commits.changed().await.expect("committed task change");
        }
    })
    .await
    .expect("bounded task completion");
    assert_eq!(operation.state, crate::DomainState::Succeeded);
    let result: Value =
        serde_json::from_str(operation.result_json.as_deref().expect("result JSON"))
            .expect("result");
    let project_id = app::ProjectId::parse(result["projectId"].as_str().expect("project ID"))
        .expect("project ID");
    let project = store
        .get_project(app::PrincipalId::local_owner(), project_id)
        .await
        .expect("registered project");
    assert_eq!(project.observation.unity_version, "2022.3.22f1");
    let options = exchange(&mut client,"launch-options",rpc::METHOD_UNITY_LAUNCH_OPTIONS,serde_json::json!({"projectId":project_id,"expectedProjectRevision":project.revision.get()})).await;
    assert_eq!(
        options["result"]["projectUnityVersion"], "2022.3.22f1",
        "{options}"
    );
    assert_eq!(
        options["result"]["exactMatchingInstallations"],
        serde_json::json!([])
    );
    let replay = exchange(
        &mut client,
        "replay",
        rpc::METHOD_TEMPLATES_APPLY_CREATE_PROJECT,
        apply_params,
    )
    .await;
    assert_eq!(replay["result"]["operationId"], operation_id.to_string());
    assert_eq!(replay["result"]["replayed"], true);
    drop(client);
    tokio::time::timeout(std::time::Duration::from_secs(5), task)
        .await
        .expect("pipe task closes")
        .expect("pipe task success");
    drop(unity);
    drop(store);
    // Only this test's uniquely named directory is removed; no registered user data is used.
    assert!(
        root.file_name()
            .expect("owned leaf")
            .to_string_lossy()
            .starts_with("alcomd-create-ipc-")
    );
    std::fs::remove_dir_all(root).expect("remove isolated test data");
}
