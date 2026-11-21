use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use alcomd_application::{
    AccessContext, CandidateStore, DependencyIdentity, IdempotencyKey, M3ErrorCode,
    M3RegistryStore, M4ErrorCode, M4Store, ManifestState, PrincipalId, ProjectObservation,
    ProjectType, RepositoryArchivePinStore, RepositoryId, RepositoryObservation,
    RepositoryPackageLinks, RepositoryPackageSourcePin, RepositoryPackageVersion, RepositorySource,
    RepositoryValidators, ResolverCatalogSource, ResolverPackageMetadata, Revision, StateStore,
    VRCHAT_CURATED_REPOSITORY_ID, VRCHAT_OFFICIAL_REPOSITORY_ID,
};
use alcomd_store::StateStoreHandle;
use rusqlite::Connection;
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

#[tokio::test]
async fn repository_pages_keep_builtins_first_and_users_in_registration_order() {
    let directory = TestDirectory::new();
    let store = StateStoreHandle::open(directory.database()).expect("repository list fixture");
    let owner = PrincipalId::local_owner();
    let older = store
        .register_repository(owner.clone(), repository(1, 1), key("older"), 10)
        .await
        .expect("register first user repository")
        .value
        .repository_id;
    let official = RepositoryId::parse(VRCHAT_OFFICIAL_REPOSITORY_ID).expect("official ID");
    let curated = RepositoryId::parse(VRCHAT_CURATED_REPOSITORY_ID).expect("curated ID");
    store
        .ensure_builtin_repositories(
            owner.clone(),
            vec![(curated, repository(2, 1)), (official, repository(3, 1))],
            10,
        )
        .await
        .expect("register built-ins after an existing user repository");
    let newer = store
        .register_repository(owner.clone(), repository(4, 1), key("newer"), 10)
        .await
        .expect("register second user repository in the same millisecond")
        .value
        .repository_id;

    let mut ids = Vec::new();
    let mut cursor = None;
    for _ in 0..4 {
        let page = store
            .list_repositories(owner.clone(), cursor, 1)
            .await
            .expect("list one repository at a time");
        ids.extend(
            page.repositories
                .into_iter()
                .map(|record| record.repository_id),
        );
        cursor = page.next_cursor;
    }
    assert_eq!(ids, [official, curated, older, newer]);
    assert!(cursor.is_none());

    let page = store
        .list_repositories(owner.clone(), None, 3)
        .await
        .expect("read through the first user repository");
    let cursor = page.next_cursor.expect("the newer repository remains");
    store
        .unregister_repository(
            owner.clone(),
            older,
            Revision::INITIAL,
            key("remove-older"),
            11,
        )
        .await
        .expect("remove the repository at the pagination boundary");
    assert_eq!(
        store
            .list_repositories(owner, Some(cursor), 1)
            .await
            .expect_err("report the lost pagination boundary")
            .code(),
        M3ErrorCode::RevisionConflict,
    );
}

#[tokio::test]
async fn repository_list_snapshot_covers_all_pages_and_registered_project_roots() {
    use alcomd_application::RepositoryListStore;
    let directory = TestDirectory::new();
    let store =
        StateStoreHandle::open(directory.database()).expect("repository list snapshot fixture");
    let owner = PrincipalId::local_owner();
    for index in 0..230_u8 {
        store
            .register_repository(
                owner.clone(),
                repository(index, 1),
                key(&format!("list-{index}")),
                10,
            )
            .await
            .expect("repository list snapshot fixture");
    }
    store
        .register_project(owner.clone(), project(1), key("list-project"), 10)
        .await
        .expect("repository list snapshot fixture");
    let first_page = store
        .list_repositories(owner.clone(), None, 100)
        .await
        .expect("repository list snapshot fixture");
    assert_eq!(first_page.repositories.len(), 100);
    assert!(first_page.next_cursor.is_some());
    let snapshot = store
        .repository_list_snapshot(owner.clone())
        .await
        .expect("repository list snapshot fixture");
    assert_eq!(snapshot.repositories.len(), 230);
    assert_eq!(snapshot.project_roots, ["C:/fixture/1"]);
    assert!(
        snapshot
            .repositories
            .iter()
            .all(|record| record.revision == 1)
    );
    assert_eq!(
        store
            .repository_list_snapshot(
                PrincipalId::parse("extension:fixture").expect("repository list snapshot fixture")
            )
            .await
            .expect("repository list snapshot fixture")
            .repositories
            .len(),
        0
    );
    drop(store);
}

fn project(identity: u8) -> ProjectObservation {
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
    }
}

#[tokio::test]
async fn project_list_view_filters_orders_and_rejects_stale_continuation() {
    use alcomd_application::{ProjectListSort, ProjectListView};

    let directory = TestDirectory::new();
    let store = StateStoreHandle::open(directory.database()).expect("project view fixture");
    let owner = PrincipalId::local_owner();
    let other = PrincipalId::parse("extension:fixture").expect("another principal");
    let mut records = Vec::new();
    for (index, name) in [
        "Avatar Favorite",
        "Avatar Older",
        "Avatar Recent",
        "Avatar Unknown",
        "World",
        "Avatar Private",
    ]
    .into_iter()
    .enumerate()
    {
        let mut observation = project(index as u8 + 1);
        observation.root_path = format!("C:\\fixture\\{name}\\");
        records.push(
            store
                .register_project(
                    if index == 5 {
                        other.clone()
                    } else {
                        owner.clone()
                    },
                    observation,
                    key(&format!("view-register-{index}")),
                    10 + index as u64,
                )
                .await
                .expect("register project view fixture")
                .value,
        );
    }
    store
        .set_project_favorite(
            owner.clone(),
            records[0].project_id,
            true,
            records[0].revision,
            key("view-favorite"),
            30,
        )
        .await
        .expect("set favorite for view fixture");
    let fixture = Connection::open(directory.database()).expect("open isolated fixture");
    fixture
        .execute(
            "UPDATE projects SET last_modified_at_ms=NULL WHERE project_id=?1",
            [records[3].project_id.to_string()],
        )
        .expect("set a legacy project activity time to unknown");
    fixture
        .execute(
            "UPDATE projects SET snapshot_json=json_set(snapshot_json,'$.unity_version','6000.0.1f1') WHERE project_id=?1",
            [records[2].project_id.to_string()],
        )
        .expect("represent a migrated editor version in the authoritative snapshot");
    drop(fixture);
    let view = ProjectListView {
        search: Some("AVATAR".into()),
        sort: ProjectListSort::Modified,
        descending: true,
    };
    let first = store
        .list_project_view(AccessContext::local_owner(), None, 2, view.clone(), None)
        .await
        .expect("first view page");
    assert_eq!(
        first
            .page
            .projects
            .iter()
            .map(|project| project.project_id)
            .collect::<Vec<_>>(),
        [records[0].project_id, records[2].project_id]
    );
    let second = store
        .list_project_view(
            AccessContext::local_owner(),
            first.page.next_cursor.clone(),
            2,
            view.clone(),
            Some(first.sequence),
        )
        .await
        .expect("continue the same view");
    assert_eq!(
        second
            .page
            .projects
            .iter()
            .map(|project| project.project_id)
            .collect::<Vec<_>>(),
        [records[1].project_id, records[3].project_id]
    );
    assert!(second.page.next_cursor.is_none());
    let ascending = store
        .list_project_view(
            AccessContext::local_owner(),
            None,
            10,
            ProjectListView {
                descending: false,
                ..view.clone()
            },
            None,
        )
        .await
        .expect("ascending view");
    assert_eq!(
        ascending
            .page
            .projects
            .iter()
            .map(|project| project.project_id)
            .collect::<Vec<_>>(),
        [
            records[0].project_id,
            records[1].project_id,
            records[2].project_id,
            records[3].project_id
        ]
    );
    let by_name = store
        .list_project_view(
            AccessContext::local_owner(),
            None,
            10,
            ProjectListView {
                sort: ProjectListSort::Name,
                descending: true,
                ..view.clone()
            },
            None,
        )
        .await
        .expect("name view");
    assert_eq!(
        by_name
            .page
            .projects
            .iter()
            .map(|project| project.project_id)
            .collect::<Vec<_>>(),
        [
            records[0].project_id,
            records[3].project_id,
            records[2].project_id,
            records[1].project_id
        ]
    );
    let by_unity = store
        .list_project_view(
            AccessContext::local_owner(),
            None,
            2,
            ProjectListView {
                sort: ProjectListSort::Unity,
                ..view.clone()
            },
            None,
        )
        .await
        .expect("Unity version view");
    assert_eq!(by_unity.page.projects[1].project_id, records[2].project_id);
    assert_eq!(
        by_unity.page.projects[1].observation.unity_version,
        "6000.0.1f1"
    );
    store
        .set_project_favorite(
            owner.clone(),
            records[3].project_id,
            true,
            records[3].revision,
            key("view-move-favorite"),
            31,
        )
        .await
        .expect("invalidate the ordered view");
    let stale = store
        .list_project_view(
            AccessContext::local_owner(),
            first.page.next_cursor,
            2,
            view,
            Some(first.sequence),
        )
        .await
        .expect_err("reordered view must restart at the beginning");
    assert_eq!(stale.code(), M3ErrorCode::RevisionConflict);
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
            presentation: None,
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
        .register_project(owner.clone(), project(1), key("project-register"), 10)
        .await
        .expect("register project");
    assert_eq!(first.value.revision.get(), 1);
    assert_eq!(first.value.last_modified_at_ms, Some(10));
    let replay = store
        .register_project(owner.clone(), project(1), key("project-register"), 20)
        .await
        .expect("replay registration");
    assert!(replay.replayed);
    assert_eq!(replay.value, first.value);
    assert_eq!(replay.value.last_modified_at_ms, Some(10));

    let no_op = store
        .refresh_project(
            owner.clone(),
            first.value.project_id,
            first.value.revision,
            project(1),
            key("project-refresh-noop"),
            30,
        )
        .await
        .expect("no-op refresh");
    assert_eq!(no_op.value.revision.get(), 1);
    assert_eq!(no_op.value, first.value);
    assert!(
        serde_json::to_value(&no_op.value.observation)
            .expect("serialize refreshed project")
            .get("observed_at_ms")
            .is_none()
    );
    assert_eq!(no_op.value.last_modified_at_ms, Some(10));
    let no_op_replay = store
        .refresh_project(
            owner.clone(),
            first.value.project_id,
            first.value.revision,
            project(1),
            key("project-refresh-noop"),
            35,
        )
        .await
        .expect("refresh replay returns the original successful observation");
    assert!(no_op_replay.replayed);
    assert_eq!(no_op_replay.value, no_op.value);
    let mut changed = project(1);
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
    assert_eq!(refreshed.value.last_modified_at_ms, Some(10));
    let read_back = store
        .get_project(owner.clone(), first.value.project_id)
        .await
        .expect("read current project");
    assert_eq!(read_back.last_modified_at_ms, Some(10));
    assert_eq!(read_back, refreshed.value);
    let events = store
        .list_events(owner.clone(), 0, 100)
        .await
        .expect("events");
    assert_eq!(events.events.len(), 2);
    assert_eq!(
        events
            .events
            .iter()
            .filter(|event| event.aggregate_kind == "project")
            .count(),
        2
    );

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
        .register_project(owner.clone(), project(7), key("favorite-register"), 10)
        .await
        .expect("register project")
        .value;
    assert!(!registered.favorite);
    assert_eq!(registered.last_modified_at_ms, Some(10));
    let later = store
        .register_project(
            owner.clone(),
            project(8),
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
    assert_eq!(favorite.value.last_modified_at_ms, Some(10));
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
            project(7),
            key("favorite-refresh"),
            40,
        )
        .await
        .expect("refresh project");
    assert!(refreshed.value.favorite);
    assert_eq!(refreshed.value.last_modified_at_ms, Some(10));
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
        .register_project(owner, project(7), key("favorite-register-again"), 60)
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
        .register_project(owner.clone(), project(9), key("favorite-cas-register"), 10)
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
        store
            .get_repository(owner.clone(), first.value.repository_id)
            .await
            .expect("updated conditional retrieval metadata")
            .observation
            .validators
            .etag
            .as_deref(),
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

#[tokio::test]
async fn repository_display_name_survives_refresh_and_only_emits_events_for_changes() {
    let directory = TestDirectory::new();
    let store = StateStoreHandle::open(directory.database()).expect("open store");
    let owner = PrincipalId::local_owner();
    let official = RepositoryId::parse(VRCHAT_OFFICIAL_REPOSITORY_ID).expect("official ID");
    store
        .ensure_builtin_repositories(owner.clone(), vec![(official, repository(1, 1))], 1)
        .await
        .expect("register built-in");

    let name = Some("My VRChat source".to_owned());
    let renamed = store
        .set_repository_display_name(
            owner.clone(),
            official,
            Revision::INITIAL,
            name.clone(),
            key("rename"),
            2,
        )
        .await
        .expect("set a built-in display name");
    assert_eq!(renamed.value.display_name, name);
    let replay = store
        .set_repository_display_name(
            owner.clone(),
            official,
            Revision::INITIAL,
            name.clone(),
            key("rename"),
            3,
        )
        .await
        .expect("replay the accepted rename");
    assert!(replay.replayed);
    assert_eq!(replay.value.revision, renamed.value.revision);
    let unchanged = store
        .set_repository_display_name(
            owner.clone(),
            official,
            renamed.value.revision,
            name.clone(),
            key("unchanged"),
            4,
        )
        .await
        .expect("save the same name");
    assert_eq!(unchanged.value.revision, renamed.value.revision);

    let mut remote = repository(1, 5);
    remote.name = Some("Updated feed name".to_owned());
    let refreshed = store
        .refresh_repository(
            owner.clone(),
            official,
            renamed.value.revision,
            remote,
            key("refresh-renamed"),
            5,
        )
        .await
        .expect("refresh repository");
    assert_eq!(refreshed.value.display_name, name);
    assert_eq!(
        refreshed.value.observation.name.as_deref(),
        Some("Updated feed name")
    );
    let stale = store
        .set_repository_display_name(
            owner.clone(),
            official,
            renamed.value.revision,
            None,
            key("stale-clear"),
            6,
        )
        .await
        .expect_err("reject a stale edit");
    assert_eq!(stale.code(), M3ErrorCode::RevisionConflict);
    let cleared = store
        .set_repository_display_name(
            owner.clone(),
            official,
            refreshed.value.revision,
            None,
            key("clear"),
            7,
        )
        .await
        .expect("restore the default name");
    assert!(cleared.value.display_name.is_none());
    let record = store
        .get_repository(owner.clone(), official)
        .await
        .expect("read saved repository");
    assert!(record.display_name.is_none());
    assert_eq!(record.observation.name, refreshed.value.observation.name);
    let events = store
        .list_events(owner, 0, 100)
        .await
        .expect("read committed events");
    assert_eq!(
        events
            .events
            .iter()
            .filter(|event| event.kind == "repository.display-name-changed")
            .count(),
        2,
    );
}

#[tokio::test]
async fn native_repository_requests_share_resources_and_recheck_the_callers_credential() {
    use alcomd_application::{
        McpIdentityApplication, McpIdentityWrite, RepositoryListStore, ToolAuthentication,
        with_log_caller, with_workflow_authentication,
    };

    let directory = TestDirectory::new();
    let store = StateStoreHandle::open(directory.database()).expect("open store");
    let identities = McpIdentityApplication::new(store.clone());
    let owner = AccessContext::local_owner();
    let client = identities
        .write(
            &owner,
            "create".into(),
            McpIdentityWrite::Create {
                name: "Repository manager".into(),
            },
            1,
        )
        .await
        .expect("create client")
        .client;
    let hash = [0x51; 32];
    let issued = identities
        .write(
            &owner,
            "issue".into(),
            McpIdentityWrite::Issue {
                id: client.client_id,
                expected: client.revision,
                hash,
            },
            2,
        )
        .await
        .expect("issue credential")
        .client;
    let access = identities.authenticate(hash).await.expect("authenticate");
    let caller = access.principal();
    let auth = ToolAuthentication::Credential(hash);
    let reference = Uuid::new_v4().to_string();
    let replacement = Uuid::new_v4().to_string();
    let registered = with_workflow_authentication(
        auth.clone(),
        with_log_caller(store.clone(), &access, async {
            store
                .register_repository_authenticated(
                    caller.clone(),
                    repository(6, 3),
                    reference.clone(),
                    key("register"),
                    3,
                )
                .await
                .expect("register with a credential")
        }),
    )
    .await
    .value;
    let id = registered.repository_id;
    assert_eq!(
        store
            .repository_credential_reference(owner.principal().clone(), id)
            .await
            .expect("shared reference"),
        Some(reference.clone())
    );
    with_workflow_authentication(
        auth.clone(),
        with_log_caller(store.clone(), &access, async {
            let set_reference = || {
                store.set_repository_credential_reference(
                    caller.clone(),
                    id,
                    registered.revision,
                    Some(replacement.clone()),
                    key("credentials"),
                    4,
                )
            };
            assert!(!set_reference().await.expect("replace credential").replayed);
            assert!(
                set_reference()
                    .await
                    .expect("replay credential change")
                    .replayed
            );
        }),
    )
    .await;
    assert_eq!(
        store
            .repository_credential_reference(owner.principal().clone(), id)
            .await
            .expect("replaced reference"),
        Some(replacement.clone())
    );
    let events = store
        .list_events(owner.principal().clone(), 0, 100)
        .await
        .expect("shared events")
        .events;
    for kind in ["repository.registered", "repository.refreshed"] {
        assert!(
            events
                .iter()
                .any(|event| event.kind == kind && event.aggregate_id == id.to_string())
        );
    }
    {
        let db = Connection::open_with_flags(
            directory.database(),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .expect("inspect isolated records");
        for method in ["repositories.register", "repositories.setCredentials"] {
            let receipt: String = db
                .query_row(
                    "SELECT principal_id FROM idempotency_records WHERE method=?1",
                    [method],
                    |row| row.get(0),
                )
                .expect("receipt");
            let log: String = db
                .query_row(
                    "SELECT owner FROM operation_log_entries WHERE action=?1",
                    [method],
                    |row| row.get(0),
                )
                .expect("operation log");
            assert_eq!(receipt, caller.as_str());
            assert_eq!(log, caller.as_str());
        }
    }
    identities
        .write(
            &owner,
            "revoke".into(),
            McpIdentityWrite::Revoke {
                id: issued.client_id,
                expected: issued.revision,
            },
            5,
        )
        .await
        .expect("revoke");
    with_workflow_authentication(auth, async {
        let denied = M3ErrorCode::PermissionDenied;
        for (request, key) in [
            (
                alcomd_application::RepositoryWriteRequest::Register {
                    source: repository(6, 3).source,
                    authenticated: true,
                },
                key("register"),
            ),
            (
                alcomd_application::RepositoryWriteRequest::SetCredentials {
                    id,
                    expected: registered.revision,
                    authenticated: true,
                },
                key("credentials"),
            ),
        ] {
            assert_eq!(
                store
                    .replay_repository_write(caller.clone(), request, key)
                    .await
                    .expect_err("revoked credentials cannot retrieve accepted native receipts")
                    .code(),
                denied
            );
        }
        assert_eq!(
            store
                .repository_list_snapshot(caller.clone())
                .await
                .expect_err("snapshot")
                .code(),
            denied
        );
        assert_eq!(
            store
                .repository_credential_reference(caller.clone(), id)
                .await
                .expect_err("credential reference")
                .code(),
            denied
        );
        assert_eq!(
            store
                .register_repository_authenticated(
                    caller.clone(),
                    repository(6, 3),
                    reference,
                    key("register"),
                    6
                )
                .await
                .expect_err("registration replay")
                .code(),
            denied
        );
        assert_eq!(
            store
                .set_repository_credential_reference(
                    caller.clone(),
                    id,
                    registered.revision,
                    Some(replacement.clone()),
                    key("credentials"),
                    6
                )
                .await
                .expect_err("credential replay")
                .code(),
            denied
        );
        assert_eq!(
            store
                .unregister_repository(caller.clone(), id, registered.revision, key("remove"), 6)
                .await
                .expect_err("remove")
                .code(),
            denied
        );
        assert_eq!(
            store
                .apply_repository_list_metadata(caller.clone(), String::new(), Vec::new(), 6)
                .await
                .expect_err("metadata import")
                .code(),
            denied
        );
    })
    .await;
    assert_eq!(
        store
            .repository_credential_reference(owner.principal().clone(), id)
            .await
            .expect("shared record retained"),
        Some(replacement)
    );
}

#[tokio::test]
async fn repository_credentials_follow_revision_and_orphan_cleanup() {
    let directory = TestDirectory::new();
    let store = StateStoreHandle::open(directory.database()).expect("open store");
    let owner = PrincipalId::local_owner();
    let other = PrincipalId::parse("extension:fixture").expect("isolated principal");
    let original = Uuid::new_v4().to_string();
    let replacement = Uuid::new_v4().to_string();
    for reference in [&original, &replacement] {
        store
            .queue_repository_credential_cleanup(reference.clone())
            .await
            .expect("stage reference");
    }
    let first = store
        .register_repository_authenticated(
            owner.clone(),
            repository(1, 10),
            original.clone(),
            key("auth-register"),
            10,
        )
        .await
        .expect("register authenticated repository");
    let id = first.value.repository_id;
    assert!(first.value.credentials_configured);
    assert!(
        !serde_json::to_string(&first.value)
            .expect("public record")
            .contains(&original)
    );
    assert_eq!(
        store
            .pending_repository_credential_cleanup()
            .await
            .expect("orphans")
            .as_slice(),
        std::slice::from_ref(&replacement)
    );
    assert_eq!(
        store
            .repository_credential_reference(other, id)
            .await
            .expect_err("other owners cannot look up the reference")
            .code(),
        M3ErrorCode::RepositoryNotRegistered
    );

    let mut observation = repository(1, 20);
    observation.name = Some("Changed".to_owned());
    let refreshed = store
        .refresh_repository(
            owner.clone(),
            id,
            first.value.revision,
            observation,
            key("auth-refresh"),
            20,
        )
        .await
        .expect("refresh authenticated index");
    assert!(refreshed.value.credentials_configured);
    assert_eq!(
        store
            .repository_credential_reference(owner.clone(), id)
            .await
            .expect("active reference"),
        Some(original.clone())
    );

    let rotated = store
        .set_repository_credential_reference(
            owner.clone(),
            id,
            refreshed.value.revision,
            Some(replacement.clone()),
            key("auth-rotate"),
            30,
        )
        .await
        .expect("rotate credentials");
    assert_eq!(
        rotated.value.revision.get(),
        refreshed.value.revision.get() + 1
    );
    assert_eq!(
        rotated.value.observation.validators,
        RepositoryValidators::default()
    );
    assert_eq!(
        store
            .pending_repository_credential_cleanup()
            .await
            .expect("orphans")
            .as_slice(),
        std::slice::from_ref(&original)
    );
    store
        .finish_repository_credential_cleanup(original)
        .await
        .expect("retire old reference");

    let cleared = store
        .set_repository_credential_reference(
            owner.clone(),
            id,
            rotated.value.revision,
            None,
            key("auth-clear"),
            40,
        )
        .await
        .expect("clear credentials");
    assert!(!cleared.value.credentials_configured);
    assert_eq!(
        store
            .repository_credential_reference(owner, id)
            .await
            .expect("cleared reference"),
        None
    );
    assert_eq!(
        store
            .pending_repository_credential_cleanup()
            .await
            .expect("orphans"),
        [replacement]
    );
    drop(store);
}

#[tokio::test]
async fn large_repository_write_replays_keep_metadata_and_paginate_package_versions() {
    let directory = TestDirectory::new();
    let store = StateStoreHandle::open(directory.database()).expect("open store");
    let owner = PrincipalId::local_owner();
    let mut index = repository(1, 10);
    index.declared_url = Some("https://example.invalid/public-index.json".into());
    index.issues.push(alcomd_application::ReadIssue {
        code: "invalid_metadata".into(),
        component: "repository".into(),
        item: "display name".into(),
        line: Some(3),
        column: Some(8),
    });
    let mut small_index = index.clone();
    small_index.refreshed_at_ms = 50;
    let package = index.packages.pop().expect("fixture package");
    index.packages = (0..1100)
        .map(|version| RepositoryPackageVersion {
            presentation: None,
            version: format!("1.0.{version}"),
            ..package.clone()
        })
        .collect();
    assert!(serde_json::to_vec(&index).expect("encode index").len() > 65_536);

    let first = store
        .register_repository(owner.clone(), index.clone(), key("large-register"), 10)
        .await
        .expect("register index above replay budget");
    assert!(first.value.observation.packages.is_empty());
    let id = first.value.repository_id;
    let mut refreshed_index = index.clone();
    refreshed_index.refreshed_at_ms = 20;
    refreshed_index.packages[0].display_name = Some("Updated".to_owned());
    let refreshed = store
        .refresh_repository(
            owner.clone(),
            id,
            first.value.revision,
            refreshed_index.clone(),
            key("large-refresh"),
            20,
        )
        .await
        .expect("refresh index above replay budget");
    assert_eq!(refreshed.value.revision.get(), 2);
    assert_eq!(refreshed.value.observation.issues, refreshed_index.issues);
    assert_eq!(
        refreshed.value.observation.declared_url,
        refreshed_index.declared_url
    );
    assert_eq!(refreshed.value.observation.refreshed_at_ms, 20);

    let not_modified = store
        .update_repository_validators(
            owner.clone(),
            id,
            refreshed.value.revision,
            RepositoryValidators::default(),
            key("large-304"),
            30,
        )
        .await
        .expect("record 304 without serializing the index");
    assert_eq!(not_modified.value.revision, refreshed.value.revision);
    assert_eq!(not_modified.value.observation.refreshed_at_ms, 30);

    let register_replay = store
        .register_repository(owner.clone(), index, key("large-register"), 40)
        .await
        .expect("replay registration after refresh");
    let refresh_replay = store
        .refresh_repository(
            owner.clone(),
            id,
            first.value.revision,
            refreshed_index,
            key("large-refresh"),
            40,
        )
        .await
        .expect("replay original refresh after 304");
    let validator_replay = store
        .update_repository_validators(
            owner.clone(),
            id,
            refreshed.value.revision,
            RepositoryValidators::default(),
            key("large-304"),
            40,
        )
        .await
        .expect("replay original 304");
    assert!(register_replay.replayed);
    assert_eq!(
        register_replay.value.repository_id,
        first.value.repository_id
    );
    assert_eq!(register_replay.value.revision, first.value.revision);
    assert_eq!(
        register_replay.value.registered_at_ms,
        first.value.registered_at_ms
    );
    assert_eq!(register_replay.value.observation, first.value.observation);
    assert!(refresh_replay.replayed && validator_replay.replayed);
    assert_eq!(refresh_replay.value, refreshed.value);
    assert_eq!(validator_replay.value, not_modified.value);

    let mut cursor = None;
    let mut count = 0;
    loop {
        let page = store
            .list_repository_packages(owner.clone(), id, cursor, 100)
            .await
            .expect("read one package page");
        if count == 0 {
            assert_eq!(page.packages[0].display_name.as_deref(), Some("Updated"));
        }
        count += page.packages.len();
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(count, 1100);
    let small = store
        .refresh_repository(
            owner.clone(),
            id,
            refreshed.value.revision,
            small_index,
            key("small-refresh"),
            50,
        )
        .await
        .expect("refresh with one package");
    store
        .unregister_repository(
            owner.clone(),
            id,
            small.value.revision,
            key("remove-before-replay"),
            60,
        )
        .await
        .expect("remove the registered repository");
    for (name, expected_revision, original) in [
        ("large-refresh", first.value.revision, refreshed.value),
        (
            "large-304",
            refresh_replay.value.revision,
            not_modified.value,
        ),
    ] {
        let restored = store
            .replay_repository_refresh(owner.clone(), id, expected_revision, key(name))
            .await
            .expect("lookup after removal")
            .expect("durable refresh receipt");
        assert!(restored.replayed);
        assert_eq!(restored.value, original);
    }
    let db = rusqlite::Connection::open(directory.database()).expect("receipt database");
    let receipt = |key: &str| -> String {
        db.query_row(
            "SELECT response_json FROM idempotency_records WHERE method='repositories.refresh' AND idempotency_key=?1",
            [key],
            |row| row.get(0),
        ).expect("frozen receipt")
    };
    let large = receipt("large-refresh");
    assert_eq!(large.len(), receipt("small-refresh").len());
    for value in [large, receipt("large-304")] {
        for private in [
            "com.example.package",
            "packages",
            "manifest_fingerprint",
            "validators",
            "source_identity_key",
        ] {
            assert!(!value.contains(private), "{private}");
        }
    }
}

fn resolver_index_fixture() -> RepositoryObservation {
    let mut observation = repository(1, 10);
    let valid = &mut observation.packages[0];
    valid.resolver = Some(ResolverPackageMetadata {
        semantic_version: valid.version.clone(),
        author_name: Some("Fixture".into()),
        author_email: None,
        artifact_url: "https://example.invalid/package.zip".into(),
        zip_sha256: None,
        unity_release: None,
        dependencies_json: "{}".into(),
        manifest_fingerprint: vec![7; 32],
        legacy_metadata_present: false,
        download_headers_present: false,
    });
    observation.packages.push(RepositoryPackageVersion {
        presentation: None,
        version: "2.0.0".into(),
        resolver: None,
        ..observation.packages[0].clone()
    });
    observation
}

#[tokio::test]
async fn resolver_catalog_preserves_package_download_header_presence() {
    let directory = TestDirectory::new();
    let store = StateStoreHandle::open(directory.database()).expect("open isolated store");
    let owner = PrincipalId::local_owner();
    let mut observation = resolver_index_fixture();
    observation.packages[0]
        .resolver
        .as_mut()
        .expect("resolver metadata")
        .download_headers_present = true;
    let id = store
        .register_repository(owner.clone(), observation, key("package-headers"), 10)
        .await
        .expect("register indexed source")
        .value
        .repository_id;
    let catalog = store
        .resolver_catalog(
            owner.clone(),
            false,
            alcomd_application::ConfigSnapshot::default()
                .settings
                .packages,
        )
        .await
        .expect("catalog");
    assert!(matches!(
        catalog.entries[0].source,
        ResolverCatalogSource::Repository {
            download_headers_present: true,
            ..
        }
    ));
    let entries = store
        .list_repository_packages(owner, id, None, 10)
        .await
        .expect("package rows");
    assert!(
        entries.packages[0]
            .resolver
            .as_ref()
            .expect("resolver metadata")
            .download_headers_present
    );
    drop(store);
}

#[tokio::test]
async fn hashless_archive_pins_survive_only_matching_repository_refreshes() {
    let directory = TestDirectory::new();
    let store = StateStoreHandle::open(directory.database()).expect("open isolated store");
    let owner = PrincipalId::local_owner();
    let observation = resolver_index_fixture();
    let repository = store
        .register_repository(
            owner.clone(),
            observation.clone(),
            key("hashless-register"),
            10,
        )
        .await
        .expect("register the first index")
        .value;
    assert!(repository.resolver_indexed);
    let id = repository.repository_id;
    let pin = RepositoryPackageSourcePin {
        credential_reference: None,
        download_headers: None,
        repository_id: id.to_string(),
        repository_revision: repository.revision.get(),
        source_identity: "remote:https://example.invalid/1?channel=stable".into(),
        manifest_fingerprint: [7; 32],
        package_id: "com.example.package".into(),
        version: "1.0.0".into(),
        artifact_url: "https://example.invalid/package.zip".into(),
        archive_sha256: Some([9; 32]),
    };
    for stale in [
        RepositoryPackageSourcePin {
            source_identity: "remote:https://wrong.invalid".into(),
            ..pin.clone()
        },
        RepositoryPackageSourcePin {
            credential_reference: None,
            download_headers: None,
            artifact_url: "https://wrong.invalid/package.zip".into(),
            ..pin.clone()
        },
        RepositoryPackageSourcePin {
            manifest_fingerprint: [8; 32],
            ..pin.clone()
        },
    ] {
        assert_eq!(
            store
                .pin_repository_archive(owner.clone(), stale)
                .await
                .expect_err("reject mismatched binding")
                .code(),
            M4ErrorCode::PlanStale
        );
    }
    let other = PrincipalId::parse("extension:other").expect("other principal");
    assert_eq!(
        store
            .pin_repository_archive(other, pin.clone())
            .await
            .expect_err("reject different owner")
            .code(),
        M4ErrorCode::PlanStale
    );
    let concurrent = RepositoryPackageSourcePin {
        archive_sha256: Some([10; 32]),
        ..pin.clone()
    };
    store
        .pin_repository_archive(owner.clone(), pin.clone())
        .await
        .expect("bind the first digest");
    assert_eq!(
        store
            .pin_repository_archive(owner.clone(), concurrent)
            .await
            .expect_err("reject a different digest")
            .code(),
        M4ErrorCode::PackageIntegrityMismatch
    );
    store
        .pin_repository_archive(owner.clone(), pin.clone())
        .await
        .expect("same digest is idempotent");
    drop(store);

    let store = StateStoreHandle::open(directory.database()).expect("reopen pinned store");
    let mut same_binding = observation.clone();
    same_binding.name = Some("Updated repository display name".into());
    let refreshed = store
        .refresh_repository(
            owner.clone(),
            id,
            repository.revision,
            same_binding,
            key("same-binding-refresh"),
            11,
        )
        .await
        .expect("refresh existing binding")
        .value;
    assert_ne!(refreshed.revision, repository.revision);
    let catalog = store
        .resolver_catalog(
            owner.clone(),
            false,
            alcomd_application::ConfigSnapshot::default()
                .settings
                .packages,
        )
        .await
        .expect("read indexed catalog");
    assert!(catalog.complete);
    assert_eq!(catalog.entries.len(), 1);
    assert!(
        matches!(catalog.entries[0].source, ResolverCatalogSource::Repository {
        zip_sha256: None, local_zip_sha256: Some(digest), ..
    } if Some(digest) == pin.archive_sha256)
    );
    let browse = store
        .list_repository_packages(owner.clone(), id, None, 100)
        .await
        .expect("browse the entire index");
    assert_eq!(browse.packages.len(), 2);
    assert!(browse.packages[1].resolver.is_none());
    let mut different_binding = observation;
    different_binding.packages[0]
        .resolver
        .as_mut()
        .expect("resolver metadata")
        .artifact_url = "https://example.invalid/replaced.zip".into();
    store
        .refresh_repository(
            owner.clone(),
            id,
            refreshed.revision,
            different_binding,
            key("new-binding-refresh"),
            12,
        )
        .await
        .expect("refresh with changed artifact binding");
    let catalog = store
        .resolver_catalog(
            owner,
            false,
            alcomd_application::ConfigSnapshot::default()
                .settings
                .packages,
        )
        .await
        .expect("read new artifact binding");
    assert!(matches!(
        catalog.entries[0].source,
        ResolverCatalogSource::Repository {
            local_zip_sha256: None,
            ..
        }
    ));
}

#[tokio::test]
async fn only_unindexed_snapshots_block_catalog_and_require_a_full_refresh() {
    let directory = TestDirectory::new();
    let store = StateStoreHandle::open(directory.database()).expect("open isolated store");
    let owner = PrincipalId::local_owner();
    let observation = resolver_index_fixture();
    let registered = store
        .register_repository(
            owner.clone(),
            observation.clone(),
            key("index-register"),
            10,
        )
        .await
        .expect("register full index")
        .value;
    let project = store
        .register_project(owner.clone(), project(1), key("index-project"), 10)
        .await
        .expect("register candidate test project")
        .value
        .project_id;
    let initial = store
        .candidate_snapshot(
            owner.clone(),
            project,
            None,
            alcomd_application::ConfigSnapshot::default()
                .settings
                .packages,
        )
        .await
        .expect("read valid candidates");
    assert!(initial.catalog_complete);
    assert_eq!(initial.entries.len(), 1);
    assert_eq!(initial.entries[0].version, "1.0.0");
    drop(store);
    let connection = Connection::open(directory.database()).expect("open fixture directly");
    connection
        .execute("UPDATE repositories SET resolver_indexed=0", [])
        .expect("fixture with a previous index");
    drop(connection);

    let store = StateStoreHandle::open(directory.database()).expect("open old index fixture");
    assert!(
        !store
            .resolver_catalog(
                owner.clone(),
                false,
                alcomd_application::ConfigSnapshot::default()
                    .settings
                    .packages
            )
            .await
            .expect("read unindexed catalog")
            .complete
    );
    let old = store
        .candidate_snapshot(
            owner.clone(),
            project,
            None,
            alcomd_application::ConfigSnapshot::default()
                .settings
                .packages,
        )
        .await
        .expect("read previous candidate snapshot");
    assert!(!old.catalog_complete);
    assert_eq!(old.entries.len(), 2);
    assert!(old.entries.iter().all(|entry| !entry.metadata_ready));
    assert_eq!(
        store
            .update_repository_validators(
                owner.clone(),
                registered.repository_id,
                registered.revision,
                RepositoryValidators::default(),
                key("304-stale"),
                11
            )
            .await
            .expect_err("reject 304 until a full index has been read")
            .code(),
        M3ErrorCode::RepositoryUnavailable
    );
    let fresh = store
        .refresh_repository(
            owner.clone(),
            registered.repository_id,
            registered.revision,
            observation,
            key("200-fresh"),
            12,
        )
        .await
        .expect("accept unchanged full index under current rules")
        .value;
    assert!(
        store
            .get_repository(owner.clone(), registered.repository_id)
            .await
            .expect("current registry")
            .resolver_indexed
    );
    assert_eq!(fresh.revision, registered.revision);
    let current = store
        .candidate_snapshot(
            owner,
            project,
            None,
            alcomd_application::ConfigSnapshot::default()
                .settings
                .packages,
        )
        .await
        .expect("read reindexed candidates");
    assert!(current.catalog_complete);
    assert_eq!(current.entries.len(), 1);
    assert_eq!(current.entries[0].version, initial.entries[0].version);
    assert_eq!(current.fingerprint_records, initial.fingerprint_records);
}

#[tokio::test]
async fn builtin_repositories_reuse_canonical_sources_and_roll_back_on_conflict() {
    let directory = TestDirectory::new();
    let store = StateStoreHandle::open(directory.database()).expect("open store");
    let owner = PrincipalId::local_owner();
    let mut existing = repository(1, 10);
    existing.name = Some("Existing source".to_owned());
    let existing = store
        .register_repository(owner.clone(), existing, key("existing-repository"), 10)
        .await
        .expect("register source already owned by the caller")
        .value;
    let builtin_id = RepositoryId::new();
    let defaults = vec![
        (RepositoryId::new(), repository(1, 0)),
        (builtin_id, repository(2, 0)),
    ];

    let added = store
        .ensure_builtin_repositories(owner.clone(), defaults.clone(), 20)
        .await
        .expect("register missing default source");
    assert_eq!(added[0], existing);
    assert_eq!(added[1].repository_id, builtin_id);
    assert_eq!(added[1].observation.refreshed_at_ms, 0);
    assert_eq!(added[1].registered_at_ms, 20);
    let replay = store
        .ensure_builtin_repositories(owner.clone(), defaults, 30)
        .await
        .expect("idempotent startup");
    assert_eq!(replay, added);

    let rolled_back_id = RepositoryId::new();
    store
        .ensure_builtin_repositories(
            owner.clone(),
            vec![
                (rolled_back_id, repository(3, 0)),
                (builtin_id, repository(4, 0)),
            ],
            40,
        )
        .await
        .expect_err("a different canonical source cannot take the existing stable ID");
    let missing = store
        .get_repository(owner.clone(), rolled_back_id)
        .await
        .expect_err("the whole seeding transaction was rolled back");
    assert_eq!(missing.code(), M3ErrorCode::RepositoryNotRegistered);
    let events = store
        .list_events(owner, 0, 100)
        .await
        .expect("read persisted registrations")
        .events;
    assert_eq!(events.len(), 2);
    assert_eq!(events[1].kind, "repository.registered");
    assert_eq!(events[1].aggregate_id, builtin_id.to_string());
}
