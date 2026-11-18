use super::*;
use rusqlite::types::Value;
fn fixture() -> Connection {
    let db = publisher_trust_removal_tests::fixture();
    for migration in [MIGRATION_V39, MIGRATION_V40, MIGRATION_V41] {
        db.execute_batch(migration).expect("State41 fixture");
    }
    db
}
fn expected(mut rows: publisher_trust_removal_tests::Rows) -> publisher_trust_removal_tests::Rows {
    for (table, values) in &mut rows {
        if table == "extension_plans" {
            for row in values {
                row.push(Value::Null);
            }
        }
    }
    rows
}
#[test]
fn state42_preserves_every_cell_foreign_key_and_local_plan_immutability() {
    let db = fixture();
    let before = publisher_trust_removal_tests::snapshot(&db);
    db.execute_batch(MIGRATION_V42).expect("State42 migration");
    assert_eq!(
        publisher_trust_removal_tests::snapshot(&db),
        expected(before)
    );
    assert_eq!(
        db.pragma_query_value(None, "foreign_keys", |r| r.get::<_, u32>(0))
            .expect("FK on"),
        1
    );
    assert_eq!(
        db.pragma_query_value(None, "legacy_alter_table", |r| r.get::<_, u32>(0))
            .expect("rename mode restored"),
        0
    );
    for column in [
        "remote_source_json",
        "installation_terms_json",
        "source_kind",
        "source_locator",
        "package_digest",
        "plan_fingerprint",
    ] {
        assert!(
            db.execute(
                &format!("UPDATE extension_plans SET {column}='changed'"),
                []
            )
            .is_err(),
            "immutable {column}"
        );
    }
    assert!(
        db.prepare("PRAGMA foreign_key_check")
            .expect("FK check")
            .query([])
            .expect("FK check")
            .next()
            .expect("FK check")
            .is_none()
    );
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .expect("version"),
        42
    );
}
#[test]
fn state42_failure_rolls_back_every_row_schema_trigger_and_version() {
    for marker in [
        "DROP TABLE extension_plans;",
        "ALTER TABLE extension_plans_v42 RENAME TO extension_plans;",
        "PRAGMA user_version=42;",
    ] {
        let db = fixture();
        let before = publisher_trust_removal_tests::snapshot(&db);
        let schema: Vec<(String, String)> = db
            .prepare("SELECT name,sql FROM sqlite_schema WHERE sql IS NOT NULL ORDER BY name")
            .expect("schema")
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .expect("schema")
            .collect::<Result<_, _>>()
            .expect("schema");
        assert!(
            db.execute_batch(&MIGRATION_V42.replace(
                marker,
                &format!("{marker}\nSELECT nonexistent_state42_column;")
            ))
            .is_err()
        );
        db.execute_batch("ROLLBACK; PRAGMA foreign_keys=ON;")
            .expect("rollback");
        assert_eq!(publisher_trust_removal_tests::snapshot(&db), before);
        let after: Vec<(String, String)> = db
            .prepare("SELECT name,sql FROM sqlite_schema WHERE sql IS NOT NULL ORDER BY name")
            .expect("schema")
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .expect("schema")
            .collect::<Result<_, _>>()
            .expect("schema");
        assert_eq!(schema, after);
        assert_eq!(
            db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .expect("version"),
            41
        );
    }
}
#[test]
fn state42_real_initializer_preserves_data_on_upgrade_and_reopen() {
    let db = fixture();
    let before = publisher_trust_removal_tests::snapshot(&db);
    let path = std::env::temp_dir().join(format!("alcomd-state42-{}.db", Uuid::new_v4()));
    db.execute("VACUUM INTO ?1", [path.to_str().expect("path")])
        .expect("durable fixture");
    drop(db);
    for _ in 0..2 {
        let db = initialize_connection(&path).expect("initializer");
        assert_eq!(
            publisher_trust_removal_tests::snapshot(&db),
            expected(before.clone())
        );
        assert_eq!(
            db.pragma_query_value(None, "foreign_keys", |r| r.get::<_, u32>(0))
                .expect("FK on"),
            1
        );
    }
    std::fs::remove_file(path).expect("owned fixture cleanup");
}

#[test]
fn applied_packaged_install_evidence_survives_uninstall_with_delete_data() {
    use crate::m6;
    use alcomd_application::{
        ExtensionDataDisposition, ExtensionInstallConsent, ExtensionInstallPlanDraft,
        ExtensionSourceKind, ExtensionUninstallPlanDraft, PlanId,
    };

    let mut db = fixture();
    db.execute_batch(MIGRATION_V42).expect("State42");
    db.execute_batch(MIGRATION_V43).expect("State43");
    let owner = PrincipalId::local_owner();
    let mut evidence = m6::get_plan(
        &db,
        PlanId::parse("11111111-2222-4333-8444-555555555555").expect("fixture plan id"),
    )
    .expect("disposable evidence")
    .evidence;
    let extension_id = "dev.example.packaged-default";
    evidence.extension_id = extension_id.into();
    evidence.source_kind = ExtensionSourceKind::FirstPartyPackaged;
    let install = m6::create_install_plan(
        &mut db,
        &owner,
        ExtensionInstallPlanDraft {
            evidence,
            expected_revision: None,
            plan_fingerprint: [3; 32],
        },
        30,
    )
    .expect("freeze packaged store fixture");
    let installed = m6::accept_plan(
        &mut db,
        &owner,
        install.plan_id,
        Some(ExtensionInstallConsent {
            plan_fingerprint: "03".repeat(32),
            installation_terms_accepted: false,
            required_permissions_accepted: false,
        }),
        &IdempotencyKey::parse("packaged-default-install").expect("fixture key"),
        31,
    )
    .expect("accept install")
    .operation_id;
    assert!(
        m6::recover_operations(&mut db, 32)
            .expect("recover accepted work")
            .contains(&installed)
    );
    m6::begin_apply(&mut db, installed, 33).expect("begin install");
    m6::finish_install(&mut db, installed, "<FIXTURE>/installed", 34).expect("commit install");
    m6::complete_operation(&mut db, installed, 35).expect("finish install");
    let extension = m6::get_extension(&db, &owner, extension_id).expect("installed extension");
    let uninstall = m6::create_uninstall_plan(
        &mut db,
        &owner,
        ExtensionUninstallPlanDraft {
            extension,
            data_disposition: ExtensionDataDisposition::DeleteData,
            plan_fingerprint: [4; 32],
        },
        36,
    )
    .expect("uninstall plan");
    let removed = m6::accept_plan(
        &mut db,
        &owner,
        uninstall.plan_id,
        None,
        &IdempotencyKey::parse("packaged-default-uninstall").expect("fixture key"),
        37,
    )
    .expect("accept uninstall")
    .operation_id;
    m6::begin_apply(&mut db, removed, 38).expect("begin uninstall");
    m6::finish_uninstall(&mut db, removed, 39).expect("commit uninstall");
    m6::complete_operation(&mut db, removed, 40).expect("finish uninstall");
    assert!(m6::get_extension(&db, &owner, extension_id).is_err());
    assert_eq!(
        db.query_row(
            "SELECT o.state FROM extension_plans p JOIN operations o
               ON o.operation_id=p.apply_operation_id
             JOIN extension_filesystem_journal j ON j.plan_id=p.plan_id
              AND j.operation_id=o.operation_id AND j.phase='state_committed' AND j.state='completed'
             WHERE p.owner_principal_id=?1 AND p.extension_id=?2
              AND p.action='install' AND p.source_kind='first_party_packaged'",
            params![owner.as_str(), extension_id],
            |row| row.get::<_, String>(0),
        )
        .expect("packaged install receipt"),
        "succeeded"
    );
    assert!(
        db.execute(
            "DELETE FROM extension_plans WHERE plan_id=?1",
            [install.plan_id.to_string()]
        )
        .is_err()
    );
}
