//! Actual HTTPS source -> typed RPC -> Core -> signed package -> durable install.
use super::*;
use alcomd_application::ExtensionSourceFetcher;
use sha2::{Digest, Sha256};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
const CERT: &[u8] = include_bytes!(
    "../../../../crates/alcomd-testing/fixtures/m8/oauth-localhost.test-certificate.der"
);
const KEY: &[u8] =
    include_bytes!("../../../../crates/alcomd-testing/fixtures/m8/oauth-localhost.test-key.pkcs8");
const ID: &str = "dev.alcomd.tools-fixture";
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
struct HttpsSource {
    base: String,
    changed: Arc<AtomicBool>,
    requests: Arc<tokio::sync::Mutex<Vec<String>>>,
    headers_clean: Arc<AtomicBool>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for HttpsSource {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl HttpsSource {
    async fn start(archive: Vec<u8>) -> Self {
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("local TLS source");
        let port = listener.local_addr().expect("source port").port();
        let base = format!("https://localhost:{port}");
        let tls = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![rustls::pki_types::CertificateDer::from(CERT.to_vec())],
                rustls::pki_types::PrivateKeyDer::Pkcs8(
                    rustls::pki_types::PrivatePkcs8KeyDer::from(KEY.to_vec()),
                ),
            )
            .expect("public fixture TLS");
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(tls));
        let changed = Arc::new(AtomicBool::new(false));
        let updated = changed.clone();
        let requests = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let observed = requests.clone();
        let headers_clean = Arc::new(AtomicBool::new(true));
        let header_check = headers_clean.clone();
        let origin = base.clone();
        let archive = Arc::new(archive);
        let task = tokio::spawn(async move {
            let mut connections = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    accepted=listener.accept()=>{
                        let Ok((stream,_))=accepted else {break};
                        let acceptor=acceptor.clone();let archive=archive.clone();let origin=origin.clone();let updated=updated.clone();let observed=observed.clone();let header_check=header_check.clone();
                        connections.spawn(async move {
                            let Ok(mut stream)=acceptor.accept(stream).await else {return};
                            let mut request=Vec::new();let mut buffer=[0;1024];
                            while !request.windows(4).any(|b|b==b"\r\n\r\n") {
                                let Ok(n)=stream.read(&mut buffer).await else{return};if n==0||request.len()+n>16384{return}request.extend_from_slice(&buffer[..n]);
                            }
                            let request=String::from_utf8(request).expect("HTTP ASCII request");
                            if request.to_ascii_lowercase().contains("authorization:") || request.to_ascii_lowercase().contains("referer:") {header_check.store(false,Ordering::SeqCst);}
                            let path=request.lines().next().and_then(|l|l.split_whitespace().nth(1)).expect("request target");observed.lock().await.push(path.into());
                            if path=="/downgrade"||path=="/redirect"||path=="/loop"||path.strip_prefix("/redirects/").and_then(|v|v.parse::<u32>().ok()).is_some_and(|n|n>0) {
                                let location=if path=="/downgrade" {"http://localhost:1/forbidden".into()} else if path=="/loop" {format!("{origin}/loop")} else if let Some(n)=path.strip_prefix("/redirects/").and_then(|v|v.parse::<u32>().ok()) {format!("{origin}/redirects/{}",n-1)} else {format!("{origin}/index?b=2&a=1&a=3")};
                                let response=format!("HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");let _=stream.write_all(response.as_bytes()).await;return;
                            }
                            if path=="/oversize" {
                                let _=stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1048577\r\nConnection: close\r\n\r\n").await;return;
                            }
                            if path=="/chunked-oversize" {
                                let _=stream.write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n").await;
                                for _ in 0..17 { if stream.write_all(b"10000\r\n").await.is_err(){return}
                            if stream.write_all(&vec![b' ';65536]).await.is_err(){return}
                            if stream.write_all(b"\r\n").await.is_err(){return} }
                                let _=stream.write_all(b"0\r\n\r\n").await;return;
                            }
                            if path=="/huge-archive" {
                                let _=stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 67108865\r\nConnection: close\r\n\r\n").await;return;
                            }
                            let bytes=if path=="/extension.alcomdextension" {archive.to_vec()} else {
                                let key=ed25519_dalek::SigningKey::from_bytes(&[23;32]).verifying_key().to_bytes();
                                let id=if path=="/wrong-id" {"dev.example.other"}else{ID};
                                let version=if path=="/wrong-version" {"1.0.1"}else{"1.0.0"};
                                let publisher=if path=="/wrong-publisher" {format!("ed25519-sha256:{}","0".repeat(64))}else{format!("ed25519-sha256:{}",hex(&Sha256::digest(key)))};
                                let sha=if path=="/wrong-hash" {"0".repeat(64)}else{hex(&Sha256::digest(archive.as_slice()))};
                                let artifact=if path=="/archive-limit" {format!("{origin}/huge-archive")}else{format!("{origin}/extension.alcomdextension")};
                                serde_json::to_vec(&json!({"format":"alcomd-extension-source-v1","packages":{id:{"displayName":if updated.load(Ordering::SeqCst){"Changed"}else{"Example"},"publisherFingerprint":publisher,"versions":{version:{"apiMajor":1,"url":artifact,"zipSHA256":sha}}}}})).expect("index")
                            };
                            let header=format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",bytes.len());let _=stream.write_all(header.as_bytes()).await;let _=stream.write_all(&bytes).await;let _=stream.shutdown().await;
                        });
                    },
                    _=connections.join_next(),if !connections.is_empty()=>{}
                }
            }
        });
        Self {
            base,
            changed,
            requests,
            headers_clean,
            task,
        }
    }
}
async fn rejected<T>(result: Result<T, alcomd_client::ClientError>, code: &str) {
    match result {
        Err(alcomd_client::ClientError::Remote(e)) => assert_eq!(e.code, code),
        Err(_) => panic!("expected stable remote refusal {code}"),
        Ok(_) => panic!("unexpected successful request, expected {code}"),
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn remote_extension_source_real_https_install_and_refusals() {
    let _serial = REAL_CORE_LOCK.lock().await;
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/m8-remote-source-tests")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&root).expect("owned test directory");
    let root = root.canonicalize().expect("absolute test directory");
    let package = root.join("fixture.alcomdextension");
    extension_tools_flow::package_with_install_requirements(&package, true);
    let archive = std::fs::read(&package).expect("signed fixture");
    let source = HttpsSource::start(archive).await;
    let cache = root.join("cache");
    let fetcher = Arc::new(
        alcomd_vpm::ExtensionSourceHttp::with_test_certificate(cache.clone(), CERT)
            .expect("isolated explicit TLS trust"),
    );
    // The normal transport has no fixture trust. No trust-store writes or invalid-cert bypass.
    let normal = alcomd_vpm::ExtensionSourceHttp::new(root.join("untrusted-cache"), None)
        .expect("normal transport");
    assert_eq!(
        normal
            .index(&format!("{}/index", source.base))
            .await
            .err()
            .expect("untrusted TLS refused")
            .code(),
        alcomd_application::M6ErrorCode::SourceUnavailable
    );
    for (path, code) in [
        ("/downgrade", "extension_source_unavailable"),
        ("/loop", "extension_source_unavailable"),
        ("/oversize", "extension_source_invalid"),
        ("/chunked-oversize", "extension_source_invalid"),
    ] {
        let result = fetcher.index(&format!("{}{path}", source.base)).await;
        let expected = if code == "extension_source_invalid" {
            alcomd_application::M6ErrorCode::SourceInvalid
        } else {
            alcomd_application::M6ErrorCode::SourceUnavailable
        };
        assert_eq!(result.err().expect("transport refusal").code(), expected);
    }
    fetcher
        .index(&format!("{}/redirects/10", source.base))
        .await
        .expect("ten HTTPS redirects allowed");
    assert_eq!(
        fetcher
            .index(&format!("{}/redirects/11", source.base))
            .await
            .err()
            .expect("eleventh redirect refused")
            .code(),
        alcomd_application::M6ErrorCode::SourceUnavailable
    );
    let core = oauth_flow::CoreConfiguration::new();
    let data = root.join("data");
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let daemon = alcomd_daemon::test_support::start_with_extension_source_until(
        core.ipc.clone(),
        DataConfig::isolated(data.clone()),
        fetcher,
        async {
            let _ = stopped.await;
        },
    )
    .await
    .expect("exclusive isolated Core");
    let mut owner = AlcomdClient::connect(core.client.clone())
        .await
        .expect("real Core client");
    assert!(
        owner
            .system_status()
            .await
            .expect("Core status")
            .capabilities
            .iter()
            .any(|c| c == CAPABILITY_EXTENSIONS_REMOTE_INSTALL_V1)
    );
    let created = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "source-denied-client".into(),
            grants: vec![],
            idempotency_key: "source-client-create".into(),
        })
        .await
        .expect("zero-grant MCP identity");
    let issued = owner
        .mcp_client_issue_credential(McpClientMutationParams {
            client_id: created.client_id,
            expected_revision: created.revision,
            idempotency_key: "source-client-issue".into(),
        })
        .await
        .expect("isolated client credential");
    let mut external =
        AlcomdClient::connect(core.client.clone().with_mcp_credential(issued.credential))
            .await
            .expect("real external Principal");
    let request_count = source.requests.lock().await.len();
    rejected(
        external
            .extension_inspect_source(ExtensionInspectSourceParams {
                url: format!("{}/index", source.base),
            })
            .await,
        "permission_denied",
    )
    .await;
    assert_eq!(
        source.requests.lock().await.len(),
        request_count,
        "unprivileged RPC rejected before network"
    );
    drop(external);
    let url = format!("{}/redirect", source.base);
    let preview = owner
        .extension_inspect_source(ExtensionInspectSourceParams { url: url.clone() })
        .await
        .expect("source preview");
    assert_eq!(preview.extension_id, ID);
    assert_eq!(preview.url, url);
    let reads = source
        .requests
        .lock()
        .await
        .iter()
        .filter(|p| *p == "/extension.alcomdextension")
        .count();
    source.changed.store(true, Ordering::SeqCst);
    rejected(
        owner
            .extension_plan_install_from_source(ExtensionPlanInstallFromSourceParams {
                url: url.clone(),
                version: "1.0.0".into(),
                expected_source_fingerprint: preview.source_fingerprint,
            })
            .await,
        "revision_conflict",
    )
    .await;
    assert_eq!(
        source
            .requests
            .lock()
            .await
            .iter()
            .filter(|p| *p == "/extension.alcomdextension")
            .count(),
        reads,
        "changed index refused before artifact download"
    );
    source.changed.store(false, Ordering::SeqCst);
    for (path, version, code) in [
        ("/wrong-hash", "1.0.0", "package_integrity_mismatch"),
        ("/archive-limit", "1.0.0", "package_download_too_large"),
        ("/wrong-id", "1.0.0", "extension_source_invalid"),
        ("/wrong-version", "1.0.1", "extension_source_invalid"),
        ("/wrong-publisher", "1.0.0", "extension_source_invalid"),
    ] {
        let url = format!("{}{path}", source.base);
        let preview = owner
            .extension_inspect_source(ExtensionInspectSourceParams { url: url.clone() })
            .await
            .expect("parse index");
        rejected(
            owner
                .extension_plan_install_from_source(ExtensionPlanInstallFromSourceParams {
                    url,
                    version: version.into(),
                    expected_source_fingerprint: preview.source_fingerprint,
                })
                .await,
            code,
        )
        .await;
    }
    assert!(
        owner
            .extensions_list(ExtensionsListParams {
                cursor: None,
                limit: Some(100)
            })
            .await
            .expect("no installation")
            .extensions
            .is_empty()
    );
    let preview = owner
        .extension_inspect_source(ExtensionInspectSourceParams { url: url.clone() })
        .await
        .expect("reread source");
    let plan = owner
        .extension_plan_install_from_source(ExtensionPlanInstallFromSourceParams {
            url: url.clone(),
            version: "1.0.0".into(),
            expected_source_fingerprint: preview.source_fingerprint,
        })
        .await
        .expect("real signed Plan")
        .plan;
    assert_eq!(plan.source_kind, "remote_extension_source");
    assert_eq!(plan.installation_source.as_ref().expect("origin").url, url);
    assert!(plan.installation_terms.is_some());
    assert_eq!(plan.required_permissions, ["projects.read"]);
    for (terms, permissions) in [(false, false), (true, false), (false, true)] {
        rejected(
            owner
                .extension_apply_install(ExtensionInstallApplyParams {
                    plan_id: plan.plan_id.clone(),
                    idempotency_key: uuid::Uuid::new_v4().to_string(),
                    consent: ExtensionInstallConsent {
                        plan_fingerprint: plan.plan_fingerprint.clone(),
                        installation_terms_accepted: terms,
                        required_permissions_accepted: permissions,
                    },
                })
                .await,
            "permission_denied",
        )
        .await;
    }
    let params = ExtensionInstallApplyParams {
        plan_id: plan.plan_id.clone(),
        idempotency_key: uuid::Uuid::new_v4().to_string(),
        consent: ExtensionInstallConsent {
            plan_fingerprint: plan.plan_fingerprint.clone(),
            installation_terms_accepted: true,
            required_permissions_accepted: true,
        },
    };
    let accepted = owner
        .extension_apply_install(params.clone())
        .await
        .expect("explicit final install");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        let op = owner
            .operation_get(accepted.operation_id.clone())
            .await
            .expect("task");
        match op.state {
            OperationState::Succeeded => break,
            OperationState::Failed | OperationState::Cancelled => {
                panic!("unexpected terminal install result {op:?}")
            }
            _ => {}
        }
        assert!(tokio::time::Instant::now() < deadline, "install deadline");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let replay = owner
        .extension_apply_install(params)
        .await
        .expect("exact replay");
    assert_eq!(replay.operation_id, accepted.operation_id);
    assert!(replay.replayed);
    let installed = owner
        .extension_get(ID.into())
        .await
        .expect("installed detail");
    assert_eq!(installed.installation_source, plan.installation_source);
    let permissions = owner
        .extension_permissions_list(ExtensionPermissionsListParams {
            extension_id: ID.into(),
            cursor: None,
            limit: Some(100),
        })
        .await
        .expect("authority grants");
    assert_eq!(permissions.grants.len(), 1);
    assert_eq!(permissions.grants[0].permission, "projects.read");
    drop(owner);
    stop.send(()).expect("stop owned Core");
    daemon.await.expect("join Core").expect("graceful Core");
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let transport = Arc::new(
        alcomd_vpm::ExtensionSourceHttp::with_test_certificate(cache, CERT).expect("verified TLS"),
    );
    let daemon = alcomd_daemon::test_support::start_with_extension_source_until(
        core.ipc.clone(),
        DataConfig::isolated(data),
        transport,
        async {
            let _ = stopped.await;
        },
    )
    .await
    .expect("restart same isolated data");
    let mut owner = AlcomdClient::connect(core.client.clone())
        .await
        .expect("reconnect");
    assert_eq!(
        owner
            .extension_get(ID.into())
            .await
            .expect("persistent origin")
            .installation_source,
        plan.installation_source
    );
    assert!(
        source.headers_clean.load(Ordering::SeqCst),
        "anonymous redirect/archive requests must not carry auth or Referer"
    );
    let requests = source.requests.lock().await.clone();
    assert!(requests.iter().any(|p| p == "/index?b=2&a=1&a=3"));
    std::fs::write(root.join("result.json"),serde_json::to_vec_pretty(&json!({"result":"passed","origin":plan.installation_source,"operationId":accepted.operation_id,"dataSchema":42,"requests":requests})).expect("evidence")).expect("persist owned evidence");
    drop(owner);
    stop.send(()).expect("stop owned Core");
    daemon.await.expect("join Core").expect("graceful Core");
}
