#![allow(unsafe_code)]

use std::{
    io,
    mem::size_of,
    path::Path,
    ptr::{null, null_mut},
};

use windows_sys::Win32::{
    Foundation::{
        ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_NO_ASSOCIATION, ERROR_NO_MORE_ITEMS,
        ERROR_PATH_NOT_FOUND, ERROR_SUCCESS, ERROR_UNSUPPORTED_TYPE, S_OK,
    },
    System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_READ, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ, RegCloseKey,
        RegDeleteTreeW, RegEnumKeyExW, RegGetValueW, RegOpenKeyExW,
    },
    UI::Shell::{
        ASSOCF_INIT_IGNOREUNKNOWN, ASSOCF_IS_PROTOCOL, ASSOCSTR_EXECUTABLE, AssocQueryStringW,
    },
};

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}

fn missing(status: u32) -> bool {
    matches!(status, ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND)
}

/// Reads the current shell handler, including the logged-in user's default selection.
pub fn default_association_executable(
    name: &str,
    protocol: bool,
) -> io::Result<Option<std::path::PathBuf>> {
    use std::os::windows::ffi::OsStringExt;

    let mut output = vec![0_u16; 32_768];
    let mut length = output.len() as u32;
    let name = wide(name);
    let flags = ASSOCF_INIT_IGNOREUNKNOWN | if protocol { ASSOCF_IS_PROTOCOL } else { 0 };
    // SAFETY: The association name is terminated and the output buffer is writable for length units.
    let result = unsafe {
        AssocQueryStringW(
            flags,
            ASSOCSTR_EXECUTABLE,
            name.as_ptr(),
            null(),
            output.as_mut_ptr(),
            &mut length,
        )
    };
    if result == S_OK && length > 1 && (length as usize) <= output.len() {
        return Ok(Some(
            std::ffi::OsString::from_wide(&output[..length as usize - 1]).into(),
        ));
    }
    // AssocQueryString reports Win32 errors as HRESULT_FROM_WIN32.
    if result == (0x8007_0000 | ERROR_NO_ASSOCIATION) as i32 {
        return Ok(None);
    }
    Err(io::Error::other(format!(
        "shell association query failed ({result:#x})"
    )))
}

fn matches_default(key: &str, value: &str) -> io::Result<bool> {
    let key = wide(key);
    let expected = wide(value);
    let mut buffer = vec![0_u16; expected.len()];
    let mut bytes =
        u32::try_from(buffer.len() * size_of::<u16>()).map_err(|_| io::ErrorKind::InvalidInput)?;
    // SAFETY: The key is NUL-terminated and the buffer is writable for the stated byte count.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            std::ptr::null(),
            RRF_RT_REG_SZ,
            null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut bytes,
        )
    };
    match status {
        ERROR_SUCCESS => {
            Ok(buffer == expected && bytes as usize == expected.len() * size_of::<u16>())
        }
        ERROR_MORE_DATA => Ok(false),
        status if missing(status) => Ok(false),
        status => Err(io::Error::from_raw_os_error(status as i32)),
    }
}

/// Removes the per-user registration written by this app for the current executable.
pub fn unregister_owned_url_handler(scheme: &str, identifier: &str, exe: &Path) -> io::Result<()> {
    if scheme.is_empty() || !scheme.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let exe = exe.to_str().ok_or(io::ErrorKind::InvalidInput)?;
    let key = format!(r"Software\Classes\{scheme}");
    if !matches_default(&key, &format!("URL:{identifier} protocol"))?
        || !matches_default(
            &format!(r"{key}\shell\open\command"),
            &format!("\"{exe}\" \"%1\""),
        )?
    {
        return Ok(());
    }
    let key = wide(&key);
    // SAFETY: The validated scheme identifies one NUL-terminated current-user class subkey.
    let status = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, key.as_ptr()) };
    if status == ERROR_SUCCESS || missing(status) {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(status as i32))
    }
}

pub(crate) struct ReadOnlyRegistryKey(HKEY);

impl Drop for ReadOnlyRegistryKey {
    fn drop(&mut self) {
        // SAFETY: The handle was returned by RegOpenKeyExW and is owned by this wrapper.
        unsafe { RegCloseKey(self.0) };
    }
}

pub(crate) fn open_read_only(
    parent: HKEY,
    name: &str,
    view: u32,
) -> io::Result<Option<ReadOnlyRegistryKey>> {
    let mut key = null_mut();
    // SAFETY: The path is terminated and the output handle is writable.
    let status =
        unsafe { RegOpenKeyExW(parent, wide(name).as_ptr(), 0, KEY_READ | view, &mut key) };
    match status {
        ERROR_SUCCESS => Ok(Some(ReadOnlyRegistryKey(key))),
        status if missing(status) => Ok(None),
        status => Err(io::Error::from_raw_os_error(status as i32)),
    }
}

impl ReadOnlyRegistryKey {
    pub(crate) fn child(&self, name: &str, view: u32) -> io::Result<Option<Self>> {
        open_read_only(self.0, name, view)
    }

    pub(crate) fn value(&self, name: &str) -> io::Result<Option<String>> {
        let mut data = [0_u16; 4097];
        let mut size = size_of_val(&data) as u32;
        // SAFETY: The key is live. The name and returned value buffer outlive the call.
        let status = unsafe {
            RegGetValueW(
                self.0,
                null(),
                wide(name).as_ptr(),
                RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ,
                null_mut(),
                data.as_mut_ptr().cast(),
                &mut size,
            )
        };
        match status {
            ERROR_SUCCESS => {
                let end = data
                    .iter()
                    .position(|unit| *unit == 0)
                    .unwrap_or(data.len());
                Ok(String::from_utf16(&data[..end]).ok())
            }
            status if missing(status) || status == ERROR_MORE_DATA => Ok(None),
            // An unrelated registry value of another type is not a discovery failure.
            ERROR_UNSUPPORTED_TYPE => Ok(None),
            status => Err(io::Error::from_raw_os_error(status as i32)),
        }
    }

    pub(crate) fn child_name(&self, index: u32) -> io::Result<Option<String>> {
        let mut data = [0_u16; 256];
        let mut size = data.len() as u32;
        // SAFETY: The key is live and the buffer capacity is supplied in UTF-16 units.
        let status = unsafe {
            RegEnumKeyExW(
                self.0,
                index,
                data.as_mut_ptr(),
                &mut size,
                null(),
                null_mut(),
                null_mut(),
                null_mut(),
            )
        };
        match status {
            ERROR_SUCCESS => Ok(Some(String::from_utf16_lossy(&data[..size as usize]))),
            ERROR_NO_MORE_ITEMS => Ok(None),
            status => Err(io::Error::from_raw_os_error(status as i32)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        os::windows::process::CommandExt,
        process::{Command, Output},
        time::{SystemTime, UNIX_EPOCH},
    };

    fn reg(args: &[&str]) -> Output {
        Command::new("reg.exe")
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .args(args)
            .output()
            .expect("Windows registry tool")
    }

    #[test]
    fn unregistered_protocol_and_extension_have_no_default_executable() {
        let name = "alcomd-association-reader-unregistered-fixture";
        assert_eq!(
            default_association_executable(name, true).expect("query unregistered protocol"),
            None
        );
        assert_eq!(
            default_association_executable(&format!(".{name}"), false)
                .expect("query unregistered file extension"),
            None
        );
    }

    #[test]
    #[ignore = "writes an isolated randomized HKCU scheme and removes it on exit"]
    fn vcc_opt_out_removes_only_its_own_user_handler() {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let scheme = format!("alcomdvccfixture{id:x}");
        let key = format!(r"HKCU\Software\Classes\{scheme}");
        let command = format!(r"{key}\shell\open\command");
        assert!(!reg(&["query", &key]).status.success());
        struct Cleanup(String);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = reg(&["delete", &self.0, "/f"]);
            }
        }
        let _cleanup = Cleanup(key.clone());
        let identifier = "com.vrclearn.alcomd.fixture";
        let exe = std::env::current_exe().expect("fixture executable");
        let path = exe.to_str().expect("fixture path");
        let add = |name: &str, data: &str| {
            assert!(
                reg(&["add", name, "/ve", "/d", data, "/f"])
                    .status
                    .success()
            );
        };
        add(&key, &format!("URL:{identifier} protocol"));
        let set_command = |exe: &str| add(&command, &format!("\"{exe}\" \"%1\""));
        set_command(r"C:\Other Handler.exe");
        unregister_owned_url_handler(&scheme, identifier, &exe).expect("leave other executable");
        assert!(reg(&["query", &command, "/ve"]).status.success());
        set_command(path);
        unregister_owned_url_handler(&scheme, "other.owner", &exe).expect("leave other identifier");
        assert!(reg(&["query", &key]).status.success());
        unregister_owned_url_handler(&scheme, identifier, &exe).expect("remove own handler");
        assert!(!reg(&["query", &key]).status.success());
        unregister_owned_url_handler(&scheme, identifier, &exe).expect("already absent");
    }
}
