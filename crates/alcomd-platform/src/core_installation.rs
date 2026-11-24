//! Read-only location of the Core from its per-user installation registration.

use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

use crate::DataConfig;

const REGISTRATION_FILE: &str = "core-installation-v1";
const MAX_RUNTIME_PATH_BYTES: usize = 4096;

/// Basename of the Core executable from the product manifest.
#[cfg(unix)]
pub const CORE_EXECUTABLE_NAME: &str = env!("ALCOMD_DAEMON_NAME");
/// Basename of the Core executable from the product manifest.
#[cfg(windows)]
pub const CORE_EXECUTABLE_NAME: &str = concat!(env!("ALCOMD_DAEMON_NAME"), ".exe");

/// Locates the registration beside the per-user state database without creating a directory.
pub fn core_installation_registration(config: &DataConfig) -> io::Result<PathBuf> {
    #[cfg(unix)]
    use crate::unix::data_directory_path;
    #[cfg(windows)]
    use crate::windows::data_directory_path;

    let directory = data_directory_path(config)?;
    if !directory.is_absolute()
        || directory.file_name().is_none()
        || directory
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(invalid_registration());
    }
    Ok(directory.join(REGISTRATION_FILE))
}

/// Returns the installed Core path after checking the registration and executable.
///
/// An absent registration returns `None`. A registration pointing to a missing
/// or unsafe executable fails instead of falling back to another executable.
pub fn registered_core_executable(config: &DataConfig) -> io::Result<Option<PathBuf>> {
    let registration = core_installation_registration(config)?;
    let file = match open_registration(&registration) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let runtime = read_runtime_directory(&file)?;
    #[cfg(unix)]
    let runtime = crate::unix::validate_absolute_runtime_path(&runtime)?;
    let candidate = runtime.join(CORE_EXECUTABLE_NAME);
    validate_core_executable(&candidate).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            invalid_registration()
        } else {
            error
        }
    })?;
    Ok(Some(candidate))
}

fn read_runtime_directory(file: &std::fs::File) -> io::Result<PathBuf> {
    let mut data = Vec::new();
    file.take((MAX_RUNTIME_PATH_BYTES + 2) as u64)
        .read_to_end(&mut data)?;
    parse_runtime_directory(&data)
}

#[cfg(windows)]
#[path = "windows_installation_registration.rs"]
mod registration;
#[cfg(windows)]
pub use registration::{
    check_windows_core_registration, publish_windows_core_registration,
    remove_windows_core_registration, windows_installation_runtime,
};

fn parse_runtime_directory(data: &[u8]) -> io::Result<PathBuf> {
    let line = data
        .strip_suffix(b"\n")
        .filter(|line| !line.is_empty() && line.len() <= MAX_RUNTIME_PATH_BYTES)
        .and_then(|line| std::str::from_utf8(line).ok())
        .ok_or_else(invalid_registration)?;
    let path = Path::new(line);
    if line.chars().any(char::is_control)
        || !path.is_absolute()
        || path.file_name().is_none()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        || path.components().collect::<PathBuf>().as_os_str() != path.as_os_str()
    {
        return Err(invalid_registration());
    }
    Ok(path.to_path_buf())
}

fn invalid_registration() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid Core installation registration",
    )
}

#[cfg(windows)]
pub(crate) use crate::windows::core_installation::{open_registration, validate_core_executable};

#[cfg(unix)]
pub(crate) fn open_registration(path: &Path) -> io::Result<std::fs::File> {
    open_protected_file(path, false, false).map(std::fs::File::from)
}

#[cfg(unix)]
pub(crate) fn validate_core_executable(path: &Path) -> io::Result<()> {
    open_protected_file(path, true, true).map(drop)
}

#[cfg(unix)]
pub(crate) fn open_protected_file(
    path: &Path,
    executable: bool,
    root_owner_allowed: bool,
) -> io::Result<rustix::fd::OwnedFd> {
    use rustix::fs::{FileType, Mode, OFlags, fstat, openat};
    use rustix::process::geteuid;

    let directory = path.parent().ok_or_else(invalid_registration)?;
    let directory = crate::unix::validate_absolute_runtime_path(directory)?;
    let directory = crate::unix::open_protected_directory_chain(&directory)?;
    if fstat(&directory).map_err(io::Error::from)?.st_mode & 0o022 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "untrusted Core installation directory",
        ));
    }
    let name = path.file_name().ok_or_else(invalid_registration)?;
    let file = openat(
        &directory,
        name,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(io::Error::from)?;
    let stat = fstat(&file).map_err(io::Error::from)?;
    let uid = geteuid().as_raw();
    let owner_allowed = stat.st_uid == uid || root_owner_allowed && stat.st_uid == 0;
    let executable_mode = if stat.st_uid == uid { 0o100 } else { 0o001 };
    if !owner_allowed
        || FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
        || stat.st_nlink != 1
        || stat.st_mode & 0o022 != 0
        || executable && (stat.st_mode & executable_mode == 0 || stat.st_mode & 0o6000 != 0)
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "untrusted Core installation file",
        ));
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timestamp() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    }

    #[test]
    fn registration_requires_exactly_one_bounded_absolute_path() {
        #[cfg(unix)]
        let fixture = "/tmp/ALCOMD-Test/runtime\n";
        #[cfg(windows)]
        let fixture = "C:\\ALCOMD-Test\\runtime\n";
        assert_eq!(
            parse_runtime_directory(fixture.as_bytes()).expect("valid registration"),
            Path::new(fixture.trim_end_matches('\n'))
        );
        for fixture in [
            "".as_bytes(),
            "relative\n".as_bytes(),
            fixture.trim_end_matches('\n').as_bytes(),
            b"\xff\n",
        ] {
            assert!(parse_runtime_directory(fixture).is_err());
        }
        assert!(parse_runtime_directory(format!("{fixture}/another\n").as_bytes()).is_err());
        assert!(parse_runtime_directory(format!("{fixture}\r\n").as_bytes()).is_err());
        let oversized = format!("/{}\n", "p".repeat(MAX_RUNTIME_PATH_BYTES));
        assert!(parse_runtime_directory(oversized.as_bytes()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn lookup_does_not_create_registration_and_rejects_writable_or_linked_runtime() {
        use std::fs;
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt, symlink};

        let root = std::env::temp_dir().join(format!(
            "alcomd-sdk-install-{}-{}",
            std::process::id(),
            timestamp()
        ));
        let root = crate::unix::validate_absolute_runtime_path(&root).expect("canonical temp root");
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .expect("fixture");
        let data = root.join("data");
        let runtime = root.join("runtime");
        let config = DataConfig::isolated(data.clone());
        assert_eq!(
            registered_core_executable(&config).expect("absent data"),
            None
        );
        assert!(!data.exists());

        fs::create_dir(&data).expect("fixture data");
        fs::create_dir(&runtime).expect("fixture runtime");
        let registration = core_installation_registration(&config).expect("registration path");
        fs::write(&registration, format!("{}\n", runtime.display())).expect("stage registration");
        assert_eq!(
            registered_core_executable(&config)
                .expect_err("stale registration")
                .kind(),
            io::ErrorKind::InvalidData
        );
        let executable = runtime.join(CORE_EXECUTABLE_NAME);
        fs::write(&executable, "test").expect("stage executable");
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).expect("executable");
        assert_eq!(
            registered_core_executable(&config).expect("installed"),
            Some(executable.clone())
        );

        fs::set_permissions(&runtime, fs::Permissions::from_mode(0o777)).expect("unsafe runtime");
        assert_eq!(
            registered_core_executable(&config)
                .expect_err("writable runtime")
                .kind(),
            io::ErrorKind::PermissionDenied
        );
        fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).expect("safe runtime");
        let alias = root.join("alias");
        symlink(&runtime, &alias).expect("symlink fixture");
        fs::write(&registration, format!("{}\n", alias.display())).expect("stage link");
        assert!(registered_core_executable(&config).is_err());

        fs::remove_file(alias).expect("remove link");
        fs::remove_file(executable).expect("remove executable");
        fs::remove_dir(runtime).expect("remove runtime");
        fs::remove_file(registration).expect("remove registration");
        fs::remove_dir(data).expect("remove data");
        fs::remove_dir(root).expect("remove root");
    }

    #[cfg(windows)]
    #[test]
    fn staged_windows_registration_is_read_only_and_rejects_multiply_linked_files() {
        use std::fs;

        let root = crate::local_app_data_directory()
            .expect("native profile")
            .join(format!(
                "alcomd-sdk-install-{}-{}",
                std::process::id(),
                timestamp()
            ));
        let data = root.join("data");
        let runtime = root.join("runtime");
        let config = DataConfig::isolated(data.clone());
        assert_eq!(
            registered_core_executable(&config).expect("absent data"),
            None
        );
        assert!(!root.exists());
        fs::create_dir_all(&data).expect("stage user data");
        fs::create_dir(&runtime).expect("stage runtime");
        let registration = core_installation_registration(&config).expect("registration path");
        fs::write(&registration, format!("{}\n", runtime.display())).expect("stage registration");
        assert_eq!(
            registered_core_executable(&config)
                .expect_err("stale registration")
                .kind(),
            io::ErrorKind::InvalidData
        );
        let executable = runtime.join(CORE_EXECUTABLE_NAME);
        fs::write(&executable, "test").expect("stage candidate");
        assert_eq!(
            registered_core_executable(&config).expect("verified registration"),
            Some(executable.clone())
        );
        let link = runtime.join("link");
        fs::hard_link(&executable, &link).expect("test hard link");
        assert_eq!(
            registered_core_executable(&config)
                .expect_err("linked executable")
                .kind(),
            io::ErrorKind::PermissionDenied
        );
        fs::remove_file(link).expect("remove hard link");
        fs::remove_file(executable).expect("remove fixture");
        fs::remove_dir(runtime).expect("remove runtime");
        fs::remove_file(registration).expect("remove registration");
        fs::remove_dir(data).expect("remove data");
        fs::remove_dir(root).expect("remove root");
    }

    #[cfg(windows)]
    #[test]
    fn windows_install_registration_is_idempotent_and_preserves_other_owners_and_data() {
        use std::fs;

        let root = crate::local_app_data_directory()
            .expect("native profile")
            .join(format!(
                "alcomd-install-registration-{}-{}",
                std::process::id(),
                timestamp()
            ));
        let data = root.join("data");
        let pending = root.join("runtime").join(env!("CARGO_PKG_VERSION"));
        let other_runtime = root.join("other");
        for directory in [&data, &pending, &other_runtime] {
            fs::create_dir_all(directory).expect("isolated directory");
        }
        for runtime in [&pending, &other_runtime] {
            fs::write(runtime.join(CORE_EXECUTABLE_NAME), "test executable").expect("fixture");
        }
        let preserved = data.join("state.db");
        fs::write(&preserved, "synthetic state").expect("fixture");
        let runtime = windows_installation_runtime(&pending.join(CORE_EXECUTABLE_NAME))
            .expect("trusted versioned runtime");
        let other_runtime = fs::canonicalize(other_runtime).expect("other canonical path");
        let config = DataConfig::isolated(data);
        let registration = core_installation_registration(&config).expect("registration path");

        assert!(!check_windows_core_registration(&config, &runtime).expect("absent pointer"));
        publish_windows_core_registration(&config, &runtime).expect("publish new pointer");
        let identity = crate::file_identity(&registration).expect("published file identity");
        publish_windows_core_registration(&config, &runtime).expect("repeat publication");
        assert_eq!(
            crate::file_identity(&registration).expect("same identity"),
            identity
        );
        assert_eq!(
            registered_core_executable(&config).expect("launchable registration"),
            Some(runtime.join(CORE_EXECUTABLE_NAME))
        );
        assert!(publish_windows_core_registration(&config, &other_runtime).is_err());
        assert!(remove_windows_core_registration(&config, &other_runtime).is_err());
        assert_eq!(
            crate::file_identity(&registration).expect("still present"),
            identity
        );
        let alias = root.join("registration-link");
        fs::hard_link(&registration, &alias).expect("linked registration");
        assert!(remove_windows_core_registration(&config, &runtime).is_err());
        assert!(registration.exists() && alias.exists());
        fs::remove_file(alias).expect("remove isolated fixture link");
        remove_windows_core_registration(&config, &runtime).expect("unregister matching pointer");
        remove_windows_core_registration(&config, &runtime).expect("repeat removal");
        assert!(!registration.exists());
        assert_eq!(
            fs::read_to_string(preserved).expect("data survives"),
            "synthetic state"
        );
        fs::remove_dir_all(root).expect("remove isolated fixture");
    }
}
