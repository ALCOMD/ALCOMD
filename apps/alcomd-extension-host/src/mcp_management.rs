use super::HostState;
use alcomd_application::{
    MCP_MANAGEMENT_BYTES, ManagementError, ManagementErrorCode as E, ManagementResponse as R,
};
use serde_json::{Value, json};

mod bindings {
    wasmtime::component::bindgen!({
        path: "../../specs/extensions/wit/mcp-management-v1",
        world: "management-v1",
        imports: { default: async },
        exports: { default: async },
    });
}
use bindings::alcomd::mcp_management::clients as wit;

pub(super) fn add_to_linker(
    linker: &mut wasmtime::component::Linker<HostState>,
) -> anyhow::Result<()> {
    wit::add_to_linker::<_, wasmtime::component::HasSelf<_>>(linker, |state| state)
        .map_err(|_| anyhow::anyhow!("management interface link failed"))
}
fn failure(code: E) -> wit::ManagementError {
    convert_error(ManagementError::from(code))
}
fn convert_error(error: ManagementError) -> wit::ManagementError {
    let code = match error.code {
        E::InvalidArgument => wit::ErrorCode::InvalidArgument,
        E::PermissionDenied => wit::ErrorCode::PermissionDenied,
        E::ScopeDenied => wit::ErrorCode::ScopeDenied,
        E::LeaseExpired => wit::ErrorCode::LeaseExpired,
        E::LeaseRevoked => wit::ErrorCode::LeaseRevoked,
        E::RevisionConflict => wit::ErrorCode::RevisionConflict,
        E::IdempotencyConflict => wit::ErrorCode::IdempotencyConflict,
        E::ResourceLimit => wit::ErrorCode::ResourceLimit,
        E::Cancelled => wit::ErrorCode::Cancelled,
        E::Unavailable => wit::ErrorCode::Unavailable,
        E::WriteOutcomeUnknown => wit::ErrorCode::WriteOutcomeUnknown,
        E::InternalError => wit::ErrorCode::InternalError,
    };
    wit::ManagementError {
        code,
        diagnostic_id: error.diagnostic_id,
    }
}
fn client(value: alcomd_application::ManagementClientSummary) -> wit::ClientSummary {
    wit::ClientSummary {
        client_id: value.client_id.to_string(),
        display_name: value.display_name,
        revision: value.revision,
        created_at_ms: value.created_at_ms,
        revoked_at_ms: value.revoked_at_ms,
        grant_count: value.grant_count,
    }
}
fn grants(items: Vec<wit::Grant>) -> Vec<alcomd_application::McpGrant> {
    items
        .into_iter()
        .map(|g| alcomd_application::McpGrant {
            permission: g.permission,
            resource_kind: g.resource_kind,
            resource_id: g.resource_id,
        })
        .collect()
}
impl HostState {
    fn management(
        &mut self,
        method: &str,
        input: Value,
        write: bool,
    ) -> Result<R, wit::ManagementError> {
        if serde_json::to_vec(&input).map_or(true, |v| v.len() > MCP_MANAGEMENT_BYTES) {
            return Err(failure(E::ResourceLimit));
        }
        let value = self
            .protocol
            .capability(&format!("host-mcp-clients.{method}"), input)
            .map_err(|error| {
                use super::types::ErrorCode;
                failure(match error.code {
                    ErrorCode::PermissionDenied => E::PermissionDenied,
                    ErrorCode::ScopeDenied => E::ScopeDenied,
                    ErrorCode::LeaseExpired => E::LeaseExpired,
                    ErrorCode::LeaseRevoked => E::LeaseRevoked,
                    ErrorCode::Cancelled => E::Cancelled,
                    ErrorCode::ResourceLimit => E::ResourceLimit,
                    _ if write => E::WriteOutcomeUnknown,
                    _ => E::Unavailable,
                })
            })?;
        if serde_json::to_vec(&value).map_or(true, |v| v.len() > MCP_MANAGEMENT_BYTES) {
            return Err(failure(if write {
                E::WriteOutcomeUnknown
            } else {
                E::ResourceLimit
            }));
        }
        if let Some(error) = value.get("error") {
            if let Ok(error) = serde_json::from_value::<ManagementError>(error.clone()) {
                return Err(convert_error(error));
            }
            let code = match error.get("code").and_then(Value::as_str) {
                Some("extension_permission_denied") => E::PermissionDenied,
                Some("extension_scope_denied") => E::ScopeDenied,
                Some("extension_instance_stale") => E::LeaseRevoked,
                Some("extension_resource_limit") => E::ResourceLimit,
                Some("cancelled") => E::Cancelled,
                _ if write => E::WriteOutcomeUnknown,
                _ => E::Unavailable,
            };
            return Err(failure(code));
        }
        serde_json::from_value(value.get("ok").cloned().ok_or_else(|| {
            failure(if write {
                E::WriteOutcomeUnknown
            } else {
                E::InternalError
            })
        })?)
        .map_err(|_| {
            failure(if write {
                E::WriteOutcomeUnknown
            } else {
                E::InternalError
            })
        })
    }
}
impl wit::Host for HostState {
    async fn list(
        &mut self,
        cursor: Option<String>,
        limit: Option<u32>,
    ) -> Result<wit::ClientPage, wit::ManagementError> {
        match self.management("list", json!({"cursor":cursor,"limit":limit}), false)? {
            R::Clients(page) => Ok(wit::ClientPage {
                items: page.items.into_iter().map(client).collect(),
                next_cursor: page.next_cursor,
                can_manage: page.can_manage,
            }),
            _ => Err(failure(E::InternalError)),
        }
    }
    async fn grants(
        &mut self,
        client_id: String,
        expected_revision: u64,
        cursor: Option<String>,
        limit: Option<u32>,
    ) -> Result<wit::GrantPage, wit::ManagementError> {
        match self.management("grants",json!({"clientId":client_id,"expectedRevision":expected_revision,"cursor":cursor,"limit":limit}),false)? {
            R::Grants(page) => Ok(wit::GrantPage { revision: page.revision, items: page.items.into_iter().map(|g|wit::Grant { permission:g.permission,resource_kind:g.resource_kind,resource_id:g.resource_id }).collect(), next_cursor: page.next_cursor }),
            _ => Err(failure(E::InternalError)),
        }
    }
    async fn projects(
        &mut self,
        cursor: Option<String>,
        limit: Option<u32>,
    ) -> Result<wit::ProjectPage, wit::ManagementError> {
        match self.management("projects", json!({"cursor":cursor,"limit":limit}), false)? {
            R::Projects(page) => Ok(wit::ProjectPage {
                items: page
                    .items
                    .into_iter()
                    .map(|p| wit::ProjectSummary {
                        project_id: p.project_id,
                        name: p.name,
                    })
                    .collect(),
                next_cursor: page.next_cursor,
            }),
            _ => Err(failure(E::InternalError)),
        }
    }
    async fn create(
        &mut self,
        display_name: String,
        idempotency_key: String,
    ) -> Result<wit::ClientSummary, wit::ManagementError> {
        match self.management(
            "create",
            json!({"displayName":display_name,"idempotencyKey":idempotency_key}),
            true,
        )? {
            R::Client(value) => Ok(client(value)),
            _ => Err(failure(E::WriteOutcomeUnknown)),
        }
    }
    async fn change_grants(
        &mut self,
        client_id: String,
        expected_revision: u64,
        add: Vec<wit::Grant>,
        remove: Vec<wit::Grant>,
        idempotency_key: String,
    ) -> Result<wit::ClientSummary, wit::ManagementError> {
        match self.management("change-grants",json!({"clientId":client_id,"expectedRevision":expected_revision,"add":grants(add),"remove":grants(remove),"idempotencyKey":idempotency_key}),true)? {
            R::Client(value) => Ok(client(value)), _ => Err(failure(E::WriteOutcomeUnknown)),
        }
    }
    async fn revoke(
        &mut self,
        client_id: String,
        expected_revision: u64,
        idempotency_key: String,
    ) -> Result<wit::ClientSummary, wit::ManagementError> {
        match self.management("revoke",json!({"clientId":client_id,"expectedRevision":expected_revision,"idempotencyKey":idempotency_key}),true)? {
            R::Client(value) => Ok(client(value)), _ => Err(failure(E::WriteOutcomeUnknown)),
        }
    }
}
