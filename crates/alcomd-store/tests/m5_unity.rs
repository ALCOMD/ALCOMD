use std::fs;
use std::path::PathBuf;

use alcomd_application::{
    IdempotencyKey, M3RegistryStore, M5UnityErrorCode, M5UnityStore, ManifestState, PrincipalId,
    ProjectObservation, ProjectType, Revision, StateStore, UnityArchitecture,
    UnityInstallationObservation, UnitySourceKind,
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
        observed_at_ms: 1,
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
