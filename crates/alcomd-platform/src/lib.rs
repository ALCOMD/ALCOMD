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
pub use unix::{
    DaemonInstance, IpcListener, IpcStream, connect, endpoint_display, file_identity_key,
    state_database_path,
};
#[cfg(windows)]
pub use windows::{
    DaemonInstance, IpcListener, IpcStream, WindowsFileIdentity, connect, endpoint_display,
    file_identity, local_app_data_directory, state_database_path,
};

#[cfg(windows)]
/// Returns the fixed 24-byte Windows filesystem-object identity key.
pub fn file_identity_key(path: &std::path::Path) -> std::io::Result<Vec<u8>> {
    Ok(file_identity(path)?.to_key().to_vec())
}

/// Resolves an existing directory to its final absolute Unicode path and opaque object identity.
pub fn resolve_directory_identity(path: &std::path::Path) -> std::io::Result<(PathBuf, Vec<u8>)> {
    if !path.is_absolute() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "path must be absolute",
        ));
    }
    let canonical = std::fs::canonicalize(path)?;
    if canonical.to_str().is_none() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "path encoding is unsupported",
        ));
    }
    if !std::fs::metadata(&canonical)?.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "path is not a directory",
        ));
    }
    let identity = file_identity_key(&canonical)?;
    Ok((canonical, identity))
}

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

/// Optional data-directory override used only by isolated tests and development.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DataConfig {
    data_directory: Option<PathBuf>,
}

impl DataConfig {
    /// Uses an explicit isolated directory instead of the formal platform path.
    #[must_use]
    pub fn isolated(data_directory: PathBuf) -> Self {
        Self {
            data_directory: Some(data_directory),
        }
    }

    pub(crate) fn data_directory(&self) -> Option<&std::path::Path> {
        self.data_directory.as_deref()
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
