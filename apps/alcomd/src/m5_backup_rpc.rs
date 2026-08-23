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
    match request.method.as_str() {
        rpc::METHOD_BACKUPS_LIST => {
            if let Some(action) =
                require_capability(&request.id, state, rpc::CAPABILITY_BACKUPS_READ_V1)
            {
                return action;
            }
            let params: rpc::BackupsListParams = match serde_json::from_value(request.params) {
                Ok(value) => value,
                Err(_) => return invalid(request.id),
            };
            let project_id = match params
                .project_id
                .map(|value| app::ProjectId::parse(&value))
                .transpose()
            {
                Ok(value) => value,
                Err(_) => return invalid(request.id),
            };
            let cursor = match params.cursor.map(parse_cursor).transpose() {
                Ok(value) => value,
                Err(()) => return invalid(request.id),
            };
            match application
                .list(access, project_id, cursor, params.limit.unwrap_or(100))
                .await
            {
                Ok(page) => success_action(
                    request.id,
                    rpc::BackupsListResult {
                        backups: page.backups.into_iter().map(record).collect(),
                        next_cursor: page.next_cursor.map(format_cursor),
                    },
                    None,
                ),
                Err(source) => backup_error(request.id, source),
            }
        }
        rpc::METHOD_BACKUPS_GET => {
            if let Some(action) =
                require_capability(&request.id, state, rpc::CAPABILITY_BACKUPS_READ_V1)
            {
                return action;
            }
            let params: rpc::BackupGetParams = match serde_json::from_value(request.params) {
                Ok(value) => value,
                Err(_) => return invalid(request.id),
            };
            let backup_id = match app::BackupId::parse(&params.backup_id) {
                Ok(value) => value,
                Err(_) => return invalid(request.id),
            };
            match application.get(access, backup_id).await {
                Ok(value) => success_action(request.id, record(value), None),
                Err(source) => backup_error(request.id, source),
            }
        }
        rpc::METHOD_BACKUPS_CREATE => {
            if let Some(action) =
                require_capability(&request.id, state, rpc::CAPABILITY_BACKUPS_CREATE_V1)
            {
                return action;
            }
            let params: rpc::BackupCreateParams = match serde_json::from_value(request.params) {
                Ok(value) => value,
                Err(_) => return invalid(request.id),
            };
            let parsed = app::ProjectId::parse(&params.project_id)
                .ok()
                .zip(Revision::new(params.expected_revision))
                .zip(IdempotencyKey::parse(params.idempotency_key).ok());
            let Some(((project_id, revision), key)) = parsed else {
                return invalid(request.id);
            };
            let compression = match params.compression_mode {
                rpc::BackupCompression::Store => app::BackupCompression::Store,
                rpc::BackupCompression::Fast => app::BackupCompression::Fast,
                rpc::BackupCompression::Maximum => app::BackupCompression::Maximum,
            };
            match application
                .create(
                    access,
                    project_id,
                    revision,
                    compression,
                    params.exclude_vpm_packages,
                    key,
                )
                .await
            {
                Ok(value) => success_action(
                    request.id,
                    rpc::BackupCreateResult {
                        operation_id: value.operation_id.to_string(),
                        backup_id: value.backup_id.to_string(),
                        replayed: value.replayed,
                    },
                    None,
                ),
                Err(source) => backup_error(request.id, source),
            }
        }
        _ => error_action(Some(request.id), rpc::RpcError::method_not_found(), false),
    }
}

fn record(value: app::BackupRecord) -> rpc::BackupRecord {
    rpc::BackupRecord {
        backup_id: value.backup_id.to_string(),
        source_project_id: value.source_project_id.to_string(),
        archive_sha256: hex(&value.archive_sha256),
        archive_bytes: value.archive_bytes,
        format_version: value.format_version,
        created_at_ms: value.created_at_ms,
        compression_mode: match value.compression_mode {
            app::BackupCompression::Store => rpc::BackupCompression::Store,
            app::BackupCompression::Fast => rpc::BackupCompression::Fast,
            app::BackupCompression::Maximum => rpc::BackupCompression::Maximum,
        },
        exclude_vpm_packages: value.exclude_vpm_packages,
    }
}

fn parse_cursor(value: String) -> Result<app::BackupCursor, ()> {
    let (time, id) = value.split_once(':').ok_or(())?;
    Ok(app::BackupCursor {
        created_at_ms: time.parse().map_err(|_| ())?,
        backup_id: app::BackupId::parse(id).map_err(|_| ())?,
    })
}

fn format_cursor(value: app::BackupCursor) -> String {
    format!("{}:{}", value.created_at_ms, value.backup_id)
}
fn hex(value: &[u8; 32]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn backup_error(id: String, source: app::M5BackupError) -> DispatchAction {
    let error = match source.code() {
        app::M5BackupErrorCode::InvalidInput => rpc::RpcError::invalid_request(),
        app::M5BackupErrorCode::PermissionDenied => rpc::RpcError::permission_denied(),
        app::M5BackupErrorCode::RevisionConflict => rpc::RpcError::revision_conflict(),
        app::M5BackupErrorCode::StoreUnavailable => rpc::RpcError::store_unavailable(),
        app::M5BackupErrorCode::Internal => rpc::RpcError::internal(OperationId::new().to_string()),
        _ => rpc::RpcError::backup(app::error_name(source.code())),
    };
    error_action(Some(id), error, false)
}
