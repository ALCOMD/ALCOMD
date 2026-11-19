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
    matches!(
        name,
        "alcomd_get_settings"
            | "alcomd_list_unity_installations"
            | "alcomd_get_project_launch_config"
            | "alcomd_list_installed_packages"
            | "alcomd_get_project_template"
            | "alcomd_edit_project_template"
    )
}

fn params<T: DeserializeOwned>(value: Value) -> Result<T, ErrorData> {
    serde_json::from_value(value).map_err(|_| invalid())
}

fn result<T: Serialize>(response: Result<T, ClientError>) -> Result<Value, Failure> {
    serde_json::to_value(response.map_err(read)?)
        .map(snake)
        .map_err(|_| Failure::InvalidCore)
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
            let response = client
                .template_edit(params)
                .await
                .map_err(|error| Failure::Remote(error, true))
                .and_then(|accepted| {
                    serde_json::to_value(accepted)
                        .map(snake)
                        .map_err(|_| Failure::InvalidCore)
                });
            (
                response,
                "Template edit accepted. Check its Operation for the result.",
            )
        }
        "alcomd_get_settings" => {
            if fields.as_object().is_none_or(|fields| !fields.is_empty()) {
                return Err(invalid());
            }
            (
                result(client.settings_get().await),
                "Current ALCOMD settings and effective storage directories.",
            )
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
