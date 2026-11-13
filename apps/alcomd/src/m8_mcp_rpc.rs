use crate::{
    ConnectionState, DispatchAction, current_time_ms, error_action, require_capability,
    success_action,
};
use alcomd_application::{
    AccessContext, McpClientId, McpGrant, McpIdentityApplication, McpIdentityError,
    McpIdentityWrite, PrincipalId, mcp_credential_hash,
};
use alcomd_protocol::*;
use alcomd_store::StateStoreHandle;

pub(super) fn error(id: String, error: McpIdentityError, close: bool) -> DispatchAction {
    let error = match error {
        McpIdentityError::PermissionDenied => RpcError::permission_denied(),
        McpIdentityError::RevisionConflict => RpcError::revision_conflict(),
        McpIdentityError::CredentialResponseUnavailable => {
            let mut error = RpcError::revision_conflict();
            error.message="The credential was issued, but its one-time response cannot be replayed. Inspect the client and explicitly issue a new credential if needed.".into();
            error
        }
        McpIdentityError::IdempotencyConflict => RpcError::idempotency_conflict(),
        McpIdentityError::InvalidInput | McpIdentityError::LimitExceeded => {
            RpcError::invalid_request()
        }
        McpIdentityError::Unavailable => RpcError::store_unavailable(),
    };
    error_action(Some(id), error, close)
}
fn grants(items: Vec<McpClientGrant>) -> Vec<McpGrant> {
    items
        .into_iter()
        .map(|g| McpGrant {
            permission: g.permission,
            resource_kind: g.resource_kind,
            resource_id: g.resource_id,
        })
        .collect()
}
fn id(value: &str) -> Result<McpClientId, McpIdentityError> {
    McpClientId::parse(value).map_err(|_| McpIdentityError::InvalidInput)
}

pub(super) async fn dispatch(
    request: RequestEnvelope,
    state: &ConnectionState,
    store: &StateStoreHandle,
    access: &AccessContext,
) -> DispatchAction {
    if let Some(action) = require_capability(&request.id, state, CAPABILITY_MCP_CLIENTS_V1) {
        return action;
    }
    let service = McpIdentityApplication::new(store.clone());
    let request_id = request.id.clone();
    let outcome = async {
        if request.method == METHOD_MCP_CLIENTS_LIST {
            let p: McpClientsListParams = serde_json::from_value(request.params)
                .map_err(|_| McpIdentityError::InvalidInput)?;
            let (items, next_cursor) = service.list(access, p.cursor, p.limit).await?;
            return serde_json::to_value(
                serde_json::json!({"items":items,"nextCursor":next_cursor}),
            )
            .map_err(|_| McpIdentityError::Unavailable);
        }
        let mut secret = None;
        let (key, command) = match request.method.as_str() {
            METHOD_MCP_CLIENTS_CREATE => {
                let p: McpClientsCreateParams = serde_json::from_value(request.params)
                    .map_err(|_| McpIdentityError::InvalidInput)?;
                (
                    p.idempotency_key,
                    McpIdentityWrite::Create {
                        name: p.display_name,
                        grants: grants(p.grants),
                    },
                )
            }
            METHOD_MCP_CLIENTS_SET_GRANTS => {
                let p: McpClientsSetGrantsParams = serde_json::from_value(request.params)
                    .map_err(|_| McpIdentityError::InvalidInput)?;
                (
                    p.idempotency_key,
                    McpIdentityWrite::SetGrants {
                        id: id(&p.client_id)?,
                        expected: p.expected_revision,
                        grants: grants(p.grants),
                    },
                )
            }
            METHOD_MCP_CLIENTS_ISSUE_CREDENTIAL | METHOD_MCP_CLIENTS_REVOKE => {
                let p: McpClientMutationParams = serde_json::from_value(request.params)
                    .map_err(|_| McpIdentityError::InvalidInput)?;
                let client_id = id(&p.client_id)?;
                let command = if request.method == METHOD_MCP_CLIENTS_ISSUE_CREDENTIAL {
                    if access.principal() != &PrincipalId::local_owner() {
                        return Err(McpIdentityError::PermissionDenied);
                    }
                    let mut entropy = [0_u8; 32];
                    getrandom::fill(&mut entropy).map_err(|_| McpIdentityError::Unavailable)?;
                    let value = entropy
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<String>();
                    let hash = mcp_credential_hash(&value);
                    secret = Some(value);
                    McpIdentityWrite::Issue {
                        id: client_id,
                        expected: p.expected_revision,
                        hash,
                    }
                } else {
                    McpIdentityWrite::Revoke {
                        id: client_id,
                        expected: p.expected_revision,
                    }
                };
                (p.idempotency_key, command)
            }
            _ => return Err(McpIdentityError::InvalidInput),
        };
        let now = current_time_ms().map_err(|_| McpIdentityError::Unavailable)?;
        let result = service.write(access, key, command, now).await?;
        if let Some(credential) = secret {
            serde_json::to_value(McpClientCredentialResult {
                client_id: result.client.client_id.to_string(),
                credential_id: result.credential_id.ok_or(McpIdentityError::Unavailable)?,
                revision: result.client.revision,
                credential,
            })
            .map_err(|_| McpIdentityError::Unavailable)
        } else {
            serde_json::to_value(result.client).map_err(|_| McpIdentityError::Unavailable)
        }
    }
    .await;
    match outcome {
        Ok(result) => success_action(request_id, result, None),
        Err(e) => error(request_id, e, false),
    }
}

/// Keep unsupported entry points closed even when a broad read permission is granted.
pub(super) fn allowed_client_method(method: &str) -> bool {
    matches!(
        method,
        "system.status"
            | "projects.get"
            | "projects.list"
            | "state.check"
            | "operations.get"
            | "operations.list"
            | "operations.cancel"
            | "events.list"
            | "packages.planRemove"
            | "packages.applyPlan"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use alcomd_application::McpIdentityStore;
    use std::collections::HashSet;

    #[tokio::test]
    async fn management_requires_negotiated_capability_and_permission_before_mutation() {
        let store = StateStoreHandle::open(
            std::env::temp_dir().join(format!("alcomd-m8-cap-{}.db", McpClientId::new())),
        )
        .expect("isolated store");
        let mut state = ConnectionState {
            mcp_hash: None,
            connection_id: "m8-management-gate".into(),
            client_instance_id: None,
            actor: "Local IPC".into(),
            handshake_complete: true,
            capabilities: HashSet::new(),
        };
        let request = || RequestEnvelope {
            id: "create".into(),
            method: METHOD_MCP_CLIENTS_CREATE.into(),
            params: serde_json::json!({"displayName":"Gate test","idempotencyKey":"gate"}),
        };
        let no_capability =
            dispatch(request(), &state, &store, &AccessContext::local_owner()).await;
        assert_eq!(
            no_capability.response["error"]["code"],
            "capability_required"
        );
        assert!(store.mcp_clients().await.expect("no mutation").is_empty());
        state.capabilities.insert(CAPABILITY_MCP_CLIENTS_V1.into());
        let unprivileged = AccessContext::new(
            PrincipalId::parse("test:unprivileged").expect("principal"),
            [],
        );
        let denied = dispatch(request(), &state, &store, &unprivileged).await;
        assert_eq!(denied.response["error"]["code"], "permission_denied");
        assert!(
            store
                .mcp_clients()
                .await
                .expect("still no mutation")
                .is_empty()
        );
        let accepted = dispatch(request(), &state, &store, &AccessContext::local_owner()).await;
        assert!(accepted.response.get("result").is_some());
        assert_eq!(
            store
                .mcp_clients()
                .await
                .expect("one authorized mutation")
                .len(),
            1
        );
    }
}
