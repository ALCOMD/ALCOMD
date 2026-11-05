use alcomd_application as app;
use alcomd_protocol as rpc;

use super::{
    AccessContext, ConnectionState, DispatchAction, M7OfficialGuiApplication, error_action,
    invalid, require_capability, success_action,
};

pub(super) async fn dispatch(
    request: rpc::RequestEnvelope,
    state: &ConnectionState,
    application: &M7OfficialGuiApplication,
    access: &AccessContext,
) -> DispatchAction {
    match request.method.as_str() {
        rpc::METHOD_UNITY_HUB_OPEN => {
            if let Some(action) =
                require_capability(&request.id, state, rpc::CAPABILITY_UNITY_HUB_OPEN_V1)
            {
                return action;
            }
            let params: rpc::UnityHubOpenParams = match serde_json::from_value(request.params) {
                Ok(value) => value,
                Err(_) => return invalid(request.id),
            };
            match application
                .open_unity_hub(access, params.expected_revision, params.expected_path)
                .await
            {
                Ok(value) => success_action(
                    request.id,
                    rpc::UnityHubOpenResult {
                        outcome: match value {
                            app::UnityHubOpenOutcome::Spawned => rpc::UnityHubOpenOutcome::Spawned,
                            app::UnityHubOpenOutcome::DownloadRequired => {
                                rpc::UnityHubOpenOutcome::DownloadRequired
                            }
                        },
                    },
                    None,
                ),
                Err(error) => hub_open_error(request.id, error),
            }
        }
        rpc::METHOD_SETTINGS_GET => {
            if request
                .params
                .as_object()
                .is_none_or(|params| !params.is_empty())
            {
                return invalid(request.id);
            }
            match application.get_settings(access).await {
                Ok(value) => {
                    let hub = application.observe_unity_hub(&value).await;
                    match application.observe_storage_directories(&value).await {
                        Ok(directories) => success_action(
                            request.id,
                            settings_result(value, hub, directories),
                            None,
                        ),
                        Err(error) => official_error(request.id, error),
                    }
                }
                Err(error) => official_error(request.id, error),
            }
        }
        rpc::METHOD_SETTINGS_UPDATE => {
            let params: rpc::SettingsUpdateParams = match serde_json::from_value(request.params) {
                Ok(value) => value,
                Err(_) => return invalid(request.id),
            };
            let update = app::ConfigUpdate {
                projects: params
                    .update
                    .projects
                    .map(|value| app::ConfigProjectUpdate {
                        auto_discover: value.auto_discover,
                    }),
                backups: params.update.backups.map(|value| app::ConfigBackupUpdate {
                    compression_mode: value.compression_mode.map(backup_compression_from_rpc),
                    include_vpm_packages: value.include_vpm_packages,
                    include_library: value.include_library,
                }),
                window: params.update.window.map(|value| app::ConfigWindowUpdate {
                    remember_state: value.remember_state,
                    fullscreen: value.fullscreen,
                }),
                paths: params.update.paths.map(|value| app::ConfigPathUpdate {
                    project_directory: directory_update_from_rpc(value.project_directory),
                    backup_directory: directory_update_from_rpc(value.backup_directory),
                }),
                unity: params.update.unity.map(|value| app::ConfigUnityUpdate {
                    default_launch_arguments: match value.default_launch_arguments {
                        rpc::NullableUpdate::Unchanged => app::ConfigNullableUpdate::Unchanged,
                        rpc::NullableUpdate::Clear => app::ConfigNullableUpdate::Clear,
                        rpc::NullableUpdate::Set(value) => app::ConfigNullableUpdate::Set(value),
                    },
                    hub_path: match value.hub_path {
                        rpc::NullableUpdate::Unchanged => app::ConfigNullableUpdate::Unchanged,
                        rpc::NullableUpdate::Clear => app::ConfigNullableUpdate::Clear,
                        rpc::NullableUpdate::Set(value) => app::ConfigNullableUpdate::Set(value),
                    },
                }),
                appearance: params
                    .update
                    .appearance
                    .map(|value| app::ConfigAppearanceUpdate {
                        mode: value.mode.map(appearance_mode_from_rpc),
                        scheme: value.scheme.map(appearance_scheme_from_rpc),
                        source_color: match value.source_color {
                            rpc::NullableUpdate::Unchanged => app::ConfigNullableUpdate::Unchanged,
                            rpc::NullableUpdate::Clear => app::ConfigNullableUpdate::Clear,
                            rpc::NullableUpdate::Set(value) => {
                                app::ConfigNullableUpdate::Set(value)
                            }
                        },
                        motion: value.motion.map(appearance_motion_from_rpc),
                    }),
                locale: params.update.locale.map(locale_from_rpc),
                packages: params
                    .update
                    .packages
                    .map(|value| app::ConfigPackageSettingsUpdate {
                        show_prerelease: value.show_prerelease,
                        hidden_repository_ids: value.hidden_repository_ids,
                        hide_local_user_packages: value.hide_local_user_packages,
                    }),
            };
            match application
                .update_settings(access, params.expected_revision, update)
                .await
            {
                Ok(value) => {
                    let hub = application.observe_unity_hub(&value).await;
                    match application.observe_storage_directories(&value).await {
                        Ok(directories) => success_action(
                            request.id,
                            settings_result(value, hub, directories),
                            None,
                        ),
                        Err(error) => official_error(request.id, error),
                    }
                }
                Err(error) => official_error(request.id, error),
            }
        }
        _ => error_action(Some(request.id), rpc::RpcError::method_not_found(), false),
    }
}

fn backup_compression_from_rpc(value: rpc::BackupCompression) -> app::BackupCompression {
    match value {
        rpc::BackupCompression::Store => app::BackupCompression::Store,
        rpc::BackupCompression::Fast => app::BackupCompression::Fast,
        rpc::BackupCompression::Level3 => app::BackupCompression::Level3,
        rpc::BackupCompression::Level5 => app::BackupCompression::Level5,
        rpc::BackupCompression::Level7 => app::BackupCompression::Level7,
        rpc::BackupCompression::Maximum => app::BackupCompression::Maximum,
    }
}

fn settings_result(
    value: app::ConfigSnapshot,
    hub: app::UnityHubLocation,
    directories: app::StorageDirectories,
) -> rpc::SettingsGetResult {
    rpc::SettingsGetResult {
        config_schema: rpc::CONFIG_SCHEMA_VERSION,
        revision: value.revision,
        directories: rpc::StorageDirectories {
            project_directory: directories.project_directory,
            backup_directory: directories.backup_directory,
        },
        unity_hub: rpc::UnityHubLocation {
            status: match hub.status {
                app::UnityHubStatus::Available => rpc::UnityHubStatus::Available,
                app::UnityHubStatus::NotFound => rpc::UnityHubStatus::NotFound,
                app::UnityHubStatus::Unavailable => rpc::UnityHubStatus::Unavailable,
            },
            source: match hub.source {
                app::UnityHubSource::Configured => rpc::UnityHubSource::Configured,
                app::UnityHubSource::Discovered => rpc::UnityHubSource::Discovered,
                app::UnityHubSource::None => rpc::UnityHubSource::None,
            },
            path: hub.path,
            configured_path_status: match hub.configured_path_status {
                app::UnityHubConfiguredPathStatus::Unset => {
                    rpc::UnityHubConfiguredPathStatus::Unset
                }
                app::UnityHubConfiguredPathStatus::Available => {
                    rpc::UnityHubConfiguredPathStatus::Available
                }
                app::UnityHubConfiguredPathStatus::Invalid => {
                    rpc::UnityHubConfiguredPathStatus::Invalid
                }
                app::UnityHubConfiguredPathStatus::Unavailable => {
                    rpc::UnityHubConfiguredPathStatus::Unavailable
                }
            },
        },
        settings: rpc::Settings {
            projects: rpc::ProjectSettings {
                auto_discover: value.settings.projects.auto_discover,
            },
            backups: rpc::BackupSettings {
                compression_mode: match value.settings.backups.compression_mode {
                    app::BackupCompression::Store => rpc::BackupCompression::Store,
                    app::BackupCompression::Fast => rpc::BackupCompression::Fast,
                    app::BackupCompression::Level3 => rpc::BackupCompression::Level3,
                    app::BackupCompression::Level5 => rpc::BackupCompression::Level5,
                    app::BackupCompression::Level7 => rpc::BackupCompression::Level7,
                    app::BackupCompression::Maximum => rpc::BackupCompression::Maximum,
                },
                include_vpm_packages: value.settings.backups.include_vpm_packages,
                include_library: value.settings.backups.include_library,
            },
            window: rpc::WindowSettings {
                remember_state: value.settings.window.remember_state,
                fullscreen: value.settings.window.fullscreen,
            },
            paths: rpc::PathSettings {
                project_directory: value.settings.paths.project_directory,
                backup_directory: value.settings.paths.backup_directory,
            },
            unity: rpc::UnitySettings {
                hub_path: value.settings.unity.hub_path,
                default_launch_arguments: value.settings.unity.default_launch_arguments,
            },
            appearance: rpc::AppearanceSettings {
                mode: match value.settings.appearance.mode {
                    app::ConfigAppearanceMode::System => rpc::AppearanceMode::System,
                    app::ConfigAppearanceMode::Light => rpc::AppearanceMode::Light,
                    app::ConfigAppearanceMode::Dark => rpc::AppearanceMode::Dark,
                },
                scheme: match value.settings.appearance.scheme {
                    app::ConfigAppearanceScheme::TonalSpot => rpc::AppearanceScheme::TonalSpot,
                    app::ConfigAppearanceScheme::Fidelity => rpc::AppearanceScheme::Fidelity,
                    app::ConfigAppearanceScheme::Monochrome => rpc::AppearanceScheme::Monochrome,
                    app::ConfigAppearanceScheme::Neutral => rpc::AppearanceScheme::Neutral,
                    app::ConfigAppearanceScheme::Vibrant => rpc::AppearanceScheme::Vibrant,
                    app::ConfigAppearanceScheme::Expressive => rpc::AppearanceScheme::Expressive,
                    app::ConfigAppearanceScheme::Content => rpc::AppearanceScheme::Content,
                    app::ConfigAppearanceScheme::Rainbow => rpc::AppearanceScheme::Rainbow,
                    app::ConfigAppearanceScheme::FruitSalad => rpc::AppearanceScheme::FruitSalad,
                },
                source_color: value.settings.appearance.source_color,
                motion: match value.settings.appearance.motion {
                    app::ConfigAppearanceMotion::System => rpc::AppearanceMotion::System,
                    app::ConfigAppearanceMotion::Full => rpc::AppearanceMotion::Full,
                    app::ConfigAppearanceMotion::Reduced => rpc::AppearanceMotion::Reduced,
                },
            },
            locale: match value.settings.locale {
                app::ConfigLocale::System => rpc::SettingsLocale::System,
                app::ConfigLocale::EnUs => rpc::SettingsLocale::EnUs,
                app::ConfigLocale::ZhCn => rpc::SettingsLocale::ZhCn,
                app::ConfigLocale::JaJp => rpc::SettingsLocale::JaJp,
            },
            packages: rpc::PackageSettings {
                show_prerelease: value.settings.packages.show_prerelease,
                hidden_repository_ids: value.settings.packages.hidden_repository_ids,
                hide_local_user_packages: value.settings.packages.hide_local_user_packages,
            },
        },
    }
}

fn directory_update_from_rpc(
    value: rpc::NullableUpdate<String>,
) -> app::ConfigNullableUpdate<String> {
    match value {
        rpc::NullableUpdate::Unchanged => app::ConfigNullableUpdate::Unchanged,
        rpc::NullableUpdate::Clear => app::ConfigNullableUpdate::Clear,
        rpc::NullableUpdate::Set(value) => app::ConfigNullableUpdate::Set(value),
    }
}

fn appearance_mode_from_rpc(value: rpc::AppearanceMode) -> app::ConfigAppearanceMode {
    match value {
        rpc::AppearanceMode::System => app::ConfigAppearanceMode::System,
        rpc::AppearanceMode::Light => app::ConfigAppearanceMode::Light,
        rpc::AppearanceMode::Dark => app::ConfigAppearanceMode::Dark,
    }
}

fn appearance_scheme_from_rpc(value: rpc::AppearanceScheme) -> app::ConfigAppearanceScheme {
    match value {
        rpc::AppearanceScheme::TonalSpot => app::ConfigAppearanceScheme::TonalSpot,
        rpc::AppearanceScheme::Fidelity => app::ConfigAppearanceScheme::Fidelity,
        rpc::AppearanceScheme::Monochrome => app::ConfigAppearanceScheme::Monochrome,
        rpc::AppearanceScheme::Neutral => app::ConfigAppearanceScheme::Neutral,
        rpc::AppearanceScheme::Vibrant => app::ConfigAppearanceScheme::Vibrant,
        rpc::AppearanceScheme::Expressive => app::ConfigAppearanceScheme::Expressive,
        rpc::AppearanceScheme::Content => app::ConfigAppearanceScheme::Content,
        rpc::AppearanceScheme::Rainbow => app::ConfigAppearanceScheme::Rainbow,
        rpc::AppearanceScheme::FruitSalad => app::ConfigAppearanceScheme::FruitSalad,
    }
}

fn appearance_motion_from_rpc(value: rpc::AppearanceMotion) -> app::ConfigAppearanceMotion {
    match value {
        rpc::AppearanceMotion::System => app::ConfigAppearanceMotion::System,
        rpc::AppearanceMotion::Full => app::ConfigAppearanceMotion::Full,
        rpc::AppearanceMotion::Reduced => app::ConfigAppearanceMotion::Reduced,
    }
}

fn locale_from_rpc(value: rpc::SettingsLocale) -> app::ConfigLocale {
    match value {
        rpc::SettingsLocale::System => app::ConfigLocale::System,
        rpc::SettingsLocale::EnUs => app::ConfigLocale::EnUs,
        rpc::SettingsLocale::ZhCn => app::ConfigLocale::ZhCn,
        rpc::SettingsLocale::JaJp => app::ConfigLocale::JaJp,
    }
}

fn official_error(id: String, error: app::OfficialGuiError) -> DispatchAction {
    let diagnostic = error.diagnostic_id().map(str::to_owned);
    let mut error = match error.kind() {
        app::OfficialGuiErrorKind::InvalidInput => rpc::RpcError::invalid_request(),
        app::OfficialGuiErrorKind::PermissionDenied => rpc::RpcError::permission_denied(),
        app::OfficialGuiErrorKind::RevisionConflict => rpc::RpcError::revision_conflict(),
        app::OfficialGuiErrorKind::Unavailable | app::OfficialGuiErrorKind::Corrupt => {
            rpc::RpcError::internal(String::new())
        }
    };
    // The source owner already recorded the failure. Encoding never writes logs
    // or invents an association for an unrecorded classification.
    error.diagnostic_id = diagnostic;
    error_action(Some(id), error, false)
}

fn hub_open_error(id: String, error: app::UnityHubOpenError) -> DispatchAction {
    let diagnostic = error.diagnostic_id().map(str::to_owned);
    let mut error = match error.kind() {
        app::UnityHubOpenErrorKind::InvalidInput => rpc::RpcError::invalid_request(),
        app::UnityHubOpenErrorKind::PermissionDenied => rpc::RpcError::permission_denied(),
        app::UnityHubOpenErrorKind::RevisionConflict => rpc::RpcError::revision_conflict(),
        app::UnityHubOpenErrorKind::PathChanged => {
            rpc::RpcError::unity_hub(rpc::error_code::HUB_PATH_CHANGED)
        }
        app::UnityHubOpenErrorKind::PathInvalid => {
            rpc::RpcError::unity_hub(rpc::error_code::HUB_PATH_INVALID)
        }
        app::UnityHubOpenErrorKind::Unavailable => {
            rpc::RpcError::unity_hub(rpc::error_code::HUB_UNAVAILABLE)
        }
        app::UnityHubOpenErrorKind::Busy => {
            rpc::RpcError::unity_hub(rpc::error_code::HUB_OPEN_BUSY)
        }
        app::UnityHubOpenErrorKind::OpenFailed => {
            rpc::RpcError::unity_hub(rpc::error_code::HUB_OPEN_FAILED)
        }
        app::UnityHubOpenErrorKind::Internal => rpc::RpcError::internal(String::new()),
    };
    error.diagnostic_id = diagnostic;
    error_action(Some(id), error, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn settings_and_hub_rpc_reuse_source_diagnostic_without_technical_fields() {
        let diagnostic = app::OperationId::new().to_string();
        let error = app::OfficialGuiError::Unavailable
            .with_source(std::io::Error::other("private I/O cause"))
            .with_path("C:/真实/配置/settings.toml")
            .with_diagnostic_id(Some(diagnostic.clone()));
        let response = official_error("settings-error".to_owned(), error).response;
        assert_eq!(response["error"]["diagnosticId"], diagnostic);
        assert_eq!(response["error"]["code"], "internal_error");
        let encoded = serde_json::to_string(&response).expect("safe settings response");
        assert!(!encoded.contains("private I/O cause"));
        assert!(!encoded.contains("settings.toml"));
        let hub = app::UnityHubOpenError::OpenFailed
            .with_source(std::io::Error::other("private spawn cause"))
            .with_path("C:/真实/Hub.exe")
            .with_diagnostic_id(Some(diagnostic.clone()));
        let response = hub_open_error("hub-error".to_owned(), hub).response;
        assert_eq!(response["error"]["diagnosticId"], diagnostic);
        assert_eq!(response["error"]["code"], "hub_open_failed");
        assert!(
            !serde_json::to_string(&response)
                .expect("safe Hub response")
                .contains("private spawn cause")
        );
        let unrecorded =
            official_error("unrecorded".to_owned(), app::OfficialGuiError::Unavailable).response;
        assert!(unrecorded["error"].get("diagnosticId").is_none());
    }

    #[test]
    fn settings_result_exposes_current_config_without_the_removed_appearance_field() {
        let mut snapshot = app::ConfigSnapshot {
            revision: 91,
            ..app::ConfigSnapshot::default()
        };
        snapshot.settings.appearance.mode = app::ConfigAppearanceMode::Dark;
        snapshot.settings.appearance.scheme = app::ConfigAppearanceScheme::FruitSalad;
        snapshot.settings.appearance.source_color = Some("#AABBCC".to_owned());
        snapshot.settings.appearance.motion = app::ConfigAppearanceMotion::Full;
        snapshot.settings.unity.hub_path = Some(r"C:\用户\Unity Hub.exe".to_owned());
        let result = settings_result(
            snapshot.clone(),
            app::UnityHubLocation::unavailable(true),
            app::StorageDirectories {
                project_directory: "/fixtures/projects".into(),
                backup_directory: "/fixtures/backups".into(),
            },
        );
        assert_eq!(result.config_schema, 10);
        assert_eq!(
            result.config_schema,
            alcomd_store::FileConfigStore::SCHEMA_VERSION
        );
        assert_eq!(result.revision, snapshot.revision);
        assert_eq!(
            result.settings.unity.hub_path,
            snapshot.settings.unity.hub_path
        );
        assert_eq!(
            serde_json::to_value(result.settings.appearance).expect("public appearance"),
            json!({"mode":"dark", "scheme":"fruit-salad", "sourceColor":"#AABBCC", "motion":"full"})
        );
    }

    #[test]
    fn backup_compression_choices_keep_the_same_meaning_in_settings_both_directions() {
        for (wire, internal) in [
            (rpc::BackupCompression::Store, app::BackupCompression::Store),
            (rpc::BackupCompression::Fast, app::BackupCompression::Fast),
            (
                rpc::BackupCompression::Level3,
                app::BackupCompression::Level3,
            ),
            (
                rpc::BackupCompression::Level5,
                app::BackupCompression::Level5,
            ),
            (
                rpc::BackupCompression::Level7,
                app::BackupCompression::Level7,
            ),
            (
                rpc::BackupCompression::Maximum,
                app::BackupCompression::Maximum,
            ),
        ] {
            assert_eq!(backup_compression_from_rpc(wire), internal);
            let mut snapshot = app::ConfigSnapshot::default();
            snapshot.settings.backups.compression_mode = internal;
            let result = settings_result(
                snapshot,
                app::UnityHubLocation::unavailable(false),
                app::StorageDirectories {
                    project_directory: "/fixtures/projects".into(),
                    backup_directory: "/fixtures/backups".into(),
                },
            );
            assert_eq!(result.settings.backups.compression_mode, wire);
        }
    }
}
