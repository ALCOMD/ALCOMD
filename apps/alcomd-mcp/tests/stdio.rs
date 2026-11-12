use std::process::Stdio;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::time::timeout;

const DEADLINE: Duration = Duration::from_secs(10);

struct Adapter {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}

impl Adapter {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_alcomd-mcp"))
            .arg("stdio")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("start independent adapter");
        Self {
            input: child.stdin.take().expect("stdin"),
            output: BufReader::new(child.stdout.take().expect("stdout")),
            child,
        }
    }

    async fn send(&mut self, request: Value) -> Value {
        let mut bytes = serde_json::to_vec(&request).expect("request JSON");
        bytes.push(b'\n');
        self.raw(&bytes).await
    }

    async fn raw(&mut self, bytes: &[u8]) -> Value {
        timeout(DEADLINE, self.input.write_all(bytes))
            .await
            .expect("bounded write")
            .expect("write request");
        self.read().await
    }

    async fn read(&mut self) -> Value {
        let mut response = String::new();
        let count = timeout(DEADLINE, self.output.read_line(&mut response))
            .await
            .expect("bounded response")
            .expect("read response");
        assert!(count > 0, "adapter closed before responding");
        serde_json::from_str(&response).expect("stdout contains only NDJSON protocol")
    }

    async fn finish(mut self) {
        drop(self.input);
        let status = timeout(DEADLINE, self.child.wait())
            .await
            .expect("exit on EOF")
            .expect("child status");
        assert!(status.success());
        let mut trailing = String::new();
        assert_eq!(self.output.read_line(&mut trailing).await.expect("EOF"), 0);
    }
}

fn request(id: u32, method: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": {"_meta": {
            "io.modelcontextprotocol/protocolVersion": "2026-07-28",
            "io.modelcontextprotocol/clientCapabilities": {}
        }}
    })
}

#[tokio::test]
async fn discover_is_complete_private_and_advertises_only_implemented_capabilities() {
    let mut adapter = Adapter::start();
    let response = adapter.send(request(1, "server/discover")).await;
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 1);
    assert_eq!(response["result"]["resultType"], "complete");
    assert_eq!(
        response["result"]["supportedVersions"],
        json!(["2026-07-28"])
    );
    assert_eq!(response["result"]["capabilities"], json!({}));
    assert_eq!(response["result"]["ttlMs"], 0);
    assert_eq!(response["result"]["cacheScope"], "private");
    let info = &response["result"]["_meta"]["io.modelcontextprotocol/serverInfo"];
    assert_eq!(info["name"], env!("CARGO_PKG_NAME"));
    assert_eq!(info["version"], env!("CARGO_PKG_VERSION"));
    adapter.finish().await;
}

#[tokio::test]
async fn metadata_is_required_on_each_request_without_inherited_identity() {
    let mut adapter = Adapter::start();
    let mut first = request(1, "server/discover");
    first["params"]["_meta"]["io.modelcontextprotocol/clientInfo"] =
        json!({"name":"local-owner", "version":"1"});
    assert!(adapter.send(first).await.get("result").is_some());
    for invalid in [
        json!({"jsonrpc":"2.0", "id":2, "method":"server/discover"}),
        json!({"jsonrpc":"2.0", "id":3, "method":"server/discover", "params":{"_meta":{
            "io.modelcontextprotocol/protocolVersion":"2026-07-28"
        }}}),
        json!({"jsonrpc":"2.0", "id":4, "method":"server/discover", "params":{"_meta":{
            "io.modelcontextprotocol/protocolVersion":"2026-07-28",
            "io.modelcontextprotocol/clientCapabilities":[]
        }}}),
    ] {
        assert_eq!(adapter.send(invalid).await["error"]["code"], -32602);
    }
    assert!(
        adapter
            .send(request(5, "server/discover"))
            .await
            .get("result")
            .is_some()
    );
    adapter.finish().await;
}

#[tokio::test]
async fn unsupported_versions_do_not_negotiate_legacy_or_poison_the_next_request() {
    let mut adapter = Adapter::start();
    for version in ["2025-11-25", "2099-01-01"] {
        let mut invalid = request(1, "server/discover");
        invalid["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"] = json!(version);
        let response = adapter.send(invalid).await;
        assert_eq!(response["error"]["code"], -32022);
        assert_eq!(
            response["error"]["data"]["supported"],
            json!(["2026-07-28"])
        );
    }
    assert!(
        adapter
            .send(request(2, "server/discover"))
            .await
            .get("result")
            .is_some()
    );
    adapter.finish().await;
}

#[tokio::test]
async fn legacy_initialize_ping_and_unimplemented_methods_never_report_success() {
    let mut adapter = Adapter::start();
    let initialize = json!({"jsonrpc":"2.0", "id":1, "method":"initialize", "params":{
        "protocolVersion":"2025-11-25", "capabilities":{},
        "clientInfo":{"name":"local-owner", "version":"1"}
    }});
    assert_eq!(adapter.send(initialize).await["error"]["code"], -32601);
    for method in [
        "ping",
        "tools/list",
        "resources/list",
        "tasks/list",
        "unknown/method",
    ] {
        assert_eq!(
            adapter.send(request(2, method)).await["error"]["code"],
            -32601
        );
    }
    assert!(
        adapter
            .send(request(3, "server/discover"))
            .await
            .get("result")
            .is_some()
    );
    adapter.finish().await;
}

#[tokio::test]
async fn malformed_json_and_utf8_have_protocol_errors_and_allow_recovery() {
    let mut adapter = Adapter::start();
    for bytes in [b"{invalid}\n".as_slice(), b"\xff\n".as_slice()] {
        let response = adapter.raw(bytes).await;
        assert_eq!(response["error"]["code"], -32700);
        assert_eq!(response["id"], Value::Null);
    }
    assert!(
        adapter
            .send(request(1, "server/discover"))
            .await
            .get("result")
            .is_some()
    );
    adapter.finish().await;
}

#[tokio::test]
async fn oversized_line_is_rejected_and_following_frame_is_preserved() {
    let mut adapter = Adapter::start();
    let mut oversized = vec![b'x'; alcomd_protocol::MAX_FRAME_PAYLOAD_BYTES + 1];
    oversized.push(b'\n');
    assert_eq!(adapter.raw(&oversized).await["error"]["code"], -32600);
    assert!(
        adapter
            .send(request(1, "server/discover"))
            .await
            .get("result")
            .is_some()
    );
    adapter.finish().await;
}

#[tokio::test]
async fn largest_valid_frame_is_not_discarded_as_oversized() {
    let mut adapter = Adapter::start();
    let mut largest = request(1, "server/discover");
    largest["params"]["_meta"]["com.vrclearn.alcomd/test"] = json!("");
    let overhead = serde_json::to_vec(&largest).expect("JSON").len();
    largest["params"]["_meta"]["com.vrclearn.alcomd/test"] =
        json!("x".repeat(alcomd_protocol::MAX_FRAME_PAYLOAD_BYTES - overhead));
    assert_eq!(
        serde_json::to_vec(&largest).expect("JSON").len(),
        alcomd_protocol::MAX_FRAME_PAYLOAD_BYTES
    );
    assert!(adapter.send(largest).await.get("result").is_some());
    adapter.finish().await;
}

#[tokio::test]
async fn fragmented_input_survives_response_dispatch_and_interleaved_error_replies() {
    let mut adapter = Adapter::start();
    let second = serde_json::to_vec(&request(2, "server/discover")).expect("JSON");
    let midpoint = second.len() / 2;
    let mut first = serde_json::to_vec(&request(1, "server/discover")).expect("JSON");
    first.push(b'\n');
    first.extend_from_slice(&second[..midpoint]);
    assert_eq!(adapter.raw(&first).await["id"], 1);
    let mut remaining = second[midpoint..].to_vec();
    remaining.extend_from_slice(b"\n{invalid}\n");
    timeout(DEADLINE, adapter.input.write_all(&remaining))
        .await
        .expect("bounded write")
        .expect("write rest");
    let responses = [adapter.read().await, adapter.read().await];
    assert_eq!(
        responses
            .iter()
            .filter(|response| response["id"] == 2 && response.get("result").is_some())
            .count(),
        1
    );
    assert_eq!(
        responses
            .iter()
            .filter(|response| response["error"]["code"] == -32700)
            .count(),
        1
    );
    adapter.finish().await;
}

#[tokio::test]
async fn concurrent_success_and_error_writes_remain_complete_unique_frames() {
    let mut adapter = Adapter::start();
    let mut bytes = Vec::new();
    for id in 1..=64 {
        bytes.extend_from_slice(b"{invalid}\n");
        bytes
            .extend_from_slice(&serde_json::to_vec(&request(id, "server/discover")).expect("JSON"));
        bytes.push(b'\n');
    }
    let input = &mut adapter.input;
    let output = &mut adapter.output;
    let (write, read) = tokio::join!(
        timeout(DEADLINE, input.write_all(&bytes)),
        timeout(DEADLINE, async {
            let mut ids = std::collections::BTreeSet::new();
            let mut errors = 0;
            for _ in 0..128 {
                let mut line = String::new();
                assert!(output.read_line(&mut line).await.expect("response") > 0);
                let response: Value = serde_json::from_str(&line).expect("complete frame");
                if response.get("result").is_some() {
                    assert!(ids.insert(response["id"].as_u64().expect("request id")));
                } else {
                    assert_eq!(response["error"]["code"], -32700);
                    errors += 1;
                }
            }
            assert_eq!(ids, (1..=64).collect());
            assert_eq!(errors, 64);
        }),
    );
    write.expect("bounded write").expect("write frames");
    read.expect("bounded responses");
    adapter.finish().await;
}

#[tokio::test]
async fn well_formed_non_rpc_input_returns_invalid_request_without_echoing_contents() {
    let mut adapter = Adapter::start();
    for value in [
        json!([]),
        json!({"secret":"must-not-be-echoed"}),
        json!(null),
    ] {
        let response = adapter.send(value).await;
        assert_eq!(response["error"]["code"], -32600);
        assert!(!response.to_string().contains("must-not-be-echoed"));
    }
    assert!(
        adapter
            .send(request(1, "server/discover"))
            .await
            .get("result")
            .is_some()
    );
    adapter.finish().await;
}

#[tokio::test]
async fn eof_before_any_request_exits_cleanly_without_protocol_or_log_output() {
    let output = timeout(
        DEADLINE,
        Command::new(env!("CARGO_BIN_EXE_alcomd-mcp"))
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .expect("bounded EOF exit")
    .expect("start adapter");
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[tokio::test]
async fn unavailable_http_exits_with_error_and_never_reports_a_listener() {
    let output = timeout(
        DEADLINE,
        Command::new(env!("CARGO_BIN_EXE_alcomd-mcp"))
            .args(["serve", "--port", "0"])
            .kill_on_drop(true)
            .output(),
    )
    .await
    .expect("bounded HTTP exit")
    .expect("start adapter");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .expect("stderr UTF-8")
            .contains("no listener was started")
    );
}
