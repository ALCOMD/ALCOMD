use alcomd_application::{
    ApplyPlanOutcome, FilesystemJournalEntry, IdempotencyKey, M4Error, M4ErrorCode, OperationId,
    PackageApplyCompletion, PackageChangeSet, PackagePlanDraft, PackagePlanRecord,
    PackageSourcePin, PlanAction, PlanId, PlanState, PrincipalId, ResolverCatalog,
    ResolverCatalogEntry, Revision,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};

pub(super) fn resolver_catalog(
    connection: &Connection,
    owner: &PrincipalId,
) -> Result<ResolverCatalog, M4Error> {
    let incomplete: i64 = connection
        .query_row(
            "SELECT count(*) FROM repository_package_versions p
             JOIN repositories r ON r.repository_id=p.repository_id
             WHERE r.owner_principal_id=?1 AND p.resolver_ready=0",
            [owner.as_str()],
            |row| row.get(0),
        )
        .map_err(store_error)?;
    let mut statement = connection
        .prepare(
            "SELECT r.repository_id, r.revision, r.priority, r.source_kind, r.source_locator,
                    r.source_identity_key, p.package_id, p.semantic_version, p.yanked,
                    p.unity_text, p.author_name, p.author_email, p.artifact_url, p.zip_sha256,
                    p.unity_release_text, p.dependencies_json, p.manifest_fingerprint,
                    p.legacy_metadata_present
             FROM repository_package_versions p
             JOIN repositories r ON r.repository_id=p.repository_id
             WHERE r.owner_principal_id=?1 AND p.resolver_ready=1
             ORDER BY p.package_id ASC, p.semantic_version ASC, r.priority ASC,
                      r.repository_id ASC",
        )
        .map_err(store_error)?;
    let rows = statement
        .query_map([owner.as_str()], |row| {
            let source_kind: String = row.get(3)?;
            let source_locator: String = row.get(4)?;
            let source_identity_key: Vec<u8> = row.get(5)?;
            let source_identity = match source_kind.as_str() {
                "remote" => format!("remote:{source_locator}"),
                "local" => format!("local-fileid-v1:{}", bytes_hex(&source_identity_key)),
                _ => return Err(invalid_query()),
            };
            Ok(ResolverCatalogEntry {
                repository_id: row.get(0)?,
                repository_revision: nonnegative(row.get(1)?)?,
                repository_priority: nonnegative(row.get(2)?)?,
                source_identity,
                package_id: row.get(6)?,
                version: row.get(7)?,
                yanked: row.get::<_, i64>(8)? != 0,
                unity: row.get(9)?,
                author_name: row.get(10)?,
                author_email: row.get(11)?,
                artifact_url: row.get(12)?,
                zip_sha256: row.get(13)?,
                unity_release: row.get(14)?,
                dependencies_json: row.get(15)?,
                manifest_fingerprint: digest(row.get(16)?)?,
                legacy_metadata_present: row.get::<_, i64>(17)? != 0,
            })
        })
        .map_err(store_error)?;
    let entries = rows.collect::<Result<Vec<_>, _>>().map_err(store_error)?;
    Ok(ResolverCatalog {
        entries,
        complete: incomplete == 0,
    })
}

pub(super) fn create_package_plan(
    connection: &mut Connection,
    owner: &PrincipalId,
    draft: PackagePlanDraft,
    created_at_ms: u64,
) -> Result<PackagePlanRecord, M4Error> {
    draft.change_set.validate_bounds()?;
    let change_set_json = serde_json::to_string(&draft.change_set).map_err(|_| internal())?;
    let source_set_json = serde_json::to_string(&draft.source_set).map_err(|_| internal())?;
    if change_set_json.len() > 4 * 1024 * 1024 || source_set_json.len() > 4 * 1024 * 1024 {
        return Err(M4Error::new(M4ErrorCode::PlanTooLarge));
    }
    let created_at_ms = sqlite_integer(created_at_ms)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(store_error)?;
    let project_revision = transaction
        .query_row(
            "SELECT revision FROM projects WHERE project_id=?1 AND owner_principal_id=?2",
            params![draft.project_id.to_string(), owner.as_str()],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(store_error)?
        .ok_or_else(|| M4Error::new(M4ErrorCode::ProjectNotRegistered))?;
    if project_revision != sqlite_revision(draft.project_revision) {
        return Err(M4Error::new(M4ErrorCode::RevisionConflict));
    }
    let plan_id = PlanId::new();
    transaction
        .execute(
            "INSERT INTO package_plans (
                plan_id, owner_principal_id, project_id, action, state, project_revision,
                project_snapshot_fingerprint, change_set_fingerprint, change_set_json,
                source_set_json, created_at_ms
             ) VALUES (?1, ?2, ?3, ?4, 'unapplied', ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                plan_id.to_string(),
                owner.as_str(),
                draft.project_id.to_string(),
                draft.action.as_str(),
                sqlite_revision(draft.project_revision),
                draft.project_snapshot_fingerprint.as_slice(),
                draft.change_set_fingerprint.as_slice(),
                change_set_json,
                source_set_json,
                created_at_ms,
            ],
        )
        .map_err(store_error)?;
    let record = load_plan(&transaction, owner, plan_id)?;
    transaction.commit().map_err(store_error)?;
    Ok(record)
}

pub(super) fn get_package_plan(
    connection: &Connection,
    owner: &PrincipalId,
    plan_id: PlanId,
) -> Result<PackagePlanRecord, M4Error> {
    load_plan(connection, owner, plan_id)
}

pub(super) fn accept_package_plan(
    connection: &mut Connection,
    owner: &PrincipalId,
    plan_id: PlanId,
    expected_revision: Revision,
    idempotency_key: &IdempotencyKey,
    created_at_ms: u64,
) -> Result<ApplyPlanOutcome, M4Error> {
    let created_at_ms = sqlite_integer(created_at_ms)?;
    let fingerprint = format!(
        "{{\"expectedRevision\":{},\"planId\":\"{}\",\"version\":1}}",
        expected_revision.get(),
        plan_id
    );
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(store_error)?;
    let existing = transaction
        .query_row(
            "SELECT request_fingerprint, response_json
             FROM idempotency_records
             WHERE principal_id=?1 AND method='packages.applyPlan' AND idempotency_key=?2",
            params![owner.as_str(), idempotency_key.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(store_error)?;
    if let Some((saved_fingerprint, response_json)) = existing {
        if saved_fingerprint != fingerprint {
            return Err(M4Error::new(M4ErrorCode::IdempotencyConflict));
        }
        let mut outcome: ApplyPlanOutcome =
            serde_json::from_str(&response_json).map_err(|_| internal())?;
        outcome.replayed = true;
        outcome.schedule = false;
        transaction.commit().map_err(store_error)?;
        return Ok(outcome);
    }

    let plan = load_plan(&transaction, owner, plan_id)?;
    if plan.state != PlanState::Unapplied {
        return Err(M4Error::with_subreason(
            M4ErrorCode::PlanStale,
            "plan_already_applied",
        ));
    }
    let current_revision = transaction
        .query_row(
            "SELECT revision FROM projects WHERE project_id=?1 AND owner_principal_id=?2",
            params![plan.project_id.to_string(), owner.as_str()],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(store_error)?
        .ok_or_else(|| M4Error::new(M4ErrorCode::ProjectNotRegistered))?;
    if current_revision != sqlite_revision(expected_revision)
        || expected_revision != plan.project_revision
    {
        return Err(M4Error::with_subreason(
            M4ErrorCode::PlanStale,
            "project_revision_changed",
        ));
    }

    let operation_id = OperationId::new();
    let request_json = format!("{{\"planId\":\"{plan_id}\",\"version\":1}}");
    transaction
        .execute(
            "INSERT INTO operations (
                operation_id, kind, state, revision, owner_principal_id, request_json,
                created_at_ms, updated_at_ms
             ) VALUES (?1, 'packages.apply', 'queued', 1, ?2, ?3, ?4, ?4)",
            params![
                operation_id.to_string(),
                owner.as_str(),
                request_json,
                created_at_ms
            ],
        )
        .map_err(store_error)?;
    transaction
        .execute(
            "UPDATE package_plans SET state='applied', apply_operation_id=?1
             WHERE plan_id=?2 AND state='unapplied' AND apply_operation_id IS NULL",
            params![operation_id.to_string(), plan_id.to_string()],
        )
        .map_err(store_error)?;
    transaction
        .execute(
            "INSERT INTO operation_journal (
                operation_id, step, kind, state, payload_json, updated_at_ms
             ) VALUES (?1, 1, 'packages.apply', 'prepared', ?2, ?3)",
            params![operation_id.to_string(), request_json, created_at_ms],
        )
        .map_err(store_error)?;
    transaction
        .execute(
            "INSERT INTO package_filesystem_journal (
                operation_id, step, plan_id, project_id, phase, state,
                project_identity_key, change_set_fingerprint, evidence_json, updated_at_ms
             ) SELECT ?1, 1, ?2, p.project_id, 'accepted', 'completed',
                      p.path_identity_key, ?3, '{}', ?4
               FROM projects p WHERE p.project_id=?5",
            params![
                operation_id.to_string(),
                plan_id.to_string(),
                plan.change_set_fingerprint.as_slice(),
                created_at_ms,
                plan.project_id.to_string()
            ],
        )
        .map_err(store_error)?;
    insert_operation_event(
        &transaction,
        owner,
        operation_id,
        "operation.created",
        "{\"state\":\"queued\"}",
        created_at_ms,
    )?;
    let saved = ApplyPlanOutcome {
        operation_id,
        replayed: false,
        schedule: true,
    };
    let response_json = serde_json::to_string(&saved).map_err(|_| internal())?;
    transaction
        .execute(
            "INSERT INTO idempotency_records (
                principal_id, method, idempotency_key, request_fingerprint, state,
                operation_id, response_json, created_at_ms
             ) VALUES (?1, 'packages.applyPlan', ?2, ?3, 'completed', ?4, ?5, ?6)",
            params![
                owner.as_str(),
                idempotency_key.as_str(),
                fingerprint,
                operation_id.to_string(),
                response_json,
                created_at_ms
            ],
        )
        .map_err(store_error)?;
    transaction.commit().map_err(store_error)?;
    Ok(saved)
}

pub(super) fn append_filesystem_journal(
    connection: &mut Connection,
    entry: FilesystemJournalEntry,
) -> Result<(), M4Error> {
    if entry.step == 0
        || entry.step > i64::MAX as u64
        || entry.project_identity_key.is_empty()
        || entry.project_identity_key.len() > 128
        || entry.evidence_json.len() > 4 * 1024 * 1024
        || serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&entry.evidence_json)
            .is_err()
    {
        return Err(M4Error::new(M4ErrorCode::InvalidInput));
    }
    connection
        .execute(
            "INSERT INTO package_filesystem_journal (
                operation_id, step, plan_id, project_id, phase, state,
                project_identity_key, change_set_fingerprint, evidence_json, updated_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                entry.operation_id.to_string(),
                sqlite_integer(entry.step)?,
                entry.plan_id.to_string(),
                entry.project_id.to_string(),
                entry.phase.as_str(),
                entry.state.as_str(),
                entry.project_identity_key,
                entry.change_set_fingerprint.as_slice(),
                entry.evidence_json,
                sqlite_integer(entry.updated_at_ms)?,
            ],
        )
        .map_err(store_error)?;
    Ok(())
}

pub(super) fn next_filesystem_journal_step(
    connection: &Connection,
    operation_id: OperationId,
) -> Result<u64, M4Error> {
    let step = next_journal_step(connection, operation_id)?;
    u64::try_from(step).map_err(|_| M4Error::new(M4ErrorCode::Internal))
}

pub(super) fn begin_package_apply(
    connection: &mut Connection,
    operation_id: OperationId,
    updated_at_ms: u64,
) -> Result<PackagePlanRecord, M4Error> {
    let updated_at_ms = sqlite_integer(updated_at_ms)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(store_error)?;
    let (owner, state, revision, plan_id) = transaction
        .query_row(
            "SELECT o.owner_principal_id, o.state, o.revision, p.plan_id
             FROM operations o JOIN package_plans p ON p.apply_operation_id=o.operation_id
             WHERE o.operation_id=?1 AND o.kind='packages.apply'",
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
        .map_err(store_error)?
        .ok_or_else(|| M4Error::new(M4ErrorCode::Internal))?;
    if !matches!(state.as_str(), "queued" | "recovering") {
        return Err(M4Error::new(M4ErrorCode::Internal));
    }
    let next_revision = revision
        .checked_add(1)
        .ok_or_else(|| M4Error::new(M4ErrorCode::Internal))?;
    transaction
        .execute(
            "UPDATE operations SET state='running', revision=?1, updated_at_ms=?2,
                    started_at_ms=coalesce(started_at_ms, ?2)
             WHERE operation_id=?3",
            params![next_revision, updated_at_ms, operation_id.to_string()],
        )
        .map_err(store_error)?;
    let owner = PrincipalId::parse(owner).map_err(|_| internal())?;
    insert_operation_event_revision(
        &transaction,
        &owner,
        operation_id,
        Revision::new(u64::try_from(next_revision).map_err(|_| internal())?)
            .ok_or_else(internal)?,
        "operation.state_changed",
        "{\"state\":\"running\"}",
        updated_at_ms,
    )?;
    let plan_id = PlanId::parse(&plan_id).map_err(|_| internal())?;
    let plan = load_plan(&transaction, &owner, plan_id)?;
    transaction.commit().map_err(store_error)?;
    Ok(plan)
}

pub(super) fn complete_package_apply(
    connection: &mut Connection,
    operation_id: OperationId,
    completion: PackageApplyCompletion,
    completed_at_ms: u64,
) -> Result<(), M4Error> {
    if completion.result_json.len() > 65_536
        || serde_json::from_str::<serde_json::Value>(&completion.result_json).is_err()
    {
        return Err(M4Error::new(M4ErrorCode::Internal));
    }
    let completed_at_ms = sqlite_integer(completed_at_ms)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(store_error)?;
    let context = load_apply_context(&transaction, operation_id)?;
    if !matches!(
        context.state.as_str(),
        "running" | "cancelling" | "recovering"
    ) {
        return Err(M4Error::new(M4ErrorCode::Internal));
    }
    let plan = load_plan(&transaction, &context.owner, context.plan_id)?;
    let project_revision = transaction
        .query_row(
            "SELECT revision FROM projects WHERE project_id=?1 AND owner_principal_id=?2",
            params![plan.project_id.to_string(), context.owner.as_str()],
            |row| row.get::<_, i64>(0),
        )
        .map_err(store_error)?;
    if project_revision != sqlite_revision(plan.project_revision) {
        return Err(M4Error::with_subreason(
            M4ErrorCode::PlanStale,
            "project_revision_changed",
        ));
    }
    let next_project_revision = project_revision
        .checked_add(1)
        .ok_or_else(|| M4Error::new(M4ErrorCode::Internal))?;
    let observation = completion.project_observation;
    let mut semantic = observation.clone();
    semantic.observed_at_ms = 0;
    let snapshot_json = serde_json::to_string(&semantic).map_err(|_| internal())?;
    transaction
        .execute(
            "UPDATE projects SET root_path=?1, path_identity_key=?2, project_type=?3,
                    unity_version=?4, unity_revision=?5, snapshot_json=?6, revision=?7,
                    observed_at_ms=?8, updated_at_ms=?9
             WHERE project_id=?10 AND owner_principal_id=?11",
            params![
                observation.root_path,
                observation.path_identity_key,
                project_type_name(observation.project_type),
                observation.unity_version,
                observation.unity_revision,
                snapshot_json,
                next_project_revision,
                sqlite_integer(observation.observed_at_ms)?,
                completed_at_ms,
                plan.project_id.to_string(),
                context.owner.as_str(),
            ],
        )
        .map_err(store_error)?;
    insert_aggregate_event(
        &transaction,
        &context.owner,
        ("project", &plan.project_id.to_string()),
        Revision::new(u64::try_from(next_project_revision).map_err(|_| internal())?)
            .ok_or_else(internal)?,
        "project.packages_changed",
        "{}",
        completed_at_ms,
    )?;
    let next_operation_revision = context
        .revision
        .checked_add(1)
        .ok_or_else(|| M4Error::new(M4ErrorCode::Internal))?;
    transaction
        .execute(
            "UPDATE operations SET state='succeeded', revision=?1, result_json=?2,
                    error_code=NULL, diagnostic_id=NULL, updated_at_ms=?3, completed_at_ms=?3
             WHERE operation_id=?4",
            params![
                next_operation_revision,
                completion.result_json,
                completed_at_ms,
                operation_id.to_string()
            ],
        )
        .map_err(store_error)?;
    transaction
        .execute(
            "UPDATE operation_journal SET state='applied', updated_at_ms=?1
             WHERE operation_id=?2 AND step=1",
            params![completed_at_ms, operation_id.to_string()],
        )
        .map_err(store_error)?;
    let next_step = next_journal_step(&transaction, operation_id)?;
    transaction
        .execute(
            "INSERT INTO package_filesystem_journal (
                operation_id, step, plan_id, project_id, phase, state,
                project_identity_key, change_set_fingerprint, evidence_json, updated_at_ms
             ) VALUES (?1, ?2, ?3, ?4, 'state_committed', 'completed', ?5, ?6, '{}', ?7)",
            params![
                operation_id.to_string(),
                next_step,
                plan.plan_id.to_string(),
                plan.project_id.to_string(),
                observation.path_identity_key,
                plan.change_set_fingerprint.as_slice(),
                completed_at_ms,
            ],
        )
        .map_err(store_error)?;
    insert_operation_event_revision(
        &transaction,
        &context.owner,
        operation_id,
        Revision::new(u64::try_from(next_operation_revision).map_err(|_| internal())?)
            .ok_or_else(internal)?,
        "operation.completed",
        "{\"state\":\"succeeded\"}",
        completed_at_ms,
    )?;
    transaction.commit().map_err(store_error)?;
    Ok(())
}

pub(super) fn fail_package_apply(
    connection: &mut Connection,
    operation_id: OperationId,
    error_code: &str,
    diagnostic_id: &str,
    completed_at_ms: u64,
) -> Result<(), M4Error> {
    if error_code.is_empty()
        || error_code.len() > 128
        || diagnostic_id.len() != 36
        || uuid::Uuid::parse_str(diagnostic_id).is_err()
    {
        return Err(M4Error::new(M4ErrorCode::Internal));
    }
    let completed_at_ms = sqlite_integer(completed_at_ms)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(store_error)?;
    let context = load_apply_context(&transaction, operation_id)?;
    if matches!(context.state.as_str(), "succeeded" | "failed" | "cancelled") {
        transaction.commit().map_err(store_error)?;
        return Ok(());
    }
    let next_revision = context
        .revision
        .checked_add(1)
        .ok_or_else(|| M4Error::new(M4ErrorCode::Internal))?;
    transaction
        .execute(
            "UPDATE operations SET state='failed', revision=?1, error_code=?2,
                    diagnostic_id=?3, updated_at_ms=?4, completed_at_ms=?4
             WHERE operation_id=?5",
            params![
                next_revision,
                error_code,
                diagnostic_id,
                completed_at_ms,
                operation_id.to_string()
            ],
        )
        .map_err(store_error)?;
    transaction
        .execute(
            "UPDATE operation_journal SET state='applied', updated_at_ms=?1
             WHERE operation_id=?2 AND step=1",
            params![completed_at_ms, operation_id.to_string()],
        )
        .map_err(store_error)?;
    insert_operation_event_revision(
        &transaction,
        &context.owner,
        operation_id,
        Revision::new(u64::try_from(next_revision).map_err(|_| internal())?)
            .ok_or_else(internal)?,
        "operation.completed",
        "{\"state\":\"failed\"}",
        completed_at_ms,
    )?;
    transaction.commit().map_err(store_error)?;
    Ok(())
}

pub(super) fn recover_package_operations(
    connection: &mut Connection,
    recovered_at_ms: u64,
) -> Result<Vec<OperationId>, M4Error> {
    let recovered_at_ms = sqlite_integer(recovered_at_ms)?;
    let candidates = {
        let mut statement = connection
            .prepare(
                "SELECT operation_id, state FROM operations
                 WHERE kind='packages.apply' AND state NOT IN ('succeeded','failed','cancelled')
                 ORDER BY created_at_ms ASC, operation_id ASC",
            )
            .map_err(store_error)?;
        statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(store_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(store_error)?
    };
    let mut schedule = Vec::new();
    for (operation_id, state) in candidates {
        let operation_id = OperationId::parse(&operation_id).map_err(|_| internal())?;
        match state.as_str() {
            "queued" => schedule.push(operation_id),
            "running" | "cancelling" | "recovering" => {
                transition_recovery(connection, operation_id, "interrupted", recovered_at_ms)?;
                transition_recovery(connection, operation_id, "recovering", recovered_at_ms)?;
                schedule.push(operation_id);
            }
            "interrupted" => {
                transition_recovery(connection, operation_id, "recovering", recovered_at_ms)?;
                schedule.push(operation_id);
            }
            _ => return Err(M4Error::new(M4ErrorCode::Internal)),
        }
    }
    Ok(schedule)
}

fn load_plan(
    connection: &Connection,
    owner: &PrincipalId,
    plan_id: PlanId,
) -> Result<PackagePlanRecord, M4Error> {
    connection
        .query_row(
            "SELECT plan_id, owner_principal_id, project_id, action, state, project_revision,
                    project_snapshot_fingerprint, change_set_fingerprint, change_set_json,
                    source_set_json, apply_operation_id, created_at_ms
             FROM package_plans WHERE plan_id=?1 AND owner_principal_id=?2",
            params![plan_id.to_string(), owner.as_str()],
            decode_plan,
        )
        .optional()
        .map_err(store_error)?
        .ok_or_else(|| M4Error::new(M4ErrorCode::PlanNotFound))
}

struct ApplyContext {
    owner: PrincipalId,
    state: String,
    revision: i64,
    plan_id: PlanId,
}

fn load_apply_context(
    connection: &Connection,
    operation_id: OperationId,
) -> Result<ApplyContext, M4Error> {
    connection
        .query_row(
            "SELECT o.owner_principal_id, o.state, o.revision, p.plan_id
             FROM operations o JOIN package_plans p ON p.apply_operation_id=o.operation_id
             WHERE o.operation_id=?1 AND o.kind='packages.apply'",
            [operation_id.to_string()],
            |row| {
                Ok(ApplyContext {
                    owner: PrincipalId::parse(row.get::<_, String>(0)?)
                        .map_err(|_| invalid_query())?,
                    state: row.get(1)?,
                    revision: row.get(2)?,
                    plan_id: PlanId::parse(&row.get::<_, String>(3)?)
                        .map_err(|_| invalid_query())?,
                })
            },
        )
        .optional()
        .map_err(store_error)?
        .ok_or_else(|| M4Error::new(M4ErrorCode::Internal))
}

fn next_journal_step(connection: &Connection, operation_id: OperationId) -> Result<i64, M4Error> {
    connection
        .query_row(
            "SELECT coalesce(max(step), 0) + 1 FROM package_filesystem_journal
             WHERE operation_id=?1",
            [operation_id.to_string()],
            |row| row.get(0),
        )
        .map_err(store_error)
}

fn transition_recovery(
    connection: &mut Connection,
    operation_id: OperationId,
    state: &str,
    recovered_at_ms: i64,
) -> Result<(), M4Error> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(store_error)?;
    let context = load_apply_context(&transaction, operation_id)?;
    let next_revision = context
        .revision
        .checked_add(1)
        .ok_or_else(|| M4Error::new(M4ErrorCode::Internal))?;
    transaction
        .execute(
            "UPDATE operations SET state=?1, revision=?2, updated_at_ms=?3
             WHERE operation_id=?4",
            params![
                state,
                next_revision,
                recovered_at_ms,
                operation_id.to_string()
            ],
        )
        .map_err(store_error)?;
    insert_operation_event_revision(
        &transaction,
        &context.owner,
        operation_id,
        Revision::new(u64::try_from(next_revision).map_err(|_| internal())?)
            .ok_or_else(internal)?,
        "operation.state_changed",
        &format!("{{\"state\":\"{state}\"}}"),
        recovered_at_ms,
    )?;
    transaction.commit().map_err(store_error)?;
    Ok(())
}

fn decode_plan(row: &Row<'_>) -> rusqlite::Result<PackagePlanRecord> {
    let plan_id = PlanId::parse(&row.get::<_, String>(0)?).map_err(|_| invalid_query())?;
    let owner = PrincipalId::parse(row.get::<_, String>(1)?).map_err(|_| invalid_query())?;
    let project_id = alcomd_application::ProjectId::parse(&row.get::<_, String>(2)?)
        .map_err(|_| invalid_query())?;
    let action = parse_action(&row.get::<_, String>(3)?)?;
    let state = parse_plan_state(&row.get::<_, String>(4)?)?;
    let project_revision = positive_revision(row.get(5)?)?;
    let project_snapshot_fingerprint = digest(row.get(6)?)?;
    let change_set_fingerprint = digest(row.get(7)?)?;
    let change_set: PackageChangeSet =
        serde_json::from_str(&row.get::<_, String>(8)?).map_err(|_| invalid_query())?;
    change_set.validate_bounds().map_err(|_| invalid_query())?;
    let source_set: Vec<PackageSourcePin> =
        serde_json::from_str(&row.get::<_, String>(9)?).map_err(|_| invalid_query())?;
    let apply_operation_id = row
        .get::<_, Option<String>>(10)?
        .map(|value| OperationId::parse(&value).map_err(|_| invalid_query()))
        .transpose()?;
    let created_at_ms = nonnegative(row.get(11)?)?;
    Ok(PackagePlanRecord {
        plan_id,
        owner,
        project_id,
        action,
        state,
        project_revision,
        project_snapshot_fingerprint,
        change_set_fingerprint,
        change_set,
        source_set,
        apply_operation_id,
        created_at_ms,
    })
}

fn insert_operation_event(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    operation_id: OperationId,
    kind: &str,
    payload: &str,
    occurred_at_ms: i64,
) -> Result<(), M4Error> {
    insert_operation_event_revision(
        transaction,
        owner,
        operation_id,
        Revision::INITIAL,
        kind,
        payload,
        occurred_at_ms,
    )
}

fn insert_operation_event_revision(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    operation_id: OperationId,
    revision: Revision,
    kind: &str,
    payload: &str,
    occurred_at_ms: i64,
) -> Result<(), M4Error> {
    insert_aggregate_event(
        transaction,
        owner,
        ("operation", &operation_id.to_string()),
        revision,
        kind,
        payload,
        occurred_at_ms,
    )
}

fn insert_aggregate_event(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    aggregate: (&str, &str),
    revision: Revision,
    kind: &str,
    payload: &str,
    occurred_at_ms: i64,
) -> Result<(), M4Error> {
    transaction
        .execute(
            "INSERT INTO events (
                event_id, kind, principal_id, aggregate_kind, aggregate_id,
                aggregate_revision, payload_json, occurred_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                uuid::Uuid::new_v4().to_string(),
                kind,
                owner.as_str(),
                aggregate.0,
                aggregate.1,
                sqlite_revision(revision),
                payload,
                occurred_at_ms
            ],
        )
        .map_err(store_error)?;
    Ok(())
}

fn project_type_name(value: alcomd_application::ProjectType) -> &'static str {
    use alcomd_application::ProjectType;
    match value {
        ProjectType::Avatars => "avatars",
        ProjectType::Worlds => "worlds",
        ProjectType::VpmStarter => "vpm-starter",
        ProjectType::UpmAvatars => "upm-avatars",
        ProjectType::UpmWorlds => "upm-worlds",
        ProjectType::UpmStarter => "upm-starter",
        ProjectType::LegacySdk2 => "legacy-sdk2",
        ProjectType::LegacyWorlds => "legacy-worlds",
        ProjectType::LegacyAvatars => "legacy-avatars",
        ProjectType::Unknown => "unknown",
    }
}

fn parse_action(value: &str) -> rusqlite::Result<PlanAction> {
    match value {
        "install" => Ok(PlanAction::Install),
        "remove" => Ok(PlanAction::Remove),
        "upgrade" => Ok(PlanAction::Upgrade),
        "downgrade" => Ok(PlanAction::Downgrade),
        "resolve" => Ok(PlanAction::Resolve),
        _ => Err(invalid_query()),
    }
}

fn parse_plan_state(value: &str) -> rusqlite::Result<PlanState> {
    match value {
        "unapplied" => Ok(PlanState::Unapplied),
        "applied" => Ok(PlanState::Applied),
        _ => Err(invalid_query()),
    }
}

fn digest(value: Vec<u8>) -> rusqlite::Result<[u8; 32]> {
    value.try_into().map_err(|_| invalid_query())
}

fn bytes_hex(value: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(value.len() * 2);
    for byte in value {
        result.push(char::from(HEX[(byte >> 4) as usize]));
        result.push(char::from(HEX[(byte & 0x0f) as usize]));
    }
    result
}

fn positive_revision(value: i64) -> rusqlite::Result<Revision> {
    u64::try_from(value)
        .ok()
        .and_then(Revision::new)
        .ok_or_else(invalid_query)
}

fn nonnegative(value: i64) -> rusqlite::Result<u64> {
    u64::try_from(value).map_err(|_| invalid_query())
}

fn sqlite_integer(value: u64) -> Result<i64, M4Error> {
    i64::try_from(value).map_err(|_| M4Error::new(M4ErrorCode::InvalidInput))
}

fn sqlite_revision(value: Revision) -> i64 {
    i64::try_from(value.get()).expect("validated Revision is SQLite-bounded")
}

fn invalid_query() -> rusqlite::Error {
    rusqlite::Error::InvalidQuery
}

fn store_error(_: rusqlite::Error) -> M4Error {
    M4Error::new(M4ErrorCode::StoreUnavailable)
}

fn internal() -> M4Error {
    M4Error::new(M4ErrorCode::Internal)
}
