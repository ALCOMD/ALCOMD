use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use alcomd_client::{AlcomdClient, ClientConfig};
use alcomd_platform::{DataConfig, IpcConfig};
use alcomd_protocol::{
    ExtensionApplyParams, ExtensionDataDisposition, ExtensionDesiredState, ExtensionGrantParams,
    ExtensionLifecycleParams, ExtensionPlanInstallParams, ExtensionPlanUninstallParams,
    ExtensionPublisherApproval, ExtensionRuntimeState, ExtensionSourceKind, OperationState,
};
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

static RPC_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
const CRASH_ROOT_ENV: &str = "ALCOMD_M6_CRASH_ROOT";
const CRASH_METADATA_ENV: &str = "ALCOMD_M6_CRASH_METADATA";
const KILL_GATE_ENV: &str = "ALCOMD_TEST_M6_KILL_GATE";
const KILL_SIGNAL_ENV: &str = "ALCOMD_TEST_M6_KILL_SIGNAL";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn signed_extension_lifecycle_runs_through_real_daemon_and_host() {
    let _serial = RPC_TEST_LOCK.lock().await;
    let fixture = TestDirectory::new();
    let runtime = fixture.path().join("runtime");
    let data = fixture.path().join("data");
    fs::create_dir(&runtime).expect("runtime");
    fs::create_dir(&data).expect("data");
    let package = fixture.path().join("fixture.alcomdext");
    create_signed_package(&package);

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

    let plan = client
        .extension_plan_install(ExtensionPlanInstallParams {
            source_kind: ExtensionSourceKind::LocalOwnerSelected,
            package_path: package.to_string_lossy().into_owned(),
            expected_revision: 0,
            publisher_approval: ExtensionPublisherApproval::ApproveForExtension,
        })
        .await
        .expect("plan install")
        .plan;
    let accepted = client
        .extension_apply_install(ExtensionApplyParams {
            plan_id: plan.plan_id,
            idempotency_key: "m6-install".to_owned(),
        })
        .await
        .expect("apply install");
    assert!(!accepted.replayed);
    assert_eq!(
        wait_for_terminal(&mut client, &accepted.operation_id).await,
        OperationState::Succeeded
    );

    let installed = client
        .extension_get("dev.alcomd.fixture".to_owned())
        .await
        .expect("installed extension")
        .extension;
    let package_parent = data
        .join("extensions/packages")
        .join(&installed.extension_id);
    let package_root = fs::read_dir(package_parent)
        .expect("live package parent")
        .next()
        .expect("live package")
        .expect("live entry")
        .path();
    let verified = alcomd_extensions::inspect_extension_directory(&package_root)
        .expect("reinspect installed package");
    assert_eq!(installed.version, verified.manifest.version);
    assert_eq!(installed.api_major, verified.manifest.api);
    assert_eq!(installed.package_digest, hex(&verified.package_digest));
    assert_eq!(
        installed.publisher_fingerprint,
        verified.publisher_fingerprint
    );
    let grant = client
        .extension_set_grant(ExtensionGrantParams {
            extension_id: installed.extension_id.clone(),
            permission: "background.run".to_owned(),
            resource_kind: "Extension".to_owned(),
            resource_id: installed.extension_id.clone(),
            expected_grant_revision: installed.grant_revision,
            idempotency_key: "m6-grant-background".to_owned(),
        })
        .await
        .expect("grant background");
    assert_eq!(grant.state, "granted");
    let revision = client
        .extension_get(installed.extension_id.clone())
        .await
        .expect("granted extension")
        .extension
        .revision;
    let enabled = client
        .extension_enable(ExtensionLifecycleParams {
            extension_id: installed.extension_id.clone(),
            expected_revision: revision,
            idempotency_key: "m6-enable".to_owned(),
        })
        .await
        .expect("enable real component")
        .extension;
    assert_eq!(enabled.runtime_state, ExtensionRuntimeState::Running);

    let revoked = client
        .extension_revoke_grant(ExtensionGrantParams {
            extension_id: enabled.extension_id.clone(),
            permission: "background.run".to_owned(),
            resource_kind: "Extension".to_owned(),
            resource_id: enabled.extension_id.clone(),
            expected_grant_revision: enabled.grant_revision,
            idempotency_key: "m6-revoke-background".to_owned(),
        })
        .await
        .expect("revoke active background grant");
    assert_eq!(revoked.state, "revoked");
    let after_revoke = client
        .extension_get(enabled.extension_id)
        .await
        .expect("extension after revoke")
        .extension;
    assert_eq!(after_revoke.desired_state, ExtensionDesiredState::Enabled);
    assert_eq!(after_revoke.runtime_state, ExtensionRuntimeState::Stopped);

    let disabled = client
        .extension_disable(ExtensionLifecycleParams {
            extension_id: after_revoke.extension_id.clone(),
            expected_revision: after_revoke.revision,
            idempotency_key: "m6-disable".to_owned(),
        })
        .await
        .expect("disable real component")
        .extension;
    assert_eq!(disabled.runtime_state, ExtensionRuntimeState::Stopped);

    let uninstall = client
        .extension_plan_uninstall(ExtensionPlanUninstallParams {
            extension_id: disabled.extension_id,
            expected_revision: disabled.revision,
            data_disposition: ExtensionDataDisposition::RetainData,
        })
        .await
        .expect("plan uninstall")
        .plan;
    let removed = client
        .extension_apply_uninstall(ExtensionApplyParams {
            plan_id: uninstall.plan_id,
            idempotency_key: "m6-uninstall".to_owned(),
        })
        .await
        .expect("apply uninstall");
    assert_eq!(
        wait_for_terminal(&mut client, &removed.operation_id).await,
        OperationState::Succeeded
    );
    assert!(
        client
            .extension_get("dev.alcomd.fixture".to_owned())
            .await
            .is_err()
    );

    shutdown.store(true, Ordering::Release);
    assert!(daemon.await.expect("join daemon").is_ok());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn repeated_real_host_crashes_quarantine_only_that_enabled_extension() {
    let _serial = RPC_TEST_LOCK.lock().await;
    let fixture = TestDirectory::new();
    let runtime = fixture.path().join("runtime");
    let data = fixture.path().join("data");
    fs::create_dir(&runtime).expect("runtime");
    fs::create_dir(&data).expect("data");
    let package = fixture.path().join("crashing.alcomdext");
    create_signed_package_with_component(
        &package,
        include_bytes!("../../../crates/alcomd-testing/fixtures/m6/trap-extension-v1.wasm"),
    );
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
    let plan = client
        .extension_plan_install(ExtensionPlanInstallParams {
            source_kind: ExtensionSourceKind::LocalOwnerSelected,
            package_path: package.to_string_lossy().into_owned(),
            expected_revision: 0,
            publisher_approval: ExtensionPublisherApproval::ApproveForExtension,
        })
        .await
        .expect("plan crash fixture")
        .plan;
    let install = client
        .extension_apply_install(ExtensionApplyParams {
            plan_id: plan.plan_id,
            idempotency_key: "m6-crash-fixture-install".to_owned(),
        })
        .await
        .expect("install crash fixture");
    assert_eq!(
        wait_for_terminal(&mut client, &install.operation_id).await,
        OperationState::Succeeded
    );
    let installed = client
        .extension_get("dev.alcomd.fixture".to_owned())
        .await
        .expect("crash fixture")
        .extension;
    client
        .extension_set_grant(ExtensionGrantParams {
            extension_id: installed.extension_id.clone(),
            permission: "background.run".to_owned(),
            resource_kind: "Extension".to_owned(),
            resource_id: installed.extension_id.clone(),
            expected_grant_revision: installed.grant_revision,
            idempotency_key: "m6-crash-fixture-grant".to_owned(),
        })
        .await
        .expect("grant crash fixture");
    let ready = client
        .extension_get(installed.extension_id.clone())
        .await
        .expect("ready fixture")
        .extension;
    assert!(
        client
            .extension_enable(ExtensionLifecycleParams {
                extension_id: ready.extension_id.clone(),
                expected_revision: ready.revision,
                idempotency_key: "m6-crash-fixture-enable".to_owned(),
            })
            .await
            .is_err()
    );
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let observed = client
            .extension_get(ready.extension_id.clone())
            .await
            .expect("observe quarantine")
            .extension;
        if observed.quarantine_state == alcomd_protocol::ExtensionQuarantineState::Quarantined {
            assert_eq!(
                observed.desired_state,
                alcomd_protocol::ExtensionDesiredState::Enabled
            );
            assert_eq!(observed.runtime_state, ExtensionRuntimeState::Stopped);
            break;
        }
        assert!(tokio::time::Instant::now() < deadline, "quarantine timeout");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    shutdown.store(true, Ordering::Release);
    assert!(daemon.await.expect("join daemon").is_ok());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn extension_filesystem_kill_restart_matrix_reuses_durable_authority() {
    let _serial = RPC_TEST_LOCK.lock().await;
    for checkpoint in [
        "archive_verified",
        "staging_complete",
        "package_published",
        "state_committed",
        "package_moved_to_backup",
    ] {
        run_kill_restart_case(checkpoint).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "subprocess entrypoint for deterministic M6 kill gates"]
async fn subprocess_extension_apply_until_killed() {
    let root = PathBuf::from(std::env::var_os(CRASH_ROOT_ENV).expect("crash root"));
    let metadata_path =
        PathBuf::from(std::env::var_os(CRASH_METADATA_ENV).expect("crash metadata path"));
    let runtime = root.join("runtime");
    let data = root.join("data");
    let package = root.join("fixture.alcomdext");
    create_signed_package(&package);
    let (ipc, config) = isolated_ipc(runtime);
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
    let install_plan = client
        .extension_plan_install(ExtensionPlanInstallParams {
            source_kind: ExtensionSourceKind::LocalOwnerSelected,
            package_path: package.to_string_lossy().into_owned(),
            expected_revision: 0,
            publisher_approval: ExtensionPublisherApproval::ApproveForExtension,
        })
        .await
        .expect("plan install")
        .plan;
    let install = client
        .extension_apply_install(ExtensionApplyParams {
            plan_id: install_plan.plan_id.clone(),
            idempotency_key: "m6-crash-install".to_owned(),
        })
        .await
        .expect("apply install");
    let checkpoint = std::env::var(KILL_GATE_ENV).expect("checkpoint");
    let metadata = if checkpoint == "package_moved_to_backup" {
        assert_eq!(
            wait_for_terminal(&mut client, &install.operation_id).await,
            OperationState::Succeeded
        );
        let extension = client
            .extension_get("dev.alcomd.fixture".to_owned())
            .await
            .expect("installed")
            .extension;
        let plan = client
            .extension_plan_uninstall(ExtensionPlanUninstallParams {
                extension_id: extension.extension_id,
                expected_revision: extension.revision,
                data_disposition: ExtensionDataDisposition::RetainData,
            })
            .await
            .expect("plan uninstall")
            .plan;
        let apply = client
            .extension_apply_uninstall(ExtensionApplyParams {
                plan_id: plan.plan_id.clone(),
                idempotency_key: "m6-crash-uninstall".to_owned(),
            })
            .await
            .expect("apply uninstall");
        CrashMetadata {
            action: "uninstall".to_owned(),
            operation_id: apply.operation_id,
            plan_id: plan.plan_id,
            idempotency_key: "m6-crash-uninstall".to_owned(),
        }
    } else {
        CrashMetadata {
            action: "install".to_owned(),
            operation_id: install.operation_id,
            plan_id: install_plan.plan_id,
            idempotency_key: "m6-crash-install".to_owned(),
        }
    };
    fs::write(
        metadata_path,
        serde_json::to_vec(&metadata).expect("metadata json"),
    )
    .expect("metadata");
    loop {
        let operation = client
            .operation_get(metadata.operation_id.clone())
            .await
            .expect("observe crash-gate operation");
        assert!(
            !matches!(
                operation.state,
                OperationState::Succeeded | OperationState::Failed | OperationState::Cancelled
            ),
            "operation became terminal before the requested kill gate: {operation:?}"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

async fn run_kill_restart_case(checkpoint: &str) {
    let fixture = TestDirectory::new();
    fs::create_dir(fixture.path().join("runtime")).expect("runtime");
    fs::create_dir(fixture.path().join("data")).expect("data");
    let signal = fixture.path().join("kill.signal");
    let metadata_path = fixture.path().join("operation.json");
    let child = Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "subprocess_extension_apply_until_killed",
            "--ignored",
            "--nocapture",
        ])
        .env(CRASH_ROOT_ENV, fixture.path())
        .env(CRASH_METADATA_ENV, &metadata_path)
        .env(KILL_GATE_ENV, checkpoint)
        .env(KILL_SIGNAL_ENV, &signal)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn crash subprocess");
    let mut child = ChildGuard(Some(child));
    wait_for_file(
        &signal,
        checkpoint,
        child.0.as_mut().expect("crash subprocess"),
    );
    wait_for_file(
        &metadata_path,
        "operation metadata",
        child.0.as_mut().expect("crash subprocess"),
    );
    let metadata: CrashMetadata =
        serde_json::from_slice(&fs::read(&metadata_path).expect("read metadata"))
            .expect("parse metadata");
    child.0.as_mut().expect("child").kill().expect("force kill");
    let _ = child.0.as_mut().expect("child").wait().expect("wait");
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
    let recovered_state = wait_for_terminal(&mut client, &metadata.operation_id).await;
    if recovered_state != OperationState::Succeeded {
        let operation = client
            .operation_get(metadata.operation_id.clone())
            .await
            .expect("failed operation evidence");
        panic!("recovery failed at {checkpoint}: {operation:?}");
    }
    let replayed = if metadata.action == "install" {
        client
            .extension_apply_install(ExtensionApplyParams {
                plan_id: metadata.plan_id.clone(),
                idempotency_key: metadata.idempotency_key.clone(),
            })
            .await
            .expect("replay install")
    } else {
        client
            .extension_apply_uninstall(ExtensionApplyParams {
                plan_id: metadata.plan_id.clone(),
                idempotency_key: metadata.idempotency_key.clone(),
            })
            .await
            .expect("replay uninstall")
    };
    assert!(replayed.replayed);
    assert_eq!(replayed.operation_id, metadata.operation_id);
    if metadata.action == "install" {
        client
            .extension_get("dev.alcomd.fixture".to_owned())
            .await
            .expect("complete installed state");
    } else {
        assert!(
            client
                .extension_get("dev.alcomd.fixture".to_owned())
                .await
                .is_err()
        );
    }
    shutdown.store(true, Ordering::Release);
    assert!(daemon.await.expect("join daemon").is_ok());
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CrashMetadata {
    action: String,
    operation_id: String,
    plan_id: String,
    idempotency_key: String,
}

fn wait_for_file(path: &Path, description: &str, child: &mut std::process::Child) {
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        if path.is_file() {
            return;
        }
        if let Some(status) = child.try_wait().expect("inspect crash subprocess") {
            panic!("crash subprocess exited before {description}: {status}");
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {description}"
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

fn create_signed_package(path: &Path) {
    create_signed_package_with_component(
        path,
        include_bytes!("../../../crates/alcomd-testing/fixtures/m6/minimal-extension-v1.wasm"),
    );
}

fn create_signed_package_with_component(path: &Path, component: &[u8]) {
    const MANIFEST_PATH: &str = "alcomd-extension.toml";
    const COMPONENT_PATH: &str = "component/extension.wasm";
    const SIGNATURE_PATH: &str = "META-INF/alcomd-signature-v1.json";
    let signing = SigningKey::from_bytes(&[17_u8; 32]);
    let public_key = signing.verifying_key().to_bytes();
    let fingerprint = format!("ed25519-sha256:{}", hex(&Sha256::digest(public_key)));
    let manifest = format!(
        "schema = 1\nid = \"dev.alcomd.fixture\"\nname = \"M6 Fixture\"\nversion = \"1.0.0\"\napi = 1\npublisher_name = \"ALCOMD Test\"\npublisher_key_fingerprint = \"{fingerprint}\"\nlicense = \"MIT\"\n\n[entrypoints]\nbackground_component = \"component/extension.wasm\"\n\n[interfaces]\nrequired = []\noptional = []\n\n[permissions]\nrequired = [\"background.run\"]\noptional = []\n"
    );
    let content = [
        (MANIFEST_PATH, manifest.as_bytes()),
        (COMPONENT_PATH, component),
    ];
    let package_digest = canonical_digest(&content);
    let mut message = b"ALCOMD-EXT-SIGNATURE-V1\0".to_vec();
    message.extend_from_slice(&package_digest);
    let signature = signing.sign(&message).to_bytes();
    let envelope = serde_json::json!({
        "formatVersion": 1,
        "algorithm": "ed25519",
        "packageDigest": hex(&package_digest),
        "publicKey": hex(&public_key),
        "publisherFingerprint": fingerprint,
        "signature": hex(&signature),
    });
    let file = File::create(path).expect("package file");
    let mut writer = zip::ZipWriter::new(file);
    for (name, bytes) in content {
        writer
            .start_file(
                name,
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
            )
            .expect("content entry");
        writer.write_all(bytes).expect("content");
    }
    writer
        .start_file(
            SIGNATURE_PATH,
            SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
        )
        .expect("signature entry");
    writer
        .write_all(envelope.to_string().as_bytes())
        .expect("signature");
    writer.finish().expect("finish package");
}

fn canonical_digest(entries: &[(&str, &[u8])]) -> [u8; 32] {
    let mut entries = entries.to_vec();
    entries.sort_by_key(|(path, _)| *path);
    let mut digest = Sha256::new();
    digest.update(b"ALCOMD-EXT-CONTENT-SHA256-V1\0");
    for (path, content) in entries {
        digest.update(u32::try_from(path.len()).expect("path").to_le_bytes());
        digest.update(path.as_bytes());
        digest.update(u64::try_from(content.len()).expect("content").to_le_bytes());
        digest.update(content);
    }
    digest.finalize().into()
}

async fn wait_for_terminal(client: &mut AlcomdClient, operation_id: &str) -> OperationState {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        let operation = client
            .operation_get(operation_id.to_owned())
            .await
            .expect("operation");
        if matches!(
            operation.state,
            OperationState::Succeeded | OperationState::Failed | OperationState::Cancelled
        ) {
            return operation.state;
        }
        assert!(tokio::time::Instant::now() < deadline, "operation timeout");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn connect_with_retry(config: ClientConfig) -> AlcomdClient {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        match AlcomdClient::connect(config.clone()).await {
            Ok(client) => return client,
            Err(_) if tokio::time::Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            Err(error) => panic!("daemon not ready: {error}"),
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

fn hex(value: &[u8]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        #[cfg(target_os = "macos")]
        let base = PathBuf::from("/private/tmp");
        #[cfg(not(target_os = "macos"))]
        let base = std::env::temp_dir();
        let path = base.join(format!("alcomd-m6-lifecycle-{}", uuid::Uuid::new_v4()));
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
