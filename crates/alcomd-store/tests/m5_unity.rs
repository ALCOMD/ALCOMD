use std::fs;
use std::path::PathBuf;

use alcomd_application::{
    IdempotencyKey, M3RegistryStore, M5UnityErrorCode, M5UnityStore, ManifestState, PrincipalId,
    ProjectEditorSelection, ProjectObservation, ProjectType, Revision, StateStore,
    UnityArchitecture, UnityInstallationObservation, UnitySourceKind,
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

    let selection = store
        .get_project_editor_selection(owner.clone(), project.project_id)
        .await
        .expect("selection");
    let (launch, replayed) = store
        .accept_launch(
            owner.clone(),
            project.clone(),
            selection.clone(),
            preference.installation_id,
            key("launch"),
            6,
        )
        .await
        .expect("accept launch");
    assert!(!replayed);
    let idempotent_launch = store
        .replay_launch(owner.clone(), project, selection, key("launch"))
        .await
        .expect("replay explicit v1 launch")
        .expect("stored explicit launch");
    assert_eq!(idempotent_launch, launch);
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

#[tokio::test]
async fn automatic_selection_clear_preserves_arguments_and_releases_installation() {
    let directory = TestDirectory::new();
    let store = StateStoreHandle::open(directory.0.join("state.db")).expect("open store");
    let owner = PrincipalId::local_owner();
    let project = store
        .register_project(owner.clone(), project(), key("selection-project"), 1)
        .await
        .expect("register project")
        .value;
    let (installation, _) = store
        .register_installation(
            owner.clone(),
            installation_observation(),
            key("selection-installation"),
            2,
        )
        .await
        .expect("register installation");

    let missing = store
        .get_project_editor_selection(owner.clone(), project.project_id)
        .await
        .expect("implicit automatic selection");
    assert_eq!(missing.selection, ProjectEditorSelection::Automatic);
    assert!(missing.arguments.is_empty());
    assert!(missing.revision.is_none());
    assert_eq!(missing.updated_at_ms, 0);
    assert_eq!(
        store
            .get_project_editor(owner.clone(), project.project_id)
            .await
            .expect_err("legacy get has no explicit preference")
            .code(),
        M5UnityErrorCode::InstallationNotFound
    );

    let (missing_clear, replayed) = store
        .clear_project_editor(
            owner.clone(),
            project.project_id,
            None,
            key("selection-clear-missing"),
            3,
        )
        .await
        .expect("clear implicit automatic");
    assert!(!replayed);
    assert!(missing_clear.revision.is_none());
    let (_, replayed) = store
        .clear_project_editor(
            owner.clone(),
            project.project_id,
            None,
            key("selection-clear-missing"),
            4,
        )
        .await
        .expect("replay missing clear");
    assert!(replayed);

    let arguments = vec!["-logFile".to_owned(), "-".to_owned()];
    let (explicit, _) = store
        .set_project_editor(
            owner.clone(),
            project.project_id,
            installation.installation_id,
            arguments.clone(),
            None,
            key("selection-explicit"),
            5,
        )
        .await
        .expect("set explicit preference");
    let in_use = store
        .remove_installation(
            owner.clone(),
            installation.installation_id,
            installation.revision,
            key("selection-remove-in-use"),
            6,
        )
        .await
        .expect_err("explicit preference protects installation");
    assert_eq!(in_use.code(), M5UnityErrorCode::InstallationInUse);

    let (automatic, replayed) = store
        .clear_project_editor(
            owner.clone(),
            project.project_id,
            Some(explicit.revision),
            key("selection-clear"),
            7,
        )
        .await
        .expect("clear explicit preference");
    assert!(!replayed);
    assert_eq!(automatic.selection, ProjectEditorSelection::Automatic);
    assert_eq!(automatic.arguments, arguments);
    assert_eq!(
        automatic.revision.expect("stored automatic revision").get(),
        2
    );
    let (clear_replay, replayed) = store
        .clear_project_editor(
            owner.clone(),
            project.project_id,
            Some(explicit.revision),
            key("selection-clear"),
            8,
        )
        .await
        .expect("replay explicit clear");
    assert!(replayed);
    assert_eq!(clear_replay, automatic);
    let (automatic_launch, replayed) = store
        .accept_launch(
            owner.clone(),
            project.clone(),
            automatic.clone(),
            installation.installation_id,
            key("selection-automatic-launch"),
            8,
        )
        .await
        .expect("accept automatic launch authority");
    assert!(!replayed);
    let mut second_observation = installation_observation();
    second_observation.executable_path = "C:/fixture/UnityB.exe".to_owned();
    second_observation.filesystem_identity = vec![3; 24];
    store
        .register_installation(
            owner.clone(),
            second_observation,
            key("selection-second-installation"),
            8,
        )
        .await
        .expect("register unrelated second installation");
    let replay_after_registry_change = store
        .replay_launch(
            owner.clone(),
            project.clone(),
            automatic.clone(),
            key("selection-automatic-launch"),
        )
        .await
        .expect("replay automatic authority after registry change")
        .expect("stored automatic launch");
    assert_eq!(replay_after_registry_change, automatic_launch);
    assert_eq!(
        store
            .get_project(owner.clone(), project.project_id)
            .await
            .expect("project after preference clear")
            .revision,
        project.revision
    );

    let automatic_revision = automatic.revision.expect("automatic revision");
    let (no_op, replayed) = store
        .clear_project_editor(
            owner.clone(),
            project.project_id,
            Some(automatic_revision),
            key("selection-clear-no-op"),
            9,
        )
        .await
        .expect("stored automatic no-op");
    assert!(!replayed);
    assert_eq!(no_op.revision, Some(automatic_revision));
    let stale_zero = store
        .clear_project_editor(
            owner.clone(),
            project.project_id,
            None,
            key("selection-clear-stale-zero"),
            10,
        )
        .await
        .expect_err("stored automatic rejects revision zero");
    assert_eq!(stale_zero.code(), M5UnityErrorCode::RevisionConflict);

    let idempotency_conflict = store
        .clear_project_editor(
            owner.clone(),
            project.project_id,
            None,
            key("selection-clear-no-op"),
            11,
        )
        .await
        .expect_err("changed clear fingerprint conflicts");
    assert_eq!(
        idempotency_conflict.code(),
        M5UnityErrorCode::IdempotencyConflict
    );

    let (legacy_explicit, replayed) = store
        .set_project_editor(
            owner.clone(),
            project.project_id,
            installation.installation_id,
            arguments.clone(),
            None,
            key("selection-legacy-set-from-automatic"),
            12,
        )
        .await
        .expect("legacy revision zero sets stored automatic");
    assert!(!replayed);
    assert_eq!(legacy_explicit.revision.get(), 3);
    let changed_selection = store
        .get_project_editor_selection(owner.clone(), project.project_id)
        .await
        .expect("read changed selection authority");
    let old_authority = store
        .replay_launch(
            owner.clone(),
            project.clone(),
            changed_selection,
            key("selection-automatic-launch"),
        )
        .await
        .expect_err("changed selection authority conflicts with old key");
    assert_eq!(old_authority.code(), M5UnityErrorCode::IdempotencyConflict);
    let stale_explicit = store
        .set_project_editor(
            owner.clone(),
            project.project_id,
            installation.installation_id,
            arguments.clone(),
            Some(automatic_revision),
            key("selection-stale-explicit"),
            13,
        )
        .await
        .expect_err("explicit stale revision conflicts");
    assert_eq!(stale_explicit.code(), M5UnityErrorCode::RevisionConflict);
    let (automatic_again, _) = store
        .clear_project_editor(
            owner.clone(),
            project.project_id,
            Some(legacy_explicit.revision),
            key("selection-clear-again"),
            14,
        )
        .await
        .expect("clear legacy explicit selection");
    assert_eq!(automatic_again.arguments, arguments);
    assert_eq!(
        automatic_again.revision.expect("automatic revision").get(),
        4
    );
    let (modern_explicit, _) = store
        .set_project_editor(
            owner.clone(),
            project.project_id,
            installation.installation_id,
            arguments.clone(),
            automatic_again.revision,
            key("selection-modern-set-from-automatic"),
            15,
        )
        .await
        .expect("modern exact revision sets stored automatic");
    assert_eq!(modern_explicit.revision.get(), 5);
    let (final_automatic, _) = store
        .clear_project_editor(
            owner.clone(),
            project.project_id,
            Some(modern_explicit.revision),
            key("selection-final-clear"),
            16,
        )
        .await
        .expect("clear modern explicit selection");
    assert_eq!(final_automatic.arguments, arguments);
    assert_eq!(
        final_automatic.revision.expect("automatic revision").get(),
        6
    );

    let removed = store
        .remove_installation(
            owner.clone(),
            installation.installation_id,
            installation.revision,
            key("selection-remove-after-clear"),
            17,
        )
        .await
        .expect("automatic preference releases installation");
    assert!(removed.0);
    assert!(!removed.1);
    let events = store.list_events(owner, 0, 100).await.expect("events");
    assert_eq!(
        events
            .events
            .iter()
            .filter(|event| event.kind == "unity.project-editor.selection_cleared")
            .count(),
        3
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
