use std::fs;
use std::path::{Path, PathBuf};

use alcomd_store::{
    CURRENT_DATA_SCHEMA, StateStoreHandle, StoreOpenError, preflight_state_database,
};
use rusqlite::{Connection, params};

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("alcomd-state1-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).expect("isolated state directory");
        Self(root)
    }

    fn database(&self) -> PathBuf {
        self.0.join("state + #% 数据.db")
    }

    fn snapshot(&self) -> Vec<(PathBuf, Vec<u8>)> {
        let mut files = fs::read_dir(&self.0)
            .expect("isolated state directory")
            .map(|entry| {
                let path = entry.expect("entry").path();
                let contents = fs::read(&path).expect("read evidence");
                (path, contents)
            })
            .collect::<Vec<_>>();
        files.sort_unstable();
        files
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("clean up isolated state directory");
    }
}

#[test]
fn bundled_sqlite_version_is_frozen() {
    assert_eq!(rusqlite::version(), "3.53.2");
}

#[test]
fn state1_initializes_and_reopens_preserving_current_rows_and_sequence() {
    let fixture = Directory::new();
    let path = fixture.database();
    preflight_state_database(&path).expect("blank state");
    assert!(fixture.snapshot().is_empty());

    let store = StateStoreHandle::open(path.clone()).expect("new State1");
    drop(store);
    let db = Connection::open(&path).expect("read structure");
    let version: u32 = db
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("stored schema");
    assert_eq!(version, CURRENT_DATA_SCHEMA);
    let journal: String = db
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .expect("journal mode");
    assert_eq!(journal, "wal");
    let actual_tables = db
        .prepare(
            "SELECT name FROM pragma_table_list WHERE schema='main' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .expect("current tables")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query")
        .collect::<Result<Vec<_>, _>>()
        .expect("table names");
    assert_eq!(
        actual_tables,
        [
            "events",
            "extension_crashes",
            "extension_data_items",
            "extension_data_namespaces",
            "extension_entry_calls",
            "extension_filesystem_journal",
            "extension_grants",
            "extension_instances",
            "extension_plans",
            "extensions",
            "idempotency_records",
            "mcp_client_credentials",
            "mcp_client_requests",
            "mcp_clients",
            "mcp_oauth_registrations",
            "mcp_oauth_tokens",
            "operation_journal",
            "operation_log_entries",
            "operations",
            "package_filesystem_journal",
            "package_plans",
            "project_copy_filesystem_journal",
            "project_copy_plans",
            "project_delete_filesystem_journal",
            "project_delete_plans",
            "project_template_revision",
            "project_templates",
            "project_unity_launch_config",
            "project_unity_migration_journal",
            "project_unity_migration_plans",
            "projects",
            "repositories",
            "repository_credential_cleanup",
            "repository_package_versions",
            "unity_installations",
            "user_package_sources",
        ]
    );
    let revision: i64 = db
        .query_row(
            "SELECT last_revision FROM project_template_revision WHERE singleton=1",
            [],
            |row| row.get(0),
        )
        .expect("template revision seed");
    assert_eq!(revision, 0);
    let violations: i64 = db
        .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .expect("foreign key check");
    assert_eq!(violations, 0);

    for (index, kind) in ["backups.create", "backups.restore"]
        .into_iter()
        .enumerate()
    {
        let operation = uuid::Uuid::new_v4().to_string();
        db.execute(
            "INSERT INTO operations(operation_id,kind,state,revision,owner_principal_id,request_json,created_at_ms,updated_at_ms)
             VALUES (?1,?2,'queued',1,'builtin:local-owner','{}',1,1)",
            params![operation, kind],
        ).expect("file task");
        db.execute(
            "INSERT INTO events(event_id,kind,aggregate_kind,aggregate_id,aggregate_revision,principal_id,occurred_at_ms,payload_json)
             VALUES (?1,?2,'operation',?3,1,'builtin:local-owner',1,'{}')",
            params![uuid::Uuid::new_v4().to_string(), kind, operation],
        ).expect("durable file task event");
        assert_eq!(db.last_insert_rowid(), index as i64 + 1);
    }
    db.execute("DELETE FROM events WHERE sequence=2", [])
        .expect("remove last event");
    drop(db);

    let before = fixture.snapshot();
    preflight_state_database(&path).expect("read-only preflight");
    assert_eq!(fixture.snapshot(), before);
    let store = StateStoreHandle::open(path.clone()).expect("reopen current database");
    drop(store);
    let db = Connection::open(&path).expect("read concrete reopened data");
    assert_eq!(
        db.query_row("SELECT count(*) FROM operations", [], |row| row
            .get::<_, u32>(0))
            .expect("file tasks retained"),
        2
    );
    assert_eq!(
        db.query_row(
            "SELECT seq FROM sqlite_sequence WHERE name='events'",
            [],
            |row| row.get::<_, u32>(0)
        )
        .expect("retained event sequence"),
        2
    );
}

#[test]
fn ddl_formatting_is_immaterial_but_constraints_and_indexes_are_required() {
    let ddl = include_str!("../src/schema.sql");
    let formatted = ddl
        .replace("NOT NULL", "not /* required */\n NuLl")
        .replace("CREATE TABLE projects ", "create TABLE PROJECTS ")
        .replace("CREATE INDEX", "create  index");
    for (sql, valid) in [
        (formatted.clone(), true),
        (formatted.replace("'queued'", "'QUEUED'"), false),
        (
            format!("{formatted}\nCREATE INDEX extra_event_time ON events(occurred_at_ms);"),
            false,
        ),
    ] {
        let fixture = Directory::new();
        let path = fixture.database();
        let db = Connection::open(&path).expect("isolated schema");
        db.execute_batch(&sql).expect("valid SQLite DDL");
        db.pragma_update(None, "user_version", CURRENT_DATA_SCHEMA)
            .expect("schema number");
        drop(db);
        let before = fixture.snapshot();
        assert_eq!(preflight_state_database(&path).is_ok(), valid);
        assert_eq!(fixture.snapshot(), before, "preflight is read-only");
        assert_eq!(StateStoreHandle::open(path).map(drop).is_ok(), valid);
        if !valid {
            assert_eq!(
                fixture.snapshot(),
                before,
                "failed open preserved all files"
            );
        }
    }
}

#[test]
fn rejects_unsupported_schema_without_starting_sqlite_recovery() {
    for version in [0, 2, i32::MAX] {
        let fixture = Directory::new();
        let path = fixture.database();
        let old = Connection::open(&path).expect("isolated prior file");
        old.execute_batch(
            "CREATE TABLE unrelated (value TEXT); INSERT INTO unrelated VALUES ('evidence')",
        )
        .expect("rows");
        old.pragma_update(None, "user_version", version)
            .expect("test version");
        drop(old);
        let wal = companion(&path, "-wal");
        let journal = companion(&path, "-journal");
        fs::write(wal, b"prior WAL evidence").expect("WAL evidence");
        fs::write(journal, b"prior journal evidence").expect("journal evidence");
        let before = fixture.snapshot();
        for result in [
            preflight_state_database(&path),
            StateStoreHandle::open(path.clone()).map(drop),
        ] {
            assert!(
                matches!(result, Err(StoreOpenError::UnsupportedDataSchema { found, supported }) if found == version as u32 && supported == CURRENT_DATA_SCHEMA)
            );
            assert_eq!(fixture.snapshot(), before);
        }
    }
}

#[test]
fn rejects_unknown_or_orphaned_state_without_creating_any_companions() {
    let fixture = Directory::new();
    let path = fixture.database();
    let wal = companion(&path, "-wal");
    fs::write(&wal, b"orphaned WAL").expect("orphan");
    let before = fixture.snapshot();
    assert!(preflight_state_database(&path).is_err());
    assert!(StateStoreHandle::open(path.clone()).is_err());
    assert_eq!(fixture.snapshot(), before);
    fs::remove_file(wal).expect("remove fixture evidence");

    fs::write(&path, b"").expect("pre-existing empty file");
    let before = fixture.snapshot();
    assert!(preflight_state_database(&path).is_err());
    assert!(StateStoreHandle::open(path.clone()).is_err());
    assert_eq!(fixture.snapshot(), before);

    fs::write(&path, b"unrelated file contents").expect("arbitrary bytes");
    let before = fixture.snapshot();
    assert!(preflight_state_database(&path).is_err());
    assert!(StateStoreHandle::open(path.clone()).is_err());
    assert_eq!(fixture.snapshot(), before);
    fs::remove_file(&path).expect("remove fixture evidence");

    let unknown = Connection::open(&path).expect("SQLite with unrelated schema");
    unknown
        .execute_batch("CREATE TABLE unrelated(value TEXT)")
        .expect("foreign schema");
    unknown
        .pragma_update(None, "user_version", CURRENT_DATA_SCHEMA)
        .expect("coincident version number");
    drop(unknown);
    let before = fixture.snapshot();
    assert!(preflight_state_database(&path).is_err());
    assert!(StateStoreHandle::open(path).is_err());
    assert_eq!(fixture.snapshot(), before);
}

fn companion(path: &Path, suffix: &str) -> PathBuf {
    let mut companion = path.as_os_str().to_owned();
    companion.push(suffix);
    PathBuf::from(companion)
}
