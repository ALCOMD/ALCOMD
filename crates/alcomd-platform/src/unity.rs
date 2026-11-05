//! Safe filesystem validation and spawning primitives for Unity Editor.

use std::ffi::OsString;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const MANIFEST_BYTE_LIMIT: u64 = 65_536;
const DISCOVERY_ENTRY_LIMIT: usize = 256;

#[cfg(windows)]
#[path = "windows_unity_version.rs"]
mod windows_version;

#[cfg(unix)]
#[path = "unity_version_probe.rs"]
mod version_probe;

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
    #[cfg(not(windows))]
    probed_version: Option<String>,
    #[cfg(windows)]
    product_version: String,
    architecture: UnityArchitecture,
}

/// One migration-owned Unity process. The child handle never leaves this safe adapter.
pub struct UnityMigrationProcess {
    child: std::process::Child,
}

/// Bounded migration exit classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnityMigrationExit {
    /// Unity exited with a successful status.
    Success,
    /// Unity exited with a non-zero status.
    NonZero,
}

impl UnityMigrationProcess {
    /// Waits for the owned Unity child and consumes the only process handle.
    pub fn wait(mut self) -> io::Result<UnityMigrationExit> {
        self.child.wait().map(|status| {
            if status.success() {
                UnityMigrationExit::Success
            } else {
                UnityMigrationExit::NonZero
            }
        })
    }
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

    #[cfg(not(windows))]
    #[must_use]
    pub fn probed_version(&self) -> Option<&str> {
        self.probed_version.as_deref()
    }

    #[cfg(windows)]
    #[must_use]
    pub fn product_version(&self) -> &str {
        &self.product_version
    }

    #[must_use]
    pub const fn architecture(&self) -> UnityArchitecture {
        self.architecture
    }
}

/// Validates the Editor layout and executable metadata without executing the Editor.
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
    let filesystem_identity = crate::file_identity_key(&executable_path)?;
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
    #[cfg(windows)]
    let product_version =
        windows_version::read_unity_product_version(&executable_path).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unity Editor version resource could not be read: {error}"),
            )
        })?;
    if crate::file_identity_key(&executable_path)? != filesystem_identity {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Unity executable identity changed during inspection",
        ));
    }
    Ok(ValidatedUnityExecutable {
        filesystem_identity,
        executable_path,
        #[cfg(not(windows))]
        probed_version: None,
        #[cfg(windows)]
        product_version,
        architecture: UnityArchitecture::Unknown,
    })
}

/// Explicitly inspects a registered/discovered installation. Unix version
/// discovery executes only `-version` in an independent empty working directory;
/// ordinary layout/identity validation above never executes an Editor.
pub fn inspect_unity_installation(path: &Path) -> io::Result<ValidatedUnityExecutable> {
    #[allow(unused_mut)]
    let mut executable = validate_unity_executable(path)?;
    #[cfg(unix)]
    {
        let result = version_probe::probe(executable.executable_path());
        if crate::file_identity_key(executable.executable_path())? != executable.filesystem_identity
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Unity executable identity changed during version probe",
            ));
        }
        executable.probed_version = Some(result?);
    }
    Ok(executable)
}

/// Proves that a Windows process executable belongs to a different product.
/// Missing or malformed metadata is an error, never proof that an Editor is absent.
/// The metadata must remain bound to the caller's observed filesystem identity.
#[cfg(windows)]
pub fn is_verified_non_editor_executable(
    path: &Path,
    expected_identity: &[u8],
) -> io::Result<bool> {
    if crate::file_identity_key(path)?.as_slice() != expected_identity {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Process executable identity changed",
        ));
    }
    let is_editor = windows_version::is_unity_editor_product(path)?;
    if crate::file_identity_key(path)?.as_slice() != expected_identity {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Process executable identity changed",
        ));
    }
    Ok(!is_editor)
}

/// Returns validated candidates from bounded, conventional Unity Hub install roots.
/// An existing candidate that cannot be verified is an error, never an empty success.
pub fn discover_unity_executables() -> io::Result<Vec<ValidatedUnityExecutable>> {
    discover_from_roots(known_install_roots())
}

fn discover_from_roots(roots: Vec<PathBuf>) -> io::Result<Vec<ValidatedUnityExecutable>> {
    let mut installations = Vec::new();
    for root in roots {
        let entries = match fs::read_dir(&root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(discovery_error(&root, error)),
        };
        let mut entries = entries
            .take(DISCOVERY_ENTRY_LIMIT)
            .collect::<io::Result<Vec<_>>>()
            .map_err(|error| discovery_error(&root, error))?;
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let file_type = entry
                .file_type()
                .map_err(|error| discovery_error(&entry.path(), error))?;
            if file_type.is_symlink() || !file_type.is_dir() {
                continue;
            }
            let path = editor_executable(&entry.path());
            match fs::symlink_metadata(&path) {
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(discovery_error(&path, error)),
            }
            let installation =
                inspect_unity_installation(&path).map_err(|error| discovery_error(&path, error))?;
            if !installations
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
    Ok(installations)
}

fn discovery_error(path: &Path, error: io::Error) -> io::Error {
    io::Error::new(
        error.kind(),
        format!("Unity Editor discovery at {}: {error}", path.display()),
    )
}

/// Spawns one validated Unity executable with an exact project selector and argv array.
pub fn launch_unity_editor(
    executable: &Path,
    project_root: &Path,
    arguments: &[OsString],
) -> io::Result<()> {
    let mut command = Command::new(crate::external_filesystem_path(executable).as_os_str());
    command
        .arg("-projectPath")
        .arg(crate::external_filesystem_path(project_root).as_os_str())
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let _ = command.spawn()?;
    Ok(())
}

/// Spawns a validated Unity executable with the fixed migration invocation profile.
pub fn spawn_unity_editor_migration(
    executable: &ValidatedUnityExecutable,
    authoritative_project_root: &Path,
) -> io::Result<UnityMigrationProcess> {
    if !authoritative_project_root.is_absolute() || !authoritative_project_root.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "authoritative project root is invalid",
        ));
    }
    let executable_path = crate::external_filesystem_path(executable.executable_path());
    let child = Command::new(executable_path.as_os_str())
        .arg("-quit")
        .arg("-batchmode")
        .arg("-projectPath")
        .arg(crate::external_filesystem_path(authoritative_project_root).as_os_str())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(UnityMigrationProcess { child })
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

#[cfg(all(test, windows))]
#[path = "../test-support/windows_unity_editor.rs"]
mod windows_editor_fixture;

#[cfg(all(test, windows))]
mod tests {
    use super::{discover_from_roots, windows_editor_fixture};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct DiscoveryFixture(PathBuf);

    impl DiscoveryFixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "alcomd-editor-discovery-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("clock")
                    .as_nanos(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&root).expect("fixture root");
            Self(root)
        }

        fn editor(&self, directory: &str, product: &str) -> PathBuf {
            let executable = super::editor_executable(&self.0.join(directory));
            let catalog = executable
                .parent()
                .expect("Editor directory")
                .join("Data/Resources/PackageManager/Editor/manifest.json");
            std::fs::create_dir_all(catalog.parent().expect("catalog parent"))
                .expect("Editor layout");
            windows_editor_fixture::write_editor_fixture(
                &executable,
                product,
                "2022.3.22f1_887be4894c44",
            )
            .expect("native Editor resource");
            std::fs::write(
                catalog,
                r#"{"schemaVersion":4,"packages":{"com.unity.collections":{"version":"1.2.4"}}}"#,
            )
            .expect("real-shaped package catalog");
            executable
        }

        fn root(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for DiscoveryFixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).expect("remove owned discovery fixture");
        }
    }

    #[test]
    fn launch_paths_use_ordinary_notation_for_the_same_editor_and_project() {
        let fixture = DiscoveryFixture::new();
        let editor = fixture.editor("中文 Editor", "Unity");
        let project = fixture.root().join("项目 with spaces");
        std::fs::create_dir(&project).expect("owned project directory");
        for path in [editor, project] {
            let canonical = std::fs::canonicalize(path).expect("canonical input");
            let argument = crate::external_filesystem_path(&canonical);
            assert!(!argument.as_os_str().to_string_lossy().starts_with(r"\\?\"));
            assert_eq!(
                crate::file_identity_key(&argument).expect("argument identity"),
                crate::file_identity_key(&canonical).expect("canonical identity")
            );
        }
    }

    #[test]
    fn launch_paths_keep_verbatim_semantics_when_ordinary_path_is_another_object() {
        let fixture = DiscoveryFixture::new();
        let ordinary = fixture.root().join("project");
        std::fs::create_dir(&ordinary).expect("ordinary directory");
        let verbatim = std::fs::canonicalize(fixture.root())
            .expect("canonical parent")
            .join("project.");
        std::fs::create_dir(&verbatim).expect("distinct verbatim directory");
        assert_ne!(
            crate::file_identity_key(&ordinary).expect("ordinary identity"),
            crate::file_identity_key(&verbatim).expect("verbatim identity")
        );
        assert_eq!(
            crate::external_filesystem_path(&verbatim).as_ref(),
            verbatim
        );
    }

    #[test]
    fn process_product_proof_requires_stable_identity_and_readable_metadata() {
        let fixture = DiscoveryFixture::new();
        for (directory, product, non_editor) in [
            ("editor", "Unity", false),
            ("same-name-cli", "unity", true),
            ("hub", "Unity Hub", true),
        ] {
            let executable = fixture.editor(directory, product);
            let identity = crate::file_identity_key(&executable).expect("fixture identity");
            assert_eq!(
                super::is_verified_non_editor_executable(&executable, &identity)
                    .expect("product proof"),
                non_editor
            );
            assert!(super::is_verified_non_editor_executable(&executable, &[0]).is_err());
        }
        let unreadable = fixture.root().join("missing-version.exe");
        std::fs::write(&unreadable, b"not a version resource").expect("owned invalid executable");
        let identity = crate::file_identity_key(&unreadable).expect("invalid executable identity");
        assert!(super::is_verified_non_editor_executable(&unreadable, &identity).is_err());
    }

    #[test]
    fn discovery_reads_editor_binary_and_deduplicates_root_identity() {
        let fixture = DiscoveryFixture::new();
        let executable = fixture.editor("directory-name-is-not-version", "Unity");
        let installations =
            discover_from_roots(vec![fixture.root().to_owned(), fixture.root().to_owned()])
                .expect("discover native metadata");
        assert_eq!(installations.len(), 1);
        assert_eq!(
            installations[0].executable_path(),
            std::fs::canonicalize(executable).expect("canonical executable")
        );
        assert_eq!(
            installations[0].product_version(),
            "2022.3.22f1_887be4894c44"
        );
    }

    #[test]
    fn discovery_does_not_silently_drop_an_existing_rejected_editor() {
        let fixture = DiscoveryFixture::new();
        fixture.editor("a-valid", "Unity");
        let rejected = fixture.editor("b-rejected", "Unity Hub");
        let error = discover_from_roots(vec![fixture.root().to_owned()])
            .expect_err("report rejected candidate");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(
            error
                .to_string()
                .contains(&rejected.to_string_lossy().to_string())
        );
        assert!(
            error
                .to_string()
                .contains("ProductName is not Unity Editor")
        );
    }

    #[test]
    fn discovery_distinguishes_absent_roots_and_non_editor_directories() {
        let fixture = DiscoveryFixture::new();
        std::fs::create_dir(fixture.root().join("unrelated-directory"))
            .expect("unrelated directory");
        let installations = discover_from_roots(vec![
            fixture.root().join("missing"),
            fixture.root().to_owned(),
        ])
        .expect("no Editor candidates");
        assert!(installations.is_empty());
    }
}
