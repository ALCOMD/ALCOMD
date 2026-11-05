//! Read-only access to owner-scoped Core business execution facts.

use serde::{Deserialize, Serialize};

pub const METHOD_LOGS_READ: &str = "logs.read";
pub const CAPABILITY_LOGS_READ_V1: &str = "logs.read.v1";

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogsReadParams {
    #[serde(
        default,
        deserialize_with = "crate::deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub before_sequence: Option<u64>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub after_sequence: Option<u64>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub limit: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogOutcome {
    Started,
    Succeeded,
    Failed,
    Cancelled,
    Waiting,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogTarget {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationLogEntry {
    pub sequence: u64,
    pub entry_id: String,
    pub execution_id: String,
    pub actor: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    pub occurred_at_ms: u64,
    pub action: String,
    pub step: String,
    pub outcome: LogOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<LogTarget>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogsReadResult {
    pub entries: Vec<OperationLogEntry>,
    pub high_watermark: u64,
    pub has_more: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_sequence: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_keeps_source_path_cause_and_actor_and_denies_an_arbitrary_path_request() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../specs/rpc/m7-operation-logs.contract-vectors.json"
        ))
        .expect("operation log contract vectors");
        let params: LogsReadParams =
            serde_json::from_value(fixture["params"].clone()).expect("read parameters");
        assert_eq!(
            serde_json::to_value(params).expect("encode parameters"),
            fixture["params"]
        );
        let result: LogsReadResult =
            serde_json::from_value(fixture["result"].clone()).expect("decode read result");
        assert_eq!(
            serde_json::to_value(result).expect("encode read result"),
            fixture["result"]
        );
        assert!(
            serde_json::from_value::<LogsReadParams>(serde_json::json!({"path": "C:/private"}))
                .is_err()
        );
        let mut no_actor = fixture["result"].clone();
        no_actor["entries"][0]
            .as_object_mut()
            .expect("entry object")
            .remove("actor");
        assert!(serde_json::from_value::<LogsReadResult>(no_actor).is_err());
    }
}
