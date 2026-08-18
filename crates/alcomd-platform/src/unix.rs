use std::env;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};

use rustix::fd::OwnedFd;
use rustix::fs::{
    AtFlags, CWD, FileType, FlockOperation, Mode, OFlags, RawMode, chmodat, fchmod, flock, fstat,
    mkdirat, openat, statat, unlinkat,
};
use rustix::io::Errno;
use rustix::process::geteuid;
use tokio::net::{UnixListener, UnixStream};

use crate::{BindError, IpcConfig};

const SOCKET_NAME: &str = "rpc-v1.sock";
const LOCK_NAME: &str = "daemon-v1.lock";
const PRIVATE_DIRECTORY_MODE: RawMode = 0o700;
const PRIVATE_FILE_MODE: RawMode = 0o600;
const PORTABLE_SOCKET_PATH_LIMIT: usize = 103;

/// Unix IPC stream used by both daemon and client.
pub type IpcStream = UnixStream;

/// Bound Unix endpoint together with the process-lifetime instance lock.
pub struct IpcListener {
    listener: UnixListener,
    runtime_fd: OwnedFd,
    _instance: InstanceGuard,
}

impl IpcListener {
    /// Acquires the instance lock, safely recovers a stale socket, and binds.
    pub fn bind(config: &IpcConfig) -> Result<Self, BindError> {
        let runtime_path = runtime_path(config)?;
        let runtime_fd = ensure_private_runtime_directory(&runtime_path)?;
        let instance = acquire_instance(&runtime_fd)?;
        remove_stale_socket(&runtime_fd)?;

        let socket_path = runtime_path.join(SOCKET_NAME);
        validate_socket_path_length(&socket_path)?;
        let listener = UnixListener::bind(&socket_path)?;
        secure_socket_node(&runtime_fd)?;

        Ok(Self {
            listener,
            runtime_fd,
            _instance: instance,
        })
    }

    /// Accepts one same-user connection.
    pub async fn accept(&mut self) -> io::Result<IpcStream> {
        self.listener.accept().await.map(|(stream, _)| stream)
    }
}

impl Drop for IpcListener {
    fn drop(&mut self) {
        if socket_is_owned(&self.runtime_fd) {
            let _ = unlinkat(&self.runtime_fd, SOCKET_NAME, AtFlags::empty());
        }
    }
}

/// Connects to the configured per-user Unix endpoint.
pub async fn connect(config: &IpcConfig) -> io::Result<IpcStream> {
    let path = runtime_path(config)?.join(SOCKET_NAME);
    validate_socket_path_length(&path)?;
    UnixStream::connect(path).await
}

/// Returns a non-sensitive endpoint string for diagnostics.
pub fn endpoint_display(config: &IpcConfig) -> io::Result<String> {
    Ok(runtime_path(config)?
        .join(SOCKET_NAME)
        .display()
        .to_string())
}

struct InstanceGuard {
    _lock_fd: OwnedFd,
}

fn acquire_instance(runtime_fd: &OwnedFd) -> Result<InstanceGuard, BindError> {
    let lock_fd = openat(
        runtime_fd,
        LOCK_NAME,
        OFlags::CREATE | OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::from_raw_mode(PRIVATE_FILE_MODE),
    )
    .map_err(io::Error::from)?;
    validate_owned_type(&lock_fd, FileType::RegularFile, "instance lock")?;
    fchmod(&lock_fd, Mode::from_raw_mode(PRIVATE_FILE_MODE)).map_err(io::Error::from)?;

    match flock(&lock_fd, FlockOperation::NonBlockingLockExclusive) {
        Ok(()) => Ok(InstanceGuard { _lock_fd: lock_fd }),
        Err(error) if error == Errno::WOULDBLOCK => Err(BindError::AlreadyRunning),
        Err(error) => Err(BindError::Io(error.into())),
    }
}

fn runtime_path(config: &IpcConfig) -> io::Result<PathBuf> {
    if let Some(path) = config.runtime_directory() {
        return validate_absolute_runtime_path(path);
    }

    #[cfg(target_os = "linux")]
    if let Some(base) = env::var_os("XDG_RUNTIME_DIR") {
        return validate_absolute_runtime_path(&PathBuf::from(base).join("alcomd"));
    }

    #[cfg(target_os = "macos")]
    if let Some(base) = env::var_os("TMPDIR") {
        return validate_absolute_runtime_path(&PathBuf::from(base).join("alcomd"));
    }

    let uid = geteuid().as_raw();
    #[cfg(target_os = "macos")]
    let fallback = PathBuf::from(format!("/private/tmp/alcomd-{uid}"));
    #[cfg(not(target_os = "macos"))]
    let fallback = PathBuf::from(format!("/tmp/alcomd-{uid}"));
    validate_absolute_runtime_path(&fallback)
}

fn validate_absolute_runtime_path(path: &Path) -> io::Result<PathBuf> {
    #[cfg(target_os = "macos")]
    let path = normalize_macos_system_alias(path);
    #[cfg(not(target_os = "macos"))]
    let path = path.to_path_buf();
    if !path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::CurDir | Component::Prefix(_)
            )
        })
        || path.file_name().is_none()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "runtime directory must be an absolute normalized path",
        ));
    }
    Ok(path)
}

#[cfg(target_os = "macos")]
fn normalize_macos_system_alias(path: &Path) -> PathBuf {
    for (alias, canonical) in [("/var", "/private/var"), ("/tmp", "/private/tmp")] {
        if path == Path::new(alias) {
            return PathBuf::from(canonical);
        }
        if let Ok(suffix) = path.strip_prefix(alias) {
            return PathBuf::from(canonical).join(suffix);
        }
    }
    path.to_path_buf()
}

fn ensure_private_runtime_directory(path: &Path) -> io::Result<OwnedFd> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "runtime directory has no parent",
        )
    })?;
    let name = path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "runtime directory has no final component",
        )
    })?;
    let parent_fd = open_directory_chain(parent)?;

    match mkdirat(
        &parent_fd,
        name,
        Mode::from_raw_mode(PRIVATE_DIRECTORY_MODE),
    ) {
        Ok(()) | Err(Errno::EXIST) => {}
        Err(error) => return Err(error.into()),
    }
    let runtime_fd = openat(
        &parent_fd,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(io::Error::from)?;
    validate_owned_type(&runtime_fd, FileType::Directory, "runtime directory")?;
    fchmod(&runtime_fd, Mode::from_raw_mode(PRIVATE_DIRECTORY_MODE)).map_err(io::Error::from)?;
    let stat = fstat(&runtime_fd).map_err(io::Error::from)?;
    if stat.st_mode & 0o777 != PRIVATE_DIRECTORY_MODE {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "runtime directory permissions are not 0700",
        ));
    }
    Ok(runtime_fd)
}

fn open_directory_chain(path: &Path) -> io::Result<OwnedFd> {
    let mut current = openat(
        CWD,
        Path::new("/"),
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(io::Error::from)?;

    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => {
                current = openat(
                    &current,
                    name,
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                    Mode::empty(),
                )
                .map_err(io::Error::from)?;
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "runtime path contains an unsafe component",
                ));
            }
        }
    }
    Ok(current)
}

fn remove_stale_socket(runtime_fd: &OwnedFd) -> io::Result<()> {
    match statat(runtime_fd, SOCKET_NAME, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => {
            if stat.st_uid != geteuid().as_raw()
                || FileType::from_raw_mode(stat.st_mode) != FileType::Socket
            {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "existing endpoint is not an owned Unix socket",
                ));
            }
            unlinkat(runtime_fd, SOCKET_NAME, AtFlags::empty()).map_err(io::Error::from)
        }
        Err(Errno::NOENT) => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn secure_socket_node(runtime_fd: &OwnedFd) -> io::Result<()> {
    let stat =
        statat(runtime_fd, SOCKET_NAME, AtFlags::SYMLINK_NOFOLLOW).map_err(io::Error::from)?;
    if stat.st_uid != geteuid().as_raw()
        || FileType::from_raw_mode(stat.st_mode) != FileType::Socket
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "new endpoint is not an owned Unix socket",
        ));
    }
    chmodat(
        runtime_fd,
        SOCKET_NAME,
        Mode::from_raw_mode(PRIVATE_FILE_MODE),
        AtFlags::empty(),
    )
    .map_err(io::Error::from)?;
    let secured =
        statat(runtime_fd, SOCKET_NAME, AtFlags::SYMLINK_NOFOLLOW).map_err(io::Error::from)?;
    if secured.st_mode & 0o777 != PRIVATE_FILE_MODE {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "socket permissions are not 0600",
        ));
    }
    Ok(())
}

fn socket_is_owned(runtime_fd: &OwnedFd) -> bool {
    statat(runtime_fd, SOCKET_NAME, AtFlags::SYMLINK_NOFOLLOW)
        .map(|stat| {
            stat.st_uid == geteuid().as_raw()
                && FileType::from_raw_mode(stat.st_mode) == FileType::Socket
        })
        .unwrap_or(false)
}

fn validate_owned_type(fd: &OwnedFd, expected: FileType, label: &str) -> io::Result<()> {
    let stat = fstat(fd).map_err(io::Error::from)?;
    if stat.st_uid != geteuid().as_raw() || FileType::from_raw_mode(stat.st_mode) != expected {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{label} is not owned by the effective user or has the wrong type"),
        ));
    }
    Ok(())
}

fn validate_socket_path_length(path: &Path) -> io::Result<()> {
    if path.as_os_str().as_bytes().len() > PORTABLE_SOCKET_PATH_LIMIT {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Unix socket path exceeds the portable sockaddr_un limit",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn isolated_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        env::temp_dir().join(format!("alcomd-m1-{label}-{}-{nonce}", std::process::id()))
    }

    #[tokio::test]
    async fn endpoint_is_private_and_single_instance_is_enforced() {
        let path = isolated_path("private");
        let config = IpcConfig::isolated(path.clone());
        let mut listener = IpcListener::bind(&config).expect("bind first listener");
        assert!(matches!(
            IpcListener::bind(&config),
            Err(BindError::AlreadyRunning)
        ));
        assert_eq!(
            fs::metadata(&path)
                .expect("runtime metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(path.join(SOCKET_NAME))
                .expect("socket metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );

        let client = connect(&config).await.expect("connect");
        let server = listener.accept().await.expect("accept");
        drop((client, server, listener));
        assert!(!path.join(SOCKET_NAME).exists());
        fs::remove_dir_all(path).expect("remove isolated runtime");
    }

    #[test]
    fn symlink_runtime_directory_is_rejected() {
        let target = isolated_path("target");
        let link = isolated_path("link");
        fs::create_dir(&target).expect("create target");
        symlink(&target, &link).expect("create symlink");
        let error = match IpcListener::bind(&IpcConfig::isolated(link.clone())) {
            Ok(_) => panic!("symlink runtime directory was accepted"),
            Err(error) => error,
        };
        assert!(matches!(error, BindError::Io(_)));
        fs::remove_file(link).expect("remove symlink");
        fs::remove_dir(target).expect("remove target");
    }

    #[test]
    fn owned_stale_socket_is_recovered_after_lock() {
        let path = isolated_path("stale");
        let config = IpcConfig::isolated(path.clone());
        fs::create_dir(&path).expect("create runtime");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).expect("chmod runtime");
        let stale = std::os::unix::net::UnixListener::bind(path.join(SOCKET_NAME))
            .expect("create stale socket");
        drop(stale);

        let listener = IpcListener::bind(&config).expect("recover stale socket");
        drop(listener);
        fs::remove_dir_all(path).expect("remove isolated runtime");
    }
}
