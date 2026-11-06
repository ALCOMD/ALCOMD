//! Credential Manager's generic per-user credential API.
#![allow(unsafe_code)]

use std::{io, ptr, slice};
use windows_sys::Win32::Foundation::ERROR_NOT_FOUND;
use windows_sys::Win32::Security::Credentials::{
    CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredDeleteW, CredFree, CredReadW,
    CredWriteW,
};

fn target(reference: &str) -> Vec<u16> {
    format!("{}:{reference}", super::SERVICE)
        .encode_utf16()
        .chain([0])
        .collect()
}

pub(super) fn write(reference: &str, value: &[u8]) -> io::Result<()> {
    let mut target = target(reference);
    let mut blob = value.to_vec();
    let credential = CREDENTIALW {
        Type: CRED_TYPE_GENERIC,
        TargetName: target.as_mut_ptr(),
        CredentialBlobSize: u32::try_from(blob.len()).map_err(|_| io::ErrorKind::InvalidInput)?,
        CredentialBlob: blob.as_mut_ptr(),
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        ..Default::default()
    };
    // SAFETY: The NUL-terminated target and bounded blob remain alive for the synchronous call.
    if unsafe { CredWriteW(&credential, 0) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub(super) fn read(reference: &str) -> io::Result<Vec<u8>> {
    let target = target(reference);
    let mut entry = ptr::null_mut();
    // SAFETY: The target is NUL-terminated and the out pointer is valid for one pointer.
    if unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut entry) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if entry.is_null() {
        return Err(io::ErrorKind::InvalidData.into());
    }
    // SAFETY: On success CredReadW owns one CREDENTIALW and its embedded byte buffer until CredFree.
    unsafe {
        let size = (*entry).CredentialBlobSize as usize;
        let blob = (*entry).CredentialBlob;
        let result = if size == 0 || size > super::MAX_BYTES || blob.is_null() {
            Err(io::ErrorKind::InvalidData.into())
        } else {
            Ok(slice::from_raw_parts(blob, size).to_vec())
        };
        CredFree(entry.cast());
        result
    }
}

pub(super) fn delete(reference: &str) -> io::Result<()> {
    let target = target(reference);
    // SAFETY: The target is a live NUL-terminated UTF-16 buffer.
    if unsafe { CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0) } == 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(ERROR_NOT_FOUND as i32) {
            return Err(error);
        }
    }
    Ok(())
}
