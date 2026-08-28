use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use alcomd_client::{AlcomdClient, ClientConfig, ClientError};
use alcomd_platform::{DataConfig, IpcConfig};
use alcomd_protocol::{
    ActivityListParams, AppearanceDensity, AppearanceMode, AppearanceMotion,
    AppearanceSettingsUpdate, DiagnosticsListParams, NullableUpdate, SettingsLocale,
    SettingsUpdate, SettingsUpdateParams,
};
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn official_settings_activity_and_diagnostics_round_trip_without_new_state_schema() {
    let fixture = TestDirectory::new();
    let runtime = fixture.path().join("runtime");
    let data = fixture.path().join("data");
    fs::create_dir(&runtime).expect("create runtime directory");
    fs::create_dir(&data).expect("create data directory");
    let (ipc, config) = isolated_ipc(runtime);
    let shutdown = Arc::new(AtomicBool::new(false));
    let daemon_shutdown = Arc::clone(&shutdown);
    let daemon_data = data.clone();
    let mut daemon = tokio::spawn(async move {
        alcomd_daemon::serve_with_data_until(
            ipc,
            DataConfig::isolated(daemon_data),
            wait_for_shutdown(daemon_shutdown),
        )
        .await
    });
    let connect = connect_with_retry(config);
    tokio::pin!(connect);
    let mut client = tokio::select! {
        client = &mut connect => client,
        result = &mut daemon => panic!("daemon stopped before bind: {result:?}"),
    };

    let defaults = client.settings_get().await.expect("read default settings");
    assert_eq!(defaults.config_schema, 1);
    assert_eq!(defaults.revision, 1);
    assert_eq!(defaults.settings.appearance.mode, AppearanceMode::System);

    let updated = client
        .settings_update(SettingsUpdateParams {
            expected_revision: 1,
            update: SettingsUpdate {
                appearance: Some(AppearanceSettingsUpdate {
                    mode: Some(AppearanceMode::Dark),
                    source_color: NullableUpdate::Set("#315DA8".to_owned()),
                    density: Some(AppearanceDensity::Compact),
                    motion: Some(AppearanceMotion::Reduced),
                }),
                locale: Some(SettingsLocale::ZhCn),
            },
        })
        .await
        .expect("update settings");
    assert_eq!(updated.revision, 2);
    assert_eq!(updated.settings.locale, SettingsLocale::ZhCn);
    assert_eq!(
        updated.settings.appearance.source_color.as_deref(),
        Some("#315DA8")
    );

    let stale = client
        .settings_update(SettingsUpdateParams {
            expected_revision: 1,
            update: SettingsUpdate {
                locale: Some(SettingsLocale::JaJp),
                ..SettingsUpdate::default()
            },
        })
        .await
        .expect_err("stale settings update");
    assert!(matches!(stale, ClientError::Remote(error) if error.code == "revision_conflict"));

    let accepted = client
        .state_check("m7-official-state-check".to_owned())
        .await
        .expect("start operation");
    for _ in 0..100 {
        let operation = client
            .operation_get(accepted.operation_id.clone())
            .await
            .expect("read operation");
        if matches!(operation.state, alcomd_protocol::OperationState::Succeeded) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let activity = client
        .activity_list(ActivityListParams {
            cursor: None,
            limit: Some(2),
        })
        .await
        .expect("list activity");
    assert_eq!(activity.items.len(), 2);
    assert!(activity.next_cursor.is_some());
    let serialized = serde_json::to_string(&activity).expect("serialize redacted activity");
    for forbidden in [
        "Authorization",
        "Bearer ",
        "request_json",
        "payload_json",
        "state.db",
    ] {
        assert!(!serialized.contains(forbidden), "leaked {forbidden}");
    }
    assert!(
        client
            .diagnostics_list(DiagnosticsListParams::default())
            .await
            .expect("list diagnostics")
            .items
            .is_empty()
    );

    shutdown.store(true, Ordering::Relaxed);
    daemon.await.expect("join daemon").expect("daemon shutdown");
    let settings_file =
        fs::read_to_string(data.join("config/settings.toml")).expect("read durable settings file");
    assert_eq!(
        settings_file,
        "schema = 1\nrevision = 2\nlocale = \"zh-CN\"\n\n[appearance]\nmode = \"dark\"\nsource_color = \"#315DA8\"\ndensity = \"compact\"\nmotion = \"reduced\"\n"
    );
}

#[cfg(unix)]
fn isolated_ipc(runtime: PathBuf) -> (IpcConfig, ClientConfig) {
    (
        IpcConfig::isolated(runtime.clone()),
        ClientConfig::default()
            .with_runtime_directory(runtime)
            .without_daemon_start(),
    )
}

#[cfg(windows)]
fn isolated_ipc(_runtime: PathBuf) -> (IpcConfig, ClientConfig) {
    (
        IpcConfig::default(),
        ClientConfig::default().without_daemon_start(),
    )
}

async fn connect_with_retry(config: ClientConfig) -> AlcomdClient {
    for _ in 0..100 {
        match AlcomdClient::connect(config.clone()).await {
            Ok(client) => return client,
            Err(_) => tokio::time::sleep(Duration::from_millis(10)).await,
        }
    }
    panic!("daemon did not bind")
}

async fn wait_for_shutdown(shutdown: Arc<AtomicBool>) {
    while !shutdown.load(Ordering::Relaxed) {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        #[cfg(target_os = "macos")]
        let base = PathBuf::from("/private/tmp");
        #[cfg(not(target_os = "macos"))]
        let base = std::env::temp_dir();
        let path = base.join(format!("alcomd-m7-official-{}", Uuid::new_v4()));
        fs::create_dir(&path).expect("create test directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        for _ in 0..20 {
            if fs::remove_dir_all(&self.0).is_ok() || !self.0.exists() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("failed to remove test directory")
    }
}
