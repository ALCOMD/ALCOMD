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
                observed_at_ms: 405,
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
                observed_at_ms: 505,
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
