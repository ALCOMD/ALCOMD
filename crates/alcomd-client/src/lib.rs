//! Official Rust client for ALCOMD RPC.
//!
//! The transport is intentionally not implemented in M0.

use alcomd_protocol::HelloResponse;
use thiserror::Error;

/// Client connection settings.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ClientConfig {
    /// Optional explicit endpoint. Production clients normally use platform discovery.
    pub endpoint: Option<String>,
}

/// A connection to the per-user `alcomd` process.
#[derive(Clone, Debug)]
pub struct AlcomdClient {
    config: ClientConfig,
}

impl AlcomdClient {
    /// Creates an unconnected client handle.
    #[must_use]
    pub const fn new(config: ClientConfig) -> Self {
        Self { config }
    }

    /// Returns the configured endpoint, if any.
    #[must_use]
    pub fn endpoint(&self) -> Option<&str> {
        self.config.endpoint.as_deref()
    }

    /// Placeholder for the M1 RPC handshake.
    pub async fn hello(&self) -> Result<HelloResponse, ClientError> {
        Err(ClientError::TransportNotImplemented)
    }
}

/// Errors produced by the RPC client.
#[derive(Debug, Error)]
pub enum ClientError {
    /// M0 contains no IPC transport.
    #[error("ALCOMD RPC transport is not implemented in the repository scaffold")]
    TransportNotImplemented,
}
