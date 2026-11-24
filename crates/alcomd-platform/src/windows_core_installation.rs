//! Per-user Core launch paths backed by local files with trusted DACLs.
use std::fs::{File, OpenOptions};
use std::io;
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::os::windows::io::AsRawHandle;
use std::path::{Component, Path, PathBuf, Prefix};

use windows_sys::Win32::Foundation::{GENERIC_ALL, GENERIC_WRITE};
use windows_sys::Win32::Storage::FileSystem::{
    DELETE, FILE_APPEND_DATA, FILE_ATTRIBUTE_REPARSE_POINT, FILE_DELETE_CHILD,
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES, FILE_READ_DATA,
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_WRITE_ATTRIBUTES, FILE_WRITE_DATA,
    FILE_WRITE_EA, READ_CONTROL, WRITE_DAC, WRITE_OWNER,
};

use super::file_link_count;
use super::security::{current_user_sid_string, is_fixed_drive, validate_installation_dacl};

pub(crate) fn open_registration(path: &Path) -> io::Result<File> {
    open_checked_file(path, FILE_READ_DATA)
}

pub(crate) fn open_registration_for_removal(path: &Path) -> io::Result<File> {
    open_checked_file(path, FILE_READ_DATA | DELETE)
}

pub(crate) fn validate_core_executable(path: &Path) -> io::Result<()> {
    open_checked_file(path, 0).map(drop)
}

const REPLACE_ACCESS: u32 =
    DELETE | FILE_DELETE_CHILD | WRITE_DAC | WRITE_OWNER | GENERIC_WRITE | GENERIC_ALL;
const MODIFY_ACCESS: u32 =
    REPLACE_ACCESS | FILE_WRITE_DATA | FILE_APPEND_DATA | FILE_WRITE_ATTRIBUTES | FILE_WRITE_EA;

pub(crate) fn validate_registration_parent(path: &Path) -> io::Result<()> {
    let root = local_drive_root(path)?;
    let mut directory = root;
    let directories = path.parent().ok_or_else(untrusted)?;
    let current_sid = current_user_sid_string()?;

    // Other users may create new folders in some volume roots. No ancestor
    // may allow them to replace the existing path being followed.
    let ancestors = directories.components().skip(2);
    check_directory(&directory, &current_sid, REPLACE_ACCESS, true)?;
    for component in ancestors {
        let Component::Normal(name) = component else {
            return Err(untrusted());
        };
        directory.push(name);
        let mask = if directory == directories {
            MODIFY_ACCESS
        } else {
            REPLACE_ACCESS
        };
        check_directory(&directory, &current_sid, mask, false)?;
    }

    Ok(())
}

fn open_checked_file(path: &Path, access: u32) -> io::Result<File> {
    validate_registration_parent(path)?;
    let current_sid = current_user_sid_string()?;
    let file = OpenOptions::new()
        .access_mode(READ_CONTROL | FILE_READ_ATTRIBUTES | access)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || file_link_count(path)? != 1
    {
        return Err(untrusted());
    }
    validate_installation_dacl(file.as_raw_handle().cast(), &current_sid, MODIFY_ACCESS)?;
    Ok(file)
}

fn check_directory(path: &Path, current_sid: &str, mask: u32, drive_root: bool) -> io::Result<()> {
    let directory = OpenOptions::new()
        .access_mode(READ_CONTROL | FILE_READ_ATTRIBUTES)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    let metadata = directory.metadata()?;
    if !metadata.is_dir() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(untrusted());
    }
    let mask = if drive_root {
        if !super::is_volume_root(&directory)? {
            return Err(untrusted());
        }
        // A volume root has no parent entry to delete. FILE_DELETE_CHILD still
        // governs the ability to replace the first component of our path.
        mask & !DELETE
    } else {
        mask
    };
    validate_installation_dacl(directory.as_raw_handle().cast(), current_sid, mask)
}

fn local_drive_root(path: &Path) -> io::Result<PathBuf> {
    let mut components = path.components();
    let prefix = match components.next() {
        Some(Component::Prefix(prefix))
            if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)) =>
        {
            prefix
        }
        _ => return Err(untrusted()),
    };
    if components.next() != Some(Component::RootDir)
        || components.any(|component| {
            !matches!(component, Component::Normal(name) if name.to_str().is_some_and(|name|
                !name.contains(':') && !name.ends_with(['.', ' '])
            ))
        })
    {
        return Err(untrusted());
    }
    let mut root = prefix.as_os_str().to_os_string();
    root.push("\\");
    let root = PathBuf::from(root);
    if !is_fixed_drive(root.as_os_str()) {
        return Err(untrusted());
    }
    Ok(root)
}

fn untrusted() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "untrusted Core installation path",
    )
}
