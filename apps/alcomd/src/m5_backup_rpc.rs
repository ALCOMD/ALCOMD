use alcomd_application as app;
use alcomd_protocol as rpc;

use super::{
    AccessContext, BackupApplication, ConnectionState, DispatchAction, IdempotencyKey, OperationId,
    Revision, error_action, invalid, require_capability, success_action,
};

pub(super) async fn dispatch(
    request: rpc::RequestEnvelope,
    state: &ConnectionState,
    application: &BackupApplication,
    access: &AccessContext,
) -> DispatchAction {
    let capability = match request.method.as_str() {
        rpc::METHOD_BACKUPS_CREATE => rpc::CAPABILITY_BACKUPS_CREATE_V2,
        rpc::METHOD_BACKUPS_RESTORE if !access.is_mcp_client() => {
            rpc::CAPABILITY_BACKUPS_RESTORE_V2
        }
        rpc::METHOD_MCP_BACKUPS_RESTORE if access.is_mcp_client() => {
            rpc::CAPABILITY_MCP_PROJECT_WORKFLOWS_V2
        }
        _ => return error_action(Some(request.id), rpc::RpcError::method_not_found(), false),
    };
    if let Some(action) = require_capability(&request.id, state, capability) {
        return action;
    }
    let outcome = match request.method.as_str() {
        rpc::METHOD_BACKUPS_CREATE => {
            let Ok(params) = serde_json::from_value::<rpc::BackupCreateParams>(request.params)
            else {
                return invalid(request.id);
            };
            let parsed = app::ProjectId::parse(&params.project_id)
                .ok()
                .zip(Revision::new(params.expected_revision))
                .zip(IdempotencyKey::parse(params.idempotency_key).ok());
            let Some(((project_id, revision), key)) = parsed else {
                return invalid(request.id);
            };
            application
                .create(
                    access,
                    project_id,
                    revision,
                    app::BackupCreateOptions {
                        compression_mode: params.compression_mode.map(compression_mode),
                        exclude_vpm_packages: params.exclude_vpm_packages,
                        include_library: params.include_library,
                        destination_path: params.destination_path,
                    },
                    key,
                )
                .await
        }
        _ => {
            let params = if access.is_mcp_client() {
                let Ok(params) =
                    serde_json::from_value::<rpc::McpBackupRestoreParams>(request.params)
                else {
                    return invalid(request.id);
                };
                if !params.confirm {
                    return invalid(request.id);
                }
                rpc::BackupRestoreParams {
                    archive_path: params.archive_path,
                    destination_path: params.destination_path,
                    project_name: params.project_name,
                    idempotency_key: params.idempotency_key,
                }
            } else {
                let Ok(params) = serde_json::from_value::<rpc::BackupRestoreParams>(request.params)
                else {
                    return invalid(request.id);
                };
                params
            };
            let Ok(key) = IdempotencyKey::parse(params.idempotency_key) else {
                return invalid(request.id);
            };
            application
                .restore(
                    access,
                    params.archive_path,
                    params.project_name,
                    params.destination_path,
                    key,
                )
                .await
        }
    };
    match outcome {
        Ok(value) => success_action(
            request.id,
            rpc::OperationAccepted {
                operation_id: value.operation_id.to_string(),
                replayed: value.replayed,
            },
            None,
        ),
        Err(source) => backup_error(request.id, source),
    }
}

fn compression_mode(value: rpc::BackupCompression) -> app::BackupCompression {
    match value {
        rpc::BackupCompression::Store => app::BackupCompression::Store,
        rpc::BackupCompression::Fast => app::BackupCompression::Fast,
        rpc::BackupCompression::Level3 => app::BackupCompression::Level3,
        rpc::BackupCompression::Level5 => app::BackupCompression::Level5,
        rpc::BackupCompression::Level7 => app::BackupCompression::Level7,
        rpc::BackupCompression::Maximum => app::BackupCompression::Maximum,
    }
}

pub(super) fn backup_error(id: String, source: app::M5BackupError) -> DispatchAction {
    let diagnostic_id = source.diagnostic_id().map(str::to_owned);
    let mut error = match source.code() {
        app::M5BackupErrorCode::InvalidInput => rpc::RpcError::invalid_request(),
        app::M5BackupErrorCode::PermissionDenied => rpc::RpcError::permission_denied(),
        app::M5BackupErrorCode::RevisionConflict => rpc::RpcError::revision_conflict(),
        app::M5BackupErrorCode::StoreUnavailable => rpc::RpcError::store_unavailable(),
        app::M5BackupErrorCode::Internal => rpc::RpcError::internal(OperationId::new().to_string()),
        _ => rpc::RpcError::backup(app::error_name(source.code())),
    };
    if diagnostic_id.is_some() {
        error.diagnostic_id = diagnostic_id;
    }
    error_action(Some(id), error, false)
}
