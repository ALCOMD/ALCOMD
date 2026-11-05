//! Technical-log transport delegates authorization to the shared application use case.
use super::{
    AccessContext, ConnectionState, DispatchAction, error_action, invalid, require_capability,
    success_action,
};
use alcomd_application as app;
use alcomd_logging::{LogQuery, LogSource};
use alcomd_protocol as rpc;
#[derive(Clone)]
pub(super) struct CoreLogReader(pub LogSource);
pub(super) type LogApplication = app::TechnicalLogApplication<CoreLogReader>;
impl app::TechnicalLogPort for CoreLogReader {
    type ReadPage = alcomd_logging::ReadPage;
    type HistoryPage = alcomd_logging::HistoryPage;
    type Health = alcomd_logging::LogHealth;
    async fn read(
        &self,
        r: app::TechnicalLogRead,
    ) -> Result<Self::ReadPage, app::TechnicalLogError> {
        self.0
            .read(r.cursor, r.limit, r.follow, query(r.query))
            .await
            .map_err(map_error)
    }
    async fn history(
        &self,
        r: app::TechnicalLogHistory,
    ) -> Result<Self::HistoryPage, app::TechnicalLogError> {
        self.0
            .history(r.cursor, r.before, r.limit, query(r.query))
            .await
            .map_err(map_error)
    }
    fn health(&self) -> Self::Health {
        self.0.health()
    }
}
fn query(q: app::TechnicalLogQuery) -> LogQuery {
    LogQuery {
        search: q.search,
        levels: q.levels,
        target: q.target,
        diagnostic_id: q.diagnostic_id,
    }
}
fn app_query(q: rpc::DiagnosticLogQuery) -> app::TechnicalLogQuery {
    app::TechnicalLogQuery {
        search: q.search,
        levels: q.levels,
        target: q.target,
        diagnostic_id: q.diagnostic_id,
    }
}
fn map_error(e: alcomd_logging::LogError) -> app::TechnicalLogError {
    match e {
        alcomd_logging::LogError::InvalidInput => app::TechnicalLogError::InvalidInput,
        alcomd_logging::LogError::CursorExpired => app::TechnicalLogError::CursorExpired,
        alcomd_logging::LogError::Unavailable => app::TechnicalLogError::Unavailable,
    }
}
pub(super) fn is_log_read(payload: &[u8]) -> bool {
    #[derive(serde::Deserialize)]
    struct Method {
        method: String,
    }
    serde_json::from_slice::<Method>(payload).is_ok_and(|r| {
        matches!(
            r.method.as_str(),
            rpc::METHOD_DIAGNOSTICS_LOG_READ
                | rpc::METHOD_DIAGNOSTICS_LOG_HISTORY
                | rpc::METHOD_DIAGNOSTICS_LOG_HEALTH
        )
    })
}
/// Only this read-only method may be abandoned when its client disconnects.
pub(super) async fn until_disconnect<R, F>(
    reader: &mut R,
    dispatch: F,
) -> std::io::Result<Option<DispatchAction>>
where
    R: tokio::io::AsyncRead + Unpin,
    F: std::future::Future<Output = DispatchAction>,
{
    use tokio::io::AsyncReadExt;
    let mut next = [0_u8; 1];
    tokio::select! {
        result = reader.read(&mut next) => match result? {
            0 => Ok(None),
            _ => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "log reads must not be pipelined")),
        },
        action = dispatch => Ok(Some(action)),
    }
}

pub(super) async fn dispatch(
    request: rpc::RequestEnvelope,
    state: &ConnectionState,
    application: &LogApplication,
    access: &AccessContext,
) -> DispatchAction {
    if let Some(action) = require_capability(&request.id, state, rpc::CAPABILITY_DIAGNOSTICS_LOG_V1)
    {
        return action;
    }
    match request.method.as_str() {
        rpc::METHOD_DIAGNOSTICS_LOG_READ => {
            let p: rpc::DiagnosticLogReadParams = match serde_json::from_value(request.params) {
                Ok(p) => p,
                Err(_) => return invalid(request.id),
            };
            match application
                .read(
                    access,
                    app::TechnicalLogRead {
                        cursor: p.cursor,
                        limit: p.limit.unwrap_or(100),
                        follow: p.follow,
                        query: app_query(p.query),
                    },
                )
                .await
            {
                Ok(page) => success_action(request.id, page, None),
                Err(e) => failure(request.id, e),
            }
        }
        rpc::METHOD_DIAGNOSTICS_LOG_HISTORY => {
            let p: rpc::DiagnosticLogHistoryParams = match serde_json::from_value(request.params) {
                Ok(p) => p,
                Err(_) => return invalid(request.id),
            };
            match application
                .history(
                    access,
                    app::TechnicalLogHistory {
                        cursor: p.cursor,
                        before: p.before,
                        limit: p.limit.unwrap_or(100),
                        query: app_query(p.query),
                    },
                )
                .await
            {
                Ok(page) => success_action(request.id, page, None),
                Err(e) => failure(request.id, e),
            }
        }
        rpc::METHOD_DIAGNOSTICS_LOG_HEALTH => {
            if request.params.as_object().is_none_or(|p| !p.is_empty()) {
                return invalid(request.id);
            }
            match application.health(access) {
                Ok(h) => success_action(request.id, h, None),
                Err(e) => failure(request.id, e),
            }
        }
        _ => error_action(Some(request.id), rpc::RpcError::method_not_found(), false),
    }
}
fn failure(id: String, error: app::TechnicalLogError) -> DispatchAction {
    let e = match error {
        app::TechnicalLogError::InvalidInput => return invalid(id),
        app::TechnicalLogError::PermissionDenied => rpc::RpcError::permission_denied(),
        app::TechnicalLogError::CursorExpired => rpc::RpcError {
            code: "log_cursor_expired".to_owned(),
            message: "The requested log position is no longer retained.".to_owned(),
            diagnostic_id: None,
            data: None,
        },
        app::TechnicalLogError::Unavailable => rpc::RpcError {
            code: "log_unavailable".to_owned(),
            message: "The technical logs could not be read.".to_owned(),
            diagnostic_id: None,
            data: None,
        },
    };
    error_action(Some(id), e, false)
}

#[cfg(all(test, windows))]
mod ipc_tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};
    use tracing_subscriber::prelude::*;
    async fn exchange(
        stream: &mut alcomd_platform::IpcStream,
        request: serde_json::Value,
    ) -> serde_json::Value {
        let bytes = serde_json::to_vec(&request).expect("request JSON");
        stream
            .write_u32_le(bytes.len() as u32)
            .await
            .expect("prefix");
        stream.write_all(&bytes).await.expect("request");
        stream.flush().await.expect("flush request");
        let length = stream.read_u32_le().await.expect("response prefix");
        assert!(length <= 4 * 1024 * 1024);
        let mut bytes = vec![0; length as usize];
        stream.read_exact(&mut bytes).await.expect("response");
        serde_json::from_slice(&bytes).expect("response JSON")
    }
    fn request(id: &str, method: &str, params: serde_json::Value) -> serde_json::Value {
        serde_json::json!({"id":id,"method":method,"params":params})
    }
    #[tokio::test]
    async fn real_private_pipe_reads_standard_records_and_health_without_touching_the_daemon_endpoint()
     {
        use tracing_subscriber::fmt::MakeWriter;
        let parent = std::env::temp_dir().canonicalize().expect("temp parent");
        let directory = parent.join(format!("alcomd-log-ipc-{}", app::OperationId::new()));
        std::fs::create_dir(&directory).expect("isolated directory");
        let runtime = alcomd_logging::LoggingRuntime::start(&directory, "core");
        let source = runtime.source();
        let application = LogApplication::new(CoreLogReader(source.clone()));
        let pipe = format!("\\\\.\\pipe\\alcomd-log-test-{}", app::OperationId::new());
        let server = ServerOptions::new()
            .first_pipe_instance(true)
            .create(&pipe)
            .expect("private test pipe");
        let client = ClientOptions::new()
            .open(&pipe)
            .expect("connect private test pipe");
        server.connect().await.expect("server connection");
        let task = tokio::spawn(async move {
            let mut stream = alcomd_platform::IpcStream::Server(server);
            let mut state = crate::ConnectionState {
                connection_id: "private-log-test".into(),
                client_instance_id: None,
                handshake_complete: false,
                capabilities: std::collections::HashSet::new(),
            };
            while let Ok(length) = stream.read_u32_le().await {
                assert!(length <= 4 * 1024 * 1024);
                let mut bytes = vec![0; length as usize];
                stream.read_exact(&mut bytes).await.expect("request body");
                let request: rpc::RequestEnvelope =
                    serde_json::from_slice(&bytes).expect("typed request");
                let action = if request.method == rpc::METHOD_SYSTEM_HELLO {
                    crate::dispatch_hello(request, &state)
                } else if !state.handshake_complete {
                    crate::error_action(
                        Some(request.id),
                        rpc::RpcError::handshake_required(),
                        false,
                    )
                } else {
                    let access = app::AccessContext::local_owner();
                    let future = dispatch(request, &state, &application, &access);
                    let Some(action) = until_disconnect(&mut stream, future)
                        .await
                        .expect("read lifecycle")
                    else {
                        break;
                    };
                    action
                };
                if let Some(caps) = action.complete_handshake {
                    state.handshake_complete = true;
                    state.capabilities = caps;
                }
                let bytes = serde_json::to_vec(&action.response).expect("response JSON");
                assert!(bytes.len() <= 4 * 1024 * 1024);
                stream
                    .write_u32_le(bytes.len() as u32)
                    .await
                    .expect("response prefix");
                stream.write_all(&bytes).await.expect("response body");
                stream.flush().await.expect("response flush");
            }
        });
        let mut client = alcomd_platform::IpcStream::Client(client);
        let hello=exchange(&mut client,request("hello",rpc::METHOD_SYSTEM_HELLO,serde_json::json!({"rpcVersion":1,"client":{"name":"log-ipc-test","version":"test","instanceId":"isolated"},"capabilities":[rpc::CAPABILITY_DIAGNOSTICS_LOG_V1]}))).await;
        assert!(hello.get("result").is_some());
        let first = exchange(
            &mut client,
            request(
                "baseline",
                rpc::METHOD_DIAGNOSTICS_LOG_READ,
                serde_json::json!({}),
            ),
        )
        .await;
        let baseline: rpc::DiagnosticLogReadResult =
            serde_json::from_value(first["result"].clone()).expect("baseline DTO");
        assert!(baseline.baseline && baseline.items.is_empty());
        {
            let maker = source.writer();
            let mut writer = maker.make_writer();
            let _ = std::io::Write::write_all(
                &mut writer,
                &vec![b'x'; alcomd_logging::RECORD_BYTES + 1],
            );
        }
        let changed = exchange(
            &mut client,
            request(
                "health",
                rpc::METHOD_DIAGNOSTICS_LOG_READ,
                serde_json::json!({"cursor":baseline.next_cursor,"follow":true}),
            ),
        )
        .await;
        let changed: rpc::DiagnosticLogReadResult =
            serde_json::from_value(changed["result"].clone()).expect("independent health DTO");
        assert!(changed.items.is_empty());
        assert_eq!(changed.health.oversize_rejected, 1);
        assert!(changed.health.has_gap);
        let subscriber = tracing_subscriber::registry().with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_writer(source.writer()),
        );
        tracing::subscriber::with_default(subscriber, || {
            tracing::error!(
                path = "C:\\项目 A\\manifest.json",
                "actual IPC source failure"
            )
        });
        let records = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            exchange(
                &mut client,
                request(
                    "record",
                    rpc::METHOD_DIAGNOSTICS_LOG_READ,
                    serde_json::json!({"cursor":changed.next_cursor,"follow":true}),
                ),
            ),
        )
        .await
        .expect("new record delivery");
        let records: rpc::DiagnosticLogReadResult =
            serde_json::from_value(records["result"].clone()).expect("record DTO");
        assert_eq!(records.items.len(), 1);
        assert_eq!(
            records.items[0].record["fields"]["path"],
            "C:\\项目 A\\manifest.json"
        );
        let old = exchange(
            &mut client,
            request(
                "history",
                rpc::METHOD_DIAGNOSTICS_LOG_HISTORY,
                serde_json::json!({}),
            ),
        )
        .await;
        let old: rpc::DiagnosticLogHistoryResult =
            serde_json::from_value(old["result"].clone()).expect("history DTO");
        assert_eq!(old.items, records.items);
        let frame = serde_json::to_vec(&request(
            "idle",
            rpc::METHOD_DIAGNOSTICS_LOG_READ,
            serde_json::json!({"cursor":records.next_cursor,"follow":true}),
        ))
        .expect("idle request");
        client
            .write_u32_le(frame.len() as u32)
            .await
            .expect("idle prefix");
        client.write_all(&frame).await.expect("idle request");
        client.flush().await.expect("idle flush");
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), client.read_u32_le())
                .await
                .is_err(),
            "healthy idle must not return empty frames"
        );
        drop(client);
        tokio::time::timeout(std::time::Duration::from_secs(2), task)
            .await
            .expect("disconnect cancels waiter")
            .expect("server task");
        let report = runtime.shutdown();
        assert!(report.drained && report.flushed);
        drop(source);
        assert!(
            directory
                .canonicalize()
                .expect("canonical isolated directory")
                .starts_with(&parent)
        );
        std::fs::remove_dir_all(directory).expect("isolated cleanup");
    }
}
