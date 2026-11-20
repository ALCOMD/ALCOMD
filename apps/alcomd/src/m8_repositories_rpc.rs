use alcomd_application as app;
use alcomd_protocol as rpc;

use super::{Applications, ConnectionState, DispatchAction, error_action, invalid, success_action};

const MAX_SAFE: u64 = 9_007_199_254_740_991;

fn entry(value: app::McpRepositoryEntry) -> rpc::McpRepositoryEntry {
    rpc::McpRepositoryEntry {
        repository_id: value.repository_id.to_string(),
        display_name: value.display_name,
        revision: value.revision.get(),
        source_kind: match value.source_kind {
            app::McpRepositorySourceKind::Local => rpc::McpRepositorySourceKind::Local,
            app::McpRepositorySourceKind::Remote => rpc::McpRepositorySourceKind::Remote,
        },
        builtin: value.builtin,
        available: value.available,
    }
}

fn target(id: &str, revision: u64) -> Option<(app::RepositoryId, app::Revision)> {
    let parsed = app::RepositoryId::parse(id).ok()?;
    let revision = app::Revision::new(revision).filter(|value| value.get() <= MAX_SAFE)?;
    Some((parsed, revision))
}

pub(super) async fn dispatch(
    request: rpc::RequestEnvelope,
    state: &ConnectionState,
    apps: &Applications,
    access: &app::AccessContext,
) -> DispatchAction {
    if let Some(action) =
        super::require_capability(&request.id, state, rpc::CAPABILITY_MCP_REPOSITORIES_V1)
    {
        return action;
    }
    let Ok(authentication) = super::m8_tools_rpc::auth(state, access) else {
        return error_action(Some(request.id), rpc::RpcError::permission_denied(), false);
    };
    let registry = &apps.m3;
    let id = request.id;
    let params = request.params;
    let (command, key) = match request.method.as_str() {
        rpc::METHOD_MCP_REPOSITORIES_LIST => {
            let Ok(params) = serde_json::from_value::<rpc::McpRepositoriesListParams>(params)
            else {
                return invalid(id);
            };
            if params
                .cursor
                .as_ref()
                .is_some_and(|cursor| cursor.registered_at_ms > MAX_SAFE)
            {
                return invalid(id);
            }
            let Ok(cursor) = params
                .cursor
                .map(super::m3_rpc::repository_cursor)
                .transpose()
            else {
                return invalid(id);
            };
            return match registry
                .mcp_repositories_list(access, cursor, params.limit.unwrap_or(100))
                .await
            {
                Ok(page) => success_action(
                    id,
                    rpc::McpRepositoriesListResult {
                        repositories: page
                            .repositories
                            .into_iter()
                            .map(|record| entry(record.into()))
                            .collect(),
                        next_cursor: page
                            .next_cursor
                            .map(super::m3_rpc::repository_cursor_to_rpc),
                    },
                    None,
                ),
                Err(error) => super::m3_rpc::m3_error(id, error),
            };
        }
        rpc::METHOD_MCP_REPOSITORIES_INSPECT => {
            let Ok(params) = serde_json::from_value::<rpc::McpRepositoryInspectParams>(params)
            else {
                return invalid(id);
            };
            if params.url.is_empty() || params.url.len() > 8192 {
                return invalid(id);
            }
            return match registry
                .mcp_repository_inspect(access, &authentication, params.url)
                .await
            {
                Ok((preview_id, display_name, version_count)) => success_action(
                    id,
                    rpc::McpRepositoryInspectResult {
                        preview_id: preview_id.map(|value| value.to_string()),
                        display_name,
                        version_count,
                    },
                    None,
                ),
                Err(error) => super::m3_rpc::m3_error(id, error),
            };
        }
        rpc::METHOD_MCP_REPOSITORIES_REGISTER => {
            let Ok(params) = serde_json::from_value::<rpc::McpRepositoryRegisterParams>(params)
            else {
                return invalid(id);
            };
            let Ok(preview_id) = app::OperationId::parse(&params.preview_id) else {
                return invalid(id);
            };
            if !params.confirm {
                return invalid(id);
            }
            (
                app::McpRepositoryCommand::Register { preview_id },
                params.idempotency_key,
            )
        }
        rpc::METHOD_MCP_REPOSITORIES_REMOVE => {
            let Ok(params) = serde_json::from_value::<rpc::McpRepositoryRemoveParams>(params)
            else {
                return invalid(id);
            };
            let Some((id_target, expected)) =
                target(&params.repository_id, params.expected_revision)
            else {
                return invalid(id);
            };
            if !params.confirm {
                return invalid(id);
            }
            (
                app::McpRepositoryCommand::Remove {
                    id: id_target,
                    expected,
                },
                params.idempotency_key,
            )
        }
        _ => return error_action(Some(id), rpc::RpcError::method_not_found(), false),
    };
    let Ok(key) = app::IdempotencyKey::parse(key) else {
        return invalid(id);
    };
    let result = app::with_log_caller(
        apps.store.clone(),
        access,
        registry.mcp_repository_write(access, authentication, command, key),
    )
    .await;
    match result {
        Ok(result) => success_action(
            id,
            rpc::McpRepositoryWriteResult {
                repository: result.repository.map(entry),
                repository_id: result.repository_id.to_string(),
                revision: result.revision.get(),
                replayed: result.replayed,
            },
            None,
        ),
        Err(error) => super::m3_rpc::m3_error(id, error),
    }
}
