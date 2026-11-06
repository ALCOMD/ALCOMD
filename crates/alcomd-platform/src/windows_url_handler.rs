#![allow(unsafe_code)]

use std::{io, mem::size_of, path::Path, ptr::null_mut};

use windows_sys::Win32::{
    Foundation::{ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_PATH_NOT_FOUND, ERROR_SUCCESS},
    System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_SZ, RegDeleteTreeW, RegGetValueW},
};

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}

fn missing(status: u32) -> bool {
    matches!(status, ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND)
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
