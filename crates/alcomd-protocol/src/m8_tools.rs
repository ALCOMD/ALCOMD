//! Extension tool catalogue and calls; no caller-supplied authority.
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub const CAPABILITY_MCP_EXTENSION_TOOLS_V1: &str = "mcp.extension-tools.v1";
pub const CAPABILITY_MCP_TOOLS_NOTIFICATIONS_V1: &str = "mcp.tools.notifications.v1";
pub const METHOD_MCP_TOOLS_CATALOG: &str = "mcp.tools.catalog";
pub const METHOD_EXTENSION_MCP_TOOLS_CALL: &str = "extensions.mcpTools.call";
pub const CAPABILITY_EXTENSION_CLI_COMMANDS_V1: &str = "extensions.cli-commands.v1";
pub const METHOD_EXTENSIONS_CLI_COMMANDS_LIST: &str = "extensions.cliCommands.list";
pub const METHOD_EXTENSIONS_CLI_COMMANDS_INVOKE: &str = "extensions.cliCommands.invoke";
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionCliCommandsListParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extension_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionCliCommandsListResult {
    pub commands: Vec<McpToolDescriptor>,
    pub next_cursor: Option<String>,
    pub catalog_fingerprint: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionCliCommandsInvokeParams {
    pub extension_id: String,
    pub name: String,
    pub arguments: Value,
}

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
    pub catalog_fingerprint: String,
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

/// Native capabilities required by the named extension-management tools.
pub fn mcp_extension_management_capability(name: &str) -> Option<&'static str> {
    Some(match name {
        "alcomd_list_extensions"
        | "alcomd_get_extension"
        | "alcomd_plan_extension_install"
        | "alcomd_apply_extension_install"
        | "alcomd_enable_extension"
        | "alcomd_disable_extension"
        | "alcomd_plan_extension_uninstall"
        | "alcomd_apply_extension_uninstall" => crate::CAPABILITY_EXTENSIONS_LIFECYCLE_V1,
        "alcomd_list_extension_permissions" => crate::CAPABILITY_EXTENSIONS_PERMISSIONS_READ_V1,
        "alcomd_inspect_extension_source" | "alcomd_plan_extension_install_from_source" => {
            crate::CAPABILITY_EXTENSIONS_REMOTE_INSTALL_V1
        }
        "alcomd_grant_extension_permission" | "alcomd_revoke_extension_permission" => {
            crate::CAPABILITY_EXTENSIONS_PERMISSIONS_V1
        }
        _ => return None,
    })
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
