use crate::{ProjectTemplateRef, ProjectType};
use serde::{Deserialize, Serialize};

pub const CAPABILITY_MCP_PROJECT_WORKFLOWS_V1: &str = "mcp.project-workflows.v1";
pub const METHOD_MCP_PROJECTS_CREATE: &str = "mcp.projects.create";
pub const METHOD_MCP_PROJECTS_REGISTER: &str = "mcp.projects.register";
pub const METHOD_MCP_PROJECTS_PLAN_COPY: &str = "mcp.projects.planCopy";

pub fn mcp_workflows_tool_capability(name: &str) -> Option<&'static str> {
    Some(match name {
        "alcomd_register_project" | "alcomd_create_project" | "alcomd_plan_project_copy" => {
            crate::CAPABILITY_MCP_PROJECT_WORKFLOWS_V1
        }
        "alcomd_plan_unity_migration" | "alcomd_apply_unity_migration" => {
            crate::CAPABILITY_PROJECTS_UNITY_MIGRATION_V1
        }
        "alcomd_apply_project_copy" => crate::CAPABILITY_PROJECTS_COPY_V1,
        "alcomd_create_backup" => crate::CAPABILITY_BACKUPS_CREATE_V1,
        "alcomd_restore_backup" => crate::CAPABILITY_BACKUPS_RESTORE_V1,
        "alcomd_list_project_templates" => crate::CAPABILITY_PROJECT_TEMPLATES_V1,
        _ => return None,
    })
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
