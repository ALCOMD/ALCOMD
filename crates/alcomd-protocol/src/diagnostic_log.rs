use serde::{Deserialize, Serialize};

pub const METHOD_DIAGNOSTICS_LOG_READ: &str = "diagnostics.log.read";
pub const CAPABILITY_DIAGNOSTICS_LOG_V1: &str = "diagnostics.log.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticLogLevel {
    Trace,
    Debug,
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticLogCursor {
    pub instance_id: String,
    pub sequence: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticLogReadParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<DiagnosticLogCursor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait_ms: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticLogItem {
    pub instance_id: String,
    pub sequence: u64,
    pub occurred_at_ms: u64,
    pub level: DiagnosticLogLevel,
    pub code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic_id: Option<String>,
    pub notify: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticLogReadResult {
    pub items: Vec<DiagnosticLogItem>,
    pub next_cursor: DiagnosticLogCursor,
    pub reset: bool,
    pub gap: bool,
}
