use super::*;
use rusqlite::types::Value;

const EXTENSION: &str = "dev.alcomd.authority-test";
fn extension_fixture() -> Connection {
    let db = current_schema_fixture();
    let fingerprint = format!("ed25519-sha256:{}", "1".repeat(64));
    db.execute("INSERT INTO extensions(extension_id,version,api_major,package_digest,manifest_digest,component_digest,publisher_fingerprint,principal_id,live_package_locator,desired_state,quarantine_state,grant_revision,lifecycle_generation,revision,installed_at_ms,updated_at_ms) VALUES (?1,'1.0.0',1,zeroblob(32),zeroblob(32),zeroblob(32),?2,'builtin:local-owner','isolated-fixture','enabled','clear',3,2,5,10,20)",params![EXTENSION,fingerprint]).expect("extension");
    db.execute("INSERT INTO extension_instances VALUES (?1,?2,'extension:dev.alcomd.authority-test',3,2,?3,'running',500,0,10,20)",params![EXTENSION,Uuid::new_v4().to_string(),Uuid::new_v4().to_string()]).expect("instance");
    db.execute(
        "INSERT INTO extension_grants VALUES (?1,'background.run','Extension',?1,'granted',2,12)",
        [EXTENSION],
    )
    .expect("background");
    db.execute(
        "INSERT INTO extension_data_namespaces VALUES (?1,?2,7,1,4,20)",
        params![EXTENSION, fingerprint],
    )
    .expect("namespace");
    db.execute(
        "INSERT INTO extension_data_items VALUES (?1,?2,'kept',X'01020304',6)",
        params![EXTENSION, fingerprint],
    )
    .expect("data");
    let id = Uuid::new_v4().to_string();
    db.execute(
        "INSERT INTO mcp_clients VALUES (?1,'mcp:'||?1,'Fixture client',4,10,NULL)",
        [&id],
    )
    .expect("MCP client");
    db.execute(
        "INSERT INTO mcp_client_credentials VALUES (?1,?2,zeroblob(32),10,NULL)",
        params![Uuid::new_v4().to_string(), id],
    )
    .expect("non-secret credential hash fixture");
    for _ in 0..3 {
        db.execute("INSERT INTO events(event_id,kind,aggregate_kind,aggregate_id,aggregate_revision,principal_id,occurred_at_ms,payload_json) VALUES (?1,'state.check.succeeded','operation',?2,1,'builtin:local-owner',20,'{}')",params![Uuid::new_v4().to_string(),Uuid::new_v4().to_string()]).expect("event");
    }
    db.execute(
        "DELETE FROM events WHERE sequence=(SELECT max(sequence) FROM events)",
        [],
    )
    .expect("keep sequence above live max");
    db
}

fn extension_plan_fixture() -> Connection {
    let db = extension_fixture();
    db.execute_batch("INSERT INTO extension_plans(plan_id,owner_principal_id,action,state,extension_id,version,api_major,profile_version,source_kind,source_locator,source_identity,package_digest,manifest_digest,component_digest,publisher_fingerprint,requested_permissions_json,requested_interfaces_json,data_disposition,plan_fingerprint,created_at_ms,ui_protocol) SELECT '11111111-2222-4333-8444-555555555555',principal_id,'install','unapplied','dev.example.frozen-plan',version,api_major,1,'local_owner_selected','isolated-package',X'01',package_digest,manifest_digest,component_digest,publisher_fingerprint,'{\"required\":[],\"optional\":[]}','{\"required\":[],\"optional\":[],\"mcp_tools\":[],\"cli_commands\":[]}','not_applicable',zeroblob(32),20,ui_protocol FROM extensions;").expect("nonempty frozen plan");
    db
}

fn rows(db: &Connection) -> Vec<(String, Vec<Vec<Value>>)> {
    let mut names=db.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name<>'extension_entry_calls' ORDER BY name").expect("valid isolated authority fixture");
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

fn client(db: &Connection, n: u32) -> String {
    let id = format!("{n:08}-1111-4111-8111-111111111111");
    db.execute(
        "INSERT INTO mcp_clients VALUES (?1, 'mcp:' || ?1, 'Client', 1, 10, NULL)",
        [&id],
    )
    .expect("client");
    id
}

#[test]
fn mcp_credentials_require_digests_and_atomic_rotation() {
    let mut db = current_schema_fixture();
    let id = client(&db, 1);
    for size in [0, 31, 33] {
        assert!(db.execute("INSERT INTO mcp_client_credentials VALUES ('22222222-1111-4111-8111-111111111111',?1,?2,10,NULL)",params![id,vec![1_u8;size]]).is_err());
    }
    db.execute("INSERT INTO mcp_client_credentials VALUES ('22222222-1111-4111-8111-111111111111',?1,zeroblob(32),10,NULL)",[&id]).expect("digest only");
    assert!(db.execute("INSERT INTO mcp_client_credentials VALUES ('33333333-1111-4111-8111-111111111111',?1,?2,11,NULL)",params![id,vec![1_u8;32]]).is_err());
    {
        let tx = db.transaction().expect("rotation");
        tx.execute(
            "UPDATE mcp_client_credentials SET revoked_at_ms=11 WHERE client_id=?1",
            [&id],
        )
        .expect("revoke old");
        assert!(tx.execute("INSERT INTO mcp_client_credentials VALUES ('33333333-1111-4111-8111-111111111111',?1,zeroblob(31),11,NULL)",[&id]).is_err());
        // Dropping a failed rotation retains the sole original live credential.
    }
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM mcp_client_credentials WHERE revoked_at_ms IS NULL",
            [],
            |r| r.get::<_, u32>(0)
        )
        .expect("old credential"),
        1
    );
    assert!(
        db.execute(
            "UPDATE mcp_clients SET principal_id='builtin:local-owner'",
            []
        )
        .is_err()
    );
    assert!(
        db.execute("UPDATE mcp_clients SET revoked_at_ms=9", [])
            .is_err()
    );
}

#[test]
fn mcp_clients_are_bounded_and_credentials_require_registered_clients() {
    let db = current_schema_fixture();
    assert!(db.execute("INSERT INTO mcp_client_credentials VALUES ('22222222-1111-4111-8111-111111111111','22222222-1111-4111-8111-111111111111',zeroblob(32),10,NULL)", []).is_err());
    for n in 1..=64 {
        client(&db, n);
    }
    assert!(db.execute("INSERT INTO mcp_clients VALUES ('99999999-1111-4111-8111-111111111111','mcp:99999999-1111-4111-8111-111111111111','Overflow',1,10,NULL)", []).is_err());
}

#[test]
fn extension_grants_require_explicit_permissions_and_exact_scopes() {
    let db = extension_fixture();
    let grant = |permission: &str, kind: &str, resource: &str| {
        db.execute(
            "INSERT INTO extension_grants VALUES (?1,?2,?3,?4,'granted',3,20)",
            params![EXTENSION, permission, kind, resource],
        )
    };
    for permission in [
        "projects.read",
        "mcp.clients.read",
        "mcp.clients.manage",
        "mcp.connections.read",
        "mcp.requests.read",
        "mcp.subscription-streams.read",
        "repositories.read",
        "packages.read",
        "packages.manage",
        "operations.read",
        "operations.cancel",
        "unity.sessions.read",
    ] {
        for (kind, resource) in [
            ("Global", "wrong-target"),
            ("Extension", EXTENSION),
            ("Project", "00000000-0000-4000-8000-000000000001"),
        ] {
            assert!(
                grant(permission, kind, resource).is_err(),
                "{permission}/{kind}"
            );
        }
        grant(permission, "Global", "*").expect("explicit global scope");
        assert!(
            grant(permission, "Global", "*").is_err(),
            "duplicate {permission}"
        );
    }
    for (permission, kind, id) in [
        ("background.run", "Global", "*"),
        ("local-ipc.connect", "Global", "*"),
        ("local-ipc.connect", "LocalIpcTarget", &"B".repeat(64)),
        ("local-ipc.connect", "LocalIpcTarget", "short"),
        ("unity.sessions.read", "LocalIpcTarget", &"a".repeat(64)),
        ("unknown.permission", "Global", "*"),
    ] {
        assert!(grant(permission, kind, id).is_err(), "{permission}/{kind}");
    }
    grant("local-ipc.connect", "LocalIpcTarget", &"a".repeat(64)).expect("explicit local target");
    assert!(db.execute("INSERT INTO extension_grants VALUES ('dev.alcomd.missing','mcp.clients.read','Global','*','granted',1,20)", []).is_err());
    db.execute("DELETE FROM extensions WHERE extension_id=?1", [EXTENSION])
        .expect("cascade");
    assert_eq!(
        db.query_row("SELECT count(*) FROM extension_grants", [], |r| r
            .get::<_, u32>(0))
            .expect("grants"),
        0
    );
}

#[test]
fn extension_plan_freezes_identity_permissions_and_installation_terms() {
    let db = extension_plan_fixture();
    let before = rows(&db);
    for mutation in [
        "publisher_fingerprint='ed25519-sha256:'||printf('%064d',0)",
        "source_locator='replaced-source'",
        "source_kind='remote_extension_source', remote_source_json='{}'",
        "package_digest=randomblob(32)",
        "requested_permissions_json='{}'",
        "installation_terms_json='{}'",
        "owner_principal_id='other-owner'",
        "plan_fingerprint=randomblob(32)",
        "state='applied'",
        "ui_protocol='portable-v1'",
    ] {
        assert!(
            db.execute(&format!("UPDATE extension_plans SET {mutation}"), [])
                .is_err(),
            "{mutation}"
        );
        assert_eq!(rows(&db), before);
    }
}

// Real Store authority/transaction tests. Native SDK/Host execution is verified separately.
fn authority_fixture() -> (Connection, alcomd_application::ToolInvocation) {
    use alcomd_application as app;
    let mut db = extension_fixture();
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
        db.execute("INSERT INTO extension_grants VALUES ('dev.alcomd.authority-test',?1,'Global','*','granted',4,30)",[permission]).expect("valid isolated authority fixture");
    }
    db.execute("UPDATE extension_instances SET bound_grant_revision=(SELECT grant_revision FROM extensions WHERE extension_id='dev.alcomd.authority-test'),lease_expires_at_ms=?1,lease_cancelled=0,runtime_state='running'",[i64::try_from(now+60000).expect("valid isolated authority fixture")]).expect("valid isolated authority fixture");
    let (instance,principal,revision,generation,epoch,publisher):(String,String,u32,u32,String,String)=db.query_row("SELECT i.instance_id,i.principal_id,i.bound_grant_revision,i.lifecycle_generation,i.daemon_epoch,e.publisher_fingerprint FROM extension_instances i JOIN extensions e USING(extension_id)",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).expect("valid isolated authority fixture");
    let origin = app::ToolOrigin {
        extension_id: "dev.alcomd.authority-test".into(),
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
            kind: alcomd_application::ExtensionEntryKind::Mcp,
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
        kind: alcomd_application::ExtensionEntryKind::Mcp,
        call_id: admission.call_id,
        principal: app::PrincipalId::parse(format!("mcp:{id}"))
            .expect("valid isolated authority fixture"),
        origin,
        declaration,
        authentication: auth,
        lease: Some(app::ExtensionInstanceLease {
            lease_id: Uuid::new_v4().to_string(),
            extension_id: "dev.alcomd.authority-test".into(),
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
        alcomd_application::McpIdentityWrite::Revoke {
            id: current.client_id,
            expected: current.revision,
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
            "SELECT core_effect_json IS NULL FROM extension_entry_calls",
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
            "SELECT core_effect_json FROM extension_entry_calls",
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
            kind: alcomd_application::ExtensionEntryKind::Mcp,
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
                kind: alcomd_application::ExtensionEntryKind::Mcp,
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
                kind: alcomd_application::ExtensionEntryKind::Mcp,
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
        db.query_row("SELECT count(*) FROM extension_entry_calls", [], |r| r
            .get::<_, u32>(0))
            .expect("valid isolated authority fixture"),
        1
    );
}

#[tokio::test]
async fn tool_restart_recovers_accepted_evidence_and_never_restarts_an_unaccepted_call() {
    use alcomd_application::ToolOperation;
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
        let before = rows(&db);
        let path = std::env::temp_dir().join(format!("alcomd-tool-restart-{}.db", Uuid::new_v4()));
        db.execute("VACUUM INTO ?1", [path.to_str().expect("fixture path")])
            .expect("durable isolated fixture");
        drop(db);
        let store = crate::StateStoreHandle::open(path.clone()).expect("reopened authority");
        store
            .tool_recover_startup()
            .await
            .expect("startup recovery");
        let (_, replay, state) = store
            .tool_begin(
                authority.authentication,
                alcomd_application::ToolWriteAdmission {
                    kind: alcomd_application::ExtensionEntryKind::Mcp,
                    origin: authority.origin,
                    name: "fixture_apply".into(),
                    key: "same-key".into(),
                    fingerprint: "a".repeat(64),
                    permissions: authority.declaration.permissions(),
                    effect: alcomd_application::ToolEffect::Apply,
                },
            )
            .await
            .expect("recover original receipt");
        assert_eq!(state, "interrupted");
        if accepted {
            assert_eq!(replay.expect("effect")["operationId"], operation_id);
        } else {
            assert!(replay.is_none());
        }
        let db = Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .expect("inspect isolated recovery");
        assert_eq!(rows(&db), before, "recovery did not execute business work");
        assert_eq!(
            db.query_row("SELECT count(*) FROM extension_entry_calls", [], |r| r
                .get::<_, u32>(0))
                .expect("one original receipt"),
            1
        );
        drop(db);
        drop(store);
        std::fs::remove_file(path).expect("cleanup");
    }
}
