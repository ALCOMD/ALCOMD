use std::io;
use std::time::{Duration, Instant};

use alcomd_application::{
    ExtensionBackgroundIntegration, ExtensionInstanceLease, LocalIpcTarget, M6Error, M6ErrorCode,
};
use alcomd_extensions::{MAX_LOCAL_IPC_BYTES, decode_local_ipc_bytes, encode_local_ipc_bytes};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[cfg(windows)]
type LocalStream = tokio::net::windows::named_pipe::NamedPipeClient;
#[cfg(unix)]
type LocalStream = tokio::net::UnixStream;

pub(super) const TICK_INTERVAL: Duration = Duration::from_secs(2);

pub(super) struct BackgroundIntegration {
    pub(super) next_tick: Instant,
    config: ExtensionBackgroundIntegration,
    channel: Option<Channel>,
}

struct Channel {
    id: String,
    scope: String,
    stream: LocalStream,
}

impl BackgroundIntegration {
    pub(super) fn new(config: ExtensionBackgroundIntegration) -> Option<Self> {
        config.enabled.then(|| Self {
            next_tick: Instant::now(),
            config,
            channel: None,
        })
    }

    pub(super) async fn invoke(
        &mut self,
        applications: &super::Applications,
        lease: ExtensionInstanceLease,
        capability: &str,
        input: Value,
        can_connect: bool,
        budget: Duration,
    ) -> Result<Value, M6Error> {
        let expiry =
            Duration::from_millis(lease.expires_at_ms.saturating_sub(
                super::current_time_ms().map_err(|_| error(M6ErrorCode::Internal))?,
            ));
        let budget = expiry.min(budget);
        if budget.is_zero() {
            return Err(error(M6ErrorCode::InstanceStale));
        }
        if capability == "host-unity-sessions.current" {
            let observe = async {
                let _authority = applications
                    .store
                    .authorize_background_integration(lease, None)
                    .await?;
                let observation = applications
                    .m5
                    .observe_local_unity_sessions()
                    .await
                    .map_err(|_| error(M6ErrorCode::Internal))?;
                Ok(json!({
                    "editorCount": observation.editor_count,
                    "currentSession": observation.current_session.map(|session| json!({
                        "projectId": session.project_id.to_string(), "startedAtMs": session.started_at_ms,
                    })),
                }))
            };
            return tokio::time::timeout(budget, observe)
                .await
                .map_err(|_| error(M6ErrorCode::ResourceLimit))?;
        }
        self.invoke_ipc(
            &applications.store,
            lease,
            capability,
            &input,
            can_connect,
            budget,
        )
        .await
    }

    async fn invoke_ipc(
        &mut self,
        store: &alcomd_store::StateStoreHandle,
        lease: ExtensionInstanceLease,
        capability: &str,
        input: &Value,
        can_connect: bool,
        budget: Duration,
    ) -> Result<Value, M6Error> {
        let elapsed_limit = budget.min(Duration::from_millis(750));
        match tokio::time::timeout(
            elapsed_limit,
            self.ipc_operation(store, lease, capability, input, can_connect),
        )
        .await
        {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(failure)) => {
                if matches!(
                    failure.code(),
                    M6ErrorCode::PermissionDenied | M6ErrorCode::InstanceStale
                ) {
                    self.channel = None;
                }
                Err(failure)
            }
            Err(_) => {
                // A timed-out write may have emitted a prefix. Never reuse that byte stream.
                if capability == "host-local-ipc.write" {
                    self.channel = None;
                }
                Ok(ipc_failure("timed_out"))
            }
        }
    }

    async fn ipc_operation(
        &mut self,
        store: &alcomd_store::StateStoreHandle,
        lease: ExtensionInstanceLease,
        capability: &str,
        input: &Value,
        can_connect: bool,
    ) -> Result<Value, M6Error> {
        if capability == "host-local-ipc.connect" {
            if !can_connect {
                return Err(error(M6ErrorCode::PermissionDenied));
            }
            if self.channel.is_some() {
                return Err(error(M6ErrorCode::ResourceLimit));
            }
            let target_id = input["targetId"]
                .as_str()
                .ok_or_else(|| error(M6ErrorCode::InvalidInput))?;
            let target = self
                .config
                .local_ipc_targets
                .get(target_id)
                .ok_or_else(|| error(M6ErrorCode::ScopeDenied))?;
            let scope = target.scope_key(target_id);
            let _authority = store
                .authorize_background_integration(lease, Some(scope.clone()))
                .await?;
            return match connect_target(target).await {
                Ok(stream) => {
                    let id = alcomd_application::OperationId::new().to_string();
                    let result = json!({"channel": id, "clientProcessId": std::process::id()});
                    self.channel = Some(Channel { id, scope, stream });
                    Ok(result)
                }
                Err(failure) if failure.kind() == io::ErrorKind::PermissionDenied => {
                    Err(error(M6ErrorCode::ScopeDenied))
                }
                Err(_) => Ok(ipc_failure("unavailable")),
            };
        }

        let id = input["channel"]
            .as_str()
            .ok_or_else(|| error(M6ErrorCode::InvalidInput))?;
        let Some(channel) = self.channel.as_mut().filter(|channel| id == channel.id) else {
            return Ok(ipc_failure("invalid_channel"));
        };
        let _authority = store
            .authorize_background_integration(lease, Some(channel.scope.clone()))
            .await?;
        match capability {
            "host-local-ipc.read" => {
                let length = input["maxBytes"]
                    .as_u64()
                    .and_then(|len| usize::try_from(len).ok())
                    .filter(|len| (1..=MAX_LOCAL_IPC_BYTES).contains(len))
                    .ok_or_else(|| error(M6ErrorCode::InvalidInput))?;
                let mut bytes = vec![0; length];
                if let Ok(size) = channel.stream.read(&mut bytes).await
                    && size > 0
                {
                    let encoded = encode_local_ipc_bytes(&bytes[..size])
                        .ok_or_else(|| error(M6ErrorCode::Internal))?;
                    return Ok(json!({"bytes": encoded}));
                }
            }
            "host-local-ipc.write" => {
                let bytes = input["bytes"]
                    .as_str()
                    .and_then(decode_local_ipc_bytes)
                    .ok_or_else(|| error(M6ErrorCode::InvalidInput))?;
                if channel.stream.write_all(&bytes).await.is_ok() {
                    return Ok(json!({}));
                }
            }
            "host-local-ipc.close" => {
                self.channel = None;
                return Ok(json!({}));
            }
            _ => return Err(error(M6ErrorCode::PermissionDenied)),
        }
        self.channel = None;
        Ok(ipc_failure("peer_closed"))
    }
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
async fn connect_target(target: &LocalIpcTarget) -> io::Result<LocalStream> {
    #[cfg(windows)]
    let platform = target.windows.as_ref();
    #[cfg(target_os = "macos")]
    let platform = target.macos.as_ref();
    #[cfg(target_os = "linux")]
    let platform = target.linux.as_ref();
    let platform = platform.ok_or(io::ErrorKind::Unsupported)?;
    #[cfg(windows)]
    let names = &platform.pipe_names;
    #[cfg(unix)]
    let names = &platform.socket_names;

    let mut identity_denied = false;
    for name in names {
        #[cfg(windows)]
        let connection = alcomd_platform::connect_verified_local_pipe(
            name.clone(),
            platform.executable_name.clone(),
            platform.publisher.clone(),
        )
        .await;
        #[cfg(target_os = "macos")]
        let connection = alcomd_platform::connect_verified_local_socket(
            name,
            alcomd_platform::UnixIpcIdentity::MacApplication {
                bundle_identifier: platform.bundle_identifier.clone(),
                team_identifier: platform.team_identifier.clone(),
            },
        )
        .await;
        #[cfg(target_os = "linux")]
        let connection = alcomd_platform::connect_verified_local_socket(
            name,
            alcomd_platform::UnixIpcIdentity::LinuxSystemExecutable(
                platform.system_executable.clone().into(),
            ),
        )
        .await;
        match connection {
            Err(failure) if failure.kind() == io::ErrorKind::PermissionDenied => {
                // Another process may occupy an earlier slot. Never use its stream.
                identity_denied = true;
            }
            Err(failure)
                if matches!(
                    failure.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
                ) => {}
            other => return other,
        }
    }
    Err(if identity_denied {
        io::ErrorKind::PermissionDenied
    } else {
        io::ErrorKind::NotFound
    }
    .into())
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
async fn connect_target(_: &LocalIpcTarget) -> io::Result<LocalStream> {
    Err(io::ErrorKind::Unsupported.into())
}

fn ipc_failure(code: &str) -> Value {
    json!({"ipcError": code})
}

fn error(code: M6ErrorCode) -> M6Error {
    M6Error::new(code)
}
