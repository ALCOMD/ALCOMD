use crate::{PrincipalId, ProjectId, ProjectType, Revision, ToolAuthentication};
use serde::{Deserialize, Serialize};
use std::future::Future;

tokio::task_local! {
    static WORKFLOW_AUTHENTICATION: ToolAuthentication;
    static WORKFLOW_PARENT_IDENTITY: Vec<u8>;
}

/// Binds a delegated plan to the parent checked by its RPC entry point.
pub async fn with_workflow_parent_identity<T>(
    expected_parent: Vec<u8>,
    work: impl Future<Output = T>,
) -> T {
    WORKFLOW_PARENT_IDENTITY.scope(expected_parent, work).await
}

pub(crate) fn workflow_parent_matches(identity: &[u8]) -> bool {
    WORKFLOW_PARENT_IDENTITY
        .try_with(|parent| parent == identity)
        .unwrap_or(false)
}

/// Carries the verified transport credential to the workflow acceptance transaction.
/// Spawned workers use the accepted immutable records and do not inherit this scope.
pub async fn with_workflow_authentication<T>(
    authentication: ToolAuthentication,
    work: impl Future<Output = T>,
) -> T {
    WORKFLOW_AUTHENTICATION.scope(authentication, work).await
}

pub fn workflow_authentication() -> Option<ToolAuthentication> {
    WORKFLOW_AUTHENTICATION.try_with(Clone::clone).ok()
}

/// Resource ownership and request ownership differ for delegated MCP operations.
/// Use only for an authenticated caller or a previously accepted operation.
pub fn workflow_resource_owner(caller: &PrincipalId) -> PrincipalId {
    if caller.as_str().starts_with("mcp:") {
        PrincipalId::local_owner()
    } else {
        caller.clone()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpRegisteredProject {
    pub project_id: ProjectId,
    pub revision: Revision,
    pub display_name: Option<String>,
    pub project_type: ProjectType,
    pub unity_version: String,
}
