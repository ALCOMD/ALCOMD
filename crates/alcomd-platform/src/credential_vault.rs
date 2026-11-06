//! OS-backed storage for bounded opaque credentials.
use std::io;

#[cfg(target_os = "linux")]
#[path = "credential_vault/linux.rs"]
mod platform;
#[cfg(target_os = "macos")]
#[path = "credential_vault/macos.rs"]
mod platform;
#[cfg(windows)]
#[path = "credential_vault/windows.rs"]
mod platform;

const SERVICE: &str = "com.vrclearn.alcomd.vpm";
const MAX_BYTES: usize = 2560;

fn validate(reference: &str) -> io::Result<()> {
    if reference.len() == 36
        && reference
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
    {
        Ok(())
    } else {
        Err(io::ErrorKind::InvalidInput.into())
    }
}

pub fn write_credential(reference: &str, value: &[u8]) -> io::Result<()> {
    validate(reference)?;
    if value.is_empty() || value.len() > MAX_BYTES {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    platform::write(reference, value)
}

pub fn read_credential(reference: &str) -> io::Result<Vec<u8>> {
    validate(reference)?;
    let value = platform::read(reference)?;
    if value.is_empty() || value.len() > MAX_BYTES {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(value)
}

pub fn delete_credential(reference: &str) -> io::Result<()> {
    validate(reference)?;
    platform::delete(reference)
}
