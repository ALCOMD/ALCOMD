use crate::{ProjectTemplateRef, ProjectType};
use serde::{Deserialize, Serialize};

pub const CAPABILITY_MCP_PROJECT_WORKFLOWS_V2: &str = "mcp.project-workflows.v2";
pub const METHOD_MCP_PROJECTS_CREATE: &str = "mcp.projects.create";
pub const METHOD_MCP_PROJECTS_REGISTER: &str = "mcp.projects.register";
pub const METHOD_MCP_PROJECTS_PLAN_COPY: &str = "mcp.projects.planCopy";
pub const METHOD_MCP_PROJECTS_APPLY_COPY: &str = "mcp.projects.applyCopy";
pub const METHOD_MCP_BACKUPS_RESTORE: &str = "mcp.backups.restore";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "lowercase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum McpProjectTarget {
    Default { name: String },
    Absolute { path: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpProjectsCreateParams {
    pub target: McpProjectTarget,
    pub unity_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<ProjectTemplateRef>,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "lowercase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum McpProjectSource {
    Default { relative_path: String },
    Absolute { path: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpProjectsRegisterParams {
    pub source: McpProjectSource,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpRegisteredProject {
    pub project_id: String,
    pub revision: u64,
    pub display_name: Option<String>,
    pub project_type: ProjectType,
    pub unity_version: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpProjectsRegisterResult {
    pub project: McpRegisteredProject,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpProjectsPlanCopyParams {
    pub source_project_id: String,
    pub expected_revision: u64,
    pub target: McpProjectTarget,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpProjectsApplyCopyParams {
    pub plan_id: String,
    pub expected_revision: u64,
    pub confirm: bool,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpProjectsCopyPlan {
    pub plan_id: String,
    pub source_project_id: String,
    pub expected_revision: u64,
    pub target_name: String,
    pub target_project_id: String,
    pub writer_evidence: crate::ProjectCopyWriterEvidence,
    pub profile: crate::ProjectCopyProfile,
    pub expires_at_ms: u64,
    pub plan_fingerprint: String,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpBackupRestoreParams {
    pub archive_path: String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub project_name: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub destination_path: Option<String>,
    pub idempotency_key: String,
    pub confirm: bool,
}
