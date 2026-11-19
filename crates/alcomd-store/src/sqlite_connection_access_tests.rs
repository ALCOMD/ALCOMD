use super::*;
use rusqlite::types::Value;

pub(super) fn expected_state44(
    mut rows: publisher_trust_removal_tests::Rows,
) -> publisher_trust_removal_tests::Rows {
    rows.retain(|(table, _)| table != "mcp_client_grants");
    for (table, values) in &mut rows {
        for row in values {
            match table.as_str() {
                "mcp_clients" => {
                    assert!(matches!(row.remove(6), Value::Integer(_)));
                }
                "mcp_oauth_tokens" => {
                    assert!(matches!(row.remove(9), Value::Text(_)));
                    assert!(matches!(row.remove(8), Value::Integer(_)));
                }
                _ => {}
            }
        }
    }
    rows
}

fn fixture() -> Connection {
    let db = publisher_trust_removal_tests::fixture();
    for sql in [
        MIGRATION_V39,
        MIGRATION_V40,
        MIGRATION_V41,
        MIGRATION_V42,
        MIGRATION_V43,
    ] {
        db.execute_batch(sql).expect("historical State43");
    }
    let registration = Uuid::new_v4().to_string();
    db.execute(
        "INSERT INTO mcp_oauth_registrations SELECT ?1,client_id,'https://localhost:5443','https://localhost:5443/mcp','[]',0,NULL,1,1,10,NULL FROM mcp_clients",
        [&registration],
    ).expect("registration evidence");
    db.execute(
        "INSERT INTO mcp_oauth_tokens VALUES (zeroblob(32),'refresh',?1,1,?2,NULL,NULL,'',2,'[]','https://localhost:5443','https://localhost:5443/mcp',10,20,30,15,17)",
        params![registration, Uuid::new_v4().to_string()],
    ).expect("consumed and revoked token evidence");
    db
}

#[test]
fn state44_preserves_every_unrelated_cell_and_reopens_after_rollback() {
    let db = fixture();
    let before = publisher_trust_removal_tests::snapshot(&db);
    let path = std::env::temp_dir().join(format!("alcomd-state44-{}.db", Uuid::new_v4()));
    db.execute("VACUUM INTO ?1", [path.to_str().expect("fixture path")])
        .expect("durable fixture");
    drop(db);

    let db = Connection::open(&path).expect("open historical state");
    let fault = MIGRATION_V44.replace(
        "PRAGMA user_version",
        "SELECT missing_column;\nPRAGMA user_version",
    );
    assert!(db.execute_batch(&fault).is_err());
    assert!(!db.is_autocommit());
    drop(db);

    let db = Connection::open(&path).expect("reopen after rollback");
    assert_eq!(publisher_trust_removal_tests::snapshot(&db), before);
    assert_eq!(
        db.pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .expect("version"),
        43
    );
    drop(db);

    let expected = expected_state44(before);
    for _ in 0..2 {
        let db = initialize_connection(&path).expect("upgrade and reopen");
        assert_eq!(publisher_trust_removal_tests::snapshot(&db), expected);
        assert_eq!(
            db.pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
                .expect("version"),
            44
        );
        assert!(db.prepare("SELECT * FROM mcp_client_grants").is_err());
        assert!(
            db.prepare("SELECT grant_revision FROM mcp_clients")
                .is_err()
        );
        assert!(
            db.prepare("SELECT grants_json FROM mcp_oauth_tokens")
                .is_err()
        );
        assert!(
            db.prepare("SELECT grant_revision FROM mcp_oauth_tokens")
                .is_err()
        );
        let violations: u32 = db
            .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })
            .expect("FK check");
        assert_eq!(violations, 0);
    }
    std::fs::remove_file(path).expect("owned fixture cleanup");
}
