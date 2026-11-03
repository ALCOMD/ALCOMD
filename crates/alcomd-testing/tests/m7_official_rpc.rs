use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use alcomd_client::{AlcomdClient, ClientConfig, ClientError};
use alcomd_platform::{DataConfig, IpcConfig, IpcStream};
use alcomd_protocol::{
    ActivityListParams, AppearanceMode, AppearanceMotion, AppearanceScheme,
    AppearanceSettingsUpdate, DiagnosticsListParams, NullableUpdate, PackageSettingsUpdate,
    SettingsLocale, SettingsUpdate, SettingsUpdateParams,
};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn official_settings_activity_and_diagnostics_round_trip_without_new_state_schema() {
    let fixture = TestDirectory::new();
    let runtime = fixture.path().join("runtime");
    let data = fixture.path().join("data");
    fs::create_dir(&runtime).expect("create runtime directory");
    fs::create_dir(&data).expect("create data directory");
    let (ipc, config) = isolated_ipc(runtime);
    let raw_ipc = ipc.clone();
    let shutdown = Arc::new(AtomicBool::new(false));
    let daemon_shutdown = Arc::clone(&shutdown);
    let daemon_data = data.clone();
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        ipc,
        DataConfig::isolated(daemon_data),
        wait_for_shutdown(daemon_shutdown),
    )
    .await
    .expect("start owned test daemon");
    let mut client = connect_with_retry(config.clone()).await;

    let defaults = client.settings_get().await.expect("read default settings");
    assert_eq!(defaults.config_schema, 10);
    assert_eq!(defaults.revision, 1);
    assert!(defaults.settings.projects.auto_discover);
    assert_eq!(
        defaults.settings.backups.compression_mode,
        alcomd_protocol::BackupCompression::Fast
    );
    assert!(defaults.settings.backups.include_vpm_packages);
    assert!(!defaults.settings.backups.include_library);
    assert!(!defaults.settings.window.remember_state);
    assert!(!defaults.settings.window.fullscreen);
    assert_eq!(defaults.settings.unity.default_launch_arguments, None);
    assert_eq!(defaults.settings.appearance.mode, AppearanceMode::System);
    assert_eq!(
        defaults.settings.appearance.scheme,
        AppearanceScheme::Vibrant
    );

    let updated = client
        .settings_update(SettingsUpdateParams {
            expected_revision: 1,
            update: SettingsUpdate {
                projects: Some(alcomd_protocol::ProjectSettingsUpdate {
                    auto_discover: false,
                }),
                backups: Some(alcomd_protocol::BackupSettingsUpdate {
                    compression_mode: Some(alcomd_protocol::BackupCompression::Maximum),
                    include_vpm_packages: Some(false),
                    include_library: Some(true),
                }),
                window: Some(alcomd_protocol::WindowSettingsUpdate {
                    remember_state: Some(true),
                    fullscreen: None,
                }),
                paths: None,
                unity: Some(alcomd_protocol::UnitySettingsUpdate {
                    default_launch_arguments: NullableUpdate::Set(vec![
                        "-logFile".to_owned(),
                        " C:\\logs with spaces\\Editor.log ".to_owned(),
                        String::new(),
                    ]),
                    ..alcomd_protocol::UnitySettingsUpdate::default()
                }),
                appearance: Some(AppearanceSettingsUpdate {
                    mode: Some(AppearanceMode::Dark),
                    scheme: Some(AppearanceScheme::Fidelity),
                    source_color: NullableUpdate::Set("#315DA8".to_owned()),
                    motion: Some(AppearanceMotion::Full),
                }),
                locale: Some(SettingsLocale::ZhCn),
                packages: Some(PackageSettingsUpdate {
                    show_prerelease: Some(true),
                    hidden_repository_ids: Some(vec![
                        "ffffffff-ffff-ffff-ffff-ffffffffffff".to_owned(),
                        "00000000-0000-0000-0000-000000000000".to_owned(),
                    ]),
                    hide_local_user_packages: Some(true),
                }),
            },
        })
        .await
        .expect("update settings");
    assert_eq!(updated.revision, 2);
    assert!(updated.settings.window.remember_state);
    assert!(!updated.settings.projects.auto_discover);
    assert_eq!(
        updated.settings.backups.compression_mode,
        alcomd_protocol::BackupCompression::Maximum
    );
    assert!(!updated.settings.backups.include_vpm_packages);
    assert!(updated.settings.backups.include_library);
    assert_eq!(
        updated.settings.unity.default_launch_arguments,
        Some(vec![
            "-logFile".to_owned(),
            " C:\\logs with spaces\\Editor.log ".to_owned(),
            String::new()
        ])
    );
    assert_eq!(updated.settings.appearance.motion, AppearanceMotion::Full);
    assert_eq!(
        updated.settings.appearance.scheme,
        AppearanceScheme::Fidelity
    );
    assert_eq!(updated.settings.locale, SettingsLocale::ZhCn);
    assert!(updated.settings.packages.show_prerelease);
    assert_eq!(
        updated.settings.packages.hidden_repository_ids,
        [
            "00000000-0000-0000-0000-000000000000",
            "ffffffff-ffff-ffff-ffff-ffffffffffff",
        ]
    );
    assert_eq!(
        updated.settings.appearance.source_color.as_deref(),
        Some("#315DA8")
    );
    let mut latest = updated;
    for scheme in [
        AppearanceScheme::TonalSpot,
        AppearanceScheme::Monochrome,
        AppearanceScheme::Neutral,
        AppearanceScheme::Vibrant,
        AppearanceScheme::Expressive,
        AppearanceScheme::Content,
        AppearanceScheme::Rainbow,
        AppearanceScheme::FruitSalad,
    ] {
        let previous = latest.clone();
        latest = client
            .settings_update(SettingsUpdateParams {
                expected_revision: previous.revision,
                update: SettingsUpdate {
                    window: None,
                    appearance: Some(AppearanceSettingsUpdate {
                        scheme: Some(scheme),
                        ..AppearanceSettingsUpdate::default()
                    }),
                    ..SettingsUpdate::default()
                },
            })
            .await
            .expect("round trip each theme scheme through the daemon");
        let mut expected = previous;
        expected.revision += 1;
        expected.settings.appearance.scheme = scheme;
        assert_eq!(latest, expected);
        assert_eq!(
            client.settings_get().await.expect("read saved theme"),
            latest
        );
    }

    // Exercise malformed wire values against the same daemon, without typed-client normalization.
    let mut raw = alcomd_platform::connect(&raw_ipc)
        .await
        .expect("connect raw settings client");
    let hello = json!({
        "rpcVersion": alcomd_protocol::RPC_VERSION,
        "client": {
            "name": "alcomd-testing",
            "version": env!("CARGO_PKG_VERSION"),
            "instanceId": Uuid::new_v4().to_string()
        },
        "capabilities": []
    });
    raw_request(
        &mut raw,
        "hello",
        alcomd_protocol::METHOD_SYSTEM_HELLO,
        hello,
    )
    .await
    .expect("handshake raw settings client");
    let settings_path = data.join("config/settings.toml");
    let bytes_before = fs::read(&settings_path).expect("read settings before malformed updates");
    for (index, invalid_update) in [
        json!({"appearance": null, "locale": "ja-JP"}),
        json!({"locale": null, "appearance": {"mode": "light"}}),
        json!({"packages": null, "locale": "ja-JP"}),
        json!({"appearance": {"mode": null, "sourceColor": null}, "locale": "ja-JP"}),
        json!({"appearance": {"scheme": null}, "locale": "ja-JP"}),
        json!({"appearance": {"density": null}, "locale": "ja-JP"}),
        json!({"appearance": {"density": "default"}, "locale": "ja-JP"}),
        json!({"appearance": {"density": "compact"}, "locale": "ja-JP"}),
        json!({"appearance": {"motion": null}, "locale": "ja-JP"}),
        json!({"packages": {"showPrerelease": null}, "locale": "ja-JP"}),
        json!({"packages": {"hiddenRepositoryIds": null}, "locale": "ja-JP"}),
        json!({"packages": {"hideLocalUserPackages": null}, "locale": "ja-JP"}),
        json!({"unity": null, "locale": "ja-JP"}),
        json!({"unity": {}, "locale": "ja-JP"}),
        json!({"unity": {"hubPath": true}, "locale": "ja-JP"}),
        json!({"unity": {"hubPath": ""}, "locale": "ja-JP"}),
        json!({"unity": {"hubPath": "relative/hub"}, "locale": "ja-JP"}),
        json!({"unity": {"hubPath": null, "launch": true}, "locale": "ja-JP"}),
    ]
    .into_iter()
    .enumerate()
    {
        let response = raw_request(
            &mut raw,
            &format!("invalid-null-{index}"),
            alcomd_protocol::METHOD_SETTINGS_UPDATE,
            json!({"expectedRevision": latest.revision, "update": invalid_update}),
        )
        .await;
        let error =
            response.expect_err("null mixed with a valid field must reject the whole request");
        assert_eq!(
            error.code,
            alcomd_protocol::error_code::INVALID_REQUEST,
            "{invalid_update}"
        );
        assert_eq!(
            fs::read(&settings_path).expect("read unchanged settings"),
            bytes_before,
            "{invalid_update}"
        );
        assert_eq!(
            client
                .settings_get()
                .await
                .expect("read after rejected update"),
            latest,
            "{invalid_update}"
        );
    }
    let clear_response = raw_request(
        &mut raw,
        "clear-source-color",
        alcomd_protocol::METHOD_SETTINGS_UPDATE,
        json!({"expectedRevision": latest.revision, "update": {"appearance": {"sourceColor": null}}}),
    )
    .await
    .expect("sourceColor null remains an intentional clear");
    let cleared: alcomd_protocol::SettingsGetResult =
        serde_json::from_value(clear_response).expect("decode cleared settings");
    let mut expected_clear = latest;
    expected_clear.revision += 1;
    expected_clear.settings.appearance.source_color = None;
    assert_eq!(cleared, expected_clear);
    assert_eq!(
        client.settings_get().await.expect("read cleared settings"),
        expected_clear
    );
    assert_ne!(
        fs::read(&settings_path).expect("read cleared settings file"),
        bytes_before
    );
    let no_capability = raw_request(
        &mut raw,
        "hub-open-without-capability",
        alcomd_protocol::METHOD_UNITY_HUB_OPEN,
        json!({"expectedRevision": expected_clear.revision, "expectedPath": null}),
    )
    .await
    .expect_err("Hub open has an independent negotiated capability");
    assert_eq!(no_capability.code, "capability_required");
    drop(raw);

    // Filesystem evidence is derived from the same accepted Config snapshot, never persisted.
    let hub_file = data.join("test Hub.exe");
    fs::write(&hub_file, b"isolated executable fixture; never launched").expect("Hub fixture");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&hub_file, fs::Permissions::from_mode(0o755))
            .expect("executable fixture");
    }
    let selected_path = fs::canonicalize(&hub_file)
        .expect("canonical fixture")
        .to_str()
        .expect("UTF-8 path")
        .to_owned();
    let hub_saved = client
        .settings_update(SettingsUpdateParams {
            expected_revision: expected_clear.revision,
            update: SettingsUpdate {
                window: None,
                unity: Some(alcomd_protocol::UnitySettingsUpdate {
                    default_launch_arguments: NullableUpdate::Unchanged,
                    hub_path: NullableUpdate::Set(
                        hub_file.to_str().expect("UTF-8 selection").to_owned(),
                    ),
                }),
                ..SettingsUpdate::default()
            },
        })
        .await
        .expect("save validated Hub location");
    let normalized_path = hub_saved
        .settings
        .unity
        .hub_path
        .as_deref()
        .expect("saved path");
    assert_eq!(
        fs::canonicalize(normalized_path).expect("saved path keeps the same file"),
        PathBuf::from(&selected_path)
    );
    #[cfg(windows)]
    assert!(!normalized_path.starts_with(r"\\?\"));
    assert_eq!(hub_saved.unity_hub.path.as_deref(), Some(normalized_path));
    assert_eq!(
        hub_saved.unity_hub.status,
        alcomd_protocol::UnityHubStatus::Available
    );
    assert_eq!(
        hub_saved.unity_hub.source,
        alcomd_protocol::UnityHubSource::Configured
    );
    assert_eq!(
        hub_saved.unity_hub.configured_path_status,
        alcomd_protocol::UnityHubConfiguredPathStatus::Available
    );
    let mut expected_settings = expected_clear.settings.clone();
    expected_settings.unity.hub_path = Some(normalized_path.to_owned());
    assert_eq!(hub_saved.settings, expected_settings);
    assert_eq!(hub_saved.revision, expected_clear.revision + 1);
    let saved_bytes = fs::read(&settings_path).expect("saved Hub config");
    assert_eq!(client.settings_get().await.expect("reread Hub"), hub_saved);
    fs::remove_file(&hub_file).expect("remove isolated Hub fixture");
    let missing = client
        .settings_get()
        .await
        .expect("missing selected Hub is evidence, not whole-page error");
    assert_eq!(missing.revision, hub_saved.revision);
    assert_eq!(missing.settings, hub_saved.settings);
    assert_eq!(
        missing.unity_hub.configured_path_status,
        alcomd_protocol::UnityHubConfiguredPathStatus::Invalid
    );
    assert_eq!(
        fs::read(&settings_path).expect("read-only discovery"),
        saved_bytes
    );
    let stale_open = client
        .unity_hub_open(alcomd_protocol::UnityHubOpenParams {
            expected_revision: missing.revision - 1,
            expected_path: missing.unity_hub.path.clone(),
        })
        .await
        .expect_err("stale Hub intent cannot launch anything");
    assert!(matches!(stale_open, ClientError::Remote(error) if error.code == "revision_conflict"));
    let changed_open = client
        .unity_hub_open(alcomd_protocol::UnityHubOpenParams {
            expected_revision: missing.revision,
            expected_path: Some(
                data.join("unselected Guard.exe")
                    .to_str()
                    .expect("UTF-8 guard")
                    .to_owned(),
            ),
        })
        .await
        .expect_err("mismatched target cannot launch anything");
    let expected_code = if missing.unity_hub.status == alcomd_protocol::UnityHubStatus::Unavailable
    {
        "hub_unavailable"
    } else {
        "hub_path_changed"
    };
    assert!(matches!(changed_open, ClientError::Remote(error) if error.code == expected_code));
    if missing.unity_hub.status == alcomd_protocol::UnityHubStatus::NotFound {
        let invalid_open = client
            .unity_hub_open(alcomd_protocol::UnityHubOpenParams {
                expected_revision: missing.revision,
                expected_path: None,
            })
            .await
            .expect_err("broken manual location is not a download diagnosis");
        assert!(
            matches!(invalid_open, ClientError::Remote(error) if error.code == "hub_path_invalid")
        );
    }
    assert_eq!(
        fs::read(&settings_path).expect("open guards are read only"),
        saved_bytes
    );
    let unrelated = client
        .settings_update(SettingsUpdateParams {
            expected_revision: missing.revision,
            update: SettingsUpdate {
                window: None,
                locale: Some(SettingsLocale::JaJp),
                ..SettingsUpdate::default()
            },
        })
        .await
        .expect("unrelated update does not revalidate removed Hub");
    assert_eq!(unrelated.settings.unity, missing.settings.unity);
    assert_eq!(unrelated.revision, missing.revision + 1);
    assert_eq!(unrelated.settings.locale, SettingsLocale::JaJp);
    let hub_cleared = client
        .settings_update(SettingsUpdateParams {
            expected_revision: unrelated.revision,
            update: SettingsUpdate {
                window: None,
                unity: Some(alcomd_protocol::UnitySettingsUpdate {
                    default_launch_arguments: NullableUpdate::Unchanged,
                    hub_path: NullableUpdate::Clear,
                }),
                ..SettingsUpdate::default()
            },
        })
        .await
        .expect("clear explicit Hub preference");
    assert_eq!(hub_cleared.settings.unity.hub_path, None);
    assert_eq!(hub_cleared.revision, unrelated.revision + 1);
    assert_eq!(
        hub_cleared.unity_hub.configured_path_status,
        alcomd_protocol::UnityHubConfiguredPathStatus::Unset
    );
    if hub_cleared.unity_hub.status == alcomd_protocol::UnityHubStatus::NotFound {
        let download = client
            .unity_hub_open(alcomd_protocol::UnityHubOpenParams {
                expected_revision: hub_cleared.revision,
                expected_path: None,
            })
            .await
            .expect("no configured or discovered Hub returns download without launching");
        assert_eq!(
            download.outcome,
            alcomd_protocol::UnityHubOpenOutcome::DownloadRequired
        );
    }

    let stale = client
        .settings_update(SettingsUpdateParams {
            expected_revision: 1,
            update: SettingsUpdate {
                window: None,
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
        r#"schema = 10
revision = 14
locale = "ja-JP"

[appearance]
mode = "dark"
scheme = "fruit-salad"
motion = "full"

[packages]
show_prerelease = true
hidden_repository_ids = ["00000000-0000-0000-0000-000000000000", "ffffffff-ffff-ffff-ffff-ffffffffffff"]
hide_local_user_packages = true

[unity]
default_launch_arguments = ["-logFile", " C:\\logs with spaces\\Editor.log ", ""]

[window]
remember_state = true
fullscreen = false

[projects]
auto_discover = false

[backups]
compression_mode = "maximum"
include_vpm_packages = false
include_library = true
"#
    );
    // A valid pre-fullscreen Config9 must retain all accepted preferences and revision.
    let config9 = settings_file
        .replacen("schema = 10\n", "schema = 9\n", 1)
        .replacen("fullscreen = false\n", "", 1);
    let settings_path = data.join("config/settings.toml");
    fs::write(&settings_path, config9).expect("isolated Config9 migration input");
    let restarted_shutdown = Arc::new(AtomicBool::new(false));
    let restarted = alcomd_daemon::test_support::start_with_data_until(
        raw_ipc,
        DataConfig::isolated(data),
        wait_for_shutdown(Arc::clone(&restarted_shutdown)),
    )
    .await
    .expect("migrate owned Config9 daemon to Config10");
    let mut reopened = connect_with_retry(config).await;
    let persisted = reopened.settings_get().await.expect("reopened settings");
    assert_eq!(persisted.config_schema, 10);
    assert_eq!(persisted.revision, 14);
    assert_eq!(persisted.settings, hub_cleared.settings);
    assert!(!persisted.settings.window.fullscreen);
    assert_eq!(
        fs::read_to_string(&settings_path).expect("migrated canonical settings"),
        settings_file,
        "migration changes only schema and the new default, without a user revision increment"
    );
    restarted_shutdown.store(true, Ordering::Relaxed);
    restarted
        .await
        .expect("join restarted daemon")
        .expect("restarted daemon shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn fullscreen_settings_survive_daemon_restart_and_narrow_changes_preserve_remember_state() {
    let fixture = TestDirectory::new();
    let runtime = fixture.path().join("runtime-fullscreen");
    let data = fixture.path().join("data-fullscreen");
    fs::create_dir(&runtime).expect("runtime");
    fs::create_dir(&data).expect("data");
    let (ipc, config) = isolated_ipc(runtime);
    let shutdown = Arc::new(AtomicBool::new(false));
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        ipc.clone(),
        DataConfig::isolated(data.clone()),
        wait_for_shutdown(Arc::clone(&shutdown)),
    )
    .await
    .expect("owned daemon");
    let mut client = connect_with_retry(config.clone()).await;
    let defaults = client.settings_get().await.expect("defaults");
    assert_eq!(defaults.config_schema, 10);
    assert!(!defaults.settings.window.fullscreen);
    let enabled = client
        .settings_update(SettingsUpdateParams {
            expected_revision: defaults.revision,
            update: SettingsUpdate {
                window: Some(alcomd_protocol::WindowSettingsUpdate {
                    fullscreen: Some(true),
                    remember_state: None,
                }),
                ..SettingsUpdate::default()
            },
        })
        .await
        .expect("enable fullscreen");
    assert!(enabled.settings.window.fullscreen);
    assert!(!enabled.settings.window.remember_state);
    let remembered = client
        .settings_update(SettingsUpdateParams {
            expected_revision: enabled.revision,
            update: SettingsUpdate {
                window: Some(alcomd_protocol::WindowSettingsUpdate {
                    fullscreen: None,
                    remember_state: Some(true),
                }),
                ..SettingsUpdate::default()
            },
        })
        .await
        .expect("remember only");
    assert!(remembered.settings.window.fullscreen);
    assert!(remembered.settings.window.remember_state);
    drop(client);
    shutdown.store(true, Ordering::Relaxed);
    daemon.await.expect("join daemon").expect("shutdown");
    let shutdown = Arc::new(AtomicBool::new(false));
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        ipc.clone(),
        DataConfig::isolated(data),
        wait_for_shutdown(Arc::clone(&shutdown)),
    )
    .await
    .expect("reopened daemon");
    let mut client = connect_with_retry(config).await;
    assert_eq!(
        client.settings_get().await.expect("reopened preferences"),
        remembered
    );
    let disabled = client
        .settings_update(SettingsUpdateParams {
            expected_revision: remembered.revision,
            update: SettingsUpdate {
                window: Some(alcomd_protocol::WindowSettingsUpdate {
                    fullscreen: Some(false),
                    remember_state: None,
                }),
                ..SettingsUpdate::default()
            },
        })
        .await
        .expect("disable fullscreen only");
    assert!(!disabled.settings.window.fullscreen);
    assert!(disabled.settings.window.remember_state);
    assert_eq!(disabled.revision, remembered.revision + 1);
    let mut stream = alcomd_platform::connect(&ipc).await.expect("raw stream");
    raw_request(
        &mut stream,
        "hello-fullscreen",
        "system.hello",
        json!({"rpcVersion":alcomd_protocol::RPC_VERSION,"client":{"name":"alcomd-testing","version":env!("CARGO_PKG_VERSION"),"instanceId":Uuid::new_v4().to_string()},"capabilities":[]}),
    )
    .await
    .expect("raw hello");
    for (index, value) in [
        json!({}),
        json!({"fullscreen":null}),
        json!({"fullscreen":"true"}),
        json!({"fullscreen":true,"width":1000}),
    ]
    .into_iter()
    .enumerate()
    {
        let result = raw_request(&mut stream, &format!("invalid-fullscreen-{index}"), "settings.update", json!({"expectedRevision":disabled.revision,"update":{"window":value,"locale":"ja-JP"}})).await;
        assert!(matches!(result, Err(error) if error.code == "invalid_request"));
        assert_eq!(
            client
                .settings_get()
                .await
                .expect("unchanged after rejection"),
            disabled
        );
    }
    drop(stream);
    drop(client);
    shutdown.store(true, Ordering::Relaxed);
    daemon
        .await
        .expect("join reopened daemon")
        .expect("reopened shutdown");
}

async fn raw_request(
    stream: &mut IpcStream,
    id: &str,
    method: &str,
    params: Value,
) -> Result<Value, alcomd_protocol::RpcError> {
    tokio::time::timeout(Duration::from_secs(10), async {
        let request = alcomd_protocol::RequestEnvelope {
            id: id.to_owned(),
            method: method.to_owned(),
            params,
        };
        let payload = serde_json::to_vec(&request).expect("serialize raw settings request");
        let frame = alcomd_protocol::encode_frame(&payload).expect("frame raw settings request");
        stream
            .write_all(&frame)
            .await
            .expect("write raw settings request");
        stream.flush().await.expect("flush raw settings request");
        let mut prefix = [0_u8; 4];
        stream
            .read_exact(&mut prefix)
            .await
            .expect("read settings response prefix");
        let length =
            alcomd_protocol::decode_frame_length(prefix).expect("valid settings response length");
        let mut payload = vec![0_u8; length];
        stream
            .read_exact(&mut payload)
            .await
            .expect("read raw settings response");
        match serde_json::from_slice::<alcomd_protocol::Response<Value>>(&payload)
            .expect("decode raw settings response")
        {
            alcomd_protocol::Response::Success(response) => {
                assert_eq!(response.id, id);
                Ok(response.result)
            }
            alcomd_protocol::Response::Error(response) => {
                assert_eq!(response.id.as_deref(), Some(id));
                Err(response.error)
            }
        }
    })
    .await
    .expect("raw settings request completes before timeout")
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
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        match AlcomdClient::connect(config.clone()).await {
            Ok(client) => return client,
            Err(ClientError::StartTimeout) if tokio::time::Instant::now() < deadline => {}
            Err(error) => panic!("daemon did not bind: {error}"),
        }
    }
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
