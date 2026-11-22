use serde::{Deserialize, Serialize};

pub const CAPABILITY_CLIENTS_REGISTRY_V1: &str = "clients.registry.v1";
pub const METHOD_CLIENTS_LIST: &str = "clients.list";
pub const METHOD_CLIENT_INSTANCES_LIST: &str = "clients.instances.list";
pub const METHOD_CLIENTS_DISCONNECT: &str = "clients.disconnect";

impl crate::RpcError {
    pub fn client_registry_stale() -> Self {
        Self::simple(
            "client_registry_stale",
            "The Core instance or client inventory cursor has expired.",
        )
    }
    pub fn client_registry_not_found() -> Self {
        Self::simple(
            "client_registry_not_found",
            "The selected client target is no longer available.",
        )
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientsListParams {
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
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientInstancesListParams {
    pub program_key: String,
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
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum ClientDisconnectTarget {
    Connection { id: String },
    Instance { id: String },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientsDisconnectParams {
    pub expected_core_instance_id: String,
    pub target: ClientDisconnectTarget,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientsDisconnectResult {
    pub disconnected_connection_ids: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClientProgramKind {
    Gui,
    Cli,
    Mcp,
    Unknown,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClientInstallationScope {
    User,
    Machine,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientInstallationSummary {
    pub scope: ClientInstallationScope,
    pub product_version: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientProgramSummary {
    pub program_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program_id: Option<String>,
    pub display_name: String,
    pub kind: ClientProgramKind,
    pub installations: Vec<ClientInstallationSummary>,
    pub active_instance_count: u32,
    pub active_connection_count: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientsListResult {
    pub core_instance_id: String,
    pub items: Vec<ClientProgramSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    pub coverage_incomplete: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientConnectionSummary {
    pub connection_id: String,
    pub connected_at_ms: u64,
    pub reported: crate::ClientInfo,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientInstanceSummary {
    pub instance_id: String,
    pub connections: Vec<ClientConnectionSummary>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientInstancesListResult {
    pub core_instance_id: String,
    pub items: Vec<ClientInstanceSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}
