//! Windows security primitives for current-user IPC and local file checks.

#![allow(unsafe_code)]

use std::ffi::{OsStr, c_void};
use std::io;
use std::mem::size_of;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::io::AsRawHandle;
use std::ptr::{addr_of, null_mut};

use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS, GetLastError,
    HANDLE, LocalFree,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo,
    SDDL_REVISION_1, SE_FILE_OBJECT, SE_KERNEL_OBJECT, SE_OBJECT_TYPE,
};
use windows_sys::Win32::Security::{
    ACCESS_ALLOWED_ACE, ACE_HEADER, ACL, ACL_SIZE_INFORMATION, AclSizeInformation,
    DACL_SECURITY_INFORMATION, GetAce, GetAclInformation, GetTokenInformation, INHERIT_ONLY_ACE,
    OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID, SECURITY_ATTRIBUTES, TOKEN_QUERY,
    TOKEN_USER, TokenUser,
};
use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;
use windows_sys::Win32::System::Threading::{CreateMutexW, GetCurrentProcess, OpenProcessToken};

use crate::BindError;

pub(super) fn is_fixed_drive(root: &OsStr) -> bool {
    const DRIVE_FIXED: u32 = 3;
    let root = wide_null(root);
    // SAFETY: the UTF-16 buffer is NUL-terminated and lives across the call.
    (unsafe { GetDriveTypeW(root.as_ptr()) }) == DRIVE_FIXED
}

pub(super) struct WindowsInstanceGuard {
    handle: HANDLE,
}

// SAFETY: a Windows kernel mutex HANDLE may be closed from any thread. The
// guard owns it exclusively, exposes no raw access, and performs only CloseHandle
// in Drop, so transferring guard ownership cannot create concurrent use.
unsafe impl Send for WindowsInstanceGuard {}

impl Drop for WindowsInstanceGuard {
    fn drop(&mut self) {
        // SAFETY: `handle` is a non-null owned handle returned by CreateMutexW,
        // and this guard closes it exactly once at the end of the daemon lifetime.
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: this wrapper is constructed only from a successful Win32
        // handle-returning call and closes that handle exactly once.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

struct LocalSecurityDescriptor(PSECURITY_DESCRIPTOR);

impl Drop for LocalSecurityDescriptor {
    fn drop(&mut self) {
        // SAFETY: ConvertStringSecurityDescriptorToSecurityDescriptorW allocates
        // this pointer with LocalAlloc and transfers ownership to the caller.
        unsafe {
            let _ = LocalFree(self.0);
        }
    }
}

pub(super) fn current_user_sid_string() -> io::Result<String> {
    // SAFETY: GetCurrentProcess returns a borrowed pseudo-handle, valid for the process lifetime.
    process_user_sid_string(unsafe { GetCurrentProcess() })
}

pub(super) fn process_user_sid_string(process: HANDLE) -> io::Result<String> {
    // SAFETY: all output pointers reference initialized local storage for the
    // duration of each call; the process pseudo-handle is borrowed, while the
    // returned token handle is immediately wrapped for deterministic closing.
    unsafe {
        let mut token = null_mut();
        if OpenProcessToken(process, TOKEN_QUERY, &mut token) == 0 {
            return Err(io::Error::last_os_error());
        }
        let token = OwnedHandle(token);

        let mut required = 0;
        let _ = GetTokenInformation(token.0, TokenUser, null_mut(), 0, &mut required);
        if GetLastError() != ERROR_INSUFFICIENT_BUFFER || required == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut buffer = vec![0_u8; required as usize];
        if GetTokenInformation(
            token.0,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            required,
            &mut required,
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        let token_user = &*buffer.as_ptr().cast::<TOKEN_USER>();
        sid_to_string(token_user.User.Sid)
    }
}

pub(super) fn create_secure_pipe(
    options: &ServerOptions,
    endpoint: &str,
) -> io::Result<NamedPipeServer> {
    let sid = current_user_sid_string()?;
    let descriptor = descriptor_for_sid(&sid)?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: 0,
    };

    // SAFETY: attributes and its LocalAlloc-backed descriptor remain valid for
    // the entire CreateNamedPipeW call. Tokio copies the security descriptor
    // into the kernel object before returning and does not retain the pointer.
    let pipe = unsafe {
        options.create_with_security_attributes_raw(
            endpoint,
            addr_of!(attributes).cast_mut().cast::<c_void>(),
        )?
    };
    validate_current_user_only_dacl(pipe.as_raw_handle().cast(), &sid)?;
    Ok(pipe)
}

pub(super) fn acquire_instance_mutex(name: &str) -> Result<WindowsInstanceGuard, BindError> {
    let sid = current_user_sid_string()?;
    let descriptor = descriptor_for_sid(&sid)?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: 0,
    };
    let name = wide_null(OsStr::new(name));

    // SAFETY: the security attributes, descriptor, and nul-terminated name all
    // remain valid throughout CreateMutexW. The returned handle is either
    // closed on the already-running path or transferred to the lifetime guard.
    let handle = unsafe { CreateMutexW(&attributes, 0, name.as_ptr()) };
    if handle.is_null() {
        return Err(io::Error::last_os_error().into());
    }
    // SAFETY: GetLastError is read immediately after CreateMutexW, before any
    // other Win32 call can overwrite the thread-local value.
    let already_exists = unsafe { GetLastError() == ERROR_ALREADY_EXISTS };
    if already_exists {
        // SAFETY: this branch owns the successful CreateMutexW handle and must
        // close it because another process remains the authoritative owner.
        unsafe {
            let _ = CloseHandle(handle);
        }
        return Err(BindError::AlreadyRunning);
    }
    if let Err(error) = validate_current_user_only_dacl(handle, &sid) {
        // SAFETY: validation failed before ownership was transferred; close the
        // valid CreateMutexW handle exactly once.
        unsafe {
            let _ = CloseHandle(handle);
        }
        return Err(error.into());
    }
    Ok(WindowsInstanceGuard { handle })
}

fn descriptor_for_sid(sid: &str) -> io::Result<LocalSecurityDescriptor> {
    let sddl = wide_null(OsStr::new(&format!("D:P(A;;GA;;;{sid})")));
    let mut descriptor = null_mut();
    // SAFETY: `sddl` is nul-terminated UTF-16 and the output pointer is valid.
    // On success Win32 returns a LocalAlloc allocation owned by the wrapper.
    unsafe {
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            null_mut(),
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(LocalSecurityDescriptor(descriptor))
}

pub(super) fn validate_current_user_only_dacl(
    handle: HANDLE,
    expected_sid: &str,
) -> io::Result<()> {
    let (_descriptor, dacl) = open_dacl(handle, SE_KERNEL_OBJECT, None)?;
    if ace_count(dacl)? != 1 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Windows IPC DACL must contain exactly one access rule",
        ));
    }
    let ace_pointer = get_ace(dacl, 0)?;
    // SAFETY: the ACE header is part of the live security descriptor.
    let header = unsafe { &*ace_pointer.cast::<ACE_HEADER>() };
    if u32::from(header.AceFlags) & INHERIT_ONLY_ACE != 0 {
        return Err(untrusted_security_descriptor());
    }
    let (mask, sid) = allowed_access(ace_pointer, header)?;
    if mask == 0 || sid != expected_sid {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Windows IPC DACL is not restricted to the current user",
        ));
    }
    Ok(())
}

pub(super) fn validate_installation_dacl(
    handle: HANDLE,
    user_sid: &str,
    unsafe_access: u32,
) -> io::Result<()> {
    let mut owner = null_mut();
    let (_descriptor, dacl) = open_dacl(handle, SE_FILE_OBJECT, Some(&mut owner))?;
    if owner.is_null() {
        return Err(untrusted_security_descriptor());
    }
    // SAFETY: the owner SID points into the still-live security descriptor.
    let owner = unsafe { sid_to_string(owner) }?;
    if !trusted_installation_sid(&owner, user_sid) {
        return Err(untrusted_security_descriptor());
    }
    for index in 0..ace_count(dacl)? {
        let ace_pointer = get_ace(dacl, index)?;
        // SAFETY: the ACE header is part of the live security descriptor.
        let header = unsafe { &*ace_pointer.cast::<ACE_HEADER>() };
        if u32::from(header.AceFlags) & INHERIT_ONLY_ACE != 0 || header.AceType == 1 {
            continue;
        }
        let (mask, sid) = allowed_access(ace_pointer, header)?;
        if mask & unsafe_access != 0 && !trusted_installation_sid(&sid, user_sid) {
            return Err(untrusted_security_descriptor());
        }
    }
    Ok(())
}

fn trusted_installation_sid(sid: &str, user_sid: &str) -> bool {
    sid == user_sid
        || matches!(
            sid,
            "S-1-5-18" // LocalSystem
                | "S-1-5-32-544" // Built-in administrators
                | "S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464" // TrustedInstaller
        )
}

fn untrusted_security_descriptor() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "untrusted Windows security descriptor",
    )
}

fn open_dacl(
    handle: HANDLE,
    object_type: SE_OBJECT_TYPE,
    owner: Option<&mut PSID>,
) -> io::Result<(LocalSecurityDescriptor, *mut ACL)> {
    let mut dacl: *mut ACL = null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
    let flags =
        DACL_SECURITY_INFORMATION | owner.as_ref().map_or(0, |_| OWNER_SECURITY_INFORMATION);
    // SAFETY: non-null output pointers reference writable storage. The
    // returned descriptor is LocalAlloc memory and owns the ACL and owner SID.
    let result = unsafe {
        GetSecurityInfo(
            handle,
            object_type,
            flags,
            owner.map_or(null_mut(), std::ptr::from_mut),
            null_mut(),
            &mut dacl,
            null_mut(),
            &mut descriptor,
        )
    };
    if result != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(result as i32));
    }
    let descriptor = LocalSecurityDescriptor(descriptor);
    if dacl.is_null() {
        return Err(untrusted_security_descriptor());
    }
    Ok((descriptor, dacl))
}

fn ace_count(dacl: *mut ACL) -> io::Result<u32> {
    let mut info = ACL_SIZE_INFORMATION::default();
    // SAFETY: the caller keeps the security descriptor owning the ACL alive.
    if unsafe {
        GetAclInformation(
            dacl,
            (&mut info as *mut ACL_SIZE_INFORMATION).cast(),
            size_of::<ACL_SIZE_INFORMATION>() as u32,
            AclSizeInformation,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(info.AceCount)
}

fn get_ace(dacl: *mut ACL, index: u32) -> io::Result<*mut c_void> {
    let mut ace_pointer = null_mut();
    // SAFETY: the caller keeps the descriptor alive and provides an in-range ACE index.
    if unsafe { GetAce(dacl, index, &mut ace_pointer) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(ace_pointer)
}

fn allowed_access(ace_pointer: *mut c_void, header: &ACE_HEADER) -> io::Result<(u32, String)> {
    if header.AceType != 0 || usize::from(header.AceSize) < size_of::<ACCESS_ALLOWED_ACE>() {
        return Err(untrusted_security_descriptor());
    }
    // SAFETY: a live ACE with the expected type and layout was checked above.
    let ace = unsafe { &*ace_pointer.cast::<ACCESS_ALLOWED_ACE>() };
    let sid_pointer = addr_of!(ace.SidStart).cast_mut().cast::<c_void>();
    // SAFETY: SidStart is the first word of the variable-length SID embedded
    // in ACCESS_ALLOWED_ACE and remains live with the containing descriptor.
    let sid = unsafe { sid_to_string(sid_pointer) }?;
    Ok((ace.Mask, sid))
}

unsafe fn sid_to_string(sid: PSID) -> io::Result<String> {
    let mut string_sid = null_mut();
    // SAFETY: callers provide a valid SID pointer whose allocation outlives this
    // call, and the output is a valid pointer slot for a LocalAlloc string.
    if unsafe { ConvertSidToStringSidW(sid, &mut string_sid) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut length = 0;
    // SAFETY: ConvertSidToStringSidW returns a valid nul-terminated UTF-16 string.
    while unsafe { *string_sid.add(length) } != 0 {
        length += 1;
    }
    // SAFETY: the loop found the terminating nul, so the preceding range is
    // initialized UTF-16 owned by the still-live LocalAlloc allocation.
    let value = unsafe { std::slice::from_raw_parts(string_sid, length) };
    let result = std::ffi::OsString::from_wide(value)
        .into_string()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "SID is not valid UTF-16"));
    // SAFETY: the string is LocalAlloc-owned and is released exactly once after
    // copying its UTF-16 contents.
    unsafe {
        let _ = LocalFree(string_sid.cast());
    }
    result
}

fn wide_null(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_sid_is_usable_in_sddl() {
        let sid = current_user_sid_string().expect("current SID");
        assert!(sid.starts_with("S-1-"));
        let _descriptor = descriptor_for_sid(&sid).expect("current-user descriptor");
    }
}
