//! Approved, bounded OAuth RPC DTOs. Secrets have no printable Debug representation.
use crate::McpClientGrant;
use serde::{Deserialize, Serialize};

pub const CAPABILITY_MCP_OAUTH_V1: &str = "mcp.oauth.v1";
pub const METHOD_MCP_OAUTH_BEGIN: &str = "mcp.oauth.begin";
pub const METHOD_MCP_OAUTH_FINISH: &str = "mcp.oauth.finish";
pub const METHOD_MCP_OAUTH_TOKEN: &str = "mcp.oauth.token";
pub const METHOD_MCP_OAUTH_REVOKE: &str = "mcp.oauth.revoke";
pub const METHOD_MCP_OAUTH_REGISTRATIONS_LIST: &str = "mcp.oauth.registrations.list";
pub const METHOD_MCP_OAUTH_REGISTRATIONS_SET: &str = "mcp.oauth.registrations.set";
pub const METHOD_MCP_OAUTH_REGISTRATIONS_REVOKE: &str = "mcp.oauth.registrations.revoke";
pub const METHOD_MCP_OAUTH_REQUESTS_GET: &str = "mcp.oauth.requests.get";
pub const METHOD_MCP_OAUTH_REQUESTS_DECIDE: &str = "mcp.oauth.requests.decide";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpOAuthRegistration {
    pub oauth_client_id: String,
    pub client_id: String,
    pub issuer: String,
    pub resource: String,
    pub redirect_uris: Vec<String>,
    pub confidential: bool,
    pub allow_refresh: bool,
    pub revision: u64,
    pub created_at_ms: u64,
    pub revoked_at_ms: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpOAuthRegistrationSetParams {
    pub client_id: String,
    pub issuer: String,
    pub resource: String,
    pub redirect_uris: Vec<String>,
    pub confidential: bool,
    pub allow_refresh: bool,
    pub expected_revision: u64,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpOAuthRegistrationRevokeParams {
    pub oauth_client_id: String,
    pub expected_revision: u64,
    pub idempotency_key: String,
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpOAuthPageParams {
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpOAuthRegistrationPage {
    pub items: Vec<McpOAuthRegistration>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpOAuthRegistrationResult {
    pub registration: McpOAuthRegistration,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpOAuthBeginParams {
    pub oauth_client_id: String,
    pub issuer: String,
    pub resource: String,
    pub redirect_uri: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
    pub scopes: Vec<String>,
    pub state: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpOAuthBeginResult {
    pub request_id: String,
    pub browser_secret: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpOAuthFinishParams {
    pub request_id: String,
    pub browser_secret: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpOAuthFinishResult {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_url: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpOAuthTokenParams {
    pub grant_type: String,
    pub oauth_client_id: String,
    pub resource: String,
    pub code: Option<String>,
    pub redirect_uri: Option<String>,
    pub code_verifier: Option<String>,
    pub refresh_token: Option<String>,
    pub scopes: Option<Vec<String>>,
    pub client_secret: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpOAuthTokenResult {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpOAuthRevokeParams {
    pub oauth_client_id: String,
    pub token: String,
    pub client_secret: Option<String>,
    pub token_type_hint: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpOAuthRequestGetParams {
    pub request_id: String,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpOAuthRequestReview {
    pub request_id: String,
    pub oauth_client_id: String,
    pub client_id: String,
    pub display_name: String,
    pub issuer: String,
    pub resource: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
    pub grants: Vec<McpClientGrant>,
    pub next_cursor: Option<String>,
    pub approval_fingerprint: String,
    pub status: String,
    pub expires_at_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpOAuthRequestDecideParams {
    pub request_id: String,
    pub approve: bool,
    pub grants: Vec<McpClientGrant>,
    pub expected_fingerprint: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpOAuthRequestDecision {
    pub request_id: String,
    pub status: String,
}

macro_rules! redacted {
    ($($ty:ty),+ $(,)?) => {$(impl std::fmt::Debug for $ty {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(concat!(stringify!($ty), "([REDACTED])"))
        }
    })+};
}
redacted!(
    McpOAuthRegistrationResult,
    McpOAuthBeginParams,
    McpOAuthBeginResult,
    McpOAuthFinishParams,
    McpOAuthFinishResult,
    McpOAuthTokenParams,
    McpOAuthTokenResult,
    McpOAuthRevokeParams
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClientInfo, HelloParams, McpAuthentication};
    use serde_json::json;
    #[test]
    fn oauth_hello_roles_are_disjoint_and_require_an_exact_access_token_shape() {
        let valid = |v| {
            serde_json::from_value::<McpAuthentication>(v).is_ok_and(|auth| {
                HelloParams {
                    rpc_version: 1,
                    client: ClientInfo {
                        name: "alcomd-gui".into(),
                        version: "test".into(),
                        instance_id: "test".into(),
                    },
                    capabilities: vec![],
                    authentication: Some(auth),
                }
                .validate()
                .is_ok()
            })
        };
        assert!(valid(json!({"kind":"mcp-oauth-endpoint"})));
        assert!(valid(
            json!({"kind":"mcp-oauth-access","credential":format!("oa1_{}","a".repeat(64)),"resource":"https://localhost:5443/mcp"})
        ));
        for input in [
            json!({"kind":"mcp-oauth-endpoint","credential":"unexpected"}),
            json!({"kind":"mcp-oauth-endpoint","resource":"https://localhost:5443/mcp"}),
            json!({"kind":"mcp-oauth-access","credential":"a".repeat(64),"resource":"https://localhost:5443/mcp"}),
            json!({"kind":"mcp-oauth-access","credential":format!("or1_{}","a".repeat(64)),"resource":"https://localhost:5443/mcp"}),
            json!({"kind":"mcp-oauth-access","credential":format!("oa1_{}","a".repeat(64))}),
            json!({"kind":"mcp-oauth-endpoint","resource":null}),
            json!({"kind":"owner"}),
        ] {
            assert!(!valid(input));
        }
    }
    #[test]
    fn oauth_secrets_are_only_serialized_in_explicit_wire_dtos_never_debug() {
        let input = McpOAuthTokenParams {
            grant_type: "refresh_token".into(),
            oauth_client_id: "client".into(),
            resource: "resource".into(),
            code: None,
            redirect_uri: None,
            code_verifier: Some("verifier-marker".into()),
            refresh_token: Some("refresh-marker".into()),
            scopes: None,
            client_secret: Some("client-secret-marker".into()),
        };
        let debug = format!("{input:?}");
        for secret in ["verifier-marker", "refresh-marker", "client-secret-marker"] {
            assert!(!debug.contains(secret));
        }
        let encoded = serde_json::to_string(&input).expect("explicit wire");
        assert!(encoded.contains("refresh-marker"));
        let mut raw = serde_json::to_value(input).expect("wire");
        raw["localOwner"] = json!(true);
        assert!(serde_json::from_value::<McpOAuthTokenParams>(raw).is_err());
    }
}
