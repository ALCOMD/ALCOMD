use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use alcomd_application::{M4Store, StateStore};
use alcomd_client::{AlcomdClient, ClientConfig};
use alcomd_domain::{OperationId, PlanId, PrincipalId};
use alcomd_platform::{DataConfig, IpcConfig};
use alcomd_protocol::{OperationState, PackageOperationPhase, RepositorySource};
use alcomd_store::StateStoreHandle;
use sha2::{Digest, Sha256};
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

const CRASH_ROOT_ENV: &str = "ALCOMD_M4_CRASH_ROOT";
const CRASH_SIGNAL_ENV: &str = "ALCOMD_M4_CRASH_SIGNAL";
const KILL_GATE_ENV: &str = "ALCOMD_TEST_M4_KILL_GATE";
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
            repository_id.clone(),
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
            source: None,
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

    let manifest_before_reinstall =
        fs::read(project.join("Packages/vpm-manifest.json")).expect("manifest before reinstall");
    let reinstall = client
        .package_plan_reinstall(alcomd_protocol::PackagePlanReinstallParams {
            project_id: project_id.clone(),
            expected_revision: 2,
            selection: alcomd_protocol::PackageReinstallSelection::Packages {
                package_ids: vec!["com.example.fixture".to_owned()],
            },
            sources: vec![alcomd_protocol::PackageReinstallSource {
                package_id: "com.example.fixture".to_owned(),
                source: alcomd_protocol::PackageSourceSelector::Repository { repository_id },
            }],
        })
        .await
        .expect("plan exact reinstall");
    assert_eq!(reinstall.change_set.format_version, 1);
    assert_eq!(reinstall.change_set.mutations.len(), 1);
    assert_eq!(
        reinstall.change_set.mutations[0].kind,
        alcomd_protocol::PackageMutationKind::Replace
    );
    assert_eq!(
        reinstall.change_set.mutations[0].from_version,
        reinstall.change_set.mutations[0].to_version
    );
    let accepted = client
        .package_apply_plan(alcomd_protocol::PackageApplyPlanParams {
            plan_id: reinstall.plan_id,
            expected_revision: 2,
            idempotency_key: "p6-apply-reinstall".to_owned(),
        })
        .await
        .expect("apply exact reinstall");
    let completed = wait_for_terminal(&mut client, &accepted.operation_id, &project).await;
    assert_eq!(completed.state, OperationState::Succeeded);
    assert_eq!(
        fs::read(project.join("Packages/vpm-manifest.json")).expect("manifest after reinstall"),
        manifest_before_reinstall,
        "reinstall must preserve exact manifest bytes"
    );

    let remove = client
        .package_plan_bulk(alcomd_protocol::PackagePlanBulkParams {
            project_id,
            expected_revision: 3,
            intents: vec![alcomd_protocol::PackageBulkIntent::Remove {
                package_id: "com.example.fixture".to_owned(),
            }],
        })
        .await
        .expect("plan atomic bulk remove");
    assert_eq!(remove.action, alcomd_protocol::PackagePlanAction::Bulk);
    assert_eq!(remove.change_set.mutations.len(), 1);
    let accepted = client
        .package_apply_plan(alcomd_protocol::PackageApplyPlanParams {
            plan_id: remove.plan_id,
            expected_revision: 3,
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
async fn user_package_plan_uses_owned_cache_after_source_mutation_and_registry_removal() {
    let _serial = RPC_TEST_LOCK.lock().await;
    let fixture = TestDirectory::new();
    let runtime = fixture.path().join("runtime-user-package");
    let data = fixture.path().join("data-user-package");
    let project = fixture.path().join("UserPackageProject");
    let source = fixture.path().join("LoosePackage");
    fs::create_dir(&runtime).expect("create runtime");
    fs::create_dir(&data).expect("create data");
    create_project(&project);
    fs::create_dir_all(source.join("Runtime")).expect("create loose package");
    fs::write(
        source.join("package.json"),
        br#"{"name":"com.example.local","version":"1.0.0","displayName":"Local fixture","vpmDependencies":{}}"#,
    )
    .expect("write loose manifest");
    fs::write(source.join("Runtime/payload.txt"), b"original").expect("write loose payload");

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
    let project_result = client
        .project_register(
            project.to_string_lossy().into_owned(),
            "p6-user-package-project".to_owned(),
        )
        .await
        .expect("register project");
    let project_id = project_result.project.project_id.expect("project ID");
    let enrolled = client
        .user_package_enroll(alcomd_protocol::UserPackageEnrollParams {
            source_path: source.to_string_lossy().into_owned(),
            idempotency_key: "p6-user-package-enroll".to_owned(),
        })
        .await
        .expect("enroll loose package");
    assert_eq!(enrolled.user_package.revision, 1);
    let user_package_id = enrolled.user_package.user_package_id.clone();
    assert_eq!(
        client
            .user_packages_list(alcomd_protocol::UserPackagesListParams {
                cursor: None,
                limit: None,
            })
            .await
            .expect("list User Packages")
            .user_packages
            .len(),
        1
    );

    let plan = client
        .package_plan_install(alcomd_protocol::PackagePlanInstallParams {
            project_id,
            expected_revision: 1,
            package_id: "com.example.local".to_owned(),
            version_range: Some("=1.0.0".to_owned()),
            repository_id: None,
            source: Some(alcomd_protocol::PackageSourceSelector::UserPackage {
                user_package_id: user_package_id.clone(),
            }),
            include_prerelease: false,
        })
        .await
        .expect("plan User Package install");
    assert_eq!(plan.change_set.format_version, 2);
    assert!(matches!(
        plan.change_set.mutations[0].source.as_ref(),
        Some(alcomd_protocol::PackageSourcePin::UserPackage(_))
    ));

    fs::write(source.join("Runtime/payload.txt"), b"mutated after plan")
        .expect("mutate loose source");
    client
        .user_package_remove(alcomd_protocol::UserPackageMutationParams {
            user_package_id,
            expected_revision: 1,
            idempotency_key: "p6-user-package-remove".to_owned(),
        })
        .await
        .expect("remove enrollment only");
    assert!(
        source.is_dir(),
        "remove must leave the user source directory"
    );

    let accepted = client
        .package_apply_plan(alcomd_protocol::PackageApplyPlanParams {
            plan_id: plan.plan_id,
            expected_revision: 1,
            idempotency_key: "p6-user-package-apply".to_owned(),
        })
        .await
        .expect("apply frozen User Package plan");
    let completed = wait_for_terminal(&mut client, &accepted.operation_id, &project).await;
    assert_eq!(completed.state, OperationState::Succeeded);
    assert_eq!(
        fs::read(project.join("Packages/com.example.local/Runtime/payload.txt"))
            .expect("installed owned snapshot"),
        b"original"
    );

    shutdown.store(true, Ordering::Release);
    let result = tokio::time::timeout(Duration::from_secs(3), daemon)
        .await
        .expect("daemon stop timeout")
        .expect("join daemon");
    assert!(result.is_ok());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn user_package_plan_does_not_fall_back_to_mutable_source_when_cache_is_missing() {
    let _serial = RPC_TEST_LOCK.lock().await;
    let fixture = TestDirectory::new();
    let runtime = fixture.path().join("runtime");
    let data = fixture.path().join("data-user-package-missing-cache");
    let project = fixture.path().join("UserPackageMissingCacheProject");
    let source = fixture.path().join("LoosePackageMissingCache");
    fs::create_dir(&runtime).expect("create runtime");
    fs::create_dir(&data).expect("create data");
    create_project(&project);
    fs::create_dir_all(source.join("Runtime")).expect("create loose package");
    fs::write(
        source.join("package.json"),
        br#"{"name":"com.example.local-missing","version":"1.0.0","vpmDependencies":{}}"#,
    )
    .expect("write loose manifest");
    fs::write(source.join("Runtime/payload.txt"), b"original").expect("write loose payload");

    let (ipc, config) = isolated_ipc(runtime);
    let shutdown = Arc::new(AtomicBool::new(false));
    let server_shutdown = Arc::clone(&shutdown);
    let daemon_data = data.clone();
    let daemon = tokio::spawn(async move {
        alcomd_daemon::serve_with_data_until(
            ipc,
            DataConfig::isolated(daemon_data),
            wait_for_shutdown(server_shutdown),
        )
        .await
    });
    let mut client = connect_with_retry(config).await;
    let project_id = client
        .project_register(
            project.to_string_lossy().into_owned(),
            "p6-user-package-missing-cache-project".to_owned(),
        )
        .await
        .expect("register project")
        .project
        .project_id
        .expect("project ID");
    let enrolled = client
        .user_package_enroll(alcomd_protocol::UserPackageEnrollParams {
            source_path: source.to_string_lossy().into_owned(),
            idempotency_key: "p6-user-package-missing-cache-enroll".to_owned(),
        })
        .await
        .expect("enroll loose package")
        .user_package;
    let plan = client
        .package_plan_install(alcomd_protocol::PackagePlanInstallParams {
            project_id,
            expected_revision: 1,
            package_id: "com.example.local-missing".to_owned(),
            version_range: Some("=1.0.0".to_owned()),
            repository_id: None,
            source: Some(alcomd_protocol::PackageSourceSelector::UserPackage {
                user_package_id: enrolled.user_package_id,
            }),
            include_prerelease: false,
        })
        .await
        .expect("plan User Package install");

    fs::write(source.join("Runtime/payload.txt"), b"mutated after plan")
        .expect("mutate loose source");
    let digest = &enrolled.archive_sha256;
    fs::remove_file(
        data.join("package-cache/sha256")
            .join(&digest[..2])
            .join(format!("{digest}.zip")),
    )
    .expect("remove owned cache object");

    let accepted = client
        .package_apply_plan(alcomd_protocol::PackageApplyPlanParams {
            plan_id: plan.plan_id,
            expected_revision: 1,
            idempotency_key: "p6-user-package-missing-cache-apply".to_owned(),
        })
        .await
        .expect("accept frozen plan before cache read");
    let completed = wait_for_terminal(&mut client, &accepted.operation_id, &project).await;
    assert_eq!(completed.state, OperationState::Failed);
    assert_eq!(completed.error_code.as_deref(), Some("offline_cache_miss"));
    assert!(!project.join("Packages/com.example.local-missing").exists());

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
    run_kill_restart_case(KillCheckpoint::ArchiveReady).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn destructive_package_transaction_kill_restart_matrix() {
    let _serial = RPC_TEST_LOCK.lock().await;
    for checkpoint in [
        KillCheckpoint::Prepared,
        KillCheckpoint::OldPackageBackedUp,
        KillCheckpoint::NewPackagePublished,
        KillCheckpoint::VpmManifestCommitted,
        KillCheckpoint::FilesystemCommitted,
    ] {
        run_kill_restart_case(checkpoint).await;
    }
}

#[test]
#[ignore = "subprocess fixture invoked by the package kill/restart test"]
fn subprocess_runs_package_apply_until_killed() {
    let root = PathBuf::from(std::env::var_os(CRASH_ROOT_ENV).expect("crash root"));
    let signal_path = PathBuf::from(std::env::var_os(CRASH_SIGNAL_ENV).expect("crash signal"));
    let checkpoint = KillCheckpoint::parse(
        &std::env::var(KILL_GATE_ENV).expect("deterministic kill checkpoint"),
    );
    let runtime = tokio::runtime::Runtime::new().expect("test runtime");
    runtime.block_on(async move {
        let runtime_root = root.join("runtime");
        let data = root.join("data");
        let project = root.join("Project");
        fs::create_dir(&runtime_root).expect("create runtime");
        fs::create_dir(&data).expect("create data");
        create_project(&project);
        if checkpoint.is_destructive() {
            create_installed_package(&project, "0.9.0", b"old");
        }
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
        let plan_params = alcomd_protocol::PackagePlanInstallParams {
            project_id,
            expected_revision: 1,
            package_id: "com.example.fixture".to_owned(),
            version_range: Some("1.0.0".to_owned()),
            repository_id: None,
            source: None,
            include_prerelease: false,
        };
        let plan = if checkpoint.is_destructive() {
            client
                .package_plan_upgrade(plan_params)
                .await
                .expect("plan crash upgrade")
        } else {
            client
                .package_plan_install(plan_params)
                .await
                .expect("plan crash install")
        };
        let idempotency_key = format!("m4-crash-apply-{}", checkpoint.as_str());
        let accepted = client
            .package_apply_plan(alcomd_protocol::PackageApplyPlanParams {
                plan_id: plan.plan_id.clone(),
                expected_revision: 1,
                idempotency_key: idempotency_key.clone(),
            })
            .await
            .expect("accept crash install");
        let signal = CrashSignal {
            operation_id: accepted.operation_id,
            plan_id: plan.plan_id,
            idempotency_key,
            checkpoint: checkpoint.as_str().to_owned(),
        };
        fs::write(
            signal_path,
            serde_json::to_vec(&signal).expect("serialize operation signal"),
        )
        .expect("write operation signal");
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum KillCheckpoint {
    ArchiveReady,
    Prepared,
    OldPackageBackedUp,
    NewPackagePublished,
    VpmManifestCommitted,
    FilesystemCommitted,
}

impl KillCheckpoint {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ArchiveReady => "archive_ready",
            Self::Prepared => "prepared",
            Self::OldPackageBackedUp => "old_package_backed_up",
            Self::NewPackagePublished => "new_package_published",
            Self::VpmManifestCommitted => "vpm_manifest_committed",
            Self::FilesystemCommitted => "filesystem_committed",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "archive_ready" => Self::ArchiveReady,
            "prepared" => Self::Prepared,
            "old_package_backed_up" => Self::OldPackageBackedUp,
            "new_package_published" => Self::NewPackagePublished,
            "vpm_manifest_committed" => Self::VpmManifestCommitted,
            "filesystem_committed" => Self::FilesystemCommitted,
            _ => panic!("unknown kill checkpoint {value}"),
        }
    }

    const fn is_destructive(self) -> bool {
        !matches!(self, Self::ArchiveReady)
    }

    const fn filesystem_phase(self) -> alcomd_application::FilesystemPhase {
        match self {
            Self::ArchiveReady => alcomd_application::FilesystemPhase::ArchiveReady,
            Self::Prepared => alcomd_application::FilesystemPhase::Prepared,
            Self::OldPackageBackedUp | Self::NewPackagePublished => {
                alcomd_application::FilesystemPhase::PackagesReplaced
            }
            Self::VpmManifestCommitted => alcomd_application::FilesystemPhase::VpmManifestCommitted,
            Self::FilesystemCommitted => alcomd_application::FilesystemPhase::FilesystemCommitted,
        }
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CrashSignal {
    operation_id: String,
    plan_id: String,
    idempotency_key: String,
    checkpoint: String,
}

async fn run_kill_restart_case(checkpoint: KillCheckpoint) {
    let fixture = TestDirectory::new();
    let signal_path = fixture.path().join("operation.signal");
    let child = Command::new(std::env::current_exe().expect("current test executable"))
        .args([
            "--exact",
            "subprocess_runs_package_apply_until_killed",
            "--ignored",
            "--nocapture",
        ])
        .env(CRASH_ROOT_ENV, fixture.path())
        .env(CRASH_SIGNAL_ENV, &signal_path)
        .env(KILL_GATE_ENV, checkpoint.as_str())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn package transaction subprocess");
    let mut child = ChildGuard(Some(child));

    let signal = wait_for_operation_signal(
        &signal_path,
        child.0.as_mut().expect("package transaction subprocess"),
    );
    assert_eq!(signal.checkpoint, checkpoint.as_str());
    let project = fixture.path().join("Project");
    let transaction_root = project
        .join("Library/ALCOMD/transactions")
        .join(&signal.operation_id);
    let attempt = wait_for_test_kill_gate(&transaction_root, checkpoint.as_str());
    assert!(attempt.join("staging").is_dir());
    assert!(attempt.join("backup").is_dir());
    assert_checkpoint_state(&project, &attempt, checkpoint);

    child
        .0
        .as_mut()
        .expect("child process")
        .kill()
        .expect("force-kill package transaction subprocess");
    let _status = child
        .0
        .as_mut()
        .expect("child process")
        .wait()
        .expect("wait for killed subprocess");
    child.0 = None;

    let data = fixture.path().join("data");
    let killed_store = open_store_with_retry(&data.join("state.db"));
    let operation_id = OperationId::parse(&signal.operation_id).expect("Operation ID");
    let killed_operation = killed_store
        .get_operation(PrincipalId::local_owner(), operation_id)
        .await
        .expect("load killed package Operation");
    assert_ne!(
        killed_operation.state,
        alcomd_domain::OperationState::Succeeded,
        "killed checkpoint must not record false success"
    );
    assert_eq!(
        killed_operation.progress_phase,
        Some(checkpoint.filesystem_phase()),
        "kill gate must follow durable journal progress"
    );
    let durable_plan = killed_store
        .get_package_plan(
            PrincipalId::local_owner(),
            PlanId::parse(&signal.plan_id).expect("Plan ID"),
        )
        .await
        .expect("load durable package Plan");
    assert_eq!(durable_plan.apply_operation_id, Some(operation_id));
    drop(killed_store);

    if checkpoint == KillCheckpoint::ArchiveReady {
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
    }

    let (ipc, config) = isolated_ipc(fixture.path().join("runtime"));
    let shutdown = Arc::new(AtomicBool::new(false));
    let server_shutdown = Arc::clone(&shutdown);
    let daemon_data = data.clone();
    let daemon = tokio::spawn(async move {
        alcomd_daemon::serve_with_data_until(
            ipc,
            DataConfig::isolated(daemon_data),
            wait_for_shutdown(server_shutdown),
        )
        .await
    });
    let mut client = connect_with_retry(config).await;
    let completed = wait_for_terminal(&mut client, &signal.operation_id, &project).await;
    assert_eq!(completed.state, OperationState::Succeeded);
    assert_eq!(
        completed.progress.expect("recovery progress").phase,
        PackageOperationPhase::StateCommitted
    );
    assert_eq!(
        completed
            .result
            .as_ref()
            .and_then(|result| result.get("planId"))
            .and_then(serde_json::Value::as_str),
        Some(signal.plan_id.as_str())
    );
    assert_complete_new_state(&project, checkpoint, &signal.operation_id);
    assert!(
        attempt
            .join(format!("test-kill-gate-{}.json", checkpoint.as_str()))
            .is_file(),
        "durable recovery evidence must remain available"
    );

    let replayed = client
        .package_apply_plan(alcomd_protocol::PackageApplyPlanParams {
            plan_id: signal.plan_id,
            expected_revision: 1,
            idempotency_key: signal.idempotency_key,
        })
        .await
        .expect("replay original Apply request");
    assert!(replayed.replayed);
    assert_eq!(replayed.operation_id, signal.operation_id);

    shutdown.store(true, Ordering::Release);
    let result = tokio::time::timeout(Duration::from_secs(3), daemon)
        .await
        .expect("daemon stop timeout")
        .expect("join daemon");
    assert!(result.is_ok());
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

fn create_installed_package(project: &Path, version: &str, content: &[u8]) {
    let package = project.join("Packages/com.example.fixture");
    fs::create_dir_all(package.join("Runtime")).expect("installed package Runtime");
    fs::write(
        package.join("package.json"),
        format!("{{\"name\":\"com.example.fixture\",\"version\":\"{version}\"}}"),
    )
    .expect("installed package manifest");
    fs::write(package.join("Runtime/fixture.txt"), content).expect("installed package content");
    fs::write(
        project.join("Packages/vpm-manifest.json"),
        format!(
            "{{\"dependencies\":{{\"com.example.fixture\":\"{version}\"}},\"locked\":{{\"com.example.fixture\":{{\"version\":\"{version}\"}}}},\"preserved\":true}}\n"
        ),
    )
    .expect("installed VPM manifest");
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

fn wait_for_operation_signal(signal: &Path, child: &mut std::process::Child) -> CrashSignal {
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        if let Ok(bytes) = fs::read(signal)
            && let Ok(value) = serde_json::from_slice(&bytes)
        {
            return value;
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                panic!("package subprocess exited before publishing durable identifiers: {status}")
            }
            Ok(None) => {}
            Err(error) => panic!("failed to inspect package subprocess status: {error}"),
        }
        assert!(
            Instant::now() < deadline,
            "package subprocess did not publish its durable identifiers within 120 seconds"
        );
        thread::sleep(Duration::from_millis(10));
    }
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

fn wait_for_test_kill_gate(transaction: &Path, checkpoint: &str) -> PathBuf {
    let file_name = format!("test-kill-gate-{checkpoint}.json");
    for _ in 0..10_000 {
        if let Ok(attempts) = fs::read_dir(transaction) {
            for attempt in attempts.filter_map(Result::ok) {
                if attempt.path().join(&file_name).is_file() {
                    return attempt.path();
                }
            }
        }
        thread::sleep(Duration::from_millis(5));
    }
    panic!("package subprocess did not durably reach {checkpoint}");
}

fn assert_checkpoint_state(project: &Path, attempt: &Path, checkpoint: KillCheckpoint) {
    let target = project.join("Packages/com.example.fixture");
    let backup = attempt.join("backup/com.example.fixture");
    match checkpoint {
        KillCheckpoint::ArchiveReady => {
            assert!(!target.exists());
            assert_manifest_version(project, None);
        }
        KillCheckpoint::Prepared => {
            assert_eq!(package_version(&target), Some("0.9.0".to_owned()));
            assert_manifest_version(project, Some("0.9.0"));
            assert!(!backup.exists());
        }
        KillCheckpoint::OldPackageBackedUp => {
            assert!(!target.exists());
            assert_eq!(package_version(&backup), Some("0.9.0".to_owned()));
            assert_eq!(
                fs::read(backup.join("Runtime/fixture.txt")).expect("backup content"),
                b"old"
            );
            assert_manifest_version(project, Some("0.9.0"));
        }
        KillCheckpoint::NewPackagePublished => {
            assert_eq!(package_version(&target), Some("1.0.0".to_owned()));
            assert_eq!(package_version(&backup), Some("0.9.0".to_owned()));
            assert_manifest_version(project, Some("0.9.0"));
        }
        KillCheckpoint::VpmManifestCommitted | KillCheckpoint::FilesystemCommitted => {
            assert_eq!(package_version(&target), Some("1.0.0".to_owned()));
            assert_eq!(package_version(&backup), Some("0.9.0".to_owned()));
            assert_manifest_version(project, Some("1.0.0"));
        }
    }
    assert_eq!(
        fs::read(project.join("Packages/manifest.json")).expect("UPM manifest"),
        b"{\"dependencies\":{}}\n"
    );
}

fn assert_complete_new_state(project: &Path, checkpoint: KillCheckpoint, operation_id: &str) {
    assert_eq!(
        package_version(&project.join("Packages/com.example.fixture")),
        Some("1.0.0".to_owned()),
        "checkpoint {checkpoint:?} recovered with transaction entries {:?}",
        transaction_entries(project, operation_id)
    );
    let value: serde_json::Value = serde_json::from_slice(
        &fs::read(project.join("Packages/vpm-manifest.json")).expect("read recovered VPM manifest"),
    )
    .expect("parse recovered VPM manifest");
    assert_eq!(
        value["locked"]["com.example.fixture"]["version"].as_str(),
        Some("1.0.0"),
        "checkpoint {checkpoint:?} recovered with transaction entries {:?}",
        transaction_entries(project, operation_id)
    );
    assert_eq!(
        fs::read(project.join("Packages/manifest.json")).expect("UPM manifest"),
        b"{\"dependencies\":{}}\n"
    );
}

fn package_version(package: &Path) -> Option<String> {
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(package.join("package.json")).ok()?).ok()?;
    value["version"].as_str().map(str::to_owned)
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
