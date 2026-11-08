use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use alcomd_application::{
    ExtensionDataDisposition, ExtensionGrantMutation, ExtensionInstallPlanDraft,
    ExtensionPackageEvidence, ExtensionSourceKind, ExtensionTrustDecision, ExtensionUiProtocol,
    ExtensionUninstallPlanDraft, IdempotencyKey, M3RegistryStore, M6ErrorCode, M6Store,
    ManifestState, PrincipalId, ProjectObservation, ProjectType,
};
use alcomd_domain::{ProjectId, Revision};
use alcomd_store::StateStoreHandle;
use rusqlite::Connection;

static NEXT_DATABASE: AtomicU64 = AtomicU64::new(0);

fn database_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "alcomd-m6-store-{name}-{}-{}.db",
        std::process::id(),
        NEXT_DATABASE.fetch_add(1, Ordering::Relaxed)
    ))
}

fn evidence() -> ExtensionPackageEvidence {
    ExtensionPackageEvidence {
        source_kind: ExtensionSourceKind::LocalOwnerSelected,
        source_locator: "C:/fixture/extension.alcomdext".to_owned(),
        source_identity: vec![7; 24],
        extension_id: "dev.example.fixture".to_owned(),
        version: "1.2.3-alpha.1+fixture".to_owned(),
        api_major: 1,
        profile_version: 1,
        package_digest: [1; 32],
        manifest_digest: [2; 32],
        component_digest: [3; 32],
        publisher_fingerprint: format!("ed25519-sha256:{}", "a".repeat(64)),
        required_permissions: vec!["background.run".to_owned()],
        optional_permissions: vec!["projects.read".to_owned()],
        required_interfaces: vec!["alcomd:extension/host-data@1.0.0".to_owned()],
        optional_interfaces: vec!["alcomd:extension/host-projects@1.0.0".to_owned()],
        ui_protocol: Some(ExtensionUiProtocol::PortableV1),
    }
}

fn cleanup(path: &Path) {
    for candidate in [
        path.to_path_buf(),
        path.with_extension("db-shm"),
        path.with_extension("db-wal"),
    ] {
        let _ = std::fs::remove_file(candidate);
    }
}

async fn permissions_fixture(name: &str) -> (StateStoreHandle, PathBuf, ExtensionPackageEvidence) {
    permissions_fixture_with_package(name, evidence()).await
}

async fn permissions_fixture_with_package(
    name: &str,
    package: ExtensionPackageEvidence,
) -> (StateStoreHandle, PathBuf, ExtensionPackageEvidence) {
    let path = database_path(name);
    let store = StateStoreHandle::open(path.clone()).expect("permissions store");
    let owner = PrincipalId::local_owner();
    let draft = |package| ExtensionInstallPlanDraft {
        evidence: package,
        trust_decision: ExtensionTrustDecision::UserApprovedForExtension,
        expected_revision: None,
        plan_fingerprint: [8; 32],
    };
    let plan = store
        .create_install_plan(owner.clone(), draft(package.clone()), 1_000)
        .await
        .expect("frozen install plan");
    let mut uninstalled = package.clone();
    uninstalled.required_permissions.clear();
    uninstalled.optional_permissions.clear();
    store
        .create_install_plan(owner.clone(), draft(uninstalled), 1_001)
        .await
        .expect("unapplied plan must not replace declarations");
    let accepted = store
        .accept_plan(
            owner,
            plan.plan_id,
            IdempotencyKey::parse("permissions-install").expect("key"),
            1_002,
        )
        .await
        .expect("accept install");
    store
        .begin_apply(accepted.operation_id, 1_003)
        .await
        .expect("begin install");
    store
        .finish_install(
            accepted.operation_id,
            "<EXTENSION_ROOT>/permissions".to_owned(),
            1_004,
        )
        .await
        .expect("finish install");
    store
        .complete_operation(accepted.operation_id, 1_005)
        .await
        .expect("complete install");
    (store, path, package)
}

async fn permission_project(store: &StateStoreHandle, index: u8) -> ProjectId {
    store
        .register_project(
            PrincipalId::local_owner(),
            ProjectObservation {
                package_presentations: Default::default(),
                root_path: format!("<PROJECT_ROOT>/permissions-{index}"),
                path_identity_key: vec![index],
                project_type: ProjectType::Avatars,
                unity_version: "2022.3.22f1".to_owned(),
                unity_revision: None,
                vpm_manifest: ManifestState::Valid,
                upm_manifest: ManifestState::Valid,
                direct_dependencies: vec![],
                locked_dependencies: vec![],
                issues: vec![],
            },
            IdempotencyKey::parse(format!("permissions-project-{index}")).expect("project key"),
            1_010 + u64::from(index),
        )
        .await
        .expect("scope project")
        .value
        .project_id
}

async fn permission_mutation(
    store: &StateStoreHandle,
    extension_id: &str,
    project: Option<ProjectId>,
    grant: bool,
    key: &str,
) {
    let owner = PrincipalId::local_owner();
    let record = store
        .get_extension(owner.clone(), extension_id.to_owned())
        .await
        .expect("current grant revision");
    store
        .set_grant(
            owner,
            ExtensionGrantMutation {
                extension_id: extension_id.to_owned(),
                permission: if project.is_some() {
                    "projects.read"
                } else {
                    "background.run"
                }
                .to_owned(),
                resource_kind: if project.is_some() {
                    "Project"
                } else {
                    "Extension"
                }
                .to_owned(),
                resource_id: project.map_or_else(|| extension_id.to_owned(), |id| id.to_string()),
                expected_revision: record.grant_revision,
                grant,
            },
            IdempotencyKey::parse(key).expect("grant key"),
            1_020,
        )
        .await
        .expect("explicit grant mutation");
}

fn enable_state(path: &Path) -> Vec<Vec<Vec<rusqlite::types::Value>>> {
    let connection = Connection::open(path).expect("own enable verification database");
    [
        "extensions",
        "extension_instances",
        "extension_grants",
        "idempotency_records",
    ]
    .into_iter()
    .map(|table| {
        let mut statement = connection
            .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
            .expect("own fixed test table");
        let columns = statement.column_count();
        statement
            .query_map([], |row| {
                (0..columns)
                    .map(|column| row.get(column))
                    .collect::<Result<Vec<_>, _>>()
            })
            .expect("read verification rows")
            .collect::<Result<Vec<_>, _>>()
            .expect("verification rows")
    })
    .collect()
}

#[tokio::test]
async fn required_background_enable_and_recovery_refuse_without_side_effects_or_consuming_key() {
    let (store, path, package) = permissions_fixture("required-enable").await;
    let owner = PrincipalId::local_owner();
    let initial = store
        .get_extension(owner.clone(), package.extension_id.clone())
        .await
        .expect("initial record");
    let before = enable_state(&path);
    let key = IdempotencyKey::parse("required-gate-key").expect("key");
    let denied = store
        .enable(
            owner.clone(),
            package.extension_id.clone(),
            initial.revision,
            key.clone(),
            1_030,
        )
        .await
        .expect_err("required background without a grant must refuse enable");
    assert_eq!(denied.code(), M6ErrorCode::PermissionDenied);
    assert_eq!(
        enable_state(&path),
        before,
        "denial must not change registry, instances, grants or idempotency"
    );
    permission_mutation(
        &store,
        &package.extension_id,
        None,
        true,
        "required-background-first-grant",
    )
    .await;
    permission_mutation(
        &store,
        &package.extension_id,
        None,
        false,
        "required-background-revoke",
    )
    .await;
    let connection = Connection::open(&path).expect("own quarantine fixture");
    connection.execute("UPDATE extensions SET desired_state='enabled', quarantine_state='quarantined', revision=revision+1 WHERE extension_id=?1", [&package.extension_id]).expect("represent a quarantined fixture");
    drop(connection);
    let quarantined = store
        .get_extension(owner.clone(), package.extension_id.clone())
        .await
        .expect("quarantined record");
    let before_recovery = enable_state(&path);
    let denied = store
        .enable(
            owner.clone(),
            package.extension_id.clone(),
            quarantined.revision,
            IdempotencyKey::parse("required-recovery-key").expect("recovery key"),
            1_031,
        )
        .await
        .expect_err("revoked required grant must not clear quarantine");
    assert_eq!(denied.code(), M6ErrorCode::PermissionDenied);
    assert_eq!(
        enable_state(&path),
        before_recovery,
        "refusal must preserve quarantine, revisions, timestamps and idempotency"
    );
    permission_mutation(
        &store,
        &package.extension_id,
        None,
        true,
        "required-background-final-grant",
    )
    .await;
    let granted = store
        .get_extension(owner.clone(), package.extension_id.clone())
        .await
        .expect("explicitly granted record");
    let enabled = store
        .enable(
            owner.clone(),
            package.extension_id.clone(),
            granted.revision,
            key.clone(),
            1_032,
        )
        .await
        .expect("denied request did not consume its key");
    assert_eq!(
        enabled.desired_state,
        alcomd_application::ExtensionDesiredState::Enabled
    );
    assert_eq!(
        enabled.quarantine_state,
        alcomd_application::ExtensionQuarantineState::Clear
    );
    let replay = store
        .enable(owner, package.extension_id, granted.revision, key, 1_033)
        .await
        .expect("existing successful exact replay remains valid");
    assert_eq!(replay, enabled);
    drop(store);
    cleanup(&path);
}

#[tokio::test]
async fn required_empty_ui_only_and_optional_permissions_enable_without_implicit_grants() {
    for optional in [
        vec![],
        vec!["background.run".to_owned(), "projects.read".to_owned()],
    ] {
        let mut package = evidence();
        package.required_permissions.clear();
        package.optional_permissions = optional;
        let (store, path, package) =
            permissions_fixture_with_package("required-empty-ui", package).await;
        let owner = PrincipalId::local_owner();
        let before = store
            .get_extension(owner.clone(), package.extension_id.clone())
            .await
            .expect("UI-only record");
        let enabled = store
            .enable(
                owner.clone(),
                package.extension_id.clone(),
                before.revision,
                IdempotencyKey::parse("required-empty-enable").expect("key"),
                1_030,
            )
            .await
            .expect("optional permissions do not block UI-only enable");
        assert_eq!(
            enabled.desired_state,
            alcomd_application::ExtensionDesiredState::Enabled
        );
        let permissions = store
            .list_permissions(owner, package.extension_id, None, 100)
            .await
            .expect("current grants");
        assert!(
            permissions.grants.is_empty(),
            "enable cannot grant optional permissions"
        );
        drop(store);
        cleanup(&path);
    }
}

#[tokio::test]
async fn required_project_permission_needs_an_active_explicit_project_scope() {
    let mut package = evidence();
    package.required_permissions = vec!["projects.read".to_owned()];
    package.optional_permissions = vec!["background.run".to_owned()];
    let (store, path, package) =
        permissions_fixture_with_package("required-project", package).await;
    let owner = PrincipalId::local_owner();
    let before = store
        .get_extension(owner.clone(), package.extension_id.clone())
        .await
        .expect("project reader record");
    let snapshot = enable_state(&path);
    let denied = store
        .enable(
            owner.clone(),
            package.extension_id.clone(),
            before.revision,
            IdempotencyKey::parse("required-project-enable").expect("key"),
            1_030,
        )
        .await
        .expect_err("required project permission cannot be bypassed by lack of background lease");
    assert_eq!(denied.code(), M6ErrorCode::PermissionDenied);
    assert_eq!(enable_state(&path), snapshot);
    let project = permission_project(&store, 11).await;
    permission_mutation(
        &store,
        &package.extension_id,
        Some(project),
        true,
        "required-project-grant",
    )
    .await;
    let granted = store
        .get_extension(owner.clone(), package.extension_id.clone())
        .await
        .expect("granted project record");
    store
        .enable(
            owner.clone(),
            package.extension_id.clone(),
            granted.revision,
            IdempotencyKey::parse("required-project-enable").expect("reused denied key"),
            1_031,
        )
        .await
        .expect("explicit project grant satisfies required permission");
    let permissions = store
        .list_permissions(owner, package.extension_id, None, 100)
        .await
        .expect("exact current scope");
    assert_eq!(permissions.grants.len(), 1);
    assert_eq!(permissions.grants[0].resource_id, project.to_string());
    assert_eq!(permissions.grants[0].permission, "projects.read");
    drop(store);
    cleanup(&path);
}

#[tokio::test]
async fn required_current_package_permission_grant_rejects_historical_declarations() {
    let (store, path, old_package) = permissions_fixture("required-current-package").await;
    let owner = PrincipalId::local_owner();
    permission_mutation(
        &store,
        &old_package.extension_id,
        None,
        true,
        "historical-background-grant",
    )
    .await;
    let old_record = store
        .get_extension(owner.clone(), old_package.extension_id.clone())
        .await
        .expect("old package record");
    let uninstall = store
        .create_uninstall_plan(
            owner.clone(),
            ExtensionUninstallPlanDraft {
                extension: old_record,
                data_disposition: ExtensionDataDisposition::RetainData,
                plan_fingerprint: [18; 32],
            },
            2_000,
        )
        .await
        .expect("old package uninstall plan");
    let accepted = store
        .accept_plan(
            owner.clone(),
            uninstall.plan_id,
            IdempotencyKey::parse("historical-uninstall").expect("key"),
            2_001,
        )
        .await
        .expect("accept uninstall");
    store
        .begin_apply(accepted.operation_id, 2_002)
        .await
        .expect("begin uninstall");
    store
        .finish_uninstall(accepted.operation_id, 2_003)
        .await
        .expect("uninstall registry");
    store
        .complete_operation(accepted.operation_id, 2_004)
        .await
        .expect("complete uninstall");
    let mut package = old_package;
    package.version = "2.0.0".to_owned();
    package.package_digest = [21; 32];
    package.manifest_digest = [22; 32];
    package.required_permissions.clear();
    package.optional_permissions = vec!["projects.read".to_owned()];
    let install = store
        .create_install_plan(
            owner.clone(),
            ExtensionInstallPlanDraft {
                evidence: package.clone(),
                trust_decision: ExtensionTrustDecision::UserApprovedForExtension,
                expected_revision: None,
                plan_fingerprint: [23; 32],
            },
            2_005,
        )
        .await
        .expect("new package install plan");
    let accepted = store
        .accept_plan(
            owner.clone(),
            install.plan_id,
            IdempotencyKey::parse("current-package-install").expect("key"),
            2_006,
        )
        .await
        .expect("accept new package");
    store
        .begin_apply(accepted.operation_id, 2_007)
        .await
        .expect("begin new package");
    store
        .finish_install(
            accepted.operation_id,
            "<EXTENSION_ROOT>/current-package".to_owned(),
            2_008,
        )
        .await
        .expect("new package state");
    store
        .complete_operation(accepted.operation_id, 2_009)
        .await
        .expect("complete new package");
    let current = store
        .get_extension(owner.clone(), package.extension_id.clone())
        .await
        .expect("new installed record");
    let snapshot = enable_state(&path);
    let key = IdempotencyKey::parse("current-package-grant-key").expect("grant key");
    let refused = store
        .set_grant(
            owner.clone(),
            ExtensionGrantMutation {
                extension_id: package.extension_id.clone(),
                permission: "background.run".to_owned(),
                resource_kind: "Extension".to_owned(),
                resource_id: package.extension_id.clone(),
                expected_revision: current.grant_revision,
                grant: true,
            },
            key.clone(),
            2_010,
        )
        .await
        .expect_err(
            "old applied Plan must not authorize an undeclared permission on the current package",
        );
    assert_eq!(refused.code(), M6ErrorCode::PermissionDenied);
    assert_eq!(
        enable_state(&path),
        snapshot,
        "refusal must preserve grants, revisions, instances and idempotency"
    );
    let project = permission_project(&store, 12).await;
    let granted = store.set_grant(owner.clone(), ExtensionGrantMutation {
        extension_id: package.extension_id.clone(), permission: "projects.read".to_owned(), resource_kind: "Project".to_owned(),
        resource_id: project.to_string(), expected_revision: current.grant_revision, grant: true,
    }, key, 2_011).await.expect("rejected permission did not consume key; current declared project scope can be explicitly granted");
    assert_eq!(granted.permission, "projects.read");
    let permissions = store
        .list_permissions(owner, package.extension_id, None, 100)
        .await
        .expect("current permission evidence");
    assert!(permissions.required_permissions.is_empty());
    assert_eq!(
        permissions.optional_permissions,
        vec!["projects.read".to_owned()]
    );
    assert_eq!(
        permissions.grants.len(),
        1,
        "old background grant must not survive reinstall or be recreated"
    );
    assert_eq!(permissions.grants[0].resource_id, project.to_string());
    drop(store);
    cleanup(&path);
}

#[tokio::test]
async fn permissions_read_frozen_declarations_and_only_active_exact_project_scopes() {
    let (store, path, package) = permissions_fixture("permissions-evidence").await;
    let owner = PrincipalId::local_owner();
    let initial = store
        .list_permissions(owner.clone(), package.extension_id.clone(), None, 100)
        .await
        .expect("no implicit grants");
    assert_eq!(initial.required_permissions, package.required_permissions);
    assert_eq!(initial.optional_permissions, package.optional_permissions);
    assert!(initial.grants.is_empty());
    assert!(initial.next_cursor.is_none());
    let project = permission_project(&store, 1).await;
    permission_mutation(
        &store,
        &package.extension_id,
        Some(project),
        true,
        "permissions-add",
    )
    .await;
    let partial = store
        .list_permissions(owner.clone(), package.extension_id.clone(), None, 100)
        .await
        .expect("partial permissions");
    assert_eq!(partial.grants.len(), 1);
    assert_eq!(partial.grants[0].permission, "projects.read");
    assert_eq!(partial.grants[0].resource_kind, "Project");
    assert_eq!(partial.grants[0].resource_id, project.to_string());
    assert_eq!(partial.required_permissions, package.required_permissions);
    permission_mutation(
        &store,
        &package.extension_id,
        Some(project),
        false,
        "permissions-revoke",
    )
    .await;
    let revoked = store
        .list_permissions(owner, package.extension_id.clone(), None, 100)
        .await
        .expect("revoked grant not public");
    assert!(revoked.grants.is_empty());
    assert!(revoked.grant_revision > partial.grant_revision);
    let connection = Connection::open(&path).expect("own test store");
    assert!(
        connection
            .execute(
                "UPDATE extension_plans SET requested_permissions_json='{}'",
                []
            )
            .is_err(),
        "frozen evidence stays immutable"
    );
    drop(connection);
    drop(store);
    cleanup(&path);
}

#[tokio::test]
async fn permissions_keyset_pages_reject_revocation_and_package_snapshot_changes() {
    let (store, path, package) = permissions_fixture("permissions-pages").await;
    let owner = PrincipalId::local_owner();
    permission_mutation(
        &store,
        &package.extension_id,
        None,
        true,
        "permissions-background",
    )
    .await;
    let mut projects = Vec::new();
    for index in 2..5 {
        let project = permission_project(&store, index).await;
        permission_mutation(
            &store,
            &package.extension_id,
            Some(project),
            true,
            &format!("permissions-grant-{index}"),
        )
        .await;
        projects.push(project);
    }
    let first = store
        .list_permissions(owner.clone(), package.extension_id.clone(), None, 1)
        .await
        .expect("first page");
    assert_eq!(first.grants[0].permission, "background.run");
    let second = store
        .list_permissions(
            owner.clone(),
            package.extension_id.clone(),
            first.next_cursor.clone(),
            2,
        )
        .await
        .expect("second page");
    let third = store
        .list_permissions(
            owner.clone(),
            package.extension_id.clone(),
            second.next_cursor,
            2,
        )
        .await
        .expect("last page");
    let all = first
        .grants
        .iter()
        .chain(second.grants.iter())
        .chain(third.grants.iter())
        .map(|grant| (&grant.permission, &grant.resource_kind, &grant.resource_id))
        .collect::<Vec<_>>();
    assert_eq!(all.len(), 4);
    assert!(all.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(third.next_cursor.is_none());
    permission_mutation(
        &store,
        &package.extension_id,
        Some(projects[0]),
        false,
        "permissions-page-revoke",
    )
    .await;
    assert_eq!(
        store
            .list_permissions(
                owner.clone(),
                package.extension_id.clone(),
                first.next_cursor,
                100
            )
            .await
            .expect_err("cannot combine grant revisions")
            .code(),
        M6ErrorCode::RevisionConflict
    );
    let fresh = store
        .list_permissions(owner.clone(), package.extension_id.clone(), None, 1)
        .await
        .expect("fresh snapshot");
    let connection = Connection::open(&path).expect("own test store");
    connection
        .execute(
            "UPDATE extensions SET package_digest=?1 WHERE extension_id=?2",
            rusqlite::params![[9_u8; 32].as_slice(), package.extension_id],
        )
        .expect("simulate changed package identity without grant change");
    drop(connection);
    assert_eq!(
        store
            .list_permissions(owner, package.extension_id, fresh.next_cursor, 100)
            .await
            .expect_err("package change invalidates cursor")
            .code(),
        M6ErrorCode::RevisionConflict
    );
    drop(store);
    cleanup(&path);
}

#[tokio::test]
async fn permissions_read_is_owner_bound_and_rejects_foreign_or_invalid_pages() {
    let (store, path, package) = permissions_fixture("permissions-owner").await;
    let foreign = PrincipalId::parse("fixture:other-owner").expect("other principal");
    assert_eq!(
        store
            .list_permissions(foreign, package.extension_id.clone(), None, 100)
            .await
            .expect_err("other owner cannot inspect")
            .code(),
        M6ErrorCode::NotInstalled
    );
    for limit in [0, 1_001] {
        assert_eq!(
            store
                .list_permissions(
                    PrincipalId::local_owner(),
                    package.extension_id.clone(),
                    None,
                    limit
                )
                .await
                .expect_err("invalid limit")
                .code(),
            M6ErrorCode::InvalidInput
        );
    }
    permission_mutation(
        &store,
        &package.extension_id,
        None,
        true,
        "permissions-owner-background",
    )
    .await;
    let project = permission_project(&store, 7).await;
    permission_mutation(
        &store,
        &package.extension_id,
        Some(project),
        true,
        "permissions-owner-project",
    )
    .await;
    let first = store
        .list_permissions(
            PrincipalId::local_owner(),
            package.extension_id.clone(),
            None,
            1,
        )
        .await
        .expect("cursor");
    let mut cursor = first.next_cursor.expect("next cursor");
    cursor.extension_id = "dev.example.other".to_owned();
    assert_eq!(
        store
            .list_permissions(
                PrincipalId::local_owner(),
                package.extension_id,
                Some(cursor),
                100
            )
            .await
            .expect_err("foreign query cursor")
            .code(),
        M6ErrorCode::InvalidInput
    );
    drop(store);
    cleanup(&path);
}

#[tokio::test]
async fn plan_authority_round_trips_and_install_recovery_is_idempotent() {
    let path = database_path("plan");
    let store = StateStoreHandle::open(path.clone()).expect("open store");
    let owner = PrincipalId::local_owner();
    let package = evidence();
    let plan = store
        .create_install_plan(
            owner.clone(),
            ExtensionInstallPlanDraft {
                evidence: package.clone(),
                trust_decision: ExtensionTrustDecision::UserApprovedForExtension,
                expected_revision: None,
                plan_fingerprint: [4; 32],
            },
            10,
        )
        .await
        .expect("create install plan");
    assert_eq!(plan.evidence.version, package.version);
    assert_eq!(plan.evidence.api_major, 1);
    assert_eq!(plan.evidence.profile_version, 1);

    let accepted = store
        .accept_plan(
            owner.clone(),
            plan.plan_id,
            IdempotencyKey::parse("install-fixture").expect("idempotency key"),
            11,
        )
        .await
        .expect("accept plan");
    assert!(!accepted.replayed);
    let running = store
        .begin_apply(accepted.operation_id, 12)
        .await
        .expect("begin apply");
    assert_eq!(running.plan_id, plan.plan_id);
    assert_eq!(running.state, "applied");
    assert_eq!(running.apply_operation_id, Some(accepted.operation_id));
    assert_eq!(running.evidence, plan.evidence);
    store
        .finish_install(
            accepted.operation_id,
            "<EXTENSION_ROOT>/fixture".to_owned(),
            13,
        )
        .await
        .expect("commit registry");
    store
        .finish_install(
            accepted.operation_id,
            "<EXTENSION_ROOT>/fixture".to_owned(),
            14,
        )
        .await
        .expect("repeat registry commit after crash");
    store
        .complete_operation(accepted.operation_id, 15)
        .await
        .expect("complete operation");

    let installed = store
        .get_extension(owner.clone(), package.extension_id.clone())
        .await
        .expect("installed extension");
    assert_eq!(installed.version, package.version);
    assert_eq!(installed.package_digest, package.package_digest);

    let duplicate = store
        .create_install_plan(
            owner,
            ExtensionInstallPlanDraft {
                evidence: package,
                trust_decision: ExtensionTrustDecision::UserApprovedForExtension,
                expected_revision: Some(installed.revision),
                plan_fingerprint: [5; 32],
            },
            16,
        )
        .await
        .expect_err("M6 install cannot become an upgrade");
    assert_eq!(duplicate.code(), M6ErrorCode::AlreadyInstalled);
    drop(store);
    cleanup(&path);
}

#[tokio::test]
async fn uninstall_begin_durably_revokes_grants_before_filesystem_mutation() {
    let path = database_path("uninstall-revoke");
    let store = StateStoreHandle::open(path.clone()).expect("open store");
    let owner = PrincipalId::local_owner();
    let package = evidence();
    let plan = store
        .create_install_plan(
            owner.clone(),
            ExtensionInstallPlanDraft {
                evidence: package.clone(),
                trust_decision: ExtensionTrustDecision::UserApprovedForExtension,
                expected_revision: None,
                plan_fingerprint: [6; 32],
            },
            20,
        )
        .await
        .expect("create install plan");
    let accepted = store
        .accept_plan(
            owner.clone(),
            plan.plan_id,
            IdempotencyKey::parse("install-before-uninstall").expect("key"),
            21,
        )
        .await
        .expect("accept install");
    store
        .begin_apply(accepted.operation_id, 22)
        .await
        .expect("begin install");
    store
        .finish_install(
            accepted.operation_id,
            "<EXTENSION_ROOT>/fixture".to_owned(),
            23,
        )
        .await
        .expect("finish install");
    store
        .complete_operation(accepted.operation_id, 24)
        .await
        .expect("complete install");
    let installed = store
        .get_extension(owner.clone(), package.extension_id.clone())
        .await
        .expect("get installed");
    let uninstall = store
        .create_uninstall_plan(
            owner.clone(),
            ExtensionUninstallPlanDraft {
                extension: installed,
                data_disposition: ExtensionDataDisposition::RetainData,
                plan_fingerprint: [7; 32],
            },
            25,
        )
        .await
        .expect("create uninstall plan");
    let accepted = store
        .accept_plan(
            owner,
            uninstall.plan_id,
            IdempotencyKey::parse("uninstall-fixture").expect("key"),
            26,
        )
        .await
        .expect("accept uninstall");
    store
        .begin_apply(accepted.operation_id, 27)
        .await
        .expect("begin uninstall");

    let connection = Connection::open(&path).expect("open read connection");
    let desired: String = connection
        .query_row(
            "SELECT desired_state FROM extensions WHERE extension_id=?1",
            [&package.extension_id],
            |row| row.get(0),
        )
        .expect("read desired state");
    let grants: i64 = connection
        .query_row(
            "SELECT count(*) FROM extension_grants WHERE extension_id=?1",
            [&package.extension_id],
            |row| row.get(0),
        )
        .expect("count grants");
    let phases: i64 = connection
        .query_row(
            "SELECT count(*) FROM extension_filesystem_journal
             WHERE operation_id=?1 AND phase IN ('grants_revoked', 'lease_revoked')
             AND state='completed'",
            [accepted.operation_id.to_string()],
            |row| row.get(0),
        )
        .expect("count revoke phases");
    assert_eq!(desired, "uninstalling");
    assert_eq!(grants, 0);
    assert_eq!(phases, 2);
    drop(connection);
    drop(store);
    cleanup(&path);
}

#[tokio::test]
async fn leases_renew_and_three_crashes_quarantine_without_losing_enabled_intent() {
    let path = database_path("crash-quarantine");
    let store = StateStoreHandle::open(path.clone()).expect("open store");
    let owner = PrincipalId::local_owner();
    let package = evidence();
    let plan = store
        .create_install_plan(
            owner.clone(),
            ExtensionInstallPlanDraft {
                evidence: package.clone(),
                trust_decision: ExtensionTrustDecision::UserApprovedForExtension,
                expected_revision: None,
                plan_fingerprint: [8; 32],
            },
            100,
        )
        .await
        .expect("create install plan");
    let accepted = store
        .accept_plan(
            owner.clone(),
            plan.plan_id,
            IdempotencyKey::parse("install-before-crashes").expect("key"),
            101,
        )
        .await
        .expect("accept install");
    store
        .begin_apply(accepted.operation_id, 102)
        .await
        .expect("begin install");
    store
        .finish_install(
            accepted.operation_id,
            "<EXTENSION_ROOT>/fixture".to_owned(),
            103,
        )
        .await
        .expect("finish install");
    store
        .complete_operation(accepted.operation_id, 104)
        .await
        .expect("complete install");

    let connection = Connection::open(&path).expect("open setup connection");
    connection
        .execute(
            "UPDATE extensions SET desired_state='enabled' WHERE extension_id=?1",
            [&package.extension_id],
        )
        .expect("enable fixture");
    connection
        .execute(
            "INSERT INTO extension_grants
             (extension_id, permission_name, resource_kind, resource_id, state,
              grant_revision, updated_at_ms)
             VALUES (?1, 'background.run', 'Extension', ?1, 'granted', 1, 105)",
            [&package.extension_id],
        )
        .expect("grant background permission");
    drop(connection);

    let mut context = store
        .prepare_instance(
            owner.clone(),
            package.extension_id.clone(),
            "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".to_owned(),
            200,
        )
        .await
        .expect("prepare first instance");
    store
        .mark_instance_running(context.lease.clone(), 201)
        .await
        .expect("mark running");
    context.lease = store
        .renew_instance(context.lease, 30_200)
        .await
        .expect("renew live lease");
    assert_eq!(context.lease.expires_at_ms, 90_200);

    for (crash_index, expected_delay) in [(0_u64, Some(1_000)), (1, Some(5_000)), (2, None)] {
        if crash_index > 0 {
            context = store
                .prepare_instance(
                    owner.clone(),
                    package.extension_id.clone(),
                    "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".to_owned(),
                    31_000 + crash_index,
                )
                .await
                .expect("prepare restarted instance");
            store
                .mark_instance_running(context.lease.clone(), 31_100 + crash_index)
                .await
                .expect("mark restarted instance running");
        }
        let decision = store
            .record_instance_crash(
                context.lease.clone(),
                "host_exited".to_owned(),
                32_000 + crash_index,
            )
            .await
            .expect("record crash");
        assert_eq!(decision.restart_delay_ms, expected_delay);
        assert_eq!(decision.quarantined, crash_index == 2);
    }

    let connection = Connection::open(&path).expect("open verification connection");
    let (desired, quarantine): (String, String) = connection
        .query_row(
            "SELECT desired_state, quarantine_state FROM extensions WHERE extension_id=?1",
            [&package.extension_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read lifecycle states");
    let instances: i64 = connection
        .query_row(
            "SELECT count(*) FROM extension_instances WHERE extension_id=?1",
            [&package.extension_id],
            |row| row.get(0),
        )
        .expect("count current instances");
    let crashes: i64 = connection
        .query_row(
            "SELECT count(*) FROM extension_crashes WHERE extension_id=?1",
            [&package.extension_id],
            |row| row.get(0),
        )
        .expect("count bounded crash evidence");
    assert_eq!(desired, "enabled");
    assert_eq!(quarantine, "quarantined");
    assert_eq!(instances, 0);
    assert_eq!(crashes, 3);
    drop(connection);
    drop(store);
    cleanup(&path);
}

#[tokio::test]
async fn project_scope_data_namespace_and_revoke_revalidate_each_lease_call() {
    let path = database_path("capabilities");
    let store = StateStoreHandle::open(path.clone()).expect("open store");
    let owner = PrincipalId::local_owner();
    let package = evidence();
    let plan = store
        .create_install_plan(
            owner.clone(),
            ExtensionInstallPlanDraft {
                evidence: package.clone(),
                trust_decision: ExtensionTrustDecision::UserApprovedForExtension,
                expected_revision: None,
                plan_fingerprint: [9; 32],
            },
            400,
        )
        .await
        .expect("plan");
    let accepted = store
        .accept_plan(
            owner.clone(),
            plan.plan_id,
            IdempotencyKey::parse("capability-install").expect("key"),
            401,
        )
        .await
        .expect("accept");
    store
        .begin_apply(accepted.operation_id, 402)
        .await
        .expect("begin");
    store
        .finish_install(
            accepted.operation_id,
            "<EXTENSION_ROOT>/capability".to_owned(),
            403,
        )
        .await
        .expect("finish");
    store
        .complete_operation(accepted.operation_id, 404)
        .await
        .expect("complete");
    let project = store
        .register_project(
            owner.clone(),
            ProjectObservation {
                package_presentations: Default::default(),
                root_path: "<PROJECT_ROOT>/fixture".to_owned(),
                path_identity_key: vec![1, 2, 3],
                project_type: ProjectType::Avatars,
                unity_version: "2022.3.22f1".to_owned(),
                unity_revision: None,
                vpm_manifest: ManifestState::Valid,
                upm_manifest: ManifestState::Valid,
                direct_dependencies: Vec::new(),
                locked_dependencies: Vec::new(),
                issues: Vec::new(),
            },
            IdempotencyKey::parse("capability-project").expect("key"),
            405,
        )
        .await
        .expect("register project")
        .value;
    let connection = Connection::open(&path).expect("setup connection");
    connection
        .execute(
            "UPDATE extensions SET desired_state='enabled' WHERE extension_id=?1",
            [&package.extension_id],
        )
        .expect("enable");
    connection
        .execute(
            "INSERT INTO extension_grants
             (extension_id, permission_name, resource_kind, resource_id, state,
              grant_revision, updated_at_ms)
             VALUES (?1, 'background.run', 'Extension', ?1, 'granted', 1, 406)",
            [&package.extension_id],
        )
        .expect("background grant");
    drop(connection);
    let project_grant = store
        .set_grant(
            owner.clone(),
            ExtensionGrantMutation {
                extension_id: package.extension_id.clone(),
                permission: "projects.read".to_owned(),
                resource_kind: "Project".to_owned(),
                resource_id: project.project_id.to_string(),
                expected_revision: Revision::new(1).expect("revision"),
                grant: true,
            },
            IdempotencyKey::parse("capability-project-grant").expect("key"),
            407,
        )
        .await
        .expect("project grant");
    let context = store
        .prepare_instance(
            owner.clone(),
            package.extension_id.clone(),
            "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb".to_owned(),
            408,
        )
        .await
        .expect("prepare instance");
    store
        .mark_instance_running(context.lease.clone(), 409)
        .await
        .expect("running");
    let summary = store
        .project_summary(context.lease.clone(), project.project_id.to_string(), 410)
        .await
        .expect("scoped summary");
    assert_eq!(summary.project_id, project.project_id.to_string());
    assert_eq!(summary.kind, "vpm");
    let outside = store
        .project_summary(context.lease.clone(), ProjectId::new().to_string(), 410)
        .await
        .expect_err("ungranted project scope");
    assert_eq!(outside.code(), M6ErrorCode::ScopeDenied);

    let write = store
        .data_set(
            context.lease.clone(),
            "fixture.key".to_owned(),
            b"private-value".to_vec(),
            None,
            411,
        )
        .await
        .expect("data set");
    let value = store
        .data_get(context.lease.clone(), "fixture.key".to_owned(), 412)
        .await
        .expect("data get")
        .expect("stored value");
    assert_eq!(value.value, b"private-value");
    assert_eq!(value.key_revision, write.key_revision);
    let oversized = store
        .data_set(
            context.lease.clone(),
            "fixture.large".to_owned(),
            vec![0; 65_537],
            None,
            413,
        )
        .await
        .expect_err("value limit");
    assert_eq!(oversized.code(), M6ErrorCode::InvalidInput);
    let invalid_key = store
        .data_get(context.lease.clone(), "../other".to_owned(), 413)
        .await
        .expect_err("invalid key");
    assert_eq!(invalid_key.code(), M6ErrorCode::InvalidInput);

    store
        .set_grant(
            owner,
            ExtensionGrantMutation {
                extension_id: package.extension_id,
                permission: "projects.read".to_owned(),
                resource_kind: "Project".to_owned(),
                resource_id: project.project_id.to_string(),
                expected_revision: project_grant.grant_revision,
                grant: false,
            },
            IdempotencyKey::parse("capability-project-revoke").expect("key"),
            414,
        )
        .await
        .expect("revoke");
    let stale = store
        .data_get(context.lease, "fixture.key".to_owned(), 415)
        .await
        .expect_err("revocation invalidates old lease");
    assert_eq!(stale.code(), M6ErrorCode::InstanceStale);
    drop(store);
    cleanup(&path);
}

#[tokio::test]
async fn first_party_policy_and_owner_approved_policy_share_the_same_runtime_authority() {
    let owner_approved = policy_capability_outcome(
        "owner-approved",
        ExtensionSourceKind::LocalOwnerSelected,
        ExtensionTrustDecision::UserApprovedForExtension,
    )
    .await;
    let first_party = policy_capability_outcome(
        "first-party",
        ExtensionSourceKind::FirstPartyPackaged,
        ExtensionTrustDecision::Official,
    )
    .await;
    assert_eq!(owner_approved, first_party);
}

#[tokio::test]
async fn extension_list_uses_exclusive_extension_id_keyset_order() {
    let path = database_path("pagination");
    let store = StateStoreHandle::open(path.clone()).expect("open pagination store");
    let owner = PrincipalId::local_owner();
    for (index, extension_id) in [
        "dev.example.z-last",
        "dev.example.a-first",
        "dev.example.m-middle",
    ]
    .into_iter()
    .enumerate()
    {
        let mut package = evidence();
        package.extension_id = extension_id.to_owned();
        package.package_digest[0] = u8::try_from(index + 1).expect("digest marker");
        let plan = store
            .create_install_plan(
                owner.clone(),
                ExtensionInstallPlanDraft {
                    evidence: package,
                    trust_decision: ExtensionTrustDecision::UserApprovedForExtension,
                    expected_revision: None,
                    plan_fingerprint: [u8::try_from(index + 1).expect("fingerprint marker"); 32],
                },
                600 + u64::try_from(index).expect("timestamp"),
            )
            .await
            .expect("pagination plan");
        let accepted = store
            .accept_plan(
                owner.clone(),
                plan.plan_id,
                IdempotencyKey::parse(format!("pagination-{index}")).expect("pagination key"),
                610 + u64::try_from(index).expect("timestamp"),
            )
            .await
            .expect("accept pagination plan");
        store
            .begin_apply(
                accepted.operation_id,
                620 + u64::try_from(index).expect("timestamp"),
            )
            .await
            .expect("begin pagination install");
        store
            .finish_install(
                accepted.operation_id,
                format!("<EXTENSION_ROOT>/{extension_id}"),
                630 + u64::try_from(index).expect("timestamp"),
            )
            .await
            .expect("finish pagination install");
        store
            .complete_operation(
                accepted.operation_id,
                640 + u64::try_from(index).expect("timestamp"),
            )
            .await
            .expect("complete pagination install");
    }

    let first = store
        .list_extensions(owner.clone(), None, 2)
        .await
        .expect("first page");
    assert_eq!(
        first
            .extensions
            .iter()
            .map(|record| record.extension_id.as_str())
            .collect::<Vec<_>>(),
        ["dev.example.a-first", "dev.example.m-middle"]
    );
    let second = store
        .list_extensions(owner, first.next_cursor, 2)
        .await
        .expect("second page");
    assert_eq!(
        second
            .extensions
            .iter()
            .map(|record| record.extension_id.as_str())
            .collect::<Vec<_>>(),
        ["dev.example.z-last"]
    );
    assert!(second.next_cursor.is_none());
    drop(store);
    cleanup(&path);
}

async fn policy_capability_outcome(
    name: &str,
    source_kind: ExtensionSourceKind,
    trust_decision: ExtensionTrustDecision,
) -> (String, Vec<u8>, Vec<String>, Vec<String>) {
    let path = database_path(name);
    let store = StateStoreHandle::open(path.clone()).expect("open policy store");
    let owner = PrincipalId::local_owner();
    let mut package = evidence();
    package.source_kind = source_kind;
    package.extension_id = format!("dev.example.{name}");
    package.source_locator = format!("C:/fixture/{name}.alcomdext");
    let plan = store
        .create_install_plan(
            owner.clone(),
            ExtensionInstallPlanDraft {
                evidence: package.clone(),
                trust_decision,
                expected_revision: None,
                plan_fingerprint: [21; 32],
            },
            500,
        )
        .await
        .expect("policy plan");
    let accepted = store
        .accept_plan(
            owner.clone(),
            plan.plan_id,
            IdempotencyKey::parse(format!("policy-{name}")).expect("policy key"),
            501,
        )
        .await
        .expect("accept policy plan");
    store
        .begin_apply(accepted.operation_id, 502)
        .await
        .expect("begin policy install");
    store
        .finish_install(
            accepted.operation_id,
            format!("<EXTENSION_ROOT>/{name}"),
            503,
        )
        .await
        .expect("finish policy install");
    store
        .complete_operation(accepted.operation_id, 504)
        .await
        .expect("complete policy install");
    let project = store
        .register_project(
            owner.clone(),
            ProjectObservation {
                package_presentations: Default::default(),
                root_path: format!("<PROJECT_ROOT>/{name}"),
                path_identity_key: name.as_bytes().to_vec(),
                project_type: ProjectType::Avatars,
                unity_version: "2022.3.22f1".to_owned(),
                unity_revision: None,
                vpm_manifest: ManifestState::Valid,
                upm_manifest: ManifestState::Valid,
                direct_dependencies: Vec::new(),
                locked_dependencies: Vec::new(),
                issues: Vec::new(),
            },
            IdempotencyKey::parse(format!("policy-project-{name}")).expect("project policy key"),
            505,
        )
        .await
        .expect("register policy project")
        .value;
    let connection = Connection::open(&path).expect("policy setup connection");
    connection
        .execute(
            "UPDATE extensions SET desired_state='enabled' WHERE extension_id=?1",
            [&package.extension_id],
        )
        .expect("enable policy extension");
    connection
        .execute(
            "INSERT INTO extension_grants
             (extension_id, permission_name, resource_kind, resource_id, state,
              grant_revision, updated_at_ms)
             VALUES (?1, 'background.run', 'Extension', ?1, 'granted', 1, 506)",
            [&package.extension_id],
        )
        .expect("policy background grant");
    drop(connection);
    store
        .set_grant(
            owner.clone(),
            ExtensionGrantMutation {
                extension_id: package.extension_id.clone(),
                permission: "projects.read".to_owned(),
                resource_kind: "Project".to_owned(),
                resource_id: project.project_id.to_string(),
                expected_revision: Revision::new(1).expect("policy grant revision"),
                grant: true,
            },
            IdempotencyKey::parse(format!("policy-grant-{name}")).expect("grant policy key"),
            507,
        )
        .await
        .expect("policy project grant");
    let context = store
        .prepare_instance(
            owner,
            package.extension_id,
            "cccccccc-cccc-4ccc-8ccc-cccccccccccc".to_owned(),
            508,
        )
        .await
        .expect("prepare policy instance");
    store
        .mark_instance_running(context.lease.clone(), 509)
        .await
        .expect("run policy instance");
    let summary = store
        .project_summary(context.lease.clone(), project.project_id.to_string(), 510)
        .await
        .expect("policy project summary");
    store
        .data_set(
            context.lease.clone(),
            "policy.key".to_owned(),
            b"same-public-data-api".to_vec(),
            None,
            511,
        )
        .await
        .expect("policy data set");
    let value = store
        .data_get(context.lease, "policy.key".to_owned(), 512)
        .await
        .expect("policy data get")
        .expect("policy data value")
        .value;
    let outcome = (
        summary.kind,
        value,
        plan.evidence.required_permissions,
        plan.evidence.required_interfaces,
    );
    drop(store);
    cleanup(&path);
    outcome
}
