//! Named MCP adapters for the project registry, frozen plans, and local packages.
use std::path::Path;

use alcomd_client::{AlcomdClient, ClientError};
use alcomd_protocol as rpc;
use rmcp::{
    ErrorData,
    model::{CallToolRequestParams, CallToolResult},
};
use serde::Deserialize;
use serde_json::Value;

use super::environment::{confirmed, params, result, write_result};
use super::{Failure, failure, invalid, ipc_fields, read, safe_numbers, snake, success, write};

fn candidate_request(mut fields: Value) -> Result<rpc::ProjectCandidateRequest, ErrorData> {
    // CandidateSource uses snake_case on native IPC as well as on MCP.
    if let Some(source) = fields.pointer_mut("/view/source") {
        *source = snake(source.take());
    }
    if let Some(overrides) = fields
        .pointer_mut("/view/sources")
        .and_then(Value::as_array_mut)
    {
        for entry in overrides {
            if let Some(source) = entry.get_mut("source") {
                *source = snake(source.take());
            }
        }
    }
    params(fields)
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
        return write_result(Ok::<_, ClientError>(result));
    };
    if plan.target_sdk_version.is_none() {
        return write_result(Ok::<_, ClientError>(result));
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
        let mut reviewed = write_result(Ok::<_, ClientError>(result))?;
        reviewed["destination_path"] = Value::String(source.root_path);
        return Ok(reviewed);
    }
    let root = Path::new(&source.root_path);
    let (Some(parent), Some(name)) = (
        root.parent().and_then(Path::to_str),
        root.file_name().and_then(|name| name.to_str()),
    ) else {
        return Err(Failure::InvalidCore);
    };
    let copy = client
        .project_plan_copy(rpc::ProjectsPlanCopyParams {
            source_project_id: plan.project_id.clone(),
            expected_revision: input.expected_project_revision,
            target: rpc::ProjectTarget::Absolute {
                parent_path: parent.to_owned(),
                name: format!("{name}-Unity6"),
            },
            idempotency_key: plan.plan_id.clone(),
        })
        .await
        .map_err(write)?;
    let path =
        Path::new(&copy.plan.target_parent_canonical_path).join(&copy.plan.normalized_target_leaf);
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
    write_result(client.project_apply_unity_migration(input).await)
}

pub(super) async fn execute(
    client: &mut AlcomdClient,
    request: CallToolRequestParams,
) -> Result<CallToolResult, ErrorData> {
    if request.input_responses.is_some() || request.request_state.is_some() {
        return Err(invalid());
    }
    let fields = ipc_fields(Value::Object(request.arguments.unwrap_or_default()))?;
    if !safe_numbers(&fields) {
        return Err(invalid());
    }
    let (response, summary) = match request.name.as_ref() {
        "alcomd_register_project" => (
            write_result(client.project_register(params(fields)?).await),
            "Project registered.",
        ),
        "alcomd_create_project" => (
            write_result(client.projects_create(params(fields)?).await),
            "Project creation accepted. Check the operation.",
        ),
        "alcomd_plan_project_copy" => (
            write_result(client.project_plan_copy(params(fields)?).await),
            "Review the project copy plan before confirming.",
        ),
        "alcomd_inspect_project" => {
            let query: rpc::ProjectsInspectParams = params(fields)?;
            (
                result(
                    client
                        .project_inspect(query.path, query.discovery_mode)
                        .await,
                ),
                "Project inspection complete.",
            )
        }
        "alcomd_refresh_project" => {
            let query: rpc::ProjectMutationParams = confirmed(fields)?;
            (
                write_result(
                    client
                        .project_refresh(
                            query.project_id,
                            query.expected_revision,
                            query.idempotency_key,
                        )
                        .await,
                ),
                "Project registration refreshed.",
            )
        }
        "alcomd_set_project_favorite" => (
            write_result(client.project_set_favorite(confirmed(fields)?).await),
            "Project favorite saved.",
        ),
        "alcomd_unregister_project" => {
            let query: rpc::ProjectMutationParams = confirmed(fields)?;
            (
                write_result(
                    client
                        .project_unregister(
                            query.project_id,
                            query.expected_revision,
                            query.idempotency_key,
                        )
                        .await,
                ),
                "Project registration removed.",
            )
        }
        "alcomd_plan_project_delete_directory" => (
            write_result(client.project_plan_delete_directory(params(fields)?).await),
            "Review the project directory and writer evidence before confirming permanent deletion.",
        ),
        "alcomd_apply_project_delete_directory" => (
            write_result(
                client
                    .project_apply_delete_directory(confirmed(fields)?)
                    .await,
            ),
            "Deletion accepted. Check the operation for the outcome.",
        ),
        "alcomd_plan_project_unity_migration" => {
            let (options, fields) = migration_options(fields)?;
            (
                plan_migration(client, params(fields)?, options).await,
                "Review the destination, editor and SDK. Back up before an in-place upgrade. After uploading this content using Unity 6, future updates require Unity 6. For copy_required, confirm the copy and plan again with its completed operation ID.",
            )
        }
        "alcomd_apply_project_unity_migration" => {
            let (options, fields) = migration_options(fields)?;
            (
                apply_migration(client, confirmed(fields)?, options).await,
                "Unity migration accepted. Check the operation. When upgrading to Unity 6, complete the VRChat SDK wizard in the editor.",
            )
        }
        "alcomd_get_plan" => (
            result(client.get_plan(params(fields)?).await),
            "Original plan for the authenticated caller.",
        ),
        "alcomd_query_project_candidates" => (
            result(
                client
                    .package_query_project_candidates(candidate_request(fields)?)
                    .await,
            ),
            "Project package candidates and recommendations.",
        ),
        "alcomd_list_user_packages" => (
            result(client.user_packages_list(params(fields)?).await),
            "Registered local package sources.",
        ),
        "alcomd_get_user_package" => (
            result(client.user_package_get(params(fields)?).await),
            "Current local package source.",
        ),
        "alcomd_enroll_user_package" => (
            write_result(client.user_package_enroll(confirmed(fields)?).await),
            "Local package source registered.",
        ),
        "alcomd_refresh_user_package" => (
            write_result(client.user_package_refresh(confirmed(fields)?).await),
            "Local package source refreshed.",
        ),
        "alcomd_remove_user_package" => (
            write_result(client.user_package_remove(confirmed(fields)?).await),
            "Local package registration removed.",
        ),
        _ => return Err(invalid()),
    };
    let plan = matches!(
        request.name.as_ref(),
        "alcomd_get_plan"
            | "alcomd_plan_project_copy"
            | "alcomd_plan_project_delete_directory"
            | "alcomd_plan_project_unity_migration"
    );
    Ok(match response {
        Ok(value) => success(value, summary.into(), plan),
        Err(error) => failure(error),
    })
}
