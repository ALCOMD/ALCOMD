use crate::{AlcomdClient, ClientConfig, ClientError};
use alcomd_protocol::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OAuthRejection {
    InvalidToken,
}

impl ClientConfig {
    /// A public OAuth protocol ingress, never a native owner connection.
    pub fn with_oauth_endpoint(mut self) -> Self {
        self.authentication = Some(McpAuthentication {
            kind: "mcp-oauth-endpoint".into(),
            credential: String::new(),
            resource: None,
        });
        self
    }
    /// A Core-issued OAuth access token, isolated from generic MCP credentials.
    pub fn with_oauth_access(mut self, credential: String, resource: String) -> Self {
        self.authentication = Some(McpAuthentication {
            kind: "mcp-oauth-access".into(),
            credential,
            resource: Some(resource),
        });
        self
    }
}
impl AlcomdClient {
    /// One-call transport metadata; never a substitute for Core permission checks.
    pub fn take_oauth_rejection(&mut self) -> Option<OAuthRejection> {
        self.oauth_rejection.take()
    }
    pub async fn oauth_begin(
        &mut self,
        p: McpOAuthBeginParams,
    ) -> Result<McpOAuthBeginResult, ClientError> {
        self.call(METHOD_MCP_OAUTH_BEGIN, p).await
    }
    pub async fn oauth_finish(
        &mut self,
        p: McpOAuthFinishParams,
    ) -> Result<McpOAuthFinishResult, ClientError> {
        self.call(METHOD_MCP_OAUTH_FINISH, p).await
    }
    pub async fn oauth_token(
        &mut self,
        p: McpOAuthTokenParams,
    ) -> Result<McpOAuthTokenResult, ClientError> {
        self.call(METHOD_MCP_OAUTH_TOKEN, p).await
    }
    pub async fn oauth_revoke(
        &mut self,
        p: McpOAuthRevokeParams,
    ) -> Result<serde_json::Value, ClientError> {
        self.call(METHOD_MCP_OAUTH_REVOKE, p).await
    }
    pub async fn oauth_registrations_list(
        &mut self,
        p: McpOAuthPageParams,
    ) -> Result<McpOAuthRegistrationPage, ClientError> {
        self.call(METHOD_MCP_OAUTH_REGISTRATIONS_LIST, p).await
    }
    pub async fn oauth_registration_set(
        &mut self,
        p: McpOAuthRegistrationSetParams,
    ) -> Result<McpOAuthRegistrationResult, ClientError> {
        self.call(METHOD_MCP_OAUTH_REGISTRATIONS_SET, p).await
    }
    pub async fn oauth_registration_revoke(
        &mut self,
        p: McpOAuthRegistrationRevokeParams,
    ) -> Result<McpOAuthRegistration, ClientError> {
        self.call(METHOD_MCP_OAUTH_REGISTRATIONS_REVOKE, p).await
    }
    pub async fn oauth_request_get(
        &mut self,
        p: McpOAuthRequestGetParams,
    ) -> Result<McpOAuthRequestReview, ClientError> {
        self.call(METHOD_MCP_OAUTH_REQUESTS_GET, p).await
    }
    pub async fn oauth_request_decide(
        &mut self,
        p: McpOAuthRequestDecideParams,
    ) -> Result<McpOAuthRequestDecision, ClientError> {
        self.call(METHOD_MCP_OAUTH_REQUESTS_DECIDE, p).await
    }
}
