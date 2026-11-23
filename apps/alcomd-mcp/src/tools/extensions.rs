use super::environment::{confirmed, params, result, write_result};
use super::{failure_with_profile, invalid, ipc_fields, success};
use alcomd_client::AlcomdClient;
use alcomd_protocol as rpc;
use rmcp::{
    ErrorData,
    model::{CallToolRequestParams, CallToolResult},
};
use serde_json::Value;

pub(super) async fn execute(
    client: &mut AlcomdClient,
    request: CallToolRequestParams,
) -> Result<CallToolResult, ErrorData> {
    if request.input_responses.is_some() || request.request_state.is_some() {
        return Err(invalid());
    }
    let fields = ipc_fields(Value::Object(request.arguments.unwrap_or_default()))?;
    let (response, summary, plan) = match request.name.as_ref() {
        "alcomd_list_extensions" => (
            result(client.extensions_list(params(fields)?).await),
            "List installed extensions.",
            false,
        ),
        "alcomd_get_extension" => (
            result(
                client
                    .extension_get(params::<rpc::ExtensionIdParams>(fields)?.extension_id)
                    .await,
            ),
            "Get extension details.",
            false,
        ),
        "alcomd_list_extension_permissions" => (
            result(client.extension_permissions_list(params(fields)?).await),
            "List extension permissions.",
            false,
        ),
        "alcomd_inspect_extension_source" => (
            result(client.extension_inspect_source(params(fields)?).await),
            "Inspect an extension source.",
            false,
        ),
        "alcomd_plan_extension_install" => (
            result(client.extension_plan_install(params(fields)?).await),
            "Plan extension installation.",
            true,
        ),
        "alcomd_plan_extension_install_from_source" => (
            result(
                client
                    .extension_plan_install_from_source(params(fields)?)
                    .await,
            ),
            "Plan extension installation from a source.",
            true,
        ),
        "alcomd_apply_extension_install" => (
            write_result(client.extension_apply_install(confirmed(fields)?).await),
            "Apply extension installation.",
            false,
        ),
        "alcomd_enable_extension" => (
            write_result(
                client
                    .extension_enable_with_consent(confirmed(fields)?)
                    .await,
            ),
            "Enable an extension.",
            false,
        ),
        "alcomd_disable_extension" => (
            write_result(client.extension_disable(confirmed(fields)?).await),
            "Disable an extension.",
            false,
        ),
        "alcomd_plan_extension_uninstall" => (
            result(client.extension_plan_uninstall(params(fields)?).await),
            "Plan extension removal.",
            true,
        ),
        "alcomd_apply_extension_uninstall" => (
            write_result(client.extension_apply_uninstall(confirmed(fields)?).await),
            "Apply extension removal.",
            false,
        ),
        "alcomd_grant_extension_permission" => (
            write_result(client.extension_set_grant(confirmed(fields)?).await),
            "Grant an extension permission.",
            false,
        ),
        "alcomd_revoke_extension_permission" => (
            write_result(client.extension_revoke_grant(confirmed(fields)?).await),
            "Revoke an extension permission.",
            false,
        ),
        _ => return Err(invalid()),
    };
    Ok(match response {
        Ok(value) => success(value, summary.into(), plan),
        Err(error) => failure_with_profile(error, Some("extension")),
    })
}
