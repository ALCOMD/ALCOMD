#![allow(unsafe_code)]

use std::io;
use std::ptr::null_mut;

use windows_sys::Win32::{
    Foundation::{ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND, ERROR_SUCCESS},
    System::Registry::{
        HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RRF_SUBKEY_WOW6464KEY, RegGetValueW,
    },
    UI::WindowsAndMessaging::{
        IDYES, MB_DEFBUTTON2, MB_ICONINFORMATION, MB_ICONQUESTION, MB_OK, MB_YESNO, MessageBoxW,
    },
};

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}

/// Reads a bounded string from the current user or machine's native registry view.
pub fn read_windows_installation_registry_string(
    machine: bool,
    key: &str,
    name: &str,
) -> io::Result<Option<String>> {
    if key.is_empty() || name.is_empty() || key.contains('\0') || name.contains('\0') {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let key = wide(key);
    let name = wide(name);
    let mut data = [0_u16; 4096];
    let mut length = size_of_val(&data) as u32;
    let root = if machine {
        HKEY_LOCAL_MACHINE
    } else {
        HKEY_CURRENT_USER
    };
    // SAFETY: The NUL-terminated strings and the writable byte buffer outlive the call.
    let status = unsafe {
        RegGetValueW(
            root,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY,
            null_mut(),
            data.as_mut_ptr().cast(),
            &mut length,
        )
    };
    match status {
        ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND => return Ok(None),
        ERROR_SUCCESS => {}
        code => return Err(io::Error::from_raw_os_error(code as i32)),
    }
    let length = length as usize;
    if !length.is_multiple_of(size_of::<u16>()) || length > size_of_val(&data) {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let value = &data[..length / size_of::<u16>()];
    let content = value
        .strip_suffix(&[0])
        .filter(|value| !value.is_empty() && !value.contains(&0))
        .ok_or(io::ErrorKind::InvalidData)?;
    String::from_utf16(content)
        .map(Some)
        .map_err(io::Error::other)
}

/// Asks for explicit consent on the current desktop, defaulting to No.
pub fn confirm_windows_installation(text: &str) -> bool {
    show_message(text, MB_YESNO | MB_ICONQUESTION | MB_DEFBUTTON2) == IDYES
}

/// Shows an installation result on the current desktop.
pub fn notify_windows_installation(text: &str) {
    show_message(text, MB_OK | MB_ICONINFORMATION);
}

fn show_message(text: &str, flags: u32) -> i32 {
    let caption = wide(env!("ALCOMD_INSTALL_DIRECTORY"));
    let text = wide(text);
    // SAFETY: Both UTF-16 strings remain alive through the modal call, with no parent window.
    unsafe { MessageBoxW(null_mut(), text.as_ptr(), caption.as_ptr(), flags) }
}
