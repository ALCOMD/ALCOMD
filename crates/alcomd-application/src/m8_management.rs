//! Bounded extension management projections. Credentials never cross this boundary.
use crate::{
    ExtensionInstanceLease, ExtensionUiInvocationAuthority, McpClient, McpClientId, McpGrant,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::future::Future;

pub const MCP_MANAGEMENT_BYTES: usize = 65_536;
pub const MCP_MANAGEMENT_INTERFACE: &str = "alcomd:mcp-management/clients@1.0.0";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManagementErrorCode {
    InvalidArgument,
    PermissionDenied,
    ScopeDenied,
    LeaseExpired,
    LeaseRevoked,
    RevisionConflict,
    IdempotencyConflict,
    ResourceLimit,
    Cancelled,
    Unavailable,
    WriteOutcomeUnknown,
    InternalError,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManagementError {
    pub code: ManagementErrorCode,
    pub diagnostic_id: Option<String>,
}
impl From<ManagementErrorCode> for ManagementError {
    fn from(code: ManagementErrorCode) -> Self {
        Self {
            code,
            diagnostic_id: None,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "method",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ManagementRequest {
    List {
        cursor: Option<String>,
        limit: Option<u32>,
    },
    Grants {
        client_id: McpClientId,
        expected_revision: u64,
        cursor: Option<String>,
        limit: Option<u32>,
    },
    Projects {
        cursor: Option<String>,
        limit: Option<u32>,
    },
    Create {
        display_name: String,
        idempotency_key: String,
    },
    ChangeGrants {
        client_id: McpClientId,
        expected_revision: u64,
        add: Vec<McpGrant>,
        remove: Vec<McpGrant>,
        idempotency_key: String,
    },
    Revoke {
        client_id: McpClientId,
        expected_revision: u64,
        idempotency_key: String,
    },
}
impl ManagementRequest {
    pub fn is_write(&self) -> bool {
        matches!(
            self,
            Self::Create { .. } | Self::ChangeGrants { .. } | Self::Revoke { .. }
        )
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManagementClientSummary {
    pub client_id: McpClientId,
    pub display_name: String,
    pub revision: u64,
    pub created_at_ms: u64,
    pub revoked_at_ms: Option<u64>,
    pub grant_count: u32,
}
impl From<&McpClient> for ManagementClientSummary {
    fn from(client: &McpClient) -> Self {
        Self {
            client_id: client.client_id,
            display_name: client.display_name.clone(),
            revision: client.revision,
            created_at_ms: client.created_at_ms,
            revoked_at_ms: client.revoked_at_ms,
            grant_count: client.grants.len() as u32,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManagementPage<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManagementClientPage {
    pub items: Vec<ManagementClientSummary>,
    pub next_cursor: Option<String>,
    /// Presentation hint only; every later write revalidates authority.
    pub can_manage: bool,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManagementGrantPage {
    pub revision: u64,
    pub items: Vec<McpGrant>,
    pub next_cursor: Option<String>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManagementProject {
    pub project_id: String,
    pub name: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum ManagementResponse {
    Clients(ManagementClientPage),
    Grants(ManagementGrantPage),
    Projects(ManagementPage<ManagementProject>),
    Client(ManagementClientSummary),
}
pub trait McpManagementStore: Clone + Send + Sync + 'static {
    fn mcp_management(
        &self,
        lease: ExtensionInstanceLease,
        authority: ExtensionUiInvocationAuthority,
        request: ManagementRequest,
        now_ms: u64,
    ) -> impl Future<Output = Result<ManagementResponse, ManagementError>> + Send;
}

/// Stable, canonical fingerprint of the trusted client authority, never guest metadata.
pub fn management_client_fingerprint(authority: &ExtensionUiInvocationAuthority) -> String {
    fn scopes(scope: &crate::AccessScopes) -> Vec<String> {
        match scope {
            crate::AccessScopes::None => vec![],
            crate::AccessScopes::All => vec!["*".into()],
            crate::AccessScopes::Exact(ids) => {
                let mut ids: Vec<_> = ids.iter().cloned().collect();
                ids.sort();
                ids
            }
        }
    }
    let access = &authority.client_access;
    let mut permissions: Vec<_> = access.permissions.iter().map(|p| p.as_str()).collect();
    permissions.sort();
    let value = serde_json::json!([
        access.principal().as_str(),
        permissions,
        scopes(&access.project_read_scopes),
        scopes(&access.extension_ui_scopes),
        authority.client_connection_id,
        authority.client_instance_id,
        authority.session_id
    ]);
    Sha256::digest(value.to_string().as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
