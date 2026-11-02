//! One Win32 move without replacement; the standard rename API can replace files.
#![allow(unsafe_code)]

use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

pub(crate) fn publish_file_no_replace(source: &Path, destination: &Path) -> io::Result<()> {
    let wide = |path: &Path| -> io::Result<Vec<u16>> {
        let mut value: Vec<u16> = path.as_os_str().encode_wide().collect();
        if value.contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "path contains NUL",
            ));
        }
        value.push(0);
        Ok(value)
    };
    let source = wide(source)?;
    let destination = wide(destination)?;
    // SAFETY: both buffers are owned, NUL-terminated UTF-16 and live for the call.
    // Flags 0 deliberately omit MOVEFILE_REPLACE_EXISTING and COPY_ALLOWED:
    // an existing target or a cross-volume move fails without replacing it.
    if unsafe {
        windows_sys::Win32::Storage::FileSystem::MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            0,
        )
    } == 0
    {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
