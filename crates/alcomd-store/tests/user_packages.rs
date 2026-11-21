use alcomd_application::{
    IdempotencyKey, M4Store, PrincipalId, ResolverCatalogSource, Revision, StateStore,
    UserPackageErrorCode, UserPackageSnapshot, UserPackageStore,
};
use alcomd_store::StateStoreHandle;

#[tokio::test]
async fn shared_catalog_keeps_actor_receipts_and_rechecks_revocation_before_replay_or_write() {
    use alcomd_application::{
        AccessContext, McpIdentityApplication, McpIdentityWrite, ToolAuthentication,
        with_log_caller, with_workflow_authentication,
    };

    let root = temporary_root("authenticated-catalog");
    std::fs::create_dir_all(&root).expect("isolated directory");
    let path = root.join("state.db");
    let store = StateStoreHandle::open(path.clone()).expect("store");
    let identities = McpIdentityApplication::new(store.clone());
    let owner = AccessContext::local_owner();
    let client = identities
        .write(
            &owner,
            "create".into(),
            McpIdentityWrite::Create {
                name: "User package manager".into(),
            },
            1,
        )
        .await
        .expect("create client")
        .client;
    let hash = [7; 32];
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
    let key = IdempotencyKey::parse("user-package-enroll").expect("key");
    let refresh = IdempotencyKey::parse("user-package-refresh").expect("key");
    let remove = IdempotencyKey::parse("user-package-remove").expect("key");
    let source = snapshot("1.0.0", b"first");
    let source_path = source.source_root_path.clone();
    let authentication = ToolAuthentication::Credential(hash);
    let enrolled = with_workflow_authentication(
        authentication.clone(),
        with_log_caller(store.clone(), &access, async {
            store
                .enroll_user_package(caller.clone(), source.clone(), key.clone(), 3)
                .await
                .expect("MCP stores a shared package")
        }),
    )
    .await;
    let id = enrolled.user_package.user_package_id;
    let revision = enrolled.user_package.revision;
    assert_eq!(enrolled.user_package.owner, *owner.principal());
    assert_eq!(
        store
            .get_user_package(owner.principal().clone(), id)
            .await
            .expect("owner reads MCP enrollment")
            .user_package_id,
        id
    );
    assert!(
        store
            .replay_user_package_enroll(owner.principal().clone(), source_path.clone(), key.clone())
            .await
            .expect("owner has no receipt")
            .is_none()
    );
    with_workflow_authentication(
        authentication.clone(),
        with_log_caller(store.clone(), &access, async {
            assert!(
                store
                    .replay_user_package_enroll(caller.clone(), source_path.clone(), key.clone())
                    .await
                    .expect("read own enrollment")
                    .expect("receipt")
                    .replayed
            );
            let refreshed = store
                .refresh_user_package(
                    caller.clone(),
                    id,
                    revision,
                    snapshot("1.1.0", b"updated"),
                    refresh.clone(),
                    4,
                )
                .await
                .expect("refresh");
            store
                .remove_user_package(
                    caller.clone(),
                    id,
                    refreshed.user_package.revision,
                    remove.clone(),
                    5,
                )
                .await
                .expect("remove");
        }),
    )
    .await;
    identities
        .write(
            &owner,
            "revoke".into(),
            McpIdentityWrite::Revoke {
                id: issued.client_id,
                expected: issued.revision,
            },
            6,
        )
        .await
        .expect("revoke");
    // Simulate an already authenticated request waiting for its SQLite worker.
    with_workflow_authentication(authentication, async {
        let denied = UserPackageErrorCode::PermissionDenied;
        assert_eq!(
            store
                .list_user_packages(caller.clone(), None, 20)
                .await
                .expect_err("list")
                .code(),
            denied
        );
        assert_eq!(
            store
                .get_user_package(caller.clone(), id)
                .await
                .expect_err("read")
                .code(),
            denied
        );
        assert_eq!(
            store
                .replay_user_package_enroll(caller.clone(), source_path, key.clone())
                .await
                .expect_err("early enrollment replay")
                .code(),
            denied
        );
        assert_eq!(
            store
                .replay_user_package_refresh(caller.clone(), id, revision, refresh.clone())
                .await
                .expect_err("early refresh replay")
                .code(),
            denied
        );
        assert_eq!(
            store
                .enroll_user_package(caller.clone(), source.clone(), key, 7)
                .await
                .expect_err("final enroll replay")
                .code(),
            denied
        );
        assert_eq!(
            store
                .refresh_user_package(caller.clone(), id, revision, source.clone(), refresh, 7)
                .await
                .expect_err("final refresh replay")
                .code(),
            denied
        );
        assert_eq!(
            store
                .remove_user_package(caller.clone(), id, revision, remove, 7)
                .await
                .expect_err("remove replay")
                .code(),
            denied
        );
        let new_key = IdempotencyKey::parse("after-revoke").expect("key");
        assert_eq!(
            store
                .enroll_user_package(caller.clone(), source, new_key, 7)
                .await
                .expect_err("new enrollment")
                .code(),
            denied
        );
    })
    .await;
    let events = store
        .list_events(owner.principal().clone(), 0, 100)
        .await
        .expect("read shared package events")
        .events;
    for kind in [
        "user_package.enrolled",
        "user_package.refreshed",
        "user_package.removed",
    ] {
        assert!(
            events
                .iter()
                .any(|event| event.kind == kind && event.aggregate_id == id.to_string())
        );
    }
    let db =
        rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .expect("inspect isolated audit");
    for (query, count) in [
        (
            "SELECT COUNT(*) FROM idempotency_records WHERE method LIKE 'packages.userPackages.%' AND principal_id=?1",
            3,
        ),
        (
            "SELECT COUNT(*) FROM operation_log_entries WHERE action LIKE 'packages.userPackages.%' AND owner=?1",
            3,
        ),
    ] {
        assert_eq!(
            db.query_row(query, [caller.as_str()], |row| row.get::<_, i64>(0))
                .expect("actor attribution"),
            count
        );
        assert_eq!(
            db.query_row(query, [owner.principal().as_str()], |row| row
                .get::<_, i64>(0))
                .expect("local owner attribution"),
            0
        );
    }
    drop(db);
    drop(identities);
    drop(store);
    std::fs::remove_dir_all(root).expect("cleanup");
}

fn temporary_root(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "alcomd-user-package-store-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ))
}

fn snapshot(version: &str, content: &[u8]) -> UserPackageSnapshot {
    let manifest_json = format!(
        "{{\"name\":\"com.example.local\",\"version\":\"{version}\",\"vpmDependencies\":{{}}}}"
    );
    UserPackageSnapshot {
        source_root_path: "C:/fixture/local".to_owned(),
        source_identity_key: vec![1, 2, 3],
        package_id: "com.example.local".to_owned(),
        version: version.to_owned(),
        display_name: None,
        dependencies_json: "{}".to_owned(),
        manifest_fingerprint: [content.first().copied().unwrap_or(1); 32],
        manifest_json,
        content_fingerprint: [content.first().copied().unwrap_or(2); 32],
        archive_sha256: [content.last().copied().unwrap_or(3); 32],
    }
}

#[tokio::test]
async fn enrollment_refresh_noop_change_and_remove_are_durable() {
    let root = temporary_root("lifecycle");
    std::fs::create_dir_all(&root).expect("root");
    let store = StateStoreHandle::open(root.join("state.db")).expect("store");
    let owner = PrincipalId::local_owner();
    let enrolled = store
        .enroll_user_package(
            owner.clone(),
            snapshot("1.0.0", b"one"),
            IdempotencyKey::parse("enroll").expect("key"),
            10,
        )
        .await
        .expect("enroll");
    assert_eq!(enrolled.user_package.revision, Revision::INITIAL);
    let id = enrolled.user_package.user_package_id;
    let replay = store
        .replay_user_package_enroll(
            owner.clone(),
            "C:/fixture/local".to_owned(),
            IdempotencyKey::parse("enroll").expect("key"),
        )
        .await
        .expect("replay lookup")
        .expect("replay response");
    assert!(replay.replayed);
    assert_eq!(replay.user_package.user_package_id, id);
    let no_op = store
        .refresh_user_package(
            owner.clone(),
            id,
            Revision::INITIAL,
            snapshot("1.0.0", b"one"),
            IdempotencyKey::parse("refresh-noop").expect("key"),
            20,
        )
        .await
        .expect("no-op");
    assert_eq!(no_op.user_package.revision, Revision::INITIAL);
    assert_eq!(no_op.user_package.updated_at_ms, 10);
    let changed = store
        .refresh_user_package(
            owner.clone(),
            id,
            Revision::INITIAL,
            snapshot("2.0.0", b"two"),
            IdempotencyKey::parse("refresh-change").expect("key"),
            30,
        )
        .await
        .expect("changed");
    assert_eq!(changed.user_package.revision.get(), 2);
    assert_eq!(changed.user_package.version, "2.0.0");
    assert!(
        store
            .resolver_catalog(
                owner.clone(),
                false,
                alcomd_application::ConfigSnapshot::default()
                    .settings
                    .packages
            )
            .await
            .expect("repository-only lookup")
            .entries
            .is_empty(),
        "archive pin lookup scans repository artifacts"
    );
    let catalog = store
        .resolver_catalog(
            owner.clone(),
            true,
            alcomd_application::ConfigSnapshot::default()
                .settings
                .packages,
        )
        .await
        .expect("package catalog");
    assert_eq!(catalog.entries.len(), 1);
    assert!(matches!(
        catalog.entries[0].source,
        ResolverCatalogSource::UserPackage { .. }
    ));
    drop(store);

    let store = StateStoreHandle::open(root.join("state.db")).expect("reopen");
    assert_eq!(
        store
            .get_user_package(owner.clone(), id)
            .await
            .expect("persisted")
            .revision
            .get(),
        2
    );
    let removed = store
        .remove_user_package(
            owner.clone(),
            id,
            Revision::new(2).expect("revision"),
            IdempotencyKey::parse("remove").expect("key"),
            40,
        )
        .await
        .expect("remove");
    assert_eq!(removed.revision.get(), 3);
    let missing = store
        .get_user_package(owner, id)
        .await
        .expect_err("removed");
    assert_eq!(missing.code(), UserPackageErrorCode::NotFound);
    let events = store
        .list_events(PrincipalId::local_owner(), 0, 100)
        .await
        .expect("events");
    assert_eq!(
        events
            .events
            .iter()
            .filter(|event| event.aggregate_kind == "user-package")
            .map(|event| event.kind.as_str())
            .collect::<Vec<_>>(),
        [
            "user_package.enrolled",
            "user_package.refreshed",
            "user_package.removed"
        ]
    );
    assert_eq!(events.events.len(), 3);
    assert!(
        events
            .events
            .iter()
            .filter(|event| event.aggregate_kind == "user-package")
            .all(|event| event.aggregate_kind == "user-package"
                && event.aggregate_id == id.to_string())
    );
    drop(store);
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[tokio::test]
async fn duplicate_identity_or_package_id_is_rejected() {
    let root = temporary_root("duplicates");
    std::fs::create_dir_all(&root).expect("root");
    let store = StateStoreHandle::open(root.join("state.db")).expect("store");
    let owner = PrincipalId::local_owner();
    store
        .enroll_user_package(
            owner.clone(),
            snapshot("1.0.0", b"one"),
            IdempotencyKey::parse("first").expect("key"),
            10,
        )
        .await
        .expect("first");
    let duplicate = store
        .enroll_user_package(
            owner,
            snapshot("2.0.0", b"two"),
            IdempotencyKey::parse("second").expect("key"),
            20,
        )
        .await
        .expect_err("duplicate");
    assert_eq!(duplicate.code(), UserPackageErrorCode::AlreadyEnrolled);
    drop(store);
    std::fs::remove_dir_all(root).expect("cleanup");
}
