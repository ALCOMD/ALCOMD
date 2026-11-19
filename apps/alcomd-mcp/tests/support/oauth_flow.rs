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
pub(super) struct CoreConfiguration {
    pub(super) ipc: IpcConfig,
    pub(super) client: ClientConfig,
    #[cfg(unix)]
    runtime: std::path::PathBuf,
}
impl CoreConfiguration {
    pub(super) fn new() -> Self {
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
        url.query_pairs_mut().extend_pairs([
            ("client_id", reg.oauth_client_id.as_str()),
            ("response_type", "code"),
            ("redirect_uri", reg.redirect_uris[0].as_str()),
            ("code_challenge", CHALLENGE),
            ("code_challenge_method", "S256"),
            ("resource", reg.resource.as_str()),
            ("scope", "mcp"),
            ("state", "isolated-state"),
        ]);
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
    async fn code(&self, owner: &mut AlcomdClient, reg: &McpOAuthRegistration) -> String {
        let (request, cookie) = self.start_request(reg).await;
        self.approve_code(owner, request, cookie).await
    }
    async fn approve_code(
        &self,
        owner: &mut AlcomdClient,
        request: String,
        cookie: String,
    ) -> String {
        let first = owner
            .oauth_request_get(McpOAuthRequestGetParams {
                request_id: request.clone(),
            })
            .await
            .expect("owner review");
        assert_eq!(first.scopes, ["mcp"]);
        owner
            .oauth_request_decide(McpOAuthRequestDecideParams {
                request_id: request.clone(),
                approve: true,
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
async fn oauth_environment_matches_native_settings_and_unity_reads() {
    use alcomd_application::{
        IdempotencyKey, M5UnityStore, PrincipalId, UnityArchitecture, UnityInstallationObservation,
        UnitySourceKind,
    };

    let _serial = REAL_CORE_LOCK.lock().await;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/m8-environment-tests")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&root).expect("isolated fixture");
    let root = root.canonicalize().expect("canonical fixture");
    let source = root.join("Project");
    let archives = root.join("Backups");
    project(&source, false);
    std::fs::create_dir(&archives).expect("isolated backup directory");
    let data_config = DataConfig::isolated(root.join("data"));
    let store = alcomd_store::StateStoreHandle::open(
        alcomd_platform::state_database_path(&data_config).expect("isolated database path"),
    )
    .expect("isolated Store");
    // Pre-verified records keep this contract test independent of machine Editor installs.
    for (index, version) in ["2022.3.22f1", "6000.0.3f1"].into_iter().enumerate() {
        store
            .register_installation(
                PrincipalId::local_owner(),
                UnityInstallationObservation {
                    executable_path: root
                        .join(version)
                        .join("Unity")
                        .to_string_lossy()
                        .into_owned(),
                    filesystem_identity: vec![1 + index as u8],
                    unity_version: version.to_owned(),
                    architecture: UnityArchitecture::X86_64,
                    source_kind: UnitySourceKind::Manual,
                    observed_at_ms: 10 + index as u64,
                },
                IdempotencyKey::parse(format!("seed-editor-{index}")).expect("seed key"),
                10 + index as u64,
            )
            .await
            .expect("seed only the isolated registry");
    }
    drop(store);
    let config = CoreConfiguration::new();
    let (shutdown, stopped) = tokio::sync::oneshot::channel();
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        config.ipc.clone(),
        data_config,
        async {
            let _ = stopped.await;
        },
    )
    .await
    .expect("isolated real Core");
    let mut owner = AlcomdClient::connect(config.client.clone())
        .await
        .expect("owner");
    let initial = owner.settings_get().await.expect("native settings");
    let default_args = vec!["-logFile", "Default file with spaces.log"];
    owner
        .settings_update(serde_json::from_value(json!({
            "expectedRevision": initial.revision,
            "update": {
                "paths": {"projectDirectory": root.to_string_lossy(), "backupDirectory": archives.to_string_lossy()},
                "unity": {"defaultLaunchArguments": default_args},
                "projects": {"autoDiscover": false}
            }
        })).expect("typed fixture settings"))
        .await
        .expect("set isolated defaults");
    let project_id = owner
        .project_register(
            source.to_string_lossy().into_owned(),
            "environment-project".into(),
        )
        .await
        .expect("register native project")
        .project
        .project_id
        .expect("project ID");
    let settings = owner.settings_get().await.expect("current native settings");
    let args = json!({"project_id":project_id});
    let actor = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "Environment read fixture".into(),
            idempotency_key: "environment-client".into(),
        })
        .await
        .expect("isolated client");
    let issued = owner
        .mcp_client_issue_credential(McpClientMutationParams {
            client_id: actor.client_id.clone(),
            expected_revision: actor.revision,
            idempotency_key: "environment-credential".into(),
        })
        .await
        .expect("isolated credential");
    let mut delegated =
        AlcomdClient::connect(config.client.clone().with_mcp_credential(issued.credential))
            .await
            .expect("same native routes with delegated identity");
    assert_eq!(
        delegated
            .settings_get()
            .await
            .expect("native delegated settings"),
        settings
    );
    assert_eq!(
        delegated
            .unity_project_launch_config_get(project_id.clone())
            .await
            .expect("native delegated config"),
        owner
            .unity_project_launch_config_get(project_id.clone())
            .await
            .expect("native owner config"),
    );
    let http = Https::start(&root, config.client.clone()).await;
    let registration = registration(&mut owner, &http, &actor, false)
        .await
        .registration;
    let code = http.code(&mut owner, &registration).await;
    let token = http.exchange(&registration, &code, None).await;
    let sdk = http
        .sdk(token["access_token"].as_str().expect("granted token"))
        .await;
    let tools = sdk
        .list_tools(None)
        .await
        .expect("authorized catalog")
        .tools;
    for name in [
        "alcomd_get_settings",
        "alcomd_list_unity_installations",
        "alcomd_get_project_launch_config",
    ] {
        assert!(tools.iter().any(|tool| tool.name == name));
    }
    let mut trace = Vec::new();
    let settings_tool = data(
        call(&sdk, &mut trace, "get_settings", json!({}))
            .await
            .expect("settings tool"),
    );
    assert_eq!(settings_tool["revision"], settings.revision);
    assert_eq!(
        settings_tool["directories"]["project_directory"],
        settings.directories.project_directory
    );
    assert_eq!(
        settings_tool["directories"]["backup_directory"],
        settings.directories.backup_directory
    );
    assert_eq!(
        settings_tool["settings"]["unity"]["default_launch_arguments"],
        json!(default_args)
    );
    let installs = data(
        call(
            &sdk,
            &mut trace,
            "list_unity_installations",
            json!({"limit":1,"include_version_options":true}),
        )
        .await
        .expect("Unity page"),
    );
    let native = owner
        .unity_installations_list(UnityInstallationsListParams {
            cursor: None,
            limit: Some(1),
            include_version_options: Some(true),
        })
        .await
        .expect("native Unity page");
    assert_eq!(
        installs["installations"][0]["installation_id"],
        native.installations[0].installation_id
    );
    assert_eq!(
        installs["installations"][0]["executable_path"],
        native.installations[0].executable_path
    );
    assert_eq!(
        installs["version_options"]["versions"],
        json!(native.version_options.expect("version choices").versions)
    );
    let cursor = native.next_cursor.expect("next page");
    assert_eq!(installs["next_cursor"], cursor);
    let next = data(
        call(
            &sdk,
            &mut trace,
            "list_unity_installations",
            json!({"limit":1,"cursor":cursor}),
        )
        .await
        .expect("Unity continuation"),
    );
    assert_ne!(
        next["installations"][0]["installation_id"],
        installs["installations"][0]["installation_id"]
    );
    assert!(next.get("next_cursor").is_none());
    let launch = data(
        call(&sdk, &mut trace, "get_project_launch_config", args.clone())
            .await
            .expect("effective defaults"),
    );
    assert_eq!(launch["effective_arguments"], json!(default_args));
    assert_eq!(launch["config"]["revision"], 0);
    let overrides = vec![
        "-logFile".to_owned(),
        "Exact Override destination.log".to_owned(),
    ];
    owner
        .unity_project_launch_config_set(ProjectUnityLaunchConfigSetParams {
            project_id: project_id.clone(),
            arguments: overrides.clone(),
            expected_revision: 0,
            idempotency_key: "environment-launch-override".into(),
        })
        .await
        .expect("native override");
    let launch = data(
        call(&sdk, &mut trace, "get_project_launch_config", args.clone())
            .await
            .expect("project override"),
    );
    let native = owner
        .unity_project_launch_config_get(project_id)
        .await
        .expect("native override view");
    assert_eq!(
        launch["config"]["arguments"],
        json!(native.config.arguments)
    );
    assert_eq!(launch["effective_arguments"], json!(overrides));
    assert_eq!(launch["default_arguments"], json!(native.default_arguments));
    let revoke = owner
        .mcp_client_revoke(McpClientMutationParams {
            client_id: actor.client_id,
            expected_revision: issued.revision,
            idempotency_key: "revoke-environment".into(),
        })
        .await
        .expect("revoke connected client");
    assert!(revoke.revoked_at_ms.is_some());
    assert!(delegated.settings_get().await.is_err());
    assert_eq!(
        raw_tool(
            &http,
            token["access_token"].as_str().expect("token"),
            "alcomd_get_project_launch_config",
            args
        )
        .await
        .status(),
        401
    );
    stop(sdk).await;
    http.finish_server().await;
    drop(delegated);
    drop(owner);
    shutdown.send(()).expect("stop fixture");
    daemon.await.expect("join").expect("fixture shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn oauth_backup_write_works_after_consent_until_revoked() {
    let _serial = REAL_CORE_LOCK.lock().await;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/m8-backup-oauth-tests")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&root).expect("isolated fixture");
    let root = root.canonicalize().expect("canonical fixture");
    let source = root.join("Source");
    project(&source, false);
    let archives = root.join("archives");
    std::fs::create_dir(&archives).expect("isolated archives");
    let config = CoreConfiguration::new();
    let (shutdown, stopped) = tokio::sync::oneshot::channel();
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        config.ipc.clone(),
        DataConfig::isolated(root.join("data")),
        async {
            let _ = stopped.await;
        },
    )
    .await
    .expect("isolated real Core");
    let mut owner = AlcomdClient::connect(config.client.clone())
        .await
        .expect("owner");
    let settings = owner.settings_get().await.expect("settings");
    owner
        .settings_update(
            serde_json::from_value(json!({
                "expectedRevision": settings.revision,
                "update": {
                    "paths": {"backupDirectory": archives.to_string_lossy()},
                    "projects": {"autoDiscover": false}
                }
            }))
            .expect("fixture settings"),
        )
        .await
        .expect("isolated backup destination");
    let registered = owner
        .project_register(
            source.to_string_lossy().into_owned(),
            "oauth-backup-project".into(),
        )
        .await
        .expect("registered project")
        .project;
    let actor = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "Backup OAuth fixture".into(),
            idempotency_key: "oauth-backup-client".into(),
        })
        .await
        .expect("client has write grant");
    let http = Https::start(&root, config.client.clone()).await;
    let registration = registration(&mut owner, &http, &actor, false)
        .await
        .registration;
    let args = json!({
        "project_id": registered.project_id.expect("registered project ID"),
        "expected_revision": registered.revision.expect("registered revision"),
        "idempotency_key": "oauth-backup-exact-key"
    });
    let code = http.code(&mut owner, &registration).await;
    let write_token = http.exchange(&registration, &code, None).await;
    let write_token = write_token["access_token"].as_str().expect("write token");
    let writer = http.sdk(write_token).await;
    let mut trace = Vec::new();
    let accepted = data(
        call(&writer, &mut trace, "create_backup", args.clone())
            .await
            .expect("authorized SDK write"),
    );
    let result = terminal(
        &writer,
        &mut trace,
        accepted["operation_id"].as_str().expect("operation"),
    )
    .await;
    assert_eq!(result["state"], "succeeded");
    assert_eq!(result["result"]["backup_id"], accepted["backup_id"]);
    let replay = data(
        call(&writer, &mut trace, "create_backup", args.clone())
            .await
            .expect("same-key replay"),
    );
    assert_eq!(replay["operation_id"], accepted["operation_id"]);
    assert_eq!(replay["replayed"], true);
    owner
        .mcp_client_revoke(McpClientMutationParams {
            client_id: actor.client_id,
            expected_revision: actor.revision,
            idempotency_key: "oauth-backup-revoke".into(),
        })
        .await
        .expect("revoke connected client");
    assert_eq!(
        raw_tool(&http, write_token, "alcomd_create_backup", args)
            .await
            .status(),
        401
    );
    assert_eq!(
        std::fs::read_dir(&archives)
            .expect("one committed archive")
            .count(),
        1
    );
    stop(writer).await;
    http.finish_server().await;
    drop(owner);
    shutdown.send(()).expect("stop fixture");
    daemon.await.expect("join").expect("fixture shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn oauth_package_catalog_and_anonymous_https_download() {
    oauth_package_catalog_and_https_download(false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn oauth_packages_use_parameterized_sources_and_download_authorization_without_extra_grants()
{
    oauth_package_catalog_and_https_download(true).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn frozen_sdk_oauth_repository_registry_confirmation_and_replay() {
    let _serial = REAL_CORE_LOCK.lock().await;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/m8-repository-parity-tests")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&root).expect("disposable evidence root");
    let root = root.canonicalize().expect("isolated fixture path");
    let source = super::extension_sources_flow::HttpsSource::protected_vpm(
        b"unused archive fixture".to_vec(),
    )
    .await;
    let configuration = CoreConfiguration::new();
    let daemon = alcomd_daemon::test_support::start_with_package_source_until(
        configuration.ipc.clone(),
        DataConfig::isolated(root.join("data")),
        CERT,
        std::future::pending(),
    )
    .await
    .expect("isolated Core");
    let mut owner = AlcomdClient::connect(configuration.client.clone())
        .await
        .expect("local test owner");
    let local_index = root.join("local-fixture.json");
    std::fs::write(
        &local_index,
        r#"{"id":"local.fixture","name":"Local fixture","packages":{}}"#,
    )
    .expect("disposable local repository");
    let local = owner
        .repository_register(
            RepositorySource::Local {
                path: local_index.to_string_lossy().into_owned(),
            },
            "local-repository-fixture".into(),
        )
        .await
        .expect("same native registry")
        .repository;
    let http = Https::start(&root, configuration.client).await;
    let a = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "Repository manager A".into(),
            idempotency_key: "repository-manager-a".into(),
        })
        .await
        .expect("client A");
    let b = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "Repository manager B".into(),
            idempotency_key: "repository-manager-b".into(),
        })
        .await
        .expect("client B");
    let reg_a = registration(&mut owner, &http, &a, false)
        .await
        .registration;
    let reg_b = registration(&mut owner, &http, &b, false)
        .await
        .registration;
    let code = http.code(&mut owner, &reg_a).await;
    let read_token = http.exchange(&reg_a, &code, None).await;
    let read_token = read_token["access_token"].as_str().expect("read access");
    let reader = http.sdk(read_token).await;
    let code = http.code(&mut owner, &reg_a).await;
    let write_token = http.exchange(&reg_a, &code, None).await;
    let writer = http
        .sdk(write_token["access_token"].as_str().expect("write access"))
        .await;
    let code = http.code(&mut owner, &reg_b).await;
    let foreign_token = http.exchange(&reg_b, &code, None).await;
    let foreign = http
        .sdk(
            foreign_token["access_token"]
                .as_str()
                .expect("other manager"),
        )
        .await;
    let tools = writer
        .list_tools(None)
        .await
        .expect("repository descriptors")
        .tools;
    let register_tool = tools
        .iter()
        .find(|tool| tool.name == "alcomd_register_repository")
        .expect("register tool");
    let annotations = register_tool
        .annotations
        .as_ref()
        .expect("register annotations");
    assert_eq!(annotations.read_only_hint, Some(false));
    assert_eq!(annotations.destructive_hint, Some(false));
    assert_eq!(annotations.idempotent_hint, Some(true));
    assert_eq!(annotations.open_world_hint, Some(true));
    let mut trace = Vec::new();
    let args = json!({"url":format!("{}{}",source.base,source.index_path)});
    let read_preview = data(
        call(&reader, &mut trace, "inspect_repository", args.clone())
            .await
            .expect("read-only inspect"),
    );
    assert!(read_preview.get("preview_id").is_none());
    let preview = data(
        call(&writer, &mut trace, "inspect_repository", args.clone())
            .await
            .expect("inspect for registration"),
    );
    assert_eq!(preview["display_name"], "M8 VPM fixture");
    assert_eq!(preview["version_count"], 1);
    assert!(!preview.to_string().contains(&source.base));
    let register = json!({"preview_id":preview["preview_id"],"confirm":true,"idempotency_key":"register-inspected"});
    let mut unconfirmed = register.clone();
    unconfirmed["confirm"] = json!(false);
    assert!(
        call(&writer, &mut trace, "register_repository", unconfirmed)
            .await
            .is_err()
    );
    error(
        call(
            &foreign,
            &mut trace,
            "register_repository",
            register.clone(),
        )
        .await
        .expect("foreign preview"),
        "revision_conflict",
    );
    source.set_changed(true);
    error(
        call(&writer, &mut trace, "register_repository", register)
            .await
            .expect("stale preview"),
        "revision_conflict",
    );
    let preview = data(
        call(&writer, &mut trace, "inspect_repository", args)
            .await
            .expect("fresh preview"),
    );
    let register = json!({"preview_id":preview["preview_id"],"confirm":true,"idempotency_key":"register-inspected"});
    let registered = data(
        call(&writer, &mut trace, "register_repository", register.clone())
            .await
            .expect("confirm registration"),
    );
    let id = registered["repository_id"]
        .as_str()
        .expect("registered repository");
    assert_eq!(
        registered["repository"]["display_name"],
        "Updated M8 VPM fixture"
    );
    assert_eq!(registered["repository"]["available"], true);
    assert!(!registered.to_string().contains(&source.base));
    let native = owner
        .repository_get(id.to_owned())
        .await
        .expect("same registry for native clients");
    assert_eq!(native.repository.repository_id.as_deref(), Some(id));
    assert_eq!(
        data(
            call(&writer, &mut trace, "register_repository", register.clone())
                .await
                .expect("same-key recovery")
        )["replayed"],
        true
    );
    let first = data(
        call(&reader, &mut trace, "list_repositories", json!({"limit":1}))
            .await
            .expect("read pagination"),
    );
    let second = data(
        call(
            &reader,
            &mut trace,
            "list_repositories",
            json!({"limit":1,"cursor":first["next_cursor"]}),
        )
        .await
        .expect("next page"),
    );
    assert_ne!(
        first["repositories"][0]["repository_id"],
        second["repositories"][0]["repository_id"]
    );
    let page = data(
        call(&reader, &mut trace, "list_repositories", json!({}))
            .await
            .expect("shared registry list"),
    );
    let repositories = page["repositories"].as_array().expect("repositories");
    assert!(repositories.iter().any(|item| item["repository_id"] == id));
    assert!(repositories.iter().any(|item| item["repository_id"]
        == local.repository_id.as_deref().expect("local ID")
        && item["source_kind"] == "local"));
    assert!(!page.to_string().contains("local-fixture.json"));
    let builtin_id = alcomd_application::VRCHAT_OFFICIAL_REPOSITORY_ID;
    error(call(&writer,&mut trace,"remove_repository",json!({"repository_id":builtin_id.to_string(),"expected_revision":1,"idempotency_key":"protected-remove","confirm":true})).await.expect("protected built-in ID, even with builtin seeding disabled in the test"),"permission_denied");
    let refresh = json!({"repository_id":id,"expected_revision":registered["revision"],"idempotency_key":"refresh-registered"});
    source.set_changed(false);
    let refreshed = data(
        call(&writer, &mut trace, "refresh_repository", refresh.clone())
            .await
            .expect("refresh existing source"),
    );
    assert_eq!(refreshed["repository"]["display_name"], "M8 VPM fixture");
    assert_eq!(
        data(
            call(&writer, &mut trace, "refresh_repository", refresh)
                .await
                .expect("refresh recovery")
        )["replayed"],
        true
    );
    let remove = json!({"repository_id":id,"expected_revision":refreshed["revision"],"idempotency_key":"remove-registered","confirm":true});
    let removed = data(
        call(&writer, &mut trace, "remove_repository", remove.clone())
            .await
            .expect("remove original registry row"),
    );
    assert_eq!(removed["repository_id"], id);
    assert!(removed.get("repository").is_none());
    assert_eq!(
        data(
            call(&writer, &mut trace, "remove_repository", remove)
                .await
                .expect("remove recovery")
        )["replayed"],
        true
    );
    assert!(owner.repository_get(id.to_owned()).await.is_err());
    owner
        .mcp_client_revoke(McpClientMutationParams {
            client_id: a.client_id.clone(),
            expected_revision: a.revision,
            idempotency_key: "revoke-repository-client".into(),
        })
        .await
        .expect("permanently revoke repository client");
    assert_eq!(
        raw_tool(
            &http,
            write_token["access_token"].as_str().expect("token"),
            "alcomd_register_repository",
            register
        )
        .await
        .status(),
        401
    );
    {
        let db = rusqlite::Connection::open_with_flags(
            root.join("data/state.db"),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .expect("inspect only the disposable idempotency database");
        let mut rows = db.prepare("SELECT principal_id, request_fingerprint, response_json FROM idempotency_records WHERE method LIKE 'mcp.repositories.%'").expect("registry receipts");
        let snapshots = rows
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .expect("receipt rows")
            .collect::<Result<Vec<_>, _>>()
            .expect("complete receipt evidence");
        assert_eq!(snapshots.len(), 3);
        for (principal, input, result) in snapshots {
            assert_eq!(principal, format!("mcp:{}", a.client_id));
            assert!(!input.contains(&source.base) && !result.contains(&source.base));
            assert!(!result.contains("credentialReference") && !result.contains("downloadHeaders"));
        }
    }
    reader.cancel().await.expect("close reader");
    writer.cancel().await.expect("close writer");
    foreign.cancel().await.expect("close other manager");
    http.finish_server().await;
    drop(owner);
    drop(daemon);
}

async fn oauth_package_catalog_and_https_download(protected: bool) {
    use std::io::{Cursor, Write};
    const PACKAGE: &str = "com.example.m8.https";

    let _serial = REAL_CORE_LOCK.lock().await;
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/m8-oauth-package-tests")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&root).expect("owned package evidence root");
    let root = root.canonicalize().expect("isolated fixture path");
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    archive
        .start_file("package.json", options)
        .expect("manifest entry");
    archive
        .write_all(
            json!({"name":PACKAGE,"version":"1.0.0"})
                .to_string()
                .as_bytes(),
        )
        .expect("manifest");
    archive
        .start_file("fixture.txt", options)
        .expect("package payload entry");
    archive
        .write_all(b"verified anonymous HTTPS fixture")
        .expect("package payload");
    let archive = archive
        .finish()
        .expect("valid package archive")
        .into_inner();
    let source = if protected {
        super::extension_sources_flow::HttpsSource::protected_vpm(archive).await
    } else {
        super::extension_sources_flow::HttpsSource::anonymous_vpm(archive).await
    };
    let configuration = CoreConfiguration::new();
    let daemon = alcomd_daemon::test_support::start_with_package_source_until(
        configuration.ipc.clone(),
        DataConfig::isolated(root.join("data")),
        CERT,
        std::future::pending(),
    )
    .await
    .expect("isolated Core with explicit TLS fixture trust");
    let mut owner = AlcomdClient::connect(configuration.client.clone())
        .await
        .expect("isolated owner");
    let registered = owner
        .repository_register(
            RepositorySource::Remote {
                url: format!("{}{}", source.base, source.index_path),
            },
            "register-anonymous-https".into(),
        )
        .await
        .expect("verified HTTPS repository registration")
        .repository;
    let repository_id = registered.repository_id.expect("registered repository ID");
    let project_root = root.join("Project");
    project(&project_root, false);
    let project = owner
        .project_register(
            project_root.to_string_lossy().into_owned(),
            "register-oauth-package-project".into(),
        )
        .await
        .expect("register isolated project")
        .project;
    let project_id = project.project_id.expect("project ID");
    let revision = project.revision.expect("project revision");
    let installed = project_root.join("Packages").join(PACKAGE);
    let owner_plan = owner
        .package_plan_install(PackagePlanInstallParams {
            project_id: project_id.clone(),
            expected_revision: revision,
            package_id: PACKAGE.into(),
            version_range: None,
            repository_id: None,
            source: Some(PackageSourceSelector::Repository {
                repository_id: repository_id.clone(),
            }),
            include_prerelease: false,
            allow_unity_incompatible: false,
        })
        .await
        .expect("same package through the local owner's Core planning API");

    let http = Https::start(&root, configuration.client.clone()).await;
    let client = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "OAuth package HTTPS fixture".into(),
            idempotency_key: "oauth-package-client".into(),
        })
        .await
        .expect("create isolated client");
    let registration = registration(&mut owner, &http, &client, false)
        .await
        .registration;
    let code = http.code(&mut owner, &registration).await;
    let token = http.exchange(&registration, &code, None).await;
    let read_token = token["access_token"].as_str().expect("read token");
    let reader = http.sdk(read_token).await;
    let mut trace = Vec::new();
    let summaries = data(
        call(
            &reader,
            &mut trace,
            "search_packages",
            json!({"repository_id":repository_id}),
        )
        .await
        .expect("query authorized anonymous source"),
    );
    assert_eq!(summaries["items"][0]["package_id"], PACKAGE);
    let visible = reader
        .list_tools(None)
        .await
        .expect("read-only authorized directory");
    assert!(
        visible
            .tools
            .iter()
            .any(|tool| tool.name == "alcomd_plan_package_install")
    );
    assert!(
        visible
            .tools
            .iter()
            .any(|tool| tool.name == "alcomd_apply_package_changes")
    );
    let arguments = json!({"project_id":project_id,"expected_revision":revision,"package_id":PACKAGE,
        "version_selector":{"kind":"latest_compatible"},"source":{"kind":"repository","repository_id":repository_id}});
    let plan = data(
        call(&reader, &mut trace, "plan_package_install", arguments)
            .await
            .expect("real latest-compatible plan"),
    );
    assert_eq!(
        plan["plan"]["change_set"]["mutations"][0]["to_version"],
        "1.0.0"
    );
    assert_eq!(plan["sources"][0]["source"]["repository_id"], repository_id);
    assert_eq!(
        plan["plan"]["change_set_fingerprint"],
        owner_plan.change_set_fingerprint
    );
    assert!(
        !plan.to_string().contains(&source.base),
        "public plan excludes the artifact and repository locator"
    );
    assert!(!plan.to_string().contains("isolated-vpm-package-fixture"));
    assert!(!installed.exists(), "planning does not publish files");
    let apply = json!({"plan_id":plan["plan"]["plan_id"],"expected_revision":revision,"idempotency_key":"oauth-https-apply","confirm":true});
    let mut unconfirmed = apply.clone();
    unconfirmed["confirm"] = json!(false);
    assert!(
        call(&reader, &mut trace, "apply_package_changes", unconfirmed)
            .await
            .is_err()
    );
    assert!(
        !installed.exists(),
        "unconfirmed apply leaves no package files"
    );
    let accepted = data(
        call(&reader, &mut trace, "apply_package_changes", apply.clone())
            .await
            .expect("use original Plan"),
    );
    let operation = accepted["operation_id"]
        .as_str()
        .expect("actual operation ID");
    let result = terminal(&reader, &mut trace, operation).await;
    assert_eq!(result["state"], "succeeded", "{result}");
    error(
        call(&reader, &mut trace, "cancel_operation", json!({
            "operation_id":operation,"expected_revision":result["revision"],"idempotency_key":"cancel-after-completion",
        }))
        .await
        .expect("terminal task still reaches the shared Core cancellation rule"),
        "operation_not_cancellable",
    );
    assert_eq!(
        std::fs::read(installed.join("fixture.txt")).expect("installed archive payload"),
        b"verified anonymous HTTPS fixture"
    );
    let requests = source.requests.lock().await;
    assert!(requests.iter().any(|request| request == source.index_path));
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.as_str() == source.archive_path)
            .count(),
        1,
        "one verified archive fetch"
    );
    assert!(
        source
            .headers_clean
            .load(std::sync::atomic::Ordering::SeqCst),
        "the endpoint receives only its declared repository authorization, with no MCP OAuth credential or referrer"
    );
    drop(requests);
    let replay = data(
        call(&reader, &mut trace, "apply_package_changes", apply)
            .await
            .expect("existing idempotent result"),
    );
    assert_eq!(replay["operation_id"], operation);
    assert_eq!(replay["replayed"], true);
    std::fs::write(
        root.join("package-flow.json"),
        serde_json::to_vec_pretty(&trace).expect("redacted tool trace"),
    )
    .expect("record package evidence");
    reader.cancel().await.expect("close read client");
    http.finish_server().await;
    drop(owner);
    drop(daemon);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn oauth_live_subscriptions_follow_refresh_and_revocation() {
    use rmcp::model::SubscriptionFilter;
    let _serial = REAL_CORE_LOCK.lock().await;
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/m8-oauth-subscription-tests")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(root.join("data")).expect("owned disposable data");
    let root = root.canonicalize().expect("normalized isolation");
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
        .expect("native test owner");
    let mut client = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "OAuth subscription fixture".into(),
            idempotency_key: "oauth-stream-client".into(),
        })
        .await
        .expect("explicit initial read grant");
    let initial_credential = owner
        .mcp_client_issue_credential(McpClientMutationParams {
            client_id: client.client_id.clone(),
            expected_revision: client.revision,
            idempotency_key: "oauth-stream-initial-credential".into(),
        })
        .await
        .expect("initial credential generation");
    client.revision = initial_credential.revision;
    let http = Https::start(&root, core.client.clone()).await;
    let reg = registration(&mut owner, &http, &client, false)
        .await
        .registration;
    let code = http.code(&mut owner, &reg).await;
    let issued = http.exchange(&reg, &code, None).await;
    let access = issued["access_token"].as_str().expect("one-time access");
    let refresh = issued["refresh_token"].as_str().expect("one-time refresh");
    let a = http.sdk(access).await;
    let mut stream_a = a
        .listen(SubscriptionFilter::builder().tools_list_changed().build())
        .await
        .expect("authenticated HTTPS subscription ack");
    let initial_tools = a.list_all_tools().await.expect("all published tools");
    assert!(
        initial_tools
            .iter()
            .any(|tool| tool.name == "alcomd_apply_package_changes")
    );
    tokio::time::timeout(Duration::from_secs(7), async {
        loop {
            let snapshot = owner
                .mcp_transport_snapshot(Default::default())
                .await
                .expect("actual TLS observations");
            if snapshot
                .observers
                .iter()
                .any(|o| o.transport == McpTransportKind::HttpsOauth)
                && snapshot
                    .subscription_streams
                    .as_ref()
                    .expect("read group")
                    .items
                    .iter()
                    .any(|s| s.client_id.as_deref() == Some(client.client_id.as_str()))
            {
                let public = serde_json::to_string(&snapshot).expect("bounded public projection");
                assert!(!public.contains(access) && !public.contains(refresh));
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("OAuth identity bound through the actual TLS adapter");
    let response = http
        .token(
            &reg,
            &[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh),
                ("resource", &reg.resource),
            ],
            None,
        )
        .await;
    assert_eq!(response.status(), 200);
    let rotated: Value = response.json().await.expect("one-time rotated tokens");
    let rotated_access = rotated["access_token"].as_str().expect("new access");
    let rotated_refresh = rotated["refresh_token"].as_str().expect("new refresh");
    assert!(
        rotated_access != access,
        "refresh must issue a distinct access token"
    );
    assert!(rotated_refresh != refresh, "refresh secret must rotate");
    let b = http.sdk(rotated_access).await;
    let mut stream_b = b
        .listen(SubscriptionFilter::builder().tools_list_changed().build())
        .await
        .expect("new token creates its own stream");
    assert_eq!(
        b.list_all_tools()
            .await
            .expect("refresh preserves catalogue")
            .len(),
        initial_tools.len()
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(1200), stream_a.next())
            .await
            .is_err(),
        "normal refresh does not revoke an unexpired old access token"
    );
    let replay = http
        .token(
            &reg,
            &[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh),
                ("resource", &reg.resource),
            ],
            None,
        )
        .await;
    assert_eq!(replay.status(), 400);
    for stream in [&mut stream_a, &mut stream_b] {
        let end = tokio::time::timeout(Duration::from_secs(4), stream.next())
            .await
            .expect("family revocation deadline");
        assert!(
            !matches!(end, Ok(Some(_))),
            "revocation must end the stream, not publish a false empty catalogue"
        );
    }
    assert_eq!(
        raw_tool(&http, rotated_access, "alcomd_list_projects", json!({}))
            .await
            .status(),
        401
    );
    stop(a).await;
    stop(b).await;

    let code = http.code(&mut owner, &reg).await;
    let issued = http.exchange(&reg, &code, None).await;
    let c = http
        .sdk(issued["access_token"].as_str().expect("new family access"))
        .await;
    let mut stream_c = c
        .listen(SubscriptionFilter::builder().tools_list_changed().build())
        .await
        .expect("new family after revocation");
    owner
        .mcp_client_issue_credential(McpClientMutationParams {
            client_id: client.client_id.clone(),
            expected_revision: client.revision,
            idempotency_key: "oauth-stream-rotate-core-credential".into(),
        })
        .await
        .expect("explicit Core credential rotation");
    let end = tokio::time::timeout(Duration::from_secs(4), stream_c.next())
        .await
        .expect("credential generation change deadline");
    assert!(
        !matches!(end, Ok(Some(_))),
        "old generation cannot keep an OAuth stream alive"
    );
    stop(c).await;

    let code = http.code(&mut owner, &reg).await;
    let issued = http.exchange(&reg, &code, None).await;
    let d = http
        .sdk(
            issued["access_token"]
                .as_str()
                .expect("current generation access"),
        )
        .await;
    let mut stream_d = d
        .listen(SubscriptionFilter::builder().tools_list_changed().build())
        .await
        .expect("current generation");
    let rotated_reg = owner
        .oauth_registration_set(McpOAuthRegistrationSetParams {
            client_id: client.client_id,
            issuer: reg.issuer.clone(),
            resource: reg.resource.clone(),
            redirect_uris: reg.redirect_uris.clone(),
            confidential: false,
            allow_refresh: true,
            expected_revision: reg.revision,
            idempotency_key: "oauth-stream-rotate-registration".into(),
        })
        .await
        .expect("explicit registration rotation")
        .registration;
    let end = tokio::time::timeout(Duration::from_secs(4), stream_d.next())
        .await
        .expect("registration revision deadline");
    assert!(
        !matches!(end, Ok(Some(_))),
        "old registration revision cannot keep a stream alive"
    );
    stop(d).await;
    let code = http.code(&mut owner, &rotated_reg).await;
    let issued = http.exchange(&rotated_reg, &code, None).await;
    let e = http
        .sdk(
            issued["access_token"]
                .as_str()
                .expect("rotated registration access"),
        )
        .await;
    let mut stream_e = e
        .listen(SubscriptionFilter::builder().tools_list_changed().build())
        .await
        .expect("rotated registration");
    owner
        .oauth_registration_revoke(McpOAuthRegistrationRevokeParams {
            oauth_client_id: rotated_reg.oauth_client_id,
            expected_revision: rotated_reg.revision,
            idempotency_key: "oauth-stream-revoke-registration".into(),
        })
        .await
        .expect("explicit final registration revocation");
    let end = tokio::time::timeout(Duration::from_secs(4), stream_e.next())
        .await
        .expect("registration revoke deadline");
    assert!(
        !matches!(end, Ok(Some(_))),
        "revoked registration cannot keep a stream alive"
    );
    stop(e).await;
    http.finish_server().await;
    drop(owner);
    daemon.abort();
    let _ = daemon.await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "uses the real ten-minute access token lifetime and an isolated Core endpoint"]
async fn oauth_subscription_ends_at_natural_access_expiry() {
    use rmcp::model::SubscriptionFilter;

    let _serial = REAL_CORE_LOCK.lock().await;
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/m8-oauth-subscription-expiry")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(root.join("data")).expect("owned disposable data");
    let root = root.canonicalize().expect("normalized isolation");
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
        .expect("native test owner");
    let client = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "OAuth subscription expiry fixture".into(),
            idempotency_key: "natural-expiry-client".into(),
        })
        .await
        .expect("explicit read grant");
    let http = Https::start(&root, core.client.clone()).await;
    let reg = registration(&mut owner, &http, &client, false)
        .await
        .registration;
    let code = http.code(&mut owner, &reg).await;
    let exchange_started = tokio::time::Instant::now();
    let issued = http.exchange(&reg, &code, None).await;
    let received = tokio::time::Instant::now();
    let lifetime = Duration::from_secs(600);
    assert_eq!(issued["expires_in"].as_u64(), Some(lifetime.as_secs()));
    let access = issued["access_token"].as_str().expect("one-time access");
    // Separate the token's natural expiry from the independent ten-minute stream limit.
    tokio::time::sleep(Duration::from_secs(25)).await;
    let sdk = http.sdk(access).await;
    let listen_started = tokio::time::Instant::now();
    let mut stream = sdk
        .listen(SubscriptionFilter::builder().tools_list_changed().build())
        .await
        .expect("authenticated HTTPS subscription ack");
    let tools = sdk.list_all_tools().await.expect("published tools");
    assert!(
        tools
            .iter()
            .any(|tool| tool.name == "alcomd_apply_package_changes")
    );
    let end = tokio::time::timeout_at(received + lifetime + Duration::from_secs(8), stream.next())
        .await
        .expect("stream closes at token expiry");
    assert!(
        !matches!(end, Ok(Some(_))),
        "expiry must not publish a false catalogue change"
    );
    let since_exchange = exchange_started.elapsed();
    let since_listen = listen_started.elapsed();
    assert!(since_exchange >= lifetime - Duration::from_secs(2));
    assert!(
        since_listen < lifetime - Duration::from_secs(10),
        "the token must expire before the independent stream limit"
    );
    assert_eq!(
        raw_tool(&http, access, "alcomd_list_projects", json!({}))
            .await
            .status(),
        401
    );
    let new_code = http.code(&mut owner, &reg).await;
    let replacement = http.exchange(&reg, &new_code, None).await;
    let fresh = http
        .sdk(replacement["access_token"].as_str().expect("fresh access"))
        .await;
    assert_eq!(
        fresh
            .list_all_tools()
            .await
            .expect("fresh access works")
            .len(),
        tools.len()
    );
    eprintln!(
        "Natural expiry closed the stream after {} ms ({} ms after exchange start)",
        since_listen.as_millis(),
        since_exchange.as_millis()
    );
    stop(fresh).await;
    stop(sdk).await;
    http.finish_server().await;
    drop(owner);
    daemon.abort();
    let _ = daemon.await;
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
    let a = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "Cursor A".into(),
            idempotency_key: "cursor-a".into(),
        })
        .await
        .expect("A");
    let b = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "Cursor B".into(),
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
    let code = http.code(&mut owner, &reg_a).await;
    let issued = http.exchange(&reg_a, &code, None).await;
    let access = issued["access_token"].as_str().expect("first access");
    let code = http.code(&mut owner, &reg_a).await;
    let other_issued = http.exchange(&reg_a, &code, None).await;
    let other_access = other_issued["access_token"]
        .as_str()
        .expect("same principal second token");
    let code = http.code(&mut owner, &reg_b).await;
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
            json!({"limit":1,"cursor":cursor})
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
async fn frozen_sdk_oauth_real_core_identity_and_package_flow() {
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
        matches!(ingress.oauth_request_get(McpOAuthRequestGetParams{request_id:uuid::Uuid::new_v4().to_string()}).await,Err(alcomd_client::ClientError::Remote(ref e)) if e.code=="permission_denied")
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
    let a = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "OAuth A".into(),
            idempotency_key: "oauth-a".into(),
        })
        .await
        .expect("client A");
    let b = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "OAuth B".into(),
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
        })
        .await
        .expect("zero review");
    assert_eq!(zero_review.scopes, ["mcp"]);
    owner
        .oauth_request_decide(McpOAuthRequestDecideParams {
            request_id: zero.clone(),
            approve: false,
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
    let code = http.code(&mut owner, &reg).await;
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
    let code_b = http.code(&mut owner, &reg_b).await;
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
    let mut names: Vec<_> = sa
        .list_tools(None)
        .await
        .expect("authorized tools")
        .tools
        .into_iter()
        .map(|tool| tool.name.into_owned())
        .collect();
    names.sort_unstable();
    let comparison = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "STDIO catalogue comparison".into(),
            idempotency_key: "compare-stdio".into(),
        })
        .await
        .expect("transport comparison client");
    let stdio = crate::sdk(
        &owner
            .mcp_client_issue_credential(McpClientMutationParams {
                client_id: comparison.client_id,
                expected_revision: comparison.revision,
                idempotency_key: "compare-stdio-catalog".into(),
            })
            .await
            .expect("separate transport client")
            .credential,
        None,
    )
    .await;
    let mut stdio_names: Vec<_> = stdio
        .list_all_tools()
        .await
        .expect("STDIO catalogue")
        .into_iter()
        .map(|tool| tool.name.into_owned())
        .collect();
    stdio_names.sort_unstable();
    assert_eq!(names, stdio_names);
    assert!(
        names
            .iter()
            .any(|name| name == "alcomd_apply_package_changes")
    );
    stop(stdio).await;
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
        persistent
            .state_check("connected-state-check".into())
            .await
            .is_ok()
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
            display_name: "OAuth confidential client".into(),
            idempotency_key: "oauth-c".into(),
        })
        .await
        .expect("client C");
    let registered_c = registration(&mut owner, &http, &c, true).await;
    let secret_c = registered_c.client_secret.expect("one-time client secret");
    let reg_c = registered_c.registration;
    let code_c = http.code(&mut owner, &reg_c).await;
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
    let code = http.code(owner, &reg).await;
    let first = http.exchange(&reg, &code, None).await;
    let access = first["access_token"]
        .as_str()
        .expect("valid isolated Provider OAuth fixture");
    let code = http.code(owner, &reg).await;
    let second = http.exchange(&reg, &code, None).await;
    let other = second["access_token"]
        .as_str()
        .expect("valid isolated Provider OAuth fixture");
    let sa = http.sdk(access).await;
    let sb = http.sdk(other).await;
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
    let result = sb
        .call_tool_once(
            CallToolRequestParams::new(pure).with_arguments(
                json!({"text":"authenticated OAuth connection"})
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
    http.finish_server().await;
    operation
}
