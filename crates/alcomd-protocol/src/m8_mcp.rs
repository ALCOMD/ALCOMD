//! Approved M8 client identity DTOs. Capability publication requires Core wiring.

use serde::{Deserialize, Serialize};

pub const CAPABILITY_MCP_CLIENTS_V1: &str = "mcp.clients.v1";
pub const METHOD_MCP_CLIENTS_LIST: &str = "mcp.clients.list";
pub const METHOD_MCP_CLIENTS_CREATE: &str = "mcp.clients.create";
pub const METHOD_MCP_CLIENTS_SET_GRANTS: &str = "mcp.clients.setGrants";
pub const METHOD_MCP_CLIENTS_ISSUE_CREDENTIAL: &str = "mcp.clients.issueCredential";
pub const METHOD_MCP_CLIENTS_REVOKE: &str = "mcp.clients.revoke";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpClientGrant {
    pub permission: String,
    pub resource_kind: String,
    pub resource_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpClientRecord {
    pub client_id: String,
    pub principal_id: String,
    pub display_name: String,
    pub revision: u64,
    pub created_at_ms: u64,
    pub revoked_at_ms: Option<u64>,
    pub grants: Vec<McpClientGrant>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpClientsListParams {
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpClientsListResult {
    pub items: Vec<McpClientRecord>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpClientsCreateParams {
    pub idempotency_key: String,
    pub display_name: String,
    #[serde(default)]
    pub grants: Vec<McpClientGrant>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpClientMutationParams {
    pub idempotency_key: String,
    pub client_id: String,
    pub expected_revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpClientsSetGrantsParams {
    pub idempotency_key: String,
    pub client_id: String,
    pub expected_revision: u64,
    pub grants: Vec<McpClientGrant>,
}

/// One-shot secret response. Never serialize it into an idempotency record.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpClientCredentialResult {
    pub client_id: String,
    pub credential_id: String,
    pub revision: u64,
    pub credential: String,
}

impl std::fmt::Debug for McpClientCredentialResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpClientCredentialResult")
            .field("client_id", &self.client_id)
            .field("credential_id", &self.credential_id)
            .field("revision", &self.revision)
            .field("credential", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn mcp_client_creation_defaults_to_zero_grants_and_rejects_identity_injection() {
        let value: McpClientsCreateParams =
            serde_json::from_value(json!({"displayName":"Client","idempotencyKey":"test"}))
                .expect("default");
        assert!(value.grants.is_empty());
        for extra in ["principalId", "credential", "localOwner"] {
            let mut input = json!({"displayName":"Client","idempotencyKey":"test"});
            input[extra] = json!("injected");
            assert!(serde_json::from_value::<McpClientsCreateParams>(input).is_err());
        }
    }

    #[test]
    fn mcp_credential_debug_is_redacted_but_explicit_wire_response_is_available() {
        let value = McpClientCredentialResult {
            client_id: "client".into(),
            credential_id: "credential-id".into(),
            revision: 1,
            credential: "one-shot-secret".into(),
        };
        assert!(!format!("{value:?}").contains("one-shot-secret"));
        assert_eq!(
            serde_json::to_value(value).expect("explicit response")["credential"],
            "one-shot-secret"
        );
    }
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpAuthentication {
    pub kind: String,
    pub credential: String,
}
impl std::fmt::Debug for McpAuthentication {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("McpAuthentication([REDACTED])")
    }
}

pub(crate) fn deserialize_authentication<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<McpAuthentication>, D::Error> {
    McpAuthentication::deserialize(deserializer).map(Some)
}
