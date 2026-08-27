use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use alcomd_client::{AlcomdClient, ClientConfig};
use alcomd_platform::{DataConfig, IpcConfig};
use alcomd_protocol::{ProjectType, RepositorySource};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn project_and_repository_read_slice_round_trips_without_source_writes() {
    let fixture = TestDirectory::new();
    let runtime = fixture.path().join("runtime");
    let data = fixture.path().join("data");
    fs::create_dir(&runtime).expect("create runtime directory");
    fs::create_dir(&data).expect("create data directory");
    let project = fixture.path().join("Project");
    create_project(&project);
    let repository = fixture.path().join("repository.json");
    fs::write(&repository, repository_document()).expect("write repository");
    let project_before = digest_inputs(&project, &repository);

    let (ipc, config) = isolated_ipc(runtime);
    let shutdown = Arc::new(AtomicBool::new(false));
    let server_shutdown = Arc::clone(&shutdown);
    let daemon = tokio::spawn(async move {
        alcomd_daemon::serve_with_data_until(
            ipc,
            DataConfig::isolated(data),
            wait_for_shutdown(server_shutdown),
        )
        .await
    });
    let mut client = connect_with_retry(config).await;

    let inspected = client
        .project_inspect(
            project.to_string_lossy().into_owned(),
            alcomd_protocol::ProjectDiscoveryMode::ExactRoot,
        )
        .await
        .expect("inspect project");
    assert_eq!(inspected.project.project_type, ProjectType::Avatars);
    assert!(inspected.project.project_id.is_none());
    assert!(inspected.project.registered_at_ms.is_none());
    let registered = client
        .project_register(
            project.to_string_lossy().into_owned(),
            "m3-project-register".to_owned(),
        )
        .await
        .expect("register project");
    assert_eq!(registered.project.revision, Some(1));
    let registered_at_ms = registered
        .project
        .registered_at_ms
        .expect("registered project timestamp");
    let project_id = registered.project.project_id.expect("registered ID");
    let no_op = client
        .project_refresh(project_id.clone(), 1, "m3-project-refresh".to_owned())
        .await
        .expect("refresh project");
    assert_eq!(no_op.project.revision, Some(1));
    assert_eq!(no_op.project.registered_at_ms, Some(registered_at_ms));
    let projects = client
        .projects_list(None, Some(100))
        .await
        .expect("list projects")
        .projects;
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].registered_at_ms, Some(registered_at_ms));

    let source = RepositorySource::Local {
        path: repository.to_string_lossy().into_owned(),
    };
    let inspected_repository = client
        .repository_inspect(source.clone())
        .await
        .expect("inspect repository");
    assert_eq!(
        inspected_repository.repository.name.as_deref(),
        Some("Fixture")
    );
    let registered_repository = client
        .repository_register(source, "m3-repository-register".to_owned())
        .await
        .expect("register repository");
    let repository_id = registered_repository
        .repository
        .repository_id
        .expect("registered repository ID");
    let packages = client
        .repository_packages(repository_id.clone(), None, Some(100))
        .await
        .expect("list package identities");
    assert_eq!(packages.packages.len(), 1);
    assert_eq!(packages.packages[0].package_id, "com.example.fixture");
    fs::write(&repository, "{").expect("write malformed repository");
    let failed = client
        .repository_refresh(
            repository_id.clone(),
            1,
            "m3-repository-refresh-failed".to_owned(),
        )
        .await
        .expect_err("malformed refresh must fail");
    assert!(matches!(
        failed,
        alcomd_client::ClientError::Remote(ref error)
            if error.code == alcomd_protocol::error_code::REPOSITORY_DOCUMENT_INVALID
    ));
    let preserved = client
        .repository_packages(repository_id.clone(), None, Some(100))
        .await
        .expect("last-known-good packages");
    assert_eq!(preserved.packages, packages.packages);
    fs::write(&repository, repository_document()).expect("restore repository");
    let no_op = client
        .repository_refresh(repository_id, 1, "m3-repository-refresh".to_owned())
        .await
        .expect("refresh repository");
    assert_eq!(no_op.repository.revision, Some(1));

    assert_eq!(project_before, digest_inputs(&project, &repository));
    shutdown.store(true, Ordering::Release);
    let result = tokio::time::timeout(Duration::from_secs(3), daemon)
        .await
        .expect("daemon stop timeout")
        .expect("join daemon");
    assert!(result.is_ok());
}

#[tokio::test]
async fn anonymous_http_refresh_uses_validators_and_keeps_last_known_good() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock server");
    let address = listener.local_addr().expect("mock address");
    let server = tokio::spawn(async move {
        for status in [200_u16, 304_u16] {
            let (mut stream, _) = listener.accept().await.expect("accept request");
            let mut request = vec![0_u8; 4096];
            let size = stream.read(&mut request).await.expect("read request");
            let request = String::from_utf8_lossy(&request[..size]);
            if status == 304 {
                assert!(
                    request
                        .to_ascii_lowercase()
                        .contains("if-none-match: \"fixture\"")
                );
                stream
                    .write_all(b"HTTP/1.1 304 Not Modified\r\nConnection: close\r\n\r\n")
                    .await
                    .expect("write 304");
            } else {
                let body = repository_document();
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: \"fixture\"\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream
                    .write_all(response.as_bytes())
                    .await
                    .expect("write 200");
            }
        }
    });
    let reader = alcomd_vpm::VpmReader::new().expect("reader");
    use alcomd_application::{M3ReadAdapter, RepositoryReadOutcome, RepositoryValidators};
    let source = alcomd_application::RepositorySource::Remote {
        url: format!("http://{address}/repository.json?channel=stable#ignored"),
    };
    let first = reader
        .inspect_repository(source.clone(), None)
        .await
        .expect("initial fetch");
    let RepositoryReadOutcome::Fresh(first) = first else {
        panic!("first response must be fresh");
    };
    assert!(
        matches!(first.source, alcomd_application::RepositorySource::Remote { ref url } if url.ends_with("?channel=stable"))
    );
    let second = reader
        .inspect_repository(
            source,
            Some(RepositoryValidators {
                etag: Some("\"fixture\"".to_owned()),
                last_modified: None,
            }),
        )
        .await
        .expect("conditional fetch");
    assert!(matches!(
        second,
        RepositoryReadOutcome::NotModified(RepositoryValidators { etag: Some(ref value), .. })
            if value == "\"fixture\""
    ));
    server.await.expect("mock server");
}

#[tokio::test]
async fn anonymous_http_rejects_declared_body_over_the_frozen_limit() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock server");
    let address = listener.local_addr().expect("mock address");
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept request");
        let mut request = [0_u8; 1024];
        let _ = stream.read(&mut request).await.expect("read request");
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 16777217\r\nConnection: close\r\n\r\n")
            .await
            .expect("write oversized response");
    });
    use alcomd_application::{M3ErrorCode, M3ReadAdapter};
    let error = alcomd_vpm::VpmReader::new()
        .expect("reader")
        .inspect_repository(
            alcomd_application::RepositorySource::Remote {
                url: format!("http://{address}/repository.json"),
            },
            None,
        )
        .await
        .expect_err("reject oversized response");
    assert_eq!(error.code(), M3ErrorCode::RepositoryDocumentTooLarge);
    server.await.expect("mock server");
}

fn create_project(root: &Path) {
    fs::create_dir_all(root.join("ProjectSettings")).expect("create ProjectSettings");
    fs::create_dir_all(root.join("Packages")).expect("create Packages");
    fs::write(
        root.join("ProjectSettings/ProjectVersion.txt"),
        "m_EditorVersion: 2022.3.22f1\nm_EditorVersionWithRevision: 2022.3.22f1 (fixture)\n",
    )
    .expect("write ProjectVersion");
    fs::write(
        root.join("Packages/vpm-manifest.json"),
        r#"{"dependencies":{"com.vrchat.avatars":"3.7.0"},"locked":{"com.vrchat.avatars":{"version":"3.7.0"}}}"#,
    )
    .expect("write vpm manifest");
}

fn repository_document() -> &'static str {
    r#"{"id":"fixture","name":"Fixture","packages":{"com.example.fixture":{"versions":{"1.0.0":{"name":"com.example.fixture","version":"1.0.0","displayName":"Example"}}}}}"#
}

fn digest_inputs(project: &Path, repository: &Path) -> Vec<Vec<u8>> {
    [
        project.join("ProjectSettings/ProjectVersion.txt"),
        project.join("Packages/vpm-manifest.json"),
        repository.to_path_buf(),
    ]
    .into_iter()
    .map(|path| fs::read(path).expect("read fixture"))
    .collect()
}

async fn connect_with_retry(config: ClientConfig) -> AlcomdClient {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        match AlcomdClient::connect(config.clone()).await {
            Ok(client) => return client,
            Err(_) if tokio::time::Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            Err(error) => panic!("daemon did not become ready: {error}"),
        }
    }
}

async fn wait_for_shutdown(shutdown: Arc<AtomicBool>) {
    while !shutdown.load(Ordering::Acquire) {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[cfg(unix)]
fn isolated_ipc(runtime: PathBuf) -> (IpcConfig, ClientConfig) {
    (
        IpcConfig::isolated(runtime.clone()),
        ClientConfig::default()
            .without_daemon_start()
            .with_runtime_directory(runtime),
    )
}

#[cfg(windows)]
fn isolated_ipc(_runtime: PathBuf) -> (IpcConfig, ClientConfig) {
    (
        IpcConfig::default(),
        ClientConfig::default().without_daemon_start(),
    )
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        #[cfg(target_os = "macos")]
        let base = PathBuf::from("/private/tmp");
        #[cfg(not(target_os = "macos"))]
        let base = std::env::temp_dir();
        let path = base.join(format!("alcomd-m3-rpc-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).expect("create fixture root");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
