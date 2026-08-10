use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const MCP_PROTOCOL_VERSION: &str = "2026-07-28";
pub const MCP_HTTP_BIND_HOST: &str = "127.0.0.1";
pub const MCP_HTTP_DEFAULT_PORT: u16 = 51_739;
pub const MCP_HTTP_PATH: &str = "/mcp";
pub const MCP_HTTP_TOKEN_ENV: &str = "ALCOMD3_MCP_BEARER_TOKEN";
pub const MCP_HTTP_MIN_TOKEN_BYTES: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientIdentity {
    pub name: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListRepositoriesOutput {
    pub ok: bool,
    pub repositories: Vec<RepositorySummary>,
    pub package_visibility: PackageVisibility,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositorySummary {
    pub id: String,
    pub url: String,
    pub name: String,
    pub display_name: String,
    pub kind: RepositoryKind,
    pub hidden: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum RepositoryKind {
    OfficialDefault,
    CuratedDefault,
    User,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackageVisibility {
    pub hide_local_user_packages: bool,
    pub show_prerelease_packages: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_transport_constants_are_stable() {
        assert_eq!(MCP_PROTOCOL_VERSION, "2026-07-28");
        assert_eq!(MCP_HTTP_BIND_HOST, "127.0.0.1");
        assert_eq!(MCP_HTTP_PATH, "/mcp");
        assert_eq!(MCP_HTTP_TOKEN_ENV, "ALCOMD3_MCP_BEARER_TOKEN");
        assert_eq!(MCP_HTTP_MIN_TOKEN_BYTES, 32);
    }

    #[test]
    fn repository_list_output_uses_canonical_fields_only() {
        let serialized = serde_json::to_value(ListRepositoriesOutput {
            ok: true,
            repositories: vec![RepositorySummary {
                id: "com.example.repository".to_string(),
                url: "https://example.com/index.json".to_string(),
                name: "Example Repository".to_string(),
                display_name: "My Repository".to_string(),
                kind: RepositoryKind::User,
                hidden: true,
            }],
            package_visibility: PackageVisibility {
                hide_local_user_packages: false,
                show_prerelease_packages: true,
            },
        })
        .unwrap();

        assert_eq!(serialized["repositories"][0]["kind"], "user");
        assert_eq!(serialized["repositories"][0]["hidden"], true);
        assert_eq!(serialized["repositories"][0]["name"], "Example Repository");
        assert_eq!(
            serialized["repositories"][0]["displayName"],
            "My Repository"
        );
        assert!(serialized["repositories"][0].get("alias").is_none());
        assert_eq!(
            serialized["packageVisibility"]["showPrereleasePackages"],
            true
        );
    }
}
