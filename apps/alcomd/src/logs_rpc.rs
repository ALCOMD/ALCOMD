use alcomd_application as app;
use alcomd_protocol as rpc;
use alcomd_store::StateStoreHandle;

pub(super) async fn dispatch(
    request: rpc::RequestEnvelope,
    state: &super::ConnectionState,
    store: &StateStoreHandle,
    access: &app::AccessContext,
) -> super::DispatchAction {
    if let Some(error) = super::require_capability(&request.id, state, rpc::CAPABILITY_LOGS_READ_V1)
    {
        return error;
    }
    let params: rpc::LogsReadParams = match serde_json::from_value(request.params) {
        Ok(params) => params,
        Err(_) => {
            return super::error_action(Some(request.id), rpc::RpcError::invalid_request(), false);
        }
    };
    let filter = params.filter.unwrap_or_default();
    match app::read_operation_logs(
        store,
        access,
        app::OperationLogQuery {
            before_sequence: params.before_sequence,
            after_sequence: params.after_sequence,
            limit: params.limit.unwrap_or(100),
            filter: app::LogFilter {
                search: filter.search,
                actor: filter.actor,
                outcome: filter.outcome.map(|outcome| match outcome {
                    rpc::LogOutcome::Started => app::LogOutcome::Started,
                    rpc::LogOutcome::Succeeded => app::LogOutcome::Succeeded,
                    rpc::LogOutcome::Failed => app::LogOutcome::Failed,
                    rpc::LogOutcome::Cancelled => app::LogOutcome::Cancelled,
                    rpc::LogOutcome::Waiting => app::LogOutcome::Waiting,
                }),
                operation_type: filter.operation_type.map(|kind| match kind {
                    rpc::OperationLogType::Read => app::OperationLogType::Read,
                    rpc::OperationLogType::Write => app::OperationLogType::Write,
                    rpc::OperationLogType::Passive => app::OperationLogType::Passive,
                    rpc::OperationLogType::Open => app::OperationLogType::Open,
                    rpc::OperationLogType::Maintenance => app::OperationLogType::Maintenance,
                }),
            },
        },
    )
    .await
    {
        Ok(page) => super::success_action(
            request.id,
            rpc::LogsReadResult {
                entries: page.entries.into_iter().map(entry_to_rpc).collect(),
                actors: page.actors,
                high_watermark: page.high_watermark,
                has_more: page.has_more,
                next_sequence: page.next_sequence,
            },
            None,
        ),
        Err(error) => super::application_error(request.id, error),
    }
}

fn entry_to_rpc(entry: app::OperationLogEntry) -> rpc::OperationLogEntry {
    rpc::OperationLogEntry {
        sequence: entry.sequence,
        entry_id: entry.entry_id,
        execution_id: entry.execution_id,
        actor: entry.actor,
        operation_id: entry.operation_id,
        occurred_at_ms: entry.occurred_at_ms,
        action: entry.action,
        step: entry.step,
        outcome: match entry.outcome {
            app::LogOutcome::Started => rpc::LogOutcome::Started,
            app::LogOutcome::Succeeded => rpc::LogOutcome::Succeeded,
            app::LogOutcome::Failed => rpc::LogOutcome::Failed,
            app::LogOutcome::Cancelled => rpc::LogOutcome::Cancelled,
            app::LogOutcome::Waiting => rpc::LogOutcome::Waiting,
        },
        target: entry.target.map(|target| rpc::LogTarget {
            kind: target.kind,
            id: target.id,
            name: target.name,
            path: target.path,
        }),
        details: entry.details,
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::{ConnectionState, M3ReadApplication, OperationId, read_frame, write_json_frame};
    use alcomd_platform::IpcStream;
    use serde_json::{Value, json};
    use std::time::Duration;
    use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};

    async fn open_private_connection(
        store: StateStoreHandle,
        entry_point: &str,
        capabilities: &[&str],
    ) -> (IpcStream, tokio::task::JoinHandle<()>) {
        let pipe = format!(r"\\.\pipe\alcomd-logs-test-{}", OperationId::new());
        let server = ServerOptions::new()
            .first_pipe_instance(true)
            .reject_remote_clients(true)
            .create(&pipe)
            .expect("private pipe");
        let mut client =
            IpcStream::Client(ClientOptions::new().open(&pipe).expect("private client"));
        server.connect().await.expect("accept private client");
        let task = tokio::spawn(async move {
            let registry = M3ReadApplication::new(
                store.clone(),
                alcomd_vpm::VpmReader::new().expect("real project reader"),
            );
            let mut stream = IpcStream::Server(server);
            let mut state = ConnectionState {
                connection_id: "private-log-test".to_owned(),
                client_instance_id: None,
                actor: "Local IPC".to_owned(),
                handshake_complete: false,
                capabilities: Default::default(),
            };
            while let Some(bytes) = read_frame(&mut stream).await.expect("read request") {
                let request: rpc::RequestEnvelope =
                    serde_json::from_slice(&bytes).expect("request DTO");
                request.validate().expect("valid request");
                let access = app::AccessContext::local_owner().with_actor(state.actor.clone());
                let action = match request.method.as_str() {
                    rpc::METHOD_SYSTEM_HELLO => crate::dispatch_hello(request, &state),
                    rpc::METHOD_PROJECTS_REGISTER => {
                        app::with_log_caller(
                            store.clone(),
                            &access,
                            crate::m3_rpc::dispatch(request, &state, &registry, &access),
                        )
                        .await
                    }
                    rpc::METHOD_LOGS_READ => dispatch(request, &state, &store, &access).await,
                    method => panic!("unexpected test method: {method}"),
                };
                if let Some(capabilities) = action.complete_handshake {
                    state.handshake_complete = true;
                    state.capabilities = capabilities;
                    state.client_instance_id = action.client_instance_id;
                    state.actor = action.actor.expect("hello entry point");
                }
                write_json_frame(&mut stream, &action.response)
                    .await
                    .expect("write response");
            }
        });
        let hello = exchange(
            &mut client,
            rpc::METHOD_SYSTEM_HELLO,
            json!({
                "rpcVersion": rpc::RPC_VERSION,
                "client": {"name": entry_point, "version": "test", "instanceId": entry_point},
                "capabilities": capabilities,
            }),
        )
        .await;
        for capability in capabilities {
            assert!(
                hello["result"]["capabilities"]
                    .as_array()
                    .expect("negotiated capabilities")
                    .contains(&json!(capability))
            );
        }
        (client, task)
    }

    async fn exchange(stream: &mut IpcStream, method: &str, params: Value) -> Value {
        write_json_frame(
            stream,
            &json!({"id": method, "method": method, "params": params}),
        )
        .await
        .expect("write request");
        let bytes = read_frame(stream)
            .await
            .expect("read frame")
            .expect("response");
        let result: Value = serde_json::from_slice(&bytes).expect("response JSON");
        assert_eq!(result["id"], method);
        result
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn reads_core_project_failure_with_initiating_entry_point_over_private_windows_ipc() {
        tokio::time::timeout(Duration::from_secs(10), async {
            let parent = std::env::temp_dir().canonicalize().expect("temp parent");
            let root = parent.join(format!("alcomd-logs-test-{}", OperationId::new()));
            std::fs::create_dir(&root).expect("isolated directory");
            let store = StateStoreHandle::open(root.join("state.db")).expect("isolated store");
            let missing = root.join("不存在的项目");
            let path = missing.to_str().expect("fixture path");
            let (mut cli, cli_server) = open_private_connection(
                store.clone(),
                "alcomd-cli",
                &[rpc::CAPABILITY_PROJECTS_REGISTRY_V1],
            )
            .await;
            let response = exchange(
                &mut cli,
                rpc::METHOD_PROJECTS_REGISTER,
                json!({
                    "path": path,
                    "idempotencyKey": "private-log-project-failure",
                }),
            )
            .await;
            assert!(response.get("error").is_some());
            assert!(!response.to_string().contains("不存在的项目"));
            drop(cli);
            cli_server.await.expect("CLI pipe task");

            let (mut gui, gui_server) = open_private_connection(
                store.clone(),
                "alcomd-gui",
                &[rpc::CAPABILITY_LOGS_READ_V1],
            )
            .await;
            let response = exchange(
                &mut gui,
                rpc::METHOD_LOGS_READ,
                json!({"filter": {"search": "不存在的项目", "actor": "CLI", "outcome": "failed", "type": "write"}}),
            )
            .await;
            let page: rpc::LogsReadResult =
                serde_json::from_value(response["result"].clone()).expect("typed logs.read result");
            assert_eq!(page.entries.len(), 1);
            assert_eq!(page.actors, ["CLI"]);
            let entry = &page.entries[0];
            assert_eq!(entry.actor, "CLI");
            assert_eq!(entry.step, "inspect_project");
            assert_eq!(entry.outcome, rpc::LogOutcome::Failed);
            assert_eq!(
                entry
                    .target
                    .as_ref()
                    .and_then(|target| target.path.as_deref()),
                Some(path)
            );
            assert!(
                entry
                    .details
                    .as_ref()
                    .is_some_and(|details| !details.is_empty())
            );
            drop(gui);
            gui_server.await.expect("GUI pipe task");
            drop(store);
            let owned = root.canonicalize().expect("owned temp directory");
            assert_eq!(owned.parent(), Some(parent.as_path()));
            assert_eq!(owned.file_name(), root.file_name());
            std::fs::remove_dir_all(owned).expect("remove isolated test data");
        })
        .await
        .expect("private IPC test completes");
    }
}
