use alcomd_application::{
    BackupRestoreApplyOutcome, BackupRestoreOperationRecord, BackupRestorePhase,
    BackupRestorePlanDraft, BackupRestorePlanRecord, IdempotencyKey, M5BackupError,
    M5BackupErrorCode, OperationId, PlanId, PrincipalId, RestoredProject, Revision,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};
use uuid::Uuid;

pub(super) fn create_plan(
    connection: &mut Connection,
    owner: &PrincipalId,
    draft: BackupRestorePlanDraft,
    created_at_ms: u64,
) -> Result<BackupRestorePlanRecord, M5BackupError> {
    connection
        .execute(
            "INSERT INTO backup_restore_plans (
                plan_id,owner_principal_id,state,backup_id,preallocated_project_id,
                backup_archive_sha256,backup_file_identity,backup_byte_size,
                backup_format_version,backup_manifest_fingerprint,exclude_vpm_packages,
                excluded_packages_json,target_parent_path,target_parent_identity,target_leaf,
                target_must_be_absent,expected_unity_project_json,plan_fingerprint,plan_json,
                created_at_ms
             ) VALUES (?1,?2,'unapplied',?3,?4,?5,?6,?7,1,?8,?9,?10,?11,?12,?13,1,?14,?15,?16,?17)",
            params![
                draft.plan_id.to_string(),
                owner.as_str(),
                draft.backup_id.to_string(),
                draft.project_id.to_string(),
                draft.archive_sha256.as_slice(),
                draft.archive_file_identity,
                integer(draft.archive_bytes)?,
                draft.manifest_fingerprint.as_slice(),
                i64::from(draft.exclude_vpm_packages),
                serde_json::to_string(&draft.excluded_packages).map_err(|_| internal())?,
                draft.target.parent,
                draft.target_parent_identity,
                draft.target.leaf,
                draft.expected_unity_project_json,
                draft.plan_fingerprint.as_slice(),
                draft.plan_json,
                integer(created_at_ms)?,
            ],
        )
        .map_err(failure)?;
    load_plan(connection, owner, draft.plan_id)
}

pub(super) fn accept(
    connection: &mut Connection,
    owner: &PrincipalId,
    plan_id: PlanId,
    key: &IdempotencyKey,
    created_at_ms: u64,
) -> Result<BackupRestoreApplyOutcome, M5BackupError> {
    let fingerprint = format!(r#"{{"planId":"{plan_id}","version":1}}"#);
    let transaction = transaction(connection)?;
    if let Some((saved, response)) = transaction
        .query_row(
            "SELECT request_fingerprint,response_json FROM idempotency_records
             WHERE principal_id=?1 AND method='backups.applyRestore' AND idempotency_key=?2",
            params![owner.as_str(), key.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(failure)?
    {
        if saved != fingerprint {
            return Err(error(M5BackupErrorCode::IdempotencyConflict));
        }
        let mut outcome: BackupRestoreApplyOutcome =
            serde_json::from_str(&response).map_err(|_| internal())?;
        outcome.replayed = true;
        outcome.schedule = false;
        transaction.commit().map_err(failure)?;
        return Ok(outcome);
    }
    let plan = load_plan(&transaction, owner, plan_id)?;
    let already_applied = transaction
        .query_row(
            "SELECT apply_operation_id FROM backup_restore_plans WHERE plan_id=?1",
            [plan_id.to_string()],
            |row| row.get::<_, Option<String>>(0),
        )
        .map_err(failure)?;
    if already_applied.is_some() {
        return Err(error(M5BackupErrorCode::BackupRestorePlanStale));
    }
    let operation_id = OperationId::new();
    let request_json = format!(r#"{{"planId":"{plan_id}","version":1}}"#);
    transaction
        .execute(
            "INSERT INTO operations (
                operation_id,kind,state,revision,owner_principal_id,request_json,
                created_at_ms,updated_at_ms
             ) VALUES (?1,'backups.restore','queued',1,?2,?3,?4,?4)",
            params![
                operation_id.to_string(),
                owner.as_str(),
                request_json,
                integer(created_at_ms)?,
            ],
        )
        .map_err(failure)?;
    transaction
        .execute(
            "UPDATE backup_restore_plans SET state='applied',apply_operation_id=?1
             WHERE plan_id=?2 AND state='unapplied' AND apply_operation_id IS NULL",
            params![operation_id.to_string(), plan_id.to_string()],
        )
        .map_err(failure)?;
    insert_journal(
        &transaction,
        operation_id,
        1,
        &plan,
        BackupRestorePhase::Accepted,
        None,
        created_at_ms,
    )?;
    let outcome = BackupRestoreApplyOutcome {
        operation_id,
        project_id: plan.draft.project_id,
        replayed: false,
        schedule: true,
    };
    transaction
        .execute(
            "INSERT INTO idempotency_records (
                principal_id,method,idempotency_key,request_fingerprint,state,
                operation_id,response_json,created_at_ms
             ) VALUES (?1,'backups.applyRestore',?2,?3,'completed',?4,?5,?6)",
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
    let (owner, state, revision, plan_id) = operation_context(&transaction, operation_id)?;
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
    let plan = load_plan(&transaction, &owner, plan_id)?;
    let phase = latest_phase(&transaction, operation_id)?;
    transaction.commit().map_err(failure)?;
    Ok(BackupRestoreOperationRecord { plan, phase })
}

pub(super) fn checkpoint(
    connection: &mut Connection,
    operation_id: OperationId,
    phase: BackupRestorePhase,
    restored: Option<RestoredProject>,
    updated_at_ms: u64,
) -> Result<(), M5BackupError> {
    let (owner, state, _, plan_id) = operation_context(connection, operation_id)?;
    if !matches!(state.as_str(), "running" | "recovering" | "cancelling") {
        return Err(internal());
    }
    let plan = load_plan(connection, &owner, plan_id)?;
    let step = phase_step(phase);
    if latest_step(connection, operation_id)? >= step {
        return Ok(());
    }
    insert_journal(
        connection,
        operation_id,
        step,
        &plan,
        phase,
        restored.as_ref(),
        updated_at_ms,
    )
}

pub(super) fn complete(
    connection: &mut Connection,
    operation_id: OperationId,
    restored: RestoredProject,
    completed_at_ms: u64,
) -> Result<(), M5BackupError> {
    let transaction = transaction(connection)?;
    let (owner, state, _revision, plan_id) = operation_context(&transaction, operation_id)?;
    if !matches!(state.as_str(), "running" | "recovering" | "cancelling") {
        return Err(internal());
    }
    let plan = load_plan(&transaction, &owner, plan_id)?;
    if plan.draft.project_id != restored.project_id
        || plan.draft.target.parent.is_empty()
        || restored.observation.root_path
            != std::path::Path::new(&plan.draft.target.parent)
                .join(&plan.draft.target.leaf)
                .to_string_lossy()
    {
        return Err(error(M5BackupErrorCode::BackupRestorePlanStale));
    }
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
            let semantic = super::m3::semantic_project(restored.observation.clone());
            transaction
                .execute(
                    "INSERT INTO projects (
                        project_id,owner_principal_id,root_path,path_identity_key,project_type,
                        unity_version,unity_revision,snapshot_json,revision,registered_at_ms,
                        observed_at_ms,updated_at_ms
                     ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,1,?9,?10,?9)",
                    params![
                        restored.project_id.to_string(),
                        owner.as_str(),
                        restored.observation.root_path,
                        restored.observation.path_identity_key,
                        super::m3::project_type(restored.observation.project_type),
                        restored.observation.unity_version,
                        restored.observation.unity_revision,
                        serde_json::to_string(&semantic).map_err(|_| internal())?,
                        integer(completed_at_ms)?,
                        integer(restored.observation.observed_at_ms)?,
                    ],
                )
                .map_err(failure)?;
            transaction
                .execute(
                    "INSERT INTO events (event_id,kind,aggregate_kind,aggregate_id,aggregate_revision,
                        principal_id,occurred_at_ms,payload_json)
                     VALUES (?1,'project.registered','project',?2,1,?3,?4,'{}')",
                    params![Uuid::new_v4().to_string(), restored.project_id.to_string(), owner.as_str(), integer(completed_at_ms)?],
                )
                .map_err(failure)?;
        }
    }
    insert_journal(
        &transaction,
        operation_id,
        phase_step(BackupRestorePhase::StateCommitted),
        &plan,
        BackupRestorePhase::StateCommitted,
        Some(&restored),
        completed_at_ms,
    )?;
    transaction.commit().map_err(failure)
}

pub(super) fn finish_success(
    connection: &mut Connection,
    operation_id: OperationId,
    completed_at_ms: u64,
) -> Result<(), M5BackupError> {
    let transaction = transaction(connection)?;
    let (owner, state, revision, plan_id) = operation_context(&transaction, operation_id)?;
    if state == "succeeded" {
        transaction.commit().map_err(failure)?;
        return Ok(());
    }
    if !matches!(state.as_str(), "running" | "recovering" | "cancelling")
        || latest_phase(&transaction, operation_id)? != BackupRestorePhase::StateCommitted
    {
        return Err(internal());
    }
    let plan = load_plan(&transaction, &owner, plan_id)?;
    let next = revision.checked_add(1).ok_or_else(internal)?;
    let result = serde_json::json!({
        "backupId": plan.draft.backup_id,
        "projectId": plan.draft.project_id,
        "target": plan.draft.target,
        "packagesRequireResolve": plan.draft.exclude_vpm_packages,
        "excludedPackages": plan.draft.excluded_packages,
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
    insert_event(
        &transaction,
        &owner,
        operation_id,
        revision_value(next)?,
        "operation.completed",
        completed_at_ms,
    )?;
    transaction.commit().map_err(failure)
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
    let (owner, state, revision, _) = operation_context(&transaction, operation_id)?;
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
    ) {
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
    let candidates = {
        let mut statement = connection
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
        connection.execute(
            "UPDATE operations SET state='recovering',revision=?1,updated_at_ms=?2 WHERE operation_id=?3",
            params![next, integer(recovered_at_ms)?, id.to_string()],
        ).map_err(failure)?;
        insert_event(
            connection,
            &PrincipalId::parse(owner).map_err(|_| internal())?,
            id,
            revision_value(u64::try_from(next).map_err(|_| internal())?)?,
            "operation.state_changed",
            recovered_at_ms,
        )?;
        result.push(id);
    }
    Ok(result)
}

pub(super) fn completed(
    connection: &Connection,
) -> Result<Vec<(OperationId, BackupRestorePlanRecord)>, M5BackupError> {
    let rows = {
        let mut statement = connection
            .prepare(
                "SELECT o.operation_id,o.owner_principal_id,p.plan_id
                 FROM operations o JOIN backup_restore_plans p
                   ON p.apply_operation_id=o.operation_id
                 WHERE o.kind='backups.restore' AND o.state='succeeded'
                   AND EXISTS (SELECT 1 FROM backup_restore_filesystem_journal j
                     WHERE j.operation_id=o.operation_id AND j.phase='state_committed')",
            )
            .map_err(failure)?;
        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(failure)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(failure)?
    };
    rows.into_iter()
        .map(|(operation, owner, plan)| {
            let operation = OperationId::parse(&operation).map_err(|_| internal())?;
            let owner = PrincipalId::parse(owner).map_err(|_| internal())?;
            let plan = PlanId::parse(&plan).map_err(|_| internal())?;
            Ok((operation, load_plan(connection, &owner, plan)?))
        })
        .collect()
}

fn load_plan(
    connection: &Connection,
    owner: &PrincipalId,
    plan_id: PlanId,
) -> Result<BackupRestorePlanRecord, M5BackupError> {
    connection
        .query_row(
            "SELECT plan_id,owner_principal_id,backup_id,preallocated_project_id,
                backup_archive_sha256,backup_file_identity,backup_byte_size,
                backup_manifest_fingerprint,exclude_vpm_packages,excluded_packages_json,
                target_parent_path,target_parent_identity,target_leaf,expected_unity_project_json,
                plan_fingerprint,plan_json,created_at_ms
             FROM backup_restore_plans WHERE plan_id=?1 AND owner_principal_id=?2",
            params![plan_id.to_string(), owner.as_str()],
            plan_from_row,
        )
        .optional()
        .map_err(failure)?
        .ok_or_else(|| error(M5BackupErrorCode::BackupRestorePlanNotFound))
}

fn plan_from_row(row: &Row<'_>) -> rusqlite::Result<BackupRestorePlanRecord> {
    let digest = |index| -> rusqlite::Result<[u8; 32]> {
        row.get::<_, Vec<u8>>(index)?
            .try_into()
            .map_err(|_| rusqlite::Error::InvalidQuery)
    };
    let owner =
        PrincipalId::parse(row.get::<_, String>(1)?).map_err(|_| rusqlite::Error::InvalidQuery)?;
    Ok(BackupRestorePlanRecord {
        draft: BackupRestorePlanDraft {
            plan_id: PlanId::parse(&row.get::<_, String>(0)?)
                .map_err(|_| rusqlite::Error::InvalidQuery)?,
            backup_id: alcomd_application::BackupId::parse(&row.get::<_, String>(2)?)
                .map_err(|_| rusqlite::Error::InvalidQuery)?,
            project_id: alcomd_application::ProjectId::parse(&row.get::<_, String>(3)?)
                .map_err(|_| rusqlite::Error::InvalidQuery)?,
            archive_sha256: digest(4)?,
            archive_file_identity: row.get(5)?,
            archive_bytes: u64::try_from(row.get::<_, i64>(6)?)
                .map_err(|_| rusqlite::Error::InvalidQuery)?,
            manifest_fingerprint: digest(7)?,
            exclude_vpm_packages: row.get::<_, i64>(8)? == 1,
            excluded_packages: serde_json::from_str(&row.get::<_, String>(9)?)
                .map_err(|_| rusqlite::Error::InvalidQuery)?,
            target: alcomd_application::BackupRestoreTarget {
                parent: row.get(10)?,
                leaf: row.get(12)?,
                must_be_absent: true,
            },
            target_parent_identity: row.get(11)?,
            expected_unity_project_json: row.get(13)?,
            plan_fingerprint: digest(14)?,
            plan_json: row.get(15)?,
        },
        owner,
        created_at_ms: u64::try_from(row.get::<_, i64>(16)?)
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
    })
}

fn operation_context(
    connection: &Connection,
    operation_id: OperationId,
) -> Result<(PrincipalId, String, u64, PlanId), M5BackupError> {
    connection
        .query_row(
            "SELECT o.owner_principal_id,o.state,o.revision,p.plan_id FROM operations o
         JOIN backup_restore_plans p ON p.apply_operation_id=o.operation_id
         WHERE o.operation_id=?1 AND o.kind='backups.restore'",
            [operation_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .map_err(failure)
        .and_then(|(owner, state, revision, plan)| {
            Ok((
                PrincipalId::parse(owner).map_err(|_| internal())?,
                state,
                u64::try_from(revision).map_err(|_| internal())?,
                PlanId::parse(&plan).map_err(|_| internal())?,
            ))
        })
}

fn insert_journal(
    connection: &Connection,
    operation_id: OperationId,
    step: u64,
    plan: &BackupRestorePlanRecord,
    phase: BackupRestorePhase,
    restored: Option<&RestoredProject>,
    updated_at_ms: u64,
) -> Result<(), M5BackupError> {
    let evidence = restored.map_or_else(
        || "{}".to_owned(),
        |value| {
            serde_json::json!({
                "targetIdentity": hex(&value.target_identity),
                "projectFingerprint": hex(&value.project_fingerprint),
            })
            .to_string()
        },
    );
    let state = if matches!(
        phase,
        BackupRestorePhase::PublishIntent | BackupRestorePhase::ProjectRegistryCommitIntent
    ) {
        "intent"
    } else {
        "completed"
    };
    connection
        .execute(
            "INSERT INTO backup_restore_filesystem_journal (
            operation_id,step,plan_id,preallocated_project_id,phase,state,
            target_parent_identity,target_identity,project_fingerprint,evidence_json,updated_at_ms
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                operation_id.to_string(),
                integer(step)?,
                plan.draft.plan_id.to_string(),
                plan.draft.project_id.to_string(),
                phase.as_str(),
                state,
                plan.draft.target_parent_identity,
                restored.map(|value| value.target_identity.clone()),
                restored.map(|value| value.project_fingerprint.to_vec()),
                evidence,
                integer(updated_at_ms)?,
            ],
        )
        .map_err(failure)?;
    Ok(())
}

fn latest_step(connection: &Connection, operation_id: OperationId) -> Result<u64, M5BackupError> {
    connection.query_row(
        "SELECT coalesce(max(step),0) FROM backup_restore_filesystem_journal WHERE operation_id=?1",
        [operation_id.to_string()], |row| row.get::<_, i64>(0),
    ).map_err(failure).and_then(|value| u64::try_from(value).map_err(|_| internal()))
}

fn latest_phase(
    connection: &Connection,
    operation_id: OperationId,
) -> Result<BackupRestorePhase, M5BackupError> {
    let value = connection.query_row(
        "SELECT phase FROM backup_restore_filesystem_journal WHERE operation_id=?1 ORDER BY step DESC LIMIT 1",
        [operation_id.to_string()], |row| row.get::<_, String>(0),
    ).map_err(failure)?;
    match value.as_str() {
        "accepted" => Ok(BackupRestorePhase::Accepted),
        "archive_verified" => Ok(BackupRestorePhase::ArchiveVerified),
        "extracting" => Ok(BackupRestorePhase::Extracting),
        "staging_complete" => Ok(BackupRestorePhase::StagingComplete),
        "publish_intent" => Ok(BackupRestorePhase::PublishIntent),
        "target_published" => Ok(BackupRestorePhase::TargetPublished),
        "project_registry_commit_intent" => Ok(BackupRestorePhase::ProjectRegistryCommitIntent),
        "state_committed" => Ok(BackupRestorePhase::StateCommitted),
        _ => Err(internal()),
    }
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
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn failure(_: rusqlite::Error) -> M5BackupError {
    error(M5BackupErrorCode::StoreUnavailable)
}
pub(super) fn unavailable() -> M5BackupError {
    error(M5BackupErrorCode::StoreUnavailable)
}
const fn internal() -> M5BackupError {
    error(M5BackupErrorCode::Internal)
}
const fn error(code: M5BackupErrorCode) -> M5BackupError {
    M5BackupError::new(code)
}
