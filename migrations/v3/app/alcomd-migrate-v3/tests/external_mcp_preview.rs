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

    fn patch(&self, client: &str, config: &str, plan: Option<&str>) -> std::process::Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_alcomd-migrate-v3"));
        command
            .args(["patch-mcp-client", "--fixture-root"])
            .arg(&self.0)
            .args([
                "--v3-gui-config",
                "gui-config.json",
                "--client",
                client,
                "--client-config",
                config,
                "--before-quick-setup",
                &format!("before/{config}"),
                "--journal",
                "transaction",
            ]);
        if let Some(plan) = plan {
            command.args(["--apply-plan", plan]);
        }
        command.output().expect("run isolated patch")
    }

    fn rollback(&self, plan: &str) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_alcomd-migrate-v3"))
            .args(["rollback-mcp-client", "--fixture-root"])
            .arg(&self.0)
            .args(["--journal", "transaction", "--plan-digest", plan])
            .output()
            .expect("restore isolated config")
    }

    fn read(&self, path: &str) -> String {
        fs::read_to_string(self.0.join(path)).expect("fixture config")
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

fn receipt(output: std::process::Output) -> Value {
    assert!(
        output.status.success(),
        "patch failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains(TOKEN));
    serde_json::from_slice(&output.stdout).expect("patch receipt")
}

#[test]
fn paired_setup_evidence_removes_only_the_v3_entry_and_rolls_back_original_bytes() {
    let start = "[mcp_servers.peer]\r\nlabel = \"keep\"\r\n";
    let end = "# preserve this comment\r\n[mcp_servers.next]\r\nurl = \"http://localhost\"\r\n";
    let toml_entry = format!(
        "[mcp_servers.alcomd3]\r\nurl = \"{ENDPOINT}\"\r\nbearer_token_env_var = \"ALCOMD3_MCP_BEARER_TOKEN\"\r\n"
    );
    let json_peer = format!(r#""peer" : {{ "token" : "{TOKEN}", "trace": [4, 1] }}"#);
    let json_next = r#""next"  : { "command" : "leave alone" }"#;
    let claude = format!(
        "{{\n \"mcpServers\": {{ {json_peer}, \"alcomd3\": {} , {json_next} }}, \"other\" : 7\n}}",
        json_config("Bearer ${ALCOMD3_MCP_BEARER_TOKEN}")["mcpServers"]["alcomd3"]
    );
    let cursor = format!(
        "{{ \"mcpServers\": {{ \"alcomd3\" : {}, {json_peer}, {json_next} }}, \"other\" : 7 }}",
        json_config("Bearer ${env:ALCOMD3_MCP_BEARER_TOKEN}")["mcpServers"]["alcomd3"]
    );
    for (client, config, before, current, exact) in [
        (
            "codex",
            "config.toml",
            format!("{start}{end}"),
            format!("{start}{toml_entry}{end}"),
            format!("{start}{end}"),
        ),
        (
            "claude-code",
            ".claude.json",
            format!("{{\"mcpServers\":{{ {json_peer},{json_next} }},\"other\":7}}"),
            claude,
            format!("{{\n \"mcpServers\": {{ {json_peer} , {json_next} }}, \"other\" : 7\n}}"),
        ),
        (
            "cursor",
            "mcp.json",
            format!("{{\"mcpServers\":{{ {json_peer},{json_next} }},\"other\":7}}"),
            cursor,
            format!("{{ \"mcpServers\": {{  {json_peer}, {json_next} }}, \"other\" : 7 }}"),
        ),
    ] {
        let fixture = Fixture::new();
        fs::create_dir(fixture.0.join("before")).expect("before-setup directory");
        fixture.write(&format!("before/{config}"), &before);
        fixture.write(config, &current);
        let review = receipt(fixture.patch(client, config, None));
        assert_eq!(review["beforeSetupSnapshotMatched"], true);
        assert_eq!(fixture.read(config), current);
        assert!(!fixture.0.join("transaction").exists());
        let plan = review["planDigest"].as_str().expect("approved digest");
        assert_eq!(
            receipt(fixture.patch(client, config, Some(plan)))["status"],
            "applied"
        );
        assert_eq!(fixture.read(config), exact);
        assert_eq!(
            receipt(fixture.patch(client, config, Some(plan)))["status"],
            "applied"
        );
        assert_eq!(receipt(fixture.rollback(plan))["status"], "rolledBack");
        assert_eq!(fixture.read(config), current);
        assert_eq!(receipt(fixture.rollback(plan))["status"], "rolledBack");
        assert_eq!(fixture.read(&format!("before/{config}")), before);
    }
}

#[test]
fn replays_interrupted_moves_and_preserves_conflicting_third_party_edits() {
    let fixture = Fixture::new();
    let path = "mcp.json";
    fs::create_dir(fixture.0.join("before")).expect("before-setup directory");
    let mut before = json_config("Bearer ${env:ALCOMD3_MCP_BEARER_TOKEN}");
    let current = before.to_string();
    before["mcpServers"]
        .as_object_mut()
        .expect("servers")
        .remove("alcomd3");
    fixture.write("before/mcp.json", &before.to_string());
    fixture.write(path, &current);
    let review = receipt(fixture.patch("cursor", path, None));
    let plan = review["planDigest"].as_str().expect("approved digest");
    assert_eq!(
        receipt(fixture.patch("cursor", path, Some(plan)))["status"],
        "applied"
    );
    let exact = fixture.read(path);
    let third_party = exact.replace("unrelatedSetting", "renamedByThirdParty");
    fixture.write(path, &third_party);
    assert!(!fixture.rollback(plan).status.success());
    assert_eq!(fixture.read(path), third_party);
    fixture.write(path, &exact);

    fs::rename(
        fixture.0.join(path),
        fixture.0.join("transaction/replacement.bin"),
    )
    .expect("interrupted rollback");
    assert_eq!(receipt(fixture.rollback(plan))["status"], "rolledBack");
    assert_eq!(fixture.read(path), current);
    fs::rename(
        fixture.0.join(path),
        fixture.0.join("transaction/original.bin"),
    )
    .expect("interrupted apply");
    assert_eq!(
        receipt(fixture.patch("cursor", path, Some(plan)))["status"],
        "applied"
    );
    assert_eq!(fixture.read(path), exact);
}

#[test]
fn rejects_a_v3_shaped_entry_when_before_setup_evidence_differs() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.0.join("before")).expect("before-setup directory");
    fixture.write(
        "before/mcp.json",
        r#"{"mcpServers":{"unrelatedPeer":{"command":"not-the-same"}}}"#,
    );
    let current = json_config("Bearer ${env:ALCOMD3_MCP_BEARER_TOKEN}").to_string();
    fixture.write("mcp.json", &current);
    assert!(!fixture.patch("cursor", "mcp.json", None).status.success());
    assert_eq!(fixture.read("mcp.json"), current);
    assert!(!fixture.0.join("transaction").exists());
}
