use std::fs::{self, File};
use std::io::{ErrorKind, Read};
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

use alcomd_application::{
    CancelOperationFingerprintV1, CheckClassification, CreateOperationOutcome, EventPage,
    EventRecord, FilesystemPhase, IdempotencyKey, OperationCursor, OperationId, OperationPage,
    OperationRecord, OperationState, PrincipalId, Revision, StateCheckFingerprintV1,
    StateCheckResult, StoreError, StoreErrorKind,
};
use rusqlite::{
    Connection, OpenFlags, OptionalExtension, Transaction, TransactionBehavior, params,
};
use uuid::Uuid;

use crate::{CURRENT_DATA_SCHEMA, StoreOpenError, is_regular_state_file};

const DATA_SCHEMA_VERSION: i64 = CURRENT_DATA_SCHEMA as i64;
const BUSY_TIMEOUT: Duration = Duration::from_millis(5_000);
const CHECK_ROW_LIMIT: usize = 100;
const CHECK_BYTE_LIMIT: usize = 65_536;
const INITIAL_SCHEMA: &str = include_str!("schema.sql");
const SQLITE_HEADER: &[u8; 16] = b"SQLite format 3\0";
const USER_VERSION_OFFSET: usize = 60;
const SQLITE_HEADER_BYTES: usize = 100;

type SchemaObject = (String, String, Vec<u8>);
static CURRENT_SCHEMA: OnceLock<Vec<SchemaObject>> = OnceLock::new();

/// Checks that an existing state database opens with the current on-disk schema.
/// A missing database is valid when it has no orphaned SQLite companions.
pub fn preflight_state_database(path: &Path) -> Result<(), StoreOpenError> {
    check_state_database(path).map(|_| ())
}

fn check_state_database(path: &Path) -> Result<bool, StoreOpenError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) if is_regular_state_file(&metadata) => Some(metadata),
        Ok(_) => return Err(StoreOpenError::Unavailable),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(_) => return Err(StoreOpenError::Unavailable),
    };
    let mut has_companions = false;
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut companion = path.as_os_str().to_owned();
        companion.push(suffix);
        match fs::symlink_metadata(companion) {
            Ok(metadata) if is_regular_state_file(&metadata) => has_companions = true,
            Ok(_) => return Err(StoreOpenError::Unavailable),
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(_) => return Err(StoreOpenError::Unavailable),
        }
    }
    if metadata.is_none() {
        return if has_companions {
            Err(StoreOpenError::Unavailable)
        } else {
            Ok(false)
        };
    }
    // Read the main file before handing it to SQLite. Opening an old database
    // through SQLite could recover a journal or create shared-memory files.
    let mut header = [0; SQLITE_HEADER_BYTES];
    File::open(path)
        .and_then(|mut file| file.read_exact(&mut header))
        .map_err(|_| StoreOpenError::Unavailable)?;
    if &header[..SQLITE_HEADER.len()] != SQLITE_HEADER {
        return Err(StoreOpenError::Unavailable);
    }
    let mut version = [0; 4];
    version.copy_from_slice(&header[USER_VERSION_OFFSET..USER_VERSION_OFFSET + 4]);
    require_current(u32::from_be_bytes(version))?;

    // The schema and user_version are committed before the database first enters
    // WAL mode. Read only that committed main-file schema, without consulting or
    // creating WAL/SHM files. The normal connection recovers current data below.
    let immutable = Connection::open_with_flags(
        immutable_database_uri(path)?,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|_| StoreOpenError::Unavailable)?;
    verify_schema(&immutable)?;
    Ok(true)
}

fn immutable_database_uri(path: &Path) -> Result<String, StoreOpenError> {
    use std::fmt::Write as _;

    let absolute = std::path::absolute(path).map_err(|_| StoreOpenError::Unavailable)?;
    #[cfg(windows)]
    let absolute = absolute.to_string_lossy().replace('\\', "/");
    #[cfg(windows)]
    let path_bytes = absolute
        .strip_prefix("//?/")
        .unwrap_or(&absolute)
        .as_bytes();
    #[cfg(not(windows))]
    let path_bytes = absolute.as_os_str().as_encoded_bytes();

    let mut uri = String::from("file:");
    // Escape extra leading slashes as path bytes so they can't represent a URI
    // authority. SQLite's Windows VFS accepts ordinary absolute drive paths.
    for (index, &byte) in path_bytes.iter().enumerate() {
        if byte.is_ascii_alphanumeric()
            || matches!(byte, b'-' | b'.' | b'_' | b'~' | b':')
            || (byte == b'/' && index > 1)
        {
            uri.push(char::from(byte));
        } else {
            write!(uri, "%{byte:02X}").expect("writing to String");
        }
    }
    uri.push_str("?mode=ro&immutable=1");
    Ok(uri)
}

fn require_current(version: u32) -> Result<(), StoreOpenError> {
    if version != CURRENT_DATA_SCHEMA {
        return Err(StoreOpenError::UnsupportedDataSchema {
            found: version,
            supported: CURRENT_DATA_SCHEMA,
        });
    }
    Ok(())
}

fn schema_objects(connection: &Connection) -> rusqlite::Result<Vec<SchemaObject>> {
    connection
        .prepare(
            "SELECT type, lower(name), sql FROM sqlite_schema WHERE sql IS NOT NULL ORDER BY type, lower(name)",
        )?
        .query_map([], |row| {
            let sql: String = row.get(2)?;
            Ok((row.get(0)?, row.get(1)?, ddl_tokens(sql.as_bytes())))
        })?
        .collect()
}

// Compare the tokens of SQLite's stored DDL. Whitespace, comments and the case
// of unquoted identifiers/keywords have no effect. Literal bytes, token
// boundaries, multi-character operators and quoted content remain significant.
fn ddl_tokens(mut sql: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(sql.len());
    while let Some(&first) = sql.first() {
        if first.is_ascii_whitespace() {
            sql = &sql[1..];
            continue;
        }
        if sql.starts_with(b"--") {
            sql = &sql[sql.iter().position(|&b| b == b'\n').unwrap_or(sql.len())..];
            continue;
        }
        if sql.starts_with(b"/*") {
            sql = &sql[sql[2..]
                .windows(2)
                .position(|part| part == b"*/")
                .map_or(sql.len(), |i| i + 4)..];
            continue;
        }
        let word = |b: u8| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'$' | 128..=255);
        let len = if matches!(first, b'\'' | b'"' | b'`' | b'[') {
            let end = if first == b'[' { b']' } else { first };
            let mut i = 1;
            while i < sql.len() {
                i += 1;
                if sql[i - 1] == end {
                    if first == b'[' || sql.get(i) != Some(&end) {
                        break;
                    }
                    i += 1; // doubled quote
                }
            }
            i
        } else if word(first) {
            sql.iter().position(|&b| !word(b)).unwrap_or(sql.len())
        } else if sql.starts_with(b"->>") {
            3
        } else if matches!(
            sql.get(..2),
            Some(b"||" | b"->" | b"==" | b"!=" | b"<>" | b"<=" | b">=" | b"<<" | b">>")
        ) {
            2
        } else {
            1
        };
        let (token, rest) = sql.split_at(len);
        if word(first) {
            output.extend(token.iter().map(u8::to_ascii_lowercase));
        } else {
            output.extend_from_slice(token);
        }
        output.push(0);
        sql = rest;
    }
    output
}

fn verify_schema(connection: &Connection) -> Result<(), StoreOpenError> {
    let actual = schema_objects(connection).map_err(|_| StoreOpenError::Unavailable)?;
    if CURRENT_SCHEMA.get().is_none() {
        let reference = Connection::open_in_memory().map_err(|_| StoreOpenError::Unavailable)?;
        reference
            .execute_batch(INITIAL_SCHEMA)
            .map_err(|_| StoreOpenError::Unavailable)?;
        let expected = schema_objects(&reference).map_err(|_| StoreOpenError::Unavailable)?;
        let _ = CURRENT_SCHEMA.set(expected);
    }
    if CURRENT_SCHEMA.get() != Some(&actual) {
        return Err(StoreOpenError::Unavailable);
    }
    Ok(())
}

pub(super) fn initialize_connection(path: &Path) -> Result<Connection, StoreOpenError> {
    let existing = check_state_database(path)?;
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let flags = if existing {
        flags
    } else {
        flags | OpenFlags::SQLITE_OPEN_CREATE
    };
    let mut connection =
        Connection::open_with_flags(path, flags).map_err(|_| StoreOpenError::Unavailable)?;
    connection
        .busy_timeout(BUSY_TIMEOUT)
        .map_err(|_| StoreOpenError::Unavailable)?;
    connection
        .execute_batch("PRAGMA foreign_keys=ON; PRAGMA synchronous=FULL;")
        .map_err(|_| StoreOpenError::Unavailable)?;
    if !existing {
        initialize_schema(&mut connection)?;
    }
    let version: u32 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|_| StoreOpenError::Unavailable)?;
    require_current(version)?;
    let journal_mode: String = connection
        .query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))
        .map_err(|_| StoreOpenError::Unavailable)?;
    let foreign_keys: u32 = connection
        .pragma_query_value(None, "foreign_keys", |row| row.get(0))
        .map_err(|_| StoreOpenError::Unavailable)?;
    let synchronous: u32 = connection
        .pragma_query_value(None, "synchronous", |row| row.get(0))
        .map_err(|_| StoreOpenError::Unavailable)?;
    if foreign_keys != 1 || synchronous != 2 || !journal_mode.eq_ignore_ascii_case("wal") {
        return Err(StoreOpenError::Unavailable);
    }
    Ok(connection)
}

fn initialize_schema(connection: &mut Connection) -> Result<(), StoreOpenError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| StoreOpenError::Unavailable)?;
    let version: u32 = transaction
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|_| StoreOpenError::Unavailable)?;
    if version != 0 {
        require_current(version)?;
        verify_schema(&transaction)?;
    } else {
        if !schema_objects(&transaction)
            .map_err(|_| StoreOpenError::Unavailable)?
            .is_empty()
        {
            return Err(StoreOpenError::Unavailable);
        }
        transaction
            .execute_batch(INITIAL_SCHEMA)
            .map_err(|_| StoreOpenError::Unavailable)?;
        transaction
            .pragma_update(None, "user_version", DATA_SCHEMA_VERSION)
            .map_err(|_| StoreOpenError::Unavailable)?;
    }
    transaction
        .commit()
        .map_err(|_| StoreOpenError::Unavailable)
}

#[cfg(test)]
pub(crate) fn current_schema_fixture() -> Connection {
    let mut connection = Connection::open_in_memory().expect("in-memory fixture");
    connection
        .execute_batch("PRAGMA foreign_keys=ON")
        .expect("foreign keys");
    initialize_schema(&mut connection).expect("initialize fixture schema");
    connection
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
    let tool_guard = super::m8_tools::guard(
        &transaction,
        alcomd_application::ToolOperation::OperationCancel,
    )
    .map_err(|_| StoreError::new(StoreErrorKind::PermissionDenied))?;
    if let Some(tool) = tool_guard.as_ref()
        && (tool.control["operation_id"].as_str() != Some(operation_id.to_string().as_str())
            || tool.control["expected_revision"].as_u64() != Some(expected_revision.get())
            || tool.control["idempotency_key"].as_str() != Some(idempotency_key.as_str()))
    {
        return Err(StoreError::new(StoreErrorKind::PermissionDenied));
    }
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
        super::m8_tools::effect(&transaction,tool_guard.as_ref(),alcomd_application::ToolOperation::OperationCancel,serde_json::json!({"kind":"cancel","operationId":operation_id.to_string(),"replayed":true})).map_err(|_|StoreError::new(StoreErrorKind::PermissionDenied))?;
        transaction.commit().map_err(store_failure)?;
        return Ok((operation, true));
    }

    let operation = load_owned_operation(&transaction, owner, operation_id)?;
    if operation.revision != expected_revision {
        return Err(StoreError::new(StoreErrorKind::RevisionConflict));
    }
    if operation.state.is_terminal() || super::desktop::is_desktop(&operation.kind) {
        return Err(StoreError::new(StoreErrorKind::OperationNotCancellable));
    }
    if matches!(
        operation.kind.as_str(),
        "projects.copy" | "projects.create" | "projectTemplates.export"
    ) && matches!(
        operation.progress_phase,
        Some(
            FilesystemPhase::PublishIntent
                | FilesystemPhase::TargetPublished
                | FilesystemPhase::ProjectRegistryCommitIntent
                | FilesystemPhase::StateCommitted
                | FilesystemPhase::CleanupComplete
                | FilesystemPhase::RecoveryRequired
        )
    ) {
        return Err(StoreError::new(StoreErrorKind::OperationNotCancellable));
    }
    if operation.kind == "projects.delete-directory"
        && matches!(
            operation.progress_phase,
            Some(
                FilesystemPhase::QuarantineIntent
                    | FilesystemPhase::RootQuarantined
                    | FilesystemPhase::RegistryCommitIntent
                    | FilesystemPhase::StateCommitted
                    | FilesystemPhase::Deleting
                    | FilesystemPhase::CleanupComplete
                    | FilesystemPhase::RecoveryRequired
            )
        )
    {
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
    super::m8_tools::effect(&transaction,tool_guard.as_ref(),alcomd_application::ToolOperation::OperationCancel,serde_json::json!({"kind":"cancel","operationId":operation_id.to_string(),"state":state_name(updated.state),"revision":updated.revision.get(),"replayed":false})).map_err(|_|StoreError::new(StoreErrorKind::PermissionDenied))?;
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
        // Each dedicated handler owns recovery for its durable operation and journal.
        if super::desktop::is_desktop(&kind) {
            super::desktop::recover(connection, operation_id, recovered_at_ms)
                .map_err(|_| corrupt_state())?;
            continue;
        }
        if kind == "packages.apply"
            || matches!(kind.as_str(), "backups.create" | "backups.restore")
            || matches!(kind.as_str(), "extensions.install" | "extensions.uninstall")
            || matches!(
                kind.as_str(),
                "projects.create"
                    | "projects.copy"
                    | "projects.delete-directory"
                    | "projects.unity-migration"
                    | "projectTemplates.save"
                    | "projectTemplates.export"
            )
        {
            continue;
        }
        if kind != "state.check" || !journal_is_recoverable(connection, operation_id)? {
            fail_recovery(connection, operation_id, recovered_at_ms)?;
            continue;
        }
        match parse_state(&state)? {
            OperationState::Queued | OperationState::Recovering => schedule.push(operation_id),
            OperationState::Running | OperationState::Cancelling => {
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
             (SELECT phase FROM project_copy_filesystem_journal j
              WHERE j.operation_id=operations.operation_id ORDER BY step DESC LIMIT 1),
             (SELECT phase FROM project_delete_filesystem_journal j
              WHERE j.operation_id=operations.operation_id ORDER BY step DESC LIMIT 1),
             (SELECT json_extract(payload_json,'$.phase') FROM operation_journal j
              WHERE j.operation_id=operations.operation_id AND j.kind IN (
                  'backups.create','backups.restore','projects.create','projectTemplates.save','projectTemplates.export')
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
             (SELECT phase FROM project_copy_filesystem_journal j
              WHERE j.operation_id=operations.operation_id ORDER BY step DESC LIMIT 1),
             (SELECT phase FROM project_delete_filesystem_journal j
              WHERE j.operation_id=operations.operation_id ORDER BY step DESC LIMIT 1),
             (SELECT json_extract(payload_json,'$.phase') FROM operation_journal j
              WHERE j.operation_id=operations.operation_id AND j.kind IN (
                  'backups.create','backups.restore','projects.create','projectTemplates.save','projectTemplates.export')
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

pub(super) fn has_unfinished_operations(connection: &Connection) -> Result<bool, StoreError> {
    connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM operations WHERE state NOT IN ('succeeded','failed','cancelled'))",
        [],
        |row| row.get(0),
    ).map_err(store_failure)
}

pub(super) fn list_operations(
    connection: &Connection,
    owner: &PrincipalId,
    cursor: Option<OperationCursor>,
    limit: u32,
    active_only: bool,
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
                 (SELECT phase FROM project_copy_filesystem_journal j
                  WHERE j.operation_id=operations.operation_id ORDER BY step DESC LIMIT 1),
                 (SELECT phase FROM project_delete_filesystem_journal j
                  WHERE j.operation_id=operations.operation_id ORDER BY step DESC LIMIT 1),
                 (SELECT json_extract(payload_json,'$.phase') FROM operation_journal j
                  WHERE j.operation_id=operations.operation_id AND j.kind IN (
                      'backups.create','backups.restore','projects.create','projectTemplates.save','projectTemplates.export')
                  ORDER BY step DESC LIMIT 1))
             FROM operations
             WHERE owner_principal_id=?1
               AND (?5=0 OR state NOT IN ('succeeded','failed','cancelled'))
               AND (created_at_ms < ?2 OR (created_at_ms = ?2 AND operation_id < ?3))
             ORDER BY created_at_ms DESC, operation_id DESC LIMIT ?4",
        )
        .map_err(store_failure)?;
    let operations = statement
        .query_map(
            params![
                owner.as_str(),
                cursor_time,
                cursor_id,
                i64::from(limit),
                active_only
            ],
            map_operation_row,
        )
        .map_err(store_failure)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(store_failure)?;
    let next_cursor = operations.last().map(|operation| OperationCursor {
        created_at_ms: operation.created_at_ms,
        operation_id: operation.operation_id,
    });
    // The single worker owns the only writer; both SELECTs execute in the same
    // uninterrupted command and cannot observe different committed states.
    let active_count: i64 = connection.query_row(
        "SELECT count(*) FROM operations WHERE owner_principal_id=?1 AND state NOT IN ('succeeded','failed','cancelled')",
        [owner.as_str()], |row| row.get(0),
    ).map_err(store_failure)?;
    let active_count = u64::try_from(active_count).map_err(|_| corrupt_state())?;
    Ok(OperationPage {
        active_count,
        operations,
        next_cursor,
    })
}

fn parse_filesystem_phase(value: &str) -> Result<FilesystemPhase, ()> {
    use FilesystemPhase as Phase;
    match value {
        "accepted" => Ok(Phase::Accepted),
        "preflight_complete" => Ok(Phase::PreflightComplete),
        "quarantine_intent" => Ok(Phase::QuarantineIntent),
        "root_quarantined" => Ok(Phase::RootQuarantined),
        "registry_commit_intent" => Ok(Phase::RegistryCommitIntent),
        "inventory_ready" => Ok(Phase::InventoryReady),
        "archiving" => Ok(Phase::Archiving),
        "archive_ready" => Ok(Phase::ArchiveReady),
        "publish_intent" => Ok(Phase::PublishIntent),
        "archive_published" => Ok(Phase::ArchivePublished),
        "archive_verified" => Ok(Phase::ArchiveVerified),
        "extracting" => Ok(Phase::Extracting),
        "staging" => Ok(Phase::Staging),
        "staging_complete" => Ok(Phase::StagingComplete),
        "target_published" => Ok(Phase::TargetPublished),
        "project_registry_commit_intent" => Ok(Phase::ProjectRegistryCommitIntent),
        "extracted" => Ok(Phase::Extracted),
        "prepared" => Ok(Phase::Prepared),
        "packages_replaced" => Ok(Phase::PackagesReplaced),
        "vpm_manifest_committed" => Ok(Phase::VpmManifestCommitted),
        "filesystem_committed" => Ok(Phase::FilesystemCommitted),
        "state_committed" => Ok(Phase::StateCommitted),
        "deleting" => Ok(Phase::Deleting),
        "cleanup_complete" => Ok(Phase::CleanupComplete),
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
    let rows = statement
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
        .map_err(store_failure)?;
    let mut events = Vec::new();
    let mut bytes = 0usize;
    const PAGE_BYTES: usize = 4 * 1024 * 1024;
    for row in rows {
        let event = row.map_err(store_failure)?;
        // Include a conservative allowance for metadata JSON escaping. The
        // transport independently checks its complete encoded envelope.
        let size = event
            .payload_json
            .len()
            .saturating_add(1024)
            .saturating_add(
                6 * (event.event_id.len()
                    + event.kind.len()
                    + event.aggregate_kind.len()
                    + event.aggregate_id.len()),
            );
        if size > PAGE_BYTES {
            return Err(corrupt_state());
        }
        if bytes.saturating_add(size) > PAGE_BYTES {
            break;
        }
        bytes += size;
        events.push(event);
    }
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

pub(super) fn insert_event(
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
    super::operation_logs::operation_edge(
        transaction,
        owner,
        operation_id,
        kind,
        payload_json,
        occurred_at_ms,
    )?;
    Ok(())
}

pub(super) fn state_name(state: OperationState) -> &'static str {
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

pub(super) fn parse_state(value: &str) -> Result<OperationState, StoreError> {
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

fn store_failure(error: rusqlite::Error) -> StoreError {
    unavailable().with_source(error)
}

pub(super) fn unavailable() -> StoreError {
    StoreError::new(StoreErrorKind::Unavailable)
}

fn corrupt_state() -> StoreError {
    StoreError::new(StoreErrorKind::CorruptState)
}

#[cfg(test)]
#[path = "sqlite_authority_tests.rs"]
mod authority_tests;
