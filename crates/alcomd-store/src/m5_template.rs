use alcomd_application::{
    CreatedTemplateProject, IdempotencyKey, M5TemplateError, M5TemplateErrorCode, OperationId,
    PlanId, PrincipalId, PublishedTemplate, Revision, StoredTemplateRecord, TemplateApplyOutcome,
    TemplateCursor, TemplateId, TemplatePlanDraft, TemplatePlanKind, TemplatePlanRecord,
    TemplatePlanState, TemplateSourceKind,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub(super) fn ensure_builtin_templates(
    connection: &mut Connection,
    owner: &PrincipalId,
    templates: Vec<StoredTemplateRecord>,
    now_ms: u64,
) -> Result<(), M5TemplateError> {
    let transaction = transaction(connection)?;
    for template in templates {
        if template.source_kind != TemplateSourceKind::Builtin
            || !template.payload_locator.starts_with("builtin:")
        {
            return Err(error(M5TemplateErrorCode::Internal));
        }
        let existing = load_template_optional(&transaction, owner, template.template_id)?;
        if let Some(existing) = existing {
            if existing.source_kind != TemplateSourceKind::Builtin
                || existing.template_version != template.template_version
                || existing.manifest_json != template.manifest_json
                || existing.payload_locator != template.payload_locator
                || existing.bundle_sha256 != template.bundle_sha256
            {
                return Err(error(M5TemplateErrorCode::TemplateImmutable));
            }
            continue;
        }
        transaction
            .execute(
                "INSERT INTO templates (
                    template_id, owner_principal_id, source_kind, template_version,
                    manifest_json, payload_locator, payload_sha256, favorite, revision,
                    created_at_ms, updated_at_ms
                 ) VALUES (?1,?2,'builtin',?3,?4,?5,?6,0,1,?7,?7)",
                params![
                    template.template_id.to_string(),
                    owner.as_str(),
                    template.template_version,
                    template.manifest_json,
                    template.payload_locator,
                    template.bundle_sha256.as_slice(),
                    integer(now_ms)?,
                ],
            )
            .map_err(failure)?;
        insert_event(
            &transaction,
            owner,
            "template.builtin_registered",
            template.template_id,
            Revision::INITIAL,
            now_ms,
        )?;
    }
    transaction.commit().map_err(failure)
}

pub(super) fn list_templates(
    connection: &Connection,
    owner: &PrincipalId,
    cursor: Option<TemplateCursor>,
    limit: u32,
) -> Result<Vec<StoredTemplateRecord>, M5TemplateError> {
    let (cursor_time, cursor_id) = cursor.map_or((i64::MAX, "~".to_owned()), |cursor| {
        (
            i64::try_from(cursor.updated_at_ms).unwrap_or(i64::MAX),
            cursor.template_id.to_string(),
        )
    });
    let mut statement = connection
        .prepare(
            "SELECT template_id,source_kind,template_version,manifest_json,payload_locator,
                    payload_sha256,favorite,revision,created_at_ms,updated_at_ms
             FROM templates
             WHERE owner_principal_id=?1
               AND (updated_at_ms<?2 OR (updated_at_ms=?2 AND template_id<?3))
             ORDER BY updated_at_ms DESC,template_id DESC LIMIT ?4",
        )
        .map_err(failure)?;
    statement
        .query_map(
            params![owner.as_str(), cursor_time, cursor_id, i64::from(limit) + 1],
            decode_template,
        )
        .map_err(failure)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(failure)
}

pub(super) fn get_template(
    connection: &Connection,
    owner: &PrincipalId,
    template_id: TemplateId,
) -> Result<StoredTemplateRecord, M5TemplateError> {
    load_template_optional(connection, owner, template_id)?
        .ok_or_else(|| error(M5TemplateErrorCode::TemplateNotFound))
}

pub(super) fn create_template_plan(
    connection: &mut Connection,
    owner: &PrincipalId,
    draft: TemplatePlanDraft,
    created_at_ms: u64,
) -> Result<TemplatePlanRecord, M5TemplateError> {
    if draft.plan_json.len() > 4 * 1024 * 1024
        || serde_json::from_str::<serde_json::Value>(&draft.plan_json).is_err()
    {
        return Err(error(M5TemplateErrorCode::InvalidInput));
    }
    let plan_id = PlanId::new();
    connection
        .execute(
            "INSERT INTO template_plans (
                plan_id,owner_principal_id,kind,state,plan_fingerprint,plan_json,created_at_ms
             ) VALUES (?1,?2,?3,'unapplied',?4,?5,?6)",
            params![
                plan_id.to_string(),
                owner.as_str(),
                plan_kind(draft.kind),
                draft.plan_fingerprint.as_slice(),
                draft.plan_json,
                integer(created_at_ms)?,
            ],
        )
        .map_err(failure)?;
    load_plan(connection, owner, plan_id)
}

pub(super) fn get_template_plan(
    connection: &Connection,
    owner: &PrincipalId,
    plan_id: PlanId,
) -> Result<TemplatePlanRecord, M5TemplateError> {
    load_plan(connection, owner, plan_id)
}

pub(super) fn accept_template_plan(
    connection: &mut Connection,
    owner: &PrincipalId,
    plan_id: PlanId,
    key: &IdempotencyKey,
    created_at_ms: u64,
) -> Result<TemplateApplyOutcome, M5TemplateError> {
    let fingerprint = format!(r#"{{"planId":"{plan_id}","version":1}}"#);
    let transaction = transaction(connection)?;
    if let Some((saved, response)) = transaction
        .query_row(
            "SELECT request_fingerprint,response_json FROM idempotency_records
             WHERE principal_id=?1 AND method='templates.applyPlan' AND idempotency_key=?2",
            params![owner.as_str(), key.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(failure)?
    {
        if saved != fingerprint {
            return Err(error(M5TemplateErrorCode::TemplateConflict));
        }
        let mut outcome: TemplateApplyOutcome =
            serde_json::from_str(&response).map_err(|_| internal())?;
        outcome.replayed = true;
        outcome.schedule = false;
        transaction.commit().map_err(failure)?;
        return Ok(outcome);
    }
    let plan = load_plan(&transaction, owner, plan_id)?;
    if plan.state != TemplatePlanState::Unapplied {
        return Err(error(M5TemplateErrorCode::TemplatePlanStale));
    }
    let operation_id = OperationId::new();
    let request_json = format!(r#"{{"planId":"{plan_id}","version":1}}"#);
    transaction
        .execute(
            "INSERT INTO operations (
                operation_id,kind,state,revision,owner_principal_id,request_json,
                created_at_ms,updated_at_ms
             ) VALUES (?1,?2,'queued',1,?3,?4,?5,?5)",
            params![
                operation_id.to_string(),
                operation_kind(plan.kind),
                owner.as_str(),
                request_json,
                integer(created_at_ms)?,
            ],
        )
        .map_err(failure)?;
    transaction
        .execute(
            "UPDATE template_plans SET state='applied',apply_operation_id=?1
             WHERE plan_id=?2 AND state='unapplied' AND apply_operation_id IS NULL",
            params![operation_id.to_string(), plan_id.to_string()],
        )
        .map_err(failure)?;
    transaction
        .execute(
            "INSERT INTO operation_journal (
                operation_id,step,kind,state,payload_json,updated_at_ms
             ) VALUES (?1,1,?2,'prepared',?3,?4)",
            params![
                operation_id.to_string(),
                operation_kind(plan.kind),
                request_json,
                integer(created_at_ms)?,
            ],
        )
        .map_err(failure)?;
    let outcome = TemplateApplyOutcome {
        operation_id,
        replayed: false,
        schedule: true,
    };
    transaction
        .execute(
            "INSERT INTO idempotency_records (
                principal_id,method,idempotency_key,request_fingerprint,state,
                operation_id,response_json,created_at_ms
             ) VALUES (?1,'templates.applyPlan',?2,?3,'completed',?4,?5,?6)",
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
    insert_operation_event(
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

pub(super) fn begin_template_apply(
    connection: &mut Connection,
    operation_id: OperationId,
    updated_at_ms: u64,
) -> Result<TemplatePlanRecord, M5TemplateError> {
    let transaction = transaction(connection)?;
    let (owner, state, revision, plan_id) = load_operation_context(&transaction, operation_id)?;
    if !matches!(state.as_str(), "queued" | "recovering") {
        return Err(internal());
    }
    let next = revision.checked_add(1).ok_or_else(internal)?;
    transaction
        .execute(
            "UPDATE operations SET state='running',revision=?1,updated_at_ms=?2,
                    started_at_ms=coalesce(started_at_ms,?2) WHERE operation_id=?3",
            params![next, integer(updated_at_ms)?, operation_id.to_string()],
        )
        .map_err(failure)?;
    insert_operation_event(
        &transaction,
        &owner,
        operation_id,
        revision_value(next)?,
        "operation.state_changed",
        updated_at_ms,
    )?;
    let plan = load_plan(&transaction, &owner, plan_id)?;
    transaction.commit().map_err(failure)?;
    Ok(plan)
}

pub(super) fn complete_template_apply(
    connection: &mut Connection,
    operation_id: OperationId,
    published: PublishedTemplate,
    completed_at_ms: u64,
) -> Result<(), M5TemplateError> {
    let transaction = transaction(connection)?;
    let (owner, state, operation_revision, plan_id) =
        load_operation_context(&transaction, operation_id)?;
    if !matches!(state.as_str(), "running" | "recovering" | "cancelling") {
        return Err(internal());
    }
    let plan = load_plan(&transaction, &owner, plan_id)?;
    let (revision, event_kind) = match plan.kind {
        TemplatePlanKind::Import => {
            let authority: ImportAuthority =
                serde_json::from_str(&plan.plan_json).map_err(|_| internal())?;
            if authority.version != 1
                || authority.kind != "import"
                || authority.template_id != published.record.template_id.to_string()
                || authority.new_bundle_sha256 != hex(&published.record.bundle_sha256)
            {
                return Err(error(M5TemplateErrorCode::TemplatePlanStale));
            }
            let existing =
                load_template_optional(&transaction, &owner, published.record.template_id)?;
            let revision = match existing {
                None if authority.expected_revision == 0
                    && authority.old_bundle_sha256.is_none() =>
                {
                    insert_user_template(&transaction, &owner, &published.record, completed_at_ms)?;
                    Revision::INITIAL
                }
                Some(existing)
                    if existing.source_kind == TemplateSourceKind::User
                        && authority.expected_revision == existing.revision.get()
                        && authority.old_bundle_sha256.as_deref()
                            == Some(&hex(&existing.bundle_sha256)) =>
                {
                    if existing.bundle_sha256 == published.record.bundle_sha256 {
                        existing.revision
                    } else {
                        let next = existing.revision.checked_next().ok_or_else(internal)?;
                        transaction
                            .execute(
                                "UPDATE templates SET template_version=?1,manifest_json=?2,
                                        payload_locator=?3,payload_sha256=?4,revision=?5,updated_at_ms=?6
                                 WHERE template_id=?7 AND owner_principal_id=?8 AND revision=?9",
                                params![
                                    published.record.template_version,
                                    published.record.manifest_json,
                                    published.record.payload_locator,
                                    published.record.bundle_sha256.as_slice(),
                                    integer(next.get())?,
                                    integer(completed_at_ms)?,
                                    published.record.template_id.to_string(),
                                    owner.as_str(),
                                    integer(existing.revision.get())?,
                                ],
                            )
                            .map_err(failure)?;
                        next
                    }
                }
                _ => return Err(error(M5TemplateErrorCode::TemplatePlanStale)),
            };
            (revision, "template.imported")
        }
        TemplatePlanKind::Derive => {
            let authority: DeriveAuthority =
                serde_json::from_str(&plan.plan_json).map_err(|_| internal())?;
            if authority.version != 1
                || authority.kind != "derive"
                || authority.template_id != published.record.template_id.to_string()
                || load_template_optional(&transaction, &owner, published.record.template_id)?
                    .is_some()
            {
                return Err(error(M5TemplateErrorCode::TemplatePlanStale));
            }
            let project_revision = transaction
                .query_row(
                    "SELECT revision FROM projects WHERE project_id=?1 AND owner_principal_id=?2",
                    params![authority.source_project_id, owner.as_str()],
                    |row| row.get::<_, i64>(0),
                )
                .optional()
                .map_err(failure)?;
            if project_revision != Some(integer(authority.source_project_revision)?) {
                return Err(error(M5TemplateErrorCode::TemplatePlanStale));
            }
            insert_user_template(&transaction, &owner, &published.record, completed_at_ms)?;
            (Revision::INITIAL, "template.derived")
        }
        TemplatePlanKind::CreateProject => return Err(internal()),
    };
    insert_event(
        &transaction,
        &owner,
        event_kind,
        published.record.template_id,
        revision,
        completed_at_ms,
    )?;
    finish_operation_success(
        &transaction,
        &owner,
        operation_id,
        operation_revision,
        &format!(
            r#"{{"revision":{},"templateId":"{}"}}"#,
            revision.get(),
            published.record.template_id
        ),
        completed_at_ms,
    )?;
    transaction.commit().map_err(failure)
}

#[cfg(feature = "test-kill-gates")]
fn test_template_kill_gate(checkpoint: &str) -> Result<(), M5TemplateError> {
    if std::env::var("ALCOMD_TEST_M5_TEMPLATE_KILL_GATE").as_deref() != Ok(checkpoint) {
        return Ok(());
    }
    let signal = std::env::var_os("ALCOMD_TEST_M5_TEMPLATE_KILL_SIGNAL").ok_or_else(internal)?;
    let path = std::path::PathBuf::from(signal);
    std::fs::write(&path, checkpoint.as_bytes()).map_err(|_| internal())?;
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .map_err(|_| internal())?;
    file.sync_all().map_err(|_| internal())?;
    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}

#[cfg(not(feature = "test-kill-gates"))]
fn test_template_kill_gate(_checkpoint: &str) -> Result<(), M5TemplateError> {
    Ok(())
}

pub(super) fn record_template_checkpoint(
    connection: &mut Connection,
    operation_id: OperationId,
    step: u64,
    phase: &str,
    updated_at_ms: u64,
) -> Result<(), M5TemplateError> {
    let expected_step = match phase {
        "staging_complete" => 2,
        "target_publish_intent" => 3,
        "target_published" => 4,
        "project_registry_commit_intent" => 5,
        _ => return Err(internal()),
    };
    if step != expected_step {
        return Err(internal());
    }
    let (_, state, _, _) = load_operation_context(connection, operation_id)?;
    if !matches!(state.as_str(), "running" | "recovering" | "cancelling") {
        return Err(internal());
    }
    connection
        .execute(
            "INSERT INTO operation_journal (
                operation_id,step,kind,state,payload_json,updated_at_ms
             ) VALUES (?1,?2,'templates.create-project','prepared',?3,?4)
             ON CONFLICT(operation_id,step) DO UPDATE SET
                payload_json=excluded.payload_json,updated_at_ms=excluded.updated_at_ms
             WHERE operation_journal.kind='templates.create-project'",
            params![
                operation_id.to_string(),
                integer(step)?,
                format!(r#"{{"phase":"{phase}"}}"#),
                integer(updated_at_ms)?,
            ],
        )
        .map_err(failure)?;
    Ok(())
}

pub(super) fn complete_template_project_create(
    connection: &mut Connection,
    operation_id: OperationId,
    project: CreatedTemplateProject,
    completed_at_ms: u64,
) -> Result<(), M5TemplateError> {
    let transaction = transaction(connection)?;
    let (owner, state, operation_revision, plan_id) =
        load_operation_context(&transaction, operation_id)?;
    if !matches!(state.as_str(), "running" | "recovering" | "cancelling") {
        return Err(internal());
    }
    let plan = load_plan(&transaction, &owner, plan_id)?;
    if plan.kind != TemplatePlanKind::CreateProject {
        return Err(internal());
    }
    let authority: CreateProjectAuthority =
        serde_json::from_str(&plan.plan_json).map_err(|_| internal())?;
    if authority.version != 1
        || authority.kind != "create-project"
        || authority.project_id != project.project_id.to_string()
        || authority.target_path != project.observation.root_path
    {
        return Err(error(M5TemplateErrorCode::TemplatePlanStale));
    }
    let template_revision = transaction
        .query_row(
            "SELECT revision,payload_sha256 FROM templates
             WHERE template_id=?1 AND owner_principal_id=?2",
            params![authority.template_id, owner.as_str()],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?)),
        )
        .optional()
        .map_err(failure)?;
    if template_revision
        != Some((
            integer(authority.template_revision)?,
            parse_hex_digest(&authority.bundle_sha256)?.to_vec(),
        ))
    {
        return Err(error(M5TemplateErrorCode::TemplatePlanStale));
    }
    let existing = transaction
        .query_row(
            "SELECT project_id,root_path,path_identity_key FROM projects
             WHERE project_id=?1 OR path_identity_key=?2",
            params![
                project.project_id.to_string(),
                project.observation.path_identity_key
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
        Some((id, root, identity))
            if id == project.project_id.to_string()
                && root == project.observation.root_path
                && identity == project.observation.path_identity_key => {}
        Some(_) => return Err(error(M5TemplateErrorCode::TemplateConflict)),
        None => {
            let semantic = super::m3::semantic_project(project.observation.clone());
            transaction
                .execute(
                    "INSERT INTO projects (
                        project_id,owner_principal_id,root_path,path_identity_key,project_type,
                        unity_version,unity_revision,snapshot_json,revision,registered_at_ms,
                        observed_at_ms,updated_at_ms
                     ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,1,?9,?10,?9)",
                    params![
                        project.project_id.to_string(),
                        owner.as_str(),
                        project.observation.root_path,
                        project.observation.path_identity_key,
                        super::m3::project_type(project.observation.project_type),
                        project.observation.unity_version,
                        project.observation.unity_revision,
                        serde_json::to_string(&semantic).map_err(|_| internal())?,
                        integer(completed_at_ms)?,
                        integer(project.observation.observed_at_ms)?,
                    ],
                )
                .map_err(failure)?;
            transaction
                .execute(
                    "INSERT INTO events (
                        event_id,kind,aggregate_kind,aggregate_id,aggregate_revision,
                        principal_id,occurred_at_ms,payload_json
                     ) VALUES (?1,'project.registered','project',?2,1,?3,?4,'{}')",
                    params![
                        Uuid::new_v4().to_string(),
                        project.project_id.to_string(),
                        owner.as_str(),
                        integer(completed_at_ms)?,
                    ],
                )
                .map_err(failure)?;
        }
    }
    finish_operation_success(
        &transaction,
        &owner,
        operation_id,
        operation_revision,
        &format!(r#"{{"projectId":"{}","revision":1}}"#, project.project_id),
        completed_at_ms,
    )?;
    test_template_kill_gate("project_registry_commit_intent")?;
    transaction.commit().map_err(failure)?;
    test_template_kill_gate("state_committed")
}

fn insert_user_template(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    record: &StoredTemplateRecord,
    now_ms: u64,
) -> Result<(), M5TemplateError> {
    transaction
        .execute(
            "INSERT INTO templates (
                template_id,owner_principal_id,source_kind,template_version,
                manifest_json,payload_locator,payload_sha256,favorite,revision,
                created_at_ms,updated_at_ms
             ) VALUES (?1,?2,'user',?3,?4,?5,?6,0,1,?7,?7)",
            params![
                record.template_id.to_string(),
                owner.as_str(),
                record.template_version,
                record.manifest_json,
                record.payload_locator,
                record.bundle_sha256.as_slice(),
                integer(now_ms)?,
            ],
        )
        .map_err(failure)?;
    Ok(())
}

pub(super) fn fail_template_apply(
    connection: &mut Connection,
    operation_id: OperationId,
    error_code: &str,
    diagnostic_id: &str,
    completed_at_ms: u64,
) -> Result<(), M5TemplateError> {
    if error_code.is_empty() || error_code.len() > 128 || Uuid::parse_str(diagnostic_id).is_err() {
        return Err(internal());
    }
    let transaction = transaction(connection)?;
    let (owner, state, revision, _) = load_operation_context(&transaction, operation_id)?;
    if matches!(state.as_str(), "succeeded" | "failed" | "cancelled") {
        transaction.commit().map_err(failure)?;
        return Ok(());
    }
    let next = revision.checked_add(1).ok_or_else(internal)?;
    transaction
        .execute(
            "UPDATE operations SET state='failed',revision=?1,error_code=?2,diagnostic_id=?3,
                    updated_at_ms=?4,completed_at_ms=?4 WHERE operation_id=?5",
            params![
                next,
                error_code,
                diagnostic_id,
                integer(completed_at_ms)?,
                operation_id.to_string(),
            ],
        )
        .map_err(failure)?;
    transaction
        .execute(
            "UPDATE operation_journal SET state='applied',updated_at_ms=?1
             WHERE operation_id=?2",
            params![integer(completed_at_ms)?, operation_id.to_string()],
        )
        .map_err(failure)?;
    insert_operation_event(
        &transaction,
        &owner,
        operation_id,
        revision_value(next)?,
        "operation.completed",
        completed_at_ms,
    )?;
    transaction.commit().map_err(failure)
}

pub(super) fn recover_template_operations(
    connection: &mut Connection,
    recovered_at_ms: u64,
) -> Result<Vec<OperationId>, M5TemplateError> {
    let candidates = {
        let mut statement = connection
            .prepare(
                "SELECT operation_id,state,revision,owner_principal_id FROM operations
                 WHERE kind IN ('templates.import','templates.derive','templates.create-project')
                   AND state NOT IN ('succeeded','failed','cancelled')
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
        let owner = PrincipalId::parse(owner).map_err(|_| internal())?;
        if state == "queued" {
            result.push(id);
            continue;
        }
        if !matches!(
            state.as_str(),
            "running" | "cancelling" | "recovering" | "interrupted"
        ) {
            return Err(internal());
        }
        let next = revision.checked_add(1).ok_or_else(internal)?;
        connection
            .execute(
                "UPDATE operations SET state='recovering',revision=?1,updated_at_ms=?2
                 WHERE operation_id=?3",
                params![next, integer(recovered_at_ms)?, id.to_string()],
            )
            .map_err(failure)?;
        insert_operation_event(
            connection,
            &owner,
            id,
            revision_value(next)?,
            "operation.state_changed",
            recovered_at_ms,
        )?;
        result.push(id);
    }
    Ok(result)
}

pub(super) fn set_template_favorite(
    connection: &mut Connection,
    owner: &PrincipalId,
    template_id: TemplateId,
    favorite: bool,
    expected: Revision,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<(StoredTemplateRecord, bool), M5TemplateError> {
    let fingerprint = format!(
        r#"{{"expectedRevision":{},"favorite":{},"templateId":"{}","version":1}}"#,
        expected.get(),
        favorite,
        template_id
    );
    let transaction = transaction(connection)?;
    if let Some(record) = replay::<StoredTemplateRecord>(
        &transaction,
        owner,
        "templates.setFavorite",
        key,
        &fingerprint,
    )? {
        transaction.commit().map_err(failure)?;
        return Ok((record, true));
    }
    let current = get_template(&transaction, owner, template_id)?;
    if current.revision != expected {
        return Err(error(M5TemplateErrorCode::TemplateRevisionConflict));
    }
    let next = expected.checked_next().ok_or_else(internal)?;
    transaction
        .execute(
            "UPDATE templates SET favorite=?1,revision=?2,updated_at_ms=?3
             WHERE template_id=?4 AND owner_principal_id=?5 AND revision=?6",
            params![
                i64::from(favorite),
                integer(next.get())?,
                integer(now_ms)?,
                template_id.to_string(),
                owner.as_str(),
                integer(expected.get())?,
            ],
        )
        .map_err(failure)?;
    let record = get_template(&transaction, owner, template_id)?;
    insert_event(
        &transaction,
        owner,
        "template.favorite_changed",
        template_id,
        next,
        now_ms,
    )?;
    save_response(
        &transaction,
        owner,
        "templates.setFavorite",
        key,
        &fingerprint,
        &record,
        now_ms,
    )?;
    transaction.commit().map_err(failure)?;
    Ok((record, false))
}

pub(super) fn remove_template(
    connection: &mut Connection,
    owner: &PrincipalId,
    template_id: TemplateId,
    expected: Revision,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<(bool, bool), M5TemplateError> {
    let fingerprint = format!(
        r#"{{"expectedRevision":{},"templateId":"{}","version":1}}"#,
        expected.get(),
        template_id
    );
    let transaction = transaction(connection)?;
    if let Some(removed) =
        replay::<bool>(&transaction, owner, "templates.remove", key, &fingerprint)?
    {
        transaction.commit().map_err(failure)?;
        return Ok((removed, true));
    }
    let current = get_template(&transaction, owner, template_id)?;
    if current.source_kind == TemplateSourceKind::Builtin {
        return Err(error(M5TemplateErrorCode::TemplateImmutable));
    }
    if current.revision != expected {
        return Err(error(M5TemplateErrorCode::TemplateRevisionConflict));
    }
    transaction
        .execute(
            "DELETE FROM templates WHERE template_id=?1 AND owner_principal_id=?2 AND revision=?3",
            params![
                template_id.to_string(),
                owner.as_str(),
                integer(expected.get())?
            ],
        )
        .map_err(failure)?;
    let next = expected.checked_next().ok_or_else(internal)?;
    insert_event(
        &transaction,
        owner,
        "template.removed",
        template_id,
        next,
        now_ms,
    )?;
    save_response(
        &transaction,
        owner,
        "templates.remove",
        key,
        &fingerprint,
        &true,
        now_ms,
    )?;
    transaction.commit().map_err(failure)?;
    Ok((true, false))
}

fn load_template_optional(
    connection: &Connection,
    owner: &PrincipalId,
    id: TemplateId,
) -> Result<Option<StoredTemplateRecord>, M5TemplateError> {
    connection
        .query_row(
            "SELECT template_id,source_kind,template_version,manifest_json,payload_locator,
                    payload_sha256,favorite,revision,created_at_ms,updated_at_ms
             FROM templates WHERE template_id=?1 AND owner_principal_id=?2",
            params![id.to_string(), owner.as_str()],
            decode_template,
        )
        .optional()
        .map_err(failure)
}

fn decode_template(row: &Row<'_>) -> rusqlite::Result<StoredTemplateRecord> {
    let id =
        TemplateId::parse(&row.get::<_, String>(0)?).map_err(|_| rusqlite::Error::InvalidQuery)?;
    let source = match row.get::<_, String>(1)?.as_str() {
        "builtin" => TemplateSourceKind::Builtin,
        "user" => TemplateSourceKind::User,
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    let digest = digest(row.get::<_, Vec<u8>>(5)?)?;
    Ok(StoredTemplateRecord {
        template_id: id,
        source_kind: source,
        template_version: row.get(2)?,
        manifest_json: row.get(3)?,
        payload_locator: row.get(4)?,
        bundle_sha256: digest,
        favorite: row.get::<_, i64>(6)? != 0,
        revision: revision(row.get(7)?)?,
        created_at_ms: unsigned(row.get(8)?)?,
        updated_at_ms: unsigned(row.get(9)?)?,
    })
}

fn load_plan(
    connection: &Connection,
    owner: &PrincipalId,
    id: PlanId,
) -> Result<TemplatePlanRecord, M5TemplateError> {
    connection
        .query_row(
            "SELECT plan_id,owner_principal_id,kind,state,plan_fingerprint,plan_json,
                    apply_operation_id,created_at_ms
             FROM template_plans WHERE plan_id=?1 AND owner_principal_id=?2",
            params![id.to_string(), owner.as_str()],
            |row| {
                let id = PlanId::parse(&row.get::<_, String>(0)?)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?;
                let owner = PrincipalId::parse(row.get::<_, String>(1)?)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?;
                let kind = parse_plan_kind(&row.get::<_, String>(2)?)?;
                let state = match row.get::<_, String>(3)?.as_str() {
                    "unapplied" => TemplatePlanState::Unapplied,
                    "applied" => TemplatePlanState::Applied,
                    _ => return Err(rusqlite::Error::InvalidQuery),
                };
                let apply = row
                    .get::<_, Option<String>>(6)?
                    .map(|value| {
                        OperationId::parse(&value).map_err(|_| rusqlite::Error::InvalidQuery)
                    })
                    .transpose()?;
                Ok(TemplatePlanRecord {
                    plan_id: id,
                    owner,
                    kind,
                    state,
                    plan_fingerprint: digest(row.get::<_, Vec<u8>>(4)?)?,
                    plan_json: row.get(5)?,
                    apply_operation_id: apply,
                    created_at_ms: unsigned(row.get(7)?)?,
                })
            },
        )
        .optional()
        .map_err(failure)?
        .ok_or_else(|| error(M5TemplateErrorCode::TemplatePlanNotFound))
}

fn load_operation_context(
    connection: &Connection,
    operation_id: OperationId,
) -> Result<(PrincipalId, String, i64, PlanId), M5TemplateError> {
    connection
        .query_row(
            "SELECT o.owner_principal_id,o.state,o.revision,p.plan_id
             FROM operations o JOIN template_plans p ON p.apply_operation_id=o.operation_id
             WHERE o.operation_id=?1",
            [operation_id.to_string()],
            |row| {
                Ok((
                    PrincipalId::parse(row.get::<_, String>(0)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    row.get(1)?,
                    row.get(2)?,
                    PlanId::parse(&row.get::<_, String>(3)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                ))
            },
        )
        .optional()
        .map_err(failure)?
        .ok_or_else(internal)
}

fn finish_operation_success(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    operation_id: OperationId,
    current_revision: i64,
    result_json: &str,
    now_ms: u64,
) -> Result<(), M5TemplateError> {
    let next = current_revision.checked_add(1).ok_or_else(internal)?;
    transaction
        .execute(
            "UPDATE operations SET state='succeeded',revision=?1,result_json=?2,
                    error_code=NULL,diagnostic_id=NULL,updated_at_ms=?3,completed_at_ms=?3
             WHERE operation_id=?4",
            params![
                next,
                result_json,
                integer(now_ms)?,
                operation_id.to_string()
            ],
        )
        .map_err(failure)?;
    transaction
        .execute(
            "UPDATE operation_journal SET state='applied',updated_at_ms=?1
             WHERE operation_id=?2",
            params![integer(now_ms)?, operation_id.to_string()],
        )
        .map_err(failure)?;
    insert_operation_event(
        transaction,
        owner,
        operation_id,
        revision_value(next)?,
        "operation.completed",
        now_ms,
    )
}

fn insert_event(
    transaction: &Connection,
    owner: &PrincipalId,
    kind: &str,
    id: TemplateId,
    revision: Revision,
    now_ms: u64,
) -> Result<(), M5TemplateError> {
    transaction
        .execute(
            "INSERT INTO events (
                event_id,kind,aggregate_kind,aggregate_id,aggregate_revision,
                principal_id,occurred_at_ms,payload_json
             ) VALUES (?1,?2,'template',?3,?4,?5,?6,'{}')",
            params![
                Uuid::new_v4().to_string(),
                kind,
                id.to_string(),
                integer(revision.get())?,
                owner.as_str(),
                integer(now_ms)?,
            ],
        )
        .map_err(failure)?;
    Ok(())
}

fn insert_operation_event(
    transaction: &Connection,
    owner: &PrincipalId,
    operation_id: OperationId,
    revision: Revision,
    kind: &str,
    now_ms: u64,
) -> Result<(), M5TemplateError> {
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

fn replay<T: serde::de::DeserializeOwned>(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    method: &str,
    key: &IdempotencyKey,
    fingerprint: &str,
) -> Result<Option<T>, M5TemplateError> {
    let existing = transaction
        .query_row(
            "SELECT request_fingerprint,response_json FROM idempotency_records
             WHERE principal_id=?1 AND method=?2 AND idempotency_key=?3",
            params![owner.as_str(), method, key.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(failure)?;
    match existing {
        None => Ok(None),
        Some((saved, response)) if saved == fingerprint => serde_json::from_str(&response)
            .map(Some)
            .map_err(|_| internal()),
        Some(_) => Err(error(M5TemplateErrorCode::TemplateConflict)),
    }
}

fn save_response<T: Serialize>(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    method: &str,
    key: &IdempotencyKey,
    fingerprint: &str,
    response: &T,
    now_ms: u64,
) -> Result<(), M5TemplateError> {
    transaction
        .execute(
            "INSERT INTO idempotency_records (
                principal_id,method,idempotency_key,request_fingerprint,state,response_json,created_at_ms
             ) VALUES (?1,?2,?3,?4,'completed',?5,?6)",
            params![
                owner.as_str(),
                method,
                key.as_str(),
                fingerprint,
                serde_json::to_string(response).map_err(|_| internal())?,
                integer(now_ms)?,
            ],
        )
        .map_err(failure)?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportAuthority {
    version: u32,
    kind: String,
    template_id: String,
    expected_revision: u64,
    old_bundle_sha256: Option<String>,
    new_bundle_sha256: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeriveAuthority {
    version: u32,
    kind: String,
    template_id: String,
    source_project_id: String,
    source_project_revision: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateProjectAuthority {
    version: u32,
    kind: String,
    template_id: String,
    template_revision: u64,
    bundle_sha256: String,
    project_id: String,
    target_path: String,
}

fn plan_kind(value: TemplatePlanKind) -> &'static str {
    match value {
        TemplatePlanKind::Import => "import",
        TemplatePlanKind::Derive => "derive",
        TemplatePlanKind::CreateProject => "create-project",
    }
}

fn parse_plan_kind(value: &str) -> rusqlite::Result<TemplatePlanKind> {
    match value {
        "import" => Ok(TemplatePlanKind::Import),
        "derive" => Ok(TemplatePlanKind::Derive),
        "create-project" => Ok(TemplatePlanKind::CreateProject),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn operation_kind(value: TemplatePlanKind) -> &'static str {
    match value {
        TemplatePlanKind::Import => "templates.import",
        TemplatePlanKind::Derive => "templates.derive",
        TemplatePlanKind::CreateProject => "templates.create-project",
    }
}

fn digest(bytes: Vec<u8>) -> rusqlite::Result<[u8; 32]> {
    bytes.try_into().map_err(|_| rusqlite::Error::InvalidQuery)
}

fn revision(value: i64) -> rusqlite::Result<Revision> {
    Revision::new(unsigned(value)?).ok_or(rusqlite::Error::InvalidQuery)
}

fn revision_value(value: i64) -> Result<Revision, M5TemplateError> {
    Revision::new(u64::try_from(value).map_err(|_| internal())?).ok_or_else(internal)
}

fn unsigned(value: i64) -> rusqlite::Result<u64> {
    u64::try_from(value).map_err(|_| rusqlite::Error::InvalidQuery)
}

fn integer(value: u64) -> Result<i64, M5TemplateError> {
    i64::try_from(value).map_err(|_| error(M5TemplateErrorCode::InvalidInput))
}

fn transaction(connection: &mut Connection) -> Result<Transaction<'_>, M5TemplateError> {
    connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(failure)
}

fn hex(value: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(64);
    for byte in value {
        result.push(char::from(HEX[(byte >> 4) as usize]));
        result.push(char::from(HEX[(byte & 0x0f) as usize]));
    }
    result
}

fn parse_hex_digest(value: &str) -> Result<[u8; 32], M5TemplateError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(internal());
    }
    let mut result = [0_u8; 32];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let text = std::str::from_utf8(chunk).map_err(|_| internal())?;
        result[index] = u8::from_str_radix(text, 16).map_err(|_| internal())?;
    }
    Ok(result)
}

fn failure(_: rusqlite::Error) -> M5TemplateError {
    error(M5TemplateErrorCode::StoreUnavailable)
}

fn internal() -> M5TemplateError {
    error(M5TemplateErrorCode::Internal)
}

const fn error(code: M5TemplateErrorCode) -> M5TemplateError {
    M5TemplateError::new(code)
}
