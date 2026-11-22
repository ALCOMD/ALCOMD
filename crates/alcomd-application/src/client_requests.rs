use crate::{
    AccessContext, IdempotencyKey, McpClientId, OAuthError, OAuthRegistrationSetParams, Permission,
    PrincipalId, oauth_validate_registration,
};
use serde::{Deserialize, Serialize};
use std::future::Future;

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
        client_id: McpClientId,
        expected_revision: u64,
    },
    RevokeClient {
        client_id: McpClientId,
        expected_revision: u64,
    },
    SetOAuthRegistration {
        client_id: McpClientId,
        issuer: String,
        resource: String,
        redirect_uris: Vec<String>,
        confidential: bool,
        allow_refresh: bool,
        expected_revision: u64,
    },
    RevokeOAuthRegistration {
        oauth_client_id: McpClientId,
        expected_revision: u64,
    },
}

impl ClientManagementAction {
    pub fn registration(&self, key: String) -> Option<OAuthRegistrationSetParams> {
        let Self::SetOAuthRegistration {
            client_id,
            issuer,
            resource,
            redirect_uris,
            confidential,
            allow_refresh,
            expected_revision,
        } = self
        else {
            return None;
        };
        Some(OAuthRegistrationSetParams {
            client_id: client_id.to_string(),
            issuer: issuer.clone(),
            resource: resource.clone(),
            redirect_uris: redirect_uris.clone(),
            confidential: *confidential,
            allow_refresh: *allow_refresh,
            expected_revision: *expected_revision,
            idempotency_key: key,
        })
    }

    pub fn validate(&self, key: &str) -> Result<(), OAuthError> {
        IdempotencyKey::parse(key.to_owned()).map_err(|_| OAuthError::InvalidInput)?;
        match self {
            Self::CreateClient { display_name }
                if display_name.trim().is_empty()
                    || display_name.chars().count() > 128
                    || display_name.chars().any(char::is_control) =>
            {
                Err(OAuthError::InvalidInput)
            }
            Self::IssueCredential {
                expected_revision, ..
            }
            | Self::RevokeClient {
                expected_revision, ..
            }
            | Self::RevokeOAuthRegistration {
                expected_revision, ..
            } if !(1..=i64::MAX as u64).contains(expected_revision) => {
                Err(OAuthError::InvalidInput)
            }
            Self::SetOAuthRegistration { .. } => oauth_validate_registration(
                &self
                    .registration(key.to_owned())
                    .ok_or(OAuthError::InvalidInput)?,
            ),
            _ => Ok(()),
        }
    }
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
    pub request_id: McpClientId,
    pub requester: PrincipalId,
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

pub trait ClientRequestStore {
    fn list_requests(
        &self,
        actor: PrincipalId,
        input: crate::OAuthPageParams,
        now: u64,
    ) -> impl Future<Output = Result<ClientRequestPage, OAuthError>> + Send;
    fn create_request(
        &self,
        requester: PrincipalId,
        action: ClientManagementAction,
        key: String,
        now: u64,
    ) -> impl Future<Output = Result<ClientManagementReview, OAuthError>> + Send;
    fn get_request(
        &self,
        actor: PrincipalId,
        id: McpClientId,
        now: u64,
    ) -> impl Future<Output = Result<ClientManagementReview, OAuthError>> + Send;
    fn decide_request(
        &self,
        owner: PrincipalId,
        input: ClientRequestDecisionInput,
        secret: String,
        now: u64,
    ) -> impl Future<Output = Result<ClientManagementDecision, OAuthError>> + Send;
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientRequestDecisionInput {
    pub request_id: McpClientId,
    pub approve: bool,
    pub expected_fingerprint: String,
    pub idempotency_key: String,
}

pub struct ClientRequestApplication<'a, S>(pub &'a S);

impl<S: ClientRequestStore> ClientRequestApplication<'_, S> {
    fn authorize(
        access: &AccessContext,
        permission: Permission,
    ) -> Result<PrincipalId, OAuthError> {
        if !access.is_interactive_client() {
            return Err(OAuthError::Denied);
        }
        access.require(permission).map_err(|_| OAuthError::Denied)?;
        Ok(access.principal().clone())
    }

    pub async fn list(
        &self,
        access: &AccessContext,
        input: crate::OAuthPageParams,
        now: u64,
    ) -> Result<ClientRequestPage, OAuthError> {
        let actor = Self::authorize(access, Permission::McpClientsRead)?;
        self.0.list_requests(actor, input, now).await
    }

    pub async fn create(
        &self,
        access: &AccessContext,
        action: ClientManagementAction,
        key: String,
        now: u64,
    ) -> Result<ClientManagementReview, OAuthError> {
        let requester = Self::authorize(access, Permission::McpClientsManage)?;
        action.validate(&key)?;
        self.0.create_request(requester, action, key, now).await
    }

    pub async fn get(
        &self,
        access: &AccessContext,
        id: McpClientId,
        now: u64,
    ) -> Result<ClientManagementReview, OAuthError> {
        let actor = Self::authorize(access, Permission::McpClientsRead)?;
        self.0.get_request(actor, id, now).await
    }

    pub async fn decide(
        &self,
        access: &AccessContext,
        input: ClientRequestDecisionInput,
        secret: String,
        now: u64,
    ) -> Result<ClientManagementDecision, OAuthError> {
        if !access.is_local_owner() {
            return Err(OAuthError::Denied);
        }
        let owner = Self::authorize(access, Permission::McpClientsManage)?;
        IdempotencyKey::parse(input.idempotency_key.clone())
            .map_err(|_| OAuthError::InvalidInput)?;
        self.0.decide_request(owner, input, secret, now).await
    }
}
