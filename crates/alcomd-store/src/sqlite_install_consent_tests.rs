use super::*;
use rusqlite::types::Value;
fn fixture() -> Connection {
    let db = publisher_trust_removal_tests::fixture();
    db.execute_batch(MIGRATION_V39)
        .expect("State41 preservation fixture");
    db.execute_batch(MIGRATION_V40)
        .expect("State41 preservation fixture");
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
fn state41_preserves_every_cell_adds_no_grants_and_keeps_plan_terms_immutable() {
    let db = fixture();
    let before = publisher_trust_removal_tests::snapshot(&db);
    db.execute_batch(MIGRATION_V41)
        .expect("State41 preservation fixture");
    assert_eq!(
        publisher_trust_removal_tests::snapshot(&db),
        expected(before)
    );
    assert!(
        db.execute(
            "UPDATE extension_plans SET installation_terms_json='{}'",
            []
        )
        .is_err()
    );
    assert!(
        db.execute(
            "UPDATE extension_plans SET requested_permissions_json='{}'",
            []
        )
        .is_err()
    );
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .expect("State41 preservation fixture"),
        41
    );
    assert!(
        db.prepare("PRAGMA foreign_key_check")
            .expect("State41 preservation fixture")
            .query([])
            .expect("State41 preservation fixture")
            .next()
            .expect("State41 preservation fixture")
            .is_none()
    );
}
#[test]
fn state41_failure_rolls_back_column_trigger_rows_and_version() {
    for marker in [
        "DROP TRIGGER extension_plans_immutable;",
        "PRAGMA user_version = 41;",
    ] {
        let db = fixture();
        let before = publisher_trust_removal_tests::snapshot(&db);
        let trigger: String = db
            .query_row(
                "SELECT sql FROM sqlite_schema WHERE name='extension_plans_immutable'",
                [],
                |r| r.get(0),
            )
            .expect("State41 preservation fixture");
        let broken = MIGRATION_V41.replace(
            marker,
            &format!("{marker}\nSELECT nonexistent_state41_column;"),
        );
        assert!(db.execute_batch(&broken).is_err());
        db.execute_batch("ROLLBACK")
            .expect("State41 preservation fixture");
        assert_eq!(publisher_trust_removal_tests::snapshot(&db), before);
        assert_eq!(
            db.query_row(
                "SELECT sql FROM sqlite_schema WHERE name='extension_plans_immutable'",
                [],
                |r| r.get::<_, String>(0)
            )
            .expect("State41 preservation fixture"),
            trigger
        );
        assert_eq!(
            db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .expect("State41 preservation fixture"),
            40
        );
    }
}
#[test]
fn state41_real_initializer_reopens_without_rewriting_data_or_grants() {
    let db = fixture();
    let before = publisher_trust_removal_tests::snapshot(&db);
    let path = std::env::temp_dir().join(format!("alcomd-state41-{}.db", Uuid::new_v4()));
    db.execute(
        "VACUUM INTO ?1",
        [path.to_str().expect("State41 preservation fixture")],
    )
    .expect("State41 preservation fixture");
    drop(db);
    for _ in 0..2 {
        let db = initialize_connection(&path).expect("State41 preservation fixture");
        assert_eq!(
            publisher_trust_removal_tests::snapshot(&db),
            expected(expected(before.clone()))
        );
        assert_eq!(
            db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .expect("State41 preservation fixture"),
            CURRENT_DATA_SCHEMA
        );
    }
    std::fs::remove_file(path).expect("State41 preservation fixture");
}
