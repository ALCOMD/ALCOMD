use alcomd_application::{
    IdempotencyKey, PrincipalId, Revision, UserPackageCursor, UserPackageError,
    UserPackageErrorCode, UserPackageId, UserPackagePage, UserPackageRecord,
    UserPackageRemoveResult, UserPackageSnapshot, UserPackageWriteResult,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use uuid::Uuid;

const ENROLL: &str = "packages.userPackages.enroll";
const REFRESH: &str = "packages.userPackages.refresh";
const REMOVE: &str = "packages.userPackages.remove";

pub(super) fn replay_enroll(
    connection: &Connection,
    owner: &PrincipalId,
    source_path: &str,
    key: &IdempotencyKey,
) -> Result<Option<UserPackageWriteResult>, UserPackageError> {
    let mut response: Option<UserPackageWriteResult> = replay(
        connection,
        owner,
        ENROLL,
        key,
        &enroll_fingerprint(source_path)?,
    )?;
    if let Some(response) = &mut response {
        response.replayed = true;
    }
    Ok(response)
}

pub(super) fn replay_refresh(
    connection: &Connection,
    owner: &PrincipalId,
    id: UserPackageId,
    expected: Revision,
    key: &IdempotencyKey,
) -> Result<Option<UserPackageWriteResult>, UserPackageError> {
    let mut response: Option<UserPackageWriteResult> = replay(
        connection,
        owner,
        REFRESH,
        key,
        &mutation_fingerprint(id, expected)?,
    )?;
    if let Some(response) = &mut response {
        response.replayed = true;
    }
    Ok(response)
}

pub(super) fn list(
    connection: &Connection,
    owner: &PrincipalId,
    cursor: Option<UserPackageCursor>,
    limit: u32,
) -> Result<UserPackagePage, UserPackageError> {
    let mut statement = connection
        .prepare(
            "SELECT user_package_id,owner_principal_id,source_root_path,source_identity_key,
                    package_id,version,manifest_json,manifest_fingerprint,content_fingerprint,
                    archive_sha256,revision,created_at_ms,updated_at_ms
             FROM user_package_sources
             WHERE owner_principal_id=?1 AND
               (?2 IS NULL OR updated_at_ms < ?2 OR
                (updated_at_ms = ?2 AND user_package_id < ?3))
             ORDER BY updated_at_ms DESC,user_package_id DESC LIMIT ?4",
        )
        .map_err(|_| unavailable())?;
    let updated_at_ms = cursor
        .as_ref()
        .map(|value| integer(value.updated_at_ms))
        .transpose()?;
    let cursor_id = cursor.map(|value| value.user_package_id.to_string());
    let rows = statement
        .query_map(
            params![
                owner.as_str(),
                updated_at_ms,
                cursor_id,
                i64::from(limit) + 1
            ],
            load_row,
        )
        .map_err(|_| unavailable())?;
    let mut records = rows
        .map(|row| row.map_err(|_| internal()).and_then(record_from_row))
        .collect::<Result<Vec<_>, _>>()?;
    let has_more = records.len() > limit as usize;
    if has_more {
        records.pop();
    }
    let next_cursor = has_more.then(|| {
        let record = records
            .last()
            .expect("non-empty paginated User Package page");
        UserPackageCursor {
            updated_at_ms: record.updated_at_ms,
            user_package_id: record.user_package_id,
        }
    });
    Ok(UserPackagePage {
        user_packages: records,
        next_cursor,
    })
}

pub(super) fn get(
    connection: &Connection,
    owner: &PrincipalId,
    id: UserPackageId,
) -> Result<UserPackageRecord, UserPackageError> {
    connection
        .query_row(
            "SELECT user_package_id,owner_principal_id,source_root_path,source_identity_key,
                    package_id,version,manifest_json,manifest_fingerprint,content_fingerprint,
                    archive_sha256,revision,created_at_ms,updated_at_ms
             FROM user_package_sources WHERE owner_principal_id=?1 AND user_package_id=?2",
            params![owner.as_str(), id.to_string()],
            load_row,
        )
        .optional()
        .map_err(|_| unavailable())?
        .ok_or_else(|| error(UserPackageErrorCode::NotFound))
        .and_then(record_from_row)
}

pub(super) fn enroll(
    connection: &mut Connection,
    owner: &PrincipalId,
    snapshot: UserPackageSnapshot,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<UserPackageWriteResult, UserPackageError> {
    let fingerprint = enroll_fingerprint(&snapshot.source_root_path)?;
    let transaction = begin(connection)?;
    if let Some(mut response) =
        existing_response::<UserPackageWriteResult>(&transaction, owner, ENROLL, key, &fingerprint)?
    {
        response.replayed = true;
        transaction.commit().map_err(|_| unavailable())?;
        return Ok(response);
    }
    let duplicate: Option<i64> = transaction
        .query_row(
            "SELECT 1 FROM user_package_sources WHERE owner_principal_id=?1 AND
             (source_identity_key=?2 OR package_id=?3) LIMIT 1",
            params![
                owner.as_str(),
                snapshot.source_identity_key,
                snapshot.package_id
            ],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| unavailable())?;
    if duplicate.is_some() {
        return Err(error(UserPackageErrorCode::AlreadyEnrolled));
    }
    let id = UserPackageId::new();
    insert_snapshot(
        &transaction,
        owner,
        id,
        Revision::INITIAL,
        now_ms,
        now_ms,
        &snapshot,
    )?;
    let record = get(&transaction, owner, id)?;
    let response = UserPackageWriteResult {
        user_package: record,
        replayed: false,
    };
    insert_event(
        &transaction,
        owner,
        id,
        Revision::INITIAL,
        "user_package.enrolled",
        now_ms,
    )?;
    save_response(
        &transaction,
        owner,
        ENROLL,
        key,
        &fingerprint,
        &response,
        now_ms,
    )?;
    transaction.commit().map_err(|_| unavailable())?;
    Ok(response)
}

pub(super) fn refresh(
    connection: &mut Connection,
    owner: &PrincipalId,
    id: UserPackageId,
    expected: Revision,
    snapshot: UserPackageSnapshot,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<UserPackageWriteResult, UserPackageError> {
    let fingerprint = mutation_fingerprint(id, expected)?;
    let transaction = begin(connection)?;
    if let Some(mut response) = existing_response::<UserPackageWriteResult>(
        &transaction,
        owner,
        REFRESH,
        key,
        &fingerprint,
    )? {
        response.replayed = true;
        transaction.commit().map_err(|_| unavailable())?;
        return Ok(response);
    }
    let current = get(&transaction, owner, id)?;
    if current.revision != expected {
        return Err(error(UserPackageErrorCode::RevisionConflict));
    }
    if current.source_identity_key != snapshot.source_identity_key
        || current.package_id != snapshot.package_id
    {
        return Err(error(UserPackageErrorCode::SourceChanged));
    }
    let changed = current.content_fingerprint != snapshot.content_fingerprint
        || current.manifest_fingerprint != snapshot.manifest_fingerprint
        || current.archive_sha256 != snapshot.archive_sha256;
    let record = if changed {
        let next = current.revision.checked_next().ok_or_else(internal)?;
        transaction
            .execute(
                "UPDATE user_package_sources SET source_root_path=?1,version=?2,
                    manifest_json=?3,manifest_fingerprint=?4,content_fingerprint=?5,
                    archive_sha256=?6,revision=?7,updated_at_ms=?8
                 WHERE owner_principal_id=?9 AND user_package_id=?10 AND revision=?11",
                params![
                    snapshot.source_root_path,
                    snapshot.version,
                    snapshot.manifest_json,
                    snapshot.manifest_fingerprint.as_slice(),
                    snapshot.content_fingerprint.as_slice(),
                    hex(&snapshot.archive_sha256),
                    integer(next.get())?,
                    integer(now_ms)?,
                    owner.as_str(),
                    id.to_string(),
                    integer(expected.get())?,
                ],
            )
            .map_err(|_| unavailable())?;
        insert_event(
            &transaction,
            owner,
            id,
            next,
            "user_package.refreshed",
            now_ms,
        )?;
        get(&transaction, owner, id)?
    } else {
        current
    };
    let response = UserPackageWriteResult {
        user_package: record,
        replayed: false,
    };
    save_response(
        &transaction,
        owner,
        REFRESH,
        key,
        &fingerprint,
        &response,
        now_ms,
    )?;
    transaction.commit().map_err(|_| unavailable())?;
    Ok(response)
}

pub(super) fn remove(
    connection: &mut Connection,
    owner: &PrincipalId,
    id: UserPackageId,
    expected: Revision,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<UserPackageRemoveResult, UserPackageError> {
    let fingerprint = mutation_fingerprint(id, expected)?;
    let transaction = begin(connection)?;
    if let Some(mut response) = existing_response::<UserPackageRemoveResult>(
        &transaction,
        owner,
        REMOVE,
        key,
        &fingerprint,
    )? {
        response.replayed = true;
        transaction.commit().map_err(|_| unavailable())?;
        return Ok(response);
    }
    let current = get(&transaction, owner, id)?;
    if current.revision != expected {
        return Err(error(UserPackageErrorCode::RevisionConflict));
    }
    let next = current.revision.checked_next().ok_or_else(internal)?;
    transaction
        .execute(
            "DELETE FROM user_package_sources WHERE owner_principal_id=?1 AND
             user_package_id=?2 AND revision=?3",
            params![owner.as_str(), id.to_string(), integer(expected.get())?],
        )
        .map_err(|_| unavailable())?;
    let response = UserPackageRemoveResult {
        user_package_id: id,
        revision: next,
        removed: true,
        replayed: false,
    };
    insert_event(
        &transaction,
        owner,
        id,
        next,
        "user_package.removed",
        now_ms,
    )?;
    save_response(
        &transaction,
        owner,
        REMOVE,
        key,
        &fingerprint,
        &response,
        now_ms,
    )?;
    transaction.commit().map_err(|_| unavailable())?;
    Ok(response)
}

fn insert_snapshot(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    id: UserPackageId,
    revision: Revision,
    created_at_ms: u64,
    updated_at_ms: u64,
    snapshot: &UserPackageSnapshot,
) -> Result<(), UserPackageError> {
    transaction
        .execute(
            "INSERT INTO user_package_sources (
                user_package_id,owner_principal_id,source_root_path,source_identity_key,
                package_id,version,manifest_json,manifest_fingerprint,content_fingerprint,
                archive_sha256,revision,created_at_ms,updated_at_ms
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
            params![
                id.to_string(),
                owner.as_str(),
                snapshot.source_root_path,
                snapshot.source_identity_key,
                snapshot.package_id,
                snapshot.version,
                snapshot.manifest_json,
                snapshot.manifest_fingerprint.as_slice(),
                snapshot.content_fingerprint.as_slice(),
                hex(&snapshot.archive_sha256),
                integer(revision.get())?,
                integer(created_at_ms)?,
                integer(updated_at_ms)?,
            ],
        )
        .map_err(|_| unavailable())?;
    Ok(())
}

type RawRow = (
    String,
    String,
    String,
    Vec<u8>,
    String,
    String,
    String,
    Vec<u8>,
    Vec<u8>,
    String,
    i64,
    i64,
    i64,
);

fn load_row(row: &Row<'_>) -> rusqlite::Result<RawRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
        row.get(9)?,
        row.get(10)?,
        row.get(11)?,
        row.get(12)?,
    ))
}

fn record_from_row(row: RawRow) -> Result<UserPackageRecord, UserPackageError> {
    let manifest: Value = serde_json::from_str(&row.6).map_err(|_| internal())?;
    let display_name = manifest
        .get("displayName")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let dependencies_json = serde_json::to_string(
        manifest
            .get("vpmDependencies")
            .unwrap_or(&Value::Object(Default::default())),
    )
    .map_err(|_| internal())?;
    Ok(UserPackageRecord {
        user_package_id: UserPackageId::parse(&row.0).map_err(|_| internal())?,
        owner: PrincipalId::parse(row.1).map_err(|_| internal())?,
        source_root_path: row.2,
        source_identity_key: row.3,
        package_id: row.4,
        version: row.5,
        display_name,
        manifest_json: row.6,
        dependencies_json,
        manifest_fingerprint: array32(row.7)?,
        content_fingerprint: array32(row.8)?,
        archive_sha256: parse_hex(&row.9)?,
        revision: revision(row.10)?,
        created_at_ms: unsigned(row.11)?,
        updated_at_ms: unsigned(row.12)?,
    })
}

fn replay<T: DeserializeOwned>(
    connection: &Connection,
    owner: &PrincipalId,
    method: &str,
    key: &IdempotencyKey,
    fingerprint: &str,
) -> Result<Option<T>, UserPackageError> {
    let row = connection
        .query_row(
            "SELECT request_fingerprint,response_json FROM idempotency_records
             WHERE principal_id=?1 AND method=?2 AND idempotency_key=?3",
            params![owner.as_str(), method, key.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|_| unavailable())?;
    match row {
        Some((stored, _)) if stored != fingerprint => {
            Err(error(UserPackageErrorCode::IdempotencyConflict))
        }
        Some((_, response)) => serde_json::from_str(&response)
            .map(Some)
            .map_err(|_| internal()),
        None => Ok(None),
    }
}

fn existing_response<T: DeserializeOwned>(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    method: &str,
    key: &IdempotencyKey,
    fingerprint: &str,
) -> Result<Option<T>, UserPackageError> {
    replay(transaction, owner, method, key, fingerprint)
}

fn save_response<T: Serialize>(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    method: &str,
    key: &IdempotencyKey,
    fingerprint: &str,
    response: &T,
    now_ms: u64,
) -> Result<(), UserPackageError> {
    transaction
        .execute(
            "INSERT INTO idempotency_records (
                principal_id,method,idempotency_key,request_fingerprint,state,
                operation_id,response_json,created_at_ms
             ) VALUES (?1,?2,?3,?4,'completed',NULL,?5,?6)",
            params![
                owner.as_str(),
                method,
                key.as_str(),
                fingerprint,
                serde_json::to_string(response).map_err(|_| internal())?,
                integer(now_ms)?,
            ],
        )
        .map_err(|_| unavailable())?;
    Ok(())
}

fn insert_event(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    id: UserPackageId,
    revision: Revision,
    kind: &str,
    now_ms: u64,
) -> Result<(), UserPackageError> {
    transaction
        .execute(
            "INSERT INTO events (
                event_id,kind,aggregate_kind,aggregate_id,aggregate_revision,
                principal_id,occurred_at_ms,payload_json
             ) VALUES (?1,?2,'user-package',?3,?4,?5,?6,'{}')",
            params![
                Uuid::new_v4().to_string(),
                kind,
                id.to_string(),
                integer(revision.get())?,
                owner.as_str(),
                integer(now_ms)?,
            ],
        )
        .map_err(|_| unavailable())?;
    Ok(())
}

fn enroll_fingerprint(source_path: &str) -> Result<String, UserPackageError> {
    serde_json::to_string(&serde_json::json!({"sourcePath": source_path, "version": 1}))
        .map_err(|_| internal())
}

fn mutation_fingerprint(id: UserPackageId, expected: Revision) -> Result<String, UserPackageError> {
    serde_json::to_string(&serde_json::json!({
        "expectedRevision": expected.get(), "userPackageId": id.to_string(), "version": 1
    }))
    .map_err(|_| internal())
}

fn begin(connection: &mut Connection) -> Result<Transaction<'_>, UserPackageError> {
    connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| unavailable())
}

fn integer(value: u64) -> Result<i64, UserPackageError> {
    i64::try_from(value).map_err(|_| internal())
}

fn unsigned(value: i64) -> Result<u64, UserPackageError> {
    u64::try_from(value).map_err(|_| internal())
}

fn revision(value: i64) -> Result<Revision, UserPackageError> {
    unsigned(value)
        .ok()
        .and_then(Revision::new)
        .ok_or_else(internal)
}

fn array32(value: Vec<u8>) -> Result<[u8; 32], UserPackageError> {
    value.try_into().map_err(|_| internal())
}

fn hex(value: &[u8; 32]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn parse_hex(value: &str) -> Result<[u8; 32], UserPackageError> {
    if value.len() != 64 {
        return Err(internal());
    }
    let mut result = [0_u8; 32];
    for (index, target) in result.iter_mut().enumerate() {
        *target =
            u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).map_err(|_| internal())?;
    }
    Ok(result)
}

fn error(code: UserPackageErrorCode) -> UserPackageError {
    UserPackageError::new(code)
}

fn unavailable() -> UserPackageError {
    error(UserPackageErrorCode::StoreUnavailable)
}

fn internal() -> UserPackageError {
    error(UserPackageErrorCode::Internal)
}
