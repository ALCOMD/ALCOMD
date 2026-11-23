use crate::{
    Applications, ConnectionState, DispatchAction, current_time_ms, error_action, invalid,
    m7_official_rpc, repository_list_rpc, require_capability, success_action,
};
use alcomd_application::{
    self as app, ExternalAdmission, ExternalError, ExternalMethod, ExternalReceipt,
    ExternalWriteStore, Permission,
};
use alcomd_protocol as rpc;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

fn normalized<T: Serialize + DeserializeOwned>(params: &Value) -> Option<Value> {
    serde_json::to_value(serde_json::from_value::<T>(params.clone()).ok()?).ok()
}

fn failure(id: String, e: ExternalError) -> DispatchAction {
    let error = match e {
        ExternalError::Denied => rpc::RpcError::permission_denied(),
        ExternalError::Conflict => rpc::RpcError::idempotency_conflict(),
        ExternalError::Unavailable => rpc::RpcError::store_unavailable(),
    };
    error_action(Some(id), error, false)
}
fn replay(id: String, r: ExternalReceipt) -> DispatchAction {
    match r {
        ExternalReceipt::Result(v) => success_action(id, v, None),
        ExternalReceipt::Error(e) => error_action(
            Some(id),
            serde_json::from_value(e).unwrap_or_else(|_| rpc::RpcError::store_unavailable()),
            false,
        ),
        ExternalReceipt::OutcomeUnknown => error_action(
            Some(id),
            rpc::RpcError::external_write_outcome_unknown(),
            false,
        ),
    }
}
pub(super) async fn dispatch(
    req: rpc::RequestEnvelope,
    state: &ConnectionState,
    apps: &Applications,
    access: &app::AccessContext,
    method: ExternalMethod,
) -> DispatchAction {
    let (args, capability, mut permissions) = match method {
        ExternalMethod::SettingsUpdate => (
            normalized::<rpc::SettingsUpdateParams>(&req.params),
            None,
            vec![Permission::SettingsManage],
        ),
        ExternalMethod::OpenHub => (
            normalized::<rpc::UnityHubOpenParams>(&req.params),
            Some(rpc::CAPABILITY_UNITY_HUB_OPEN_V1),
            vec![Permission::SettingsRead, Permission::UnityHubOpen],
        ),
        ExternalMethod::ExportRepositories => (
            normalized::<rpc::RepositoryListExportParams>(&req.params),
            Some(rpc::CAPABILITY_REPOSITORIES_LIST_FILES_V1),
            vec![Permission::RepositoriesRead],
        ),
    };
    if let Some(cap) = capability
        && let Some(e) = require_capability(&req.id, state, cap)
    {
        return e;
    }
    let Some(mut args) = args else {
        return invalid(req.id);
    };
    if method == ExternalMethod::SettingsUpdate {
        if args
            .pointer("/update/unity/defaultLaunchArguments")
            .is_some()
        {
            permissions.push(Permission::UnityManage);
        }
        if args.pointer("/update/projects").is_some() {
            permissions.push(Permission::ProjectsManage);
        }
    }
    if (access.principal().as_str() != app::PrincipalId::LOCAL_OWNER && !access.is_mcp_client())
        || permissions.iter().any(|p| access.require(*p).is_err())
    {
        return failure(req.id, ExternalError::Denied);
    }
    let Some(key) = args
        .as_object_mut()
        .and_then(|o| o.remove("idempotencyKey"))
        .and_then(|k| k.as_str().and_then(|s| app::IdempotencyKey::parse(s).ok()))
    else {
        return invalid(req.id);
    };
    let Ok(fingerprint) = app::tool_request_fingerprint(method.name(), &args) else {
        return invalid(req.id);
    };
    let id = req.id.clone();
    let input = app::ExternalRequest {
        method,
        caller: access.principal().clone(),
        key,
        fingerprint,
        permissions,
    };
    let check = || apps.store.external_admit(input.clone(), false, 0);
    match check().await {
        Ok(ExternalAdmission::Ready) => {}
        Ok(ExternalAdmission::Replayed(r)) => return replay(id, r),
        Ok(ExternalAdmission::Accepted(_)) => return failure(id, ExternalError::Unavailable),
        Err(e) => return failure(id, e),
    }
    let acceptance = app::ExternalAcceptance::new(apps.store.clone(), input.clone());
    let action = acceptance
        .scope(async {
            match method {
                ExternalMethod::SettingsUpdate | ExternalMethod::OpenHub => {
                    m7_official_rpc::dispatch(req, state, &apps.official_gui, access).await
                }
                ExternalMethod::ExportRepositories => {
                    repository_list_rpc::dispatch(
                        req,
                        state,
                        &apps.repository_lists,
                        &apps.m3,
                        access,
                    )
                    .await
                }
            }
        })
        .await;
    if let Some(observed) = acceptance.observed() {
        return match observed {
            Ok(r) => replay(id, r),
            Err(e) => failure(id, e),
        };
    }
    if let Some(&operation) = acceptance.accepted.get() {
        let receipt = if let Some(result) = action.response.get("result") {
            ExternalReceipt::Result(result.clone())
        } else if let Some(e) = action.response.get("error") {
            if matches!(
                method,
                ExternalMethod::SettingsUpdate | ExternalMethod::OpenHub
            ) && e.get("code").and_then(Value::as_str) == Some(rpc::error_code::INTERNAL_ERROR)
            {
                ExternalReceipt::OutcomeUnknown
            } else {
                ExternalReceipt::Error(e.clone())
            }
        } else {
            ExternalReceipt::OutcomeUnknown
        };
        let Ok(now) = current_time_ms() else {
            return replay(id, ExternalReceipt::OutcomeUnknown);
        };
        if apps
            .store
            .external_finish(input, operation, receipt.clone(), now)
            .await
            .is_err()
        {
            return replay(id, ExternalReceipt::OutcomeUnknown);
        }
        return replay(id, receipt);
    }
    // A concurrent invocation can finish while this request waits for the settings lock.
    match check().await {
        Ok(ExternalAdmission::Replayed(r)) => replay(id, r),
        Err(e) => failure(id, e),
        _ => action,
    }
}
