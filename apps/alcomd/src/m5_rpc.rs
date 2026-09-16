use alcomd_application as app;
use alcomd_protocol as rpc;

use super::{
    AccessContext, ConnectionState, DispatchAction, IdempotencyKey, M5UnityApplication,
    OperationId, Revision, error_action, invalid, require_capability, success_action,
};

macro_rules! require {
    ($request:ident, $state:ident, $capability:expr) => {
        if let Some(action) = require_capability(&$request.id, $state, $capability) {
            return action;
        }
    };
}

macro_rules! parse {
    ($request:ident, $type:ty) => {
        match serde_json::from_value::<$type>($request.params) {
            Ok(value) => value,
            Err(_) => return invalid($request.id),
        }
    };
}

pub(super) async fn dispatch(
    request: rpc::RequestEnvelope,
    state: &ConnectionState,
    application: &M5UnityApplication,
    access: &AccessContext,
) -> DispatchAction {
    match request.method.as_str() {
        rpc::METHOD_UNITY_INSTALLATIONS_LIST => {
            require!(request, state, rpc::CAPABILITY_UNITY_READ_V1);
            let params = parse!(request, rpc::UnityInstallationsListParams);
            let cursor = match params.cursor.map(parse_cursor).transpose() {
                Ok(value) => value,
                Err(()) => return invalid(request.id),
            };
            match application
                .list_installations(access, cursor, params.limit.unwrap_or(100))
                .await
            {
                Ok(page) => success_action(request.id, page_result(page, false), None),
                Err(error) => m5_error(request.id, error),
            }
        }
        rpc::METHOD_UNITY_INSTALLATIONS_GET => {
            require!(request, state, rpc::CAPABILITY_UNITY_READ_V1);
            let params = parse!(request, rpc::UnityInstallationIdParams);
            let id = match app::UnityInstallationId::parse(&params.installation_id) {
                Ok(value) => value,
                Err(_) => return invalid(request.id),
            };
            match application.get_installation(access, id).await {
                Ok(value) => success_action(
                    request.id,
                    rpc::UnityInstallationResult {
                        installation: installation(value),
                        replayed: false,
                    },
                    None,
                ),
                Err(error) => m5_error(request.id, error),
            }
        }
        rpc::METHOD_UNITY_INSTALLATIONS_REGISTER => {
            require!(request, state, rpc::CAPABILITY_UNITY_MANAGE_V1);
            let params = parse!(request, rpc::UnityInstallationRegisterParams);
            let key = match IdempotencyKey::parse(params.idempotency_key) {
                Ok(value) => value,
                Err(_) => return invalid(request.id),
            };
            match application
                .register_installation(access, params.executable_path, key)
                .await
            {
                Ok((value, replayed)) => success_action(
                    request.id,
                    rpc::UnityInstallationResult {
                        installation: installation(value),
                        replayed,
                    },
                    None,
                ),
                Err(error) => m5_error(request.id, error),
            }
        }
        rpc::METHOD_UNITY_INSTALLATIONS_REMOVE => {
            require!(request, state, rpc::CAPABILITY_UNITY_MANAGE_V1);
            let params = parse!(request, rpc::UnityInstallationRemoveParams);
            let values = app::UnityInstallationId::parse(&params.installation_id)
                .ok()
                .zip(Revision::new(params.expected_revision))
                .zip(IdempotencyKey::parse(params.idempotency_key).ok());
            let Some(((id, revision), key)) = values else {
                return invalid(request.id);
            };
            match application
                .remove_installation(access, id, revision, key)
                .await
            {
                Ok((removed, replayed)) => success_action(
                    request.id,
                    rpc::UnityInstallationRemoveResult {
                        installation_id: id.to_string(),
                        removed,
                        replayed,
                    },
                    None,
                ),
                Err(error) => m5_error(request.id, error),
            }
        }
        rpc::METHOD_UNITY_INSTALLATIONS_REFRESH => {
            require!(request, state, rpc::CAPABILITY_UNITY_MANAGE_V1);
            let params = parse!(request, rpc::UnityInstallationRefreshParams);
            let key = match IdempotencyKey::parse(params.idempotency_key) {
                Ok(value) => value,
                Err(_) => return invalid(request.id),
            };
            match application.refresh_installations(access, key).await {
                Ok((page, replayed)) => {
                    success_action(request.id, page_result(page, replayed), None)
                }
                Err(error) => m5_error(request.id, error),
            }
        }
        rpc::METHOD_UNITY_PROJECT_LAUNCH_CONFIG_GET => {
            require!(request, state, rpc::CAPABILITY_UNITY_READ_V1);
            let params = parse!(request, rpc::UnityProjectIdParams);
            let id = match app::ProjectId::parse(&params.project_id) {
                Ok(value) => value,
                Err(_) => return invalid(request.id),
            };
            match application.get_project_launch_config(access, id).await {
                Ok(value) => success_action(
                    request.id,
                    rpc::ProjectUnityLaunchConfigResult {
                        config: launch_config(value),
                    },
                    None,
                ),
                Err(error) => m5_error(request.id, error),
            }
        }
        rpc::METHOD_UNITY_PROJECT_LAUNCH_CONFIG_SET => {
            require!(request, state, rpc::CAPABILITY_UNITY_MANAGE_V1);
            let params = parse!(request, rpc::ProjectUnityLaunchConfigSetParams);
            let parsed = app::ProjectId::parse(&params.project_id)
                .ok()
                .zip(IdempotencyKey::parse(params.idempotency_key).ok());
            let Some((project_id, key)) = parsed else {
                return invalid(request.id);
            };
            let expected = if params.expected_revision == 0 {
                None
            } else {
                Revision::new(params.expected_revision)
            };
            if params.expected_revision != 0 && expected.is_none() {
                return invalid(request.id);
            }
            match application
                .set_project_launch_config(access, project_id, params.arguments, expected, key)
                .await
            {
                Ok((value, changed, replayed)) => success_action(
                    request.id,
                    rpc::ProjectUnityLaunchConfigMutationResult {
                        config: launch_config(value),
                        changed,
                        replayed,
                    },
                    None,
                ),
                Err(error) => m5_error(request.id, error),
            }
        }
        rpc::METHOD_UNITY_PROJECT_LAUNCH_CONFIG_CLEAR => {
            require!(request, state, rpc::CAPABILITY_UNITY_MANAGE_V1);
            let params = parse!(request, rpc::ProjectUnityLaunchConfigClearParams);
            let parsed = app::ProjectId::parse(&params.project_id)
                .ok()
                .zip(IdempotencyKey::parse(params.idempotency_key).ok());
            let Some((project_id, key)) = parsed else {
                return invalid(request.id);
            };
            let expected = if params.expected_revision == 0 {
                None
            } else {
                Revision::new(params.expected_revision)
            };
            if params.expected_revision != 0 && expected.is_none() {
                return invalid(request.id);
            }
            match application
                .clear_project_launch_config(access, project_id, expected, key)
                .await
            {
                Ok((value, changed, replayed)) => success_action(
                    request.id,
                    rpc::ProjectUnityLaunchConfigMutationResult {
                        config: launch_config(value),
                        changed,
                        replayed,
                    },
                    None,
                ),
                Err(error) => m5_error(request.id, error),
            }
        }
        rpc::METHOD_UNITY_WRITER_STATE => {
            require!(request, state, rpc::CAPABILITY_UNITY_READ_V1);
            let params = parse!(request, rpc::UnityProjectIdParams);
            let id = match app::ProjectId::parse(&params.project_id) {
                Ok(value) => value,
                Err(_) => return invalid(request.id),
            };
            match application.writer_state(access, id).await {
                Ok(value) => success_action(request.id, writer_state(value), None),
                Err(error) => m5_error(request.id, error),
            }
        }
        rpc::METHOD_UNITY_LAUNCH_OPTIONS => {
            require!(request, state, rpc::CAPABILITY_UNITY_LAUNCH_V1);
            let params = parse!(request, rpc::UnityLaunchOptionsParams);
            let values = app::ProjectId::parse(&params.project_id)
                .ok()
                .zip(Revision::new(params.expected_project_revision));
            let Some((project_id, revision)) = values else {
                return invalid(request.id);
            };
            match application
                .launch_options(access, project_id, revision)
                .await
            {
                Ok(value) => success_action(
                    request.id,
                    rpc::UnityLaunchOptionsResult {
                        project_id: value.project_id.to_string(),
                        project_revision: value.project_revision.get(),
                        project_unity_version: value.project_unity_version,
                        exact_matching_installations: value
                            .exact_matching_installations
                            .into_iter()
                            .map(installation)
                            .collect(),
                    },
                    None,
                ),
                Err(error) => m5_error(request.id, error),
            }
        }
        rpc::METHOD_UNITY_LAUNCH => {
            require!(request, state, rpc::CAPABILITY_UNITY_LAUNCH_V1);
            let params = parse!(request, rpc::UnityLaunchParams);
            let parsed = app::ProjectId::parse(&params.project_id)
                .ok()
                .zip(app::UnityInstallationId::parse(&params.installation_id).ok())
                .zip(Revision::new(params.expected_project_revision))
                .zip(IdempotencyKey::parse(params.idempotency_key).ok());
            let Some((((project_id, installation_id), revision), key)) = parsed else {
                return invalid(request.id);
            };
            match application
                .launch(access, project_id, installation_id, revision, key)
                .await
            {
                Ok((value, replayed)) => success_action(
                    request.id,
                    rpc::UnityLaunchResult {
                        launch: launch(value),
                        replayed,
                    },
                    None,
                ),
                Err(error) => m5_error(request.id, error),
            }
        }
        rpc::METHOD_UNITY_LAUNCH_STATUS => {
            require!(request, state, rpc::CAPABILITY_UNITY_LAUNCH_V1);
            let params = parse!(request, rpc::UnityLaunchStatusParams);
            let id = match app::UnityLaunchId::parse(&params.launch_id) {
                Ok(value) => value,
                Err(_) => return invalid(request.id),
            };
            match application.launch_status(access, id).await {
                Ok(value) => success_action(
                    request.id,
                    rpc::UnityLaunchResult {
                        launch: launch(value),
                        replayed: false,
                    },
                    None,
                ),
                Err(error) => m5_error(request.id, error),
            }
        }
        _ => error_action(Some(request.id), rpc::RpcError::method_not_found(), false),
    }
}

fn page_result(
    page: app::UnityInstallationPage,
    replayed: bool,
) -> rpc::UnityInstallationsListResult {
    rpc::UnityInstallationsListResult {
        installations: page.installations.into_iter().map(installation).collect(),
        next_cursor: page.next_cursor.map(format_cursor),
        replayed,
    }
}

fn installation(value: app::UnityInstallationRecord) -> rpc::UnityInstallation {
    rpc::UnityInstallation {
        installation_id: value.installation_id.to_string(),
        executable_path: value.observation.executable_path,
        filesystem_identity: hex(&value.observation.filesystem_identity),
        unity_version: value.observation.unity_version,
        architecture: match value.observation.architecture {
            app::UnityArchitecture::X86_64 => rpc::UnityArchitecture::X86_64,
            app::UnityArchitecture::Arm64 => rpc::UnityArchitecture::Arm64,
            app::UnityArchitecture::Universal => rpc::UnityArchitecture::Universal,
            app::UnityArchitecture::Unknown => rpc::UnityArchitecture::Unknown,
        },
        source_kind: match value.observation.source_kind {
            app::UnitySourceKind::Manual => rpc::UnitySourceKind::Manual,
            app::UnitySourceKind::HubConfig => rpc::UnitySourceKind::HubConfig,
            app::UnitySourceKind::KnownInstallRoot => rpc::UnitySourceKind::KnownInstallRoot,
            app::UnitySourceKind::UnityCliHint => rpc::UnitySourceKind::UnityCliHint,
        },
        revision: value.revision.get(),
        observed_at_ms: value.observation.observed_at_ms,
        updated_at_ms: value.updated_at_ms,
    }
}

fn launch_config(value: app::ProjectUnityLaunchConfig) -> rpc::ProjectUnityLaunchConfig {
    rpc::ProjectUnityLaunchConfig {
        project_id: value.project_id.to_string(),
        arguments: value.arguments,
        revision: value.revision.map_or(0, Revision::get),
        updated_at_ms: value.updated_at_ms,
    }
}

fn writer_state(value: app::UnityWriterState) -> rpc::UnityWriterState {
    rpc::UnityWriterState {
        project_id: value.project_id.to_string(),
        state: match value.state {
            app::UnityWriterStateKind::RunningConfirmed => {
                rpc::UnityWriterStateKind::RunningConfirmed
            }
            app::UnityWriterStateKind::RunningSuspected => {
                rpc::UnityWriterStateKind::RunningSuspected
            }
            app::UnityWriterStateKind::NotObserved => rpc::UnityWriterStateKind::NotObserved,
            app::UnityWriterStateKind::Unknown => rpc::UnityWriterStateKind::Unknown,
        },
        evidence: value
            .evidence
            .into_iter()
            .map(|kind| rpc::UnityWriterEvidence {
                kind: match kind {
                    app::UnityWriterEvidenceKind::ProcessProjectArgument => {
                        rpc::UnityWriterEvidenceKind::ProcessProjectArgument
                    }
                    app::UnityWriterEvidenceKind::ProcessUnreadable => {
                        rpc::UnityWriterEvidenceKind::ProcessUnreadable
                    }
                    app::UnityWriterEvidenceKind::InspectionError => {
                        rpc::UnityWriterEvidenceKind::InspectionError
                    }
                },
            })
            .collect(),
        checked_at_ms: value.checked_at_ms,
    }
}

fn launch(value: app::UnityLaunchRecord) -> rpc::UnityLaunchRecord {
    rpc::UnityLaunchRecord {
        launch_id: value.launch_id.to_string(),
        project_id: value.project_id.to_string(),
        installation_id: value.installation_id.to_string(),
        state: match value.state {
            app::UnityLaunchState::Opening => rpc::UnityLaunchState::Opening,
            app::UnityLaunchState::Open => rpc::UnityLaunchState::Open,
            app::UnityLaunchState::Failed => rpc::UnityLaunchState::Failed,
        },
        spawn_accepted: value.spawn_accepted,
        created_at_ms: value.created_at_ms,
    }
}

fn parse_cursor(value: String) -> Result<app::UnityInstallationCursor, ()> {
    let (time, id) = value.split_once(':').ok_or(())?;
    Ok(app::UnityInstallationCursor {
        updated_at_ms: time.parse().map_err(|_| ())?,
        installation_id: app::UnityInstallationId::parse(id).map_err(|_| ())?,
    })
}

fn format_cursor(value: app::UnityInstallationCursor) -> String {
    format!("{}:{}", value.updated_at_ms, value.installation_id)
}

fn m5_error(id: String, error: app::M5UnityError) -> DispatchAction {
    let rpc_error = match error.code() {
        app::M5UnityErrorCode::InvalidInput => rpc::RpcError::invalid_request(),
        app::M5UnityErrorCode::PermissionDenied => rpc::RpcError::permission_denied(),
        app::M5UnityErrorCode::RevisionConflict => rpc::RpcError::revision_conflict(),
        app::M5UnityErrorCode::IdempotencyConflict => rpc::RpcError::idempotency_conflict(),
        app::M5UnityErrorCode::ProjectNotRegistered => {
            rpc::RpcError::m3_resource(rpc::error_code::PROJECT_NOT_REGISTERED)
        }
        app::M5UnityErrorCode::InstallationNotFound => {
            rpc::RpcError::unity(rpc::error_code::UNITY_INSTALLATION_NOT_FOUND)
        }
        app::M5UnityErrorCode::InstallationInvalid => {
            rpc::RpcError::unity(rpc::error_code::UNITY_INSTALLATION_INVALID)
        }
        app::M5UnityErrorCode::InstallationInUse => {
            rpc::RpcError::unity(rpc::error_code::UNITY_INSTALLATION_IN_USE)
        }
        app::M5UnityErrorCode::EditorSelectionRequired => {
            rpc::RpcError::unity(rpc::error_code::UNITY_EDITOR_SELECTION_REQUIRED)
        }
        app::M5UnityErrorCode::VersionUnverified => {
            rpc::RpcError::unity(rpc::error_code::UNITY_VERSION_UNVERIFIED)
        }
        app::M5UnityErrorCode::VersionMismatch => {
            rpc::RpcError::unity(rpc::error_code::UNITY_VERSION_MISMATCH)
        }
        app::M5UnityErrorCode::ProjectSelectorForbidden => {
            rpc::RpcError::unity(rpc::error_code::UNITY_PROJECT_SELECTOR_FORBIDDEN)
        }
        app::M5UnityErrorCode::ProjectRunning => {
            rpc::RpcError::unity(rpc::error_code::UNITY_PROJECT_RUNNING)
        }
        app::M5UnityErrorCode::LaunchStateUncertain => {
            rpc::RpcError::unity(rpc::error_code::UNITY_LAUNCH_STATE_UNCERTAIN)
        }
        app::M5UnityErrorCode::LaunchFailed => {
            rpc::RpcError::unity(rpc::error_code::UNITY_LAUNCH_FAILED)
        }
        app::M5UnityErrorCode::LaunchNotFound => {
            rpc::RpcError::unity(rpc::error_code::UNITY_LAUNCH_NOT_FOUND)
        }
        app::M5UnityErrorCode::StoreUnavailable => rpc::RpcError::store_unavailable(),
        app::M5UnityErrorCode::Internal => rpc::RpcError::internal(OperationId::new().to_string()),
    };
    error_action(Some(id), rpc_error, false)
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push(char::from(HEX[(byte >> 4) as usize]));
        value.push(char::from(HEX[(byte & 0x0f) as usize]));
    }
    value
}
