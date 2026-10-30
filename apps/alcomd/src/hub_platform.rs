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
        self.launcher.launch(path).map_err(|error| match error {
            platform::UnityHubLaunchError::PathInvalid => app::UnityHubOpenError::PathInvalid,
            platform::UnityHubLaunchError::PathChanged => app::UnityHubOpenError::PathChanged,
            platform::UnityHubLaunchError::Unavailable => app::UnityHubOpenError::Unavailable,
            platform::UnityHubLaunchError::Busy => app::UnityHubOpenError::Busy,
            platform::UnityHubLaunchError::OpenFailed => app::UnityHubOpenError::OpenFailed,
        })
    }
}

impl app::UnityHubPlatform for PlatformUnityHubAdapter {
    fn normalize_path(&self, path: &str) -> Result<String, app::OfficialGuiError> {
        platform::normalize_unity_hub_path(path).map_err(|error| match error {
            platform::UnityHubPathError::Invalid => app::OfficialGuiError::InvalidInput,
            platform::UnityHubPathError::Unavailable => app::OfficialGuiError::Unavailable,
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
