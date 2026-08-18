use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
#[cfg(unix)]
use std::time::{SystemTime, UNIX_EPOCH};

use alcomd_client::{AlcomdClient, ClientConfig};
use alcomd_platform::{IpcConfig, IpcStream};
use alcomd_protocol::{
    CAPABILITY_STATE_CHECK_V1, ErrorResponse, HelloParams, METHOD_STATE_CHECK, METHOD_SYSTEM_HELLO,
    OperationState, RPC_VERSION, RequestEnvelope, Response, StateCheckParams, decode_frame_length,
    encode_frame, error_code,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn m2_state_check_round_trips_through_real_ipc_and_durable_store() {
    let (ipc, client_config, cleanup) = isolated_configuration();
    let shutdown = Arc::new(AtomicBool::new(false));
    let server_shutdown = Arc::clone(&shutdown);
    let server_ipc = ipc.clone();
    let daemon = tokio::spawn(async move {
        alcomd_daemon::serve_until(server_ipc, wait_for_shutdown(server_shutdown)).await
    });

    let mut raw = connect_raw_with_retry(&ipc).await;
    write_request(
        &mut raw,
        RequestEnvelope {
            id: "hello-no-capability".to_owned(),
            method: METHOD_SYSTEM_HELLO.to_owned(),
            params: serde_json::to_value(HelloParams {
                rpc_version: RPC_VERSION,
                client: alcomd_protocol::ClientInfo {
                    name: "compat-client".to_owned(),
                    version: "1".to_owned(),
                    instance_id: "no-m2-capabilities".to_owned(),
                },
                capabilities: Vec::new(),
            })
            .expect("hello params"),
        },
    )
    .await;
    let _: serde_json::Value = read_response(&mut raw).await;
    write_request(
        &mut raw,
        RequestEnvelope {
            id: "missing-capability".to_owned(),
            method: METHOD_STATE_CHECK.to_owned(),
            params: serde_json::to_value(StateCheckParams {
                idempotency_key: "must-not-run".to_owned(),
            })
            .expect("state.check params"),
        },
    )
    .await;
    let response: Response<serde_json::Value> = read_response(&mut raw).await;
    let Response::Error(ErrorResponse { error, .. }) = response else {
        panic!("missing capability must fail");
    };
    assert_eq!(error.code, error_code::CAPABILITY_REQUIRED);
    drop(raw);

    let mut abandoned = connect_raw_with_retry(&ipc).await;
    write_hello(
        &mut abandoned,
        "abandoned-response",
        vec![CAPABILITY_STATE_CHECK_V1.to_owned()],
    )
    .await;
    let _: serde_json::Value = read_response(&mut abandoned).await;
    write_request(
        &mut abandoned,
        RequestEnvelope {
            id: "lost-state-check-response".to_owned(),
            method: METHOD_STATE_CHECK.to_owned(),
            params: serde_json::to_value(StateCheckParams {
                idempotency_key: "m2-lost-response".to_owned(),
            })
            .expect("state.check params"),
        },
    )
    .await;
    drop(abandoned);

    let mut recovery_client = connect_with_retry(client_config.clone()).await;
    let recovered = recovery_client
        .state_check("m2-lost-response".to_owned())
        .await
        .expect("recover response with the same key");
    assert!(recovered.replayed);

    let mut first_client = connect_with_retry(client_config.clone()).await;
    let mut second_client = connect_with_retry(client_config.clone()).await;
    let (first, second) = tokio::join!(
        first_client.state_check("m2-concurrent-key".to_owned()),
        second_client.state_check("m2-concurrent-key".to_owned())
    );
    let first = first.expect("first concurrent state check");
    let second = second.expect("second concurrent state check");
    assert_eq!(first.operation_id, second.operation_id);
    assert_ne!(first.replayed, second.replayed);

    let mut client = connect_with_retry(client_config).await;
    let accepted = client
        .state_check("m2-e2e-check".to_owned())
        .await
        .expect("start state check");
    assert!(!accepted.replayed);
    let completed = wait_for_terminal(&mut client, &accepted.operation_id).await;
    assert_eq!(completed.state, OperationState::Succeeded);
    assert!(completed.result.is_some());

    let replay = client
        .state_check("m2-e2e-check".to_owned())
        .await
        .expect("replay state check");
    assert_eq!(replay.operation_id, accepted.operation_id);
    assert!(replay.replayed);

    let operations = client
        .operations_list(None, None)
        .await
        .expect("list operations");
    assert_eq!(operations.operations.len(), 3);
    let events = client.events_list(0, None).await.expect("list events");
    assert!(events.events.len() >= 5);
    assert_eq!(
        events.next_sequence,
        events.events.last().expect("last event").sequence
    );
    let empty = client
        .events_list(events.next_sequence, None)
        .await
        .expect("read empty event page");
    assert!(empty.events.is_empty());
    assert_eq!(empty.next_sequence, events.next_sequence);

    shutdown.store(true, Ordering::Release);
    let result = tokio::time::timeout(Duration::from_secs(3), daemon)
        .await
        .expect("daemon stop timeout")
        .expect("join daemon");
    assert!(result.is_ok());
    if let Some(path) = cleanup {
        let _ = std::fs::remove_dir_all(path);
    }
}

async fn write_hello(stream: &mut IpcStream, instance_id: &str, capabilities: Vec<String>) {
    write_request(
        stream,
        RequestEnvelope {
            id: format!("hello-{instance_id}"),
            method: METHOD_SYSTEM_HELLO.to_owned(),
            params: serde_json::to_value(HelloParams {
                rpc_version: RPC_VERSION,
                client: alcomd_protocol::ClientInfo {
                    name: "m2-raw-client".to_owned(),
                    version: "1".to_owned(),
                    instance_id: instance_id.to_owned(),
                },
                capabilities,
            })
            .expect("hello params"),
        },
    )
    .await;
}

async fn wait_for_terminal(
    client: &mut AlcomdClient,
    operation_id: &str,
) -> alcomd_protocol::Operation {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let operation = client
            .operation_get(operation_id.to_owned())
            .await
            .expect("get operation");
        if matches!(
            operation.state,
            OperationState::Succeeded | OperationState::Failed | OperationState::Cancelled
        ) {
            return operation;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "operation timed out"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn connect_with_retry(config: ClientConfig) -> AlcomdClient {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        match AlcomdClient::connect(config.clone()).await {
            Ok(client) => return client,
            Err(_) if tokio::time::Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            Err(error) => panic!("daemon did not become ready: {error}"),
        }
    }
}

async fn connect_raw_with_retry(config: &IpcConfig) -> IpcStream {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        match alcomd_platform::connect(config).await {
            Ok(stream) => return stream,
            Err(_) if tokio::time::Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            Err(error) => panic!("raw client did not connect: {error}"),
        }
    }
}

async fn write_request(stream: &mut IpcStream, request: RequestEnvelope) {
    let payload = serde_json::to_vec(&request).expect("serialize request");
    let frame = encode_frame(&payload).expect("frame request");
    stream.write_all(&frame).await.expect("write request");
    stream.flush().await.expect("flush request");
}

async fn read_response<T: serde::de::DeserializeOwned>(stream: &mut IpcStream) -> T {
    let mut prefix = [0_u8; 4];
    stream.read_exact(&mut prefix).await.expect("read prefix");
    let length = decode_frame_length(prefix).expect("response length");
    let mut payload = vec![0_u8; length];
    stream.read_exact(&mut payload).await.expect("read payload");
    serde_json::from_slice(&payload).expect("response JSON")
}

async fn wait_for_shutdown(shutdown: Arc<AtomicBool>) {
    while !shutdown.load(Ordering::Acquire) {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[cfg(unix)]
fn isolated_configuration() -> (IpcConfig, ClientConfig, Option<PathBuf>) {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    #[cfg(target_os = "macos")]
    let base = PathBuf::from("/private/tmp");
    #[cfg(not(target_os = "macos"))]
    let base = std::env::temp_dir();
    let path = base.join(format!("acm2-rpc-{}-{nonce}", std::process::id()));
    (
        IpcConfig::isolated(path.clone()),
        ClientConfig::default()
            .without_daemon_start()
            .with_runtime_directory(path.clone()),
        Some(path),
    )
}

#[cfg(windows)]
fn isolated_configuration() -> (IpcConfig, ClientConfig, Option<PathBuf>) {
    (
        IpcConfig::default(),
        ClientConfig::default().without_daemon_start(),
        None,
    )
}
