use alcomd_client::{AlcomdClient, ClientError};
use alcomd_protocol as rpc;
use rmcp::{
    ErrorData,
    model::{CallToolRequestParams, CallToolResult},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

use super::{Failure, failure, invalid, ipc_fields, read, snake, success, write};

pub(super) fn recognized(name: &str) -> bool {
    rpc::mcp_workflows_tool_capability(name).is_some()
}

fn params<T: DeserializeOwned>(value: Value) -> Result<T, ErrorData> {
    serde_json::from_value(value).map_err(|_| invalid())
}

fn confirmed<T: DeserializeOwned>(mut value: Value) -> Result<T, ErrorData> {
    if value
        .as_object_mut()
        .and_then(|fields| fields.remove("confirm"))
        != Some(Value::Bool(true))
    {
        return Err(invalid());
    }
    params(value)
}

fn reply<T: Serialize>(value: Result<T, ClientError>, writing: bool) -> Result<Value, Failure> {
    let value = value.map_err(if writing { write } else { read })?;
    serde_json::to_value(value)
        .map(snake)
        .map_err(|_| Failure::InvalidCore)
}

pub(super) fn operation_kind(kind: &str) -> bool {
    matches!(
        kind,
        "projects.create"
            | "projects.copy"
            | "projects.unity-migration"
            | "backups.create"
            | "backups.restore"
            | "projectTemplates.save"
            | "projectTemplates.export"
    )
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreatedProject {
    project_id: String,
    revision: u64,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CopiedProject {
    target_project_id: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SavedTemplate {
    template: rpc::ProjectTemplateRecord,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreatedBackup {
    archive_name: String,
    archive_bytes: u64,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RestoredProject {
    project_id: String,
    target_name: String,
}

fn project_receipt<T: Serialize + DeserializeOwned>(result: Value) -> Result<Value, Failure> {
    let safe: T = serde_json::from_value(result).map_err(|_| Failure::InvalidCore)?;
    serde_json::to_value(safe).map_err(|_| Failure::InvalidCore)
}

pub(super) fn operation_result(kind: &str, result: Value) -> Result<Value, Failure> {
    match kind {
        "projects.create" => project_receipt::<CreatedProject>(result),
        "projects.copy" => project_receipt::<CopiedProject>(result),
        "projectTemplates.save" => project_receipt::<SavedTemplate>(result),
        "projectTemplates.export" => project_receipt::<rpc::ProjectTemplateRef>(result),
        "backups.restore" => project_receipt::<RestoredProject>(result),
        "backups.create" => project_receipt::<CreatedBackup>(result),
        _ => Err(Failure::InvalidCore),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MigrationOptions {
    #[serde(default)]
    in_place: bool,
    copy_operation_id: Option<String>,
}

fn migration_options(mut value: Value) -> Result<(MigrationOptions, Value), ErrorData> {
    let fields = value.as_object_mut().ok_or_else(invalid)?;
    let mut options = serde_json::Map::new();
    for key in ["inPlace", "copyOperationId"] {
        if let Some(value) = fields.remove(key) {
            options.insert(key.into(), value);
        }
    }
    let options: MigrationOptions = params(Value::Object(options))?;
    if options.in_place && options.copy_operation_id.is_some() {
        return Err(invalid());
    }
    Ok((options, value))
}

async fn check_copy(client: &mut AlcomdClient, id: String, target: &str) -> Result<(), Failure> {
    let operation = client.operation_get(id).await.map_err(read)?;
    if operation.kind != "projects.copy"
        || operation.state != rpc::OperationState::Succeeded
        || operation
            .result
            .as_ref()
            .and_then(|r| r.get("targetProjectId"))
            .and_then(Value::as_str)
            != Some(target)
    {
        return Err(read(ClientError::Remote(rpc::RpcError::invalid_request())));
    }
    Ok(())
}

async fn plan_migration(
    client: &mut AlcomdClient,
    input: rpc::ProjectsPlanUnityMigrationParams,
    options: MigrationOptions,
) -> Result<Value, Failure> {
    let result = client
        .project_plan_unity_migration(input.clone())
        .await
        .map_err(write)?;
    let rpc::ProjectsPlanUnityMigrationResult::Planned { plan, .. } = &result else {
        return reply(Ok(result), true);
    };
    if plan.target_sdk_version.is_none() {
        return reply(Ok(result), true);
    }
    if let Some(id) = &options.copy_operation_id {
        check_copy(client, id.clone(), &plan.project_id).await?;
    }
    let source = client
        .project_get(plan.project_id.clone())
        .await
        .map_err(read)?
        .project;
    if options.in_place || options.copy_operation_id.is_some() {
        let mut reviewed = reply(Ok(result), true)?;
        reviewed["destination_path"] = Value::String(source.root_path);
        return Ok(reviewed);
    }
    let path = format!("{}-Unity6", source.root_path);
    let copy = client
        .mcp_project_plan_copy(rpc::McpProjectsPlanCopyParams {
            source_project_id: plan.project_id.clone(),
            expected_revision: input.expected_project_revision,
            target: rpc::McpProjectTarget::Absolute { path: path.clone() },
            idempotency_key: plan.plan_id.clone(),
        })
        .await
        .map_err(write)?;
    Ok(
        serde_json::json!({"kind":"copy_required", "copy_plan": snake(serde_json::to_value(copy).map_err(|_| Failure::InvalidCore)?),
        "source_path":source.root_path,"destination_path":path,"target_installation_id":plan.target_installation_id,
        "target_unity_version":plan.target_unity_version,"target_sdk_version":plan.target_sdk_version}),
    )
}

async fn apply_migration(
    client: &mut AlcomdClient,
    input: rpc::ProjectsApplyUnityMigrationParams,
    options: MigrationOptions,
) -> Result<Value, Failure> {
    let rpc::PlansGetResult::ProjectUnityMigration { plan } = client
        .get_plan(rpc::PlansGetParams {
            kind: rpc::PlanKind::ProjectUnityMigration,
            plan_id: input.plan_id.clone(),
        })
        .await
        .map_err(read)?
    else {
        return Err(Failure::InvalidCore);
    };
    if plan.target_sdk_version.is_some() && !options.in_place {
        let id = options
            .copy_operation_id
            .ok_or_else(|| read(ClientError::Remote(rpc::RpcError::invalid_request())))?;
        check_copy(client, id, &plan.project_id).await?;
    }
    reply(client.project_apply_unity_migration(input).await, true)
}

pub(super) async fn execute(
    client: &mut AlcomdClient,
    request: CallToolRequestParams,
) -> Result<CallToolResult, ErrorData> {
    if request.input_responses.is_some() || request.request_state.is_some() {
        return Err(invalid());
    }
    let value = ipc_fields(Value::Object(request.arguments.unwrap_or_default()))?;
    let (result, summary, plan) = match request.name.as_ref() {
        "alcomd_register_project" => (
            reply(client.mcp_project_register(params(value)?).await, true),
            "Project registered.",
            false,
        ),
        "alcomd_list_project_templates" => (
            reply(client.project_templates_list(params(value)?).await, false),
            "Project templates available for creation.",
            false,
        ),
        "alcomd_create_project" => (
            reply(client.mcp_project_create(params(value)?).await, true),
            "Project creation accepted. Check the operation.",
            false,
        ),
        "alcomd_plan_project_copy" => (
            reply(client.mcp_project_plan_copy(params(value)?).await, true),
            "Review the project copy plan before confirming.",
            true,
        ),
        "alcomd_apply_project_copy" => {
            let input = confirmed(value)?;
            (
                reply(client.project_apply_copy(input).await, true),
                "Project copy accepted. Check the operation.",
                false,
            )
        }
        "alcomd_plan_unity_migration" => {
            let (options, fields) = migration_options(value)?;
            (
                plan_migration(client, params(fields)?, options).await,
                "Review the destination, editor and SDK. Back up before an in-place upgrade. After uploading this content using Unity 6, future updates require Unity 6. For copy_required, confirm the copy and plan again with its completed operation ID.",
                true,
            )
        }
        "alcomd_apply_unity_migration" => {
            let (options, fields) = migration_options(value)?;
            (
                apply_migration(client, confirmed(fields)?, options).await,
                "Unity migration accepted. Check the operation. When upgrading to Unity 6, complete the VRChat SDK wizard in the editor.",
                false,
            )
        }
        "alcomd_create_backup" => (
            reply(client.backup_create(params(value)?).await, true),
            "Backup creation accepted. Check the operation.",
            false,
        ),
        "alcomd_restore_backup" => {
            let input = confirmed(value)?;
            (
                reply(client.backup_restore(input).await, true),
                "Backup restore accepted. Check the operation.",
                false,
            )
        }
        _ => return Err(invalid()),
    };
    Ok(match result {
        Ok(value) => success(value, summary.into(), plan),
        Err(error) => failure(error),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn apply_requires_confirmation_before_building_the_native_request() {
        for confirm in [Value::Null, json!(false), json!("true"), json!(true)] {
            let fields = json!({"planId": "plan", "expectedRevision": 1, "idempotencyKey": "key", "confirm": confirm});
            assert_eq!(
                confirmed::<rpc::ProjectsApplyCopyParams>(fields).is_ok(),
                confirm == json!(true)
            );
        }
        assert!(
            confirmed::<rpc::BackupRestoreParams>(
                json!({"archivePath":"archive.zip","idempotencyKey":"key"})
            )
            .is_err()
        );
    }

    #[test]
    fn restore_result_does_not_accept_the_native_absolute_target() {
        let mut result = json!({
            "projectId": "00000000-0000-4000-8000-000000000002",
            "targetName": "Restored project"
        });
        assert!(matches!(
            operation_result("backups.restore", result.clone()),
            Ok(projected) if projected == result
        ));
        result["target"] = json!({"parent": "/private/path", "leaf": "Restored project"});
        assert!(operation_result("backups.restore", result).is_err());
    }
}
