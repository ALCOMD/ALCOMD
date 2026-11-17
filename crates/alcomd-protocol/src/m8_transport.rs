//! Approved public transport observation DTOs; no application/domain objects.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum McpTransportKind {
    Stdio,
    Http,
    HttpsOauth,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum McpTransportConnectionStage {
    Accepted,
    Tls,
    Idle,
    Active,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum McpTransportRequestStage {
    Receiving,
    Executing,
    Streaming,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum McpTransportMethod {
    #[serde(rename = "server/discover")]
    Discover,
    #[serde(rename = "tools/list")]
    ListTools,
    #[serde(rename = "tools/call")]
    CallTool,
    #[serde(rename = "subscriptions/listen")]
    Listen,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpTransportConnectionInput {
    pub local_ref: String,
    pub stage: McpTransportConnectionStage,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpTransportRequestInput {
    pub local_ref: String,
    pub connection_ref: String,
    pub method: McpTransportMethod,
    pub stage: McpTransportRequestStage,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpTransportStreamInput {
    pub local_ref: String,
    pub request_ref: String,
    pub tools_list_changed: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpTransportReport {
    pub sequence: u64,
    pub transport: McpTransportKind,
    pub connections: Vec<McpTransportConnectionInput>,
    pub requests: Vec<McpTransportRequestInput>,
    pub subscription_streams: Vec<McpTransportStreamInput>,
    pub coverage_incomplete: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpTransportBinding {
    pub request_ref: String,
    pub ticket: String,
}
impl std::fmt::Debug for McpTransportBinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpTransportBinding")
            .field("request_ref", &self.request_ref)
            .field("ticket", &"[REDACTED]")
            .finish()
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpTransportReportResult {
    pub observer_id: String,
    pub request_bindings: Vec<McpTransportBinding>,
    pub coverage_incomplete: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpTransportBindParams {
    pub ticket: String,
}
impl std::fmt::Debug for McpTransportBindParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("McpTransportBindParams { ticket: [REDACTED] }")
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpTransportBindResult {
    pub bound: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpTransportQuery {
    pub client_id: Option<String>,
    pub limit: Option<u32>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum McpTransportAvailability {
    Available,
    Stale,
    Unavailable,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum McpTransportSource {
    #[serde(rename = "adapterReported")]
    AdapterReported,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpTransportObserverStatus {
    pub observer_id: String,
    pub transport: McpTransportKind,
    pub last_report_at_ms: u64,
    pub status: McpTransportAvailability,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpTransportConnectionRow {
    pub connection_id: String,
    pub observer_id: String,
    pub source: McpTransportSource,
    pub first_observed_at_ms: u64,
    pub stage: McpTransportConnectionStage,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpTransportRequestRow {
    pub request_id: String,
    pub connection_id: String,
    pub observer_id: String,
    pub source: McpTransportSource,
    pub first_observed_at_ms: u64,
    pub method: McpTransportMethod,
    pub stage: McpTransportRequestStage,
    pub client_id: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpTransportStreamRow {
    pub stream_id: String,
    pub request_id: String,
    pub observer_id: String,
    pub source: McpTransportSource,
    pub first_observed_at_ms: u64,
    pub tools_list_changed: bool,
    pub client_id: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpTransportRows<T> {
    pub items: Vec<T>,
    pub total_tracked: u32,
    pub truncated: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpTransportSnapshot {
    pub instance_id: String,
    pub observed_at_ms: u64,
    pub status: McpTransportAvailability,
    pub can_read_connections: bool,
    pub can_read_requests: bool,
    pub can_read_subscription_streams: bool,
    pub coverage_incomplete: bool,
    pub observers: Vec<McpTransportObserverStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connections: Option<McpTransportRows<McpTransportConnectionRow>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requests: Option<McpTransportRows<McpTransportRequestRow>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscription_streams: Option<McpTransportRows<McpTransportStreamRow>>,
}

pub const CAPABILITY_MCP_TRANSPORT_REPORT_V1: &str = "mcp.transport.report.v1";
pub const CAPABILITY_MCP_TRANSPORT_READ_V1: &str = "mcp.transport.read.v1";
pub const METHOD_MCP_TRANSPORT_REPORT: &str = "mcp.transport.report";
pub const METHOD_MCP_TRANSPORT_BIND_REQUEST: &str = "mcp.transport.bindRequest";
pub const METHOD_MCP_TRANSPORT_SNAPSHOT: &str = "mcp.transport.snapshot";

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reports_cannot_claim_identity_and_tickets_are_not_debugged() {
        let params = McpTransportBindParams {
            ticket: "test-only-sensitive-value".into(),
        };
        assert!(!format!("{params:?}").contains(&params.ticket));
        assert!(
            serde_json::from_value::<McpTransportBindParams>(
                serde_json::json!({"ticket":"x","clientId":"spoof"})
            )
            .is_err()
        );
        let mut value = serde_json::json!({"sequence":1,"transport":"http","connections":[],"requests":[],"subscriptionStreams":[],"coverageIncomplete":false});
        for forbidden in [
            "clientId",
            "principal",
            "coreConnectionId",
            "url",
            "headers",
            "params",
            "credential",
        ] {
            value[forbidden] = serde_json::json!("spoof");
            assert!(serde_json::from_value::<McpTransportReport>(value.clone()).is_err());
            value.as_object_mut().expect("object").remove(forbidden);
        }
        assert!(serde_json::from_value::<McpTransportReport>(value).is_ok());
    }
}
