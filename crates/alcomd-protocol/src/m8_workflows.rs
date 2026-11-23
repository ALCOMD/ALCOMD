use serde::{Deserialize, Serialize};

pub fn mcp_workflows_tool_capability(name: &str) -> Option<&'static str> {
    Some(match name {
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
pub enum ProjectTarget {
    Default { name: String },
    Absolute { parent_path: String, name: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "lowercase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ProjectSource {
    Default { relative_path: String },
    Absolute { path: String },
}
