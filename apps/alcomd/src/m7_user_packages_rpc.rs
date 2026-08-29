use alcomd_application as app;
use alcomd_protocol as rpc;

use super::{
    AccessContext, ConnectionState, DispatchAction, IdempotencyKey, OperationId, Revision,
    UserPackageApplication, error_action, invalid, require_capability, success_action,
};

pub(super) async fn dispatch(
    request: rpc::RequestEnvelope,
    state: &ConnectionState,
    application: &UserPackageApplication,
    access: &AccessContext,
) -> DispatchAction {
    if let Some(value) = require_capability(
        &request.id,
        state,
        rpc::CAPABILITY_PACKAGES_USER_PACKAGES_V1,
    ) {
        return value;
    }
    match request.method.as_str() {
        rpc::METHOD_PACKAGES_USER_PACKAGES_LIST => list(request, application, access).await,
        rpc::METHOD_PACKAGES_USER_PACKAGES_GET => get(request, application, access).await,
        rpc::METHOD_PACKAGES_USER_PACKAGES_ENROLL => enroll(request, application, access).await,
        rpc::METHOD_PACKAGES_USER_PACKAGES_REFRESH => refresh(request, application, access).await,
        rpc::METHOD_PACKAGES_USER_PACKAGES_REMOVE => remove(request, application, access).await,
        _ => error_action(Some(request.id), rpc::RpcError::method_not_found(), false),
    }
}

async fn list(
    request: rpc::RequestEnvelope,
    application: &UserPackageApplication,
    access: &AccessContext,
) -> DispatchAction {
    let params: rpc::UserPackagesListParams = match serde_json::from_value(request.params) {
        Ok(value) => value,
        Err(_) => return invalid(request.id),
    };
    let cursor = match params.cursor.map(cursor_from_rpc).transpose() {
        Ok(value) => value,
        Err(()) => return invalid(request.id),
    };
    match application
        .list(
            access,
            cursor,
            params.limit.unwrap_or(app::DEFAULT_USER_PACKAGE_PAGE_LIMIT),
        )
        .await
    {
        Ok(page) => success_action(
            request.id,
            rpc::UserPackagesListResult {
                user_packages: page.user_packages.into_iter().map(record).collect(),
                next_cursor: page.next_cursor.map(cursor_to_rpc),
            },
            None,
        ),
        Err(error) => user_package_error(request.id, error),
    }
}

async fn get(
    request: rpc::RequestEnvelope,
    application: &UserPackageApplication,
    access: &AccessContext,
) -> DispatchAction {
    let params: rpc::UserPackageGetParams = match serde_json::from_value(request.params) {
        Ok(value) => value,
        Err(_) => return invalid(request.id),
    };
    let id = match app::UserPackageId::parse(&params.user_package_id) {
        Ok(value) => value,
        Err(_) => return invalid(request.id),
    };
    match application.get(access, id).await {
        Ok(value) => success_action(
            request.id,
            rpc::UserPackageResult {
                user_package: record(value),
            },
            None,
        ),
        Err(error) => user_package_error(request.id, error),
    }
}

async fn enroll(
    request: rpc::RequestEnvelope,
    application: &UserPackageApplication,
    access: &AccessContext,
) -> DispatchAction {
    let params: rpc::UserPackageEnrollParams = match serde_json::from_value(request.params) {
        Ok(value) => value,
        Err(_) => return invalid(request.id),
    };
    if params.source_path.is_empty() || params.source_path.len() > 32_768 {
        return invalid(request.id);
    }
    let key = match IdempotencyKey::parse(params.idempotency_key) {
        Ok(value) => value,
        Err(_) => return invalid(request.id),
    };
    match application.enroll(access, params.source_path, key).await {
        Ok(value) => success_action(request.id, write_result(value), None),
        Err(error) => user_package_error(request.id, error),
    }
}

async fn refresh(
    request: rpc::RequestEnvelope,
    application: &UserPackageApplication,
    access: &AccessContext,
) -> DispatchAction {
    let (id, revision, key) = match mutation_params(request.params) {
        Ok(value) => value,
        Err(()) => return invalid(request.id),
    };
    match application.refresh(access, id, revision, key).await {
        Ok(value) => success_action(request.id, write_result(value), None),
        Err(error) => user_package_error(request.id, error),
    }
}

async fn remove(
    request: rpc::RequestEnvelope,
    application: &UserPackageApplication,
    access: &AccessContext,
) -> DispatchAction {
    let (id, revision, key) = match mutation_params(request.params) {
        Ok(value) => value,
        Err(()) => return invalid(request.id),
    };
    match application.remove(access, id, revision, key).await {
        Ok(value) => success_action(
            request.id,
            rpc::UserPackageRemoveResult {
                user_package_id: value.user_package_id.to_string(),
                revision: value.revision.get(),
                removed: value.removed,
                replayed: value.replayed,
            },
            None,
        ),
        Err(error) => user_package_error(request.id, error),
    }
}

fn mutation_params(
    value: serde_json::Value,
) -> Result<(app::UserPackageId, Revision, IdempotencyKey), ()> {
    let params: rpc::UserPackageMutationParams = serde_json::from_value(value).map_err(|_| ())?;
    Ok((
        app::UserPackageId::parse(&params.user_package_id).map_err(|_| ())?,
        Revision::new(params.expected_revision).ok_or(())?,
        IdempotencyKey::parse(params.idempotency_key).map_err(|_| ())?,
    ))
}

fn cursor_from_rpc(value: rpc::UserPackageCursor) -> Result<app::UserPackageCursor, ()> {
    Ok(app::UserPackageCursor {
        updated_at_ms: value.updated_at_ms,
        user_package_id: app::UserPackageId::parse(&value.user_package_id).map_err(|_| ())?,
    })
}

fn cursor_to_rpc(value: app::UserPackageCursor) -> rpc::UserPackageCursor {
    rpc::UserPackageCursor {
        updated_at_ms: value.updated_at_ms,
        user_package_id: value.user_package_id.to_string(),
    }
}

fn record(value: app::UserPackageRecord) -> rpc::UserPackageRecord {
    rpc::UserPackageRecord {
        user_package_id: value.user_package_id.to_string(),
        source_root_path: value.source_root_path,
        package_id: value.package_id,
        version: value.version,
        display_name: value.display_name,
        revision: value.revision.get(),
        archive_sha256: hex(&value.archive_sha256),
        created_at_ms: value.created_at_ms,
        updated_at_ms: value.updated_at_ms,
    }
}

fn write_result(value: app::UserPackageWriteResult) -> rpc::UserPackageWriteResult {
    rpc::UserPackageWriteResult {
        user_package: record(value.user_package),
        replayed: value.replayed,
    }
}

fn user_package_error(id: String, error: app::UserPackageError) -> DispatchAction {
    let rpc_error = match error.code() {
        app::UserPackageErrorCode::InvalidInput => rpc::RpcError::invalid_request(),
        app::UserPackageErrorCode::PermissionDenied => rpc::RpcError::permission_denied(),
        app::UserPackageErrorCode::RevisionConflict => rpc::RpcError::revision_conflict(),
        app::UserPackageErrorCode::IdempotencyConflict => rpc::RpcError::idempotency_conflict(),
        app::UserPackageErrorCode::StoreUnavailable => rpc::RpcError::store_unavailable(),
        app::UserPackageErrorCode::Internal => {
            rpc::RpcError::internal(OperationId::new().to_string())
        }
        code => rpc::RpcError::m4_resource(code.as_str(), None),
    };
    error_action(Some(id), rpc_error, false)
}

fn hex(value: &[u8; 32]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}
