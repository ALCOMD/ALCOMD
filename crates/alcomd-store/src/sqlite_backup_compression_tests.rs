use super::*;
use alcomd_application::{BackupCompression, BackupId, M5BackupStore};
use rusqlite::types::Value;
use std::fs;
use std::path::PathBuf;

const BACKUP_ID: &str = "00000000-0000-4000-8000-000000000801";
const PLAN_ID: &str = "00000000-0000-4000-8000-000000000802";

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("alcomd-state18-{}", Uuid::new_v4()));
        fs::create_dir(&path).expect("isolated migration fixture");
        Self(path)
    }

    fn database(&self) -> PathBuf {
        self.0.join("state.db")
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        for _ in 0..20 {
            if fs::remove_dir_all(&self.0).is_ok() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("migration fixture could not be removed");
    }
}

fn fixture(path: &Path) -> Connection {
    let connection = Connection::open(path).expect("fixture connection");
    connection
        .execute_batch("PRAGMA foreign_keys=ON;")
        .expect("foreign keys");
    for migration in [
        MIGRATION_V1,
        MIGRATION_V2,
        MIGRATION_V3,
        MIGRATION_V4,
        MIGRATION_V5,
        MIGRATION_V6,
        MIGRATION_V7,
        MIGRATION_V8,
        MIGRATION_V9,
        MIGRATION_V10,
        MIGRATION_V11,
        MIGRATION_V12,
        MIGRATION_V13,
        MIGRATION_V14,
        MIGRATION_V15,
        MIGRATION_V16,
        MIGRATION_V17,
    ] {
        connection
            .execute_batch(migration)
            .expect("historical migration");
    }
    for (index, mode) in [(1, "store"), (2, "fast"), (3, "maximum")] {
        insert_backup(&connection, index, mode);
    }
    connection.execute_batch(r#"
        INSERT INTO backup_restore_plans (
            plan_id, owner_principal_id, state, backup_id, preallocated_project_id,
            backup_archive_sha256, backup_file_identity, backup_byte_size,
            backup_format_version, backup_manifest_fingerprint, exclude_vpm_packages,
            excluded_packages_json, target_parent_path, target_parent_identity,
            target_leaf, target_must_be_absent, expected_unity_project_json,
            plan_fingerprint, plan_json, apply_operation_id, created_at_ms
        ) VALUES ('00000000-0000-4000-8000-000000000802', 'builtin:local-owner',
                  'unapplied', '00000000-0000-4000-8000-000000000801',
                  '00000000-0000-4000-8000-000000000803', zeroblob(32), x'01',
                  42, 1, zeroblob(32), 1, '[]', 'X:/Projects', x'02', 'Restored',
                  1, '{}', zeroblob(32),
                  '{"version":1,"backupId":"00000000-0000-4000-8000-000000000801","preallocatedProjectId":"00000000-0000-4000-8000-000000000803","targetMustBeAbsent":true}',
                  NULL, 1);
        INSERT INTO operations (
            operation_id, kind, state, revision, owner_principal_id, request_json,
            cancel_requested, created_at_ms, updated_at_ms
        ) VALUES ('00000000-0000-4000-8000-000000000804', 'backups.restore',
                  'queued', 1, 'builtin:local-owner', '{}', 0, 1, 1);
        UPDATE backup_restore_plans
        SET state='applied', apply_operation_id='00000000-0000-4000-8000-000000000804'
        WHERE plan_id='00000000-0000-4000-8000-000000000802';
        INSERT INTO backup_restore_filesystem_journal (
            operation_id, step, plan_id, preallocated_project_id, phase, state,
            target_parent_identity, target_identity, project_fingerprint,
            evidence_json, updated_at_ms
        ) VALUES ('00000000-0000-4000-8000-000000000804', 1,
                  '00000000-0000-4000-8000-000000000802',
                  '00000000-0000-4000-8000-000000000803', 'accepted', 'completed',
                  x'02', NULL, NULL, '{}', 1);
        INSERT INTO events (
            event_id,kind,aggregate_kind,aggregate_id,aggregate_revision,
            principal_id,occurred_at_ms,payload_json
        ) VALUES ('00000000-0000-4000-8000-000000000805','backups.restore.accepted',
                  'operation','00000000-0000-4000-8000-000000000804',1,
                  'builtin:local-owner',1,'{ "retained": true }');
        INSERT INTO idempotency_records (
            principal_id,method,idempotency_key,request_fingerprint,state,
            operation_id,response_json,created_at_ms
        ) VALUES ('builtin:local-owner','backups.create','preserved-key','{}',
                  'completed',NULL,'{ "retained": true }',1);
        UPDATE sqlite_sequence SET seq=99 WHERE name='events';
        CREATE INDEX fixture_backups_created ON backups(created_at_ms);
        CREATE TRIGGER fixture_backups_insert AFTER INSERT ON backups BEGIN SELECT 1; END;
    "#).expect("restore authority, event, replay, index and trigger fixture");
    assert_eq!(integer(&connection, "PRAGMA user_version"), 17);
    assert_eq!(
        integer(&connection, "SELECT count(*) FROM pragma_foreign_key_check"),
        0
    );
    connection
}

fn insert_backup(connection: &Connection, index: u32, mode: &str) {
    connection
        .execute(
            "INSERT INTO backups (
            rowid, backup_id, owner_principal_id, source_project_id, archive_locator,
            file_identity_key, archive_sha256, byte_size, format_version,
            created_at_ms, compression_mode, exclude_vpm_packages
        ) VALUES (?1, ?2, 'builtin:local-owner', '00000000-0000-4000-8000-000000000901', ?3, x'0102', zeroblob(32),
                  42, 1, ?4, ?5, 1)",
            params![
                index * 10,
                format!("00000000-0000-4000-8000-{:012}", 800 + index),
                format!("backup:state18:{index}"),
                index,
                mode
            ],
        )
        .expect("insert backup with original rowid gaps");
}

fn integer(connection: &Connection, sql: &str) -> i64 {
    connection
        .query_row(sql, [], |row| row.get(0))
        .expect("integer query")
}

fn schema(connection: &Connection) -> Vec<(String, String, String)> {
    connection
        .prepare("SELECT type,name,sql FROM sqlite_schema WHERE sql IS NOT NULL ORDER BY type,name")
        .expect("schema query")
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .expect("schema rows")
        .map(|row| row.expect("schema row"))
        .collect()
}

fn rows(connection: &Connection) -> Vec<(String, Vec<Vec<Value>>)> {
    let names: Vec<String> = connection
        .prepare("SELECT name FROM sqlite_schema WHERE type='table' ORDER BY name")
        .expect("tables")
        .query_map([], |row| row.get(0))
        .expect("table names")
        .map(|row| row.expect("table name"))
        .collect();
    names
        .into_iter()
        .map(|name| {
            let mut statement = connection
                .prepare(&format!(
                    "SELECT rowid,* FROM \"{}\" ORDER BY rowid",
                    name.replace('"', "\"\"")
                ))
                .expect("table values");
            let count = statement.column_count();
            let values = statement
                .query_map([], |row| {
                    (0..count)
                        .map(|index| row.get(index))
                        .collect::<rusqlite::Result<Vec<Value>>>()
                })
                .expect("row query")
                .map(|row| row.expect("row values"))
                .collect();
            (name, values)
        })
        .collect()
}

#[tokio::test]
async fn state17_to18_preserves_every_row_index_trigger_and_restore_foreign_key_then_reads_six_levels()
 {
    let directory = TestDirectory::new();
    let path = directory.database();
    let connection = fixture(&path);
    let before_rows = rows(&connection);
    let mut expected_schema = schema(&connection);
    for (_, name, sql) in &mut expected_schema {
        if name == "backups" {
            *sql = sql.replace(
                "'store', 'fast', 'maximum'",
                "'store', 'fast', 'level3', 'level5', 'level7', 'maximum'",
            );
        }
    }
    drop(connection);
    let mut connection = Connection::open(&path).expect("reopen State17 fixture");
    connection
        .execute_batch("PRAGMA foreign_keys=ON")
        .expect("foreign key enforcement");
    migrate_backup_compression(&mut connection, MIGRATION_V18)
        .expect("production State18 migrator");
    assert_eq!(integer(&connection, "PRAGMA user_version"), 18);
    assert_eq!(integer(&connection, "PRAGMA foreign_keys"), 1);
    assert_eq!(
        integer(&connection, "SELECT count(*) FROM pragma_foreign_key_check"),
        0
    );
    assert_eq!(rows(&connection), before_rows);
    let normalized_schema = schema(&connection)
        .into_iter()
        .map(|(kind, name, sql)| {
            (
                kind,
                name,
                sql.replace("CREATE TABLE \"backups\"", "CREATE TABLE backups"),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(normalized_schema, expected_schema);
    assert_eq!(
        integer(
            &connection,
            "SELECT strict FROM pragma_table_list WHERE name='backups'"
        ),
        1
    );
    assert!(
        connection
            .execute("DELETE FROM backups WHERE backup_id=?1", [BACKUP_ID])
            .is_err()
    );
    assert!(
        connection
            .execute(
                "DELETE FROM backup_restore_plans WHERE plan_id=?1",
                [PLAN_ID]
            )
            .is_err()
    );
    for invalid in [
        "level0", "level1", "level2", "level4", "level6", "level8", "level9", "normal", "",
    ] {
        assert!(
            connection
                .execute(
                    "UPDATE backups SET compression_mode=?1 WHERE backup_id=?2",
                    params![invalid, BACKUP_ID]
                )
                .is_err(),
            "invalid {invalid}"
        );
    }
    for (index, mode) in [(4, "level3"), (5, "level5"), (6, "level7")] {
        insert_backup(&connection, index, mode);
    }
    drop(connection);
    let connection =
        initialize_connection(&path).expect("latest initialization before read-only reopen");
    let accepted = rows(&connection);
    drop(connection);
    let store = crate::StateStoreHandle::open(path.clone()).expect("read retained backup metadata");
    let modes = [
        BackupCompression::Store,
        BackupCompression::Fast,
        BackupCompression::Maximum,
        BackupCompression::Level3,
        BackupCompression::Level5,
        BackupCompression::Level7,
    ];
    for (index, expected) in modes.into_iter().enumerate() {
        let id = BackupId::parse(&format!("00000000-0000-4000-8000-{:012}", 801 + index))
            .expect("backup ID");
        let record = store
            .get_backup(PrincipalId::local_owner(), id)
            .await
            .expect("strongly typed stored backup");
        assert_eq!(record.record.compression_mode, expected);
    }
    assert_eq!(
        store
            .list_backups(PrincipalId::local_owner(), None, None, 100)
            .await
            .expect("six levels list")
            .len(),
        6
    );
    drop(store);
    let connection = initialize_connection(&path).expect("stable reopen");
    assert_eq!(rows(&connection), accepted);
    assert_eq!(integer(&connection, "PRAGMA foreign_keys"), 1);
}

#[test]
fn state18_failure_after_table_replacement_rolls_back_schema_rows_version_and_restores_foreign_keys()
 {
    let mut connection = fixture(Path::new(":memory:"));
    let before_rows = rows(&connection);
    let before_schema = schema(&connection);
    let interrupted = format!("{MIGRATION_V18}\nSELECT * FROM unavailable_migration_probe;\n");
    assert!(migrate_backup_compression(&mut connection, &interrupted).is_err());
    assert!(connection.is_autocommit());
    assert_eq!(integer(&connection, "PRAGMA user_version"), 17);
    assert_eq!(integer(&connection, "PRAGMA foreign_keys"), 1);
    assert_eq!(rows(&connection), before_rows);
    assert_eq!(schema(&connection), before_schema);
    assert_eq!(
        integer(&connection, "SELECT count(*) FROM pragma_foreign_key_check"),
        0
    );
}

#[test]
fn state18_foreign_key_check_rejects_orphan_and_retains_original_state17_authority() {
    let mut connection = fixture(Path::new(":memory:"));
    connection
        .execute_batch("PRAGMA foreign_keys=OFF;")
        .expect("corrupt isolated fixture");
    connection
        .execute("DELETE FROM backups WHERE backup_id=?1", [BACKUP_ID])
        .expect("orphan fixture");
    connection
        .execute_batch("PRAGMA foreign_keys=ON;")
        .expect("restore enforcement");
    let before_rows = rows(&connection);
    let before_schema = schema(&connection);
    assert_eq!(
        integer(&connection, "SELECT count(*) FROM pragma_foreign_key_check"),
        1
    );
    assert!(migrate_backup_compression(&mut connection, MIGRATION_V18).is_err());
    assert!(connection.is_autocommit());
    assert_eq!(integer(&connection, "PRAGMA user_version"), 17);
    assert_eq!(integer(&connection, "PRAGMA foreign_keys"), 1);
    assert_eq!(rows(&connection), before_rows);
    assert_eq!(schema(&connection), before_schema);
    assert_eq!(
        integer(&connection, "SELECT count(*) FROM pragma_foreign_key_check"),
        1
    );
}
