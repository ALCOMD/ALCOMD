use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectsCreateParams {
    pub name: String,
    pub target_parent: String,
    pub unity_version: String,
    pub idempotency_key: String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub template: Option<ProjectTemplateRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectsCreateResult {
    pub operation_id: String,
    pub project_id: String,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectTemplateRef {
    pub id: String,
    pub revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectTemplateDependency {
    pub package_id: String,
    pub version_range: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ProjectTemplateSource {
    File {
        source_file: String,
    },
    Project {
        project_id: String,
        expected_project_revision: u64,
        id: String,
        name: String,
        #[serde(default)]
        include_assets: bool,
        #[serde(default)]
        lock_package_versions: bool,
    },
    Manual {
        id: String,
        name: String,
        unity_version: String,
        dependencies: Vec<ProjectTemplateDependency>,
        #[serde(
            default,
            deserialize_with = "crate::deserialize_optional_non_null",
            skip_serializing_if = "Option::is_none"
        )]
        base_template: Option<ProjectTemplateRef>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectTemplateRecord {
    pub id: String,
    pub name: String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub editor_version: Option<String>,
    pub revision: u64,
    pub locked: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectTemplatesListParams {
    #[serde(
        default,
        deserialize_with = "crate::deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub cursor: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectTemplatesListResult {
    pub templates: Vec<ProjectTemplateRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectTemplatesSaveParams {
    pub source: ProjectTemplateSource,
    pub idempotency_key: String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub expected_revision: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectTemplatesSaveResult {
    pub operation_id: String,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectTemplatesExportParams {
    pub id: String,
    pub revision: u64,
    pub destination_file: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectTemplatesRemoveParams {
    pub id: String,
    pub expected_revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectTemplatesRemoveResult {
    pub id: String,
    pub revision: u64,
}
