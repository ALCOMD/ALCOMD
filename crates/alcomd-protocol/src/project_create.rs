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
pub struct ProjectTemplateDeclaration {
    pub id: String,
    pub name: String,
    pub unity: ProjectTemplateUnity,
    pub dependencies: Vec<ProjectTemplateDependency>,
    pub constraints: Vec<ProjectTemplateConstraint>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectTemplateUnity {
    pub series: Vec<String>,
    pub presets: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectTemplateDependency {
    pub package_id: String,
    pub version_range: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectTemplateConstraint {
    pub unity_version: String,
    pub package_id: String,
    pub version_range: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectTemplateRecord {
    pub declaration: ProjectTemplateDeclaration,
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
    pub declaration: ProjectTemplateDeclaration,
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
    pub template: ProjectTemplateRecord,
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
