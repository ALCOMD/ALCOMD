//! M1 operating-system adapters for the per-user daemon endpoint.
//!
//! This crate owns endpoint discovery, local IPC access control, and the
//! lifecycle-bound single-instance primitive. It intentionally contains no RPC
//! DTOs or business rules.

use std::path::PathBuf;

mod process;
mod unity;

pub use process::{ProcessEvidence, ProcessSnapshot, observe_processes};
pub use unity::{
    UnityArchitecture, ValidatedUnityExecutable, discover_unity_executables, launch_unity_editor,
    validate_unity_executable,
};

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
    file_identity, file_link_count, local_app_data_directory, state_database_path,
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

/// Flushes metadata for an existing directory using the platform's ordinary filesystem handle.
#[cfg(all(unix, not(target_os = "macos")))]
pub fn sync_directory(path: &std::path::Path) -> std::io::Result<()> {
    std::fs::File::open(path)?.sync_all()
}

/// Flushes directory metadata when macOS exposes a supported directory-sync operation.
///
/// macOS permits opening a directory but returns `EINVAL` from `fsync(2)` for that handle.
/// The exact unsupported-operation result is therefore the strongest available success
/// condition after the directory was opened; every other open or sync failure is preserved.
#[cfg(target_os = "macos")]
pub fn sync_directory(path: &std::path::Path) -> std::io::Result<()> {
    const INVALID_ARGUMENT: i32 = 22;

    let directory = std::fs::File::open(path)?;
    match directory.sync_all() {
        Err(error) if error.raw_os_error() == Some(INVALID_ARGUMENT) => Ok(()),
        result => result,
    }
}

/// Flushes metadata for an existing directory using a backup-semantics filesystem handle.
#[cfg(windows)]
pub fn sync_directory(path: &std::path::Path) -> std::io::Result<()> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS;

    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)?
        .sync_all()
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

#[cfg(test)]
mod tests {
    use super::sync_directory;

    #[test]
    fn existing_directory_sync_uses_the_platform_durability_primitive() {
        let path = std::env::temp_dir().join(format!(
            "alcomd-platform-sync-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir(&path).expect("create directory");
        sync_directory(&path).expect("sync directory");
        std::fs::remove_dir(&path).expect("remove directory");
    }

    #[test]
    fn missing_directory_sync_preserves_the_io_error() {
        let path = std::env::temp_dir().join(format!(
            "alcomd-platform-missing-sync-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        assert_eq!(
            sync_directory(&path).expect_err("missing directory").kind(),
            std::io::ErrorKind::NotFound
        );
    }
}
