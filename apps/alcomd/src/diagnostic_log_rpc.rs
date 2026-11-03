use alcomd_application as app;
use alcomd_protocol as rpc;

use super::{
    AccessContext, ConnectionState, DispatchAction, error_action, invalid, require_capability,
    success_action,
};

pub(super) fn is_log_read(payload: &[u8]) -> bool {
    #[derive(serde::Deserialize)]
    struct Method {
        method: String,
    }
    serde_json::from_slice::<Method>(payload)
        .is_ok_and(|request| request.method == rpc::METHOD_DIAGNOSTICS_LOG_READ)
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
    log: &app::DiagnosticLog,
    access: &AccessContext,
) -> DispatchAction {
    if let Some(action) = require_capability(&request.id, state, rpc::CAPABILITY_DIAGNOSTICS_LOG_V1)
    {
        return action;
    }
    let params: rpc::DiagnosticLogReadParams = match serde_json::from_value(request.params) {
        Ok(params) => params,
        Err(_) => return invalid(request.id),
    };
    let cursor = params.cursor.map(|value| app::DiagnosticLogCursor {
        instance_id: value.instance_id,
        sequence: value.sequence,
    });
    match log.read(access, cursor, params.limit, params.wait_ms).await {
        Ok(page) => success_action(
            request.id,
            rpc::DiagnosticLogReadResult {
                items: page
                    .items
                    .into_iter()
                    .map(|item| rpc::DiagnosticLogItem {
                        instance_id: item.instance_id,
                        sequence: item.sequence,
                        occurred_at_ms: item.occurred_at_ms,
                        level: match item.level {
                            app::DiagnosticLogLevel::Trace => rpc::DiagnosticLogLevel::Trace,
                            app::DiagnosticLogLevel::Debug => rpc::DiagnosticLogLevel::Debug,
                            app::DiagnosticLogLevel::Info => rpc::DiagnosticLogLevel::Info,
                            app::DiagnosticLogLevel::Warning => rpc::DiagnosticLogLevel::Warning,
                            app::DiagnosticLogLevel::Error => rpc::DiagnosticLogLevel::Error,
                        },
                        code: item.code,
                        diagnostic_id: item.diagnostic_id,
                        notify: item.notify,
                    })
                    .collect(),
                next_cursor: rpc::DiagnosticLogCursor {
                    instance_id: page.next_cursor.instance_id,
                    sequence: page.next_cursor.sequence,
                },
                reset: page.reset,
                gap: page.gap,
            },
            None,
        ),
        Err(app::DiagnosticLogError::InvalidInput) => invalid(request.id),
        Err(app::DiagnosticLogError::PermissionDenied) => {
            error_action(Some(request.id), rpc::RpcError::permission_denied(), false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use app::{Permission, PrincipalId};
    use serde_json::json;

    fn state(allowed: bool) -> ConnectionState {
        ConnectionState {
            connection_id: "test-connection".to_owned(),
            client_instance_id: Some("test-client".to_owned()),
            handshake_complete: true,
            capabilities: if allowed {
                [rpc::CAPABILITY_DIAGNOSTICS_LOG_V1.to_owned()].into()
            } else {
                Default::default()
            },
        }
    }

    fn request(params: serde_json::Value) -> rpc::RequestEnvelope {
        rpc::RequestEnvelope {
            id: "log-read".to_owned(),
            method: rpc::METHOD_DIAGNOSTICS_LOG_READ.to_owned(),
            params,
        }
    }

    #[tokio::test]
    async fn disconnect_cancels_only_a_log_wait_and_pipelining_is_rejected() {
        use tokio::io::AsyncWriteExt;
        assert!(is_log_read(
            br#"{"method":"diagnostics.log.read","params":{}}"#
        ));
        assert!(is_log_read(
            br#"{"method":"diagnostics.log.\u0072ead","params":{}}"#
        ));
        assert!(!is_log_read(
            br#"{"method":"projects.applyCopy","params":{"method":"diagnostics.log.read"}}"#
        ));
        let log = app::DiagnosticLog::new();
        let owner = AccessContext::local_owner();
        let state = state(true);
        let page = log
            .read(&owner, None, None, None)
            .await
            .expect("valid diagnostic log test fixture");
        let params = json!({"cursor":{"instanceId":page.next_cursor.instance_id,"sequence":page.next_cursor.sequence},"waitMs":20000});
        let (client, mut server) = tokio::io::duplex(64);
        drop(client);
        let response = tokio::time::timeout(
            std::time::Duration::from_millis(500),
            until_disconnect(
                &mut server,
                dispatch(request(params.clone()), &state, &log, &owner),
            ),
        )
        .await
        .expect("client closure must cancel the read promptly")
        .expect("valid diagnostic log test fixture");
        assert!(response.is_none());
        let (mut client, mut server) = tokio::io::duplex(64);
        client
            .write_all(&[1])
            .await
            .expect("valid diagnostic log test fixture");
        let error = until_disconnect(&mut server, dispatch(request(params), &state, &log, &owner))
            .await
            .err()
            .expect("a serial log read cannot accept a pipelined request");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }

    #[tokio::test]
    async fn log_rpc_is_capability_and_owner_guarded_and_rejects_unknown_fields() {
        let log = app::DiagnosticLog::new();
        let owner = AccessContext::local_owner();
        let response = dispatch(request(json!({})), &state(false), &log, &owner)
            .await
            .response;
        assert_eq!(
            response["error"]["code"],
            rpc::error_code::CAPABILITY_REQUIRED
        );
        let external = AccessContext::new(
            PrincipalId::parse("client:external").expect("valid diagnostic log test fixture"),
            [Permission::DiagnosticsRead],
        );
        let response = dispatch(request(json!({})), &state(true), &log, &external)
            .await
            .response;
        assert_eq!(response["error"]["code"], "permission_denied");
        let no_permission = AccessContext::new(PrincipalId::local_owner(), []);
        let response = dispatch(request(json!({})), &state(true), &log, &no_permission)
            .await
            .response;
        assert_eq!(response["error"]["code"], "permission_denied");
        for params in [
            json!({"limit":0}),
            json!({"limit":201}),
            json!({"waitMs":20001}),
            json!({"path":"private"}),
        ] {
            let response = dispatch(request(params), &state(true), &log, &owner)
                .await
                .response;
            assert_eq!(response["error"]["code"], "invalid_request");
        }
    }

    #[tokio::test]
    async fn log_rpc_round_trips_one_source_and_long_reads_do_not_lock_other_reads() {
        use std::future::Future;
        use std::task::Poll;
        let log = app::DiagnosticLog::new();
        let owner = AccessContext::local_owner();
        let state = state(true);
        let initial = dispatch(request(json!({})), &state, &log, &owner)
            .await
            .response;
        let cursor = initial["result"]["nextCursor"].clone();
        let mut waiting = Box::pin(dispatch(
            request(json!({"cursor":cursor,"waitMs":20000})),
            &state,
            &log,
            &owner,
        ));
        std::future::poll_fn(|context| Poll::Ready(waiting.as_mut().poll(context).is_pending()))
            .await
            .then_some(())
            .expect("long read must wait for a new record");
        let snapshot = tokio::time::timeout(
            std::time::Duration::from_millis(500),
            dispatch(request(json!({"cursor":cursor})), &state, &log, &owner),
        )
        .await
        .expect("another read is not blocked")
        .response;
        assert_eq!(snapshot["result"]["items"], json!([]));
        let item = log.record(
            app::DiagnosticLogLevel::Error,
            Some("core.failure"),
            None,
            None,
        );
        let response = waiting.await.response;
        let result: rpc::DiagnosticLogReadResult =
            serde_json::from_value(response["result"].clone())
                .expect("valid diagnostic log test fixture");
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].sequence, item.sequence);
        assert_eq!(result.items[0].code, "core.failure");
        assert_eq!(result.items[0].diagnostic_id, item.diagnostic_id);
        assert!(result.items[0].notify);
        assert_eq!(result.next_cursor.sequence, item.sequence);
        assert!(!result.reset && !result.gap);
    }
}
