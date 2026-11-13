//! Real frozen SDK -> STDIO executable or loopback HTTP -> Core -> project transaction.
use alcomd_client::{AlcomdClient, ClientConfig};
use alcomd_platform::{DataConfig, IpcConfig};
use alcomd_protocol::*;
use rmcp::{
    ClientLifecycleMode, ClientServiceExt, RoleClient, ServiceError,
    model::{CallToolRequestParams, CallToolResponse, CallToolResult, ProtocolVersion},
    service::RunningService,
    transport::TokioChildProcess,
};
use serde_json::{Value, json};
use std::{path::Path, time::Duration};
type Sdk = RunningService<RoleClient, ()>;
static REAL_CORE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct HttpFixture {
    url: String,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<std::io::Result<()>>,
}
impl HttpFixture {
    async fn start() -> Self {
        let server =
            alcomd_mcp::HttpServer::bind(0, ClientConfig::default().without_daemon_start())
                .await
                .expect("owned HTTP listener");
        let url = format!(
            "http://{}/mcp",
            server.local_addr().expect("loopback address")
        );
        let (stop, stopped) = tokio::sync::oneshot::channel();
        Self {
            url,
            stop: Some(stop),
            task: tokio::spawn(server.run_until(async {
                let _ = stopped.await;
            })),
        }
    }
    async fn finish(mut self) {
        self.stop
            .take()
            .expect("HTTP stop sender")
            .send(())
            .expect("normal HTTP shutdown");
        tokio::time::timeout(Duration::from_secs(6), &mut self.task)
            .await
            .expect("HTTP shutdown deadline")
            .expect("HTTP join")
            .expect("HTTP stop");
    }
}
impl Drop for HttpFixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

// A test-only TCP fault injector: after durable acceptance, deliver only a
// genuine HTTP response prefix to the actual SDK, then close both directions.
struct HttpResponseFault {
    armed: std::sync::Arc<std::sync::atomic::AtomicBool>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for HttpResponseFault {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl HttpResponseFault {
    async fn start(secret: &str, target: &str) -> (Sdk, Self) {
        use rmcp::transport::{
            StreamableHttpClientTransport,
            streamable_http_client::StreamableHttpClientTransportConfig,
        };
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("owned response fault listener");
        let url = format!(
            "http://{}/mcp",
            listener.local_addr().expect("fault address")
        );
        let authority = target
            .strip_prefix("http://")
            .and_then(|v| v.strip_suffix("/mcp"))
            .expect("real HTTP authority")
            .to_owned();
        let upstream = authority.clone();
        let armed = Arc::new(AtomicBool::new(false));
        let fault = Arc::clone(&armed);
        let task = tokio::spawn(async move {
            let mut connections = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    accepted=listener.accept()=>{
                        let (client,_)=accepted.expect("fault client socket");
                        let upstream=upstream.clone();let fault=Arc::clone(&fault);
                        connections.spawn(async move {
                            let server=tokio::net::TcpStream::connect(upstream).await.expect("real server connection");
                            let (mut from_client,mut to_client)=client.into_split();
                            let (mut from_server,mut to_server)=server.into_split();
                            let outbound=tokio::io::copy(&mut from_client,&mut to_server);
                            let inbound=async {
                                let mut bytes=[0u8;8192];
                                loop {
                                    let count=from_server.read(&mut bytes).await?;
                                    if count==0 {return Ok::<(),std::io::Error>(());}
                                    if fault.swap(false,Ordering::SeqCst) {
                                        to_client.write_all(&bytes[..count.min(16)]).await?;
                                        to_client.shutdown().await?;
                                        return Ok(());
                                    }
                                    to_client.write_all(&bytes[..count]).await?;
                                }
                            };
                            tokio::select! { _=outbound=>{}, _=inbound=>{} }
                        });
                    },
                    _=connections.join_next(),if !connections.is_empty()=>{},
                }
            }
        });
        let headers = std::collections::HashMap::from([(
            hyper::header::HOST,
            authority.parse().expect("real host"),
        )]);
        let transport = StreamableHttpClientTransport::with_client(
            reqwest::Client::builder()
                .no_proxy()
                .build()
                .expect("fault SDK client"),
            StreamableHttpClientTransportConfig::with_uri(url)
                .auth_header(secret)
                .custom_headers(headers),
        );
        let sdk = ()
            .serve_with_lifecycle(
                transport,
                ClientLifecycleMode::Discover {
                    preferred_versions: vec![ProtocolVersion::V_2026_07_28],
                },
            )
            .await
            .expect("real SDK Discover through transparent fault connection");
        (sdk, Self { armed, task })
    }
}

async fn sdk(secret: &str, url: Option<&str>) -> Sdk {
    if let Some(url) = url {
        use rmcp::transport::{
            StreamableHttpClientTransport,
            streamable_http_client::StreamableHttpClientTransportConfig,
        };
        let transport = StreamableHttpClientTransport::with_client(
            reqwest::Client::builder()
                .no_proxy()
                .build()
                .expect("local HTTP client"),
            StreamableHttpClientTransportConfig::with_uri(url.to_owned()).auth_header(secret),
        );
        return ()
            .serve_with_lifecycle(
                transport,
                ClientLifecycleMode::Discover {
                    preferred_versions: vec![ProtocolVersion::V_2026_07_28],
                },
            )
            .await
            .expect("actual frozen SDK HTTP Discover");
    }
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_alcomd-mcp"));
    command
        .arg("stdio")
        .env("ALCOMD_MCP_CLIENT_CREDENTIAL", secret)
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let transport = TokioChildProcess::new(command).expect("owned MCP STDIO process");
    ().serve_with_lifecycle(
        transport,
        ClientLifecycleMode::Discover {
            preferred_versions: vec![ProtocolVersion::V_2026_07_28],
        },
    )
    .await
    .expect("real SDK Discover, no legacy fallback")
}
async fn stop(client: Sdk) {
    tokio::time::timeout(Duration::from_secs(10), client.cancel())
        .await
        .expect("SDK close deadline")
        .expect("SDK close");
}

// One real keep-alive TCP stream deliberately alternates identities and reuses
// JSON-RPC IDs. No adapter mock or pooled-backend assumption is involved.
async fn http_identity_and_admission(url: &str, a: &str, b: &str, a_project: &str) {
    use http_body_util::{BodyExt, Full};
    use hyper::{Request, body::Bytes};
    use hyper_util::rt::TokioIo;
    let authority = url
        .strip_prefix("http://")
        .and_then(|v| v.strip_suffix("/mcp"))
        .expect("local URL");
    let stream = tokio::net::TcpStream::connect(authority)
        .await
        .expect("owned TCP stream");
    let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
        .await
        .expect("HTTP/1 connection");
    let connection = tokio::spawn(connection);
    let request = json!({"jsonrpc":"2.0","id":7,"method":"tools/call","params":{
        "name":"alcomd_list_projects","arguments":{},"_meta":{
            "io.modelcontextprotocol/protocolVersion":"2026-07-28",
            "io.modelcontextprotocol/clientCapabilities":{}}}});
    for (token, allowed) in [(a, true), (b, false), (a, true)] {
        let request = Request::builder()
            .method("POST")
            .uri("/mcp")
            .header("Host", authority)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json,text/event-stream")
            .header("MCP-Protocol-Version", "2026-07-28")
            .header("Mcp-Method", "tools/call")
            .header("Mcp-Name", "alcomd_list_projects")
            .body(Full::new(Bytes::from(
                serde_json::to_vec(&request).expect("request JSON"),
            )))
            .expect("HTTP request");
        let response = sender
            .send_request(request)
            .await
            .expect("same TCP request");
        assert_eq!(response.status(), 200);
        assert!(!response.headers().contains_key("mcp-session-id"));
        assert_eq!(response.headers()["cache-control"], "no-store");
        let bytes = tokio::time::timeout(Duration::from_secs(10), response.into_body().collect())
            .await
            .expect("SSE deadline")
            .expect("complete response")
            .to_bytes();
        let text = std::str::from_utf8(&bytes).expect("UTF-8 response");
        assert!(!text.contains(a) && !text.contains(b));
        let value: Value = serde_json::from_str(text).unwrap_or_else(|_| {
            serde_json::from_str(
                text.lines()
                    .find_map(|v| v.strip_prefix("data:"))
                    .expect("SSE data")
                    .trim(),
            )
            .expect("SSE JSON")
        });
        assert_eq!(value["id"], 7);
        if allowed {
            assert_eq!(
                value["result"]["structuredContent"]["data"]["projects"][0]["project_id"],
                a_project
            );
        } else {
            assert_eq!(
                value["result"]["structuredContent"]["error"]["code"],
                "permission_denied"
            );
        }
    }
    drop(sender);
    connection
        .await
        .expect("HTTP connection join")
        .expect("normal TCP close");

    let client = reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("HTTP test client");
    for (body, method, name, version, status, code) in [
        (
            json!({"jsonrpc":"2.0","id":7,"method":"tools/list","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}}),
            "tools/call",
            None,
            "2026-07-28",
            400,
            -32020,
        ),
        (
            request.clone(),
            "tools/call",
            Some("alcomd_get_project"),
            "2026-07-28",
            400,
            -32020,
        ),
        (
            request.clone(),
            "tools/call",
            Some("alcomd_list_projects"),
            "2025-11-25",
            400,
            -32020,
        ),
        (
            json!({"jsonrpc":"2.0","id":7,"method":"tools/list","params":{}}),
            "tools/list",
            None,
            "2026-07-28",
            400,
            -32602,
        ),
        (
            json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":7}}),
            "notifications/cancelled",
            None,
            "2026-07-28",
            400,
            -32600,
        ),
        (json!([]), "tools/list", None, "2026-07-28", 400, -32600),
        (
            json!({"jsonrpc":"2.0","id":7,"result":{}}),
            "tools/list",
            None,
            "2026-07-28",
            400,
            -32600,
        ),
        (
            json!({"jsonrpc":"2.0","id":7,"method":"initialize","params":{}}),
            "initialize",
            None,
            "2026-07-28",
            404,
            -32601,
        ),
        (
            json!({"jsonrpc":"2.0","id":7,"method":"ping","params":{}}),
            "ping",
            None,
            "2026-07-28",
            404,
            -32601,
        ),
    ] {
        let mut request = client
            .post(url)
            .bearer_auth(a)
            .header("Accept", "application/json,text/event-stream")
            .header("MCP-Protocol-Version", version)
            .header("Mcp-Method", method)
            .json(&body);
        if let Some(name) = name {
            request = request.header("Mcp-Name", name);
        }
        let response = request.send().await.expect("HTTP negative response");
        assert_eq!(
            response.status().as_u16(),
            status,
            "boundary {method}/{version}"
        );
        let text = response.text().await.expect("bounded JSON error");
        assert!(!text.contains(a) && !text.contains(b));
        assert_eq!(
            serde_json::from_str::<Value>(&text).expect("JSON error")["error"]["code"],
            code
        );
    }
    let response = client
        .post(url)
        .bearer_auth("0".repeat(64))
        .header("Accept", "application/json,text/event-stream")
        .header("Content-Type", "application/json")
        .body("{}")
        .send()
        .await
        .expect("invalid credential response");
    assert_eq!(
        response.status(),
        401,
        "no owner fallback for unknown token"
    );
    // Reuse an ID concurrently across separate authenticated HTTP exchanges.
    let post = |token: &str| {
        client
            .post(url)
            .bearer_auth(token)
            .header("Accept", "application/json,text/event-stream")
            .header("MCP-Protocol-Version", "2026-07-28")
            .header("Mcp-Method", "tools/call")
            .header("Mcp-Name", "alcomd_list_projects")
            .json(&request)
            .send()
    };
    let (first, second) = tokio::join!(post(a), post(b));
    let first = first
        .expect("concurrent A")
        .text()
        .await
        .expect("A response");
    let second = second
        .expect("concurrent B")
        .text()
        .await
        .expect("B response");
    assert!(first.contains(a_project) && !first.contains("permission_denied"));
    assert!(second.contains("permission_denied") && !second.contains(a_project));
    assert!(!first.contains(a) && !second.contains(b));
    for (encoded, status, code) in [
        (
            "=?base64?YWxjb21kX2xpc3RfcHJvamVjdHM=?=".to_owned(),
            200,
            None,
        ),
        ("=?base64?%%%?=".to_owned(), 400, Some(-32020)),
        (
            format!("=?base64?{}eA==?=", "eHh4".repeat(1365)),
            400,
            Some(-32020),
        ),
    ] {
        let response = client
            .post(url)
            .bearer_auth(a)
            .header("Accept", "application/json,text/event-stream")
            .header("MCP-Protocol-Version", "2026-07-28")
            .header("Mcp-Method", "tools/call")
            .header("Mcp-Name", encoded)
            .json(&request)
            .send()
            .await
            .expect("Base64 HTTP header response");
        assert_eq!(response.status().as_u16(), status);
        let text = response.text().await.expect("header response body");
        assert!(!text.contains(a) && !text.contains(b) && !text.contains(&"x".repeat(100)));
        if let Some(code) = code {
            assert_eq!(
                serde_json::from_str::<Value>(&text).expect("bounded error")["error"]["code"],
                code
            );
        } else {
            assert!(text.contains(a_project));
        }
    }
    let mut unsupported = request.clone();
    unsupported["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"] = json!("2027-01-01");
    let response = client
        .post(url)
        .bearer_auth(a)
        .header("Accept", "application/json,text/event-stream")
        .header("MCP-Protocol-Version", "2027-01-01")
        .header("Mcp-Method", "tools/call")
        .header("Mcp-Name", "alcomd_list_projects")
        .json(&unsupported)
        .send()
        .await
        .expect("unsupported future version");
    assert_eq!(response.status(), 400);
    assert_eq!(
        response.json::<Value>().await.expect("protocol error")["error"]["code"],
        -32022
    );
    http_request_limits(url, a).await;
}

async fn http_request_limits(url: &str, secret: &str) {
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
    let authority = url
        .strip_prefix("http://")
        .and_then(|v| v.strip_suffix("/mcp"))
        .expect("local authority");
    let mut held = Vec::new();
    // 100 Continue is emitted only when the admitted request polls its body,
    // after its independent Core authentication and semaphore acquisition.
    for _ in 0..16 {
        let mut stream = BufReader::new(
            tokio::net::TcpStream::connect(authority)
                .await
                .expect("held upload"),
        );
        stream.get_mut().write_all(format!("POST /mcp HTTP/1.1\r\nHost: {authority}\r\nAuthorization: Bearer {secret}\r\nContent-Type: application/json\r\nAccept: application/json,text/event-stream\r\nContent-Length: 1\r\nExpect: 100-continue\r\n\r\n").as_bytes()).await.expect("held headers");
        let mut first = String::new();
        tokio::time::timeout(Duration::from_secs(5), stream.read_line(&mut first))
            .await
            .expect("admission deadline")
            .expect("100 response");
        assert_eq!(first, "HTTP/1.1 100 Continue\r\n");
        first.clear();
        stream.read_line(&mut first).await.expect("100 header end");
        assert_eq!(first, "\r\n");
        held.push(stream);
    }
    let client = reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("limit client");
    let response = client
        .post(url)
        .bearer_auth(secret)
        .header("Accept", "application/json,text/event-stream")
        .header("Content-Type", "application/json")
        .body("{}")
        .send()
        .await
        .expect("saturation response");
    assert_eq!(response.status(), 429);
    assert_eq!(response.headers()["retry-after"], "1");
    drop(held);
    // Await cleanup of our disconnected uploads without starting any writes.
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let response = client
                .post(url)
                .bearer_auth(secret)
                .header("Accept", "application/json,text/event-stream")
                .header("Content-Type", "application/json")
                .body("{}")
                .send()
                .await
                .expect("released request");
            if response.status() != 429 {
                assert_eq!(response.status(), 400);
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("disconnect releases request quota");

    let mut stream = BufReader::new(
        tokio::net::TcpStream::connect(authority)
            .await
            .expect("chunked upload"),
    );
    stream.get_mut().write_all(format!("POST /mcp HTTP/1.1\r\nHost: {authority}\r\nAuthorization: Bearer {secret}\r\nContent-Type: application/json\r\nAccept: application/json,text/event-stream\r\nTransfer-Encoding: chunked\r\nExpect: 100-continue\r\nConnection: close\r\n\r\n").as_bytes()).await.expect("chunk headers");
    let mut first = String::new();
    stream.read_line(&mut first).await.expect("chunk admission");
    assert_eq!(first, "HTTP/1.1 100 Continue\r\n");
    first.clear();
    stream.read_line(&mut first).await.expect("100 end");
    let mut chunk = b"10000\r\n".to_vec();
    chunk.extend(vec![b' '; 65536]);
    chunk.extend_from_slice(b"\r\n");
    for _ in 0..64 {
        stream
            .get_mut()
            .write_all(&chunk)
            .await
            .expect("within-limit chunk");
    }
    stream
        .get_mut()
        .write_all(b"1\r\nx\r\n0\r\n\r\n")
        .await
        .expect("one byte over limit");
    let mut response = String::new();
    tokio::time::timeout(Duration::from_secs(5), stream.read_to_string(&mut response))
        .await
        .expect("413 deadline")
        .expect("413 response");
    assert!(
        response.starts_with("HTTP/1.1 413"),
        "chunked cannot bypass cumulative limit"
    );
    assert!(!response.contains(secret));

    let exact = json!({"jsonrpc":"2.0","id":19,"method":"tools/list","params":{"_meta":{
        "io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}}).to_string();
    let mut exact = exact.into_bytes();
    exact.resize(4 * 1024 * 1024, b' ');
    let response = client
        .post(url)
        .bearer_auth(secret)
        .header("Accept", "application/json,text/event-stream")
        .header("Content-Type", "application/json")
        .header("MCP-Protocol-Version", "2026-07-28")
        .header("Mcp-Method", "tools/list")
        .body(exact)
        .send()
        .await
        .expect("exact maximum request");
    assert_eq!(response.status(), 200, "4 MiB valid JSON remains accepted");
    assert!(
        response
            .text()
            .await
            .expect("maximum request response")
            .contains("alcomd_list_projects")
    );

    let mut slow = BufReader::new(
        tokio::net::TcpStream::connect(authority)
            .await
            .expect("slow body connection"),
    );
    slow.get_mut().write_all(format!("POST /mcp HTTP/1.1\r\nHost: {authority}\r\nAuthorization: Bearer {secret}\r\nContent-Type: application/json\r\nAccept: application/json,text/event-stream\r\nContent-Length: 1\r\nExpect: 100-continue\r\nConnection: close\r\n\r\n").as_bytes()).await.expect("slow body headers");
    let mut first = String::new();
    slow.read_line(&mut first)
        .await
        .expect("admitted slow body");
    assert_eq!(first, "HTTP/1.1 100 Continue\r\n");
    first.clear();
    slow.read_line(&mut first).await.expect("100 end");
    let mut response = String::new();
    tokio::time::timeout(Duration::from_secs(17), slow.read_to_string(&mut response))
        .await
        .expect("15 second body deadline")
        .expect("408 response");
    assert!(response.starts_with("HTTP/1.1 408") && !response.contains(secret));
}

async fn http_hello_does_not_freeze_grants(
    url: &str,
    secret: &str,
    owner: &mut AlcomdClient,
    client: &McpClientRecord,
    args: &Value,
) -> McpClientRecord {
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
    let authority = url
        .strip_prefix("http://")
        .and_then(|v| v.strip_suffix("/mcp"))
        .expect("local authority");
    let body = json!({"jsonrpc":"2.0","id":81,"method":"tools/call","params":{
        "name":"alcomd_apply_package_changes","arguments":args,"_meta":{
            "io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}}).to_string();
    let mut stream = BufReader::new(
        tokio::net::TcpStream::connect(authority)
            .await
            .expect("grant race connection"),
    );
    stream.get_mut().write_all(format!("POST /mcp HTTP/1.1\r\nHost: {authority}\r\nAuthorization: Bearer {secret}\r\nContent-Type: application/json\r\nAccept: application/json,text/event-stream\r\nMCP-Protocol-Version: 2026-07-28\r\nMcp-Method: tools/call\r\nMcp-Name: alcomd_apply_package_changes\r\nContent-Length: {}\r\nExpect: 100-continue\r\nConnection: close\r\n\r\n",body.len()).as_bytes()).await.expect("headers");
    let mut first = String::new();
    tokio::time::timeout(Duration::from_secs(5), stream.read_line(&mut first))
        .await
        .expect("real authenticated hello barrier")
        .expect("100 Continue");
    assert_eq!(first, "HTTP/1.1 100 Continue\r\n");
    first.clear();
    stream.read_line(&mut first).await.expect("100 end");
    let reduced = client
        .grants
        .iter()
        .filter(|v| v.permission != "packages.manage")
        .cloned()
        .collect();
    let reduced = update(owner, client, reduced, "http-withdraw-after-hello").await;
    stream
        .get_mut()
        .write_all(body.as_bytes())
        .await
        .expect("Apply after withdrawal");
    let mut response = String::new();
    tokio::time::timeout(
        Duration::from_secs(10),
        stream.read_to_string(&mut response),
    )
    .await
    .expect("race response deadline")
    .expect("response");
    assert!(response.starts_with("HTTP/1.1 200") && response.contains("permission_denied"));
    assert!(!response.contains("operation_id") && !response.contains(secret));
    update(
        owner,
        &reduced,
        client.grants.clone(),
        "http-restore-after-race",
    )
    .await
}
async fn call(
    client: &Sdk,
    trace: &mut Vec<Value>,
    name: &str,
    args: Value,
) -> Result<CallToolResult, ServiceError> {
    let params = CallToolRequestParams::new(format!("alcomd_{name}"))
        .with_arguments(args.as_object().expect("tool args").clone());
    let response = tokio::time::timeout(Duration::from_secs(30), client.call_tool_once(params))
        .await
        .expect("tool response deadline");
    match response {
        Ok(CallToolResponse::Complete(result)) => {
            trace.push(json!({"name":format!("alcomd_{name}"),"arguments":args,"result":result}));
            Ok(result)
        }
        Ok(_) => panic!("no Tasks or MRTR in this toolset"),
        Err(error) => {
            trace.push(
                json!({"name":format!("alcomd_{name}"),"arguments":args,"protocol_error":true}),
            );
            Err(error)
        }
    }
}
fn data(result: CallToolResult) -> Value {
    assert_ne!(result.is_error, Some(true), "business success required");
    result.structured_content.expect("structured data")["data"].clone()
}
fn error(result: CallToolResult, code: &str) {
    assert_eq!(result.is_error, Some(true));
    assert_eq!(
        result.structured_content.expect("structured error")["error"]["code"],
        code
    );
}
fn grant(permission: &str, kind: &str, id: &str) -> McpClientGrant {
    McpClientGrant {
        permission: permission.into(),
        resource_kind: kind.into(),
        resource_id: id.into(),
    }
}
fn grants(project: &str, write: bool) -> Vec<McpClientGrant> {
    let mut result = vec![grant("projects.read", "project", project)];
    if write {
        result.push(grant("packages.manage", "project", project));
    }
    for p in [
        "packages.read",
        "repositories.read",
        "operations.read",
        "operations.cancel",
    ] {
        result.push(grant(p, "global", "*"));
    }
    result
}
async fn update(
    owner: &mut AlcomdClient,
    client: &McpClientRecord,
    grants: Vec<McpClientGrant>,
    key: &str,
) -> McpClientRecord {
    owner
        .mcp_client_set_grants(McpClientsSetGrantsParams {
            client_id: client.client_id.clone(),
            expected_revision: client.revision,
            idempotency_key: key.into(),
            grants,
        })
        .await
        .expect("explicit owner grants")
}
fn project(root: &Path, risk: bool) {
    std::fs::create_dir_all(root.join("ProjectSettings")).expect("isolated project");
    std::fs::create_dir(root.join("Packages")).expect("isolated packages");
    std::fs::write(
        root.join("ProjectSettings/ProjectVersion.txt"),
        "m_EditorVersion: 2022.3.22f1\n",
    )
    .expect("version");
    std::fs::write(
        root.join("Packages/manifest.json"),
        r#"{"dependencies":{}}"#,
    )
    .expect("UPM");
    let package = |name: &str, dependencies: Value| {
        let path = root.join("Packages").join(name);
        std::fs::create_dir(&path).expect("package folder");
        std::fs::write(
            path.join("package.json"),
            json!({"name":name,"version":"1.0.0","dependencies":dependencies}).to_string(),
        )
        .expect("package manifest");
    };
    let manifest = if risk {
        package("com.example.a", json!({"com.example.b":"1.0.0"}));
        package("com.example.b", json!({}));
        json!({"dependencies":{"com.example.a":{"version":"1.0.0"}},"locked":{"com.example.a":{"version":"1.0.0","dependencies":{"com.example.b":"1.0.0"}},"com.example.b":{"version":"1.0.0"}}})
    } else {
        package("com.example.fixture", json!({}));
        json!({"dependencies":{"com.example.fixture":{"version":"1.0.0"}},"locked":{"com.example.fixture":{"version":"1.0.0"}}})
    };
    std::fs::write(
        root.join("Packages/vpm-manifest.json"),
        manifest.to_string(),
    )
    .expect("VPM");
}
async fn terminal(client: &Sdk, trace: &mut Vec<Value>, id: &str) -> Value {
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let op = data(
                call(client, trace, "get_operation", json!({"operation_id":id}))
                    .await
                    .expect("operation read"),
            );
            if ["succeeded", "failed", "cancelled"].contains(&op["state"].as_str().expect("state"))
            {
                return op;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("business terminal deadline")
}

/// Bind the real per-user endpoint before any client is created. On Windows the
/// authorized preservation runner must release the original Core first.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn frozen_sdk_package_removal_real_core_flow() {
    package_removal_flow(false).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn frozen_sdk_http_package_removal_real_core_flow() {
    package_removal_flow(true).await;
}
async fn package_removal_flow(http: bool) {
    let _serial = REAL_CORE_LOCK.lock().await;
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/m8-business-tests")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&root).expect("owned evidence root");
    let root = root.canonicalize().expect("fixture root");
    let data_root = root.join("data");
    std::fs::create_dir(&data_root).expect("isolated Core data");
    let ipc = IpcConfig::default();
    let config = ClientConfig::default().without_daemon_start();
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        ipc.clone(),
        DataConfig::isolated(data_root.clone()),
        std::future::pending(),
    )
    .await
    .expect("exclusive real Core bind");
    let http = if http {
        Some(HttpFixture::start().await)
    } else {
        None
    };
    let url = http.as_ref().map(|value| value.url.as_str());
    let mut owner = AlcomdClient::connect(config.clone())
        .await
        .expect("native test owner");
    let p = root.join("Project");
    let risk = root.join("Risk");
    let cancel = root.join("Cancellation");
    let detached = root.join("Detached");
    project(&p, false);
    project(&risk, true);
    project(&cancel, false);
    project(&detached, false);
    let first = owner
        .project_register(p.to_string_lossy().into_owned(), "register-one".into())
        .await
        .expect("register")
        .project;
    let risk_record = owner
        .project_register(risk.to_string_lossy().into_owned(), "register-risk".into())
        .await
        .expect("register risk")
        .project;
    let cancel_record = owner
        .project_register(
            cancel.to_string_lossy().into_owned(),
            "register-cancel".into(),
        )
        .await
        .expect("register cancellation")
        .project;
    let detached_record = owner
        .project_register(
            detached.to_string_lossy().into_owned(),
            "register-detached".into(),
        )
        .await
        .expect("register detached")
        .project;
    let id = first.project_id.clone().expect("ID");
    let rid = risk_record.project_id.clone().expect("risk ID");
    let cid = cancel_record.project_id.clone().expect("cancel ID");
    let before = std::fs::read(p.join("Packages/vpm-manifest.json")).expect("before manifest");
    let risk_before = std::fs::read(risk.join("Packages/vpm-manifest.json")).expect("risk before");
    let upm_before = std::fs::read(p.join("Packages/manifest.json")).expect("UPM before");
    let mut a = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "Tools A".into(),
            grants: vec![],
            idempotency_key: "client-a".into(),
        })
        .await
        .expect("create A");
    let mut b = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "Tools B".into(),
            grants: vec![],
            idempotency_key: "client-b".into(),
        })
        .await
        .expect("create B");
    let token_a = owner
        .mcp_client_issue_credential(McpClientMutationParams {
            client_id: a.client_id.clone(),
            expected_revision: a.revision,
            idempotency_key: "issue-a".into(),
        })
        .await
        .expect("issue A");
    a.revision = token_a.revision;
    let token_b = owner
        .mcp_client_issue_credential(McpClientMutationParams {
            client_id: b.client_id.clone(),
            expected_revision: b.revision,
            idempotency_key: "issue-b".into(),
        })
        .await
        .expect("issue B");
    b.revision = token_b.revision;
    let sa = sdk(&token_a.credential, url).await;
    let sb = sdk(&token_b.credential, url).await;
    let mut trace = vec![];
    let definitions = sa.list_tools(None).await.expect("SDK tools/list");
    assert_eq!(definitions.tools.len(), 6);
    assert_eq!(definitions.ttl_ms, Some(0));
    std::fs::write(
        root.join("tools-list.json"),
        serde_json::to_vec_pretty(&definitions).expect("list JSON"),
    )
    .expect("definitions evidence");
    error(
        call(&sa, &mut trace, "list_projects", json!({}))
            .await
            .expect("default reject"),
        "permission_denied",
    );
    a = update(&mut owner, &a, grants(&id, false), "read-a").await;
    if let Some(url) = url {
        http_identity_and_admission(url, &token_a.credential, &token_b.credential, &id).await;
    }
    let listed = data(
        call(&sa, &mut trace, "list_projects", json!({"limit":1}))
            .await
            .expect("filtered read"),
    );
    assert_eq!(listed["projects"].as_array().expect("projects").len(), 1);
    assert_eq!(listed["projects"][0]["project_id"], id);
    error(
        call(&sa, &mut trace, "get_project", json!({"project_id":rid}))
            .await
            .expect("outside scope"),
        "permission_denied",
    );
    let read = data(
        call(&sa, &mut trace, "get_project", json!({"project_id":id}))
            .await
            .expect("snapshot"),
    );
    assert_eq!(
        read["locked_dependencies"][0]["package_id"],
        "com.example.fixture"
    );
    assert!(read.get("root_path").is_none());
    error(call(&sa,&mut trace,"plan_package_uninstall",json!({"project_id":id,"expected_revision":first.revision.expect("revision"),"package_id":"com.example.absent"})).await.expect("real business failure"),"package_not_found");
    let old_plan=data(call(&sa,&mut trace,"plan_package_uninstall",json!({"project_id":id,"expected_revision":first.revision.expect("revision"),"package_id":"com.example.fixture"})).await.expect("real plan"));
    let args = json!({"plan_id":old_plan["plan_id"],"expected_revision":old_plan["project_revision"],"idempotency_key":"apply","confirm":true});
    error(
        call(&sa, &mut trace, "apply_package_changes", args.clone())
            .await
            .expect("read cannot write"),
        "permission_denied",
    );
    let mut unconfirmed = args.clone();
    unconfirmed["confirm"] = json!(false);
    assert!(
        call(&sa, &mut trace, "apply_package_changes", unconfirmed)
            .await
            .is_err()
    );
    assert_eq!(
        std::fs::read(p.join("Packages/vpm-manifest.json")).expect("no pre-confirm write"),
        before
    );
    a = update(&mut owner, &a, grants(&id, true), "write-a").await;
    update(&mut owner, &b, grants(&id, true), "same-resource-b").await;
    error(
        call(&sb, &mut trace, "apply_package_changes", args.clone())
            .await
            .expect("foreign plan"),
        "plan_not_found",
    );
    let mut changed: Value = serde_json::from_slice(&before).expect("manifest");
    changed["dependencies"]["com.example.fixture"]["version"] = json!("^1.0.0");
    std::fs::write(p.join("Packages/vpm-manifest.json"), changed.to_string())
        .expect("authorized disposable external edit");
    let refreshed = owner
        .project_refresh(
            id.clone(),
            first.revision.expect("revision"),
            "refresh-fixture".into(),
        )
        .await
        .expect("current registry refresh")
        .project;
    assert!(refreshed.revision > first.revision);
    error(
        call(&sa, &mut trace, "apply_package_changes", args)
            .await
            .expect("old plan must fail"),
        "plan_stale",
    );
    let mut scoped = grants(&id, true);
    scoped.push(grant("projects.read", "project", &rid));
    scoped.push(grant("projects.read", "project", &cid));
    scoped.push(grant("packages.manage", "project", &cid));
    a = update(&mut owner, &a, scoped, "explicit-second-projects").await;
    let risks=data(call(&sa,&mut trace,"plan_package_uninstall",json!({"project_id":rid,"expected_revision":risk_record.revision,"package_id":"com.example.b"})).await.expect("dependency risk plan"));
    assert_eq!(
        risks["change_set"]["removal_conflicts"][0]["package_id"],
        "com.example.b"
    );
    error(call(&sa,&mut trace,"apply_package_changes",json!({"plan_id":risks["plan_id"],"expected_revision":risks["project_revision"],"idempotency_key":"no-write-scope","confirm":true})).await.expect("read scope is not write scope"),"permission_denied");
    let plan=data(call(&sa,&mut trace,"plan_package_uninstall",json!({"project_id":id,"expected_revision":refreshed.revision,"package_id":"com.example.fixture"})).await.expect("fresh plan"));
    let apply = json!({"plan_id":plan["plan_id"],"expected_revision":plan["project_revision"],"idempotency_key":"accepted-removal","confirm":true});
    if let Some(url) = url {
        a = http_hello_does_not_freeze_grants(url, &token_a.credential, &mut owner, &a, &apply)
            .await;
    }
    let accepted = data(
        call(&sa, &mut trace, "apply_package_changes", apply.clone())
            .await
            .expect("actual Core Apply"),
    );
    let operation = accepted["operation_id"]
        .as_str()
        .expect("operation ID")
        .to_owned();
    error(
        call(
            &sb,
            &mut trace,
            "get_operation",
            json!({"operation_id":operation}),
        )
        .await
        .expect("foreign task"),
        "operation_not_found",
    );
    let completed = terminal(&sa, &mut trace, &operation).await;
    assert_eq!(completed["state"], "succeeded");
    assert_eq!(completed["result"]["plan_id"], plan["plan_id"]);
    assert_eq!(
        completed["result"]["change_set_fingerprint"],
        plan["change_set_fingerprint"]
    );
    let replay = data(
        call(&sa, &mut trace, "apply_package_changes", apply.clone())
            .await
            .expect("same key replay"),
    );
    assert_eq!(replay["operation_id"], operation);
    assert_eq!(replay["replayed"], true);
    assert!(!p.join("Packages/com.example.fixture").exists());
    let after: Value = serde_json::from_slice(
        &std::fs::read(p.join("Packages/vpm-manifest.json")).expect("actual manifest"),
    )
    .expect("actual JSON");
    assert!(after["dependencies"].get("com.example.fixture").is_none());
    assert!(after["locked"].get("com.example.fixture").is_none());
    assert_eq!(
        std::fs::read(p.join("Packages/manifest.json")).expect("UPM after"),
        upm_before
    );
    a = update(&mut owner, &a, grants(&id, false), "withdraw-write").await;
    error(
        call(&sa, &mut trace, "apply_package_changes", apply.clone())
            .await
            .expect("replay cannot bypass withdrawal"),
        "permission_denied",
    );
    a = update(&mut owner, &a, grants(&cid, true), "cancel-project").await;
    let cancel_plan=data(call(&sa,&mut trace,"plan_package_uninstall",json!({"project_id":cid,"expected_revision":cancel_record.revision,"package_id":"com.example.fixture"})).await.expect("cancel plan"));
    let cancel_apply=data(call(&sa,&mut trace,"apply_package_changes",json!({"plan_id":cancel_plan["plan_id"],"expected_revision":cancel_plan["project_revision"],"idempotency_key":"cancel-task","confirm":true})).await.expect("cancel candidate"));
    let cancel_id = cancel_apply["operation_id"].as_str().expect("cancel ID");
    let state = data(
        call(
            &sa,
            &mut trace,
            "get_operation",
            json!({"operation_id":cancel_id}),
        )
        .await
        .expect("current task revision"),
    );
    let cancellation=call(&sa,&mut trace,"cancel_operation",json!({"operation_id":cancel_id,"expected_revision":state["revision"],"idempotency_key":"cancel"})).await.expect("real cancellation response");
    if cancellation.is_error == Some(true) {
        let code = &cancellation.structured_content.expect("cancel error")["error"]["code"];
        assert!(code == "operation_not_cancellable" || code == "revision_conflict");
    } else {
        let returned = data(cancellation);
        assert!(
            returned["operation"]["state"] == "cancelling"
                || returned["operation"]["state"] == "cancelled"
                || returned["operation"]["state"] == "succeeded"
        );
    }
    let cancel_terminal = terminal(&sa, &mut trace, cancel_id).await;
    assert!(cancel_terminal["state"] == "succeeded" || cancel_terminal["state"] == "cancelled");
    let detached_id = detached_record.project_id.expect("detached project ID");
    a = update(
        &mut owner,
        &a,
        grants(&detached_id, true),
        "detached-grants",
    )
    .await;
    let detached_plan=data(call(&sa,&mut trace,"plan_package_uninstall",json!({"project_id":detached_id,"expected_revision":detached_record.revision,"package_id":"com.example.fixture"})).await.expect("detached plan"));
    let detached_op=data(call(&sa,&mut trace,"apply_package_changes",json!({"plan_id":detached_plan["plan_id"],"expected_revision":detached_plan["project_revision"],"idempotency_key":"disconnect-after-accept","confirm":true})).await.expect("accepted before EOF"));
    // Close the actual SDK/STDIO process immediately after acceptance. Query the
    // accepted task using a new independently authenticated SDK connection.
    stop(sa).await;
    let reconnected = sdk(&token_a.credential, url).await;
    let detached_terminal = terminal(
        &reconnected,
        &mut trace,
        detached_op["operation_id"]
            .as_str()
            .expect("detached operation"),
    )
    .await;
    assert_eq!(detached_terminal["state"], "succeeded");
    assert!(!detached.join("Packages/com.example.fixture").exists());
    stop(reconnected).await;
    stop(sb).await;
    drop(owner);
    daemon.abort();
    let _ = daemon.await;
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        ipc,
        DataConfig::isolated(data_root.clone()),
        std::future::pending(),
    )
    .await
    .expect("restart authoritative data");
    let mut owner = AlcomdClient::connect(config.clone())
        .await
        .expect("native owner after restart");
    a = update(&mut owner, &a, grants(&id, true), "restore-original-scope").await;
    let sa = sdk(&token_a.credential, url).await;
    let sb = sdk(&token_b.credential, url).await;
    let persisted = data(
        call(
            &sa,
            &mut trace,
            "get_operation",
            json!({"operation_id":operation}),
        )
        .await
        .expect("persisted result"),
    );
    assert_eq!(persisted["state"], "succeeded");
    let persisted_replay = data(
        call(&sa, &mut trace, "apply_package_changes", apply)
            .await
            .expect("persistent idempotency"),
    );
    assert_eq!(persisted_replay["operation_id"], operation);
    let mut rotated_secret = None;
    let sa = if let Some(url) = url {
        let rotated = owner
            .mcp_client_issue_credential(McpClientMutationParams {
                client_id: a.client_id.clone(),
                expected_revision: a.revision,
                idempotency_key: "http-rotate-a".into(),
            })
            .await
            .expect("rotate while HTTP SDK remains alive");
        a.revision = rotated.revision;
        assert!(
            sa.list_tools(None).await.is_err(),
            "existing SDK must not keep old authentication"
        );
        let response = reqwest::Client::builder()
            .no_proxy()
            .build()
            .expect("rotation probe")
            .post(url)
            .bearer_auth(&token_a.credential)
            .header("Accept", "application/json,text/event-stream")
            .header("Content-Type", "application/json")
            .body("{}")
            .send()
            .await
            .expect("rotation status");
        assert_eq!(response.status(), 401);
        stop(sa).await;
        let fresh = sdk(&rotated.credential, Some(url)).await;
        assert_eq!(
            data(
                call(&fresh, &mut trace, "get_project", json!({"project_id":id}))
                    .await
                    .expect("new token uses original Principal")
            )["project_id"],
            id
        );
        rotated_secret = Some(rotated.credential);
        fresh
    } else {
        sa
    };
    owner
        .mcp_client_revoke(McpClientMutationParams {
            client_id: a.client_id.clone(),
            expected_revision: a.revision,
            idempotency_key: "revoke-a".into(),
        })
        .await
        .expect("revoke connected client");
    let revoked = call(&sa, &mut trace, "get_project", json!({"project_id":id})).await;
    if http.is_some() {
        assert!(
            revoked.is_err(),
            "fresh HTTP hello rejects revoked credentials"
        );
        trace.last_mut().expect("revoked trace")["http_authentication_rejected"] = json!(true);
        let response = reqwest::Client::builder()
            .no_proxy()
            .build()
            .expect("revocation probe")
            .post(url.expect("HTTP URL"))
            .bearer_auth(rotated_secret.as_ref().expect("new token"))
            .header("Accept", "application/json,text/event-stream")
            .header("Content-Type", "application/json")
            .body("{}")
            .send()
            .await
            .expect("revocation status");
        assert_eq!(response.status(), 401);
    } else {
        error(
            revoked.expect("revoked tool returns error"),
            "permission_denied",
        );
    }
    assert!(sa.list_tools(None).await.is_err());
    assert_eq!(
        data(
            call(&sb, &mut trace, "get_project", json!({"project_id":id}))
                .await
                .expect("independent B")
        )["project_id"],
        id
    );
    stop(sa).await;
    stop(sb).await;
    drop(owner);
    daemon.abort();
    let _ = daemon.await;
    assert_eq!(
        std::fs::read(risk.join("Packages/vpm-manifest.json")).expect("risk unchanged"),
        risk_before
    );
    let db = rusqlite::Connection::open_with_flags(
        data_root.join("state.db"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .expect("isolated read-only evidence");
    let owner_id: String = db
        .query_row(
            "SELECT owner_principal_id FROM projects WHERE project_id=?1",
            [&id],
            |r| r.get(0),
        )
        .expect("resource owner");
    assert_eq!(owner_id, "builtin:local-owner");
    let task_owner: String = db
        .query_row(
            "SELECT owner_principal_id FROM operations WHERE operation_id=?1",
            [&operation],
            |r| r.get(0),
        )
        .expect("caller owner");
    assert_eq!(task_owner, a.principal_id);
    let stored_plan: String = db
        .query_row(
            "SELECT plan_id FROM package_plans WHERE apply_operation_id=?1",
            [&operation],
            |r| r.get(0),
        )
        .expect("original plan");
    assert_eq!(stored_plan, plan["plan_id"]);
    let trace_json = serde_json::to_vec_pretty(&trace).expect("trace");
    let visible = String::from_utf8_lossy(&trace_json);
    assert!(!visible.contains(&token_a.credential));
    assert!(!visible.contains(&token_b.credential));
    if let Some(secret) = rotated_secret {
        assert!(!visible.contains(&secret));
    }
    assert!(!visible.contains("root_path"));
    std::fs::write(root.join("tool-trace.json"), trace_json).expect("public contract evidence");
    std::fs::write(root.join("coverage.json"),json!({"cancelTerminal":cancel_terminal["state"],"realSdk":"rmcp 3.10.0 Discover","business":"removed actual VPM package","revoked":true,"resourceOwnerPreserved":true}).to_string()).expect("coverage");
    if let Some(http) = http {
        http.finish().await;
    }
    println!(
        "Frozen SDK business closure verified; isolated evidence: {}",
        root.display()
    );
}

/// A real Apply is durably accepted before each transport fault is released.
/// No mocked result, owner substitution, automatic retry or business cancel.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn frozen_sdk_inflight_cancel_and_lost_write_response() {
    transport_risks(false).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn frozen_sdk_http_inflight_cancel_and_lost_write_response() {
    transport_risks(true).await;
}
async fn transport_risks(http: bool) {
    use rmcp::model::{ClientRequest, Request};
    use rmcp::service::PeerRequestOptions;

    let _serial = REAL_CORE_LOCK.lock().await;
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/m8-business-tests")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&root).expect("owned risk evidence");
    let root = root.canonicalize().expect("risk root");
    let data_root = root.join("data");
    std::fs::create_dir(&data_root).expect("isolated data");
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        IpcConfig::default(),
        DataConfig::isolated(data_root.clone()),
        std::future::pending(),
    )
    .await
    .expect("exclusive isolated Core");
    let http = if http {
        Some(HttpFixture::start().await)
    } else {
        None
    };
    let url = http.as_ref().map(|value| value.url.as_str());
    let mut owner = AlcomdClient::connect(ClientConfig::default().without_daemon_start())
        .await
        .expect("isolated native owner");
    let mut projects = Vec::new();
    let mut names = vec!["RequestCancellation", "LostWriteResponse"];
    if http.is_some() {
        names.push("LostHttpResponse");
        names.push("CoreDeadline");
        names.push("ServerShutdown");
    }
    for name in names {
        let directory = root.join(name);
        project(&directory, false);
        let snapshot = owner
            .project_register(directory.to_string_lossy().into_owned(), name.into())
            .await
            .expect("isolated registration")
            .project;
        projects.push((directory, snapshot));
    }
    let client = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "Transport risk client".into(),
            grants: {
                let mut scope = grants(projects[0].1.project_id.as_deref().expect("id"), true);
                for (_, snapshot) in projects.iter().skip(1) {
                    let id = snapshot.project_id.as_deref().expect("id");
                    scope.push(grant("projects.read", "project", id));
                    scope.push(grant("packages.manage", "project", id));
                }
                scope
            },
            idempotency_key: "risk-client".into(),
        })
        .await
        .expect("scoped client");
    let token = owner
        .mcp_client_issue_credential(McpClientMutationParams {
            client_id: client.client_id.clone(),
            expected_revision: client.revision,
            idempotency_key: "risk-credential".into(),
        })
        .await
        .expect("real credential");
    let mut trace = Vec::new();
    let mut expected_requests = 0;
    let mut operations = Vec::new();
    for (index, (directory, snapshot)) in projects.iter().enumerate() {
        let id = snapshot.project_id.as_deref().expect("project ID");
        let stopping_server = if index == 4 {
            Some(HttpFixture::start().await)
        } else {
            None
        };
        let request_url = stopping_server
            .as_ref()
            .map(|server| server.url.as_str())
            .or(url);
        let (sdk_session, wire_fault) = if index == 2 {
            let (sdk, fault) =
                HttpResponseFault::start(&token.credential, url.expect("HTTP risk URL")).await;
            (sdk, Some(fault))
        } else {
            (sdk(&token.credential, request_url).await, None)
        };
        let plan = data(call(&sdk_session, &mut trace, "plan_package_uninstall", json!({
            "project_id":id,"expected_revision":snapshot.revision,"package_id":"com.example.fixture"
        })).await.expect("real frozen removal plan"));
        let args = json!({"plan_id":plan["plan_id"],"expected_revision":plan["project_revision"],
            "idempotency_key":format!("risk-apply-{index}"),"confirm":true});
        let gate = daemon.hold_next_mcp_apply_response(index == 1).await;
        if let Some(fault) = &wire_fault {
            fault.armed.store(true, std::sync::atomic::Ordering::SeqCst);
        }
        let request = sdk_session
            .send_cancellable_request(
                ClientRequest::CallToolRequest(Request::new(
                    CallToolRequestParams::new("alcomd_apply_package_changes")
                        .with_arguments(args.as_object().expect("args").clone()),
                )),
                if index == 3 {
                    PeerRequestOptions::with_timeout(Duration::from_secs(75))
                } else {
                    PeerRequestOptions::no_options()
                },
            )
            .await
            .expect("actual SDK Apply request");
        let accepted = tokio::time::timeout(Duration::from_secs(10), gate.accepted())
            .await
            .expect("real acceptance before response fault");
        assert!(!accepted.replayed);
        expected_requests += 1;
        assert_eq!(daemon.mcp_apply_request_count(), expected_requests);
        if index == 0 {
            // Observe the SDK cancellation separately: STDIO sends its existing
            // notification; modern HTTP closes the POST, without a cancel RPC.
            let mut request = request;
            let (_unused, replacement) = tokio::sync::oneshot::channel();
            let response = std::mem::replace(&mut request.rx, replacement);
            request
                .cancel(Some("isolated request cancellation".into()))
                .await
                .expect("cancel notification");
            assert!(matches!(
                response.await.expect("SDK cancellation response"),
                Err(ServiceError::Cancelled { .. })
            ));
            // Before the held Core response is released, cancellation must free
            // the adapter's handler and quarantine its interrupted IPC stream.
            let followup = tokio::time::timeout(
                Duration::from_secs(5),
                call(
                    &sdk_session,
                    &mut trace,
                    "get_project",
                    json!({"project_id":id}),
                ),
            )
            .await
            .expect("cancelled handler releases its interrupted connection")
            .expect("bounded tool failure");
            if http.is_some() {
                assert_eq!(data(followup)["project_id"], id);
            } else {
                error(followup, "core_unavailable");
            }
            gate.release();
        } else if index == 1 {
            // Send a genuine length-prefixed Core response, truncate its body
            // and close IPC after the durable acceptance transaction.
            gate.release();
            let response = tokio::time::timeout(Duration::from_secs(10), request.await_response())
                .await
                .expect("truncated write response deadline")
                .expect("MCP business error response");
            let rmcp::model::ServerResult::CallToolResult(result) = response else {
                panic!("tool result")
            };
            assert_eq!(
                result
                    .structured_content
                    .as_ref()
                    .expect("unknown response")["error"]["outcome"],
                "unknown"
            );
            assert_eq!(
                result
                    .structured_content
                    .as_ref()
                    .expect("unknown response")["error"]["retryable"],
                false
            );
            trace.push(
                json!({"name":"alcomd_apply_package_changes","arguments":args,"result":result}),
            );
            error(
                serde_json::from_value(trace.last().expect("trace")["result"].clone())
                    .expect("result"),
                "core_write_outcome_unknown",
            );
            let followup = call(
                &sdk_session,
                &mut trace,
                "get_project",
                json!({"project_id":id}),
            )
            .await
            .expect("post-fault request");
            if http.is_some() {
                assert_eq!(data(followup)["project_id"], id);
            } else {
                error(followup, "core_unavailable");
            }
        } else if index == 2 {
            gate.release();
            let response = tokio::time::timeout(Duration::from_secs(10), request.await_response())
                .await
                .expect("truncated HTTP response deadline");
            assert!(
                response.is_err(),
                "actual SDK cannot read a truncated real HTTP response"
            );
        } else if index == 3 {
            // Exercise the real production 60-second Core handling deadline,
            // with a durable Operation already accepted and no response released.
            let response = tokio::time::timeout(Duration::from_secs(65), request.await_response())
                .await
                .expect("real Core processing deadline")
                .expect("complete unknown outcome tool response");
            let rmcp::model::ServerResult::CallToolResult(result) = response else {
                panic!("tool result")
            };
            let structured = result.structured_content.as_ref().expect("unknown result");
            assert_eq!(structured["error"]["code"], "core_write_outcome_unknown");
            assert_eq!(structured["error"]["outcome"], "unknown");
            assert_eq!(structured["error"]["retryable"], false);
            trace.push(
                json!({"name":"alcomd_apply_package_changes","arguments":args,"result":result}),
            );
            gate.release();
        } else {
            stopping_server
                .expect("owned shutdown target")
                .finish()
                .await;
            let response = tokio::time::timeout(Duration::from_secs(5), request.await_response())
                .await
                .expect("shutdown closes only protocol exchange");
            assert!(response.is_err());
            gate.release();
        }
        assert_eq!(
            daemon.mcp_apply_request_count(),
            expected_requests,
            "no automatic Apply retry"
        );
        stop(sdk_session).await;
        drop(wire_fault);
        // Explicit operator recovery: same identity, original Plan/revision/key.
        let recovered = sdk(&token.credential, url).await;
        let replay = data(
            call(
                &recovered,
                &mut trace,
                "apply_package_changes",
                args.clone(),
            )
            .await
            .expect("explicit idempotent recovery"),
        );
        expected_requests += 1;
        assert_eq!(daemon.mcp_apply_request_count(), expected_requests);
        assert_eq!(replay["operation_id"], accepted.operation_id);
        assert_eq!(replay["replayed"], true);
        let completed = terminal(&recovered, &mut trace, &accepted.operation_id).await;
        assert_eq!(
            completed["state"], "succeeded",
            "protocol fault never cancels accepted business"
        );
        let current = data(
            call(
                &recovered,
                &mut trace,
                "get_project",
                json!({"project_id":id}),
            )
            .await
            .expect("post-removal project"),
        );
        assert_eq!(
            current["revision"],
            snapshot.revision.expect("revision") + 1,
            "project commits once"
        );
        assert!(!directory.join("Packages/com.example.fixture").exists());
        stop(recovered).await;
        operations.push(accepted.operation_id);
    }
    drop(owner);
    daemon.abort();
    let _ = daemon.await;
    let db = rusqlite::Connection::open_with_flags(
        data_root.join("state.db"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .expect("read-only isolated authority evidence");
    for operation in &operations {
        let (state, cancel, owner): (String,i64,String) = db.query_row(
            "SELECT state,cancel_requested,owner_principal_id FROM operations WHERE operation_id=?1", [operation],
            |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
        ).expect("authoritative operation");
        assert_eq!(state, "succeeded");
        assert_eq!(cancel, 0);
        assert_eq!(owner, client.principal_id);
        let accepted: i64 = db.query_row("SELECT count(*) FROM package_filesystem_journal WHERE operation_id=?1 AND phase='accepted' AND state='completed'", [operation], |r|r.get(0)).expect("acceptance journal");
        assert_eq!(accepted, 1);
    }
    let count: i64 = db
        .query_row(
            "SELECT count(*) FROM operations WHERE kind='packages.apply'",
            [],
            |r| r.get(0),
        )
        .expect("operation count");
    assert_eq!(
        count,
        projects.len() as i64,
        "one Operation per original Plan, no duplicate execution"
    );
    let visible = serde_json::to_string_pretty(&trace).expect("public trace");
    assert!(!visible.contains(&token.credential));
    std::fs::write(root.join("risk-tool-trace.json"), visible).expect("risk trace");
    std::fs::write(root.join("risk-coverage.json"),json!({"protocol":"2026-07-28","cancelledRequestPreservedOperation":true,"truncatedCoreWriteResponse":true,"truncatedHttpResponse":http.is_some(),"coreDeadline":http.is_some(),"serverShutdown":http.is_some(),"automaticApplyRetries":0,"explicitRecoveryRequests":projects.len(),"applyRequests":expected_requests,"operations":operations,"businessCancelRequested":false,"terminal":"succeeded"}).to_string()).expect("coverage");
    if let Some(http) = http {
        http.finish().await;
    }
    println!(
        "Frozen SDK transport risks verified; isolated evidence: {}",
        root.display()
    );
}
