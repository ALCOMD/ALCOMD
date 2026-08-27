use alcomd_application::{
    IdempotencyKey, M7CopyError, M7CopyErrorCode, OperationId, PlanId, PrincipalId,
    ProjectCopyApplyOutcome, ProjectCopyInventoryEvidence, ProjectCopyOperationRecord,
    ProjectCopyPhase, ProjectCopyPlanDraft, ProjectCopyPlanOutcome, ProjectCopyPlanRecord,
    ProjectId, PublishedProjectCopy, Revision,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};
use uuid::Uuid;

pub(super) fn create_plan(
    connection: &mut Connection,
    owner: &PrincipalId,
    draft: ProjectCopyPlanDraft,
) -> Result<ProjectCopyPlanOutcome, M7CopyError> {
    let fingerprint = plan_request_fingerprint(&draft)?;
    let transaction = begin(connection)?;
    if let Some((stored, response)) = transaction
        .query_row(
            "SELECT request_fingerprint,response_json FROM idempotency_records
             WHERE principal_id=?1 AND method='projects.planCopy' AND idempotency_key=?2",
            params![owner.as_str(), draft.plan_idempotency_key.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(failure)?
    {
        if stored != fingerprint {
            return Err(error(M7CopyErrorCode::IdempotencyConflict));
        }
        let mut outcome: ProjectCopyPlanOutcome =
            serde_json::from_str(&response).map_err(|_| internal())?;
        outcome.replayed = true;
        transaction.commit().map_err(failure)?;
        return Ok(outcome);
    }
    transaction
        .execute(
            "INSERT INTO project_copy_plans (
                plan_id,owner_principal_id,state,source_project_id,source_revision,
                source_root_path,source_root_identity,source_snapshot_json,
                target_parent_path,target_parent_identity,target_parent_identity_sha256,
                target_leaf,target_project_id,profile_version,writer_evidence_json,
                plan_fingerprint,plan_json,plan_idempotency_key,created_at_ms,expires_at_ms
             ) VALUES (?1,?2,'unapplied',?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)",
            params![
                draft.plan_id.to_string(),
                owner.as_str(),
                draft.source_project.project_id.to_string(),
                integer(draft.source_project.revision.get())?,
                draft.source_project.observation.root_path,
                draft.source_root_identity,
                json(&draft.source_project)?,
                draft.target_parent_path,
                draft.target_parent_identity,
                draft.target_parent_identity_sha256.as_slice(),
                draft.target_leaf,
                draft.target_project_id.to_string(),
                i64::from(draft.profile_version),
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
    let outcome = ProjectCopyPlanOutcome {
        plan,
        replayed: false,
    };
    transaction
        .execute(
            "INSERT INTO idempotency_records (
                principal_id,method,idempotency_key,request_fingerprint,state,
                operation_id,response_json,created_at_ms
             ) VALUES (?1,'projects.planCopy',?2,?3,'completed',NULL,?4,?5)",
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
) -> Result<ProjectCopyPlanRecord, M7CopyError> {
    load_plan(connection, owner, plan_id)
}

pub(super) fn replay_apply(
    connection: &Connection,
    owner: &PrincipalId,
    plan_id: PlanId,
    expected_revision: Revision,
    key: &IdempotencyKey,
) -> Result<Option<ProjectCopyApplyOutcome>, M7CopyError> {
    let expected_fingerprint = apply_request_fingerprint(plan_id, expected_revision)?;
    let Some((stored_fingerprint, response)) = connection
        .query_row(
            "SELECT request_fingerprint,response_json FROM idempotency_records
             WHERE principal_id=?1 AND method='projects.applyCopy' AND idempotency_key=?2",
            params![owner.as_str(), key.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(failure)?
    else {
        return Ok(None);
    };
    if stored_fingerprint != expected_fingerprint {
        return Err(error(M7CopyErrorCode::IdempotencyConflict));
    }
    let mut outcome: ProjectCopyApplyOutcome =
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
) -> Result<ProjectCopyApplyOutcome, M7CopyError> {
    let fingerprint = apply_request_fingerprint(plan_id, expected_revision)?;
    let transaction = begin(connection)?;
    if let Some((stored, response)) = transaction
        .query_row(
            "SELECT request_fingerprint,response_json FROM idempotency_records
             WHERE principal_id=?1 AND method='projects.applyCopy' AND idempotency_key=?2",
            params![owner.as_str(), key.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(failure)?
    {
        if stored != fingerprint {
            return Err(error(M7CopyErrorCode::IdempotencyConflict));
        }
        let mut outcome: ProjectCopyApplyOutcome =
            serde_json::from_str(&response).map_err(|_| internal())?;
        outcome.replayed = true;
        outcome.schedule = false;
        transaction.commit().map_err(failure)?;
        return Ok(outcome);
    }
    let plan = load_plan(&transaction, owner, plan_id)?;
    if now_ms >= plan.draft.expires_at_ms || plan.draft.source_project.revision != expected_revision
    {
        return Err(error(M7CopyErrorCode::ProjectCopyPlanStale));
    }
    let applied: Option<String> = transaction
        .query_row(
            "SELECT apply_operation_id FROM project_copy_plans WHERE plan_id=?1",
            [plan_id.to_string()],
            |row| row.get(0),
        )
        .map_err(failure)?;
    if applied.is_some() {
        return Err(error(M7CopyErrorCode::ProjectCopyPlanStale));
    }
    let operation_id = OperationId::new();
    transaction
        .execute(
            "INSERT INTO operations (
                operation_id,kind,state,revision,owner_principal_id,request_json,
                created_at_ms,updated_at_ms
             ) VALUES (?1,'projects.copy','queued',1,?2,?3,?4,?4)",
            params![
                operation_id.to_string(),
                owner.as_str(),
                fingerprint,
                integer(now_ms)?,
            ],
        )
        .map_err(failure)?;
    transaction
        .execute(
            "UPDATE project_copy_plans SET state='applied',apply_operation_id=?1
             WHERE plan_id=?2 AND state='unapplied' AND apply_operation_id IS NULL",
            params![operation_id.to_string(), plan_id.to_string()],
        )
        .map_err(failure)?;
    insert_journal(
        &transaction,
        operation_id,
        1,
        &plan,
        ProjectCopyPhase::Accepted,
        (None, None),
        now_ms,
    )?;
    let outcome = ProjectCopyApplyOutcome {
        operation_id,
        target_project_id: plan.draft.target_project_id,
        replayed: false,
        schedule: true,
    };
    transaction
        .execute(
            "INSERT INTO idempotency_records (
                principal_id,method,idempotency_key,request_fingerprint,state,
                operation_id,response_json,created_at_ms
             ) VALUES (?1,'projects.applyCopy',?2,?3,'completed',?4,?5,?6)",
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
    insert_event(
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
) -> Result<ProjectCopyOperationRecord, M7CopyError> {
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
                    started_at_ms=coalesce(started_at_ms,?2) WHERE operation_id=?3",
            params![integer(next)?, integer(now_ms)?, operation_id.to_string()],
        )
        .map_err(failure)?;
    insert_event(
        &transaction,
        &owner,
        operation_id,
        revision(next)?,
        "operation.progress",
        now_ms,
    )?;
    let plan = load_plan(&transaction, &owner, plan_id)?;
    let (phase, inventory, published) = latest_evidence(&transaction, operation_id)?;
    transaction.commit().map_err(failure)?;
    Ok(ProjectCopyOperationRecord {
        plan,
        phase,
        inventory,
        published,
    })
}

pub(super) fn checkpoint(
    connection: &mut Connection,
    operation_id: OperationId,
    phase: ProjectCopyPhase,
    inventory: Option<ProjectCopyInventoryEvidence>,
    published: Option<PublishedProjectCopy>,
    now_ms: u64,
) -> Result<(), M7CopyError> {
    let transaction = begin(connection)?;
    let (owner, state, operation_revision, plan_id) =
        operation_context(&transaction, operation_id)?;
    if !matches!(state.as_str(), "running" | "cancelling") {
        return Err(internal());
    }
    let plan = load_plan(&transaction, &owner, plan_id)?;
    let step = latest_step(&transaction, operation_id)? + 1;
    insert_journal(
        &transaction,
        operation_id,
        step,
        &plan,
        phase,
        (inventory, published),
        now_ms,
    )?;
    let next = operation_revision.checked_add(1).ok_or_else(internal)?;
    transaction
        .execute(
            "UPDATE operations SET revision=?1,updated_at_ms=?2 WHERE operation_id=?3",
            params![integer(next)?, integer(now_ms)?, operation_id.to_string()],
        )
        .map_err(failure)?;
    insert_event(
        &transaction,
        &owner,
        operation_id,
        revision(next)?,
        "operation.progress",
        now_ms,
    )?;
    transaction.commit().map_err(failure)
}

pub(super) fn complete(
    connection: &mut Connection,
    operation_id: OperationId,
    published: PublishedProjectCopy,
    now_ms: u64,
) -> Result<(), M7CopyError> {
    let transaction = begin(connection)?;
    let (owner, state, operation_revision, plan_id) =
        operation_context(&transaction, operation_id)?;
    if state != "running" {
        return Err(internal());
    }
    let plan = load_plan(&transaction, &owner, plan_id)?;
    let observation = published.observation.clone();
    let semantic = super::m3::semantic_project(observation.clone());
    transaction
        .execute(
            "INSERT INTO projects (
                project_id,owner_principal_id,root_path,path_identity_key,project_type,
                unity_version,unity_revision,snapshot_json,revision,registered_at_ms,
                observed_at_ms,updated_at_ms
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,1,?9,?10,?9)",
            params![
                plan.draft.target_project_id.to_string(),
                owner.as_str(),
                observation.root_path,
                observation.path_identity_key,
                super::m3::project_type(observation.project_type),
                observation.unity_version,
                observation.unity_revision,
                json(&semantic)?,
                integer(now_ms)?,
                integer(observation.observed_at_ms)?,
            ],
        )
        .map_err(failure)?;
    insert_journal(
        &transaction,
        operation_id,
        latest_step(&transaction, operation_id)? + 1,
        &plan,
        ProjectCopyPhase::StateCommitted,
        (
            latest_evidence(&transaction, operation_id)?.1,
            Some(published),
        ),
        now_ms,
    )?;
    let next = operation_revision.checked_add(1).ok_or_else(internal)?;
    transaction
        .execute(
            "UPDATE operations SET revision=?1,updated_at_ms=?2 WHERE operation_id=?3",
            params![integer(next)?, integer(now_ms)?, operation_id.to_string()],
        )
        .map_err(failure)?;
    insert_project_event(
        &transaction,
        &owner,
        plan.draft.target_project_id,
        "project.registered",
        now_ms,
    )?;
    insert_event(
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
) -> Result<(), M7CopyError> {
    let transaction = begin(connection)?;
    let (owner, state, operation_revision, plan_id) =
        operation_context(&transaction, operation_id)?;
    if state != "running" {
        return Err(internal());
    }
    let plan = load_plan(&transaction, &owner, plan_id)?;
    let inventory = latest_evidence(&transaction, operation_id)?.1;
    insert_journal(
        &transaction,
        operation_id,
        latest_step(&transaction, operation_id)? + 1,
        &plan,
        ProjectCopyPhase::CleanupComplete,
        (inventory, None),
        now_ms,
    )?;
    let next = operation_revision.checked_add(1).ok_or_else(internal)?;
    transaction
        .execute(
            "UPDATE operations SET state='succeeded',revision=?1,updated_at_ms=?2,
                    completed_at_ms=?2,result_json=?3 WHERE operation_id=?4",
            params![
                integer(next)?,
                integer(now_ms)?,
                json(&serde_json::json!({"targetProjectId": plan.draft.target_project_id}))?,
                operation_id.to_string(),
            ],
        )
        .map_err(failure)?;
    insert_event(
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
) -> Result<(), M7CopyError> {
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
    let phase = latest_evidence(&transaction, operation_id)?.0;
    let next = operation_revision.checked_add(1).ok_or_else(internal)?;
    if phase_at_or_after_intent(phase) {
        let plan = load_plan(&transaction, &owner, plan_id)?;
        insert_journal(
            &transaction,
            operation_id,
            latest_step(&transaction, operation_id)? + 1,
            &plan,
            ProjectCopyPhase::RecoveryRequired,
            (
                latest_evidence(&transaction, operation_id)?.1,
                latest_evidence(&transaction, operation_id)?.2,
            ),
            now_ms,
        )?;
        transaction
            .execute(
                "UPDATE operations SET state='recovering',revision=?1,updated_at_ms=?2,
                        error_code='project_copy_recovery_required',diagnostic_id=?3
                 WHERE operation_id=?4",
                params![
                    integer(next)?,
                    integer(now_ms)?,
                    diagnostic_id,
                    operation_id.to_string(),
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
                    operation_id.to_string(),
                ],
            )
            .map_err(failure)?;
    }
    insert_event(
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
) -> Result<Vec<OperationId>, M7CopyError> {
    let transaction = begin(connection)?;
    let mut statement = transaction
        .prepare(
            "SELECT operation_id FROM operations
             WHERE kind='projects.copy' AND state IN ('queued','running','recovering','interrupted')
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
) -> Result<bool, M7CopyError> {
    connection
        .query_row(
            "SELECT cancel_requested FROM operations
             WHERE operation_id=?1 AND kind='projects.copy'",
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
) -> Result<(), M7CopyError> {
    let transaction = begin(connection)?;
    let (owner, state, operation_revision, _) = operation_context(&transaction, operation_id)?;
    if !matches!(state.as_str(), "running" | "cancelling" | "recovering") {
        return Err(internal());
    }
    if phase_at_or_after_intent(latest_evidence(&transaction, operation_id)?.0) {
        return Err(error(M7CopyErrorCode::ProjectCopyRecoveryRequired));
    }
    let next = operation_revision.checked_add(1).ok_or_else(internal)?;
    transaction
        .execute(
            "UPDATE operations SET state='cancelled',revision=?1,updated_at_ms=?2,
                    completed_at_ms=?2 WHERE operation_id=?3",
            params![integer(next)?, integer(now_ms)?, operation_id.to_string()],
        )
        .map_err(failure)?;
    insert_event(
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
) -> Result<ProjectCopyPlanRecord, M7CopyError> {
    connection
        .query_row(
            "SELECT source_snapshot_json,source_root_identity,target_parent_path,
                    target_parent_identity,target_parent_identity_sha256,target_leaf,
                    target_project_id,profile_version,writer_evidence_json,plan_fingerprint,
                    plan_json,plan_idempotency_key,created_at_ms,expires_at_ms
             FROM project_copy_plans WHERE plan_id=?1 AND owner_principal_id=?2",
            params![plan_id.to_string(), owner.as_str()],
            |row| plan_from_row(row, plan_id, owner.clone()),
        )
        .optional()
        .map_err(failure)?
        .ok_or_else(|| error(M7CopyErrorCode::ProjectCopyPlanNotFound))
}

fn plan_from_row(
    row: &Row<'_>,
    plan_id: PlanId,
    owner: PrincipalId,
) -> rusqlite::Result<ProjectCopyPlanRecord> {
    let source_project = serde_json::from_str(&row.get::<_, String>(0)?)
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    let writer_evidence = serde_json::from_str(&row.get::<_, String>(8)?)
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    let target_project_id =
        ProjectId::parse(&row.get::<_, String>(6)?).map_err(|_| rusqlite::Error::InvalidQuery)?;
    Ok(ProjectCopyPlanRecord {
        draft: ProjectCopyPlanDraft {
            plan_id,
            source_project,
            source_root_identity: row.get(1)?,
            target_parent_path: row.get(2)?,
            target_parent_identity: row.get(3)?,
            target_parent_identity_sha256: array32(row.get(4)?)?,
            target_leaf: row.get(5)?,
            target_project_id,
            profile_version: row.get(7)?,
            writer_evidence,
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
) -> Result<(PrincipalId, String, u64, PlanId), M7CopyError> {
    let (owner, state, revision, plan_id) = connection
        .query_row(
            "SELECT o.owner_principal_id,o.state,o.revision,p.plan_id
             FROM operations o JOIN project_copy_plans p ON p.apply_operation_id=o.operation_id
             WHERE o.operation_id=?1 AND o.kind='projects.copy'",
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
        unsigned(revision).map_err(|_| internal())?,
        PlanId::parse(&plan_id).map_err(|_| internal())?,
    ))
}

fn insert_journal(
    transaction: &Transaction<'_>,
    operation_id: OperationId,
    step: u64,
    plan: &ProjectCopyPlanRecord,
    phase: ProjectCopyPhase,
    evidence: (
        Option<ProjectCopyInventoryEvidence>,
        Option<PublishedProjectCopy>,
    ),
    now_ms: u64,
) -> Result<(), M7CopyError> {
    let (inventory, published) = evidence;
    let evidence_json = json(&serde_json::json!({
        "inventory": inventory,
        "published": published
    }))?;
    let inventory_locator = inventory
        .as_ref()
        .map(|value| value.private_locator.as_str());
    let inventory_sha256 = inventory.as_ref().map(|value| value.sha256.as_slice());
    let inventory_byte_length = inventory
        .as_ref()
        .map(|value| integer(value.byte_length))
        .transpose()?;
    let owner_marker = inventory.as_ref().map(|value| value.owner_marker.as_str());
    let target_identity = published
        .as_ref()
        .map(|value| value.target_identity.as_slice());
    transaction
        .execute(
            "INSERT INTO project_copy_filesystem_journal (
                operation_id,step,plan_id,source_project_id,target_project_id,phase,state,
                source_identity,target_parent_identity,target_identity,inventory_locator,
                inventory_sha256,inventory_byte_length,owner_marker,evidence_json,updated_at_ms
             ) VALUES (?1,?2,?3,?4,?5,?6,'completed',?7,?8,?9,?10,?11,?12,?13,?14,?15)",
            params![
                operation_id.to_string(),
                integer(step)?,
                plan.draft.plan_id.to_string(),
                plan.draft.source_project.project_id.to_string(),
                plan.draft.target_project_id.to_string(),
                phase.as_str(),
                plan.draft.source_root_identity,
                plan.draft.target_parent_identity,
                target_identity,
                inventory_locator,
                inventory_sha256,
                inventory_byte_length,
                owner_marker,
                evidence_json,
                integer(now_ms)?,
            ],
        )
        .map_err(failure)?;
    Ok(())
}

fn latest_evidence(
    connection: &Connection,
    operation_id: OperationId,
) -> Result<
    (
        ProjectCopyPhase,
        Option<ProjectCopyInventoryEvidence>,
        Option<PublishedProjectCopy>,
    ),
    M7CopyError,
> {
    let (phase, evidence): (String, String) = connection
        .query_row(
            "SELECT phase,evidence_json FROM project_copy_filesystem_journal
             WHERE operation_id=?1 ORDER BY step DESC LIMIT 1",
            [operation_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(failure)?;
    let value: serde_json::Value = serde_json::from_str(&evidence).map_err(|_| internal())?;
    let inventory = serde_json::from_value(value["inventory"].clone()).map_err(|_| internal())?;
    let published = serde_json::from_value(value["published"].clone()).map_err(|_| internal())?;
    Ok((parse_phase(&phase)?, inventory, published))
}

fn latest_step(connection: &Connection, operation_id: OperationId) -> Result<u64, M7CopyError> {
    connection
        .query_row(
            "SELECT coalesce(max(step),0) FROM project_copy_filesystem_journal WHERE operation_id=?1",
            [operation_id.to_string()],
            |row| row.get::<_, i64>(0),
        )
        .map_err(failure)
        .and_then(unsigned)
}

fn parse_phase(value: &str) -> Result<ProjectCopyPhase, M7CopyError> {
    match value {
        "accepted" => Ok(ProjectCopyPhase::Accepted),
        "inventory_ready" => Ok(ProjectCopyPhase::InventoryReady),
        "staging" => Ok(ProjectCopyPhase::Staging),
        "staging_complete" => Ok(ProjectCopyPhase::StagingComplete),
        "publish_intent" => Ok(ProjectCopyPhase::PublishIntent),
        "target_published" => Ok(ProjectCopyPhase::TargetPublished),
        "project_registry_commit_intent" => Ok(ProjectCopyPhase::ProjectRegistryCommitIntent),
        "state_committed" => Ok(ProjectCopyPhase::StateCommitted),
        "cleanup_complete" => Ok(ProjectCopyPhase::CleanupComplete),
        "recovery_required" => Ok(ProjectCopyPhase::RecoveryRequired),
        _ => Err(internal()),
    }
}

fn phase_at_or_after_intent(phase: ProjectCopyPhase) -> bool {
    matches!(
        phase,
        ProjectCopyPhase::PublishIntent
            | ProjectCopyPhase::TargetPublished
            | ProjectCopyPhase::ProjectRegistryCommitIntent
            | ProjectCopyPhase::StateCommitted
            | ProjectCopyPhase::CleanupComplete
            | ProjectCopyPhase::RecoveryRequired
    )
}

fn plan_request_fingerprint(draft: &ProjectCopyPlanDraft) -> Result<String, M7CopyError> {
    json(&serde_json::json!({
        "sourceProjectId": draft.source_project.project_id,
        "sourceRevision": draft.source_project.revision,
        "targetParentIdentity": draft.target_parent_identity,
        "targetLeaf": draft.target_leaf,
        "version": 1
    }))
}

fn apply_request_fingerprint(
    plan_id: PlanId,
    expected_revision: Revision,
) -> Result<String, M7CopyError> {
    json(&serde_json::json!({
        "expectedRevision": expected_revision.get(),
        "planId": plan_id,
        "version": 1
    }))
}

fn insert_event(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    operation_id: OperationId,
    revision: Revision,
    kind: &str,
    now_ms: u64,
) -> Result<(), M7CopyError> {
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

fn insert_project_event(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    project_id: ProjectId,
    kind: &str,
    now_ms: u64,
) -> Result<(), M7CopyError> {
    transaction
        .execute(
            "INSERT INTO events (
                event_id,kind,aggregate_kind,aggregate_id,aggregate_revision,
                principal_id,occurred_at_ms,payload_json
             ) VALUES (?1,?2,'project',?3,1,?4,?5,'{}')",
            params![
                Uuid::new_v4().to_string(),
                kind,
                project_id.to_string(),
                owner.as_str(),
                integer(now_ms)?,
            ],
        )
        .map_err(failure)?;
    Ok(())
}

fn begin(connection: &mut Connection) -> Result<Transaction<'_>, M7CopyError> {
    connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(failure)
}

fn json<T: serde::Serialize>(value: &T) -> Result<String, M7CopyError> {
    serde_json::to_string(value).map_err(|_| internal())
}
fn integer(value: u64) -> Result<i64, M7CopyError> {
    i64::try_from(value).map_err(|_| internal())
}
fn unsigned(value: i64) -> Result<u64, M7CopyError> {
    u64::try_from(value).map_err(|_| internal())
}
fn revision(value: u64) -> Result<Revision, M7CopyError> {
    Revision::new(value).ok_or_else(internal)
}
fn array32(value: Vec<u8>) -> rusqlite::Result<[u8; 32]> {
    value.try_into().map_err(|_| rusqlite::Error::InvalidQuery)
}
fn error(code: M7CopyErrorCode) -> M7CopyError {
    M7CopyError::new(code)
}
fn internal() -> M7CopyError {
    error(M7CopyErrorCode::Internal)
}
fn failure(source: rusqlite::Error) -> M7CopyError {
    #[cfg(feature = "test-kill-gates")]
    eprintln!("M7 Project Copy test-only SQLite failure: {source}");
    #[cfg(not(feature = "test-kill-gates"))]
    let _ = source;
    error(M7CopyErrorCode::StoreUnavailable)
}
pub(super) fn unavailable() -> M7CopyError {
    error(M7CopyErrorCode::StoreUnavailable)
}
