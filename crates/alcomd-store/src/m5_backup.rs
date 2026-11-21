use alcomd_application::{
    BackupArchiveEvidence, BackupCompression, BackupCreateOptions, BackupCreateOutcome,
    BackupCreateRequest, BackupOperationRecord, BackupPhase, M5BackupError, M5BackupErrorCode,
    OperationId, PrincipalId, ProjectId, Revision,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use uuid::Uuid;

pub(super) fn accept_backup_create(
    connection: &mut Connection,
    owner: &PrincipalId,
    request: BackupCreateRequest,
    options: BackupCreateOptions,
    key: &alcomd_application::IdempotencyKey,
) -> Result<BackupCreateOutcome, M5BackupError> {
    let transaction = transaction(connection)?;
    let fingerprint = create_fingerprint(&request, &options);
    if let Some(outcome) = replay_backup_create(
        &transaction,
        owner,
        request.project_id,
        request.expected_revision,
        options,
        key,
    )? {
        transaction.commit().map_err(failure)?;
        return Ok(outcome);
    }
    let resource_owner = alcomd_application::workflow_resource_owner(owner);
    let revision = transaction
        .query_row(
            "SELECT revision FROM projects WHERE owner_principal_id=?1 AND project_id=?2",
            params![resource_owner.as_str(), request.project_id.to_string()],
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

fn create_fingerprint(request: &BackupCreateRequest, options: &BackupCreateOptions) -> String {
    serde_json::json!({
        "projectId": request.project_id,
        "expectedRevision": request.expected_revision,
        "compressionMode": request.compression_mode,
        "excludeVpmPackages": request.exclude_vpm_packages,
        "includeLibrary": request.include_library,
        "destinationPath": options.destination_path,
    })
    .to_string()
}

pub(super) fn replay_backup_create(
    connection: &Connection,
    owner: &PrincipalId,
    project_id: ProjectId,
    expected_revision: Revision,
    options: BackupCreateOptions,
    key: &alcomd_application::IdempotencyKey,
) -> Result<Option<BackupCreateOutcome>, M5BackupError> {
    use alcomd_application::Permission;
    super::m8_workflows::require(
        connection,
        owner,
        &[Permission::BackupsManage, Permission::ProjectsRead],
    )
    .map_err(|cause| error(M5BackupErrorCode::PermissionDenied).with_source(cause))?;
    if let Some((existing_fingerprint, response)) = connection
        .query_row(
            "SELECT request_fingerprint,response_json FROM idempotency_records
         WHERE principal_id=?1 AND method='backups.create' AND idempotency_key=?2",
            params![owner.as_str(), key.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(failure)?
    {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct AcceptedSelection {
            compression_mode: BackupCompression,
            exclude_vpm_packages: bool,
            expected_revision: Revision,
            project_id: ProjectId,
            include_library: bool,
            destination_path: Option<String>,
        }
        let accepted: AcceptedSelection =
            serde_json::from_str(&existing_fingerprint).map_err(|_| internal())?;
        if accepted.project_id != project_id
            || accepted.expected_revision != expected_revision
            || options.destination_path != accepted.destination_path
            || options
                .compression_mode
                .is_some_and(|value| value != accepted.compression_mode)
            || options
                .exclude_vpm_packages
                .is_some_and(|value| value != accepted.exclude_vpm_packages)
            || options
                .include_library
                .is_some_and(|value| value != accepted.include_library)
        {
            return Err(error(M5BackupErrorCode::IdempotencyConflict));
        }
        let mut outcome: BackupCreateOutcome =
            serde_json::from_str(&response).map_err(|_| internal())?;
        outcome.replayed = true;
        outcome.schedule = false;
        return Ok(Some(outcome));
    }
    Ok(None)
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
    if !matches!(
        state.as_str(),
        "queued" | "recovering" | "cancelling" | "cancelled"
    ) {
        return Err(internal());
    }
    let start = matches!(state.as_str(), "queued" | "recovering");
    let next = if start {
        revision.checked_add(1).ok_or_else(internal)?
    } else {
        revision
    };
    if start {
        transaction.execute(
            "UPDATE operations SET state='running',revision=?1,updated_at_ms=?2,started_at_ms=coalesce(started_at_ms,?2) WHERE operation_id=?3",
            params![next, integer(now_ms)?, operation_id.to_string()],
        ).map_err(failure)?;
    }
    let latest = latest_checkpoint(&transaction, operation_id)?;
    let request: BackupCreateRequest =
        serde_json::from_str(&request_json).map_err(|_| internal())?;
    let owner = PrincipalId::parse(owner).map_err(|_| internal())?;
    if start {
        insert_event(
            &transaction,
            &owner,
            operation_id,
            revision_value(next)?,
            "operation.state_changed",
            now_ms,
        )?;
    }
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
        super::operation_logs::operation_phase(
            &transaction,
            &owner,
            operation_id,
            phase.as_str(),
            integer(now_ms)?,
        )
        .map_err(|error| unavailable().with_source(error))?;
    }
    transaction.commit().map_err(failure)
}

pub(super) fn complete_backup_create(
    connection: &mut Connection,
    operation_id: OperationId,
    archive_name: String,
    now_ms: u64,
) -> Result<(), M5BackupError> {
    let transaction = transaction(connection)?;
    let (owner, state, revision) = operation_context(&transaction, operation_id)?;
    if !matches!(state.as_str(), "running" | "recovering" | "cancelling") {
        return Err(internal());
    }
    let evidence = latest_checkpoint(&transaction, operation_id)?
        .1
        .ok_or_else(|| error(M5BackupErrorCode::RecoveryRequired))?;
    if evidence.archive_file_name != archive_name {
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
    let result = serde_json::json!({
        "archiveName": archive_name,
        "archiveBytes": evidence.archive_bytes,
    })
    .to_string();
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
    transaction.commit().map_err(failure)?;
    Ok(())
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
        "SELECT operation_id,state,revision,owner_principal_id FROM operations WHERE kind='backups.create' AND state IN ('queued','running','cancelling','interrupted','recovering') ORDER BY created_at_ms,operation_id"
    ).map_err(failure)?;
    let values = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(failure)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(failure)?;
    drop(statement);
    let mut operations = Vec::new();
    for (value, state, current_revision, owner) in values {
        let operation_id = OperationId::parse(&value).map_err(|_| internal())?;
        if state != "recovering" {
            let next = current_revision.checked_add(1).ok_or_else(internal)?;
            transaction.execute(
                "UPDATE operations SET state='recovering',revision=?1,updated_at_ms=?2 WHERE operation_id=?3",
                params![next, integer(now_ms)?, value],
            ).map_err(failure)?;
            insert_event(
                &transaction,
                &PrincipalId::parse(owner).map_err(|_| internal())?,
                operation_id,
                revision_value(next)?,
                "operation.recovered",
                now_ms,
            )?;
        }
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
    super::operation_logs::operation_edge(
        connection,
        owner,
        operation_id,
        kind,
        "{}",
        integer(now_ms)?,
    )
    .map_err(|error| unavailable().with_source(error))?;
    Ok(())
}

fn integer(value: u64) -> Result<i64, M5BackupError> {
    i64::try_from(value).map_err(|_| error(M5BackupErrorCode::InvalidInput))
}
fn revision_value(value: i64) -> Result<Revision, M5BackupError> {
    Revision::new(u64::try_from(value).map_err(|_| internal())?).ok_or_else(internal)
}
fn transaction(connection: &mut Connection) -> Result<Transaction<'_>, M5BackupError> {
    connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(failure)
}
pub(super) fn unavailable() -> M5BackupError {
    error(M5BackupErrorCode::StoreUnavailable)
}
fn failure(source: rusqlite::Error) -> M5BackupError {
    unavailable().with_source(source)
}
fn internal() -> M5BackupError {
    error(M5BackupErrorCode::Internal)
}
fn error(code: M5BackupErrorCode) -> M5BackupError {
    M5BackupError::new(code)
}
