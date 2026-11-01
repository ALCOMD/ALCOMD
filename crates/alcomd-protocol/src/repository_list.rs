//! Approved repository-list exchange DTOs; no persisted workflow or State Schema.
use serde::{Deserialize, Serialize};

pub const METHOD_REPOSITORIES_INSPECT_LIST_FILE: &str = "repositories.inspectListFile";
pub const METHOD_REPOSITORIES_PREVIEW_LIST_EXPORT: &str = "repositories.previewListExport";
pub const METHOD_REPOSITORIES_EXPORT_LIST_FILE: &str = "repositories.exportListFile";
pub const CAPABILITY_REPOSITORIES_LIST_FILES_V1: &str = "repositories.list-files.v1";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryListInspectParams {
    pub path: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryListPreviewParams {}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryListExportParams {
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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryListEntry {
    pub line: u32,
    pub status: RepositoryListEntryStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<RepositoryListSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryListInspection {
    pub entries: Vec<RepositoryListEntry>,
    pub ignored_lines: u32,
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
