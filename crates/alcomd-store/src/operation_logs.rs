use std::cell::RefCell;

use alcomd_application::{
    LogOutcome, LogTarget, LogWriteContext, OperationId, OperationLogEntry, OperationLogPage,
    OperationLogQuery, OperationLogStore, PrincipalId, StoreError, StoreErrorKind,
};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::StateStoreHandle;

thread_local! {
    static CONTEXT: RefCell<LogWriteContext> = RefCell::default();
}

pub(super) struct ContextGuard(LogWriteContext);

impl ContextGuard {
    pub(super) fn install(context: LogWriteContext) -> Self {
        Self(CONTEXT.replace(context))
    }
}

impl Drop for ContextGuard {
    fn drop(&mut self) {
        CONTEXT.replace(std::mem::take(&mut self.0));
    }
}

/// One actual durable step, written inside the transaction that changed business state.
pub(super) struct CommittedStep<'a> {
    pub action: &'a str,
    pub step: &'a str,
    pub outcome: LogOutcome,
    pub target: Option<LogTarget>,
    pub execution_id: Option<String>,
    pub operation_id: Option<OperationId>,
    pub details: Option<String>,
}

impl<'a> CommittedStep<'a> {
    pub(super) fn succeeded(action: &'a str, step: &'a str, target: Option<LogTarget>) -> Self {
        Self {
            action,
            step,
            outcome: LogOutcome::Succeeded,
            target,
            execution_id: None,
            operation_id: None,
            details: None,
        }
    }
}

pub(super) fn committed(
    connection: &Connection,
    owner: &PrincipalId,
    occurred_at_ms: u64,
    fact: CommittedStep<'_>,
) -> Result<(), StoreError> {
    let context = CONTEXT.with(|context| context.borrow().clone());
    let call = context.call.filter(|call| &call.owner == owner);
    let operation_id = fact.operation_id.map(|id| id.to_string());
    let existing = if let Some(id) = &operation_id {
        connection
            .query_row(
                "SELECT execution_id, actor, target_json, action FROM operation_log_entries
             WHERE owner=?1 AND operation_id=?2 AND step='accepted' ORDER BY sequence LIMIT 1",
                params![owner.as_str(), id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()
            .map_err(failure)?
    } else {
        None
    };
    let (execution_id, actor, mut target, action) = match (existing, call.as_ref()) {
        (Some((execution_id, actor, target, action)), caller) => {
            let actor = if fact.step == "request_cancel" {
                caller.map_or(actor, |call| call.actor.clone())
            } else {
                actor
            };
            (
                execution_id,
                actor,
                fact.target
                    .or(serde_json::from_str(&target).map_err(failure)?),
                action,
            )
        }
        (None, Some(call)) => (
            fact.execution_id
                .unwrap_or_else(|| call.execution_id.clone()),
            call.actor.clone(),
            fact.target,
            fact.action.to_owned(),
        ),
        // No invented initiating actor for pre-existing operations or fixture-only store calls.
        (None, None) => return Ok(()),
    };
    if let Some(call) = call {
        let steps = call.take_steps();
        if target.is_none() {
            target = steps.iter().rev().find_map(|entry| entry.target.clone());
        }
        for mut entry in steps {
            entry.execution_id.clone_from(&execution_id);
            entry.operation_id.clone_from(&operation_id);
            append(connection, owner, &entry)?;
        }
    }
    let details = context
        .operation_failure
        .and_then(|(id, details)| {
            (Some(id) == fact.operation_id && fact.outcome == LogOutcome::Failed).then_some(details)
        })
        .or(fact.details);
    append(
        connection,
        owner,
        &OperationLogEntry {
            sequence: 0,
            entry_id: OperationId::new().to_string(),
            execution_id,
            actor,
            operation_id,
            occurred_at_ms,
            action,
            step: fact.step.to_owned(),
            outcome: fact.outcome,
            target,
            details,
        },
    )
}

pub(super) fn plan(
    connection: &Connection,
    owner: &PrincipalId,
    plan: alcomd_application::PlanId,
    action: &str,
    target: Option<LogTarget>,
    occurred_at_ms: u64,
) -> Result<(), StoreError> {
    let mut step = CommittedStep::succeeded(action, "planned", target);
    step.execution_id = Some(plan.to_string());
    committed(connection, owner, occurred_at_ms, step)
}

pub(super) fn project_target(
    connection: &Connection,
    owner: &PrincipalId,
    id: String,
) -> Result<LogTarget, StoreError> {
    let path = connection
        .query_row(
            "SELECT root_path FROM projects WHERE owner_principal_id=?1 AND project_id=?2",
            params![owner.as_str(), id],
            |r| r.get(0),
        )
        .optional()
        .map_err(failure)?;
    Ok(LogTarget {
        kind: "project".to_owned(),
        id: Some(id),
        name: None,
        path,
    })
}

pub(super) fn destination_target(parent: &str, leaf: &str) -> LogTarget {
    let path = std::path::Path::new(parent)
        .join(leaf)
        .to_string_lossy()
        .into_owned();
    let mut target = alcomd_application::log_path("project", path);
    target.name = Some(leaf.to_owned());
    target
}

/// Records the same actual Operation edge as the accompanying business event,
/// in the transaction that made that edge true. It never reads the Event table.
pub(super) fn operation_edge(
    connection: &Connection,
    owner: &PrincipalId,
    id: OperationId,
    edge: &str,
    phase_payload: &str,
    occurred_at_ms: i64,
) -> Result<(), StoreError> {
    if edge == "operation.progress" && phase_payload == "{}" {
        return Ok(());
    }
    let (action, state, request, error): (String, String, String, Option<String>) = connection.query_row(
        "SELECT kind, state, request_json, error_code FROM operations WHERE owner_principal_id=?1 AND operation_id=?2",
        params![owner.as_str(), id.to_string()],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    ).map_err(failure)?;
    let phase;
    let (step, outcome) = match edge {
        "operation.created" => ("accepted", LogOutcome::Started),
        "operation.cancel_requested" => ("request_cancel", LogOutcome::Succeeded),
        "operation.progress" => {
            phase = serde_json::from_str::<serde_json::Value>(phase_payload).map_err(failure)?;
            let Some(step) = phase.get("phase").and_then(serde_json::Value::as_str) else {
                return Ok(());
            };
            let outcome = match step {
                "inventory_ready"
                | "archive_ready"
                | "archive_verified"
                | "archive_published"
                | "staging_complete"
                | "target_published"
                | "state_committed"
                | "preflight_complete"
                | "root_quarantined"
                | "preparation_complete"
                | "unity_started"
                | "unity_exited"
                | "project_reobserved"
                | "packages_replaced"
                | "vpm_manifest_committed"
                | "filesystem_committed"
                | "extracted"
                | "prepared"
                | "cleanup_complete" => LogOutcome::Succeeded,
                "recovery_required" => LogOutcome::Waiting,
                _ => LogOutcome::Started,
            };
            (step, outcome)
        }
        "operation.recovered" => ("recovered", LogOutcome::Succeeded),
        "operation.state_changed"
        | "operation.completed"
        | "operation.succeeded"
        | "operation.failed"
        | "operation.cancelled"
        | "operation.interrupted"
        | "operation.recovering"
        | "operation.recovery_failed" => match state.as_str() {
            "succeeded" => ("completed", LogOutcome::Succeeded),
            "failed" => ("failed", LogOutcome::Failed),
            "cancelled" => ("cancelled", LogOutcome::Cancelled),
            "recovering" => ("recover", LogOutcome::Started),
            "interrupted" => ("interrupted", LogOutcome::Waiting),
            "waiting_for_input" => ("waiting_for_input", LogOutcome::Waiting),
            "running" => ("execute", LogOutcome::Started),
            "planning" => ("plan", LogOutcome::Started),
            "cancelling" => ("request_cancel", LogOutcome::Started),
            "queued" => ("queued", LogOutcome::Started),
            _ => return Ok(()),
        },
        _ => return Ok(()),
    };
    let mut accepted = if edge == "operation.created" {
        acceptance_target(connection, owner, &action, &request)?
    } else {
        AcceptedLog::default()
    };
    if action == "backups.create" && step == "completed" {
        accepted.target = completed_backup_target(connection, owner, id)?;
    }
    committed(
        connection,
        owner,
        u64::try_from(occurred_at_ms).map_err(failure)?,
        CommittedStep {
            action: accepted.action.as_deref().unwrap_or(&action),
            step,
            outcome,
            target: accepted.target,
            execution_id: accepted
                .execution_id
                .or_else(|| (edge == "operation.created").then(|| id.to_string())),
            operation_id: Some(id),
            details: if outcome == LogOutcome::Failed {
                error
            } else {
                None
            },
        },
    )
}

pub(super) fn operation_phase(
    connection: &Connection,
    owner: &PrincipalId,
    id: OperationId,
    phase: &str,
    occurred_at_ms: i64,
) -> Result<(), StoreError> {
    operation_edge(
        connection,
        owner,
        id,
        "operation.progress",
        &serde_json::json!({ "phase": phase }).to_string(),
        occurred_at_ms,
    )
}

#[derive(Default)]
struct AcceptedLog {
    execution_id: Option<String>,
    target: Option<LogTarget>,
    action: Option<String>,
}

fn acceptance_target(
    connection: &Connection,
    owner: &PrincipalId,
    action: &str,
    request: &str,
) -> Result<AcceptedLog, StoreError> {
    let request: serde_json::Value = serde_json::from_str(request).map_err(failure)?;
    if action == "projects.create" {
        let name = request.get("name").and_then(serde_json::Value::as_str);
        let parent = request
            .get("targetParent")
            .and_then(serde_json::Value::as_str);
        return Ok(AcceptedLog {
            target: parent
                .zip(name)
                .map(|(parent, name)| destination_target(parent, name)),
            ..AcceptedLog::default()
        });
    }
    let plan = request.get("planId").and_then(serde_json::Value::as_str);
    let mut planned_action = None;
    if let Some(id) = plan {
        let recorded: Option<(String, String)> = connection.query_row(
            "SELECT target_json, action FROM operation_log_entries WHERE owner=?1 AND execution_id=?2 AND step='planned' ORDER BY sequence DESC LIMIT 1",
            params![owner.as_str(), id], |r| Ok((r.get(0)?, r.get(1)?)),
        ).optional().map_err(failure)?;
        if let Some((json, action)) = recorded {
            let target = serde_json::from_str::<Option<LogTarget>>(&json).map_err(failure)?;
            if target.is_some() {
                return Ok(AcceptedLog {
                    execution_id: Some(id.to_owned()),
                    target,
                    action: Some(action),
                });
            }
            planned_action = Some(action);
        }
    }
    let mut project = request
        .get("projectId")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    if project.is_none()
        && action == "packages.apply"
        && let Some(plan_id) = plan
    {
        project = connection
            .query_row(
                "SELECT project_id FROM package_plans WHERE owner_principal_id=?1 AND plan_id=?2",
                params![owner.as_str(), plan_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(failure)?;
    }
    let target = if let Some(id) = project {
        let path = connection
            .query_row(
                "SELECT root_path FROM projects WHERE owner_principal_id=?1 AND project_id=?2",
                params![owner.as_str(), id],
                |r| r.get(0),
            )
            .optional()
            .map_err(failure)?;
        Some(LogTarget {
            kind: "project".to_owned(),
            id: Some(id),
            name: None,
            path,
        })
    } else {
        None
    };
    Ok(AcceptedLog {
        execution_id: plan.map(str::to_owned),
        target,
        action: planned_action,
    })
}

fn completed_backup_target(
    connection: &Connection,
    owner: &PrincipalId,
    operation: OperationId,
) -> Result<Option<LogTarget>, StoreError> {
    let result: Option<String> = connection
        .query_row(
            "SELECT result_json FROM operations WHERE owner_principal_id=?1 AND operation_id=?2",
            params![owner.as_str(), operation.to_string()],
            |row| row.get(0),
        )
        .map_err(failure)?;
    let Some(result) = result else {
        return Ok(None);
    };
    let record: alcomd_application::BackupRecord =
        serde_json::from_str(&result).map_err(failure)?;
    let locator: Option<String> = connection
        .query_row(
            "SELECT archive_locator FROM backups WHERE owner_principal_id=?1 AND backup_id=?2",
            params![owner.as_str(), record.backup_id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(failure)?;
    Ok(locator.map(|locator| {
        let path = std::path::Path::new(&locator);
        LogTarget {
            kind: "backup".to_owned(),
            id: Some(record.backup_id.to_string()),
            name: path
                .file_name()
                .and_then(std::ffi::OsStr::to_str)
                .map(str::to_owned),
            path: path.is_absolute().then_some(locator),
        }
    }))
}

fn failure(error: impl std::error::Error + Send + Sync + 'static) -> StoreError {
    StoreError::new(StoreErrorKind::Unavailable).with_source(error)
}

pub(super) fn append(
    connection: &Connection,
    owner: &PrincipalId,
    entry: &OperationLogEntry,
) -> Result<(), StoreError> {
    if !entry.valid() {
        return Err(StoreError::new(StoreErrorKind::CorruptState));
    }
    let outcome = entry.outcome.as_str();
    connection.execute(
        "INSERT INTO operation_log_entries
         (entry_id, owner, execution_id, actor, operation_id, occurred_at_ms, action, step, outcome, target_json, details)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![entry.entry_id, owner.as_str(), entry.execution_id, entry.actor, entry.operation_id,
            i64::try_from(entry.occurred_at_ms).map_err(failure)?, entry.action, entry.step, outcome,
            serde_json::to_string(&entry.target).map_err(failure)?, entry.details],
    ).map_err(failure)?;
    Ok(())
}

fn row_entry(row: &Row<'_>) -> rusqlite::Result<OperationLogEntry> {
    let target: String = row.get(9)?;
    let outcome: String = row.get(8)?;
    let decode = |error: serde_json::Error| {
        rusqlite::Error::FromSqlConversionFailure(9, rusqlite::types::Type::Text, Box::new(error))
    };
    Ok(OperationLogEntry {
        sequence: nonnegative(row, 0)?,
        entry_id: row.get(1)?,
        execution_id: row.get(2)?,
        actor: row.get(3)?,
        operation_id: row.get(4)?,
        occurred_at_ms: nonnegative(row, 5)?,
        action: row.get(6)?,
        step: row.get(7)?,
        outcome: serde_json::from_value(serde_json::Value::String(outcome)).map_err(decode)?,
        target: serde_json::from_str(&target).map_err(decode)?,
        details: row.get(10)?,
    })
}

fn nonnegative(row: &Row<'_>, column: usize) -> rusqlite::Result<u64> {
    u64::try_from(row.get::<_, i64>(column)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Integer,
            Box::new(error),
        )
    })
}

pub(super) fn read(
    connection: &mut Connection,
    owner: &PrincipalId,
    query: OperationLogQuery,
) -> Result<OperationLogPage, StoreError> {
    if !query.valid() {
        return Err(StoreError::new(StoreErrorKind::CorruptState));
    }
    let transaction = connection.transaction().map_err(failure)?;
    let high_watermark = transaction
        .query_row(
            "SELECT coalesce(max(sequence), 0) FROM operation_log_entries WHERE owner=?1",
            [owner.as_str()],
            |r| nonnegative(r, 0),
        )
        .map_err(failure)?;
    let actors = {
        let mut statement = transaction
            .prepare("SELECT DISTINCT actor FROM operation_log_entries WHERE owner=?1 ORDER BY actor COLLATE BINARY LIMIT 257")
            .map_err(failure)?;
        statement
            .query_map([owner.as_str()], |row| row.get::<_, String>(0))
            .map_err(failure)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(failure)?
    };
    if actors.len() > 256 {
        return Err(StoreError::new(StoreErrorKind::Unavailable));
    }
    let forward = query.after_sequence.is_some();
    let sql = if forward {
        "SELECT sequence, entry_id, execution_id, actor, operation_id, occurred_at_ms, action, step, outcome, target_json, details
         FROM operation_log_entries WHERE owner=?1 AND sequence>?2
         AND (?4 IS NULL OR actor=?4) AND (?5 IS NULL OR outcome=?5)
         ORDER BY sequence ASC LIMIT ?3"
    } else {
        "SELECT sequence, entry_id, execution_id, actor, operation_id, occurred_at_ms, action, step, outcome, target_json, details
         FROM operation_log_entries WHERE owner=?1 AND sequence<?2
         AND (?4 IS NULL OR actor=?4) AND (?5 IS NULL OR outcome=?5)
         ORDER BY sequence DESC LIMIT ?3"
    };
    let cursor = i64::try_from(
        query
            .after_sequence
            .or(query.before_sequence)
            .unwrap_or(alcomd_application::LOG_MAX_SAFE_SEQUENCE + 1),
    )
    .map_err(failure)?;
    let search = query
        .filter
        .search
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    let operation_type = query.filter.operation_type;
    // Rust filters stream the matching owner range. A limit on the raw rows would
    // hide matches beyond the first unfiltered page.
    let scan_limit = if search.is_empty() && operation_type.is_none() {
        i64::from(query.limit) + 1
    } else {
        -1
    };
    let mut statement = transaction.prepare(sql).map_err(failure)?;
    let rows = statement
        .query_map(
            params![
                owner.as_str(),
                cursor,
                scan_limit,
                query.filter.actor,
                query.filter.outcome.map(LogOutcome::as_str)
            ],
            row_entry,
        )
        .map_err(failure)?;
    let mut entries = Vec::new();
    let mut bytes = serde_json::to_vec(&actors).map_err(failure)?.len();
    let mut has_more = false;
    for row in rows {
        let entry = row.map_err(failure)?;
        if operation_type.is_some_and(|kind| entry.operation_type() != Some(kind))
            || (!search.is_empty() && !entry.contains_search(&search))
        {
            continue;
        }
        if !entry.valid() {
            return Err(StoreError::new(StoreErrorKind::CorruptState));
        }
        let size = serde_json::to_vec(&entry).map_err(failure)?.len();
        // Reserve envelope space; stop at a complete record, never skip an overflow item.
        if entries.len() == query.limit as usize || bytes + size + 1 > 4 * 1024 * 1024 - 4096 {
            has_more = true;
            break;
        }
        bytes += size + 1;
        entries.push(entry);
    }
    let next_sequence = has_more.then(|| {
        entries
            .last()
            .expect("a complete record fits the frame")
            .sequence
    });
    if !forward {
        entries.reverse();
    }
    Ok(OperationLogPage {
        entries,
        actors,
        high_watermark,
        has_more,
        next_sequence,
    })
}

impl OperationLogStore for StateStoreHandle {
    async fn append_operation_logs(
        &self,
        owner: PrincipalId,
        entries: Vec<OperationLogEntry>,
    ) -> Result<(), StoreError> {
        self.request_worker(
            move |connection| {
                let transaction = connection.transaction().map_err(failure)?;
                for entry in entries {
                    append(&transaction, &owner, &entry)?;
                }
                transaction.commit().map_err(failure)
            },
            crate::sqlite::unavailable,
        )
        .await
    }

    async fn read_operation_logs(
        &self,
        owner: PrincipalId,
        query: OperationLogQuery,
    ) -> Result<OperationLogPage, StoreError> {
        self.request_worker(
            move |connection| read(connection, &owner, query),
            crate::sqlite::unavailable,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alcomd_application::{
        AccessContext, Application, EventInvalidation, Permission, StateStore,
    };
    use std::time::Duration;

    #[test]
    fn state21_upgrade_preserves_preexisting_state20_and_does_not_create_old_log_facts() {
        let directory = crate::events_tests::Directory::new();
        let path = directory.database();
        let mut connection = crate::sqlite::state21_fixture(&path);
        let owner = PrincipalId::local_owner();
        let operation = crate::sqlite::create_state_check(
            &mut connection,
            &owner,
            &alcomd_application::IdempotencyKey::parse("state21-existing-operation")
                .expect("fixture key"),
            25,
        )
        .expect("pre-existing operation");
        let snapshot = |connection: &Connection| {
            let row = connection
                .query_row(
                    "SELECT state,revision,request_json FROM operations WHERE operation_id=?1",
                    [operation.operation_id.to_string()],
                    |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, i64>(1)?,
                            r.get::<_, String>(2)?,
                        ))
                    },
                )
                .expect("snapshot operation");
            let event = connection
                .query_row(
                    "SELECT sequence,event_id,payload_json FROM events WHERE aggregate_id=?1",
                    [operation.operation_id.to_string()],
                    |r| {
                        Ok((
                            r.get::<_, i64>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, String>(2)?,
                        ))
                    },
                )
                .expect("snapshot event");
            (row, event)
        };
        let before = snapshot(&connection);
        connection
            .execute_batch("DROP TABLE operation_log_entries; PRAGMA user_version=20;")
            .expect("simulate previous isolated schema");
        drop(connection);
        let updated = crate::sqlite::initialize_connection(&path)
            .expect("migrate State20 through Core's Store");
        assert_eq!(snapshot(&updated), before);
        assert_eq!(
            updated
                .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                .expect("current schema"),
            i64::from(crate::CURRENT_DATA_SCHEMA)
        );
        assert_eq!(
            updated
                .query_row("SELECT count(*) FROM operation_log_entries", [], |r| r
                    .get::<_, i64>(0))
                .expect("new log table is empty"),
            0
        );
        assert_eq!(
            updated
                .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r
                    .get::<_, i64>(
                    0
                ))
                .expect("foreign key check"),
            0
        );
    }

    fn fact(actor: &str, details: Option<String>) -> OperationLogEntry {
        OperationLogEntry {
            sequence: 0,
            entry_id: OperationId::new().to_string(),
            execution_id: OperationId::new().to_string(),
            actor: actor.to_owned(),
            operation_id: None,
            occurred_at_ms: 1_793_937_603_000,
            action: "projects.register".to_owned(),
            step: "inspect_project".to_owned(),
            outcome: LogOutcome::Failed,
            target: Some(alcomd_application::log_path(
                "project",
                r"C:\ALCOMD-Test\项目\Packages\vpm-manifest.json".to_owned(),
            )),
            details,
        }
    }

    #[tokio::test]
    async fn log_commit_publishes_only_log_scope_and_history_survives_reopen() {
        let directory = crate::events_tests::Directory::new();
        let store = StateStoreHandle::open(directory.database()).expect("isolated store");
        let owner = PrincipalId::local_owner();
        let access = AccessContext::local_owner();
        let app = Application::new(store.clone());
        let mut subscriber = app
            .subscribe_events(&access, None, 10, true)
            .expect("subscribe");
        let mut ordinary = app
            .subscribe_events(&access, None, 10, false)
            .expect("subscribe without log scope");
        let no_permission = AccessContext::new(owner.clone(), [Permission::EventsRead]);
        assert!(matches!(
            app.subscribe_events(&no_permission, None, 10, true),
            Err(alcomd_application::ApplicationError::PermissionDenied)
        ));
        let entry = fact("CLI", Some("access denied (os error 5)".to_owned()));
        let pending = store.clone();
        let (entered, reached) = tokio::sync::oneshot::channel();
        let (release, resume) = std::sync::mpsc::channel();
        let expected = entry.clone();
        let caller = tokio::spawn(async move {
            pending
                .request_worker(
                    move |connection| {
                        let transaction = connection.transaction().map_err(failure)?;
                        append(&transaction, &owner, &entry)?;
                        entered.send(()).expect("test still observing");
                        resume.recv().expect("release transaction");
                        transaction.commit().map_err(failure)
                    },
                    crate::sqlite::unavailable,
                )
                .await
        });
        reached.await.expect("write reached pre-commit barrier");
        assert!(
            tokio::time::timeout(Duration::from_millis(20), subscriber.next_page())
                .await
                .is_err()
        );
        caller.abort();
        let _ = caller.await;
        release.send(()).expect("release after caller cancellation");
        let notification = tokio::time::timeout(Duration::from_secs(2), subscriber.next_page())
            .await
            .expect("commit woke subscriber")
            .expect("log scope");
        assert!(notification.events.is_empty());
        assert_eq!(
            notification.invalidated_scopes,
            [EventInvalidation::OperationLog]
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(20), ordinary.next_page())
                .await
                .is_err()
        );
        drop((subscriber, ordinary, app, store));
        let reopened = StateStoreHandle::open(directory.database()).expect("reopen same database");
        let page = alcomd_application::read_operation_logs(
            &reopened,
            &access,
            OperationLogQuery {
                limit: 10,
                ..Default::default()
            },
        )
        .await
        .expect("read persisted log");
        assert_eq!(page.high_watermark, 1);
        assert_eq!(
            page.entries,
            [OperationLogEntry {
                sequence: 1,
                ..expected
            }]
        );
    }

    #[tokio::test]
    async fn filters_cover_history_beyond_unfiltered_pages_and_keep_owner_isolation() {
        use alcomd_application::{LogFilter, OperationLogType};

        let directory = crate::events_tests::Directory::new();
        let store = StateStoreHandle::open(directory.database()).expect("isolated store");
        let owner = PrincipalId::local_owner();
        let foreign_owner = PrincipalId::parse("test:foreign-log-owner").expect("foreign owner");
        let failed = || fact("CLI", Some("Überprüfung fehlgeschlagen".to_owned()));
        let mut records = vec![
            failed(),
            OperationLogEntry {
                action: "projects.refresh".to_owned(),
                ..failed()
            },
            OperationLogEntry {
                outcome: LogOutcome::Succeeded,
                ..failed()
            },
            OperationLogEntry {
                actor: "Core".to_owned(),
                ..failed()
            },
            fact("MCP", None),
            failed(),
        ];
        records.extend((0..120).map(|_| fact("GUI", None)));
        store
            .append_operation_logs(owner.clone(), records)
            .await
            .expect("persist history");
        store
            .append_operation_logs(foreign_owner, vec![fact("foreign", None), failed()])
            .await
            .expect("persist foreign history");

        let query = OperationLogQuery {
            limit: 1,
            filter: LogFilter {
                search: Some(" ÜBERPRÜFUNG ".to_owned()),
                actor: Some("CLI".to_owned()),
                outcome: Some(LogOutcome::Failed),
                operation_type: Some(OperationLogType::Write),
            },
            ..Default::default()
        };
        for (start, first, second) in [(None, 6, 1), (Some(0), 1, 6)] {
            let newest = store
                .read_operation_logs(
                    owner.clone(),
                    OperationLogQuery {
                        after_sequence: start,
                        ..query.clone()
                    },
                )
                .await
                .expect("first filtered page");
            assert_eq!(newest.actors, ["CLI", "Core", "GUI", "MCP"]);
            assert_eq!(newest.high_watermark, 126);
            assert_eq!(newest.entries.len(), 1);
            assert_eq!(newest.entries[0].sequence, first);
            assert!(newest.has_more);
            assert_eq!(newest.next_sequence, Some(first));
            let next = store
                .read_operation_logs(
                    owner.clone(),
                    OperationLogQuery {
                        before_sequence: start.is_none().then_some(first),
                        after_sequence: start.map(|_| first),
                        ..query.clone()
                    },
                )
                .await
                .expect("next filtered page");
            assert_eq!(next.entries.len(), 1);
            assert_eq!(next.entries[0].sequence, second);
            assert!(!next.has_more);
            assert_eq!(next.next_sequence, None);
        }
    }

    #[tokio::test]
    async fn log_reads_are_bounded_and_oversize_batch_rolls_back() {
        let directory = crate::events_tests::Directory::new();
        let store = StateStoreHandle::open(directory.database()).expect("isolated store");
        let owner = PrincipalId::local_owner();
        let mut commits = store.event_commits();
        commits.borrow_and_update();
        let huge = fact(
            "GUI",
            Some("x".repeat(alcomd_application::LOG_MAX_DETAILS_BYTES + 1)),
        );
        assert!(
            store
                .append_operation_logs(owner.clone(), vec![fact("GUI", None), huge])
                .await
                .is_err()
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(20), commits.changed())
                .await
                .is_err()
        );
        let entries = (0..30)
            .map(|_| fact("GUI", Some("x".repeat(240 * 1024))))
            .collect();
        store
            .append_operation_logs(owner.clone(), entries)
            .await
            .expect("persist bounded records");
        let mut position = 0;
        let mut count = 0;
        loop {
            let page = store
                .read_operation_logs(
                    owner.clone(),
                    OperationLogQuery {
                        after_sequence: Some(position),
                        limit: 200,
                        ..Default::default()
                    },
                )
                .await
                .expect("bounded batch");
            assert!(
                serde_json::to_vec(&page.entries)
                    .expect("encode log batch")
                    .len()
                    < 4 * 1024 * 1024
            );
            assert_eq!(page.high_watermark, 30);
            for entry in page.entries {
                position += 1;
                assert_eq!(entry.sequence, position);
                count += 1;
            }
            if !page.has_more {
                break;
            }
            assert_eq!(page.next_sequence, Some(position));
        }
        assert_eq!(count, 30);
    }
}
