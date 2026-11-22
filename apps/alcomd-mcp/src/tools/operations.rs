use super::{Failure, failure, invalid, read, success, write};
use alcomd_client::AlcomdClient;
use alcomd_protocol as rpc;
use rmcp::{
    ErrorData,
    model::{CallToolRequestParams, CallToolResult},
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::sync::LazyLock;

pub(super) const CONTRACT: &str = include_str!("../../../../specs/mcp/operations-v1.schema.json");
static TOOLS: LazyLock<Value> =
    LazyLock::new(|| serde_json::from_str(CONTRACT).expect("checked business tool contract"));

pub(super) fn recognized(name: &str) -> bool {
    name == "alcomd_update_settings" || rpc::mcp_operation_tool_capability(name).is_some()
}

fn parse<T: DeserializeOwned>(fields: Value) -> Result<T, ErrorData> {
    serde_json::from_value(fields).map_err(|_| invalid())
}

fn data<T: Serialize>(
    value: Result<T, alcomd_client::ClientError>,
    writing: bool,
) -> Result<Value, Failure> {
    serde_json::to_value(value.map_err(if writing { write } else { read })?)
        .map_err(|_| Failure::InvalidCore)
}

pub(super) async fn execute(
    client: &mut AlcomdClient,
    request: CallToolRequestParams,
    events_config: Option<&alcomd_client::ClientConfig>,
) -> Result<CallToolResult, ErrorData> {
    if request.input_responses.is_some() || request.request_state.is_some() {
        return Err(invalid());
    }
    let entry = TOOLS["x-alcomd-tools"]
        .as_array()
        .and_then(|tools| tools.iter().find(|t| t["name"] == request.name.as_ref()))
        .ok_or_else(invalid)?;
    let writing = entry["effect"] == "apply";
    let mut arguments = request.arguments.unwrap_or_default();
    if writing && arguments.remove("confirm") != Some(Value::Bool(true)) {
        return Err(invalid());
    }
    let fields =
        crate::tools::tool_fields::to_core(Value::Object(arguments), &entry["inputSchema"])?;
    let result = match request.name.as_ref() {
        "alcomd_subscribe_events" => {
            let params = parse(fields)?;
            let config = events_config
                .ok_or_else(super::super::unsupported_method)?
                .clone();
            match alcomd_client::EventSubscription::connect(config, params).await {
                Err(error) => Err(read(error)),
                Ok(mut stream) => {
                    let mut observation = serde_json::json!({"ready":stream.ready()});
                    if !stream.ready().resync_required {
                        match tokio::time::timeout(
                            std::time::Duration::from_secs(10),
                            stream.next_notification(),
                        )
                        .await
                        {
                            Ok(Ok(notification)) => {
                                observation["notification"] =
                                    serde_json::to_value(notification).map_err(|_| invalid())?
                            }
                            Ok(Err(error)) => return Ok(failure(read(error))),
                            Err(_) => {}
                        }
                    }
                    Ok(observation)
                }
            }
        }
        "alcomd_list_events" => {
            let p: rpc::EventsListParams = parse(fields)?;
            data(client.events_list(p.after_sequence, p.limit).await, writing)
        }
        "alcomd_read_logs" => data(client.logs_read(parse(fields)?).await, writing),
        "alcomd_check_state" => {
            let p: rpc::StateCheckParams = parse(fields)?;
            data(client.state_check(p.idempotency_key).await, writing)
        }
        "alcomd_update_settings" => data(client.settings_update(parse(fields)?).await, writing),
        "alcomd_list_repositories" => {
            let p: rpc::RegistryListParams = parse(fields)?;
            data(client.repositories_list(p.cursor, p.limit).await, writing)
        }
        "alcomd_inspect_repository" => {
            let p: rpc::RepositoryInspectParams = parse(fields)?;
            data(
                client
                    .repository_inspect_with_credentials(p.source, p.credentials)
                    .await,
                writing,
            )
        }
        "alcomd_register_repository" => {
            let p: rpc::RepositoryRegisterParams = parse(fields)?;
            data(
                client
                    .repository_register(p.preview_id, p.idempotency_key)
                    .await,
                writing,
            )
        }
        "alcomd_refresh_repository" => {
            let p: rpc::RepositoryMutationParams = parse(fields)?;
            data(
                client
                    .repository_refresh(p.repository_id, p.expected_revision, p.idempotency_key)
                    .await,
                writing,
            )
        }
        "alcomd_remove_repository" => {
            let p: rpc::RepositoryMutationParams = parse(fields)?;
            data(
                client
                    .repository_unregister(p.repository_id, p.expected_revision, p.idempotency_key)
                    .await,
                writing,
            )
        }
        "alcomd_get_repository" => {
            let p: rpc::RepositoryIdParams = parse(fields)?;
            data(client.repository_get(p.repository_id).await, writing)
        }
        "alcomd_list_repository_packages" => {
            let p: rpc::RepositoryPackagesParams = parse(fields)?;
            data(
                client
                    .repository_packages(p.repository_id, p.cursor, p.limit)
                    .await,
                writing,
            )
        }
        "alcomd_move_repository" => data(client.repository_move(parse(fields)?).await, writing),
        "alcomd_set_repository_display_name" => data(
            client.repository_set_display_name(parse(fields)?).await,
            writing,
        ),
        "alcomd_refresh_all_repositories" => data(
            client.repositories_refresh_all(parse(fields)?).await,
            writing,
        ),
        "alcomd_set_repository_credentials" => data(
            client.repository_set_credentials(parse(fields)?).await,
            writing,
        ),
        "alcomd_inspect_repository_file" => {
            let p: rpc::RepositoryListInspectParams = parse(fields)?;
            data(client.repository_list_inspect_file(p.path).await, writing)
        }
        "alcomd_import_repository_file" => data(
            client.repository_list_import_file(parse(fields)?).await,
            writing,
        ),
        "alcomd_cancel_repository_import" => data(
            client
                .repository_list_cancel_registration(parse(fields)?)
                .await,
            writing,
        ),
        "alcomd_preview_repository_export" => {
            let p: rpc::RepositoryListPreviewParams = parse(fields)?;
            data(
                client
                    .repository_configuration_preview_export(p.format)
                    .await,
                writing,
            )
        }
        "alcomd_export_repository_file" => {
            let p: rpc::RepositoryListExportParams = parse(fields)?;
            data(
                client
                    .repository_configuration_export_file(
                        p.target_path,
                        p.expected_fingerprint,
                        p.format,
                        p.idempotency_key,
                    )
                    .await,
                writing,
            )
        }
        "alcomd_get_unity_installation" => {
            let p: rpc::UnityInstallationIdParams = parse(fields)?;
            data(
                client.unity_installation_get(p.installation_id).await,
                writing,
            )
        }
        "alcomd_refresh_unity_installations" => {
            let p: rpc::UnityInstallationRefreshParams = parse(fields)?;
            data(
                client.unity_installations_refresh(p.idempotency_key).await,
                writing,
            )
        }
        "alcomd_register_unity_installation" => data(
            client.unity_installation_register(parse(fields)?).await,
            writing,
        ),
        "alcomd_remove_unity_installation" => data(
            client.unity_installation_remove(parse(fields)?).await,
            writing,
        ),
        "alcomd_get_unity_launch_options" => {
            data(client.unity_launch_options(parse(fields)?).await, writing)
        }
        "alcomd_get_unity_writer_state" => {
            let p: rpc::UnityProjectIdParams = parse(fields)?;
            data(client.unity_writer_state(p.project_id).await, writing)
        }
        "alcomd_launch_unity" => data(client.unity_launch(parse(fields)?).await, writing),
        "alcomd_open_unity_hub" => data(client.unity_hub_open(parse(fields)?).await, writing),
        "alcomd_set_project_launch_config" => data(
            client.unity_project_launch_config_set(parse(fields)?).await,
            writing,
        ),
        "alcomd_clear_project_launch_config" => data(
            client
                .unity_project_launch_config_clear(parse(fields)?)
                .await,
            writing,
        ),
        _ => return Err(invalid()),
    };
    Ok(match result {
        Ok(value) => {
            let output = super::tool_fields::from_core(value, &entry["outputSchema"])?;
            success(
                output,
                entry["title"].as_str().unwrap_or_default().into(),
                false,
            )
        }
        Err(error) => failure(error),
    })
}
