use std::collections::BTreeMap;
use std::path::PathBuf;

use alcomd_application::{IdempotencyKey, PrincipalId};
use rusqlite::{Connection, params, types::Value};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("alcomd-remove-history-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&path).expect("isolated fixture");
        Self(path)
    }
    fn database(&self) -> PathBuf {
        self.0.join("state.db")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let path = self.0.canonicalize().expect("fixture path");
        assert!(path.starts_with(std::env::temp_dir().canonicalize().expect("temporary root")));
        assert!(
            path.file_name()
                .expect("fixture name")
                .to_string_lossy()
                .starts_with("alcomd-remove-history-")
        );
        std::fs::remove_dir_all(&path).expect("remove isolated fixture");
    }
}

const TRIGGERS: &[&str] = &[
    "activities_created",
    "activities_changed",
    "operation_activity_created",
    "operation_activity_required",
    "operation_activity_changed",
];

// This is deletion input only, never an implementation or compatibility path.
fn legacy_fixture(connection: &mut Connection) {
    crate::sqlite::create_state_check(
        connection,
        &PrincipalId::local_owner(),
        &IdempotencyKey::parse("preserved-operation").expect("key"),
        10,
    )
    .expect("business operation");
    let event_sql: String = connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE name='events'",
            [],
            |row| row.get(0),
        )
        .expect("event schema");
    assert!(event_sql.contains("'user-package'"));
    connection.execute_batch("BEGIN; DROP INDEX events_principal_sequence; DROP INDEX events_aggregate_sequence; ALTER TABLE events RENAME TO old_fixture_events;").expect("prepare old event contract");
    connection
        .execute_batch(&event_sql.replace("'user-package'", "'user-package', 'activity'"))
        .expect("old accepted aggregate");
    connection.execute_batch("INSERT INTO events SELECT * FROM old_fixture_events; DROP TABLE old_fixture_events; CREATE INDEX events_principal_sequence ON events(principal_id,sequence); CREATE INDEX events_aggregate_sequence ON events(aggregate_kind,aggregate_id,sequence); CREATE TABLE activities(activity_id TEXT PRIMARY KEY); COMMIT;").expect("legacy input");
    connection
        .execute(
            "INSERT INTO activities VALUES(?1)",
            ["11111111-1111-4111-8111-111111111111"],
        )
        .expect("old history");
    connection.execute("INSERT INTO events(event_id,kind,aggregate_kind,aggregate_id,aggregate_revision,principal_id,occurred_at_ms,payload_json) VALUES(?1,'activity.changed','activity',?2,1,?3,11,'{}')",
        params![uuid::Uuid::new_v4().to_string(), "11111111-1111-4111-8111-111111111111", PrincipalId::local_owner().as_str()]).expect("old history event");
    for name in TRIGGERS {
        connection
            .execute_batch(&format!(
                "CREATE TRIGGER {name} AFTER INSERT ON activities BEGIN SELECT 1; END;"
            ))
            .expect("old trigger identity");
    }
    connection
        .execute_batch("DROP TABLE operation_log_entries;")
        .expect("the isolated State19 fixture has no State21 table");
    connection
        .pragma_update(None, "user_version", 19)
        .expect("old version");
}

fn business_rows(connection: &Connection) -> BTreeMap<String, Vec<Vec<Value>>> {
    let mut tables = connection.prepare("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name NOT IN ('activities','operation_log_entries','templates','template_plans','project_templates','project_template_revision') ORDER BY name").expect("tables");
    let names = tables
        .query_map([], |row| row.get::<_, String>(0))
        .expect("names")
        .collect::<Result<Vec<_>, _>>()
        .expect("names");
    names
        .into_iter()
        .map(|name| {
            let filter = if name == "events" {
                " WHERE aggregate_kind<>'activity'"
            } else {
                ""
            };
            let quoted = name.replace('"', "\"\"");
            let mut query = connection
                .prepare(&format!("SELECT * FROM \"{quoted}\"{filter}"))
                .expect("rows");
            let count = query.column_count();
            let mut rows = query
                .query_map([], |row| {
                    (0..count)
                        .map(|index| row.get::<_, Value>(index))
                        .collect::<Result<Vec<_>, _>>()
                })
                .expect("business rows")
                .collect::<Result<Vec<_>, _>>()
                .expect("business rows");
            // Tables are not rebuilt except Events, whose sequence is its row identity.
            rows.sort_by_key(|row| format!("{row:?}"));
            (name, rows)
        })
        .collect()
}

#[test]
fn new_database_has_no_history_table_or_triggers() {
    let fixture = Fixture::new();
    let connection =
        crate::sqlite::initialize_connection(&fixture.database()).expect("new database");
    assert_eq!(
        connection
            .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            .expect("version"),
        i64::from(crate::CURRENT_DATA_SCHEMA)
    );
    assert_eq!(connection.query_row("SELECT count(*) FROM sqlite_schema WHERE name='activities' OR name LIKE 'operation_activity_%' OR name LIKE 'activities_%'", [], |row| row.get::<_, i64>(0)).expect("removed objects"), 0);
    drop(connection);
}

#[test]
fn state19_removal_preserves_all_business_rows_and_never_reuses_event_sequence() {
    let fixture = Fixture::new();
    let mut connection = crate::sqlite::state21_fixture(&fixture.database());
    legacy_fixture(&mut connection);
    let before = business_rows(&connection);
    let watermark: i64 = connection
        .query_row(
            "SELECT seq FROM sqlite_sequence WHERE name='events'",
            [],
            |row| row.get(0),
        )
        .expect("old sequence");
    drop(connection);
    let connection =
        crate::sqlite::initialize_connection(&fixture.database()).expect("approved removal");
    assert_eq!(business_rows(&connection), before);
    assert_eq!(
        connection
            .query_row(
                "SELECT count(*) FROM events WHERE aggregate_kind='activity'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .expect("removed events"),
        0
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name='activities'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .expect("removed table"),
        0
    );
    for name in TRIGGERS {
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM sqlite_schema WHERE type='trigger' AND name=?1",
                    [name],
                    |row| row.get::<_, i64>(0)
                )
                .expect("removed trigger"),
            0
        );
    }
    connection.execute("INSERT INTO events(event_id,kind,aggregate_kind,aggregate_id,aggregate_revision,principal_id,occurred_at_ms,payload_json) VALUES(?1,'project.refreshed','project',?2,1,?3,12,'{}')",
        params![uuid::Uuid::new_v4().to_string(), uuid::Uuid::new_v4().to_string(), PrincipalId::local_owner().as_str()]).expect("business event remains writable");
    assert_eq!(connection.last_insert_rowid(), watermark + 1);
    drop(connection);
}

#[test]
fn deleting_the_last_history_event_does_not_reset_the_sequence() {
    let fixture = Fixture::new();
    let mut connection = crate::sqlite::state21_fixture(&fixture.database());
    legacy_fixture(&mut connection);
    let watermark: i64 = connection
        .query_row(
            "SELECT seq FROM sqlite_sequence WHERE name='events'",
            [],
            |row| row.get(0),
        )
        .expect("watermark");
    connection
        .execute("DELETE FROM events WHERE aggregate_kind<>'activity'", [])
        .expect("history-only event input");
    drop(connection);
    let connection =
        crate::sqlite::initialize_connection(&fixture.database()).expect("approved removal");
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM events", [], |row| row
                .get::<_, i64>(0))
            .expect("empty events"),
        0
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT seq FROM sqlite_sequence WHERE name='events'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .expect("retained watermark"),
        watermark
    );
    drop(connection);
}

#[test]
fn failure_rolls_back_history_removal_and_preserves_original_version() {
    let fixture = Fixture::new();
    let mut connection = crate::sqlite::state21_fixture(&fixture.database());
    legacy_fixture(&mut connection);
    connection
        .execute_batch("CREATE TABLE events_before_activity_removal(sentinel TEXT);")
        .expect("controlled DDL failure");
    let before = business_rows(&connection);
    drop(connection);
    assert!(crate::sqlite::initialize_connection(&fixture.database()).is_err());
    let connection = Connection::open(fixture.database()).expect("inspect rolled-back fixture");
    assert_eq!(business_rows(&connection), before);
    assert_eq!(
        connection
            .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            .expect("version"),
        19
    );
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM activities", [], |row| row
                .get::<_, i64>(0))
            .expect("old history preserved"),
        1
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT count(*) FROM events WHERE aggregate_kind='activity'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .expect("old event preserved"),
        1
    );
    for name in TRIGGERS {
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM sqlite_schema WHERE type='trigger' AND name=?1",
                    [name],
                    |row| row.get::<_, i64>(0)
                )
                .expect("old trigger preserved"),
            1
        );
    }
    drop(connection);
}
