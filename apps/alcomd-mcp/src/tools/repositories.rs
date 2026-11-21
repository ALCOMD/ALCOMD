use alcomd_client::{AlcomdClient, ClientError};
use alcomd_protocol as rpc;
use rmcp::{
    ErrorData,
    model::{CallToolRequestParams, CallToolResult},
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

use super::{Failure, failure, invalid, ipc_fields, read, snake, success, write};

pub(super) fn recognized(name: &str) -> bool {
    rpc::mcp_repositories_tool_capability(name).is_some()
}

fn params<T: DeserializeOwned>(value: Value) -> Result<T, ErrorData> {
    serde_json::from_value(value).map_err(|_| invalid())
}

fn reply<T: Serialize>(value: Result<T, ClientError>, writing: bool) -> Result<Value, Failure> {
    let value = value.map_err(if writing { write } else { read })?;
    serde_json::to_value(value)
        .map(snake)
        .map_err(|_| Failure::InvalidCore)
}

async fn refresh(
    client: &mut AlcomdClient,
    input: rpc::RepositoryMutationParams,
) -> Result<Value, Failure> {
    let result = client
        .repository_refresh(
            input.repository_id,
            input.expected_revision,
            input.idempotency_key,
        )
        .await
        .map_err(write)?;
    let record = result.repository;
    let repository_id = record.repository_id.ok_or(Failure::InvalidCore)?;
    let revision = record.revision.ok_or(Failure::InvalidCore)?;
    let repository = rpc::McpRepositoryEntry {
        repository_id: repository_id.clone(),
        revision,
        display_name: record.display_name.or(record.name),
        source_kind: match record.source {
            rpc::RepositorySource::Local { .. } => rpc::McpRepositorySourceKind::Local,
            rpc::RepositorySource::Remote { .. } => rpc::McpRepositorySourceKind::Remote,
        },
        builtin: record.builtin.is_some(),
        // Successful refresh has committed a resolver index or validated the existing index.
        available: true,
    };
    reply(
        Ok(rpc::McpRepositoryWriteResult {
            repository: Some(repository),
            repository_id,
            revision,
            replayed: result.replayed,
        }),
        true,
    )
}

pub(super) async fn execute(
    client: &mut AlcomdClient,
    request: CallToolRequestParams,
) -> Result<CallToolResult, ErrorData> {
    if request.input_responses.is_some() || request.request_state.is_some() {
        return Err(invalid());
    }
    let value = ipc_fields(Value::Object(request.arguments.unwrap_or_default()))?;
    let (result, summary) = match request.name.as_ref() {
        "alcomd_list_repositories" => (
            reply(client.mcp_repositories_list(params(value)?).await, false),
            "Registered repositories.",
        ),
        "alcomd_inspect_repository" => (
            reply(client.mcp_repository_inspect(params(value)?).await, false),
            "Repository inspected. Review the source before confirming registration.",
        ),
        "alcomd_register_repository" => {
            let input: rpc::McpRepositoryRegisterParams = params(value)?;
            if !input.confirm {
                return Err(invalid());
            }
            (
                reply(client.mcp_repository_register(input).await, true),
                "Repository registered.",
            )
        }
        "alcomd_refresh_repository" => (
            refresh(client, params(value)?).await,
            "Repository refreshed.",
        ),
        "alcomd_remove_repository" => {
            let input: rpc::McpRepositoryRemoveParams = params(value)?;
            if !input.confirm {
                return Err(invalid());
            }
            (
                reply(client.mcp_repository_remove(input).await, true),
                "Repository removed.",
            )
        }
        _ => return Err(invalid()),
    };
    Ok(match result {
        Ok(data) => success(data, summary.into(), false),
        Err(error) => failure(error),
    })
}
