use super::*;

fn fixture() -> Connection {
    let db = extension_mcp_grants_migration_tests::fixture();
    db.execute_batch(MIGRATION_V35).expect("State35");
    db
}
#[test]
fn state36_preserves_all_old_rows_schema_sequence_and_foreign_keys() {
    let db = fixture();
    let before = extension_mcp_grants_migration_tests::snapshot(&db);
    let old_schema: Vec<(String, String)> = db
        .prepare("SELECT name,sql FROM sqlite_schema WHERE sql IS NOT NULL ORDER BY name")
        .expect("schema")
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .expect("rows")
        .collect::<Result<_, _>>()
        .expect("values");
    let sequence: i64 = db
        .query_row(
            "SELECT seq FROM sqlite_sequence WHERE name='events'",
            [],
            |r| r.get(0),
        )
        .expect("sequence");
    db.execute_batch(MIGRATION_V36).expect("State36");
    assert_eq!(extension_mcp_grants_migration_tests::snapshot(&db), before);
    for (name, sql) in old_schema {
        assert_eq!(
            db.query_row("SELECT sql FROM sqlite_schema WHERE name=?1", [name], |r| r
                .get::<_, String>(0))
                .expect("old schema"),
            sql
        )
    }
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .expect("version"),
        36
    );
    assert_eq!(
        db.pragma_query_value(None, "foreign_keys", |r| r.get::<_, i64>(0))
            .expect("FK"),
        1
    );
    assert!(
        db.prepare("PRAGMA foreign_key_check")
            .expect("FK check")
            .query([])
            .expect("rows")
            .next()
            .expect("row")
            .is_none()
    );
    for name in ["mcp_oauth_registrations", "mcp_oauth_tokens"] {
        assert_eq!(
            db.query_row(&format!("SELECT count(*) FROM {name}"), [], |r| r
                .get::<_, i64>(0))
                .expect("new empty table"),
            0
        )
    }
    db.execute("INSERT INTO events(event_id,kind,aggregate_kind,aggregate_id,aggregate_revision,principal_id,occurred_at_ms,payload_json) VALUES (?1,'state.check.succeeded','operation',?2,1,'builtin:local-owner',21,'{}')",params![Uuid::new_v4().to_string(),Uuid::new_v4().to_string()]).expect("next event");
    assert_eq!(db.last_insert_rowid(), sequence + 1);
    assert!(db.execute("INSERT INTO mcp_oauth_registrations(oauth_client_id,client_id,issuer,resource,redirect_uris_json,confidential,allow_refresh,revision,created_at_ms) VALUES (?1,?2,'https://localhost:5443','https://localhost:5443/mcp','[]',0,0,1,1)",params![Uuid::new_v4().to_string(),Uuid::new_v4().to_string()]).is_err(),"unknown Core client FK rejected");
}
#[test]
fn state36_failure_rolls_back_new_tables_without_touching_old_state() {
    let db = fixture();
    let before = extension_mcp_grants_migration_tests::snapshot(&db);
    let injected = MIGRATION_V36.replace(
        "PRAGMA user_version=36;",
        "INSERT INTO nonexistent_oauth_fault VALUES (1); PRAGMA user_version=36;",
    );
    assert!(db.execute_batch(&injected).is_err());
    db.execute_batch("ROLLBACK;").expect("rollback");
    assert_eq!(extension_mcp_grants_migration_tests::snapshot(&db), before);
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .expect("version"),
        35
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM sqlite_schema WHERE name LIKE 'mcp_oauth_%'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .expect("new objects"),
        0
    );
}
