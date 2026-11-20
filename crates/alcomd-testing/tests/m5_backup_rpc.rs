use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use alcomd_client::{AlcomdClient, ClientConfig, ClientError};
use alcomd_platform::{DataConfig, IpcConfig};
use alcomd_protocol::{
    BackupCompression, BackupCreateParams, BackupRestoreParams, McpBackupRestoreParams,
    McpClientMutationParams, McpClientsCreateParams, OperationAccepted, OperationState,
};

const CRASH_ROOT_ENV: &str = "ALCOMD_M5_BACKUP_CRASH_ROOT";
const KILL_GATE_ENV: &str = "ALCOMD_TEST_BACKUP_KILL_GATE";
const KILL_SIGNAL_ENV: &str = "ALCOMD_TEST_BACKUP_KILL_SIGNAL";
const OPERATION_SIGNAL_ENV: &str = "ALCOMD_TEST_BACKUP_OPERATION_SIGNAL";
const RESTORE_CRASH_ROOT_ENV: &str = "ALCOMD_M5_BACKUP_RESTORE_CRASH_ROOT";
const RESTORE_KILL_GATE_ENV: &str = "ALCOMD_TEST_BACKUP_RESTORE_KILL_GATE";
const RESTORE_KILL_SIGNAL_ENV: &str = "ALCOMD_TEST_BACKUP_RESTORE_KILL_SIGNAL";
const RESTORE_OPERATION_SIGNAL_ENV: &str = "ALCOMD_TEST_BACKUP_RESTORE_OPERATION_SIGNAL";
const RESTORE_PAUSE_GATE_ENV: &str = "ALCOMD_TEST_BACKUP_RESTORE_PAUSE_GATE";
const RESTORE_PAUSE_SIGNAL_ENV: &str = "ALCOMD_TEST_BACKUP_RESTORE_PAUSE_SIGNAL";
const RESTORE_PAUSE_RELEASE_ENV: &str = "ALCOMD_TEST_BACKUP_RESTORE_PAUSE_RELEASE";
static RPC_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mcp_restores_a_zip_outside_the_configured_directory_and_rechecks_access() {
    let _serial = RPC_TEST_LOCK.lock().await;
    let fixture = TestDirectory::new();
    let project = fixture.path().join("Owned project");
    create_project(&project);
    let runtime = fixture.path().join("runtime");
    let data = fixture.path().join("data");
    let archive_dir = fixture.path().join("archives");
    let projects = fixture.path().join("projects");
    let external = fixture.path().join("外部 ZIP 档案");
    for path in [&runtime, &data, &archive_dir, &projects, &external] {
        fs::create_dir(path).expect("isolated directory");
    }
    create_project(&projects.join("Existing project"));
    let (ipc, config) = isolated_ipc(runtime);
    let shutdown = Arc::new(AtomicBool::new(false));
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        ipc,
        DataConfig::isolated(data),
        wait_for_shutdown(Arc::clone(&shutdown)),
    )
    .await
    .expect("start owned test daemon");
    let mut owner = connect_with_retry(config.clone()).await;
    let settings = owner.settings_get().await.expect("isolated settings");
    owner
        .settings_update(
            serde_json::from_value(serde_json::json!({
                "expectedRevision": settings.revision,
                "update": {
                    "paths": { "backupDirectory": archive_dir.to_string_lossy(), "projectDirectory": projects.to_string_lossy() },
                    "projects": { "autoDiscover": false }
                }
            }))
            .expect("directory update"),
        )
        .await
        .expect("override backup destination within fixture");
    let project = owner
        .project_register(
            project.to_string_lossy().into_owned(),
            "backup-mcp-register".to_owned(),
        )
        .await
        .expect("register")
        .project;
    let archive = external.join("Unity project.zip");
    let request = BackupCreateParams {
        project_id: project.project_id.expect("registered project id"),
        expected_revision: project.revision.expect("registered project revision"),
        compression_mode: None,
        exclude_vpm_packages: Some(true),
        include_library: None,
        destination_path: Some(path_string(&archive)),
        idempotency_key: "shared-key".to_owned(),
    };
    let mut actors = Vec::new();
    for name in ["First MCP", "Second MCP"] {
        let record = owner
            .mcp_client_create(McpClientsCreateParams {
                idempotency_key: format!("create-{name}"),
                display_name: name.to_owned(),
            })
            .await
            .expect("independent client");
        let issued = owner
            .mcp_client_issue_credential(McpClientMutationParams {
                client_id: record.client_id,
                expected_revision: record.revision,
                idempotency_key: format!("issue-{name}"),
            })
            .await
            .expect("credential");
        let client = AlcomdClient::connect(config.clone().with_mcp_credential(issued.credential))
            .await
            .expect("authenticate");
        actors.push((issued.client_id, issued.revision, client));
    }
    let created = owner
        .backup_create(request.clone())
        .await
        .expect("save external ZIP");
    let completed = wait_for_operation(&mut owner, &created.operation_id).await;
    assert_eq!(completed.state, OperationState::Succeeded, "{completed:?}");
    assert_archive_profile(&archive, false);
    assert_eq!(
        fs::read_dir(&archive_dir)
            .expect("configured backup directory")
            .count(),
        0
    );
    let same = owner.backup_create(request).await.expect("same create");
    assert!(same.replayed);
    assert_eq!(same.operation_id, created.operation_id);

    let inferred = BackupRestoreParams {
        archive_path: path_string(&archive),
        project_name: None,
        destination_path: None,
        idempotency_key: "derive-name-from-archive".to_owned(),
    };
    let saved = owner
        .backup_restore(inferred.clone())
        .await
        .expect("restore to configured project root");
    let saved = wait_for_operation(&mut owner, &saved.operation_id).await;
    assert_eq!(saved.state, OperationState::Succeeded, "{saved:?}");
    assert!(projects.join("Unity project/Assets/source.bin").is_file());
    let mut new_request = inferred;
    new_request.idempotency_key = "same-target-different-key".to_owned();
    assert!(matches!(
        owner.backup_restore(new_request).await,
        Err(ClientError::Remote(e)) if e.code == "backup_target_exists"
    ));

    let ((client_id, revision, first), (_, _, second)) = match actors.as_mut_slice() {
        [first, second] => (first, second),
        _ => panic!("two clients required"),
    };
    let target = external.join("Restored project");
    let input = McpBackupRestoreParams {
        archive_path: path_string(&archive),
        project_name: None,
        destination_path: Some(path_string(&target)),
        idempotency_key: "restore-external".to_owned(),
        confirm: true,
    };
    let mut unconfirmed = input.clone();
    unconfirmed.confirm = false;
    assert!(first.mcp_backup_restore(unconfirmed).await.is_err());
    assert!(!target.exists());
    let accepted = first
        .mcp_backup_restore(input.clone())
        .await
        .expect("absolute file restore");
    let completed = wait_for_operation(first, &accepted.operation_id).await;
    assert_eq!(completed.state, OperationState::Succeeded, "{completed:?}");
    let receipt = completed.result.expect("task receipt");
    assert_eq!(receipt["targetName"], "Restored project");
    assert!(!receipt.to_string().contains(&path_string(&external)));
    let project_id = receipt["projectId"].as_str().expect("restored project id");
    assert!(owner.project_get(project_id.to_owned()).await.is_ok());
    assert_eq!(
        fs::read(target.join("Assets/source.bin")).expect("assets"),
        vec![0x5a; 256 * 1024]
    );

    let same = first
        .mcp_backup_restore(input.clone())
        .await
        .expect("restore replay");
    assert!(same.replayed);
    assert_eq!(accepted.operation_id, same.operation_id);
    assert!(matches!(
        second.mcp_backup_restore(input.clone()).await,
        Err(ClientError::Remote(e)) if e.code == "backup_target_exists"
    ));
    owner
        .mcp_client_revoke(McpClientMutationParams {
            client_id: client_id.clone(),
            expected_revision: *revision,
            idempotency_key: "revoke-backup-manage".to_owned(),
        })
        .await
        .expect("revoke existing connection");
    assert!(matches!(
        first.mcp_backup_restore(input).await,
        Err(ClientError::Remote(e)) if e.code == "permission_denied"
    ));
    shutdown.store(true, Ordering::Release);
    daemon.await.expect("join").expect("daemon");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn restore_rejects_an_unrelated_zip_with_a_nested_project() {
    use std::io::Write;

    let _serial = RPC_TEST_LOCK.lock().await;
    let fixture = TestDirectory::new();
    let archive = fixture.path().join("Unrelated archive.zip");
    let mut writer = zip::ZipWriter::new(fs::File::create(&archive).expect("archive"));
    for (relative, bytes) in [
        ("nested/Assets/content.txt", "not a project root"),
        ("nested/Packages/manifest.json", "{}"),
        (
            "nested/ProjectSettings/ProjectVersion.txt",
            "m_EditorVersion: 2022.3.22f1\n",
        ),
    ] {
        writer
            .start_file(relative, zip::write::SimpleFileOptions::default())
            .expect("entry");
        writer.write_all(bytes.as_bytes()).expect("write");
    }
    writer.finish().expect("finish");
    let source_before = fs::read(&archive).expect("source bytes");
    let runtime = fixture.path().join("runtime");
    let data = fixture.path().join("data");
    fs::create_dir(&runtime).expect("runtime");
    fs::create_dir(&data).expect("data");
    let (ipc, config) = isolated_ipc(runtime);
    let shutdown = Arc::new(AtomicBool::new(false));
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        ipc,
        DataConfig::isolated(data),
        wait_for_shutdown(Arc::clone(&shutdown)),
    )
    .await
    .expect("daemon");
    let mut client = connect_with_retry(config).await;
    let target = fixture.path().join("Rejected project");
    let result = client
        .backup_restore(BackupRestoreParams {
            archive_path: path_string(&archive),
            project_name: None,
            destination_path: Some(path_string(&target)),
            idempotency_key: "restore-unrelated-zip".into(),
        })
        .await;
    match result {
        Ok(accepted) => {
            let failure = wait_for_operation(&mut client, &accepted.operation_id).await;
            assert_eq!(
                failure.error_code.as_deref(),
                Some("backup_invalid_archive")
            );
        }
        Err(ClientError::Remote(error)) => assert_eq!(error.code, "backup_invalid_archive"),
        Err(other) => panic!("unexpected transport failure: {other}"),
    }
    assert!(!target.exists());
    assert_eq!(source_before, fs::read(&archive).expect("preserved source"));
    shutdown.store(true, Ordering::Release);
    daemon.await.expect("join").expect("daemon");
}

fn path_string(path: &Path) -> String {
    path.to_str().expect("Unicode fixture path").to_owned()
}

fn restore_kill_request(root: &Path) -> BackupRestoreParams {
    BackupRestoreParams {
        archive_path: path_string(&root.join("Source.zip")),
        project_name: None,
        destination_path: Some(path_string(&root.join("Restored"))),
        idempotency_key: "restore-kill-apply".to_owned(),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn backup_restore_external_target_during_extract_and_publish_intent_is_preserved() {
    let _serial = RPC_TEST_LOCK.lock().await;
    for checkpoint in ["extracting", "publish_intent"] {
        let fixture = TestDirectory::new();
        let pause_signal = fixture.path().join("restore-pause.txt");
        let pause_release = fixture.path().join("restore-release.txt");
        let operation_signal = fixture.path().join("restore-operation.json");
        let child = Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "subprocess_runs_backup_restore_until_parent_kills_it",
                "--ignored",
                "--nocapture",
            ])
            .env(RESTORE_CRASH_ROOT_ENV, fixture.path())
            .env(RESTORE_PAUSE_GATE_ENV, checkpoint)
            .env(RESTORE_PAUSE_SIGNAL_ENV, &pause_signal)
            .env(RESTORE_PAUSE_RELEASE_ENV, &pause_release)
            .env(RESTORE_OPERATION_SIGNAL_ENV, &operation_signal)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn");
        let mut child = ChildGuard(Some(child));
        wait_for_file(&operation_signal, None);
        wait_for_file(&pause_signal, Some(checkpoint.as_bytes()));
        let signal: RestoreCrashSignal =
            serde_json::from_slice(&fs::read(&operation_signal).expect("operation signal"))
                .expect("parse operation signal");
        let target = fixture.path().join("Restored");
        fs::create_dir(&target).expect("external target");
        fs::write(target.join("sentinel.txt"), checkpoint).expect("external sentinel");
        fs::write(&pause_release, b"release").expect("release worker");

        let (_, config) = isolated_ipc(fixture.path().join("runtime"));
        let mut client = connect_with_retry(config).await;
        let failed = wait_for_operation_error(&mut client, &signal.operation_id).await;
        assert!(
            matches!(
                failed.state,
                OperationState::Failed | OperationState::Recovering
            ),
            "{checkpoint}: {failed:?}"
        );
        assert_eq!(
            failed.error_code.as_deref(),
            Some("backup_target_exists"),
            "{checkpoint}"
        );
        assert_eq!(
            fs::read_to_string(target.join("sentinel.txt")).expect("preserved sentinel"),
            checkpoint
        );
        child.0.as_mut().expect("child").kill().expect("stop child");
        child.0.as_mut().expect("child").wait().expect("wait child");
        child.0 = None;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn backup_create_real_kill_restart_matrix_reuses_operation_and_idempotency() {
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn backup_restore_real_kill_restart_matrix_reuses_operation_project_and_idempotency() {
    let _serial = RPC_TEST_LOCK.lock().await;
    for checkpoint in [
        "archive_verified",
        "extracting",
        "staging_complete",
        "publish_intent",
        "target_published",
        "project_registry_commit_intent",
        "state_committed",
    ] {
        run_restore_kill_restart_case(checkpoint).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn backup_restore_post_publish_external_change_requires_manual_recovery() {
    let _serial = RPC_TEST_LOCK.lock().await;
    let fixture = TestDirectory::new();
    let kill_signal = fixture.path().join("restore-kill.txt");
    let operation_signal = fixture.path().join("restore-operation.json");
    let child = Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "subprocess_runs_backup_restore_until_parent_kills_it",
            "--ignored",
            "--nocapture",
        ])
        .env(RESTORE_CRASH_ROOT_ENV, fixture.path())
        .env(RESTORE_KILL_GATE_ENV, "target_published")
        .env(RESTORE_KILL_SIGNAL_ENV, &kill_signal)
        .env(RESTORE_OPERATION_SIGNAL_ENV, &operation_signal)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn");
    let mut child = ChildGuard(Some(child));
    wait_for_file(&operation_signal, None);
    wait_for_file(&kill_signal, Some(b"target_published"));
    let signal: RestoreCrashSignal =
        serde_json::from_slice(&fs::read(&operation_signal).expect("signal"))
            .expect("parse signal");
    child.0.as_mut().expect("child").kill().expect("force kill");
    child.0.as_mut().expect("child").wait().expect("wait");
    child.0 = None;
    fs::write(
        fixture.path().join("Restored/Assets/external.txt"),
        b"external",
    )
    .expect("external mutation");

    let (ipc, config) = isolated_ipc(fixture.path().join("runtime"));
    let shutdown = Arc::new(AtomicBool::new(false));
    let server_shutdown = Arc::clone(&shutdown);
    let data = fixture.path().join("data");
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        ipc,
        DataConfig::isolated(data),
        wait_for_shutdown(server_shutdown),
    )
    .await
    .expect("start owned test daemon");
    let mut client = connect_with_retry(config).await;
    let replay = client
        .backup_restore(restore_kill_request(fixture.path()))
        .await
        .expect("replay");
    assert!(replay.replayed);
    tokio::time::sleep(Duration::from_millis(300)).await;
    let operation = client
        .operation_get(signal.operation_id)
        .await
        .expect("operation");
    assert_eq!(operation.state, OperationState::Recovering, "{operation:?}");
    assert_eq!(
        operation.error_code.as_deref(),
        Some("backup_restore_recovery_required")
    );
    assert_eq!(
        fs::read(fixture.path().join("Restored/Assets/external.txt")).expect("external file"),
        b"external"
    );
    let listed = client
        .projects_list(None, Some(100))
        .await
        .expect("projects");
    assert_eq!(
        listed.projects.len(),
        1,
        "modified target was not registered"
    );
    shutdown.store(true, Ordering::Release);
    daemon.await.expect("join").expect("daemon");
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RestoreCrashSignal {
    operation_id: String,
}

#[test]
#[ignore = "subprocess fixture invoked by Backup Restore kill/restart matrix"]
fn subprocess_runs_backup_restore_until_parent_kills_it() {
    let root = PathBuf::from(std::env::var_os(RESTORE_CRASH_ROOT_ENV).expect("crash root"));
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    runtime.block_on(async move {
        let source = root.join("Source");
        create_project(&source);
        let runtime_root = root.join("runtime");
        let data = root.join("data");
        fs::create_dir_all(&runtime_root).expect("runtime root");
        fs::create_dir_all(&data).expect("data");
        let (ipc, config) = isolated_ipc(runtime_root);
        let shutdown = Arc::new(AtomicBool::new(false));
        let server_shutdown = Arc::clone(&shutdown);
        let _daemon = alcomd_daemon::test_support::start_with_data_until(
            ipc,
            DataConfig::isolated(data),
            wait_for_shutdown(server_shutdown),
        )
        .await
        .expect("start owned test daemon");
        let mut client = connect_with_retry(config).await;
        let registered = client
            .project_register(
                source.to_string_lossy().into_owned(),
                "restore-kill-source".to_owned(),
            )
            .await
            .expect("register source");
        if matches!(
            std::env::var(RESTORE_KILL_GATE_ENV).as_deref(),
            Ok("target_published" | "project_registry_commit_intent")
        ) {
            let settings = client.settings_get().await.expect("settings");
            assert!(settings.settings.projects.auto_discover);
            client
                .settings_update(
                    serde_json::from_value(serde_json::json!({
                        "expectedRevision": settings.revision,
                        "update": { "paths": { "projectDirectory": root.to_string_lossy() } }
                    }))
                    .expect("discovery root update"),
                )
                .await
                .expect("use isolated restore parent as default root");
        }
        let created = client
            .backup_create(BackupCreateParams {
                project_id: registered.project.project_id.expect("source id"),
                expected_revision: 1,
                compression_mode: Some(BackupCompression::Fast),
                exclude_vpm_packages: Some(true),
                include_library: Some(false),
                destination_path: Some(path_string(&root.join("Source.zip"))),
                idempotency_key: "restore-kill-backup".to_owned(),
            })
            .await
            .expect("backup");
        assert_eq!(
            wait_for_operation(&mut client, &created.operation_id)
                .await
                .state,
            OperationState::Succeeded
        );
        let accepted = client
            .backup_restore(restore_kill_request(&root))
            .await
            .expect("restore");
        let signal = RestoreCrashSignal {
            operation_id: accepted.operation_id,
        };
        fs::write(
            std::env::var_os(RESTORE_OPERATION_SIGNAL_ENV).expect("operation signal"),
            serde_json::to_vec(&signal).expect("serialize signal"),
        )
        .expect("write operation signal");
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });
}

async fn run_restore_kill_restart_case(checkpoint: &str) {
    let fixture = TestDirectory::new();
    let kill_signal = fixture.path().join("restore-kill.txt");
    let operation_signal = fixture.path().join("restore-operation.json");
    let child = Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "subprocess_runs_backup_restore_until_parent_kills_it",
            "--ignored",
            "--nocapture",
        ])
        .env(RESTORE_CRASH_ROOT_ENV, fixture.path())
        .env(RESTORE_KILL_GATE_ENV, checkpoint)
        .env(RESTORE_KILL_SIGNAL_ENV, &kill_signal)
        .env(RESTORE_OPERATION_SIGNAL_ENV, &operation_signal)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn");
    let mut child = ChildGuard(Some(child));
    wait_for_file(&operation_signal, None);
    wait_for_file(&kill_signal, Some(checkpoint.as_bytes()));
    let signal: RestoreCrashSignal =
        serde_json::from_slice(&fs::read(&operation_signal).expect("signal"))
            .expect("parse signal");
    child.0.as_mut().expect("child").kill().expect("force kill");
    child.0.as_mut().expect("child").wait().expect("wait");
    child.0 = None;

    let (ipc, config) = isolated_ipc(fixture.path().join("runtime"));
    let shutdown = Arc::new(AtomicBool::new(false));
    let server_shutdown = Arc::clone(&shutdown);
    let data = fixture.path().join("data");
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        ipc,
        DataConfig::isolated(data),
        wait_for_shutdown(server_shutdown),
    )
    .await
    .expect("start owned test daemon");
    let mut client = connect_with_retry(config).await;
    if matches!(
        checkpoint,
        "target_published" | "project_registry_commit_intent"
    ) {
        let settings = client
            .settings_get()
            .await
            .expect("recovery remains ready for settings reads");
        assert!(settings.settings.projects.auto_discover);
        assert_eq!(
            fs::canonicalize(settings.directories.project_directory).expect("configured root"),
            fs::canonicalize(fixture.path()).expect("fixture root")
        );
    }
    let replay = client
        .backup_restore(restore_kill_request(fixture.path()))
        .await
        .expect("restore replay");
    assert!(replay.replayed, "{checkpoint}");
    assert_eq!(replay.operation_id, signal.operation_id, "{checkpoint}");
    let completed = wait_for_operation(&mut client, &signal.operation_id).await;
    assert_eq!(
        completed.state,
        OperationState::Succeeded,
        "{checkpoint}: {completed:?}"
    );
    let project_id = completed.result.as_ref().expect("restore receipt")["projectId"]
        .as_str()
        .expect("project ID")
        .to_owned();
    let project = client
        .project_get(project_id.clone())
        .await
        .expect("project");
    assert_eq!(
        project.project.last_modified_at_ms, project.project.registered_at_ms,
        "{checkpoint}"
    );
    assert!(
        project.project.last_modified_at_ms.is_some(),
        "{checkpoint}"
    );
    let replay = client
        .backup_restore(restore_kill_request(fixture.path()))
        .await
        .expect("replay completed recovered restore");
    assert!(replay.replayed, "{checkpoint}");
    assert_eq!(
        client
            .project_get(project_id.clone())
            .await
            .expect("recovered target after replay")
            .project
            .last_modified_at_ms,
        project.project.last_modified_at_ms,
        "{checkpoint}: recovery replay preserves target activity"
    );
    assert_eq!(
        project.project.project_id.as_deref(),
        Some(project_id.as_str())
    );
    let target = fixture.path().join("Restored");
    assert!(
        target.join("ProjectSettings/ProjectVersion.txt").is_file(),
        "{checkpoint}"
    );
    assert!(!target.join("project").exists(), "{checkpoint}");
    if matches!(
        checkpoint,
        "target_published" | "project_registry_commit_intent"
    ) {
        let projects = client
            .projects_list(None, Some(100))
            .await
            .expect("registry after startup recovery");
        assert_eq!(
            projects.projects.len(),
            2,
            "only source and the reserved restore target"
        );
        let targets = projects
            .projects
            .iter()
            .filter(|project| {
                fs::canonicalize(&project.root_path).expect("registered path")
                    == fs::canonicalize(&target).expect("restored target")
            })
            .collect::<Vec<_>>();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].project_id.as_deref(), Some(project_id.as_str()));
        let events = client
            .events_list(0, Some(100))
            .await
            .expect("restore registration events");
        assert_eq!(
            events
                .events
                .iter()
                .filter(
                    |event| event.kind == "project.registered" && event.aggregate_id == project_id
                )
                .count(),
            1
        );
        assert_eq!(
            fs::read(target.join("Assets/source.bin")).expect("restored source content"),
            vec![0x5a; 256 * 1024]
        );
    }
    assert!(
        !fixture
            .path()
            .join(format!(".alcomd-restore-{}", signal.operation_id))
            .exists(),
        "{checkpoint}"
    );
    shutdown.store(true, Ordering::Release);
    daemon.await.expect("join").expect("daemon");
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
        let _daemon = alcomd_daemon::test_support::start_with_data_until(
            ipc,
            DataConfig::isolated(data),
            wait_for_shutdown(server_shutdown),
        )
        .await
        .expect("start owned test daemon");
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
                compression_mode: Some(BackupCompression::Fast),
                exclude_vpm_packages: Some(true),
                include_library: Some(true),
                destination_path: None,
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
    let accepted: OperationAccepted =
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
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        ipc,
        DataConfig::isolated(data),
        wait_for_shutdown(server_shutdown),
    )
    .await
    .expect("start owned test daemon");
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
            compression_mode: Some(BackupCompression::Fast),
            exclude_vpm_packages: Some(true),
            include_library: Some(true),
            destination_path: None,
            idempotency_key: "backup-kill-create".to_owned(),
        })
        .await
        .expect("replay");
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, accepted.operation_id);
    let completed = wait_for_operation(&mut client, &accepted.operation_id).await;
    assert_eq!(
        completed.state,
        OperationState::Succeeded,
        "{checkpoint}: {completed:?}"
    );
    let objects = PathBuf::from(
        client
            .settings_get()
            .await
            .expect("backup location")
            .directories
            .backup_directory,
    );
    assert_eq!(
        fs::read_dir(&objects).expect("objects").count(),
        1,
        "{checkpoint}"
    );
    let result = completed.result.expect("archive receipt");
    let archive = objects.join(result["archiveName"].as_str().expect("archive name"));
    assert_eq!(
        Some(fs::metadata(&archive).expect("archive").len()),
        result["archiveBytes"].as_u64()
    );
    assert_archive_profile(&archive, true);
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
        "Library/Artifacts",
        "Logs",
        "UserSettings",
        ".idea",
        "Assets/.git",
    ] {
        fs::create_dir_all(root.join(directory)).expect("directory");
    }
    fs::write(root.join("Assets/source.bin"), vec![0x5a; 256 * 1024]).expect("source");
    fs::write(root.join("Library/Artifacts/cache.bin"), b"library cache").expect("library cache");
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
    fs::write(root.join("Packages/vpm-manifest.json"), b"{\"dependencies\":{\"com.example.locked\":{\"version\":\"1.0.0\"}},\"locked\":{\"com.example.locked\":{\"version\":\"1.0.0\"}}}").expect("vpm");
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

fn assert_archive_profile(path: &Path, include_library: bool) {
    let file = fs::File::open(path).expect("archive");
    let mut archive = zip::ZipArchive::new(file).expect("zip");
    let names = (0..archive.len())
        .map(|index| {
            archive
                .by_index(index)
                .expect("entry")
                .name()
                .expect("entry name")
                .into_owned()
        })
        .collect::<Vec<_>>();
    assert!(names.contains(&"Assets/source.bin".to_owned()));
    assert!(names.contains(&"ProjectSettings/ProjectVersion.txt".to_owned()));
    assert!(names.contains(&"Packages/manifest.json".to_owned()));
    assert!(names.contains(&"Packages/vpm-manifest.json".to_owned()));
    assert!(names.contains(&"Packages/com.example.embedded/package.json".to_owned()));
    assert!(names.contains(&"LibraryCache/LastSceneManagerSetup.txt".to_owned()));
    assert!(
        !names
            .iter()
            .any(|name| name.starts_with("Packages/com.example.locked/"))
    );
    assert!(!names.iter().any(|name| name.starts_with("Logs/")));
    assert!(!names.iter().any(|name| name.contains("/.git/")));
    assert!(!names.contains(&"LibraryCache/Sub/LastSceneManagerSetup.txt".to_owned()));
    assert_eq!(
        names.contains(&"Library/Artifacts/cache.bin".to_owned()),
        include_library
    );
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

async fn wait_for_operation_error(
    client: &mut AlcomdClient,
    operation_id: &str,
) -> alcomd_protocol::Operation {
    for _ in 0..600 {
        let operation = client
            .operation_get(operation_id.to_owned())
            .await
            .expect("operation");
        if operation.error_code.is_some() {
            return operation;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("operation did not record a stable error")
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
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        match AlcomdClient::connect(config.clone()).await {
            Ok(client) => return client,
            Err(ClientError::StartTimeout) if tokio::time::Instant::now() < deadline => {}
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
