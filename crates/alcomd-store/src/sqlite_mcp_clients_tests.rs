use super::*;

fn fixture() -> Connection {
    fixture_at(Path::new(":memory:"))
}

fn fixture_at(path: &Path) -> Connection {
    let mut db = state21_fixture(path);
    migrate_remove_templates(&mut db).expect("State22");
    for migration in [
        MIGRATION_V23,
        MIGRATION_V24,
        MIGRATION_V25,
        MIGRATION_V26,
        MIGRATION_V27,
    ] {
        db.execute_batch(migration).expect("historical migration");
    }
    migrate_template_archives(&mut db).expect("State28");
    for migration in [MIGRATION_V29, MIGRATION_V30, MIGRATION_V31, MIGRATION_V32] {
        db.execute_batch(migration).expect("State32");
    }
    db.execute("INSERT INTO repositories(repository_id,owner_principal_id,source_kind,source_locator,source_identity_key,issues_json,revision,registered_at_ms,refreshed_at_ms,updated_at_ms,priority,resolver_indexed,etag,last_modified,display_name) VALUES ('11111111-1111-4111-8111-111111111111','builtin:local-owner','remote','https://fixture.invalid/index.json',X'010203','[]',9,1,2,3,7,1,'etag','last','Kept')", []).expect("existing resource");
    db
}

fn original_state(db: &Connection) -> Vec<(String, Vec<Vec<rusqlite::types::Value>>)> {
    let mut schema = db.prepare("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'mcp_%' ORDER BY name").expect("tables");
    let names: Vec<String> = schema
        .query_map([], |r| r.get(0))
        .expect("names")
        .collect::<Result<_, _>>()
        .expect("table names");
    names
        .into_iter()
        .map(|name| {
            let mut query = db
                .prepare(&format!(
                    "SELECT * FROM \"{}\" ORDER BY rowid",
                    name.replace('"', "\"\"")
                ))
                .expect("existing table");
            let count = query.column_count();
            let rows = query
                .query_map([], |r| {
                    (0..count).map(|i| r.get(i)).collect::<Result<Vec<_>, _>>()
                })
                .expect("rows")
                .collect::<Result<Vec<_>, _>>()
                .expect("values");
            (name, rows)
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
fn state33_preserves_all_existing_rows_and_schema_objects() {
    let db = fixture();
    let rows = original_state(&db);
    let objects: Vec<(String, String)> = db
        .prepare("SELECT name,sql FROM sqlite_schema WHERE sql IS NOT NULL ORDER BY name")
        .expect("schema")
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .expect("objects")
        .collect::<Result<_, _>>()
        .expect("snapshot");
    db.execute_batch(MIGRATION_V33).expect("State33");
    assert_eq!(original_state(&db), rows);
    for (name, sql) in objects {
        assert_eq!(
            db.query_row("SELECT sql FROM sqlite_schema WHERE name=?1", [name], |r| r
                .get::<_, String>(0))
                .expect("preserved object"),
            sql
        );
    }
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .expect("version"),
        33
    );
    let fk: Option<String> = db
        .query_row("PRAGMA foreign_key_check", [], |r| r.get(0))
        .optional()
        .expect("foreign keys");
    assert!(fk.is_none());
}

#[test]
fn state33_bounds_clients_and_grants_and_rejects_orphans() {
    let db = fixture();
    db.execute_batch(MIGRATION_V33).expect("migration");
    let id = client(&db, 1);
    assert_eq!(
        db.query_row("SELECT count(*) FROM mcp_client_grants", [], |r| r
            .get::<_, u32>(0))
            .expect("default grants"),
        0
    );
    assert!(
        db.execute(
            "INSERT INTO mcp_client_grants VALUES ('missing','projects.read','project','id')",
            []
        )
        .is_err()
    );
    for n in 0..64 {
        db.execute(
            "INSERT INTO mcp_client_grants VALUES (?1,?2,'project','id')",
            params![id, format!("permission.{n}")],
        )
        .expect("bounded permission");
    }
    assert!(
        db.execute(
            "INSERT INTO mcp_client_grants VALUES (?1,'extra.permission','project','id')",
            [&id]
        )
        .is_err()
    );
    db.execute("DELETE FROM mcp_client_grants", [])
        .expect("reset isolated grants");
    for n in 0..4096 {
        db.execute(
            "INSERT INTO mcp_client_grants VALUES (?1,'projects.read','project',?2)",
            params![id, n.to_string()],
        )
        .expect("bounded scope");
    }
    assert!(
        db.execute(
            "INSERT INTO mcp_client_grants VALUES (?1,'projects.read','project','extra')",
            [&id]
        )
        .is_err()
    );
    assert!(
        db.execute(
            "INSERT INTO mcp_client_grants VALUES (?1,'projects.read','project','0')",
            [&id]
        )
        .is_err()
    );
    for n in 2..=64 {
        client(&db, n);
    }
    assert!(db.execute("INSERT INTO mcp_clients VALUES ('99999999-1111-4111-8111-111111111111','mcp:99999999-1111-4111-8111-111111111111','Overflow',1,10,NULL)",[]).is_err());
}

#[test]
fn state33_requires_digests_and_atomic_credential_rotation() {
    let mut db = fixture();
    db.execute_batch(MIGRATION_V33).expect("migration");
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
fn state33_failed_migration_rolls_back_all_new_objects() {
    let db = fixture();
    db.execute_batch("CREATE TABLE mcp_client_credentials(sentinel TEXT); INSERT INTO mcp_client_credentials VALUES ('preserved');").expect("conflicting schema");
    let old = original_state(&db);
    assert!(db.execute_batch(MIGRATION_V33).is_err());
    db.execute_batch("ROLLBACK")
        .expect("rollback failed initialization");
    assert_eq!(original_state(&db), old);
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .expect("version"),
        32
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM sqlite_schema WHERE name IN ('mcp_clients','mcp_client_grants')",
            [],
            |r| r.get::<_, u32>(0)
        )
        .expect("no partial tables"),
        0
    );
    assert_eq!(
        db.query_row("SELECT sentinel FROM mcp_client_credentials", [], |r| {
            r.get::<_, String>(0)
        })
        .expect("pre-existing data"),
        "preserved"
    );
}

#[test]
fn state33_initialization_upgrades_and_reopens_an_existing_database() {
    let path = std::env::temp_dir().join(format!("alcomd-state33-{}.db", Uuid::new_v4()));
    let db = fixture_at(&path);
    let before = original_state(&db);
    drop(db);
    for _ in 0..2 {
        let db = initialize_connection(&path).expect("actual initialization chain");
        assert_eq!(original_state(&db), before);
        assert_eq!(
            db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .expect("version"),
            33
        );
        assert_eq!(
            db.query_row("SELECT count(*) FROM mcp_clients", [], |r| r
                .get::<_, u32>(0))
                .expect("no implicit client"),
            0
        );
    }
    std::fs::remove_file(&path).expect("remove isolated database");
}
