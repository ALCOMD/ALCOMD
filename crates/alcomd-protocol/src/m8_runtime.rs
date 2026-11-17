//! Approved bounded Core connection/call observation DTOs, independent of domain types.
use serde::{Deserialize, Serialize};

pub const CAPABILITY_MCP_RUNTIME_READ_V1: &str = "mcp.runtime.read.v1";
pub const METHOD_MCP_RUNTIME_SNAPSHOT: &str = "mcp.runtime.snapshot";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpRuntimeSnapshotParams {
    pub client_id: Option<String>,
    pub limit: Option<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpRuntimeConnectionRecord {
    pub connection_id: String,
    pub client_id: String,
    pub connected_at_ms: u64,
    pub active_call_count: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpRuntimeCallRecord {
    pub call_id: String,
    pub connection_id: String,
    pub client_id: String,
    pub method: McpObservedCoreMethod,
    pub started_at_ms: u64,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum McpObservedCoreMethod {
    #[serde(rename = "mcp.tools.catalog")]
    ToolCatalog,
    #[serde(rename = "extensions.mcpTools.call")]
    ExtensionTool,
    #[serde(rename = "system.status")]
    Status,
    #[serde(rename = "projects.get")]
    ProjectGet,
    #[serde(rename = "projects.list")]
    ProjectList,
    #[serde(rename = "state.check")]
    StateCheck,
    #[serde(rename = "operations.get")]
    OperationGet,
    #[serde(rename = "operations.list")]
    OperationList,
    #[serde(rename = "operations.cancel")]
    OperationCancel,
    #[serde(rename = "events.list")]
    EventList,
    #[serde(rename = "packages.planRemove")]
    PackagePlanRemove,
    #[serde(rename = "packages.applyPlan")]
    PackageApply,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpRuntimeRows<T> {
    pub items: Vec<T>,
    pub total_tracked: u32,
    pub truncated: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpRuntimeSnapshotResult {
    pub instance_id: String,
    pub observed_at_ms: u64,
    pub can_read_connections: bool,
    pub can_read_requests: bool,
    pub coverage_incomplete: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connections: Option<McpRuntimeRows<McpRuntimeConnectionRecord>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requests: Option<McpRuntimeRows<McpRuntimeCallRecord>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn runtime_query_rejects_unknown_fields_and_snapshot_omits_unavailable_groups() {
        assert!(
            serde_json::from_value::<McpRuntimeSnapshotParams>(
                serde_json::json!({"credential":"secret"})
            )
            .is_err()
        );
        let query: McpRuntimeSnapshotParams =
            serde_json::from_str("{}").expect("bounded observation DTO");
        assert!(query.client_id.is_none());
        let snapshot = McpRuntimeSnapshotResult {
            instance_id: "00000000-0000-4000-8000-000000000001".into(),
            observed_at_ms: 1,
            can_read_connections: true,
            can_read_requests: false,
            coverage_incomplete: false,
            connections: Some(McpRuntimeRows {
                items: vec![],
                total_tracked: 0,
                truncated: false,
            }),
            requests: None,
        };
        let encoded = serde_json::to_value(snapshot).expect("bounded observation DTO");
        assert!(encoded.get("requests").is_none());
        assert_eq!(encoded["canReadRequests"], false);
        assert!(
            serde_json::from_value::<McpObservedCoreMethod>(serde_json::json!(
                "arbitrary-user-method"
            ))
            .is_err()
        );
    }
}
