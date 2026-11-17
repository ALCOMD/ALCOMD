use super::*;

fn fixture() -> Connection {
    let db = publisher_trust_removal_tests::fixture();
    db.execute_batch(MIGRATION_V39).expect("real State39 input");
    db
}
#[test]
fn state40_preserves_all_cells_and_only_adds_two_exact_global_read_grants() {
    let db = fixture();
    let before = publisher_trust_removal_tests::snapshot(&db);
    for permission in ["mcp.connections.read", "mcp.requests.read"] {
        assert!(db.execute("INSERT INTO extension_grants VALUES ('dev.alcomd.state35',?1,'Global','*','granted',3,20)",[permission]).is_err());
    }
    db.execute_batch(MIGRATION_V40).expect("approved migration");
    assert_eq!(publisher_trust_removal_tests::snapshot(&db), before);
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .expect("State40 fixture"),
        40
    );
    for permission in ["mcp.connections.read", "mcp.requests.read"] {
        db.execute("INSERT INTO extension_grants VALUES ('dev.alcomd.state35',?1,'Global','*','granted',3,20)",[permission]).expect("explicit read grant only");
        for (kind, id) in [
            ("Global", "specific-client"),
            ("Project", "00000000-0000-4000-8000-000000000001"),
            ("Extension", "dev.alcomd.state35"),
        ] {
            assert!(db.execute("INSERT INTO extension_grants VALUES ('dev.alcomd.state35',?1,?2,?3,'granted',3,20)",rusqlite::params![permission,kind,id]).is_err());
        }
    }
    assert!(
        db.prepare("PRAGMA foreign_key_check")
            .expect("State40 fixture")
            .query([])
            .expect("State40 fixture")
            .next()
            .expect("State40 fixture")
            .is_none()
    );
}
#[test]
fn state40_ddl_failure_rolls_back_every_row_table_and_version() {
    for marker in [
        "INSERT INTO extension_grants_v40 SELECT * FROM extension_grants;",
        "ALTER TABLE extension_grants_v40 RENAME TO extension_grants;",
    ] {
        let db = fixture();
        let before = publisher_trust_removal_tests::snapshot(&db);
        let old_sql: String = db
            .query_row(
                "SELECT sql FROM sqlite_schema WHERE name='extension_grants'",
                [],
                |r| r.get(0),
            )
            .expect("State40 fixture");
        let broken = MIGRATION_V40.replace(
            marker,
            &format!("{marker}\nSELECT nonexistent_state40_column;"),
        );
        assert!(db.execute_batch(&broken).is_err());
        db.execute_batch("ROLLBACK").expect("State40 fixture");
        assert_eq!(publisher_trust_removal_tests::snapshot(&db), before);
        assert_eq!(
            db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .expect("State40 fixture"),
            39
        );
        assert_eq!(
            db.query_row(
                "SELECT sql FROM sqlite_schema WHERE name='extension_grants'",
                [],
                |r| r.get::<_, String>(0)
            )
            .expect("State40 fixture"),
            old_sql
        );
    }
}
#[test]
fn state40_real_initializer_preserves_data_and_does_not_regrant_on_reopen() {
    let db = fixture();
    let before = publisher_trust_removal_tests::snapshot(&db);
    let mut before = before;
    for (table, rows) in &mut before {
        if table == "extension_plans" {
            for row in rows {
                row.push(rusqlite::types::Value::Null);
            }
        }
    }
    let path = std::env::temp_dir().join(format!("alcomd-state40-{}.db", Uuid::new_v4()));
    db.execute("VACUUM INTO ?1", [path.to_str().expect("State40 fixture")])
        .expect("State40 fixture");
    drop(db);
    for _ in 0..2 {
        let db = initialize_connection(&path).expect("State40 fixture");
        assert_eq!(publisher_trust_removal_tests::snapshot(&db), before);
        assert_eq!(
            db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .expect("State40 fixture"),
            CURRENT_DATA_SCHEMA
        );
    }
    std::fs::remove_file(path).expect("State40 fixture");
}
