use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use alcomd_application::StateStore;
use alcomd_client::{AlcomdClient, ClientConfig};
use alcomd_domain::{OperationId, PrincipalId};
use alcomd_platform::{DataConfig, IpcConfig};
use alcomd_protocol::{OperationState, PackageOperationPhase, RepositorySource};
use alcomd_store::StateStoreHandle;
use sha2::{Digest, Sha256};
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

const CRASH_ROOT_ENV: &str = "ALCOMD_M4_CRASH_ROOT";
const CRASH_SIGNAL_ENV: &str = "ALCOMD_M4_CRASH_SIGNAL";
static RPC_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn install_and_remove_round_trip_through_rpc_without_network() {
    let _serial = RPC_TEST_LOCK.lock().await;
    let fixture = TestDirectory::new();
    let runtime = fixture.path().join("runtime");
    let data = fixture.path().join("data");
    let project = fixture.path().join("Project");
    fs::create_dir(&runtime).expect("create runtime");
    fs::create_dir(&data).expect("create data");
    create_project(&project);

    let archive = fixture.path().join("package.zip");
    create_package_archive(&archive);
    let digest: [u8; 32] = Sha256::digest(fs::read(&archive).expect("read archive")).into();
    publish_cache_object(&data, &archive, &digest);
    let repository = fixture.path().join("repository.json");
    fs::write(&repository, repository_document(&hex(&digest))).expect("write repository");

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

    let registered_project = client
        .project_register(
            project.to_string_lossy().into_owned(),
            "m4-project-register".to_owned(),
        )
        .await
        .expect("register project");
    let project_id = registered_project.project.project_id.expect("project ID");
    let source = RepositorySource::Local {
        path: repository.to_string_lossy().into_owned(),
    };
    let registered_repository = client
        .repository_register(source, "m4-repository-register".to_owned())
        .await
        .expect("register repository");
    let repository_id = registered_repository
        .repository
        .repository_id
        .expect("repository ID");
    client
        .repository_refresh(
            repository_id,
            registered_repository.repository.revision.expect("revision"),
            "m4-repository-refresh".to_owned(),
        )
        .await
        .expect("make repository resolver-ready");

    let install = client
        .package_plan_install(alcomd_protocol::PackagePlanInstallParams {
            project_id: project_id.clone(),
            expected_revision: 1,
            package_id: "com.example.fixture".to_owned(),
            version_range: Some("1.0.0".to_owned()),
            repository_id: None,
            include_prerelease: false,
        })
        .await
        .expect("plan install");
    assert_eq!(install.change_set.mutations.len(), 1);
    let accepted = client
        .package_apply_plan(alcomd_protocol::PackageApplyPlanParams {
            plan_id: install.plan_id,
            expected_revision: 1,
            idempotency_key: "m4-apply-install".to_owned(),
        })
        .await
        .expect("apply install");
    let completed = wait_for_terminal(&mut client, &accepted.operation_id, &project).await;
    assert_eq!(completed.state, OperationState::Succeeded);
    assert_eq!(
        completed.progress.expect("package progress").phase,
        PackageOperationPhase::StateCommitted
    );
    assert!(
        project
            .join("Packages/com.example.fixture/package.json")
            .is_file()
    );
    assert_manifest_version(&project, Some("1.0.0"));
    assert_eq!(
        fs::read(project.join("Packages/manifest.json")).expect("UPM"),
        b"{\"dependencies\":{}}\n"
    );

    let remove = client
        .package_plan_remove(alcomd_protocol::PackagePlanRemoveParams {
            project_id,
            expected_revision: 2,
            package_id: "com.example.fixture".to_owned(),
        })
        .await
        .expect("plan remove");
    let accepted = client
        .package_apply_plan(alcomd_protocol::PackageApplyPlanParams {
            plan_id: remove.plan_id,
            expected_revision: 2,
            idempotency_key: "m4-apply-remove".to_owned(),
        })
        .await
        .expect("apply remove");
    let completed = wait_for_terminal(&mut client, &accepted.operation_id, &project).await;
    assert_eq!(completed.state, OperationState::Succeeded);
    assert_eq!(
        completed.progress.expect("package progress").phase,
        PackageOperationPhase::StateCommitted
    );
    assert!(!project.join("Packages/com.example.fixture").exists());
    assert_manifest_version(&project, None);

    shutdown.store(true, Ordering::Release);
    let result = tokio::time::timeout(Duration::from_secs(3), daemon)
        .await
        .expect("daemon stop timeout")
        .expect("join daemon");
    assert!(result.is_ok());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn killed_package_apply_resumes_from_durable_archive_phase() {
    let _serial = RPC_TEST_LOCK.lock().await;
    let fixture = TestDirectory::new();
    let signal = fixture.path().join("operation.signal");
    let child = Command::new(std::env::current_exe().expect("current test executable"))
        .args([
            "--exact",
            "subprocess_runs_package_apply_until_killed",
            "--ignored",
            "--nocapture",
        ])
        .env(CRASH_ROOT_ENV, fixture.path())
        .env(CRASH_SIGNAL_ENV, &signal)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn package transaction subprocess");
    let mut child = ChildGuard(Some(child));

    let operation_id = wait_for_operation_signal(&signal);
    wait_for_archive_ready_marker(fixture.path(), &operation_id);
    child
        .0
        .as_mut()
        .expect("child process")
        .kill()
        .expect("force-kill package transaction subprocess");
    let _ = child
        .0
        .as_mut()
        .expect("child process")
        .wait()
        .expect("wait for killed subprocess");
    child.0 = None;

    let runtime = fixture.path().join("runtime");
    let data = fixture.path().join("data");
    let project = fixture.path().join("Project");
    let killed_store = open_store_with_retry(&data.join("state.db"));
    let killed_operation = killed_store
        .get_operation(
            PrincipalId::local_owner(),
            OperationId::parse(&operation_id).expect("Operation ID"),
        )
        .await
        .expect("load killed package Operation");
    assert_eq!(
        killed_operation.state,
        alcomd_domain::OperationState::Running
    );
    assert_eq!(
        killed_operation.progress_phase,
        Some(alcomd_application::FilesystemPhase::ArchiveReady)
    );
    drop(killed_store);
    let transaction_root = project
        .join("Library/ALCOMD/transactions")
        .join(&operation_id);
    alcomd_platform::sync_directory(&transaction_root).expect("sync recovered transaction root");
    let digest: [u8; 32] = Sha256::digest(
        fs::read(fixture.path().join("package.zip")).expect("read cached crash archive"),
    )
    .into();
    alcomd_vpm::PackageCache::new(data.join("package-cache"))
        .expect("open package cache")
        .get(
            digest,
            "https://network-must-not-be-used.invalid/package.zip",
            true,
        )
        .await
        .expect("revalidate offline cache after kill");
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
    let completed = wait_for_terminal(&mut client, &operation_id, &project).await;
    assert_eq!(completed.state, OperationState::Succeeded);
    assert_eq!(
        completed.progress.expect("recovery progress").phase,
        PackageOperationPhase::StateCommitted
    );
    assert_manifest_version(&project, Some("1.0.0"));
    assert!(
        project
            .join("Packages/com.example.fixture/package.json")
            .is_file()
    );

    shutdown.store(true, Ordering::Release);
    let result = tokio::time::timeout(Duration::from_secs(3), daemon)
        .await
        .expect("daemon stop timeout")
        .expect("join daemon");
    assert!(result.is_ok());
}

#[test]
#[ignore = "subprocess fixture invoked by the package kill/restart test"]
fn subprocess_runs_package_apply_until_killed() {
    let root = PathBuf::from(std::env::var_os(CRASH_ROOT_ENV).expect("crash root"));
    let signal = PathBuf::from(std::env::var_os(CRASH_SIGNAL_ENV).expect("crash signal"));
    let runtime = tokio::runtime::Runtime::new().expect("test runtime");
    runtime.block_on(async move {
        let runtime_root = root.join("runtime");
        let data = root.join("data");
        let project = root.join("Project");
        fs::create_dir(&runtime_root).expect("create runtime");
        fs::create_dir(&data).expect("create data");
        create_project(&project);
        let archive = root.join("package.zip");
        create_large_package_archive(&archive);
        let digest: [u8; 32] = Sha256::digest(fs::read(&archive).expect("read archive")).into();
        publish_cache_object(&data, &archive, &digest);
        let repository = root.join("repository.json");
        fs::write(&repository, repository_document(&hex(&digest))).expect("write repository");

        let (ipc, config) = isolated_ipc(runtime_root);
        let shutdown = Arc::new(AtomicBool::new(false));
        let daemon_shutdown = Arc::clone(&shutdown);
        let _daemon = tokio::spawn(async move {
            alcomd_daemon::serve_with_data_until(
                ipc,
                DataConfig::isolated(data),
                wait_for_shutdown(daemon_shutdown),
            )
            .await
        });
        let mut client = connect_with_retry(config).await;
        let project_result = client
            .project_register(
                project.to_string_lossy().into_owned(),
                "m4-crash-project".to_owned(),
            )
            .await
            .expect("register crash project");
        let project_id = project_result.project.project_id.expect("project ID");
        let repository_result = client
            .repository_register(
                RepositorySource::Local {
                    path: repository.to_string_lossy().into_owned(),
                },
                "m4-crash-repository".to_owned(),
            )
            .await
            .expect("register crash repository");
        let repository_id = repository_result
            .repository
            .repository_id
            .expect("repository ID");
        client
            .repository_refresh(
                repository_id,
                repository_result.repository.revision.expect("revision"),
                "m4-crash-refresh".to_owned(),
            )
            .await
            .expect("refresh crash repository");
        let plan = client
            .package_plan_install(alcomd_protocol::PackagePlanInstallParams {
                project_id,
                expected_revision: 1,
                package_id: "com.example.fixture".to_owned(),
                version_range: Some("1.0.0".to_owned()),
                repository_id: None,
                include_prerelease: false,
            })
            .await
            .expect("plan crash install");
        let accepted = client
            .package_apply_plan(alcomd_protocol::PackageApplyPlanParams {
                plan_id: plan.plan_id,
                expected_revision: 1,
                idempotency_key: "m4-crash-apply".to_owned(),
            })
            .await
            .expect("accept crash install");
        fs::write(signal, &accepted.operation_id).expect("write operation signal");
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });
}

fn create_project(root: &Path) {
    fs::create_dir_all(root.join("ProjectSettings")).expect("ProjectSettings");
    fs::create_dir_all(root.join("Packages")).expect("Packages");
    fs::write(
        root.join("ProjectSettings/ProjectVersion.txt"),
        "m_EditorVersion: 2022.3.22f1\n",
    )
    .expect("ProjectVersion");
    fs::write(
        root.join("Packages/vpm-manifest.json"),
        b"{\"dependencies\":{},\"locked\":{},\"preserved\":true}\n",
    )
    .expect("VPM manifest");
    fs::write(
        root.join("Packages/manifest.json"),
        b"{\"dependencies\":{}}\n",
    )
    .expect("UPM manifest");
}

fn create_package_archive(path: &Path) {
    let file = fs::File::create(path).expect("archive");
    let mut writer = zip::ZipWriter::new(file);
    writer
        .start_file(
            "package.json",
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
        )
        .expect("package entry");
    writer
        .write_all(b"{\"name\":\"com.example.fixture\",\"version\":\"1.0.0\"}")
        .expect("package manifest");
    writer
        .start_file(
            "Runtime/fixture.txt",
            SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
        )
        .expect("runtime entry");
    writer.write_all(b"fixture").expect("runtime content");
    writer.finish().expect("finish archive");
}

fn create_large_package_archive(path: &Path) {
    let file = fs::File::create(path).expect("large archive");
    let mut writer = zip::ZipWriter::new(file);
    writer
        .start_file(
            "package.json",
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
        )
        .expect("package entry");
    writer
        .write_all(b"{\"name\":\"com.example.fixture\",\"version\":\"1.0.0\"}")
        .expect("package manifest");
    writer
        .start_file(
            "Runtime/payload.bin",
            SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
        )
        .expect("payload entry");
    let block = [0x5a_u8; 64 * 1024];
    for _ in 0..128 {
        writer.write_all(&block).expect("payload content");
    }
    writer.finish().expect("finish large archive");
}

fn wait_for_operation_signal(signal: &Path) -> String {
    for _ in 0..10_000 {
        if let Ok(value) = fs::read_to_string(signal) {
            return value;
        }
        thread::sleep(Duration::from_millis(5));
    }
    panic!("package subprocess did not publish its Operation ID");
}

struct ChildGuard(Option<std::process::Child>);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn open_store_with_retry(database: &Path) -> StateStoreHandle {
    for _ in 0..1_000 {
        if let Ok(store) = StateStoreHandle::open(database.to_path_buf()) {
            return store;
        }
        thread::sleep(Duration::from_millis(1));
    }
    panic!("killed package subprocess did not release state.db");
}

fn wait_for_archive_ready_marker(root: &Path, operation_id: &str) {
    let transaction = root
        .join("Project/Library/ALCOMD/transactions")
        .join(operation_id);
    for _ in 0..10_000 {
        if marker_contains_phase(&transaction, "archive_ready") {
            return;
        }
        thread::sleep(Duration::from_millis(1));
    }
    panic!("package subprocess did not durably reach archive_ready");
}

fn marker_contains_phase(transaction: &Path, phase: &str) -> bool {
    let Ok(attempts) = fs::read_dir(transaction) else {
        return false;
    };
    attempts.filter_map(Result::ok).any(|attempt| {
        fs::read_dir(attempt.path()).is_ok_and(|entries| {
            entries.filter_map(Result::ok).any(|entry| {
                entry.file_name().to_string_lossy().starts_with("marker-")
                    && fs::read_to_string(entry.path())
                        .is_ok_and(|value| value.contains(&format!("\"phase\":\"{phase}\"")))
            })
        })
    })
}

fn publish_cache_object(data: &Path, archive: &Path, digest: &[u8; 32]) {
    let text = hex(digest);
    let parent = data.join("package-cache/sha256").join(&text[..2]);
    fs::create_dir_all(&parent).expect("cache parent");
    fs::copy(archive, parent.join(format!("{text}.zip"))).expect("cache object");
}

fn repository_document(digest: &str) -> String {
    format!(
        "{{\"id\":\"fixture\",\"name\":\"Fixture\",\"packages\":{{\"com.example.fixture\":{{\"versions\":{{\"1.0.0\":{{\"name\":\"com.example.fixture\",\"displayName\":\"Fixture\",\"version\":\"1.0.0\",\"url\":\"https://network-must-not-be-used.invalid/package.zip\",\"zipSHA256\":\"{digest}\",\"author\":{{\"name\":\"Fixture\",\"email\":\"fixture@example.invalid\"}}}}}}}}}}}}"
    )
}

fn assert_manifest_version(project: &Path, expected: Option<&str>) {
    let value: serde_json::Value = serde_json::from_slice(
        &fs::read(project.join("Packages/vpm-manifest.json")).expect("read VPM manifest"),
    )
    .expect("parse VPM manifest");
    assert_eq!(
        value["locked"]["com.example.fixture"]["version"].as_str(),
        expected
    );
    assert_eq!(value["preserved"], true);
}

async fn wait_for_terminal(
    client: &mut AlcomdClient,
    operation_id: &str,
    project: &Path,
) -> alcomd_protocol::Operation {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        let operation = client
            .operation_get(operation_id.to_owned())
            .await
            .expect("get operation");
        if matches!(
            operation.state,
            OperationState::Succeeded | OperationState::Failed | OperationState::Cancelled
        ) {
            return operation;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "operation timed out in state {:?} with progress {:?}",
            operation.state,
            (
                operation.progress,
                transaction_entries(project, operation_id)
            )
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn transaction_entries(project: &Path, operation_id: &str) -> Vec<String> {
    let root = project
        .join("Library/ALCOMD/transactions")
        .join(operation_id);
    let Ok(attempts) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut result = Vec::new();
    for attempt in attempts.filter_map(Result::ok) {
        let attempt_name = attempt.file_name().to_string_lossy().into_owned();
        result.push(format!(
            "{attempt_name}/sync={:?}",
            alcomd_platform::sync_directory(&attempt.path()).map_err(|error| error.raw_os_error())
        ));
        let Ok(entries) = fs::read_dir(attempt.path()) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            result.push(format!(
                "{attempt_name}/{}",
                entry.file_name().to_string_lossy()
            ));
        }
    }
    result.sort();
    result
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

fn hex(value: &[u8; 32]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        #[cfg(target_os = "macos")]
        let base = PathBuf::from("/private/tmp");
        #[cfg(not(target_os = "macos"))]
        let base = std::env::temp_dir();
        let path = base.join(format!("alcomd-m4-rpc-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).expect("fixture root");
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
