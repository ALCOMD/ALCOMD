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
async fn unity_registry_preference_and_launch_are_revisioned_evented_and_idempotent() {
    let directory = TestDirectory::new();
    let store = StateStoreHandle::open(directory.0.join("state.db")).expect("open store");
    let owner = PrincipalId::local_owner();
    let project = store
        .register_project(owner.clone(), project(), key("project"), 1)
        .await
        .expect("register project")
        .value;
    let (installation_record, replayed) = store
        .register_installation(owner.clone(), installation(), key("installation"), 2)
        .await
        .expect("register installation");
    assert!(!replayed);
    let replay = store
        .register_installation(owner.clone(), installation(), key("installation"), 3)
        .await
        .expect("replay installation");
    assert!(replay.1);
    assert_eq!(
        replay.0.installation_id,
        installation_record.installation_id
    );

    let (preference, replayed) = store
        .set_project_editor(
            owner.clone(),
            project.project_id,
            installation_record.installation_id,
            vec!["-logFile".to_owned(), "-".to_owned()],
            None,
            key("preference"),
            4,
        )
        .await
        .expect("set preference");
    assert!(!replayed);
    assert_eq!(preference.revision, Revision::INITIAL);
    let conflict = store
        .set_project_editor(
            owner.clone(),
            project.project_id,
            installation_record.installation_id,
            Vec::new(),
            Some(Revision::INITIAL),
            key("preference"),
            5,
        )
        .await
        .expect_err("idempotency fingerprint includes arguments");
    assert_eq!(conflict.code(), M5UnityErrorCode::IdempotencyConflict);

    let (launch, replayed) = store
        .accept_launch(owner.clone(), project, preference, key("launch"), 6)
        .await
        .expect("accept launch");
    assert!(!replayed);
    let replay = store
        .get_launch(owner.clone(), launch.launch_id)
        .await
        .expect("read launch");
    assert_eq!(replay, launch);

    let remove = store
        .remove_installation(
            owner.clone(),
            installation_record.installation_id,
            installation_record.revision,
            key("remove-in-use"),
            7,
        )
        .await
        .expect_err("preference protects installation");
    assert_eq!(remove.code(), M5UnityErrorCode::InstallationInUse);
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
            .any(|event| event.kind == "unity.project-editor.updated")
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

fn installation() -> UnityInstallationObservation {
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
