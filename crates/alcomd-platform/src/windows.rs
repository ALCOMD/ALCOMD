use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::windows::named_pipe::{
    ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
};

use crate::{BindError, IpcConfig};

#[path = "windows_security.rs"]
mod security;

/// A connected Windows Named Pipe endpoint.
pub enum IpcStream {
    Server(NamedPipeServer),
    Client(NamedPipeClient),
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
        reject_runtime_override(config)?;
        let sid = security::current_user_sid_string()?;
        let endpoint = format!(r"\\.\pipe\CQMHV.ALCOMD.{sid}.rpc-v1");
        let mutex_name = format!(r"Local\CQMHV.ALCOMD.{sid}.daemon-v1");
        let instance = security::acquire_instance_mutex(&mutex_name)?;
        let mut options = ServerOptions::new();
        options
            .first_pipe_instance(true)
            .reject_remote_clients(true);
        let next = security::create_secure_pipe(&options, &endpoint)?;
        Ok(Self {
            endpoint,
            next,
            _instance: instance,
        })
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

fn reject_runtime_override(config: &IpcConfig) -> io::Result<()> {
    if config.runtime_directory().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "runtime-directory overrides are not supported on Windows",
        ));
    }
    Ok(())
}
