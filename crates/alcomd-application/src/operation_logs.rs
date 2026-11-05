//! Core-owned, append-only facts about business execution, independent of UI feedback.

use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::{AccessContext, ApplicationError, OperationId, Permission, PrincipalId, StoreError};

pub const LOG_MAX_SAFE_SEQUENCE: u64 = 9_007_199_254_740_991;
pub const LOG_MAX_ENTRY_BYTES: usize = 512 * 1024;
pub const LOG_MAX_DETAILS_BYTES: usize = 256 * 1024;
pub const LOG_MAX_PATH_BYTES: usize = 128 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogOutcome {
    Started,
    Succeeded,
    Failed,
    Cancelled,
    Waiting,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogTarget {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationLogEntry {
    pub sequence: u64,
    pub entry_id: String,
    pub execution_id: String,
    /// The invoking program/entry point, never an inferred human/AI identity.
    pub actor: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    pub occurred_at_ms: u64,
    pub action: String,
    pub step: String,
    pub outcome: LogOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<LogTarget>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

impl OperationLogEntry {
    pub fn valid(&self) -> bool {
        self.sequence <= LOG_MAX_SAFE_SEQUENCE
            && self.entry_id.len() == 36
            && self.execution_id.len() == 36
            && !self.actor.is_empty()
            && self.actor.len() <= 64
            && !self.action.is_empty()
            && self.action.len() <= 128
            && !self.step.is_empty()
            && self.step.len() <= 128
            && self
                .details
                .as_ref()
                .is_none_or(|s| s.len() <= LOG_MAX_DETAILS_BYTES)
            && self.target.as_ref().is_none_or(|t| {
                t.path
                    .as_ref()
                    .is_none_or(|s| s.len() <= LOG_MAX_PATH_BYTES)
            })
            && serde_json::to_vec(self).is_ok_and(|bytes| bytes.len() <= LOG_MAX_ENTRY_BYTES)
    }
}

#[derive(Clone, Debug, Default)]
pub struct OperationLogQuery {
    pub before_sequence: Option<u64>,
    pub after_sequence: Option<u64>,
    pub limit: u32,
}

impl OperationLogQuery {
    pub fn valid(&self) -> bool {
        (1..=200).contains(&self.limit)
            && !(self.before_sequence.is_some() && self.after_sequence.is_some())
            && self
                .before_sequence
                .is_none_or(|s| s <= LOG_MAX_SAFE_SEQUENCE)
            && self
                .after_sequence
                .is_none_or(|s| s <= LOG_MAX_SAFE_SEQUENCE)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationLogPage {
    pub entries: Vec<OperationLogEntry>,
    pub high_watermark: u64,
    pub has_more: bool,
    pub next_sequence: Option<u64>,
}

pub trait OperationLogStore: Clone + Send + Sync + 'static {
    fn append_operation_logs(
        &self,
        owner: PrincipalId,
        entries: Vec<OperationLogEntry>,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    fn read_operation_logs(
        &self,
        owner: PrincipalId,
        query: OperationLogQuery,
    ) -> impl Future<Output = Result<OperationLogPage, StoreError>> + Send;
}

type WriteFuture = Pin<Box<dyn Future<Output = Result<(), StoreError>> + Send>>;
type Writer = Arc<dyn Fn(PrincipalId, Vec<OperationLogEntry>) -> WriteFuture + Send + Sync>;

#[derive(Clone)]
pub struct LogCall {
    pub owner: PrincipalId,
    pub actor: String,
    pub execution_id: String,
    // Only explicit, named application steps enter this short buffer. A replay
    // never commits them. An actual store change flushes them in its transaction.
    pending: Arc<Mutex<Vec<OperationLogEntry>>>,
}

impl LogCall {
    pub fn take_steps(&self) -> Vec<OperationLogEntry> {
        std::mem::take(
            &mut *self
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }
}

#[derive(Clone, Default)]
pub struct LogWriteContext {
    pub call: Option<LogCall>,
    pub operation_failure: Option<(OperationId, String)>,
}

tokio::task_local! {
    static CALL: (LogCall, Writer);
    static OPERATION_FAILURE: (OperationId, String);
}

/// Bind existing connection metadata only. This does not record requests or responses.
pub async fn with_log_caller<S: OperationLogStore, T>(
    store: S,
    access: &AccessContext,
    body: impl Future<Output = T>,
) -> T {
    let writer: Writer = Arc::new(move |owner, entries| {
        let store = store.clone();
        Box::pin(async move { store.append_operation_logs(owner, entries).await })
    });
    let call = LogCall {
        owner: access.principal().clone(),
        actor: access.actor().to_owned(),
        execution_id: OperationId::new().to_string(),
        pending: Arc::default(),
    };
    CALL.scope((call, writer), body).await
}

pub fn log_write_context() -> LogWriteContext {
    LogWriteContext {
        call: CALL.try_with(|(call, _)| call.clone()).ok(),
        operation_failure: OPERATION_FAILURE.try_with(Clone::clone).ok(),
    }
}

/// Attach a source error to the transaction that records the operation's actual failure.
pub async fn with_operation_log_failure<T>(
    operation_id: OperationId,
    details: String,
    body: impl Future<Output = T>,
) -> T {
    OPERATION_FAILURE.scope((operation_id, details), body).await
}

pub fn operation_log_error(error: &(dyn Error + 'static), path: Option<&str>) -> String {
    let chain = crate::technical_error_chain(error);
    match path {
        Some(path) if !chain.contains(path) => format!("{chain}\nPath: {path}"),
        _ => chain,
    }
}

/// Finish a real non-database step once the owned external operation completed.
pub async fn flush_log_steps() {
    if let Ok((call, writer)) = CALL.try_with(Clone::clone) {
        let entries = call.take_steps();
        if !entries.is_empty() && writer(call.owner, entries).await.is_err() {
            eprintln!("operation log could not persist a completed external step");
        }
    }
}

pub async fn log_external_fact(
    action: &str,
    step: &str,
    outcome: LogOutcome,
    target: Option<LogTarget>,
    details: Option<String>,
) {
    let Ok((call, writer)) = CALL.try_with(Clone::clone) else {
        return;
    };
    let mut entries = call.take_steps();
    entries.push(OperationLogEntry {
        sequence: 0,
        entry_id: OperationId::new().to_string(),
        execution_id: call.execution_id,
        actor: call.actor,
        operation_id: None,
        occurred_at_ms: log_time_ms(),
        action: action.to_owned(),
        step: step.to_owned(),
        outcome,
        target,
        details,
    });
    if writer(call.owner, entries).await.is_err() {
        eprintln!("operation log could not persist an external step");
    }
}

/// A rejected authorized write has no commit on which to attach its failure.
/// Successful writes already record their actual committed fact in Store.
pub async fn log_transaction_step<T, E: Error + 'static>(
    action: &str,
    step: &str,
    target: Option<LogTarget>,
    work: impl Future<Output = Result<T, E>>,
) -> Result<T, E> {
    let result = work.await;
    if let Err(error) = &result {
        log_external_fact(
            action,
            step,
            LogOutcome::Failed,
            target,
            Some(operation_log_error(error, None)),
        )
        .await;
    }
    result
}

fn log_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        .unwrap_or(0)
}

/// Carry only this execution context into an already-owned asynchronous task.
pub fn carry_log_caller<T>(body: impl Future<Output = T>) -> impl Future<Output = T> {
    let caller = CALL.try_with(Clone::clone).ok();
    async move {
        if let Some(caller) = caller {
            CALL.scope(caller, body).await
        } else {
            body.await
        }
    }
}

pub fn log_path(kind: &str, path: String) -> LogTarget {
    LogTarget {
        kind: kind.to_owned(),
        id: None,
        name: None,
        path: Some(path),
    }
}

/// Record a real application step; successful checks join the subsequent store
/// transaction. A check that failed before a transaction gets its own fact.
pub async fn log_execution_step<T, E: Error + 'static>(
    action: &str,
    step: &str,
    target: Option<LogTarget>,
    body: impl Future<Output = Result<T, E>>,
    error_path: impl FnOnce(&E) -> Option<String>,
) -> Result<T, E> {
    let result = body.await;
    let Ok((call, writer)) = CALL.try_with(Clone::clone) else {
        return result;
    };
    let (outcome, target, details) = match &result {
        Ok(_) => (LogOutcome::Succeeded, target, None),
        Err(error) => {
            let target = error_path(error).map_or(target.clone(), |path| {
                let mut target = target.unwrap_or_else(|| log_path("file", path.clone()));
                target.path = Some(path);
                Some(target)
            });
            (
                LogOutcome::Failed,
                target,
                Some(crate::technical_error_chain(error)),
            )
        }
    };
    let occurred_at_ms = log_time_ms();
    let entry = OperationLogEntry {
        sequence: 0,
        entry_id: OperationId::new().to_string(),
        execution_id: call.execution_id.clone(),
        actor: call.actor.clone(),
        operation_id: None,
        occurred_at_ms,
        action: action.to_owned(),
        step: step.to_owned(),
        outcome,
        target,
        details,
    };
    call.pending
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .push(entry);
    if result.is_err() {
        let entries = call.take_steps();
        if writer(call.owner, entries).await.is_err() {
            // No recursive log attempt or invented entry. Preserve the original failure.
            eprintln!("operation log could not persist a failed step");
        }
    }
    result
}

pub async fn read_operation_logs<S: OperationLogStore>(
    store: &S,
    access: &AccessContext,
    query: OperationLogQuery,
) -> Result<OperationLogPage, ApplicationError> {
    access.require(Permission::OperationsRead)?;
    if access.principal() != &PrincipalId::local_owner() {
        return Err(ApplicationError::PermissionDenied);
    }
    if !query.valid() {
        return Err(ApplicationError::InvalidInput);
    }
    Ok(store
        .read_operation_logs(access.principal().clone(), query)
        .await?)
}
