use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use rustix::fd::AsFd;
use rustix::fs::{
    AtFlags, FileType, Mode, OFlags, RenameFlags, fstat, fsync, openat, renameat_with, statat,
    unlinkat,
};
use rustix::io::Errno;
use rustix::process::geteuid;

use super::{
    CORE_EXECUTABLE_NAME, core_installation_registration, invalid_registration, open_registration,
    parse_runtime_directory, read_runtime_directory, validate_core_executable,
    validate_installation_record_parent,
};
use crate::{DataConfig, PortableImage, PortableOrigin};

pub const LINUX_BOOTSTRAP_EXECUTABLE_NAME: &str = env!("ALCOMD_BOOTSTRAP_NAME");
pub const LINUX_GUI_EXECUTABLE_NAME: &str = env!("ALCOMD_GUI_NAME");

fn package_runtime() -> PathBuf {
    package_runtime_root().join(env!("CARGO_PKG_VERSION"))
}

fn package_runtime_root() -> PathBuf {
    Path::new("/usr/lib")
        .join(CORE_EXECUTABLE_NAME)
        .join("runtime")
}

fn runtime_version<'a>(runtime: &'a Path, parent: &Path) -> Option<&'a OsStr> {
    if runtime.parent() != Some(parent) {
        return None;
    }
    let name = runtime.file_name()?;
    let version = name.to_str()?.parse::<semver::Version>().ok()?;
    (version.major == 4 && version.build.is_empty()).then_some(name)
}

fn runtime_version_is_absent(directory: impl AsFd, version: &OsStr) -> io::Result<bool> {
    match statat(directory, version, AtFlags::SYMLINK_NOFOLLOW) {
        Err(Errno::NOENT) => Ok(true),
        Ok(_) => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn retired_deb_runtime_is_absent(runtime: &Path) -> io::Result<bool> {
    let parent = package_runtime_root();
    let Some(version) = runtime_version(runtime, &parent) else {
        return Ok(false);
    };
    let directory = crate::unix::open_system_directory_chain(&parent)?;
    // A missing executable alone does not establish that dpkg removed the runtime.
    runtime_version_is_absent(&directory, version)
}

fn require_system_executable(path: &Path) -> io::Result<()> {
    let parent = path.parent().ok_or_else(invalid_registration)?;
    let directory = crate::unix::open_system_directory_chain(parent)?;
    let file = openat(
        &directory,
        path.file_name().ok_or_else(invalid_registration)?,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(io::Error::from)?;
    let stat = fstat(&file).map_err(io::Error::from)?;
    if stat.st_uid != 0
        || stat.st_nlink != 1
        || stat.st_mode & 0o6022 != 0
        || stat.st_mode & 0o001 == 0
        || FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "untrusted system installation executable",
        ));
    }
    Ok(())
}

/// Locates the current user's versioned AppImage runtime without creating it.
pub fn linux_portable_runtime(config: &DataConfig) -> io::Result<PathBuf> {
    Ok(portable_runtime_parent(config)?.join(env!("CARGO_PKG_VERSION")))
}

/// Resolves a signed transaction's canonical v4 runtime name in the portable data root.
pub fn linux_portable_runtime_for_version(
    config: &DataConfig,
    version: &str,
) -> io::Result<PathBuf> {
    let version = version
        .parse::<semver::Version>()
        .map_err(|_| invalid_registration())?;
    if version.major != 4 || !version.build.is_empty() {
        return Err(invalid_registration());
    }
    Ok(portable_runtime_parent(config)?.join(version.to_string()))
}

pub(super) fn is_portable_runtime_path(config: &DataConfig, runtime: &Path) -> io::Result<bool> {
    Ok(runtime.parent() == Some(portable_runtime_parent(config)?.as_path()))
}

/// Keeps per-account migration out of the system package manager's root process.
pub fn require_unprivileged_linux_installation() -> io::Result<()> {
    if geteuid().as_raw() == 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "run the migration under the account that owns the data",
        ));
    }
    Ok(())
}

fn portable_runtime_parent(config: &DataConfig) -> io::Result<PathBuf> {
    Ok(crate::unix::data_directory_path(config)?.join("runtime"))
}

/// Checks whether an existing runtime tree belongs to a current portable installation.
pub fn linux_portable_runtime_is_known(config: &DataConfig, entry: &Path) -> io::Result<bool> {
    let runtime = linux_portable_runtime(config)?;
    if runtime.parent() != Some(entry) {
        return Ok(false);
    }
    let directory = crate::unix::open_protected_directory_chain(entry)?;
    let stat = fstat(&directory).map_err(io::Error::from)?;
    if stat.st_uid != geteuid().as_raw() || stat.st_mode & 0o777 != 0o700 {
        return Err(invalid_registration());
    }
    validate_core_executable(&runtime.join(CORE_EXECUTABLE_NAME))?;
    PortableOrigin::read(&runtime)?;
    Ok(true)
}

/// Resolves the versioned runtime of a protected Linux package or AppImage installation.
pub fn linux_installation_runtime(executable: &Path, config: &DataConfig) -> io::Result<PathBuf> {
    let runtime = executable.parent().ok_or_else(invalid_registration)?;
    if runtime == package_runtime() {
        require_system_executable(executable)?;
        require_system_executable(&runtime.join(CORE_EXECUTABLE_NAME))?;
    } else if runtime == linux_portable_runtime(config)? {
        validate_core_executable(executable)?;
        validate_core_executable(&runtime.join(CORE_EXECUTABLE_NAME))?;
        PortableOrigin::read(runtime)?.revalidate()?;
    } else {
        return Err(invalid_registration());
    }
    Ok(runtime.to_path_buf())
}

/// Finds Bootstrap from the protected CLI or GUI installed by the Linux system package.
pub fn linux_installation_bootstrap(entry_point: &Path) -> io::Result<Option<PathBuf>> {
    let data = DataConfig::default();
    let package = package_runtime();
    let runtime = if is_deb_entry_point(entry_point, &package) {
        require_system_executable(entry_point)?;
        package
    } else if let Ok(portable) = linux_portable_runtime(&data)
        && entry_point == portable.join(LINUX_GUI_EXECUTABLE_NAME)
    {
        validate_core_executable(entry_point)?;
        portable
    } else {
        return Ok(None);
    };
    let bootstrap = runtime.join(LINUX_BOOTSTRAP_EXECUTABLE_NAME);
    linux_installation_runtime(&bootstrap, &data)?;
    Ok(Some(bootstrap))
}

/// Finds the update worker for an initialized AppImage's installed GUI, excluding system packages.
pub fn linux_portable_update_bootstrap(
    entry_point: &Path,
    config: &DataConfig,
) -> io::Result<Option<PathBuf>> {
    let runtime = linux_portable_runtime(config)?;
    if entry_point != runtime.join(LINUX_GUI_EXECUTABLE_NAME)
        || !check_linux_core_registration(config, &runtime)?
    {
        return Ok(None);
    }
    let origin = PortableOrigin::read(&runtime)?;
    origin.revalidate()?;
    validate_core_executable(entry_point)?;
    let bootstrap = runtime.join(LINUX_BOOTSTRAP_EXECUTABLE_NAME);
    validate_core_executable(&bootstrap)?;
    match PortableImage::open_for_update(origin.image_path()) {
        Ok(_) => Ok(Some(bootstrap)),
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => Ok(None),
        Err(error) => Err(error),
    }
}

fn is_deb_entry_point(entry_point: &Path, runtime: &Path) -> bool {
    entry_point == Path::new("/usr/bin").join(env!("ALCOMD_CLI_NAME"))
        || entry_point == runtime.join(LINUX_GUI_EXECUTABLE_NAME)
}

/// Finds the current DEB Bootstrap when an official entry sees a removed DEB registration.
pub fn linux_retired_deb_installation_bootstrap(
    entry_point: &Path,
    config: &DataConfig,
) -> io::Result<Option<PathBuf>> {
    let package = package_runtime();
    if !is_deb_entry_point(entry_point, &package)
        || !matches!(
            inspect_registration(config, &package)?,
            RegistrationState::RetiredDeb { .. }
        )
    {
        return Ok(None);
    }
    linux_installation_bootstrap(entry_point)
}

enum RegistrationState {
    Missing,
    Current,
    RetiredDeb { file: File, runtime: PathBuf },
}

fn inspect_registration(config: &DataConfig, runtime: &Path) -> io::Result<RegistrationState> {
    let registration = core_installation_registration(config)?;
    match open_registration(&registration) {
        Ok(file) => {
            let registered = read_runtime_directory(&file)?;
            if registered == runtime {
                validate_core_executable(&runtime.join(CORE_EXECUTABLE_NAME))?;
                return Ok(RegistrationState::Current);
            }
            if runtime == package_runtime() && retired_deb_runtime_is_absent(&registered)? {
                validate_installation_record_parent(&registration)?;
                return Ok(RegistrationState::RetiredDeb {
                    file,
                    runtime: registered,
                });
            }
            Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Core registration belongs to another installed runtime",
            ))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(RegistrationState::Missing),
        Err(error) => Err(error),
    }
}

/// Checks whether this runtime already owns the registration before Core initializes.
pub fn check_linux_core_registration(config: &DataConfig, runtime: &Path) -> io::Result<bool> {
    Ok(matches!(
        inspect_registration(config, runtime)?,
        RegistrationState::Current
    ))
}

/// Publishes the checked runtime after Core initializes under its instance lock.
pub fn publish_linux_core_registration(config: &DataConfig, runtime: &Path) -> io::Result<()> {
    linux_installation_runtime(&runtime.join(CORE_EXECUTABLE_NAME), config)?;
    let state = inspect_registration(config, runtime)?;
    if matches!(state, RegistrationState::Current) {
        return Ok(());
    }
    let previous = match state {
        RegistrationState::RetiredDeb { file, runtime } => Some((file, runtime)),
        _ => None,
    };
    publish_registration(config, runtime, previous)
}

fn portable_update_registration(
    config: &DataConfig,
    previous: &Path,
    selected: &Path,
) -> io::Result<(File, PathBuf)> {
    let parent = portable_runtime_parent(config)?;
    let version_at = |runtime: &Path| -> io::Result<semver::Version> {
        let name = runtime_version(runtime, &parent).ok_or_else(invalid_registration)?;
        let name = name.to_str().ok_or_else(invalid_registration)?;
        name.parse().map_err(|_| invalid_registration())
    };
    if !version_at(selected)?
        .cmp_precedence(&version_at(previous)?)
        .is_gt()
    {
        return Err(invalid_registration());
    }
    let registration = core_installation_registration(config)?;
    validate_installation_record_parent(&registration)?;
    let file = open_registration(&registration)?;
    let runtime = read_runtime_directory(&file)?;
    if runtime != previous && runtime != selected {
        return Err(invalid_registration());
    }
    validate_core_executable(&runtime.join(CORE_EXECUTABLE_NAME))?;
    Ok((file, runtime))
}

/// Ensures an interrupted update can only operate on its exact portable registration.
pub fn check_linux_portable_update_registration(
    config: &DataConfig,
    previous: &Path,
    selected: &Path,
) -> io::Result<bool> {
    portable_update_registration(config, previous, selected).map(|(_, runtime)| runtime == selected)
}

/// Activates a fully verified portable runtime while Bootstrap holds the Core instance lock.
pub fn publish_linux_portable_update_registration(
    config: &DataConfig,
    previous: &Path,
    selected: &Path,
) -> io::Result<()> {
    let (file, runtime) = portable_update_registration(config, previous, selected)?;
    let old_origin = PortableOrigin::read(previous)?;
    let new_origin = PortableOrigin::read(selected)?;
    if old_origin.image_path() != new_origin.image_path() {
        return Err(invalid_registration());
    }
    new_origin.revalidate()?;
    validate_core_executable(&selected.join(CORE_EXECUTABLE_NAME))?;
    if runtime == selected {
        return Ok(());
    }
    publish_registration(config, selected, Some((file, runtime)))
}

fn publish_registration(
    config: &DataConfig,
    runtime: &Path,
    previous: Option<(File, PathBuf)>,
) -> io::Result<()> {
    let registration = core_installation_registration(config)?;
    let parent = registration.parent().ok_or_else(invalid_registration)?;
    let name = registration.file_name().ok_or_else(invalid_registration)?;
    let directory = crate::unix::open_protected_directory_chain(parent)?;
    let stat = fstat(&directory).map_err(io::Error::from)?;
    if stat.st_uid != geteuid().as_raw() || stat.st_mode & 0o777 != 0o700 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Core registration directory is not private",
        ));
    }

    let line = format!("{}\n", runtime.to_str().ok_or_else(invalid_registration)?);
    parse_runtime_directory(line.as_bytes())?;
    static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let temporary = format!(
        ".core-installation-{}-{stamp}-{}",
        std::process::id(),
        NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed)
    );
    let mut file = File::from(
        openat(
            &directory,
            temporary.as_str(),
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )
        .map_err(io::Error::from)?,
    );
    let result = (|| {
        file.write_all(line.as_bytes())?;
        file.sync_all()?;
        let flags = if let Some((previous, previous_runtime)) = previous {
            if runtime == package_runtime() && !retired_deb_runtime_is_absent(&previous_runtime)? {
                return Err(invalid_registration());
            }
            let current = open_registration(&registration)?;
            let current_runtime = read_runtime_directory(&current)?;
            let previous = fstat(&previous).map_err(io::Error::from)?;
            let current = fstat(&current).map_err(io::Error::from)?;
            let anchored =
                statat(&directory, name, AtFlags::SYMLINK_NOFOLLOW).map_err(io::Error::from)?;
            if previous_runtime != current_runtime
                || (previous.st_dev, previous.st_ino) != (current.st_dev, current.st_ino)
                || (current.st_dev, current.st_ino) != (anchored.st_dev, anchored.st_ino)
            {
                return Err(invalid_registration());
            }
            RenameFlags::empty()
        } else {
            RenameFlags::NOREPLACE
        };
        renameat_with(&directory, temporary.as_str(), &directory, name, flags)
            .map_err(io::Error::from)?;
        fsync(&directory).map_err(io::Error::from)
    })();
    if result.is_err() {
        let _ = unlinkat(&directory, temporary.as_str(), AtFlags::empty());
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;

    #[test]
    fn recognizes_only_siblings_named_for_a_canonical_v4_release() {
        let parent = package_runtime_root();
        for name in ["4.0.0-alpha.0", "4.0.0-alpha.1", "4.10.1"] {
            assert_eq!(
                runtime_version(&parent.join(name), &parent),
                Some(OsStr::new(name))
            );
        }
        for name in ["local", "04.0.0", "3.4.0", "4.0.0+local", "4.0.1/elsewhere"] {
            assert_eq!(runtime_version(&parent.join(name), &parent), None);
        }
        assert_eq!(
            runtime_version(Path::new("/home/app/runtime/4.0.0-alpha.1"), &parent),
            None
        );
    }

    #[test]
    fn only_a_missing_directory_entry_can_be_retired() {
        let root =
            std::env::temp_dir().join(format!("alcomd-deb-retirement-{}", std::process::id()));
        fs::create_dir(&root).expect("private test tree");
        let directory = File::open(&root).expect("fixture directory");
        let version = OsStr::new("4.0.0-alpha.1");
        assert!(runtime_version_is_absent(&directory, version).expect("missing"));
        fs::create_dir(root.join(version)).expect("remaining runtime directory");
        let directory_was_absent =
            runtime_version_is_absent(&directory, version).expect("directory");
        fs::remove_dir(root.join(version)).expect("clear fixture directory");
        symlink("missing-target", root.join(version)).expect("dangling runtime link");
        let link_was_absent =
            runtime_version_is_absent(&directory, version).expect("no-follow stat");
        fs::remove_file(root.join(version)).expect("remove fixture link");
        fs::remove_dir(root).expect("remove fixture tree");
        assert!(!directory_was_absent);
        assert!(!link_was_absent);
    }
}
