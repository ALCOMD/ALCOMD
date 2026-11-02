use alcomd_application as app;
use alcomd_protocol as rpc;

use super::{
    AccessContext, ConnectionState, DispatchAction, M7OfficialGuiApplication, OperationId,
    error_action, invalid, require_capability, success_action,
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
                paths: params.update.paths.map(|value| app::ConfigPathUpdate {
                    project_directory: directory_update_from_rpc(value.project_directory),
                    backup_directory: directory_update_from_rpc(value.backup_directory),
                }),
                unity: params.update.unity.map(|value| app::ConfigUnityUpdate {
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
        rpc::METHOD_ACTIVITY_LIST => {
            let params: rpc::ActivityListParams = match serde_json::from_value(request.params) {
                Ok(value) => value,
                Err(_) => return invalid(request.id),
            };
            let cursor = params.cursor.map(|value| app::OfficialActivityCursor {
                occurred_at_ms: value.occurred_at_ms,
                source_rank: value.source_rank,
                stable_id: value.stable_id,
            });
            match application
                .list_activity(access, cursor, params.limit)
                .await
            {
                Ok(page) => success_action(
                    request.id,
                    rpc::ActivityListResult {
                        items: page
                            .items
                            .into_iter()
                            .map(|item| rpc::ActivityItem {
                                occurred_at_ms: item.occurred_at_ms,
                                item_type: match item.kind {
                                    app::OfficialActivityKind::Operation => {
                                        rpc::ActivityItemType::Operation
                                    }
                                    app::OfficialActivityKind::Event => {
                                        rpc::ActivityItemType::Event
                                    }
                                },
                                summary_code: item.summary_code,
                                operation_id: item.operation_id,
                                event_sequence: item.event_sequence,
                                resource_kind: item.resource_kind,
                                resource_id: item.resource_id,
                                state: item.state,
                            })
                            .collect(),
                        next_cursor: page.next_cursor.map(|value| rpc::ActivityCursor {
                            occurred_at_ms: value.occurred_at_ms,
                            source_rank: value.source_rank,
                            stable_id: value.stable_id,
                        }),
                    },
                    None,
                ),
                Err(error) => official_error(request.id, error),
            }
        }
        rpc::METHOD_DIAGNOSTICS_LIST => {
            let params: rpc::DiagnosticsListParams = match serde_json::from_value(request.params) {
                Ok(value) => value,
                Err(_) => return invalid(request.id),
            };
            let cursor = params.cursor.map(|value| app::OfficialDiagnosticCursor {
                occurred_at_ms: value.occurred_at_ms,
                operation_id: value.operation_id,
            });
            match application
                .list_diagnostics(access, cursor, params.limit)
                .await
            {
                Ok(page) => success_action(
                    request.id,
                    rpc::DiagnosticsListResult {
                        items: page
                            .items
                            .into_iter()
                            .map(|item| rpc::DiagnosticItem {
                                occurred_at_ms: item.occurred_at_ms,
                                severity: rpc::DiagnosticSeverity::Error,
                                subsystem: item.subsystem,
                                code: item.code,
                                diagnostic_id: item.diagnostic_id,
                                operation_id: Some(item.operation_id),
                                summary: "The operation failed. Use the diagnostic ID when requesting support."
                                    .to_owned(),
                            })
                            .collect(),
                        next_cursor: page.next_cursor.map(|value| rpc::DiagnosticCursor {
                            occurred_at_ms: value.occurred_at_ms,
                            operation_id: value.operation_id,
                        }),
                    },
                    None,
                ),
                Err(error) => official_error(request.id, error),
            }
        }
        _ => error_action(Some(request.id), rpc::RpcError::method_not_found(), false),
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
            paths: rpc::PathSettings {
                project_directory: value.settings.paths.project_directory,
                backup_directory: value.settings.paths.backup_directory,
            },
            unity: rpc::UnitySettings {
                hub_path: value.settings.unity.hub_path,
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
    let error = match error {
        app::OfficialGuiError::InvalidInput => rpc::RpcError::invalid_request(),
        app::OfficialGuiError::PermissionDenied => rpc::RpcError::permission_denied(),
        app::OfficialGuiError::RevisionConflict => rpc::RpcError::revision_conflict(),
        app::OfficialGuiError::Unavailable | app::OfficialGuiError::Corrupt => {
            rpc::RpcError::internal(OperationId::new().to_string())
        }
    };
    error_action(Some(id), error, false)
}

fn hub_open_error(id: String, error: app::UnityHubOpenError) -> DispatchAction {
    let error = match error {
        app::UnityHubOpenError::InvalidInput => rpc::RpcError::invalid_request(),
        app::UnityHubOpenError::PermissionDenied => rpc::RpcError::permission_denied(),
        app::UnityHubOpenError::RevisionConflict => rpc::RpcError::revision_conflict(),
        app::UnityHubOpenError::PathChanged => {
            rpc::RpcError::unity_hub(rpc::error_code::HUB_PATH_CHANGED)
        }
        app::UnityHubOpenError::PathInvalid => {
            rpc::RpcError::unity_hub(rpc::error_code::HUB_PATH_INVALID)
        }
        app::UnityHubOpenError::Unavailable => {
            rpc::RpcError::unity_hub(rpc::error_code::HUB_UNAVAILABLE)
        }
        app::UnityHubOpenError::Busy => rpc::RpcError::unity_hub(rpc::error_code::HUB_OPEN_BUSY),
        app::UnityHubOpenError::OpenFailed => {
            rpc::RpcError::unity_hub(rpc::error_code::HUB_OPEN_FAILED)
        }
        app::UnityHubOpenError::Internal => rpc::RpcError::internal(OperationId::new().to_string()),
    };
    error_action(Some(id), error, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn settings_result_exposes_config4_without_the_removed_appearance_field() {
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
        assert_eq!(result.config_schema, 5);
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
}
