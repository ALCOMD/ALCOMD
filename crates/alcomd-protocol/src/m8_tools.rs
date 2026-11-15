//! Extension tool catalogue and calls; no caller-supplied authority.
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub const CAPABILITY_MCP_EXTENSION_TOOLS_V1: &str = "mcp.extension-tools.v1";
pub const METHOD_MCP_TOOLS_CATALOG: &str = "mcp.tools.catalog";
pub const METHOD_EXTENSION_MCP_TOOLS_CALL: &str = "extensions.mcpTools.call";
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpToolsCatalogParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpToolsCatalogResult {
    pub tools: Vec<McpToolDescriptor>,
    pub next_cursor: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpToolDescriptor {
    pub name: String,
    pub title: String,
    pub description: String,
    pub input_schema: Value,
    pub output_schema: Value,
    pub effect: String,
    pub required_permissions: Vec<String>,
    pub origin: Option<McpToolOrigin>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpToolOrigin {
    pub extension_id: String,
    pub publisher_fingerprint: String,
    pub package_digest: String,
    pub version: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionMcpToolsCallParams {
    pub name: String,
    pub arguments: Value,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionMcpToolsCallResult {
    pub data: Value,
    pub core_effect: Option<Value>,
    pub replayed: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpToolPreview {
    pub name: String,
    pub title: String,
    pub description: String,
    pub required_permissions: Vec<String>,
    pub effect: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_catalogue_filters_are_omitted_and_authority_fields_are_rejected() {
        assert_eq!(
            serde_json::to_value(McpToolsCatalogParams::default()).expect("catalogue params"),
            serde_json::json!({})
        );
        assert!(
            serde_json::from_value::<ExtensionMcpToolsCallParams>(serde_json::json!({
                "name":"example", "arguments":{}, "principalId":"builtin:local-owner"
            }))
            .is_err()
        );
    }
}
