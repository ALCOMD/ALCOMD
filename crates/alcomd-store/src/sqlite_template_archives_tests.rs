use super::*;

const OLD_CREATE: &str = "28000000-0000-4000-8000-000000000001";
const PLAIN_CREATE: &str = "28000000-0000-4000-8000-000000000002";
const TEMPLATE: &str = "28000000-0000-4000-8000-000000000003";
const PROJECT: &str = "28000000-0000-4000-8000-000000000004";

fn state27_fixture(path: &Path) -> Connection {
    let mut connection = state21_fixture(path);
    migrate_remove_templates(&mut connection).expect("State22");
    for migration in [
        MIGRATION_V23,
        MIGRATION_V24,
        MIGRATION_V25,
        MIGRATION_V26,
        MIGRATION_V27,
    ] {
        connection
            .execute_batch(migration)
            .expect("historical schema");
    }
    connection
}

fn count(connection: &Connection, sql: &str) -> i64 {
    connection
        .query_row(sql, [], |row| row.get(0))
        .expect("fixture count")
}

fn seed(connection: &Connection) {
    connection.execute_batch(&format!(
        r#"INSERT INTO operations (
            rowid,operation_id,kind,state,revision,owner_principal_id,request_json,created_at_ms,updated_at_ms
        ) VALUES
            (11,'{OLD_CREATE}','projects.create','succeeded',2,'local-owner',
             '{{"template":{{"id":"com.example.old","revision":7}}}}',1,2),
            (12,'{PLAIN_CREATE}','projects.create','queued',1,'local-owner','{{}}',1,1);
        INSERT INTO operation_journal VALUES
            ('{OLD_CREATE}',1,'projects.create','applied','{{}}',2),
            ('{OLD_CREATE}',2,'projects.create.declaration','applied','"old JSON declaration"',2),
            ('{OLD_CREATE}',3,'projects.create.packages','applied','"package evidence"',2),
            ('{PLAIN_CREATE}',1,'projects.create','prepared','{{}}',1);
        INSERT INTO idempotency_records (
            principal_id,method,idempotency_key,request_fingerprint,state,
            operation_id,response_json,created_at_ms
        ) VALUES ('local-owner','projects.create','old-create','{{}}','completed','{OLD_CREATE}','{{}}',1);
        INSERT INTO project_templates VALUES
            ('local-owner','com.example.old','{TEMPLATE}','{{"id":"com.example.old"}}',7,0);
        UPDATE project_template_revision SET last_revision=7;
        INSERT INTO projects (
            project_id,owner_principal_id,root_path,path_identity_key,project_type,
            unity_version,snapshot_json,revision,registered_at_ms,updated_at_ms
        ) VALUES ('{PROJECT}','local-owner','/fixture/project',x'01','unknown','2022.3.22f1','{{}}',1,1,1);
        INSERT INTO events (
            event_id,kind,aggregate_kind,aggregate_id,aggregate_revision,principal_id,occurred_at_ms,payload_json
        ) VALUES
            ('28000000-0000-4000-8000-000000000005','project.registered','project','{PROJECT}',1,'local-owner',1,'{{}}'),
            ('28000000-0000-4000-8000-000000000006','project-template.changed','project-template','{TEMPLATE}',7,'local-owner',2,'{{}}');
        UPDATE sqlite_sequence SET seq=42 WHERE name='events';"#
    )).expect("State27 template and unrelated records");
}

#[test]
fn state28_removes_old_declarations_and_template_events_only() {
    let mut connection = state27_fixture(Path::new(":memory:"));
    seed(&connection);
    migrate_template_archives(&mut connection).expect("State28");
    assert_eq!(count(&connection, "PRAGMA user_version"), 28);
    assert_eq!(count(&connection, "PRAGMA foreign_keys"), 1);
    assert_eq!(
        count(&connection, "SELECT count(*) FROM pragma_foreign_key_check"),
        0
    );
    assert_eq!(
        count(&connection, "SELECT count(*) FROM project_templates"),
        0
    );
    assert_eq!(
        count(
            &connection,
            "SELECT last_revision FROM project_template_revision"
        ),
        7
    );
    assert_eq!(count(&connection, "SELECT count(*) FROM events"), 1);
    assert_eq!(
        count(
            &connection,
            "SELECT seq FROM sqlite_sequence WHERE name='events'"
        ),
        42
    );
    assert_eq!(count(&connection, "SELECT sum(rowid) FROM operations"), 23);
    assert_eq!(
        count(&connection, "SELECT count(*) FROM operation_journal"),
        3
    );
    assert_eq!(
        count(&connection, "SELECT count(*) FROM idempotency_records"),
        1
    );
    assert_eq!(count(&connection, "SELECT count(*) FROM projects"), 1);
    assert!(
        connection
            .execute(
                "INSERT INTO project_templates VALUES
         ('local-owner','com.example.old',?1,'{\"id\":\"com.example.old\"}',8,0)",
                [TEMPLATE],
            )
            .is_err(),
        "old declarations must no longer satisfy the template table"
    );
}

#[test]
fn state28_refuses_to_open_while_old_declaration_is_in_use() {
    let dir = crate::events_tests::Directory::new();
    let path = dir.database();
    let connection = state27_fixture(&path);
    seed(&connection);
    connection
        .execute(
            "UPDATE operations SET state='running' WHERE operation_id=?1",
            [OLD_CREATE],
        )
        .expect("active template job");
    drop(connection);
    assert!(matches!(
        initialize_connection(&path),
        Err(StoreOpenError::LegacyTemplateOperationsActive)
    ));
    let connection = Connection::open(path).expect("State27 still readable");
    assert_eq!(count(&connection, "PRAGMA user_version"), 27);
    assert_eq!(
        count(&connection, "SELECT count(*) FROM project_templates"),
        1
    );
    assert_eq!(
        count(&connection, "SELECT count(*) FROM operation_journal"),
        4
    );
}
