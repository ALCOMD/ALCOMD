use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use alcomd_client::{AlcomdClient, ClientConfig, ClientError};
use alcomd_platform::{DataConfig, IpcConfig};
use alcomd_protocol::{
    BackupApplyRestoreParams, BackupCompression, BackupCreateParams, BackupPlanRestoreParams,
    BackupsListParams, OperationState,
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
async fn backup_restore_plans_applies_and_registers_one_new_project() {
    let _serial = RPC_TEST_LOCK.lock().await;
    let fixture = TestDirectory::new();
    let source = fixture.path().join("Source");
    let target = fixture.path().join("Restored");
    create_project(&source);
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
    let registered = client
        .project_register(
            source.to_string_lossy().into_owned(),
            "restore-source".to_owned(),
        )
        .await
        .expect("register source");
    let source_id = registered.project.project_id.expect("source id");
    let created = client
        .backup_create(BackupCreateParams {
            project_id: source_id,
            expected_revision: 1,
            compression_mode: BackupCompression::Fast,
            exclude_vpm_packages: true,
            idempotency_key: "restore-backup".to_owned(),
        })
        .await
        .expect("create backup");
    assert_eq!(
        wait_for_operation(&mut client, &created.operation_id)
            .await
            .state,
        OperationState::Succeeded
    );

    fs::create_dir(fixture.path().join("PreExistingTarget")).expect("pre-existing target");
    let preexisting = client
        .backup_plan_restore(BackupPlanRestoreParams {
            backup_id: created.backup_id.clone(),
            target_parent: fixture.path().to_string_lossy().into_owned(),
            target_leaf: "PreExistingTarget".to_owned(),
        })
        .await
        .expect_err("Plan must reject an existing target");
    assert!(matches!(
        preexisting,
        alcomd_client::ClientError::Remote(ref remote)
            if remote.code == "backup_target_exists"
    ));
    let plan = client
        .backup_plan_restore(BackupPlanRestoreParams {
            backup_id: created.backup_id,
            target_parent: fixture.path().to_string_lossy().into_owned(),
            target_leaf: "Restored".to_owned(),
        })
        .await
        .expect("plan restore");
    assert!(!target.exists());
    let accepted = client
        .backup_apply_restore(BackupApplyRestoreParams {
            plan_id: plan.plan_id.clone(),
            idempotency_key: "restore-apply".to_owned(),
        })
        .await
        .expect("apply restore");
    assert_eq!(accepted.project_id, plan.project_id);
    let completed = wait_for_operation(&mut client, &accepted.operation_id).await;
    assert_eq!(completed.state, OperationState::Succeeded, "{completed:?}");
    assert!(target.join("ProjectSettings/ProjectVersion.txt").is_file());
    assert!(target.join("Packages/manifest.json").is_file());
    assert!(!target.join("project").exists());
    assert!(
        !fixture
            .path()
            .join(format!(".alcomd-restore-{}", accepted.operation_id))
            .exists()
    );
    let project = client
        .project_get(plan.project_id.clone())
        .await
        .expect("restored project");
    assert_eq!(
        project.project.project_id.as_deref(),
        Some(plan.project_id.as_str())
    );
    assert_eq!(project.project.favorite, Some(false));
    let replay = client
        .backup_apply_restore(BackupApplyRestoreParams {
            plan_id: plan.plan_id,
            idempotency_key: "restore-apply".to_owned(),
        })
        .await
        .expect("restore replay");
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, accepted.operation_id);
    shutdown.store(true, Ordering::Release);
    daemon.await.expect("join").expect("daemon");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn backup_restore_fails_closed_for_artifact_and_target_races() {
    let _serial = RPC_TEST_LOCK.lock().await;
    let fixture = TestDirectory::new();
    let source = fixture.path().join("Source");
    create_project(&source);
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
    let registered = client
        .project_register(
            source.to_string_lossy().into_owned(),
            "race-source".to_owned(),
        )
        .await
        .expect("register");
    let source_id = registered.project.project_id.expect("source id");
    let created = client
        .backup_create(BackupCreateParams {
            project_id: source_id.clone(),
            expected_revision: 1,
            compression_mode: BackupCompression::Fast,
            exclude_vpm_packages: false,
            idempotency_key: "race-backup".to_owned(),
        })
        .await
        .expect("backup");
    assert_eq!(
        wait_for_operation(&mut client, &created.operation_id)
            .await
            .state,
        OperationState::Succeeded
    );

    let target_plan = client
        .backup_plan_restore(BackupPlanRestoreParams {
            backup_id: created.backup_id.clone(),
            target_parent: fixture.path().to_string_lossy().into_owned(),
            target_leaf: "TargetRace".to_owned(),
        })
        .await
        .expect("target plan");
    fs::create_dir(fixture.path().join("TargetRace")).expect("racing target");
    fs::write(fixture.path().join("TargetRace/sentinel.txt"), b"external").expect("sentinel");
    let target_apply = client
        .backup_apply_restore(BackupApplyRestoreParams {
            plan_id: target_plan.plan_id,
            idempotency_key: "target-race".to_owned(),
        })
        .await
        .expect("target apply accepted");
    let target_failed = wait_for_operation(&mut client, &target_apply.operation_id).await;
    assert_eq!(target_failed.state, OperationState::Failed);
    assert_eq!(
        target_failed.error_code.as_deref(),
        Some("backup_target_exists")
    );
    assert_eq!(
        fs::read(fixture.path().join("TargetRace/sentinel.txt")).expect("sentinel"),
        b"external"
    );

    for (index, mutation) in ["replace", "truncate", "modify", "recreate"]
        .into_iter()
        .enumerate()
    {
        let artifact = client
            .backup_create(BackupCreateParams {
                project_id: source_id.clone(),
                expected_revision: 1,
                compression_mode: BackupCompression::Fast,
                exclude_vpm_packages: false,
                idempotency_key: format!("artifact-backup-{mutation}"),
            })
            .await
            .expect("artifact backup");
        assert_eq!(
            wait_for_operation(&mut client, &artifact.operation_id)
                .await
                .state,
            OperationState::Succeeded
        );
        let target_leaf = format!("ArtifactRace{index}");
        let artifact_plan = client
            .backup_plan_restore(BackupPlanRestoreParams {
                backup_id: artifact.backup_id.clone(),
                target_parent: fixture.path().to_string_lossy().into_owned(),
                target_leaf: target_leaf.clone(),
            })
            .await
            .expect("artifact plan");
        let archive = fixture
            .path()
            .join("data/backups/objects")
            .join(format!("{}.zip", artifact.backup_id));
        let original = fs::read(&archive).expect("read original archive");
        match mutation {
            "replace" => {
                let displaced = archive.with_extension("original");
                fs::rename(&archive, &displaced).expect("displace archive");
                fs::copy(&displaced, &archive).expect("replace archive");
            }
            "truncate" => fs::OpenOptions::new()
                .write(true)
                .open(&archive)
                .expect("open archive")
                .set_len(u64::try_from(original.len() / 2).expect("length"))
                .expect("truncate archive"),
            "modify" => {
                let mut changed = original;
                let offset = changed.len() / 2;
                changed[offset] ^= 0x5a;
                fs::write(&archive, changed).expect("modify archive bytes");
            }
            "recreate" => {
                let removed = archive.with_extension("removed");
                fs::rename(&archive, &removed).expect("preserve removed archive identity");
                fs::write(&archive, original).expect("recreate archive");
            }
            _ => unreachable!(),
        }
        let artifact_apply = client
            .backup_apply_restore(BackupApplyRestoreParams {
                plan_id: artifact_plan.plan_id,
                idempotency_key: format!("artifact-race-{mutation}"),
            })
            .await
            .expect("artifact apply accepted");
        let artifact_failed = wait_for_operation(&mut client, &artifact_apply.operation_id).await;
        assert_eq!(artifact_failed.state, OperationState::Failed, "{mutation}");
        assert!(
            matches!(
                artifact_failed.error_code.as_deref(),
                Some("backup_restore_plan_stale" | "backup_integrity_mismatch")
            ),
            "{mutation}: {artifact_failed:?}"
        );
        assert!(!fixture.path().join(target_leaf).exists(), "{mutation}");
    }
    shutdown.store(true, Ordering::Release);
    daemon.await.expect("join").expect("daemon");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn backup_restore_same_target_is_serialized_by_project_create_lock() {
    let _serial = RPC_TEST_LOCK.lock().await;
    let fixture = TestDirectory::new();
    let source = fixture.path().join("Source");
    create_project(&source);
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
    let mut client = connect_with_retry(config.clone()).await;
    let registered = client
        .project_register(
            source.to_string_lossy().into_owned(),
            "concurrent-source".to_owned(),
        )
        .await
        .expect("register");
    let created = client
        .backup_create(BackupCreateParams {
            project_id: registered.project.project_id.expect("source id"),
            expected_revision: 1,
            compression_mode: BackupCompression::Fast,
            exclude_vpm_packages: false,
            idempotency_key: "concurrent-backup".to_owned(),
        })
        .await
        .expect("backup");
    assert_eq!(
        wait_for_operation(&mut client, &created.operation_id)
            .await
            .state,
        OperationState::Succeeded
    );
    let params = |leaf: &str| BackupPlanRestoreParams {
        backup_id: created.backup_id.clone(),
        target_parent: fixture.path().to_string_lossy().into_owned(),
        target_leaf: leaf.to_owned(),
    };
    let plan_a = client
        .backup_plan_restore(params("Contended"))
        .await
        .expect("plan a");
    let plan_b = client
        .backup_plan_restore(params("Contended"))
        .await
        .expect("plan b");
    let mut second = connect_with_retry(config).await;
    let (apply_a, apply_b) = tokio::join!(
        client.backup_apply_restore(BackupApplyRestoreParams {
            plan_id: plan_a.plan_id,
            idempotency_key: "contended-a".to_owned(),
        }),
        second.backup_apply_restore(BackupApplyRestoreParams {
            plan_id: plan_b.plan_id,
            idempotency_key: "contended-b".to_owned(),
        })
    );
    let apply_a = apply_a.expect("apply a");
    let apply_b = apply_b.expect("apply b");
    let result_a = wait_for_operation(&mut client, &apply_a.operation_id).await;
    let result_b = wait_for_operation(&mut client, &apply_b.operation_id).await;
    let states = [result_a.state, result_b.state];
    assert_eq!(
        states
            .iter()
            .filter(|state| **state == OperationState::Succeeded)
            .count(),
        1
    );
    assert_eq!(
        states
            .iter()
            .filter(|state| **state == OperationState::Failed)
            .count(),
        1
    );
    assert!(
        fixture
            .path()
            .join("Contended/ProjectSettings/ProjectVersion.txt")
            .is_file()
    );
    let projects = client
        .projects_list(None, Some(10))
        .await
        .expect("projects");
    assert_eq!(projects.projects.len(), 2);
    shutdown.store(true, Ordering::Release);
    daemon.await.expect("join").expect("daemon");
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn backup_restore_real_kill_restart_matrix_reuses_plan_operation_project_and_idempotency() {
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
    let daemon = tokio::spawn(async move {
        alcomd_daemon::serve_with_data_until(
            ipc,
            DataConfig::isolated(data),
            wait_for_shutdown(server_shutdown),
        )
        .await
    });
    let mut client = connect_with_retry(config).await;
    let replay = client
        .backup_apply_restore(BackupApplyRestoreParams {
            plan_id: signal.plan_id,
            idempotency_key: "restore-kill-apply".to_owned(),
        })
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
    assert!(client.project_get(signal.project_id).await.is_err());
    shutdown.store(true, Ordering::Release);
    daemon.await.expect("join").expect("daemon");
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RestoreCrashSignal {
    plan_id: String,
    operation_id: String,
    project_id: String,
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
                source.to_string_lossy().into_owned(),
                "restore-kill-source".to_owned(),
            )
            .await
            .expect("register source");
        let created = client
            .backup_create(BackupCreateParams {
                project_id: registered.project.project_id.expect("source id"),
                expected_revision: 1,
                compression_mode: BackupCompression::Fast,
                exclude_vpm_packages: true,
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
        let plan = client
            .backup_plan_restore(BackupPlanRestoreParams {
                backup_id: created.backup_id,
                target_parent: root.to_string_lossy().into_owned(),
                target_leaf: "Restored".to_owned(),
            })
            .await
            .expect("plan");
        let accepted = client
            .backup_apply_restore(BackupApplyRestoreParams {
                plan_id: plan.plan_id.clone(),
                idempotency_key: "restore-kill-apply".to_owned(),
            })
            .await
            .expect("apply");
        let signal = RestoreCrashSignal {
            plan_id: plan.plan_id,
            operation_id: accepted.operation_id,
            project_id: plan.project_id,
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
    let daemon = tokio::spawn(async move {
        alcomd_daemon::serve_with_data_until(
            ipc,
            DataConfig::isolated(data),
            wait_for_shutdown(server_shutdown),
        )
        .await
    });
    let mut client = connect_with_retry(config).await;
    let replay = client
        .backup_apply_restore(BackupApplyRestoreParams {
            plan_id: signal.plan_id.clone(),
            idempotency_key: "restore-kill-apply".to_owned(),
        })
        .await
        .expect("restore replay");
    assert!(replay.replayed, "{checkpoint}");
    assert_eq!(replay.operation_id, signal.operation_id, "{checkpoint}");
    assert_eq!(replay.project_id, signal.project_id, "{checkpoint}");
    let completed = wait_for_operation(&mut client, &signal.operation_id).await;
    assert_eq!(
        completed.state,
        OperationState::Succeeded,
        "{checkpoint}: {completed:?}"
    );
    let project = client
        .project_get(signal.project_id.clone())
        .await
        .expect("project");
    assert_eq!(
        project.project.project_id.as_deref(),
        Some(signal.project_id.as_str())
    );
    let target = fixture.path().join("Restored");
    assert!(
        target.join("ProjectSettings/ProjectVersion.txt").is_file(),
        "{checkpoint}"
    );
    assert!(!target.join("project").exists(), "{checkpoint}");
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
