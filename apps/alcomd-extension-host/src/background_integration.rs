//! Versioned guest bindings for Core-owned Unity observation and local IPC.

use alcomd_extensions::{
    HostStableError, MAX_LOCAL_IPC_BYTES, decode_local_ipc_bytes, encode_local_ipc_bytes,
};
use serde_json::{Value, json};
use wasmtime::component::{HasSelf, Linker};

use super::{HostState, trap_error, types, wit_error};

mod bindings {
    wasmtime::component::bindgen!({
        path: "../../specs/extensions/wit/extension-v1",
        world: "background-integration-v1",
        imports: { default: async },
        exports: { default: async },
        with: { "alcomd:extension/types@1.0.0": crate::types },
    });
}
pub(super) use bindings::BackgroundIntegrationV1 as Bindings;

pub(super) fn preflight(
    instance: wasmtime::component::InstancePre<HostState>,
) -> anyhow::Result<()> {
    bindings::BackgroundIntegrationV1Pre::new(instance)
        .map_err(|_| anyhow::anyhow!("invalid background ABI"))?;
    Ok(())
}
use bindings::alcomd::extension::{host_local_ipc as ipc, host_unity_sessions as unity};

pub(super) fn add_to_linker(linker: &mut Linker<HostState>) -> anyhow::Result<()> {
    unity::add_to_linker::<_, HasSelf<_>>(linker, |state| state)
        .map_err(|_| anyhow::anyhow!("Unity sessions link"))?;
    ipc::add_to_linker::<_, HasSelf<_>>(linker, |state| state)
        .map_err(|_| anyhow::anyhow!("Local IPC link"))?;
    Ok(())
}

pub(super) async fn tick(
    bindings: &Bindings,
    store: &mut wasmtime::Store<HostState>,
) -> Result<Value, HostStableError> {
    match bindings
        .alcomd_extension_guest_background()
        .call_tick(store)
        .await
    {
        Ok(Ok(())) => Ok(json!({})),
        Ok(Err(error)) => Err(wit_error(error)),
        Err(error) => Err(trap_error(&error)),
    }
}

impl unity::Host for HostState {
    async fn current(&mut self) -> Result<unity::Observation, unity::SessionError> {
        use unity::SessionError as Error;
        let value = self
            .protocol
            .capability("host-unity-sessions.current", json!({}))
            .map_err(|failure| match failure.code {
                types::ErrorCode::PermissionDenied | types::ErrorCode::ScopeDenied => {
                    Error::PermissionDenied
                }
                types::ErrorCode::LeaseExpired => Error::LeaseExpired,
                types::ErrorCode::LeaseRevoked => Error::LeaseRevoked,
                _ => Error::Unavailable,
            })?;
        let current_session = match value.get("currentSession") {
            Some(Value::Null) => None,
            Some(session) => Some(unity::UnitySession {
                project_id: session["projectId"]
                    .as_str()
                    .ok_or(Error::Unavailable)?
                    .to_owned(),
                started_at_ms: session["startedAtMs"].as_u64().ok_or(Error::Unavailable)?,
            }),
            None => return Err(Error::Unavailable),
        };
        let editor_count = value["editorCount"]
            .as_u64()
            .and_then(|count| u32::try_from(count).ok())
            .ok_or(Error::Unavailable)?;
        Ok(unity::Observation {
            current_session,
            editor_count,
        })
    }
}

impl HostState {
    fn invoke_local_ipc(&mut self, capability: &str, input: Value) -> Result<Value, ipc::IpcError> {
        use ipc::IpcError as Error;
        let response =
            self.protocol
                .capability(capability, input)
                .map_err(|failure| match failure.code {
                    types::ErrorCode::InvalidArgument => Error::InvalidArgument,
                    types::ErrorCode::PermissionDenied => Error::PermissionDenied,
                    types::ErrorCode::ScopeDenied => Error::ScopeDenied,
                    types::ErrorCode::LeaseExpired => Error::LeaseExpired,
                    types::ErrorCode::LeaseRevoked => Error::LeaseRevoked,
                    types::ErrorCode::ResourceLimit => Error::ResourceLimit,
                    _ => Error::Unavailable,
                })?;
        match response.get("ipcError").and_then(Value::as_str) {
            Some("timed_out") => Err(Error::TimedOut),
            Some("peer_closed") => Err(Error::PeerClosed),
            Some("invalid_channel") => Err(Error::InvalidChannel),
            Some(_) => Err(Error::Unavailable),
            None => Ok(response),
        }
    }
}

impl ipc::Host for HostState {
    async fn connect(&mut self, target_id: String) -> Result<ipc::Connection, ipc::IpcError> {
        let value =
            self.invoke_local_ipc("host-local-ipc.connect", json!({"targetId": target_id}))?;
        Ok(ipc::Connection {
            channel: value["channel"]
                .as_str()
                .ok_or(ipc::IpcError::Unavailable)?
                .to_owned(),
            client_process_id: value["clientProcessId"]
                .as_u64()
                .and_then(|pid| u32::try_from(pid).ok())
                .ok_or(ipc::IpcError::Unavailable)?,
        })
    }

    async fn read(&mut self, channel: String, max_bytes: u32) -> Result<Vec<u8>, ipc::IpcError> {
        if !(1..=MAX_LOCAL_IPC_BYTES as u32).contains(&max_bytes) {
            return Err(ipc::IpcError::InvalidArgument);
        }
        let value = self.invoke_local_ipc(
            "host-local-ipc.read",
            json!({"channel": channel, "maxBytes": max_bytes}),
        )?;
        value["bytes"]
            .as_str()
            .and_then(decode_local_ipc_bytes)
            .filter(|bytes| !bytes.is_empty() && bytes.len() <= max_bytes as usize)
            .ok_or(ipc::IpcError::Unavailable)
    }

    async fn write(&mut self, channel: String, bytes: Vec<u8>) -> Result<(), ipc::IpcError> {
        let bytes = encode_local_ipc_bytes(&bytes).ok_or(ipc::IpcError::InvalidArgument)?;
        self.invoke_local_ipc(
            "host-local-ipc.write",
            json!({"channel": channel, "bytes": bytes}),
        )?;
        Ok(())
    }

    async fn close(&mut self, channel: String) -> Result<(), ipc::IpcError> {
        self.invoke_local_ipc("host-local-ipc.close", json!({"channel": channel}))?;
        Ok(())
    }
}
