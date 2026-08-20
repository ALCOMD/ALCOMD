//! Private Windows FFI for authoritative filesystem-object identity.

#![allow(unsafe_code)]

use std::ffi::{OsStr, c_void};
use std::io;
use std::mem::{MaybeUninit, size_of};
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_ID_INFO, FILE_SHARE_DELETE, FILE_SHARE_READ,
    FILE_SHARE_WRITE, FILE_STANDARD_INFO, FileIdInfo, FileStandardInfo,
    GetFileInformationByHandleEx, OPEN_EXISTING,
};

/// Authoritative identity of one filesystem object on the current Windows machine.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WindowsFileIdentity {
    volume_serial_number: u64,
    file_id: [u8; 16],
}

impl WindowsFileIdentity {
    /// Stable M3 encoding: 8-byte little-endian volume serial followed by 16-byte file ID.
    #[must_use]
    pub fn to_key(self) -> [u8; 24] {
        let mut key = [0_u8; 24];
        key[..8].copy_from_slice(&self.volume_serial_number.to_le_bytes());
        key[8..].copy_from_slice(&self.file_id);
        key
    }
}

struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: this guard is created only for a successful CreateFileW result,
        // owns that HANDLE exclusively, exposes no raw access, and closes it once.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// Returns the final target object's volume serial and 128-bit file ID.
pub fn file_identity(path: &Path) -> io::Result<WindowsFileIdentity> {
    let handle = open_object(path)?;
    let mut information = MaybeUninit::<FILE_ID_INFO>::zeroed();
    // SAFETY: `handle` is a valid owned HANDLE. `information` is writable,
    // correctly aligned storage exactly `size_of::<FILE_ID_INFO>()` bytes long;
    // GetFileInformationByHandleEx initializes it on a nonzero return.
    let succeeded = unsafe {
        GetFileInformationByHandleEx(
            handle.0,
            FileIdInfo,
            information.as_mut_ptr().cast::<c_void>(),
            size_of::<FILE_ID_INFO>() as u32,
        )
    };
    if succeeded == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the successful call above initialized the complete FILE_ID_INFO
    // output buffer. Its Win32 representation is copied immediately into pure
    // Rust integers/bytes and no FILE_ID_INFO or pointer escapes this module.
    let information = unsafe { information.assume_init() };
    Ok(WindowsFileIdentity {
        volume_serial_number: information.VolumeSerialNumber,
        file_id: information.FileId.Identifier,
    })
}

/// Returns the authoritative hard-link count for one final target filesystem object.
pub fn file_link_count(path: &Path) -> io::Result<u32> {
    let handle = open_object(path)?;
    let mut information = MaybeUninit::<FILE_STANDARD_INFO>::zeroed();
    // SAFETY: `handle` is a valid exclusively owned HANDLE and `information` is writable,
    // correctly aligned storage exactly `size_of::<FILE_STANDARD_INFO>()` bytes long.
    // FileStandardInfo initializes the complete output on a nonzero return.
    let succeeded = unsafe {
        GetFileInformationByHandleEx(
            handle.0,
            FileStandardInfo,
            information.as_mut_ptr().cast::<c_void>(),
            size_of::<FILE_STANDARD_INFO>() as u32,
        )
    };
    if succeeded == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the successful FileStandardInfo query initialized the complete
    // FILE_STANDARD_INFO buffer. NumberOfLinks is copied into a pure Rust u32;
    // neither the Win32 structure nor the owned HANDLE escapes this module.
    let information = unsafe { information.assume_init() };
    Ok(information.NumberOfLinks)
}

fn open_object(path: &Path) -> io::Result<OwnedHandle> {
    let wide_path = nul_terminated(path.as_os_str())?;
    // SAFETY: `wide_path` is NUL-terminated and remains alive for the call. All
    // other pointers are null as required. OPEN_REPARSE_POINT is intentionally
    // absent, so CreateFileW resolves a root symlink/junction to its final target.
    let raw_handle = unsafe {
        CreateFileW(
            wide_path.as_ptr(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            null_mut(),
        )
    };
    if raw_handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    Ok(OwnedHandle(raw_handle))
}

fn nul_terminated(path: &OsStr) -> io::Result<Vec<u16>> {
    let mut wide = path.encode_wide().collect::<Vec<_>>();
    if wide.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "filesystem path contains an embedded NUL",
        ));
    }
    wide.push(0);
    Ok(wide)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Arc;
    use std::thread;

    use super::*;

    fn temporary_directory(label: &str) -> std::path::PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "alcomd-file-identity-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        fs::create_dir(&directory).expect("create temporary directory");
        directory
    }

    #[test]
    fn key_encoding_is_fixed_24_byte_little_endian() {
        let identity = WindowsFileIdentity {
            volume_serial_number: 0x0102_0304_0506_0708,
            file_id: [0xA5; 16],
        };
        assert_eq!(
            identity.to_key(),
            [
                0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, 0xA5, 0xA5, 0xA5, 0xA5, 0xA5, 0xA5,
                0xA5, 0xA5, 0xA5, 0xA5, 0xA5, 0xA5, 0xA5, 0xA5, 0xA5, 0xA5,
            ]
        );
    }

    #[test]
    fn identity_tracks_object_across_spelling_symlink_and_rename() {
        let root = temporary_directory("object");
        let target = root.join("TargetDirectory");
        let other = root.join("OtherDirectory");
        let link = root.join("TargetLink");
        fs::create_dir(&target).expect("create target");
        fs::create_dir(&other).expect("create other");
        std::os::windows::fs::symlink_dir(&target, &link).expect("create directory symlink");

        let expected = file_identity(&target).expect("target identity");
        assert_eq!(expected, file_identity(&target).expect("repeat identity"));
        assert_eq!(
            expected,
            file_identity(&std::path::PathBuf::from(
                target.as_os_str().to_string_lossy().to_uppercase()
            ))
            .expect("case-insensitive identity")
        );
        assert_eq!(
            expected,
            file_identity(&link).expect("symlink target identity")
        );
        assert_ne!(expected, file_identity(&other).expect("other identity"));

        let renamed = root.join("RenamedDirectory");
        fs::rename(&target, &renamed).expect("rename target");
        assert_eq!(expected, file_identity(&renamed).expect("renamed identity"));

        fs::remove_dir(&link).expect("remove symlink");
        fs::remove_dir(&renamed).expect("remove renamed target");
        fs::create_dir(&target).expect("recreate path");
        let _fresh_identity = file_identity(&target).expect("fresh identity query");
        fs::remove_dir_all(&root).expect("remove temporary tree");
    }

    #[test]
    fn parallel_short_queries_do_not_block_rename_or_leak_handles() {
        let root = temporary_directory("parallel");
        let target = Arc::new(root.join("Target"));
        fs::create_dir(target.as_ref()).expect("create target");
        let expected = file_identity(target.as_ref()).expect("initial identity");
        let mut workers = Vec::new();
        for _ in 0..8 {
            let target = Arc::clone(&target);
            workers.push(thread::spawn(move || {
                for _ in 0..128 {
                    assert_eq!(
                        expected,
                        file_identity(target.as_ref()).expect("parallel identity")
                    );
                }
            }));
        }
        for worker in workers {
            worker.join().expect("identity worker");
        }
        let renamed = root.join("Renamed");
        fs::rename(target.as_ref(), &renamed).expect("rename after all handles close");
        fs::remove_dir_all(&root).expect("remove temporary tree");
    }

    #[test]
    fn missing_target_is_an_io_error() {
        let root = temporary_directory("missing");
        let missing = root.join("missing");
        assert_eq!(
            file_identity(&missing)
                .expect_err("missing identity")
                .kind(),
            io::ErrorKind::NotFound
        );
        fs::remove_dir(&root).expect("remove temporary directory");
    }

    #[test]
    fn link_count_is_one_for_an_ordinary_file_and_handles_are_released() {
        let root = temporary_directory("link-count-one");
        let file = root.join("ordinary.txt");
        let directory = root.join("ordinary-directory");
        fs::write(&file, b"ordinary").expect("write ordinary file");
        fs::create_dir(&directory).expect("create ordinary directory");
        for _ in 0..256 {
            assert_eq!(file_link_count(&file).expect("query link count"), 1);
            let _ = file_link_count(&directory).expect("query directory link count");
        }
        let renamed = root.join("renamed.txt");
        let renamed_directory = root.join("renamed-directory");
        fs::rename(&file, &renamed).expect("rename after link-count queries");
        fs::rename(&directory, &renamed_directory)
            .expect("rename directory after link-count queries");
        fs::remove_file(&renamed).expect("remove after link-count queries");
        fs::remove_dir(&renamed_directory).expect("remove directory after link-count queries");
        fs::remove_dir(&root).expect("remove fixture root");
    }

    #[test]
    fn link_count_detects_links_inside_and_outside_a_project() {
        let root = temporary_directory("hard-links");
        let project = root.join("Project");
        fs::create_dir(&project).expect("create project");
        let source = project.join("source.txt");
        let inside = project.join("inside.txt");
        let outside = root.join("outside.txt");
        fs::write(&source, b"linked").expect("write source");
        fs::hard_link(&source, &inside).expect("create inside hard link");
        assert_eq!(file_link_count(&source).expect("two links"), 2);
        fs::hard_link(&source, &outside).expect("create outside hard link");
        assert_eq!(file_link_count(&source).expect("three links"), 3);
        fs::remove_dir_all(&root).expect("remove hard-link fixture");
    }

    #[test]
    fn link_count_query_failure_is_an_io_error() {
        let root = temporary_directory("link-count-missing");
        let missing = root.join("missing.txt");
        assert_eq!(
            file_link_count(&missing)
                .expect_err("missing link count")
                .kind(),
            io::ErrorKind::NotFound
        );
        fs::remove_dir(&root).expect("remove fixture root");
    }
}
