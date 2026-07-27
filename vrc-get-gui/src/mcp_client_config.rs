use alcomd3_mcp_protocol::MCP_HTTP_TOKEN_ENV;
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::io;
use std::path::PathBuf;

#[cfg(any(windows, test))]
use serde_json::{Map as JsonMap, Value as JsonValue};
#[cfg(any(windows, test))]
use std::str::FromStr;
#[cfg(windows)]
use std::{io::Write as _, path::Path};
#[cfg(windows)]
use tempfile::NamedTempFile;
#[cfg(any(windows, test))]
use toml_edit::{DocumentMut, Item, Table, value};

const CODEX_CONFIG_FILE_NAME: &str = "config.toml";
const CODEX_DEFAULT_HOME_DIR_NAME: &str = ".codex";
const CODEX_HOME_ENV: &str = "CODEX_HOME";
const CLAUDE_CODE_CONFIG_DIR_ENV: &str = "CLAUDE_CONFIG_DIR";
const CLAUDE_CODE_CONFIG_FILE_NAME: &str = ".claude.json";
const CURSOR_DEFAULT_HOME_DIR_NAME: &str = ".cursor";
const CURSOR_CONFIG_FILE_NAME: &str = "mcp.json";
const MCP_SERVER_NAME: &str = "alcomd3";
const AUTHORIZATION_HEADER_NAME: &str = "Authorization";
#[cfg(windows)]
const WINDOWS_USER_ENVIRONMENT_REGISTRY_KEY: &str = "Environment";

#[cfg(windows)]
static CLIENT_CONFIG_WRITE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum McpClient {
    Codex,
    ClaudeCode,
    Cursor,
}

impl McpClient {
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::ClaudeCode => "Claude Code",
            Self::Cursor => "Cursor",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum McpClientSetupStatus {
    Configured,
    AlreadyConfigured,
    RequiresConfirmation,
    #[allow(dead_code)]
    UnsupportedPlatform,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct McpClientSetupResult {
    pub status: McpClientSetupStatus,
    pub config_path: String,
    pub environment_variable: String,
    pub config_conflict: bool,
    pub environment_conflict: bool,
}

pub async fn configure_client(
    client: McpClient,
    endpoint: &str,
    token: &str,
    overwrite: bool,
) -> io::Result<McpClientSetupResult> {
    let config_path = client_config_path(client)?;

    #[cfg(not(windows))]
    {
        let _ = (endpoint, token, overwrite);
        return Ok(McpClientSetupResult {
            status: McpClientSetupStatus::UnsupportedPlatform,
            config_path: config_path.display().to_string(),
            environment_variable: MCP_HTTP_TOKEN_ENV.to_string(),
            config_conflict: false,
            environment_conflict: false,
        });
    }

    #[cfg(windows)]
    {
        let _guard = CLIENT_CONFIG_WRITE_LOCK.lock().await;
        let source = match tokio::fs::read_to_string(&config_path).await {
            Ok(source) => source,
            Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(error),
        };
        let analysis = analyze_client_config(client, &source, endpoint)?;
        let user_token = read_windows_user_environment_variable(MCP_HTTP_TOKEN_ENV)?;
        let environment_conflict = user_token.as_deref().is_some_and(|value| value != token);

        if !overwrite && (analysis.config_conflict || environment_conflict) {
            return Ok(McpClientSetupResult {
                status: McpClientSetupStatus::RequiresConfirmation,
                config_path: config_path.display().to_string(),
                environment_variable: MCP_HTTP_TOKEN_ENV.to_string(),
                config_conflict: analysis.config_conflict,
                environment_conflict,
            });
        }

        let environment_changed = user_token.as_deref() != Some(token);
        let config_changed = !analysis.already_configured;
        let updated_config = if config_changed {
            Some(update_client_config(client, &source, endpoint)?)
        } else {
            None
        };

        if environment_changed {
            write_windows_user_environment_variable(MCP_HTTP_TOKEN_ENV, token)?;
        }

        if let Some(updated) = updated_config {
            let path = config_path.clone();
            tokio::task::spawn_blocking(move || write_file_atomically(&path, updated.as_bytes()))
                .await
                .map_err(io::Error::other)??;
        }

        Ok(McpClientSetupResult {
            status: if config_changed || environment_changed {
                McpClientSetupStatus::Configured
            } else {
                McpClientSetupStatus::AlreadyConfigured
            },
            config_path: config_path.display().to_string(),
            environment_variable: MCP_HTTP_TOKEN_ENV.to_string(),
            config_conflict: false,
            environment_conflict: false,
        })
    }
}

pub const fn quick_setup_supported() -> bool {
    cfg!(windows)
}

fn client_config_path(client: McpClient) -> io::Result<PathBuf> {
    let user_home = dirs_next::home_dir();
    match client {
        McpClient::Codex => {
            codex_config_path_from_parts(std::env::var_os(CODEX_HOME_ENV), user_home)
        }
        McpClient::ClaudeCode => claude_code_config_path_from_parts(
            std::env::var_os(CLAUDE_CODE_CONFIG_DIR_ENV),
            user_home,
        ),
        McpClient::Cursor => cursor_config_path_from_home(user_home),
    }
}

fn codex_config_path_from_parts(
    codex_home: Option<OsString>,
    user_home: Option<PathBuf>,
) -> io::Result<PathBuf> {
    let codex_home = codex_home
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .or_else(|| user_home.map(|home| home.join(CODEX_DEFAULT_HOME_DIR_NAME)))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "user home is unavailable"))?;
    Ok(codex_home.join(CODEX_CONFIG_FILE_NAME))
}

fn claude_code_config_path_from_parts(
    config_dir: Option<OsString>,
    user_home: Option<PathBuf>,
) -> io::Result<PathBuf> {
    if let Some(config_dir) = config_dir.filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(config_dir).join(CLAUDE_CODE_CONFIG_FILE_NAME));
    }
    user_home
        .map(|home| home.join(CLAUDE_CODE_CONFIG_FILE_NAME))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "user home is unavailable"))
}

fn cursor_config_path_from_home(user_home: Option<PathBuf>) -> io::Result<PathBuf> {
    user_home
        .map(|home| {
            home.join(CURSOR_DEFAULT_HOME_DIR_NAME)
                .join(CURSOR_CONFIG_FILE_NAME)
        })
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "user home is unavailable"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(any(windows, test))]
struct ClientConfigAnalysis {
    already_configured: bool,
    config_conflict: bool,
}

#[cfg(any(windows, test))]
fn analyze_client_config(
    client: McpClient,
    source: &str,
    endpoint: &str,
) -> io::Result<ClientConfigAnalysis> {
    match client {
        McpClient::Codex => analyze_codex_config(source, endpoint),
        McpClient::ClaudeCode | McpClient::Cursor => {
            analyze_json_client_config(client, source, endpoint)
        }
    }
}

#[cfg(any(windows, test))]
fn update_client_config(client: McpClient, source: &str, endpoint: &str) -> io::Result<String> {
    match client {
        McpClient::Codex => update_codex_config(source, endpoint),
        McpClient::ClaudeCode | McpClient::Cursor => {
            update_json_client_config(client, source, endpoint)
        }
    }
}

#[cfg(any(windows, test))]
fn analyze_codex_config(source: &str, endpoint: &str) -> io::Result<ClientConfigAnalysis> {
    let document = parse_codex_config(source)?;
    let existing = document
        .get("mcp_servers")
        .and_then(Item::as_table_like)
        .and_then(|servers| servers.get(MCP_SERVER_NAME));

    let Some(existing) = existing else {
        return Ok(ClientConfigAnalysis {
            already_configured: false,
            config_conflict: false,
        });
    };

    let Some(table) = existing.as_table_like() else {
        return Ok(ClientConfigAnalysis {
            already_configured: false,
            config_conflict: true,
        });
    };
    let already_configured = table.get("url").and_then(Item::as_str) == Some(endpoint)
        && table.get("bearer_token_env_var").and_then(Item::as_str) == Some(MCP_HTTP_TOKEN_ENV)
        && table.get("command").is_none();

    Ok(ClientConfigAnalysis {
        already_configured,
        config_conflict: !already_configured,
    })
}

#[cfg(any(windows, test))]
fn update_codex_config(source: &str, endpoint: &str) -> io::Result<String> {
    let mut document = parse_codex_config(source)?;
    let servers = document
        .entry("mcp_servers")
        .or_insert_with(|| Item::Table(Table::new()))
        .as_table_mut()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Codex mcp_servers configuration is not a table",
            )
        })?;

    let mut alcomd3 = Table::new();
    alcomd3["url"] = value(endpoint);
    alcomd3["bearer_token_env_var"] = value(MCP_HTTP_TOKEN_ENV);
    servers[MCP_SERVER_NAME] = Item::Table(alcomd3);
    Ok(document.to_string())
}

#[cfg(any(windows, test))]
fn parse_codex_config(source: &str) -> io::Result<DocumentMut> {
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    if source.trim().is_empty() {
        return Ok(DocumentMut::new());
    }
    DocumentMut::from_str(source).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to parse Codex config.toml: {error}"),
        )
    })
}

#[cfg(any(windows, test))]
fn analyze_json_client_config(
    client: McpClient,
    source: &str,
    endpoint: &str,
) -> io::Result<ClientConfigAnalysis> {
    let document = parse_json_client_config(client, source)?;
    let existing = document
        .as_object()
        .and_then(|root| root.get("mcpServers"))
        .and_then(JsonValue::as_object)
        .and_then(|servers| servers.get(MCP_SERVER_NAME));

    let Some(existing) = existing else {
        return Ok(ClientConfigAnalysis {
            already_configured: false,
            config_conflict: false,
        });
    };
    let Some(server) = existing.as_object() else {
        return Ok(ClientConfigAnalysis {
            already_configured: false,
            config_conflict: true,
        });
    };
    let authorization = server
        .get("headers")
        .and_then(JsonValue::as_object)
        .and_then(|headers| headers.get(AUTHORIZATION_HEADER_NAME))
        .and_then(JsonValue::as_str);
    let already_configured = server.get("type").and_then(JsonValue::as_str) == Some("http")
        && server.get("url").and_then(JsonValue::as_str) == Some(endpoint)
        && authorization == Some(authorization_header_value(client).as_str())
        && !server.contains_key("command");

    Ok(ClientConfigAnalysis {
        already_configured,
        config_conflict: !already_configured,
    })
}

#[cfg(any(windows, test))]
fn update_json_client_config(
    client: McpClient,
    source: &str,
    endpoint: &str,
) -> io::Result<String> {
    let mut document = parse_json_client_config(client, source)?;
    let root = document.as_object_mut().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} configuration root is not an object",
                client.display_name()
            ),
        )
    })?;
    let servers = root
        .entry("mcpServers")
        .or_insert_with(|| JsonValue::Object(JsonMap::new()))
        .as_object_mut()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{} mcpServers configuration is not an object",
                    client.display_name()
                ),
            )
        })?;

    servers.insert(
        MCP_SERVER_NAME.to_string(),
        desired_json_server(client, endpoint),
    );
    let mut serialized = serde_json::to_string_pretty(&document).map_err(io::Error::other)?;
    serialized.push('\n');
    Ok(serialized)
}

#[cfg(any(windows, test))]
fn parse_json_client_config(client: McpClient, source: &str) -> io::Result<JsonValue> {
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    if source.trim().is_empty() {
        return Ok(JsonValue::Object(JsonMap::new()));
    }
    let document: JsonValue = serde_json::from_str(source).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "failed to parse {} configuration: {error}",
                client.display_name()
            ),
        )
    })?;
    if !document.is_object() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} configuration root is not an object",
                client.display_name()
            ),
        ));
    }
    Ok(document)
}

#[cfg(any(windows, test))]
fn desired_json_server(client: McpClient, endpoint: &str) -> JsonValue {
    let mut headers = JsonMap::new();
    headers.insert(
        AUTHORIZATION_HEADER_NAME.to_string(),
        JsonValue::String(authorization_header_value(client)),
    );

    let mut server = JsonMap::new();
    server.insert("type".to_string(), JsonValue::String("http".to_string()));
    server.insert("url".to_string(), JsonValue::String(endpoint.to_string()));
    server.insert("headers".to_string(), JsonValue::Object(headers));
    JsonValue::Object(server)
}

#[cfg(any(windows, test))]
fn authorization_header_value(client: McpClient) -> String {
    match client {
        McpClient::ClaudeCode => format!("Bearer ${{{MCP_HTTP_TOKEN_ENV}}}"),
        McpClient::Cursor => format!("Bearer ${{env:{MCP_HTTP_TOKEN_ENV}}}"),
        McpClient::Codex => unreachable!("Codex does not use a JSON authorization header"),
    }
}

#[cfg(windows)]
fn write_file_atomically(path: &Path, contents: &[u8]) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "client configuration path has no parent directory",
        )
    })?;
    std::fs::create_dir_all(parent)?;

    let mut temporary = NamedTempFile::new_in(parent)?;
    temporary.write_all(contents)?;
    temporary.flush()?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(windows)]
fn read_windows_user_environment_variable(name: &str) -> io::Result<Option<String>> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};

    let root = RegKey::predef(HKEY_CURRENT_USER);
    let key = match root.open_subkey_with_flags(WINDOWS_USER_ENVIRONMENT_REGISTRY_KEY, KEY_READ) {
        Ok(key) => key,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    match key.get_value(name) {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(windows)]
fn write_windows_user_environment_variable(name: &str, value: &str) -> io::Result<()> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let root = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = root.create_subkey(WINDOWS_USER_ENVIRONMENT_REGISTRY_KEY)?;
    key.set_value(name, &value)?;
    broadcast_windows_environment_change();
    Ok(())
}

#[cfg(windows)]
fn broadcast_windows_environment_change() {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
    };

    let environment = "Environment\0".encode_utf16().collect::<Vec<_>>();
    let result = unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            WPARAM(0),
            LPARAM(environment.as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            5_000,
            None,
        )
    };
    if result.0 == 0 {
        log::warn!("failed to broadcast the Windows user environment change");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENDPOINT: &str = "http://127.0.0.1:51739/mcp";

    #[test]
    fn codex_config_path_prefers_codex_home() {
        assert_eq!(
            codex_config_path_from_parts(
                Some(OsString::from("C:/custom-codex")),
                Some(PathBuf::from("C:/Users/example")),
            )
            .unwrap(),
            PathBuf::from("C:/custom-codex/config.toml")
        );
    }

    #[test]
    fn codex_config_path_defaults_under_user_home() {
        assert_eq!(
            codex_config_path_from_parts(None, Some(PathBuf::from("C:/Users/example"))).unwrap(),
            PathBuf::from("C:/Users/example/.codex/config.toml")
        );
    }

    #[test]
    fn claude_code_config_path_respects_config_dir() {
        assert_eq!(
            claude_code_config_path_from_parts(
                Some(OsString::from("C:/custom-claude")),
                Some(PathBuf::from("C:/Users/example")),
            )
            .unwrap(),
            PathBuf::from("C:/custom-claude/.claude.json")
        );
    }

    #[test]
    fn claude_code_config_path_defaults_under_user_home() {
        assert_eq!(
            claude_code_config_path_from_parts(None, Some(PathBuf::from("C:/Users/example")))
                .unwrap(),
            PathBuf::from("C:/Users/example/.claude.json")
        );
    }

    #[test]
    fn cursor_config_path_defaults_under_user_home() {
        assert_eq!(
            cursor_config_path_from_home(Some(PathBuf::from("C:/Users/example"))).unwrap(),
            PathBuf::from("C:/Users/example/.cursor/mcp.json")
        );
    }

    #[test]
    fn empty_codex_config_can_be_configured() {
        let updated = update_codex_config("", ENDPOINT).unwrap();
        let document = DocumentMut::from_str(&updated).unwrap();
        let server = document["mcp_servers"][MCP_SERVER_NAME].as_table().unwrap();
        assert_eq!(server["url"].as_str(), Some(ENDPOINT));
        assert_eq!(
            server["bearer_token_env_var"].as_str(),
            Some(MCP_HTTP_TOKEN_ENV)
        );
    }

    #[test]
    fn updating_codex_preserves_other_settings_and_servers() {
        let source = r#"# personal settings
model = "gpt-example"

[mcp_servers.other]
url = "https://example.com/mcp"

[mcp_servers.alcomd3]
command = "old-bridge"
"#;
        let updated = update_codex_config(source, ENDPOINT).unwrap();
        let document = DocumentMut::from_str(&updated).unwrap();
        assert_eq!(document["model"].as_str(), Some("gpt-example"));
        assert_eq!(
            document["mcp_servers"]["other"]["url"].as_str(),
            Some("https://example.com/mcp")
        );
        assert_eq!(
            document["mcp_servers"][MCP_SERVER_NAME]["url"].as_str(),
            Some(ENDPOINT)
        );
        assert!(
            document["mcp_servers"][MCP_SERVER_NAME]
                .get("command")
                .is_none()
        );
        assert!(updated.contains("# personal settings"));
    }

    #[test]
    fn matching_codex_config_is_idempotent() {
        let source = update_codex_config("", ENDPOINT).unwrap();
        assert_eq!(
            analyze_codex_config(&source, ENDPOINT).unwrap(),
            ClientConfigAnalysis {
                already_configured: true,
                config_conflict: false,
            }
        );
    }

    #[test]
    fn different_codex_config_requires_confirmation() {
        let source = r#"[mcp_servers.alcomd3]
url = "http://127.0.0.1:1234/mcp"
bearer_token_env_var = "OTHER_TOKEN"
"#;
        assert_eq!(
            analyze_codex_config(source, ENDPOINT).unwrap(),
            ClientConfigAnalysis {
                already_configured: false,
                config_conflict: true,
            }
        );
    }

    #[test]
    fn claude_code_config_uses_http_and_environment_expansion() {
        let updated = update_json_client_config(McpClient::ClaudeCode, "", ENDPOINT).unwrap();
        let document: JsonValue = serde_json::from_str(&updated).unwrap();
        let server = &document["mcpServers"][MCP_SERVER_NAME];
        assert_eq!(server["type"], "http");
        assert_eq!(server["url"], ENDPOINT);
        assert_eq!(
            server["headers"][AUTHORIZATION_HEADER_NAME],
            format!("Bearer ${{{MCP_HTTP_TOKEN_ENV}}}")
        );
    }

    #[test]
    fn cursor_config_uses_cursor_environment_expansion() {
        let updated = update_json_client_config(McpClient::Cursor, "", ENDPOINT).unwrap();
        let document: JsonValue = serde_json::from_str(&updated).unwrap();
        let server = &document["mcpServers"][MCP_SERVER_NAME];
        assert_eq!(server["type"], "http");
        assert_eq!(server["url"], ENDPOINT);
        assert_eq!(
            server["headers"][AUTHORIZATION_HEADER_NAME],
            format!("Bearer ${{env:{MCP_HTTP_TOKEN_ENV}}}")
        );
    }

    #[test]
    fn updating_json_config_preserves_other_settings_and_servers() {
        let source = r#"{
    "theme": "dark",
    "mcpServers": {
        "other": {
            "type": "http",
            "url": "https://example.com/mcp"
        },
        "alcomd3": {
            "command": "old-bridge"
        }
    }
}"#;
        let updated = update_json_client_config(McpClient::ClaudeCode, source, ENDPOINT).unwrap();
        let document: JsonValue = serde_json::from_str(&updated).unwrap();
        assert_eq!(document["theme"], "dark");
        assert_eq!(
            document["mcpServers"]["other"]["url"],
            "https://example.com/mcp"
        );
        assert_eq!(document["mcpServers"][MCP_SERVER_NAME]["url"], ENDPOINT);
        assert!(
            document["mcpServers"][MCP_SERVER_NAME]
                .get("command")
                .is_none()
        );
    }

    #[test]
    fn matching_json_config_is_idempotent() {
        for client in [McpClient::ClaudeCode, McpClient::Cursor] {
            let source = update_json_client_config(client, "", ENDPOINT).unwrap();
            assert_eq!(
                analyze_json_client_config(client, &source, ENDPOINT).unwrap(),
                ClientConfigAnalysis {
                    already_configured: true,
                    config_conflict: false,
                }
            );
        }
    }

    #[test]
    fn different_json_config_requires_confirmation() {
        let source = r#"{
    "mcpServers": {
        "alcomd3": {
            "type": "http",
            "url": "http://127.0.0.1:1234/mcp",
            "headers": {
                "Authorization": "Bearer other"
            }
        }
    }
}"#;
        assert_eq!(
            analyze_json_client_config(McpClient::Cursor, source, ENDPOINT).unwrap(),
            ClientConfigAnalysis {
                already_configured: false,
                config_conflict: true,
            }
        );
    }

    #[test]
    fn invalid_client_configuration_is_preserved_as_an_error() {
        assert_eq!(
            update_codex_config("[mcp_servers", ENDPOINT)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(
            update_json_client_config(McpClient::Cursor, "{", ENDPOINT)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[cfg(windows)]
    #[test]
    fn atomic_write_replaces_existing_client_config() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(CODEX_CONFIG_FILE_NAME);
        std::fs::write(&path, "old").unwrap();

        write_file_atomically(&path, b"new").unwrap();

        assert_eq!(std::fs::read_to_string(path).unwrap(), "new");
    }
}
