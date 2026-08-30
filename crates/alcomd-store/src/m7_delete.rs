use alcomd_application::{
    IdempotencyKey, M7DeleteError, M7DeleteErrorCode, OperationId, PlanId, PrincipalId,
    ProjectDeleteApplyOutcome, ProjectDeleteFilesystemEvidence, ProjectDeleteOperationRecord,
    ProjectDeletePhase, ProjectDeletePlanDraft, ProjectDeletePlanOutcome, ProjectDeletePlanRecord,
    Revision,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};
use uuid::Uuid;

pub(super) fn create_plan(
    connection: &mut Connection,
    owner: &PrincipalId,
    draft: ProjectDeletePlanDraft,
) -> Result<ProjectDeletePlanOutcome, M7DeleteError> {
    let fingerprint = plan_request_fingerprint(&draft)?;
    let transaction = begin(connection)?;
    if let Some((stored, response)) = transaction
        .query_row(
            "SELECT request_fingerprint,response_json FROM idempotency_records
             WHERE principal_id=?1 AND method='projects.planDeleteDirectory' AND idempotency_key=?2",
            params![owner.as_str(), draft.plan_idempotency_key.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(failure)?
    {
        if stored != fingerprint {
            return Err(error(M7DeleteErrorCode::IdempotencyConflict));
        }
        let mut outcome: ProjectDeletePlanOutcome =
            serde_json::from_str(&response).map_err(|_| internal())?;
        outcome.replayed = true;
        transaction.commit().map_err(failure)?;
        return Ok(outcome);
    }
    transaction
        .execute(
            "INSERT INTO project_delete_plans (
                plan_id,owner_principal_id,state,project_id,project_revision,root_path,
                root_identity,parent_path,parent_identity,parent_identity_sha256,normalized_leaf,
                project_snapshot_json,marker_fingerprint,writer_evidence_json,deletion_mode,
                safety_profile_id,safety_profile_version,protected_root_profile_version,
                plan_fingerprint,plan_json,plan_idempotency_key,created_at_ms,expires_at_ms
             ) VALUES (?1,?2,'unapplied',?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,
                'sibling-quarantine-permanent-v1','alcomd-project-delete',1,1,?14,?15,?16,?17,?18)",
            params![
                draft.plan_id.to_string(),
                owner.as_str(),
                draft.project.project_id.to_string(),
                integer(draft.project.revision.get())?,
                draft.project.observation.root_path,
                draft.root_identity,
                draft.canonical_parent_path,
                draft.parent_identity,
                draft.parent_identity_sha256.as_slice(),
                draft.normalized_leaf,
                json(&draft.project)?,
                draft.project_marker_sha256.as_slice(),
                json(&draft.writer_evidence)?,
                draft.plan_fingerprint.as_slice(),
                draft.plan_json,
                draft.plan_idempotency_key.as_str(),
                integer(draft.created_at_ms)?,
                integer(draft.expires_at_ms)?,
            ],
        )
        .map_err(failure)?;
    let plan = load_plan(&transaction, owner, draft.plan_id)?;
    let outcome = ProjectDeletePlanOutcome {
        plan,
        replayed: false,
    };
    transaction
        .execute(
            "INSERT INTO idempotency_records (
                principal_id,method,idempotency_key,request_fingerprint,state,
                operation_id,response_json,created_at_ms
             ) VALUES (?1,'projects.planDeleteDirectory',?2,?3,'completed',NULL,?4,?5)",
            params![
                owner.as_str(),
                draft.plan_idempotency_key.as_str(),
                fingerprint,
                json(&outcome)?,
                integer(draft.created_at_ms)?,
            ],
        )
        .map_err(failure)?;
    transaction.commit().map_err(failure)?;
    Ok(outcome)
}

pub(super) fn get_plan(
    connection: &Connection,
    owner: &PrincipalId,
    plan_id: PlanId,
) -> Result<ProjectDeletePlanRecord, M7DeleteError> {
    load_plan(connection, owner, plan_id)
}

pub(super) fn replay_apply(
    connection: &Connection,
    owner: &PrincipalId,
    plan_id: PlanId,
    expected_revision: Revision,
    key: &IdempotencyKey,
) -> Result<Option<ProjectDeleteApplyOutcome>, M7DeleteError> {
    let expected = apply_request_fingerprint(plan_id, expected_revision)?;
    let Some((stored, response)) = connection
        .query_row(
            "SELECT request_fingerprint,response_json FROM idempotency_records
             WHERE principal_id=?1 AND method='projects.applyDeleteDirectory' AND idempotency_key=?2",
            params![owner.as_str(), key.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(failure)?
    else {
        return Ok(None);
    };
    if stored != expected {
        return Err(error(M7DeleteErrorCode::IdempotencyConflict));
    }
    let mut outcome: ProjectDeleteApplyOutcome =
        serde_json::from_str(&response).map_err(|_| internal())?;
    outcome.replayed = true;
    outcome.schedule = false;
    Ok(Some(outcome))
}

pub(super) fn accept(
    connection: &mut Connection,
    owner: &PrincipalId,
    plan_id: PlanId,
    expected_revision: Revision,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<ProjectDeleteApplyOutcome, M7DeleteError> {
    if let Some(outcome) = replay_apply(connection, owner, plan_id, expected_revision, key)? {
        return Ok(outcome);
    }
    let fingerprint = apply_request_fingerprint(plan_id, expected_revision)?;
    let transaction = begin(connection)?;
    let plan = load_plan(&transaction, owner, plan_id)?;
    if now_ms >= plan.draft.expires_at_ms || plan.draft.project.revision != expected_revision {
        return Err(error(M7DeleteErrorCode::ProjectDeletePlanStale));
    }
    let operation_id = OperationId::new();
    transaction
        .execute(
            "INSERT INTO operations (
                operation_id,kind,state,revision,owner_principal_id,request_json,
                created_at_ms,updated_at_ms
             ) VALUES (?1,'projects.delete-directory','queued',1,?2,?3,?4,?4)",
            params![
                operation_id.to_string(),
                owner.as_str(),
                fingerprint,
                integer(now_ms)?,
            ],
        )
        .map_err(failure)?;
    let changed = transaction
        .execute(
            "UPDATE project_delete_plans SET state='applied',apply_operation_id=?1
             WHERE plan_id=?2 AND state='unapplied' AND apply_operation_id IS NULL",
            params![operation_id.to_string(), plan_id.to_string()],
        )
        .map_err(failure)?;
    if changed != 1 {
        return Err(error(M7DeleteErrorCode::ProjectDeletePlanStale));
    }
    let evidence = ProjectDeleteFilesystemEvidence {
        quarantine_locator: quarantine_locator(operation_id, &plan),
        quarantine_identity: None,
        entry_count: None,
        safe_evidence: Vec::new(),
    };
    insert_journal(
        &transaction,
        operation_id,
        1,
        &plan,
        ProjectDeletePhase::Accepted,
        &evidence,
        now_ms,
    )?;
    let outcome = ProjectDeleteApplyOutcome {
        operation_id,
        project_id: plan.draft.project.project_id,
        replayed: false,
        schedule: true,
    };
    transaction
        .execute(
            "INSERT INTO idempotency_records (
                principal_id,method,idempotency_key,request_fingerprint,state,
                operation_id,response_json,created_at_ms
             ) VALUES (?1,'projects.applyDeleteDirectory',?2,?3,'completed',?4,?5,?6)",
            params![
                owner.as_str(),
                key.as_str(),
                fingerprint,
                operation_id.to_string(),
                json(&outcome)?,
                integer(now_ms)?,
            ],
        )
        .map_err(failure)?;
    insert_operation_event(
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
) -> Result<ProjectDeleteOperationRecord, M7DeleteError> {
    let transaction = begin(connection)?;
    let (owner, state, operation_revision, plan_id) =
        operation_context(&transaction, operation_id)?;
    if !matches!(state.as_str(), "queued" | "recovering" | "running") {
        return Err(internal());
    }
    let next = operation_revision.checked_add(1).ok_or_else(internal)?;
    transaction
        .execute(
            "UPDATE operations SET state='running',revision=?1,updated_at_ms=?2,
                    started_at_ms=coalesce(started_at_ms,?2),error_code=NULL,diagnostic_id=NULL
             WHERE operation_id=?3",
            params![integer(next)?, integer(now_ms)?, operation_id.to_string()],
        )
        .map_err(failure)?;
    insert_operation_event(
        &transaction,
        &owner,
        operation_id,
        revision(next)?,
        "operation.progress",
        now_ms,
    )?;
    let plan = load_plan(&transaction, &owner, plan_id)?;
    let (phase, evidence) = latest_evidence(&transaction, operation_id)?;
    transaction.commit().map_err(failure)?;
    Ok(ProjectDeleteOperationRecord {
        plan,
        phase,
        evidence,
    })
}

pub(super) fn checkpoint(
    connection: &mut Connection,
    operation_id: OperationId,
    phase: ProjectDeletePhase,
    evidence: ProjectDeleteFilesystemEvidence,
    now_ms: u64,
) -> Result<(), M7DeleteError> {
    let transaction = begin(connection)?;
    let (owner, state, operation_revision, plan_id) =
        operation_context(&transaction, operation_id)?;
    if !matches!(state.as_str(), "running" | "cancelling") {
        return Err(internal());
    }
    let plan = load_plan(&transaction, &owner, plan_id)?;
    insert_journal(
        &transaction,
        operation_id,
        latest_step(&transaction, operation_id)? + 1,
        &plan,
        phase,
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
    insert_operation_event(
        &transaction,
        &owner,
        operation_id,
        revision(next)?,
        "operation.progress",
        now_ms,
    )?;
    transaction.commit().map_err(failure)
}

pub(super) fn commit_registry(
    connection: &mut Connection,
    operation_id: OperationId,
    now_ms: u64,
) -> Result<(), M7DeleteError> {
    let transaction = begin(connection)?;
    let (owner, state, operation_revision, plan_id) =
        operation_context(&transaction, operation_id)?;
    if state != "running" {
        return Err(internal());
    }
    let plan = load_plan(&transaction, &owner, plan_id)?;
    let deleted = transaction
        .execute(
            "DELETE FROM projects WHERE owner_principal_id=?1 AND project_id=?2 AND revision=?3",
            params![
                owner.as_str(),
                plan.draft.project.project_id.to_string(),
                integer(plan.draft.project.revision.get())?,
            ],
        )
        .map_err(failure)?;
    let already_committed = transaction
        .query_row(
            "SELECT count(*) FROM events WHERE kind='project.directory_deleted'
             AND aggregate_kind='project' AND aggregate_id=?1",
            [plan.draft.project.project_id.to_string()],
            |row| row.get::<_, i64>(0),
        )
        .map_err(failure)?
        == 1;
    if deleted == 0 && !already_committed {
        return Err(error(M7DeleteErrorCode::ProjectDeleteSourceChanged));
    }
    let (_, evidence) = latest_evidence(&transaction, operation_id)?;
    if !already_committed {
        insert_project_deleted_event(&transaction, &owner, &plan, now_ms)?;
    }
    insert_journal(
        &transaction,
        operation_id,
        latest_step(&transaction, operation_id)? + 1,
        &plan,
        ProjectDeletePhase::StateCommitted,
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
    insert_operation_event(
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
) -> Result<(), M7DeleteError> {
    let transaction = begin(connection)?;
    let (owner, state, operation_revision, plan_id) =
        operation_context(&transaction, operation_id)?;
    if state != "running"
        || latest_evidence(&transaction, operation_id)?.0 != ProjectDeletePhase::CleanupComplete
    {
        return Err(internal());
    }
    let plan = load_plan(&transaction, &owner, plan_id)?;
    let next = operation_revision.checked_add(1).ok_or_else(internal)?;
    transaction
        .execute(
            "UPDATE operations SET state='succeeded',revision=?1,updated_at_ms=?2,
                    completed_at_ms=?2,result_json=?3,error_code=NULL,diagnostic_id=NULL
             WHERE operation_id=?4",
            params![
                integer(next)?,
                integer(now_ms)?,
                json(&serde_json::json!({"projectId": plan.draft.project.project_id}))?,
                operation_id.to_string(),
            ],
        )
        .map_err(failure)?;
    insert_operation_event(
        &transaction,
        &owner,
        operation_id,
        revision(next)?,
        "operation.succeeded",
        now_ms,
    )?;
    transaction.commit().map_err(failure)
}

pub(super) fn fail(
    connection: &mut Connection,
    operation_id: OperationId,
    code: &str,
    diagnostic_id: &str,
    now_ms: u64,
) -> Result<(), M7DeleteError> {
    let transaction = begin(connection)?;
    let (owner, state, operation_revision, plan_id) =
        operation_context(&transaction, operation_id)?;
    if !matches!(
        state.as_str(),
        "running" | "cancelling" | "recovering" | "queued"
    ) {
        transaction.commit().map_err(failure)?;
        return Ok(());
    }
    let (phase, evidence) = latest_evidence(&transaction, operation_id)?;
    let next = operation_revision.checked_add(1).ok_or_else(internal)?;
    if phase_at_or_after_intent(phase) {
        let plan = load_plan(&transaction, &owner, plan_id)?;
        insert_journal(
            &transaction,
            operation_id,
            latest_step(&transaction, operation_id)? + 1,
            &plan,
            ProjectDeletePhase::RecoveryRequired,
            &evidence,
            now_ms,
        )?;
        transaction
            .execute(
                "UPDATE operations SET state='recovering',revision=?1,updated_at_ms=?2,
                        error_code='project_delete_recovery_required',diagnostic_id=?3
                 WHERE operation_id=?4",
                params![
                    integer(next)?,
                    integer(now_ms)?,
                    diagnostic_id,
                    operation_id.to_string()
                ],
            )
            .map_err(failure)?;
    } else {
        transaction
            .execute(
                "UPDATE operations SET state='failed',revision=?1,updated_at_ms=?2,
                        completed_at_ms=?2,error_code=?3,diagnostic_id=?4
                 WHERE operation_id=?5",
                params![
                    integer(next)?,
                    integer(now_ms)?,
                    code,
                    diagnostic_id,
                    operation_id.to_string()
                ],
            )
            .map_err(failure)?;
    }
    insert_operation_event(
        &transaction,
        &owner,
        operation_id,
        revision(next)?,
        if phase_at_or_after_intent(phase) {
            "operation.recovering"
        } else {
            "operation.failed"
        },
        now_ms,
    )?;
    transaction.commit().map_err(failure)
}

pub(super) fn recover(
    connection: &mut Connection,
    now_ms: u64,
) -> Result<Vec<OperationId>, M7DeleteError> {
    let transaction = begin(connection)?;
    let mut statement = transaction
        .prepare(
            "SELECT operation_id FROM operations WHERE kind='projects.delete-directory'
             AND state IN ('queued','running','recovering','interrupted')
             ORDER BY created_at_ms,operation_id",
        )
        .map_err(failure)?;
    let values = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(failure)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(failure)?;
    drop(statement);
    let mut result = Vec::new();
    for value in values {
        let operation_id = OperationId::parse(&value).map_err(|_| internal())?;
        transaction
            .execute(
                "UPDATE operations SET state='recovering',updated_at_ms=?1
                 WHERE operation_id=?2 AND state!='queued'",
                params![integer(now_ms)?, value],
            )
            .map_err(failure)?;
        result.push(operation_id);
    }
    transaction.commit().map_err(failure)?;
    Ok(result)
}

pub(super) fn cancellation_requested(
    connection: &Connection,
    operation_id: OperationId,
) -> Result<bool, M7DeleteError> {
    connection
        .query_row(
            "SELECT cancel_requested FROM operations
             WHERE operation_id=?1 AND kind='projects.delete-directory'",
            [operation_id.to_string()],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(failure)?
        .map(|value| value == 1)
        .ok_or_else(internal)
}

pub(super) fn finish_cancelled(
    connection: &mut Connection,
    operation_id: OperationId,
    now_ms: u64,
) -> Result<(), M7DeleteError> {
    let transaction = begin(connection)?;
    let (owner, state, operation_revision, _) = operation_context(&transaction, operation_id)?;
    if !matches!(state.as_str(), "running" | "cancelling" | "recovering") {
        return Err(internal());
    }
    if phase_at_or_after_intent(latest_evidence(&transaction, operation_id)?.0) {
        return Err(error(M7DeleteErrorCode::ProjectDeleteRecoveryRequired));
    }
    let next = operation_revision.checked_add(1).ok_or_else(internal)?;
    transaction
        .execute(
            "UPDATE operations SET state='cancelled',revision=?1,updated_at_ms=?2,
                    completed_at_ms=?2 WHERE operation_id=?3",
            params![integer(next)?, integer(now_ms)?, operation_id.to_string()],
        )
        .map_err(failure)?;
    insert_operation_event(
        &transaction,
        &owner,
        operation_id,
        revision(next)?,
        "operation.cancelled",
        now_ms,
    )?;
    transaction.commit().map_err(failure)
}

fn load_plan(
    connection: &Connection,
    owner: &PrincipalId,
    plan_id: PlanId,
) -> Result<ProjectDeletePlanRecord, M7DeleteError> {
    connection
        .query_row(
            "SELECT project_snapshot_json,root_identity,parent_path,parent_identity,
                    parent_identity_sha256,normalized_leaf,marker_fingerprint,
                    writer_evidence_json,safety_profile_version,plan_fingerprint,plan_json,
                    plan_idempotency_key,created_at_ms,expires_at_ms
             FROM project_delete_plans WHERE plan_id=?1 AND owner_principal_id=?2",
            params![plan_id.to_string(), owner.as_str()],
            |row| plan_from_row(row, plan_id, owner.clone()),
        )
        .optional()
        .map_err(failure)?
        .ok_or_else(|| error(M7DeleteErrorCode::ProjectDeletePlanNotFound))
}

fn plan_from_row(
    row: &Row<'_>,
    plan_id: PlanId,
    owner: PrincipalId,
) -> rusqlite::Result<ProjectDeletePlanRecord> {
    let project = serde_json::from_str(&row.get::<_, String>(0)?)
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    let writer_evidence = serde_json::from_str(&row.get::<_, String>(7)?)
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    Ok(ProjectDeletePlanRecord {
        draft: ProjectDeletePlanDraft {
            plan_id,
            project,
            root_identity: row.get(1)?,
            canonical_parent_path: row.get(2)?,
            parent_identity: row.get(3)?,
            parent_identity_sha256: array32(row.get(4)?)?,
            normalized_leaf: row.get(5)?,
            project_marker_sha256: array32(row.get(6)?)?,
            writer_evidence,
            profile_version: row.get(8)?,
            plan_fingerprint: array32(row.get(9)?)?,
            plan_json: row.get(10)?,
            plan_idempotency_key: IdempotencyKey::parse(row.get::<_, String>(11)?)
                .map_err(|_| rusqlite::Error::InvalidQuery)?,
            created_at_ms: u64::try_from(row.get::<_, i64>(12)?)
                .map_err(|_| rusqlite::Error::InvalidQuery)?,
            expires_at_ms: u64::try_from(row.get::<_, i64>(13)?)
                .map_err(|_| rusqlite::Error::InvalidQuery)?,
        },
        owner,
    })
}

fn operation_context(
    connection: &Connection,
    operation_id: OperationId,
) -> Result<(PrincipalId, String, u64, PlanId), M7DeleteError> {
    let (owner, state, revision_value, plan_id) = connection
        .query_row(
            "SELECT o.owner_principal_id,o.state,o.revision,p.plan_id
             FROM operations o JOIN project_delete_plans p ON p.apply_operation_id=o.operation_id
             WHERE o.operation_id=?1 AND o.kind='projects.delete-directory'",
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
        PrincipalId::parse(&owner).map_err(|_| internal())?,
        state,
        unsigned(revision_value)?,
        PlanId::parse(&plan_id).map_err(|_| internal())?,
    ))
}

fn insert_journal(
    transaction: &Transaction<'_>,
    operation_id: OperationId,
    step: u64,
    plan: &ProjectDeletePlanRecord,
    phase: ProjectDeletePhase,
    evidence: &ProjectDeleteFilesystemEvidence,
    now_ms: u64,
) -> Result<(), M7DeleteError> {
    let state = if matches!(
        phase,
        ProjectDeletePhase::QuarantineIntent
            | ProjectDeletePhase::RegistryCommitIntent
            | ProjectDeletePhase::Deleting
    ) {
        "intent"
    } else {
        "completed"
    };
    transaction
        .execute(
            "INSERT INTO project_delete_filesystem_journal (
                operation_id,step,plan_id,project_id,phase,state,root_identity,parent_identity,
                quarantine_identity,payload_identity,quarantine_locator,owner_marker,
                attempt_count,entries_processed,safe_reason,evidence_json,updated_at_ms
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?9,?10,'owner.json',1,?11,?12,?13,?14)",
            params![
                operation_id.to_string(),
                integer(step)?,
                plan.draft.plan_id.to_string(),
                plan.draft.project.project_id.to_string(),
                phase.as_str(),
                state,
                plan.draft.root_identity,
                plan.draft.parent_identity,
                evidence.quarantine_identity,
                evidence.quarantine_locator,
                integer(evidence.entry_count.unwrap_or(0))?,
                evidence.safe_evidence.last(),
                json(evidence)?,
                integer(now_ms)?,
            ],
        )
        .map_err(failure)?;
    Ok(())
}

fn latest_evidence(
    connection: &Connection,
    operation_id: OperationId,
) -> Result<(ProjectDeletePhase, ProjectDeleteFilesystemEvidence), M7DeleteError> {
    let (phase, evidence): (String, String) = connection
        .query_row(
            "SELECT phase,evidence_json FROM project_delete_filesystem_journal
             WHERE operation_id=?1 ORDER BY step DESC LIMIT 1",
            [operation_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(failure)?;
    Ok((
        parse_phase(&phase)?,
        serde_json::from_str(&evidence).map_err(|_| internal())?,
    ))
}

fn latest_step(connection: &Connection, operation_id: OperationId) -> Result<u64, M7DeleteError> {
    connection
        .query_row(
            "SELECT coalesce(max(step),0) FROM project_delete_filesystem_journal WHERE operation_id=?1",
            [operation_id.to_string()],
            |row| row.get::<_, i64>(0),
        )
        .map_err(failure)
        .and_then(unsigned)
}

fn parse_phase(value: &str) -> Result<ProjectDeletePhase, M7DeleteError> {
    match value {
        "accepted" => Ok(ProjectDeletePhase::Accepted),
        "preflight_complete" => Ok(ProjectDeletePhase::PreflightComplete),
        "quarantine_intent" => Ok(ProjectDeletePhase::QuarantineIntent),
        "root_quarantined" => Ok(ProjectDeletePhase::RootQuarantined),
        "registry_commit_intent" => Ok(ProjectDeletePhase::RegistryCommitIntent),
        "state_committed" => Ok(ProjectDeletePhase::StateCommitted),
        "deleting" => Ok(ProjectDeletePhase::Deleting),
        "cleanup_complete" => Ok(ProjectDeletePhase::CleanupComplete),
        "recovery_required" => Ok(ProjectDeletePhase::RecoveryRequired),
        _ => Err(internal()),
    }
}

fn phase_at_or_after_intent(phase: ProjectDeletePhase) -> bool {
    matches!(
        phase,
        ProjectDeletePhase::QuarantineIntent
            | ProjectDeletePhase::RootQuarantined
            | ProjectDeletePhase::RegistryCommitIntent
            | ProjectDeletePhase::StateCommitted
            | ProjectDeletePhase::Deleting
            | ProjectDeletePhase::CleanupComplete
            | ProjectDeletePhase::RecoveryRequired
    )
}

fn quarantine_locator(operation_id: OperationId, plan: &ProjectDeletePlanRecord) -> String {
    std::path::Path::new(&plan.draft.canonical_parent_path)
        .join(format!(".alcomd-delete-{operation_id}.quarantine"))
        .to_string_lossy()
        .into_owned()
}

fn plan_request_fingerprint(draft: &ProjectDeletePlanDraft) -> Result<String, M7DeleteError> {
    json(&serde_json::json!({
        "projectId": draft.project.project_id,
        "expectedRevision": draft.project.revision,
        "version": 1
    }))
}

fn apply_request_fingerprint(
    plan_id: PlanId,
    expected_revision: Revision,
) -> Result<String, M7DeleteError> {
    json(&serde_json::json!({
        "expectedRevision": expected_revision.get(),
        "planId": plan_id,
        "version": 1
    }))
}

fn insert_operation_event(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    operation_id: OperationId,
    revision: Revision,
    kind: &str,
    now_ms: u64,
) -> Result<(), M7DeleteError> {
    transaction
        .execute(
            "INSERT INTO events (
                event_id,kind,aggregate_kind,aggregate_id,aggregate_revision,
                principal_id,occurred_at_ms,payload_json
             ) VALUES (?1,?2,'operation',?3,?4,?5,?6,'{}')",
            params![
                Uuid::new_v4().to_string(),
                kind,
                operation_id.to_string(),
                integer(revision.get())?,
                owner.as_str(),
                integer(now_ms)?,
            ],
        )
        .map_err(failure)?;
    Ok(())
}

fn insert_project_deleted_event(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    plan: &ProjectDeletePlanRecord,
    now_ms: u64,
) -> Result<(), M7DeleteError> {
    let next = plan
        .draft
        .project
        .revision
        .checked_next()
        .ok_or_else(internal)?;
    transaction
        .execute(
            "INSERT INTO events (
                event_id,kind,aggregate_kind,aggregate_id,aggregate_revision,
                principal_id,occurred_at_ms,payload_json
             ) VALUES (?1,'project.directory_deleted','project',?2,?3,?4,?5,?6)",
            params![
                Uuid::new_v4().to_string(),
                plan.draft.project.project_id.to_string(),
                integer(next.get())?,
                owner.as_str(),
                integer(now_ms)?,
                json(&serde_json::json!({
                    "mode": "sibling-quarantine-permanent-v1",
                    "profileVersion": plan.draft.profile_version
                }))?,
            ],
        )
        .map_err(failure)?;
    Ok(())
}

fn begin(connection: &mut Connection) -> Result<Transaction<'_>, M7DeleteError> {
    connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(failure)
}

fn json<T: serde::Serialize>(value: &T) -> Result<String, M7DeleteError> {
    serde_json::to_string(value).map_err(|_| internal())
}

fn integer(value: u64) -> Result<i64, M7DeleteError> {
    i64::try_from(value).map_err(|_| internal())
}

fn unsigned(value: i64) -> Result<u64, M7DeleteError> {
    u64::try_from(value).map_err(|_| internal())
}

fn revision(value: u64) -> Result<Revision, M7DeleteError> {
    Revision::new(value).ok_or_else(internal)
}

fn array32(value: Vec<u8>) -> rusqlite::Result<[u8; 32]> {
    value.try_into().map_err(|_| rusqlite::Error::InvalidQuery)
}

fn error(code: M7DeleteErrorCode) -> M7DeleteError {
    M7DeleteError::new(code)
}

fn internal() -> M7DeleteError {
    error(M7DeleteErrorCode::Internal)
}

fn failure(source: rusqlite::Error) -> M7DeleteError {
    #[cfg(feature = "test-kill-gates")]
    eprintln!("M7 Project Delete test-only SQLite failure: {source}");
    #[cfg(not(feature = "test-kill-gates"))]
    let _ = source;
    error(M7DeleteErrorCode::StoreUnavailable)
}

pub(super) fn unavailable() -> M7DeleteError {
    error(M7DeleteErrorCode::StoreUnavailable)
}
