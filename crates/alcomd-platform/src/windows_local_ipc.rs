//! Verified outbound connections to explicitly named, local Windows pipes.
#![allow(unsafe_code)]

use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io;
use std::mem::size_of;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::{Path, PathBuf};
use std::ptr::{from_mut, null_mut};

use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};
use windows_sys::Win32::Foundation::{ERROR_PIPE_BUSY, HANDLE};
use windows_sys::Win32::Security::Cryptography::{
    CERT_NAME_ATTR_TYPE, CertGetNameStringW, szOID_ORGANIZATION_NAME,
};
use windows_sys::Win32::Security::WinTrust::{
    WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_DATA_0, WINTRUST_FILE_INFO,
    WTD_CACHE_ONLY_URL_RETRIEVAL, WTD_CHOICE_FILE, WTD_REVOKE_NONE, WTD_STATEACTION_CLOSE,
    WTD_STATEACTION_VERIFY, WTD_UI_NONE, WTHelperGetProvCertFromChain,
    WTHelperGetProvSignerFromChain, WTHelperProvDataFromStateData, WinVerifyTrustEx,
};
use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_READ, SECURITY_IDENTIFICATION};
use windows_sys::Win32::System::Pipes::GetNamedPipeServerProcessId;
use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};

use super::security::{current_user_sid_string, process_user_sid_string};

/// Returns a pipe only after verifying the connected server's user, session,
/// executable name, and trusted Authenticode signer organization.
pub async fn connect_verified_local_pipe(
    name: String,
    executable_name: String,
    publisher: String,
) -> io::Result<NamedPipeClient> {
    let endpoint = local_endpoint(&name)?;
    let pipe = ClientOptions::new()
        .security_qos_flags(SECURITY_IDENTIFICATION)
        .open(endpoint)
        .map_err(|failure| {
            if failure.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) {
                io::Error::new(io::ErrorKind::WouldBlock, failure)
            } else {
                failure
            }
        })?;
    tokio::task::spawn_blocking(move || {
        verify_peer(&pipe, &executable_name, &publisher)?;
        Ok(pipe)
    })
    .await
    .map_err(io::Error::other)?
}

fn local_endpoint(name: &str) -> io::Result<String> {
    if name.is_empty()
        || name.len() > 80
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    Ok(format!(r"\\.\pipe\{name}"))
}

fn verify_peer(
    pipe: &NamedPipeClient,
    expected_image: &str,
    expected_publisher: &str,
) -> io::Result<()> {
    let pipe_handle = pipe.as_raw_handle();
    let pid = server_pid(pipe_handle)?;
    let caller_pid = std::process::id();
    if pid == caller_pid || session_id(pid)? != session_id(caller_pid)? {
        return Err(denied());
    }
    // SAFETY: OpenProcess returns a new handle, which is checked and closed via OwnedHandle.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the successful OpenProcess handle is owned by this call.
    let process = unsafe { OwnedHandle::from_raw_handle(process) };
    if process_user_sid_string(process.as_raw_handle())? != current_user_sid_string()? {
        return Err(denied());
    }

    let path = executable_path(process.as_raw_handle())?;
    if !path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case(expected_image))
    {
        return Err(denied());
    }
    verify_publisher(&path, expected_publisher)?;
    if server_pid(pipe_handle)? != pid || executable_path(process.as_raw_handle())? != path {
        return Err(denied());
    }
    Ok(())
}

pub(super) fn verify_local_server_user(pipe: &NamedPipeClient, sid: &str) -> io::Result<()> {
    let handle = pipe.as_raw_handle();
    let pid = server_pid(handle)?;
    // SAFETY: a non-null OpenProcess result is an owned query handle for the named-pipe peer.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return Err(denied());
    }
    // SAFETY: ownership transfers to OwnedHandle and the query handle closes on return.
    let process = unsafe { OwnedHandle::from_raw_handle(process) };
    if process_user_sid_string(process.as_raw_handle()).map_err(|_| denied())? != sid
        || server_pid(handle)? != pid
    {
        return Err(denied());
    }
    Ok(())
}

fn server_pid(pipe: HANDLE) -> io::Result<u32> {
    let mut pid = 0;
    // SAFETY: `pipe` is borrowed from a live client handle; the output points to a writable u32.
    if unsafe { GetNamedPipeServerProcessId(pipe, &mut pid) } == 0 || pid == 0 {
        return Err(denied());
    }
    Ok(pid)
}

fn session_id(pid: u32) -> io::Result<u32> {
    let mut session = 0;
    // SAFETY: ProcessIdToSessionId writes at most one u32 to this output pointer.
    if unsafe { ProcessIdToSessionId(pid, &mut session) } == 0 {
        return Err(denied());
    }
    Ok(session)
}

fn executable_path(process: HANDLE) -> io::Result<PathBuf> {
    let mut buffer = vec![0_u16; 32_768];
    let mut length = buffer.len() as u32;
    // SAFETY: the process handle is borrowed; the buffer length is provided in UTF-16 code units.
    if unsafe { QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut length) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(PathBuf::from(OsString::from_wide(
        &buffer[..length as usize],
    )))
}

pub(crate) fn verify_publisher(path: &Path, publisher: &str) -> io::Result<()> {
    let image = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(path)?;
    let wide_path: Vec<_> = path.as_os_str().encode_wide().chain([0]).collect();
    let mut file = WINTRUST_FILE_INFO {
        cbStruct: size_of::<WINTRUST_FILE_INFO>() as u32,
        pcwszFilePath: wide_path.as_ptr(),
        hFile: image.as_raw_handle(),
        pgKnownSubject: null_mut(),
    };
    let mut data = WINTRUST_DATA {
        cbStruct: size_of::<WINTRUST_DATA>() as u32,
        dwUIChoice: WTD_UI_NONE,
        fdwRevocationChecks: WTD_REVOKE_NONE,
        dwUnionChoice: WTD_CHOICE_FILE,
        Anonymous: WINTRUST_DATA_0 {
            pFile: from_mut(&mut file),
        },
        dwStateAction: WTD_STATEACTION_VERIFY,
        dwProvFlags: WTD_CACHE_ONLY_URL_RETRIEVAL,
        ..Default::default()
    };
    let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
    // SAFETY: file and path outlive both WinVerifyTrustEx calls. The state is always closed
    // after any copied signer information has been read.
    let matches = unsafe {
        let trusted = WinVerifyTrustEx(null_mut(), &mut action, &mut data) == 0;
        let accepted = trusted && signer_matches(data.hWVTStateData, publisher);
        data.dwStateAction = WTD_STATEACTION_CLOSE;
        let _ = WinVerifyTrustEx(null_mut(), &mut action, &mut data);
        accepted
    };
    if matches { Ok(()) } else { Err(denied()) }
}

unsafe fn signer_matches(state: HANDLE, expected: &str) -> bool {
    if state.is_null() {
        return false;
    }
    // SAFETY: the caller holds an open, successfully verified WinTrust state.
    // Each borrowed chain object is checked for null before its use.
    unsafe {
        let data = WTHelperProvDataFromStateData(state);
        if data.is_null() {
            return false;
        }
        let signer = WTHelperGetProvSignerFromChain(data, 0, 0, 0);
        if signer.is_null() {
            return false;
        }
        let Some(entry) = WTHelperGetProvCertFromChain(signer, 0).as_ref() else {
            return false;
        };
        if entry.pCert.is_null() {
            return false;
        }
        let mut name = [0_u16; 257];
        let length = CertGetNameStringW(
            entry.pCert,
            CERT_NAME_ATTR_TYPE,
            0,
            szOID_ORGANIZATION_NAME.cast(),
            name.as_mut_ptr(),
            name.len() as u32,
        ) as usize;
        length > 1
            && length <= name.len()
            && name[..length - 1]
                .iter()
                .copied()
                .eq(expected.encode_utf16())
    }
}

fn denied() -> io::Error {
    io::ErrorKind::PermissionDenied.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn occupied_pipe_reports_would_block_without_validating_or_writing() {
        use tokio::net::windows::named_pipe::ServerOptions;

        let name = format!("alcomd-m9-busy-{}", std::process::id());
        let endpoint = local_endpoint(&name).expect("local fixture name");
        let server = ServerOptions::new()
            .first_pipe_instance(true)
            .max_instances(1)
            .create(&endpoint)
            .expect("one-instance fixture pipe");
        let client = ClientOptions::new()
            .open(&endpoint)
            .expect("occupy the pipe");
        server.connect().await.expect("fixture connection");
        let error = connect_verified_local_pipe(name, "Discord.exe".into(), "Discord Inc.".into())
            .await
            .expect_err("occupied fixture cannot be opened");
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        drop(client);
    }

    #[test]
    fn remote_and_private_path_syntax_cannot_be_opened() {
        assert_eq!(
            local_endpoint("example-ipc-0").expect("local name"),
            r"\\.\pipe\example-ipc-0"
        );
        for name in [
            r"\\remote\pipe\host",
            "nested/name",
            "pipe.name",
            "",
            "abc:123",
        ] {
            assert_eq!(
                local_endpoint(name).expect_err("not a local name").kind(),
                io::ErrorKind::InvalidInput
            );
        }
    }

    #[tokio::test]
    #[ignore = "requires the signed Discord desktop client in the same Windows session"]
    async fn verified_discord_pipe_accepts_only_the_matching_publisher() {
        use std::time::Duration;

        for index in 0..10 {
            let name = format!("discord-ipc-{index}");
            match tokio::time::timeout(
                Duration::from_secs(5),
                connect_verified_local_pipe(
                    name.clone(),
                    "Discord.exe".into(),
                    "Discord Inc.".into(),
                ),
            )
            .await
            .expect("bounded verifier")
            {
                Ok(pipe) => {
                    drop(pipe);
                    let wrong_publisher = connect_verified_local_pipe(
                        name,
                        "Discord.exe".into(),
                        "Unrelated publisher".into(),
                    )
                    .await
                    .expect_err("verified signer did not match");
                    assert_eq!(wrong_publisher.kind(), io::ErrorKind::PermissionDenied);
                    return;
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => panic!("native verifier rejected a live candidate: {error}"),
            }
        }
        panic!("no live Discord pipe passed verification");
    }
}
