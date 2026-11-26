//! M1 operating-system adapters for the per-user daemon endpoint.
//!
//! This crate owns endpoint discovery, local IPC access control, and the
//! lifecycle-bound single-instance primitive. It intentionally contains no RPC
//! DTOs or business rules.

use std::path::PathBuf;

pub mod client_installation;
pub mod client_peer;
mod core_installation;
mod credential_vault;
pub mod desktop;
mod external_path;
#[cfg(target_os = "linux")]
mod linux_portable_cli;
#[cfg(target_os = "linux")]
mod linux_portable_origin;
mod process;
mod project_delete;
mod project_discovery;
mod storage_directories;
mod unity;
mod unity_hub;
mod unity_hub_open;
mod unity_registration;

pub use core_installation::{
    CORE_EXECUTABLE_NAME, PRODUCT_PAYLOAD_MANIFEST_NAME, WINDOWS_CRT_FILE,
    WINDOWS_PRODUCT_SIDECARS, core_installation_registration, open_owned_installation_record,
    registered_core_executable, validate_installation_record_parent,
};
#[cfg(windows)]
pub use core_installation::{
    WINDOWS_SCOPE_DESTINATION_COMMIT_RECORD, WINDOWS_SCOPE_DESTINATION_RECORD,
    WindowsScopeTransitionDestination,
};
#[cfg(target_os = "linux")]
pub use core_installation::{
    LINUX_BOOTSTRAP_EXECUTABLE_NAME, LINUX_GUI_EXECUTABLE_NAME, check_linux_core_registration,
    check_linux_portable_update_registration, linux_installation_bootstrap,
    linux_installation_runtime, linux_portable_runtime, linux_portable_runtime_for_version,
    linux_portable_runtime_is_known, linux_portable_update_bootstrap,
    linux_retired_deb_installation_bootstrap, publish_linux_core_registration,
    publish_linux_portable_update_registration,
};
#[cfg(windows)]
pub use core_installation::{
    WINDOWS_ACTIVE_RUNTIME_RECORD, WINDOWS_BOOTSTRAP_EXECUTABLE_NAME, WINDOWS_CLI_EXECUTABLE_NAME,
    WINDOWS_GUI_EXECUTABLE_NAME, WINDOWS_SCOPE_TRANSITION_RECORD, WINDOWS_UPDATE_INTENT_RECORD,
    WindowsInstallationUpdate, WindowsScopeRegistrationTransfer, WindowsScopeTransitionSource,
    check_windows_core_registration,
    create_windows_machine_installation_directory, create_windows_machine_installation_record,
    hold_windows_installation, open_windows_machine_installation_record,
    publish_windows_core_registration, remove_windows_core_registration,
    validate_windows_machine_installation_record_parent, windows_current_user_installation_root,
    windows_installation_bootstrap, windows_installation_runtime,
    windows_machine_installation_root, windows_machine_runtime_bootstrap,
    windows_retired_machine_installation_bootstrap,
};
pub use credential_vault::{delete_credential, read_credential, write_credential};
pub use external_path::external_filesystem_path;
#[cfg(target_os = "linux")]
pub use linux_portable_cli::{
    LINUX_CLI_EXECUTABLE_NAME, PORTABLE_CLI_LAUNCHER_NAME, PortableCliEntry, PortableCliEntryState,
    linux_portable_cli_bootstrap, lock_linux_portable_cli,
};
#[cfg(target_os = "linux")]
pub use linux_portable_origin::{PORTABLE_ORIGIN_FILE, PortableImage, PortableOrigin};
pub use storage_directories::{
    default_storage_directories, normalize_storage_directory, open_plain_file,
    prepare_storage_directory, publish_path_no_replace, resolve_plain_project_directory,
    resolve_plain_storage_directory,
};
pub use unity_hub_open::{UnityHubLaunchError, UnityHubLaunchErrorKind, UnityHubProcessLauncher};

pub use unity_hub::{
    ConfiguredUnityHubStatus, UnityHubAvailability, UnityHubInspection, UnityHubOrigin,
    UnityHubPathError, UnityHubPathErrorKind, inspect_unity_hub, normalize_unity_hub_path,
};

pub use process::{ProcessEvidence, ProcessSnapshot, observe_processes};

#[cfg(target_os = "macos")]
/// Process that currently receives keyboard input in the macOS desktop session, when available.
#[must_use]
pub fn foreground_process_id() -> Option<u32> {
    objc2::rc::autoreleasepool(|_| {
        let application = objc2_app_kit::NSWorkspace::sharedWorkspace().frontmostApplication()?;
        let pid = u32::try_from(application.processIdentifier()).ok()?;
        (pid != 0).then_some(pid)
    })
}
pub use project_delete::{
    ProjectDeleteFilesystemError, ProjectDeleteFilesystemErrorKind, project_delete_cleanup,
    project_delete_preflight, verify_project_path_boundary,
};
pub use project_discovery::{
    DirectoryDiscovery, DiscoveredDirectory, discover_directories, discovered_directory_is_current,
};
#[cfg(windows)]
pub use unity::is_verified_non_editor_executable;
pub use unity::{
    DiscoveredUnityExecutable, UnityArchitecture, UnityMigrationExit, UnityMigrationProcess,
    ValidatedUnityExecutable, discover_unity_executables, inspect_unity_installation,
    launch_unity_editor, spawn_unity_editor_migration, validate_unity_executable,
};

#[cfg(unix)]
mod unix;
#[cfg(unix)]
mod unix_local_ipc;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
mod windows_app_identity;
#[cfg(windows)]
mod windows_unity_window;
#[cfg(windows)]
mod windows_url_handler;

#[cfg(windows)]
pub use windows_app_identity::set_current_process_app_user_model_id;
#[cfg(windows)]
pub use windows_unity_window::{
    UnityWindowFocus, UnityWindowMonitor, UnityWindowReader, UnityWindowSnapshot, UnityWindowState,
    focus_unity_window, foreground_process_id,
};
#[cfg(windows)]
pub use windows_url_handler::unregister_owned_url_handler;

#[cfg(unix)]
pub use unix::{
    DaemonInstance, IpcListener, IpcStream, connect, endpoint_display, file_identity_key,
    state_database_path,
};
#[cfg(unix)]
pub use unix_local_ipc::{UnixIpcIdentity, connect_verified_local_socket};
#[cfg(windows)]
pub use windows::{
    CoreStartupGuard, DaemonInstance, IpcListener, IpcStream, WindowsFileIdentity, connect,
    connect_verified_local_pipe, endpoint_display, file_identity, file_link_count,
    local_app_data_directory, start_core_process, state_database_path, windows_current_account_sid,
};
#[cfg(windows)]
pub use windows::{
    WindowsElevatedProcess, WindowsMachineUpdateInput, WindowsMachineUpdateOutput,
    prepare_windows_machine_update, receive_windows_machine_update, start_elevated_windows_program,
    windows_process_is_elevated,
};

/// Verifies the Windows trust chain and the exact organization of an executable's signer.
#[cfg(windows)]
pub fn verify_windows_program_publisher(
    path: &std::path::Path,
    publisher: &str,
) -> std::io::Result<()> {
    windows::verify_program_publisher(path, publisher)
}

/// Locates the state database without creating or changing the data directory.
pub fn state_database_location(config: &DataConfig) -> std::io::Result<std::path::PathBuf> {
    #[cfg(unix)]
    let directory = unix::data_directory_path(config)?;
    #[cfg(windows)]
    let directory = windows::data_directory_path(config)?;
    if !directory.is_absolute()
        || directory.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "state data directory must be an absolute normalized path",
        ));
    }
    Ok(directory.join("state.db"))
}

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

    /// Returns the explicit isolated override without creating or reading any path.
    pub fn data_directory(&self) -> Option<&std::path::Path> {
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
