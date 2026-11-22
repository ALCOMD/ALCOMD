//! Named MCP adapters for the project registry, frozen plans, and local packages.
use alcomd_client::AlcomdClient;
use alcomd_protocol as rpc;
use rmcp::{
    ErrorData,
    model::{CallToolRequestParams, CallToolResult},
};
use serde_json::Value;

use super::environment::{confirmed, params, result, write_result};
use super::{failure, invalid, ipc_fields, safe_numbers, snake, success};

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
        "alcomd_plan_project_unity_migration" => (
            write_result(client.project_plan_unity_migration(params(fields)?).await),
            "Review the project, target installation and migration steps.",
        ),
        "alcomd_apply_project_unity_migration" => (
            write_result(
                client
                    .project_apply_unity_migration(confirmed(fields)?)
                    .await,
            ),
            "Unity migration accepted. Check the operation for the outcome.",
        ),
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
