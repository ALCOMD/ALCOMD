use alcomd_client::{AlcomdClient, ClientError};
use alcomd_protocol as rpc;
use rmcp::{
    ErrorData,
    model::{CallToolRequestParams, CallToolResult},
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

use super::{Failure, failure, invalid, ipc_fields, read, snake, success};

pub(super) fn recognized(name: &str) -> bool {
    matches!(name, "alcomd_get_system_status" | "alcomd_get_settings")
        || rpc::mcp_environment_tool_capability(name).is_some()
}

pub(super) fn params<T: DeserializeOwned>(value: Value) -> Result<T, ErrorData> {
    serde_json::from_value(value).map_err(|_| invalid())
}

pub(super) fn result<T: Serialize>(response: Result<T, ClientError>) -> Result<Value, Failure> {
    serde_json::to_value(response.map_err(read)?)
        .map(snake)
        .map_err(|_| Failure::InvalidCore)
}

pub(super) fn write_result<T: Serialize>(
    response: Result<T, ClientError>,
) -> Result<Value, Failure> {
    serde_json::to_value(response.map_err(|error| Failure::Remote(error, true))?)
        .map(snake)
        .map_err(|_| Failure::InvalidCore)
}

pub(super) fn confirmed<T: DeserializeOwned>(mut fields: Value) -> Result<T, ErrorData> {
    if fields
        .as_object_mut()
        .and_then(|fields| fields.remove("confirm"))
        != Some(Value::Bool(true))
    {
        return Err(invalid());
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
    let (response, summary) = match request.name.as_ref() {
        "alcomd_get_mcp_runtime_snapshot" => (
            result(client.mcp_runtime_snapshot(params(fields)?).await),
            "Current MCP runtime activity.",
        ),
        "alcomd_get_mcp_transport_snapshot" => (
            result(client.mcp_transport_snapshot(params(fields)?).await),
            "Current MCP transport activity.",
        ),
        "alcomd_list_ai_clients" => (
            result(client.mcp_clients_list(params(fields)?).await),
            "List AI client identities.",
        ),
        "alcomd_request_client_change" => (
            write_result(client.mcp_client_request_create(confirmed(fields)?).await),
            "Request AI client administration.",
        ),
        "alcomd_get_client_change_request" => (
            result(client.mcp_client_request_get(params(fields)?).await),
            "Read a client management request.",
        ),
        "alcomd_list_client_change_requests" => (
            result(client.mcp_client_request_list(params(fields)?).await),
            "List client management requests.",
        ),
        "alcomd_list_oauth_registrations" => (
            result(client.oauth_registrations_list(params(fields)?).await),
            "List OAuth registrations.",
        ),
        "alcomd_get_oauth_request" => (
            result(client.oauth_request_get(params(fields)?).await),
            "Read an OAuth authorization request.",
        ),
        "alcomd_focus_project_unity" => (
            write_result(client.desktop_focus_project_unity(params(fields)?).await),
            "Focused the project's Unity window on the Core machine.",
        ),
        "alcomd_open_project_directory" => (
            write_result(client.desktop_open_project_directory(params(fields)?).await),
            "Requested the registered directory in the Core machine's file manager.",
        ),
        "alcomd_open_unity_hub_download" => (
            write_result(
                client
                    .desktop_open_unity_hub_download(params(fields)?)
                    .await,
            ),
            "Requested the Unity download page in the Core machine's browser.",
        ),
        "alcomd_list_clients" => (
            result(client.clients_list(params(fields)?).await),
            "Installed and active client programs.",
        ),
        "alcomd_list_client_instances" => (
            result(client.client_instances_list(params(fields)?).await),
            "Client instances and connections.",
        ),
        "alcomd_disconnect_client" => (
            write_result(client.clients_disconnect(confirmed(fields)?).await),
            "Selected connections disconnected.",
        ),
        "alcomd_list_installed_packages" => (
            result(client.project_inventory(params(fields)?).await),
            "Locally observed packages and resolution evidence.",
        ),
        "alcomd_get_project_template" => (
            result(client.template_get(params(fields)?).await),
            "Current template and attachments.",
        ),
        "alcomd_edit_project_template" => {
            let params: rpc::ProjectTemplateEditParams = params(fields)?;
            if !params.confirm {
                return Err(invalid());
            }
            (
                write_result(client.template_edit(params).await),
                "Template edit accepted. Check its Operation for the result.",
            )
        }
        "alcomd_create_project_template" => (
            write_result(client.project_templates_save(confirmed(fields)?).await),
            "Template save accepted. Check its Operation for the result.",
        ),
        "alcomd_export_project_template" => (
            write_result(client.project_templates_export(confirmed(fields)?).await),
            "Template export accepted. Check its Operation for the result.",
        ),
        "alcomd_remove_project_template" => (
            write_result(client.project_templates_remove(confirmed(fields)?).await),
            "Template removed.",
        ),
        "alcomd_get_system_status" | "alcomd_get_settings" => {
            if fields.as_object().is_none_or(|fields| !fields.is_empty()) {
                return Err(invalid());
            }
            if request.name == "alcomd_get_system_status" {
                (
                    result(client.system_status().await),
                    "Running Core status and capabilities negotiated for this connection.",
                )
            } else {
                (
                    result(client.settings_get().await),
                    "Current ALCOMD settings and effective storage directories.",
                )
            }
        }
        "alcomd_list_unity_installations" => (
            result(client.unity_installations_list(params(fields)?).await),
            "Registered Unity installations.",
        ),
        "alcomd_get_project_launch_config" => {
            let params: rpc::UnityProjectIdParams = params(fields)?;
            (
                result(
                    client
                        .unity_project_launch_config_get(params.project_id)
                        .await,
                ),
                "Project overrides, defaults, and effective Unity launch arguments.",
            )
        }
        _ => return Err(invalid()),
    };
    Ok(match response {
        Ok(value) => success(value, summary.into(), false),
        Err(error) => failure(error),
    })
}
