use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use rustix::fd::{AsFd, OwnedFd};
use rustix::fs::{
    AtFlags, FileType, FlockOperation, Mode, OFlags, flock, fstat, fsync, mkdirat, openat,
    readlinkat, statat, symlinkat, unlinkat,
};
use rustix::io::Errno;
use rustix::process::geteuid;

use crate::{
    DataConfig, LINUX_BOOTSTRAP_EXECUTABLE_NAME, LINUX_GUI_EXECUTABLE_NAME,
    check_linux_core_registration, linux_portable_runtime,
};

pub const LINUX_CLI_EXECUTABLE_NAME: &str = env!("ALCOMD_CLI_NAME");
pub const PORTABLE_CLI_LAUNCHER_NAME: &str =
    concat!(env!("ALCOMD_CLI_NAME"), "-portable-launcher-v1");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PortableCliEntryState {
    Missing,
    Owned,
    Conflict,
}

/// Holds the user's private bin directory so writes never follow a replacement directory link.
pub struct PortableCliEntry {
    directory: Option<OwnedFd>,
    launcher: PathBuf,
}

impl PortableCliEntry {
    pub fn open(launcher: &Path, create: bool) -> io::Result<Self> {
        Self::in_home(&crate::unix::home_directory()?, launcher, create)
    }

    fn in_home(home: &Path, launcher: &Path, create: bool) -> io::Result<Self> {
        let launcher = crate::unix::validate_absolute_runtime_path(launcher)?;
        let mut directory = crate::unix::open_protected_directory_chain(home)?;
        require_owned_directory(&directory)?;
        for name in [".local", "bin"] {
            if create {
                match mkdirat(&directory, name, Mode::from_raw_mode(0o700)) {
                    Ok(()) => fsync(&directory).map_err(io::Error::from)?,
                    Err(Errno::EXIST) => {}
                    Err(error) => return Err(error.into()),
                }
            }
            directory = match openat(
                &directory,
                name,
                OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            ) {
                Ok(directory) => directory,
                Err(Errno::NOENT) if !create => {
                    return Ok(Self {
                        directory: None,
                        launcher,
                    });
                }
                Err(error) => return Err(error.into()),
            };
            require_owned_directory(&directory)?;
        }
        Ok(Self {
            directory: Some(directory),
            launcher,
        })
    }

    pub fn state(&self) -> io::Result<PortableCliEntryState> {
        let Some(directory) = &self.directory else {
            return Ok(PortableCliEntryState::Missing);
        };
        let stat = match statat(
            directory,
            LINUX_CLI_EXECUTABLE_NAME,
            AtFlags::SYMLINK_NOFOLLOW,
        ) {
            Ok(stat) => stat,
            Err(Errno::NOENT) => return Ok(PortableCliEntryState::Missing),
            Err(error) => return Err(error.into()),
        };
        if stat.st_uid != geteuid().as_raw()
            || FileType::from_raw_mode(stat.st_mode) != FileType::Symlink
        {
            return Ok(PortableCliEntryState::Conflict);
        }
        let target = readlinkat(directory, LINUX_CLI_EXECUTABLE_NAME, Vec::new())
            .map_err(io::Error::from)?;
        Ok(
            if target.to_bytes() == self.launcher.as_os_str().as_bytes() {
                PortableCliEntryState::Owned
            } else {
                PortableCliEntryState::Conflict
            },
        )
    }

    /// Publishes without replacing any existing entry, including a dangling link.
    pub fn create(&self) -> io::Result<()> {
        let directory = self.directory.as_ref().ok_or(io::ErrorKind::NotFound)?;
        symlinkat(&self.launcher, directory, LINUX_CLI_EXECUTABLE_NAME).map_err(io::Error::from)?;
        fsync(directory).map_err(io::Error::from)
    }

    /// Removes only a link to this installation's fixed private launcher.
    pub fn remove(&self) -> io::Result<bool> {
        if self.state()? != PortableCliEntryState::Owned {
            return Ok(false);
        }
        let directory = self.directory.as_ref().ok_or(io::ErrorKind::NotFound)?;
        unlinkat(directory, LINUX_CLI_EXECUTABLE_NAME, AtFlags::empty())
            .map_err(io::Error::from)?;
        fsync(directory).map_err(io::Error::from)?;
        Ok(true)
    }
}

fn require_owned_directory(directory: impl AsFd) -> io::Result<()> {
    let stat = fstat(directory).map_err(io::Error::from)?;
    if stat.st_uid != geteuid().as_raw() || stat.st_mode & 0o022 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "untrusted portable CLI directory",
        ));
    }
    Ok(())
}

/// Serializes integration actions without taking the lock held by the running Core.
pub fn lock_linux_portable_cli(directory: &Path) -> io::Result<OwnedFd> {
    let directory = crate::unix::open_protected_directory_chain(directory)?;
    require_owned_directory(&directory)?;
    flock(&directory, FlockOperation::NonBlockingLockExclusive).map_err(io::Error::from)?;
    Ok(directory)
}

/// Resolves a portable CLI integration worker for the initialized AppImage's desktop.
/// The original image may be absent when removing the optional integration.
pub fn linux_portable_cli_bootstrap(
    entry_point: &Path,
    data: &DataConfig,
) -> io::Result<Option<PathBuf>> {
    let runtime = linux_portable_runtime(data)?;
    if entry_point != runtime.join(LINUX_GUI_EXECUTABLE_NAME)
        || !check_linux_core_registration(data, &runtime)?
    {
        return Ok(None);
    }
    crate::core_installation::validate_core_executable(entry_point)?;
    let bootstrap = runtime.join(LINUX_BOOTSTRAP_EXECUTABLE_NAME);
    crate::core_installation::validate_core_executable(&bootstrap)?;
    Ok(Some(bootstrap))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};

    #[test]
    fn entry_never_overwrites_or_removes_an_unowned_path() {
        let home = std::env::temp_dir().join(format!(
            "alcomd-portable-cli-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir(&home).expect("fixture home");
        let launcher = home.join("合成 安装").join(PORTABLE_CLI_LAUNCHER_NAME);
        let lookup = PortableCliEntry::in_home(&home, &launcher, false).expect("read-only lookup");
        assert_eq!(
            lookup.state().expect("absent"),
            PortableCliEntryState::Missing
        );
        assert!(!home.join(".local").exists());

        let entry = PortableCliEntry::in_home(&home, &launcher, true).expect("user bin");
        let path = home.join(".local/bin").join(LINUX_CLI_EXECUTABLE_NAME);
        fs::write(&path, b"third-party command").expect("foreign file");
        assert_eq!(
            entry.state().expect("file state"),
            PortableCliEntryState::Conflict
        );
        assert_eq!(
            entry.create().expect_err("preserve file").kind(),
            io::ErrorKind::AlreadyExists
        );
        assert!(!entry.remove().expect("preserve file"));
        assert_eq!(
            fs::read(&path).expect("foreign command"),
            b"third-party command"
        );
        fs::remove_file(&path).expect("remove fixture");
        symlink("/usr/bin/alcomd-cli", &path).expect("foreign symlink");
        assert!(!entry.remove().expect("preserve foreign link"));
        assert_eq!(
            fs::read_link(&path).expect("foreign symlink"),
            Path::new("/usr/bin/alcomd-cli")
        );
        fs::remove_file(&path).expect("remove fixture");

        entry.create().expect("create owned CLI entry");
        assert_eq!(
            entry.state().expect("owned link"),
            PortableCliEntryState::Owned
        );
        assert_eq!(fs::read_link(&path).expect("stable target"), launcher);
        assert!(entry.remove().expect("remove owned link"));
        assert!(!entry.remove().expect("idempotent removal"));
        fs::set_permissions(home.join(".local/bin"), fs::Permissions::from_mode(0o777))
            .expect("unsafe directory");
        assert!(PortableCliEntry::in_home(&home, &launcher, false).is_err());
        fs::remove_dir(home.join(".local/bin")).expect("remove bin fixture");
        symlink(&home, home.join(".local/bin")).expect("linked bin");
        assert!(PortableCliEntry::in_home(&home, &launcher, true).is_err());
        fs::remove_dir_all(home).expect("remove fixture home");
    }
}
