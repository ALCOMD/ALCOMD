use rusqlite::{Connection, Error, params};

const MIGRATION_V1: &str = include_str!("../migrations/0001_state.sql");
const PRINCIPAL: &str = "builtin:local-owner";
const OPERATION_A: &str = "00000000-0000-4000-8000-000000000001";
const OPERATION_B: &str = "00000000-0000-4000-8000-000000000002";

fn migrated_connection() -> Connection {
    let connection = Connection::open_in_memory().expect("open database");
    connection
        .execute_batch("PRAGMA foreign_keys=ON;")
        .expect("enable foreign keys");
    connection
        .execute_batch(MIGRATION_V1)
        .expect("apply migration");
    connection
}

fn insert_operation(connection: &Connection, operation_id: &str, created_at_ms: i64) {
    connection
        .execute(
            "INSERT INTO operations (
                operation_id, kind, state, revision, owner_principal_id, request_json,
                created_at_ms, updated_at_ms
            ) VALUES (?1, 'state.check', 'queued', 1, ?2, '{\"version\":1}', ?3, ?3)",
            params![operation_id, PRINCIPAL, created_at_ms],
        )
        .expect("insert operation");
}

#[test]
fn operation_acceptance_is_atomic_and_rollback_leaves_no_partial_rows() {
    let connection = migrated_connection();
    connection
        .execute_batch("BEGIN IMMEDIATE;")
        .expect("begin transaction");
    insert_operation(&connection, OPERATION_A, 10);
    connection
        .execute(
            "INSERT INTO operation_journal (
                operation_id, step, kind, state, payload_json, updated_at_ms
            ) VALUES (?1, 1, 'integrity_check', 'prepared', '{}', 10)",
            [OPERATION_A],
        )
        .expect("insert journal");
    let error = connection
        .execute(
            "INSERT INTO events (
                event_id, kind, aggregate_kind, aggregate_id, aggregate_revision,
                principal_id, occurred_at_ms, payload_json
            ) VALUES (?1, 'operation.created', 'operation', ?2, 0, ?3, 10, '{}')",
            params![
                "00000000-0000-4000-8000-000000000101",
                OPERATION_A,
                PRINCIPAL
            ],
        )
        .expect_err("invalid revision must abort unit of work");
    assert!(matches!(error, Error::SqliteFailure(_, _)));
    connection.execute_batch("ROLLBACK;").expect("rollback");

    for table in [
        "operations",
        "operation_journal",
        "events",
        "idempotency_records",
    ] {
        let count: i64 = connection
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("count rows");
        assert_eq!(count, 0, "{table} must not contain a partial commit");
    }
}

#[test]
fn committed_acceptance_has_one_operation_journal_event_and_saved_response() {
    let connection = migrated_connection();
    connection
        .execute_batch("BEGIN IMMEDIATE;")
        .expect("begin transaction");
    insert_operation(&connection, OPERATION_A, 10);
    connection
        .execute(
            "INSERT INTO operation_journal (
                operation_id, step, kind, state, payload_json, updated_at_ms
            ) VALUES (?1, 1, 'integrity_check', 'prepared', '{}', 10)",
            [OPERATION_A],
        )
        .expect("insert journal");
    connection
        .execute(
            "INSERT INTO events (
                event_id, kind, aggregate_kind, aggregate_id, aggregate_revision,
                principal_id, occurred_at_ms, payload_json
            ) VALUES (?1, 'operation.created', 'operation', ?2, 1, ?3, 10, '{}')",
            params![
                "00000000-0000-4000-8000-000000000101",
                OPERATION_A,
                PRINCIPAL
            ],
        )
        .expect("insert event");
    connection
        .execute(
            "INSERT INTO idempotency_records (
                principal_id, method, idempotency_key, request_fingerprint, state,
                operation_id, response_json, created_at_ms
            ) VALUES (?1, 'state.check', 'check-once', '{\"version\":1}', 'completed',
                ?2, '{\"operationId\":\"00000000-0000-4000-8000-000000000001\",\"replayed\":false}', 10)",
            params![PRINCIPAL, OPERATION_A],
        )
        .expect("save idempotent response");
    connection.execute_batch("COMMIT;").expect("commit");

    let aggregate_revision: i64 = connection
        .query_row("SELECT aggregate_revision FROM events", [], |row| {
            row.get(0)
        })
        .expect("event revision");
    let operation_revision: i64 = connection
        .query_row("SELECT revision FROM operations", [], |row| row.get(0))
        .expect("operation revision");
    assert_eq!(aggregate_revision, operation_revision);
    assert_eq!(operation_revision, 1);
}

#[test]
fn idempotency_scope_is_principal_method_and_permanent_key() {
    let connection = migrated_connection();
    insert_operation(&connection, OPERATION_A, 10);
    connection
        .execute(
            "INSERT INTO idempotency_records (
                principal_id, method, idempotency_key, request_fingerprint, state,
                operation_id, response_json, created_at_ms
            ) VALUES (?1, 'state.check', 'same-key', '{\"version\":1}', 'completed',
                ?2, '{}', 10)",
            params![PRINCIPAL, OPERATION_A],
        )
        .expect("insert idempotency record");

    let duplicate = connection.execute(
        "INSERT INTO idempotency_records (
            principal_id, method, idempotency_key, request_fingerprint, state,
            operation_id, response_json, created_at_ms
        ) VALUES (?1, 'state.check', 'same-key', '{\"version\":2}', 'completed',
            ?2, '{}', 20)",
        params![PRINCIPAL, OPERATION_A],
    );
    assert!(duplicate.is_err());

    connection
        .execute(
            "INSERT INTO idempotency_records (
                principal_id, method, idempotency_key, request_fingerprint, state,
                operation_id, response_json, created_at_ms
            ) VALUES ('synthetic:other', 'state.check', 'same-key', '{\"version\":1}',
                'completed', ?1, '{}', 20)",
            [OPERATION_A],
        )
        .expect("same key under another Principal");
}

#[test]
fn operation_pagination_uses_strict_descending_tuple_cursor() {
    let connection = migrated_connection();
    insert_operation(&connection, OPERATION_A, 20);
    insert_operation(&connection, OPERATION_B, 20);

    let mut statement = connection
        .prepare(
            "SELECT operation_id FROM operations
             WHERE owner_principal_id = ?1
               AND (created_at_ms < ?2 OR (created_at_ms = ?2 AND operation_id < ?3))
             ORDER BY created_at_ms DESC, operation_id DESC
             LIMIT ?4",
        )
        .expect("prepare operation page");
    let page = statement
        .query_map(params![PRINCIPAL, 20, OPERATION_B, 100], |row| {
            row.get::<_, String>(0)
        })
        .expect("query page")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect page");
    assert_eq!(page, [OPERATION_A]);
}

#[test]
fn event_pagination_uses_exclusive_sequence_and_preserves_empty_cursor() {
    let connection = migrated_connection();
    for (sequence, suffix) in [(2, "101"), (5, "102")] {
        connection
            .execute(
                "INSERT INTO events (
                    sequence, event_id, kind, aggregate_kind, aggregate_id,
                    aggregate_revision, principal_id, occurred_at_ms, payload_json
                ) VALUES (?1, ?2, 'operation.changed', 'operation', ?3, 1, ?4, 10, '{}')",
                params![
                    sequence,
                    format!("00000000-0000-4000-8000-000000000{suffix}"),
                    OPERATION_A,
                    PRINCIPAL
                ],
            )
            .expect("insert event");
    }
    let after_sequence = 2_i64;
    let page = connection
        .prepare(
            "SELECT sequence FROM events
             WHERE principal_id = ?1 AND sequence > ?2
             ORDER BY sequence ASC LIMIT ?3",
        )
        .expect("prepare event page")
        .query_map(params![PRINCIPAL, after_sequence, 100], |row| {
            row.get::<_, i64>(0)
        })
        .expect("query page")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect page");
    assert_eq!(page, [5]);
    assert_eq!(page.last().copied().unwrap_or(after_sequence), 5);

    let empty_after = 5_i64;
    let empty = connection
        .query_row(
            "SELECT max(sequence) FROM events WHERE principal_id = ?1 AND sequence > ?2",
            params![PRINCIPAL, empty_after],
            |row| row.get::<_, Option<i64>>(0),
        )
        .expect("empty page");
    assert_eq!(empty.unwrap_or(empty_after), empty_after);
}
