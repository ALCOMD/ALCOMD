use super::tools::{Alcomd3Mcp, ToolInvocationLimiter};
use super::types::{MCP_HTTP_BIND_HOST, MCP_HTTP_MIN_TOKEN_BYTES, MCP_HTTP_PATH};
use axum::{
    Router,
    extract::{Request, State},
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
    middleware::{self, Next},
    response::Response,
};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    for index in 0..left.len().max(right.len()) {
        difference |= usize::from(
            left.get(index).copied().unwrap_or_default()
                ^ right.get(index).copied().unwrap_or_default(),
        );
    }
    difference == 0
}

fn has_valid_bearer_token(headers: &HeaderMap, expected_token: &str) -> bool {
    headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .is_some_and(|actual_token| {
            constant_time_eq(actual_token.as_bytes(), expected_token.as_bytes())
        })
}

async fn require_bearer_token(
    State(expected_token): State<Arc<str>>,
    request: Request,
    next: Next,
) -> std::result::Result<Response, StatusCode> {
    if !has_valid_bearer_token(request.headers(), &expected_token) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(next.run(request).await)
}

pub(super) struct ActiveMcpHttpServer {
    task: tokio::task::JoinHandle<io::Result<()>>,
    cancellation_token: CancellationToken,
    pub(super) host: String,
    pub(super) port: u16,
    pub(super) token: String,
}

impl ActiveMcpHttpServer {
    pub(super) fn is_finished(&self) -> bool {
        self.task.is_finished()
    }

    pub(super) async fn stop(self) {
        self.cancellation_token.cancel();
        match self.task.await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                log::error!("embedded MCP Streamable HTTP server failed: {error}");
            }
            Err(error) if error.is_cancelled() => {}
            Err(error) => {
                log::error!("failed to join embedded MCP Streamable HTTP server: {error}");
            }
        }
    }
}

pub(super) async fn start(
    app: AppHandle,
    port: u16,
    token: String,
    limiter: ToolInvocationLimiter,
) -> io::Result<ActiveMcpHttpServer> {
    if token.len() < MCP_HTTP_MIN_TOKEN_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("MCP bearer token must contain at least {MCP_HTTP_MIN_TOKEN_BYTES} bytes"),
        ));
    }

    let token_for_middleware: Arc<str> = token.clone().into();
    let listener = bind_loopback(port).await?;
    let local_address = listener.local_addr()?;
    let cancellation_token = CancellationToken::new();
    let tasks = app.state::<super::McpState>().protocol_tasks();
    let handler_app = app.clone();
    let service: StreamableHttpService<Alcomd3Mcp, LocalSessionManager> =
        StreamableHttpService::new(
            move || {
                Ok(Alcomd3Mcp::new(
                    handler_app.clone(),
                    limiter.clone(),
                    tasks.clone(),
                ))
            },
            Default::default(),
            streamable_http_server_config(local_address.port(), cancellation_token.clone()),
        );
    let router =
        Router::new()
            .nest_service(MCP_HTTP_PATH, service)
            .layer(middleware::from_fn_with_state(
                token_for_middleware,
                require_bearer_token,
            ));

    let shutdown = cancellation_token.clone();
    let task = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown.cancelled_owned())
            .await
    });
    log::info!(
        "embedded ALCOMD3 MCP Streamable HTTP server listening on http://{local_address}{MCP_HTTP_PATH}"
    );

    Ok(ActiveMcpHttpServer {
        task,
        cancellation_token,
        host: MCP_HTTP_BIND_HOST.to_string(),
        port: local_address.port(),
        token,
    })
}

async fn bind_loopback(port: u16) -> io::Result<TcpListener> {
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    TcpListener::bind(address).await.map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("binding embedded ALCOMD3 MCP server to {address}: {error}"),
        )
    })
}

fn streamable_http_server_config(
    port: u16,
    cancellation_token: CancellationToken,
) -> StreamableHttpServerConfig {
    StreamableHttpServerConfig::default()
        .with_legacy_session_mode(true)
        .with_stateless_protocol_metadata_required(true)
        .with_allowed_hosts([
            MCP_HTTP_BIND_HOST.to_string(),
            format!("{MCP_HTTP_BIND_HOST}:{port}"),
            "localhost".to_string(),
            format!("localhost:{port}"),
        ])
        .with_allowed_origins([
            format!("http://{MCP_HTTP_BIND_HOST}:{port}"),
            format!("http://localhost:{port}"),
        ])
        .with_cancellation_token(cancellation_token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn streamable_http_config_is_loopback_only_and_strict_for_2026() {
        let config = streamable_http_server_config(51_739, CancellationToken::new());

        assert!(config.legacy_session_mode);
        assert!(config.stateless_protocol_metadata_required);
        assert_eq!(
            config.allowed_hosts,
            [
                "127.0.0.1",
                "127.0.0.1:51739",
                "localhost",
                "localhost:51739"
            ]
        );
        assert_eq!(
            config.allowed_origins,
            ["http://127.0.0.1:51739", "http://localhost:51739"]
        );
    }

    #[tokio::test]
    async fn loopback_binding_rejects_an_occupied_port() {
        let listener = bind_loopback(0).await.unwrap();
        let address = listener.local_addr().unwrap();

        assert_eq!(address.ip(), Ipv4Addr::LOCALHOST);
        assert!(bind_loopback(address.port()).await.is_err());
    }

    #[test]
    fn bearer_authentication_requires_the_exact_token() {
        let token = "0123456789abcdef0123456789abcdef";
        let mut headers = HeaderMap::new();

        assert!(!has_valid_bearer_token(&headers, token));
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Basic ignored"));
        assert!(!has_valid_bearer_token(&headers, token));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_static("Bearer 0123456789abcdef0123456789abcdee"),
        );
        assert!(!has_valid_bearer_token(&headers, token));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_static("Bearer 0123456789abcdef0123456789abcdef"),
        );
        assert!(has_valid_bearer_token(&headers, token));
    }

    #[test]
    fn constant_time_comparison_handles_length_and_content_mismatches() {
        assert!(constant_time_eq(b"same-token", b"same-token"));
        assert!(!constant_time_eq(b"same-token", b"different-token"));
        assert!(!constant_time_eq(b"same-token", b"same-token-longer"));
    }
}
