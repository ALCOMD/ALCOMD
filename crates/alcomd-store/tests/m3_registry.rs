use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use alcomd_application::{
    DependencyIdentity, IdempotencyKey, M3ErrorCode, M3RegistryStore, ManifestState, PrincipalId,
    ProjectObservation, ProjectType, RepositoryObservation, RepositoryPackageVersion,
    RepositorySource, RepositoryValidators, StateStore,
};
use alcomd_store::StateStoreHandle;
use uuid::Uuid;

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("alcomd-m3-store-test-{}", Uuid::new_v4()));
        fs::create_dir(&path).expect("create test directory");
        Self(path)
    }

    fn database(&self) -> PathBuf {
        self.0.join("state.db")
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        for _ in 0..20 {
            if fs::remove_dir_all(&self.0).is_ok() {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("failed to remove isolated store directory");
    }
}

fn key(value: &str) -> IdempotencyKey {
    IdempotencyKey::parse(value).expect("valid key")
}

fn project(identity: u8, observed_at_ms: u64) -> ProjectObservation {
    ProjectObservation {
        root_path: format!("C:/fixture/{identity}"),
        path_identity_key: vec![identity; 24],
        project_type: ProjectType::Avatars,
        unity_version: "2022.3.22f1".to_owned(),
        unity_revision: None,
        vpm_manifest: ManifestState::Valid,
        upm_manifest: ManifestState::Missing,
        direct_dependencies: vec![DependencyIdentity {
            package_id: "com.vrchat.avatars".to_owned(),
            value: "3.7.0".to_owned(),
        }],
        locked_dependencies: Vec::new(),
        issues: Vec::new(),
        observed_at_ms,
    }
}

fn repository(identity: u8, refreshed_at_ms: u64) -> RepositoryObservation {
    RepositoryObservation {
        source: RepositorySource::Remote {
            url: format!("https://example.invalid/{identity}?channel=stable"),
        },
        source_identity_key: vec![identity],
        declared_id: Some(format!("repo-{identity}")),
        name: Some("Fixture".to_owned()),
        declared_url: None,
        issues: Vec::new(),
        packages: vec![RepositoryPackageVersion {
            package_id: "com.example.package".to_owned(),
            version: "1.0.0".to_owned(),
            display_name: Some("Example".to_owned()),
            description: None,
            yanked: false,
            unity: None,
        }],
        validators: RepositoryValidators {
            etag: Some("\"one\"".to_owned()),
            last_modified: None,
        },
        refreshed_at_ms,
    }
}

#[tokio::test]
async fn project_registry_is_revisioned_evented_and_idempotent() {
    let directory = TestDirectory::new();
    let store = StateStoreHandle::open(directory.database()).expect("open store");
    let owner = PrincipalId::local_owner();
    let first = store
        .register_project(owner.clone(), project(1, 10), key("project-register"), 10)
        .await
        .expect("register project");
    assert_eq!(first.value.revision.get(), 1);
    let replay = store
        .register_project(owner.clone(), project(1, 20), key("project-register"), 20)
        .await
        .expect("replay registration");
    assert!(replay.replayed);
    assert_eq!(replay.value.observation.observed_at_ms, 10);

    let no_op = store
        .refresh_project(
            owner.clone(),
            first.value.project_id,
            first.value.revision,
            project(1, 30),
            key("project-refresh-noop"),
            30,
        )
        .await
        .expect("no-op refresh");
    assert_eq!(no_op.value.revision.get(), 1);
    let mut changed = project(1, 40);
    changed.project_type = ProjectType::Worlds;
    let refreshed = store
        .refresh_project(
            owner.clone(),
            first.value.project_id,
            no_op.value.revision,
            changed,
            key("project-refresh-change"),
            40,
        )
        .await
        .expect("changed refresh");
    assert_eq!(refreshed.value.revision.get(), 2);
    let events = store
        .list_events(owner.clone(), 0, 100)
        .await
        .expect("events");
    assert_eq!(events.events.len(), 2);

    let removed = store
        .unregister_project(
            owner.clone(),
            first.value.project_id,
            refreshed.value.revision,
            key("project-remove"),
            50,
        )
        .await
        .expect("unregister");
    assert_eq!(removed.revision.get(), 3);
    let replay = store
        .unregister_project(
            owner,
            first.value.project_id,
            refreshed.value.revision,
            key("project-remove"),
            60,
        )
        .await
        .expect("replay unregister");
    assert!(replay.replayed);
}

#[tokio::test]
async fn repository_refresh_preserves_snapshot_on_error_and_304_is_no_op() {
    let directory = TestDirectory::new();
    let store = StateStoreHandle::open(directory.database()).expect("open store");
    let owner = PrincipalId::local_owner();
    let first = store
        .register_repository(owner.clone(), repository(1, 10), key("repo-register"), 10)
        .await
        .expect("register repository");
    let not_modified = store
        .update_repository_validators(
            owner.clone(),
            first.value.repository_id,
            first.value.revision,
            RepositoryValidators {
                etag: Some("\"two\"".to_owned()),
                last_modified: None,
            },
            key("repo-refresh-304"),
            20,
        )
        .await
        .expect("304 refresh");
    assert_eq!(not_modified.value.revision.get(), 1);
    assert_eq!(
        not_modified.value.observation.validators.etag.as_deref(),
        Some("\"two\"")
    );

    let page = store
        .list_repository_packages(owner.clone(), first.value.repository_id, None, 100)
        .await
        .expect("list packages");
    assert_eq!(page.packages.len(), 1);
    let duplicate = store
        .register_repository(owner, repository(1, 30), key("repo-duplicate"), 30)
        .await
        .expect_err("reject duplicate source identity");
    assert_eq!(duplicate.code(), M3ErrorCode::RepositoryAlreadyRegistered);
}
