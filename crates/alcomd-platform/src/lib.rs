//! M1 operating-system adapters for the per-user daemon endpoint.
//!
//! This crate owns endpoint discovery, local IPC access control, and the
//! lifecycle-bound single-instance primitive. It intentionally contains no RPC
//! DTOs or business rules.

use std::path::PathBuf;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub use unix::{IpcListener, IpcStream, connect, endpoint_display};
#[cfg(windows)]
pub use windows::{IpcListener, IpcStream, connect, endpoint_display};

/// Optional runtime-path override used by isolated tests and development tools.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct IpcConfig {
    runtime_directory: Option<PathBuf>,
}

impl IpcConfig {
    /// Creates a configuration for an explicitly isolated Unix runtime directory.
    #[must_use]
    pub fn isolated(runtime_directory: PathBuf) -> Self {
        Self {
            runtime_directory: Some(runtime_directory),
        }
    }

    pub(crate) fn runtime_directory(&self) -> Option<&std::path::Path> {
        self.runtime_directory.as_deref()
    }
}

/// Failure to bind the authoritative per-user daemon endpoint.
#[derive(Debug)]
pub enum BindError {
    /// Another process already owns the lifecycle-bound instance lock.
    AlreadyRunning,
    /// Endpoint preparation or listener creation failed.
    Io(std::io::Error),
}

impl std::fmt::Display for BindError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyRunning => {
                formatter.write_str("another ALCOMD daemon instance is running")
            }
            Self::Io(error) => write!(
                formatter,
                "failed to prepare the ALCOMD IPC endpoint: {error}"
            ),
        }
    }
}

impl std::error::Error for BindError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::AlreadyRunning => None,
            Self::Io(error) => Some(error),
        }
    }
}

impl From<std::io::Error> for BindError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}
