//! Outbound local socket connections authenticated by the kernel-reported peer.

use std::io;
use std::os::unix::fs::FileTypeExt;
use std::path::PathBuf;

use tokio::net::UnixStream;

pub enum UnixIpcIdentity {
    #[cfg(target_os = "macos")]
    MacApplication {
        bundle_identifier: String,
        team_identifier: String,
    },
    #[cfg(target_os = "linux")]
    LinuxSystemExecutable(PathBuf),
}

pub async fn connect_verified_local_socket(
    name: &str,
    identity: UnixIpcIdentity,
) -> io::Result<UnixStream> {
    if name.is_empty()
        || name.len() > 80
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    // Runtime directories come from Core's environment, never the guest's input.
    let root = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .find(|path| path.is_absolute())
        .unwrap_or_else(std::env::temp_dir);
    if !root.is_absolute() {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let address = root.join(name);
    if !tokio::fs::symlink_metadata(&address)
        .await?
        .file_type()
        .is_socket()
    {
        return Err(denied());
    }
    let stream = UnixStream::connect(address).await?;
    let credentials = stream.peer_cred()?;
    let pid = credentials
        .pid()
        .filter(|pid| *pid > 0 && *pid as u32 != std::process::id())
        .ok_or_else(denied)?;
    if credentials.uid() != rustix::process::geteuid().as_raw() {
        return Err(denied());
    }
    tokio::task::spawn_blocking(move || {
        verify_process(pid, identity)?;
        if stream.peer_cred()? != credentials {
            return Err(denied());
        }
        Ok(stream)
    })
    .await
    .map_err(io::Error::other)?
}

#[cfg(target_os = "linux")]
fn verify_process(pid: i32, identity: UnixIpcIdentity) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;

    let UnixIpcIdentity::LinuxSystemExecutable(expected) = identity;
    let process_image = PathBuf::from(format!("/proc/{pid}/exe"));
    if !expected.is_absolute()
        || std::fs::read_link(&process_image)? != expected
        || std::fs::canonicalize(&expected)? != expected
    {
        return Err(denied());
    }
    for path in expected.ancestors() {
        let metadata = std::fs::symlink_metadata(path)?;
        if metadata.uid() != 0 || metadata.mode() & 0o022 != 0 || metadata.is_symlink() {
            return Err(denied());
        }
    }
    let installed = std::fs::metadata(expected)?;
    let loaded = std::fs::metadata(process_image)?;
    if !installed.is_file() || (loaded.dev(), loaded.ino()) != (installed.dev(), installed.ino()) {
        return Err(denied());
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn verify_process(pid: i32, identity: UnixIpcIdentity) -> io::Result<()> {
    use security_framework::os::macos::code_signing::{
        Flags, GuestAttributes, SecCode, SecRequirement,
    };

    let UnixIpcIdentity::MacApplication {
        bundle_identifier,
        team_identifier,
    } = identity;
    if bundle_identifier.is_empty()
        || bundle_identifier.len() > 160
        || !bundle_identifier
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b".-".contains(&byte))
        || team_identifier.len() != 10
        || !team_identifier
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let requirement: SecRequirement = format!(
        "anchor apple generic and identifier \"{bundle_identifier}\" and certificate leaf[subject.OU] = \"{team_identifier}\""
    ).parse().map_err(|_| denied())?;
    let mut attributes = GuestAttributes::new();
    attributes.set_pid(pid);
    let code =
        SecCode::copy_guest_with_attribues(None, &attributes, Flags::NONE).map_err(|_| denied())?;
    code.check_validity(Flags::NO_NETWORK_ACCESS, &requirement)
        .map_err(|_| denied())
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn verify_process(_: i32, _: UnixIpcIdentity) -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}

fn denied() -> io::Error {
    io::ErrorKind::PermissionDenied.into()
}
