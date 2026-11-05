use alcomd_application as app;
use alcomd_platform as platform;

#[derive(Clone, Copy, Default)]
pub(super) struct PlatformUnityHubAdapter;

#[derive(Clone, Default)]
pub(super) struct PlatformUnityHubLauncher {
    launcher: platform::UnityHubProcessLauncher,
}

impl app::UnityHubLauncher for PlatformUnityHubLauncher {
    fn launch(&self, path: &str) -> Result<(), app::UnityHubOpenError> {
        self.launcher.launch(path).map_err(|error| {
            let classified = match error.kind() {
                platform::UnityHubLaunchErrorKind::PathInvalid => {
                    app::UnityHubOpenError::PathInvalid
                }
                platform::UnityHubLaunchErrorKind::PathChanged => {
                    app::UnityHubOpenError::PathChanged
                }
                platform::UnityHubLaunchErrorKind::Unavailable => {
                    app::UnityHubOpenError::Unavailable
                }
                platform::UnityHubLaunchErrorKind::Busy => app::UnityHubOpenError::Busy,
                platform::UnityHubLaunchErrorKind::OpenFailed => app::UnityHubOpenError::OpenFailed,
            };
            classified
                .with_source(error)
                .with_path(path)
                .with_reason("failed to open Unity Hub")
        })
    }
}

impl app::UnityHubPlatform for PlatformUnityHubAdapter {
    fn normalize_path(&self, path: &str) -> Result<String, app::OfficialGuiError> {
        platform::normalize_unity_hub_path(path).map_err(|error| {
            let classified = match error.kind() {
                platform::UnityHubPathErrorKind::Invalid => app::OfficialGuiError::InvalidInput,
                platform::UnityHubPathErrorKind::Unavailable => app::OfficialGuiError::Unavailable,
            };
            classified
                .with_source(error)
                .with_path(path)
                .with_reason("failed to validate Unity Hub executable")
        })
    }

    fn inspect(&self, configured_path: Option<&str>) -> app::UnityHubLocation {
        let value = platform::inspect_unity_hub(configured_path);
        app::UnityHubLocation {
            status: match value.status {
                platform::UnityHubAvailability::Available => app::UnityHubStatus::Available,
                platform::UnityHubAvailability::NotFound => app::UnityHubStatus::NotFound,
                platform::UnityHubAvailability::Unavailable => app::UnityHubStatus::Unavailable,
            },
            source: match value.source {
                platform::UnityHubOrigin::Configured => app::UnityHubSource::Configured,
                platform::UnityHubOrigin::Discovered => app::UnityHubSource::Discovered,
                platform::UnityHubOrigin::None => app::UnityHubSource::None,
            },
            path: value.path,
            configured_path_status: match value.configured_path_status {
                platform::ConfiguredUnityHubStatus::Unset => {
                    app::UnityHubConfiguredPathStatus::Unset
                }
                platform::ConfiguredUnityHubStatus::Available => {
                    app::UnityHubConfiguredPathStatus::Available
                }
                platform::ConfiguredUnityHubStatus::Invalid => {
                    app::UnityHubConfiguredPathStatus::Invalid
                }
                platform::ConfiguredUnityHubStatus::Unavailable => {
                    app::UnityHubConfiguredPathStatus::Unavailable
                }
            },
        }
    }
}
