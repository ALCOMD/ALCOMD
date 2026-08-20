//! Safe filesystem validation and spawning primitives for Unity Editor.

use std::ffi::OsString;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const MANIFEST_BYTE_LIMIT: u64 = 65_536;
const DISCOVERY_ENTRY_LIMIT: usize = 256;

/// Architecture reported without guessing the executable's binary format.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnityArchitecture {
    /// Architecture was not verified by the current adapter.
    Unknown,
}

/// Validated Unity Editor executable and metadata, expressed only in safe Rust types.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedUnityExecutable {
    executable_path: PathBuf,
    filesystem_identity: Vec<u8>,
    version_manifest: Vec<u8>,
    architecture: UnityArchitecture,
}

impl ValidatedUnityExecutable {
    #[must_use]
    pub fn executable_path(&self) -> &Path {
        &self.executable_path
    }

    #[must_use]
    pub fn filesystem_identity(&self) -> &[u8] {
        &self.filesystem_identity
    }

    #[must_use]
    pub fn version_manifest(&self) -> &[u8] {
        &self.version_manifest
    }

    #[must_use]
    pub const fn architecture(&self) -> UnityArchitecture {
        self.architecture
    }
}

/// Validates an actual executable and the adjacent Unity PackageManager Editor manifest.
pub fn validate_unity_executable(path: &Path) -> io::Result<ValidatedUnityExecutable> {
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "executable path must be absolute",
        ));
    }
    let executable_path = fs::canonicalize(path)?;
    let metadata = fs::metadata(&executable_path)?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Unity executable is not a file",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Unity executable is not executable",
            ));
        }
    }
    #[cfg(windows)]
    if !executable_path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Unity executable extension is invalid",
        ));
    }

    let manifest = locate_editor_manifest(&executable_path).ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "Unity version manifest is missing")
    })?;
    let manifest_metadata = fs::metadata(&manifest)?;
    if !manifest_metadata.is_file() || manifest_metadata.len() > MANIFEST_BYTE_LIMIT {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Unity version manifest is invalid",
        ));
    }
    let mut bytes = Vec::new();
    fs::File::open(&manifest)?
        .take(MANIFEST_BYTE_LIMIT + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MANIFEST_BYTE_LIMIT {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Unity version manifest is too large",
        ));
    }
    Ok(ValidatedUnityExecutable {
        filesystem_identity: crate::file_identity_key(&executable_path)?,
        executable_path,
        version_manifest: bytes,
        architecture: UnityArchitecture::Unknown,
    })
}

/// Returns validated candidates from bounded, conventional Unity Hub install roots.
pub fn discover_unity_executables() -> Vec<ValidatedUnityExecutable> {
    let mut installations = Vec::new();
    for root in known_install_roots() {
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        let mut entries = entries
            .filter_map(Result::ok)
            .take(DISCOVERY_ENTRY_LIMIT)
            .collect::<Vec<_>>();
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }
            let path = editor_executable(&entry.path());
            if let Ok(installation) = validate_unity_executable(&path)
                && !installations
                    .iter()
                    .any(|existing: &ValidatedUnityExecutable| {
                        existing.filesystem_identity == installation.filesystem_identity
                    })
            {
                installations.push(installation);
            }
        }
    }
    installations.sort_by(|left, right| left.executable_path.cmp(&right.executable_path));
    installations
}

/// Spawns one validated Unity executable with an exact project selector and argv array.
pub fn launch_unity_editor(
    executable: &Path,
    project_root: &Path,
    arguments: &[OsString],
) -> io::Result<()> {
    let mut command = Command::new(executable);
    command
        .arg("-projectPath")
        .arg(project_root)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let _ = command.spawn()?;
    Ok(())
}

fn locate_editor_manifest(executable: &Path) -> Option<PathBuf> {
    let parent = executable.parent()?;
    let windows_or_linux = parent.join("Data/Resources/PackageManager/Editor/manifest.json");
    if windows_or_linux.is_file() {
        return Some(windows_or_linux);
    }
    let contents = parent.parent()?;
    let macos = contents.join("Resources/PackageManager/Editor/manifest.json");
    macos.is_file().then_some(macos)
}

fn editor_executable(version_root: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    return version_root.join("Unity.app/Contents/MacOS/Unity");
    #[cfg(windows)]
    return version_root.join("Editor/Unity.exe");
    #[cfg(all(unix, not(target_os = "macos")))]
    return version_root.join("Editor/Unity");
}

fn known_install_roots() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("ProgramFiles")
            .map(PathBuf::from)
            .into_iter()
            .map(|path| path.join("Unity/Hub/Editor"))
            .collect()
    }
    #[cfg(target_os = "macos")]
    {
        vec![PathBuf::from("/Applications/Unity/Hub/Editor")]
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .into_iter()
            .map(|path| path.join("Unity/Hub/Editor"))
            .collect()
    }
}
