//! Private Windows Known Folder FFI for the single approved Local AppData query.

#![allow(unsafe_code)]

use std::ffi::{OsString, c_void};
use std::io;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::ptr::null_mut;
use std::rc::Rc;

use windows_sys::Win32::Foundation::{RPC_E_CHANGED_MODE, S_FALSE, S_OK};
#[cfg(test)]
use windows_sys::Win32::System::Com::COINIT_APARTMENTTHREADED;
use windows_sys::Win32::System::Com::{
    COINIT_MULTITHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize,
};
use windows_sys::Win32::UI::Shell::{FOLDERID_LocalAppData, SHGetKnownFolderPath};
use windows_sys::core::{HRESULT, PWSTR};

struct ComInitializationGuard {
    must_uninitialize: bool,
    _thread_bound: std::marker::PhantomData<Rc<()>>,
}

impl ComInitializationGuard {
    fn initialize() -> io::Result<Self> {
        // SAFETY: the reserved pointer is required to be null. This call affects
        // only the current thread. Every S_OK/S_FALSE success is represented by
        // a guard whose Drop calls CoUninitialize once on this same thread.
        let result = unsafe { CoInitializeEx(null_mut(), COINIT_MULTITHREADED as u32) };
        match result {
            S_OK | S_FALSE => Ok(Self {
                must_uninitialize: true,
                _thread_bound: std::marker::PhantomData,
            }),
            RPC_E_CHANGED_MODE => Ok(Self {
                must_uninitialize: false,
                _thread_bound: std::marker::PhantomData,
            }),
            failure => Err(hresult_error("failed to initialize COM", failure)),
        }
    }
}

impl Drop for ComInitializationGuard {
    fn drop(&mut self) {
        if self.must_uninitialize {
            // SAFETY: this guard is neither Send nor moved to another thread;
            // it exists only after a successful CoInitializeEx on this thread,
            // and Drop balances that exact S_OK/S_FALSE call once.
            unsafe {
                CoUninitialize();
            }
        }
    }
}

struct KnownFolderPath(PWSTR);

impl KnownFolderPath {
    fn to_path_buf(&self) -> io::Result<PathBuf> {
        if self.0.is_null() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Local AppData Known Folder returned a null path",
            ));
        }
        let mut length = 0_usize;
        // SAFETY: SHGetKnownFolderPath returned a successful, non-null PWSTR
        // owned by this guard. Its contract guarantees a nul-terminated UTF-16
        // string that remains valid until this guard calls CoTaskMemFree.
        while unsafe { *self.0.add(length) } != 0 {
            length = length.checked_add(1).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "Known Folder path is too long")
            })?;
        }
        // SAFETY: the preceding scan found the terminating nul while the
        // CoTaskMem-owned allocation is still live, so this initialized range
        // can be copied into an owned OsString before Drop frees it.
        let value = unsafe { std::slice::from_raw_parts(self.0, length) };
        Ok(PathBuf::from(OsString::from_wide(value)))
    }
}

impl Drop for KnownFolderPath {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: SHGetKnownFolderPath allocates a successful or partially
            // returned non-null PWSTR with the COM task allocator. This guard
            // owns that pointer and releases it exactly once with CoTaskMemFree.
            unsafe {
                CoTaskMemFree(self.0.cast::<c_void>());
            }
        }
    }
}

pub(super) fn local_app_data_directory() -> io::Result<PathBuf> {
    let _com = ComInitializationGuard::initialize()?;
    let mut path: PWSTR = null_mut();
    // SAFETY: `path` is initialized to null and passed as a valid output slot.
    // The Known Folder GUID is static, flags are default (0), and null token
    // selects the current user. Any non-null output is immediately owned by the
    // RAII guard below, including on an HRESULT failure.
    let result = unsafe { SHGetKnownFolderPath(&FOLDERID_LocalAppData, 0, null_mut(), &mut path) };
    let path = KnownFolderPath(path);
    if result < 0 {
        return Err(hresult_error(
            "failed to resolve the Local AppData Known Folder",
            result,
        ));
    }
    path.to_path_buf()
}

fn hresult_error(context: &str, result: HRESULT) -> io::Error {
    io::Error::other(format!("{context} (HRESULT 0x{:08x})", result as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_app_data_is_an_absolute_existing_directory() {
        let directory = local_app_data_directory().expect("Local AppData Known Folder");
        assert!(directory.is_absolute());
        assert!(directory.is_dir());
    }

    #[test]
    fn changed_apartment_mode_uses_existing_com_without_unbalancing_it() {
        // SAFETY: this test initializes COM on its own test thread with a null
        // reserved pointer and balances the successful S_OK/S_FALSE result once
        // at the end of the same thread.
        let result = unsafe { CoInitializeEx(null_mut(), COINIT_APARTMENTTHREADED as u32) };
        assert!(matches!(result, S_OK | S_FALSE));
        let directory = local_app_data_directory().expect("query from existing STA");
        assert!(directory.is_absolute());
        // SAFETY: the successful explicit CoInitializeEx above is balanced once
        // on this same thread. The nested query saw RPC_E_CHANGED_MODE and did
        // not call CoUninitialize for the existing apartment.
        unsafe {
            CoUninitialize();
        }
    }
}
