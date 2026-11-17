use super::*;
use rusqlite::types::Value;

pub(super) type Rows = Vec<(String, Vec<Vec<Value>>)>;

pub(super) fn fixture() -> Connection {
    let db = extension_mcp_grants_migration_tests::fixture();
    for migration in [MIGRATION_V35, MIGRATION_V36, MIGRATION_V37, MIGRATION_V38] {
        db.execute_batch(migration).expect("historical State38");
    }
    db.execute_batch("INSERT INTO extension_plans(plan_id,owner_principal_id,action,state,extension_id,version,api_major,profile_version,source_kind,source_locator,source_identity,package_digest,manifest_digest,component_digest,publisher_fingerprint,trust_decision,requested_permissions_json,requested_interfaces_json,data_disposition,plan_fingerprint,created_at_ms,ui_protocol) SELECT '11111111-2222-4333-8444-555555555555',principal_id,'install','unapplied','dev.example.frozen-plan',version,api_major,1,'local_owner_selected','isolated-package',X'01',package_digest,manifest_digest,component_digest,publisher_fingerprint,trust_decision,'{\"required\":[],\"optional\":[]}','{\"required\":[],\"optional\":[]}','not_applicable',zeroblob(32),20,ui_protocol FROM extensions;").expect("nonempty frozen plan");
    db.execute_batch("INSERT INTO extension_mcp_calls SELECT '22222222-2222-4222-8222-222222222222',principal_id,'fixture_apply','preserved-key',printf('%064d',0),'dev.alcomd.state35','preserved-publisher',printf('%064d',0),'[\"packages.manage\"]','apply','finished','{\"kind\":\"apply\",\"operationId\":\"33333333-3333-4333-8333-333333333333\"}',30,31 FROM mcp_clients LIMIT 1;").expect("accepted tool receipt");
    db
}

pub(super) fn snapshot(db: &Connection) -> Rows {
    let mut names = db
        .prepare("SELECT name FROM sqlite_schema WHERE type='table' ORDER BY name")
        .expect("tables");
    let names = names
        .query_map([], |r| r.get::<_, String>(0))
        .expect("table names")
        .collect::<Result<Vec<_>, _>>()
        .expect("names");
    names
        .into_iter()
        .map(|name| {
            let mut query = db
                .prepare(&format!(
                    "SELECT * FROM \"{}\" ORDER BY rowid",
                    name.replace('"', "\"\"")
                ))
                .expect("rows");
            let count = query.column_count();
            let rows = query
                .query_map([], |r| {
                    (0..count)
                        .map(|n| r.get(n))
                        .collect::<Result<Vec<Value>, _>>()
                })
                .expect("query")
                .collect::<Result<Vec<_>, _>>()
                .expect("cells");
            (name, rows)
        })
        .collect()
}

// Only current-initializer tests apply this approved deletion delta. Historical
// migration-to-migration preservation comparisons still include both old fields.
pub(super) fn expected_state39(mut rows: Rows) -> Rows {
    for (table, values) in &mut rows {
        let index = match table.as_str() {
            "extensions" => 7,
            "extension_plans" => 16,
            _ => continue,
        };
        for row in values {
            let removed = row.remove(index);
            assert!(
                matches!(removed, Value::Text(_)),
                "only the approved trust column"
            );
        }
    }
    rows
}

fn columns(db: &Connection, table: &str) -> Vec<String> {
    db.prepare(&format!("PRAGMA table_info({table})"))
        .expect("column metadata")
        .query_map([], |r| r.get(1))
        .expect("columns")
        .collect::<Result<Vec<_>, _>>()
        .expect("names")
}

#[test]
fn state39_drops_only_approved_columns_and_preserves_every_other_cell_and_object() {
    let db = fixture();
    let expected = expected_state39(snapshot(&db));
    let old_columns = ["extensions", "extension_plans"].map(|table| {
        let mut names = columns(&db, table);
        names.retain(|name| name != "trust_decision");
        names
    });
    let objects = |db: &Connection| {
        db.prepare("SELECT type,name,sql FROM sqlite_schema WHERE type IN ('trigger','index') AND name<>'extension_plans_immutable' ORDER BY type,name")
            .expect("objects")
            .query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,Option<String>>(2)?)))
            .expect("objects query").collect::<Result<Vec<_>,_>>().expect("object values")
    };
    let before_objects = objects(&db);
    db.execute_batch(MIGRATION_V39).expect("approved migration");
    assert_eq!(snapshot(&db), expected);
    assert_eq!(objects(&db), before_objects);
    for (table, expected) in ["extensions", "extension_plans"]
        .into_iter()
        .zip(old_columns)
    {
        assert_eq!(columns(&db, table), expected);
    }
    assert!(
        db.prepare("PRAGMA foreign_key_check")
            .expect("FK query")
            .query([])
            .expect("FK rows")
            .next()
            .expect("FK row")
            .is_none()
    );
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .expect("version"),
        39
    );
}

#[test]
fn state39_plan_immutability_keeps_source_identity_permissions_and_original_confirmation() {
    let db = fixture();
    db.execute_batch(MIGRATION_V39).expect("approved migration");
    let before = snapshot(&db);
    for mutation in [
        "publisher_fingerprint='ed25519-sha256:'||printf('%064d',0)",
        "source_locator='replaced-source'",
        "package_digest=randomblob(32)",
        "requested_permissions_json='{}'",
        "owner_principal_id='other-owner'",
        "plan_fingerprint=randomblob(32)",
        "state='applied'",
        "ui_protocol='portable-v1'",
    ] {
        assert!(
            db.execute(&format!("UPDATE extension_plans SET {mutation}"), [])
                .is_err(),
            "{mutation}"
        );
        assert_eq!(snapshot(&db), before);
    }
    assert!(db.execute("DELETE FROM extension_plans", []).is_err());
    let trigger: String = db
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE name='extension_plans_immutable'",
            [],
            |r| r.get(0),
        )
        .expect("immutable trigger");
    let previous = include_str!("../migrations/0009_portable_extension_ui.sql");
    let expected = previous[previous
        .find("CREATE TRIGGER extension_plans_immutable")
        .expect("trigger start")
        ..previous.find("PRAGMA user_version").expect("version")]
        .replace("    AND OLD.trust_decision = NEW.trust_decision\n", "");
    assert_eq!(
        trigger.trim_end_matches(';').trim(),
        expected.trim().trim_end_matches(';')
    );
}

#[test]
fn state39_each_drop_failure_rolls_back_columns_trigger_rows_and_version() {
    for marker in [
        "ALTER TABLE extensions DROP COLUMN trust_decision;",
        "ALTER TABLE extension_plans DROP COLUMN trust_decision;",
    ] {
        let db = fixture();
        let before = snapshot(&db);
        let old_trigger: String = db
            .query_row(
                "SELECT sql FROM sqlite_schema WHERE name='extension_plans_immutable'",
                [],
                |r| r.get(0),
            )
            .expect("trigger");
        let sql = MIGRATION_V39.replace(
            marker,
            &format!("{marker}\nSELECT nonexistent_state39_column;"),
        );
        assert!(db.execute_batch(&sql).is_err());
        db.execute_batch("ROLLBACK").expect("rollback");
        assert_eq!(snapshot(&db), before);
        assert!(columns(&db, "extensions").contains(&"trust_decision".into()));
        assert!(columns(&db, "extension_plans").contains(&"trust_decision".into()));
        assert_eq!(
            db.query_row(
                "SELECT sql FROM sqlite_schema WHERE name='extension_plans_immutable'",
                [],
                |r| r.get::<_, String>(0)
            )
            .expect("restored trigger"),
            old_trigger
        );
        assert_eq!(
            db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .expect("version"),
            38
        );
    }
}

#[test]
fn state39_real_initializer_upgrades_and_reopens_without_any_trust_model() {
    let db = fixture();
    let expected = expected_state39(snapshot(&db));
    let mut expected = expected;
    for (table, rows) in &mut expected {
        if table == "extension_plans" {
            for row in rows {
                row.push(Value::Null);
            }
        }
    }
    let path = std::env::temp_dir().join(format!("alcomd-state39-{}.db", Uuid::new_v4()));
    db.execute("VACUUM INTO ?1", [path.to_str().expect("fixture path")])
        .expect("durable fixture");
    drop(db);
    for _ in 0..2 {
        let db = initialize_connection(&path).expect("upgrade and reopen");
        assert_eq!(snapshot(&db), expected);
        assert_eq!(
            db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .expect("version"),
            CURRENT_DATA_SCHEMA
        );
        assert!(!columns(&db, "extensions").contains(&"trust_decision".into()));
        assert!(!columns(&db, "extension_plans").contains(&"trust_decision".into()));
    }
    std::fs::remove_file(path).expect("owned fixture cleanup");
}
