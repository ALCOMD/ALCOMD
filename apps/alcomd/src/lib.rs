//! Minimal M1 daemon transport and dispatcher.

use std::future::Future;
use std::io;

use alcomd_platform::{BindError, IpcConfig, IpcListener, IpcStream};
use alcomd_protocol::{
    ErrorResponse, HelloParams, HelloResult, METHOD_SYSTEM_HELLO, METHOD_SYSTEM_STATUS,
    RPC_VERSION, RequestEnvelope, RpcError, SuccessResponse, SystemStatusResult,
    decode_frame_length, encode_frame,
};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Runs the M1 daemon until the supplied shutdown signal completes.
pub async fn serve_until<F>(config: IpcConfig, shutdown: F) -> Result<(), BindError>
where
    F: Future<Output = ()>,
{
    let listener = IpcListener::bind(&config)?;
    run_listener(listener, shutdown)
        .await
        .map_err(BindError::Io)
}

async fn run_listener<F>(mut listener: IpcListener, shutdown: F) -> io::Result<()>
where
    F: Future<Output = ()>,
{
    let mut connections = tokio::task::JoinSet::new();
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            result = listener.accept() => {
                let stream = result?;
                connections.spawn(async move {
                    let _ = serve_connection(stream).await;
                });
            }
            result = connections.join_next(), if !connections.is_empty() => {
                let _ = result;
            }
            () = &mut shutdown => break,
        }
    }

    connections.abort_all();
    while connections.join_next().await.is_some() {}
    Ok(())
}

async fn serve_connection(mut stream: IpcStream) -> io::Result<()> {
    let mut handshake_complete = false;
    loop {
        let payload = match read_frame(&mut stream).await? {
            Some(payload) => payload,
            None => return Ok(()),
        };
        let action = dispatch_payload(&payload, handshake_complete);
        write_json_frame(&mut stream, &action.response).await?;
        if action.complete_handshake {
            handshake_complete = true;
        }
        if action.close_after_response {
            return Ok(());
        }
    }
}

struct DispatchAction {
    response: Value,
    complete_handshake: bool,
    close_after_response: bool,
}

fn dispatch_payload(payload: &[u8], handshake_complete: bool) -> DispatchAction {
    let parsed_value: Value = match serde_json::from_slice(payload) {
        Ok(value) => value,
        Err(_) => return error_action(None, RpcError::invalid_request(), false),
    };
    let recovered_id = parsed_value
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty() && id.len() <= alcomd_protocol::MAX_REQUEST_ID_BYTES)
        .map(str::to_owned);
    let request: RequestEnvelope = match serde_json::from_value(parsed_value) {
        Ok(request) => request,
        Err(_) => return error_action(recovered_id, RpcError::invalid_request(), false),
    };
    if request.validate().is_err() {
        return error_action(Some(request.id), RpcError::invalid_request(), false);
    }

    if request.method == METHOD_SYSTEM_HELLO {
        if handshake_complete {
            return error_action(
                Some(request.id),
                RpcError::handshake_already_completed(),
                false,
            );
        }
        let hello: HelloParams = match serde_json::from_value(request.params) {
            Ok(hello) => hello,
            Err(_) => {
                return error_action(Some(request.id), RpcError::invalid_request(), false);
            }
        };
        if hello.validate().is_err() {
            return error_action(Some(request.id), RpcError::invalid_request(), false);
        }
        if hello.rpc_version != RPC_VERSION {
            return error_action(
                Some(request.id),
                RpcError::rpc_version_unsupported(hello.rpc_version),
                true,
            );
        }
        return success_action(request.id, HelloResult::m1(), true);
    }

    if !handshake_complete {
        return error_action(Some(request.id), RpcError::handshake_required(), false);
    }

    if request.method == METHOD_SYSTEM_STATUS {
        if request
            .params
            .as_object()
            .is_none_or(|params| !params.is_empty())
        {
            return error_action(Some(request.id), RpcError::invalid_request(), false);
        }
        let application_status = alcomd_application::system_status();
        let result = SystemStatusResult {
            product: alcomd_protocol::PRODUCT_FAMILY.to_owned(),
            daemon_version: env!("CARGO_PKG_VERSION").to_owned(),
            rpc_version: RPC_VERSION,
            state: application_status.state().as_str().to_owned(),
            capabilities: Vec::new(),
        };
        return success_action(request.id, result, false);
    }

    error_action(Some(request.id), RpcError::method_not_found(), false)
}

fn success_action<T: serde::Serialize>(id: String, result: T, handshake: bool) -> DispatchAction {
    let response = serde_json::to_value(SuccessResponse { id, result })
        .expect("approved response DTOs must serialize");
    DispatchAction {
        response,
        complete_handshake: handshake,
        close_after_response: false,
    }
}

fn error_action(id: Option<String>, error: RpcError, close: bool) -> DispatchAction {
    let response = serde_json::to_value(ErrorResponse { id, error })
        .expect("approved error DTO must serialize");
    DispatchAction {
        response,
        complete_handshake: false,
        close_after_response: close,
    }
}

async fn read_frame(stream: &mut IpcStream) -> io::Result<Option<Vec<u8>>> {
    let mut prefix = [0_u8; 4];
    let count = stream.read(&mut prefix[..1]).await?;
    if count == 0 {
        return Ok(None);
    }
    stream.read_exact(&mut prefix[1..]).await?;
    let length = decode_frame_length(prefix)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let mut payload = vec![0_u8; length];
    stream.read_exact(&mut payload).await?;
    Ok(Some(payload))
}

async fn write_json_frame(stream: &mut IpcStream, value: &Value) -> io::Result<()> {
    let payload = serde_json::to_vec(value)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let frame = encode_frame(&payload)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    stream.write_all(&frame).await?;
    stream.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use alcomd_protocol::{ClientInfo, error_code};
    use serde_json::json;

    fn request(id: &str, method: &str, params: Value) -> Vec<u8> {
        serde_json::to_vec(&RequestEnvelope {
            id: id.to_owned(),
            method: method.to_owned(),
            params,
        })
        .expect("serialize request")
    }

    #[test]
    fn status_requires_handshake() {
        let action = dispatch_payload(&request("1", METHOD_SYSTEM_STATUS, json!({})), false);
        assert_eq!(
            action.response["error"]["code"],
            error_code::HANDSHAKE_REQUIRED
        );
    }

    #[test]
    fn hello_then_status_are_truthful() {
        let hello = HelloParams {
            rpc_version: RPC_VERSION,
            client: ClientInfo {
                name: "test".to_owned(),
                version: "1".to_owned(),
                instance_id: "instance".to_owned(),
            },
            capabilities: vec!["unknown.safe-capability".to_owned()],
        };
        let action = dispatch_payload(
            &request(
                "hello",
                METHOD_SYSTEM_HELLO,
                serde_json::to_value(hello).expect("hello params"),
            ),
            false,
        );
        assert!(action.complete_handshake);
        assert_eq!(action.response["result"]["capabilities"], json!([]));

        let status = dispatch_payload(&request("status", METHOD_SYSTEM_STATUS, json!({})), true);
        assert_eq!(status.response["result"]["state"], "ready");
        assert!(status.response["result"].get("pid").is_none());
    }

    #[test]
    fn unsupported_major_closes_only_after_structured_error() {
        let action = dispatch_payload(
            &request(
                "hello",
                METHOD_SYSTEM_HELLO,
                json!({
                    "rpcVersion": 99,
                    "client": {"name": "test", "version": "1", "instanceId": "i"},
                    "capabilities": []
                }),
            ),
            false,
        );
        assert_eq!(
            action.response["error"]["code"],
            error_code::RPC_VERSION_UNSUPPORTED
        );
        assert!(action.close_after_response);
    }

    #[test]
    fn repeated_hello_and_unknown_method_have_stable_errors() {
        let repeated = dispatch_payload(
            &request(
                "hello-again",
                METHOD_SYSTEM_HELLO,
                json!({
                    "rpcVersion": RPC_VERSION,
                    "client": {"name": "test", "version": "1", "instanceId": "i"},
                    "capabilities": []
                }),
            ),
            true,
        );
        assert_eq!(
            repeated.response["error"]["code"],
            error_code::HANDSHAKE_ALREADY_COMPLETED
        );
        assert!(!repeated.close_after_response);

        let unknown = dispatch_payload(&request("unknown", "system.unknown", json!({})), true);
        assert_eq!(
            unknown.response["error"]["code"],
            error_code::METHOD_NOT_FOUND
        );
    }

    #[test]
    fn complete_malformed_payloads_return_invalid_request() {
        for payload in [b"not-json".as_slice(), br#"{"id":"ok"}"#.as_slice()] {
            let action = dispatch_payload(payload, false);
            assert_eq!(
                action.response["error"]["code"],
                error_code::INVALID_REQUEST
            );
            assert!(!action.close_after_response);
        }
    }
}
