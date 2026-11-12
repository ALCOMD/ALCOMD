use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use alcomd_client::{AlcomdClient, ClientConfig, ClientError};
use alcomd_platform::{DataConfig, IpcConfig};
use alcomd_protocol::{ProjectSetFavoriteParams, ProjectType, RepositorySource};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn directory_state_is_a_fresh_root_only_projection_without_registry_mutation() {
    use alcomd_application::{
        AccessContext, IdempotencyKey, M3Application, M3RegistryStore, PrincipalId,
        ProjectDirectoryState,
    };

    let fixture = TestDirectory::new();
    let source = fixture.path().join("中文项目");
    create_project(&source);
    let store = alcomd_store::StateStoreHandle::open(fixture.path().join("directory-state.db"))
        .expect("store");
    let registry = M3Application::new(store.clone(), alcomd_vpm::VpmReader::new().expect("reader"));
    let access = AccessContext::local_owner();
    let registered = registry
        .register_project(
            &access,
            source.to_string_lossy().into_owned(),
            IdempotencyKey::parse("directory-state-source").expect("key"),
        )
        .await
        .expect("register")
        .value;
    let id = registered.project_id;
    let read_state = |page: alcomd_application::ProjectPage| page.directory_states[&id];
    assert_eq!(
        read_state(
            registry
                .list_projects(&access, None, 100)
                .await
                .expect("available list")
        ),
        ProjectDirectoryState::Available
    );

    // Directory observation must not parse or hash the contents again.
    fs::remove_file(source.join("ProjectSettings/ProjectVersion.txt"))
        .expect("remove version fixture");
    assert_eq!(
        registry
            .get_project(&access, id)
            .await
            .expect("root still exists")
            .directory_state,
        ProjectDirectoryState::Available
    );
    fs::remove_dir_all(&source).expect("remove source fixture");
    assert_eq!(
        read_state(
            registry
                .list_projects(&access, None, 100)
                .await
                .expect("missing list")
        ),
        ProjectDirectoryState::Missing
    );
    let missing = registry
        .get_project(&access, id)
        .await
        .expect("missing detail");
    assert_eq!(missing.directory_state, ProjectDirectoryState::Missing);
    assert_eq!(missing.project, registered);
    assert_eq!(
        store
            .get_project(PrincipalId::local_owner(), id)
            .await
            .expect("stored record"),
        registered
    );

    create_project(&source);
    assert_eq!(
        registry
            .get_project(&access, id)
            .await
            .expect("restored folder")
            .directory_state,
        ProjectDirectoryState::Available
    );
    fs::remove_dir_all(&source).expect("remove fixture directory");
    fs::write(&source, b"not a directory").expect("file at registered path");
    assert_eq!(
        registry
            .get_project(&access, id)
            .await
            .expect("non-directory path")
            .directory_state,
        ProjectDirectoryState::Unavailable
    );
    assert_eq!(
        store
            .get_project(PrincipalId::local_owner(), id)
            .await
            .expect("unchanged record"),
        registered
    );
}

#[tokio::test]
async fn operation_log_records_real_filesystem_checks_and_preserves_the_background_actor() {
    use alcomd_application::{
        AccessContext, Application, IdempotencyKey, LogOutcome, M3Application, OperationLogQuery,
        read_operation_logs, with_log_caller,
    };

    let fixture = TestDirectory::new();
    let project = fixture.path().join("项目带空格 Test");
    create_project(&project);
    let store = alcomd_store::StateStoreHandle::open(fixture.path().join("state.db"))
        .expect("isolated database");
    let registry = M3Application::new(
        store.clone(),
        alcomd_vpm::VpmReader::new().expect("production file reader"),
    );
    let cli = AccessContext::local_owner().with_actor("CLI".to_owned());
    let gui = AccessContext::local_owner().with_actor("GUI".to_owned());
    let path = project.to_string_lossy().into_owned();
    let key = IdempotencyKey::parse("real-operation-log-registration").expect("key");
    let registered = with_log_caller(
        store.clone(),
        &cli,
        registry.register_project(&cli, path.clone(), key.clone()),
    )
    .await
    .expect("register fixture project");
    assert!(
        with_log_caller(
            store.clone(),
            &cli,
            registry.register_project(&cli, path, key)
        )
        .await
        .expect("replay")
        .replayed
    );
    let missing = fixture
        .path()
        .join("不存在的项目")
        .to_string_lossy()
        .into_owned();
    let fail = registry.register_project(
        &gui,
        missing.clone(),
        IdempotencyKey::parse("real-operation-log-failure").expect("key"),
    );
    assert!(with_log_caller(store.clone(), &gui, fail).await.is_err());
    let query = OperationLogQuery {
        limit: 100,
        ..Default::default()
    };
    let first = read_operation_logs(&store, &gui, query.clone())
        .await
        .expect("read facts");
    assert_eq!(
        first
            .entries
            .iter()
            .map(|entry| (entry.actor.as_str(), entry.step.as_str()))
            .collect::<Vec<_>>(),
        [
            ("CLI", "inspect_project"),
            ("CLI", "registered"),
            ("GUI", "inspect_project")
        ]
    );
    assert_eq!(
        first.entries[1]
            .target
            .as_ref()
            .and_then(|target| target.path.as_deref()),
        Some(registered.value.observation.root_path.as_str())
    );
    assert_eq!(first.entries[2].outcome, LogOutcome::Failed);
    assert!(
        first.entries[2]
            .target
            .as_ref()
            .and_then(|target| target.path.as_ref())
            .is_some_and(|path| path.ends_with("不存在的项目"))
    );
    assert!(
        first.entries[2]
            .details
            .as_ref()
            .is_some_and(|details| !details.is_empty())
    );

    let application = Application::new(store.clone());
    let mut changes = application
        .subscribe_events(&gui, None, 100, true)
        .expect("subscribe");
    let operation = with_log_caller(
        store.clone(),
        &cli,
        application.state_check(
            &cli,
            IdempotencyKey::parse("background-log-actor").expect("key"),
        ),
    )
    .await
    .expect("schedule Core task");
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let batch = changes.next_page().await.expect("committed changes");
            let finished = batch.events.iter().any(|event| {
                event.aggregate_id == operation.operation_id.to_string()
                    && event.kind == "operation.completed"
            });
            changes
                .advance(batch.next_sequence, batch.resources_revision)
                .expect("advance");
            if finished {
                break;
            }
        }
    })
    .await
    .expect("background completion");
    let final_records = read_operation_logs(&store, &gui, query)
        .await
        .expect("read task facts");
    let operation_id = operation.operation_id.to_string();
    let task = final_records
        .entries
        .iter()
        .filter(|entry| entry.operation_id.as_ref() == Some(&operation_id))
        .collect::<Vec<_>>();
    assert_eq!(
        task.iter()
            .map(|entry| entry.step.as_str())
            .collect::<Vec<_>>(),
        ["accepted", "execute", "completed"]
    );
    assert!(
        task.iter()
            .all(|entry| entry.actor == "CLI" && entry.execution_id == operation_id)
    );
}

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
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        ipc,
        DataConfig::isolated(data),
        wait_for_shutdown(server_shutdown),
    )
    .await
    .expect("start owned test daemon");
    let mut client = connect_with_retry(config.clone().with_client_name("alcomd-cli")).await;
    let mut log_reader = connect_with_retry(config.with_client_name("alcomd-gui")).await;

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
    assert!(inspected.project.last_modified_at_ms.is_none());
    assert!(inspected.project.favorite.is_none());
    assert!(
        serde_json::to_value(&inspected.project)
            .expect("serialize anonymous project")
            .get("favorite")
            .is_none()
    );
    let registered = client
        .project_register(
            project.to_string_lossy().into_owned(),
            "m3-project-register".to_owned(),
        )
        .await
        .expect("register project");
    assert_eq!(registered.project.revision, Some(1));
    let log_params = alcomd_protocol::LogsReadParams::default();
    let first_logs = log_reader
        .logs_read(log_params.clone())
        .await
        .expect("read Core logs through GUI client");
    assert_eq!(
        first_logs
            .entries
            .iter()
            .map(|entry| entry.step.as_str())
            .collect::<Vec<_>>(),
        ["inspect_project", "registered"]
    );
    assert!(
        first_logs
            .entries
            .iter()
            .all(|entry| entry.actor == "CLI" && entry.action == "projects.register")
    );
    assert_eq!(
        first_logs.entries[0].execution_id,
        first_logs.entries[1].execution_id
    );
    assert_eq!(
        first_logs.entries[1]
            .target
            .as_ref()
            .and_then(|target| target.path.as_deref()),
        Some(registered.project.root_path.as_str())
    );
    client
        .project_register(
            project.to_string_lossy().into_owned(),
            "m3-project-register".to_owned(),
        )
        .await
        .expect("replay registration");
    assert_eq!(
        log_reader
            .logs_read(log_params.clone())
            .await
            .expect("read after replay"),
        first_logs
    );
    let missing = fixture
        .path()
        .join("不存在的项目")
        .to_string_lossy()
        .into_owned();
    assert!(
        log_reader
            .project_register(missing.clone(), "m7-operation-log-failed-check".to_owned())
            .await
            .is_err()
    );
    let failure = log_reader
        .logs_read(alcomd_protocol::LogsReadParams {
            after_sequence: Some(first_logs.high_watermark),
            ..log_params
        })
        .await
        .expect("read failed check");
    assert_eq!(failure.entries.len(), 1);
    assert_eq!(failure.entries[0].actor, "GUI");
    assert_eq!(
        failure.entries[0].outcome,
        alcomd_protocol::LogOutcome::Failed
    );
    assert!(
        failure.entries[0]
            .target
            .as_ref()
            .and_then(|target| target.path.as_ref())
            .is_some_and(|path| path.ends_with("不存在的项目"))
    );
    assert!(failure.entries[0].details.as_ref().is_some_and(
        |details| !details.is_empty() && !details.contains("execution request failed")
    ));
    assert_eq!(registered.project.favorite, Some(false));
    let registered_at_ms = registered
        .project
        .registered_at_ms
        .expect("registered project timestamp");
    let last_modified_at_ms = registered
        .project
        .last_modified_at_ms
        .expect("newly registered project has authoritative modified time");
    assert_eq!(last_modified_at_ms, registered_at_ms);
    let project_id = registered
        .project
        .project_id
        .clone()
        .expect("registered ID");
    let no_op = client
        .project_refresh(project_id.clone(), 1, "m3-project-refresh".to_owned())
        .await
        .expect("refresh project");
    assert_eq!(no_op.project.revision, Some(1));
    assert_eq!(no_op.project.registered_at_ms, Some(registered_at_ms));
    assert_eq!(no_op.project.last_modified_at_ms, Some(last_modified_at_ms));
    assert_eq!(no_op.project, registered.project);
    for snapshot in [&inspected.project, &registered.project, &no_op.project] {
        assert!(
            serde_json::to_value(snapshot)
                .expect("serialize project snapshot")
                .get("observedAtMs")
                .is_none()
        );
    }
    let projects = client
        .projects_list(None, Some(100))
        .await
        .expect("list projects")
        .projects;
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].registered_at_ms, Some(registered_at_ms));
    assert_eq!(projects[0].last_modified_at_ms, Some(last_modified_at_ms));
    assert_eq!(projects[0].favorite, Some(false));
    let favorite = client
        .project_set_favorite(ProjectSetFavoriteParams {
            project_id: project_id.clone(),
            favorite: true,
            expected_revision: 1,
            idempotency_key: "m7-project-favorite".to_owned(),
        })
        .await
        .expect("favorite project over RPC");
    assert_eq!(favorite.project.revision, Some(2));
    assert_eq!(favorite.project.favorite, Some(true));
    assert_eq!(
        favorite.project.last_modified_at_ms,
        Some(last_modified_at_ms)
    );
    let read_back = client
        .project_get(project_id.clone())
        .await
        .expect("get registered project metadata");
    assert_eq!(
        read_back.project.last_modified_at_ms,
        Some(last_modified_at_ms)
    );
    assert_eq!(read_back.project, favorite.project);
    assert_eq!(
        client
            .projects_list(None, Some(100))
            .await
            .expect("list favorite project")
            .projects[0]
            .favorite,
        Some(true)
    );

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
        r#"{"dependencies":{"com.vrchat.avatars":{"version":"3.7.0"}},"locked":{"com.vrchat.avatars":{"version":"3.7.0"}}}"#,
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
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        match AlcomdClient::connect(config.clone()).await {
            Ok(client) => return client,
            Err(ClientError::StartTimeout) if tokio::time::Instant::now() < deadline => {}
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
