//! HTTPS OAuth facade. Core owns registration, consent, secrets and authorization.
use super::*;
use alcomd_protocol::*;
use hyper::Response;
use std::collections::BTreeMap;

pub(super) fn tls_config(
    cert: &std::path::Path,
    key: &std::path::Path,
) -> io::Result<rustls::ServerConfig> {
    fn invalid() -> io::Error {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "OAuth TLS requires a valid localhost DER certificate and matching PKCS8 key; configure client trust separately",
        )
    }
    fn read(path: &std::path::Path) -> io::Result<Vec<u8>> {
        use std::io::Read;
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .map_err(|_| invalid())?
            .take(65537)
            .read_to_end(&mut bytes)
            .map_err(|_| invalid())?;
        if bytes.is_empty() || bytes.len() > 65536 {
            return Err(invalid());
        }
        Ok(bytes)
    }
    let cert = rustls::pki_types::CertificateDer::from(read(cert)?);
    let parsed = rustls::server::ParsedCertificate::try_from(&cert).map_err(|_| invalid())?;
    rustls::client::verify_server_name(
        &parsed,
        &rustls::pki_types::ServerName::try_from("localhost").map_err(|_| invalid())?,
    )
    .map_err(|_| invalid())?;
    let key = rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(
        read(key)?,
    ));
    let mut config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::aws_lc_rs::default_provider(),
    ))
    .with_protocol_versions(&[&rustls::version::TLS13])
    .map_err(|_| invalid())?
    .with_no_client_auth()
    .with_single_cert(vec![cert], key)
    .map_err(|_| invalid())?;
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(config)
}

pub(super) struct Rate {
    start: Instant,
    total: u8,
    per_client: BTreeMap<String, u8>,
}
impl Default for Rate {
    fn default() -> Self {
        Self {
            start: Instant::now(),
            total: 0,
            per_client: BTreeMap::new(),
        }
    }
}
impl Rate {
    fn admit(&mut self, key: &str) -> bool {
        if self.start.elapsed() >= Duration::from_secs(60) {
            *self = Self::default()
        }
        if self.total >= 60 || (self.per_client.len() >= 64 && !self.per_client.contains_key(key)) {
            return false;
        }
        self.total += 1;
        let count = self.per_client.entry(key.to_string()).or_default();
        if *count >= 10 {
            return false;
        }
        *count += 1;
        true
    }
}
pub(super) fn tool_scopes(name: Option<&str>) -> &'static [&'static str] {
    match name {
        Some("alcomd_list_projects" | "alcomd_get_project") => &["projects.read"],
        Some("alcomd_list_package_sources" | "alcomd_search_packages" | "alcomd_get_package") => {
            &["repositories.read", "packages.read"]
        }
        Some(
            "alcomd_plan_package_uninstall"
            | "alcomd_plan_package_install"
            | "alcomd_plan_package_reinstall",
        ) => &["projects.read", "repositories.read", "packages.read"],
        Some("alcomd_apply_package_changes") => &[
            "projects.read",
            "repositories.read",
            "packages.read",
            "packages.manage",
        ],
        Some("alcomd_get_operation") => &["operations.read"],
        Some("alcomd_cancel_operation") => &["operations.cancel"],
        _ => &[],
    }
}
fn response(status: StatusCode, kind: &'static str, bytes: Vec<u8>) -> Response<GuardedBody> {
    let mut r = Response::new(GuardedBody::new(
        Full::new(Bytes::from(bytes)).boxed(),
        None,
        None,
    ));
    *r.status_mut() = status;
    for (name, value) in [
        (header::CONTENT_TYPE, kind),
        (header::CACHE_CONTROL, "no-store"),
        (header::REFERRER_POLICY, "no-referrer"),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        (
            header::CONTENT_SECURITY_POLICY,
            "default-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'",
        ),
    ] {
        r.headers_mut()
            .insert(name, header::HeaderValue::from_static(value));
    }
    r
}
fn json_response(status: StatusCode, value: Value) -> Response<GuardedBody> {
    response(
        status,
        "application/json",
        serde_json::to_vec(&value).expect("bounded OAuth JSON"),
    )
}
fn failure(status: StatusCode, tag: &'static str) -> Response<GuardedBody> {
    let mut r = json_response(status, json!({"error":tag}));
    if status == StatusCode::TOO_MANY_REQUESTS {
        r.headers_mut()
            .insert(header::RETRY_AFTER, header::HeaderValue::from_static("60"));
    }
    if tag == "invalid_client" {
        r.headers_mut().insert(
            header::WWW_AUTHENTICATE,
            header::HeaderValue::from_static("Basic realm=\"alcomd-oauth\""),
        );
    }
    r
}
fn token_outcome_unknown() -> Response<GuardedBody> {
    json_response(
        StatusCode::SERVICE_UNAVAILABLE,
        json!({"error":"temporarily_unavailable","error_description":"The token exchange outcome is unknown. Start authorization again; no automatic retry or reissue was made."}),
    )
}
pub(super) fn challenge(
    issuer: &str,
    status: StatusCode,
    error: Option<&'static str>,
    scopes: &[&str],
) -> Response<GuardedBody> {
    let mut r = json_response(
        status,
        json!({"error":error.unwrap_or("authorization_required")}),
    );
    let mut value =
        format!("Bearer resource_metadata=\"{issuer}/.well-known/oauth-protected-resource/mcp\"");
    if let Some(error) = error {
        value.push_str(&format!(", error=\"{error}\""))
    }
    if !scopes.is_empty() {
        value.push_str(&format!(", scope=\"{}\"", scopes.join(" ")))
    }
    if let Ok(value) = header::HeaderValue::from_str(&value) {
        r.headers_mut().insert(header::WWW_AUTHENTICATE, value);
    }
    r
}
fn from_core(error: ClientError) -> Response<GuardedBody> {
    let ClientError::Remote(error) = error else {
        return failure(StatusCode::SERVICE_UNAVAILABLE, "temporarily_unavailable");
    };
    let tag = error
        .data
        .as_ref()
        .and_then(|v| v["oauthError"].as_str())
        .unwrap_or("temporarily_unavailable");
    match tag {
        "invalid_client" => failure(StatusCode::UNAUTHORIZED, "invalid_client"),
        "invalid_grant" => failure(StatusCode::BAD_REQUEST, "invalid_grant"),
        "invalid_scope" => failure(StatusCode::BAD_REQUEST, "invalid_scope"),
        "invalid_target" => failure(StatusCode::BAD_REQUEST, "invalid_target"),
        "invalid_request" => failure(StatusCode::BAD_REQUEST, "invalid_request"),
        "access_denied" => failure(StatusCode::FORBIDDEN, "access_denied"),
        _ => failure(StatusCode::SERVICE_UNAVAILABLE, "temporarily_unavailable"),
    }
}
fn id(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
            }
        })
}
fn decode(value: &str) -> Result<String, ()> {
    let mut out = Vec::with_capacity(value.len());
    let mut iter = value.bytes();
    while let Some(b) = iter.next() {
        match b {
            b'+' => out.push(b' '),
            b'%' => {
                let a = (iter.next().ok_or(())? as char).to_digit(16).ok_or(())?;
                let b = (iter.next().ok_or(())? as char).to_digit(16).ok_or(())?;
                out.push((a * 16 + b) as u8)
            }
            b => out.push(b),
        }
    }
    let text = String::from_utf8(out).map_err(|_| ())?;
    if text.chars().any(char::is_control) {
        return Err(());
    }
    Ok(text)
}
fn form(text: &str) -> Result<BTreeMap<String, String>, ()> {
    if text.len() > 16384 {
        return Err(());
    }
    let mut result = BTreeMap::new();
    if text.is_empty() {
        return Ok(result);
    }
    for pair in text.split('&') {
        let (key, value) = pair.split_once('=').ok_or(())?;
        let key = decode(key)?;
        let value = decode(value)?;
        if key.is_empty() || result.insert(key, value).is_some() {
            return Err(());
        }
    }
    Ok(result)
}
fn required(values: &BTreeMap<String, String>, name: &str) -> Result<String, ()> {
    values
        .get(name)
        .filter(|v| !v.is_empty())
        .cloned()
        .ok_or(())
}
fn scope(text: Option<&String>) -> Vec<String> {
    text.map(|s| s.split_ascii_whitespace().map(str::to_string).collect())
        .unwrap_or_else(|| vec!["projects.read".into()])
}

pub(super) fn access_bearer(headers: &hyper::HeaderMap) -> Result<Option<String>, ()> {
    let Some(value) = unique(headers, header::AUTHORIZATION.as_str())? else {
        return Ok(None);
    };
    let mut parts = value.split_ascii_whitespace();
    let scheme = parts.next().ok_or(())?;
    let token = parts.next().ok_or(())?;
    if !scheme.eq_ignore_ascii_case("Bearer")
        || parts.next().is_some()
        || token.len() > 256
        || !token.bytes().all(|b| b.is_ascii_graphic())
    {
        return Err(());
    }
    Ok(Some(token.into()))
}
/// Bounded RFC4648 decoder for the sole Basic header field; no new codec dependency.
fn basic64(value: &str) -> Result<Vec<u8>, ()> {
    if value.len() > 1024 || value.is_empty() || !value.len().is_multiple_of(4) {
        return Err(());
    }
    fn digit(b: u8) -> Result<u32, ()> {
        match b {
            b'A'..=b'Z' => Ok((b - b'A') as u32),
            b'a'..=b'z' => Ok((b - b'a' + 26) as u32),
            b'0'..=b'9' => Ok((b - b'0' + 52) as u32),
            b'+' => Ok(62),
            b'/' => Ok(63),
            _ => Err(()),
        }
    }
    let mut out = Vec::new();
    let count = value.len() / 4;
    for (i, c) in value.as_bytes().chunks_exact(4).enumerate() {
        let pad = if c[2] == b'=' {
            if c[3] != b'=' {
                return Err(());
            }
            2
        } else if c[3] == b'=' {
            1
        } else {
            0
        };
        if pad > 0 && i + 1 != count {
            return Err(());
        }
        let a = digit(c[0])?;
        let b = digit(c[1])?;
        let d = if pad == 2 { 0 } else { digit(c[2])? };
        let e = if pad > 0 { 0 } else { digit(c[3])? };
        if (pad == 2 && b & 15 != 0) || (pad == 1 && d & 3 != 0) {
            return Err(());
        }
        let n = (a << 18) | (b << 12) | (d << 6) | e;
        out.push((n >> 16) as u8);
        if pad < 2 {
            out.push((n >> 8) as u8)
        }
        if pad == 0 {
            out.push(n as u8)
        }
    }
    Ok(out)
}
fn client_auth(
    headers: &hyper::HeaderMap,
    values: &BTreeMap<String, String>,
) -> Result<(String, Option<String>), ()> {
    if values.contains_key("client_secret") {
        return Err(());
    }
    let Some(header) = unique(headers, header::AUTHORIZATION.as_str())? else {
        return Ok((required(values, "client_id")?, None));
    };
    let mut parts = header.split_ascii_whitespace();
    let scheme = parts.next().ok_or(())?;
    let encoded = parts.next().ok_or(())?;
    if !scheme.eq_ignore_ascii_case("Basic") || parts.next().is_some() {
        return Err(());
    }
    let bytes = basic64(encoded)?;
    let raw = std::str::from_utf8(&bytes).map_err(|_| ())?;
    let (client, secret) = raw.split_once(':').ok_or(())?;
    let client = decode(client)?;
    let secret = decode(secret)?;
    if values.get("client_id").is_some_and(|v| v != &client)
        || client.is_empty()
        || secret.is_empty()
    {
        return Err(());
    }
    Ok((client, Some(secret)))
}
fn waiting(request: &str) -> Response<GuardedBody> {
    // request is Core-generated, checked canonical UUID; never reflect arbitrary input.
    let html = format!(
        "<!DOCTYPE html><html lang=\"zh-CN\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\"><title>MCP 授权确认</title></head><body><main><h1>等待本机授权</h1><p>在本机 CLI 查看并确认本次请求，再继续。</p><p>查看命令：<code>alcomd-cli mcp oauth request show {request}</code></p><p>请求标识：<code>{request}</code></p><p><a href=\"/authorize/result?request_id={request}\">已完成本机确认，继续连接</a></p></main></body></html>"
    );
    response(
        StatusCode::OK,
        "text/html; charset=utf-8",
        html.into_bytes(),
    )
}
async fn endpoint(state: &State) -> Result<AlcomdClient, Response<GuardedBody>> {
    match tokio::time::timeout(
        CORE_TIME,
        AlcomdClient::connect(
            state
                .config
                .clone()
                .with_client_name("alcomd-mcp")
                .with_oauth_endpoint(),
        ),
    )
    .await
    {
        Ok(Ok(client)) => Ok(client),
        _ => Err(failure(
            StatusCode::SERVICE_UNAVAILABLE,
            "temporarily_unavailable",
        )),
    }
}

pub(super) async fn handle(
    state: &State,
    issuer: &str,
    parts: hyper::http::request::Parts,
    body: Incoming,
) -> Response<GuardedBody> {
    let origin = match unique(&parts.headers, header::ORIGIN.as_str()) {
        Ok(v) => v,
        Err(()) => return failure(StatusCode::BAD_REQUEST, "invalid_request"),
    };
    if origin.is_some_and(|o| {
        o != issuer && !(issuer == "https://localhost:443" && o == "https://localhost")
    }) {
        return failure(StatusCode::FORBIDDEN, "access_denied");
    }
    if parts.headers.contains_key("mcp-session-id") || parts.headers.contains_key("last-event-id") {
        return failure(StatusCode::BAD_REQUEST, "invalid_request");
    }
    let path = parts.uri.path();
    let get = matches!(
        path,
        "/.well-known/oauth-protected-resource/mcp"
            | "/.well-known/oauth-authorization-server"
            | "/authorize"
            | "/authorize/result"
    );
    let post = matches!(path, "/token" | "/revoke");
    if !get && !post {
        return failure(StatusCode::NOT_FOUND, "invalid_request");
    }
    if (get && parts.method != hyper::Method::GET) || (post && parts.method != hyper::Method::POST)
    {
        let mut r = failure(StatusCode::METHOD_NOT_ALLOWED, "invalid_request");
        r.headers_mut().insert(
            header::ALLOW,
            header::HeaderValue::from_static(if get { "GET" } else { "POST" }),
        );
        return r;
    }
    let Ok(_permit) = state.requests.clone().try_acquire_owned() else {
        return failure(StatusCode::TOO_MANY_REQUESTS, "temporarily_unavailable");
    };
    if path.starts_with("/.well-known/") {
        if parts.uri.query().is_some() {
            return failure(StatusCode::BAD_REQUEST, "invalid_request");
        }
        return if path.ends_with("/mcp") {
            json_response(
                StatusCode::OK,
                json!({"resource":format!("{issuer}/mcp"),"authorization_servers":[issuer],"bearer_methods_supported":["header"],"scopes_supported":["projects.read"]}),
            )
        } else {
            json_response(
                StatusCode::OK,
                json!({"issuer":issuer,"authorization_endpoint":format!("{issuer}/authorize"),"token_endpoint":format!("{issuer}/token"),"revocation_endpoint":format!("{issuer}/revoke"),"response_types_supported":["code"],"grant_types_supported":["authorization_code","refresh_token"],"code_challenge_methods_supported":["S256"],"token_endpoint_auth_methods_supported":["none","client_secret_basic"],"revocation_endpoint_auth_methods_supported":["none","client_secret_basic"],"authorization_response_iss_parameter_supported":true,"scopes_supported":["projects.read","packages.read","repositories.read","packages.manage","operations.read","operations.cancel","state.check","events.read"]}),
            )
        };
    }
    let text = if get {
        parts.uri.query().unwrap_or_default().to_string()
    } else {
        if parts.uri.query().is_some() {
            return failure(StatusCode::BAD_REQUEST, "invalid_request");
        }
        let media = unique(&parts.headers, header::CONTENT_TYPE.as_str())
            .ok()
            .flatten();
        if !media.is_some_and(|v| {
            v.eq_ignore_ascii_case("application/x-www-form-urlencoded")
                || v.eq_ignore_ascii_case("application/x-www-form-urlencoded; charset=utf-8")
        }) {
            return failure(StatusCode::UNSUPPORTED_MEDIA_TYPE, "invalid_request");
        }
        if parts.headers.contains_key(header::CONTENT_ENCODING)
            && unique(&parts.headers, header::CONTENT_ENCODING.as_str()) != Ok(Some("identity"))
        {
            return failure(StatusCode::UNSUPPORTED_MEDIA_TYPE, "invalid_request");
        }
        let bytes = match tokio::time::timeout(BODY_TIME, Limited::new(body, 16384).collect()).await
        {
            Ok(Ok(data)) => data.to_bytes(),
            Ok(Err(e)) if e.is::<http_body_util::LengthLimitError>() => {
                return failure(StatusCode::PAYLOAD_TOO_LARGE, "invalid_request");
            }
            Err(_) => return failure(StatusCode::REQUEST_TIMEOUT, "invalid_request"),
            _ => return failure(StatusCode::BAD_REQUEST, "invalid_request"),
        };
        match String::from_utf8(bytes.to_vec()) {
            Ok(s) => s,
            Err(_) => return failure(StatusCode::BAD_REQUEST, "invalid_request"),
        }
    };
    let values = match form(&text) {
        Ok(v) => v,
        Err(()) => return failure(StatusCode::BAD_REQUEST, "invalid_request"),
    };
    let rate_key = if path == "/authorize/result" {
        "__result__"
    } else {
        values
            .get("client_id")
            .filter(|s| id(s))
            .map(String::as_str)
            .unwrap_or("__invalid__")
    };
    if !state.oauth_rate.lock().await.admit(rate_key) {
        return failure(StatusCode::TOO_MANY_REQUESTS, "temporarily_unavailable");
    }
    let mut client = match endpoint(state).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let invoke = async {
        match path {
            "/authorize" => {
                let input = (|| -> Result<_, ()> {
                    if required(&values, "response_type")? != "code"
                        || values.keys().any(|k| {
                            ![
                                "client_id",
                                "response_type",
                                "redirect_uri",
                                "code_challenge",
                                "code_challenge_method",
                                "resource",
                                "scope",
                                "state",
                            ]
                            .contains(&k.as_str())
                        })
                    {
                        return Err(());
                    }
                    Ok(McpOAuthBeginParams {
                        oauth_client_id: required(&values, "client_id")?,
                        issuer: issuer.into(),
                        resource: required(&values, "resource")?,
                        redirect_uri: required(&values, "redirect_uri")?,
                        code_challenge: required(&values, "code_challenge")?,
                        code_challenge_method: required(&values, "code_challenge_method")?,
                        scopes: scope(values.get("scope")),
                        state: values.get("state").cloned(),
                    })
                })();
                let input = match input {
                    Ok(i) => i,
                    Err(()) => return failure(StatusCode::BAD_REQUEST, "invalid_request"),
                };
                let result = match client.oauth_begin(input).await {
                    Ok(r) => r,
                    Err(e) => return from_core(e),
                };
                if !id(&result.request_id)
                    || result.browser_secret.len() != 64
                    || !result.browser_secret.bytes().all(|b| b.is_ascii_hexdigit())
                {
                    return failure(StatusCode::SERVICE_UNAVAILABLE, "temporarily_unavailable");
                }
                let mut r = waiting(&result.request_id);
                let cookie = format!(
                    "__Host-alcomd-oauth-{}={}; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=600",
                    result.request_id, result.browser_secret
                );
                match header::HeaderValue::from_str(&cookie) {
                    Ok(value) => {
                        r.headers_mut().insert(header::SET_COOKIE, value);
                        r
                    }
                    Err(_) => failure(StatusCode::SERVICE_UNAVAILABLE, "temporarily_unavailable"),
                }
            }
            "/authorize/result" => {
                let request = match required(&values, "request_id") {
                    Ok(s) if id(&s) && values.len() == 1 => s,
                    _ => return failure(StatusCode::BAD_REQUEST, "invalid_request"),
                };
                let name = format!("__Host-alcomd-oauth-{request}");
                let cookie = match unique(&parts.headers, header::COOKIE.as_str()) {
                    Ok(Some(s)) => s,
                    _ => return failure(StatusCode::FORBIDDEN, "access_denied"),
                };
                let matches: Vec<_> = cookie
                    .split(';')
                    .filter_map(|p| p.trim().split_once('='))
                    .filter(|(k, _)| *k == name)
                    .collect();
                if matches.len() != 1 || matches[0].1.len() != 64 {
                    return failure(StatusCode::FORBIDDEN, "access_denied");
                }
                let result = match client
                    .oauth_finish(McpOAuthFinishParams {
                        request_id: request.clone(),
                        browser_secret: matches[0].1.into(),
                    })
                    .await
                {
                    Ok(r) => r,
                    Err(e) => return from_core(e),
                };
                if result.status == "pending" && result.redirect_url.is_none() {
                    return waiting(&request);
                }
                let Some(location) = result.redirect_url.filter(|s| s.len() <= 16384) else {
                    return failure(StatusCode::SERVICE_UNAVAILABLE, "temporarily_unavailable");
                };
                let Ok(location) = header::HeaderValue::from_str(&location) else {
                    return failure(StatusCode::SERVICE_UNAVAILABLE, "temporarily_unavailable");
                };
                let mut r = response(
                    StatusCode::SEE_OTHER,
                    "text/plain; charset=utf-8",
                    b"Authorization response ready.".to_vec(),
                );
                r.headers_mut().insert(header::LOCATION, location);
                if let Ok(cookie) = header::HeaderValue::from_str(&format!(
                    "{name}=; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=0"
                )) {
                    r.headers_mut().insert(header::SET_COOKIE, cookie);
                }
                r
            }
            "/token" | "/revoke" => {
                let (id, secret) = match client_auth(&parts.headers, &values) {
                    Ok(v) => v,
                    Err(()) => return failure(StatusCode::UNAUTHORIZED, "invalid_client"),
                };
                if path == "/revoke" {
                    if values
                        .keys()
                        .any(|k| !["client_id", "token", "token_type_hint"].contains(&k.as_str()))
                    {
                        return failure(StatusCode::BAD_REQUEST, "invalid_request");
                    }
                    let token = match required(&values, "token") {
                        Ok(s) => s,
                        Err(()) => return failure(StatusCode::BAD_REQUEST, "invalid_request"),
                    };
                    return match client
                        .oauth_revoke(McpOAuthRevokeParams {
                            oauth_client_id: id,
                            token,
                            client_secret: secret,
                            token_type_hint: values.get("token_type_hint").cloned(),
                        })
                        .await
                    {
                        Ok(_) => json_response(StatusCode::OK, json!({})),
                        Err(e) => from_core(e),
                    };
                }
                if values.keys().any(|k| {
                    ![
                        "client_id",
                        "grant_type",
                        "code",
                        "redirect_uri",
                        "code_verifier",
                        "refresh_token",
                        "resource",
                        "scope",
                    ]
                    .contains(&k.as_str())
                }) {
                    return failure(StatusCode::BAD_REQUEST, "invalid_request");
                }
                let grant = match required(&values, "grant_type") {
                    Ok(s) => s,
                    Err(()) => return failure(StatusCode::BAD_REQUEST, "invalid_request"),
                };
                if !matches!(grant.as_str(), "authorization_code" | "refresh_token") {
                    return failure(StatusCode::BAD_REQUEST, "unsupported_grant_type");
                }
                let resource = match required(&values, "resource") {
                    Ok(s) => s,
                    Err(()) => return failure(StatusCode::BAD_REQUEST, "invalid_target"),
                };
                if resource != format!("{issuer}/mcp") {
                    return failure(StatusCode::BAD_REQUEST, "invalid_target");
                }
                let result = client
                    .oauth_token(McpOAuthTokenParams {
                        grant_type: grant,
                        oauth_client_id: id,
                        resource,
                        code: values.get("code").cloned(),
                        redirect_uri: values.get("redirect_uri").cloned(),
                        code_verifier: values.get("code_verifier").cloned(),
                        refresh_token: values.get("refresh_token").cloned(),
                        scopes: values.get("scope").map(|s| scope(Some(s))),
                        client_secret: secret,
                    })
                    .await;
                match result {
                    Ok(r) => {
                        let mut value = json!({"access_token":r.access_token,"token_type":r.token_type,"expires_in":r.expires_in,"scope":r.scope});
                        if let Some(refresh) = r.refresh_token {
                            value["refresh_token"] = json!(refresh)
                        }
                        json_response(StatusCode::OK, value)
                    }
                    Err(ClientError::Remote(error))
                        if !matches!(
                            error.code.as_str(),
                            "store_unavailable" | "internal_error"
                        ) =>
                    {
                        from_core(ClientError::Remote(error))
                    }
                    Err(_) => token_outcome_unknown(),
                }
            }
            _ => failure(StatusCode::NOT_FOUND, "invalid_request"),
        }
    };
    tokio::time::timeout(CORE_TIME, invoke)
        .await
        .unwrap_or_else(|_| {
            if path == "/token" {
                token_outcome_unknown()
            } else {
                failure(StatusCode::SERVICE_UNAVAILABLE, "temporarily_unavailable")
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn form_rejects_duplicate_encoded_keys_invalid_utf8_and_controls() {
        for input in ["a=1&%61=2", "x=%zz", "x=%FF", "x=%0A", "missing"] {
            assert!(form(input).is_err(), "{input}")
        }
        assert_eq!(form("state=a%2Bb+c").expect("form")["state"], "a+b c");
    }
    #[test]
    fn basic_decoding_is_strict_and_accepts_registered_ascii_credentials() {
        assert_eq!(
            basic64("Y2xpZW50OnNlY3JldA==").expect("base64"),
            b"client:secret"
        );
        for input in ["A===", "YQ=Z", "YQ==AAAA", "YR==", "YQ", "YQ==\n"] {
            assert!(basic64(input).is_err(), "{input}")
        }
    }
    #[test]
    fn rate_is_bounded_and_cancel_challenge_lists_all_permissions() {
        let mut r = Rate::default();
        for _ in 0..10 {
            assert!(r.admit("same"))
        }
        assert!(!r.admit("same"));
        assert_eq!(
            tool_scopes(Some("alcomd_cancel_operation")),
            &["operations.cancel"]
        );
        assert_eq!(
            tool_scopes(Some("alcomd_get_operation")),
            &["operations.read"]
        );
    }
}
