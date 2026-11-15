use super::*;
use rusqlite::types::Value;

type Rows = Vec<(String, Vec<Vec<Value>>)>;

/// Exact authorized delta, independent of the migration's SQL. Every other
/// cell must stay byte-for-byte identical, including events and sequences.
pub(super) fn expected_extension_delta(mut rows: Rows) -> Rows {
    let affected = rows
        .iter()
        .find(|(name, _)| name == "extension_grants")
        .expect("grants")
        .1
        .iter()
        .filter(|row| row[1] == Value::Text("projects.read".into()))
        .map(|row| row[0].clone())
        .collect::<Vec<_>>();
    for (name, values) in &mut rows {
        match name.as_str() {
            "extension_grants" => {
                values.retain(|row| row[1] != Value::Text("projects.read".into()))
            }
            "extensions" => {
                for row in values.iter_mut().filter(|row| affected.contains(&row[0])) {
                    for column in [12, 14] {
                        let Value::Integer(old) = row[column] else {
                            panic!("revision type")
                        };
                        row[column] = Value::Integer(old + 1);
                    }
                }
            }
            "extension_instances" => {
                for row in values.iter_mut().filter(|row| affected.contains(&row[0])) {
                    row[8] = Value::Integer(1);
                }
            }
            _ => {}
        }
    }
    rows
}

fn fixture() -> Connection {
    let db = extension_mcp_grants_migration_tests::fixture();
    db.execute_batch(MIGRATION_V35).expect("35");
    db.execute_batch(MIGRATION_V36).expect("36");
    db.execute("INSERT INTO extension_grants VALUES ('dev.alcomd.state35','projects.read','Project',?1,'granted',3,20)", [Uuid::new_v4().to_string()]).expect("active old grant");
    db.execute("INSERT INTO extensions SELECT 'dev.alcomd.unaffected',version,api_major,package_digest,manifest_digest,component_digest,publisher_fingerprint,trust_decision,principal_id,live_package_locator,desired_state,quarantine_state,grant_revision,lifecycle_generation,revision,installed_at_ms,updated_at_ms,ui_protocol FROM extensions LIMIT 1",[]).expect("unaffected extension");
    db.execute("INSERT INTO extension_grants VALUES ('dev.alcomd.unaffected','mcp.clients.read','Global','*','granted',3,20)",[]).expect("unaffected grant");
    db
}

fn snapshot(db: &Connection) -> Rows {
    let mut rows = extension_mcp_grants_migration_tests::snapshot(db);
    for name in ["mcp_oauth_registrations", "mcp_oauth_tokens"] {
        let mut query = db
            .prepare(&format!("SELECT * FROM {name} ORDER BY rowid"))
            .expect("OAuth table");
        let count = query.column_count();
        let data = query
            .query_map([], |row| {
                (0..count)
                    .map(|i| row.get(i))
                    .collect::<Result<Vec<Value>, _>>()
            })
            .expect("query")
            .collect::<Result<Vec<_>, _>>()
            .expect("rows");
        rows.push((name.into(), data));
    }
    rows
}

fn with_mcp_scopes() -> (Connection, String, String) {
    let db = fixture();
    let affected = db
        .query_row("SELECT client_id FROM mcp_clients", [], |r| {
            r.get::<_, String>(0)
        })
        .expect("existing client");
    for permission in ["projects.read", "packages.manage"] {
        db.execute(
            "INSERT INTO mcp_client_grants VALUES (?1,2,?2,'project',?3)",
            params![affected, permission, Uuid::new_v4().to_string()],
        )
        .expect("old partial grants");
    }
    let unaffected = Uuid::new_v4().to_string();
    db.execute(
        "INSERT INTO mcp_clients VALUES (?1,'mcp:'||?1,'Unaffected',1,10,NULL,1)",
        [&unaffected],
    )
    .expect("unaffected client");
    db.execute(
        "INSERT INTO mcp_client_grants VALUES (?1,1,'operations.read','global','*')",
        [&unaffected],
    )
    .expect("unaffected grant");
    for client in [&affected, &unaffected] {
        let registration = Uuid::new_v4().to_string();
        db.execute("INSERT INTO mcp_oauth_registrations(oauth_client_id,client_id,issuer,resource,redirect_uris_json,confidential,allow_refresh,revision,created_at_ms) VALUES (?1,?2,'https://localhost:5443','https://localhost:5443/mcp','[\"http://localhost:1234/callback\"]',0,1,1,10)",params![registration,client]).expect("registration");
        let family = Uuid::new_v4().to_string();
        let global=serde_json::json!([{"permission":"operations.read","resourceKind":"global","resourceId":"*"}]).to_string();
        let project=serde_json::json!([{"permission":"projects.read","resourceKind":"project","resourceId":Uuid::new_v4().to_string()}]).to_string();
        for (tag, kind, grants) in if client == &affected {
            vec![
                (1u8, "access", global.clone()),
                (2, "refresh", global.clone()),
            ]
        } else {
            vec![(3u8, "access", project), (4, "refresh", global.clone())]
        } {
            db.execute("INSERT INTO mcp_oauth_tokens(token_hash,kind,oauth_client_id,registration_revision,family_id,credential_generation,grant_revision,grants_json,issuer,resource,created_at_ms,expires_at_ms,family_expires_at_ms) VALUES (?1,?2,?3,1,?4,'',1,?5,'https://localhost:5443','https://localhost:5443/mcp',10,100000,200000)",params![[tag;32].as_slice(),kind,registration,family,grants]).expect("old token family");
        }
        if client == &unaffected {
            db.execute("INSERT INTO mcp_oauth_tokens(token_hash,kind,oauth_client_id,registration_revision,family_id,credential_generation,grant_revision,grants_json,issuer,resource,created_at_ms,expires_at_ms,family_expires_at_ms) VALUES (?1,'access',?2,1,?3,'',1,?4,'https://localhost:5443','https://localhost:5443/mcp',10,100000,200000)",params![[5u8;32].as_slice(),registration,Uuid::new_v4().to_string(),global]).expect("unaffected family");
        }
    }
    (db, affected, unaffected)
}

#[test]
fn state37_current_mcp_snapshot_drops_old_scopes_and_revokes_related_oauth_families_only() {
    let (db, affected, unaffected) = with_mcp_scopes();
    let history = crate::m8_mcp::grant_snapshot(&db, &affected, 2).expect("old snapshot");
    let unaffected_before = crate::m8_mcp::client(&db, &unaffected).expect("unaffected");
    let credentials = snapshot(&db)
        .into_iter()
        .find(|(name, _)| name == "mcp_client_credentials")
        .expect("credential hashes");
    db.execute_batch(MIGRATION_V37).expect("37");
    let current = crate::m8_mcp::client(&db, &affected).expect("current");
    assert_eq!(current.revision, 5);
    assert_eq!(current.grants.len(), 1);
    assert_eq!(current.grants[0].permission, "state.check");
    assert_eq!(
        crate::m8_mcp::grant_snapshot(&db, &affected, 2).expect("retained history"),
        history
    );
    assert_eq!(
        crate::m8_mcp::client(&db, &unaffected).expect("unchanged"),
        unaffected_before
    );
    assert_eq!(
        snapshot(&db)
            .into_iter()
            .find(|(name, _)| name == "mcp_client_credentials")
            .expect("same credentials"),
        credentials
    );
    for tag in 1u8..=4 {
        let revoked = db
            .query_row(
                "SELECT revoked_at_ms>=created_at_ms FROM mcp_oauth_tokens WHERE token_hash=?1",
                [[tag; 32].as_slice()],
                |r| r.get::<_, bool>(0),
            )
            .expect("revoked family");
        assert!(revoked);
        assert!(
            crate::m8_oauth::authenticate(
                &db,
                &alcomd_application::OAuthAccessGuard::new(
                    [tag; 32],
                    "https://localhost:5443/mcp".into()
                ),
                20
            )
            .is_err()
        );
    }
    assert!(
        crate::m8_oauth::authenticate(
            &db,
            &alcomd_application::OAuthAccessGuard::new(
                [5; 32],
                "https://localhost:5443/mcp".into()
            ),
            20
        )
        .is_ok(),
        "unaffected family remains usable"
    );
    assert_eq!(db.query_row("SELECT count(*) FROM mcp_client_grants WHERE client_id=?1 AND grant_revision=5 AND resource_kind='project'",[&affected],|r|r.get::<_,u32>(0)).expect("no scope or widening"),0);
}

#[test]
fn state37_revokes_old_project_grants_and_leases_without_widening_or_other_data_changes() {
    let db = fixture();
    let expected = expected_extension_delta(snapshot(&db));
    db.execute_batch(MIGRATION_V37).expect("37");
    assert_eq!(snapshot(&db), expected);
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .expect("version"),
        37
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM extension_grants WHERE permission_name='projects.read'",
            [],
            |r| r.get::<_, u32>(0)
        )
        .expect("no widening"),
        0
    );
    db.execute("INSERT INTO extension_grants VALUES ('dev.alcomd.state35','projects.read','Global','*','granted',4,21)",[]).expect("explicit global read");
    for (kind, id) in [
        ("Project", Uuid::new_v4().to_string()),
        ("Global", "other".into()),
        ("Extension", "dev.alcomd.state35".into()),
    ] {
        assert!(db.execute("INSERT INTO extension_grants VALUES ('dev.alcomd.state35','projects.read',?1,?2,'granted',4,21)",params![kind,id]).is_err());
    }
    assert!(db.execute("INSERT INTO extension_grants VALUES ('dev.alcomd.state35','projects.read','Global','*','granted',4,21)",[]).is_err(),"PK stays unique");
    assert!(db.execute("INSERT INTO extension_grants VALUES ('dev.alcomd.missing','projects.read','Global','*','granted',4,21)",[]).is_err(),"FK preserved");
    assert!(
        db.prepare("PRAGMA foreign_key_check")
            .expect("FK")
            .query([])
            .expect("query")
            .next()
            .expect("row")
            .is_none()
    );
}

#[test]
fn state37_fault_rolls_back_deleted_grants_revisions_leases_and_table_rebuild() {
    let (db, _, _) = with_mcp_scopes();
    let before = snapshot(&db);
    let fault = MIGRATION_V37.replace(
        "PRAGMA user_version=37;",
        "SELECT nonexistent_state37_column; PRAGMA user_version=37;",
    );
    assert!(db.execute_batch(&fault).is_err());
    db.execute_batch("ROLLBACK;").expect("rollback");
    assert_eq!(snapshot(&db), before);
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .expect("version"),
        36
    );
    assert!(
        !db.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='extension_grants_v37')",
            [],
            |r| r.get::<_, bool>(0)
        )
        .expect("no partial table")
    );
}

#[test]
fn state37_real_initializer_migrates_once_and_reopens_without_regranting() {
    let db = fixture();
    let expected = expected_extension_delta(snapshot(&db));
    let path = std::env::temp_dir().join(format!("alcomd-state37-{}.db", Uuid::new_v4()));
    db.execute("VACUUM INTO ?1", [path.to_str().expect("path")])
        .expect("isolated copy");
    drop(db);
    for _ in 0..2 {
        let db = initialize_connection(&path).expect("initializer");
        assert_eq!(snapshot(&db), expected);
        assert_eq!(
            db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .expect("version"),
            CURRENT_DATA_SCHEMA
        );
    }
    std::fs::remove_file(path).expect("owned fixture cleanup");
}
