use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use alcomd_application::{
    DependencyIdentity, IdempotencyKey, M3ErrorCode, M3RegistryStore, M5UnityStore, ManifestState,
    PrincipalId, ProjectId, ProjectObservation, ProjectRecord, ProjectType, RepositoryObservation,
    RepositoryPackageLinks, RepositoryPackageVersion, RepositorySource, RepositoryValidators,
    Revision, StateStore, SyncWrite, UnityInstallationId,
};
use alcomd_store::StateStoreHandle;
use rusqlite::{Connection, params};
use uuid::Uuid;

const MIGRATIONS_V1_TO_V10: [&str; 10] = [
    include_str!("../migrations/0001_state.sql"),
    include_str!("../migrations/0002_projects_repositories.sql"),
    include_str!("../migrations/0003_package_transactions.sql"),
    include_str!("../migrations/0004_local_workflows.sql"),
    include_str!("../migrations/0005_template_plans.sql"),
    include_str!("../migrations/0006_backup_create.sql"),
    include_str!("../migrations/0007_backup_restore.sql"),
    include_str!("../migrations/0008_extension_runtime.sql"),
    include_str!("../migrations/0009_portable_extension_ui.sql"),
    include_str!("../migrations/0010_project_copy.sql"),
];

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
            links: Some(RepositoryPackageLinks {
                documentation: Some("https://example.invalid/docs".to_owned()),
                changelog: None,
            }),
            resolver: None,
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
async fn project_favorite_is_revisioned_idempotent_and_preserved_by_refresh() {
    let directory = TestDirectory::new();
    let store = StateStoreHandle::open(directory.database()).expect("open store");
    let owner = PrincipalId::local_owner();
    let registered = store
        .register_project(owner.clone(), project(7, 10), key("favorite-register"), 10)
        .await
        .expect("register project")
        .value;
    assert!(!registered.favorite);
    let later = store
        .register_project(
            owner.clone(),
            project(8, 15),
            key("favorite-order-register"),
            15,
        )
        .await
        .expect("register later project")
        .value;
    let before = store
        .list_projects(owner.clone(), None, 1)
        .await
        .expect("first project page");
    assert_eq!(before.projects[0].project_id, later.project_id);
    let before_cursor = before.next_cursor.expect("first page cursor");

    let favorite = store
        .set_project_favorite(
            owner.clone(),
            registered.project_id,
            true,
            registered.revision,
            key("favorite-true"),
            20,
        )
        .await
        .expect("favorite project");
    assert!(favorite.value.favorite);
    assert_eq!(favorite.value.revision.get(), 2);
    let after = store
        .list_projects(owner.clone(), None, 1)
        .await
        .expect("first page after favorite");
    assert_eq!(after.projects[0].project_id, later.project_id);
    assert_eq!(after.next_cursor, Some(before_cursor));

    let replay = store
        .set_project_favorite(
            owner.clone(),
            registered.project_id,
            true,
            registered.revision,
            key("favorite-true"),
            21,
        )
        .await
        .expect("replay favorite");
    assert!(replay.replayed);
    assert_eq!(replay.value, favorite.value);

    let idempotency_conflict = store
        .set_project_favorite(
            owner.clone(),
            registered.project_id,
            false,
            favorite.value.revision,
            key("favorite-true"),
            22,
        )
        .await
        .expect_err("reject changed fingerprint");
    assert_eq!(
        idempotency_conflict.code(),
        M3ErrorCode::IdempotencyConflict
    );

    let no_op = store
        .set_project_favorite(
            owner.clone(),
            registered.project_id,
            true,
            favorite.value.revision,
            key("favorite-no-op"),
            30,
        )
        .await
        .expect("same-value no-op");
    assert_eq!(no_op.value.revision, favorite.value.revision);
    assert_eq!(no_op.value.favorite, favorite.value.favorite);

    let stale_no_op = store
        .set_project_favorite(
            owner.clone(),
            registered.project_id,
            true,
            registered.revision,
            key("favorite-stale"),
            31,
        )
        .await
        .expect_err("stale same-value request conflicts");
    assert_eq!(stale_no_op.code(), M3ErrorCode::RevisionConflict);

    let refreshed = store
        .refresh_project(
            owner.clone(),
            registered.project_id,
            favorite.value.revision,
            project(7, 40),
            key("favorite-refresh"),
            40,
        )
        .await
        .expect("refresh project");
    assert!(refreshed.value.favorite);
    let events = store
        .list_events(owner.clone(), 0, 100)
        .await
        .expect("list events");
    assert_eq!(
        events
            .events
            .iter()
            .filter(|event| event.kind == "project.favorite_changed")
            .count(),
        1
    );

    let removed = store
        .unregister_project(
            owner.clone(),
            registered.project_id,
            refreshed.value.revision,
            key("favorite-remove"),
            50,
        )
        .await
        .expect("unregister favorite project");
    let registered_again = store
        .register_project(owner, project(7, 60), key("favorite-register-again"), 60)
        .await
        .expect("re-register same identity");
    assert_ne!(registered_again.value.project_id, removed.id);
    assert!(!registered_again.value.favorite);
}

#[tokio::test]
async fn concurrent_favorite_mutations_use_exact_project_revision_cas() {
    let directory = TestDirectory::new();
    let store = StateStoreHandle::open(directory.database()).expect("open store");
    let owner = PrincipalId::local_owner();
    let registered = store
        .register_project(
            owner.clone(),
            project(9, 10),
            key("favorite-cas-register"),
            10,
        )
        .await
        .expect("register project")
        .value;
    let first = store.set_project_favorite(
        owner.clone(),
        registered.project_id,
        true,
        registered.revision,
        key("favorite-cas-a"),
        20,
    );
    let second = store.set_project_favorite(
        owner.clone(),
        registered.project_id,
        true,
        registered.revision,
        key("favorite-cas-b"),
        21,
    );
    let (first, second) = tokio::join!(first, second);
    let outcomes = [first, second];
    assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        outcomes
            .iter()
            .filter_map(|result| result.as_ref().err())
            .filter(|error| error.code() == M3ErrorCode::RevisionConflict)
            .count(),
        1
    );
    let current = store
        .get_project(owner, registered.project_id)
        .await
        .expect("read favorite project");
    assert!(current.favorite);
    assert_eq!(current.revision.get(), 2);
}

#[test]
fn legacy_durable_project_record_json_defaults_favorite_to_false() {
    let record = ProjectRecord {
        project_id: alcomd_application::ProjectId::new(),
        observation: project(10, 10),
        revision: alcomd_application::Revision::INITIAL,
        registered_at_ms: 10,
        favorite: true,
    };
    let mut legacy = serde_json::to_value(record).expect("serialize record");
    legacy
        .as_object_mut()
        .expect("record object")
        .remove("favorite");
    let restored: ProjectRecord = serde_json::from_value(legacy).expect("read v10 record JSON");
    assert!(!restored.favorite);
}

#[tokio::test]
async fn v10_project_idempotency_and_unity_arguments_survive_current_migration() {
    let directory = TestDirectory::new();
    let database = directory.database();
    let owner = PrincipalId::local_owner();
    let project_id = ProjectId::parse("00000000-0000-4000-8000-000000000711").expect("project ID");
    let installation_id = UnityInstallationId::parse("00000000-0000-4000-8000-000000000712")
        .expect("installation ID");
    let observation = project(11, 10);
    {
        let connection = Connection::open(&database).expect("open v10 fixture");
        for migration in MIGRATIONS_V1_TO_V10 {
            connection
                .execute_batch(migration)
                .expect("apply v10 chain");
        }
        let mut semantic = observation.clone();
        semantic.observed_at_ms = 0;
        connection
            .execute(
                "INSERT INTO projects (
                    project_id, owner_principal_id, root_path, path_identity_key, project_type,
                    unity_version, unity_revision, snapshot_json, revision, registered_at_ms,
                    observed_at_ms, updated_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, 'avatars', ?5, NULL, ?6, 1, 10, 10, 10)",
                params![
                    project_id.to_string(),
                    owner.as_str(),
                    observation.root_path,
                    observation.path_identity_key,
                    observation.unity_version,
                    serde_json::to_string(&semantic).expect("serialize semantic project"),
                ],
            )
            .expect("insert v10 project");
        connection
            .execute(
                "INSERT INTO unity_installations (
                    installation_id, owner_principal_id, executable_path,
                    filesystem_identity_key, unity_version, architecture, source_kind,
                    revision, observed_at_ms, updated_at_ms
                 ) VALUES (?1, ?2, 'X:/Unity.exe', x'12', '2022.3.40f1', 'x86_64',
                           'manual', 1, 10, 10)",
                params![installation_id.to_string(), owner.as_str()],
            )
            .expect("insert v10 installation");
        connection
            .execute(
                "INSERT INTO project_editor_preferences (
                    project_id, installation_id, arguments_json, revision, updated_at_ms
                 ) VALUES (?1, ?2, '[\"-logFile\",\"-\"]', 1, 10)",
                params![project_id.to_string(), installation_id.to_string()],
            )
            .expect("insert v10 preference");

        let record = ProjectRecord {
            project_id,
            observation: observation.clone(),
            revision: Revision::INITIAL,
            registered_at_ms: 10,
            favorite: false,
        };
        let mut project_response = serde_json::to_value(SyncWrite {
            value: record,
            replayed: false,
        })
        .expect("serialize project response");
        project_response["value"]
            .as_object_mut()
            .expect("project record")
            .remove("favorite");
        let register_fingerprint = serde_json::to_string(&serde_json::json!({
            "pathIdentityKey": observation.path_identity_key,
            "rootPath": observation.root_path,
            "version": 1
        }))
        .expect("register fingerprint");
        let refresh_fingerprint = serde_json::to_string(&serde_json::json!({
            "expectedRevision": 1,
            "id": project_id.to_string(),
            "version": 1
        }))
        .expect("refresh fingerprint");
        for (method, key, fingerprint) in [
            ("projects.register", "legacy-register", register_fingerprint),
            ("projects.refresh", "legacy-refresh", refresh_fingerprint),
        ] {
            connection
                .execute(
                    "INSERT INTO idempotency_records (
                        principal_id, method, idempotency_key, request_fingerprint, state,
                        operation_id, response_json, created_at_ms
                     ) VALUES (?1, ?2, ?3, ?4, 'completed', NULL, ?5, 10)",
                    params![
                        owner.as_str(),
                        method,
                        key,
                        fingerprint,
                        serde_json::to_string(&project_response)
                            .expect("serialize legacy project response"),
                    ],
                )
                .expect("insert legacy project response");
        }
    }

    let store = StateStoreHandle::open(database).expect("open and migrate v10 fixture");
    let register = store
        .register_project(
            owner.clone(),
            observation.clone(),
            key("legacy-register"),
            20,
        )
        .await
        .expect("replay v10 project register");
    assert!(register.replayed);
    assert!(!register.value.favorite);
    let refresh = store
        .refresh_project(
            owner.clone(),
            project_id,
            Revision::INITIAL,
            observation,
            key("legacy-refresh"),
            21,
        )
        .await
        .expect("replay v10 project refresh");
    assert!(refresh.replayed);
    assert!(!refresh.value.favorite);
    let config = store
        .get_project_launch_config(owner, project_id)
        .await
        .expect("read migrated launch arguments");
    assert_eq!(config.arguments, ["-logFile", "-"]);
    assert_eq!(config.revision, Some(Revision::INITIAL));
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
    assert_eq!(
        page.packages[0]
            .links
            .as_ref()
            .and_then(|links| links.documentation.as_deref()),
        Some("https://example.invalid/docs")
    );
    let duplicate = store
        .register_repository(owner, repository(1, 30), key("repo-duplicate"), 30)
        .await
        .expect_err("reject duplicate source identity");
    assert_eq!(duplicate.code(), M3ErrorCode::RepositoryAlreadyRegistered);
}
