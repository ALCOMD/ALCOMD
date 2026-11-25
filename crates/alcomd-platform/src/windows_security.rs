//! Windows security primitives for current-user IPC, file checks and Core startup.

#![allow(unsafe_code)]

use std::ffi::{OsStr, OsString, c_void};
use std::fs::File;
use std::io;
use std::mem::size_of;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::path::Path;
use std::process::Command;
use std::ptr::{NonNull, addr_of, null, null_mut};

use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS, GENERIC_READ,
    GENERIC_WRITE, GetLastError, HANDLE, INVALID_HANDLE_VALUE, LocalFree,
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
use windows_sys::Win32::Storage::FileSystem::{
    CREATE_NEW, CreateDirectoryW, CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_SHARE_READ, GetDriveTypeW,
};
use windows_sys::Win32::System::Threading::{
    CREATE_NO_WINDOW, CREATE_UNICODE_ENVIRONMENT, CreateMutexW, CreateProcessW, GetCurrentProcess,
    OpenProcessToken, PROCESS_INFORMATION, STARTUPINFOW,
};

use crate::BindError;

pub(super) fn is_fixed_drive(root: &OsStr) -> bool {
    const DRIVE_FIXED: u32 = 3;
    let root = wide_null(root);
    // SAFETY: the UTF-16 buffer is NUL-terminated and lives across the call.
    (unsafe { GetDriveTypeW(root.as_ptr()) }) == DRIVE_FIXED
}

/// Starts the selected Core in the background without inheriting caller handles.
/// The child receives the current environment except for the named variable.
pub fn start_core_process<'a>(
    program: &Path,
    arguments: impl IntoIterator<Item = &'a OsStr>,
    working_directory: Option<&Path>,
    excluded_variable: &str,
) -> io::Result<()> {
    create_background_process(program, arguments, working_directory, excluded_variable).map(drop)
}

fn create_background_process<'a>(
    program: &Path,
    arguments: impl IntoIterator<Item = &'a OsStr>,
    working_directory: Option<&Path>,
    excluded_variable: &str,
) -> io::Result<OwnedHandle> {
    let mut command_line =
        background_command_line(&wide_process_value(program.as_os_str())?, arguments)?;
    let absolute_program = std::path::absolute(program)?;
    let mut application = wide_process_value(absolute_program.as_os_str())?;
    if application.len() > 260 {
        // Canonical Windows paths use the verbatim prefix, which allows the
        // verified executable to retain its identity beyond MAX_PATH.
        application = wide_process_value(absolute_program.canonicalize()?.as_os_str())?;
    }
    let directory = working_directory
        .map(|path| wide_process_value(crate::external_filesystem_path(path).as_os_str()))
        .transpose()?;
    let environment = core_environment(std::env::vars_os(), excluded_variable);
    let startup = STARTUPINFOW {
        cb: size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut process = PROCESS_INFORMATION::default();
    // SAFETY: all inputs are live, NUL-terminated UTF-16 buffers. The Windows
    // API can write into the command line and the output structure. Inheritance
    // is disabled and no standard handles are supplied in the startup structure.
    if unsafe {
        CreateProcessW(
            application.as_ptr(),
            command_line.as_mut_ptr(),
            null(),
            null(),
            0,
            CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT,
            environment.as_ptr().cast(),
            directory.as_ref().map_or(null(), |value| value.as_ptr()),
            &startup,
            &mut process,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let _thread = OwnedHandle(process.hThread);
    Ok(OwnedHandle(process.hProcess))
}

pub(super) fn wide_process_value(value: &OsStr) -> io::Result<Vec<u16>> {
    let wide = wide_null(value);
    if wide[..wide.len() - 1].contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Core launch argument contains a NUL",
        ));
    }
    Ok(wide)
}

fn background_command_line<'a>(
    application: &[u16],
    args: impl IntoIterator<Item = &'a OsStr>,
) -> io::Result<Vec<u16>> {
    const QUOTE: u16 = b'"' as u16;
    const MAX_COMMAND_LINE_UNITS: usize = 32_767;
    let program = &application[..application.len() - 1];
    if program.is_empty() || program.contains(&QUOTE) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid Core launch executable",
        ));
    }

    // argv[0] has its own Windows parsing rule. A filesystem path has no
    // literal quote, so enclose it to preserve any whitespace.
    let mut command_line = vec![QUOTE];
    command_line.extend_from_slice(program);
    command_line.push(QUOTE);
    for arg in args {
        let wide = wide_process_value(arg)?;
        command_line.push(b' ' as u16);
        push_quoted_argument(&mut command_line, &wide[..wide.len() - 1]);
    }
    command_line.push(0);
    if command_line.len() > MAX_COMMAND_LINE_UNITS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Core launch command line exceeds the Windows limit",
        ));
    }
    Ok(command_line)
}

pub(super) fn push_quoted_argument(command_line: &mut Vec<u16>, argument: &[u16]) {
    const QUOTE: u16 = b'"' as u16;
    const BACKSLASH: u16 = b'\\' as u16;
    command_line.push(QUOTE);
    let mut backslashes = 0;
    for &unit in argument {
        if unit == BACKSLASH {
            backslashes += 1;
            continue;
        }
        if unit == QUOTE {
            command_line.extend(std::iter::repeat_n(BACKSLASH, 2 * backslashes + 1));
        } else {
            command_line.extend(std::iter::repeat_n(BACKSLASH, backslashes));
        }
        command_line.push(unit);
        backslashes = 0;
    }
    command_line.extend(std::iter::repeat_n(BACKSLASH, 2 * backslashes));
    command_line.push(QUOTE);
}

fn core_environment(
    variables: impl IntoIterator<Item = (OsString, OsString)>,
    excluded_variable: &str,
) -> Vec<u16> {
    // Use Command's Windows environment-key ordering and ordinal casing. This
    // sorts the snapshot and removes every OS-equivalent spelling of the
    // excluded name, while preserving per-drive variables beginning with '='.
    let mut snapshot = Command::new(OsStr::new("unused"));
    snapshot
        .env_clear()
        .envs(variables)
        .env_remove(excluded_variable);
    let mut environment = Vec::new();
    for (name, value) in snapshot.get_envs() {
        let Some(value) = value else { continue };
        environment.extend(name.encode_wide());
        environment.push(b'=' as u16);
        environment.extend(value.encode_wide());
        environment.push(0);
    }
    if environment.is_empty() {
        environment.push(0);
    }
    environment.push(0);
    environment
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
    security_descriptor(&format!("D:P(A;;GA;;;{sid})"))
}

pub(super) fn create_machine_update_pipe(
    options: &ServerOptions,
    endpoint: &str,
    requester_sid: &str,
) -> io::Result<NamedPipeServer> {
    let descriptor = security_descriptor(&format!("O:BAD:P(A;;GA;;;BA)(A;;GW;;;{requester_sid})"))?;
    let mut attributes = machine_file_attributes(&descriptor);
    // SAFETY: the descriptor and attributes remain alive through creation. The
    // pipe does not inherit handles or retain the attributes pointer.
    unsafe { options.create_with_security_attributes_raw(endpoint, (&raw mut attributes).cast()) }
}

fn security_descriptor(sddl: &str) -> io::Result<LocalSecurityDescriptor> {
    let sddl = wide_null(OsStr::new(sddl));
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

fn machine_file_attributes(descriptor: &LocalSecurityDescriptor) -> SECURITY_ATTRIBUTES {
    SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: 0,
    }
}

pub(crate) fn create_machine_directory(path: &Path) -> io::Result<()> {
    let name = wide_process_value(path.as_os_str())?;
    // Inherit rights to machine-owned children. Standard users can traverse and read.
    let descriptor =
        security_descriptor("O:BAD:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;0x1200a9;;;BU)")?;
    let attributes = machine_file_attributes(&descriptor);
    // SAFETY: the name is nul-terminated and attributes and descriptor outlive the Win32 call.
    if unsafe { CreateDirectoryW(name.as_ptr(), &attributes) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub(crate) fn create_machine_record(path: &Path) -> io::Result<File> {
    let name = wide_process_value(path.as_os_str())?;
    let descriptor = security_descriptor("O:BAD:P(A;;FA;;;SY)(A;;FA;;;BA)(A;;0x1200a9;;;BU)")?;
    let attributes = machine_file_attributes(&descriptor);
    // SAFETY: the name and pointed-to security descriptor live through the call. CREATE_NEW
    // rejects an existing path. No handles are inherited, and the supplied template is null.
    let result = unsafe {
        CreateFileW(
            name.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ,
            &attributes,
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
            null_mut(),
        )
    };
    if result == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: CreateFileW returned a new owned file handle. File closes it once on drop.
    Ok(unsafe { File::from_raw_handle(result.cast()) })
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
    let header = unsafe { ace_pointer.cast::<ACE_HEADER>().as_ref() };
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
    user_sid: Option<&str>,
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
        let header = unsafe { ace_pointer.cast::<ACE_HEADER>().as_ref() };
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

fn trusted_installation_sid(sid: &str, user_sid: Option<&str>) -> bool {
    Some(sid) == user_sid
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

fn get_ace(dacl: *mut ACL, index: u32) -> io::Result<NonNull<c_void>> {
    let mut ace_pointer: Option<NonNull<c_void>> = None;
    // SAFETY: the caller keeps the descriptor alive and provides an in-range ACE index.
    // Option<NonNull<c_void>> has the layout of a nullable C pointer.
    if unsafe { GetAce(dacl, index, (&raw mut ace_pointer).cast()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    ace_pointer.ok_or_else(untrusted_security_descriptor)
}

fn allowed_access(ace_pointer: NonNull<c_void>, header: &ACE_HEADER) -> io::Result<(u32, String)> {
    if header.AceType != 0 || usize::from(header.AceSize) < size_of::<ACCESS_ALLOWED_ACE>() {
        return Err(untrusted_security_descriptor());
    }
    // SAFETY: a live ACE with the expected type and layout was checked above.
    let ace = unsafe { ace_pointer.cast::<ACCESS_ALLOWED_ACE>().as_ref() };
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
    use std::time::Duration;
    use windows_sys::Win32::Foundation::{ERROR_BROKEN_PIPE, WAIT_OBJECT_0, WAIT_TIMEOUT};
    use windows_sys::Win32::System::Pipes::{CreatePipe, PeekNamedPipe};
    use windows_sys::Win32::System::Threading::WaitForSingleObject;
    use windows_sys::Win32::UI::Shell::CommandLineToArgvW;

    const DETACHED_CHILD: &str = "alcomd-detached-core-launch-probe";
    const TEST_CREDENTIAL: &str = "ALCOMD_MCP_CLIENT_CREDENTIAL";

    #[test]
    fn current_sid_is_usable_in_sddl() {
        let sid = current_user_sid_string().expect("current SID");
        assert!(sid.starts_with("S-1-"));
        let _descriptor = descriptor_for_sid(&sid).expect("current-user descriptor");
    }

    #[test]
    fn background_arguments_roundtrip_the_native_windows_parser() {
        let path = OsStr::new(r"C:\Program Files\ALCOMD\alcomd.exe");
        let surrogate_path = OsString::from_wide(&[b'X' as u16, 0xd800, b'\\' as u16]);
        let arguments = [
            OsStr::new(""),
            OsStr::new("--data-dir"),
            OsStr::new("C:\\some path\twith spaces\\"),
            OsStr::new("one\\\\\"two"),
            surrogate_path.as_os_str(),
        ];
        let command = background_command_line(&wide_null(path), arguments).expect("command line");
        let mut count = 0;
        // SAFETY: command is terminated by a NUL and count is a writable i32.
        let parsed = unsafe { CommandLineToArgvW(command.as_ptr(), &mut count) };
        assert!(!parsed.is_null(), "{}", io::Error::last_os_error());
        // SAFETY: Windows returned count readable pointers, all NUL-terminated,
        // in a LocalAlloc block. Copy before releasing the allocation.
        let decoded = unsafe {
            let arguments = std::slice::from_raw_parts(parsed, count as usize);
            let decoded: Vec<_> = arguments
                .iter()
                .map(|&argument| {
                    let length = (0..).take_while(|&at| *argument.add(at) != 0).count();
                    OsString::from_wide(std::slice::from_raw_parts(argument, length))
                })
                .collect();
            let _ = LocalFree(parsed.cast());
            decoded
        };
        assert_eq!(
            decoded,
            [path].into_iter().chain(arguments).collect::<Vec<_>>()
        );
        let too_long = OsString::from("a".repeat(32_767));
        assert!(background_command_line(&wide_null(path), [too_long.as_os_str()]).is_err());
        assert!(background_command_line(&wide_null(path), [OsStr::new("a\0b")]).is_err());
    }

    #[test]
    fn core_environment_excludes_case_variants_and_preserves_native_values() {
        let units = [
            ("=C:", r"C:\work"),
            ("alcomd_mcp_client_credential", "test-marker"),
            ("Path", r"C:\bin"),
            ("UNICODE", "目录"),
        ];
        let variables = units.map(|(name, value)| (name.into(), value.into()));
        let block = core_environment(variables, TEST_CREDENTIAL);
        let expected: Vec<_> = "=C:=C:\\work\0Path=C:\\bin\0UNICODE=目录\0\0"
            .encode_utf16()
            .collect();
        assert_eq!(block, expected);
        assert_eq!(core_environment([], TEST_CREDENTIAL), [0, 0]);
    }

    #[test]
    fn background_process_does_not_keep_an_inheritable_pipe_open() {
        // Keep the inheritable fixture handle in a dedicated process so other
        // tests spawning children cannot inherit it in parallel.
        let probe = std::process::Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "windows::security::tests::inheritable_pipe_probe",
                "--ignored",
            ])
            .output()
            .expect("run isolated inheritance probe");
        assert!(
            probe.status.success(),
            "{}",
            String::from_utf8_lossy(&probe.stdout)
        );
    }

    #[test]
    #[ignore = "isolates the inheritable handle from unrelated test processes"]
    fn inheritable_pipe_probe() {
        let attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: null_mut(),
            bInheritHandle: 1,
        };
        let mut reader = null_mut();
        let mut writer = null_mut();
        // SAFETY: the two writable handle slots and initialized attributes live
        // across CreatePipe. On success both handles are owned by this test.
        let created = unsafe { CreatePipe(&mut reader, &mut writer, &attributes, 0) };
        assert_ne!(created, 0);
        let reader = OwnedHandle(reader);
        let writer = OwnedHandle(writer);
        let program = std::env::current_exe().expect("current test executable");
        let args = [
            "--exact",
            "windows::security::tests::background_process_child",
            "--skip",
            DETACHED_CHILD,
        ];
        let child = create_background_process(
            &program,
            args.map(OsStr::new),
            program.parent(),
            TEST_CREDENTIAL,
        )
        .expect("start detached test child");
        drop(writer);
        // SAFETY: child owns a valid process handle and this wait does not
        // consume it. The child must still be alive while its pipe is checked.
        assert_eq!(unsafe { WaitForSingleObject(child.0, 0) }, WAIT_TIMEOUT);
        // SAFETY: only the live pipe handle is needed to check for a writer.
        // All optional output pointers are null and no buffer is supplied.
        let peeked =
            unsafe { PeekNamedPipe(reader.0, null_mut(), 0, null_mut(), null_mut(), null_mut()) };
        assert_eq!(
            peeked, 0,
            "the detached child inherited the writable handle"
        );
        assert_eq!(
            io::Error::last_os_error().raw_os_error(),
            Some(ERROR_BROKEN_PIPE as i32)
        );
        // SAFETY: the live process handle is still owned by child. Verify that
        // the pipe had no writer while the process was definitely still alive.
        let during_check = unsafe { WaitForSingleObject(child.0, 0) };
        assert_eq!(during_check, WAIT_TIMEOUT);
        // SAFETY: the process handle remains valid through this bounded wait.
        let exited = unsafe { WaitForSingleObject(child.0, 10_000) };
        assert_eq!(exited, WAIT_OBJECT_0);
    }

    #[test]
    fn background_process_child() {
        if std::env::args_os().any(|argument| argument == DETACHED_CHILD) {
            std::thread::sleep(Duration::from_millis(800));
        }
    }
}
