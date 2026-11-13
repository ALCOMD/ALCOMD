use crate::AccessContext;
pub use alcomd_domain::McpClientId;
use alcomd_domain::{Permission, PrincipalId, ProjectId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::future::Future;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpGrant {
    pub permission: String,
    pub resource_kind: String,
    pub resource_id: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpClient {
    pub client_id: McpClientId,
    pub principal_id: PrincipalId,
    pub display_name: String,
    pub revision: u64,
    pub created_at_ms: u64,
    pub revoked_at_ms: Option<u64>,
    pub grants: Vec<McpGrant>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum McpIdentityError {
    InvalidInput,
    PermissionDenied,
    RevisionConflict,
    CredentialResponseUnavailable,
    IdempotencyConflict,
    LimitExceeded,
    Unavailable,
}

pub enum McpIdentityWrite {
    Create {
        name: String,
        grants: Vec<McpGrant>,
    },
    SetGrants {
        id: McpClientId,
        expected: u64,
        grants: Vec<McpGrant>,
    },
    Issue {
        id: McpClientId,
        expected: u64,
        hash: [u8; 32],
    },
    Revoke {
        id: McpClientId,
        expected: u64,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpIdentityWriteResult {
    pub client: McpClient,
    pub credential_id: Option<String>,
}

pub trait McpIdentityStore: Clone + Send + Sync + 'static {
    fn mcp_clients(&self) -> impl Future<Output = Result<Vec<McpClient>, McpIdentityError>> + Send;
    fn mcp_authenticate(
        &self,
        hash: [u8; 32],
    ) -> impl Future<Output = Result<McpClient, McpIdentityError>> + Send;
    fn mcp_write(
        &self,
        actor: PrincipalId,
        key: String,
        command: McpIdentityWrite,
        now: u64,
    ) -> impl Future<Output = Result<McpIdentityWriteResult, McpIdentityError>> + Send;
}

pub fn mcp_credential_hash(secret: &str) -> [u8; 32] {
    Sha256::digest(secret.as_bytes()).into()
}

/// Validates only combinations currently enforced at every corresponding use case.
/// Unsupported write/global-owner grants are rejected, never silently widened.
pub fn validate_mcp_grants(grants: &[McpGrant]) -> Result<(), McpIdentityError> {
    if grants.len() > 4096 {
        return Err(McpIdentityError::LimitExceeded);
    }
    let mut keys = HashSet::new();
    let mut permissions = HashSet::new();
    for grant in grants {
        let valid = match grant.permission.as_str() {
            "projects.read" | "packages.manage" => {
                grant.resource_kind == "project"
                    && ProjectId::parse(&grant.resource_id)
                        .is_ok_and(|id| id.to_string() == grant.resource_id)
            }
            "state.check" | "operations.read" | "operations.cancel" | "events.read"
            | "repositories.read" | "packages.read" => {
                grant.resource_kind == "global" && grant.resource_id == "*"
            }
            _ => false,
        };
        if !valid || !keys.insert((&grant.permission, &grant.resource_kind, &grant.resource_id)) {
            return Err(McpIdentityError::InvalidInput);
        }
        permissions.insert(&grant.permission);
    }
    if permissions.len() > 64 {
        return Err(McpIdentityError::LimitExceeded);
    }
    Ok(())
}

pub fn mcp_access(client: &McpClient) -> Result<AccessContext, McpIdentityError> {
    if client.revoked_at_ms.is_some()
        || client.principal_id.as_str() != format!("mcp:{}", client.client_id)
    {
        return Err(McpIdentityError::PermissionDenied);
    }
    validate_mcp_grants(&client.grants)?;
    let mut permissions = Vec::new();
    let mut projects = Vec::new();
    let mut package_writes = Vec::new();
    for grant in &client.grants {
        permissions.push(match grant.permission.as_str() {
            "projects.read" => {
                projects.push(grant.resource_id.clone());
                Permission::ProjectsRead
            }
            "state.check" => Permission::StateCheck,
            "operations.read" => Permission::OperationsRead,
            "operations.cancel" => Permission::OperationsCancel,
            "events.read" => Permission::EventsRead,
            "repositories.read" => Permission::RepositoriesRead,
            "packages.read" => Permission::PackagesRead,
            "packages.manage" => {
                package_writes.push(grant.resource_id.clone());
                Permission::PackagesManage
            }
            _ => return Err(McpIdentityError::PermissionDenied),
        });
    }
    Ok(AccessContext::new(client.principal_id.clone(), permissions)
        .with_project_read_scopes(projects)
        .with_project_package_write_scopes(package_writes)
        .with_mcp_identity())
}

#[derive(Clone)]
pub struct McpIdentityApplication<S: McpIdentityStore> {
    store: S,
}
impl<S: McpIdentityStore> McpIdentityApplication<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }
    pub async fn authenticate(&self, hash: [u8; 32]) -> Result<AccessContext, McpIdentityError> {
        mcp_access(&self.store.mcp_authenticate(hash).await?)
    }
    pub async fn list(
        &self,
        access: &AccessContext,
        cursor: Option<String>,
        limit: Option<u32>,
    ) -> Result<(Vec<McpClient>, Option<String>), McpIdentityError> {
        access
            .require(Permission::McpClientsRead)
            .map_err(|_| McpIdentityError::PermissionDenied)?;
        let limit = limit.unwrap_or(100);
        if !(1..=1000).contains(&limit) {
            return Err(McpIdentityError::InvalidInput);
        }
        let all = self.store.mcp_clients().await?;
        let bytes = serde_json::to_vec(&(access.principal(), &all))
            .map_err(|_| McpIdentityError::Unavailable)?;
        let fingerprint = Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let start = if let Some(cursor) = cursor {
            let (hash, id) = cursor
                .split_once(':')
                .ok_or(McpIdentityError::InvalidInput)?;
            if hash != fingerprint {
                return Err(McpIdentityError::RevisionConflict);
            }
            all.iter()
                .position(|c| c.client_id.to_string() == id)
                .ok_or(McpIdentityError::RevisionConflict)?
                + 1
        } else {
            0
        };
        let mut page = Vec::new();
        let mut size = 1024;
        for client in all.iter().skip(start).take(limit as usize) {
            let len = serde_json::to_vec(client)
                .map_err(|_| McpIdentityError::Unavailable)?
                .len();
            if size + len > 3 * 1024 * 1024 {
                break;
            }
            size += len;
            page.push(client.clone());
        }
        let next = if start + page.len() < all.len() {
            Some(format!(
                "{fingerprint}:{}",
                page.last()
                    .ok_or(McpIdentityError::LimitExceeded)?
                    .client_id
            ))
        } else {
            None
        };
        Ok((page, next))
    }
    pub async fn write(
        &self,
        access: &AccessContext,
        key: String,
        command: McpIdentityWrite,
        now: u64,
    ) -> Result<McpIdentityWriteResult, McpIdentityError> {
        access
            .require(Permission::McpClientsManage)
            .map_err(|_| McpIdentityError::PermissionDenied)?;
        if crate::IdempotencyKey::parse(key.clone()).is_err() || now > i64::MAX as u64 {
            return Err(McpIdentityError::InvalidInput);
        }
        if let McpIdentityWrite::SetGrants { expected, .. }
        | McpIdentityWrite::Issue { expected, .. }
        | McpIdentityWrite::Revoke { expected, .. } = &command
            && (*expected == 0 || *expected > i64::MAX as u64)
        {
            return Err(McpIdentityError::InvalidInput);
        }
        match &command {
            McpIdentityWrite::Create { name, grants } => {
                if name.trim().is_empty()
                    || name.chars().count() > 128
                    || name.chars().any(char::is_control)
                {
                    return Err(McpIdentityError::InvalidInput);
                }
                validate_mcp_grants(grants)?;
            }
            McpIdentityWrite::SetGrants { grants, .. } => validate_mcp_grants(grants)?,
            McpIdentityWrite::Issue { .. } if access.principal() != &PrincipalId::local_owner() => {
                return Err(McpIdentityError::PermissionDenied);
            }
            _ => {}
        }
        self.store
            .mcp_write(access.principal().clone(), key, command, now)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record(grants: Vec<McpGrant>) -> McpClient {
        let id = McpClientId::new();
        McpClient {
            client_id: id,
            principal_id: PrincipalId::parse(format!("mcp:{id}")).expect("Principal"),
            display_name: "Client".into(),
            revision: 1,
            created_at_ms: 1,
            revoked_at_ms: None,
            grants,
        }
    }
    #[test]
    fn zero_grants_never_inherit_owner_or_implicit_permissions() {
        let access = mcp_access(&record(vec![])).expect("authenticated but unprivileged");
        assert!(access.is_mcp_client());
        assert_ne!(access.principal(), &PrincipalId::local_owner());
        for permission in [
            Permission::ProjectsRead,
            Permission::ProjectsManage,
            Permission::McpClientsManage,
            Permission::StateCheck,
            Permission::SettingsRead,
        ] {
            assert!(access.require(permission).is_err());
        }
    }
    #[test]
    fn project_read_is_exact_and_never_expands_to_writes() {
        let project = ProjectId::new().to_string();
        let access = mcp_access(&record(vec![McpGrant {
            permission: "projects.read".into(),
            resource_kind: "project".into(),
            resource_id: project.clone(),
        }]))
        .expect("scoped identity");
        assert!(access.require_project_read_scope(&project).is_ok());
        assert!(
            access
                .require_project_read_scope(&ProjectId::new().to_string())
                .is_err()
        );
        assert!(access.require(Permission::ProjectsManage).is_err());
        assert!(access.require(Permission::PackagesManage).is_err());
    }
    #[test]
    fn unknown_global_owner_and_duplicate_grants_are_rejected() {
        for (permission, kind, id) in [
            ("projects.read", "global", "*"),
            ("mcp.clients.manage", "global", "*"),
            ("unknown", "global", "*"),
            ("projects.read", "project", "builtin:local-owner"),
        ] {
            assert_eq!(
                validate_mcp_grants(&[McpGrant {
                    permission: permission.into(),
                    resource_kind: kind.into(),
                    resource_id: id.into()
                }]),
                Err(McpIdentityError::InvalidInput)
            );
        }
        let g = McpGrant {
            permission: "operations.read".into(),
            resource_kind: "global".into(),
            resource_id: "*".into(),
        };
        assert_eq!(
            validate_mcp_grants(&[g.clone(), g]),
            Err(McpIdentityError::InvalidInput)
        );
    }
    #[test]
    fn revoked_or_forged_owner_records_fail_closed() {
        let mut client = record(vec![]);
        client.revoked_at_ms = Some(2);
        assert!(matches!(
            mcp_access(&client),
            Err(McpIdentityError::PermissionDenied)
        ));
        client.revoked_at_ms = None;
        client.principal_id = PrincipalId::local_owner();
        assert!(matches!(
            mcp_access(&client),
            Err(McpIdentityError::PermissionDenied)
        ));
    }

    #[test]
    fn package_write_is_an_exact_explicit_scope_not_implied_by_read_or_global_grants() {
        let id = ProjectId::new().to_string();
        let grant = |permission: &str, kind: &str, resource: &str| McpGrant {
            permission: permission.into(),
            resource_kind: kind.into(),
            resource_id: resource.into(),
        };
        let access = mcp_access(&record(vec![
            grant("projects.read", "project", &id),
            grant("packages.manage", "project", &id),
        ]))
        .expect("approved tuple");
        assert!(access.require_project_package_write_scope(&id).is_ok());
        assert!(
            access
                .require_project_package_write_scope(&ProjectId::new().to_string())
                .is_err()
        );
        assert!(validate_mcp_grants(&[grant("packages.manage", "global", "*")]).is_err());
        let read = mcp_access(&record(vec![grant("projects.read", "project", &id)])).expect("read");
        assert!(read.require_project_package_write_scope(&id).is_err());
    }
}
