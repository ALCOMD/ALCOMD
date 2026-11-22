//! Approved M8 client identity DTOs. Capability publication requires Core wiring.

use serde::{Deserialize, Serialize};

pub const CAPABILITY_MCP_CLIENTS_V1: &str = "mcp.clients.v1";
pub const METHOD_MCP_CLIENTS_LIST: &str = "mcp.clients.list";
pub const METHOD_MCP_CLIENTS_CREATE: &str = "mcp.clients.create";
pub const METHOD_MCP_CLIENTS_ISSUE_CREDENTIAL: &str = "mcp.clients.issueCredential";
pub const METHOD_MCP_CLIENTS_REVOKE: &str = "mcp.clients.revoke";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpClientRecord {
    pub client_id: String,
    pub principal_id: String,
    pub display_name: String,
    pub revision: u64,
    pub created_at_ms: u64,
    pub revoked_at_ms: Option<u64>,
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
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpClientMutationParams {
    pub idempotency_key: String,
    pub client_id: String,
    pub expected_revision: u64,
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
    fn mcp_client_creation_rejects_identity_and_grant_injection() {
        let value: McpClientsCreateParams =
            serde_json::from_value(json!({"displayName":"Client","idempotencyKey":"test"}))
                .expect("default");
        assert_eq!(value.display_name, "Client");
        for extra in ["principalId", "credential", "localOwner", "grants"] {
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
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub credential: String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub resource: Option<String>,
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

pub const METHOD_MCP_CLIENT_REQUESTS_CREATE: &str = "mcp.clients.requests.create";
pub const METHOD_MCP_CLIENT_REQUESTS_GET: &str = "mcp.clients.requests.get";
pub const METHOD_MCP_CLIENT_REQUESTS_LIST: &str = "mcp.clients.requests.list";
pub const METHOD_MCP_CLIENT_REQUESTS_DECIDE: &str = "mcp.clients.requests.decide";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ClientManagementAction {
    CreateClient {
        display_name: String,
    },
    IssueCredential {
        client_id: String,
        expected_revision: u64,
    },
    RevokeClient {
        client_id: String,
        expected_revision: u64,
    },
    SetOAuthRegistration {
        client_id: String,
        issuer: String,
        resource: String,
        redirect_uris: Vec<String>,
        confidential: bool,
        allow_refresh: bool,
        expected_revision: u64,
    },
    RevokeOAuthRegistration {
        oauth_client_id: String,
        expected_revision: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClientRequestStatus {
    Pending,
    Approved,
    Denied,
    Expired,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientManagementReview {
    pub request_id: String,
    pub requester: String,
    pub action: ClientManagementAction,
    pub status: ClientRequestStatus,
    pub expires_at_ms: u64,
    pub fingerprint: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientManagementDecision {
    pub request: ClientManagementReview,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientRequestPage {
    pub items: Vec<ClientManagementReview>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpClientRequestCreateParams {
    pub action: ClientManagementAction,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpClientRequestGetParams {
    pub request_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpClientRequestDecideParams {
    pub request_id: String,
    pub approve: bool,
    pub expected_fingerprint: String,
    pub idempotency_key: String,
}

/// Copy the definitions reachable from one advertised tool schema so a catalogue
/// page stays bounded as independently published tools are added.
pub fn mcp_tool_schema(
    schema: &serde_json::Value,
    contract: &serde_json::Value,
) -> serde_json::Value {
    use serde_json::Value;
    fn references(value: &Value, pending: &mut Vec<String>) {
        match value {
            Value::Object(object) => {
                if let Some(path) = object
                    .get("$ref")
                    .and_then(Value::as_str)
                    .and_then(|reference| reference.strip_prefix("#/$defs/"))
                {
                    let key = path.split('/').next().unwrap_or_default();
                    pending.push(key.replace("~1", "/").replace("~0", "~"));
                }
                for child in object.values() {
                    references(child, pending);
                }
            }
            Value::Array(values) => {
                for value in values {
                    references(value, pending);
                }
            }
            _ => {}
        }
    }
    let mut bound = schema.clone();
    let embedded = bound
        .as_object_mut()
        .and_then(|object| object.remove("$defs"));
    let definitions = embedded.as_ref().unwrap_or(&contract["$defs"]);
    let mut selected = serde_json::Map::new();
    let mut pending = Vec::new();
    references(&bound, &mut pending);
    while let Some(key) = pending.pop() {
        if selected.contains_key(&key) {
            continue;
        }
        if let Some(child) = definitions.get(&key) {
            references(child, &mut pending);
            selected.insert(key, child.clone());
        }
    }
    if !selected.is_empty() {
        bound["$defs"] = Value::Object(selected);
    }
    bound
}
