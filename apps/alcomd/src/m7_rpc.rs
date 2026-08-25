use alcomd_application as app;
use alcomd_protocol as rpc;

use super::{
    AccessContext, ConnectionState, DispatchAction, M7ExtensionApplication, OperationId,
    error_action, invalid, require_capability, success_action,
};

#[derive(Clone, Copy)]
pub(super) struct ProtocolUiValidator;

impl app::M7UiValidator for ProtocolUiValidator {
    fn validate_document(&self, document: &[u8]) -> Result<(), app::M7ValidationFailure> {
        let document = serde_json::from_slice::<rpc::UiDocument>(document)
            .map_err(|_| app::M7ValidationFailure::Invalid)?;
        document.validate().map_err(map_validation)?;
        let snapshot = rpc::UiSnapshot {
            session_id: "00000000-0000-4000-8000-000000000000".to_owned(),
            snapshot_revision: i64::MAX as u64,
            document,
        };
        if serde_json::to_vec(&snapshot).map_or(true, |encoded| {
            encoded.len() > rpc::PORTABLE_UI_SNAPSHOT_BYTES
        }) {
            return Err(app::M7ValidationFailure::LimitExceeded);
        }
        Ok(())
    }

    fn validate_action(
        &self,
        document: &[u8],
        action: &[u8],
    ) -> Result<(), app::M7ValidationFailure> {
        let document = serde_json::from_slice::<rpc::UiDocument>(document)
            .map_err(|_| app::M7ValidationFailure::Invalid)?;
        let action = serde_json::from_slice::<rpc::UiAction>(action)
            .map_err(|_| app::M7ValidationFailure::Invalid)?;
        document.validate_action(&action).map_err(map_validation)
    }
}

pub(super) async fn dispatch(
    request: rpc::RequestEnvelope,
    state: &ConnectionState,
    application: &M7ExtensionApplication,
    access: &AccessContext,
) -> DispatchAction {
    if let Some(action) = require_capability(
        &request.id,
        state,
        rpc::CAPABILITY_EXTENSIONS_UI_PORTABLE_V1,
    ) {
        return action;
    }
    match request.method.as_str() {
        rpc::METHOD_EXTENSIONS_UI_OPEN => {
            let params: rpc::ExtensionUiOpenParams = match serde_json::from_value(request.params) {
                Ok(value) => value,
                Err(_) => return invalid(request.id),
            };
            match application
                .open(
                    access,
                    state.connection_id.clone(),
                    state.client_instance_id.clone().unwrap_or_default(),
                    params.extension_id,
                    params.locale,
                    now_ms(),
                )
                .await
            {
                Ok(result) => match open_result(result) {
                    Ok(result) => success_action(request.id, result, None),
                    Err(error) => m7_error(request.id, error),
                },
                Err(error) => m7_error(request.id, error),
            }
        }
        rpc::METHOD_EXTENSIONS_UI_REFRESH => {
            let params: rpc::ExtensionUiRefreshParams = match serde_json::from_value(request.params)
            {
                Ok(value) => value,
                Err(_) => return invalid(request.id),
            };
            match application
                .refresh(
                    access,
                    &state.connection_id,
                    &params.session_id,
                    params.expected_snapshot_revision,
                    now_ms(),
                )
                .await
            {
                Ok(snapshot) => match rpc_snapshot(snapshot) {
                    Ok(snapshot) => success_action(
                        request.id,
                        rpc::ExtensionUiSnapshotResult { snapshot },
                        None,
                    ),
                    Err(error) => m7_error(request.id, error),
                },
                Err(error) => m7_error(request.id, error),
            }
        }
        rpc::METHOD_EXTENSIONS_UI_DISPATCH => {
            let params: rpc::ExtensionUiDispatchParams =
                match serde_json::from_value(request.params) {
                    Ok(value) => value,
                    Err(_) => return invalid(request.id),
                };
            if serde_json::to_vec(&params).map_or(true, |encoded| {
                encoded.len() > rpc::PORTABLE_UI_DISPATCH_BYTES
            }) {
                let error = application
                    .reject_client_request(
                        access,
                        &state.connection_id,
                        &params.session_id,
                        app::M7ErrorCode::LimitExceeded,
                        now_ms(),
                    )
                    .await;
                return m7_error(request.id, error);
            }
            let action = match serde_json::to_vec(&params.action) {
                Ok(action) => action,
                Err(_) => return invalid(request.id),
            };
            let fingerprint = alcomd_extensions::portable_ui_action_fingerprint(&action);
            match application
                .dispatch(
                    access,
                    &state.connection_id,
                    &params.session_id,
                    params.expected_snapshot_revision,
                    params.sequence,
                    params.request_id,
                    action,
                    fingerprint,
                    now_ms(),
                )
                .await
            {
                Ok(result) => match rpc_snapshot(result.snapshot) {
                    Ok(snapshot) => success_action(
                        request.id,
                        rpc::ExtensionUiDispatchResult {
                            snapshot,
                            replayed: result.replayed,
                        },
                        None,
                    ),
                    Err(error) => m7_error(request.id, error),
                },
                Err(error) => m7_error(request.id, error),
            }
        }
        rpc::METHOD_EXTENSIONS_UI_CLOSE => {
            let params: rpc::ExtensionUiCloseParams = match serde_json::from_value(request.params) {
                Ok(value) => value,
                Err(_) => return invalid(request.id),
            };
            let closed = application
                .close(access, &state.connection_id, &params.session_id, now_ms())
                .await;
            success_action(request.id, rpc::ExtensionUiCloseResult { closed }, None)
        }
        _ => error_action(Some(request.id), rpc::RpcError::method_not_found(), false),
    }
}

fn open_result(result: app::M7UiOpenResult) -> Result<rpc::ExtensionUiOpenResult, app::M7Error> {
    Ok(rpc::ExtensionUiOpenResult {
        session: rpc::ExtensionUiSession {
            session_id: result.session.session_id,
            extension_id: result.session.extension_id,
            locale: result.session.locale,
            idle_timeout_ms: result.session.idle_timeout_ms,
            absolute_timeout_ms: result.session.absolute_timeout_ms,
        },
        snapshot: rpc_snapshot(result.snapshot)?,
    })
}

fn rpc_snapshot(value: app::M7UiSnapshot) -> Result<rpc::UiSnapshot, app::M7Error> {
    let document = serde_json::from_slice(&value.document)
        .map_err(|_| app::M7Error::new(app::M7ErrorCode::Internal))?;
    Ok(rpc::UiSnapshot {
        session_id: value.session_id,
        snapshot_revision: value.snapshot_revision,
        document,
    })
}

fn m7_error(id: String, source: app::M7Error) -> DispatchAction {
    let error = match source.code() {
        app::M7ErrorCode::InvalidInput => rpc::RpcError::invalid_request(),
        app::M7ErrorCode::PermissionDenied => rpc::RpcError::permission_denied(),
        app::M7ErrorCode::Internal => rpc::RpcError::internal(OperationId::new().to_string()),
        code => rpc::RpcError::extension(error_name(code)),
    };
    error_action(Some(id), error, false)
}

fn error_name(code: app::M7ErrorCode) -> &'static str {
    match code {
        app::M7ErrorCode::NotInstalled => rpc::error_code::EXTENSION_NOT_INSTALLED,
        app::M7ErrorCode::NotEnabled => rpc::error_code::EXTENSION_NOT_ENABLED,
        app::M7ErrorCode::Quarantined => rpc::error_code::EXTENSION_QUARANTINED,
        app::M7ErrorCode::UiNotAvailable => rpc::error_code::EXTENSION_UI_NOT_AVAILABLE,
        app::M7ErrorCode::UiProtocolUnsupported => {
            rpc::error_code::EXTENSION_UI_PROTOCOL_UNSUPPORTED
        }
        app::M7ErrorCode::SessionNotFound => rpc::error_code::EXTENSION_UI_SESSION_NOT_FOUND,
        app::M7ErrorCode::SessionStale => rpc::error_code::EXTENSION_UI_SESSION_STALE,
        app::M7ErrorCode::SnapshotStale => rpc::error_code::EXTENSION_UI_SNAPSHOT_STALE,
        app::M7ErrorCode::DocumentInvalid => rpc::error_code::EXTENSION_UI_DOCUMENT_INVALID,
        app::M7ErrorCode::ActionInvalid => rpc::error_code::EXTENSION_UI_ACTION_INVALID,
        app::M7ErrorCode::LimitExceeded => rpc::error_code::EXTENSION_UI_LIMIT_EXCEEDED,
        app::M7ErrorCode::ExtensionPermissionDenied => rpc::error_code::EXTENSION_PERMISSION_DENIED,
        app::M7ErrorCode::ExtensionScopeDenied => rpc::error_code::EXTENSION_SCOPE_DENIED,
        app::M7ErrorCode::InstanceStale => rpc::error_code::EXTENSION_INSTANCE_STALE,
        app::M7ErrorCode::Crashed => rpc::error_code::EXTENSION_CRASHED,
        app::M7ErrorCode::ResourceLimit => rpc::error_code::EXTENSION_RESOURCE_LIMIT,
        app::M7ErrorCode::StoreUnavailable => rpc::error_code::STORE_UNAVAILABLE,
        app::M7ErrorCode::InvalidInput
        | app::M7ErrorCode::PermissionDenied
        | app::M7ErrorCode::Internal => rpc::error_code::INTERNAL_ERROR,
    }
}

fn map_validation(error: rpc::PortableUiValidationError) -> app::M7ValidationFailure {
    match error {
        rpc::PortableUiValidationError::Invalid => app::M7ValidationFailure::Invalid,
        rpc::PortableUiValidationError::LimitExceeded => app::M7ValidationFailure::LimitExceeded,
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}
