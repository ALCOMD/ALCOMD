//! Windows Shell application identity, package registration, and registered activation.

#![allow(unsafe_code)]

use std::io;

use std::ptr::{null, null_mut};
use windows_sys::Win32::{
    Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS},
    Storage::Packaging::Appx::GetPackagesByPackageFamily,
    UI::{
        Shell::{SetCurrentProcessExplicitAppUserModelID, ShellExecuteW},
        WindowsAndMessaging::SW_SHOWNORMAL,
    },
};

const HUB_FAMILY: &str = "UnityTechnologies.UnityHub_2vrhnee42bhxm";

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}

/// Sets the current process's explicit taskbar identity before any UI is created.
pub fn set_current_process_app_user_model_id(app_id: &str) -> io::Result<()> {
    let mut wide: Vec<u16> = app_id.encode_utf16().collect();
    if wide.is_empty()
        || wide.len() > 128
        || wide.contains(&0)
        || app_id.chars().any(char::is_whitespace)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Windows application identity must contain 1-128 UTF-16 units without NUL or whitespace",
        ));
    }
    wide.push(0);
    // SAFETY: `wide` owns a validated, NUL-terminated UTF-16 string and remains
    // alive for the synchronous Shell call. The API copies the process identity;
    // it neither retains this pointer nor transfers ownership of the buffer.
    let result = unsafe { SetCurrentProcessExplicitAppUserModelID(wide.as_ptr()) };
    if result < 0 {
        return Err(io::Error::other(format!(
            "failed to set Windows application identity: HRESULT {result:#010x}"
        )));
    }
    Ok(())
}

pub(crate) fn unity_hub_msix_registered() -> io::Result<bool> {
    let mut count = 0;
    let mut length = 0;
    // SAFETY: The family is terminated and count/length query outputs are writable.
    let status = unsafe {
        GetPackagesByPackageFamily(
            wide(HUB_FAMILY).as_ptr(),
            &mut count,
            null_mut(),
            &mut length,
            null_mut(),
        )
    } as u32;
    match status {
        ERROR_SUCCESS | ERROR_INSUFFICIENT_BUFFER => Ok(count > 0),
        status => Err(io::Error::from_raw_os_error(status as i32)),
    }
}

pub(crate) fn activate_unity_hub_msix() -> io::Result<()> {
    if !unity_hub_msix_registered()? {
        return Err(io::ErrorKind::NotFound.into());
    }
    let app = wide(&format!(r"shell:AppsFolder\{HUB_FAMILY}!UnityHub"));
    // SAFETY: The URI is fixed to Unity's registered AUMID and remains valid for the call.
    // No command line, arguments or caller-controlled URI are passed.
    let result = unsafe {
        ShellExecuteW(
            null_mut(),
            null(),
            app.as_ptr(),
            null(),
            null(),
            SW_SHOWNORMAL,
        )
    };
    if result as isize <= 32 {
        return Err(io::Error::other(
            "Windows did not accept Unity Hub activation",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::System::Com::CoTaskMemFree;
    use windows_sys::Win32::UI::Shell::GetCurrentProcessExplicitAppUserModelID;

    #[test]
    fn native_identity_roundtrip_and_rejected_inputs_preserve_the_process_identity() {
        let product: toml::Value =
            toml::from_str(include_str!("../../../alcomd.product.toml")).expect("product metadata");
        let expected = product["identity"]["windows_aumid"]
            .as_str()
            .expect("Windows identity");
        set_current_process_app_user_model_id(expected).expect("set native process identity");
        assert_eq!(read_current_identity(), expected);

        for invalid in [
            "".to_owned(),
            "a\0b".to_owned(),
            "a b".to_owned(),
            "x".repeat(129),
        ] {
            assert_eq!(
                set_current_process_app_user_model_id(&invalid)
                    .expect_err("invalid identity must be rejected")
                    .kind(),
                io::ErrorKind::InvalidInput
            );
            assert_eq!(read_current_identity(), expected);
        }
    }

    fn read_current_identity() -> String {
        let mut value = std::ptr::null_mut();
        // SAFETY: the API writes one owned, NUL-terminated PWSTR to this valid
        // output pointer. After success, it is freed exactly once below.
        let result = unsafe { GetCurrentProcessExplicitAppUserModelID(&mut value) };
        assert!(result >= 0, "read process identity: HRESULT {result:#010x}");
        assert!(!value.is_null(), "successful identity query returned null");
        let mut length = 0;
        // SAFETY: a successful Shell query returned a NUL-terminated allocation
        // which remains live until the explicit CoTaskMemFree below.
        while unsafe { *value.add(length) } != 0 {
            length += 1;
        }
        // SAFETY: the preceding scan found the terminator in the same live
        // allocation. Copy the initialized UTF-16 range before freeing it.
        let copied = unsafe { std::slice::from_raw_parts(value, length) }.to_vec();
        // SAFETY: this allocation was returned by the Shell getter, whose
        // contract requires CoTaskMemFree. This is its only release.
        unsafe { CoTaskMemFree(value.cast()) };
        String::from_utf16(&copied).expect("native identity is valid UTF-16")
    }
}
