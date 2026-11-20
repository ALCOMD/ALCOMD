use std::fs;
use std::path::{Path, PathBuf};

use alcomd_store::{CURRENT_DATA_SCHEMA, StateStoreHandle};
use rusqlite::{Connection, params, types::Value};
use uuid::Uuid;

const PROJECT_ID: &str = "00000000-0000-4000-8000-000000000301";
const INHERITING_PROJECT_ID: &str = "00000000-0000-4000-8000-000000000302";

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("alcomd-unity-override-test-{}", Uuid::new_v4()));
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
fn empty_override_and_event_sequence_survive_reopen() {
    let directory = TestDirectory::new();
    let path = directory.database();
    let connection = fixture(&path);
    let before = row_snapshot(&connection);
    let sequence_before = integer(
        &connection,
        "SELECT seq FROM sqlite_sequence WHERE name='events'",
    );
    drop(connection);

    let store = StateStoreHandle::open(path.clone()).expect("reopen current authority");
    let connection = Connection::open(&path).expect("inspect current fixture");
    assert_eq!(
        integer(&connection, "PRAGMA user_version"),
        i64::from(CURRENT_DATA_SCHEMA)
    );
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
    assert_eq!(
        integer(&connection, "PRAGMA user_version"),
        i64::from(CURRENT_DATA_SCHEMA)
    );
    assert_eq!(
        integer(&connection, "SELECT count(*) FROM pragma_foreign_key_check"),
        0
    );
    drop(connection);
    drop(reopened);
}

#[test]
fn current_schema_keeps_strict_array_bounds_revision_clock_and_cascading_foreign_key() {
    let directory = TestDirectory::new();
    let connection = fixture(&directory.database());
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

fn fixture(path: &Path) -> Connection {
    drop(StateStoreHandle::open(path.to_owned()).expect("initialize isolated fixture"));
    let connection = Connection::open(path).expect("isolated authority fixture");
    connection
        .execute_batch("PRAGMA foreign_keys=ON;")
        .expect("foreign keys");
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
    assert_eq!(
        integer(&connection, "PRAGMA user_version"),
        i64::from(CURRENT_DATA_SCHEMA)
    );
    connection
}

fn integer(connection: &Connection, sql: &str) -> i64 {
    connection
        .query_row(sql, [], |row| row.get(0))
        .expect("integer query")
}

fn row_snapshot(connection: &Connection) -> Vec<(String, Vec<Vec<Value>>)> {
    let names: Vec<String> = connection.prepare("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name NOT IN ('operation_log_entries','templates','template_plans','project_templates','project_template_revision') ORDER BY name").expect("retained business tables").query_map([], |row| row.get(0)).expect("table names").map(|row| row.expect("table name")).collect();
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
