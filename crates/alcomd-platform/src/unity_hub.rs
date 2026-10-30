//! Bounded, read-only Unity Hub executable location inspection.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const MAX_PATH_BYTES: usize = 4096;
const MAX_DEFAULT_CANDIDATES: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnityHubPathError {
    Invalid,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnityHubAvailability {
    Available,
    NotFound,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnityHubOrigin {
    Configured,
    Discovered,
    None,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfiguredUnityHubStatus {
    Unset,
    Available,
    Invalid,
    Unavailable,
}

/// Filesystem evidence only: this is not a vendor identity or execution guarantee.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnityHubInspection {
    pub status: UnityHubAvailability,
    pub source: UnityHubOrigin,
    pub path: Option<String>,
    pub configured_path_status: ConfiguredUnityHubStatus,
}

/// Validates one selected path without running it or reading Hub's configuration.
pub fn normalize_unity_hub_path(path: &str) -> Result<String, UnityHubPathError> {
    if !valid_text(path) || !Path::new(path).is_absolute() {
        return Err(UnityHubPathError::Invalid);
    }
    let selected = PathBuf::from(path);
    #[cfg(target_os = "macos")]
    let selected = if selected
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("app"))
    {
        selected.join("Contents/MacOS/Unity Hub")
    } else {
        selected
    };
    let canonical = fs::canonicalize(selected).map_err(classify_io)?;
    let metadata = fs::metadata(&canonical).map_err(classify_io)?;
    if !metadata.is_file() {
        return Err(UnityHubPathError::Invalid);
    }
    #[cfg(windows)]
    if !canonical
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
    {
        return Err(UnityHubPathError::Invalid);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(UnityHubPathError::Invalid);
        }
    }
    #[cfg(windows)]
    let canonical = readable_windows_path(canonical);
    canonical
        .to_str()
        .filter(|value| valid_text(value))
        .map(str::to_owned)
        .ok_or(UnityHubPathError::Invalid)
}

/// Use ordinary DOS/UNC notation only when it resolves to the same canonical file.
/// Device/volume paths and paths that require verbatim semantics stay unchanged.
#[cfg(windows)]
fn readable_windows_path(canonical: PathBuf) -> PathBuf {
    let mut components = canonical.components();
    let Some(std::path::Component::Prefix(prefix)) = components.next() else {
        return canonical;
    };
    let mut readable = match prefix.kind() {
        std::path::Prefix::VerbatimDisk(drive) => PathBuf::from(format!("{}:", char::from(drive))),
        std::path::Prefix::VerbatimUNC(server, share) => {
            let mut path = std::ffi::OsString::from(r"\\");
            path.push(server);
            path.push(r"\");
            path.push(share);
            PathBuf::from(path)
        }
        _ => return canonical,
    };
    readable.push(components.as_path());
    if fs::canonicalize(&readable).is_ok_and(|resolved| resolved == canonical) {
        readable
    } else {
        canonical
    }
}

/// Searches only fixed platform candidates after the configured executable.
#[must_use]
pub fn inspect_unity_hub(configured_path: Option<&str>) -> UnityHubInspection {
    inspect_candidates(configured_path, default_candidates())
}

fn inspect_candidates(configured_path: Option<&str>, defaults: Vec<PathBuf>) -> UnityHubInspection {
    let mut configured_path_status = ConfiguredUnityHubStatus::Unset;
    if let Some(path) = configured_path {
        match normalize_unity_hub_path(path) {
            Ok(path) => {
                return UnityHubInspection {
                    status: UnityHubAvailability::Available,
                    source: UnityHubOrigin::Configured,
                    path: Some(path),
                    configured_path_status: ConfiguredUnityHubStatus::Available,
                };
            }
            Err(UnityHubPathError::Invalid) => {
                configured_path_status = ConfiguredUnityHubStatus::Invalid;
            }
            Err(UnityHubPathError::Unavailable) => {
                return missing(
                    UnityHubAvailability::Unavailable,
                    ConfiguredUnityHubStatus::Unavailable,
                );
            }
        }
    }
    let mut seen = Vec::new();
    for candidate in defaults.into_iter().take(MAX_DEFAULT_CANDIDATES) {
        if !candidate.is_absolute() || seen.contains(&candidate) {
            continue;
        }
        seen.push(candidate.clone());
        let Some(path) = candidate.to_str() else {
            continue;
        };
        match normalize_unity_hub_path(path) {
            Ok(path) => {
                return UnityHubInspection {
                    status: UnityHubAvailability::Available,
                    source: UnityHubOrigin::Discovered,
                    path: Some(path),
                    configured_path_status,
                };
            }
            Err(UnityHubPathError::Invalid) => {}
            Err(UnityHubPathError::Unavailable) => {
                return missing(UnityHubAvailability::Unavailable, configured_path_status);
            }
        }
    }
    missing(UnityHubAvailability::NotFound, configured_path_status)
}

fn missing(
    status: UnityHubAvailability,
    configured_path_status: ConfiguredUnityHubStatus,
) -> UnityHubInspection {
    UnityHubInspection {
        status,
        source: UnityHubOrigin::None,
        path: None,
        configured_path_status,
    }
}

fn valid_text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_PATH_BYTES
        && !value.bytes().any(|byte| byte < 32 || byte == 127)
}

fn classify_io(error: io::Error) -> UnityHubPathError {
    match error.kind() {
        io::ErrorKind::NotFound
        | io::ErrorKind::NotADirectory
        | io::ErrorKind::InvalidInput
        | io::ErrorKind::InvalidData => UnityHubPathError::Invalid,
        _ => UnityHubPathError::Unavailable,
    }
}

#[cfg(windows)]
fn default_candidates() -> Vec<PathBuf> {
    [
        ("ProgramFiles", "Unity Hub/Unity Hub.exe"),
        ("ProgramFiles(x86)", "Unity Hub/Unity Hub.exe"),
        ("LOCALAPPDATA", "Programs/Unity Hub/Unity Hub.exe"),
    ]
    .into_iter()
    .filter_map(|(variable, relative)| {
        std::env::var_os(variable).map(|root| PathBuf::from(root).join(relative))
    })
    .collect()
}

#[cfg(target_os = "macos")]
fn default_candidates() -> Vec<PathBuf> {
    let mut paths = vec![PathBuf::from(
        "/Applications/Unity Hub.app/Contents/MacOS/Unity Hub",
    )];
    if let Some(home) = std::env::var_os("HOME") {
        paths.push(PathBuf::from(home).join("Applications/Unity Hub.app/Contents/MacOS/Unity Hub"));
    }
    paths
}

#[cfg(all(unix, not(target_os = "macos")))]
fn default_candidates() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| home.as_ref().map(|root| root.join(".local/share")));
    let mut paths = Vec::new();
    if let Some(home) = home {
        paths.push(home.join("Applications/Unity Hub.AppImage"));
    }
    paths.extend([
        PathBuf::from("/usr/bin/unity-hub"),
        PathBuf::from("/opt/unityhub/unityhub"),
    ]);
    if let Some(data_home) = data_home {
        paths.push(data_home.join("flatpak/exports/bin/com.unity.UnityHub"));
    }
    paths.push(PathBuf::from(
        "/var/lib/flatpak/exports/bin/com.unity.UnityHub",
    ));
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "alcomd-hub-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::SeqCst)
            ));
            fs::create_dir_all(&path).expect("fixture directory");
            Self(path)
        }

        fn executable(&self, name: &str) -> PathBuf {
            let path = self.0.join(format!("{name}.exe"));
            fs::write(&path, b"not executed").expect("fixture executable");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).expect("execute bit");
            }
            path
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn configured_executable_wins_and_inspection_does_not_modify_it() {
        let fixture = Fixture::new();
        let configured = fixture.executable("自定义 Hub");
        let fallback = fixture.executable("fallback");
        let value = inspect_candidates(configured.to_str(), vec![fallback]);
        assert_eq!(value.status, UnityHubAvailability::Available);
        assert_eq!(value.source, UnityHubOrigin::Configured);
        assert_eq!(
            value.configured_path_status,
            ConfiguredUnityHubStatus::Available
        );
        assert_eq!(
            value.path,
            Some(normalize_unity_hub_path(configured.to_str().expect("UTF-8")).expect("valid"))
        );
        assert_eq!(
            fs::read(configured).expect("unchanged executable"),
            b"not executed"
        );
    }

    #[test]
    fn stale_configuration_falls_back_in_order_and_search_is_bounded() {
        let fixture = Fixture::new();
        let missing = fixture.0.join("missing.exe");
        let first = fixture.executable("first");
        let second = fixture.executable("second");
        let evidence = inspect_candidates(
            missing.to_str(),
            vec![missing.clone(), first.clone(), second],
        );
        assert_eq!(evidence.source, UnityHubOrigin::Discovered);
        assert_eq!(
            evidence.configured_path_status,
            ConfiguredUnityHubStatus::Invalid
        );
        assert_eq!(
            evidence.path,
            Some(normalize_unity_hub_path(first.to_str().expect("UTF-8")).expect("valid"))
        );
        let mut paths = vec![missing; MAX_DEFAULT_CANDIDATES];
        paths.push(first);
        assert_eq!(
            inspect_candidates(None, paths).status,
            UnityHubAvailability::NotFound
        );
    }

    #[test]
    fn invalid_paths_never_become_available() {
        let fixture = Fixture::new();
        for path in [
            "",
            "relative.exe",
            "a\0.exe",
            &"x".repeat(MAX_PATH_BYTES + 1),
        ] {
            assert_eq!(
                normalize_unity_hub_path(path),
                Err(UnityHubPathError::Invalid)
            );
        }
        assert_eq!(
            normalize_unity_hub_path(fixture.0.to_str().expect("UTF-8")),
            Err(UnityHubPathError::Invalid)
        );
        assert_eq!(
            inspect_candidates(None, vec![]),
            missing(
                UnityHubAvailability::NotFound,
                ConfiguredUnityHubStatus::Unset
            )
        );
    }

    #[test]
    fn unreadable_metadata_is_not_classified_as_an_absent_or_invalid_path() {
        assert_eq!(
            classify_io(io::Error::from(io::ErrorKind::PermissionDenied)),
            UnityHubPathError::Unavailable
        );
        assert_eq!(
            classify_io(io::Error::from(io::ErrorKind::Other)),
            UnityHubPathError::Unavailable
        );
        assert_eq!(
            classify_io(io::Error::from(io::ErrorKind::NotFound)),
            UnityHubPathError::Invalid
        );
        assert!(default_candidates().len() <= MAX_DEFAULT_CANDIDATES);
    }

    #[cfg(windows)]
    #[test]
    fn windows_hub_path_is_readable_and_keeps_the_same_file_identity() {
        let fixture = Fixture::new();
        let selected = fixture.executable("中文 Hub");
        let canonical = fs::canonicalize(&selected).expect("canonical fixture");
        let normalized =
            normalize_unity_hub_path(selected.to_str().expect("UTF-8")).expect("valid Hub");
        assert!(!normalized.starts_with(r"\\?\"));
        assert_eq!(fs::canonicalize(&normalized).expect("same file"), canonical);
        assert_eq!(
            normalize_unity_hub_path(canonical.to_str().expect("UTF-8")),
            Ok(normalized)
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_hub_path_keeps_unknown_verbatim_volume_paths() {
        let path = PathBuf::from(r"\\?\Volume{00000000-0000-0000-0000-000000000000}\Hub.exe");
        assert_eq!(readable_windows_path(path.clone()), path);
    }

    #[cfg(windows)]
    #[test]
    fn windows_requires_exe_extension() {
        let fixture = Fixture::new();
        let path = fixture.0.join("hub.txt");
        fs::write(&path, b"text").expect("file");
        assert_eq!(
            normalize_unity_hub_path(path.to_str().expect("UTF-8")),
            Err(UnityHubPathError::Invalid)
        );
    }

    #[cfg(unix)]
    #[test]
    fn unix_requires_execute_permission_and_resolves_symlink() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let fixture = Fixture::new();
        let executable = fixture.executable("hub");
        let alias = fixture.0.join("alias");
        symlink(&executable, &alias).expect("symlink");
        assert_eq!(
            normalize_unity_hub_path(alias.to_str().expect("UTF-8")),
            normalize_unity_hub_path(executable.to_str().expect("UTF-8"))
        );
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o600))
            .expect("no execute bit");
        assert_eq!(
            normalize_unity_hub_path(executable.to_str().expect("UTF-8")),
            Err(UnityHubPathError::Invalid)
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_bundle_resolves_to_hub_executable() {
        let fixture = Fixture::new();
        let bundle = fixture.0.join("Unity Hub.app");
        let executable = bundle.join("Contents/MacOS/Unity Hub");
        fs::create_dir_all(executable.parent().expect("parent")).expect("bundle");
        fs::write(&executable, b"not executed").expect("file");
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).expect("execute bit");
        assert_eq!(
            normalize_unity_hub_path(bundle.to_str().expect("UTF-8")),
            normalize_unity_hub_path(executable.to_str().expect("UTF-8"))
        );
    }
}
