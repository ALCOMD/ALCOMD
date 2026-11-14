//! Core OAuth use-case records, separate from public protocol DTOs.
use crate::McpGrant;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OAuthRegistration {
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
pub struct OAuthRegistrationSetParams {
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
pub struct OAuthRegistrationRevokeParams {
    pub oauth_client_id: String,
    pub expected_revision: u64,
    pub idempotency_key: String,
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OAuthPageParams {
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OAuthRegistrationPage {
    pub items: Vec<OAuthRegistration>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OAuthRegistrationResult {
    pub registration: OAuthRegistration,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OAuthBeginParams {
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
pub struct OAuthBeginResult {
    pub request_id: String,
    pub browser_secret: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OAuthFinishParams {
    pub request_id: String,
    pub browser_secret: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OAuthFinishResult {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_url: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OAuthTokenParams {
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
pub struct OAuthTokenResult {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OAuthRevokeParams {
    pub oauth_client_id: String,
    pub token: String,
    pub client_secret: Option<String>,
    pub token_type_hint: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OAuthRequestGetParams {
    pub request_id: String,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OAuthRequestReview {
    pub request_id: String,
    pub oauth_client_id: String,
    pub client_id: String,
    pub display_name: String,
    pub issuer: String,
    pub resource: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
    pub grants: Vec<McpGrant>,
    pub next_cursor: Option<String>,
    pub approval_fingerprint: String,
    pub status: String,
    pub expires_at_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OAuthRequestDecideParams {
    pub request_id: String,
    pub approve: bool,
    pub grants: Vec<McpGrant>,
    pub expected_fingerprint: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OAuthRequestDecision {
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
    OAuthRegistrationResult,
    OAuthBeginParams,
    OAuthBeginResult,
    OAuthFinishParams,
    OAuthFinishResult,
    OAuthTokenParams,
    OAuthTokenResult,
    OAuthRevokeParams
);
