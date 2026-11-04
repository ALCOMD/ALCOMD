use std::fs;
use std::path::PathBuf;

use alcomd_application::{
    IdempotencyKey, M3RegistryStore, M5UnityErrorCode, M5UnityStore, ManifestState, PrincipalId,
    ProjectObservation, ProjectType, Revision, StateStore, UnityArchitecture,
    UnityInstallationObservation, UnityLaunchState, UnitySourceKind,
};
use alcomd_store::StateStoreHandle;
use uuid::Uuid;

#[tokio::test]
async fn unity_registry_launch_config_and_launch_are_revisioned_evented_and_idempotent() {
    let directory = TestDirectory::new();
    let store = StateStoreHandle::open(directory.0.join("state.db")).expect("open store");
    let owner = PrincipalId::local_owner();
    let project = store
        .register_project(owner.clone(), project(), key("project"), 1)
        .await
        .expect("register project")
        .value;
    let (installation_record, replayed) = store
        .register_installation(
            owner.clone(),
            installation_observation(),
            key("installation"),
            2,
        )
        .await
        .expect("register installation");
    assert!(!replayed);
    let replay = store
        .register_installation(
            owner.clone(),
            installation_observation(),
            key("installation"),
            3,
        )
        .await
        .expect("replay installation");
    assert!(replay.1);
    assert_eq!(
        replay.0.installation_id,
        installation_record.installation_id
    );

    let arguments = vec!["-logFile".to_owned(), "Project.log".to_owned()];
    let (config, changed, replayed) = store
        .set_project_launch_config(
            owner.clone(),
            project.project_id,
            arguments.clone(),
            None,
            key("launch-config"),
            4,
        )
        .await
        .expect("set launch config");
    assert!(changed);
    assert!(!replayed);
    assert_eq!(config.revision, Some(Revision::INITIAL));
    assert_eq!(config.arguments, arguments);
    let conflict = store
        .set_project_launch_config(
            owner.clone(),
            project.project_id,
            Vec::new(),
            Some(Revision::INITIAL),
            key("launch-config"),
            5,
        )
        .await
        .expect_err("idempotency fingerprint includes arguments");
    assert_eq!(conflict.code(), M5UnityErrorCode::IdempotencyConflict);

    let (launch, replayed) = store
        .accept_launch(
            owner.clone(),
            project.clone(),
            config.clone(),
            installation_record.installation_id,
            key("launch"),
            6,
        )
        .await
        .expect("accept launch");
    assert!(!replayed);
    let idempotent_launch = store
        .replay_launch(
            owner.clone(),
            project,
            config,
            installation_record.installation_id,
            key("launch"),
        )
        .await
        .expect("replay one-shot launch")
        .expect("stored launch");
    assert_eq!(idempotent_launch, launch);
    let replay = store
        .get_launch(owner.clone(), launch.launch_id)
        .await
        .expect("read launch");
    assert_eq!(replay, launch);

    let events = store.list_events(owner, 0, 100).await.expect("events");
    assert!(
        events
            .events
            .iter()
            .any(|event| event.kind == "unity.installation.registered")
    );
    assert!(
        events
            .events
            .iter()
            .any(|event| event.kind == "unity.project_launch_config_changed")
    );
}

#[tokio::test]
async fn project_modified_time_tracks_first_launch_acceptance_without_changing_content_revision() {
    let directory = TestDirectory::new();
    let store = StateStoreHandle::open(directory.0.join("state.db")).expect("open store");
    let owner = PrincipalId::local_owner();
    let registered = store
        .register_project(owner.clone(), project(), key("modified-project"), 100)
        .await
        .expect("register project")
        .value;
    let (installation, _) = store
        .register_installation(
            owner.clone(),
            installation_observation(),
            key("modified-editor"),
            110,
        )
        .await
        .expect("register installation");
    let (configured, _, _) = store
        .set_project_launch_config(
            owner.clone(),
            registered.project_id,
            vec!["-logFile".to_owned()],
            None,
            key("modified-config"),
            200,
        )
        .await
        .expect("configure launch arguments");
    let (cleared, _, _) = store
        .clear_project_launch_config(
            owner.clone(),
            registered.project_id,
            configured.revision,
            key("modified-config-clear"),
            230,
        )
        .await
        .expect("clear launch arguments");
    assert_eq!(
        store
            .get_project(owner.clone(), registered.project_id)
            .await
            .expect("read configured project"),
        registered,
        "launch settings do not modify the project time or snapshot"
    );

    let (launch, replayed) = store
        .accept_launch(
            owner.clone(),
            registered.clone(),
            cleared.clone(),
            installation.installation_id,
            key("modified-launch"),
            300,
        )
        .await
        .expect("accept launch before any platform spawn");
    assert!(!replayed);
    assert!(!launch.spawn_accepted);
    let accepted = store
        .get_project(owner.clone(), registered.project_id)
        .await
        .expect("read accepted launch time");
    assert_eq!(accepted.last_modified_at_ms, Some(300));
    assert_eq!(accepted.revision, registered.revision);
    assert_eq!(accepted.observation, registered.observation);
    assert_eq!(accepted.registered_at_ms, 100);
    store
        .set_launch_state(
            owner.clone(),
            launch.launch_id,
            UnityLaunchState::Failed,
            false,
        )
        .await
        .expect("record subsequent spawn failure");
    let (replayed_launch, replayed) = store
        .accept_launch(
            owner.clone(),
            accepted.clone(),
            cleared.clone(),
            installation.installation_id,
            key("modified-launch"),
            1_000,
        )
        .await
        .expect("retry the same durable launch after failure");
    assert!(replayed);
    assert_eq!(replayed_launch.launch_id, launch.launch_id);
    assert_eq!(replayed_launch.state, UnityLaunchState::Failed);
    assert_eq!(
        store
            .get_project(owner.clone(), registered.project_id)
            .await
            .expect("read after replay"),
        accepted
    );
    store
        .get_launch(owner.clone(), launch.launch_id)
        .await
        .expect("poll failed launch");
    assert_eq!(
        store
            .get_project(owner.clone(), registered.project_id)
            .await
            .expect("read after poll"),
        accepted
    );

    store
        .accept_launch(
            owner.clone(),
            accepted.clone(),
            cleared.clone(),
            installation.installation_id,
            key("modified-backward-clock"),
            250,
        )
        .await
        .expect("new launch with a clock earlier than the recorded project time");
    assert_eq!(
        store
            .get_project(owner.clone(), registered.project_id)
            .await
            .expect("read monotonic project time"),
        accepted
    );
    store
        .accept_launch(
            owner.clone(),
            accepted,
            cleared.clone(),
            installation.installation_id,
            key("modified-forward-clock"),
            500,
        )
        .await
        .expect("accept a later distinct launch");
    let later = store
        .get_project(owner.clone(), registered.project_id)
        .await
        .expect("read later launch time");
    assert_eq!(later.last_modified_at_ms, Some(500));
    assert_eq!(later.revision, registered.revision);
    let failed = store
        .accept_launch(
            owner.clone(),
            later.clone(),
            cleared.clone(),
            installation.installation_id,
            key("modified-invalid-clock"),
            u64::MAX,
        )
        .await
        .expect_err("out-of-range time must not partially accept the launch");
    assert_eq!(failed.code(), M5UnityErrorCode::Internal);
    assert_eq!(
        store
            .get_project(owner.clone(), registered.project_id)
            .await
            .expect("read after failed transaction"),
        later
    );
    let (_, replayed) = store
        .accept_launch(
            owner.clone(),
            later,
            cleared,
            installation.installation_id,
            key("modified-invalid-clock"),
            600,
        )
        .await
        .expect("failed transaction did not consume the durable key");
    assert!(!replayed);
    assert_eq!(
        store
            .get_project(owner, registered.project_id)
            .await
            .expect("read recovered request time")
            .last_modified_at_ms,
        Some(600)
    );
}

#[tokio::test]
async fn missing_set_same_value_clear_and_one_shot_installation_are_explicit() {
    let directory = TestDirectory::new();
    let store = StateStoreHandle::open(directory.0.join("state.db")).expect("open store");
    let owner = PrincipalId::local_owner();
    let project = store
        .register_project(owner.clone(), project(), key("selection-project"), 1)
        .await
        .expect("register project")
        .value;
    let (first, _) = store
        .register_installation(
            owner.clone(),
            installation_observation(),
            key("selection-installation"),
            2,
        )
        .await
        .expect("register installation");

    let missing = store
        .get_project_launch_config(owner.clone(), project.project_id)
        .await
        .expect("missing launch config sentinel");
    assert!(missing.arguments.is_empty());
    assert!(missing.revision.is_none());
    assert_eq!(missing.updated_at_ms, 0);

    let (missing_clear, changed, replayed) = store
        .clear_project_launch_config(
            owner.clone(),
            project.project_id,
            None,
            key("selection-clear-missing"),
            3,
        )
        .await
        .expect("clear missing config");
    assert!(!changed);
    assert!(!replayed);
    assert!(missing_clear.revision.is_none());
    let (_, changed, replayed) = store
        .clear_project_launch_config(
            owner.clone(),
            project.project_id,
            None,
            key("selection-clear-missing"),
            4,
        )
        .await
        .expect("replay missing clear");
    assert!(!changed);
    assert!(replayed);

    let arguments = vec!["-logFile".to_owned(), "-".to_owned()];
    let (configured, changed, replayed) = store
        .set_project_launch_config(
            owner.clone(),
            project.project_id,
            arguments.clone(),
            None,
            key("launch-config-set"),
            5,
        )
        .await
        .expect("set launch config");
    assert!(changed);
    assert!(!replayed);
    assert_eq!(configured.arguments, arguments);
    assert_eq!(configured.revision, Some(Revision::INITIAL));

    let (same, changed, replayed) = store
        .set_project_launch_config(
            owner.clone(),
            project.project_id,
            arguments.clone(),
            Some(Revision::INITIAL),
            key("launch-config-same"),
            6,
        )
        .await
        .expect("same-value set");
    assert!(!changed);
    assert!(!replayed);
    assert_eq!(same, configured);

    let (first_launch, replayed) = store
        .accept_launch(
            owner.clone(),
            project.clone(),
            configured.clone(),
            first.installation_id,
            key("one-shot-launch"),
            7,
        )
        .await
        .expect("accept explicit one-shot launch authority");
    assert!(!replayed);
    let mut second_observation = installation_observation();
    second_observation.executable_path = "C:/fixture/UnityB.exe".to_owned();
    second_observation.filesystem_identity = vec![3; 24];
    let (second, _) = store
        .register_installation(
            owner.clone(),
            second_observation,
            key("second-installation"),
            8,
        )
        .await
        .expect("register unrelated second installation");
    let replay_after_registry_change = store
        .replay_launch(
            owner.clone(),
            project.clone(),
            configured.clone(),
            first.installation_id,
            key("one-shot-launch"),
        )
        .await
        .expect("replay launch after registry change")
        .expect("stored launch");
    assert_eq!(replay_after_registry_change, first_launch);
    let idempotency_conflict = store
        .replay_launch(
            owner.clone(),
            project.clone(),
            configured.clone(),
            second.installation_id,
            key("one-shot-launch"),
        )
        .await
        .expect_err("same key cannot select another installation");
    assert_eq!(
        idempotency_conflict.code(),
        M5UnityErrorCode::IdempotencyConflict
    );

    let stale = store
        .clear_project_launch_config(
            owner.clone(),
            project.project_id,
            None,
            key("launch-config-clear-stale"),
            9,
        )
        .await
        .expect_err("stored config rejects revision-zero clear");
    assert_eq!(stale.code(), M5UnityErrorCode::RevisionConflict);

    let (cleared, changed, replayed) = store
        .clear_project_launch_config(
            owner.clone(),
            project.project_id,
            Some(Revision::INITIAL),
            key("launch-config-clear"),
            10,
        )
        .await
        .expect("clear launch config");
    assert!(changed);
    assert!(!replayed);
    assert!(cleared.arguments.is_empty());
    assert_eq!(cleared.revision.expect("clear revision").get(), 2);

    let removed = store
        .remove_installation(
            owner.clone(),
            first.installation_id,
            first.revision,
            key("remove-after-one-shot-launch"),
            11,
        )
        .await
        .expect("one-shot selection is not a persisted preference");
    assert!(removed.0);
    assert!(!removed.1);
    let events = store.list_events(owner, 0, 100).await.expect("events");
    assert_eq!(
        events
            .events
            .iter()
            .filter(|event| event.kind == "unity.project_launch_config_changed")
            .count(),
        2
    );
}

fn key(value: &str) -> IdempotencyKey {
    IdempotencyKey::parse(value).expect("valid key")
}

#[tokio::test]
async fn default_arguments_are_inherited_overridden_and_bound_to_launch_retries() {
    use alcomd_application::{AccessContext, M5UnityApplication, Permission};
    use std::sync::{Arc, Mutex};

    let directory = TestDirectory::new();
    let store = StateStoreHandle::open(directory.0.join("defaults.db")).expect("store");
    let owner = PrincipalId::local_owner();
    let access = AccessContext::new(
        owner.clone(),
        [
            Permission::ProjectsRead,
            Permission::UnityRead,
            Permission::UnityManage,
            Permission::UnityLaunch,
        ],
    );
    let project = store
        .register_project(owner.clone(), project(), key("defaults-project"), 1)
        .await
        .expect("project")
        .value;
    let mut editor = installation_observation();
    editor.unity_version = project.observation.unity_version.clone();
    let (editor, _) = store
        .register_installation(owner.clone(), editor, key("defaults-editor"), 2)
        .await
        .expect("editor");
    let literal = vec![
        "-logFile".to_owned(),
        r#" C:\logs with spaces\"quoted"\ "#.to_owned(),
        String::new(),
        "尾部空格 ".to_owned(),
        "x".repeat(4_096),
    ];
    let defaults = Arc::new(TestLaunchDefaults(Mutex::new(Some(literal.clone()))));
    let platform = CapturingUnityPlatform::default();
    let app = M5UnityApplication::new(store.clone(), platform.clone(), defaults.clone());
    let view = app
        .get_project_launch_config(&access, project.project_id)
        .await
        .expect("inherited view");
    assert!(view.config.revision.is_none());
    assert!(view.config.arguments.is_empty());
    assert_eq!(view.default_arguments, literal);
    assert_eq!(view.effective_arguments, literal);
    let (launch, replayed) = app
        .launch(
            &access,
            project.project_id,
            editor.installation_id,
            project.revision,
            key("defaults-launch"),
        )
        .await
        .expect("launch inherited argv");
    assert!(!replayed);
    assert_eq!(
        *platform.launches.lock().expect("captures"),
        vec![literal.clone()]
    );
    let (replay, replayed) = app
        .launch(
            &access,
            project.project_id,
            editor.installation_id,
            project.revision,
            key("defaults-launch"),
        )
        .await
        .expect("same effective argv replay");
    assert!(replayed);
    assert_eq!(launch.launch_id, replay.launch_id);
    *defaults.0.lock().expect("defaults") = Some(vec!["-batchmode".to_owned()]);
    assert_eq!(
        app.launch(
            &access,
            project.project_id,
            editor.installation_id,
            project.revision,
            key("defaults-launch")
        )
        .await
        .expect_err("changed defaults cannot respawn same key")
        .code(),
        M5UnityErrorCode::IdempotencyConflict
    );
    assert_eq!(platform.launches.lock().expect("captures").len(), 1);

    let (empty, changed, _) = app
        .set_project_launch_config(
            &access,
            project.project_id,
            Vec::new(),
            None,
            key("defaults-empty"),
        )
        .await
        .expect("explicit empty override");
    assert!(changed);
    assert_eq!(empty.revision, Some(Revision::INITIAL));
    let view = app
        .get_project_launch_config(&access, project.project_id)
        .await
        .expect("override view");
    assert_eq!(view.default_arguments, ["-batchmode"]);
    assert!(view.effective_arguments.is_empty());
    *defaults.0.lock().expect("defaults") = None;
    app.launch(
        &access,
        project.project_id,
        editor.installation_id,
        project.revision,
        key("defaults-empty-launch"),
    )
    .await
    .expect("explicit override does not need global settings");
    assert_eq!(
        platform.launches.lock().expect("captures").last(),
        Some(&Vec::<String>::new())
    );
    app.clear_project_launch_config(
        &access,
        project.project_id,
        empty.revision,
        key("defaults-clear"),
    )
    .await
    .expect("clear override");
    assert_eq!(
        app.launch(
            &access,
            project.project_id,
            editor.installation_id,
            project.revision,
            key("defaults-unavailable")
        )
        .await
        .expect_err("unavailable inherited defaults fail closed")
        .code(),
        M5UnityErrorCode::Internal
    );
    *defaults.0.lock().expect("defaults") = Some(literal.clone());
    let view = app
        .get_project_launch_config(&access, project.project_id)
        .await
        .expect("inherit after clear");
    assert!(view.config.revision.is_none());
    assert_eq!(view.effective_arguments, literal);
    let denied = AccessContext::new(owner, [Permission::ProjectsRead]);
    assert_eq!(
        app.get_project_launch_config(&denied, project.project_id)
            .await
            .expect_err("UnityRead required")
            .code(),
        M5UnityErrorCode::PermissionDenied
    );
}

#[tokio::test]
async fn launch_blocks_only_a_writer_linked_to_the_requested_project() {
    use alcomd_application::{
        AccessContext, M5UnityApplication, Permission, UnityProcessObservation,
        UnityWriterStateKind,
    };
    use std::sync::{Arc, Mutex};

    let candidate = |identity, arguments| UnityProcessObservation {
        pid: 7,
        start_time: 8,
        name: "Unity.exe".to_owned(),
        executable_identity: identity,
        verified_non_editor: false,
        arguments,
    };
    let argv = |path: &str| {
        Some(vec![
            "Unity.exe".to_owned(),
            "-projectPath".to_owned(),
            path.to_owned(),
        ])
    };
    let cases = [
        (
            "other-registered",
            Some(candidate(Some(vec![2; 24]), argv("other-project"))),
            false,
            UnityWriterStateKind::NotObserved,
            None,
        ),
        (
            "other-unregistered",
            Some(candidate(Some(vec![3; 24]), argv("other-project"))),
            false,
            UnityWriterStateKind::NotObserved,
            None,
        ),
        (
            "unattributed",
            Some(candidate(None, None)),
            false,
            UnityWriterStateKind::Unknown,
            None,
        ),
        (
            "inspection-failed",
            None,
            true,
            UnityWriterStateKind::Unknown,
            None,
        ),
        (
            "unreadable-path",
            Some(candidate(Some(vec![2; 24]), argv("unreadable-project"))),
            false,
            UnityWriterStateKind::Unknown,
            None,
        ),
        (
            "same-registered",
            Some(candidate(Some(vec![2; 24]), argv("fixture-project"))),
            false,
            UnityWriterStateKind::RunningConfirmed,
            Some(M5UnityErrorCode::ProjectRunning),
        ),
        (
            "same-unregistered",
            Some(candidate(Some(vec![3; 24]), argv("fixture-project"))),
            false,
            UnityWriterStateKind::RunningSuspected,
            Some(M5UnityErrorCode::LaunchStateUncertain),
        ),
    ];
    for (name, process, inspection_failed, expected_state, expected_error) in cases {
        let directory = TestDirectory::new();
        let store = StateStoreHandle::open(directory.0.join("writer.db")).expect("store");
        let owner = PrincipalId::local_owner();
        let access = AccessContext::new(
            owner.clone(),
            [
                Permission::ProjectsRead,
                Permission::UnityRead,
                Permission::UnityLaunch,
            ],
        );
        let project = store
            .register_project(owner.clone(), project(), key("writer-project"), 1)
            .await
            .expect("project")
            .value;
        let mut editor = installation_observation();
        editor.unity_version = project.observation.unity_version.clone();
        let (editor, _) = store
            .register_installation(owner, editor, key("writer-editor"), 2)
            .await
            .expect("editor");
        let platform = CapturingUnityPlatform::default();
        *platform.processes.lock().expect("process fixture") = process.into_iter().collect();
        platform
            .inspection_fails
            .store(inspection_failed, std::sync::atomic::Ordering::Relaxed);
        let app = M5UnityApplication::new(
            store,
            platform.clone(),
            Arc::new(TestLaunchDefaults(Mutex::new(Some(Vec::new())))),
        );
        assert_eq!(
            app.writer_state(&access, project.project_id)
                .await
                .expect(name)
                .state,
            expected_state,
            "{name}"
        );
        let result = app
            .launch(
                &access,
                project.project_id,
                editor.installation_id,
                project.revision,
                key("writer-launch"),
            )
            .await;
        if let Some(code) = expected_error {
            assert_eq!(result.expect_err(name).code(), code, "{name}");
            assert!(
                platform.launches.lock().expect("captures").is_empty(),
                "{name}: no duplicate spawn"
            );
        } else {
            let (launch, replayed) = result.expect(name);
            assert!(!replayed);
            assert!(launch.spawn_accepted, "{name}");
            assert_eq!(
                platform.launches.lock().expect("captures").len(),
                1,
                "{name}"
            );
        }
    }
}

struct TestLaunchDefaults(std::sync::Mutex<Option<Vec<String>>>);

impl alcomd_application::UnityLaunchDefaultsSource for TestLaunchDefaults {
    fn arguments(
        &self,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<Vec<String>, alcomd_application::M5UnityError>>
                + Send
                + '_,
        >,
    > {
        Box::pin(async {
            self.0
                .lock()
                .expect("defaults")
                .clone()
                .ok_or_else(|| alcomd_application::M5UnityError::new(M5UnityErrorCode::Internal))
        })
    }
}

#[derive(Clone, Default)]
struct CapturingUnityPlatform {
    launches: std::sync::Arc<std::sync::Mutex<Vec<Vec<String>>>>,
    processes: std::sync::Arc<std::sync::Mutex<Vec<alcomd_application::UnityProcessObservation>>>,
    inspection_fails: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl alcomd_application::M5UnityPlatform for CapturingUnityPlatform {
    async fn validate_installation(
        &self,
        _: String,
        _: UnitySourceKind,
    ) -> Result<UnityInstallationObservation, alcomd_application::M5UnityError> {
        unreachable!("not used")
    }
    async fn discover_installations(
        &self,
    ) -> Result<Vec<UnityInstallationObservation>, alcomd_application::M5UnityError> {
        Ok(Vec::new())
    }
    async fn observe_processes(
        &self,
    ) -> Result<Vec<alcomd_application::UnityProcessObservation>, alcomd_application::M5UnityError>
    {
        if self
            .inspection_fails
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            Err(alcomd_application::M5UnityError::new(
                M5UnityErrorCode::Internal,
            ))
        } else {
            Ok(self.processes.lock().expect("process fixture").clone())
        }
    }
    async fn path_identity(
        &self,
        path: String,
    ) -> Result<Vec<u8>, alcomd_application::M5UnityError> {
        match path.as_str() {
            "other-project" => Ok(vec![9; 24]),
            "unreadable-project" => Err(alcomd_application::M5UnityError::new(
                M5UnityErrorCode::Internal,
            )),
            _ => Ok(vec![1; 24]),
        }
    }
    async fn launch(
        &self,
        _: String,
        _: String,
        arguments: Vec<String>,
    ) -> Result<(), alcomd_application::M5UnityError> {
        self.launches.lock().expect("captures").push(arguments);
        Ok(())
    }
}

fn project() -> ProjectObservation {
    ProjectObservation {
        root_path: "C:/fixture/project".to_owned(),
        path_identity_key: vec![1; 24],
        project_type: ProjectType::Unknown,
        unity_version: "2022.3.22f1".to_owned(),
        unity_revision: None,
        vpm_manifest: ManifestState::Valid,
        upm_manifest: ManifestState::Missing,
        direct_dependencies: Vec::new(),
        locked_dependencies: Vec::new(),
        issues: Vec::new(),
    }
}

fn installation_observation() -> UnityInstallationObservation {
    UnityInstallationObservation {
        executable_path: "C:/fixture/Unity.exe".to_owned(),
        filesystem_identity: vec![2; 24],
        unity_version: "2022.3.40f1".to_owned(),
        architecture: UnityArchitecture::Unknown,
        source_kind: UnitySourceKind::Manual,
        observed_at_ms: 2,
    }
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("alcomd-m5-store-{}", Uuid::new_v4()));
        fs::create_dir(&path).expect("create directory");
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
