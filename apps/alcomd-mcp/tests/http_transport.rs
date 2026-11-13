//! Admission tests never authenticate against or start the user's running Core.
use alcomd_client::ClientConfig;
use alcomd_mcp::HttpServer;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn raw(address: std::net::SocketAddr, request: &str) -> String {
    let mut stream = tokio::net::TcpStream::connect(address)
        .await
        .expect("test HTTP connect");
    stream.write_all(request.as_bytes()).await.expect("request");
    let mut response = Vec::new();
    tokio::time::timeout(Duration::from_secs(3), stream.read_to_end(&mut response))
        .await
        .expect("response deadline")
        .expect("HTTP read");
    String::from_utf8(response).expect("bounded UTF-8 error")
}

#[tokio::test]
async fn loopback_admission_is_closed_without_core_or_credentials() {
    let server = HttpServer::bind(0, ClientConfig::default().without_daemon_start())
        .await
        .expect("loopback bind");
    let address = server.local_addr().expect("bound address");
    assert_eq!(address.ip(), std::net::Ipv4Addr::LOCALHOST);
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(server.run_until(async {
        let _ = stopped.await;
    }));
    let auth = "a".repeat(64);
    for (headers, path, method, status) in [
        (format!("Host: {address}\r\n"), "/mcp", "POST", 401),
        (
            format!("Host: {address}\r\nCookie: token=secret-marker\r\n"),
            "/mcp",
            "POST",
            401,
        ),
        (
            format!(
                "Host: {address}\r\nMCP-Protocol-Version: 2026-07-28\r\nMCP-Protocol-Version: 2025-11-25\r\n"
            ),
            "/mcp",
            "POST",
            400,
        ),
        (
            format!(
                "Host: {address}\r\nMCP-Protocol-Version: 2026-07-28\r\nMCP-Protocol-Version: 2026-07-28\r\n"
            ),
            "/mcp",
            "POST",
            400,
        ),
        (
            format!("Host: {address}\r\nOrigin: null\r\n"),
            "/mcp",
            "POST",
            403,
        ),
        (
            format!("Host: {address}\r\nOrigin: http://evil.example\r\n"),
            "/mcp",
            "POST",
            403,
        ),
        ("Host: evil.example\r\n".into(), "/mcp", "POST", 403),
        (
            format!("Host: localhost:{}\r\n", address.port()),
            "/mcp",
            "POST",
            403,
        ),
        (
            format!("Host: {address}\r\n"),
            "/mcp?token=secret-marker",
            "POST",
            400,
        ),
        (
            format!("Host: {address}\r\nMcp-Session-Id: secret-marker\r\n"),
            "/mcp",
            "POST",
            400,
        ),
        (
            format!("Host: {address}\r\nLast-Event-ID: secret-marker\r\n"),
            "/mcp",
            "POST",
            400,
        ),
        (format!("Host: {address}\r\n"), "/mcp", "GET", 405),
        (format!("Host: {address}\r\n"), "/mcp", "DELETE", 405),
        (
            format!("Host: {address}\r\nOrigin: null\r\n"),
            "/mcp",
            "OPTIONS",
            403,
        ),
        (format!("Host: {address}\r\n"), "/missing", "POST", 404),
        (
            format!("Host: {address}\r\nAuthorization: Bearer secret-marker\r\n"),
            "/mcp",
            "POST",
            400,
        ),
        (
            format!(
                "Host: {address}\r\nAuthorization: Bearer {auth}\r\nAuthorization: Bearer {auth}\r\n"
            ),
            "/mcp",
            "POST",
            400,
        ),
        (
            format!(
                "Host: {address}\r\nAuthorization: Bearer {auth}\r\nContent-Type: text/plain\r\n"
            ),
            "/mcp",
            "POST",
            415,
        ),
        (
            format!(
                "Host: {address}\r\nAuthorization: Bearer {auth}\r\nContent-Type: application/json\r\nAccept: application/json\r\n"
            ),
            "/mcp",
            "POST",
            406,
        ),
        (
            format!(
                "Host: {address}\r\nAuthorization: Bearer {auth}\r\nContent-Type: application/json\r\nAccept: application/json,text/event-stream\r\nContent-Encoding: gzip\r\n"
            ),
            "/mcp",
            "POST",
            415,
        ),
    ] {
        let request = format!(
            "{method} {path} HTTP/1.1\r\n{headers}Content-Length: 0\r\nConnection: close\r\n\r\n"
        );
        let response = raw(address, &request).await;
        assert!(
            response.starts_with(&format!("HTTP/1.1 {status}")),
            "unexpected status for {method} {path}"
        );
        assert!(!response.contains("secret-marker") && !response.contains(&auth));
        assert!(
            !response
                .to_ascii_lowercase()
                .contains("access-control-allow-origin")
        );
        if status == 401 {
            assert!(
                response
                    .to_ascii_lowercase()
                    .contains("www-authenticate: bearer")
            );
        }
    }
    let conflicting = format!(
        "POST http://evil.example/mcp HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\nContent-Length: 0\r\n\r\n"
    );
    assert!(raw(address, &conflicting).await.starts_with("HTTP/1.1 400"));
    let huge = format!(
        "POST /mcp HTTP/1.1\r\nHost: {address}\r\nX-Test: {}\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
        "x".repeat(17 * 1024)
    );
    assert!(raw(address, &huge).await.starts_with("HTTP/1.1 431"));
    for headers in [
        String::new(),
        format!("Host: {address}\r\nHost: {address}\r\n"),
    ] {
        assert!(
            raw(
                address,
                &format!(
                    "POST /mcp HTTP/1.1\r\n{headers}Content-Length: 0\r\nConnection: close\r\n\r\n"
                )
            )
            .await
            .starts_with("HTTP/1.1 400")
        );
    }
    let oversize = format!(
        "POST /mcp HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {auth}\r\nContent-Type: application/json\r\nAccept: application/json,text/event-stream\r\nContent-Length: 4194305\r\nConnection: close\r\n\r\n"
    );
    assert!(
        raw(address, &oversize).await.starts_with("HTTP/1.1 413"),
        "known oversize is rejected without any Core connection or body wait"
    );
    stop.send(()).expect("stop owned HTTP server");
    tokio::time::timeout(Duration::from_secs(6), task)
        .await
        .expect("server exits")
        .expect("join")
        .expect("server result");
    assert!(tokio::net::TcpStream::connect(address).await.is_err());
}

#[tokio::test]
async fn tcp_quota_slow_headers_and_shutdown_are_bounded() {
    use http_body_util::{BodyExt, Full};
    use hyper::{Request, body::Bytes};
    use hyper_util::rt::TokioIo;
    let server = HttpServer::bind(0, ClientConfig::default().without_daemon_start())
        .await
        .expect("own listener");
    let address = server.local_addr().expect("address");
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(server.run_until(async {
        let _ = stopped.await;
    }));
    let mut clients = Vec::new();
    for _ in 0..32 {
        let stream = tokio::net::TcpStream::connect(address)
            .await
            .expect("quota connection");
        let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
            .await
            .expect("client connection");
        let connection = tokio::spawn(connection);
        let response = sender
            .send_request(
                Request::builder()
                    .method("GET")
                    .uri("/mcp")
                    .header("Host", address.to_string())
                    .body(Full::new(Bytes::new()))
                    .expect("request"),
            )
            .await
            .expect("prove server accepted each connection");
        assert_eq!(response.status(), 405);
        response
            .into_body()
            .collect()
            .await
            .expect("drain response");
        clients.push((sender, connection));
    }
    let mut extra = tokio::net::TcpStream::connect(address)
        .await
        .expect("quota probe socket");
    let mut byte = [0u8; 1];
    let read = tokio::time::timeout(Duration::from_secs(2), extra.read(&mut byte))
        .await
        .expect("excess TCP closed, no unbounded queue");
    assert!(matches!(read, Ok(0) | Err(_)));
    drop(extra);
    for (sender, connection) in clients {
        drop(sender);
        connection
            .await
            .expect("client join")
            .expect("normal close");
    }
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(2))
        .build()
        .expect("release client");
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Ok(response) = client.post(format!("http://{address}/mcp")).send().await {
                assert_eq!(response.status(), 401);
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("closed connections release all slots");
    let mut slow = tokio::net::TcpStream::connect(address)
        .await
        .expect("slow header socket");
    slow.write_all(b"POST /mcp HTTP/1.1\r\nHost:")
        .await
        .expect("partial header");
    let mut response = String::new();
    tokio::time::timeout(Duration::from_secs(12), slow.read_to_string(&mut response))
        .await
        .expect("10 second header deadline")
        .expect("timeout response");
    assert!(response.starts_with("HTTP/1.1 408"));
    let mut idle = tokio::net::TcpStream::connect(address)
        .await
        .expect("shutdown socket");
    stop.send(()).expect("normal stop");
    tokio::time::timeout(Duration::from_secs(6), task)
        .await
        .expect("bounded server shutdown")
        .expect("join")
        .expect("stop");
    let read = tokio::time::timeout(Duration::from_secs(1), idle.read(&mut byte))
        .await
        .expect("idle connection closed");
    assert!(matches!(read, Ok(0) | Err(_)));
}
