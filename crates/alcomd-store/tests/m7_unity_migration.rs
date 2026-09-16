use std::fs;
use std::path::PathBuf;

use alcomd_application::{
    IdempotencyKey, M3RegistryStore, M5UnityStore, M7UnityMigrationErrorCode,
    M7UnityMigrationStore, ManifestState, PrincipalId, ProjectObservation, ProjectType, StateStore,
    UnityArchitecture, UnityInstallationObservation, UnityMigrationClassificationKind,
    UnityMigrationEvidence, UnityMigrationPhase, UnityMigrationPlanDraft,
    UnityMigrationPlanOutcome, UnitySourceKind, UnityWriterState, UnityWriterStateKind,
};
use alcomd_store::StateStoreHandle;
use rusqlite::params;
use uuid::Uuid;

#[tokio::test]
async fn unity_migration_plan_reads_only_its_immutable_snapshots() {
    let directory = TestDirectory::new();
    let database_path = directory.0.join("state.db");
    let store = StateStoreHandle::open(database_path.clone()).expect("open store");
    let (owner, draft) = registered_draft(&store).await;

    let planned = store
        .create_unity_migration_plan(owner.clone(), draft.clone())
        .await
        .expect("create Unity migration plan");
    let UnityMigrationPlanOutcome::Planned { plan, replayed } = planned else {
        panic!("expected durable migration plan");
    };
    assert!(!replayed);

    let mut changed_project = draft.project.observation.clone();
    changed_project.root_path = "C:/mutated/project".to_owned();
    changed_project.unity_version = "2023.2.20f1".to_owned();
    let connection = rusqlite::Connection::open(database_path).expect("open inspection connection");
    connection
        .execute(
            "UPDATE projects SET root_path=?1,unity_version=?2,snapshot_json=?3,revision=revision+1
             WHERE project_id=?4",
            params![
                changed_project.root_path,
                changed_project.unity_version,
                serde_json::to_string(&changed_project).expect("serialize changed Project"),
                draft.project.project_id.to_string()
            ],
        )
        .expect("mutate current Project registry authority");
    connection
        .execute(
            "UPDATE unity_installations
             SET executable_path='C:/mutated/Unity.exe',unity_version='2023.2.20f1',revision=revision+1
             WHERE installation_id=?1",
            [draft.target_installation.installation_id.to_string()],
        )
        .expect("mutate current Unity installation authority");

    let reloaded = store
        .get_unity_migration_plan(owner, draft.plan_id)
        .await
        .expect("reload immutable migration plan");
    assert_eq!(reloaded, *plan);
    assert_eq!(reloaded.draft, draft);
}

#[tokio::test]
async fn unity_migration_succeeds_only_after_cleanup_and_commits_one_project_event() {
    let directory = TestDirectory::new();
    let store = StateStoreHandle::open(directory.0.join("state.db")).expect("open store");
    let (owner, draft) = registered_draft(&store).await;
    store
        .create_unity_migration_plan(owner.clone(), draft.clone())
        .await
        .expect("create Unity migration plan");
    let accepted = store
        .accept_unity_migration(
            owner.clone(),
            draft.plan_id,
            key("unity-migration-apply"),
            20,
        )
        .await
        .expect("accept Unity migration");
    store
        .begin_unity_migration(accepted.operation_id, 21)
        .await
        .expect("begin Unity migration");

    let reobserved = UnityMigrationEvidence {
        preparation_kind: "vrchat-2019-to-2022-v1".to_owned(),
        preparation_complete: true,
        spawn_accepted: true,
        exit_observation: Some("exited_successfully".to_owned()),
        reobserved_version: Some(draft.target_unity_version.clone()),
        reobserved_root_identity: Some(draft.project_root_identity.clone()),
        reobserved_marker_sha256: Some([9; 32]),
        writer_inactive_checked_at_ms: Some(30),
        reobserved_at_ms: Some(30),
        safe_evidence: vec!["target_project_reobserved".to_owned()],
        ..UnityMigrationEvidence::default()
    };
    store
        .record_unity_migration_checkpoint(
            accepted.operation_id,
            UnityMigrationPhase::ProjectReobserved,
            reobserved.clone(),
            30,
        )
        .await
        .expect("persist reobservation evidence");
    let mut target_observation = draft.project.observation.clone();
    target_observation.unity_version = draft.target_unity_version.clone();
    target_observation.unity_revision = draft.target_revision_metadata.clone();
    target_observation.observed_at_ms = 30;
    store
        .commit_unity_migration_project(accepted.operation_id, target_observation, 31)
        .await
        .expect("commit final Project revision");

    let premature = store
        .finish_unity_migration_success(accepted.operation_id, 32)
        .await
        .expect_err("state_committed is not yet a successful terminal state");
    assert_eq!(
        premature.code(),
        M7UnityMigrationErrorCode::RecoveryRequired
    );

    store
        .record_unity_migration_checkpoint(
            accepted.operation_id,
            UnityMigrationPhase::CleanupComplete,
            reobserved,
            33,
        )
        .await
        .expect("persist cleanup completion");
    store
        .finish_unity_migration_success(accepted.operation_id, 34)
        .await
        .expect("finish successful migration");

    let events = store.list_events(owner, 0, 100).await.expect("list events");
    let project_events = events
        .events
        .iter()
        .filter(|event| event.kind == "project.unity_version_migrated")
        .collect::<Vec<_>>();
    assert_eq!(project_events.len(), 1);
    assert_eq!(project_events[0].aggregate_revision.get(), 2);
    assert!(
        events
            .events
            .iter()
            .all(|event| !event.kind.starts_with("package.")),
        "private migration preparation must not publish package Events"
    );
    assert_eq!(
        events
            .events
            .iter()
            .filter(|event| event.kind == "operation.succeeded")
            .count(),
        1
    );
}

async fn registered_draft(store: &StateStoreHandle) -> (PrincipalId, UnityMigrationPlanDraft) {
    let owner = PrincipalId::local_owner();
    let project = store
        .register_project(owner.clone(), project(), key("migration-project"), 1)
        .await
        .expect("register Project")
        .value;
    let (target_installation, _) = store
        .register_installation(
            owner.clone(),
            target_installation(),
            key("migration-installation"),
            2,
        )
        .await
        .expect("register target Unity installation");
    let writer_evidence = UnityWriterState {
        project_id: project.project_id,
        state: UnityWriterStateKind::NotObserved,
        evidence: Vec::new(),
        checked_at_ms: 3,
    };
    let draft = UnityMigrationPlanDraft {
        plan_id: alcomd_application::PlanId::new(),
        source_unity_version: project.observation.unity_version.clone(),
        source_revision_metadata: project.observation.unity_revision.clone(),
        project_root_identity: project.observation.path_identity_key.clone(),
        project_version_marker_sha256: [7; 32],
        target_unity_version: target_installation.observation.unity_version.clone(),
        target_revision_metadata: Some("abcdef012345".to_owned()),
        target_installation,
        writer_evidence,
        classification: UnityMigrationClassificationKind::MajorUpgrade,
        preparation_profile: Some("vrchat-2019-to-2022-v1".to_owned()),
        plan_fingerprint: "sealed-plan-fingerprint".to_owned(),
        request_fingerprint: r#"{"version":1,"request":"sealed"}"#.to_owned(),
        plan_idempotency_key: key("unity-migration-plan"),
        created_at_ms: 10,
        expires_at_ms: 910_000,
        project,
    };
    (owner, draft)
}

fn key(value: &str) -> IdempotencyKey {
    IdempotencyKey::parse(value).expect("valid key")
}

fn project() -> ProjectObservation {
    ProjectObservation {
        root_path: "C:/fixture/project".to_owned(),
        path_identity_key: vec![1; 24],
        project_type: ProjectType::Avatars,
        unity_version: "2019.4.31f1".to_owned(),
        unity_revision: Some("bd5abf232a62".to_owned()),
        vpm_manifest: ManifestState::Valid,
        upm_manifest: ManifestState::Valid,
        direct_dependencies: Vec::new(),
        locked_dependencies: Vec::new(),
        issues: Vec::new(),
        observed_at_ms: 1,
    }
}

fn target_installation() -> UnityInstallationObservation {
    UnityInstallationObservation {
        executable_path: "C:/fixture/Unity.exe".to_owned(),
        filesystem_identity: vec![2; 24],
        unity_version: "2022.3.22f1".to_owned(),
        architecture: UnityArchitecture::Unknown,
        source_kind: UnitySourceKind::Manual,
        observed_at_ms: 2,
    }
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("alcomd-m7-unity-store-{}", Uuid::new_v4()));
        fs::create_dir(&path).expect("create test directory");
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
