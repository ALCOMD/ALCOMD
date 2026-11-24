use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::{
    CORE_EXECUTABLE_NAME, core_installation_registration, invalid_registration, open_registration,
    parse_runtime_directory, read_runtime_directory, validate_core_executable,
};
use crate::windows::core_installation::{
    open_registration_for_removal, validate_registration_parent,
};
use crate::windows::{delete_open_file, file_identity_from_file};
use crate::{DataConfig, publish_path_no_replace, resolve_plain_storage_directory, sync_directory};

/// Resolves a member of the current versioned Windows runtime through trusted paths.
pub fn windows_installation_runtime(executable: &Path) -> io::Result<PathBuf> {
    let parent = executable.parent().ok_or_else(invalid_registration)?;
    let (runtime, _) = resolve_plain_storage_directory(parent)?;
    if !is_versioned_runtime(&runtime) {
        return Err(invalid_registration());
    }
    validate_core_executable(&runtime.join(CORE_EXECUTABLE_NAME))?;
    Ok(runtime)
}

/// Finds Bootstrap for the official GUI or stable CLI in a protected Windows installation.
pub fn windows_installation_bootstrap(entry_point: &Path) -> io::Result<Option<PathBuf>> {
    let Some(directory) = entry_point.parent() else {
        return Ok(None);
    };
    let core = if has_name(entry_point, concat!(env!("ALCOMD_GUI_NAME"), ".exe"))
        && is_versioned_runtime(directory)
    {
        directory.join(CORE_EXECUTABLE_NAME)
    } else if has_name(entry_point, concat!(env!("ALCOMD_CLI_NAME"), ".exe"))
        && has_name(directory, "bin")
        && let Some(root) = directory.parent()
    {
        let versions = root.join("runtime");
        match fs::symlink_metadata(&versions) {
            Ok(_) => versions
                .join(env!("CARGO_PKG_VERSION"))
                .join(CORE_EXECUTABLE_NAME),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        }
    } else {
        return Ok(None);
    };
    // Validate the unnormalized paths so a reparse point cannot redirect either role.
    validate_core_executable(entry_point)?;
    validate_core_executable(&core)?;
    let runtime = windows_installation_runtime(&core)?;
    let bootstrap = runtime.join(concat!(env!("ALCOMD_BOOTSTRAP_NAME"), ".exe"));
    validate_core_executable(&bootstrap)?;
    Ok(Some(bootstrap))
}

fn is_versioned_runtime(directory: &Path) -> bool {
    has_name(directory, env!("CARGO_PKG_VERSION"))
        && directory
            .parent()
            .is_some_and(|parent| has_name(parent, "runtime"))
}

fn has_name(path: &Path, name: &str) -> bool {
    path.file_name()
        .and_then(|part| part.to_str())
        .is_some_and(|part| part.eq_ignore_ascii_case(name))
}

/// Checks whether the selected runtime already owns this user's registration.
/// A different, stale, or malformed registration blocks an ordinary install.
pub fn check_windows_core_registration(config: &DataConfig, runtime: &Path) -> io::Result<bool> {
    let registration = core_installation_registration(config)?;
    match open_registration(&registration) {
        Ok(file) => {
            require_owner(&file, runtime)?;
            Ok(true)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

/// Publishes a registration after Core has completed startup checks under its instance lock.
pub fn publish_windows_core_registration(config: &DataConfig, runtime: &Path) -> io::Result<()> {
    validate_core_executable(&runtime.join(CORE_EXECUTABLE_NAME))?;
    let registration = core_installation_registration(config)?;
    validate_registration_parent(&registration)?;
    if check_windows_core_registration(config, runtime)? {
        return Ok(());
    }

    let line = format!("{}\n", runtime.to_str().ok_or_else(invalid_registration)?);
    parse_runtime_directory(line.as_bytes())?;
    let parent = registration.parent().ok_or_else(invalid_registration)?;
    let (_, parent_identity) = resolve_plain_storage_directory(parent)?;
    static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let temporary = registration.with_extension(format!(
        "new-{}-{stamp}-{}",
        std::process::id(),
        NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let identity = file_identity_from_file(&file)?;
    let result = (|| {
        file.write_all(line.as_bytes())?;
        file.sync_all()?;
        drop(file);
        let checked = open_registration(&temporary)?;
        require_owner(&checked, runtime)?;
        if file_identity_from_file(&checked)? != identity {
            return Err(invalid_registration());
        }
        drop(checked);
        if resolve_plain_storage_directory(parent)?.1 != parent_identity {
            return Err(invalid_registration());
        }
        publish_path_no_replace(&temporary, &registration)?;
        sync_directory(parent)
    })();
    if result.is_err()
        && let Ok(file) = open_registration_for_removal(&temporary)
        && file_identity_from_file(&file).is_ok_and(|current| current == identity)
    {
        let _ = delete_open_file(file);
    }
    result
}

/// Removes only the selected runtime's registration, using the validated file handle.
pub fn remove_windows_core_registration(config: &DataConfig, runtime: &Path) -> io::Result<()> {
    let registration = core_installation_registration(config)?;
    let file = match open_registration_for_removal(&registration) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    require_owner(&file, runtime)?;
    delete_open_file(file)?;
    sync_directory(registration.parent().ok_or_else(invalid_registration)?)
}

fn require_owner(file: &File, runtime: &Path) -> io::Result<()> {
    let registered = read_runtime_directory(file)?;
    validate_core_executable(&registered.join(CORE_EXECUTABLE_NAME))?;
    let registered = fs::canonicalize(registered)?;
    if registered != runtime {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "Core registration belongs to another installed runtime",
        ));
    }
    Ok(())
}
