use std::env;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};

use rustix::fd::OwnedFd;
use rustix::fs::{
    AtFlags, CWD, FileType, FlockOperation, Mode, OFlags, RawMode, RenameFlags, chmodat, fchmod,
    flock, fstat, mkdirat, openat, renameat_with, statat, unlinkat,
};
use rustix::io::Errno;
use rustix::process::geteuid;
use tokio::net::{UnixListener, UnixStream};

use crate::{BindError, DataConfig, IpcConfig};

/// Returns the current Unix filesystem object's device/inode identity.
pub fn file_identity_key(path: &std::path::Path) -> io::Result<Vec<u8>> {
    let metadata = rustix::fs::stat(path).map_err(io::Error::from)?;
    let mut key = Vec::with_capacity(16);
    key.extend_from_slice(&(metadata.st_dev as u64).to_le_bytes());
    key.extend_from_slice(&(metadata.st_ino as u64).to_le_bytes());
    Ok(key)
}

const SOCKET_NAME: &str = "rpc-v1.sock";
const STAGED_SOCKET_NAME: &str = ".rpc-bind";
const LOCK_NAME: &str = "daemon-v1.lock";
const PRIVATE_DIRECTORY_MODE: RawMode = 0o700;
const PRIVATE_FILE_MODE: RawMode = 0o600;
const PORTABLE_SOCKET_PATH_LIMIT: usize = 103;

/// Unix IPC stream used by both daemon and client.
pub type IpcStream = UnixStream;

/// Process-lifetime ownership acquired before the authoritative store opens.
pub struct DaemonInstance {
    runtime_path: PathBuf,
    runtime_fd: OwnedFd,
    instance: InstanceGuard,
}

impl DaemonInstance {
    /// Acquires the per-user instance lock without publishing the IPC endpoint.
    pub fn acquire(config: &IpcConfig) -> Result<Self, BindError> {
        let runtime_path = runtime_path(config)?;
        let runtime_fd = ensure_private_runtime_directory(&runtime_path)?;
        let instance = acquire_instance(&runtime_fd)?;
        Ok(Self {
            runtime_path,
            runtime_fd,
            instance,
        })
    }

    /// Recovers the owned stale socket and publishes the ready endpoint.
    pub fn bind(self) -> Result<IpcListener, BindError> {
        validate_socket_path_length(&self.runtime_path.join(SOCKET_NAME))?;
        remove_stale_socket(&self.runtime_fd, SOCKET_NAME)?;
        remove_stale_socket(&self.runtime_fd, STAGED_SOCKET_NAME)?;

        // An ordinary bind creates a socket with the caller's umask. Publish it
        // under the client-visible name only after setting the required mode.
        let listener = UnixListener::bind(self.runtime_path.join(STAGED_SOCKET_NAME))?;
        let publish = secure_socket_node(&self.runtime_fd, STAGED_SOCKET_NAME).and_then(|()| {
            renameat_with(
                &self.runtime_fd,
                STAGED_SOCKET_NAME,
                &self.runtime_fd,
                SOCKET_NAME,
                RenameFlags::NOREPLACE,
            )
            .map_err(io::Error::from)
        });
        if let Err(error) = publish {
            let _ = remove_stale_socket(&self.runtime_fd, STAGED_SOCKET_NAME);
            return Err(error.into());
        }
        Ok(IpcListener {
            listener,
            runtime_fd: self.runtime_fd,
            _instance: self.instance,
        })
    }
}

/// Bound Unix endpoint together with the process-lifetime instance lock.
pub struct IpcListener {
    listener: UnixListener,
    runtime_fd: OwnedFd,
    _instance: InstanceGuard,
}

impl IpcListener {
    /// Acquires the instance lock, safely recovers a stale socket, and binds.
    pub fn bind(config: &IpcConfig) -> Result<Self, BindError> {
        DaemonInstance::acquire(config)?.bind()
    }

    /// Accepts one same-user connection.
    pub async fn accept(&mut self) -> io::Result<IpcStream> {
        loop {
            let (stream, _) = self.listener.accept().await?;
            if stream
                .peer_cred()
                .is_ok_and(|peer| peer.uid() == geteuid().as_raw())
            {
                return Ok(stream);
            }
        }
    }
}

impl Drop for IpcListener {
    fn drop(&mut self) {
        if socket_is_owned(&self.runtime_fd) {
            let _ = unlinkat(&self.runtime_fd, SOCKET_NAME, AtFlags::empty());
        }
    }
}

/// Connects to the configured per-user Unix endpoint after checking both ends.
pub async fn connect(config: &IpcConfig) -> io::Result<IpcStream> {
    let runtime = runtime_path(config)?;
    let path = runtime.join(SOCKET_NAME);
    validate_socket_path_length(&path)?;
    let directory = open_protected_directory_chain(&runtime).map_err(endpoint_path_error)?;
    validate_owned_type(&directory, FileType::Directory, "runtime directory")?;
    let directory_stat = fstat(&directory).map_err(io::Error::from)?;
    if directory_stat.st_mode & 0o777 != PRIVATE_DIRECTORY_MODE {
        return Err(insecure_endpoint());
    }
    let initial_socket = protected_socket_stat(&directory)?;
    let stream = UnixStream::connect(path).await?;
    if !stream
        .peer_cred()
        .is_ok_and(|peer| peer.uid() == geteuid().as_raw())
    {
        return Err(insecure_endpoint());
    }
    let current_directory =
        open_protected_directory_chain(&runtime).map_err(|_| insecure_endpoint())?;
    let current_directory_stat = fstat(&current_directory).map_err(|_| insecure_endpoint())?;
    let current_socket =
        protected_socket_stat(&current_directory).map_err(|_| insecure_endpoint())?;
    if current_directory_stat.st_dev != directory_stat.st_dev
        || current_directory_stat.st_ino != directory_stat.st_ino
        || current_directory_stat.st_mode != directory_stat.st_mode
        || current_directory_stat.st_uid != directory_stat.st_uid
        || current_socket.st_dev != initial_socket.st_dev
        || current_socket.st_ino != initial_socket.st_ino
    {
        return Err(insecure_endpoint());
    }
    Ok(stream)
}

fn protected_socket_stat(directory: &OwnedFd) -> io::Result<rustix::fs::Stat> {
    let stat =
        statat(directory, SOCKET_NAME, AtFlags::SYMLINK_NOFOLLOW).map_err(io::Error::from)?;
    if stat.st_uid != geteuid().as_raw()
        || FileType::from_raw_mode(stat.st_mode) != FileType::Socket
        || stat.st_mode & 0o777 != PRIVATE_FILE_MODE
    {
        return Err(insecure_endpoint());
    }
    Ok(stat)
}

fn endpoint_path_error(error: io::Error) -> io::Error {
    if matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
    ) {
        error
    } else {
        insecure_endpoint()
    }
}

fn insecure_endpoint() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "untrusted Unix IPC endpoint",
    )
}

/// Returns a non-sensitive endpoint string for diagnostics.
pub fn endpoint_display(config: &IpcConfig) -> io::Result<String> {
    Ok(runtime_path(config)?
        .join(SOCKET_NAME)
        .display()
        .to_string())
}

/// Creates a current-user-owned private data directory and returns `state.db`.
pub fn state_database_path(config: &DataConfig) -> io::Result<PathBuf> {
    if let Some(path) = config.data_directory() {
        let path = validate_absolute_runtime_path(path)?;
        let _directory = ensure_private_runtime_directory(&path)?;
        return Ok(path.join("state.db"));
    }

    let home = home_directory()?;
    let home_fd = open_directory_chain(&home)?;
    validate_owned_type(&home_fd, FileType::Directory, "home directory")?;
    let (base_path, components) = user_data_layout(&home)?;
    let base_fd = if base_path == home {
        home_fd
    } else {
        let fd = open_directory_chain(&base_path)?;
        validate_owned_type(&fd, FileType::Directory, "XDG data directory")?;
        fd
    };
    let directory_fd = ensure_owned_relative_directories(base_fd, components)?;
    fchmod(&directory_fd, Mode::from_raw_mode(PRIVATE_DIRECTORY_MODE)).map_err(io::Error::from)?;
    Ok(join_data_components(base_path, components).join("state.db"))
}

pub(super) fn home_directory() -> io::Result<PathBuf> {
    let home = env::var_os("HOME").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "HOME is required for the data directory",
        )
    })?;
    validate_absolute_runtime_path(&PathBuf::from(home))
}

fn user_data_layout(home: &Path) -> io::Result<(PathBuf, &'static [&'static str])> {
    #[cfg(target_os = "linux")]
    if let Some(xdg) = env::var_os("XDG_DATA_HOME") {
        return Ok((
            validate_absolute_runtime_path(&PathBuf::from(xdg))?,
            &[env!("ALCOMD_DAEMON_NAME")],
        ));
    }
    #[cfg(target_os = "linux")]
    let components: &[&str] = &[".local", "share", env!("ALCOMD_DAEMON_NAME")];
    #[cfg(target_os = "macos")]
    let components: &[&str] = &[
        "Library",
        "Application Support",
        env!("ALCOMD_PRODUCT_DIRECTORY"),
    ];
    Ok((home.to_path_buf(), components))
}

fn join_data_components(base: PathBuf, components: &[&str]) -> PathBuf {
    components
        .iter()
        .fold(base, |path, component| path.join(component))
}

pub(super) fn data_directory_path(config: &DataConfig) -> io::Result<PathBuf> {
    if let Some(path) = config.data_directory() {
        return validate_absolute_runtime_path(path);
    }
    let (base, components) = user_data_layout(&home_directory()?)?;
    Ok(join_data_components(base, components))
}

fn ensure_owned_relative_directories(
    mut current: OwnedFd,
    components: &[&str],
) -> io::Result<OwnedFd> {
    for component in components {
        match mkdirat(
            &current,
            *component,
            Mode::from_raw_mode(PRIVATE_DIRECTORY_MODE),
        ) {
            Ok(()) | Err(Errno::EXIST) => {}
            Err(error) => return Err(error.into()),
        }
        let next = openat(
            &current,
            *component,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
        )
        .map_err(io::Error::from)?;
        validate_owned_type(&next, FileType::Directory, "data directory")?;
        current = next;
    }
    Ok(current)
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
        let candidate = validate_absolute_runtime_path(&PathBuf::from(base).join("alcomd"))?;
        if socket_path_fits(&candidate) {
            return Ok(candidate);
        }
    }

    let uid = geteuid().as_raw();
    #[cfg(target_os = "macos")]
    let fallback = PathBuf::from(format!("/private/tmp/alcomd-{uid}"));
    #[cfg(not(target_os = "macos"))]
    let fallback = PathBuf::from(format!("/tmp/alcomd-{uid}"));
    validate_absolute_runtime_path(&fallback)
}

pub(super) fn validate_absolute_runtime_path(path: &Path) -> io::Result<PathBuf> {
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
    open_directory_chain_with(path, |_| Ok(()))
}

pub(super) fn open_protected_directory_chain(path: &Path) -> io::Result<OwnedFd> {
    open_directory_chain_with(path, |directory| {
        let stat = fstat(directory).map_err(io::Error::from)?;
        let root_owned_sticky = stat.st_uid == 0 && stat.st_mode & 0o1000 != 0;
        if (stat.st_uid == 0 || stat.st_uid == geteuid().as_raw())
            && (stat.st_mode & 0o022 == 0 || root_owned_sticky)
        {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "untrusted directory permissions",
            ))
        }
    })
}

#[cfg(target_os = "linux")]
pub(super) fn open_system_directory_chain(path: &Path) -> io::Result<OwnedFd> {
    open_directory_chain_with(path, |directory| {
        let stat = fstat(directory).map_err(io::Error::from)?;
        if stat.st_uid != 0 || stat.st_mode & 0o022 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "untrusted system installation directory",
            ));
        }
        Ok(())
    })
}

fn open_directory_chain_with(
    path: &Path,
    check_directory: impl Fn(&OwnedFd) -> io::Result<()>,
) -> io::Result<OwnedFd> {
    let mut current = openat(
        CWD,
        Path::new("/"),
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(io::Error::from)?;
    check_directory(&current)?;

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
                check_directory(&current)?;
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

fn remove_stale_socket(runtime_fd: &OwnedFd, name: &str) -> io::Result<()> {
    match statat(runtime_fd, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => {
            if stat.st_uid != geteuid().as_raw()
                || FileType::from_raw_mode(stat.st_mode) != FileType::Socket
            {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "existing endpoint is not an owned Unix socket",
                ));
            }
            unlinkat(runtime_fd, name, AtFlags::empty()).map_err(io::Error::from)
        }
        Err(Errno::NOENT) => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn secure_socket_node(runtime_fd: &OwnedFd, name: &str) -> io::Result<()> {
    let stat = statat(runtime_fd, name, AtFlags::SYMLINK_NOFOLLOW).map_err(io::Error::from)?;
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
        name,
        Mode::from_raw_mode(PRIVATE_FILE_MODE),
        AtFlags::empty(),
    )
    .map_err(io::Error::from)?;
    let secured = statat(runtime_fd, name, AtFlags::SYMLINK_NOFOLLOW).map_err(io::Error::from)?;
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

#[cfg(target_os = "macos")]
fn socket_path_fits(runtime_path: &Path) -> bool {
    runtime_path.join(SOCKET_NAME).as_os_str().as_bytes().len() <= PORTABLE_SOCKET_PATH_LIMIT
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
        #[cfg(target_os = "macos")]
        let base = PathBuf::from("/private/tmp");
        #[cfg(not(target_os = "macos"))]
        let base = env::temp_dir();
        base.join(format!("acm1-{label}-{}-{nonce}", std::process::id()))
    }

    #[tokio::test]
    async fn endpoint_is_private_and_single_instance_is_enforced() {
        let path = isolated_path("private");
        let config = IpcConfig::isolated(path.clone());
        let mut listener = IpcListener::bind(&config).expect("bind first listener");
        assert!(!path.join(STAGED_SOCKET_NAME).exists());
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

    #[tokio::test]
    async fn connect_distinguishes_absence_from_untrusted_socket_or_directory() {
        let path = isolated_path("connect");
        let config = IpcConfig::isolated(path.clone());
        assert_eq!(
            connect(&config).await.expect_err("absent endpoint").kind(),
            io::ErrorKind::NotFound
        );
        assert!(!path.exists());

        let listener = IpcListener::bind(&config).expect("private listener");
        let socket_path = path.join(SOCKET_NAME);
        fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o660))
            .expect("unsafe socket mode");
        assert_eq!(
            connect(&config).await.expect_err("unsafe socket").kind(),
            io::ErrorKind::PermissionDenied
        );
        fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600))
            .expect("restore socket mode");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o770)).expect("unsafe runtime mode");
        assert_eq!(
            connect(&config).await.expect_err("unsafe directory").kind(),
            io::ErrorKind::PermissionDenied
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
            .expect("restore runtime mode");
        drop(listener);

        symlink(path.join("absent"), &socket_path).expect("dangling socket symlink");
        assert_eq!(
            connect(&config).await.expect_err("socket symlink").kind(),
            io::ErrorKind::PermissionDenied
        );
        fs::remove_file(socket_path).expect("remove fixture symlink");
        fs::remove_file(path.join(LOCK_NAME)).expect("remove fixture lock");
        fs::remove_dir(path).expect("remove fixture directory");
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

    #[tokio::test]
    async fn owned_stale_socket_is_recovered_after_lock() {
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

    #[test]
    fn isolated_state_directory_is_private_and_symlinks_are_rejected() {
        let path = isolated_path("data");
        let database =
            state_database_path(&DataConfig::isolated(path.clone())).expect("isolated data path");
        assert_eq!(database, path.join("state.db"));
        assert_eq!(
            fs::metadata(&path)
                .expect("data metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        fs::remove_dir(path).expect("remove isolated data directory");

        let target = isolated_path("data-target");
        let link = isolated_path("data-link");
        fs::create_dir(&target).expect("create data target");
        symlink(&target, &link).expect("create data symlink");
        assert!(state_database_path(&DataConfig::isolated(link.clone())).is_err());
        fs::remove_file(link).expect("remove data symlink");
        fs::remove_dir(target).expect("remove data target");
    }
}
