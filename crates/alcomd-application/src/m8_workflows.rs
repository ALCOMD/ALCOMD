use crate::{PrincipalId, ProjectId, ProjectType, Revision, ToolAuthentication};
use serde::{Deserialize, Serialize};
use std::future::Future;

tokio::task_local! {
    static WORKFLOW_AUTHENTICATION: ToolAuthentication;
    static WORKFLOW_PATH: (WorkflowPlanPath, Option<Vec<u8>>);
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum WorkflowPlanPath {
    Default,
    Approved { digest: String },
}

pub async fn with_workflow_plan_path<T>(
    path: WorkflowPlanPath,
    expected_parent: Option<Vec<u8>>,
    work: impl Future<Output = T>,
) -> T {
    WORKFLOW_PATH.scope((path, expected_parent), work).await
}

pub fn workflow_plan_path() -> Option<WorkflowPlanPath> {
    WORKFLOW_PATH.try_with(|(path, _)| path.clone()).ok()
}

pub(crate) fn workflow_parent_matches(identity: &[u8]) -> bool {
    WORKFLOW_PATH
        .try_with(|(_, parent)| parent.as_deref() == Some(identity))
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
