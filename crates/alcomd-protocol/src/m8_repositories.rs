use serde::{Deserialize, Serialize};

use crate::RegistryCursor;

pub const CAPABILITY_MCP_REPOSITORIES_V1: &str = "mcp.repositories.v1";
pub const METHOD_MCP_REPOSITORIES_LIST: &str = "mcp.repositories.list";
pub const METHOD_MCP_REPOSITORIES_INSPECT: &str = "mcp.repositories.inspect";
pub const METHOD_MCP_REPOSITORIES_REGISTER: &str = "mcp.repositories.register";
pub const METHOD_MCP_REPOSITORIES_REMOVE: &str = "mcp.repositories.remove";

pub fn mcp_repositories_tool_capability(name: &str) -> Option<&'static str> {
    Some(match name {
        "alcomd_list_repositories"
        | "alcomd_inspect_repository"
        | "alcomd_register_repository"
        | "alcomd_remove_repository" => crate::CAPABILITY_MCP_REPOSITORIES_V1,
        "alcomd_refresh_repository" => crate::CAPABILITY_REPOSITORIES_REGISTRY_V1,
        _ => return None,
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpRepositoriesListParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<RegistryCursor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpRepositoryInspectParams {
    pub url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpRepositoryInspectResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_id: Option<String>,
    pub display_name: Option<String>,
    pub version_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpRepositoryRegisterParams {
    pub preview_id: String,
    pub confirm: bool,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpRepositoryRemoveParams {
    pub repository_id: String,
    pub expected_revision: u64,
    pub confirm: bool,
    pub idempotency_key: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpRepositorySourceKind {
    Local,
    Remote,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpRepositoryEntry {
    pub repository_id: String,
    pub display_name: Option<String>,
    pub revision: u64,
    pub source_kind: McpRepositorySourceKind,
    pub builtin: bool,
    pub available: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpRepositoriesListResult {
    pub repositories: Vec<McpRepositoryEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<RegistryCursor>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpRepositoryWriteResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<McpRepositoryEntry>,
    pub repository_id: String,
    pub revision: u64,
    pub replayed: bool,
}
