use rusqlite::{Connection, params};

const MIGRATION_V1: &str = include_str!("../migrations/0001_state.sql");
const MIGRATION_V2: &str = include_str!("../migrations/0002_projects_repositories.sql");

#[test]
fn bundled_sqlite_version_is_frozen() {
    assert_eq!(rusqlite::version(), "3.53.2");
}

#[test]
fn migration_v1_is_single_transaction_and_sets_version_last() {
    let begin = MIGRATION_V1.find("BEGIN IMMEDIATE;").expect("begin");
    let version = MIGRATION_V1
        .find("PRAGMA user_version = 1;")
        .expect("user_version");
    let commit = MIGRATION_V1.rfind("COMMIT;").expect("commit");
    assert_eq!(begin, 0);
    assert!(version < commit);
    assert!(!MIGRATION_V1[version + 1..commit].contains("CREATE "));

    let connection = Connection::open_in_memory().expect("open SQLite");
    connection
        .execute_batch("PRAGMA foreign_keys=ON;")
        .expect("enable foreign keys");
    connection
        .execute_batch(MIGRATION_V1)
        .expect("apply migration");
    assert_eq!(user_version(&connection), 1);
    assert_eq!(pragma_i64(&connection, "PRAGMA foreign_keys"), 1);
}

#[test]
fn failed_migration_rolls_back_every_table_and_user_version() {
    let connection = Connection::open_in_memory().expect("open database");
    let failing = MIGRATION_V1.replace(
        "PRAGMA user_version = 1;",
        "THIS IS NOT VALID SQL;\nPRAGMA user_version = 1;",
    );
    assert!(connection.execute_batch(&failing).is_err());
    connection
        .execute_batch("ROLLBACK;")
        .expect("rollback failed migration");
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("read user version");
    let tables: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_schema
             WHERE type='table' AND name IN (
                'operations', 'operation_journal', 'events', 'idempotency_records'
             )",
            [],
            |row| row.get(0),
        )
        .expect("count contract tables");
    assert_eq!(version, 0);
    assert_eq!(tables, 0);
}

#[test]
fn schema_v1_has_only_the_approved_strict_tables_and_indexes() {
    let connection = migrated_connection();
    let mut statement = connection
        .prepare(
            "SELECT name, sql FROM sqlite_schema \
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .expect("prepare schema query");
    let tables = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .expect("query tables")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect tables");
    assert_eq!(
        tables
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        [
            "events",
            "idempotency_records",
            "operation_journal",
            "operations"
        ]
    );
    assert!(tables.iter().all(|(_, sql)| sql.ends_with("STRICT")));

    for index in [
        "events_principal_sequence",
        "operations_owner_page",
        "operations_recovery",
    ] {
        let count: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE type='index' AND name=?1",
                [index],
                |row| row.get(0),
            )
            .expect("query index");
        assert_eq!(count, 1, "missing index {index}");
    }
}

#[test]
fn schema_v1_rejects_invalid_operation_and_finite_states() {
    let connection = migrated_connection();
    insert_operation(&connection, "queued", 1).expect("valid operation");

    assert!(insert_operation(&connection, "unknown", 1).is_err());
    assert!(insert_operation(&connection, "running", 0).is_err());

    let operation_id = "00000000-0000-4000-8000-000000000001";
    assert!(
        connection
            .execute(
                "INSERT INTO operation_journal \
                 (operation_id, step, kind, state, payload_json, updated_at_ms) \
                 VALUES (?1, 1, 'integrity-check', 'unknown', '{}', 1)",
                [operation_id],
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "INSERT INTO idempotency_records \
                 (principal_id, method, idempotency_key, request_fingerprint, state, \
                  operation_id, response_json, created_at_ms) \
                 VALUES ('builtin:local-owner', 'state.check', 'key', '{}', 'expired', \
                         ?1, NULL, 1)",
                [operation_id],
            )
            .is_err()
    );
}

#[test]
fn schema_v1_has_no_expiry_or_future_business_tables() {
    assert!(!MIGRATION_V1.contains("expires_at"));
    for forbidden in [
        "projects",
        "packages",
        "repositories",
        "settings",
        "extensions",
    ] {
        assert!(!MIGRATION_V1.contains(&format!("CREATE TABLE {forbidden}")));
    }
}

#[test]
fn migration_v2_is_atomic_and_adds_only_the_three_m3_tables() {
    let connection = migrated_v2_connection();
    assert_eq!(user_version(&connection), 2);
    let tables = connection
        .prepare(
            "SELECT name FROM sqlite_schema
             WHERE type='table' AND name NOT LIKE 'sqlite_%'
             ORDER BY name",
        )
        .expect("prepare tables")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query tables")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect tables");
    assert_eq!(
        tables,
        [
            "events",
            "idempotency_records",
            "operation_journal",
            "operations",
            "projects",
            "repositories",
            "repository_package_versions",
        ]
    );
    assert!(MIGRATION_V2.starts_with("BEGIN IMMEDIATE;"));
    assert!(MIGRATION_V2.ends_with("COMMIT;\n"));
    for forbidden in [
        "CREATE TABLE packages",
        "CREATE TABLE dependencies",
        "CREATE TABLE credentials",
        "CREATE TABLE plans",
    ] {
        assert!(!MIGRATION_V2.contains(forbidden));
    }
}

#[test]
fn migration_v2_preserves_events_indexes_and_autoincrement() {
    let connection = migrated_connection();
    insert_operation(&connection, "queued", 1).expect("valid operation");
    connection
        .execute(
            "INSERT INTO events (
                sequence, event_id, kind, aggregate_kind, aggregate_id, aggregate_revision,
                principal_id, occurred_at_ms, payload_json
             ) VALUES (7, '00000000-0000-4000-8000-000000000107', 'operation.queued',
                       'operation', '00000000-0000-4000-8000-000000000001', 1,
                       'builtin:local-owner', 1, '{}')",
            [],
        )
        .expect("insert event with sequence gap");
    connection
        .execute_batch(MIGRATION_V2)
        .expect("apply migration v2");
    assert_eq!(user_version(&connection), 2);
    assert_eq!(
        connection
            .query_row("SELECT sequence FROM events", [], |row| row
                .get::<_, i64>(0))
            .expect("preserved event"),
        7
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT seq FROM sqlite_sequence WHERE name='events'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("event sequence"),
        7
    );
    for index in ["events_principal_sequence", "events_aggregate_sequence"] {
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM sqlite_schema WHERE type='index' AND name=?1",
                    [index],
                    |row| row.get::<_, i64>(0),
                )
                .expect("event index"),
            1
        );
    }
    connection
        .execute(
            "INSERT INTO events (
                event_id, kind, aggregate_kind, aggregate_id, aggregate_revision,
                principal_id, occurred_at_ms, payload_json
             ) VALUES ('00000000-0000-4000-8000-000000000108', 'project.registered',
                       'project', '00000000-0000-4000-8000-000000000201', 1,
                       'builtin:local-owner', 2, '{}')",
            [],
        )
        .expect("insert M3 event");
    assert_eq!(
        connection
            .query_row("SELECT max(sequence) FROM events", [], |row| row
                .get::<_, i64>(0))
            .expect("next event sequence"),
        8
    );
}

#[test]
fn migration_v2_allows_completed_sync_idempotency_without_operation() {
    let connection = migrated_v2_connection();
    connection
        .execute(
            "INSERT INTO idempotency_records (
                principal_id, method, idempotency_key, request_fingerprint, state,
                operation_id, response_json, created_at_ms
             ) VALUES ('builtin:local-owner', 'projects.register', 'project-once', '{}',
                       'completed', NULL, '{\"projectId\":\"00000000-0000-4000-8000-000000000201\"}', 1)",
            [],
        )
        .expect("completed synchronous idempotency");
    assert!(
        connection
            .execute(
                "INSERT INTO idempotency_records (
                    principal_id, method, idempotency_key, request_fingerprint, state,
                    operation_id, response_json, created_at_ms
                 ) VALUES ('builtin:local-owner', 'projects.refresh', 'pending-without-operation',
                           '{}', 'pending', NULL, NULL, 1)",
                [],
            )
            .is_err()
    );
}

#[test]
fn failed_migration_v2_restores_complete_v1() {
    let connection = migrated_connection();
    let failing = MIGRATION_V2.replace(
        "PRAGMA user_version = 2;",
        "THIS IS NOT VALID SQL;\nPRAGMA user_version = 2;",
    );
    assert!(connection.execute_batch(&failing).is_err());
    connection
        .execute_batch("ROLLBACK;")
        .expect("rollback failed migration");
    assert_eq!(user_version(&connection), 1);
    let business_tables: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_schema
             WHERE type='table' AND name IN ('projects','repositories','repository_package_versions')",
            [],
            |row| row.get(0),
        )
        .expect("count M3 tables");
    assert_eq!(business_tables, 0);
    connection
        .execute(
            "INSERT INTO events (
                event_id, kind, aggregate_kind, aggregate_id, aggregate_revision,
                principal_id, occurred_at_ms, payload_json
             ) VALUES ('00000000-0000-4000-8000-000000000109', 'project.registered',
                       'project', '00000000-0000-4000-8000-000000000201', 1,
                       'builtin:local-owner', 2, '{}')",
            [],
        )
        .expect_err("v1 must still reject project aggregate");
}

fn migrated_connection() -> Connection {
    let connection = Connection::open_in_memory().expect("open SQLite");
    connection
        .execute_batch("PRAGMA foreign_keys=ON;")
        .expect("enable foreign keys");
    connection
        .execute_batch(MIGRATION_V1)
        .expect("apply migration");
    connection
}

fn migrated_v2_connection() -> Connection {
    let connection = migrated_connection();
    connection
        .execute_batch(MIGRATION_V2)
        .expect("apply migration v2");
    connection
}

fn insert_operation(
    connection: &Connection,
    state: &str,
    revision: i64,
) -> rusqlite::Result<usize> {
    connection.execute(
        "INSERT INTO operations \
         (operation_id, kind, state, revision, owner_principal_id, request_json, \
          cancel_requested, created_at_ms, updated_at_ms) \
         VALUES (?1, 'state.check', ?2, ?3, 'builtin:local-owner', '{}', 0, 1, 1)",
        params![
            if state == "queued" {
                "00000000-0000-4000-8000-000000000001"
            } else {
                "00000000-0000-4000-8000-000000000002"
            },
            state,
            revision
        ],
    )
}

fn user_version(connection: &Connection) -> i64 {
    pragma_i64(connection, "PRAGMA user_version")
}

fn pragma_i64(connection: &Connection, pragma: &str) -> i64 {
    connection
        .query_row(pragma, [], |row| row.get(0))
        .expect("read pragma")
}
