use super::*;
use rusqlite::types::Value;

const EXTENSION: &str = "dev.alcomd.state35";
fn fixture() -> Connection {
    let db = mcp_state32_fixture();
    db.execute_batch(MIGRATION_V33).expect("33");
    db.execute_batch(MIGRATION_V34).expect("34");
    let fingerprint = format!("ed25519-sha256:{}", "1".repeat(64));
    db.execute("INSERT INTO extensions(extension_id,version,api_major,package_digest,manifest_digest,component_digest,publisher_fingerprint,trust_decision,principal_id,live_package_locator,desired_state,quarantine_state,grant_revision,lifecycle_generation,revision,installed_at_ms,updated_at_ms) VALUES (?1,'1.0.0',1,zeroblob(32),zeroblob(32),zeroblob(32),?2,'user_approved_for_extension','builtin:local-owner','isolated-fixture','enabled','clear',3,2,5,10,20)",params![EXTENSION,fingerprint]).expect("extension");
    db.execute("INSERT INTO extension_instances VALUES (?1,?2,'extension:dev.alcomd.state35',3,2,?3,'running',500,0,10,20)",params![EXTENSION,Uuid::new_v4().to_string(),Uuid::new_v4().to_string()]).expect("instance");
    db.execute(
        "INSERT INTO extension_grants VALUES (?1,'background.run','Extension',?1,'granted',2,12)",
        [EXTENSION],
    )
    .expect("background");
    db.execute(
        "INSERT INTO extension_grants VALUES (?1,'projects.read','Project',?2,'revoked',3,20)",
        params![EXTENSION, Uuid::new_v4().to_string()],
    )
    .expect("revoked project scope");
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
        "INSERT INTO mcp_clients VALUES (?1,'mcp:'||?1,'Kept client',4,10,NULL,2)",
        [&id],
    )
    .expect("MCP client");
    db.execute(
        "INSERT INTO mcp_client_grants VALUES (?1,2,'state.check','global','*')",
        [&id],
    )
    .expect("current snapshot");
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
fn snapshot(db: &Connection) -> Vec<(String, Vec<Vec<Value>>)> {
    let mut rows = mcp_clients_migration_tests::original_state(db);
    for name in ["mcp_clients", "mcp_client_grants", "mcp_client_credentials"] {
        let mut query = db
            .prepare(&format!("SELECT * FROM {name} ORDER BY rowid"))
            .expect("MCP table");
        let count = query.column_count();
        let values = query
            .query_map([], |r| {
                (0..count)
                    .map(|i| r.get(i))
                    .collect::<Result<Vec<Value>, _>>()
            })
            .expect("rows")
            .collect::<Result<Vec<_>, _>>()
            .expect("values");
        rows.push((name.into(), values));
    }
    rows
}
#[test]
fn state35_preserves_every_existing_row_instance_namespace_credential_and_event_sequence() {
    let db = fixture();
    let before = snapshot(&db);
    let sequence: i64 = db
        .query_row(
            "SELECT seq FROM sqlite_sequence WHERE name='events'",
            [],
            |r| r.get(0),
        )
        .expect("sequence");
    db.execute_batch(MIGRATION_V35).expect("State35");
    assert_eq!(snapshot(&db), before);
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .expect("version"),
        35
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
    db.execute("INSERT INTO events(event_id,kind,aggregate_kind,aggregate_id,aggregate_revision,principal_id,occurred_at_ms,payload_json) VALUES (?1,'state.check.succeeded','operation',?2,1,'builtin:local-owner',21,'{}')",params![Uuid::new_v4().to_string(),Uuid::new_v4().to_string()]).expect("next event");
    assert_eq!(db.last_insert_rowid(), sequence + 1);
}
#[test]
fn state35_adds_only_exact_management_combinations_and_keeps_pk_fk_and_old_checks() {
    let db = fixture();
    db.execute_batch(MIGRATION_V35).expect("35");
    for permission in ["mcp.clients.read", "mcp.clients.manage"] {
        db.execute(
            "INSERT INTO extension_grants VALUES (?1,?2,'Global','*','granted',3,20)",
            params![EXTENSION, permission],
        )
        .expect("approved grant");
        assert!(
            db.execute(
                "INSERT INTO extension_grants VALUES (?1,?2,'Global','*','granted',3,20)",
                params![EXTENSION, permission]
            )
            .is_err()
        );
    }
    for (permission, kind, id) in [
        ("background.run", "Global", "*"),
        ("projects.read", "Global", "*"),
        (
            "mcp.clients.manage",
            "Project",
            "00000000-0000-4000-8000-000000000001",
        ),
        ("mcp.clients.read", "Global", "any"),
        ("mcp.clients.read", "Extension", EXTENSION),
        ("unknown.permission", "Global", "*"),
    ] {
        assert!(
            db.execute(
                "INSERT INTO extension_grants VALUES (?1,?2,?3,?4,'granted',3,20)",
                params![EXTENSION, permission, kind, id]
            )
            .is_err()
        );
    }
    assert!(db.execute("INSERT INTO extension_grants VALUES ('dev.alcomd.missing','mcp.clients.read','Global','*','granted',1,20)",[]).is_err());
    db.execute("DELETE FROM extensions WHERE extension_id=?1", [EXTENSION])
        .expect("existing cascade");
    assert_eq!(
        db.query_row("SELECT count(*) FROM extension_grants", [], |r| r
            .get::<_, i64>(0))
            .expect("count"),
        0
    );
}
#[test]
fn state35_failed_rebuild_rolls_back_durably_then_initialization_reopens_current_state() {
    let db = fixture();
    let before = snapshot(&db);
    let path = std::env::temp_dir().join(format!("alcomd-state35-{}.db", Uuid::new_v4()));
    db.execute("VACUUM INTO ?1", [path.to_str().expect("fixture path")])
        .expect("copy isolated State34");
    drop(db);
    let db = Connection::open(&path).expect("open isolated fixture");
    db.execute_batch("PRAGMA foreign_keys=ON").expect("FK");
    let fault = MIGRATION_V35.replace(
        "DROP TABLE extension_grants;",
        "SELECT deliberately_missing_state35_column;",
    );
    assert!(db.execute_batch(&fault).is_err());
    assert!(!db.is_autocommit());
    drop(db);
    let db = Connection::open(&path).expect("reopen after failed transaction");
    assert_eq!(snapshot(&db), before);
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .expect("unchanged version"),
        34
    );
    assert!(
        !db.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='extension_grants_v35')",
            [],
            |r| r.get::<_, bool>(0)
        )
        .expect("no partial table")
    );
    drop(db);
    for _ in 0..2 {
        let db = initialize_connection(&path).expect("actual initializer upgrade/reopen");
        assert_eq!(snapshot(&db), before);
        assert_eq!(
            db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .expect("version"),
            35
        );
    }
    std::fs::remove_file(&path).expect("owned database cleanup");
}
