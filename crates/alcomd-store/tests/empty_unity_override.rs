use std::fs;
use std::path::{Path, PathBuf};

use alcomd_store::{CURRENT_DATA_SCHEMA, StateStoreHandle, StoreOpenError};
use rusqlite::{Connection, params, types::Value};
use uuid::Uuid;

const MIGRATION_V17: &str = include_str!("../migrations/0017_empty_unity_launch_override.sql");
const PROJECT_ID: &str = "00000000-0000-4000-8000-000000000301";
const INHERITING_PROJECT_ID: &str = "00000000-0000-4000-8000-000000000302";

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("alcomd-state17-test-{}", Uuid::new_v4()));
        fs::create_dir(&path).expect("isolated fixture directory");
        Self(path)
    }

    fn database(&self) -> PathBuf {
        self.0.join("state.db")
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove isolated fixture");
    }
}

#[test]
fn state16_upgrade_preserves_every_existing_row_and_empty_override_survives_reopen() {
    let directory = TestDirectory::new();
    let path = directory.database();
    let connection = fixture(&path);
    let before = row_snapshot(&connection);
    let sequence_before = integer(
        &connection,
        "SELECT seq FROM sqlite_sequence WHERE name='events'",
    );
    drop(connection);

    let store = StateStoreHandle::open(path.clone()).expect("registered State16 to latest upgrade");
    let connection = Connection::open(&path).expect("inspect upgraded fixture");
    assert_eq!(integer(&connection, "PRAGMA user_version"), 18);
    assert_eq!(CURRENT_DATA_SCHEMA, 18);
    assert_eq!(row_snapshot(&connection), before);
    assert_eq!(
        integer(
            &connection,
            "SELECT seq FROM sqlite_sequence WHERE name='events'"
        ),
        sequence_before
    );
    assert_eq!(
        integer(
            &connection,
            "SELECT count(*) FROM project_unity_launch_config"
        ),
        1
    );
    assert_eq!(
        integer(&connection, "SELECT count(*) FROM pragma_foreign_key_check"),
        0
    );
    connection
        .execute(
            "INSERT INTO project_unity_launch_config VALUES (?1,'[]',1,0)",
            [INHERITING_PROJECT_ID],
        )
        .expect("explicit empty override");
    let accepted = row_snapshot(&connection);
    drop(connection);
    drop(store);

    let reopened = StateStoreHandle::open(path.clone()).expect("reopen latest state");
    let connection = Connection::open(&path).expect("inspect reopened fixture");
    assert_eq!(row_snapshot(&connection), accepted);
    assert_eq!(integer(&connection, "PRAGMA user_version"), 18);
    assert_eq!(
        integer(&connection, "SELECT count(*) FROM pragma_foreign_key_check"),
        0
    );
    drop(connection);
    drop(reopened);
}

#[test]
fn state17_keeps_strict_array_bounds_revision_clock_and_cascading_foreign_key() {
    let directory = TestDirectory::new();
    let connection = fixture(&directory.database());
    connection.execute_batch(MIGRATION_V17).expect("State17");
    assert_eq!(integer(&connection, "PRAGMA foreign_keys"), 1);
    assert_eq!(
        integer(
            &connection,
            "SELECT strict FROM pragma_table_list WHERE name='project_unity_launch_config'"
        ),
        1
    );
    for invalid in [
        "{",
        "{}",
        "null",
        "1",
        "\"string\"",
        &serde_json::to_string(&vec!["arg"; 65]).expect("65 arguments"),
        &serde_json::to_string(&vec!["a".repeat(65536)]).expect("oversized JSON"),
    ] {
        assert!(
            connection
                .execute(
                    "INSERT INTO project_unity_launch_config VALUES (?1,?2,1,0)",
                    params![INHERITING_PROJECT_ID, invalid]
                )
                .is_err(),
            "invalid array must fail"
        );
    }
    for (revision, clock) in [(0, 0), (-1, 0), (1, -1)] {
        assert!(
            connection
                .execute(
                    "INSERT INTO project_unity_launch_config VALUES (?1,'[]',?2,?3)",
                    params![INHERITING_PROJECT_ID, revision, clock]
                )
                .is_err()
        );
    }
    assert!(
        connection
            .execute(
                "INSERT INTO project_unity_launch_config VALUES (?1,x'00',1,0)",
                [INHERITING_PROJECT_ID]
            )
            .is_err()
    );
    assert!(connection.execute("INSERT INTO project_unity_launch_config VALUES ('00000000-0000-4000-8000-000000000399','[]',1,0)", []).is_err());
    let full = serde_json::to_string(&vec!["arg"; 64]).expect("64 arguments");
    connection
        .execute(
            "INSERT INTO project_unity_launch_config VALUES (?1,?2,1,0)",
            params![INHERITING_PROJECT_ID, full],
        )
        .expect("64 argument boundary");
    connection
        .execute(
            "DELETE FROM projects WHERE project_id=?1",
            [INHERITING_PROJECT_ID],
        )
        .expect("delete parent");
    assert_eq!(
        integer(
            &connection,
            "SELECT count(*) FROM project_unity_launch_config"
        ),
        1
    );
    assert_eq!(
        integer(&connection, "SELECT count(*) FROM pragma_foreign_key_check"),
        0
    );
}

#[test]
fn failed_state17_copy_rolls_back_without_replacing_legacy_authority() {
    let directory = TestDirectory::new();
    let path = directory.database();
    let connection = fixture(&path);
    connection
        .execute_batch("PRAGMA ignore_check_constraints=ON;")
        .expect("inject corrupt fixture");
    connection
        .execute(
            "UPDATE project_unity_launch_config SET arguments_json='{}'",
            [],
        )
        .expect("corrupt legacy array");
    connection
        .execute_batch("PRAGMA ignore_check_constraints=OFF;")
        .expect("restore checks");
    let before = row_snapshot(&connection);
    drop(connection);
    assert!(matches!(
        StateStoreHandle::open(path.clone()),
        Err(StoreOpenError::Unavailable)
    ));
    let connection = Connection::open(&path).expect("inspect failed upgrade");
    assert_eq!(integer(&connection, "PRAGMA user_version"), 16);
    assert_eq!(row_snapshot(&connection), before);
    assert_eq!(
        integer(
            &connection,
            "SELECT count(*) FROM sqlite_schema WHERE name='project_unity_launch_config_v17'"
        ),
        0
    );
    let ddl: String = connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE name='project_unity_launch_config'",
            [],
            |row| row.get(0),
        )
        .expect("legacy table authority");
    assert!(ddl.contains("BETWEEN 1 AND 64"));
}

fn fixture(path: &Path) -> Connection {
    let connection = Connection::open(path).expect("isolated State16 fixture");
    connection
        .execute_batch("PRAGMA foreign_keys=ON;")
        .expect("foreign keys");
    for migration in [
        include_str!("../migrations/0001_state.sql"),
        include_str!("../migrations/0002_projects_repositories.sql"),
        include_str!("../migrations/0003_package_transactions.sql"),
        include_str!("../migrations/0004_local_workflows.sql"),
        include_str!("../migrations/0005_template_plans.sql"),
        include_str!("../migrations/0006_backup_create.sql"),
        include_str!("../migrations/0007_backup_restore.sql"),
        include_str!("../migrations/0008_extension_runtime.sql"),
        include_str!("../migrations/0009_portable_extension_ui.sql"),
        include_str!("../migrations/0010_project_copy.sql"),
        include_str!("../migrations/0011_project_preferences.sql"),
        include_str!("../migrations/0012_package_functional_closure.sql"),
        include_str!("../migrations/0013_project_directory_delete.sql"),
        include_str!("../migrations/0014_unity_project_version.sql"),
        include_str!("../migrations/0015_project_last_modified.sql"),
        include_str!("../migrations/0016_remove_project_observed.sql"),
    ] {
        connection
            .execute_batch(migration)
            .expect("authentic historical migration");
    }
    for (id, identity, path) in [
        (PROJECT_ID, vec![1_u8], "X:/fixture/custom"),
        (INHERITING_PROJECT_ID, vec![2_u8], "X:/fixture/inherited"),
    ] {
        connection.execute("INSERT INTO projects (project_id,owner_principal_id,root_path,path_identity_key,project_type,unity_version,snapshot_json,revision,registered_at_ms,updated_at_ms,last_modified_at_ms) VALUES (?1,'builtin:local-owner',?2,?3,'unknown','2022.3.22f1','{}',9,101,303,404)", params![id,path,identity]).expect("existing project");
    }
    connection
        .execute(
            "INSERT INTO project_unity_launch_config VALUES (?1,?2,7,123456)",
            params![
                PROJECT_ID,
                r#"[ "-logFile", "C:\\User Data\\a \"quoted\".log" ]"#
            ],
        )
        .expect("existing custom arguments with original JSON bytes");
    connection.execute_batch(r#"INSERT INTO operations (operation_id,kind,state,revision,owner_principal_id,request_json,result_json,created_at_ms,updated_at_ms,completed_at_ms) VALUES ('00000000-0000-4000-8000-000000000311','state.check','succeeded',1,'builtin:local-owner','{}','{}',1,2,2);
        INSERT INTO events (event_id,kind,aggregate_kind,aggregate_id,aggregate_revision,principal_id,occurred_at_ms,payload_json) VALUES ('00000000-0000-4000-8000-000000000321','state.check.succeeded','operation','00000000-0000-4000-8000-000000000311',1,'builtin:local-owner',2,'{}');
        INSERT INTO idempotency_records (principal_id,method,idempotency_key,request_fingerprint,state,operation_id,response_json,created_at_ms) VALUES ('builtin:local-owner','unity.projectLaunchConfig.set','retained-key','{}','completed',NULL,'{ "retained": true }',1);
        UPDATE sqlite_sequence SET seq=99 WHERE name='events';"#).expect("unrelated operation event and replay authority");
    assert_eq!(integer(&connection, "PRAGMA user_version"), 16);
    connection
}

fn integer(connection: &Connection, sql: &str) -> i64 {
    connection
        .query_row(sql, [], |row| row.get(0))
        .expect("integer query")
}

fn row_snapshot(connection: &Connection) -> Vec<(String, Vec<Vec<Value>>)> {
    let names: Vec<String> = connection.prepare("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name").expect("tables").query_map([], |row| row.get(0)).expect("table names").map(|row| row.expect("table name")).collect();
    names
        .into_iter()
        .map(|name| {
            let mut statement = connection
                .prepare(&format!(
                    "SELECT * FROM \"{}\" ORDER BY rowid",
                    name.replace('"', "\"\"")
                ))
                .expect("table rows");
            let count = statement.column_count();
            let rows = statement
                .query_map([], |row| {
                    (0..count)
                        .map(|index| row.get(index))
                        .collect::<rusqlite::Result<Vec<Value>>>()
                })
                .expect("row query")
                .map(|row| row.expect("row values"))
                .collect();
            (name, rows)
        })
        .collect()
}
