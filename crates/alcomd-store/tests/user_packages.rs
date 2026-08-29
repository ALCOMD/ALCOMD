use alcomd_application::{
    IdempotencyKey, M4Store, PrincipalId, ResolverCatalogSource, Revision, StateStore,
    UserPackageErrorCode, UserPackageSnapshot, UserPackageStore,
};
use alcomd_store::StateStoreHandle;

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
            .resolver_catalog(owner.clone(), false)
            .await
            .expect("v1 catalog")
            .entries
            .is_empty(),
        "v1 resolver must exclude User Packages"
    );
    let v2_catalog = store
        .resolver_catalog(owner.clone(), true)
        .await
        .expect("v2 catalog");
    assert_eq!(v2_catalog.entries.len(), 1);
    assert!(matches!(
        v2_catalog.entries[0].source,
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
            .map(|event| event.kind.as_str())
            .collect::<Vec<_>>(),
        [
            "user_package.enrolled",
            "user_package.refreshed",
            "user_package.removed"
        ]
    );
    assert!(events.events.iter().all(
        |event| event.aggregate_kind == "user-package" && event.aggregate_id == id.to_string()
    ));
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
