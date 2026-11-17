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
