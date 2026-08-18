//! Official M1 Rust client for the per-user ALCOMD daemon.

use std::io;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use alcomd_platform::{IpcConfig, IpcStream};
use alcomd_protocol::{
    ClientInfo, HelloParams, HelloResult, METHOD_SYSTEM_HELLO, METHOD_SYSTEM_STATUS, RPC_VERSION,
    RequestEnvelope, Response, RpcError, SystemStatusResult, decode_frame_length, encode_frame,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::json;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const START_TIMEOUT: Duration = Duration::from_secs(5);
const RETRY_INTERVAL: Duration = Duration::from_millis(50);
static INSTANCE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Client connection settings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientConfig {
    runtime_directory: Option<PathBuf>,
    start_daemon: bool,
    daemon_path: Option<PathBuf>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            runtime_directory: None,
            start_daemon: true,
            daemon_path: None,
        }
    }
}

impl ClientConfig {
    /// Disables automatic daemon startup.
    #[must_use]
    pub fn without_daemon_start(mut self) -> Self {
        self.start_daemon = false;
        self
    }

    /// Uses an isolated Unix runtime directory.
    #[must_use]
    pub fn with_runtime_directory(mut self, path: PathBuf) -> Self {
        self.runtime_directory = Some(path);
        self
    }

    /// Overrides the daemon executable for an isolated integration test.
    #[must_use]
    pub fn with_daemon_path(mut self, path: PathBuf) -> Self {
        self.daemon_path = Some(path);
        self
    }

    fn ipc_config(&self) -> IpcConfig {
        self.runtime_directory
            .clone()
            .map(IpcConfig::isolated)
            .unwrap_or_default()
    }
}

/// A connection to the per-user `alcomd` process.
pub struct AlcomdClient {
    stream: IpcStream,
    next_request_id: u64,
}

impl AlcomdClient {
    /// Connects to the per-user daemon, optionally starting its sibling binary.
    pub async fn connect(config: ClientConfig) -> Result<Self, ClientError> {
        let stream = connect_with_policy(&config).await?;
        let mut client = Self {
            stream,
            next_request_id: 1,
        };
        let _ = client.hello().await?;
        Ok(client)
    }

    /// Performs the mandatory M1 handshake.
    async fn hello(&mut self) -> Result<HelloResult, ClientError> {
        let sequence = INSTANCE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let params = HelloParams {
            rpc_version: RPC_VERSION,
            client: ClientInfo {
                name: "alcomd-client".to_owned(),
                version: env!("CARGO_PKG_VERSION").to_owned(),
                instance_id: format!("{}-{sequence}", std::process::id()),
            },
            capabilities: Vec::new(),
        };
        self.call(METHOD_SYSTEM_HELLO, params).await
    }

    /// Queries the truthful minimal daemon status.
    pub async fn system_status(&mut self) -> Result<SystemStatusResult, ClientError> {
        self.call(METHOD_SYSTEM_STATUS, json!({})).await
    }

    async fn call<P, T>(&mut self, method: &str, params: P) -> Result<T, ClientError>
    where
        P: Serialize,
        T: DeserializeOwned,
    {
        let id = self.next_request_id.to_string();
        self.next_request_id = self.next_request_id.saturating_add(1);
        let request = RequestEnvelope {
            id,
            method: method.to_owned(),
            params: serde_json::to_value(params).map_err(|_| ClientError::InvalidResponse)?,
        };
        let payload = serde_json::to_vec(&request).map_err(|_| ClientError::InvalidResponse)?;
        let frame = encode_frame(&payload).map_err(|_| ClientError::InvalidResponse)?;
        self.stream
            .write_all(&frame)
            .await
            .map_err(ClientError::Transport)?;
        self.stream.flush().await.map_err(ClientError::Transport)?;

        let payload = read_frame(&mut self.stream).await?;
        let response: Response<T> =
            serde_json::from_slice(&payload).map_err(|_| ClientError::InvalidResponse)?;
        match response {
            Response::Success(success) if success.id == request.id => Ok(success.result),
            Response::Success(_) => Err(ClientError::InvalidResponse),
            Response::Error(error) => Err(ClientError::Remote(error.error)),
        }
    }
}

async fn connect_with_policy(config: &ClientConfig) -> Result<IpcStream, ClientError> {
    let ipc = config.ipc_config();
    match alcomd_platform::connect(&ipc).await {
        Ok(stream) => return Ok(stream),
        Err(error) if is_endpoint_absent(&error) && config.start_daemon => spawn_daemon(config)?,
        Err(error) if is_transient(&error) => {}
        Err(error) => return Err(ClientError::Transport(error)),
    }

    let deadline = Instant::now() + START_TIMEOUT;
    loop {
        match alcomd_platform::connect(&ipc).await {
            Ok(stream) => return Ok(stream),
            Err(error) if is_transient(&error) && Instant::now() < deadline => {
                tokio::time::sleep(RETRY_INTERVAL).await;
            }
            Err(error) if is_transient(&error) => return Err(ClientError::StartTimeout),
            Err(error) => return Err(ClientError::Transport(error)),
        }
    }
}

fn spawn_daemon(config: &ClientConfig) -> Result<(), ClientError> {
    let daemon = match &config.daemon_path {
        Some(path) => path.clone(),
        None => sibling_daemon_path()?,
    };
    let mut command = Command::new(daemon);
    if let Some(runtime_directory) = &config.runtime_directory {
        command.arg("--runtime-dir").arg(runtime_directory);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(ClientError::StartDaemon)?;
    Ok(())
}

fn sibling_daemon_path() -> Result<PathBuf, ClientError> {
    let executable = std::env::current_exe().map_err(ClientError::StartDaemon)?;
    let directory = executable
        .parent()
        .ok_or(ClientError::DaemonPathUnavailable)?;
    #[cfg(windows)]
    let name = "alcomd.exe";
    #[cfg(not(windows))]
    let name = "alcomd";
    Ok(directory.join(name))
}

fn is_endpoint_absent(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
    )
}

fn is_transient(error: &io::Error) -> bool {
    is_endpoint_absent(error)
        || error.kind() == io::ErrorKind::WouldBlock
        || cfg!(windows) && error.raw_os_error() == Some(231)
}

async fn read_frame(stream: &mut IpcStream) -> Result<Vec<u8>, ClientError> {
    let mut prefix = [0_u8; 4];
    stream
        .read_exact(&mut prefix)
        .await
        .map_err(ClientError::Transport)?;
    let length = decode_frame_length(prefix).map_err(|_| ClientError::InvalidResponse)?;
    let mut payload = vec![0_u8; length];
    stream
        .read_exact(&mut payload)
        .await
        .map_err(ClientError::Transport)?;
    Ok(payload)
}

/// Errors produced by the RPC client.
#[derive(Debug, Error)]
pub enum ClientError {
    /// Local IPC failed. Callers should avoid exposing the source path verbatim.
    #[error("failed to communicate with the ALCOMD daemon")]
    Transport(#[source] io::Error),
    /// Starting the sibling daemon failed.
    #[error("failed to start the ALCOMD daemon")]
    StartDaemon(#[source] io::Error),
    /// The process layout does not contain a sibling daemon location.
    #[error("the ALCOMD daemon executable location is unavailable")]
    DaemonPathUnavailable,
    /// The daemon did not become reachable within the approved five-second bound.
    #[error("the ALCOMD daemon did not become ready within five seconds")]
    StartTimeout,
    /// The daemon returned a stable public RPC error.
    #[error("daemon request failed")]
    Remote(RpcError),
    /// The daemon response violated the frozen M1 contract.
    #[error("the ALCOMD daemon returned an invalid RPC response")]
    InvalidResponse,
}
