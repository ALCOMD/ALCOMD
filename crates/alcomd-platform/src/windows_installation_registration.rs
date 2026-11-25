use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use semver::Version;

use super::{
    CORE_EXECUTABLE_NAME, PRODUCT_PAYLOAD_MANIFEST_NAME, core_installation_registration,
    invalid_registration, open_registration, parse_runtime_directory, read_runtime_directory,
    validate_core_executable,
};
use crate::windows::core_installation::{
    open_installation_manifest, open_registration_for_removal, validate_registration_parent,
};
use crate::windows::{
    WindowsFileIdentity, delete_open_file, file_identity_from_file, lock_file_byte,
    replace_owned_file,
};
use crate::{
    BindError, DaemonInstance, DataConfig, IpcConfig, publish_path_no_replace,
    resolve_plain_storage_directory, sync_directory,
};

// The Inno uninstaller takes this byte exclusively. It is beyond the bounded
// signed manifest, so verification can still read the complete file.
const MANIFEST_LEASE_OFFSET: u64 = 1 << 32;
const MAX_VERSION_BYTES: usize = 80;
const MAX_INTENT_BYTES: usize = 8192;

/// Basename of the signed Windows Bootstrap executable.
pub const WINDOWS_BOOTSTRAP_EXECUTABLE_NAME: &str = concat!(env!("ALCOMD_BOOTSTRAP_NAME"), ".exe");

/// Basename of the Windows stable CLI entry point.
pub const WINDOWS_CLI_EXECUTABLE_NAME: &str = concat!(env!("ALCOMD_CLI_NAME"), ".exe");

/// Protected Windows installation-root record containing the active version and an LF.
pub const WINDOWS_ACTIVE_RUNTIME_RECORD: &str = "active-runtime-v1";

/// A durable update intent prevents either installed version from starting until recovery.
pub const WINDOWS_UPDATE_INTENT_RECORD: &str = "update-intent-v1";

/// Resolves the current user's standard Windows product root for maintenance.
pub fn windows_current_user_installation_root() -> io::Result<PathBuf> {
    let path = crate::local_app_data_directory()?
        .join("Programs")
        .join(env!("ALCOMD_INSTALL_DIRECTORY"));
    resolve_plain_storage_directory(&path).map(|(path, _)| path)
}

/// Holds a shared lease for a running installed Core. Scratch and sibling builds have no lease.
pub fn hold_windows_installation(executable: &Path) -> io::Result<Option<File>> {
    // Windows reports the spelling used at launch, which can include a short name or junction.
    let executable = fs::canonicalize(executable)?;
    if !executable
        .parent()
        .is_some_and(|runtime| runtime_root(runtime).is_ok())
    {
        return Ok(None);
    }
    let runtime = windows_installation_runtime(&executable)?;
    let manifest = lease_runtime(&runtime, false)?;
    require_active_runtime(&runtime)?;
    Ok(Some(manifest))
}

fn lease_runtime(runtime: &Path, exclusive: bool) -> io::Result<File> {
    let manifest = open_installation_manifest(&runtime.join(PRODUCT_PAYLOAD_MANIFEST_NAME))?;
    lock_file_byte(&manifest, MANIFEST_LEASE_OFFSET, exclusive)?;
    Ok(manifest)
}

/// Resolves a member of the current versioned Windows runtime through trusted paths.
pub fn windows_installation_runtime(executable: &Path) -> io::Result<PathBuf> {
    let parent = executable.parent().ok_or_else(invalid_registration)?;
    let (runtime, _) = resolve_plain_storage_directory(parent)?;
    if !is_versioned_runtime(&runtime) {
        return Err(invalid_registration());
    }
    validate_core_executable(&runtime.join(CORE_EXECUTABLE_NAME))?;
    Ok(runtime)
}

/// Finds Bootstrap for the official GUI or stable CLI in a protected Windows installation.
pub fn windows_installation_bootstrap(entry_point: &Path) -> io::Result<Option<PathBuf>> {
    let Some(directory) = entry_point.parent() else {
        return Ok(None);
    };
    let runtime = if has_name(entry_point, concat!(env!("ALCOMD_GUI_NAME"), ".exe"))
        && is_versioned_runtime(directory)
    {
        require_active_runtime(directory)?;
        directory.to_path_buf()
    } else if has_name(entry_point, WINDOWS_CLI_EXECUTABLE_NAME)
        && has_name(directory, "bin")
        && let Some(root) = directory.parent()
    {
        let versions = root.join("runtime");
        match fs::symlink_metadata(&versions) {
            Ok(_) => {
                let version = read_active_version(&open_registration(
                    &root.join(WINDOWS_ACTIVE_RUNTIME_RECORD),
                )?)?;
                versions.join(version.to_string())
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        }
    } else {
        return Ok(None);
    };
    // Validate the unnormalized paths so a reparse point cannot redirect either role.
    validate_core_executable(entry_point)?;
    let runtime = validate_runtime(&runtime)?;
    let bootstrap = runtime.join(WINDOWS_BOOTSTRAP_EXECUTABLE_NAME);
    validate_core_executable(&bootstrap)?;
    Ok(Some(bootstrap))
}

fn validate_runtime(runtime: &Path) -> io::Result<PathBuf> {
    validate_core_executable(&runtime.join(CORE_EXECUTABLE_NAME))?;
    resolve_plain_storage_directory(runtime).map(|(runtime, _)| runtime)
}

fn runtime_root(runtime: &Path) -> io::Result<&Path> {
    runtime
        .parent()
        .filter(|versions| has_name(versions, "runtime"))
        .and_then(Path::parent)
        .ok_or_else(invalid_registration)
}

fn require_active_runtime(runtime: &Path) -> io::Result<()> {
    let root = runtime_root(runtime)?;
    match fs::symlink_metadata(root.join(WINDOWS_UPDATE_INTENT_RECORD)) {
        Ok(_) => return Err(installation_not_ready()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let version = read_active_version(&open_registration(
        &root.join(WINDOWS_ACTIVE_RUNTIME_RECORD),
    )?)?;
    if runtime.file_name().and_then(|name| name.to_str()) != Some(version.to_string().as_str()) {
        return Err(installation_not_ready());
    }
    Ok(())
}

pub(super) fn check_registered_runtime_is_active(runtime: &Path) -> io::Result<()> {
    if runtime_root(runtime).is_ok() {
        require_active_runtime(runtime)?;
    }
    Ok(())
}

fn installation_not_ready() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "Windows product runtime is inactive or undergoing an update",
    )
}

fn parse_version(version: &str) -> io::Result<Version> {
    if !(5..=MAX_VERSION_BYTES).contains(&version.len()) {
        return Err(invalid_registration());
    }
    Version::parse(version)
        .ok()
        .filter(|parsed| parsed.build.is_empty() && parsed.to_string() == version)
        .ok_or_else(invalid_registration)
}

fn read_active_version(file: &File) -> io::Result<Version> {
    let mut bytes = Vec::new();
    file.take((MAX_VERSION_BYTES + 2) as u64)
        .read_to_end(&mut bytes)?;
    let version = bytes
        .strip_suffix(b"\n")
        .and_then(|version| std::str::from_utf8(version).ok())
        .ok_or_else(invalid_registration)?;
    parse_version(version)
}

fn is_versioned_runtime(directory: &Path) -> bool {
    has_name(directory, env!("CARGO_PKG_VERSION")) && runtime_root(directory).is_ok()
}

fn has_name(path: &Path, name: &str) -> bool {
    path.file_name()
        .and_then(|part| part.to_str())
        .is_some_and(|part| part.eq_ignore_ascii_case(name))
}

/// Holds both version leases and the current user's Core instance lock during activation.
/// Bootstrap is responsible for verifying the signed payloads and exact update intent.
pub struct WindowsInstallationUpdate {
    root: PathBuf,
    source: PathBuf,
    target: PathBuf,
    target_version: Version,
    _source_lease: File,
    _instance: DaemonInstance,
    target_lease: Option<File>,
}

impl WindowsInstallationUpdate {
    /// Starts or resumes a same-root update from the current user's installed Bootstrap.
    pub fn acquire(executable: &Path, target_version: &str) -> io::Result<Self> {
        Self::acquire_at(
            executable,
            target_version,
            windows_current_user_installation_root()?,
        )
    }

    fn acquire_at(executable: &Path, target_version: &str, root: PathBuf) -> io::Result<Self> {
        if !has_name(executable, WINDOWS_BOOTSTRAP_EXECUTABLE_NAME)
            || target_version.eq_ignore_ascii_case(env!("CARGO_PKG_VERSION"))
        {
            return Err(invalid_registration());
        }
        let target_version = parse_version(target_version)?;
        let source_version = parse_version(env!("CARGO_PKG_VERSION"))?;
        if !target_version.cmp_precedence(&source_version).is_gt() {
            return Err(invalid_registration());
        }
        validate_core_executable(executable)?;
        let source = windows_installation_runtime(executable)?;
        if runtime_root(&source)? != root {
            return Err(invalid_registration());
        }
        let source_lease = lease_runtime(&source, true)?;
        let active = read_active_version(&open_registration(
            &root.join(WINDOWS_ACTIVE_RUNTIME_RECORD),
        )?)?;
        if active == target_version {
            open_registration(&root.join(WINDOWS_UPDATE_INTENT_RECORD))?;
        } else if active != source_version {
            return Err(installation_not_ready());
        }
        let target = source
            .parent()
            .ok_or_else(invalid_registration)?
            .join(target_version.to_string());
        let instance =
            DaemonInstance::acquire(&IpcConfig::default()).map_err(|error| match error {
                BindError::AlreadyRunning => installation_not_ready(),
                BindError::Io(error) => error,
            })?;
        Ok(Self {
            root,
            source,
            target,
            target_version,
            _source_lease: source_lease,
            _instance: instance,
            target_lease: None,
        })
    }

    /// Returns the exact target directory, derived from the verified installer version.
    #[must_use]
    pub fn target_runtime(&self) -> &Path {
        &self.target
    }

    /// Acquires the target lease after setup finishes, before final payload verification.
    pub fn lock_target(&mut self) -> io::Result<()> {
        open_registration(&self.root.join(WINDOWS_UPDATE_INTENT_RECORD))?;
        if self.target_lease.is_some() {
            return Ok(());
        }
        if validate_runtime(&self.target)? != self.target {
            return Err(invalid_registration());
        }
        self.target_lease = Some(lease_runtime(&self.target, true)?);
        Ok(())
    }

    /// Replaces only this user's source registration. Uninitialized users stay unregistered.
    pub fn publish_core_registration(&self, config: &DataConfig) -> io::Result<()> {
        self.require_target_lease()?;
        open_registration(&self.root.join(WINDOWS_UPDATE_INTENT_RECORD))?;
        let path = core_installation_registration(config)?;
        let file = match open_registration(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };
        let registered = registered_runtime(&file)?;
        if registered == self.target {
            return sync_directory(path.parent().ok_or_else(invalid_registration)?);
        }
        if registered != self.source {
            return Err(other_registration_owner());
        }
        let identity = file_identity_from_file(&file)?;
        drop(file);
        let line = registration_line(&self.target)?;
        publish_owned_record(&path, line.as_bytes(), Some(identity))
    }

    /// Publishes the fully verified target as the sole active version under both leases.
    pub fn activate(&self) -> io::Result<()> {
        self.require_target_lease()?;
        open_registration(&self.root.join(WINDOWS_UPDATE_INTENT_RECORD))?;
        let path = self.root.join(WINDOWS_ACTIVE_RUNTIME_RECORD);
        let file = open_registration(&path)?;
        let active = read_active_version(&file)?;
        if active == self.target_version {
            return sync_directory(&self.root);
        }
        if active.to_string() != env!("CARGO_PKG_VERSION") {
            return Err(installation_not_ready());
        }
        let identity = file_identity_from_file(&file)?;
        drop(file);
        publish_owned_record(
            &path,
            format!("{}\n", self.target_version).as_bytes(),
            Some(identity),
        )
    }

    /// Ends maintenance by removing the exact update intent after the target is active.
    pub fn finish(&self, expected_intent: &[u8]) -> io::Result<()> {
        self.require_target_lease()?;
        if expected_intent.is_empty() || expected_intent.len() > MAX_INTENT_BYTES {
            return Err(invalid_registration());
        }
        if read_active_version(&open_registration(
            &self.root.join(WINDOWS_ACTIVE_RUNTIME_RECORD),
        )?)? != self.target_version
        {
            return Err(installation_not_ready());
        }
        let intent = self.root.join(WINDOWS_UPDATE_INTENT_RECORD);
        let file = match open_registration_for_removal(&intent) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return sync_directory(&self.root);
            }
            Err(error) => return Err(error),
        };
        let mut bytes = Vec::new();
        (&file)
            .take((MAX_INTENT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes != expected_intent {
            return Err(invalid_registration());
        }
        delete_open_file(file)?;
        sync_directory(&self.root)
    }

    fn require_target_lease(&self) -> io::Result<()> {
        if self.target_lease.is_none() {
            return Err(installation_not_ready());
        }
        Ok(())
    }
}

/// Checks whether the selected runtime already owns this user's registration.
/// A different, stale, or malformed registration blocks an ordinary install.
pub fn check_windows_core_registration(config: &DataConfig, runtime: &Path) -> io::Result<bool> {
    let registration = core_installation_registration(config)?;
    match open_registration(&registration) {
        Ok(file) => {
            require_owner(&file, runtime)?;
            Ok(true)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

/// Publishes a registration after Core has completed startup checks under its instance lock.
pub fn publish_windows_core_registration(config: &DataConfig, runtime: &Path) -> io::Result<()> {
    validate_core_executable(&runtime.join(CORE_EXECUTABLE_NAME))?;
    let registration = core_installation_registration(config)?;
    if check_windows_core_registration(config, runtime)? {
        return Ok(());
    }
    let line = registration_line(runtime)?;
    publish_owned_record(&registration, line.as_bytes(), None)
}

fn registration_line(runtime: &Path) -> io::Result<String> {
    let line = format!("{}\n", runtime.to_str().ok_or_else(invalid_registration)?);
    parse_runtime_directory(line.as_bytes())?;
    Ok(line)
}

fn publish_owned_record(
    registration: &Path,
    content: &[u8],
    previous: Option<WindowsFileIdentity>,
) -> io::Result<()> {
    validate_registration_parent(registration)?;
    let parent = registration.parent().ok_or_else(invalid_registration)?;
    let (_, parent_identity) = resolve_plain_storage_directory(parent)?;
    static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let temporary = registration.with_extension(format!(
        "new-{}-{stamp}-{}",
        std::process::id(),
        NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let identity = file_identity_from_file(&file)?;
    let result = (|| {
        file.write_all(content)?;
        file.sync_all()?;
        drop(file);
        let checked = open_registration(&temporary)?;
        let mut observed = Vec::new();
        (&checked)
            .take((content.len() + 1) as u64)
            .read_to_end(&mut observed)?;
        if file_identity_from_file(&checked)? != identity || observed != content {
            return Err(invalid_registration());
        }
        drop(checked);
        if resolve_plain_storage_directory(parent)?.1 != parent_identity {
            return Err(invalid_registration());
        }
        if let Some(previous) = previous {
            let old = open_registration(registration)?;
            if file_identity_from_file(&old)? != previous {
                return Err(invalid_registration());
            }
            // No other installation writer can publish while the caller owns the leases.
            drop(old);
            replace_owned_file(&temporary, registration)?;
        } else {
            publish_path_no_replace(&temporary, registration)?;
        }
        sync_directory(parent)
    })();
    if result.is_err()
        && let Ok(file) = open_registration_for_removal(&temporary)
        && file_identity_from_file(&file).is_ok_and(|current| current == identity)
    {
        let _ = delete_open_file(file);
    }
    result
}

/// Removes only the selected runtime's registration, using the validated file handle.
pub fn remove_windows_core_registration(config: &DataConfig, runtime: &Path) -> io::Result<()> {
    let registration = core_installation_registration(config)?;
    let file = match open_registration_for_removal(&registration) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    require_owner(&file, runtime)?;
    delete_open_file(file)?;
    sync_directory(registration.parent().ok_or_else(invalid_registration)?)
}

fn require_owner(file: &File, runtime: &Path) -> io::Result<()> {
    if registered_runtime(file)? != runtime {
        return Err(other_registration_owner());
    }
    Ok(())
}

fn registered_runtime(file: &File) -> io::Result<PathBuf> {
    let registered = read_runtime_directory(file)?;
    validate_core_executable(&registered.join(CORE_EXECUTABLE_NAME))?;
    fs::canonicalize(registered)
}

fn other_registration_owner() -> io::Error {
    io::Error::new(
        io::ErrorKind::AlreadyExists,
        "Core registration belongs to another installed runtime",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn installed_core_lease_excludes_maintenance_without_locking_manifest_contents()
    -> io::Result<()> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time is after the Unix epoch")
            .as_nanos();
        let root = crate::local_app_data_directory()?
            .join(format!("alcomd-lease-{}-{stamp}", std::process::id()));
        fs::create_dir(&root)?;
        let runtime = root.join("runtime").join(env!("CARGO_PKG_VERSION"));
        fs::create_dir_all(&runtime)?;
        let executable = runtime.join(CORE_EXECUTABLE_NAME);
        fs::write(&executable, [])?;
        assert!(hold_windows_installation(&executable).is_err());
        let sibling = root.join(CORE_EXECUTABLE_NAME);
        fs::write(&sibling, [])?;
        assert!(hold_windows_installation(&sibling)?.is_none());

        let path = runtime.join(PRODUCT_PAYLOAD_MANIFEST_NAME);
        fs::write(&path, b"manifest contents")?;
        assert!(hold_windows_installation(&executable).is_err());
        fs::write(
            root.join(WINDOWS_ACTIVE_RUNTIME_RECORD),
            concat!(env!("CARGO_PKG_VERSION"), "\r\n"),
        )?;
        assert!(hold_windows_installation(&executable).is_err());
        fs::write(
            root.join(WINDOWS_ACTIVE_RUNTIME_RECORD),
            concat!(env!("CARGO_PKG_VERSION"), "\n"),
        )?;
        let first = hold_windows_installation(&executable)?.expect("first shared lease");
        let second = hold_windows_installation(&executable)?.expect("second shared lease");
        let maintenance = open_installation_manifest(&path)?;
        assert!(lock_file_byte(&maintenance, MANIFEST_LEASE_OFFSET, true).is_err());
        drop(first);
        assert!(lock_file_byte(&maintenance, MANIFEST_LEASE_OFFSET, true).is_err());
        drop(second);
        lock_file_byte(&maintenance, MANIFEST_LEASE_OFFSET, true)?;
        assert_eq!(fs::read(&path)?, b"manifest contents");
        assert!(hold_windows_installation(&executable).is_err());
        drop(maintenance);
        assert!(hold_windows_installation(&executable)?.is_some());
        fs::remove_dir_all(root)
    }

    #[test]
    fn same_root_update_requires_matching_registration_and_keeps_both_versions_paused()
    -> io::Result<()> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time is after the Unix epoch")
            .as_nanos();
        let root = crate::local_app_data_directory()?
            .join(format!("alcomd-activation-{}-{stamp}", std::process::id()));
        fs::create_dir(&root)?;
        let root = fs::canonicalize(root)?;
        let versions = root.join("runtime");
        let current = parse_version(env!("CARGO_PKG_VERSION"))?;
        let next = Version::new(current.major + 1, 0, 0).to_string();
        let source = versions.join(env!("CARGO_PKG_VERSION"));
        let target = versions.join(&next);
        let unrelated = root.join("unrelated");
        let data = root.join("data");
        for runtime in [&source, &target, &unrelated] {
            fs::create_dir_all(runtime)?;
            fs::write(runtime.join(CORE_EXECUTABLE_NAME), [])?;
            fs::write(runtime.join(PRODUCT_PAYLOAD_MANIFEST_NAME), [])?;
        }
        fs::create_dir(&data)?;
        let config = DataConfig::isolated(data);
        let cold_config = DataConfig::isolated(root.join("uninitialized"));
        let registration = core_installation_registration(&config)?;
        let active = root.join(WINDOWS_ACTIVE_RUNTIME_RECORD);
        let intent = root.join(WINDOWS_UPDATE_INTENT_RECORD);
        fs::write(&active, concat!(env!("CARGO_PKG_VERSION"), "\n"))?;
        let bootstrap = source.join(WINDOWS_BOOTSTRAP_EXECUTABLE_NAME);
        fs::write(&bootstrap, [])?;

        let running = hold_windows_installation(&source.join(CORE_EXECUTABLE_NAME))?
            .expect("source initially active");
        assert!(WindowsInstallationUpdate::acquire_at(&bootstrap, &next, root.clone()).is_err());
        drop(running);
        assert!(
            WindowsInstallationUpdate::acquire_at(&bootstrap, "../other", root.clone()).is_err()
        );
        assert!(require_active_runtime(&target).is_err());

        let mut update = WindowsInstallationUpdate::acquire_at(&bootstrap, &next, root.clone())?;
        assert_eq!(update.target_runtime(), target);
        assert!(update.lock_target().is_err());
        fs::write(&intent, b"exact sealed update intent")?;
        update.lock_target()?;
        assert!(update.finish(b"exact sealed update intent").is_err());
        assert!(require_active_runtime(&source).is_err());
        assert!(require_active_runtime(&target).is_err());
        update.publish_core_registration(&cold_config)?;
        assert!(!core_installation_registration(&cold_config)?.exists());

        publish_windows_core_registration(&config, &unrelated)?;
        assert!(update.publish_core_registration(&config).is_err());
        assert_eq!(
            fs::read_to_string(&registration)?,
            registration_line(&unrelated)?
        );
        remove_windows_core_registration(&config, &unrelated)?;
        publish_windows_core_registration(&config, &source)?;
        update.publish_core_registration(&config)?;
        drop(update);
        assert!(crate::registered_core_executable(&config).is_err());
        assert!(hold_windows_installation(&source.join(CORE_EXECUTABLE_NAME)).is_err());

        let mut update = WindowsInstallationUpdate::acquire_at(&bootstrap, &next, root.clone())?;
        update.lock_target()?;
        update.publish_core_registration(&config)?;
        update.activate()?;
        update.activate()?;
        drop(update);

        assert_eq!(fs::read_to_string(&active)?, format!("{next}\n"));
        assert!(crate::registered_core_executable(&config).is_err());
        let mut update = WindowsInstallationUpdate::acquire_at(&bootstrap, &next, root.clone())?;
        update.lock_target()?;
        assert!(update.finish(b"different sealed update intent").is_err());
        update.finish(b"exact sealed update intent")?;
        update.finish(b"exact sealed update intent")?;
        drop(update);
        assert!(!intent.exists());
        assert_eq!(
            crate::registered_core_executable(&config)?,
            Some(target.join(CORE_EXECUTABLE_NAME))
        );
        assert!(hold_windows_installation(&source.join(CORE_EXECUTABLE_NAME)).is_err());
        assert!(WindowsInstallationUpdate::acquire_at(&bootstrap, &next, root.clone()).is_err());
        fs::create_dir(&intent)?;
        assert!(crate::registered_core_executable(&config).is_err());
        fs::remove_dir_all(root)
    }
}
