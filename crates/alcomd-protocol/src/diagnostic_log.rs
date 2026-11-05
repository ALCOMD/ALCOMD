//! Local-owner technical records retain the standard tracing JSON body.
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const METHOD_DIAGNOSTICS_LOG_READ: &str = "diagnostics.log.read";
pub const METHOD_DIAGNOSTICS_LOG_HISTORY: &str = "diagnostics.log.history";
pub const METHOD_DIAGNOSTICS_LOG_HEALTH: &str = "diagnostics.log.health";
pub const CAPABILITY_DIAGNOSTICS_LOG_V1: &str = "diagnostics.log.v1";

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticLogQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub levels: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic_id: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticLogReadParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default)]
    pub follow: bool,
    #[serde(default)]
    pub query: DiagnosticLogQuery,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticLogHistoryParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default)]
    pub query: DiagnosticLogQuery,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticLogItem {
    pub source_id: String,
    pub session_id: String,
    pub segment_id: String,
    pub offset: u64,
    pub record: Value,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticLogFailure {
    pub kind: String,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticLogHealth {
    pub source_id: String,
    pub session_id: String,
    pub revision: u64,
    pub state: String,
    pub queue_dropped: u64,
    pub oversize_rejected: u64,
    pub write_failed: u64,
    pub rotation_failed: u64,
    pub flush_failed: u64,
    pub has_gap: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_failure: Option<DiagnosticLogFailure>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticLogReadIssue {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segment_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticLogReadResult {
    pub items: Vec<DiagnosticLogItem>,
    pub next_cursor: String,
    pub baseline: bool,
    pub health: DiagnosticLogHealth,
    pub issues: Vec<DiagnosticLogReadIssue>,
    pub has_more: bool,
    pub scanned_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticLogHistoryResult {
    pub items: Vec<DiagnosticLogItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    pub health: DiagnosticLogHealth,
    pub issues: Vec<DiagnosticLogReadIssue>,
    pub has_more: bool,
    pub scanned_bytes: u64,
}
