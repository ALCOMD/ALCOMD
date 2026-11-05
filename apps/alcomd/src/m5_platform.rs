use std::ffi::OsString;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use alcomd_application as app;

#[derive(Clone, Copy, Default)]
pub(super) struct PlatformUnityAdapter;

impl app::M5UnityPlatform for PlatformUnityAdapter {
    async fn validate_installation(
        &self,
        executable_path: String,
        source_kind: app::UnitySourceKind,
    ) -> Result<app::UnityInstallationObservation, app::M5UnityError> {
        let executable_path = PathBuf::from(executable_path);
        let inspected_path = executable_path.clone();
        let value = tokio::task::spawn_blocking(move || {
            alcomd_platform::inspect_unity_installation(&inspected_path)
        })
        .await
        .map_err(|error| {
            invalid_installation()
                .with_path(executable_path.to_string_lossy())
                .with_source(error)
        })?
        .map_err(|error| {
            installation_validation_error(error.kind())
                .with_path(executable_path.to_string_lossy())
                .with_source(error)
        })?;
        observation(value, source_kind)
            .map_err(|error| error.with_path(executable_path.to_string_lossy()))
    }

    async fn discover_installations(
        &self,
    ) -> Result<Vec<app::UnityInstallationObservation>, app::M5UnityError> {
        tokio::task::spawn_blocking(alcomd_platform::discover_unity_executables)
            .await
            .map_err(|error| invalid_installation().with_source(error))?
            .map_err(|error| installation_validation_error(error.kind()).with_source(error))?
            .into_iter()
            .map(|value| {
                let path = value.executable_path().to_path_buf();
                observation(value, app::UnitySourceKind::KnownInstallRoot)
                    .map_err(|error| error.with_path(path.to_string_lossy()))
            })
            .collect()
    }

    async fn observe_processes(
        &self,
    ) -> Result<Vec<app::UnityProcessObservation>, app::M5UnityError> {
        let snapshot = alcomd_platform::observe_processes();
        Ok(snapshot
            .processes()
            .iter()
            .map(|process| {
                let name = process.name().to_string_lossy().into_owned();
                let executable_identity = process
                    .executable()
                    .and_then(|path| alcomd_platform::file_identity_key(path).ok());
                let verified_non_editor = verified_non_editor_process(
                    &name,
                    process.executable(),
                    executable_identity.as_deref(),
                );
                app::UnityProcessObservation {
                    pid: process.pid(),
                    start_time: process.start_time(),
                    name,
                    executable_identity,
                    verified_non_editor,
                    arguments: process.arguments().and_then(|arguments| {
                        arguments
                            .iter()
                            .map(|argument| argument.to_str().map(str::to_owned))
                            .collect()
                    }),
                }
            })
            .collect())
    }

    async fn path_identity(&self, path: String) -> Result<Vec<u8>, app::M5UnityError> {
        alcomd_platform::resolve_directory_identity(&PathBuf::from(&path))
            .map(|(_, identity)| identity)
            .map_err(|error| {
                app::M5UnityError::new(app::M5UnityErrorCode::Internal)
                    .with_path(path)
                    .with_source(error)
            })
    }

    async fn launch(
        &self,
        executable_path: String,
        project_root: String,
        arguments: Vec<String>,
    ) -> Result<(), app::M5UnityError> {
        let arguments = arguments
            .into_iter()
            .map(OsString::from)
            .collect::<Vec<_>>();
        alcomd_platform::launch_unity_editor(
            &PathBuf::from(&executable_path),
            &PathBuf::from(&project_root),
            &arguments,
        )
        .map_err(|error| {
            app::M5UnityError::new(app::M5UnityErrorCode::LaunchFailed)
                .with_path(executable_path)
                .with_source(error)
        })
    }
}

fn verified_non_editor_process(
    name: &str,
    executable: Option<&std::path::Path>,
    identity: Option<&[u8]>,
) -> bool {
    if !name.eq_ignore_ascii_case("unity") && !name.eq_ignore_ascii_case("unity.exe") {
        return false;
    }
    #[cfg(windows)]
    {
        match (executable, identity) {
            (Some(path), Some(identity)) => {
                alcomd_platform::is_verified_non_editor_executable(path, identity).unwrap_or(false)
            }
            _ => false,
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (executable, identity);
        false
    }
}

fn observation(
    value: alcomd_platform::ValidatedUnityExecutable,
    source_kind: app::UnitySourceKind,
) -> Result<app::UnityInstallationObservation, app::M5UnityError> {
    #[cfg(not(windows))]
    let unity_version = probe_output_version(
        value
            .probed_version()
            .ok_or_else(|| app::M5UnityError::new(app::M5UnityErrorCode::VersionUnverified))?,
    )?;
    #[cfg(windows)]
    let unity_version = editor_product_version(value.product_version())?;
    let unity_version = app::canonical_unity_version(unity_version)?;
    Ok(app::UnityInstallationObservation {
        executable_path: value
            .executable_path()
            .to_str()
            .ok_or_else(invalid_installation)?
            .to_owned(),
        filesystem_identity: value.filesystem_identity().to_vec(),
        unity_version,
        architecture: match value.architecture() {
            alcomd_platform::UnityArchitecture::Unknown => app::UnityArchitecture::Unknown,
        },
        source_kind,
        observed_at_ms: now_ms()?,
    })
}

/// Unity's -version output begins with the complete version and may include its
/// hexadecimal source revision. Arbitrary trailing output is not version proof.
#[cfg(any(not(windows), test))]
fn probe_output_version(value: &str) -> Result<&str, app::M5UnityError> {
    let invalid = || app::M5UnityError::new(app::M5UnityErrorCode::VersionUnverified);
    let mut parts = value.split_ascii_whitespace();
    let version = parts.next().ok_or_else(invalid)?;
    if let Some(revision) = parts.next() {
        let revision = revision
            .strip_prefix('(')
            .and_then(|value| value.strip_suffix(')'))
            .ok_or_else(invalid)?;
        if !matches!(revision.len(), 12 | 40)
            || !revision.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(invalid());
        }
    }
    if parts.next().is_some() {
        return Err(invalid());
    }
    app::canonical_unity_version(version)?;
    Ok(version)
}

#[cfg(test)]
mod version_output_tests {
    #[test]
    fn version_output_requires_complete_version_and_only_optional_hash() {
        for value in ["2022.3.22f1", "2022.3.22f1 (887be4894c44)"] {
            assert_eq!(
                super::probe_output_version(value).expect("version"),
                "2022.3.22f1"
            );
        }
        for value in [
            "2022.3",
            "version 2022.3.22f1",
            "2022.3.22f1 unrelated",
            "2022.3.22f1 (garbage)",
            "2022.3.22f1 (887be4894c44) more",
        ] {
            assert!(super::probe_output_version(value).is_err(), "{value}");
        }
    }
}

#[cfg(windows)]
fn editor_product_version(value: &str) -> Result<&str, app::M5UnityError> {
    let version = if let Some((version, revision)) = value.split_once('_') {
        if !matches!(revision.len(), 12 | 40)
            || !revision.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(app::M5UnityError::new(
                app::M5UnityErrorCode::VersionUnverified,
            ));
        }
        version
    } else {
        value
    };
    Ok(version)
}

fn installation_validation_error(kind: std::io::ErrorKind) -> app::M5UnityError {
    match kind {
        std::io::ErrorKind::InvalidData | std::io::ErrorKind::NotFound => {
            app::M5UnityError::new(app::M5UnityErrorCode::VersionUnverified)
        }
        _ => invalid_installation(),
    }
}

fn now_ms() -> Result<u64, app::M5UnityError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| app::M5UnityError::new(app::M5UnityErrorCode::Internal))?;
    u64::try_from(duration.as_millis())
        .map_err(|_| app::M5UnityError::new(app::M5UnityErrorCode::Internal))
}

fn invalid_installation() -> app::M5UnityError {
    app::M5UnityError::new(app::M5UnityErrorCode::InstallationInvalid)
}

#[cfg(all(test, windows))]
#[path = "../../../crates/alcomd-platform/test-support/windows_unity_editor.rs"]
mod windows_editor_fixture;

#[cfg(all(test, windows))]
mod tests {
    use super::{PlatformUnityAdapter, app, windows_editor_fixture};
    use alcomd_application::M5UnityPlatform;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct EditorFixture(PathBuf);

    impl EditorFixture {
        fn new(product: &str, version: &str, catalog: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "alcomd-editor-version-{}-{}-{}",
                std::process::id(),
                super::now_ms().expect("clock"),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
            let editor = root.join("2019.4.31f1/Editor");
            let catalog_path = editor.join("Data/Resources/PackageManager/Editor/manifest.json");
            std::fs::create_dir_all(catalog_path.parent().expect("catalog parent"))
                .expect("fixture directories");
            windows_editor_fixture::write_editor_fixture(
                &editor.join("Unity.exe"),
                product,
                version,
            )
            .expect("owned executable version resource");
            std::fs::write(catalog_path, catalog).expect("package catalog");
            Self(root)
        }

        fn executable(&self) -> &Path {
            &self.0
        }

        async fn inspect(&self) -> Result<app::UnityInstallationObservation, app::M5UnityError> {
            PlatformUnityAdapter
                .validate_installation(
                    self.executable()
                        .join("2019.4.31f1/Editor/Unity.exe")
                        .to_string_lossy()
                        .into_owned(),
                    app::UnitySourceKind::KnownInstallRoot,
                )
                .await
        }
    }

    impl Drop for EditorFixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).expect("remove owned Editor fixture");
        }
    }

    #[tokio::test]
    async fn non_editor_process_proof_never_uses_a_failed_inspection() {
        for (product, expected) in [("Unity", false), ("unity", true), ("Unity Hub", true)] {
            let fixture = EditorFixture::new(product, "1.0.0", "{}");
            let executable = fixture.executable().join("2019.4.31f1/Editor/Unity.exe");
            let identity =
                alcomd_platform::file_identity_key(&executable).expect("fixture identity");
            assert_eq!(
                super::verified_non_editor_process("Unity.exe", Some(&executable), Some(&identity)),
                expected
            );
            assert!(!super::verified_non_editor_process(
                "Unity.exe",
                Some(&executable),
                Some(&[0])
            ));
            assert!(!super::verified_non_editor_process(
                "Unity.exe",
                None,
                Some(&identity)
            ));
            assert!(!super::verified_non_editor_process(
                "Unity.exe",
                Some(&executable),
                None
            ));
        }
    }

    #[tokio::test]
    async fn windows_editor_version_is_not_read_from_package_catalog() {
        let fixture = EditorFixture::new(
            "Unity",
            "2022.3.22f1_887be4894c44",
            r#"{"schemaVersion":4,"packages":{"com.unity.collections":{"version":"1.2.4"}},"metadataPackageName":"com.unity.package-manager.metadata-2022.3"}"#,
        );
        let observed = fixture
            .inspect()
            .await
            .expect("verify Editor binary version");
        assert_eq!(observed.unity_version, "2022.3.22f1");
    }

    #[tokio::test]
    async fn windows_editor_version_ignores_catalog_and_directory_versions() {
        let fixture = EditorFixture::new(
            "Unity",
            "2022.3.22f1_887be4894c44",
            r#"{"version":"2019.4.31f1"}"#,
        );
        assert_eq!(
            fixture
                .inspect()
                .await
                .expect("binary metadata")
                .unity_version,
            "2022.3.22f1"
        );
    }

    #[tokio::test]
    async fn windows_editor_version_rejects_non_unity_and_unverified_metadata() {
        for (product, version) in [
            ("Unity Hub", "2022.3.22f1"),
            ("Unity", "2022.3.22"),
            ("Unity", "2022.3.22f1_bad"),
        ] {
            let fixture = EditorFixture::new(product, version, r#"{"version":"2022.3.22f1"}"#);
            assert_eq!(
                fixture
                    .inspect()
                    .await
                    .expect_err("reject unverified Editor")
                    .code(),
                app::M5UnityErrorCode::VersionUnverified
            );
        }
    }
}
