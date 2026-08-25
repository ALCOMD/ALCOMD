use std::path::Path;
use std::time::Duration;

use alcomd_application::{
    CancelOperationFingerprintV1, CheckClassification, CreateOperationOutcome, EventPage,
    EventRecord, FilesystemPhase, IdempotencyKey, OperationCursor, OperationId, OperationPage,
    OperationRecord, OperationState, PrincipalId, Revision, StateCheckFingerprintV1,
    StateCheckResult, StoreError, StoreErrorKind,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use uuid::Uuid;

use crate::{CURRENT_DATA_SCHEMA, StoreOpenError};

const DATA_SCHEMA_VERSION: i64 = 9;
const BUSY_TIMEOUT: Duration = Duration::from_millis(5_000);
const CHECK_ROW_LIMIT: usize = 100;
const CHECK_BYTE_LIMIT: usize = 65_536;
const MIGRATION_V1: &str = include_str!("../migrations/0001_state.sql");
const MIGRATION_V2: &str = include_str!("../migrations/0002_projects_repositories.sql");
const MIGRATION_V3: &str = include_str!("../migrations/0003_package_transactions.sql");
const MIGRATION_V4: &str = include_str!("../migrations/0004_local_workflows.sql");
const MIGRATION_V5: &str = include_str!("../migrations/0005_template_plans.sql");
const MIGRATION_V6: &str = include_str!("../migrations/0006_backup_create.sql");
const MIGRATION_V7: &str = include_str!("../migrations/0007_backup_restore.sql");
const MIGRATION_V8: &str = include_str!("../migrations/0008_extension_runtime.sql");
const MIGRATION_V9: &str = include_str!("../migrations/0009_portable_extension_ui.sql");

pub(super) fn initialize_connection(path: &Path) -> Result<Connection, StoreOpenError> {
    let connection = Connection::open(path).map_err(|_| StoreOpenError::Unavailable)?;
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|_| StoreOpenError::Unavailable)?;
    if version > DATA_SCHEMA_VERSION {
        return Err(StoreOpenError::UnsupportedDataSchema {
            found: u32::try_from(version).unwrap_or(u32::MAX),
            supported: CURRENT_DATA_SCHEMA,
        });
    }
    connection
        .execute_batch("PRAGMA foreign_keys=ON;")
        .map_err(|_| StoreOpenError::Unavailable)?;
    let foreign_keys: i64 = connection
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .map_err(|_| StoreOpenError::Unavailable)?;
    if foreign_keys != 1 {
        return Err(StoreOpenError::Unavailable);
    }
    let journal_mode: String = connection
        .query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))
        .map_err(|_| StoreOpenError::Unavailable)?;
    if !journal_mode.eq_ignore_ascii_case("wal") {
        return Err(StoreOpenError::Unavailable);
    }
    connection
        .execute_batch("PRAGMA synchronous=FULL;")
        .map_err(|_| StoreOpenError::Unavailable)?;
    let synchronous: i64 = connection
        .query_row("PRAGMA synchronous", [], |row| row.get(0))
        .map_err(|_| StoreOpenError::Unavailable)?;
    if synchronous != 2 {
        return Err(StoreOpenError::Unavailable);
    }
    connection
        .busy_timeout(BUSY_TIMEOUT)
        .map_err(|_| StoreOpenError::Unavailable)?;
    if version == 0 {
        connection
            .execute_batch(MIGRATION_V1)
            .map_err(|_| StoreOpenError::Unavailable)?;
    }
    if version <= 1 {
        connection
            .execute_batch(MIGRATION_V2)
            .map_err(|_| StoreOpenError::Unavailable)?;
    }
    if version <= 2 {
        connection
            .execute_batch(MIGRATION_V3)
            .map_err(|_| StoreOpenError::Unavailable)?;
    }
    if version <= 3 {
        connection
            .execute_batch(MIGRATION_V4)
            .map_err(|_| StoreOpenError::Unavailable)?;
    }
    if version <= 4 {
        connection
            .execute_batch(MIGRATION_V5)
            .map_err(|_| StoreOpenError::Unavailable)?;
    }
    if version <= 5 {
        connection
            .execute_batch(MIGRATION_V6)
            .map_err(|_| StoreOpenError::Unavailable)?;
    }
    if version <= 6 {
        connection
            .execute_batch(MIGRATION_V7)
            .map_err(|_| StoreOpenError::Unavailable)?;
    }
    if version <= 7 {
        connection
            .execute_batch(MIGRATION_V8)
            .map_err(|_| StoreOpenError::Unavailable)?;
    }
    if version <= 8 {
        connection
            .execute_batch(MIGRATION_V9)
            .map_err(|_| StoreOpenError::Unavailable)?;
    }
    let final_version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|_| StoreOpenError::Unavailable)?;
    let final_foreign_keys: i64 = connection
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .map_err(|_| StoreOpenError::Unavailable)?;
    let final_journal: String = connection
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .map_err(|_| StoreOpenError::Unavailable)?;
    if final_version != DATA_SCHEMA_VERSION
        || final_foreign_keys != 1
        || !final_journal.eq_ignore_ascii_case("wal")
    {
        return Err(StoreOpenError::Unavailable);
    }
    Ok(connection)
}

pub(super) fn create_state_check(
    connection: &mut Connection,
    owner: &PrincipalId,
    idempotency_key: &IdempotencyKey,
    created_at_ms: u64,
) -> Result<CreateOperationOutcome, StoreError> {
    let created_at_ms = sqlite_integer(created_at_ms)?;
    let fingerprint = StateCheckFingerprintV1.canonical_json();
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(store_failure)?;
    let existing = transaction
        .query_row(
            "SELECT request_fingerprint, operation_id
             FROM idempotency_records
             WHERE principal_id=?1 AND method='state.check' AND idempotency_key=?2",
            params![owner.as_str(), idempotency_key.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(store_failure)?;
    if let Some((existing_fingerprint, operation_id)) = existing {
        if existing_fingerprint != fingerprint {
            return Err(StoreError::new(StoreErrorKind::IdempotencyConflict));
        }
        let operation_id = OperationId::parse(&operation_id).map_err(|_| corrupt_state())?;
        transaction.commit().map_err(store_failure)?;
        return Ok(CreateOperationOutcome {
            operation_id,
            replayed: true,
            schedule: false,
        });
    }

    let operation_id = OperationId::new();
    let operation_id_text = operation_id.to_string();
    transaction
        .execute(
            "INSERT INTO operations (
                operation_id, kind, state, revision, owner_principal_id, request_json,
                created_at_ms, updated_at_ms
            ) VALUES (?1, 'state.check', 'queued', 1, ?2, ?3, ?4, ?4)",
            params![
                operation_id_text,
                owner.as_str(),
                fingerprint,
                created_at_ms
            ],
        )
        .map_err(store_failure)?;
    transaction
        .execute(
            "INSERT INTO operation_journal (
                operation_id, step, kind, state, payload_json, updated_at_ms
            ) VALUES (?1, 1, 'state.check', 'prepared', '{}', ?2)",
            params![operation_id_text, created_at_ms],
        )
        .map_err(store_failure)?;
    insert_event(
        &transaction,
        owner,
        operation_id,
        Revision::INITIAL,
        "operation.created",
        "{\"state\":\"queued\"}",
        created_at_ms,
    )?;
    let response = serde_json::to_string(&CreateOperationOutcome {
        operation_id,
        replayed: false,
        schedule: true,
    })
    .map_err(|_| corrupt_state())?;
    transaction
        .execute(
            "INSERT INTO idempotency_records (
                principal_id, method, idempotency_key, request_fingerprint, state,
                operation_id, response_json, created_at_ms
            ) VALUES (?1, 'state.check', ?2, ?3, 'completed', ?4, ?5, ?6)",
            params![
                owner.as_str(),
                idempotency_key.as_str(),
                fingerprint,
                operation_id_text,
                response,
                created_at_ms
            ],
        )
        .map_err(store_failure)?;
    transaction.commit().map_err(store_failure)?;
    Ok(CreateOperationOutcome {
        operation_id,
        replayed: false,
        schedule: true,
    })
}

pub(super) fn cancel_operation(
    connection: &mut Connection,
    owner: &PrincipalId,
    operation_id: OperationId,
    expected_revision: Revision,
    idempotency_key: &IdempotencyKey,
    updated_at_ms: u64,
) -> Result<(OperationRecord, bool), StoreError> {
    let updated_at_ms = sqlite_integer(updated_at_ms)?;
    let fingerprint =
        CancelOperationFingerprintV1::new(operation_id, expected_revision).canonical_json();
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(store_failure)?;
    let existing = transaction
        .query_row(
            "SELECT request_fingerprint, response_json
             FROM idempotency_records
             WHERE principal_id=?1 AND method='operations.cancel' AND idempotency_key=?2",
            params![owner.as_str(), idempotency_key.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(store_failure)?;
    if let Some((existing_fingerprint, response_json)) = existing {
        if existing_fingerprint != fingerprint {
            return Err(StoreError::new(StoreErrorKind::IdempotencyConflict));
        }
        let operation = serde_json::from_str(&response_json).map_err(|_| corrupt_state())?;
        transaction.commit().map_err(store_failure)?;
        return Ok((operation, true));
    }

    let operation = load_owned_operation(&transaction, owner, operation_id)?;
    if operation.revision != expected_revision {
        return Err(StoreError::new(StoreErrorKind::RevisionConflict));
    }
    if operation.state.is_terminal() {
        return Err(StoreError::new(StoreErrorKind::OperationNotCancellable));
    }
    let next_state = match operation.state {
        OperationState::Queued => OperationState::Cancelled,
        OperationState::Running | OperationState::Recovering => OperationState::Cancelling,
        OperationState::Cancelling | OperationState::Interrupted => operation.state,
        OperationState::Planning | OperationState::WaitingForInput => {
            return Err(corrupt_state());
        }
        OperationState::Succeeded | OperationState::Failed | OperationState::Cancelled => {
            return Err(StoreError::new(StoreErrorKind::OperationNotCancellable));
        }
    };
    let changed = !operation.cancel_requested || next_state != operation.state;
    let updated = if changed {
        let next_revision = operation
            .revision
            .checked_next()
            .ok_or_else(corrupt_state)?;
        let completed_at_ms = (next_state == OperationState::Cancelled).then_some(updated_at_ms);
        transaction
            .execute(
                "UPDATE operations SET state=?1, revision=?2, cancel_requested=1,
                    updated_at_ms=?3, completed_at_ms=?4
                 WHERE operation_id=?5",
                params![
                    state_name(next_state),
                    sqlite_revision(next_revision),
                    updated_at_ms,
                    completed_at_ms,
                    operation_id.to_string()
                ],
            )
            .map_err(store_failure)?;
        if next_state == OperationState::Cancelled {
            transaction
                .execute(
                    "UPDATE operation_journal SET state='applied', updated_at_ms=?1
                     WHERE operation_id=?2 AND step=1",
                    params![updated_at_ms, operation_id.to_string()],
                )
                .map_err(store_failure)?;
        }
        insert_event(
            &transaction,
            owner,
            operation_id,
            next_revision,
            "operation.cancel_requested",
            &format!("{{\"state\":\"{}\"}}", state_name(next_state)),
            updated_at_ms,
        )?;
        load_owned_operation(&transaction, owner, operation_id)?
    } else {
        operation
    };
    let response_json = serde_json::to_string(&updated).map_err(|_| corrupt_state())?;
    transaction
        .execute(
            "INSERT INTO idempotency_records (
                principal_id, method, idempotency_key, request_fingerprint, state,
                operation_id, response_json, created_at_ms
            ) VALUES (?1, 'operations.cancel', ?2, ?3, 'completed', ?4, ?5, ?6)",
            params![
                owner.as_str(),
                idempotency_key.as_str(),
                fingerprint,
                operation_id.to_string(),
                response_json,
                updated_at_ms
            ],
        )
        .map_err(store_failure)?;
    transaction.commit().map_err(store_failure)?;
    Ok((updated, false))
}

pub(super) fn begin_state_check(
    connection: &mut Connection,
    operation_id: OperationId,
    updated_at_ms: u64,
) -> Result<OperationRecord, StoreError> {
    let updated_at_ms = sqlite_integer(updated_at_ms)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(store_failure)?;
    let operation = load_operation(&transaction, operation_id)?;
    let next_state = match (operation.state, operation.cancel_requested) {
        (OperationState::Queued | OperationState::Recovering, false) => OperationState::Running,
        (OperationState::Recovering, true) => OperationState::Cancelling,
        _ => return Err(corrupt_state()),
    };
    let next_revision = operation
        .revision
        .checked_next()
        .ok_or_else(corrupt_state)?;
    transaction
        .execute(
            "UPDATE operations SET state=?1, revision=?2, updated_at_ms=?3,
                started_at_ms=coalesce(started_at_ms, ?3) WHERE operation_id=?4",
            params![
                state_name(next_state),
                sqlite_revision(next_revision),
                updated_at_ms,
                operation_id.to_string()
            ],
        )
        .map_err(store_failure)?;
    insert_event(
        &transaction,
        &operation.owner,
        operation_id,
        next_revision,
        "operation.state_changed",
        &format!("{{\"state\":\"{}\"}}", state_name(next_state)),
        updated_at_ms,
    )?;
    let updated = load_operation(&transaction, operation_id)?;
    transaction.commit().map_err(store_failure)?;
    Ok(updated)
}

pub(super) fn finish_state_check(
    connection: &mut Connection,
    operation_id: OperationId,
    result: StateCheckResult,
    completed_at_ms: u64,
) -> Result<OperationRecord, StoreError> {
    let result_json = serde_json::to_string(&result).map_err(|_| corrupt_state())?;
    finish_operation(
        connection,
        operation_id,
        OperationState::Succeeded,
        Some(&result_json),
        None,
        None,
        completed_at_ms,
    )
}

pub(super) fn finish_cancelled(
    connection: &mut Connection,
    operation_id: OperationId,
    completed_at_ms: u64,
) -> Result<OperationRecord, StoreError> {
    finish_operation(
        connection,
        operation_id,
        OperationState::Cancelled,
        None,
        None,
        None,
        completed_at_ms,
    )
}

pub(super) fn finish_failed(
    connection: &mut Connection,
    operation_id: OperationId,
    error_code: &str,
    diagnostic_id: &str,
    completed_at_ms: u64,
) -> Result<OperationRecord, StoreError> {
    finish_operation(
        connection,
        operation_id,
        OperationState::Failed,
        None,
        Some(error_code),
        Some(diagnostic_id),
        completed_at_ms,
    )
}

fn finish_operation(
    connection: &mut Connection,
    operation_id: OperationId,
    terminal: OperationState,
    result_json: Option<&str>,
    error_code: Option<&str>,
    diagnostic_id: Option<&str>,
    completed_at_ms: u64,
) -> Result<OperationRecord, StoreError> {
    let completed_at_ms = sqlite_integer(completed_at_ms)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(store_failure)?;
    let operation = load_operation(&transaction, operation_id)?;
    if operation.state.is_terminal() {
        return Ok(operation);
    }
    if !operation.state.can_transition_to(terminal) {
        return Err(corrupt_state());
    }
    let next_revision = operation
        .revision
        .checked_next()
        .ok_or_else(corrupt_state)?;
    transaction
        .execute(
            "UPDATE operations SET state=?1, revision=?2, result_json=?3,
                error_code=?4, diagnostic_id=?5, updated_at_ms=?6, completed_at_ms=?6
             WHERE operation_id=?7",
            params![
                state_name(terminal),
                sqlite_revision(next_revision),
                result_json,
                error_code,
                diagnostic_id,
                completed_at_ms,
                operation_id.to_string()
            ],
        )
        .map_err(store_failure)?;
    transaction
        .execute(
            "UPDATE operation_journal SET state='applied', updated_at_ms=?1
             WHERE operation_id=?2 AND step=1",
            params![completed_at_ms, operation_id.to_string()],
        )
        .map_err(store_failure)?;
    insert_event(
        &transaction,
        &operation.owner,
        operation_id,
        next_revision,
        "operation.completed",
        &format!("{{\"state\":\"{}\"}}", state_name(terminal)),
        completed_at_ms,
    )?;
    let updated = load_operation(&transaction, operation_id)?;
    transaction.commit().map_err(store_failure)?;
    Ok(updated)
}

pub(super) fn recover(
    connection: &mut Connection,
    recovered_at_ms: u64,
) -> Result<Vec<OperationId>, StoreError> {
    let recovered_at_ms = sqlite_integer(recovered_at_ms)?;
    let candidates = {
        let mut statement = connection
            .prepare(
                "SELECT operation_id, state, kind FROM operations
                 WHERE state NOT IN ('succeeded', 'failed', 'cancelled')
                 ORDER BY created_at_ms ASC, operation_id ASC",
            )
            .map_err(store_failure)?;
        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(store_failure)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(store_failure)?
    };
    let mut schedule = Vec::new();
    for (operation_id, state, kind) in candidates {
        let operation_id = OperationId::parse(&operation_id).map_err(|_| corrupt_state())?;
        // Later milestone handlers own recovery for their durable Operation kinds. The M2
        // state-check recovery pass must leave those rows untouched so the package/Template
        // recovery pass can reuse its own journal and immutable Plan authority.
        if kind == "packages.apply"
            || kind.starts_with("templates.")
            || matches!(kind.as_str(), "backups.create" | "backups.restore")
            || matches!(kind.as_str(), "extensions.install" | "extensions.uninstall")
        {
            continue;
        }
        if kind != "state.check" || !journal_is_recoverable(connection, operation_id)? {
            fail_recovery(connection, operation_id, recovered_at_ms)?;
            continue;
        }
        match parse_state(&state)? {
            OperationState::Queued => schedule.push(operation_id),
            OperationState::Running | OperationState::Cancelling | OperationState::Recovering => {
                transition_recovery(
                    connection,
                    operation_id,
                    OperationState::Interrupted,
                    recovered_at_ms,
                )?;
                transition_recovery(
                    connection,
                    operation_id,
                    OperationState::Recovering,
                    recovered_at_ms,
                )?;
                schedule.push(operation_id);
            }
            OperationState::Interrupted => {
                transition_recovery(
                    connection,
                    operation_id,
                    OperationState::Recovering,
                    recovered_at_ms,
                )?;
                schedule.push(operation_id);
            }
            OperationState::Planning | OperationState::WaitingForInput => {
                fail_recovery(connection, operation_id, recovered_at_ms)?;
            }
            OperationState::Succeeded | OperationState::Failed | OperationState::Cancelled => {}
        }
    }
    Ok(schedule)
}

fn journal_is_recoverable(
    connection: &Connection,
    operation_id: OperationId,
) -> Result<bool, StoreError> {
    let count: i64 = connection
        .query_row(
            "SELECT count(*) FROM operation_journal
             WHERE operation_id=?1 AND step=1 AND kind='state.check' AND state='prepared'",
            [operation_id.to_string()],
            |row| row.get(0),
        )
        .map_err(store_failure)?;
    Ok(count == 1)
}

fn fail_recovery(
    connection: &mut Connection,
    operation_id: OperationId,
    recovered_at_ms: i64,
) -> Result<(), StoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(store_failure)?;
    let operation = load_operation(&transaction, operation_id)?;
    let revision = operation
        .revision
        .checked_next()
        .ok_or_else(corrupt_state)?;
    let diagnostic_id = Uuid::new_v4().to_string();
    transaction
        .execute(
            "UPDATE operations SET state='failed', revision=?1, error_code='internal_error',
                diagnostic_id=?2, updated_at_ms=?3, completed_at_ms=?3
             WHERE operation_id=?4",
            params![
                sqlite_revision(revision),
                diagnostic_id,
                recovered_at_ms,
                operation_id.to_string()
            ],
        )
        .map_err(store_failure)?;
    insert_event(
        &transaction,
        &operation.owner,
        operation_id,
        revision,
        "operation.recovery_failed",
        "{\"state\":\"failed\",\"errorCode\":\"internal_error\"}",
        recovered_at_ms,
    )?;
    transaction.commit().map_err(store_failure)
}

fn transition_recovery(
    connection: &mut Connection,
    operation_id: OperationId,
    next_state: OperationState,
    recovered_at_ms: i64,
) -> Result<(), StoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(store_failure)?;
    let operation = load_operation(&transaction, operation_id)?;
    if !operation.state.can_transition_to(next_state) {
        return Err(corrupt_state());
    }
    let revision = operation
        .revision
        .checked_next()
        .ok_or_else(corrupt_state)?;
    transaction
        .execute(
            "UPDATE operations SET state=?1, revision=?2, updated_at_ms=?3
             WHERE operation_id=?4",
            params![
                state_name(next_state),
                sqlite_revision(revision),
                recovered_at_ms,
                operation_id.to_string()
            ],
        )
        .map_err(store_failure)?;
    insert_event(
        &transaction,
        &operation.owner,
        operation_id,
        revision,
        "operation.recovered",
        &format!("{{\"state\":\"{}\"}}", state_name(next_state)),
        recovered_at_ms,
    )?;
    transaction.commit().map_err(store_failure)
}

pub(super) fn load_owned_operation(
    connection: &Connection,
    owner: &PrincipalId,
    operation_id: OperationId,
) -> Result<OperationRecord, StoreError> {
    query_operation(
        connection,
        "SELECT operation_id, kind, state, revision, owner_principal_id,
            cancel_requested, created_at_ms, updated_at_ms, started_at_ms,
            completed_at_ms, result_json, error_code, diagnostic_id,
            coalesce((SELECT phase FROM package_filesystem_journal j
             WHERE j.operation_id=operations.operation_id ORDER BY step DESC LIMIT 1),
             (SELECT phase FROM backup_restore_filesystem_journal j
              WHERE j.operation_id=operations.operation_id ORDER BY step DESC LIMIT 1),
             (SELECT json_extract(payload_json,'$.phase') FROM operation_journal j
              WHERE j.operation_id=operations.operation_id AND j.kind='backups.create'
              ORDER BY step DESC LIMIT 1))
         FROM operations WHERE operation_id=?1 AND owner_principal_id=?2",
        params![operation_id.to_string(), owner.as_str()],
    )
}

fn load_operation(
    connection: &Connection,
    operation_id: OperationId,
) -> Result<OperationRecord, StoreError> {
    query_operation(
        connection,
        "SELECT operation_id, kind, state, revision, owner_principal_id,
            cancel_requested, created_at_ms, updated_at_ms, started_at_ms,
            completed_at_ms, result_json, error_code, diagnostic_id,
            coalesce((SELECT phase FROM package_filesystem_journal j
             WHERE j.operation_id=operations.operation_id ORDER BY step DESC LIMIT 1),
             (SELECT phase FROM backup_restore_filesystem_journal j
              WHERE j.operation_id=operations.operation_id ORDER BY step DESC LIMIT 1),
             (SELECT json_extract(payload_json,'$.phase') FROM operation_journal j
              WHERE j.operation_id=operations.operation_id AND j.kind='backups.create'
              ORDER BY step DESC LIMIT 1))
         FROM operations WHERE operation_id=?1",
        [operation_id.to_string()],
    )
}

fn query_operation<P: rusqlite::Params>(
    connection: &Connection,
    sql: &str,
    parameters: P,
) -> Result<OperationRecord, StoreError> {
    connection
        .query_row(sql, parameters, map_operation_row)
        .optional()
        .map_err(store_failure)?
        .ok_or_else(|| StoreError::new(StoreErrorKind::OperationNotFound))
}

fn map_operation_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<OperationRecord> {
    let operation_id =
        OperationId::parse(&row.get::<_, String>(0)?).map_err(|_| rusqlite::Error::InvalidQuery)?;
    let state =
        parse_state(&row.get::<_, String>(2)?).map_err(|_| rusqlite::Error::InvalidQuery)?;
    let revision_value = row.get::<_, i64>(3)?;
    let revision =
        Revision::new(u64::try_from(revision_value).map_err(|_| rusqlite::Error::InvalidQuery)?)
            .ok_or(rusqlite::Error::InvalidQuery)?;
    let owner =
        PrincipalId::parse(row.get::<_, String>(4)?).map_err(|_| rusqlite::Error::InvalidQuery)?;
    Ok(OperationRecord {
        operation_id,
        kind: row.get(1)?,
        state,
        revision,
        owner,
        cancel_requested: row.get::<_, i64>(5)? != 0,
        created_at_ms: positive_or_zero(row.get(6)?)?,
        updated_at_ms: positive_or_zero(row.get(7)?)?,
        started_at_ms: optional_positive_or_zero(row.get(8)?)?,
        completed_at_ms: optional_positive_or_zero(row.get(9)?)?,
        result_json: row.get(10)?,
        error_code: row.get(11)?,
        diagnostic_id: row.get(12)?,
        progress_phase: row
            .get::<_, Option<String>>(13)?
            .map(|value| parse_filesystem_phase(&value))
            .transpose()
            .map_err(|()| rusqlite::Error::InvalidQuery)?,
    })
}

pub(super) fn list_operations(
    connection: &Connection,
    owner: &PrincipalId,
    cursor: Option<OperationCursor>,
    limit: u32,
) -> Result<OperationPage, StoreError> {
    if limit == 0 || limit > 1_000 {
        return Err(corrupt_state());
    }
    let (cursor_time, cursor_id) = match cursor {
        Some(cursor) => (
            sqlite_integer(cursor.created_at_ms)?,
            cursor.operation_id.to_string(),
        ),
        None => (i64::MAX, "ffffffff-ffff-ffff-ffff-ffffffffffff".to_owned()),
    };
    let mut statement = connection
        .prepare(
            "SELECT operation_id, kind, state, revision, owner_principal_id,
                cancel_requested, created_at_ms, updated_at_ms, started_at_ms,
                completed_at_ms, result_json, error_code, diagnostic_id,
                coalesce((SELECT phase FROM package_filesystem_journal j
                 WHERE j.operation_id=operations.operation_id ORDER BY step DESC LIMIT 1),
                 (SELECT phase FROM backup_restore_filesystem_journal j
                  WHERE j.operation_id=operations.operation_id ORDER BY step DESC LIMIT 1),
                 (SELECT json_extract(payload_json,'$.phase') FROM operation_journal j
                  WHERE j.operation_id=operations.operation_id AND j.kind='backups.create'
                  ORDER BY step DESC LIMIT 1))
             FROM operations
             WHERE owner_principal_id=?1
               AND (created_at_ms < ?2 OR (created_at_ms = ?2 AND operation_id < ?3))
             ORDER BY created_at_ms DESC, operation_id DESC LIMIT ?4",
        )
        .map_err(store_failure)?;
    let operations = statement
        .query_map(
            params![owner.as_str(), cursor_time, cursor_id, i64::from(limit)],
            map_operation_row,
        )
        .map_err(store_failure)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(store_failure)?;
    let next_cursor = operations.last().map(|operation| OperationCursor {
        created_at_ms: operation.created_at_ms,
        operation_id: operation.operation_id,
    });
    Ok(OperationPage {
        operations,
        next_cursor,
    })
}

fn parse_filesystem_phase(value: &str) -> Result<FilesystemPhase, ()> {
    use FilesystemPhase as Phase;
    match value {
        "accepted" => Ok(Phase::Accepted),
        "inventory_ready" => Ok(Phase::InventoryReady),
        "archiving" => Ok(Phase::Archiving),
        "archive_ready" => Ok(Phase::ArchiveReady),
        "publish_intent" => Ok(Phase::PublishIntent),
        "archive_published" => Ok(Phase::ArchivePublished),
        "archive_verified" => Ok(Phase::ArchiveVerified),
        "extracting" => Ok(Phase::Extracting),
        "staging_complete" => Ok(Phase::StagingComplete),
        "target_published" => Ok(Phase::TargetPublished),
        "project_registry_commit_intent" => Ok(Phase::ProjectRegistryCommitIntent),
        "extracted" => Ok(Phase::Extracted),
        "prepared" => Ok(Phase::Prepared),
        "packages_replaced" => Ok(Phase::PackagesReplaced),
        "vpm_manifest_committed" => Ok(Phase::VpmManifestCommitted),
        "filesystem_committed" => Ok(Phase::FilesystemCommitted),
        "state_committed" => Ok(Phase::StateCommitted),
        "rolling_back" => Ok(Phase::RollingBack),
        "rolled_back" => Ok(Phase::RolledBack),
        "recovery_required" => Ok(Phase::RecoveryRequired),
        _ => Err(()),
    }
}

pub(super) fn list_events(
    connection: &Connection,
    owner: &PrincipalId,
    after_sequence: u64,
    limit: u32,
) -> Result<EventPage, StoreError> {
    if limit == 0 || limit > 1_000 {
        return Err(corrupt_state());
    }
    let after_sequence_sql = sqlite_integer(after_sequence)?;
    let mut statement = connection
        .prepare(
            "SELECT sequence, event_id, kind, aggregate_kind, aggregate_id,
                aggregate_revision, occurred_at_ms, payload_json
             FROM events WHERE principal_id=?1 AND sequence>?2
             ORDER BY sequence ASC LIMIT ?3",
        )
        .map_err(store_failure)?;
    let events = statement
        .query_map(
            params![owner.as_str(), after_sequence_sql, i64::from(limit)],
            |row| {
                let revision_value = row.get::<_, i64>(5)?;
                Ok(EventRecord {
                    sequence: positive(row.get(0)?)?,
                    event_id: row.get(1)?,
                    kind: row.get(2)?,
                    aggregate_kind: row.get(3)?,
                    aggregate_id: row.get(4)?,
                    aggregate_revision: Revision::new(
                        u64::try_from(revision_value).map_err(|_| rusqlite::Error::InvalidQuery)?,
                    )
                    .ok_or(rusqlite::Error::InvalidQuery)?,
                    occurred_at_ms: positive_or_zero(row.get(6)?)?,
                    payload_json: row.get(7)?,
                })
            },
        )
        .map_err(store_failure)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(store_failure)?;
    let next_sequence = events.last().map_or(after_sequence, |event| event.sequence);
    Ok(EventPage {
        events,
        next_sequence,
    })
}

pub(super) fn cancellation_requested(
    connection: &Connection,
    operation_id: OperationId,
) -> Result<bool, StoreError> {
    connection
        .query_row(
            "SELECT cancel_requested FROM operations WHERE operation_id=?1",
            [operation_id.to_string()],
            |row| Ok(row.get::<_, i64>(0)? != 0),
        )
        .optional()
        .map_err(store_failure)?
        .ok_or_else(|| StoreError::new(StoreErrorKind::OperationNotFound))
}

pub(super) fn check_integrity(connection: &Connection) -> Result<CheckClassification, StoreError> {
    classify_rows(connection, "PRAGMA integrity_check(100)", true)
}

pub(super) fn check_foreign_keys(
    connection: &Connection,
) -> Result<CheckClassification, StoreError> {
    classify_rows(connection, "PRAGMA foreign_key_check", false)
}

fn classify_rows(
    connection: &Connection,
    sql: &str,
    integrity_mode: bool,
) -> Result<CheckClassification, StoreError> {
    let mut statement = connection.prepare(sql).map_err(store_failure)?;
    let mut rows = statement.query([]).map_err(store_failure)?;
    let mut count = 0_usize;
    let mut bytes = 0_usize;
    let mut only_ok = false;
    while let Some(row) = rows.next().map_err(store_failure)? {
        let first = row.get::<_, String>(0).map_err(store_failure)?;
        count += 1;
        bytes = bytes.saturating_add(first.len());
        only_ok = integrity_mode && count == 1 && first == "ok";
        if count >= CHECK_ROW_LIMIT || bytes >= CHECK_BYTE_LIMIT {
            return Ok(CheckClassification::IssuesTruncated);
        }
    }
    if (integrity_mode && only_ok && count == 1) || (!integrity_mode && count == 0) {
        Ok(CheckClassification::Ok)
    } else {
        Ok(CheckClassification::IssuesDetected)
    }
}

fn insert_event(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    operation_id: OperationId,
    revision: Revision,
    kind: &str,
    payload_json: &str,
    occurred_at_ms: i64,
) -> Result<(), StoreError> {
    transaction
        .execute(
            "INSERT INTO events (
                event_id, kind, aggregate_kind, aggregate_id, aggregate_revision,
                principal_id, occurred_at_ms, payload_json
            ) VALUES (?1, ?2, 'operation', ?3, ?4, ?5, ?6, ?7)",
            params![
                Uuid::new_v4().to_string(),
                kind,
                operation_id.to_string(),
                sqlite_revision(revision),
                owner.as_str(),
                occurred_at_ms,
                payload_json
            ],
        )
        .map_err(store_failure)?;
    Ok(())
}

fn state_name(state: OperationState) -> &'static str {
    match state {
        OperationState::Queued => "queued",
        OperationState::Planning => "planning",
        OperationState::WaitingForInput => "waiting_for_input",
        OperationState::Running => "running",
        OperationState::Cancelling => "cancelling",
        OperationState::Succeeded => "succeeded",
        OperationState::Failed => "failed",
        OperationState::Cancelled => "cancelled",
        OperationState::Interrupted => "interrupted",
        OperationState::Recovering => "recovering",
    }
}

fn parse_state(value: &str) -> Result<OperationState, StoreError> {
    match value {
        "queued" => Ok(OperationState::Queued),
        "planning" => Ok(OperationState::Planning),
        "waiting_for_input" => Ok(OperationState::WaitingForInput),
        "running" => Ok(OperationState::Running),
        "cancelling" => Ok(OperationState::Cancelling),
        "succeeded" => Ok(OperationState::Succeeded),
        "failed" => Ok(OperationState::Failed),
        "cancelled" => Ok(OperationState::Cancelled),
        "interrupted" => Ok(OperationState::Interrupted),
        "recovering" => Ok(OperationState::Recovering),
        _ => Err(corrupt_state()),
    }
}

fn sqlite_integer(value: u64) -> Result<i64, StoreError> {
    i64::try_from(value).map_err(|_| corrupt_state())
}

fn sqlite_revision(value: Revision) -> i64 {
    i64::try_from(value.get()).expect("validated Revision is SQLite-bounded")
}

fn positive(value: i64) -> rusqlite::Result<u64> {
    if value <= 0 {
        return Err(rusqlite::Error::InvalidQuery);
    }
    u64::try_from(value).map_err(|_| rusqlite::Error::InvalidQuery)
}

fn positive_or_zero(value: i64) -> rusqlite::Result<u64> {
    u64::try_from(value).map_err(|_| rusqlite::Error::InvalidQuery)
}

fn optional_positive_or_zero(value: Option<i64>) -> rusqlite::Result<Option<u64>> {
    value.map(positive_or_zero).transpose()
}

fn store_failure(_: rusqlite::Error) -> StoreError {
    unavailable()
}

pub(super) fn unavailable() -> StoreError {
    StoreError::new(StoreErrorKind::Unavailable)
}

fn corrupt_state() -> StoreError {
    StoreError::new(StoreErrorKind::CorruptState)
}
