use crate::{ProjectTemplateDependency, ProjectTemplateRecord};
use serde::{Deserialize, Serialize};

pub const CAPABILITY_PROJECT_INVENTORY_V1: &str = "packages.project-inventory.v1";
pub const CAPABILITY_TEMPLATE_EDIT_V1: &str = "project-templates.edit.v1";
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
