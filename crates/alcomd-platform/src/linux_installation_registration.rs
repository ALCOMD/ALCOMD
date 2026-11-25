use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use rustix::fs::{
    AtFlags, FileType, Mode, OFlags, RenameFlags, fstat, fsync, openat, renameat_with, unlinkat,
};
use rustix::process::geteuid;

use super::{
    CORE_EXECUTABLE_NAME, core_installation_registration, invalid_registration, open_registration,
    parse_runtime_directory, read_runtime_directory, validate_core_executable,
};
use crate::DataConfig;

fn package_runtime() -> PathBuf {
    Path::new("/usr/lib")
        .join(CORE_EXECUTABLE_NAME)
        .join("runtime")
        .join(env!("CARGO_PKG_VERSION"))
}

fn require_system_executable(path: &Path) -> io::Result<()> {
    let parent = path.parent().ok_or_else(invalid_registration)?;
    let directory = crate::unix::open_system_directory_chain(parent)?;
    let file = openat(
        &directory,
        path.file_name().ok_or_else(invalid_registration)?,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(io::Error::from)?;
    let stat = fstat(&file).map_err(io::Error::from)?;
    if stat.st_uid != 0
        || stat.st_nlink != 1
        || stat.st_mode & 0o6022 != 0
        || stat.st_mode & 0o001 == 0
        || FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "untrusted system installation executable",
        ));
    }
    Ok(())
}

/// Resolves the versioned runtime of the Linux system package.
pub fn linux_installation_runtime(executable: &Path) -> io::Result<PathBuf> {
    let runtime = package_runtime();
    if executable.parent() != Some(runtime.as_path()) {
        return Err(invalid_registration());
    }
    require_system_executable(executable)?;
    require_system_executable(&runtime.join(CORE_EXECUTABLE_NAME))?;
    Ok(runtime)
}

/// Finds Bootstrap from the protected CLI or GUI installed by the Linux system package.
pub fn linux_installation_bootstrap(entry_point: &Path) -> io::Result<Option<PathBuf>> {
    let runtime = package_runtime();
    if entry_point != Path::new("/usr/bin").join(env!("ALCOMD_CLI_NAME"))
        && entry_point != runtime.join(env!("ALCOMD_GUI_NAME"))
    {
        return Ok(None);
    }
    require_system_executable(entry_point)?;
    let bootstrap = runtime.join(env!("ALCOMD_BOOTSTRAP_NAME"));
    linux_installation_runtime(&bootstrap)?;
    Ok(Some(bootstrap))
}

/// Checks whether the selected runtime already owns this user's registration.
pub fn check_linux_core_registration(config: &DataConfig, runtime: &Path) -> io::Result<bool> {
    match open_registration(&core_installation_registration(config)?) {
        Ok(file) => {
            if read_runtime_directory(&file)? != runtime {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "Core registration belongs to another installed runtime",
                ));
            }
            validate_core_executable(&runtime.join(CORE_EXECUTABLE_NAME))?;
            Ok(true)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

/// Publishes the checked runtime after Core initializes under its instance lock.
pub fn publish_linux_core_registration(config: &DataConfig, runtime: &Path) -> io::Result<()> {
    linux_installation_runtime(&runtime.join(CORE_EXECUTABLE_NAME))?;
    if check_linux_core_registration(config, runtime)? {
        return Ok(());
    }
    let registration = core_installation_registration(config)?;
    let parent = registration.parent().ok_or_else(invalid_registration)?;
    let name = registration.file_name().ok_or_else(invalid_registration)?;
    let directory = crate::unix::open_protected_directory_chain(parent)?;
    let stat = fstat(&directory).map_err(io::Error::from)?;
    if stat.st_uid != geteuid().as_raw() || stat.st_mode & 0o777 != 0o700 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Core registration directory is not private",
        ));
    }

    let line = format!("{}\n", runtime.to_str().ok_or_else(invalid_registration)?);
    parse_runtime_directory(line.as_bytes())?;
    static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let temporary = format!(
        ".core-installation-{}-{stamp}-{}",
        std::process::id(),
        NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed)
    );
    let mut file = File::from(
        openat(
            &directory,
            temporary.as_str(),
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )
        .map_err(io::Error::from)?,
    );
    let result = (|| {
        file.write_all(line.as_bytes())?;
        file.sync_all()?;
        renameat_with(
            &directory,
            temporary.as_str(),
            &directory,
            name,
            RenameFlags::NOREPLACE,
        )
        .map_err(io::Error::from)?;
        fsync(&directory).map_err(io::Error::from)
    })();
    if result.is_err() {
        let _ = unlinkat(&directory, temporary.as_str(), AtFlags::empty());
    }
    result
}
