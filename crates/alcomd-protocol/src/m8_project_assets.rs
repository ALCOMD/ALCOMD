use crate::{ProjectTemplateDependency, ProjectTemplateRecord};
use serde::{Deserialize, Serialize};

pub const CAPABILITY_PROJECT_INVENTORY_V1: &str = "packages.project-inventory.v1";
pub const CAPABILITY_TEMPLATE_EDIT_V1: &str = "project-templates.edit.v1";
pub fn mcp_environment_tool_capability(name: &str) -> Option<&'static str> {
    Some(match name {
        "alcomd_focus_project_unity"
        | "alcomd_open_project_directory"
        | "alcomd_open_unity_hub_download" => crate::CAPABILITY_DESKTOP_ACTIONS_V1,
        "alcomd_get_mcp_runtime_snapshot" => crate::CAPABILITY_MCP_RUNTIME_READ_V1,
        "alcomd_get_mcp_transport_snapshot" => crate::CAPABILITY_MCP_TRANSPORT_READ_V1,
        "alcomd_list_ai_clients"
        | "alcomd_request_client_change"
        | "alcomd_get_client_change_request"
        | "alcomd_list_client_change_requests" => crate::CAPABILITY_MCP_CLIENTS_V1,
        "alcomd_list_oauth_registrations" | "alcomd_get_oauth_request" => {
            crate::CAPABILITY_MCP_OAUTH_V1
        }
        "alcomd_list_clients" | "alcomd_list_client_instances" | "alcomd_disconnect_client" => {
            crate::CAPABILITY_CLIENTS_REGISTRY_V1
        }
        "alcomd_list_unity_installations" | "alcomd_get_project_launch_config" => {
            crate::CAPABILITY_UNITY_READ_V1
        }
        "alcomd_list_installed_packages" => CAPABILITY_PROJECT_INVENTORY_V1,
        "alcomd_get_project_template" | "alcomd_edit_project_template" => {
            CAPABILITY_TEMPLATE_EDIT_V1
        }
        "alcomd_create_project_template"
        | "alcomd_export_project_template"
        | "alcomd_remove_project_template" => crate::CAPABILITY_PROJECT_TEMPLATES_V1,
        _ => return None,
    })
}

/// Each tool requires one of the listed native capabilities.
pub fn mcp_project_catalog_capabilities(name: &str) -> Option<&'static [&'static str]> {
    use crate::*;
    Some(match name {
        "alcomd_inspect_project" => &[CAPABILITY_PROJECTS_READ_V1],
        "alcomd_register_project"
        | "alcomd_refresh_project"
        | "alcomd_set_project_favorite"
        | "alcomd_unregister_project" => &[CAPABILITY_PROJECTS_REGISTRY_V1],
        "alcomd_create_project" => &[CAPABILITY_PROJECTS_CREATE_V1],
        "alcomd_plan_project_copy" => &[CAPABILITY_PROJECTS_COPY_V1],
        "alcomd_plan_project_delete_directory" | "alcomd_apply_project_delete_directory" => {
            &[CAPABILITY_PROJECTS_DELETE_V1]
        }
        "alcomd_plan_project_unity_migration" | "alcomd_apply_project_unity_migration" => {
            &[CAPABILITY_PROJECTS_UNITY_MIGRATION_V1]
        }
        "alcomd_get_plan" => &[
            CAPABILITY_EXTENSIONS_LIFECYCLE_V1,
            CAPABILITY_PACKAGES_PLAN_V1,
            CAPABILITY_PROJECTS_COPY_V1,
            CAPABILITY_PROJECTS_DELETE_V1,
            CAPABILITY_PROJECTS_UNITY_MIGRATION_V1,
        ],
        "alcomd_query_project_candidates" => &[CAPABILITY_PACKAGES_CANDIDATES_V1],
        "alcomd_list_user_packages"
        | "alcomd_get_user_package"
        | "alcomd_enroll_user_package"
        | "alcomd_refresh_user_package"
        | "alcomd_remove_user_package" => &[CAPABILITY_PACKAGES_USER_PACKAGES_V1],
        _ => return None,
    })
}

pub const METHOD_PROJECT_INVENTORY: &str = "packages.projectInventory";
pub const METHOD_TEMPLATE_GET: &str = "projectTemplates.get";
pub const METHOD_TEMPLATE_EDIT: &str = "projectTemplates.edit";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectInventoryParams {
    pub project_id: String,
    pub expected_revision: u64,
    pub cursor: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstalledEvidence {
    Matched,
    Missing,
    VersionMismatch,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectInventoryEntry {
    pub package_id: String,
    pub declared_range: Option<String>,
    pub locked_version: Option<String>,
    pub observed_version: Option<String>,
    pub state: InstalledEvidence,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectResolutionEvidence {
    Needed,
    NoIssueObserved,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectInventoryResult {
    pub entries: Vec<ProjectInventoryEntry>,
    pub resolution: ProjectResolutionEvidence,
    pub reason: String,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectTemplateGetParams {
    pub id: String,
    pub cursor: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectTemplateGetResult {
    pub template: ProjectTemplateRecord,
    pub dependencies: Vec<ProjectTemplateDependency>,
    pub attachments: Vec<String>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectTemplateAttachment {
    pub source_file: String,
    pub target_path: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct ProjectTemplateChanges {
    pub name: Option<String>,
    pub set_dependencies: Vec<ProjectTemplateDependency>,
    pub remove_dependencies: Vec<String>,
    pub add_attachments: Vec<ProjectTemplateAttachment>,
    pub remove_attachments: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectTemplateEditParams {
    pub id: String,
    pub expected_revision: u64,
    pub changes: ProjectTemplateChanges,
    pub confirm: bool,
    pub idempotency_key: String,
}
