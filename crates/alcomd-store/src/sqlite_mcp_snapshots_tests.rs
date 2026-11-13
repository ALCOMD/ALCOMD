use super::*;

fn state33() -> Connection {
    let db = super::mcp_clients_migration_tests::fixture();
    db.execute_batch(MIGRATION_V33).expect("original State33");
    db.execute("INSERT INTO mcp_clients VALUES ('11111111-1111-4111-8111-111111111111','mcp:11111111-1111-4111-8111-111111111111','Preserved',7,10,NULL)",[]).expect("old client");
    db.execute("INSERT INTO mcp_client_grants VALUES ('11111111-1111-4111-8111-111111111111','operations.read','global','*')",[]).expect("old grant");
    db.execute("INSERT INTO mcp_client_credentials VALUES ('22222222-2222-4222-8222-222222222222','11111111-1111-4111-8111-111111111111',zeroblob(32),11,NULL)",[]).expect("old digest");
    db
}

#[test]
fn state34_preserves_state33_clients_effective_grants_credentials_and_other_data() {
    let db = state33();
    let before = super::mcp_clients_migration_tests::original_state(&db);
    db.execute_batch(MIGRATION_V34).expect("State34");
    assert_eq!(
        super::mcp_clients_migration_tests::original_state(&db),
        before
    );
    let client: (String, i64, i64) = db
        .query_row(
            "SELECT display_name,revision,grant_revision FROM mcp_clients",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .expect("client");
    assert_eq!(client, ("Preserved".into(), 7, 1));
    let grant: (String, String, String, i64) = db
        .query_row(
            "SELECT permission,resource_kind,resource_id,grant_revision FROM mcp_client_grants",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .expect("grant");
    assert_eq!(
        grant,
        ("operations.read".into(), "global".into(), "*".into(), 1)
    );
    let hash: Vec<u8> = db
        .query_row(
            "SELECT token_hash FROM mcp_client_credentials WHERE revoked_at_ms IS NULL",
            [],
            |r| r.get(0),
        )
        .expect("same digest");
    assert_eq!(hash, vec![0; 32]);
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .expect("version"),
        34
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r
            .get::<_, u32>(
            0
        ))
        .expect("foreign keys"),
        0
    );
}

#[test]
fn state34_snapshots_are_immutable_and_only_current_snapshot_is_effective() {
    let db = state33();
    db.execute_batch(MIGRATION_V34).expect("migration");
    assert!(
        db.execute("UPDATE mcp_client_grants SET permission='state.check'", [])
            .is_err()
    );
    assert!(db.execute("DELETE FROM mcp_client_grants", []).is_err());
    db.execute("INSERT INTO mcp_client_grants VALUES ('11111111-1111-4111-8111-111111111111',8,'events.read','global','*')",[]).expect("new snapshot");
    db.execute("UPDATE mcp_clients SET revision=8,grant_revision=8", [])
        .expect("switch current");
    let active:String=db.query_row("SELECT g.permission FROM mcp_client_grants g JOIN mcp_clients c USING(client_id) WHERE g.grant_revision=c.grant_revision",[],|r|r.get(0)).expect("effective grant");
    assert_eq!(active, "events.read");
    assert_eq!(
        db.query_row("SELECT count(*) FROM mcp_client_grants", [], |r| r
            .get::<_, u32>(0))
            .expect("history retained"),
        2
    );
}

#[test]
fn state34_failure_rolls_back_pointer_column_table_and_version() {
    let db = state33();
    db.execute_batch("CREATE TABLE mcp_client_grants_v33(sentinel TEXT);INSERT INTO mcp_client_grants_v33 VALUES ('keep');").expect("conflict");
    let before = super::mcp_clients_migration_tests::original_state(&db);
    assert!(db.execute_batch(MIGRATION_V34).is_err());
    db.execute_batch("ROLLBACK").expect("rollback");
    assert_eq!(
        super::mcp_clients_migration_tests::original_state(&db),
        before
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM pragma_table_info('mcp_clients') WHERE name='grant_revision'",
            [],
            |r| r.get::<_, u32>(0)
        )
        .expect("no partial column"),
        0
    );
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .expect("old version"),
        33
    );
    assert_eq!(
        db.query_row("SELECT permission FROM mcp_client_grants", [], |r| r
            .get::<_, String>(0))
            .expect("old grant"),
        "operations.read"
    );
}

#[test]
fn state34_preserves_limits_per_snapshot_without_counting_historical_grants_as_active() {
    let db = state33();
    db.execute_batch(MIGRATION_V34).expect("migration");
    db.execute_batch("WITH RECURSIVE n(x) AS (VALUES(0) UNION ALL SELECT x+1 FROM n WHERE x<4095) INSERT INTO mcp_client_grants SELECT '11111111-1111-4111-8111-111111111111',8,'projects.read','project',CAST(x AS TEXT) FROM n;").expect("4096 scopes");
    assert!(db.execute("INSERT INTO mcp_client_grants VALUES ('11111111-1111-4111-8111-111111111111',8,'projects.read','project','overflow')",[]).is_err());
    db.execute("INSERT INTO mcp_client_grants VALUES ('11111111-1111-4111-8111-111111111111',9,'events.read','global','*')",[]).expect("different snapshot has its own bound");
    db.execute_batch("WITH RECURSIVE n(x) AS (VALUES(0) UNION ALL SELECT x+1 FROM n WHERE x<63) INSERT INTO mcp_client_grants SELECT '11111111-1111-4111-8111-111111111111',10,'permission.'||x,'global','*' FROM n;").expect("64 permission kinds");
    assert!(db.execute("INSERT INTO mcp_client_grants VALUES ('11111111-1111-4111-8111-111111111111',10,'extra.permission','global','*')",[]).is_err());
    let active:i64=db.query_row("SELECT count(*) FROM mcp_client_grants g JOIN mcp_clients c USING(client_id) WHERE g.grant_revision=c.grant_revision",[],|r|r.get(0)).expect("only original active snapshot");
    assert_eq!(active, 1);
}
