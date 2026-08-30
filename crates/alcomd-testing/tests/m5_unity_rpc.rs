use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use alcomd_client::{AlcomdClient, ClientConfig, ClientError};
use alcomd_platform::{DataConfig, IpcConfig};
use alcomd_protocol::{
    ProjectEditorClearParams, ProjectEditorSelection, ProjectEditorSetParams,
    UnityInstallationRegisterParams, UnityInstallationsListParams, UnityLaunchParams,
    UnityWriterStateKind,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unity_registry_writer_gate_preference_and_launch_round_trip_over_rpc() {
    let fixture = TestDirectory::new();
    let runtime = fixture.path().join("runtime");
    let data = fixture.path().join("data");
    fs::create_dir(&runtime).expect("create runtime directory");
    fs::create_dir(&data).expect("create data directory");
    let project = fixture.path().join("Project");
    create_project(&project);
    let editor = create_fake_editor(fixture.path(), "Editor");

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
    let capabilities = client
        .system_status()
        .await
        .expect("M5 daemon status")
        .capabilities;
    for capability in [
        alcomd_protocol::CAPABILITY_UNITY_READ_V1,
        alcomd_protocol::CAPABILITY_UNITY_MANAGE_V1,
        alcomd_protocol::CAPABILITY_UNITY_LAUNCH_V1,
    ] {
        assert!(capabilities.iter().any(|value| value == capability));
    }

    let project = client
        .project_register(
            project.to_string_lossy().into_owned(),
            "m5-project-register".to_owned(),
        )
        .await
        .expect("register project");
    let project_id = project.project.project_id.expect("project ID");
    let implicit = client
        .project_editor_selection_get(project_id.clone())
        .await
        .expect("read implicit automatic selection");
    assert_eq!(
        implicit.preference.selection,
        ProjectEditorSelection::Automatic
    );
    assert_eq!(implicit.preference.revision, 0);
    assert_eq!(implicit.preference.updated_at_ms, 0);
    assert!(implicit.preference.arguments.is_empty());
    let no_candidate = client
        .unity_launch(UnityLaunchParams {
            project_id: project_id.clone(),
            expected_project_revision: project.project.revision.expect("project revision"),
            idempotency_key: "m7-unity-launch-zero-candidate".to_owned(),
        })
        .await
        .expect_err("automatic launch without candidates fails");
    assert!(matches!(
        no_candidate,
        alcomd_client::ClientError::Remote(ref remote)
            if remote.code == "unity_installation_not_found"
    ));
    let installation = client
        .unity_installation_register(UnityInstallationRegisterParams {
            executable_path: editor.to_string_lossy().into_owned(),
            idempotency_key: "m5-editor-register".to_owned(),
        })
        .await
        .expect("register Editor");
    assert_eq!(installation.installation.unity_version, "2022.3.40f1");
    assert!(
        client
            .unity_installation_register(UnityInstallationRegisterParams {
                executable_path: editor.to_string_lossy().into_owned(),
                idempotency_key: "m5-editor-register".to_owned(),
            })
            .await
            .expect("replay Editor registration")
            .replayed
    );
    assert_eq!(
        client
            .unity_installations_list(UnityInstallationsListParams::default())
            .await
            .expect("list Editors")
            .installations
            .len(),
        1
    );
    let preference = client
        .unity_project_editor_set(ProjectEditorSetParams {
            project_id: project_id.clone(),
            installation_id: installation.installation.installation_id.clone(),
            arguments: vec!["-logFile".to_owned(), "-".to_owned()],
            expected_revision: 0,
            idempotency_key: "m5-editor-preference".to_owned(),
        })
        .await
        .expect("set Editor preference");
    assert_eq!(preference.preference.revision, 1);
    let explicit = client
        .project_editor_selection_get(project_id.clone())
        .await
        .expect("read explicit selection");
    assert_eq!(
        explicit.preference.selection,
        ProjectEditorSelection::Explicit {
            installation_id: installation.installation.installation_id.clone(),
        }
    );
    assert_eq!(explicit.preference.revision, 1);
    let cleared = client
        .project_editor_clear(ProjectEditorClearParams {
            project_id: project_id.clone(),
            expected_revision: explicit.preference.revision,
            idempotency_key: "m7-editor-clear".to_owned(),
        })
        .await
        .expect("clear explicit Editor selection");
    assert_eq!(
        cleared.preference.selection,
        ProjectEditorSelection::Automatic
    );
    assert_eq!(cleared.preference.revision, 2);
    assert_eq!(
        cleared.preference.arguments,
        vec!["-logFile".to_owned(), "-".to_owned()]
    );
    let legacy_get = client
        .unity_project_editor_get(project_id.clone())
        .await
        .expect_err("legacy explicit view hides automatic selection");
    assert!(matches!(
        legacy_get,
        alcomd_client::ClientError::Remote(ref remote)
            if remote.code == "unity_installation_not_found"
    ));
    let writer = client
        .unity_writer_state(project_id.clone())
        .await
        .expect("observe writer state");
    match writer.state {
        UnityWriterStateKind::NotObserved => assert!(writer.evidence.is_empty()),
        UnityWriterStateKind::RunningSuspected => {
            assert!(
                !writer.evidence.is_empty(),
                "an unrelated or unreadable Unity process must retain conservative evidence"
            );
            let error = client
                .unity_launch(UnityLaunchParams {
                    project_id,
                    expected_project_revision: project.project.revision.expect("project revision"),
                    idempotency_key: "m5-unity-launch-suspected".to_owned(),
                })
                .await
                .expect_err("suspected external Unity must conservatively block launch");
            assert!(matches!(
                error,
                alcomd_client::ClientError::Remote(ref remote)
                    if remote.code == "unity_launch_state_uncertain"
            ));
            shutdown.store(true, Ordering::Release);
            let result = tokio::time::timeout(Duration::from_secs(3), daemon)
                .await
                .expect("daemon stop timeout")
                .expect("join daemon");
            assert!(result.is_ok());
            return;
        }
        other => panic!("unexpected writer state before fixture launch: {other:?}"),
    }
    let launch = client
        .unity_launch(UnityLaunchParams {
            project_id,
            expected_project_revision: project.project.revision.expect("project revision"),
            idempotency_key: "m5-unity-launch".to_owned(),
        })
        .await
        .expect("accept Unity launch");
    assert!(launch.launch.spawn_accepted);
    let second_editor = create_fake_editor(fixture.path(), "EditorB");
    client
        .unity_installation_register(UnityInstallationRegisterParams {
            executable_path: second_editor.to_string_lossy().into_owned(),
            idempotency_key: "m7-editor-register-second".to_owned(),
        })
        .await
        .expect("register second compatible Editor");
    let replay = client
        .unity_launch(UnityLaunchParams {
            project_id: launch.launch.project_id.clone(),
            expected_project_revision: project.project.revision.expect("project revision"),
            idempotency_key: "m5-unity-launch".to_owned(),
        })
        .await
        .expect("replay Unity launch");
    assert!(replay.replayed);
    assert_eq!(replay.launch.launch_id, launch.launch.launch_id);
    let multiple = client
        .unity_launch(UnityLaunchParams {
            project_id: launch.launch.project_id.clone(),
            expected_project_revision: project.project.revision.expect("project revision"),
            idempotency_key: "m7-unity-launch-multiple".to_owned(),
        })
        .await
        .expect_err("new automatic launch key sees multiple candidates");
    assert!(matches!(
        multiple,
        alcomd_client::ClientError::Remote(ref remote)
            if remote.code == "unity_editor_selection_required"
    ));

    shutdown.store(true, Ordering::Release);
    let result = tokio::time::timeout(Duration::from_secs(3), daemon)
        .await
        .expect("daemon stop timeout")
        .expect("join daemon");
    assert!(result.is_ok());
}

fn create_fake_editor(root: &Path, directory: &str) -> PathBuf {
    #[cfg(windows)]
    let executable = root.join(directory).join("Unity.exe");
    #[cfg(not(windows))]
    let executable = root.join(directory).join("Unity");
    let manifest = root
        .join(directory)
        .join("Data/Resources/PackageManager/Editor/manifest.json");
    fs::create_dir_all(manifest.parent().expect("manifest parent")).expect("create Editor layout");
    fs::copy(
        std::env::current_exe().expect("current test executable"),
        &executable,
    )
    .expect("copy executable fixture");
    fs::write(manifest, r#"{"version":"2022.3.40f1"}"#).expect("write Editor manifest");
    executable
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
        r#"{"dependencies":{},"locked":{}}"#,
    )
    .expect("write vpm manifest");
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
        let path = base.join(format!("alcomd-m5-rpc-{}", uuid::Uuid::new_v4()));
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
