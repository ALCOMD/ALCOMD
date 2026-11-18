use crate::{ProjectTemplateRef, ProjectType};
use serde::{Deserialize, Serialize};

pub const CAPABILITY_MCP_PROJECT_WORKFLOWS_V1: &str = "mcp.project-workflows.v1";
pub const METHOD_MCP_PROJECT_PATH_APPROVE: &str = "mcp.projectPaths.approve";
pub const METHOD_MCP_PROJECTS_CREATE: &str = "mcp.projects.create";
pub const METHOD_MCP_PROJECTS_REGISTER: &str = "mcp.projects.register";
pub const METHOD_MCP_PROJECTS_PLAN_COPY: &str = "mcp.projects.planCopy";
pub const METHOD_MCP_PROJECTS_APPLY_COPY: &str = "mcp.projects.applyCopy";
pub const METHOD_MCP_BACKUPS_PLAN_RESTORE: &str = "mcp.backups.planRestore";
pub const METHOD_MCP_BACKUPS_APPLY_RESTORE: &str = "mcp.backups.applyRestore";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpProjectPathPurpose {
    Register,
    Create,
    Copy,
    Restore,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpProjectPathApproveParams {
    pub client_id: String,
    pub purpose: McpProjectPathPurpose,
    pub path: String,
    pub confirm: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpProjectPathApproveResult {
    pub approval_id: String,
    pub expires_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "lowercase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum McpProjectTarget {
    Default { name: String },
    Approved { approval_id: String },
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
    Approved { approval_id: String },
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path_approval_id: Option<String>,
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
pub struct McpBackupPlanRestoreParams {
    pub backup_id: String,
    pub target: McpProjectTarget,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpBackupApplyRestoreParams {
    pub plan_id: String,
    pub confirm: bool,
    pub idempotency_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path_approval_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpBackupRestorePlan {
    pub plan_id: String,
    pub project_id: String,
    pub backup_id: String,
    pub target_name: String,
    pub archive_sha256: String,
    pub packages_require_resolve: bool,
    pub excluded_packages: Vec<crate::BackupRestoreExcludedPackage>,
    pub plan_fingerprint: String,
}
