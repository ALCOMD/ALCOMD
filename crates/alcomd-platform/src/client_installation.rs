//! Installer-owned records. Reading the catalogue never starts a program or creates a directory.

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{DataConfig, core_installation, file_identity_key};

pub const DIRECTORY: &str = "clients.d";
const RECORD_BYTES: u64 = 16 * 1024;
const DIRECTORY_LIMIT: usize = 512;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum ClientKind {
    Gui,
    Cli,
    Mcp,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum InstallScope {
    User,
    Machine,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientInstallation {
    pub schema: u32,
    pub program_id: String,
    pub kind: ClientKind,
    pub display_name: String,
    pub product_version: String,
    pub install_root: PathBuf,
    pub executable: PathBuf,
}

#[derive(Clone, Debug)]
pub struct InstalledClient {
    pub record: ClientInstallation,
    pub scope: InstallScope,
    pub executable_identity: Vec<u8>,
}

#[derive(Default)]
pub struct Catalogue {
    pub items: Vec<InstalledClient>,
    pub incomplete: bool,
}

pub fn valid_program_id(value: &str) -> bool {
    value.len() <= 128
        && value.contains('.')
        && value.split('.').all(|segment| {
            !segment.is_empty()
                && segment.starts_with(|c: char| c.is_ascii_lowercase())
                && !segment.ends_with('-')
                && segment
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}

fn official(kind: ClientKind) -> (&'static str, &'static str) {
    match kind {
        ClientKind::Gui => (
            concat!(env!("ALCOMD_BUNDLE_ID"), ".gui"),
            env!("ALCOMD_GUI_NAME"),
        ),
        ClientKind::Cli => (
            concat!(env!("ALCOMD_BUNDLE_ID"), ".cli"),
            env!("ALCOMD_CLI_NAME"),
        ),
        ClientKind::Mcp => (
            concat!(env!("ALCOMD_BUNDLE_ID"), ".mcp"),
            env!("ALCOMD_MCP_NAME"),
        ),
    }
}

fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid client installation record",
    )
}

impl ClientInstallation {
    pub fn official_actor(&self) -> Option<&'static str> {
        (official(self.kind).0 == self.program_id).then_some(match self.kind {
            ClientKind::Gui => "GUI",
            ClientKind::Cli => "CLI",
            ClientKind::Mcp => "MCP",
        })
    }
    fn validate(&self, core_path: Option<&Path>) -> io::Result<Vec<u8>> {
        let plain = |s: &str, limit| {
            !s.trim().is_empty() && s.len() <= limit && !s.chars().any(char::is_control)
        };
        if self.schema != 1
            || !valid_program_id(&self.program_id)
            || !plain(&self.display_name, 128)
            || !plain(&self.product_version, 64)
            || !self.install_root.is_absolute()
            || self
                .install_root
                .components()
                .any(|c| matches!(c, Component::CurDir | Component::ParentDir))
            || self.executable.as_os_str().is_empty()
            || self
                .executable
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err(invalid());
        }
        let executable = self.install_root.join(&self.executable);
        core_installation::validate_core_executable(&executable)?;
        let key = file_identity_key(&executable)?;
        if self.program_id == env!("ALCOMD_BUNDLE_ID")
            || self
                .program_id
                .starts_with(concat!(env!("ALCOMD_BUNDLE_ID"), "."))
        {
            let (id, name) = official(self.kind);
            let file = format!("{name}{}", std::env::consts::EXE_SUFFIX);
            let core_path = core_path.ok_or_else(invalid)?;
            let core_dir = core_path.parent().ok_or_else(invalid)?;
            let mut candidates = vec![core_dir.join(&file)];
            #[cfg(windows)]
            if self.kind == ClientKind::Cli
                && let Ok(runtime) = crate::windows_installation_runtime(core_path)
                && let Some(root) = runtime.parent().and_then(Path::parent)
            {
                candidates.push(root.join("bin").join(&file));
            }
            #[cfg(target_os = "macos")]
            if core_dir.ends_with("Helpers")
                && self.kind == ClientKind::Gui
                && let Some(contents) = core_dir.parent().filter(|dir| dir.ends_with("Contents"))
            {
                candidates.push(contents.join("MacOS").join(&file));
            }
            #[cfg(target_os = "linux")]
            if self.kind == ClientKind::Cli && core_dir.starts_with("/usr/lib") {
                candidates.push(Path::new("/usr/bin").join(&file));
            }
            if id != self.program_id
                || !candidates
                    .iter()
                    .any(|path| file_identity_key(path).is_ok_and(|other| other == key))
            {
                return Err(invalid());
            }
        } else if self.kind != ClientKind::Gui {
            return Err(invalid());
        }
        Ok(key)
    }
}

pub fn directories(data: &DataConfig) -> io::Result<Vec<(InstallScope, PathBuf)>> {
    let state = crate::state_database_location(data)?;
    let mut roots = vec![(InstallScope::User, state.with_file_name(DIRECTORY))];
    if data.data_directory().is_none() {
        #[cfg(windows)]
        let machine =
            vec![crate::windows::program_data_directory()?.join(env!("ALCOMD_PRODUCT_DIRECTORY"))];
        #[cfg(target_os = "macos")]
        let machine =
            vec![Path::new("/Library/Application Support").join(env!("ALCOMD_PRODUCT_DIRECTORY"))];
        #[cfg(target_os = "linux")]
        let machine = ["/usr/local/share", "/usr/share"]
            .map(|dir| Path::new(dir).join(env!("ALCOMD_DAEMON_NAME")));
        roots.extend(
            machine
                .into_iter()
                .map(|root| (InstallScope::Machine, root.join(DIRECTORY))),
        );
    }
    Ok(roots)
}

fn read_record(path: &Path, scope: InstallScope) -> io::Result<ClientInstallation> {
    #[cfg(unix)]
    let file = std::fs::File::from(core_installation::open_protected_file(
        path,
        false,
        scope == InstallScope::Machine,
    )?);
    #[cfg(windows)]
    let file = {
        let _ = scope;
        core_installation::open_registration(path)?
    };
    let mut data = Vec::new();
    file.take(RECORD_BYTES + 1).read_to_end(&mut data)?;
    if data.len() as u64 > RECORD_BYTES {
        return Err(invalid());
    }
    let record: ClientInstallation = serde_json::from_slice(&data).map_err(|_| invalid())?;
    if path.file_stem() != Some(std::ffi::OsStr::new(&record.program_id)) {
        return Err(invalid());
    }
    Ok(record)
}

pub fn scan(data: &DataConfig) -> Catalogue {
    let mut result = Catalogue::default();
    let Ok(roots) = directories(data) else {
        result.incomplete = true;
        return result;
    };
    let core = std::env::current_exe()
        .ok()
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name == core_installation::CORE_EXECUTABLE_NAME)
        })
        .or_else(|| crate::registered_core_executable(data).ok().flatten());
    for (scope, root) in roots {
        let directory = match fs::read_dir(&root) {
            Ok(dir) => dir,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(_) => {
                result.incomplete = true;
                continue;
            }
        };
        for (offset, entry) in directory.enumerate() {
            if offset == DIRECTORY_LIMIT {
                result.incomplete = true;
                break;
            }
            let Ok(entry) = entry else {
                result.incomplete = true;
                continue;
            };
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "json") {
                continue;
            }
            match read_record(&path, scope).and_then(|record| {
                let executable_identity = record.validate(core.as_deref())?;
                Ok(InstalledClient {
                    record,
                    scope,
                    executable_identity,
                })
            }) {
                Ok(item) => result.items.push(item),
                Err(_) => result.incomplete = true,
            }
        }
    }
    result.items.sort_by(|a, b| {
        (&a.record.program_id, a.scope, &a.record.product_version).cmp(&(
            &b.record.program_id,
            b.scope,
            &b.record.product_version,
        ))
    });
    result
}

fn write_directory(data: &DataConfig, scope: InstallScope) -> io::Result<PathBuf> {
    directories(data)?
        .into_iter()
        .find(|(candidate, _)| *candidate == scope)
        .map(|(_, root)| root)
        .ok_or_else(invalid)
}

fn installer_lock(root: &Path) -> io::Result<fs::File> {
    let path = root.join(".installer.lock");
    let lock = match fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .read(true)
        .open(&path)
    {
        Ok(lock) => lock,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let (lock, _) = crate::open_plain_file(&path)?;
            lock
        }
        Err(error) => return Err(error),
    };
    lock.lock()?;
    Ok(lock)
}

/// Called by the installer after the payload is in its final directory. Core reads this record.
pub fn publish(
    data: &DataConfig,
    scope: InstallScope,
    record: &ClientInstallation,
    core_executable: Option<&Path>,
) -> io::Result<()> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQUENCE: AtomicU64 = AtomicU64::new(1);
    record.validate(core_executable)?;
    let bytes = serde_json::to_vec_pretty(record).map_err(io::Error::other)?;
    if bytes.len() as u64 > RECORD_BYTES {
        return Err(invalid());
    }
    let root = write_directory(data, scope)?;
    crate::prepare_storage_directory(&root)?;
    let _lock = installer_lock(&root)?;
    let destination = root.join(format!("{}.json", record.program_id));
    let stage = root.join(format!(
        ".client-{}-{}.tmp",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(if scope == InstallScope::Machine {
            0o644
        } else {
            0o600
        });
    }
    let mut file = options.open(&stage)?;
    let result = (|| {
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&stage, &destination)?;
        crate::sync_directory(&root)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&stage);
    }
    result
}

/// An uninstall for an old location must not remove the replacement's record.
pub fn unpublish(
    data: &DataConfig,
    scope: InstallScope,
    record: &ClientInstallation,
) -> io::Result<bool> {
    if !valid_program_id(&record.program_id) {
        return Err(invalid());
    }
    let root = write_directory(data, scope)?;
    if !root.try_exists()? {
        return Ok(false);
    }
    let _lock = installer_lock(&root)?;
    let path = root.join(format!("{}.json", record.program_id));
    let current = match read_record(&path, scope) {
        Ok(current) => current,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if current.install_root != record.install_root
        || current.executable != record.executable
        || current.product_version != record.product_version
    {
        return Ok(false);
    }
    fs::remove_file(path)?;
    crate::sync_directory(&root)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn stage(path: &Path, executable: bool) {
        fs::write(path, b"isolated fixture image").expect("isolated installer fixture");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                path,
                fs::Permissions::from_mode(if executable { 0o700 } else { 0o600 }),
            )
            .expect("isolated installer fixture");
        }
        #[cfg(windows)]
        let _ = executable;
    }

    #[test]
    fn installer_lists_official_programs_and_independent_gui_before_they_run() {
        #[cfg(windows)]
        let parent = crate::local_app_data_directory().expect("isolated installer fixture");
        #[cfg(unix)]
        let parent = std::env::temp_dir()
            .canonicalize()
            .expect("isolated installer fixture");
        let root = parent.join(format!(
            "alcomd-client-registry-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("isolated installer fixture")
                .as_nanos()
        ));
        let runtime = root.join("runtime").join(env!("CARGO_PKG_VERSION"));
        let bin = root.join("bin");
        #[cfg(windows)]
        let cli_dir = &bin;
        #[cfg(not(windows))]
        let cli_dir = &runtime;
        let data = DataConfig::isolated(root.join("data"));
        for dir in [&root, &runtime, &bin, &root.join("data")] {
            crate::prepare_storage_directory(dir).expect("isolated installer fixture");
        }
        let core = runtime.join(core_installation::CORE_EXECUTABLE_NAME);
        stage(&core, true);
        let registration =
            crate::core_installation_registration(&data).expect("isolated installer fixture");
        stage(&registration, false);
        fs::write(&registration, format!("{}\n", runtime.display()))
            .expect("isolated installer fixture");
        let mut records = Vec::new();
        for (kind, dir) in [
            (ClientKind::Gui, &runtime),
            (ClientKind::Cli, cli_dir),
            (ClientKind::Mcp, &runtime),
        ] {
            let (id, name) = official(kind);
            let file = format!("{name}{}", std::env::consts::EXE_SUFFIX);
            stage(&dir.join(&file), true);
            let rec = ClientInstallation {
                schema: 1,
                program_id: id.into(),
                kind,
                display_name: name.into(),
                product_version: env!("CARGO_PKG_VERSION").into(),
                install_root: dir.clone(),
                executable: file.into(),
            };
            publish(&data, InstallScope::User, &rec, Some(&core))
                .expect("isolated installer fixture");
            records.push(rec);
        }
        let file = format!("independent{}", std::env::consts::EXE_SUFFIX);
        stage(&bin.join(&file), true);
        let independent = ClientInstallation {
            schema: 1,
            program_id: "org.example.gui".into(),
            kind: ClientKind::Gui,
            display_name: "Independent".into(),
            product_version: "1.0".into(),
            install_root: bin.clone(),
            executable: file.into(),
        };
        publish(&data, InstallScope::User, &independent, Some(&core))
            .expect("isolated installer fixture");
        let catalogue = scan(&data);
        assert!(!catalogue.incomplete);
        assert_eq!(catalogue.items.len(), 4);
        #[cfg(windows)]
        {
            let mut unrelated_cli = records[1].clone();
            unrelated_cli.install_root = root.clone();
            stage(&root.join(&unrelated_cli.executable), true);
            assert!(publish(&data, InstallScope::User, &unrelated_cli, Some(&core)).is_err());
        }
        let mut forged = independent.clone();
        forged.program_id = official(ClientKind::Gui).0.into();
        assert!(publish(&data, InstallScope::User, &forged, Some(&core)).is_err());
        let catalog =
            write_directory(&data, InstallScope::User).expect("isolated installer fixture");
        stage(&catalog.join("org.example.invalid.json"), false);
        let catalogue = scan(&data);
        assert!(catalogue.incomplete);
        assert_eq!(catalogue.items.len(), 4);
        let mut newer = independent.clone();
        newer.product_version = "1.1".into();
        publish(&data, InstallScope::User, &newer, Some(&core))
            .expect("isolated installer fixture");
        assert!(
            !unpublish(&data, InstallScope::User, &independent)
                .expect("isolated installer fixture")
        );
        for rec in records.iter().chain(std::iter::once(&newer)) {
            assert!(unpublish(&data, InstallScope::User, rec).expect("isolated installer fixture"));
        }
        fs::remove_dir_all(root).expect("isolated installer fixture");
    }
}
