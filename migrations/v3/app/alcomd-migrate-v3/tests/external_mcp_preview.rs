use std::{fs, path::PathBuf, process::Command};

use serde_json::{Value, json};

const PORT: u16 = 48179;
const ENDPOINT: &str = "http://127.0.0.1:48179/mcp";
const TOKEN: &str = "synthetic-credential-never-printed";

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let parent = fs::canonicalize(std::env::temp_dir()).expect("fixture parent");
        let root = parent.join(format!("alcomd-mcp-preview-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).expect("isolated directory");
        let fixture = Self(root);
        fixture.write(
            "gui-config.json",
            &json!({
                "mcpHttpPort": PORT, "mcpHttpToken": TOKEN
            })
            .to_string(),
        );
        fixture
    }

    fn write(&self, path: &str, source: &str) {
        fs::write(self.0.join(path), source).expect("isolated config");
    }

    fn preview(&self, client: &str, config: &str) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_alcomd-migrate-v3"))
            .args(["preview-mcp-client", "--fixture-root"])
            .arg(&self.0)
            .args([
                "--v3-gui-config",
                "gui-config.json",
                "--client",
                client,
                "--client-config",
                config,
            ])
            .output()
            .expect("read isolated configuration")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn json_config(authorization: &str) -> Value {
    json!({
        "unrelatedSetting": TOKEN,
        "mcpServers": {
            "peer": {"token": TOKEN, "url": ENDPOINT},
            "alcomd3": { "type": "http", "url": ENDPOINT, "headers": { "Authorization": authorization } }
        }
    })
}

fn assert_preview(output: std::process::Output, client: &str, status: &str) {
    assert!(output.status.success(), "preview returned error");
    assert!(output.stderr.is_empty());
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).expect("stable report"),
        json!({ "client": client, "status": status, "ownershipVerified": false }),
    );
}

#[test]
fn all_three_formats_report_exact_transport_without_claiming_ownership_or_writing() {
    let fixture = Fixture::new();
    let codex = format!(
        r#"# preserve comment
[mcp_servers.peer]
token = "{TOKEN}"
[mcp_servers.alcomd3]
url = "{ENDPOINT}"
bearer_token_env_var = "ALCOMD3_MCP_BEARER_TOKEN"
"#
    );
    for (arg, report, path, source) in [
        ("codex", "codex", "config.toml", codex),
        (
            "claude-code",
            "claudeCode",
            ".claude.json",
            json_config("Bearer ${ALCOMD3_MCP_BEARER_TOKEN}").to_string(),
        ),
        (
            "cursor",
            "cursor",
            "mcp.json",
            json_config("Bearer ${env:ALCOMD3_MCP_BEARER_TOKEN}").to_string(),
        ),
    ] {
        fixture.write(path, &source);
        assert_preview(fixture.preview(arg, path), report, "exactV3Transport");
        assert_eq!(
            fs::read_to_string(fixture.0.join(path)).expect("unchanged fixture"),
            source
        );
    }
    assert_eq!(fs::read_dir(&fixture.0).expect("fixture items").count(), 4);
}

#[test]
fn preserves_manual_extra_field_unbound_and_unrelated_entries() {
    let fixture = Fixture::new();
    let mut source = json_config(&format!("Bearer {TOKEN}"));
    fixture.write(".claude.json", &source.to_string());
    assert_preview(
        fixture.preview("claude-code", ".claude.json"),
        "claudeCode",
        "unverified",
    );
    source["mcpServers"]["alcomd3"]["headers"]["Authorization"] =
        json!("Bearer ${ALCOMD3_MCP_BEARER_TOKEN}");
    source["mcpServers"]["alcomd3"]["command"] = json!("custom-client");
    fixture.write(".claude.json", &source.to_string());
    assert_preview(
        fixture.preview("claude-code", ".claude.json"),
        "claudeCode",
        "unverified",
    );

    fixture.write(
        "mcp.json",
        &json_config("Bearer ${env:ALCOMD3_MCP_BEARER_TOKEN}").to_string(),
    );
    fixture.write(
        "gui-config.json",
        &json!({"mcpHttpPort": PORT + 1}).to_string(),
    );
    assert_preview(
        fixture.preview("cursor", "mcp.json"),
        "cursor",
        "unverified",
    );
    fixture.write("gui-config.json", "{}");
    assert_preview(
        fixture.preview("cursor", "mcp.json"),
        "cursor",
        "unverified",
    );

    fixture.write(
        "mcp.json",
        &json!({"mcpServers": {"other": {"url": ENDPOINT}}}).to_string(),
    );
    assert_preview(fixture.preview("cursor", "mcp.json"), "cursor", "absent");
}

#[test]
fn refuses_escaping_paths_and_ambiguous_config_without_leaking_values() {
    let fixture = Fixture::new();
    let source = format!(r#"{{"mcpServers":{{"alcomd3":{{}},"alcomd3":{{"secret":"{TOKEN}"}}}}}}"#);
    fixture.write("mcp.json", &source);
    for output in [
        fixture.preview("cursor", "../mcp.json"),
        fixture.preview("cursor", "mcp.json"),
    ] {
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).expect("safe error bytes");
        assert!(!stderr.contains(TOKEN));
        assert!(!stderr.contains("mcp.json"));
    }
    assert_eq!(
        fs::read_to_string(fixture.0.join("mcp.json")).expect("preserved invalid fixture"),
        source
    );
}
