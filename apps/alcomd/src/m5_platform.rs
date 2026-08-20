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
        std::fs::metadata(&executable_path).map_err(|_| invalid_installation())?;
        let value =
            alcomd_platform::validate_unity_executable(&executable_path).map_err(|error| {
                match error.kind() {
                    std::io::ErrorKind::InvalidData | std::io::ErrorKind::NotFound => {
                        app::M5UnityError::new(app::M5UnityErrorCode::VersionUnverified)
                    }
                    _ => invalid_installation(),
                }
            })?;
        observation(value, source_kind)
    }

    async fn discover_installations(
        &self,
    ) -> Result<Vec<app::UnityInstallationObservation>, app::M5UnityError> {
        Ok(alcomd_platform::discover_unity_executables()
            .into_iter()
            .filter_map(|value| observation(value, app::UnitySourceKind::KnownInstallRoot).ok())
            .collect())
    }

    async fn observe_processes(
        &self,
    ) -> Result<Vec<app::UnityProcessObservation>, app::M5UnityError> {
        let snapshot = alcomd_platform::observe_processes();
        Ok(snapshot
            .processes()
            .iter()
            .map(|process| app::UnityProcessObservation {
                pid: process.pid(),
                start_time: process.start_time(),
                name: process.name().to_string_lossy().into_owned(),
                executable_identity: process
                    .executable()
                    .and_then(|path| alcomd_platform::file_identity_key(path).ok()),
                arguments: process.arguments().and_then(|arguments| {
                    arguments
                        .iter()
                        .map(|argument| argument.to_str().map(str::to_owned))
                        .collect()
                }),
            })
            .collect())
    }

    async fn path_identity(&self, path: String) -> Result<Vec<u8>, app::M5UnityError> {
        alcomd_platform::resolve_directory_identity(&PathBuf::from(path))
            .map(|(_, identity)| identity)
            .map_err(|_| app::M5UnityError::new(app::M5UnityErrorCode::Internal))
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
            &PathBuf::from(executable_path),
            &PathBuf::from(project_root),
            &arguments,
        )
        .map_err(|_| app::M5UnityError::new(app::M5UnityErrorCode::LaunchFailed))
    }
}

fn observation(
    value: alcomd_platform::ValidatedUnityExecutable,
    source_kind: app::UnitySourceKind,
) -> Result<app::UnityInstallationObservation, app::M5UnityError> {
    let document: serde_json::Value = serde_json::from_slice(value.version_manifest())
        .map_err(|_| app::M5UnityError::new(app::M5UnityErrorCode::VersionUnverified))?;
    let unity_version = document
        .get("version")
        .and_then(serde_json::Value::as_str)
        .filter(|version| valid_unity_version(version))
        .ok_or_else(|| app::M5UnityError::new(app::M5UnityErrorCode::VersionUnverified))?
        .to_owned();
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

fn valid_unity_version(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.is_ascii()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-')
        && value.contains('.')
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
