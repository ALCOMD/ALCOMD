use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::windows::named_pipe::{
    ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
};

use crate::{BindError, DataConfig, IpcConfig};

#[path = "windows_file_identity.rs"]
mod file_identity;
#[path = "windows_known_folder.rs"]
mod known_folder;
#[path = "windows_security.rs"]
mod security;

pub use file_identity::{WindowsFileIdentity, file_identity};

/// A connected Windows Named Pipe endpoint.
pub enum IpcStream {
    Server(NamedPipeServer),
    Client(NamedPipeClient),
}

/// Process-lifetime ownership acquired before the authoritative store opens.
pub struct DaemonInstance {
    endpoint: String,
    instance: security::WindowsInstanceGuard,
}

impl DaemonInstance {
    /// Acquires the current-user named mutex without publishing a pipe endpoint.
    pub fn acquire(config: &IpcConfig) -> Result<Self, BindError> {
        reject_runtime_override(config)?;
        let sid = security::current_user_sid_string()?;
        let endpoint = format!(r"\\.\pipe\CQMHV.ALCOMD.{sid}.rpc-v1");
        let mutex_name = format!(r"Local\CQMHV.ALCOMD.{sid}.daemon-v1");
        let instance = security::acquire_instance_mutex(&mutex_name)?;
        Ok(Self { endpoint, instance })
    }

    /// Publishes the first secure pipe after store initialization and recovery.
    pub fn bind(self) -> Result<IpcListener, BindError> {
        let mut options = ServerOptions::new();
        options
            .first_pipe_instance(true)
            .reject_remote_clients(true);
        let next = security::create_secure_pipe(&options, &self.endpoint)?;
        Ok(IpcListener {
            endpoint: self.endpoint,
            next,
            _instance: self.instance,
        })
    }
}

impl AsyncRead for IpcStream {
    fn poll_read(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Server(stream) => Pin::new(stream).poll_read(context, buffer),
            Self::Client(stream) => Pin::new(stream).poll_read(context, buffer),
        }
    }
}

impl AsyncWrite for IpcStream {
    fn poll_write(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        match self.get_mut() {
            Self::Server(stream) => Pin::new(stream).poll_write(context, buffer),
            Self::Client(stream) => Pin::new(stream).poll_write(context, buffer),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        match self.get_mut() {
            Self::Server(stream) => Pin::new(stream).poll_flush(context),
            Self::Client(stream) => Pin::new(stream).poll_flush(context),
        }
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        match self.get_mut() {
            Self::Server(stream) => Pin::new(stream).poll_shutdown(context),
            Self::Client(stream) => Pin::new(stream).poll_shutdown(context),
        }
    }
}

/// Bound current-user Named Pipe and lifecycle-bound named mutex.
pub struct IpcListener {
    endpoint: String,
    next: NamedPipeServer,
    _instance: security::WindowsInstanceGuard,
}

impl IpcListener {
    /// Acquires the per-user mutex and creates the first secure pipe instance.
    pub fn bind(config: &IpcConfig) -> Result<Self, BindError> {
        DaemonInstance::acquire(config)?.bind()
    }

    /// Accepts one client and immediately provisions the next secure instance.
    pub async fn accept(&mut self) -> io::Result<IpcStream> {
        self.next.connect().await?;
        let mut options = ServerOptions::new();
        options.reject_remote_clients(true);
        let replacement = security::create_secure_pipe(&options, &self.endpoint)?;
        let connected = std::mem::replace(&mut self.next, replacement);
        Ok(IpcStream::Server(connected))
    }
}

/// Opens the current-user Named Pipe.
pub async fn connect(config: &IpcConfig) -> io::Result<IpcStream> {
    reject_runtime_override(config)?;
    let endpoint = endpoint_display(config)?;
    ClientOptions::new().open(endpoint).map(IpcStream::Client)
}

/// Returns the SID-qualified pipe name.
pub fn endpoint_display(config: &IpcConfig) -> io::Result<String> {
    reject_runtime_override(config)?;
    let sid = security::current_user_sid_string()?;
    Ok(format!(r"\\.\pipe\CQMHV.ALCOMD.{sid}.rpc-v1"))
}

/// Returns the current user's formal Windows Local AppData known folder.
pub fn local_app_data_directory() -> io::Result<std::path::PathBuf> {
    known_folder::local_app_data_directory()
}

/// Creates the product data directory and returns its state database path.
pub fn state_database_path(config: &DataConfig) -> io::Result<std::path::PathBuf> {
    let directory = match config.data_directory() {
        Some(directory) => directory.to_path_buf(),
        None => local_app_data_directory()?.join("ALCOMD").join("data"),
    };
    if !directory.is_absolute()
        || directory.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "state data directory must be an absolute normalized path",
        ));
    }
    std::fs::create_dir_all(&directory)?;
    let metadata = std::fs::symlink_metadata(&directory)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "state data path is not a directory",
        ));
    }
    Ok(directory.join("state.db"))
}

fn reject_runtime_override(config: &IpcConfig) -> io::Result<()> {
    if config.runtime_directory().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "runtime-directory overrides are not supported on Windows",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isolated_state_database_path_stays_inside_the_override() {
        let directory = std::env::temp_dir().join(format!(
            "alcomd-data-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        let path = state_database_path(&DataConfig::isolated(directory.clone()))
            .expect("isolated state path");
        assert_eq!(path, directory.join("state.db"));
        std::fs::remove_dir(directory).expect("remove isolated data directory");
    }
}
