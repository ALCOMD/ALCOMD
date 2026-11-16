use super::*;
use rusqlite::types::Value;
fn fixture() -> Connection {
    let db = extension_mcp_grants_migration_tests::fixture();
    for sql in [MIGRATION_V35, MIGRATION_V36, MIGRATION_V37] {
        db.execute_batch(sql).expect("previous migration");
    }
    db
}
fn rows(db: &Connection) -> Vec<(String, Vec<Vec<Value>>)> {
    let mut names=db.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name<>'extension_mcp_calls' ORDER BY name").expect("valid isolated authority fixture");
    let names = names
        .query_map([], |r| r.get::<_, String>(0))
        .expect("valid isolated authority fixture")
        .collect::<Result<Vec<_>, _>>()
        .expect("valid isolated authority fixture");
    names
        .into_iter()
        .map(|name| {
            let mut query = db
                .prepare(&format!(
                    "SELECT * FROM \"{}\" ORDER BY rowid",
                    name.replace('"', "\"\"")
                ))
                .expect("valid isolated authority fixture");
            let count = query.column_count();
            let values = query
                .query_map([], |r| {
                    (0..count)
                        .map(|i| r.get(i))
                        .collect::<Result<Vec<Value>, _>>()
                })
                .expect("valid isolated authority fixture")
                .collect::<Result<Vec<_>, _>>()
                .expect("valid isolated authority fixture");
            (name, values)
        })
        .collect()
}
#[test]
fn state38_preserves_every_old_cell_and_event_sequence() {
    let db = fixture();
    let before = rows(&db);
    db.execute_batch(MIGRATION_V38)
        .expect("valid isolated authority fixture");
    assert_eq!(rows(&db), before);
    assert_eq!(
        db.query_row("SELECT count(*) FROM extension_mcp_calls", [], |r| r
            .get::<_, u32>(0))
            .expect("valid isolated authority fixture"),
        0
    );
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .expect("valid isolated authority fixture"),
        38
    );
    assert!(
        db.prepare("PRAGMA foreign_key_check")
            .expect("valid isolated authority fixture")
            .query([])
            .expect("valid isolated authority fixture")
            .next()
            .expect("valid isolated authority fixture")
            .is_none()
    );
}
#[test]
fn state38_only_global_capabilities_and_existing_pk_fk() {
    let db = fixture();
    db.execute_batch(MIGRATION_V38)
        .expect("valid isolated authority fixture");
    for permission in [
        "mcp.tools.provide",
        "repositories.read",
        "packages.read",
        "packages.manage",
        "operations.read",
        "operations.cancel",
    ] {
        db.execute("INSERT INTO extension_grants VALUES ('dev.alcomd.state35',?1,'Global','*','granted',4,30)",[permission]).expect("valid isolated authority fixture");
        assert!(db.execute("INSERT INTO extension_grants VALUES ('dev.alcomd.state35',?1,'Project','anything','granted',4,30)",[permission]).is_err());
        assert!(
            db.execute(
                "INSERT INTO extension_grants VALUES ('missing',?1,'Global','*','granted',4,30)",
                [permission]
            )
            .is_err()
        );
        assert!(db.execute("INSERT INTO extension_grants VALUES ('dev.alcomd.state35',?1,'Global','*','granted',4,30)",[permission]).is_err());
    }
}
#[test]
fn state38_failure_rolls_back_rebuild_and_all_rows() {
    let db = fixture();
    let before = rows(&db);
    let sql = MIGRATION_V38.replace(
        "PRAGMA user_version=38;",
        "SELECT nonexistent_state38_column; PRAGMA user_version=38;",
    );
    assert!(db.execute_batch(&sql).is_err());
    db.execute_batch("ROLLBACK")
        .expect("valid isolated authority fixture");
    assert_eq!(rows(&db), before);
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .expect("valid isolated authority fixture"),
        37
    );
    assert!(db.prepare("SELECT * FROM extension_mcp_calls").is_err());
}
#[test]
fn state38_real_initializer_upgrades_once_then_reopens() {
    let source = fixture();
    let path = std::env::temp_dir().join(format!("alcomd-state38-{}.db", Uuid::new_v4()));
    source
        .execute(
            "VACUUM INTO ?1",
            [path.to_str().expect("valid isolated authority fixture")],
        )
        .expect("valid isolated authority fixture");
    let before = publisher_trust_removal_tests::expected_state39(rows(&source));
    for _ in 0..2 {
        let db = initialize_connection(&path).expect("valid isolated authority fixture");
        assert_eq!(rows(&db), before);
        assert_eq!(
            db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .expect("valid isolated authority fixture"),
            CURRENT_DATA_SCHEMA
        );
    }
    std::fs::remove_file(path).expect("valid isolated authority fixture");
}
// Real Store authority/transaction tests. Native SDK/Host execution is verified separately.
fn authority_fixture() -> (Connection, alcomd_application::ToolInvocation) {
    use alcomd_application as app;
    let mut db = fixture();
    db.execute_batch(MIGRATION_V38)
        .expect("valid isolated authority fixture");
    let now = crate::m8_oauth::wall_time().expect("valid isolated authority fixture");
    let id: String = db
        .query_row("SELECT client_id FROM mcp_clients", [], |r| r.get(0))
        .expect("valid isolated authority fixture");
    for permission in [
        "projects.read",
        "repositories.read",
        "packages.read",
        "packages.manage",
        "operations.read",
        "operations.cancel",
    ] {
        db.execute(
            "INSERT INTO mcp_client_grants VALUES (?1,2,?2,'global','*')",
            params![id, permission],
        )
        .expect("valid isolated authority fixture");
        db.execute("INSERT INTO extension_grants VALUES ('dev.alcomd.state35',?1,'Global','*','granted',4,30)",[permission]).expect("valid isolated authority fixture");
    }
    db.execute("INSERT INTO extension_grants VALUES ('dev.alcomd.state35','mcp.tools.provide','Global','*','granted',4,30)",[]).expect("valid isolated authority fixture");
    db.execute("UPDATE extension_instances SET bound_grant_revision=(SELECT grant_revision FROM extensions WHERE extension_id='dev.alcomd.state35'),lease_expires_at_ms=?1,lease_cancelled=0,runtime_state='running'",[i64::try_from(now+60000).expect("valid isolated authority fixture")]).expect("valid isolated authority fixture");
    let (instance,principal,revision,generation,epoch,publisher):(String,String,u32,u32,String,String)=db.query_row("SELECT i.instance_id,i.principal_id,i.bound_grant_revision,i.lifecycle_generation,i.daemon_epoch,e.publisher_fingerprint FROM extension_instances i JOIN extensions e USING(extension_id)",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).expect("valid isolated authority fixture");
    let origin = app::ToolOrigin {
        extension_id: "dev.alcomd.state35".into(),
        publisher_fingerprint: publisher.clone(),
        package_digest: "0".repeat(64),
        version: "1.0.0".into(),
    };
    let declaration = app::ToolDeclaration {
        name: "apply".into(),
        title: "Apply".into(),
        description: "Fixture".into(),
        input_schema: serde_json::json!({"type":"object","properties":{},"additionalProperties":false}),
        output_schema: serde_json::json!({"type":"object"}),
        uses: vec![app::ToolOperation::ApplyPlan],
    };
    let auth = app::ToolAuthentication::Credential([0; 32]);
    let admission = crate::m8_tools::begin(
        &mut db,
        &auth,
        &alcomd_application::ToolWriteAdmission {
            origin: origin.clone(),
            name: ("fixture_apply").into(),
            key: ("same-key").into(),
            fingerprint: "a".repeat(64),
            permissions: declaration.permissions(),
            effect: app::ToolEffect::Apply,
        },
    )
    .expect("valid isolated authority fixture");
    let authority = app::ToolInvocation {
        call_id: admission.call_id,
        principal: app::PrincipalId::parse(format!("mcp:{id}"))
            .expect("valid isolated authority fixture"),
        origin,
        declaration,
        authentication: auth,
        lease: Some(app::ExtensionInstanceLease {
            lease_id: Uuid::new_v4().to_string(),
            extension_id: "dev.alcomd.state35".into(),
            instance_id: instance,
            principal_id: app::PrincipalId::parse(principal)
                .expect("valid isolated authority fixture"),
            publisher_fingerprint: publisher,
            grant_revision: app::Revision::new(u64::from(revision))
                .expect("valid isolated authority fixture"),
            lifecycle_generation: app::Revision::new(u64::from(generation))
                .expect("valid isolated authority fixture"),
            daemon_epoch: epoch,
            expires_at_ms: now + 60000,
        }),
        control: serde_json::json!({}),
        active: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
        deadline_ms: now + 60000,
    };
    (db, authority)
}
#[test]
fn tool_authority_checks_both_sides_source_lease_and_closed_operation_each_time() {
    use alcomd_application::{ToolError as E, ToolOperation as O};
    let (mut db, authority) = authority_fixture();
    assert!(crate::m8_tools::check(&db, &authority, Some(O::ApplyPlan)).is_ok());
    assert_eq!(
        crate::m8_tools::check(&db, &authority, Some(O::ProjectsGet)).err(),
        Some(E::PermissionDenied)
    );
    db.execute(
        "UPDATE extension_grants SET state='revoked' WHERE permission_name='packages.manage'",
        [],
    )
    .expect("valid isolated authority fixture");
    assert_eq!(
        crate::m8_tools::check(&db, &authority, None).err(),
        Some(E::PermissionDenied)
    );
    db.execute(
        "UPDATE extension_grants SET state='granted' WHERE permission_name='packages.manage'",
        [],
    )
    .expect("valid isolated authority fixture");
    db.execute("UPDATE extensions SET package_digest=X'0101010101010101010101010101010101010101010101010101010101010101'",[]).expect("valid isolated authority fixture");
    assert_eq!(
        crate::m8_tools::check(&db, &authority, None).err(),
        Some(E::ProviderUnavailable)
    );
    db.execute("UPDATE extensions SET package_digest=zeroblob(32)", [])
        .expect("valid isolated authority fixture");
    let current = crate::m8_tools::authenticate(&db, &authority.authentication)
        .expect("valid isolated authority fixture");
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .expect("valid isolated authority fixture");
    crate::m8_mcp::write_on(
        &tx,
        alcomd_application::PrincipalId::local_owner(),
        "revoke-manage".into(),
        alcomd_application::McpIdentityWrite::SetGrants {
            id: current.client_id,
            expected: current.revision,
            grants: current
                .grants
                .into_iter()
                .filter(|g| g.permission != "packages.manage")
                .collect(),
        },
        crate::m8_oauth::wall_time().expect("valid isolated authority fixture"),
    )
    .expect("valid isolated authority fixture");
    tx.commit().expect("valid isolated authority fixture");
    assert_eq!(
        crate::m8_tools::check(&db, &authority, None).err(),
        Some(E::PermissionDenied)
    );
}
#[test]
fn tool_credential_rotation_cancellation_and_lease_revocation_are_live() {
    use alcomd_application::ToolError as E;
    let (db, authority) = authority_fixture();
    db.execute(
        "UPDATE extension_instances SET lease_expires_at_ms=lease_expires_at_ms+1000",
        [],
    )
    .expect("valid isolated authority fixture");
    assert!(
        crate::m8_tools::check(&db, &authority, None).is_ok(),
        "normal lease renewal keeps this bound instance usable"
    );
    db.execute("UPDATE mcp_client_credentials SET revoked_at_ms=30", [])
        .expect("valid isolated authority fixture");
    assert_eq!(
        crate::m8_tools::check(&db, &authority, None).err(),
        Some(E::PermissionDenied)
    );
    db.execute("UPDATE mcp_client_credentials SET revoked_at_ms=NULL", [])
        .expect("valid isolated authority fixture");
    db.execute("UPDATE extension_instances SET lease_cancelled=1", [])
        .expect("valid isolated authority fixture");
    assert_eq!(
        crate::m8_tools::check(&db, &authority, None).err(),
        Some(E::PermissionDenied)
    );
    db.execute("UPDATE extension_instances SET lease_cancelled=0", [])
        .expect("valid isolated authority fixture");
    authority.cancel();
    assert_eq!(
        crate::m8_tools::check(&db, &authority, None).err(),
        Some(E::Interrupted)
    );
}
#[test]
fn tool_receipt_is_atomic_with_business_transaction_and_rejects_second_effect() {
    use alcomd_application::{ToolError as E, ToolOperation as O};
    let (mut db, authority) = authority_fixture();
    let _context = crate::m8_tools::ContextGuard::install(Some(authority.clone()));
    let before: Vec<(String, Vec<Vec<Value>>)> = rows(&db);
    {
        let tx = db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("valid isolated authority fixture");
        let guard =
            crate::m8_tools::guard(&tx, O::ApplyPlan).expect("valid isolated authority fixture");
        tx.execute(
            "UPDATE extension_data_namespaces SET revision=revision+1",
            [],
        )
        .expect("valid isolated authority fixture");
        crate::m8_tools::effect(
            &tx,
            guard.as_ref(),
            O::ApplyPlan,
            serde_json::json!({"kind":"apply","operationId":Uuid::new_v4().to_string()}),
        )
        .expect("valid isolated authority fixture");
        tx.rollback().expect("valid isolated authority fixture");
    }
    assert_eq!(rows(&db), before);
    assert!(
        db.query_row(
            "SELECT core_effect_json IS NULL FROM extension_mcp_calls",
            [],
            |r| r.get::<_, bool>(0)
        )
        .expect("valid isolated authority fixture")
    );
    {
        let tx = db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("valid isolated authority fixture");
        let guard =
            crate::m8_tools::guard(&tx, O::ApplyPlan).expect("valid isolated authority fixture");
        tx.execute(
            "UPDATE extension_data_namespaces SET revision=revision+1",
            [],
        )
        .expect("valid isolated authority fixture");
        crate::m8_tools::effect(
            &tx,
            guard.as_ref(),
            O::ApplyPlan,
            serde_json::json!({"kind":"apply","operationId":Uuid::new_v4().to_string()}),
        )
        .expect("valid isolated authority fixture");
        tx.commit().expect("valid isolated authority fixture");
    }
    assert_eq!(
        crate::m8_tools::guard(&db, O::ApplyPlan).err(),
        Some(E::IdempotencyConflict)
    );
    let receipt: String = db
        .query_row(
            "SELECT core_effect_json FROM extension_mcp_calls",
            [],
            |r| r.get(0),
        )
        .expect("valid isolated authority fixture");
    let value: serde_json::Value =
        serde_json::from_str(&receipt).expect("valid isolated authority fixture");
    assert_eq!(value["source"]["packageDigest"], "0".repeat(64));
}
#[test]
fn tool_same_key_is_principal_isolated_and_argument_conflicts_are_not_retried() {
    use alcomd_application::{ToolEffect, ToolError as E};
    let (mut db, authority) = authority_fixture();
    let auth = authority.authentication.clone();
    let required = authority.declaration.permissions();
    let replay = crate::m8_tools::begin(
        &mut db,
        &auth,
        &alcomd_application::ToolWriteAdmission {
            origin: authority.origin.clone(),
            name: ("fixture_apply").into(),
            key: ("same-key").into(),
            fingerprint: "a".repeat(64),
            permissions: required.clone(),
            effect: ToolEffect::Apply,
        },
    )
    .expect("valid isolated authority fixture");
    assert_eq!(replay.call_id, authority.call_id);
    assert_eq!(replay.state, "running");
    assert_eq!(
        crate::m8_tools::begin(
            &mut db,
            &auth,
            &alcomd_application::ToolWriteAdmission {
                origin: authority.origin.clone(),
                name: ("fixture_apply").into(),
                key: ("same-key").into(),
                fingerprint: "b".repeat(64),
                permissions: required.clone(),
                effect: ToolEffect::Apply
            }
        )
        .err(),
        Some(E::IdempotencyConflict)
    );
    db.execute("UPDATE mcp_client_credentials SET revoked_at_ms=30", [])
        .expect("valid isolated authority fixture");
    assert_eq!(
        crate::m8_tools::begin(
            &mut db,
            &auth,
            &alcomd_application::ToolWriteAdmission {
                origin: authority.origin.clone(),
                name: ("fixture_apply").into(),
                key: ("same-key").into(),
                fingerprint: "a".repeat(64),
                permissions: required.clone(),
                effect: ToolEffect::Apply
            }
        )
        .err(),
        Some(E::PermissionDenied)
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM extension_mcp_calls", [], |r| r
            .get::<_, u32>(0))
            .expect("valid isolated authority fixture"),
        1
    );
}

#[tokio::test]
async fn tool_restart_recovers_accepted_evidence_and_never_restarts_an_unaccepted_call() {
    use alcomd_application::{ToolError, ToolOperation};
    for accepted in [false, true] {
        let (mut db, authority) = authority_fixture();
        let operation_id = Uuid::new_v4().to_string();
        if accepted {
            let tx = db
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .expect("effect transaction");
            crate::m8_tools::effect(
                &tx,
                Some(&authority),
                ToolOperation::ApplyPlan,
                serde_json::json!({"kind":"apply","operationId":operation_id}),
            )
            .expect("accepted effect evidence");
            tx.commit().expect("durable receipt");
        }
        let before = publisher_trust_removal_tests::expected_state39(rows(&db));
        let path = std::env::temp_dir().join(format!("alcomd-tool-restart-{}.db", Uuid::new_v4()));
        db.execute("VACUUM INTO ?1", [path.to_str().expect("fixture path")])
            .expect("durable isolated fixture");
        drop(db);
        let store = crate::StateStoreHandle::open(path.clone()).expect("reopened authority");
        store
            .tool_recover_startup()
            .await
            .expect("startup recovery");
        let replay = store
            .tool_replay(
                authority.authentication,
                "fixture_apply".into(),
                "same-key".into(),
                "a".repeat(64),
            )
            .await
            .expect("explicit same-key recovery")
            .expect("receipt survives restart");
        assert!(replay.replayed);
        assert!(replay.data.is_null());
        if accepted {
            assert_eq!(replay.error, None);
            assert_eq!(
                replay.core_effect.expect("effect")["operationId"],
                operation_id
            );
        } else {
            assert_eq!(replay.error, Some(ToolError::Interrupted));
            assert!(replay.core_effect.is_none());
        }
        let db = Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .expect("inspect isolated recovery");
        assert_eq!(rows(&db), before, "recovery did not execute business work");
        assert_eq!(
            db.query_row("SELECT count(*) FROM extension_mcp_calls", [], |r| r
                .get::<_, u32>(0))
                .expect("one original receipt"),
            1
        );
    }
}
