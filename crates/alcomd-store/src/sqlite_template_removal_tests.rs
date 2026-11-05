use super::*;
use alcomd_application::StateStore;

const LEGACY_OP: &str = "10000000-0000-4000-8000-000000000001";
const RETAINED_OP: &str = "10000000-0000-4000-8000-000000000002";
const PROJECT: &str = "10000000-0000-4000-8000-000000000003";
const TEMPLATE: &str = "10000000-0000-4000-8000-000000000004";
const PLAN: &str = "10000000-0000-4000-8000-000000000005";

fn event(connection: &Connection, kind: &str, aggregate: &str, id: &str) {
    connection
        .execute(
            "INSERT INTO events (event_id,kind,aggregate_kind,aggregate_id,aggregate_revision,
                                 principal_id,occurred_at_ms,payload_json)
             VALUES (?1,?2,?3,?4,1,'local-owner',1,'{}')",
            params![Uuid::new_v4().to_string(), kind, aggregate, id],
        )
        .expect("fixture event");
}

fn log(connection: &Connection, action: &str, operation: Option<&str>) {
    connection
        .execute(
            "INSERT INTO operation_log_entries (
                entry_id,owner,execution_id,actor,operation_id,occurred_at_ms,action,step,outcome,
                target_json
             ) VALUES (?1,'local-owner',?2,'GUI',?3,1,?4,'completed','succeeded','null')",
            params![
                Uuid::new_v4().to_string(),
                Uuid::new_v4().to_string(),
                operation,
                action
            ],
        )
        .expect("fixture log");
}

fn count(connection: &Connection, sql: &str) -> i64 {
    connection
        .query_row(sql, [], |row| row.get(0))
        .expect("fixture count")
}

#[test]
fn state22_removes_only_legacy_authority_and_preserves_foreign_keys_and_high_water() {
    let mut connection = state21_fixture(Path::new(":memory:"));
    connection
        .execute_batch(&format!(
            r#"INSERT INTO operations (rowid,operation_id,kind,state,revision,owner_principal_id,
                                     request_json,created_at_ms,updated_at_ms) VALUES
                 (10,'{LEGACY_OP}','templates.create-project','succeeded',2,'local-owner','{{}}',1,1),
                 (20,'{RETAINED_OP}','packages.apply','succeeded',2,'local-owner','{{}}',1,1);
             INSERT INTO operation_journal (operation_id,step,kind,state,payload_json,updated_at_ms)
             VALUES ('{LEGACY_OP}',1,'templates.create-project','applied','{{}}',1),
                    ('{RETAINED_OP}',1,'packages.apply','applied','{{}}',1);
             INSERT INTO projects (
                 project_id,owner_principal_id,root_path,path_identity_key,project_type,unity_version,
                 snapshot_json,revision,registered_at_ms,updated_at_ms
             ) VALUES ('{PROJECT}','local-owner','/fixture/project',x'01','unknown','2022.3.22f1','{{}}',1,1,1);
             INSERT INTO templates (
                 template_id,owner_principal_id,source_kind,template_version,manifest_json,
                 payload_locator,payload_sha256,revision,created_at_ms,updated_at_ms
             ) VALUES ('{TEMPLATE}','local-owner','user','1','{{}}','fixture',zeroblob(32),1,1,1);
             INSERT INTO template_plans (
                 plan_id,owner_principal_id,kind,state,plan_fingerprint,plan_json,
                 apply_operation_id,created_at_ms
             ) VALUES ('{PLAN}','local-owner','create-project','applied',zeroblob(32),
                       '{{"version":1,"kind":"create-project"}}','{LEGACY_OP}',1);
             INSERT INTO idempotency_records (principal_id,method,idempotency_key,request_fingerprint,
                                              state,operation_id,response_json,created_at_ms) VALUES
                ('local-owner','templates.applyPlan','create','{{}}','completed','{LEGACY_OP}','{{}}',1),
                ('local-owner','operations.cancel','legacy','{{}}','completed','{LEGACY_OP}','{{}}',1),
                ('local-owner','templates.setFavorite','favorite','{{}}','completed',NULL,'{{}}',1),
                ('local-owner','packages.apply','retain','{{}}','completed','{RETAINED_OP}','{{}}',1);"#
        ))
        .expect("isolated legacy authority");
    let retained_plan = Uuid::new_v4().to_string();
    connection
        .execute(
            r#"INSERT INTO package_plans (plan_id,owner_principal_id,project_id,action,state,
                project_revision,project_snapshot_fingerprint,change_set_fingerprint,change_set_json,
                source_set_json,apply_operation_id,created_at_ms)
             VALUES (?1,'local-owner',?2,'install','applied',1,zeroblob(32),zeroblob(32),
                '{"formatVersion":1,"mutations":[],"dependencyEdges":[]}', '[]',?3,1)"#,
            params![retained_plan, PROJECT, RETAINED_OP],
        )
        .expect("unrelated referencing plan");
    event(&connection, "project.registered", "project", PROJECT);
    event(&connection, "operation.completed", "operation", RETAINED_OP);
    event(&connection, "operation.completed", "operation", LEGACY_OP);
    event(
        &connection,
        "template.favorite_changed",
        "template",
        TEMPLATE,
    );
    log(&connection, "projects.register", None);
    log(&connection, "operations.cancel", Some(LEGACY_OP));
    log(&connection, "templates.favorite", None);
    let event_high = count(
        &connection,
        "SELECT seq FROM sqlite_sequence WHERE name='events'",
    );
    let log_high = count(
        &connection,
        "SELECT seq FROM sqlite_sequence WHERE name='operation_log_entries'",
    );

    migrate_remove_templates(&mut connection).expect("approved cleanup");
    assert_eq!(count(&connection, "PRAGMA user_version"), 22);
    assert_eq!(count(&connection, "PRAGMA foreign_keys"), 1);
    assert_eq!(
        count(&connection, "SELECT count(*) FROM pragma_foreign_key_check"),
        0
    );
    assert_eq!(
        count(
            &connection,
            "SELECT count(*) FROM sqlite_schema WHERE tbl_name IN ('templates','template_plans')"
        ),
        0
    );
    assert_eq!(count(&connection, "SELECT rowid FROM operations"), 20);
    for table in [
        "operation_journal",
        "idempotency_records",
        "package_plans",
        "projects",
        "operation_log_entries",
    ] {
        assert_eq!(
            count(&connection, &format!("SELECT count(*) FROM {table}")),
            1,
            "{table}"
        );
    }
    assert_eq!(count(&connection, "SELECT count(*) FROM events"), 2);
    assert_eq!(
        count(
            &connection,
            "SELECT seq FROM sqlite_sequence WHERE name='events'"
        ),
        event_high
    );
    assert_eq!(
        count(
            &connection,
            "SELECT seq FROM sqlite_sequence WHERE name='operation_log_entries'"
        ),
        log_high
    );

    connection.execute_batch(MIGRATION_V23).expect("State23");
    assert_eq!(count(&connection, "PRAGMA user_version"), 23);
    event(
        &connection,
        "project-template.changed",
        "project-template",
        TEMPLATE,
    );
    assert_eq!(connection.last_insert_rowid(), event_high + 1);
    log(&connection, "projects.create", None);
    assert_eq!(connection.last_insert_rowid(), log_high + 1);
}

#[test]
fn active_old_task_blocks_cleanup_and_rollback_restores_foreign_key_enforcement() {
    let mut connection = state21_fixture(Path::new(":memory:"));
    connection
        .execute(
            "INSERT INTO operations (operation_id,kind,state,revision,owner_principal_id,
                                 request_json,created_at_ms,updated_at_ms)
         VALUES (?1,'templates.import','interrupted',1,'local-owner','{}',1,1)",
            [LEGACY_OP],
        )
        .expect("unfinished operation");
    assert!(matches!(
        migrate_remove_templates(&mut connection),
        Err(StoreOpenError::LegacyTemplateOperationsActive)
    ));
    assert_eq!(count(&connection, "PRAGMA foreign_keys"), 1);
    assert_eq!(count(&connection, "PRAGMA user_version"), 21);
    assert_eq!(count(&connection, "SELECT count(*) FROM operations"), 1);
    assert_eq!(
        count(
            &connection,
            "SELECT count(*) FROM sqlite_schema WHERE name='templates'"
        ),
        1
    );
}

#[test]
fn preflight_keeps_an_older_database_reopenable_by_the_previous_core() {
    let directory = crate::events_tests::Directory::new();
    let path = directory.database();
    let connection = state21_fixture(&path);
    connection
        .execute_batch("DROP TABLE operation_log_entries; PRAGMA user_version=20;")
        .expect("State20 fixture");
    connection
        .execute(
            "INSERT INTO operations (operation_id,kind,state,revision,owner_principal_id,
                                     request_json,created_at_ms,updated_at_ms)
             VALUES (?1,'templates.derive','running',1,'local-owner','{}',1,1)",
            [LEGACY_OP],
        )
        .expect("running template task");
    drop(connection);

    assert!(matches!(
        initialize_connection(&path),
        Err(StoreOpenError::LegacyTemplateOperationsActive)
    ));
    let connection = Connection::open(&path).expect("previous database remains available");
    assert_eq!(count(&connection, "PRAGMA user_version"), 20);
    assert_eq!(
        count(
            &connection,
            "SELECT count(*) FROM sqlite_schema WHERE name='operation_log_entries'"
        ),
        0
    );
    assert_eq!(count(&connection, "SELECT count(*) FROM operations"), 1);
}

#[test]
fn legacy_retirement_report_is_true_only_for_the_open_that_removed_existing_state() {
    let directory = crate::events_tests::Directory::new();
    let path = directory.database();
    let legacy = state21_fixture(&path);
    event(&legacy, "template.favorite_changed", "template", TEMPLATE);
    log(&legacy, "templates.favorite", None);
    drop(legacy);

    let migrated = crate::StateStoreHandle::open(path.clone()).expect("retire old schema");
    assert!(migrated.retired_legacy_template_state());
    let commits = migrated.event_commits();
    assert_eq!(commits.borrow().sequence, 1);
    assert_eq!(commits.borrow().log_sequence, 1);
    drop(migrated);
    let reopened = crate::StateStoreHandle::open(path).expect("already migrated");
    assert!(!reopened.retired_legacy_template_state());
    drop(reopened);

    let fresh = crate::StateStoreHandle::open(directory.database().with_file_name("fresh.db"))
        .expect("fresh profile");
    assert!(!fresh.retired_legacy_template_state());
}
