use std::collections::BTreeSet;

use alcomd_application::{
    IdempotencyKey, M3ErrorCode, OperationId, OperationState, PrincipalId, ProjectCreateError,
    ProjectCreateErrorCode, ProjectId, ProjectRecord, ProjectTemplateDeclaration,
    ProjectTemplateJobAccepted, ProjectTemplateJobRequest, ProjectTemplateJobWork,
    ProjectTemplateSource, Revision,
};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};

use super::project_create::{begin, db_error, integer, internal};
use super::{m3, project_templates, sqlite};

#[cfg(test)]
#[path = "project_template_jobs_tests.rs"]
mod tests;

const SAVE: &str = "projectTemplates.save";
const EXPORT: &str = "projectTemplates.export";

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Journal {
    phase: Phase,
    export_declaration: Option<ProjectTemplateDeclaration>,
}

#[derive(Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Accepted,
    PublishIntent,
    StateCommitted,
}

struct Context {
    owner: PrincipalId,
    request: ProjectTemplateJobRequest,
    state: OperationState,
    revision: Revision,
    cancel_requested: bool,
    journal: Journal,
}

pub(super) fn add_archive_references(
    connection: &Connection,
    referenced: &mut BTreeSet<String>,
) -> Result<(), ProjectCreateError> {
    let mut statement = connection
        .prepare(
            "SELECT o.operation_id FROM operations o
             WHERE o.kind=?1 AND o.state NOT IN ('succeeded','failed','cancelled')",
        )
        .map_err(db_error)?;
    let rows = statement
        .query_map([EXPORT], |row| row.get::<_, String>(0))
        .map_err(db_error)?;
    for row in rows {
        let id = OperationId::parse(&row.map_err(db_error)?).map_err(|_| internal())?;
        let declaration = load(connection, id)?
            .journal
            .export_declaration
            .ok_or_else(internal)?;
        referenced.insert(declaration.archive_sha256);
    }
    Ok(())
}

pub(super) fn source_project(
    connection: &Connection,
    owner: &PrincipalId,
    project_id: ProjectId,
) -> Result<ProjectRecord, ProjectCreateError> {
    m3::get_project(connection, owner, project_id).map_err(|source| {
        let code = match source.code() {
            M3ErrorCode::ProjectNotFound | M3ErrorCode::ProjectNotRegistered => {
                ProjectCreateErrorCode::TemplateStale
            }
            M3ErrorCode::StoreUnavailable => ProjectCreateErrorCode::StoreUnavailable,
            _ => ProjectCreateErrorCode::Internal,
        };
        ProjectCreateError::new(code).with_source(source)
    })
}

pub(super) fn accept(
    connection: &mut Connection,
    owner: &PrincipalId,
    request: ProjectTemplateJobRequest,
    key: &IdempotencyKey,
    fingerprint: &str,
    now_ms: u64,
) -> Result<ProjectTemplateJobAccepted, ProjectCreateError> {
    let transaction = begin(connection)?;
    let kind = request_kind(&request);
    let existing = transaction
        .query_row(
            "SELECT request_fingerprint, operation_id FROM idempotency_records
             WHERE principal_id=?1 AND method=?2 AND idempotency_key=?3",
            params![owner.as_str(), kind, key.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(db_error)?;
    if let Some((stored_fingerprint, id)) = existing {
        if stored_fingerprint != fingerprint {
            return Err(ProjectCreateError::new(
                ProjectCreateErrorCode::IdempotencyConflict,
            ));
        }
        return Ok(ProjectTemplateJobAccepted {
            operation_id: OperationId::parse(&id).map_err(|_| internal())?,
            replayed: true,
        });
    }
    let export_declaration = match &request {
        ProjectTemplateJobRequest::Export { reference, .. } => {
            Some(project_templates::frozen(&transaction, owner, reference)?)
        }
        ProjectTemplateJobRequest::Save { source, .. } => {
            check_source_revision(&transaction, owner, source)?;
            None
        }
    };
    let id = OperationId::new();
    let timestamp = integer(now_ms)?;
    let encoded_request = json(&request)?;
    let journal = json(&Journal {
        phase: Phase::Accepted,
        export_declaration,
    })?;
    if encoded_request.len() > 65_536 || journal.len() > 65_536 {
        return Err(ProjectCreateError::new(
            ProjectCreateErrorCode::InvalidInput,
        ));
    }
    transaction
        .execute(
            "INSERT INTO operations (
                operation_id,kind,state,revision,owner_principal_id,request_json,
                created_at_ms,updated_at_ms
             ) VALUES (?1,?2,'queued',1,?3,?4,?5,?5)",
            params![
                id.to_string(),
                kind,
                owner.as_str(),
                encoded_request,
                timestamp
            ],
        )
        .map_err(db_error)?;
    transaction
        .execute(
            "INSERT INTO operation_journal (operation_id,step,kind,state,payload_json,updated_at_ms)
             VALUES (?1,1,?2,'prepared',?3,?4)",
            params![id.to_string(), kind, journal, timestamp],
        )
        .map_err(db_error)?;
    transaction
        .execute(
            "INSERT INTO idempotency_records (
                principal_id,method,idempotency_key,request_fingerprint,state,
                operation_id,response_json,created_at_ms
             ) VALUES (?1,?2,?3,?4,'completed',?5,'{}',?6)",
            params![
                owner.as_str(),
                kind,
                key.as_str(),
                fingerprint,
                id.to_string(),
                timestamp
            ],
        )
        .map_err(db_error)?;
    sqlite::insert_event(
        &transaction,
        owner,
        id,
        Revision::INITIAL,
        "operation.created",
        r#"{"state":"queued"}"#,
        timestamp,
    )
    .map_err(db_error)?;
    transaction.commit().map_err(db_error)?;
    Ok(ProjectTemplateJobAccepted {
        operation_id: id,
        replayed: false,
    })
}

pub(super) fn begin_work(
    connection: &mut Connection,
    id: OperationId,
    now_ms: u64,
) -> Result<Option<ProjectTemplateJobWork>, ProjectCreateError> {
    let transaction = begin(connection)?;
    let mut context = load(&transaction, id)?;
    if context.state.is_terminal() {
        return Ok(None);
    }
    if matches!(
        context.state,
        OperationState::Queued | OperationState::Recovering
    ) && !context.cancel_requested
    {
        transition(
            &transaction,
            id,
            &mut context,
            OperationState::Running,
            integer(now_ms)?,
        )?;
    }
    let work = ProjectTemplateJobWork {
        owner: context.owner,
        request: context.request,
        export_declaration: context.journal.export_declaration,
        cancel_requested: context.cancel_requested,
        publishing: context.journal.phase == Phase::PublishIntent,
    };
    transaction.commit().map_err(db_error)?;
    Ok(Some(work))
}

pub(super) fn publish_intent(
    connection: &mut Connection,
    id: OperationId,
    now_ms: u64,
) -> Result<bool, ProjectCreateError> {
    let transaction = begin(connection)?;
    let mut context = load(&transaction, id)?;
    if context.cancel_requested {
        return Ok(false);
    }
    if context.state != OperationState::Running
        || !matches!(context.request, ProjectTemplateJobRequest::Export { .. })
    {
        return Err(internal());
    }
    if context.journal.phase != Phase::PublishIntent {
        let timestamp = integer(now_ms)?;
        let revision = context.revision.checked_next().ok_or_else(internal)?;
        context.journal.phase = Phase::PublishIntent;
        write_journal(&transaction, id, &context.journal, false, timestamp)?;
        transaction
            .execute(
                "UPDATE operations SET revision=?1,updated_at_ms=?2 WHERE operation_id=?3",
                params![integer(revision.get())?, timestamp, id.to_string()],
            )
            .map_err(db_error)?;
        sqlite::insert_event(
            &transaction,
            &context.owner,
            id,
            revision,
            "operation.progress",
            r#"{"phase":"publish_intent"}"#,
            timestamp,
        )
        .map_err(db_error)?;
    }
    transaction.commit().map_err(db_error)?;
    Ok(true)
}

pub(super) fn finish_save(
    connection: &mut Connection,
    id: OperationId,
    declaration: ProjectTemplateDeclaration,
    now_ms: u64,
) -> Result<(), ProjectCreateError> {
    let transaction = begin(connection)?;
    let context = load(&transaction, id)?;
    if context.state.is_terminal() {
        return Ok(());
    }
    let ProjectTemplateJobRequest::Save {
        source,
        expected_revision,
    } = &context.request
    else {
        return Err(internal());
    };
    let timestamp = integer(now_ms)?;
    if context.cancel_requested {
        terminal(&transaction, id, context, Completion::Cancelled, timestamp)?;
    } else {
        check_source_revision(&transaction, &context.owner, source)?;
        let record = project_templates::save_in_transaction(
            &transaction,
            &context.owner,
            declaration,
            *expected_revision,
            now_ms,
        )?;
        let declaration = record.declaration;
        let mut template = serde_json::json!({
            "id": declaration.id, "name": declaration.name,
            "revision": record.revision, "locked": record.locked,
        });
        if let Some(version) = declaration.editor_version {
            template["editorVersion"] = version.into();
        }
        let result = serde_json::json!({ "template": template }).to_string();
        terminal(
            &transaction,
            id,
            context,
            Completion::Succeeded(&result),
            timestamp,
        )?;
    }
    transaction.commit().map_err(db_error)
}

pub(super) fn finish_export(
    connection: &mut Connection,
    id: OperationId,
    now_ms: u64,
) -> Result<(), ProjectCreateError> {
    let transaction = begin(connection)?;
    let context = load(&transaction, id)?;
    if context.state.is_terminal() {
        return Ok(());
    }
    let ProjectTemplateJobRequest::Export { reference, .. } = &context.request else {
        return Err(internal());
    };
    if context.journal.phase != Phase::PublishIntent || context.cancel_requested {
        return Err(internal());
    }
    let result = json(reference)?;
    terminal(
        &transaction,
        id,
        context,
        Completion::Succeeded(&result),
        integer(now_ms)?,
    )?;
    transaction.commit().map_err(db_error)
}

pub(super) fn fail(
    connection: &mut Connection,
    id: OperationId,
    error_code: &str,
    diagnostic_id: &str,
    package_id: Option<&str>,
    now_ms: u64,
) -> Result<(), ProjectCreateError> {
    let transaction = begin(connection)?;
    let context = load(&transaction, id)?;
    if context.state.is_terminal() {
        return Ok(());
    }
    let completion = if context.cancel_requested && context.journal.phase != Phase::PublishIntent {
        Completion::Cancelled
    } else {
        Completion::Failed {
            error_code,
            diagnostic_id,
            package_id,
        }
    };
    terminal(&transaction, id, context, completion, integer(now_ms)?)?;
    transaction.commit().map_err(db_error)
}

pub(super) fn cancel(
    connection: &mut Connection,
    id: OperationId,
    now_ms: u64,
) -> Result<(), ProjectCreateError> {
    let transaction = begin(connection)?;
    let context = load(&transaction, id)?;
    if context.state.is_terminal() {
        return Ok(());
    }
    if !context.cancel_requested || context.journal.phase == Phase::PublishIntent {
        return Err(internal());
    }
    terminal(
        &transaction,
        id,
        context,
        Completion::Cancelled,
        integer(now_ms)?,
    )?;
    transaction.commit().map_err(db_error)
}

pub(super) fn pending(
    connection: &mut Connection,
    now_ms: u64,
) -> Result<Vec<OperationId>, ProjectCreateError> {
    let transaction = begin(connection)?;
    let candidates = {
        let mut statement = transaction
            .prepare(
                "SELECT operation_id FROM operations
                 WHERE kind IN (?1,?2) AND state NOT IN ('succeeded','failed','cancelled')
                 ORDER BY created_at_ms,operation_id",
            )
            .map_err(db_error)?;
        statement
            .query_map([SAVE, EXPORT], |row| row.get::<_, String>(0))
            .map_err(db_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(db_error)?
    };
    let timestamp = integer(now_ms)?;
    let mut operations = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let id = OperationId::parse(&candidate).map_err(|_| internal())?;
        let mut context = load(&transaction, id)?;
        if matches!(
            context.state,
            OperationState::Running | OperationState::Cancelling
        ) {
            transition(
                &transaction,
                id,
                &mut context,
                OperationState::Interrupted,
                timestamp,
            )?;
        }
        if context.state == OperationState::Interrupted {
            transition(
                &transaction,
                id,
                &mut context,
                OperationState::Recovering,
                timestamp,
            )?;
        }
        operations.push(id);
    }
    transaction.commit().map_err(db_error)?;
    Ok(operations)
}

fn check_source_revision(
    connection: &Connection,
    owner: &PrincipalId,
    source: &ProjectTemplateSource,
) -> Result<(), ProjectCreateError> {
    if let ProjectTemplateSource::Project {
        project_id,
        expected_project_revision,
        ..
    } = source
        && source_project(connection, owner, *project_id)?
            .revision
            .get()
            != *expected_project_revision
    {
        return Err(ProjectCreateError::new(
            ProjectCreateErrorCode::TemplateStale,
        ));
    }
    Ok(())
}

fn request_kind(request: &ProjectTemplateJobRequest) -> &'static str {
    match request {
        ProjectTemplateJobRequest::Save { .. } => SAVE,
        ProjectTemplateJobRequest::Export { .. } => EXPORT,
    }
}

fn load(connection: &Connection, id: OperationId) -> Result<Context, ProjectCreateError> {
    let (owner, request, state, revision, cancel_requested, journal): (
        String, String, String, i64, bool, String,
    ) = connection
        .query_row(
            "SELECT o.owner_principal_id,o.request_json,o.state,o.revision,
                    o.cancel_requested,j.payload_json
             FROM operations o JOIN operation_journal j ON j.operation_id=o.operation_id AND j.step=1
             WHERE o.operation_id=?1 AND o.kind IN (?2,?3) AND j.kind=o.kind",
            params![id.to_string(), SAVE, EXPORT],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?)),
        )
        .map_err(db_error)?;
    Ok(Context {
        owner: PrincipalId::parse(owner).map_err(|_| internal())?,
        request: serde_json::from_str(&request).map_err(|cause| internal().with_source(cause))?,
        state: sqlite::parse_state(&state).map_err(db_error)?,
        revision: u64::try_from(revision)
            .ok()
            .and_then(Revision::new)
            .ok_or_else(internal)?,
        cancel_requested,
        journal: serde_json::from_str(&journal).map_err(|cause| internal().with_source(cause))?,
    })
}

fn transition(
    transaction: &Transaction<'_>,
    id: OperationId,
    context: &mut Context,
    next: OperationState,
    timestamp: i64,
) -> Result<(), ProjectCreateError> {
    if !context.state.can_transition_to(next) {
        return Err(internal());
    }
    let revision = context.revision.checked_next().ok_or_else(internal)?;
    let name = sqlite::state_name(next);
    transaction
        .execute(
            "UPDATE operations SET state=?1,revision=?2,updated_at_ms=?3,
                started_at_ms=CASE WHEN ?1='running' THEN coalesce(started_at_ms,?3) ELSE started_at_ms END
             WHERE operation_id=?4",
            params![name, integer(revision.get())?, timestamp, id.to_string()],
        )
        .map_err(db_error)?;
    state_event(transaction, &context.owner, id, revision, name, timestamp)?;
    context.state = next;
    context.revision = revision;
    Ok(())
}

enum Completion<'a> {
    Succeeded(&'a str),
    Failed {
        error_code: &'a str,
        diagnostic_id: &'a str,
        package_id: Option<&'a str>,
    },
    Cancelled,
}

fn terminal(
    transaction: &Transaction<'_>,
    id: OperationId,
    mut context: Context,
    completion: Completion<'_>,
    timestamp: i64,
) -> Result<(), ProjectCreateError> {
    let package_details = match &completion {
        Completion::Failed {
            package_id: Some(package_id),
            ..
        } => Some(serde_json::json!({ "packageId": package_id }).to_string()),
        _ => None,
    };
    let (next, result, error_code, diagnostic_id) = match completion {
        Completion::Succeeded(result) => {
            context.journal.phase = Phase::StateCommitted;
            (OperationState::Succeeded, Some(result), None, None)
        }
        Completion::Failed {
            error_code,
            diagnostic_id,
            ..
        } => (
            OperationState::Failed,
            package_details.as_deref(),
            Some(error_code),
            Some(diagnostic_id),
        ),
        Completion::Cancelled => (OperationState::Cancelled, None, None, None),
    };
    if !context.state.can_transition_to(next) {
        return Err(internal());
    }
    let name = sqlite::state_name(next);
    let revision = context.revision.checked_next().ok_or_else(internal)?;
    transaction
        .execute(
            "UPDATE operations SET state=?1,revision=?2,result_json=?3,error_code=?4,
                diagnostic_id=?5,updated_at_ms=?6,completed_at_ms=?6 WHERE operation_id=?7",
            params![
                name,
                integer(revision.get())?,
                result,
                error_code,
                diagnostic_id,
                timestamp,
                id.to_string()
            ],
        )
        .map_err(db_error)?;
    context.journal.export_declaration = None;
    write_journal(transaction, id, &context.journal, true, timestamp)?;
    state_event(transaction, &context.owner, id, revision, name, timestamp)
}

fn state_event(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    id: OperationId,
    revision: Revision,
    state: &str,
    timestamp: i64,
) -> Result<(), ProjectCreateError> {
    sqlite::insert_event(
        transaction,
        owner,
        id,
        revision,
        "operation.state_changed",
        &serde_json::json!({ "state": state }).to_string(),
        timestamp,
    )
    .map_err(db_error)
}

fn write_journal(
    transaction: &Transaction<'_>,
    id: OperationId,
    journal: &Journal,
    applied: bool,
    timestamp: i64,
) -> Result<(), ProjectCreateError> {
    transaction
        .execute(
            "UPDATE operation_journal SET state=?1,payload_json=?2,updated_at_ms=?3
             WHERE operation_id=?4 AND step=1",
            params![
                if applied { "applied" } else { "prepared" },
                json(journal)?,
                timestamp,
                id.to_string(),
            ],
        )
        .map_err(db_error)?;
    Ok(())
}

fn json<T: Serialize>(value: &T) -> Result<String, ProjectCreateError> {
    serde_json::to_string(value).map_err(|cause| internal().with_source(cause))
}
