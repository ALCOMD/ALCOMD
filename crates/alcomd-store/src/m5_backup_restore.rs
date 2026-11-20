use alcomd_application::{
    BackupCreateOutcome, BackupRestoreInput, BackupRestoreOperationRecord, BackupRestorePhase,
    BackupRestoreRequest, IdempotencyKey, M5BackupError, M5BackupErrorCode, OperationId,
    PrincipalId, RestoredProject, Revision,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use uuid::Uuid;

fn permission(cause: alcomd_application::ToolError) -> M5BackupError {
    error(M5BackupErrorCode::PermissionDenied).with_source(cause)
}

fn require_mcp_restore(db: &Connection, owner: &PrincipalId) -> Result<(), M5BackupError> {
    use alcomd_application::Permission;
    super::m8_workflows::require(
        db,
        owner,
        &[
            Permission::ProjectsCreate,
            Permission::BackupsRead,
            Permission::BackupsManage,
        ],
    )
    .map_err(permission)
}

pub(super) fn replay(
    connection: &Connection,
    owner: &PrincipalId,
    input: &BackupRestoreInput,
    key: &IdempotencyKey,
) -> Result<Option<BackupCreateOutcome>, M5BackupError> {
    let fingerprint = serde_json::to_string(input).map_err(|_| internal())?;
    require_mcp_restore(connection, owner)?;
    let Some((saved, response)) = connection
        .query_row(
            "SELECT request_fingerprint,response_json FROM idempotency_records
             WHERE principal_id=?1 AND method='backups.restore' AND idempotency_key=?2",
            params![owner.as_str(), key.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(failure)?
    else {
        return Ok(None);
    };
    if saved != fingerprint {
        return Err(error(M5BackupErrorCode::IdempotencyConflict));
    }
    let mut outcome: BackupCreateOutcome =
        serde_json::from_str(&response).map_err(|_| internal())?;
    outcome.replayed = true;
    outcome.schedule = false;
    Ok(Some(outcome))
}

pub(super) fn accept(
    connection: &mut Connection,
    owner: &PrincipalId,
    input: &BackupRestoreInput,
    request: BackupRestoreRequest,
    key: &IdempotencyKey,
    created_at_ms: u64,
) -> Result<BackupCreateOutcome, M5BackupError> {
    let fingerprint = serde_json::to_string(input).map_err(|_| internal())?;
    let transaction = transaction(connection)?;
    if let Some(outcome) = replay(&transaction, owner, input, key)? {
        return Ok(outcome);
    }
    let operation_id = OperationId::new();
    transaction
        .execute(
            "INSERT INTO operations (
                operation_id,kind,state,revision,owner_principal_id,request_json,
                created_at_ms,updated_at_ms
             ) VALUES (?1,'backups.restore','queued',1,?2,'{}',?3,?3)",
            params![
                operation_id.to_string(),
                owner.as_str(),
                integer(created_at_ms)?,
            ],
        )
        .map_err(failure)?;
    insert_journal(
        &transaction,
        operation_id,
        1,
        Some(&request),
        BackupRestorePhase::Accepted,
        None,
        created_at_ms,
    )?;
    let outcome = BackupCreateOutcome {
        operation_id,
        replayed: false,
        schedule: true,
    };
    transaction
        .execute(
            "INSERT INTO idempotency_records (
                principal_id,method,idempotency_key,request_fingerprint,state,
                operation_id,response_json,created_at_ms
             ) VALUES (?1,'backups.restore',?2,?3,'completed',?4,?5,?6)",
            params![
                owner.as_str(),
                key.as_str(),
                fingerprint,
                operation_id.to_string(),
                serde_json::to_string(&outcome).map_err(|_| internal())?,
                integer(created_at_ms)?,
            ],
        )
        .map_err(failure)?;
    insert_event(
        &transaction,
        owner,
        operation_id,
        Revision::INITIAL,
        "operation.created",
        created_at_ms,
    )?;
    transaction.commit().map_err(failure)?;
    Ok(outcome)
}

pub(super) fn begin(
    connection: &mut Connection,
    operation_id: OperationId,
    updated_at_ms: u64,
) -> Result<BackupRestoreOperationRecord, M5BackupError> {
    let transaction = transaction(connection)?;
    let (owner, state, revision) = operation_context(&transaction, operation_id)?;
    if !matches!(state.as_str(), "queued" | "recovering") {
        return Err(internal());
    }
    let next = revision.checked_add(1).ok_or_else(internal)?;
    transaction
        .execute(
            "UPDATE operations SET state='running',revision=?1,updated_at_ms=?2,
                    started_at_ms=coalesce(started_at_ms,?2) WHERE operation_id=?3",
            params![
                integer(next)?,
                integer(updated_at_ms)?,
                operation_id.to_string()
            ],
        )
        .map_err(failure)?;
    insert_event(
        &transaction,
        &owner,
        operation_id,
        revision_value(next)?,
        "operation.state_changed",
        updated_at_ms,
    )?;
    let request = load_request(&transaction, operation_id)?;
    let phase = latest_phase(&transaction, operation_id)?;
    transaction.commit().map_err(failure)?;
    Ok(BackupRestoreOperationRecord { request, phase })
}

pub(super) fn checkpoint(
    connection: &mut Connection,
    operation_id: OperationId,
    phase: BackupRestorePhase,
    restored: Option<RestoredProject>,
    updated_at_ms: u64,
) -> Result<(), M5BackupError> {
    let transaction = transaction(connection)?;
    let (owner, state, revision) = operation_context(&transaction, operation_id)?;
    if !matches!(state.as_str(), "running" | "recovering" | "cancelling") {
        return Err(internal());
    }
    let step = phase_step(phase);
    if latest_step(&transaction, operation_id)? >= step {
        return Ok(());
    }
    insert_journal(
        &transaction,
        operation_id,
        step,
        None,
        phase,
        restored.as_ref(),
        updated_at_ms,
    )?;
    let next = revision.checked_add(1).ok_or_else(internal)?;
    transaction
        .execute(
            "UPDATE operations SET revision=?1,updated_at_ms=?2 WHERE operation_id=?3",
            params![
                integer(next)?,
                integer(updated_at_ms)?,
                operation_id.to_string()
            ],
        )
        .map_err(failure)?;
    insert_event(
        &transaction,
        &owner,
        operation_id,
        revision_value(next)?,
        "operation.progress",
        updated_at_ms,
    )?;
    super::operation_logs::operation_phase(
        &transaction,
        &owner,
        operation_id,
        phase.as_str(),
        integer(updated_at_ms)?,
    )
    .map_err(|error| unavailable().with_source(error))?;
    transaction.commit().map_err(failure)
}

pub(super) fn complete(
    connection: &mut Connection,
    operation_id: OperationId,
    restored: RestoredProject,
    completed_at_ms: u64,
) -> Result<(), M5BackupError> {
    let transaction = transaction(connection)?;
    let (owner, state, revision) = operation_context(&transaction, operation_id)?;
    if !matches!(state.as_str(), "running" | "recovering" | "cancelling") {
        return Err(internal());
    }
    let request = load_request(&transaction, operation_id)?;
    if request.project_id != restored.project_id
        || restored.observation.root_path
            != request
                .target_parent
                .join(&request.target_name)
                .to_string_lossy()
    {
        return Err(error(M5BackupErrorCode::BackupRestoreRecoveryRequired));
    }
    let evidence: String = transaction
        .query_row(
            "SELECT payload_json FROM operation_journal
         WHERE operation_id=?1 AND kind='backups.restore' AND step=?2",
            params![
                operation_id.to_string(),
                integer(phase_step(BackupRestorePhase::ProjectRegistryCommitIntent))?
            ],
            |row| row.get(0),
        )
        .map_err(failure)?;
    let evidence: serde_json::Value = serde_json::from_str(&evidence).map_err(|_| internal())?;
    if evidence["restoredFingerprint"] != serde_json::json!(restored.project_fingerprint) {
        return Err(error(M5BackupErrorCode::BackupRestoreRecoveryRequired));
    }
    let resource_owner = alcomd_application::workflow_resource_owner(&owner);
    let existing = transaction
        .query_row(
            "SELECT project_id,root_path,path_identity_key FROM projects
             WHERE project_id=?1 OR path_identity_key=?2",
            params![
                restored.project_id.to_string(),
                restored.observation.path_identity_key
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                ))
            },
        )
        .optional()
        .map_err(failure)?;
    match existing {
        Some((id, path, identity))
            if id == restored.project_id.to_string()
                && path == restored.observation.root_path
                && identity == restored.observation.path_identity_key => {}
        Some(_) => return Err(error(M5BackupErrorCode::BackupRestoreRecoveryRequired)),
        None => {
            transaction
                .execute(
                    "INSERT INTO projects (
                        project_id,owner_principal_id,root_path,path_identity_key,project_type,
                        unity_version,unity_revision,snapshot_json,revision,registered_at_ms,
                        updated_at_ms,last_modified_at_ms
                     ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,1,?9,?9,?9)",
                    params![
                        restored.project_id.to_string(),
                        resource_owner.as_str(),
                        restored.observation.root_path,
                        restored.observation.path_identity_key,
                        super::m3::project_type(restored.observation.project_type),
                        restored.observation.unity_version,
                        restored.observation.unity_revision,
                        serde_json::to_string(&restored.observation).map_err(|_| internal())?,
                        integer(completed_at_ms)?,
                    ],
                )
                .map_err(failure)?;

            transaction
                .execute(
                    "INSERT INTO events (event_id,kind,aggregate_kind,aggregate_id,aggregate_revision,
                        principal_id,occurred_at_ms,payload_json)
                     VALUES (?1,'project.registered','project',?2,1,?3,?4,'{}')",
                    params![Uuid::new_v4().to_string(), restored.project_id.to_string(), resource_owner.as_str(), integer(completed_at_ms)?],
                )
                .map_err(failure)?;
        }
    }
    if latest_step(&transaction, operation_id)? >= phase_step(BackupRestorePhase::StateCommitted) {
        return transaction.commit().map_err(failure);
    }
    insert_journal(
        &transaction,
        operation_id,
        phase_step(BackupRestorePhase::StateCommitted),
        None,
        BackupRestorePhase::StateCommitted,
        Some(&restored),
        completed_at_ms,
    )?;
    let next = revision.checked_add(1).ok_or_else(internal)?;
    transaction
        .execute(
            "UPDATE operations SET revision=?1,updated_at_ms=?2 WHERE operation_id=?3",
            params![
                integer(next)?,
                integer(completed_at_ms)?,
                operation_id.to_string()
            ],
        )
        .map_err(failure)?;
    insert_event(
        &transaction,
        &owner,
        operation_id,
        revision_value(next)?,
        "operation.progress",
        completed_at_ms,
    )?;
    super::operation_logs::operation_phase(
        &transaction,
        &owner,
        operation_id,
        "state_committed",
        integer(completed_at_ms)?,
    )
    .map_err(|error| unavailable().with_source(error))?;
    transaction.commit().map_err(failure)
}

pub(super) fn finish_success(
    connection: &mut Connection,
    operation_id: OperationId,
    completed_at_ms: u64,
) -> Result<(), M5BackupError> {
    let transaction = transaction(connection)?;
    let (owner, state, revision) = operation_context(&transaction, operation_id)?;
    if state == "succeeded" {
        transaction.commit().map_err(failure)?;
        return Ok(());
    }
    if !matches!(state.as_str(), "running" | "recovering" | "cancelling")
        || latest_phase(&transaction, operation_id)? != BackupRestorePhase::StateCommitted
    {
        return Err(internal());
    }
    let request = load_request(&transaction, operation_id)?;
    let next = revision.checked_add(1).ok_or_else(internal)?;
    let result = serde_json::json!({
        "projectId": request.project_id,
        "targetName": request.target_name,
    })
    .to_string();
    transaction
        .execute(
            "UPDATE operations SET state='succeeded',revision=?1,result_json=?2,error_code=NULL,
                    diagnostic_id=NULL,updated_at_ms=?3,completed_at_ms=?3 WHERE operation_id=?4",
            params![
                integer(next)?,
                result,
                integer(completed_at_ms)?,
                operation_id.to_string()
            ],
        )
        .map_err(failure)?;
    transaction
        .execute(
            "UPDATE operation_journal SET state='applied',updated_at_ms=?1 WHERE operation_id=?2",
            params![integer(completed_at_ms)?, operation_id.to_string()],
        )
        .map_err(failure)?;
    insert_event(
        &transaction,
        &owner,
        operation_id,
        revision_value(next)?,
        "operation.completed",
        completed_at_ms,
    )?;
    transaction.commit().map_err(failure)?;
    Ok(())
}

pub(super) fn fail(
    connection: &mut Connection,
    operation_id: OperationId,
    error_code: &str,
    diagnostic_id: &str,
    completed_at_ms: u64,
) -> Result<(), M5BackupError> {
    if error_code.is_empty() || error_code.len() > 128 || Uuid::parse_str(diagnostic_id).is_err() {
        return Err(internal());
    }
    let transaction = transaction(connection)?;
    let (owner, state, revision) = operation_context(&transaction, operation_id)?;
    if matches!(state.as_str(), "succeeded" | "failed" | "cancelled") {
        transaction.commit().map_err(failure)?;
        return Ok(());
    }
    let phase = latest_phase(&transaction, operation_id)?;
    let terminal = if matches!(
        phase,
        BackupRestorePhase::PublishIntent
            | BackupRestorePhase::TargetPublished
            | BackupRestorePhase::ProjectRegistryCommitIntent
            | BackupRestorePhase::StateCommitted
    ) && error_code != "backup_target_exists"
    {
        "recovering"
    } else {
        "failed"
    };
    let next = revision.checked_add(1).ok_or_else(internal)?;
    transaction
        .execute(
            "UPDATE operations SET state=?1,revision=?2,error_code=?3,diagnostic_id=?4,
                    updated_at_ms=?5,completed_at_ms=CASE WHEN ?1='failed' THEN ?5 ELSE NULL END
             WHERE operation_id=?6",
            params![
                terminal,
                integer(next)?,
                error_code,
                diagnostic_id,
                integer(completed_at_ms)?,
                operation_id.to_string()
            ],
        )
        .map_err(failure)?;
    insert_event(
        &transaction,
        &owner,
        operation_id,
        revision_value(next)?,
        if terminal == "failed" {
            "operation.completed"
        } else {
            "operation.state_changed"
        },
        completed_at_ms,
    )?;
    transaction.commit().map_err(failure)
}

pub(super) fn recover(
    connection: &mut Connection,
    recovered_at_ms: u64,
) -> Result<Vec<OperationId>, M5BackupError> {
    let transaction = transaction(connection)?;
    let candidates = {
        let mut statement = transaction
            .prepare(
                "SELECT operation_id,state,revision,owner_principal_id FROM operations
             WHERE kind='backups.restore' AND state NOT IN ('succeeded','failed','cancelled')
             ORDER BY created_at_ms,operation_id",
            )
            .map_err(failure)?;
        statement
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
            .map_err(failure)?
    };
    let mut result = Vec::new();
    for (id, state, revision, owner) in candidates {
        let id = OperationId::parse(&id).map_err(|_| internal())?;
        if state == "queued" || state == "recovering" {
            result.push(id);
            continue;
        }
        let next = revision.checked_add(1).ok_or_else(internal)?;
        transaction.execute(
            "UPDATE operations SET state='recovering',revision=?1,updated_at_ms=?2 WHERE operation_id=?3",
            params![next, integer(recovered_at_ms)?, id.to_string()],
        ).map_err(failure)?;
        insert_event(
            &transaction,
            &PrincipalId::parse(owner).map_err(|_| internal())?,
            id,
            revision_value(u64::try_from(next).map_err(|_| internal())?)?,
            "operation.state_changed",
            recovered_at_ms,
        )?;
        result.push(id);
    }
    transaction.commit().map_err(failure)?;
    Ok(result)
}

/// A restore can publish its directory before the registration transaction commits.
pub(super) fn target_reserved(
    connection: &Connection,
    root: &std::path::Path,
) -> Result<bool, M5BackupError> {
    let mut statement = connection
        .prepare(
            "SELECT json_extract(j.payload_json,'$.request.targetParent'),
                json_extract(j.payload_json,'$.request.targetName')
         FROM operations o JOIN operation_journal j
           ON j.operation_id=o.operation_id AND j.kind='backups.restore' AND j.step=1
         WHERE o.kind='backups.restore' AND o.state NOT IN ('succeeded','failed','cancelled')",
        )
        .map_err(failure)?;
    let candidates = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(failure)?;
    for candidate in candidates {
        let (parent, name) = candidate.map_err(failure)?;
        if std::path::Path::new(&parent).join(name) == root {
            return Ok(true);
        }
    }
    Ok(false)
}

fn load_request(
    connection: &Connection,
    operation_id: OperationId,
) -> Result<BackupRestoreRequest, M5BackupError> {
    let value = connection
        .query_row(
            "SELECT payload_json FROM operation_journal
         WHERE operation_id=?1 AND step=1 AND kind='backups.restore'",
            [operation_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .map_err(failure)?;
    let mut payload: serde_json::Value = serde_json::from_str(&value).map_err(|_| internal())?;
    serde_json::from_value(payload["request"].take()).map_err(|_| internal())
}

fn operation_context(
    connection: &Connection,
    operation_id: OperationId,
) -> Result<(PrincipalId, String, u64), M5BackupError> {
    let (owner, state, revision) = connection
        .query_row(
            "SELECT owner_principal_id,state,revision FROM operations
         WHERE operation_id=?1 AND kind='backups.restore'",
            [operation_id.to_string()],
            |row| Ok((row.get::<_, String>(0)?, row.get(1)?, row.get::<_, i64>(2)?)),
        )
        .map_err(failure)?;
    Ok((
        PrincipalId::parse(owner).map_err(|_| internal())?,
        state,
        u64::try_from(revision).map_err(|_| internal())?,
    ))
}

fn insert_journal(
    connection: &Connection,
    operation_id: OperationId,
    step: u64,
    request: Option<&BackupRestoreRequest>,
    phase: BackupRestorePhase,
    restored: Option<&RestoredProject>,
    updated_at_ms: u64,
) -> Result<(), M5BackupError> {
    let payload = serde_json::json!({
        "phase": phase,
        "request": request,
        "restoredFingerprint": restored.map(|value| value.project_fingerprint),
    })
    .to_string();
    connection.execute(
        "INSERT INTO operation_journal (operation_id,step,kind,state,payload_json,updated_at_ms)
         VALUES (?1,?2,'backups.restore','prepared',?3,?4)",
        params![
            operation_id.to_string(),
            integer(step)?,
            payload,
            integer(updated_at_ms)?,
        ],
    ).map_err(failure)?;
    Ok(())
}

fn latest_step(connection: &Connection, operation_id: OperationId) -> Result<u64, M5BackupError> {
    connection
        .query_row(
            "SELECT coalesce(max(step),0) FROM operation_journal
         WHERE operation_id=?1 AND kind='backups.restore'",
            [operation_id.to_string()],
            |row| row.get::<_, i64>(0),
        )
        .map_err(failure)
        .and_then(|value| u64::try_from(value).map_err(|_| internal()))
}

fn latest_phase(
    connection: &Connection,
    operation_id: OperationId,
) -> Result<BackupRestorePhase, M5BackupError> {
    let value = connection
        .query_row(
            "SELECT payload_json FROM operation_journal
         WHERE operation_id=?1 AND kind='backups.restore' ORDER BY step DESC LIMIT 1",
            [operation_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .map_err(failure)?;
    let mut payload: serde_json::Value = serde_json::from_str(&value).map_err(|_| internal())?;
    serde_json::from_value(payload["phase"].take()).map_err(|_| internal())
}

const fn phase_step(phase: BackupRestorePhase) -> u64 {
    match phase {
        BackupRestorePhase::Accepted => 1,
        BackupRestorePhase::ArchiveVerified => 2,
        BackupRestorePhase::Extracting => 3,
        BackupRestorePhase::StagingComplete => 4,
        BackupRestorePhase::PublishIntent => 5,
        BackupRestorePhase::TargetPublished => 6,
        BackupRestorePhase::ProjectRegistryCommitIntent => 7,
        BackupRestorePhase::StateCommitted => 8,
    }
}

fn insert_event(
    connection: &Connection,
    owner: &PrincipalId,
    operation_id: OperationId,
    revision: Revision,
    kind: &str,
    now_ms: u64,
) -> Result<(), M5BackupError> {
    connection
        .execute(
            "INSERT INTO events (event_id,kind,aggregate_kind,aggregate_id,aggregate_revision,
            principal_id,occurred_at_ms,payload_json) VALUES (?1,?2,'operation',?3,?4,?5,?6,'{}')",
            params![
                Uuid::new_v4().to_string(),
                kind,
                operation_id.to_string(),
                integer(revision.get())?,
                owner.as_str(),
                integer(now_ms)?
            ],
        )
        .map_err(failure)?;
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

fn transaction(connection: &mut Connection) -> Result<Transaction<'_>, M5BackupError> {
    connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(failure)
}

fn integer(value: u64) -> Result<i64, M5BackupError> {
    i64::try_from(value).map_err(|_| internal())
}
fn revision_value(value: u64) -> Result<Revision, M5BackupError> {
    Revision::new(value).ok_or_else(internal)
}
fn failure(source: rusqlite::Error) -> M5BackupError {
    error(M5BackupErrorCode::StoreUnavailable).with_source(source)
}
pub(super) fn unavailable() -> M5BackupError {
    error(M5BackupErrorCode::StoreUnavailable)
}
fn internal() -> M5BackupError {
    error(M5BackupErrorCode::Internal)
}
fn error(code: M5BackupErrorCode) -> M5BackupError {
    M5BackupError::new(code)
}
