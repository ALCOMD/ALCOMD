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
                    success_action(request.id, settings_result(value, hub), None)
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
                        density: value.density.map(appearance_density_from_rpc),
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
                    success_action(request.id, settings_result(value, hub), None)
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
) -> rpc::SettingsGetResult {
    rpc::SettingsGetResult {
        config_schema: rpc::CONFIG_SCHEMA_VERSION,
        revision: value.revision,
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
                density: match value.settings.appearance.density {
                    app::ConfigAppearanceDensity::Default => rpc::AppearanceDensity::Default,
                    app::ConfigAppearanceDensity::Compact => rpc::AppearanceDensity::Compact,
                },
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

fn appearance_density_from_rpc(value: rpc::AppearanceDensity) -> app::ConfigAppearanceDensity {
    match value {
        rpc::AppearanceDensity::Default => app::ConfigAppearanceDensity::Default,
        rpc::AppearanceDensity::Compact => app::ConfigAppearanceDensity::Compact,
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
