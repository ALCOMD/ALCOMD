use super::*;
use rmcp::model::{ClientRequest, Request};
use rmcp::service::PeerRequestOptions;
use rmcp::transport::{
    StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
};

const CERT: &[u8] = include_bytes!(
    "../../../../crates/alcomd-testing/fixtures/m8/oauth-localhost.test-certificate.der"
);
const KEY: &[u8] =
    include_bytes!("../../../../crates/alcomd-testing/fixtures/m8/oauth-localhost.test-key.pkcs8");
const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
struct CoreConfiguration {
    ipc: IpcConfig,
    client: ClientConfig,
    #[cfg(unix)]
    runtime: std::path::PathBuf,
}
impl CoreConfiguration {
    fn new() -> Self {
        #[cfg(unix)]
        {
            // Reuse the CLI fixture's private, short Unix endpoint convention.
            #[cfg(target_os = "macos")]
            let base = std::path::PathBuf::from("/private/tmp");
            #[cfg(not(target_os = "macos"))]
            let base = std::env::temp_dir();
            let runtime = base.join(format!(
                "ac-oauth-{}-{}",
                std::process::id(),
                uuid::Uuid::new_v4().simple()
            ));
            assert!(!runtime.exists(), "new owned runtime path");
            Self {
                ipc: IpcConfig::isolated(runtime.clone()),
                client: ClientConfig::default()
                    .without_daemon_start()
                    .with_runtime_directory(runtime.clone()),
                runtime,
            }
        }
        #[cfg(windows)]
        {
            // Windows keeps its unchanged, protected per-user endpoint.
            Self {
                ipc: IpcConfig::default(),
                client: ClientConfig::default().without_daemon_start(),
            }
        }
    }
}
#[cfg(unix)]
impl Drop for CoreConfiguration {
    fn drop(&mut self) {
        if !self.runtime.exists() {
            return;
        }
        assert!(
            !self
                .runtime
                .symlink_metadata()
                .expect("owned runtime metadata")
                .file_type()
                .is_symlink()
        );
        let path = self.runtime.canonicalize().expect("owned runtime");
        let parent = self
            .runtime
            .parent()
            .expect("runtime parent")
            .canonicalize()
            .expect("runtime base");
        assert_eq!(path.parent(), Some(parent.as_path()));
        assert!(
            path.file_name()
                .expect("runtime name")
                .to_string_lossy()
                .starts_with(&format!("ac-oauth-{}-", std::process::id()))
        );
        std::fs::remove_dir_all(path).expect("remove only owned private runtime");
    }
}
struct Https {
    issuer: String,
    client: reqwest::Client,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<std::io::Result<()>>,
}
impl Drop for Https {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Https {
    async fn start(root: &Path, config: ClientConfig) -> Self {
        let cert = root.join("public-test-cert.der");
        let key = root.join("public-test-key.pkcs8");
        std::fs::write(&cert, CERT).expect("test cert");
        std::fs::write(&key, KEY).expect("public test key");
        let mut bound = None;
        for _ in 0..4 {
            let reservation = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
                .expect("free local port");
            let port = reservation.local_addr().expect("port").port();
            drop(reservation);
            match alcomd_mcp::HttpServer::bind_oauth(
                port,
                config.clone(),
                cert.clone(),
                key.clone(),
            )
            .await
            {
                Ok(server) => {
                    bound = Some((server, port));
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {}
                Err(_) => panic!("valid isolated TLS configuration must bind"),
            }
        }
        let (server, port) = bound.expect("exclusive free local listener");
        let issuer = format!("https://localhost:{port}");
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .add_root_certificate(
                reqwest::Certificate::from_der(CERT).expect("public fixture certificate"),
            )
            .resolve(
                "localhost",
                std::net::SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, port)),
            )
            .build()
            .expect("client with isolated explicit trust");
        let (stop, stopped) = tokio::sync::oneshot::channel();
        Self {
            issuer,
            client,
            stop: Some(stop),
            task: tokio::spawn(server.run_until(async {
                let _ = stopped.await;
            })),
        }
    }
    fn resource(&self) -> String {
        format!("{}/mcp", self.issuer)
    }
    async fn sdk(&self, token: &str) -> Sdk {
        let transport = StreamableHttpClientTransport::with_client(
            self.client.clone(),
            StreamableHttpClientTransportConfig::with_uri(self.resource()).auth_header(token),
        );
        ().serve_with_lifecycle(
            transport,
            ClientLifecycleMode::Discover {
                preferred_versions: vec![ProtocolVersion::V_2026_07_28],
            },
        )
        .await
        .expect("frozen real SDK over verified TLS")
    }
    async fn start_request(&self, reg: &McpOAuthRegistration) -> (String, String) {
        let mut url = reqwest::Url::parse(&format!("{}/authorize", self.issuer)).expect("URL");
        url.query_pairs_mut().extend_pairs([("client_id",reg.oauth_client_id.as_str()),("response_type","code"),("redirect_uri",reg.redirect_uris[0].as_str()),("code_challenge",CHALLENGE),("code_challenge_method","S256"),("resource",reg.resource.as_str()),("scope","projects.read repositories.read packages.read packages.manage operations.read operations.cancel"),("state","isolated-state")]);
        let response = self.client.get(url).send().await.expect("authorize HTTP");
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["referrer-policy"], "no-referrer");
        let cookie = response.headers()["set-cookie"]
            .to_str()
            .expect("cookie")
            .split(';')
            .next()
            .expect("cookie pair")
            .to_string();
        let request = cookie
            .split_once('=')
            .expect("pair")
            .0
            .strip_prefix("__Host-alcomd-oauth-")
            .expect("host cookie")
            .to_string();
        let page = response.text().await.expect("waiting document");
        assert!(page.contains("<main>") && page.contains(&request));
        assert!(!page.contains(cookie.split_once('=').expect("secret").1));
        (request, cookie)
    }
    async fn finish(&self, request: &str, cookie: &str) -> reqwest::Response {
        self.client
            .get(format!(
                "{}/authorize/result?request_id={request}",
                self.issuer
            ))
            .header("Cookie", cookie)
            .send()
            .await
            .expect("finish HTTP")
    }
    async fn code(
        &self,
        owner: &mut AlcomdClient,
        reg: &McpOAuthRegistration,
        allowed: &[McpClientGrant],
    ) -> String {
        let (request, cookie) = self.start_request(reg).await;
        let first = owner
            .oauth_request_get(McpOAuthRequestGetParams {
                request_id: request.clone(),
                cursor: None,
                limit: Some(1),
            })
            .await
            .expect("first review page");
        let mut all = first.grants;
        let mut cursor = first.next_cursor;
        while let Some(c) = cursor {
            let next = owner
                .oauth_request_get(McpOAuthRequestGetParams {
                    request_id: request.clone(),
                    cursor: Some(c),
                    limit: Some(1),
                })
                .await
                .expect("next consistent page");
            assert_eq!(next.approval_fingerprint, first.approval_fingerprint);
            all.extend(next.grants);
            cursor = next.next_cursor;
        }
        assert!(allowed.iter().all(|g| all.contains(g)));
        owner
            .oauth_request_decide(McpOAuthRequestDecideParams {
                request_id: request.clone(),
                approve: true,
                grants: allowed.to_vec(),
                expected_fingerprint: first.approval_fingerprint,
                idempotency_key: format!("consent-{request}"),
            })
            .await
            .expect("real explicit owner consent");
        let response = self.finish(&request, &cookie).await;
        assert_eq!(response.status(), 303);
        let url = reqwest::Url::parse(response.headers()["location"].to_str().expect("Location"))
            .expect("validated callback");
        assert!(
            url.query_pairs()
                .any(|(k, v)| k == "iss" && v == self.issuer)
        );
        assert!(
            url.query_pairs()
                .any(|(k, v)| k == "state" && v == "isolated-state")
        );
        url.query_pairs()
            .find(|(k, _)| k == "code")
            .expect("one-time code")
            .1
            .into_owned()
    }
    async fn token(
        &self,
        reg: &McpOAuthRegistration,
        fields: &[(&str, &str)],
        secret: Option<&str>,
    ) -> reqwest::Response {
        let mut form = reqwest::Url::parse("https://localhost/").expect("serialization base");
        form.query_pairs_mut()
            .append_pair("client_id", &reg.oauth_client_id)
            .extend_pairs(fields.iter().copied());
        let mut request = self
            .client
            .post(format!("{}/token", self.issuer))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(form.query().expect("form body").to_string());
        if let Some(secret) = secret {
            request = request.basic_auth(&reg.oauth_client_id, Some(secret));
        }
        request.send().await.expect("standard token HTTP")
    }
    async fn exchange(
        &self,
        reg: &McpOAuthRegistration,
        code: &str,
        secret: Option<&str>,
    ) -> Value {
        let response = self
            .token(
                reg,
                &[
                    ("grant_type", "authorization_code"),
                    ("code", code),
                    ("redirect_uri", &reg.redirect_uris[0]),
                    ("code_verifier", VERIFIER),
                    ("resource", &reg.resource),
                ],
                secret,
            )
            .await;
        assert_eq!(response.status(), 200, "real OAuth exchange");
        response.json().await.expect("token JSON")
    }
    async fn finish_server(mut self) {
        self.stop
            .take()
            .expect("stop sender")
            .send(())
            .expect("stop HTTP");
        tokio::time::timeout(Duration::from_secs(6), &mut self.task)
            .await
            .expect("stop deadline")
            .expect("join")
            .expect("stop");
    }
}
async fn registration(
    owner: &mut AlcomdClient,
    http: &Https,
    client: &McpClientRecord,
    confidential: bool,
) -> McpOAuthRegistrationResult {
    owner
        .oauth_registration_set(McpOAuthRegistrationSetParams {
            client_id: client.client_id.clone(),
            issuer: http.issuer.clone(),
            resource: http.resource(),
            redirect_uris: vec!["http://127.0.0.1:3456/callback".into()],
            confidential,
            allow_refresh: true,
            expected_revision: 0,
            idempotency_key: format!("register-{}", client.client_id),
        })
        .await
        .expect("real OAuth registration")
}
async fn raw_tool(http: &Https, token: &str, name: &str, args: Value) -> reqwest::Response {
    http.client.post(http.resource()).bearer_auth(token).header("Content-Type","application/json").header("Accept","application/json,text/event-stream").header("MCP-Protocol-Version","2026-07-28").header("Mcp-Method","tools/call").header("Mcp-Name",name)
        .json(&json!({"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":name,"arguments":args,"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}})).send().await.expect("raw frozen HTTP tool")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn oauth_project_cursor_is_bound_to_token_snapshot_and_position() {
    let _serial = REAL_CORE_LOCK.lock().await;
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/m8-oauth-cursor-real")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(root.join("data")).expect("disposable data");
    let root = root.canonicalize().expect("isolated root");
    let core = CoreConfiguration::new();
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        core.ipc.clone(),
        DataConfig::isolated(root.join("data")),
        std::future::pending(),
    )
    .await
    .expect("exclusive isolated Core");
    let mut owner = AlcomdClient::connect(core.client.clone())
        .await
        .expect("owner");
    let allowed = vec![
        grant("projects.read", "global", "*"),
        grant("operations.read", "global", "*"),
    ];
    for name in ["First", "Second", "Third"] {
        let path = root.join(name);
        project(&path, false);
        owner
            .project_register(
                path.to_string_lossy().into_owned(),
                format!("register-{name}"),
            )
            .await
            .expect("project")
            .project
            .project_id
            .expect("id");
    }
    let native = owner
        .projects_list(None, Some(1))
        .await
        .expect("native page");
    let native_cursor = native.next_cursor.expect("native next");
    assert!(native_cursor.authorization_binding.is_none());
    assert!(
        serde_json::to_value(&native_cursor)
            .expect("native wire")
            .get("authorizationBinding")
            .is_none()
    );
    assert!(
        owner
            .projects_list(Some(native_cursor), Some(1))
            .await
            .is_ok()
    );
    let mut a = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "Cursor A".into(),
            grants: allowed.clone(),
            idempotency_key: "cursor-a".into(),
        })
        .await
        .expect("A");
    let b = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "Cursor B".into(),
            grants: allowed.clone(),
            idempotency_key: "cursor-b".into(),
        })
        .await
        .expect("B");
    let http = Https::start(&root, core.client.clone()).await;
    let reg_a = registration(&mut owner, &http, &a, false)
        .await
        .registration;
    let reg_b = registration(&mut owner, &http, &b, false)
        .await
        .registration;
    let code = http.code(&mut owner, &reg_a, &allowed[..1]).await;
    let issued = http.exchange(&reg_a, &code, None).await;
    let access = issued["access_token"].as_str().expect("first access");
    let code = http.code(&mut owner, &reg_a, &allowed).await;
    let other_issued = http.exchange(&reg_a, &code, None).await;
    let other_access = other_issued["access_token"]
        .as_str()
        .expect("same principal second token");
    let code = http.code(&mut owner, &reg_b, &allowed).await;
    let b_issued = http.exchange(&reg_b, &code, None).await;
    let b_access = b_issued["access_token"].as_str().expect("B token");
    let sa = http.sdk(access).await;
    let sa2 = http.sdk(other_access).await;
    let sb = http.sdk(b_access).await;
    let mut trace = vec![];
    let first = data(
        call(&sa, &mut trace, "list_projects", json!({"limit":1}))
            .await
            .expect("first page"),
    );
    let cursor = first["next_cursor"].clone();
    let binding = cursor["authorization_binding"].as_str().expect("Core MAC");
    assert_eq!(binding.len(), 64);
    assert!(
        binding
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    );
    let next = data(
        call(
            &sa,
            &mut trace,
            "list_projects",
            json!({"limit":1,"cursor":cursor}),
        )
        .await
        .expect("bound continuation"),
    );
    assert_eq!(next["projects"].as_array().expect("next projects").len(), 1);
    assert_ne!(
        first["projects"][0]["project_id"],
        next["projects"][0]["project_id"]
    );
    let mut missing = cursor.clone();
    missing
        .as_object_mut()
        .expect("cursor")
        .remove("authorization_binding");
    let mut forged = cursor.clone();
    forged["authorization_binding"] = json!(if let Some(rest) = binding.strip_prefix('0') {
        format!("1{rest}")
    } else {
        format!("0{}", &binding[1..])
    });
    let mut timestamp = cursor.clone();
    timestamp["registered_at_ms"] = json!(cursor["registered_at_ms"].as_u64().expect("time") + 1);
    let mut position = cursor.clone();
    position["project_id"] = next["projects"][0]["project_id"].clone();
    for invalid in [missing, forged, timestamp, position] {
        error(
            call(
                &sa,
                &mut trace,
                "list_projects",
                json!({"limit":1,"cursor":invalid}),
            )
            .await
            .expect("bounded rejection"),
            "revision_conflict",
        );
    }
    for client in [&sa2, &sb] {
        error(
            call(
                client,
                &mut trace,
                "list_projects",
                json!({"limit":1,"cursor":cursor}),
            )
            .await
            .expect("cross token rejected"),
            "revision_conflict",
        );
    }
    let mut direct = AlcomdClient::connect(
        core.client
            .clone()
            .with_oauth_access(access.into(), http.resource()),
    )
    .await
    .expect("typed authenticated Core");
    let mut wire = ProjectRegistryCursor {
        registered_at_ms: cursor["registered_at_ms"].as_u64().expect("time"),
        id: cursor["project_id"].as_str().expect("id").into(),
        authorization_binding: Some(binding.into()),
    };
    assert!(
        direct
            .projects_list(Some(wire.clone()), Some(1))
            .await
            .is_ok()
    );
    wire.authorization_binding = None;
    assert!(
        matches!(direct.projects_list(Some(wire),Some(1)).await,Err(alcomd_client::ClientError::Remote(ref e)) if e.code=="revision_conflict"),
        "no adapter-only enforcement"
    );
    let mut changed = allowed.clone();
    changed.push(grant("state.check", "global", "*"));
    a = update(&mut owner, &a, changed, "cursor-grants-changed").await;
    error(
        call(
            &sa,
            &mut trace,
            "list_projects",
            json!({"limit":1,"cursor":cursor}),
        )
        .await
        .expect("stale authorization snapshot"),
        "revision_conflict",
    );
    let restarted = data(
        call(&sa, &mut trace, "list_projects", json!({"limit":1}))
            .await
            .expect("restart on current grant snapshot"),
    );
    assert_ne!(
        restarted["next_cursor"]["authorization_binding"],
        cursor["authorization_binding"]
    );
    assert!(
        call(
            &sa,
            &mut trace,
            "list_projects",
            json!({"limit":1,"cursor":restarted["next_cursor"]})
        )
        .await
        .expect("new snapshot continuation")
        .is_error
            != Some(true)
    );
    owner
        .mcp_client_revoke(McpClientMutationParams {
            client_id: a.client_id,
            expected_revision: a.revision,
            idempotency_key: "cursor-revoke".into(),
        })
        .await
        .expect("revoke");
    assert_eq!(
        raw_tool(
            &http,
            access,
            "alcomd_list_projects",
            json!({"limit":1,"cursor":restarted["next_cursor"]})
        )
        .await
        .status(),
        401
    );
    assert!(
        direct.projects_list(None, Some(1)).await.is_err(),
        "live Core connection also revoked"
    );
    let b_first = data(
        call(&sb, &mut trace, "list_projects", json!({"limit":1}))
            .await
            .expect("B before restart"),
    );
    let b_cursor = &b_first["next_cursor"];
    let b_wire = ProjectRegistryCursor {
        registered_at_ms: b_cursor["registered_at_ms"].as_u64().expect("B time"),
        id: b_cursor["project_id"].as_str().expect("B id").into(),
        authorization_binding: Some(
            b_cursor["authorization_binding"]
                .as_str()
                .expect("B MAC")
                .into(),
        ),
    };
    let original_resource = http.resource();
    drop(direct);
    drop(owner);
    stop(sa).await;
    stop(sa2).await;
    stop(sb).await;
    http.finish_server().await;
    daemon.abort();
    let _ = daemon.await;
    let reopened = alcomd_daemon::test_support::start_with_data_until(
        core.ipc.clone(),
        DataConfig::isolated(root.join("data")),
        std::future::pending(),
    )
    .await
    .expect("reopen only isolated Core");
    let mut live = AlcomdClient::connect(
        core.client
            .clone()
            .with_oauth_access(b_access.into(), original_resource),
    )
    .await
    .expect("persistent token after restart");
    assert!(
        matches!(live.projects_list(Some(b_wire),Some(1)).await,Err(alcomd_client::ClientError::Remote(ref e)) if e.code=="revision_conflict"),
        "private cursor key rotates on Core restart"
    );
    let fresh = live
        .projects_list(None, Some(1))
        .await
        .expect("restart first page");
    assert!(live.projects_list(fresh.next_cursor, Some(1)).await.is_ok());
    drop(live);
    reopened.abort();
    let _ = reopened.await;
    let visible = serde_json::to_vec_pretty(&trace).expect("public results");
    let text = String::from_utf8_lossy(&visible);
    for secret in [access, other_access, b_access] {
        assert!(!text.contains(secret));
    }
    std::fs::write(root.join("cursor-trace.json"), visible).expect("public trace");
    println!(
        "OAuth cursor binding verified through real HTTPS/Core/SDK; isolated evidence: {}",
        root.display()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn frozen_sdk_oauth_real_core_identity_scope_and_package_flow() {
    let _serial = REAL_CORE_LOCK.lock().await;
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/m8-oauth-real")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(root.join("data")).expect("disposable data");
    let root = root.canonicalize().expect("root");
    let core = CoreConfiguration::new();
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        core.ipc.clone(),
        DataConfig::isolated(root.join("data")),
        std::future::pending(),
    )
    .await
    .expect("exclusive isolated Core");
    let mut owner = AlcomdClient::connect(core.client.clone())
        .await
        .expect("real native owner");
    let mut ingress = AlcomdClient::connect(
        core.client
            .clone()
            .with_client_name("alcomd-gui")
            .with_oauth_endpoint(),
    )
    .await
    .expect("restricted endpoint role");
    assert!(
        matches!(ingress.mcp_clients_list(McpClientsListParams::default()).await,Err(alcomd_client::ClientError::Remote(ref e)) if e.code=="permission_denied")
    );
    assert!(
        matches!(ingress.oauth_request_get(McpOAuthRequestGetParams{request_id:uuid::Uuid::new_v4().to_string(),cursor:None,limit:None}).await,Err(alcomd_client::ClientError::Remote(ref e)) if e.code=="permission_denied")
    );
    drop(ingress);
    let http = Https::start(&root, core.client.clone()).await;
    let port = reqwest::Url::parse(&http.issuer)
        .expect("issuer")
        .port()
        .expect("port");
    let untrusted = reqwest::Client::builder()
        .no_proxy()
        .resolve(
            "localhost",
            std::net::SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, port)),
        )
        .build()
        .expect("default TLS verifier");
    assert!(
        untrusted
            .get(format!(
                "{}/.well-known/oauth-authorization-server",
                http.issuer
            ))
            .send()
            .await
            .is_err(),
        "test certificate must not become globally trusted"
    );
    let challenge = http
        .client
        .post(http.resource())
        .send()
        .await
        .expect("challenge");
    assert_eq!(challenge.status(), 401);
    assert!(
        challenge.headers()["www-authenticate"]
            .to_str()
            .expect("challenge")
            .contains("resource_metadata=")
    );
    let prm: Value = http
        .client
        .get(format!(
            "{}/.well-known/oauth-protected-resource/mcp",
            http.issuer
        ))
        .send()
        .await
        .expect("PRM")
        .json()
        .await
        .expect("PRM JSON");
    assert_eq!(prm["resource"], http.resource());
    let metadata: Value = http
        .client
        .get(format!(
            "{}/.well-known/oauth-authorization-server",
            http.issuer
        ))
        .send()
        .await
        .expect("AS metadata")
        .json()
        .await
        .expect("AS JSON");
    assert_eq!(metadata["issuer"], http.issuer);
    assert_eq!(
        metadata["code_challenge_methods_supported"],
        json!(["S256"])
    );
    assert!(metadata.get("registration_endpoint").is_none());
    let mut a = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "OAuth A".into(),
            grants: vec![],
            idempotency_key: "oauth-a".into(),
        })
        .await
        .expect("client A");
    let mut b = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "OAuth B".into(),
            grants: vec![],
            idempotency_key: "oauth-b".into(),
        })
        .await
        .expect("client B");
    let reg = registration(&mut owner, &http, &a, false)
        .await
        .registration;
    let reg_b = registration(&mut owner, &http, &b, false)
        .await
        .registration;
    let (zero, cookie) = http.start_request(&reg).await;
    let zero_review = owner
        .oauth_request_get(McpOAuthRequestGetParams {
            request_id: zero.clone(),
            cursor: None,
            limit: None,
        })
        .await
        .expect("zero review");
    assert!(zero_review.grants.is_empty());
    assert!(
        matches!(owner.oauth_request_decide(McpOAuthRequestDecideParams{request_id:zero.clone(),approve:true,grants:vec![grant("projects.read","global","*")],expected_fingerprint:zero_review.approval_fingerprint.clone(),idempotency_key:"zero-cannot-authorize".into()}).await,Err(alcomd_client::ClientError::Remote(ref e)) if e.code=="permission_denied")
    );
    owner
        .oauth_request_decide(McpOAuthRequestDecideParams {
            request_id: zero.clone(),
            approve: false,
            grants: vec![],
            expected_fingerprint: zero_review.approval_fingerprint,
            idempotency_key: "deny-zero".into(),
        })
        .await
        .expect("explicit denial");
    assert_eq!(http.finish(&zero, &cookie).await.status(), 303);
    let mut projects = Vec::new();
    for name in [
        "Selected",
        "Unselected",
        "OtherClient",
        "Cancellation",
        "LostWrite",
    ] {
        let path = root.join(name);
        project(&path, false);
        projects.push((
            path,
            owner
                .project_register(
                    root.join(name).to_string_lossy().into_owned(),
                    format!("register-{name}"),
                )
                .await
                .expect("register disposable project")
                .project,
        ));
    }
    let selected = projects[0].1.project_id.clone().expect("selected");
    let unselected = projects[1].1.project_id.clone().expect("unselected");
    let other = projects[2].1.project_id.clone().expect("other");
    let allowed = grants(true);
    let mut all = allowed.clone();
    all.push(grant("state.check", "global", "*"));
    a = update(&mut owner, &a, all, "grant-a").await;
    b = update(&mut owner, &b, grants(true), "grant-b").await;
    let code = http.code(&mut owner, &reg, &allowed).await;
    let bad = http
        .token(
            &reg,
            &[
                ("grant_type", "authorization_code"),
                ("code", &code),
                ("redirect_uri", &reg.redirect_uris[0]),
                ("code_verifier", &"a".repeat(43)),
                ("resource", &reg.resource),
            ],
            None,
        )
        .await;
    assert_eq!(bad.status(), 400);
    let issued = http.exchange(&reg, &code, None).await;
    let access = issued["access_token"].as_str().expect("access").to_string();
    let refresh = issued["refresh_token"]
        .as_str()
        .expect("refresh")
        .to_string();
    assert_eq!(
        http.token(
            &reg,
            &[
                ("grant_type", "authorization_code"),
                ("code", &code),
                ("redirect_uri", &reg.redirect_uris[0]),
                ("code_verifier", VERIFIER),
                ("resource", &reg.resource)
            ],
            None
        )
        .await
        .status(),
        400,
        "lost token response cannot be reissued"
    );
    let code_b = http.code(&mut owner, &reg_b, &b.grants).await;
    let issued_b = http.exchange(&reg_b, &code_b, None).await;
    let access_b = issued_b["access_token"]
        .as_str()
        .expect("B access")
        .to_string();
    let sa = http.sdk(&access).await;
    let sb = http.sdk(&access_b).await;
    let mut trace = vec![];
    let mut persistent = AlcomdClient::connect(
        core.client
            .clone()
            .with_client_name("alcomd-gui")
            .with_oauth_access(access.clone(), http.resource()),
    )
    .await
    .expect("live OAuth backend connection");
    assert!(persistent.project_get(selected.clone()).await.is_ok());
    assert!(
        matches!(persistent.mcp_clients_list(McpClientsListParams::default()).await,Err(alcomd_client::ClientError::Remote(ref e)) if e.code=="permission_denied")
    );
    assert_eq!(
        sa.list_tools(None)
            .await
            .expect("existing tools")
            .tools
            .len(),
        6
    );
    let mut cursor = None;
    let mut ids = vec![];
    loop {
        let args = if let Some(c) = cursor.take() {
            json!({"limit":1,"cursor":c})
        } else {
            json!({"limit":1})
        };
        let page = data(
            call(&sa, &mut trace, "list_projects", args)
                .await
                .expect("scoped page"),
        );
        for p in page["projects"].as_array().expect("projects") {
            ids.push(p["project_id"].as_str().expect("ID").to_string());
        }
        cursor = page.get("next_cursor").filter(|v| !v.is_null()).cloned();
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(ids.len(), 5);
    assert!(ids.contains(&selected));
    assert!(ids.contains(&unselected));
    assert!(ids.contains(&other));
    assert_eq!(
        data(
            call(
                &sa,
                &mut trace,
                "get_project",
                json!({"project_id":unselected})
            )
            .await
            .expect("one read capability covers all projects")
        )["project_id"],
        unselected
    );
    assert!(
        matches!(persistent.state_check("outside-token-capability-ceiling".into()).await,Err(alcomd_client::ClientError::Remote(ref error)) if error.code=="permission_denied"),
        "Core capability added after consent is not in the token ceiling"
    );
    let plan=data(call(&sa,&mut trace,"plan_package_uninstall",json!({"project_id":selected,"expected_revision":projects[0].1.revision.expect("revision"),"package_id":"com.example.fixture"})).await.expect("real plan"));
    error(call(&sb,&mut trace,"apply_package_changes",json!({"plan_id":plan["plan_id"],"expected_revision":plan["project_revision"],"idempotency_key":"shared-apply-key","confirm":true})).await.expect("other identity rejected"),"plan_not_found");
    let applied=data(call(&sa,&mut trace,"apply_package_changes",json!({"plan_id":plan["plan_id"],"expected_revision":plan["project_revision"],"idempotency_key":"shared-apply-key","confirm":true})).await.expect("original Plan apply"));
    let operation = applied["operation_id"].as_str().expect("operation");
    assert_eq!(
        terminal(&sa, &mut trace, operation).await["state"],
        "succeeded"
    );
    error(
        call(
            &sb,
            &mut trace,
            "get_operation",
            json!({"operation_id":operation}),
        )
        .await
        .expect("cross operation"),
        "operation_not_found",
    );
    assert!(!projects[0].0.join("Packages/com.example.fixture").exists());
    assert!(projects[1].0.join("Packages/com.example.fixture").exists());
    for (index, cancel) in [(3, true), (4, false)] {
        let id = projects[index]
            .1
            .project_id
            .as_deref()
            .expect("risk project");
        let plan=data(call(&sa,&mut trace,"plan_package_uninstall",json!({"project_id":id,"expected_revision":projects[index].1.revision.expect("revision"),"package_id":"com.example.fixture"})).await.expect("risk plan"));
        let key = if cancel {
            "oauth-inflight-cancel"
        } else {
            "oauth-lost-core-response"
        };
        let gate = daemon.hold_next_mcp_apply_response(!cancel).await;
        let args = json!({"plan_id":plan["plan_id"],"expected_revision":plan["project_revision"],"idempotency_key":key,"confirm":true});
        if cancel {
            let params = CallToolRequestParams::new("alcomd_apply_package_changes")
                .with_arguments(args.as_object().expect("args").clone());
            let mut request = sa
                .send_cancellable_request(
                    ClientRequest::CallToolRequest(Request::new(params)),
                    PeerRequestOptions::no_options(),
                )
                .await
                .expect("cancellable HTTPS request");
            let accepted = tokio::time::timeout(Duration::from_secs(10), gate.accepted())
                .await
                .expect("durable acceptance");
            let (_sender, replacement) = tokio::sync::oneshot::channel();
            let response = std::mem::replace(&mut request.rx, replacement);
            request
                .cancel(Some("isolated OAuth request cancellation".into()))
                .await
                .expect("protocol request cancellation");
            assert!(matches!(
                response.await.expect("SDK cancellation response"),
                Err(ServiceError::Cancelled { .. })
            ));
            assert!(
                tokio::time::timeout(Duration::from_secs(5), sa.list_tools(None))
                    .await
                    .expect("cancelled exchange must release independently")
                    .is_ok()
            );
            gate.release();
            let terminal = terminal(&sa, &mut trace, &accepted.operation_id).await;
            assert_eq!(terminal["state"], "succeeded");
        } else {
            let (accepted, result) = {
                let pending = call(&sa, &mut trace, "apply_package_changes", args.clone());
                tokio::pin!(pending);
                let accepted = tokio::select! {v=gate.accepted()=>v,_=&mut pending=>panic!("response cannot finish before acceptance gate")};
                gate.release();
                (
                    accepted,
                    pending
                        .await
                        .expect("transport produces a truthful unknown result"),
                )
            };
            error(result, "core_write_outcome_unknown");
            let recovered = data(
                call(&sa, &mut trace, "apply_package_changes", args)
                    .await
                    .expect("explicit same-key recovery"),
            );
            assert_eq!(recovered["operation_id"], accepted.operation_id);
            assert_eq!(
                terminal(&sa, &mut trace, &accepted.operation_id).await["state"],
                "succeeded"
            );
        }
        assert!(
            !projects[index]
                .0
                .join("Packages/com.example.fixture")
                .exists()
        );
    }
    let c = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "OAuth confidential read-only".into(),
            grants: vec![grant("projects.read", "global", "*")],
            idempotency_key: "oauth-c".into(),
        })
        .await
        .expect("client C");
    let registered_c = registration(&mut owner, &http, &c, true).await;
    let secret_c = registered_c.client_secret.expect("one-time client secret");
    let reg_c = registered_c.registration;
    let code_c = http.code(&mut owner, &reg_c, &c.grants).await;
    let missing_secret = http
        .token(
            &reg_c,
            &[
                ("grant_type", "authorization_code"),
                ("code", &code_c),
                ("redirect_uri", &reg_c.redirect_uris[0]),
                ("code_verifier", VERIFIER),
                ("resource", &reg_c.resource),
            ],
            None,
        )
        .await;
    assert_eq!(missing_secret.status(), 401);
    let issued_c = http.exchange(&reg_c, &code_c, Some(&secret_c)).await;
    let access_c = issued_c["access_token"].as_str().expect("C access");
    let denied=raw_tool(&http,access_c,"alcomd_cancel_operation",json!({"operation_id":uuid::Uuid::new_v4().to_string(),"expected_revision":1,"idempotency_key":"c-cancel-denied"})).await;
    assert_eq!(denied.status(), 403);
    let challenge = denied.headers()["www-authenticate"]
        .to_str()
        .expect("scope challenge");
    assert!(challenge.contains("scope=\"operations.read operations.cancel\""));
    assert_eq!(
        http.client
            .post(http.resource())
            .header("Origin", "https://attacker.invalid")
            .bearer_auth(access_c)
            .send()
            .await
            .expect("Origin")
            .status(),
        403
    );
    assert_eq!(
        http.client
            .post(http.resource())
            .header("Host", "attacker.invalid")
            .bearer_auth(access_c)
            .send()
            .await
            .expect("Host")
            .status(),
        403
    );
    assert_eq!(
        http.client
            .post(http.resource())
            .bearer_auth("a".repeat(64))
            .send()
            .await
            .expect("generic credential rejected")
            .status(),
        401
    );
    assert_eq!(
        http.client
            .post(http.resource())
            .bearer_auth(&refresh)
            .send()
            .await
            .expect("refresh is not access")
            .status(),
        401
    );
    let duplicate = http
        .client
        .post(format!("{}/token", http.issuer))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body("client_id=x&%63lient_id=y")
        .send()
        .await
        .expect("duplicate form");
    assert_eq!(duplicate.status(), 400);
    let too_large = http
        .client
        .post(format!("{}/token", http.issuer))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body("x".repeat(16385))
        .send()
        .await
        .expect("bounded form");
    assert_eq!(too_large.status(), 413);
    let mut revoke_form = reqwest::Url::parse("https://localhost/").expect("form");
    revoke_form
        .query_pairs_mut()
        .append_pair("client_id", &reg_c.oauth_client_id)
        .append_pair("token", access_c);
    let revoked = http
        .client
        .post(format!("{}/revoke", http.issuer))
        .basic_auth(&reg_c.oauth_client_id, Some(&secret_c))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(revoke_form.query().expect("body").to_string())
        .send()
        .await
        .expect("standard token revocation");
    assert_eq!(revoked.status(), 200);
    assert_eq!(
        raw_tool(&http, access_c, "alcomd_list_projects", json!({}))
            .await
            .status(),
        401
    );
    owner
        .oauth_registration_revoke(McpOAuthRegistrationRevokeParams {
            oauth_client_id: reg_c.oauth_client_id,
            expected_revision: reg_c.revision,
            idempotency_key: "revoke-registration-c".into(),
        })
        .await
        .expect("revoke registration");
    assert_eq!(
        raw_tool(&http, access_c, "alcomd_list_projects", json!({}))
            .await
            .status(),
        401
    );
    let a_current = owner
        .mcp_clients_list(McpClientsListParams {
            cursor: None,
            limit: Some(64),
        })
        .await
        .expect("current list")
        .items
        .into_iter()
        .find(|c| c.client_id == a.client_id)
        .expect("A");
    let reduced = update(&mut owner, &a_current, vec![], "remove-authority").await;
    assert!(
        matches!(persistent.project_get(selected.clone()).await,Err(alcomd_client::ClientError::Remote(ref e)) if e.code=="permission_denied")
    );
    error(
        call(
            &sa,
            &mut trace,
            "get_project",
            json!({"project_id":selected}),
        )
        .await
        .expect("live revocation of grants"),
        "permission_denied",
    );
    let _restored = update(&mut owner, &reduced, a_current.grants, "restore-authority").await;
    let refreshed = http
        .token(
            &reg,
            &[
                ("grant_type", "refresh_token"),
                ("refresh_token", &refresh),
                ("resource", &reg.resource),
            ],
            None,
        )
        .await;
    assert_eq!(refreshed.status(), 200);
    let rotated: Value = refreshed.json().await.expect("rotated tokens");
    let rotated_access = rotated["access_token"].as_str().expect("new access");
    let sc = http.sdk(rotated_access).await;
    stop(sc).await;
    let rotated_refresh = rotated["refresh_token"].as_str().expect("rotated refresh");
    let lost_gate = daemon.hold_next_oauth_token_response().await;
    let lost = {
        let fields = [
            ("grant_type", "refresh_token"),
            ("refresh_token", rotated_refresh),
            ("resource", reg.resource.as_str()),
        ];
        let pending = http.token(&reg, &fields, None);
        tokio::pin!(pending);
        tokio::select! {_=lost_gate.accepted()=>{},_=&mut pending=>panic!("token reply cannot precede its committed response gate")};
        lost_gate.release();
        pending.await
    };
    assert_eq!(lost.status(), 503);
    let lost_body: Value = lost.json().await.expect("bounded unknown-outcome response");
    assert_eq!(lost_body["error"], "temporarily_unavailable");
    assert!(
        lost_body["error_description"]
            .as_str()
            .expect("truthful outcome")
            .contains("outcome is unknown")
    );
    assert!(lost_body.get("access_token").is_none());
    assert_eq!(
        http.token(
            &reg,
            &[
                ("grant_type", "refresh_token"),
                ("refresh_token", rotated_refresh),
                ("resource", &reg.resource)
            ],
            None
        )
        .await
        .status(),
        400
    );
    assert_eq!(
        raw_tool(&http, rotated_access, "alcomd_list_projects", json!({}))
            .await
            .status(),
        401,
        "refresh replay revokes whole family"
    );
    assert!(
        matches!(persistent.system_status().await,Err(alcomd_client::ClientError::Remote(ref e)) if e.code=="mcp_oauth_invalid_grant")
    );
    drop(persistent);
    let b = owner
        .mcp_client_revoke(McpClientMutationParams {
            client_id: b.client_id,
            expected_revision: b.revision,
            idempotency_key: "revoke-b".into(),
        })
        .await
        .expect("Core revoke B");
    assert!(b.revoked_at_ms.is_some());
    assert_eq!(
        raw_tool(&http, &access_b, "alcomd_list_projects", json!({}))
            .await
            .status(),
        401
    );
    stop(sa).await;
    stop(sb).await;
    assert_eq!(
        daemon.mcp_apply_request_count(),
        5,
        "no automatic business write retries"
    );
    drop(owner);
    http.finish_server().await;
    daemon.abort();
    let _ = daemon.await;
    let db = rusqlite::Connection::open_with_flags(
        root.join("data/state.db"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .expect("isolated state evidence");
    let owner_id: String = db
        .query_row(
            "SELECT owner_principal_id FROM projects WHERE project_id=?1",
            [selected],
            |r| r.get(0),
        )
        .expect("owner");
    assert_eq!(owner_id, "builtin:local-owner");
    let actor: String = db
        .query_row(
            "SELECT owner_principal_id FROM operations WHERE operation_id=?1",
            [operation],
            |r| r.get(0),
        )
        .expect("actor");
    assert_eq!(actor, a.principal_id);
    let tokens: i64 = db
        .query_row(
            "SELECT count(*) FROM mcp_oauth_tokens WHERE oauth_client_id=?1",
            [&reg.oauth_client_id],
            |row| row.get(0),
        )
        .expect("durable token exchanges");
    assert_eq!(
        tokens, 6,
        "lost reply commits one pair and replay never reissues"
    );
    let count:i64=db.query_row("SELECT count(*) FROM idempotency_records WHERE principal_id=?1 AND method='packages.applyPlan' AND idempotency_key='oauth-lost-core-response'",[&a.principal_id],|r|r.get(0)).expect("one recovery record");
    assert_eq!(count, 1);
    let evidence = serde_json::to_vec_pretty(&trace).expect("bounded public results");
    let visible = String::from_utf8_lossy(&evidence);
    for secret in [
        &access,
        &access_b,
        &refresh,
        rotated_access,
        rotated_refresh,
        access_c,
        &secret_c,
    ] {
        assert!(!visible.contains(secret));
    }
    std::fs::write(root.join("tool-trace.json"), evidence).expect("nonsecret evidence");
    println!(
        "OAuth real Core/HTTPS/SDK flow verified; isolated evidence: {}",
        root.display()
    );
}
pub(super) async fn extension_provider_oauth(
    owner: &mut AlcomdClient,
    root: &Path,
    client: &McpClientRecord,
) -> String {
    let http = Https::start(root, ClientConfig::default().without_daemon_start()).await;
    let reg = registration(owner, &http, client, false).await.registration;
    let code = http.code(owner, &reg, &grants(true)).await;
    let first = http.exchange(&reg, &code, None).await;
    let access = first["access_token"]
        .as_str()
        .expect("valid isolated Provider OAuth fixture");
    let code = http.code(owner, &reg, &grants(true)).await;
    let second = http.exchange(&reg, &code, None).await;
    let other = second["access_token"]
        .as_str()
        .expect("valid isolated Provider OAuth fixture");
    let code = http
        .code(owner, &reg, &[grant("operations.read", "global", "*")])
        .await;
    let empty = http.exchange(&reg, &code, None).await;
    let empty_access = empty["access_token"]
        .as_str()
        .expect("valid isolated Provider OAuth fixture");
    let sa = http.sdk(access).await;
    let sb = http.sdk(other).await;
    let zero = http.sdk(empty_access).await;
    let page = sa
        .list_tools(None)
        .await
        .expect("valid isolated Provider OAuth fixture");
    let cursor = page.next_cursor.expect("provider OAuth pagination");
    assert!(
        sb.list_tools(Some(
            rmcp::model::PaginatedRequestParams::default().with_cursor(Some(cursor))
        ))
        .await
        .is_err(),
        "same principal, distinct token cannot reuse catalogue cursor"
    );
    let pure = alcomd_application::tool_public_name("dev.alcomd.tools-fixture", "echo");
    let reading = alcomd_application::tool_public_name("dev.alcomd.tools-fixture", "projects");
    let page = zero
        .list_tools(None)
        .await
        .expect("valid isolated Provider OAuth fixture");
    assert!(page.tools.iter().all(|t| t.name != reading));
    let result = zero
        .call_tool_once(
            CallToolRequestParams::new(pure).with_arguments(
                json!({"text":"limited OAuth ceiling"})
                    .as_object()
                    .expect("valid isolated Provider OAuth fixture")
                    .clone(),
            ),
        )
        .await
        .expect("valid isolated Provider OAuth fixture");
    let CallToolResponse::Complete(result) = result else {
        panic!("normal result")
    };
    assert_ne!(result.is_error, Some(true));
    assert!(
        zero.call_tool_once(CallToolRequestParams::new(reading).with_arguments(Default::default()))
            .await
            .is_err(),
        "no upper-bound escalation"
    );
    let directory = root.join("OAuth-provider-project");
    project(&directory, false);
    let registered = owner
        .project_register(
            directory.to_string_lossy().into_owned(),
            "provider-oauth-project".into(),
        )
        .await
        .expect("valid isolated Provider OAuth fixture")
        .project;
    let plan=super::extension_tools_flow::ext(&sa,"plan_remove",json!({"project_id":registered.project_id,"expected_revision":registered.revision,"package_id":"com.example.fixture","idempotency_key":"oauth-provider-plan"})).await.structured_content.expect("valid isolated Provider OAuth fixture");
    let plan = &plan["coreEffect"];
    let applied=super::extension_tools_flow::ext(&sa,"apply",json!({"plan_id":plan["planId"],"expected_revision":plan["projectRevision"],"idempotency_key":"oauth-provider-apply","confirm":true})).await;
    assert_ne!(applied.is_error, Some(true));
    let operation = applied
        .structured_content
        .expect("valid isolated Provider OAuth fixture")["coreEffect"]["operationId"]
        .as_str()
        .expect("valid isolated Provider OAuth fixture")
        .to_owned();
    let mut trace = vec![];
    assert_eq!(
        terminal(&sa, &mut trace, &operation).await["state"],
        "succeeded"
    );
    assert!(!directory.join("Packages/com.example.fixture").exists());
    let mut form =
        reqwest::Url::parse("https://localhost/").expect("valid isolated Provider OAuth fixture");
    form.query_pairs_mut()
        .append_pair("client_id", &reg.oauth_client_id)
        .append_pair("token", access);
    assert_eq!(
        http.client
            .post(format!("{}/revoke", http.issuer))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(
                form.query()
                    .expect("valid isolated Provider OAuth fixture")
                    .to_owned()
            )
            .send()
            .await
            .expect("valid isolated Provider OAuth fixture")
            .status(),
        200
    );
    assert!(
        sa.list_tools(None).await.is_err(),
        "revoked OAuth provider connection"
    );
    stop(sa).await;
    stop(sb).await;
    stop(zero).await;
    http.finish_server().await;
    operation
}
