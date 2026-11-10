//! Approved repository-list exchange DTOs; no persisted workflow or State Schema.
use serde::{Deserialize, Serialize};

pub const METHOD_REPOSITORIES_INSPECT_LIST_FILE: &str = "repositories.inspectListFile";
pub const METHOD_REPOSITORIES_PREVIEW_LIST_EXPORT: &str = "repositories.previewListExport";
pub const METHOD_REPOSITORIES_EXPORT_LIST_FILE: &str = "repositories.exportListFile";
pub const CAPABILITY_REPOSITORIES_LIST_FILES_V1: &str = "repositories.list-files.v1";
pub const METHOD_REPOSITORIES_IMPORT_LIST_FILE: &str = "repositories.importListFile";
pub const METHOD_REPOSITORIES_CANCEL_LIST_REGISTRATION: &str =
    "repositories.cancelListRegistration";
pub const CAPABILITY_REPOSITORIES_LIST_IMPORT_V1: &str = "repositories.list-import.v1";

pub const REPOSITORY_CONFIGURATION_EXTENSION: &str = "alcomdrepositories";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryListFormat {
    Native,
    #[default]
    Legacy,
}

impl RepositoryListFormat {
    fn is_legacy(&self) -> bool {
        *self == Self::Legacy
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryConfiguration {
    pub display_name: Option<String>,
    pub priority: u64,
    pub enabled: bool,
    pub disabled_package_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryListImportParams {
    pub path: String,
    pub expected_fingerprint: String,
    pub idempotency_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overwrite_existing: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum RepositoryListImportItem {
    Added { line: u32 },
    Existing { line: u32 },
    Updated { line: u32 },
    Failed { line: u32, code: String },
    Unknown { line: u32, code: String },
    Unprocessed { line: u32 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryListImportResult {
    pub items: Vec<RepositoryListImportItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryListCancelParams {
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryListCancelResult {
    pub stopped: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryListInspectParams {
    pub path: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryListPreviewParams {
    #[serde(default, skip_serializing_if = "RepositoryListFormat::is_legacy")]
    pub format: RepositoryListFormat,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryListExportParams {
    #[serde(default, skip_serializing_if = "RepositoryListFormat::is_legacy")]
    pub format: RepositoryListFormat,
    pub target_path: String,
    pub expected_fingerprint: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryListEntryStatus {
    Candidate,
    Duplicate,
    Invalid,
    Unsupported,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum RepositoryListSource {
    Remote { url: String },
    Local { path: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryListEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configuration: Option<RepositoryConfiguration>,
    pub line: u32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub credentials_configured: bool,
    pub status: RepositoryListEntryStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<RepositoryListSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryListInspection {
    #[serde(default, skip_serializing_if = "RepositoryListFormat::is_legacy")]
    pub format: RepositoryListFormat,
    pub entries: Vec<RepositoryListEntry>,
    pub ignored_lines: u32,
    pub fingerprint: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryListExclusionReason {
    LocalSource,
    AuthenticationNotSupported,
    UnsupportedSource,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryListExclusion {
    pub repository_id: String,
    pub reason: RepositoryListExclusionReason,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryListPreview {
    pub exportable_count: u32,
    pub excluded: Vec<RepositoryListExclusion>,
    pub byte_count: u32,
    pub fingerprint: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryListExportResult {
    pub exported_count: u32,
    pub byte_count: u32,
    pub sha256: String,
}
