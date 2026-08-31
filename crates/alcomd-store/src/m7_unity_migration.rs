use alcomd_application::{
    IdempotencyKey, M7UnityMigrationError, M7UnityMigrationErrorCode, OperationId, PlanId,
    PrincipalId, ProjectObservation, ProjectRecord, Revision, UnityInstallationRecord,
    UnityMigrationApplyOutcome, UnityMigrationClassificationKind, UnityMigrationEvidence,
    UnityMigrationOperationRecord, UnityMigrationPhase, UnityMigrationPlanDraft,
    UnityMigrationPlanOutcome, UnityMigrationPlanRecord, UnityWriterState,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};
use uuid::Uuid;

pub(super) fn create_plan(
    connection: &mut Connection,
    owner: &PrincipalId,
    draft: UnityMigrationPlanDraft,
) -> Result<UnityMigrationPlanOutcome, M7UnityMigrationError> {
    let transaction = begin(connection)?;
    if let Some((stored, response)) = transaction
        .query_row(
            "SELECT request_fingerprint,response_json FROM idempotency_records
         WHERE principal_id=?1 AND method='projects.planUnityMigration' AND idempotency_key=?2",
            params![owner.as_str(), draft.plan_idempotency_key.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(failure)?
    {
        if stored != draft.request_fingerprint {
            return Err(error(M7UnityMigrationErrorCode::IdempotencyConflict));
        }
        let outcome = serde_json::from_str(&response).map_err(|_| internal())?;
        transaction.commit().map_err(failure)?;
        return Ok(mark_replayed(outcome));
    }
    transaction.execute(
        "INSERT INTO project_unity_migration_plans (
            plan_id,owner_principal_id,project_id,project_revision,source_unity_version,
            source_revision_metadata,project_root_identity,project_snapshot_json,
            project_version_marker_sha256,
            target_unity_version,target_revision_metadata,target_installation_id,
            target_installation_revision,target_installation_identity,
            target_installation_snapshot_json,writer_evidence_revision,writer_evidence_json,
            classification,preparation_profile,plan_fingerprint,request_fingerprint,
            plan_idempotency_key,created_at_ms,expires_at_ms,state,operation_id
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,'unapplied',NULL)",
        params![
            draft.plan_id.to_string(), owner.as_str(), draft.project.project_id.to_string(),
            integer(draft.project.revision.get())?, draft.source_unity_version,
            draft.source_revision_metadata, draft.project_root_identity,
            json(&draft.project)?, draft.project_version_marker_sha256.as_slice(),
            draft.target_unity_version,
            draft.target_revision_metadata, draft.target_installation.installation_id.to_string(),
            integer(draft.target_installation.revision.get())?,
            draft.target_installation.observation.filesystem_identity,
            json(&draft.target_installation)?, integer(draft.writer_evidence.checked_at_ms.max(1))?,
            json(&draft.writer_evidence)?, draft.classification.as_str(),
            draft.preparation_profile, draft.plan_fingerprint, draft.request_fingerprint,
            draft.plan_idempotency_key.as_str(), integer(draft.created_at_ms)?,
            integer(draft.expires_at_ms)?,
        ],
    ).map_err(failure)?;
    let plan = load_plan(&transaction, owner, draft.plan_id)?;
    let outcome = UnityMigrationPlanOutcome::Planned {
        plan: Box::new(plan),
        replayed: false,
    };
    transaction.execute(
        "INSERT INTO idempotency_records (
            principal_id,method,idempotency_key,request_fingerprint,state,operation_id,response_json,created_at_ms
         ) VALUES (?1,'projects.planUnityMigration',?2,?3,'completed',NULL,?4,?5)",
        params![owner.as_str(), draft.plan_idempotency_key.as_str(), draft.request_fingerprint,
            json(&outcome)?, integer(draft.created_at_ms)?],
    ).map_err(failure)?;
    transaction.commit().map_err(failure)?;
    Ok(outcome)
}

pub(super) fn get_plan(
    connection: &Connection,
    owner: &PrincipalId,
    plan_id: PlanId,
) -> Result<UnityMigrationPlanRecord, M7UnityMigrationError> {
    load_plan(connection, owner, plan_id)
}

pub(super) fn replay_apply(
    connection: &Connection,
    owner: &PrincipalId,
    plan_id: PlanId,
    key: &IdempotencyKey,
) -> Result<Option<UnityMigrationApplyOutcome>, M7UnityMigrationError> {
    let fingerprint = apply_fingerprint(plan_id)?;
    let value = connection
        .query_row(
            "SELECT request_fingerprint,response_json FROM idempotency_records
         WHERE principal_id=?1 AND method='projects.applyUnityMigration' AND idempotency_key=?2",
            params![owner.as_str(), key.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(failure)?;
    let Some((stored, response)) = value else {
        return Ok(None);
    };
    if stored != fingerprint {
        return Err(error(M7UnityMigrationErrorCode::IdempotencyConflict));
    }
    let mut outcome: UnityMigrationApplyOutcome =
        serde_json::from_str(&response).map_err(|_| internal())?;
    outcome.replayed = true;
    outcome.schedule = false;
    Ok(Some(outcome))
}

pub(super) fn accept(
    connection: &mut Connection,
    owner: &PrincipalId,
    plan_id: PlanId,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<UnityMigrationApplyOutcome, M7UnityMigrationError> {
    if let Some(outcome) = replay_apply(connection, owner, plan_id, key)? {
        return Ok(outcome);
    }
    let transaction = begin(connection)?;
    let plan = load_plan(&transaction, owner, plan_id)?;
    if now_ms >= plan.draft.expires_at_ms {
        return Err(error(M7UnityMigrationErrorCode::PlanStale));
    }
    let operation_id = OperationId::new();
    let request = apply_fingerprint(plan_id)?;
    transaction.execute(
        "INSERT INTO operations (operation_id,kind,state,revision,owner_principal_id,request_json,created_at_ms,updated_at_ms)
         VALUES (?1,'projects.unity-migration','queued',1,?2,?3,?4,?4)",
        params![operation_id.to_string(), owner.as_str(), request, integer(now_ms)?],
    ).map_err(failure)?;
    if transaction
        .execute(
            "UPDATE project_unity_migration_plans SET state='applied',operation_id=?1
         WHERE plan_id=?2 AND state='unapplied' AND operation_id IS NULL",
            params![operation_id.to_string(), plan_id.to_string()],
        )
        .map_err(failure)?
        != 1
    {
        return Err(error(M7UnityMigrationErrorCode::PlanStale));
    }
    let evidence = UnityMigrationEvidence {
        preparation_kind: plan
            .draft
            .preparation_profile
            .clone()
            .unwrap_or_else(|| "none".to_owned()),
        ..UnityMigrationEvidence::default()
    };
    insert_journal(
        &transaction,
        operation_id,
        1,
        UnityMigrationPhase::Accepted,
        &evidence,
        now_ms,
    )?;
    let outcome = UnityMigrationApplyOutcome {
        operation_id,
        replayed: false,
        schedule: true,
    };
    transaction.execute(
        "INSERT INTO idempotency_records (
            principal_id,method,idempotency_key,request_fingerprint,state,operation_id,response_json,created_at_ms
         ) VALUES (?1,'projects.applyUnityMigration',?2,?3,'completed',?4,?5,?6)",
        params![owner.as_str(), key.as_str(), request, operation_id.to_string(), json(&outcome)?, integer(now_ms)?],
    ).map_err(failure)?;
    operation_event(
        &transaction,
        owner,
        operation_id,
        Revision::INITIAL,
        "operation.created",
        now_ms,
    )?;
    transaction.commit().map_err(failure)?;
    Ok(outcome)
}

pub(super) fn begin_operation(
    connection: &mut Connection,
    operation_id: OperationId,
    now_ms: u64,
) -> Result<UnityMigrationOperationRecord, M7UnityMigrationError> {
    let transaction = begin(connection)?;
    let (owner, state, current, plan_id) = operation_context(&transaction, operation_id)?;
    if !matches!(state.as_str(), "queued" | "running" | "recovering") {
        return Err(internal());
    }
    let next = current.checked_add(1).ok_or_else(internal)?;
    transaction.execute(
        "UPDATE operations SET state='running',revision=?1,updated_at_ms=?2,
            started_at_ms=coalesce(started_at_ms,?2),error_code=NULL,diagnostic_id=NULL WHERE operation_id=?3",
        params![integer(next)?, integer(now_ms)?, operation_id.to_string()],
    ).map_err(failure)?;
    operation_event(
        &transaction,
        &owner,
        operation_id,
        revision(next)?,
        "operation.progress",
        now_ms,
    )?;
    let plan = load_plan(&transaction, &owner, plan_id)?;
    let (phase, evidence) = latest(&transaction, operation_id)?;
    transaction.commit().map_err(failure)?;
    Ok(UnityMigrationOperationRecord {
        plan,
        phase,
        evidence,
    })
}

pub(super) fn checkpoint(
    connection: &mut Connection,
    operation_id: OperationId,
    phase: UnityMigrationPhase,
    evidence: UnityMigrationEvidence,
    now_ms: u64,
) -> Result<(), M7UnityMigrationError> {
    let transaction = begin(connection)?;
    let (owner, state, current, _) = operation_context(&transaction, operation_id)?;
    if !matches!(state.as_str(), "running" | "cancelling" | "recovering") {
        return Err(internal());
    }
    insert_journal(
        &transaction,
        operation_id,
        latest_step(&transaction, operation_id)? + 1,
        phase,
        &evidence,
        now_ms,
    )?;
    let next = current.checked_add(1).ok_or_else(internal)?;
    transaction
        .execute(
            "UPDATE operations SET revision=?1,updated_at_ms=?2 WHERE operation_id=?3",
            params![integer(next)?, integer(now_ms)?, operation_id.to_string()],
        )
        .map_err(failure)?;
    operation_event(
        &transaction,
        &owner,
        operation_id,
        revision(next)?,
        "operation.progress",
        now_ms,
    )?;
    transaction.commit().map_err(failure)
}

pub(super) fn commit_project(
    connection: &mut Connection,
    operation_id: OperationId,
    observation: ProjectObservation,
    now_ms: u64,
) -> Result<(), M7UnityMigrationError> {
    let transaction = begin(connection)?;
    let (owner, state, operation_revision, plan_id) =
        operation_context(&transaction, operation_id)?;
    if state != "running" {
        return Err(internal());
    }
    let plan = load_plan(&transaction, &owner, plan_id)?;
    let (phase, evidence) = latest(&transaction, operation_id)?;
    if phase != UnityMigrationPhase::ProjectReobserved
        || evidence.reobserved_version.as_deref() != Some(plan.draft.target_unity_version.as_str())
        || evidence.reobserved_root_identity.as_deref()
            != Some(plan.draft.project_root_identity.as_slice())
        || evidence.reobserved_marker_sha256.is_none()
        || evidence.writer_inactive_checked_at_ms.is_none()
        || evidence.reobserved_at_ms.is_none()
    {
        return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
    }
    if observation.path_identity_key != plan.draft.project_root_identity
        || observation.unity_version != plan.draft.target_unity_version
    {
        return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
    }
    let next_project = plan
        .draft
        .project
        .revision
        .checked_next()
        .ok_or_else(internal)?;
    if transaction
        .execute(
            "UPDATE projects SET snapshot_json=?1,revision=?2,observed_at_ms=?3,updated_at_ms=?3
         WHERE owner_principal_id=?4 AND project_id=?5 AND revision=?6",
            params![
                json(&observation)?,
                integer(next_project.get())?,
                integer(now_ms)?,
                owner.as_str(),
                plan.draft.project.project_id.to_string(),
                integer(plan.draft.project.revision.get())?
            ],
        )
        .map_err(failure)?
        != 1
    {
        return Err(error(M7UnityMigrationErrorCode::SourceChanged));
    }
    transaction.execute(
        "INSERT INTO events (event_id,kind,aggregate_kind,aggregate_id,aggregate_revision,principal_id,occurred_at_ms,payload_json)
         VALUES (?1,'project.unity_version_migrated','project',?2,?3,?4,?5,?6)",
        params![Uuid::new_v4().to_string(), plan.draft.project.project_id.to_string(), integer(next_project.get())?,
            owner.as_str(), integer(now_ms)?, json(&serde_json::json!({
                "fromUnityVersion": plan.draft.source_unity_version,
                "toUnityVersion": plan.draft.target_unity_version,
                "classification": plan.draft.classification.as_str(),
                "operationId": operation_id,
                "revision": next_project.get()
            }))?],
    ).map_err(failure)?;
    insert_journal(
        &transaction,
        operation_id,
        latest_step(&transaction, operation_id)? + 1,
        UnityMigrationPhase::StateCommitted,
        &evidence,
        now_ms,
    )?;
    let next = operation_revision.checked_add(1).ok_or_else(internal)?;
    transaction
        .execute(
            "UPDATE operations SET revision=?1,updated_at_ms=?2 WHERE operation_id=?3",
            params![integer(next)?, integer(now_ms)?, operation_id.to_string()],
        )
        .map_err(failure)?;
    operation_event(
        &transaction,
        &owner,
        operation_id,
        revision(next)?,
        "operation.progress",
        now_ms,
    )?;
    transaction.commit().map_err(failure)
}

pub(super) fn finish_success(
    connection: &mut Connection,
    operation_id: OperationId,
    now_ms: u64,
) -> Result<(), M7UnityMigrationError> {
    if latest(connection, operation_id)?.0 != UnityMigrationPhase::CleanupComplete {
        return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
    }
    finish_terminal(
        connection,
        operation_id,
        now_ms,
        "succeeded",
        "operation.succeeded",
        None,
    )
}

pub(super) fn fail(
    connection: &mut Connection,
    operation_id: OperationId,
    code: &str,
    diagnostic_id: &str,
    now_ms: u64,
) -> Result<(), M7UnityMigrationError> {
    let (phase, evidence) = latest(connection, operation_id)?;
    if !matches!(
        phase,
        UnityMigrationPhase::Accepted | UnityMigrationPhase::PreflightComplete
    ) && !evidence.safe_terminal_failure
    {
        return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
    }
    finish_terminal(
        connection,
        operation_id,
        now_ms,
        "failed",
        "operation.failed",
        Some((code, diagnostic_id)),
    )
}

pub(super) fn recover(
    connection: &mut Connection,
    now_ms: u64,
) -> Result<Vec<OperationId>, M7UnityMigrationError> {
    let transaction = begin(connection)?;
    let values = {
        let mut statement = transaction.prepare(
            "SELECT operation_id FROM operations WHERE kind='projects.unity-migration'
             AND state IN ('queued','running','recovering','interrupted') ORDER BY created_at_ms,operation_id"
        ).map_err(failure)?;
        statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(failure)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(failure)?
    };
    let mut result = Vec::new();
    for value in values {
        let id = OperationId::parse(&value).map_err(|_| internal())?;
        transaction.execute("UPDATE operations SET state='recovering',updated_at_ms=?1 WHERE operation_id=?2 AND state!='queued'",
            params![integer(now_ms)?, value]).map_err(failure)?;
        result.push(id);
    }
    transaction.commit().map_err(failure)?;
    Ok(result)
}

pub(super) fn cancellation_requested(
    connection: &Connection,
    operation_id: OperationId,
) -> Result<bool, M7UnityMigrationError> {
    connection.query_row(
        "SELECT cancel_requested FROM operations WHERE operation_id=?1 AND kind='projects.unity-migration'",
        [operation_id.to_string()], |row| row.get::<_, i64>(0)
    ).optional().map_err(failure)?.map(|value| value == 1).ok_or_else(internal)
}

pub(super) fn finish_cancelled(
    connection: &mut Connection,
    operation_id: OperationId,
    now_ms: u64,
) -> Result<(), M7UnityMigrationError> {
    let (phase, evidence) = latest(connection, operation_id)?;
    let allowed = matches!(
        phase,
        UnityMigrationPhase::Accepted | UnityMigrationPhase::PreflightComplete
    ) || (evidence.preparation_kind == "none"
        && matches!(
            phase,
            UnityMigrationPhase::PreparationIntent | UnityMigrationPhase::PreparationComplete
        ));
    if !allowed {
        return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
    }
    finish_terminal(
        connection,
        operation_id,
        now_ms,
        "cancelled",
        "operation.cancelled",
        None,
    )
}

fn finish_terminal(
    connection: &mut Connection,
    operation_id: OperationId,
    now_ms: u64,
    state_target: &str,
    event: &str,
    failure_data: Option<(&str, &str)>,
) -> Result<(), M7UnityMigrationError> {
    let transaction = begin(connection)?;
    let (owner, state, current, _) = operation_context(&transaction, operation_id)?;
    if !matches!(
        state.as_str(),
        "queued" | "running" | "cancelling" | "recovering"
    ) {
        transaction.commit().map_err(failure)?;
        return Ok(());
    }
    let next = current.checked_add(1).ok_or_else(internal)?;
    let (error_code, diagnostic_id) = failure_data.map_or((None, None), |(code, diagnostic)| {
        (Some(code), Some(diagnostic))
    });
    transaction.execute(
        "UPDATE operations SET state=?1,revision=?2,updated_at_ms=?3,completed_at_ms=?3,error_code=?4,diagnostic_id=?5 WHERE operation_id=?6",
        params![state_target, integer(next)?, integer(now_ms)?, error_code, diagnostic_id, operation_id.to_string()]
    ).map_err(failure)?;
    operation_event(
        &transaction,
        &owner,
        operation_id,
        revision(next)?,
        event,
        now_ms,
    )?;
    transaction.commit().map_err(failure)
}

fn load_plan(
    connection: &Connection,
    owner: &PrincipalId,
    plan_id: PlanId,
) -> Result<UnityMigrationPlanRecord, M7UnityMigrationError> {
    connection
        .query_row(
            "SELECT project_snapshot_json,source_unity_version,source_revision_metadata,
                project_root_identity,project_version_marker_sha256,target_unity_version,
                target_revision_metadata,target_installation_snapshot_json,writer_evidence_json,
                classification,preparation_profile,plan_fingerprint,request_fingerprint,
                plan_idempotency_key,created_at_ms,expires_at_ms
         FROM project_unity_migration_plans WHERE plan_id=?1 AND owner_principal_id=?2",
            params![plan_id.to_string(), owner.as_str()],
            |row| plan_from_row(row, plan_id, owner.clone()),
        )
        .optional()
        .map_err(failure)?
        .ok_or_else(|| error(M7UnityMigrationErrorCode::PlanNotFound))
}

fn plan_from_row(
    row: &Row<'_>,
    plan_id: PlanId,
    owner: PrincipalId,
) -> rusqlite::Result<UnityMigrationPlanRecord> {
    let project = serde_json::from_str::<ProjectRecord>(&row.get::<_, String>(0)?)
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    let target_installation =
        serde_json::from_str::<UnityInstallationRecord>(&row.get::<_, String>(7)?)
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
    let writer_evidence = serde_json::from_str::<UnityWriterState>(&row.get::<_, String>(8)?)
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    let request_fingerprint: String = row.get(12)?;
    let key = IdempotencyKey::parse(row.get::<_, String>(13)?)
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    Ok(UnityMigrationPlanRecord {
        owner,
        draft: UnityMigrationPlanDraft {
            plan_id,
            project,
            source_unity_version: row.get(1)?,
            source_revision_metadata: row.get(2)?,
            project_root_identity: row.get(3)?,
            project_version_marker_sha256: array32(row.get(4)?)?,
            target_unity_version: row.get(5)?,
            target_revision_metadata: row.get(6)?,
            target_installation,
            writer_evidence,
            classification: parse_classification(&row.get::<_, String>(9)?)?,
            preparation_profile: row.get(10)?,
            plan_fingerprint: row.get(11)?,
            request_fingerprint,
            plan_idempotency_key: key,
            created_at_ms: unsigned_sql(row.get(14)?)?,
            expires_at_ms: unsigned_sql(row.get(15)?)?,
        },
    })
}

fn operation_context(
    connection: &Connection,
    operation_id: OperationId,
) -> Result<(PrincipalId, String, u64, PlanId), M7UnityMigrationError> {
    let row = connection
        .query_row(
            "SELECT o.owner_principal_id,o.state,o.revision,p.plan_id FROM operations o
         JOIN project_unity_migration_plans p ON p.operation_id=o.operation_id
         WHERE o.operation_id=?1 AND o.kind='projects.unity-migration'",
            [operation_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()
        .map_err(failure)?
        .ok_or_else(internal)?;
    Ok((
        PrincipalId::parse(&row.0).map_err(|_| internal())?,
        row.1,
        unsigned(row.2)?,
        PlanId::parse(&row.3).map_err(|_| internal())?,
    ))
}

fn insert_journal(
    transaction: &Transaction<'_>,
    operation_id: OperationId,
    step: u64,
    phase: UnityMigrationPhase,
    evidence: &UnityMigrationEvidence,
    now_ms: u64,
) -> Result<(), M7UnityMigrationError> {
    transaction.execute(
        "INSERT INTO project_unity_migration_journal (operation_id,step,phase,evidence_json,updated_at_ms) VALUES (?1,?2,?3,?4,?5)",
        params![operation_id.to_string(), integer(step)?, phase.as_str(), json(evidence)?, integer(now_ms)?]
    ).map_err(failure)?;
    Ok(())
}

fn latest(
    connection: &Connection,
    operation_id: OperationId,
) -> Result<(UnityMigrationPhase, UnityMigrationEvidence), M7UnityMigrationError> {
    let row: (String, String) = connection.query_row(
        "SELECT phase,evidence_json FROM project_unity_migration_journal WHERE operation_id=?1 ORDER BY step DESC LIMIT 1",
        [operation_id.to_string()], |row| Ok((row.get(0)?, row.get(1)?))).map_err(failure)?;
    Ok((
        parse_phase(&row.0)?,
        serde_json::from_str(&row.1).map_err(|_| internal())?,
    ))
}

fn latest_step(
    connection: &Connection,
    operation_id: OperationId,
) -> Result<u64, M7UnityMigrationError> {
    connection.query_row("SELECT coalesce(max(step),0) FROM project_unity_migration_journal WHERE operation_id=?1",
        [operation_id.to_string()], |row| row.get::<_, i64>(0)).map_err(failure).and_then(unsigned)
}

fn operation_event(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    operation_id: OperationId,
    revision_value: Revision,
    kind: &str,
    now_ms: u64,
) -> Result<(), M7UnityMigrationError> {
    transaction.execute(
        "INSERT INTO events (event_id,kind,aggregate_kind,aggregate_id,aggregate_revision,principal_id,occurred_at_ms,payload_json)
         VALUES (?1,?2,'operation',?3,?4,?5,?6,'{}')",
        params![Uuid::new_v4().to_string(), kind, operation_id.to_string(), integer(revision_value.get())?, owner.as_str(), integer(now_ms)?]
    ).map_err(failure)?;
    Ok(())
}

fn mark_replayed(mut value: UnityMigrationPlanOutcome) -> UnityMigrationPlanOutcome {
    if let UnityMigrationPlanOutcome::Planned { replayed, .. } = &mut value {
        *replayed = true;
    }
    value
}

fn apply_fingerprint(plan_id: PlanId) -> Result<String, M7UnityMigrationError> {
    json(&serde_json::json!({"planId": plan_id, "version": 1}))
}
fn parse_classification(value: &str) -> rusqlite::Result<UnityMigrationClassificationKind> {
    match value {
        "patch_or_minor_upgrade" => Ok(UnityMigrationClassificationKind::PatchOrMinorUpgrade),
        "major_upgrade" => Ok(UnityMigrationClassificationKind::MajorUpgrade),
        "patch_or_minor_downgrade" => Ok(UnityMigrationClassificationKind::PatchOrMinorDowngrade),
        "major_downgrade" => Ok(UnityMigrationClassificationKind::MajorDowngrade),
        "china_variant_change" => Ok(UnityMigrationClassificationKind::ChinaVariantChange),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}
fn parse_phase(value: &str) -> Result<UnityMigrationPhase, M7UnityMigrationError> {
    match value {
        "accepted" => Ok(UnityMigrationPhase::Accepted),
        "preflight_complete" => Ok(UnityMigrationPhase::PreflightComplete),
        "preparation_intent" => Ok(UnityMigrationPhase::PreparationIntent),
        "preparation_complete" => Ok(UnityMigrationPhase::PreparationComplete),
        "launch_intent" => Ok(UnityMigrationPhase::LaunchIntent),
        "unity_started" => Ok(UnityMigrationPhase::UnityStarted),
        "unity_exited" => Ok(UnityMigrationPhase::UnityExited),
        "project_reobserved" => Ok(UnityMigrationPhase::ProjectReobserved),
        "state_committed" => Ok(UnityMigrationPhase::StateCommitted),
        "cleanup_complete" => Ok(UnityMigrationPhase::CleanupComplete),
        "recovery_required" => Ok(UnityMigrationPhase::RecoveryRequired),
        _ => Err(internal()),
    }
}
fn begin(connection: &mut Connection) -> Result<Transaction<'_>, M7UnityMigrationError> {
    connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(failure)
}
fn json<T: serde::Serialize>(value: &T) -> Result<String, M7UnityMigrationError> {
    serde_json::to_string(value).map_err(|_| internal())
}
fn integer(value: u64) -> Result<i64, M7UnityMigrationError> {
    i64::try_from(value).map_err(|_| internal())
}
fn unsigned(value: i64) -> Result<u64, M7UnityMigrationError> {
    u64::try_from(value).map_err(|_| internal())
}
fn revision(value: u64) -> Result<Revision, M7UnityMigrationError> {
    Revision::new(value).ok_or_else(internal)
}
fn unsigned_sql(value: i64) -> rusqlite::Result<u64> {
    u64::try_from(value).map_err(|_| rusqlite::Error::InvalidQuery)
}
fn array32(value: Vec<u8>) -> rusqlite::Result<[u8; 32]> {
    value.try_into().map_err(|_| rusqlite::Error::InvalidQuery)
}
fn error(code: M7UnityMigrationErrorCode) -> M7UnityMigrationError {
    M7UnityMigrationError::new(code)
}
fn internal() -> M7UnityMigrationError {
    error(M7UnityMigrationErrorCode::Internal)
}
fn failure(_: rusqlite::Error) -> M7UnityMigrationError {
    error(M7UnityMigrationErrorCode::StoreUnavailable)
}
pub(super) fn unavailable() -> M7UnityMigrationError {
    error(M7UnityMigrationErrorCode::StoreUnavailable)
}
