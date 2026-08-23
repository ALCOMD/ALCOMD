use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use alcomd_client::{AlcomdClient, ClientConfig};
use alcomd_platform::{DataConfig, IpcConfig};
use alcomd_protocol::{BackupCompression, BackupCreateParams, BackupsListParams, OperationState};

const CRASH_ROOT_ENV: &str = "ALCOMD_M5_BACKUP_CRASH_ROOT";
const KILL_GATE_ENV: &str = "ALCOMD_TEST_BACKUP_KILL_GATE";
const KILL_SIGNAL_ENV: &str = "ALCOMD_TEST_BACKUP_KILL_SIGNAL";
const OPERATION_SIGNAL_ENV: &str = "ALCOMD_TEST_BACKUP_OPERATION_SIGNAL";
static RPC_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn backup_create_round_trips_with_exact_exclusions_and_idempotency() {
    let _serial = RPC_TEST_LOCK.lock().await;
    let fixture = TestDirectory::new();
    let project = fixture.path().join("Project");
    create_project(&project);
    let runtime = fixture.path().join("runtime");
    let data = fixture.path().join("data");
    fs::create_dir(&runtime).expect("runtime");
    fs::create_dir(&data).expect("data");
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
    let status = client.system_status().await.expect("status");
    assert!(
        status
            .capabilities
            .contains(&alcomd_protocol::CAPABILITY_BACKUPS_READ_V1.to_owned())
    );
    assert!(
        status
            .capabilities
            .contains(&alcomd_protocol::CAPABILITY_BACKUPS_CREATE_V1.to_owned())
    );
    let registered = client
        .project_register(
            project.to_string_lossy().into_owned(),
            "backup-register".to_owned(),
        )
        .await
        .expect("register");
    let project_id = registered.project.project_id.expect("project id");
    let request = BackupCreateParams {
        project_id: project_id.clone(),
        expected_revision: 1,
        compression_mode: BackupCompression::Fast,
        exclude_vpm_packages: true,
        idempotency_key: "backup-create".to_owned(),
    };
    let accepted = client.backup_create(request.clone()).await.expect("create");
    let completed = wait_for_operation(&mut client, &accepted.operation_id).await;
    assert_eq!(completed.state, OperationState::Succeeded, "{completed:?}");
    assert_eq!(
        completed.progress.expect("progress").phase,
        alcomd_protocol::PackageOperationPhase::StateCommitted
    );
    let replay = client.backup_create(request).await.expect("replay");
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, accepted.operation_id);
    assert_eq!(replay.backup_id, accepted.backup_id);
    let page = client
        .backups_list(BackupsListParams {
            project_id: Some(project_id),
            cursor: None,
            limit: Some(10),
        })
        .await
        .expect("list");
    assert_eq!(page.backups.len(), 1);
    let record = client
        .backup_get(accepted.backup_id.clone())
        .await
        .expect("get");
    assert_eq!(record.backup_id, accepted.backup_id);
    let archive = fixture
        .path()
        .join("data/backups/objects")
        .join(format!("{}.zip", record.backup_id));
    assert_archive_profile(&archive);
    assert_eq!(
        fs::read_dir(fixture.path().join("data/backups/partial"))
            .expect("partials")
            .count(),
        0
    );
    shutdown.store(true, Ordering::Release);
    daemon.await.expect("join").expect("daemon");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn backup_create_real_kill_restart_matrix_reuses_operation_backup_and_idempotency() {
    let _serial = RPC_TEST_LOCK.lock().await;
    for checkpoint in [
        "inventory_ready",
        "archiving",
        "archive_ready",
        "publish_intent",
        "archive_published",
        "state_committed",
    ] {
        run_kill_restart_case(checkpoint).await;
    }
}

#[test]
#[ignore = "subprocess fixture invoked by Backup kill/restart matrix"]
fn subprocess_runs_backup_until_parent_kills_it() {
    let root = PathBuf::from(std::env::var_os(CRASH_ROOT_ENV).expect("crash root"));
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    runtime.block_on(async move {
        let project = root.join("Project");
        create_project(&project);
        let runtime_root = root.join("runtime");
        let data = root.join("data");
        fs::create_dir_all(&runtime_root).expect("runtime root");
        fs::create_dir_all(&data).expect("data");
        let (ipc, config) = isolated_ipc(runtime_root);
        let shutdown = Arc::new(AtomicBool::new(false));
        let server_shutdown = Arc::clone(&shutdown);
        let _daemon = tokio::spawn(async move {
            alcomd_daemon::serve_with_data_until(
                ipc,
                DataConfig::isolated(data),
                wait_for_shutdown(server_shutdown),
            )
            .await
        });
        let mut client = connect_with_retry(config).await;
        let registered = client
            .project_register(
                project.to_string_lossy().into_owned(),
                "backup-kill-register".to_owned(),
            )
            .await
            .expect("register");
        let project_id = registered.project.project_id.expect("project id");
        let _ = client
            .backup_create(BackupCreateParams {
                project_id,
                expected_revision: 1,
                compression_mode: BackupCompression::Fast,
                exclude_vpm_packages: true,
                idempotency_key: "backup-kill-create".to_owned(),
            })
            .await;
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });
}

async fn run_kill_restart_case(checkpoint: &str) {
    let fixture = TestDirectory::new();
    let kill_signal = fixture.path().join("kill.txt");
    let operation_signal = fixture.path().join("operation.json");
    let child = Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "subprocess_runs_backup_until_parent_kills_it",
            "--ignored",
            "--nocapture",
        ])
        .env(CRASH_ROOT_ENV, fixture.path())
        .env(KILL_GATE_ENV, checkpoint)
        .env(KILL_SIGNAL_ENV, &kill_signal)
        .env(OPERATION_SIGNAL_ENV, &operation_signal)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn");
    let mut child = ChildGuard(Some(child));
    wait_for_file(&operation_signal, None);
    wait_for_file(&kill_signal, Some(checkpoint.as_bytes()));
    let accepted: alcomd_protocol::BackupCreateResult =
        serde_json::from_slice(&fs::read(&operation_signal).expect("signal"))
            .expect("parse signal");
    child.0.as_mut().expect("child").kill().expect("force kill");
    child.0.as_mut().expect("child").wait().expect("wait");
    child.0 = None;

    let runtime = fixture.path().join("runtime");
    let data = fixture.path().join("data");
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
    let projects = client
        .projects_list(None, Some(10))
        .await
        .expect("projects");
    let project = projects.projects.first().expect("project");
    let project_id = project.project_id.clone().expect("project id");
    let replay = client
        .backup_create(BackupCreateParams {
            project_id: project_id.clone(),
            expected_revision: 1,
            compression_mode: BackupCompression::Fast,
            exclude_vpm_packages: true,
            idempotency_key: "backup-kill-create".to_owned(),
        })
        .await
        .expect("replay");
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, accepted.operation_id);
    assert_eq!(replay.backup_id, accepted.backup_id);
    let completed = wait_for_operation(&mut client, &accepted.operation_id).await;
    assert_eq!(
        completed.state,
        OperationState::Succeeded,
        "{checkpoint}: {completed:?}"
    );
    let backups = client
        .backups_list(BackupsListParams {
            project_id: Some(project_id),
            cursor: None,
            limit: Some(10),
        })
        .await
        .expect("backups");
    assert_eq!(backups.backups.len(), 1, "{checkpoint}");
    assert_eq!(backups.backups[0].backup_id, accepted.backup_id);
    let objects = fixture.path().join("data/backups/objects");
    assert_eq!(
        fs::read_dir(&objects).expect("objects").count(),
        1,
        "{checkpoint}"
    );
    assert_archive_profile(&objects.join(format!("{}.zip", accepted.backup_id)));
    assert_eq!(
        fs::read_dir(fixture.path().join("data/backups/partial"))
            .expect("partials")
            .count(),
        0,
        "{checkpoint}"
    );
    shutdown.store(true, Ordering::Release);
    daemon.await.expect("join").expect("daemon");
}

fn create_project(root: &Path) {
    for directory in [
        "Assets/Empty",
        "ProjectSettings",
        "Packages/com.example.locked",
        "Packages/com.example.embedded",
        "LibraryCache/Sub",
        "Logs",
        "UserSettings",
        ".idea",
        "Assets/.git",
    ] {
        fs::create_dir_all(root.join(directory)).expect("directory");
    }
    fs::write(root.join("Assets/source.bin"), vec![0x5a; 256 * 1024]).expect("source");
    fs::write(
        root.join("ProjectSettings/ProjectVersion.txt"),
        "m_EditorVersion: 2022.3.22f1\n",
    )
    .expect("version");
    fs::write(
        root.join("Packages/manifest.json"),
        b"{\"dependencies\":{}}",
    )
    .expect("manifest");
    fs::write(root.join("Packages/vpm-manifest.json"), b"{\"dependencies\":{\"com.example.locked\":\"1.0.0\"},\"locked\":{\"com.example.locked\":{\"version\":\"1.0.0\"}}}").expect("vpm");
    fs::write(root.join("Packages/com.example.locked/package.json"), b"{}").expect("locked");
    fs::write(
        root.join("Packages/com.example.embedded/package.json"),
        b"{}",
    )
    .expect("embedded");
    fs::write(
        root.join("LibraryCache/LastSceneManagerSetup.txt"),
        b"scene",
    )
    .expect("scene");
    fs::write(
        root.join("LibraryCache/Sub/LastSceneManagerSetup.txt"),
        b"nested",
    )
    .expect("nested");
    fs::write(root.join("Logs/log.txt"), b"log").expect("log");
    fs::write(root.join("Assets/.git/config"), b"git").expect("git");
}

fn assert_archive_profile(path: &Path) {
    let file = fs::File::open(path).expect("archive");
    let mut archive = zip::ZipArchive::new(file).expect("zip");
    let names = (0..archive.len())
        .map(|index| archive.by_index(index).expect("entry").name().to_owned())
        .collect::<Vec<_>>();
    assert!(names.contains(&"backup.json".to_owned()));
    assert!(names.contains(&"project/ProjectSettings/ProjectVersion.txt".to_owned()));
    assert!(names.contains(&"project/Packages/manifest.json".to_owned()));
    assert!(names.contains(&"project/Packages/vpm-manifest.json".to_owned()));
    assert!(names.contains(&"project/Packages/com.example.embedded/package.json".to_owned()));
    assert!(names.contains(&"project/LibraryCache/LastSceneManagerSetup.txt".to_owned()));
    assert!(
        !names
            .iter()
            .any(|name| name.starts_with("project/Packages/com.example.locked/"))
    );
    assert!(!names.iter().any(|name| name.starts_with("project/Logs/")));
    assert!(!names.iter().any(|name| name.contains("/.git/")));
    assert!(!names.contains(&"project/LibraryCache/Sub/LastSceneManagerSetup.txt".to_owned()));
}

async fn wait_for_operation(
    client: &mut AlcomdClient,
    operation_id: &str,
) -> alcomd_protocol::Operation {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        let operation = client
            .operation_get(operation_id.to_owned())
            .await
            .expect("operation");
        if matches!(
            operation.state,
            OperationState::Succeeded | OperationState::Failed | OperationState::Cancelled
        ) {
            return operation;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "operation timeout: {operation:?}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn wait_for_file(path: &Path, expected: Option<&[u8]>) {
    for _ in 0..1_000 {
        if fs::read(path).is_ok_and(|bytes| expected.is_none_or(|expected| bytes == expected)) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for {}", path.display());
}

async fn connect_with_retry(config: ClientConfig) -> AlcomdClient {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        match AlcomdClient::connect(config.clone()).await {
            Ok(client) => return client,
            Err(_) if tokio::time::Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(25)).await
            }
            Err(error) => panic!("daemon unavailable: {error}"),
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
fn isolated_ipc(_: PathBuf) -> (IpcConfig, ClientConfig) {
    (
        IpcConfig::default(),
        ClientConfig::default().without_daemon_start(),
    )
}

struct ChildGuard(Option<std::process::Child>);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

struct TestDirectory(PathBuf);
impl TestDirectory {
    fn new() -> Self {
        #[cfg(target_os = "macos")]
        let base = PathBuf::from("/private/tmp");
        #[cfg(not(target_os = "macos"))]
        let base = std::env::temp_dir();
        let path = base.join(format!("alcomd-m5-backup-rpc-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).expect("fixture");
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
