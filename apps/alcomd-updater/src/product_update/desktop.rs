use std::fs;
use std::io;
use std::path::PathBuf;

use alcomd_platform::{
    DataConfig, open_owned_installation_record, state_database_location, sync_directory,
    validate_installation_record_parent,
};

const DESKTOP_FAILURE: &str = "desktop-update-failed-v1";

/// Finds the per-user update directory without creating or changing anything.
pub fn desktop_update_storage_location() -> io::Result<PathBuf> {
    let state = state_database_location(&DataConfig::default())?;
    let data = state
        .parent()
        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
    Ok(data.join("product-updates"))
}

/// Creates or reopens the protected directory used by the desktop's signed download.
pub fn prepare_desktop_update_storage() -> io::Result<PathBuf> {
    let directory = desktop_update_storage_location()?;
    let data = directory
        .parent()
        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
    validate_installation_record_parent(&directory)?;
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.recursive(false);
    match builder.create(&directory) {
        Ok(()) => sync_directory(data)?,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    validate_installation_record_parent(&directory.join(DESKTOP_FAILURE))?;
    Ok(directory)
}

/// Reads the advisory outcome from the previous desktop-initiated restart.
pub fn desktop_update_failed() -> io::Result<bool> {
    let path = desktop_update_storage_location()?.join(DESKTOP_FAILURE);
    let file = match open_owned_installation_record(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if file.metadata()?.len() != 0 {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(true)
}

/// Writes or clears only the desktop outcome. The signed platform journal controls recovery.
pub fn record_desktop_update_failure(failed: bool) -> io::Result<()> {
    if desktop_update_failed()? == failed {
        return Ok(());
    }
    let directory = prepare_desktop_update_storage()?;
    let path = directory.join(DESKTOP_FAILURE);
    if failed {
        let mut options = fs::OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        options.open(&path)?.sync_all()?;
    } else {
        fs::remove_file(&path)?;
    }
    sync_directory(&directory)
}
