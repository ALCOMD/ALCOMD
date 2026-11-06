use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password,
};
use std::io;

pub(super) fn write(reference: &str, value: &[u8]) -> io::Result<()> {
    set_generic_password(super::SERVICE, reference, value)
        .map_err(|_| io::ErrorKind::PermissionDenied.into())
}

pub(super) fn read(reference: &str) -> io::Result<Vec<u8>> {
    get_generic_password(super::SERVICE, reference)
        .map_err(|_| io::ErrorKind::PermissionDenied.into())
}

pub(super) fn delete(reference: &str) -> io::Result<()> {
    match delete_generic_password(super::SERVICE, reference) {
        Ok(()) => Ok(()),
        // errSecItemNotFound is success for idempotent deletion.
        Err(error) if error.code() == -25300 => Ok(()),
        Err(_) => Err(io::ErrorKind::PermissionDenied.into()),
    }
}
