//! Native Windows maintenance prompts, registration reads and one-shot elevation transfer.

#![allow(unsafe_code)]

use std::ffi::OsStr;
use std::io::{self, Read};
use std::mem::size_of;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::Path;
use std::ptr::{null, null_mut};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::{
    ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
};
use tokio::runtime::{Builder, Runtime};
use tokio::time::{Instant, sleep, timeout_at};
use windows_sys::Win32::Foundation::{
    ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND, ERROR_PIPE_BUSY, ERROR_SUCCESS, WAIT_OBJECT_0,
    WAIT_TIMEOUT,
};
use windows_sys::Win32::Security::{
    GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation,
};
use windows_sys::Win32::Storage::FileSystem::SECURITY_IDENTIFICATION;
use windows_sys::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
};
use windows_sys::Win32::System::Pipes::{GetNamedPipeClientProcessId, GetNamedPipeServerProcessId};
use windows_sys::Win32::System::Registry::{
    HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RRF_SUBKEY_WOW6464KEY, RegGetValueW,
};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetExitCodeProcess, GetProcessId, OpenProcess, OpenProcessToken,
    PROCESS_QUERY_LIMITED_INFORMATION, WaitForSingleObject,
};
use windows_sys::Win32::UI::Shell::{
    SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
    ShellExecuteExW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    IDYES, MB_DEFBUTTON2, MB_ICONINFORMATION, MB_ICONQUESTION, MB_OK, MB_YESNO, MessageBoxW,
    SW_HIDE,
};

use super::security::{create_machine_update_pipe, push_quoted_argument, wide_process_value};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(180);

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

/// Indicates whether the current Windows process has an elevated token.
pub fn windows_process_is_elevated() -> io::Result<bool> {
    let mut token = null_mut();
    // SAFETY: the current-process pseudo-handle is live and the output
    // pointer receives one owned token on success.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: OpenProcessToken just returned an owned non-null handle.
    let token = unsafe { OwnedHandle::from_raw_handle(token.cast()) };
    let mut elevation = TOKEN_ELEVATION::default();
    let mut length = 0;
    // SAFETY: the token is owned for this call and both output pointers have
    // writable storage for the data types and size given to Windows.
    if unsafe {
        GetTokenInformation(
            token.as_raw_handle().cast(),
            TokenElevation,
            (&raw mut elevation).cast(),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut length,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(elevation.TokenIsElevated != 0)
}

/// Owns the exact process returned by the Windows elevation broker.
pub struct WindowsElevatedProcess(OwnedHandle);

impl WindowsElevatedProcess {
    /// Identifies the one process allowed to receive the update payload.
    pub fn id(&self) -> io::Result<u32> {
        // SAFETY: this live process handle came from the successful Shell call.
        let id = unsafe { GetProcessId(self.0.as_raw_handle().cast()) };
        if id == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(id)
    }

    /// Waits without terminating a possibly committed update.
    pub fn wait(self) -> io::Result<()> {
        let handle = self.0.as_raw_handle().cast();
        // SAFETY: the owned process handle remains live until both calls return.
        match unsafe { WaitForSingleObject(handle, 720_000) } {
            WAIT_OBJECT_0 => {
                let mut code = 0;
                // SAFETY: the handle is a terminated process and code is writable.
                if unsafe { GetExitCodeProcess(handle, &mut code) } == 0 {
                    return Err(io::Error::last_os_error());
                }
                if code != 0 {
                    return Err(io::Error::other("Windows maintenance did not complete"));
                }
                Ok(())
            }
            WAIT_TIMEOUT => Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Windows maintenance outcome is unknown",
            )),
            _ => Err(io::Error::last_os_error()),
        }
    }
}

/// Requests UAC elevation for a machine-owned executable verified by the caller.
pub fn start_elevated_windows_program(
    program: &Path,
    working_directory: &Path,
    arguments: &[&OsStr],
) -> io::Result<WindowsElevatedProcess> {
    let executable = wide_process_value(crate::external_filesystem_path(program).as_os_str())?;
    let directory =
        wide_process_value(crate::external_filesystem_path(working_directory).as_os_str())?;
    let verb = wide_process_value(OsStr::new("runas"))?;
    let mut parameters: Vec<u16> = Vec::new();
    for argument in arguments {
        let wide = wide_process_value(argument)?;
        if !parameters.is_empty() {
            parameters.push(u16::from(b' '));
        }
        push_quoted_argument(&mut parameters, &wide[..wide.len() - 1]);
    }
    parameters.push(0);
    let mut request = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
        lpVerb: verb.as_ptr(),
        lpFile: executable.as_ptr(),
        lpParameters: parameters.as_ptr(),
        lpDirectory: directory.as_ptr(),
        nShow: SW_HIDE,
        ..Default::default()
    };
    // SAFETY: this initializes COM on the calling thread with a null reserved pointer.
    let initialized = unsafe {
        CoInitializeEx(
            null(),
            (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
        )
    };
    if initialized < 0 {
        return Err(io::Error::other(
            "Windows could not initialize the elevation broker",
        ));
    }
    // SAFETY: all referenced UTF-16 strings, the request and its outputs live
    // through the synchronous Shell operation.
    let launched = unsafe { ShellExecuteExW(&mut request) } != 0;
    let error = (!launched).then(io::Error::last_os_error);
    // SAFETY: balances the successful COM initialization on this thread.
    unsafe { CoUninitialize() };
    if let Some(error) = error {
        return Err(error);
    }
    if request.hProcess.is_null() {
        return Err(io::Error::other(
            "Windows did not return the maintenance worker",
        ));
    }
    // SAFETY: SEE_MASK_NOCLOSEPROCESS transfers the returned handle to the caller.
    Ok(WindowsElevatedProcess(unsafe {
        OwnedHandle::from_raw_handle(request.hProcess.cast())
    }))
}

/// The elevated worker's read-only view of the signed update transfer.
pub struct WindowsMachineUpdateInput {
    runtime: Runtime,
    pipe: NamedPipeClient,
    deadline: Instant,
    _requester: OwnedHandle,
}

impl Read for WindowsMachineUpdateInput {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.runtime.block_on(async {
            timeout_at(self.deadline, self.pipe.read(output))
                .await
                .map_err(|_| transfer_timeout())?
        })
    }
}

/// Opens the requester's endpoint read-only after verifying the owning process.
pub fn receive_windows_machine_update(
    transfer: &str,
    requester_pid: u32,
) -> io::Result<WindowsMachineUpdateInput> {
    let endpoint = endpoint(transfer)?;
    // SAFETY: OpenProcess returns a new handle on success. The requested
    // rights only allow reading the initiating process identity.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, requester_pid) };
    if process.is_null() {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: ownership of the live process handle transfers here.
    let requester = unsafe { OwnedHandle::from_raw_handle(process.cast()) };
    let runtime = Builder::new_current_thread().enable_all().build()?;
    let pipe = runtime.block_on(async {
        let mut options = ClientOptions::new();
        options.write(false).security_qos_flags(SECURITY_IDENTIFICATION);
        let pipe = timeout_at(Instant::now() + CONNECT_TIMEOUT, async {
            loop {
                match options.open(&endpoint) {
                    Ok(pipe) => return Ok(pipe),
                    Err(error) if matches!(error.raw_os_error(), Some(code) if code == ERROR_FILE_NOT_FOUND as i32 || code == ERROR_PIPE_BUSY as i32) => {
                        sleep(Duration::from_millis(30)).await;
                    }
                    Err(error) => return Err(error),
                }
            }
        })
        .await
        .map_err(|_| transfer_timeout())??;
        let mut server = 0;
        // SAFETY: pipe is a live client pipe and the server PID output is writable.
        if unsafe { GetNamedPipeServerProcessId(pipe.as_raw_handle().cast(), &mut server) } == 0 {
            return Err(io::Error::last_os_error());
        }
        if server != requester_pid {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "wrong Windows update sender",
            ));
        }
        Ok(pipe)
    })?;
    Ok(WindowsMachineUpdateInput {
        runtime,
        pipe,
        deadline: Instant::now() + TRANSFER_TIMEOUT,
        _requester: requester,
    })
}

/// Owns the standard requester's one-shot outbound endpoint.
pub struct WindowsMachineUpdateOutput {
    runtime: Runtime,
    pipe: NamedPipeServer,
}

/// Creates an endpoint owned by the caller before requesting elevation.
pub fn prepare_windows_machine_update(transfer: &str) -> io::Result<WindowsMachineUpdateOutput> {
    let endpoint = endpoint(transfer)?;
    let runtime = Builder::new_current_thread().enable_all().build()?;
    let pipe = runtime.block_on(async {
        create_machine_update_pipe(
            ServerOptions::new()
                .first_pipe_instance(true)
                .max_instances(1)
                .access_inbound(false)
                .reject_remote_clients(true),
            &endpoint,
        )
    })?;
    Ok(WindowsMachineUpdateOutput { runtime, pipe })
}

impl WindowsMachineUpdateOutput {
    /// Sends only to the exact elevated process returned by the broker.
    pub fn send(
        mut self,
        worker: &WindowsElevatedProcess,
        envelope: &[u8],
        asset: impl Read,
        size: u64,
    ) -> io::Result<()> {
        let expected_client = worker.id()?;
        let length = u32::try_from(envelope.len()).map_err(|_| io::ErrorKind::InvalidInput)?;
        self.runtime.block_on(async {
            timeout_at(Instant::now() + CONNECT_TIMEOUT, self.pipe.connect())
                .await
                .map_err(|_| transfer_timeout())??;
            let mut client = 0;
            // SAFETY: pipe is a live server pipe and the client PID output is writable.
            if unsafe { GetNamedPipeClientProcessId(self.pipe.as_raw_handle().cast(), &mut client) }
                == 0
            {
                return Err(io::Error::last_os_error());
            }
            if client != expected_client {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "wrong Windows update receiver",
                ));
            }
            timeout_at(Instant::now() + TRANSFER_TIMEOUT, async {
                self.pipe.write_all(&length.to_le_bytes()).await?;
                self.pipe.write_all(envelope).await?;
                let mut buffer = vec![0_u8; 64 * 1024];
                let mut asset = asset.take(size);
                while asset.limit() != 0 {
                    let count = asset.read(&mut buffer)?;
                    if count == 0 {
                        return Err(io::ErrorKind::UnexpectedEof.into());
                    }
                    self.pipe.write_all(&buffer[..count]).await?;
                }
                Ok(())
            })
            .await
            .map_err(|_| transfer_timeout())?
        })
    }
}

fn endpoint(transfer: &str) -> io::Result<String> {
    if transfer.len() != 32
        || !transfer
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    Ok(format!(
        r"\\.\pipe\{}.machine-update.{transfer}",
        env!("ALCOMD_BUNDLE_ID")
    ))
}

fn transfer_timeout() -> io::Error {
    io::Error::new(io::ErrorKind::TimedOut, "Windows update transfer timed out")
}
