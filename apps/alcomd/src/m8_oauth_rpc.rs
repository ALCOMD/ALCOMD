use crate::{
    ConnectionState, DispatchAction, current_time_ms, error_action, require_capability,
    success_action,
};
use alcomd_application::{self as app, AccessContext, OAuthApplication, OAuthEntropy, OAuthError};
use alcomd_protocol::*;
use alcomd_store::StateStoreHandle;
use serde::{Serialize, de::DeserializeOwned};

pub(super) fn error(id: String, error: OAuthError, close: bool) -> DispatchAction {
    let (mut rpc, tag) = match error {
        OAuthError::InvalidInput => (RpcError::invalid_request(), "invalid_request"),
        OAuthError::InvalidScope => (RpcError::invalid_request(), "invalid_scope"),
        OAuthError::InvalidTarget => (RpcError::invalid_request(), "invalid_target"),
        OAuthError::InvalidClient => (RpcError::permission_denied(), "invalid_client"),
        OAuthError::Denied => (RpcError::permission_denied(), "access_denied"),
        OAuthError::RevisionConflict => (RpcError::revision_conflict(), "invalid_grant"),
        OAuthError::IdempotencyConflict => (RpcError::idempotency_conflict(), "invalid_request"),
        OAuthError::LimitExceeded => (RpcError::invalid_request(), "temporarily_unavailable"),
        OAuthError::Unavailable => (RpcError::store_unavailable(), "temporarily_unavailable"),
        OAuthError::InvalidGrant | OAuthError::Expired | OAuthError::ResponseUnavailable => {
            let code = match error {
                OAuthError::Expired => "mcp_oauth_expired",
                OAuthError::ResponseUnavailable => "mcp_oauth_response_unavailable",
                _ => "mcp_oauth_invalid_grant",
            };
            (
                RpcError {
                    code: code.into(),
                    message: "The OAuth authorization is invalid or no longer available.".into(),
                    diagnostic_id: None,
                    data: None,
                },
                "invalid_grant",
            )
        }
    };
    rpc.data = Some(serde_json::json!({"oauthError":tag}));
    error_action(Some(id), rpc, close)
}
fn convert<T: DeserializeOwned>(value: impl Serialize) -> Result<T, OAuthError> {
    serde_json::from_value(serde_json::to_value(value).map_err(|_| OAuthError::InvalidInput)?)
        .map_err(|_| OAuthError::InvalidInput)
}
fn value(value: impl Serialize) -> Result<serde_json::Value, OAuthError> {
    serde_json::to_value(value).map_err(|_| OAuthError::Unavailable)
}
fn entropy() -> Result<OAuthEntropy, OAuthError> {
    let mut bytes = [0u8; 64];
    getrandom::fill(&mut bytes).map_err(|_| OAuthError::Unavailable)?;
    Ok(OAuthEntropy {
        id: app::McpClientId::new().to_string(),
        secret: app::oauth_hex(&bytes[..32]),
        extra: app::oauth_hex(&bytes[32..]),
    })
}
pub(super) fn ingress_method(method: &str) -> bool {
    matches!(
        method,
        METHOD_MCP_OAUTH_BEGIN
            | METHOD_MCP_OAUTH_FINISH
            | METHOD_MCP_OAUTH_TOKEN
            | METHOD_MCP_OAUTH_REVOKE
    )
}
pub(super) fn required_scopes(method: &str) -> &'static [&'static str] {
    match method {
        "projects.list" | "projects.get" => &["projects.read"],
        "packages.planRemove" => &["projects.read", "repositories.read", "packages.read"],
        METHOD_MCP_PACKAGES_QUERY => &["repositories.read", "packages.read"],
        METHOD_MCP_PACKAGES_PLAN => &["projects.read", "repositories.read", "packages.read"],
        "packages.applyPlan" => &[
            "projects.read",
            "repositories.read",
            "packages.read",
            "packages.manage",
        ],
        "operations.get" | "operations.list" => &["operations.read"],
        "operations.cancel" => &["operations.cancel"],
        "state.check" => &["state.check"],
        "events.list" => &["events.read"],
        _ => &[],
    }
}
pub(super) async fn dispatch(
    request: RequestEnvelope,
    state: &ConnectionState,
    service: &OAuthApplication<StateStoreHandle>,
    access: &AccessContext,
) -> DispatchAction {
    if let Some(action) = require_capability(&request.id, state, CAPABILITY_MCP_OAUTH_V1) {
        return action;
    }
    if state.oauth_endpoint != ingress_method(&request.method) {
        return error(request.id, OAuthError::Denied, false);
    }
    let id = request.id.clone();
    let outcome = async {
        let now = current_time_ms().map_err(|_| OAuthError::Unavailable)?;
        match request.method.as_str() {
            METHOD_MCP_OAUTH_BEGIN => {
                let input: McpOAuthBeginParams = convert(request.params)?;
                value(service.begin(convert(input)?, entropy()?, now).await?)
            }
            METHOD_MCP_OAUTH_FINISH => {
                let input: McpOAuthFinishParams = convert(request.params)?;
                value(service.finish(convert(input)?, entropy()?, now).await?)
            }
            METHOD_MCP_OAUTH_TOKEN => {
                let input: McpOAuthTokenParams = convert(request.params)?;
                value(service.token(convert(input)?, entropy()?, now).await?)
            }
            METHOD_MCP_OAUTH_REVOKE => {
                let input: McpOAuthRevokeParams = convert(request.params)?;
                service.revoke(convert(input)?, now).await?;
                value(serde_json::json!({}))
            }
            METHOD_MCP_OAUTH_REGISTRATIONS_LIST => {
                let input: McpOAuthPageParams = convert(request.params)?;
                value(service.list(access, convert(input)?).await?)
            }
            METHOD_MCP_OAUTH_REGISTRATIONS_SET => {
                let input: McpOAuthRegistrationSetParams = convert(request.params)?;
                value(
                    service
                        .set_registration(access, convert(input)?, entropy()?, now)
                        .await?,
                )
            }
            METHOD_MCP_OAUTH_REGISTRATIONS_REVOKE => {
                let input: McpOAuthRegistrationRevokeParams = convert(request.params)?;
                value(
                    service
                        .revoke_registration(access, convert(input)?, now)
                        .await?,
                )
            }
            METHOD_MCP_OAUTH_REQUESTS_GET => {
                let input: McpOAuthRequestGetParams = convert(request.params)?;
                value(service.get(access, convert(input)?, now).await?)
            }
            METHOD_MCP_OAUTH_REQUESTS_DECIDE => {
                let input: McpOAuthRequestDecideParams = convert(request.params)?;
                value(service.decide(access, convert(input)?, now).await?)
            }
            _ => Err(OAuthError::Denied),
        }
    }
    .await;
    match outcome {
        Ok(v) => success_action(id, v, None),
        Err(e) => error(id, e, false),
    }
}
