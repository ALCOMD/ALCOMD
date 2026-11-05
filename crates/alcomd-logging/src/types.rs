use serde::{Deserialize, Serialize};

pub const RECORD_BYTES: usize = 512 * 1024;
pub const QUEUE_RECORDS: usize = 64;
pub const FILE_BYTES: u64 = 5 * 1024 * 1024;
pub const FILE_COUNT: usize = 5;
pub const SCAN_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub levels: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogRecord {
    pub source_id: String,
    pub session_id: String,
    pub segment_id: String,
    pub offset: u64,
    pub record: serde_json::Value,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogFailure {
    pub kind: String,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogHealth {
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
    pub last_failure: Option<LogFailure>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadIssue {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segment_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadPage {
    pub items: Vec<LogRecord>,
    pub next_cursor: String,
    pub baseline: bool,
    pub health: LogHealth,
    pub issues: Vec<ReadIssue>,
    pub has_more: bool,
    pub scanned_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistoryPage {
    pub items: Vec<LogRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    pub health: LogHealth,
    pub issues: Vec<ReadIssue>,
    pub has_more: bool,
    pub scanned_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogError {
    InvalidInput,
    CursorExpired,
    Unavailable,
}

impl std::fmt::Display for LogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidInput => "invalid log read parameters",
            Self::CursorExpired => "the requested log position is no longer retained",
            Self::Unavailable => "the requested logs are unavailable",
        })
    }
}
impl std::error::Error for LogError {}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShutdownReport {
    pub drained: bool,
    pub flushed: bool,
    pub timed_out: bool,
    pub health: LogHealth,
}
