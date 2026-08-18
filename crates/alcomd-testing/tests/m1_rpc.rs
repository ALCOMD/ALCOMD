use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
#[cfg(unix)]
use std::time::{SystemTime, UNIX_EPOCH};

use alcomd_client::{AlcomdClient, ClientConfig};
use alcomd_platform::{BindError, IpcConfig, IpcListener, IpcStream};
use alcomd_protocol::{
    ErrorResponse, MAX_FRAME_PAYLOAD_BYTES, METHOD_SYSTEM_STATUS, RequestEnvelope, Response,
    decode_frame_length, encode_frame, error_code,
};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn m1_rpc_and_ipc_contract() {
    real_daemon_client_and_single_instance_contract().await;
    wire_errors_are_structured_but_framing_errors_close().await;
}

async fn real_daemon_client_and_single_instance_contract() {
    let (ipc, client_config, cleanup) = isolated_configuration();
    let server_config = ipc.clone();
    let shutdown = Arc::new(AtomicBool::new(false));
    let server_shutdown = Arc::clone(&shutdown);
    let daemon = tokio::spawn(async move {
        alcomd_daemon::serve_until(server_config, wait_for_shutdown(server_shutdown)).await
    });

    let mut client = connect_with_retry(client_config.clone()).await;
    let status = client.system_status().await.expect("query system status");
    assert_eq!(status.state, "ready");
    assert_eq!(status.rpc_version, 1);
    assert_eq!(status.capabilities.len(), 3);

    assert!(matches!(
        IpcListener::bind(&ipc),
        Err(BindError::AlreadyRunning)
    ));

    let (first, second) = tokio::join!(
        AlcomdClient::connect(client_config.clone()),
        AlcomdClient::connect(client_config)
    );
    assert!(first.is_ok());
    assert!(second.is_ok());

    drop((client, first, second));
    stop_daemon(shutdown, daemon).await;
    cleanup_runtime(cleanup);
}

async fn wire_errors_are_structured_but_framing_errors_close() {
    let (ipc, _, cleanup) = isolated_configuration();
    let server_config = ipc.clone();
    let shutdown = Arc::new(AtomicBool::new(false));
    let server_shutdown = Arc::clone(&shutdown);
    let daemon = tokio::spawn(async move {
        alcomd_daemon::serve_until(server_config, wait_for_shutdown(server_shutdown)).await
    });

    let mut stream = connect_raw_with_retry(&ipc).await;
    let request = RequestEnvelope {
        id: "pre-hello".to_owned(),
        method: METHOD_SYSTEM_STATUS.to_owned(),
        params: json!({}),
    };
    write_request(&mut stream, &request).await;
    let response: Response<serde_json::Value> = read_response(&mut stream).await;
    let Response::Error(ErrorResponse { error, .. }) = response else {
        panic!("expected structured handshake error");
    };
    assert_eq!(error.code, error_code::HANDSHAKE_REQUIRED);

    let mut malformed = connect_raw_with_retry(&ipc).await;
    malformed
        .write_all(&((MAX_FRAME_PAYLOAD_BYTES as u32) + 1).to_le_bytes())
        .await
        .expect("write oversized prefix");
    let mut byte = [0_u8; 1];
    let closed = tokio::time::timeout(Duration::from_secs(2), malformed.read(&mut byte))
        .await
        .expect("server closes framing error")
        .expect("read close");
    assert_eq!(closed, 0);

    drop((stream, malformed));
    stop_daemon(shutdown, daemon).await;
    cleanup_runtime(cleanup);
}

async fn wait_for_shutdown(shutdown: Arc<AtomicBool>) {
    while !shutdown.load(Ordering::Acquire) {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn stop_daemon(
    shutdown: Arc<AtomicBool>,
    daemon: tokio::task::JoinHandle<Result<(), BindError>>,
) {
    shutdown.store(true, Ordering::Release);
    let result = tokio::time::timeout(Duration::from_secs(2), daemon)
        .await
        .expect("daemon stops before timeout")
        .expect("join daemon");
    assert!(result.is_ok(), "daemon stopped with error: {result:?}");
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
            Err(error) if is_absent(&error) && tokio::time::Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            Err(error) => panic!("raw daemon connection failed: {error}"),
        }
    }
}

async fn write_request(stream: &mut IpcStream, request: &RequestEnvelope) {
    let payload = serde_json::to_vec(request).expect("serialize request");
    let frame = encode_frame(&payload).expect("frame request");
    stream.write_all(&frame).await.expect("write request");
    stream.flush().await.expect("flush request");
}

async fn read_response<T: serde::de::DeserializeOwned>(stream: &mut IpcStream) -> T {
    let mut prefix = [0_u8; 4];
    stream.read_exact(&mut prefix).await.expect("read prefix");
    let length = decode_frame_length(prefix).expect("valid response length");
    let mut payload = vec![0_u8; length];
    stream
        .read_exact(&mut payload)
        .await
        .expect("read response");
    serde_json::from_slice(&payload).expect("decode response")
}

fn is_absent(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
    ) || error.kind() == io::ErrorKind::WouldBlock
        || cfg!(windows) && error.raw_os_error() == Some(231)
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
    let path = base.join(format!("acm1-rpc-{}-{nonce}", std::process::id()));
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

fn cleanup_runtime(path: Option<PathBuf>) {
    if let Some(path) = path {
        let _ = std::fs::remove_dir_all(path);
    }
}
