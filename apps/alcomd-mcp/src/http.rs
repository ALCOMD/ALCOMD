//! Approved loopback-only, preconfigured Bearer HTTP binding. Core owns identity.
use super::{MCP_PROTOCOL_VERSION, ProtocolService, tools};
use alcomd_client::{AlcomdClient, ClientConfig, ClientError};
use http_body_util::{BodyExt, Full, Limited, combinators::BoxBody};
use hyper::{
    Request as HttpRequest, Response, StatusCode,
    body::{Body, Bytes, Frame, Incoming, SizeHint},
    header,
    service::service_fn,
};
use hyper_util::rt::{TokioIo, TokioTimer};
use rmcp::{
    ErrorData, RoleServer, ServerHandler, Service,
    model::*,
    service::RequestContext,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::never::NeverSessionManager,
    },
};
use serde_json::{Value, json};
use std::{
    borrow::Cow,
    convert::Infallible,
    future::Future,
    io,
    net::SocketAddr,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};
use tokio::{
    net::TcpListener,
    sync::{Mutex, OwnedSemaphorePermit, Semaphore, watch},
    time::{Instant, Sleep},
};

const BODY_LIMIT: usize = 4 * 1024 * 1024;
const HEAD_LIMIT: usize = 16 * 1024;
const HEAD_COUNT: usize = 32;
const HEADER_TIME: Duration = Duration::from_secs(10);
const BODY_TIME: Duration = Duration::from_secs(15);
const CORE_TIME: Duration = Duration::from_secs(60);
const OUTPUT_TIME: Duration = Duration::from_secs(10);
type Sdk = StreamableHttpService<HttpHandler, NeverSessionManager>;

pub struct HttpServer {
    listener: TcpListener,
    config: ClientConfig,
}
impl HttpServer {
    /// The address is deliberately not a parameter: non-loopback is unsupported.
    pub async fn bind(port: u16, config: ClientConfig) -> io::Result<Self> {
        Ok(Self {
            listener: TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?,
            config,
        })
    }
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }
    pub async fn run_until(self, shutdown: impl Future<Output = ()> + Send) -> io::Result<()> {
        let authority = self.local_addr()?.to_string();
        let config = StreamableHttpServerConfig::default()
            .with_legacy_session_mode(false)
            .with_stateless_protocol_metadata_required(true)
            .with_allowed_hosts([authority.clone()])
            .enforce_origin_validation()
            .with_json_response(false)
            .with_sse_retry(None);
        let mut config = config;
        config.max_request_body_bytes = BODY_LIMIT;
        let sdk_shutdown = config.cancellation_token.clone();
        let state = Arc::new(State {
            authority,
            config: self.config,
            requests: Arc::new(Semaphore::new(16)),
            http_config: config,
        });
        let connections = Arc::new(Semaphore::new(32));
        let (stop, _) = watch::channel(false);
        let mut tasks = tokio::task::JoinSet::new();
        tokio::pin!(shutdown);
        loop {
            tokio::select! {
                biased;
                () = &mut shutdown => break,
                accepted = self.listener.accept() => {
                    let (stream, _) = accepted?;
                    let Ok(permit) = connections.clone().try_acquire_owned() else { drop(stream); continue; };
                    let state = Arc::clone(&state);
                    let mut stopping = stop.subscribe();
                    tasks.spawn(async move {
                        let _connection_permit = permit;
                        let handler = service_fn(move |request| {
                            let state = Arc::clone(&state);
                            Box::pin(async move { Ok::<_, Infallible>(state.handle(request).await) })
                        });
                        let mut builder = hyper::server::conn::http1::Builder::new();
                        builder.max_headers(HEAD_COUNT).max_buf_size(HEAD_LIMIT)
                            .header_read_timeout(HEADER_TIME).timer(TokioTimer::new());
                        let mut connection = builder.serve_connection(TokioIo::new(stream), handler);
                        let completed = tokio::select! {
                            result = std::future::poll_fn(|cx| connection.poll_without_shutdown(cx)) => Some(result),
                            _ = stopping.changed() => None,
                        };
                        // Hyper reports a header timeout without serializing 408.
                        // Its HTTP/1 parser is done; send only this fixed closing
                        // response before dropping the socket. Never echo input.
                        if let Some(completed) = completed {
                            use tokio::io::AsyncWriteExt;
                            let mut stream=connection.into_parts().io.into_inner();
                            if completed.is_err_and(|error| error.is_timeout()) {
                                let _=tokio::time::timeout(OUTPUT_TIME,stream.write_all(b"HTTP/1.1 408 Request Timeout\r\nContent-Length: 0\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n")).await;
                            }
                            // Preserve Hyper's normal write-half shutdown after
                            // taking ownership of the completed connection.
                            let _=tokio::time::timeout(OUTPUT_TIME,stream.shutdown()).await;
                        }
                    });
                }
                _ = tasks.join_next(), if !tasks.is_empty() => {},
            }
        }
        let _ = stop.send(true);
        sdk_shutdown.cancel();
        if tokio::time::timeout(Duration::from_secs(5), async {
            while tasks.join_next().await.is_some() {}
        })
        .await
        .is_err()
        {
            tasks.shutdown().await;
        }
        Ok(())
    }
}

struct State {
    authority: String,
    config: ClientConfig,
    requests: Arc<Semaphore>,
    http_config: StreamableHttpServerConfig,
}
#[derive(Clone)]
struct BoundRequest {
    service: Arc<ProtocolService>,
    deadline: Instant,
}

impl State {
    async fn handle(&self, request: HttpRequest<Incoming>) -> Response<GuardedBody> {
        let (mut parts, body) = request.into_parts();
        if parts.version != hyper::Version::HTTP_11 {
            return rejected(StatusCode::BAD_REQUEST);
        }
        let headers = &parts.headers;
        let host = match unique(headers, header::HOST.as_str()) {
            Ok(Some(value)) => value,
            _ => return rejected(StatusCode::BAD_REQUEST),
        };
        if let Some(authority) = parts.uri.authority()
            && (authority.as_str() != host || parts.uri.scheme_str() != Some("http"))
        {
            return rejected(StatusCode::BAD_REQUEST);
        }
        if host != self.authority {
            return rejected(StatusCode::FORBIDDEN);
        }
        if headers.contains_key(header::ORIGIN) {
            return rejected(StatusCode::FORBIDDEN);
        }
        if parts.uri.path() != "/mcp" {
            return rejected(StatusCode::NOT_FOUND);
        }
        if parts.uri.query().is_some()
            || headers.contains_key("mcp-session-id")
            || headers.contains_key("last-event-id")
        {
            return rejected(StatusCode::BAD_REQUEST);
        }
        if parts.method != hyper::Method::POST {
            let mut response = rejected(StatusCode::METHOD_NOT_ALLOWED);
            response
                .headers_mut()
                .insert(header::ALLOW, header::HeaderValue::from_static("POST"));
            return response;
        }
        // Unlike the SDK's Mcp-Method/Name helper, its protocol-version
        // lookup reads a single value. Reject ambiguous versions up front.
        if unique(headers, "mcp-protocol-version").is_err() {
            return rejected(StatusCode::BAD_REQUEST);
        }
        let credential = match bearer(headers) {
            Ok(Some(secret)) => secret,
            Ok(None) => return rejected(StatusCode::UNAUTHORIZED),
            Err(()) => return rejected(StatusCode::BAD_REQUEST),
        };
        if !json_content_type(headers) {
            return rejected(StatusCode::UNSUPPORTED_MEDIA_TYPE);
        }
        if let Ok(Some(encoding)) = unique(headers, header::CONTENT_ENCODING.as_str()) {
            if !encoding.eq_ignore_ascii_case("identity") {
                return rejected(StatusCode::UNSUPPORTED_MEDIA_TYPE);
            }
        } else if headers.contains_key(header::CONTENT_ENCODING) {
            return rejected(StatusCode::UNSUPPORTED_MEDIA_TYPE);
        }
        if !accepts_both(headers) {
            return rejected(StatusCode::NOT_ACCEPTABLE);
        }
        if body.size_hint().lower() > BODY_LIMIT as u64 {
            return rejected(StatusCode::PAYLOAD_TOO_LARGE);
        }
        let Ok(permit) = self.requests.clone().try_acquire_owned() else {
            return rejected(StatusCode::TOO_MANY_REQUESTS);
        };
        let deadline = Instant::now() + CORE_TIME;
        // A request never borrows STDIO's startup identity and never retries as owner.
        let core = match tokio::time::timeout_at(
            deadline,
            AlcomdClient::connect(
                self.config
                    .clone()
                    .with_client_name("alcomd-mcp")
                    .with_mcp_credential(credential),
            ),
        )
        .await
        {
            Ok(Ok(core)) => core,
            Ok(Err(ClientError::Remote(e)))
                if matches!(
                    e.code.as_str(),
                    "permission_denied" | "authorization_required"
                ) =>
            {
                return rejected(StatusCode::UNAUTHORIZED);
            }
            _ => return rejected(StatusCode::SERVICE_UNAVAILABLE),
        };
        // Remove secrets before SDK request Parts can enter any context or tracing.
        parts.headers.remove(header::AUTHORIZATION);
        parts.headers.remove(header::COOKIE);
        parts.headers.remove("baggage");
        let bytes =
            match tokio::time::timeout(BODY_TIME, Limited::new(body, BODY_LIMIT).collect()).await {
                Err(_) => return rejected(StatusCode::REQUEST_TIMEOUT),
                Ok(Err(e)) if e.is::<http_body_util::LengthLimitError>() => {
                    return rejected(StatusCode::PAYLOAD_TOO_LARGE);
                }
                Ok(Err(_)) => return rejected(StatusCode::BAD_REQUEST),
                Ok(Ok(data)) => data.to_bytes(),
            };
        // Admission policy only. Parsing/version/header/body agreement remain SDK-owned.
        let value: Value = match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(_) => return protocol_rejected(StatusCode::BAD_REQUEST, Value::Null, -32700),
        };
        if !value.is_object() || value.get("id").is_none() || !value["method"].is_string() {
            return protocol_rejected(StatusCode::BAD_REQUEST, Value::Null, -32600);
        }
        let id = value["id"].clone();
        let method = value["method"].as_str().unwrap_or_default();
        if !matches!(method, "server/discover" | "tools/list" | "tools/call") {
            return protocol_rejected(StatusCode::NOT_FOUND, id, -32601);
        }
        let service = Arc::new(ProtocolService {
            core: Mutex::new(Some(core)),
            bound: true,
        });
        let binding = BoundRequest {
            service: Arc::clone(&service),
            deadline,
        };
        let request = HttpRequest::from_parts(parts, Full::new(bytes));
        let sdk: Sdk = StreamableHttpService::new(
            move || {
                Ok(HttpHandler {
                    binding: binding.clone(),
                })
            },
            Arc::new(NeverSessionManager::default()),
            self.http_config.clone(),
        );
        let response = sdk.handle(request).await;
        let (mut parts, body) = response.into_parts();
        parts.headers.insert(
            header::CACHE_CONTROL,
            header::HeaderValue::from_static("no-store"),
        );
        if parts.status != StatusCode::OK {
            let status = parts.status;
            let data =
                tokio::time::timeout(OUTPUT_TIME, Limited::new(body, BODY_LIMIT).collect()).await;
            let code = data
                .ok()
                .and_then(Result::ok)
                .and_then(|b| serde_json::from_slice::<Value>(&b.to_bytes()).ok())
                .and_then(|v| v["error"]["code"].as_i64())
                .filter(|v| {
                    [
                        -32700, -32600, -32601, -32602, -32603, -32020, -32021, -32022,
                    ]
                    .contains(v)
                })
                .unwrap_or(-32603);
            return protocol_rejected(status, id, code);
        }
        // These handlers return complete results only. The SDK awaits their
        // first terminal message before returning this response, so output time
        // starts here, separately from the handler's Core deadline.
        Response::from_parts(parts, GuardedBody::new(body, Some(permit), Some(service)))
    }
}

fn unique<'a>(headers: &'a hyper::HeaderMap, name: &str) -> Result<Option<&'a str>, ()> {
    let mut values = headers.get_all(name).iter();
    let Some(first) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err(());
    }
    first.to_str().map(Some).map_err(|_| ())
}
fn bearer(headers: &hyper::HeaderMap) -> Result<Option<String>, ()> {
    let Some(value) = unique(headers, header::AUTHORIZATION.as_str())? else {
        return Ok(None);
    };
    let Some((scheme, token)) = value.split_once(' ') else {
        return Err(());
    };
    // HTTP auth-scheme is followed by one or more SP, never arbitrary whitespace.
    let token = token.trim_start_matches(' ');
    if !scheme.eq_ignore_ascii_case("bearer")
        || token.len() != 64
        || !token
            .bytes()
            .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
    {
        return Err(());
    }
    Ok(Some(token.to_owned()))
}
fn json_content_type(headers: &hyper::HeaderMap) -> bool {
    let Ok(Some(content)) = unique(headers, header::CONTENT_TYPE.as_str()) else {
        return false;
    };
    let mut parts = content.split(';').map(str::trim);
    parts
        .next()
        .is_some_and(|v| v.eq_ignore_ascii_case("application/json"))
        && parts.all(|v| {
            v.eq_ignore_ascii_case("charset=utf-8") || v.eq_ignore_ascii_case("charset=\"utf-8\"")
        })
}
fn accepts_both(headers: &hyper::HeaderMap) -> bool {
    let Ok(Some(value)) = unique(headers, header::ACCEPT.as_str()) else {
        return false;
    };
    let accepts = |wanted: &str| {
        value.split(',').any(|entry| {
            let mut parts = entry.trim().split(';').map(str::trim);
            let media = parts.next().unwrap_or_default();
            (media.eq_ignore_ascii_case(wanted) || media == "*/*")
                && !parts.any(|p| {
                    p.strip_prefix("q=").is_some_and(|v| {
                        v.parse::<f32>()
                            .map_or(true, |q| !q.is_finite() || q <= 0.0 || q > 1.0)
                    })
                })
        })
    };
    accepts("application/json") && accepts("text/event-stream")
}
fn rejected(status: StatusCode) -> Response<GuardedBody> {
    let mut response = Response::new(GuardedBody::new(
        Full::new(Bytes::from_static(b"{\"error\":\"HTTP request rejected\"}")).boxed(),
        None,
        None,
    ));
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/json"),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    if status == StatusCode::UNAUTHORIZED {
        response.headers_mut().insert(
            header::WWW_AUTHENTICATE,
            header::HeaderValue::from_static("Bearer realm=\"alcomd-mcp\""),
        );
    }
    if status == StatusCode::TOO_MANY_REQUESTS {
        response
            .headers_mut()
            .insert(header::RETRY_AFTER, header::HeaderValue::from_static("1"));
    }
    response
}
fn protocol_rejected(status: StatusCode, id: Value, code: i64) -> Response<GuardedBody> {
    let mut response = rejected(status);
    let mut error = json!({"code":code,"message":"MCP request rejected"});
    if code == -32603 {
        static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        error["data"] = json!({"diagnostic_id":format!("mcp-http-{}-{}",std::process::id(),SEQUENCE.fetch_add(1,std::sync::atomic::Ordering::Relaxed))});
    }
    let bytes = serde_json::to_vec(&json!({"jsonrpc":"2.0","id":id,"error":error}))
        .expect("bounded JSON values");
    *response.body_mut() = GuardedBody::new(Full::new(Bytes::from(bytes)).boxed(), None, None);
    response
}

struct GuardedBody {
    body: BoxBody<Bytes, Infallible>,
    remaining: usize,
    deadline: Pin<Box<Sleep>>,
    permit: Option<OwnedSemaphorePermit>,
    core: Option<Arc<ProtocolService>>,
    ended: bool,
}
impl GuardedBody {
    fn new(
        body: BoxBody<Bytes, Infallible>,
        permit: Option<OwnedSemaphorePermit>,
        core: Option<Arc<ProtocolService>>,
    ) -> Self {
        Self {
            body,
            remaining: BODY_LIMIT,
            deadline: Box::pin(tokio::time::sleep(OUTPUT_TIME)),
            permit,
            core,
            ended: false,
        }
    }
    fn finish(&mut self) {
        self.ended = true;
        self.permit.take();
        self.core.take();
    }
}
impl Body for GuardedBody {
    type Data = Bytes;
    type Error = io::Error;
    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, io::Error>>> {
        if self.ended {
            return Poll::Ready(None);
        }
        if self.deadline.as_mut().poll(cx).is_ready() {
            self.finish();
            return Poll::Ready(Some(Err(io::ErrorKind::TimedOut.into())));
        }
        match Pin::new(&mut self.body).poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(data) = frame.data_ref() {
                    if data.len() > self.remaining {
                        self.finish();
                        return Poll::Ready(Some(Err(io::ErrorKind::InvalidData.into())));
                    }
                    self.remaining -= data.len();
                }
                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Ready(None) => {
                self.finish();
                Poll::Ready(None)
            }
            Poll::Ready(Some(Err(error))) => match error {},
            Poll::Pending => Poll::Pending,
        }
    }
    fn is_end_stream(&self) -> bool {
        self.ended || self.body.is_end_stream()
    }
    fn size_hint(&self) -> SizeHint {
        SizeHint::default()
    }
}

struct HttpHandler {
    binding: BoundRequest,
}
impl ServerHandler for HttpHandler {
    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Owned(vec![MCP_PROTOCOL_VERSION])
    }
    fn get_info(&self) -> ServerConfig {
        super::ProtocolService {
            core: Mutex::new(None),
            bound: true,
        }
        .get_info()
    }
    fn get_tool(&self, name: &str) -> Option<Tool> {
        tools::definitions()
            .ok()?
            .into_iter()
            .find(|t| t.name == name)
    }
    async fn discover(
        &self,
        context: RequestContext<RoleServer>,
    ) -> Result<DiscoverResult, ErrorData> {
        let bound = self.binding.clone();
        tokio::select! {
            biased;
            () = context.ct.cancelled() => return Err(ErrorData::internal_error("MCP request cancelled",None)),
            result = tokio::time::timeout_at(bound.deadline, bound.service.ensure_current()) => {
                result.map_err(|_| ErrorData::internal_error("Core connection timed out",None))??;
            },
        }
        Ok(DiscoverResult::from_server_info(
            vec![MCP_PROTOCOL_VERSION],
            ServerHandler::get_info(self),
        ))
    }
    async fn list_tools(
        &self,
        params: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let bound = self.binding.clone();
        let response = tokio::time::timeout_at(
            bound.deadline,
            bound.service.handle_request(
                ClientRequest::ListToolsRequest(RequestOptionalParam {
                    method: Default::default(),
                    params,
                    extensions: Default::default(),
                }),
                context,
            ),
        )
        .await
        .map_err(|_| ErrorData::internal_error("Core connection timed out", None))??;
        match response {
            ServerResult::ListToolsResult(value) => Ok(value),
            _ => Err(super::unsupported_method()),
        }
    }
    async fn call_tool(
        &self,
        params: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let bound = self.binding.clone();
        let writing = matches!(
            params.name.as_ref(),
            "alcomd_plan_package_uninstall"
                | "alcomd_apply_package_changes"
                | "alcomd_cancel_operation"
        );
        let response = tokio::time::timeout_at(
            bound.deadline,
            bound.service.handle_request(
                ClientRequest::CallToolRequest(Request::new(params)),
                context,
            ),
        )
        .await;
        match response {
            Err(_) => Ok(CallToolResponse::Complete(tools::interrupted(writing))),
            Ok(Ok(ServerResult::CallToolResult(value))) => Ok(CallToolResponse::Complete(value)),
            Ok(Err(error)) => Err(error),
            _ => Err(super::unsupported_method()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn truncated_output_is_a_transport_error_and_releases_the_request_slot() {
        let slots = Arc::new(Semaphore::new(1));
        let permit = slots.clone().try_acquire_owned().expect("test slot");
        let mut body = GuardedBody::new(
            Full::new(Bytes::from_static(b"oversize")).boxed(),
            Some(permit),
            None,
        );
        body.remaining = 3;
        assert_eq!(
            body.collect()
                .await
                .expect_err("cannot truncate into success")
                .kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(slots.available_permits(), 1);

        let permit = slots.clone().try_acquire_owned().expect("released slot");
        let mut body = GuardedBody::new(
            Full::new(Bytes::from_static(b"late")).boxed(),
            Some(permit),
            None,
        );
        body.deadline
            .as_mut()
            .reset(Instant::now() - Duration::from_millis(1));
        assert_eq!(
            body.collect()
                .await
                .expect_err("deadline cannot become clean EOF")
                .kind(),
            io::ErrorKind::TimedOut
        );
        assert_eq!(slots.available_permits(), 1);
    }

    #[tokio::test]
    async fn dropping_a_response_releases_the_request_slot() {
        let slots = Arc::new(Semaphore::new(1));
        let permit = slots.clone().try_acquire_owned().expect("test slot");
        let body = GuardedBody::new(
            Full::new(Bytes::from_static(b"response")).boxed(),
            Some(permit),
            None,
        );
        assert_eq!(slots.available_permits(), 0);
        drop(body);
        assert_eq!(slots.available_permits(), 1);
    }

    #[test]
    fn bearer_and_media_admission_never_accept_ambiguous_values() {
        let mut headers = hyper::HeaderMap::new();
        let token = "a".repeat(64);
        headers.insert(
            header::AUTHORIZATION,
            format!("bEaReR {token}").parse().expect("header"),
        );
        assert_eq!(bearer(&headers), Ok(Some(token)));
        headers.insert(
            header::AUTHORIZATION,
            format!("Bearer   {}", "b".repeat(64))
                .parse()
                .expect("HTTP auth spacing"),
        );
        assert_eq!(bearer(&headers), Ok(Some("b".repeat(64))));
        headers.append(
            header::AUTHORIZATION,
            "Bearer invalid".parse().expect("header"),
        );
        assert!(bearer(&headers).is_err());
        for accept in [
            "application/json",
            "application/json,text/event-stream;q=0",
            "*/*;q=NaN",
            "*/*;q=2",
        ] {
            headers.insert(header::ACCEPT, accept.parse().expect("header"));
            assert!(!accepts_both(&headers));
        }
        headers.insert(
            header::ACCEPT,
            "application/json,text/event-stream;q=0.5"
                .parse()
                .expect("header"),
        );
        assert!(accepts_both(&headers));
        headers.insert(
            header::CONTENT_TYPE,
            "application/json; charset=utf-8".parse().expect("header"),
        );
        assert!(json_content_type(&headers));
        headers.append(header::CONTENT_TYPE, "text/plain".parse().expect("header"));
        assert!(!json_content_type(&headers));
    }
}
