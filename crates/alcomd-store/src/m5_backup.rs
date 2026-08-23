use alcomd_application::{
    BackupArchiveEvidence, BackupCompression, BackupCreateOutcome, BackupCreateRequest,
    BackupCursor, BackupId, BackupOperationRecord, BackupPhase, BackupRecord, M5BackupError,
    M5BackupErrorCode, OperationId, PrincipalId, ProjectId, Revision, StoredBackupRecord,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};
use uuid::Uuid;

pub(super) fn list_backups(
    connection: &Connection,
    owner: &PrincipalId,
    project_id: Option<ProjectId>,
    cursor: Option<BackupCursor>,
    limit: u32,
) -> Result<Vec<StoredBackupRecord>, M5BackupError> {
    let (cursor_time, cursor_id) = cursor.map_or((i64::MAX, "~".to_owned()), |value| {
        (
            i64::try_from(value.created_at_ms).unwrap_or(i64::MAX),
            value.backup_id.to_string(),
        )
    });
    let mut statement = connection
        .prepare(
            "SELECT backup_id,source_project_id,archive_locator,file_identity_key,
                archive_sha256,byte_size,format_version,created_at_ms,compression_mode,
                exclude_vpm_packages
         FROM backups
         WHERE owner_principal_id=?1 AND (?2 IS NULL OR source_project_id=?2)
           AND (created_at_ms < ?3 OR (created_at_ms=?3 AND backup_id < ?4))
         ORDER BY created_at_ms DESC,backup_id DESC LIMIT ?5",
        )
        .map_err(failure)?;
    let rows = statement
        .query_map(
            params![
                owner.as_str(),
                project_id.map(|value| value.to_string()),
                cursor_time,
                cursor_id,
                i64::from(limit)
            ],
            backup_from_row,
        )
        .map_err(failure)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(failure)
}

pub(super) fn get_backup(
    connection: &Connection,
    owner: &PrincipalId,
    backup_id: BackupId,
) -> Result<StoredBackupRecord, M5BackupError> {
    connection
        .query_row(
            "SELECT backup_id,source_project_id,archive_locator,file_identity_key,
                archive_sha256,byte_size,format_version,created_at_ms,compression_mode,
                exclude_vpm_packages
         FROM backups WHERE owner_principal_id=?1 AND backup_id=?2",
            params![owner.as_str(), backup_id.to_string()],
            backup_from_row,
        )
        .optional()
        .map_err(failure)?
        .ok_or_else(|| error(M5BackupErrorCode::BackupNotFound))
}

pub(super) fn accept_backup_create(
    connection: &mut Connection,
    owner: &PrincipalId,
    request: BackupCreateRequest,
    key: &alcomd_application::IdempotencyKey,
) -> Result<BackupCreateOutcome, M5BackupError> {
    let transaction = transaction(connection)?;
    let fingerprint = format!(
        "{{\"compressionMode\":\"{}\",\"excludeVpmPackages\":{},\"expectedRevision\":{},\"projectId\":\"{}\"}}",
        request.compression_mode.as_str(),
        request.exclude_vpm_packages,
        request.expected_revision.get(),
        request.project_id
    );
    if let Some((existing_fingerprint, response)) = transaction
        .query_row(
            "SELECT request_fingerprint,response_json FROM idempotency_records
         WHERE principal_id=?1 AND method='backups.create' AND idempotency_key=?2",
            params![owner.as_str(), key.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(failure)?
    {
        if existing_fingerprint != fingerprint {
            return Err(error(M5BackupErrorCode::IdempotencyConflict));
        }
        let mut outcome: BackupCreateOutcome =
            serde_json::from_str(&response).map_err(|_| internal())?;
        outcome.replayed = true;
        outcome.schedule = false;
        transaction.commit().map_err(failure)?;
        return Ok(outcome);
    }
    let revision = transaction
        .query_row(
            "SELECT revision FROM projects WHERE owner_principal_id=?1 AND project_id=?2",
            params![owner.as_str(), request.project_id.to_string()],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(failure)?
        .ok_or_else(|| error(M5BackupErrorCode::ProjectNotRegistered))?;
    if revision != integer(request.expected_revision.get())? {
        return Err(error(M5BackupErrorCode::RevisionConflict));
    }
    let operation_id = OperationId::new();
    let request_json = serde_json::to_string(&request).map_err(|_| internal())?;
    transaction.execute(
        "INSERT INTO operations (operation_id,kind,state,revision,owner_principal_id,request_json,created_at_ms,updated_at_ms)
         VALUES (?1,'backups.create','queued',1,?2,?3,?4,?4)",
        params![operation_id.to_string(), owner.as_str(), request_json, integer(request.created_at_ms)?],
    ).map_err(failure)?;
    transaction.execute(
        "INSERT INTO operation_journal (operation_id,step,kind,state,payload_json,updated_at_ms)
         VALUES (?1,1,'backups.create','prepared',?2,?3)",
        params![operation_id.to_string(), checkpoint_payload(BackupPhase::Accepted, Some(&request), None)?, integer(request.created_at_ms)?],
    ).map_err(failure)?;
    let outcome = BackupCreateOutcome {
        operation_id,
        backup_id: request.backup_id,
        replayed: false,
        schedule: true,
    };
    let response_json = serde_json::to_string(&outcome).map_err(|_| internal())?;
    transaction.execute(
        "INSERT INTO idempotency_records (principal_id,method,idempotency_key,request_fingerprint,state,operation_id,response_json,created_at_ms)
         VALUES (?1,'backups.create',?2,?3,'completed',?4,?5,?6)",
        params![owner.as_str(), key.as_str(), fingerprint, operation_id.to_string(), response_json, integer(request.created_at_ms)?],
    ).map_err(failure)?;
    insert_event(
        &transaction,
        owner,
        operation_id,
        Revision::INITIAL,
        "operation.created",
        request.created_at_ms,
    )?;
    transaction.commit().map_err(failure)?;
    Ok(outcome)
}

pub(super) fn begin_backup_create(
    connection: &mut Connection,
    operation_id: OperationId,
    now_ms: u64,
) -> Result<BackupOperationRecord, M5BackupError> {
    let transaction = transaction(connection)?;
    let (owner, request_json, state, revision) = transaction.query_row(
        "SELECT owner_principal_id,request_json,state,revision FROM operations WHERE operation_id=?1 AND kind='backups.create'",
        [operation_id.to_string()], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, i64>(3)?)),
    ).optional().map_err(failure)?.ok_or_else(internal)?;
    if !matches!(state.as_str(), "queued" | "recovering") {
        return Err(internal());
    }
    let next = revision.checked_add(1).ok_or_else(internal)?;
    transaction.execute(
        "UPDATE operations SET state='running',revision=?1,updated_at_ms=?2,started_at_ms=coalesce(started_at_ms,?2) WHERE operation_id=?3",
        params![next, integer(now_ms)?, operation_id.to_string()],
    ).map_err(failure)?;
    let latest = latest_checkpoint(&transaction, operation_id)?;
    let request: BackupCreateRequest =
        serde_json::from_str(&request_json).map_err(|_| internal())?;
    let owner = PrincipalId::parse(owner).map_err(|_| internal())?;
    insert_event(
        &transaction,
        &owner,
        operation_id,
        revision_value(next)?,
        "operation.state_changed",
        now_ms,
    )?;
    transaction.commit().map_err(failure)?;
    Ok(BackupOperationRecord {
        owner,
        request,
        phase: latest.0,
        evidence: latest.1,
    })
}

pub(super) fn record_backup_checkpoint(
    connection: &mut Connection,
    operation_id: OperationId,
    phase: BackupPhase,
    evidence: Option<BackupArchiveEvidence>,
    now_ms: u64,
) -> Result<(), M5BackupError> {
    let transaction = transaction(connection)?;
    let step = phase_step(&phase);
    let payload = checkpoint_payload(phase.clone(), None, evidence.as_ref())?;
    let inserted = transaction.execute(
        "INSERT INTO operation_journal (operation_id,step,kind,state,payload_json,updated_at_ms)
         VALUES (?1,?2,'backups.create','prepared',?3,?4)
         ON CONFLICT(operation_id,step) DO NOTHING",
        params![operation_id.to_string(), step, payload, integer(now_ms)?],
    ).map_err(failure)?;
    let existing: String = transaction
        .query_row(
            "SELECT payload_json FROM operation_journal WHERE operation_id=?1 AND step=?2",
            params![operation_id.to_string(), step],
            |row| row.get(0),
        )
        .map_err(failure)?;
    if existing != payload {
        return Err(error(M5BackupErrorCode::RecoveryRequired));
    }
    if inserted == 1 {
        let (owner, state, revision) = operation_context(&transaction, operation_id)?;
        if !matches!(state.as_str(), "running" | "recovering" | "cancelling") {
            return Err(internal());
        }
        let next = revision.checked_add(1).ok_or_else(internal)?;
        transaction
            .execute(
                "UPDATE operations SET revision=?1,updated_at_ms=?2 WHERE operation_id=?3",
                params![next, integer(now_ms)?, operation_id.to_string()],
            )
            .map_err(failure)?;
        insert_event(
            &transaction,
            &owner,
            operation_id,
            revision_value(next)?,
            "operation.progress",
            now_ms,
        )?;
    }
    transaction.commit().map_err(failure)
}

pub(super) fn complete_backup_create(
    connection: &mut Connection,
    operation_id: OperationId,
    backup: StoredBackupRecord,
    now_ms: u64,
) -> Result<(), M5BackupError> {
    let transaction = transaction(connection)?;
    let (owner, state, revision) = operation_context(&transaction, operation_id)?;
    if !matches!(state.as_str(), "running" | "recovering" | "cancelling") {
        return Err(internal());
    }
    transaction.execute(
        "INSERT INTO backups (backup_id,owner_principal_id,source_project_id,archive_locator,file_identity_key,archive_sha256,byte_size,format_version,created_at_ms,compression_mode,exclude_vpm_packages)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
         ON CONFLICT(backup_id) DO NOTHING",
        params![backup.record.backup_id.to_string(), owner.as_str(), backup.record.source_project_id.to_string(), backup.archive_locator, backup.file_identity_key, backup.record.archive_sha256.as_slice(), integer(backup.record.archive_bytes)?, i64::from(backup.record.format_version), integer(backup.record.created_at_ms)?, backup.record.compression_mode.as_str(), i64::from(backup.record.exclude_vpm_packages)],
    ).map_err(failure)?;
    let stored = get_backup(&transaction, &owner, backup.record.backup_id)?;
    if stored != backup {
        return Err(error(M5BackupErrorCode::BackupIntegrityMismatch));
    }
    let evidence = latest_checkpoint(&transaction, operation_id)?
        .1
        .ok_or_else(|| error(M5BackupErrorCode::RecoveryRequired))?;
    if evidence.archive_sha256 != backup.record.archive_sha256
        || evidence.archive_bytes != backup.record.archive_bytes
    {
        return Err(error(M5BackupErrorCode::BackupIntegrityMismatch));
    }
    transaction.execute(
        "INSERT INTO operation_journal (operation_id,step,kind,state,payload_json,updated_at_ms)
         VALUES (?1,7,'backups.create','prepared',?2,?3)
         ON CONFLICT(operation_id,step) DO NOTHING",
        params![
            operation_id.to_string(),
            checkpoint_payload(BackupPhase::StateCommitted, None, Some(&evidence))?,
            integer(now_ms)?
        ],
    ).map_err(failure)?;
    let next = revision.checked_add(1).ok_or_else(internal)?;
    let result = serde_json::to_string(&backup.record).map_err(|_| internal())?;
    transaction.execute(
        "UPDATE operations SET state='succeeded',revision=?1,result_json=?2,error_code=NULL,diagnostic_id=NULL,updated_at_ms=?3,completed_at_ms=?3 WHERE operation_id=?4",
        params![next, result, integer(now_ms)?, operation_id.to_string()],
    ).map_err(failure)?;
    transaction
        .execute(
            "UPDATE operation_journal SET state='applied',updated_at_ms=?1 WHERE operation_id=?2",
            params![integer(now_ms)?, operation_id.to_string()],
        )
        .map_err(failure)?;
    insert_event(
        &transaction,
        &owner,
        operation_id,
        revision_value(next)?,
        "operation.completed",
        now_ms,
    )?;
    transaction.commit().map_err(failure)
}

pub(super) fn fail_backup_create(
    connection: &mut Connection,
    operation_id: OperationId,
    error_code: &str,
    diagnostic_id: &str,
    now_ms: u64,
) -> Result<(), M5BackupError> {
    let transaction = transaction(connection)?;
    let (owner, state, revision) = operation_context(&transaction, operation_id)?;
    if matches!(state.as_str(), "succeeded" | "failed" | "cancelled") {
        transaction.commit().map_err(failure)?;
        return Ok(());
    }
    let next = revision.checked_add(1).ok_or_else(internal)?;
    transaction.execute(
        "UPDATE operations SET state='failed',revision=?1,error_code=?2,diagnostic_id=?3,updated_at_ms=?4,completed_at_ms=?4 WHERE operation_id=?5",
        params![next, error_code, diagnostic_id, integer(now_ms)?, operation_id.to_string()],
    ).map_err(failure)?;
    insert_event(
        &transaction,
        &owner,
        operation_id,
        revision_value(next)?,
        "operation.failed",
        now_ms,
    )?;
    transaction.commit().map_err(failure)
}

pub(super) fn defer_backup_recovery(
    connection: &mut Connection,
    operation_id: OperationId,
    diagnostic_id: &str,
    now_ms: u64,
) -> Result<(), M5BackupError> {
    let transaction = transaction(connection)?;
    let (owner, state, revision) = operation_context(&transaction, operation_id)?;
    if !matches!(state.as_str(), "running" | "recovering" | "cancelling") {
        return Err(internal());
    }
    let next = revision.checked_add(1).ok_or_else(internal)?;
    transaction
        .execute(
            "UPDATE operations SET state='interrupted',revision=?1,error_code='backup_unavailable',
                diagnostic_id=?2,updated_at_ms=?3 WHERE operation_id=?4",
            params![
                next,
                diagnostic_id,
                integer(now_ms)?,
                operation_id.to_string()
            ],
        )
        .map_err(failure)?;
    insert_event(
        &transaction,
        &owner,
        operation_id,
        revision_value(next)?,
        "operation.interrupted",
        now_ms,
    )?;
    transaction.commit().map_err(failure)
}

pub(super) fn recover_backup_operations(
    connection: &mut Connection,
    now_ms: u64,
) -> Result<Vec<OperationId>, M5BackupError> {
    let transaction = transaction(connection)?;
    let mut statement = transaction.prepare(
        "SELECT operation_id FROM operations WHERE kind='backups.create' AND state IN ('queued','running','cancelling','interrupted','recovering') ORDER BY created_at_ms,operation_id"
    ).map_err(failure)?;
    let values = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(failure)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(failure)?;
    drop(statement);
    let mut operations = Vec::new();
    for value in values {
        let operation_id = OperationId::parse(&value).map_err(|_| internal())?;
        transaction
            .execute(
                "UPDATE operations SET state='recovering',updated_at_ms=?1 WHERE operation_id=?2",
                params![integer(now_ms)?, value],
            )
            .map_err(failure)?;
        operations.push(operation_id);
    }
    transaction.commit().map_err(failure)?;
    Ok(operations)
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckpointPayload {
    phase: BackupPhase,
    #[serde(skip_serializing_if = "Option::is_none")]
    request: Option<BackupCreateRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    evidence: Option<BackupArchiveEvidence>,
}

fn checkpoint_payload(
    phase: BackupPhase,
    request: Option<&BackupCreateRequest>,
    evidence: Option<&BackupArchiveEvidence>,
) -> Result<String, M5BackupError> {
    serde_json::to_string(&CheckpointPayload {
        phase,
        request: request.cloned(),
        evidence: evidence.cloned(),
    })
    .map_err(|_| internal())
}

fn latest_checkpoint(
    connection: &Connection,
    operation_id: OperationId,
) -> Result<(BackupPhase, Option<BackupArchiveEvidence>), M5BackupError> {
    let payload: String = connection.query_row(
        "SELECT payload_json FROM operation_journal WHERE operation_id=?1 AND kind='backups.create' ORDER BY step DESC LIMIT 1",
        [operation_id.to_string()], |row| row.get(0),
    ).map_err(failure)?;
    let checkpoint: CheckpointPayload = serde_json::from_str(&payload).map_err(|_| internal())?;
    Ok((checkpoint.phase, checkpoint.evidence))
}

fn phase_step(phase: &BackupPhase) -> i64 {
    match phase {
        BackupPhase::Accepted => 1,
        BackupPhase::InventoryReady => 2,
        BackupPhase::Archiving => 3,
        BackupPhase::ArchiveReady => 4,
        BackupPhase::PublishIntent => 5,
        BackupPhase::ArchivePublished => 6,
        BackupPhase::StateCommitted => 7,
    }
}

fn backup_from_row(row: &Row<'_>) -> rusqlite::Result<StoredBackupRecord> {
    let project = row
        .get::<_, Option<String>>(1)?
        .ok_or(rusqlite::Error::InvalidQuery)?;
    let compression = match row.get::<_, String>(8)?.as_str() {
        "store" => BackupCompression::Store,
        "fast" => BackupCompression::Fast,
        "maximum" => BackupCompression::Maximum,
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    Ok(StoredBackupRecord {
        record: BackupRecord {
            backup_id: BackupId::parse(&row.get::<_, String>(0)?)
                .map_err(|_| rusqlite::Error::InvalidQuery)?,
            source_project_id: ProjectId::parse(&project)
                .map_err(|_| rusqlite::Error::InvalidQuery)?,
            archive_sha256: digest(row.get(4)?)?,
            archive_bytes: unsigned(row.get(5)?)?,
            format_version: u32::try_from(row.get::<_, i64>(6)?)
                .map_err(|_| rusqlite::Error::InvalidQuery)?,
            created_at_ms: unsigned(row.get(7)?)?,
            compression_mode: compression,
            exclude_vpm_packages: row.get::<_, i64>(9)? == 1,
        },
        archive_locator: row.get(2)?,
        file_identity_key: row.get(3)?,
    })
}

fn operation_context(
    connection: &Connection,
    operation_id: OperationId,
) -> Result<(PrincipalId, String, i64), M5BackupError> {
    connection.query_row(
        "SELECT owner_principal_id,state,revision FROM operations WHERE operation_id=?1 AND kind='backups.create'",
        [operation_id.to_string()], |row| Ok((PrincipalId::parse(row.get::<_, String>(0)?).map_err(|_| rusqlite::Error::InvalidQuery)?, row.get(1)?, row.get(2)?)),
    ).optional().map_err(failure)?.ok_or_else(internal)
}

fn insert_event(
    connection: &Connection,
    owner: &PrincipalId,
    operation_id: OperationId,
    revision: Revision,
    kind: &str,
    now_ms: u64,
) -> Result<(), M5BackupError> {
    connection.execute(
        "INSERT INTO events (event_id,kind,aggregate_kind,aggregate_id,aggregate_revision,principal_id,occurred_at_ms,payload_json) VALUES (?1,?2,'operation',?3,?4,?5,?6,'{}')",
        params![Uuid::new_v4().to_string(), kind, operation_id.to_string(), integer(revision.get())?, owner.as_str(), integer(now_ms)?],
    ).map_err(failure)?;
    Ok(())
}

fn digest(bytes: Vec<u8>) -> rusqlite::Result<[u8; 32]> {
    bytes.try_into().map_err(|_| rusqlite::Error::InvalidQuery)
}
fn unsigned(value: i64) -> rusqlite::Result<u64> {
    u64::try_from(value).map_err(|_| rusqlite::Error::InvalidQuery)
}
fn integer(value: u64) -> Result<i64, M5BackupError> {
    i64::try_from(value).map_err(|_| error(M5BackupErrorCode::InvalidInput))
}
fn revision_value(value: i64) -> Result<Revision, M5BackupError> {
    Revision::new(unsigned(value).map_err(failure)?).ok_or_else(internal)
}
fn transaction(connection: &mut Connection) -> Result<Transaction<'_>, M5BackupError> {
    connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(failure)
}
pub(super) const fn unavailable() -> M5BackupError {
    error(M5BackupErrorCode::StoreUnavailable)
}
fn failure(_: rusqlite::Error) -> M5BackupError {
    unavailable()
}
const fn internal() -> M5BackupError {
    error(M5BackupErrorCode::Internal)
}
const fn error(code: M5BackupErrorCode) -> M5BackupError {
    M5BackupError::new(code)
}
