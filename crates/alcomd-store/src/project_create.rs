use alcomd_application::{
    IdempotencyKey, OperationId, OperationState, PrincipalId, ProjectCreateAcceptance,
    ProjectCreateAccepted, ProjectCreateError, ProjectCreateErrorCode, ProjectCreateInput,
    ProjectCreatePackages, ProjectCreateStore, ProjectCreateWork, ProjectId, ProjectObservation,
    ProjectTemplateDeclaration, ProjectTemplatePage, ProjectTemplateRecord, Revision,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{project_templates, sqlite};

#[cfg(test)]
#[path = "project_create_tests.rs"]
mod tests;

const KIND: &str = "projects.create";
const DECLARATION: &str = "projects.create.declaration";
const PACKAGES: &str = "projects.create.packages";
const MAX_JOURNAL_BYTES: usize = 4 * 1024 * 1024;
const JOURNAL_CHUNK_BYTES: usize = 8 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Journal {
    project_id: String,
    parent_identity: Vec<u8>,
    canonical_parent: String,
    phase: Phase,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    PreflightComplete,
    Prepared,
    PublishIntent,
    StateCommitted,
}

struct Context {
    owner: PrincipalId,
    state: OperationState,
    revision: Revision,
    cancel_requested: bool,
    request: ProjectCreateInput,
    journal: Journal,
}

pub(super) fn replay(
    connection: &Connection,
    owner: &PrincipalId,
    fingerprint: &str,
    key: &IdempotencyKey,
) -> Result<Option<ProjectCreateAccepted>, ProjectCreateError> {
    let existing = connection
        .query_row(
            "SELECT request_fingerprint, operation_id, response_json
             FROM idempotency_records
             WHERE principal_id=?1 AND method=?2 AND idempotency_key=?3",
            params![owner.as_str(), KIND, key.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()
        .map_err(db_error)?;
    let Some((previous_fingerprint, operation_id, response)) = existing else {
        return Ok(None);
    };
    if previous_fingerprint != fingerprint {
        return Err(error(ProjectCreateErrorCode::IdempotencyConflict));
    }
    let response: AcceptedResponse = decode(&response)?;
    Ok(Some(ProjectCreateAccepted {
        operation_id: OperationId::parse(&operation_id).map_err(|_| internal())?,
        project_id: ProjectId::parse(&response.project_id).map_err(|_| internal())?,
        replayed: true,
    }))
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AcceptedResponse {
    project_id: String,
}

pub(super) fn accept(
    connection: &mut Connection,
    acceptance: ProjectCreateAcceptance,
    now_ms: u64,
) -> Result<ProjectCreateAccepted, ProjectCreateError> {
    let ProjectCreateAcceptance {
        owner,
        input,
        parent_identity,
        canonical_parent,
        fingerprint,
        key,
        allow_package_changes,
    } = acceptance;
    let transaction = begin(connection)?;
    if let Some(accepted) = replay(&transaction, &owner, &fingerprint, &key)? {
        return Ok(accepted);
    }
    let declaration = input
        .template
        .as_ref()
        .map(|reference| project_templates::frozen(&transaction, &owner, reference))
        .transpose()?;
    if let Some(declaration) = &declaration {
        if !allow_package_changes && !declaration.dependencies.is_empty() {
            return Err(error(ProjectCreateErrorCode::PermissionDenied));
        }
        if !declaration.unity.allows(&input.unity_version) {
            return Err(error(ProjectCreateErrorCode::DependencyUnsatisfied));
        }
    }
    let operation_id = OperationId::new();
    let project_id = ProjectId::new();
    let timestamp = integer(now_ms)?;
    let request = json(&input)?;
    let journal = Journal {
        project_id: project_id.to_string(),
        parent_identity,
        canonical_parent,
        phase: Phase::PreflightComplete,
    };
    let evidence = json(&journal)?;
    if request.len() > 65_536 || evidence.len() > 65_536 {
        return Err(error(ProjectCreateErrorCode::InvalidInput));
    }
    transaction
        .execute(
            "INSERT INTO operations (
                operation_id,kind,state,revision,owner_principal_id,request_json,
                created_at_ms,updated_at_ms
             ) VALUES (?1,?2,'queued',1,?3,?4,?5,?5)",
            params![
                operation_id.to_string(),
                KIND,
                owner.as_str(),
                request,
                timestamp
            ],
        )
        .map_err(db_error)?;
    transaction
        .execute(
            "INSERT INTO operation_journal (operation_id,step,kind,state,payload_json,updated_at_ms)
             VALUES (?1,1,?2,'prepared',?3,?4)",
            params![operation_id.to_string(), KIND, evidence, timestamp],
        )
        .map_err(db_error)?;
    if let Some(declaration) = &declaration {
        write_chunks(
            &transaction,
            operation_id,
            DECLARATION,
            &json(declaration)?,
            timestamp,
        )?;
    }
    transaction
        .execute(
            "INSERT INTO idempotency_records (
                principal_id,method,idempotency_key,request_fingerprint,state,
                operation_id,response_json,created_at_ms
             ) VALUES (?1,?2,?3,?4,'completed',?5,?6,?7)",
            params![
                owner.as_str(),
                KIND,
                key.as_str(),
                fingerprint,
                operation_id.to_string(),
                json(&AcceptedResponse {
                    project_id: project_id.to_string()
                })?,
                timestamp
            ],
        )
        .map_err(db_error)?;
    operation_event(
        &transaction,
        &owner,
        operation_id,
        Revision::INITIAL,
        "operation.created",
        r#"{"state":"queued"}"#,
        timestamp,
    )?;
    transaction.commit().map_err(db_error)?;
    Ok(ProjectCreateAccepted {
        operation_id,
        project_id,
        replayed: false,
    })
}

pub(super) fn begin_work(
    connection: &mut Connection,
    id: OperationId,
    now_ms: u64,
) -> Result<Option<ProjectCreateWork>, ProjectCreateError> {
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
    let declaration = read_chunks(&transaction, id, DECLARATION)?
        .map(|encoded| decode::<ProjectTemplateDeclaration>(&encoded))
        .transpose()?;
    if context.request.template.is_some() != declaration.is_some() {
        return Err(internal());
    }
    let packages = read_chunks(&transaction, id, PACKAGES)?
        .map(|encoded| decode::<ProjectCreatePackages>(&encoded))
        .transpose()?;
    let publishing = context.journal.phase == Phase::PublishIntent;
    let work = ProjectCreateWork {
        project_id: ProjectId::parse(&context.journal.project_id).map_err(|_| internal())?,
        owner: context.owner,
        input: context.request,
        parent_identity: context.journal.parent_identity,
        canonical_parent: context.journal.canonical_parent,
        declaration,
        packages,
        publishing,
        cancel_requested: context.cancel_requested,
    };
    transaction.commit().map_err(db_error)?;
    Ok(Some(work))
}

pub(super) fn save_packages(
    connection: &mut Connection,
    id: OperationId,
    packages: ProjectCreatePackages,
    now_ms: u64,
) -> Result<(), ProjectCreateError> {
    let transaction = begin(connection)?;
    let mut context = load(&transaction, id)?;
    let encoded = json(&packages)?;
    if let Some(existing) = read_chunks(&transaction, id, PACKAGES)? {
        return if existing == encoded {
            Ok(())
        } else {
            Err(internal())
        };
    }
    if context.state.is_terminal() || context.journal.phase != Phase::PreflightComplete {
        return Err(internal());
    }
    let timestamp = integer(now_ms)?;
    write_chunks(&transaction, id, PACKAGES, &encoded, timestamp)?;
    progress(
        &transaction,
        id,
        &mut context,
        Phase::Prepared,
        "prepared",
        timestamp,
    )?;
    transaction.commit().map_err(db_error)
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
    if context.state != OperationState::Running {
        return Err(internal());
    }
    if context.journal.phase != Phase::PublishIntent {
        progress(
            &transaction,
            id,
            &mut context,
            Phase::PublishIntent,
            "publish_intent",
            integer(now_ms)?,
        )?;
    }
    transaction.commit().map_err(db_error)?;
    Ok(true)
}

pub(super) fn finish(
    connection: &mut Connection,
    id: OperationId,
    observation: ProjectObservation,
    now_ms: u64,
) -> Result<(), ProjectCreateError> {
    let transaction = begin(connection)?;
    let context = load(&transaction, id)?;
    if context.state == OperationState::Succeeded {
        return Ok(());
    }
    if context.state != OperationState::Running
        || context.journal.phase != Phase::PublishIntent
        || observation.unity_version != context.request.unity_version
    {
        return Err(internal());
    }
    let project_id = &context.journal.project_id;
    let existing: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE project_id=?1 OR path_identity_key=?2)",
            params![project_id, observation.path_identity_key],
            |row| row.get(0),
        )
        .map_err(db_error)?;
    if existing {
        return Err(error(ProjectCreateErrorCode::DestinationChanged));
    }
    let timestamp = integer(now_ms)?;
    transaction
        .execute(
            "INSERT INTO projects (
                project_id,owner_principal_id,root_path,path_identity_key,project_type,
                unity_version,unity_revision,snapshot_json,revision,registered_at_ms,
                updated_at_ms,last_modified_at_ms
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,1,?9,?9,?9)",
            params![
                project_id,
                context.owner.as_str(),
                observation.root_path,
                observation.path_identity_key,
                super::m3::project_type(observation.project_type),
                observation.unity_version,
                observation.unity_revision,
                json(&observation)?,
                timestamp
            ],
        )
        .map_err(db_error)?;
    transaction
        .execute(
            "INSERT INTO events (
                event_id,kind,aggregate_kind,aggregate_id,aggregate_revision,
                principal_id,occurred_at_ms,payload_json
             ) VALUES (?1,'project.registered','project',?2,1,?3,?4,'{}')",
            params![
                Uuid::new_v4().to_string(),
                project_id,
                context.owner.as_str(),
                timestamp
            ],
        )
        .map_err(db_error)?;
    let result = serde_json::json!({ "projectId": project_id, "revision": 1 }).to_string();
    terminal(
        &transaction,
        id,
        context,
        Completion::Succeeded(&result),
        timestamp,
    )?;
    transaction.commit().map_err(db_error)
}

pub(super) fn fail(
    connection: &mut Connection,
    id: OperationId,
    error_code: &str,
    diagnostic_id: &str,
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
                 WHERE kind=?1 AND state NOT IN ('succeeded','failed','cancelled')
                 ORDER BY created_at_ms,operation_id",
            )
            .map_err(db_error)?;
        statement
            .query_map([KIND], |row| row.get::<_, String>(0))
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

fn load(connection: &Connection, id: OperationId) -> Result<Context, ProjectCreateError> {
    let (owner, state, revision, cancel_requested, request, journal): (
        String, String, i64, bool, String, String,
    ) = connection
        .query_row(
            "SELECT o.owner_principal_id,o.state,o.revision,o.cancel_requested,o.request_json,j.payload_json
             FROM operations o JOIN operation_journal j ON j.operation_id=o.operation_id AND j.step=1
             WHERE o.operation_id=?1 AND o.kind=?2 AND j.kind=?2",
            params![id.to_string(), KIND],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?)),
        )
        .map_err(db_error)?;
    Ok(Context {
        owner: PrincipalId::parse(owner).map_err(|_| internal())?,
        state: sqlite::parse_state(&state).map_err(db_error)?,
        revision: u64::try_from(revision)
            .ok()
            .and_then(Revision::new)
            .ok_or_else(internal)?,
        cancel_requested,
        request: decode(&request)?,
        journal: decode(&journal)?,
    })
}

fn transition(
    transaction: &Transaction<'_>,
    id: OperationId,
    context: &mut Context,
    state: OperationState,
    timestamp: i64,
) -> Result<(), ProjectCreateError> {
    if !context.state.can_transition_to(state) {
        return Err(internal());
    }
    let revision = next_revision(context)?;
    let state_name = sqlite::state_name(state);
    transaction
        .execute(
            "UPDATE operations SET state=?1,revision=?2,updated_at_ms=?3,
                started_at_ms=CASE WHEN ?1='running' THEN coalesce(started_at_ms,?3) ELSE started_at_ms END
             WHERE operation_id=?4",
            params![state_name, integer(revision.get())?, timestamp, id.to_string()],
        )
        .map_err(db_error)?;
    operation_event(
        transaction,
        &context.owner,
        id,
        revision,
        "operation.state_changed",
        &serde_json::json!({ "state": state_name }).to_string(),
        timestamp,
    )?;
    context.state = state;
    context.revision = revision;
    Ok(())
}

fn progress(
    transaction: &Transaction<'_>,
    id: OperationId,
    context: &mut Context,
    phase: Phase,
    name: &str,
    timestamp: i64,
) -> Result<(), ProjectCreateError> {
    context.journal.phase = phase;
    update_journal(transaction, id, &context.journal, false, timestamp)?;
    let revision = next_revision(context)?;
    transaction
        .execute(
            "UPDATE operations SET revision=?1,updated_at_ms=?2 WHERE operation_id=?3",
            params![integer(revision.get())?, timestamp, id.to_string()],
        )
        .map_err(db_error)?;
    operation_event(
        transaction,
        &context.owner,
        id,
        revision,
        "operation.progress",
        &serde_json::json!({ "phase": name }).to_string(),
        timestamp,
    )
}

enum Completion<'a> {
    Succeeded(&'a str),
    Failed {
        error_code: &'a str,
        diagnostic_id: &'a str,
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
    let (state, result, error_code, diagnostic_id) = match completion {
        Completion::Succeeded(result) => {
            context.journal.phase = Phase::StateCommitted;
            (OperationState::Succeeded, Some(result), None, None)
        }
        Completion::Failed {
            error_code,
            diagnostic_id,
        } => (
            OperationState::Failed,
            None,
            Some(error_code),
            Some(diagnostic_id),
        ),
        Completion::Cancelled => (OperationState::Cancelled, None, None, None),
    };
    if !context.state.can_transition_to(state) {
        return Err(internal());
    }
    let name = sqlite::state_name(state);
    let revision = next_revision(&context)?;
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
    update_journal(transaction, id, &context.journal, true, timestamp)?;
    operation_event(
        transaction,
        &context.owner,
        id,
        revision,
        "operation.state_changed",
        &serde_json::json!({ "state": name }).to_string(),
        timestamp,
    )
}

fn update_journal(
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
                id.to_string()
            ],
        )
        .map_err(db_error)?;
    Ok(())
}

fn write_chunks(
    transaction: &Transaction<'_>,
    id: OperationId,
    kind: &str,
    mut encoded: &str,
    timestamp: i64,
) -> Result<(), ProjectCreateError> {
    if encoded.len() > MAX_JOURNAL_BYTES {
        return Err(error(ProjectCreateErrorCode::DependencyUnsatisfied));
    }
    let id = id.to_string();
    let mut step = transaction
        .query_row(
            "SELECT max(step) FROM operation_journal WHERE operation_id=?1",
            [&id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(db_error)?;
    while !encoded.is_empty() {
        let mut end = encoded.len().min(JOURNAL_CHUNK_BYTES);
        while !encoded.is_char_boundary(end) {
            end -= 1;
        }
        let (chunk, rest) = encoded.split_at(end);
        step = step.checked_add(1).ok_or_else(internal)?;
        transaction
            .execute(
                "INSERT INTO operation_journal (operation_id,step,kind,state,payload_json,updated_at_ms)
                 VALUES (?1,?2,?3,'applied',?4,?5)",
                params![id, step, kind, json(chunk)?, timestamp],
            )
            .map_err(db_error)?;
        encoded = rest;
    }
    Ok(())
}

fn read_chunks(
    connection: &Connection,
    id: OperationId,
    kind: &str,
) -> Result<Option<String>, ProjectCreateError> {
    let mut statement = connection
        .prepare(
            "SELECT payload_json FROM operation_journal
             WHERE operation_id=?1 AND kind=?2 ORDER BY step",
        )
        .map_err(db_error)?;
    let chunks = statement
        .query_map(params![id.to_string(), kind], |row| row.get::<_, String>(0))
        .map_err(db_error)?;
    let mut encoded = String::new();
    for chunk in chunks {
        let chunk: String = decode(&chunk.map_err(db_error)?)?;
        if encoded.len().saturating_add(chunk.len()) > MAX_JOURNAL_BYTES {
            return Err(internal());
        }
        encoded.push_str(&chunk);
    }
    Ok((!encoded.is_empty()).then_some(encoded))
}

fn next_revision(context: &Context) -> Result<Revision, ProjectCreateError> {
    context.revision.checked_next().ok_or_else(internal)
}

fn operation_event(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    id: OperationId,
    revision: Revision,
    kind: &str,
    payload: &str,
    timestamp: i64,
) -> Result<(), ProjectCreateError> {
    sqlite::insert_event(transaction, owner, id, revision, kind, payload, timestamp)
        .map_err(db_error)
}

pub(super) fn begin(connection: &mut Connection) -> Result<Transaction<'_>, ProjectCreateError> {
    connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(db_error)
}

pub(super) fn integer(value: u64) -> Result<i64, ProjectCreateError> {
    i64::try_from(value).map_err(|_| internal())
}

fn json<T: Serialize + ?Sized>(value: &T) -> Result<String, ProjectCreateError> {
    serde_json::to_string(value).map_err(|source| internal().with_source(source))
}

fn decode<T: serde::de::DeserializeOwned>(value: &str) -> Result<T, ProjectCreateError> {
    serde_json::from_str(value).map_err(|source| internal().with_source(source))
}

pub(super) fn db_error(
    source: impl std::error::Error + Send + Sync + 'static,
) -> ProjectCreateError {
    unavailable().with_source(source)
}

fn error(code: ProjectCreateErrorCode) -> ProjectCreateError {
    ProjectCreateError::new(code)
}

pub(super) fn internal() -> ProjectCreateError {
    error(ProjectCreateErrorCode::Internal)
}

pub(super) fn unavailable() -> ProjectCreateError {
    error(ProjectCreateErrorCode::StoreUnavailable)
}

impl ProjectCreateStore for crate::StateStoreHandle {
    async fn project_create_replay(
        &self,
        owner: PrincipalId,
        fingerprint: String,
        key: IdempotencyKey,
    ) -> Result<Option<ProjectCreateAccepted>, ProjectCreateError> {
        self.request_worker(
            move |connection| replay(connection, &owner, &fingerprint, &key),
            unavailable,
        )
        .await
    }

    async fn project_create_accept(
        &self,
        acceptance: ProjectCreateAcceptance,
        now_ms: u64,
    ) -> Result<ProjectCreateAccepted, ProjectCreateError> {
        self.request_worker(
            move |connection| accept(connection, acceptance, now_ms),
            unavailable,
        )
        .await
    }

    async fn project_create_begin(
        &self,
        id: OperationId,
        now_ms: u64,
    ) -> Result<Option<ProjectCreateWork>, ProjectCreateError> {
        self.request_worker(
            move |connection| begin_work(connection, id, now_ms),
            unavailable,
        )
        .await
    }

    async fn project_create_save_packages(
        &self,
        id: OperationId,
        packages: ProjectCreatePackages,
        now_ms: u64,
    ) -> Result<(), ProjectCreateError> {
        self.request_worker(
            move |connection| save_packages(connection, id, packages, now_ms),
            unavailable,
        )
        .await
    }

    async fn project_create_publish_intent(
        &self,
        id: OperationId,
        now_ms: u64,
    ) -> Result<bool, ProjectCreateError> {
        self.request_worker(
            move |connection| publish_intent(connection, id, now_ms),
            unavailable,
        )
        .await
    }

    async fn project_create_finish(
        &self,
        id: OperationId,
        observation: ProjectObservation,
        now_ms: u64,
    ) -> Result<(), ProjectCreateError> {
        self.request_worker(
            move |connection| finish(connection, id, observation, now_ms),
            unavailable,
        )
        .await
    }

    async fn project_create_fail(
        &self,
        id: OperationId,
        error_code: String,
        diagnostic_id: String,
        now_ms: u64,
    ) -> Result<(), ProjectCreateError> {
        self.request_worker(
            move |connection| fail(connection, id, &error_code, &diagnostic_id, now_ms),
            unavailable,
        )
        .await
    }

    async fn project_create_cancel(
        &self,
        id: OperationId,
        now_ms: u64,
    ) -> Result<(), ProjectCreateError> {
        self.request_worker(
            move |connection| cancel(connection, id, now_ms),
            unavailable,
        )
        .await
    }

    async fn project_create_pending(
        &self,
        now_ms: u64,
    ) -> Result<Vec<OperationId>, ProjectCreateError> {
        self.request_worker(move |connection| pending(connection, now_ms), unavailable)
            .await
    }

    async fn project_templates_list(
        &self,
        owner: PrincipalId,
        limit: usize,
        cursor: Option<String>,
    ) -> Result<ProjectTemplatePage, ProjectCreateError> {
        self.request_worker(
            move |connection| project_templates::list(connection, &owner, limit, cursor),
            unavailable,
        )
        .await
    }

    async fn project_templates_save(
        &self,
        owner: PrincipalId,
        declaration: ProjectTemplateDeclaration,
        expected_revision: Option<u64>,
        now_ms: u64,
    ) -> Result<ProjectTemplateRecord, ProjectCreateError> {
        self.request_worker(
            move |connection| {
                project_templates::save(connection, &owner, declaration, expected_revision, now_ms)
            },
            unavailable,
        )
        .await
    }

    async fn project_templates_remove(
        &self,
        owner: PrincipalId,
        id: String,
        expected_revision: u64,
        now_ms: u64,
    ) -> Result<u64, ProjectCreateError> {
        self.request_worker(
            move |connection| {
                project_templates::remove(connection, &owner, &id, expected_revision, now_ms)
            },
            unavailable,
        )
        .await
    }

    async fn project_templates_seed(
        &self,
        owner: PrincipalId,
        declarations: Vec<ProjectTemplateDeclaration>,
    ) -> Result<(), ProjectCreateError> {
        self.request_worker(
            move |connection| project_templates::seed(connection, &owner, declarations),
            unavailable,
        )
        .await
    }
}
