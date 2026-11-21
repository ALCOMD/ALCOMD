//! Read-only classification of v3.4.0 quick-setup transport shapes.

use std::path::PathBuf;

use anyhow::{Context, Result, ensure};
use clap::{Args, ValueEnum};
use serde::{Deserialize, Serialize, de::IgnoredAny};

const ENTRY: &str = "alcomd3";
const TOKEN_VARIABLE: &str = "ALCOMD3_MCP_BEARER_TOKEN";
const MAX_CONFIG_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Args)]
pub(super) struct Arguments {
    /// An explicitly selected, isolated directory containing both fixture files.
    #[arg(long)]
    fixture_root: PathBuf,
    /// Relative path of the stopped v3 profile's gui-config.json.
    #[arg(long)]
    v3_gui_config: PathBuf,
    #[arg(long, value_enum)]
    client: Client,
    /// Relative path of the selected client's fixture configuration.
    #[arg(long)]
    client_config: PathBuf,
}

#[derive(Clone, Copy, Serialize, ValueEnum)]
#[serde(rename_all = "camelCase")]
enum Client {
    Codex,
    ClaudeCode,
    Cursor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
enum Status {
    Absent,
    ExactV3Transport,
    Unverified,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProfilePort {
    mcp_http_port: Option<u16>,
}

#[derive(Deserialize, Default)]
struct JsonServers {
    #[serde(default)]
    alcomd3: JsonCandidate,
}

#[derive(Deserialize)]
struct JsonConfig {
    #[serde(default, rename = "mcpServers")]
    servers: JsonServers,
}

#[derive(Deserialize, Default)]
#[serde(untagged)]
enum JsonCandidate {
    #[default]
    #[serde(skip)]
    Absent,
    Http(JsonHttp),
    Other(IgnoredAny),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonHttp {
    #[serde(rename = "type")]
    transport: String,
    url: String,
    headers: JsonHeaders,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonHeaders {
    #[serde(rename = "Authorization")]
    authorization: String,
}

pub(super) fn preview(args: Arguments) -> Result<serde_json::Value> {
    let (root, identity) = alcomd_platform::resolve_plain_storage_directory(&args.fixture_root)?;
    let config_name = match args.client {
        Client::Codex => "config.toml",
        Client::ClaudeCode => ".claude.json",
        Client::Cursor => "mcp.json",
    };
    ensure!(
        args.v3_gui_config
            .file_name()
            .is_some_and(|name| name == "gui-config.json")
            && args
                .client_config
                .file_name()
                .is_some_and(|name| name == config_name),
        "unexpected client or profile filename"
    );
    let port = super::profile::read_file(&root, &args.v3_gui_config, MAX_CONFIG_BYTES)?
        .map(|bytes| serde_json::from_slice::<ProfilePort>(without_bom(&bytes)))
        .transpose()
        .context("invalid legacy GUI config")?
        .and_then(|profile| profile.mcp_http_port)
        .filter(|port| *port != 0);
    let endpoint = port.map(|port| format!("http://127.0.0.1:{port}/mcp"));
    let status = match super::profile::read_file(&root, &args.client_config, MAX_CONFIG_BYTES)? {
        Some(contents) => classify(args.client, without_bom(&contents), endpoint.as_deref())?,
        None => Status::Absent,
    };
    let (_, current_identity) = alcomd_platform::resolve_plain_storage_directory(&root)?;
    ensure!(
        current_identity == identity,
        "fixture directory changed during inspection"
    );
    Ok(serde_json::json!({
        "client": args.client,
        "status": status,
        "ownershipVerified": false,
    }))
}

fn without_bom(contents: &[u8]) -> &[u8] {
    contents.strip_prefix(b"\xef\xbb\xbf").unwrap_or(contents)
}

fn classify(client: Client, contents: &[u8], endpoint: Option<&str>) -> Result<Status> {
    let source = std::str::from_utf8(contents).context("client config must be UTF-8")?;
    if source.trim().is_empty() {
        return Ok(Status::Absent);
    }
    let is_exact = match client {
        Client::Codex => {
            let document: toml::Table = toml::from_str(source).context("invalid Codex config")?;
            let Some(servers) = document.get("mcp_servers") else {
                return Ok(Status::Absent);
            };
            let servers = servers.as_table().context("invalid Codex MCP table")?;
            let Some(entry) = servers.get(ENTRY) else {
                return Ok(Status::Absent);
            };
            entry.as_table().is_some_and(|fields| {
                fields.len() == 2
                    && endpoint.is_some_and(|endpoint| {
                        fields.get("url").and_then(toml::Value::as_str) == Some(endpoint)
                    })
                    && fields
                        .get("bearer_token_env_var")
                        .and_then(toml::Value::as_str)
                        == Some(TOKEN_VARIABLE)
            })
        }
        Client::ClaudeCode | Client::Cursor => {
            let document: JsonConfig =
                serde_json::from_str(source).context("invalid MCP JSON config")?;
            let authorization = match client {
                Client::ClaudeCode => format!("Bearer {dollar}{{{TOKEN_VARIABLE}}}", dollar = '$'),
                Client::Cursor => format!("Bearer {dollar}{{env:{TOKEN_VARIABLE}}}", dollar = '$'),
                Client::Codex => unreachable!(),
            };
            match document.servers.alcomd3 {
                JsonCandidate::Absent => return Ok(Status::Absent),
                JsonCandidate::Http(entry) => {
                    entry.transport == "http"
                        && endpoint == Some(entry.url.as_str())
                        && entry.headers.authorization == authorization
                }
                JsonCandidate::Other(_) => false,
            }
        }
    };
    Ok(if is_exact {
        Status::ExactV3Transport
    } else {
        Status::Unverified
    })
}
