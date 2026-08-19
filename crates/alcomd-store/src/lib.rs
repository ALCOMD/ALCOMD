//! SQLite persistence adapter for the M2 state/Operation vertical slice.
//!
//! One dedicated worker thread owns the only SQLite connection. Async callers
//! submit bounded commands and never run synchronous SQLite work on Tokio.

use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use alcomd_application::{
    ApplyPlanOutcome, CheckClassification, CreateOperationOutcome, EventPage,
    FilesystemJournalEntry, IdempotencyKey, M3Error, M3RegistryStore, M4Error, M4Store,
    OperationCursor, OperationId, OperationPage, OperationRecord, PackageApplyCompletion,
    PackageCursor, PackagePage, PackagePlanDraft, PackagePlanRecord, PlanId, PrincipalId,
    ProjectId, ProjectObservation, ProjectPage, ProjectRecord, RegistryCursor, RepositoryId,
    RepositoryObservation, RepositoryPage, RepositoryRecord, RepositoryValidators, ResolverCatalog,
    Revision, StateCheckResult, StateStore, StoreError, SyncWrite, UnregisterResult,
};
use tokio::sync::{mpsc, oneshot};

mod m3;
mod m4;
mod sqlite;

/// Stable crate identifier used by repository checks.
pub const CRATE_NAME: &str = "alcomd-store";

/// Current supported SQLite data schema.
pub const CURRENT_DATA_SCHEMA: u32 = 3;

/// Safe state-store initialization failure.
#[derive(Debug)]
pub enum StoreOpenError {
    /// The database uses a newer schema and was not modified.
    UnsupportedDataSchema {
        /// Version read before any initialization pragma or migration.
        found: u32,
        /// Highest version supported by this binary.
        supported: u32,
    },
    /// The worker thread could not be started or initialized.
    Unavailable,
}

impl fmt::Display for StoreOpenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedDataSchema { found, supported } => write!(
                formatter,
                "state data schema {found} is newer than supported schema {supported}"
            ),
            Self::Unavailable => formatter.write_str("state store initialization failed"),
        }
    }
}

impl std::error::Error for StoreOpenError {}

/// Cloneable async handle to the dedicated SQLite worker.
#[derive(Clone, Debug)]
pub struct StateStoreHandle {
    inner: Arc<StoreInner>,
}

#[derive(Debug)]
struct StoreInner {
    commands: Mutex<Option<mpsc::Sender<Command>>>,
    worker: Option<thread::JoinHandle<()>>,
}

impl Drop for StoreInner {
    fn drop(&mut self) {
        let commands = match self.commands.get_mut() {
            Ok(commands) => commands,
            Err(poisoned) => poisoned.into_inner(),
        };
        commands.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl StateStoreHandle {
    /// Opens, migrates, verifies, and starts the single SQLite worker.
    pub fn open(path: PathBuf) -> Result<Self, StoreOpenError> {
        let (commands, receiver) = mpsc::channel(128);
        let (ready_sender, ready_receiver) = std::sync::mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("alcomd-state-store".to_owned())
            .spawn(move || match sqlite::initialize_connection(&path) {
                Ok(connection) => {
                    let _ = ready_sender.send(Ok(()));
                    run_worker(connection, receiver);
                }
                Err(error) => {
                    let _ = ready_sender.send(Err(error));
                }
            })
            .map_err(|_| StoreOpenError::Unavailable)?;
        ready_receiver
            .recv()
            .map_err(|_| StoreOpenError::Unavailable)??;
        Ok(Self {
            inner: Arc::new(StoreInner {
                commands: Mutex::new(Some(commands)),
                worker: Some(worker),
            }),
        })
    }

    async fn request<T>(
        &self,
        create: impl FnOnce(oneshot::Sender<Result<T, StoreError>>) -> Command,
    ) -> Result<T, StoreError> {
        let (sender, receiver) = oneshot::channel();
        let commands = self
            .inner
            .commands
            .lock()
            .map_err(|_| sqlite::unavailable())?
            .as_ref()
            .cloned()
            .ok_or_else(sqlite::unavailable)?;
        commands
            .send(create(sender))
            .await
            .map_err(|_| sqlite::unavailable())?;
        receiver.await.map_err(|_| sqlite::unavailable())?
    }

    async fn request_m3<T: Send + 'static>(
        &self,
        work: impl FnOnce(&mut rusqlite::Connection) -> Result<T, M3Error> + Send + 'static,
    ) -> Result<T, M3Error> {
        self.request_worker(work, m3::unavailable).await
    }

    async fn request_worker<T: Send + 'static, E: Send + 'static>(
        &self,
        work: impl FnOnce(&mut rusqlite::Connection) -> Result<T, E> + Send + 'static,
        unavailable: impl Fn() -> E,
    ) -> Result<T, E> {
        let (reply, response) = oneshot::channel();
        let commands = self
            .inner
            .commands
            .lock()
            .map_err(|_| unavailable())?
            .as_ref()
            .cloned()
            .ok_or_else(&unavailable)?;
        commands
            .send(Command::M3(Box::new(move |connection| {
                let _ = reply.send(work(connection));
            })))
            .await
            .map_err(|_| unavailable())?;
        response.await.map_err(|_| unavailable())?
    }
}

impl M3RegistryStore for StateStoreHandle {
    async fn register_project(
        &self,
        owner: PrincipalId,
        observation: ProjectObservation,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> Result<SyncWrite<ProjectRecord>, M3Error> {
        self.request_m3(move |connection| {
            m3::register_project(connection, &owner, observation, &key, now_ms)
        })
        .await
    }

    async fn get_project(
        &self,
        owner: PrincipalId,
        id: ProjectId,
    ) -> Result<ProjectRecord, M3Error> {
        self.request_m3(move |connection| m3::get_project(connection, &owner, id))
            .await
    }

    async fn list_projects(
        &self,
        owner: PrincipalId,
        cursor: Option<RegistryCursor<ProjectId>>,
        limit: u32,
    ) -> Result<ProjectPage, M3Error> {
        self.request_m3(move |connection| m3::list_projects(connection, &owner, cursor, limit))
            .await
    }

    async fn refresh_project(
        &self,
        owner: PrincipalId,
        id: ProjectId,
        expected: Revision,
        observation: ProjectObservation,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> Result<SyncWrite<ProjectRecord>, M3Error> {
        self.request_m3(move |connection| {
            m3::refresh_project(connection, &owner, id, expected, observation, &key, now_ms)
        })
        .await
    }

    async fn unregister_project(
        &self,
        owner: PrincipalId,
        id: ProjectId,
        expected: Revision,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> Result<UnregisterResult<ProjectId>, M3Error> {
        self.request_m3(move |connection| {
            m3::unregister_project(connection, &owner, id, expected, &key, now_ms)
        })
        .await
    }

    async fn register_repository(
        &self,
        owner: PrincipalId,
        observation: RepositoryObservation,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> Result<SyncWrite<RepositoryRecord>, M3Error> {
        self.request_m3(move |connection| {
            m3::register_repository(connection, &owner, observation, &key, now_ms)
        })
        .await
    }

    async fn get_repository(
        &self,
        owner: PrincipalId,
        id: RepositoryId,
    ) -> Result<RepositoryRecord, M3Error> {
        self.request_m3(move |connection| m3::get_repository(connection, &owner, id))
            .await
    }

    async fn list_repositories(
        &self,
        owner: PrincipalId,
        cursor: Option<RegistryCursor<RepositoryId>>,
        limit: u32,
    ) -> Result<RepositoryPage, M3Error> {
        self.request_m3(move |connection| m3::list_repositories(connection, &owner, cursor, limit))
            .await
    }

    async fn list_repository_packages(
        &self,
        owner: PrincipalId,
        id: RepositoryId,
        cursor: Option<PackageCursor>,
        limit: u32,
    ) -> Result<PackagePage, M3Error> {
        self.request_m3(move |connection| {
            m3::list_repository_packages(connection, &owner, id, cursor, limit)
        })
        .await
    }

    async fn refresh_repository(
        &self,
        owner: PrincipalId,
        id: RepositoryId,
        expected: Revision,
        observation: RepositoryObservation,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> Result<SyncWrite<RepositoryRecord>, M3Error> {
        self.request_m3(move |connection| {
            m3::refresh_repository(connection, &owner, id, expected, observation, &key, now_ms)
        })
        .await
    }

    async fn update_repository_validators(
        &self,
        owner: PrincipalId,
        id: RepositoryId,
        expected: Revision,
        validators: RepositoryValidators,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> Result<SyncWrite<RepositoryRecord>, M3Error> {
        self.request_m3(move |connection| {
            m3::update_repository_validators(
                connection, &owner, id, expected, validators, &key, now_ms,
            )
        })
        .await
    }

    async fn unregister_repository(
        &self,
        owner: PrincipalId,
        id: RepositoryId,
        expected: Revision,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> Result<UnregisterResult<RepositoryId>, M3Error> {
        self.request_m3(move |connection| {
            m3::unregister_repository(connection, &owner, id, expected, &key, now_ms)
        })
        .await
    }
}

impl M4Store for StateStoreHandle {
    async fn resolver_catalog(&self, owner: PrincipalId) -> Result<ResolverCatalog, M4Error> {
        self.request_worker(
            move |connection| m4::resolver_catalog(connection, &owner),
            || M4Error::new(alcomd_application::M4ErrorCode::StoreUnavailable),
        )
        .await
    }

    async fn create_package_plan(
        &self,
        owner: PrincipalId,
        draft: PackagePlanDraft,
        created_at_ms: u64,
    ) -> Result<PackagePlanRecord, M4Error> {
        self.request_worker(
            move |connection| m4::create_package_plan(connection, &owner, draft, created_at_ms),
            || M4Error::new(alcomd_application::M4ErrorCode::StoreUnavailable),
        )
        .await
    }

    async fn get_package_plan(
        &self,
        owner: PrincipalId,
        plan_id: PlanId,
    ) -> Result<PackagePlanRecord, M4Error> {
        self.request_worker(
            move |connection| m4::get_package_plan(connection, &owner, plan_id),
            || M4Error::new(alcomd_application::M4ErrorCode::StoreUnavailable),
        )
        .await
    }

    async fn accept_package_plan(
        &self,
        owner: PrincipalId,
        plan_id: PlanId,
        expected_revision: Revision,
        idempotency_key: IdempotencyKey,
        created_at_ms: u64,
    ) -> Result<ApplyPlanOutcome, M4Error> {
        self.request_worker(
            move |connection| {
                m4::accept_package_plan(
                    connection,
                    &owner,
                    plan_id,
                    expected_revision,
                    &idempotency_key,
                    created_at_ms,
                )
            },
            || M4Error::new(alcomd_application::M4ErrorCode::StoreUnavailable),
        )
        .await
    }

    async fn append_filesystem_journal(
        &self,
        entry: FilesystemJournalEntry,
    ) -> Result<(), M4Error> {
        self.request_worker(
            move |connection| m4::append_filesystem_journal(connection, entry),
            || M4Error::new(alcomd_application::M4ErrorCode::StoreUnavailable),
        )
        .await
    }

    async fn next_filesystem_journal_step(
        &self,
        operation_id: OperationId,
    ) -> Result<u64, M4Error> {
        self.request_worker(
            move |connection| m4::next_filesystem_journal_step(connection, operation_id),
            || M4Error::new(alcomd_application::M4ErrorCode::StoreUnavailable),
        )
        .await
    }

    async fn begin_package_apply(
        &self,
        operation_id: OperationId,
        updated_at_ms: u64,
    ) -> Result<PackagePlanRecord, M4Error> {
        self.request_worker(
            move |connection| m4::begin_package_apply(connection, operation_id, updated_at_ms),
            || M4Error::new(alcomd_application::M4ErrorCode::StoreUnavailable),
        )
        .await
    }

    async fn complete_package_apply(
        &self,
        operation_id: OperationId,
        completion: PackageApplyCompletion,
        completed_at_ms: u64,
    ) -> Result<(), M4Error> {
        self.request_worker(
            move |connection| {
                m4::complete_package_apply(connection, operation_id, completion, completed_at_ms)
            },
            || M4Error::new(alcomd_application::M4ErrorCode::StoreUnavailable),
        )
        .await
    }

    async fn fail_package_apply(
        &self,
        operation_id: OperationId,
        error_code: String,
        diagnostic_id: String,
        completed_at_ms: u64,
    ) -> Result<(), M4Error> {
        self.request_worker(
            move |connection| {
                m4::fail_package_apply(
                    connection,
                    operation_id,
                    &error_code,
                    &diagnostic_id,
                    completed_at_ms,
                )
            },
            || M4Error::new(alcomd_application::M4ErrorCode::StoreUnavailable),
        )
        .await
    }

    async fn recover_package_operations(
        &self,
        recovered_at_ms: u64,
    ) -> Result<Vec<OperationId>, M4Error> {
        self.request_worker(
            move |connection| m4::recover_package_operations(connection, recovered_at_ms),
            || M4Error::new(alcomd_application::M4ErrorCode::StoreUnavailable),
        )
        .await
    }
}

impl StateStore for StateStoreHandle {
    async fn create_state_check(
        &self,
        owner: PrincipalId,
        idempotency_key: IdempotencyKey,
        created_at_ms: u64,
    ) -> Result<CreateOperationOutcome, StoreError> {
        self.request(|reply| Command::CreateStateCheck {
            owner,
            idempotency_key,
            created_at_ms,
            reply,
        })
        .await
    }

    async fn get_operation(
        &self,
        owner: PrincipalId,
        operation_id: OperationId,
    ) -> Result<OperationRecord, StoreError> {
        self.request(|reply| Command::GetOperation {
            owner,
            operation_id,
            reply,
        })
        .await
    }

    async fn list_operations(
        &self,
        owner: PrincipalId,
        cursor: Option<OperationCursor>,
        limit: u32,
    ) -> Result<OperationPage, StoreError> {
        self.request(|reply| Command::ListOperations {
            owner,
            cursor,
            limit,
            reply,
        })
        .await
    }

    async fn cancel_operation(
        &self,
        owner: PrincipalId,
        operation_id: OperationId,
        expected_revision: Revision,
        idempotency_key: IdempotencyKey,
        updated_at_ms: u64,
    ) -> Result<(OperationRecord, bool), StoreError> {
        self.request(|reply| Command::CancelOperation {
            owner,
            operation_id,
            expected_revision,
            idempotency_key,
            updated_at_ms,
            reply,
        })
        .await
    }

    async fn list_events(
        &self,
        owner: PrincipalId,
        after_sequence: u64,
        limit: u32,
    ) -> Result<EventPage, StoreError> {
        self.request(|reply| Command::ListEvents {
            owner,
            after_sequence,
            limit,
            reply,
        })
        .await
    }

    async fn begin_state_check(
        &self,
        operation_id: OperationId,
        updated_at_ms: u64,
    ) -> Result<OperationRecord, StoreError> {
        self.request(|reply| Command::BeginStateCheck {
            operation_id,
            updated_at_ms,
            reply,
        })
        .await
    }

    async fn check_integrity(&self) -> Result<CheckClassification, StoreError> {
        self.request(|reply| Command::CheckIntegrity { reply })
            .await
    }

    async fn check_foreign_keys(&self) -> Result<CheckClassification, StoreError> {
        self.request(|reply| Command::CheckForeignKeys { reply })
            .await
    }

    async fn cancellation_requested(&self, operation_id: OperationId) -> Result<bool, StoreError> {
        self.request(|reply| Command::CancellationRequested {
            operation_id,
            reply,
        })
        .await
    }

    async fn finish_state_check(
        &self,
        operation_id: OperationId,
        result: StateCheckResult,
        completed_at_ms: u64,
    ) -> Result<OperationRecord, StoreError> {
        self.request(|reply| Command::FinishStateCheck {
            operation_id,
            result,
            completed_at_ms,
            reply,
        })
        .await
    }

    async fn finish_cancelled(
        &self,
        operation_id: OperationId,
        completed_at_ms: u64,
    ) -> Result<OperationRecord, StoreError> {
        self.request(|reply| Command::FinishCancelled {
            operation_id,
            completed_at_ms,
            reply,
        })
        .await
    }

    async fn finish_failed(
        &self,
        operation_id: OperationId,
        error_code: String,
        diagnostic_id: String,
        completed_at_ms: u64,
    ) -> Result<OperationRecord, StoreError> {
        self.request(|reply| Command::FinishFailed {
            operation_id,
            error_code,
            diagnostic_id,
            completed_at_ms,
            reply,
        })
        .await
    }

    async fn recover(&self, recovered_at_ms: u64) -> Result<Vec<OperationId>, StoreError> {
        self.request(|reply| Command::Recover {
            recovered_at_ms,
            reply,
        })
        .await
    }
}

enum Command {
    M3(Box<dyn FnOnce(&mut rusqlite::Connection) + Send>),
    CreateStateCheck {
        owner: PrincipalId,
        idempotency_key: IdempotencyKey,
        created_at_ms: u64,
        reply: oneshot::Sender<Result<CreateOperationOutcome, StoreError>>,
    },
    GetOperation {
        owner: PrincipalId,
        operation_id: OperationId,
        reply: oneshot::Sender<Result<OperationRecord, StoreError>>,
    },
    ListOperations {
        owner: PrincipalId,
        cursor: Option<OperationCursor>,
        limit: u32,
        reply: oneshot::Sender<Result<OperationPage, StoreError>>,
    },
    CancelOperation {
        owner: PrincipalId,
        operation_id: OperationId,
        expected_revision: Revision,
        idempotency_key: IdempotencyKey,
        updated_at_ms: u64,
        reply: oneshot::Sender<Result<(OperationRecord, bool), StoreError>>,
    },
    ListEvents {
        owner: PrincipalId,
        after_sequence: u64,
        limit: u32,
        reply: oneshot::Sender<Result<EventPage, StoreError>>,
    },
    BeginStateCheck {
        operation_id: OperationId,
        updated_at_ms: u64,
        reply: oneshot::Sender<Result<OperationRecord, StoreError>>,
    },
    CheckIntegrity {
        reply: oneshot::Sender<Result<CheckClassification, StoreError>>,
    },
    CheckForeignKeys {
        reply: oneshot::Sender<Result<CheckClassification, StoreError>>,
    },
    CancellationRequested {
        operation_id: OperationId,
        reply: oneshot::Sender<Result<bool, StoreError>>,
    },
    FinishStateCheck {
        operation_id: OperationId,
        result: StateCheckResult,
        completed_at_ms: u64,
        reply: oneshot::Sender<Result<OperationRecord, StoreError>>,
    },
    FinishCancelled {
        operation_id: OperationId,
        completed_at_ms: u64,
        reply: oneshot::Sender<Result<OperationRecord, StoreError>>,
    },
    FinishFailed {
        operation_id: OperationId,
        error_code: String,
        diagnostic_id: String,
        completed_at_ms: u64,
        reply: oneshot::Sender<Result<OperationRecord, StoreError>>,
    },
    Recover {
        recovered_at_ms: u64,
        reply: oneshot::Sender<Result<Vec<OperationId>, StoreError>>,
    },
}

fn run_worker(mut connection: rusqlite::Connection, mut receiver: mpsc::Receiver<Command>) {
    while let Some(command) = receiver.blocking_recv() {
        match command {
            Command::M3(work) => work(&mut connection),
            Command::CreateStateCheck {
                owner,
                idempotency_key,
                created_at_ms,
                reply,
            } => {
                let _ = reply.send(sqlite::create_state_check(
                    &mut connection,
                    &owner,
                    &idempotency_key,
                    created_at_ms,
                ));
            }
            Command::GetOperation {
                owner,
                operation_id,
                reply,
            } => {
                let _ = reply.send(sqlite::load_owned_operation(
                    &connection,
                    &owner,
                    operation_id,
                ));
            }
            Command::ListOperations {
                owner,
                cursor,
                limit,
                reply,
            } => {
                let _ = reply.send(sqlite::list_operations(&connection, &owner, cursor, limit));
            }
            Command::CancelOperation {
                owner,
                operation_id,
                expected_revision,
                idempotency_key,
                updated_at_ms,
                reply,
            } => {
                let _ = reply.send(sqlite::cancel_operation(
                    &mut connection,
                    &owner,
                    operation_id,
                    expected_revision,
                    &idempotency_key,
                    updated_at_ms,
                ));
            }
            Command::ListEvents {
                owner,
                after_sequence,
                limit,
                reply,
            } => {
                let _ = reply.send(sqlite::list_events(
                    &connection,
                    &owner,
                    after_sequence,
                    limit,
                ));
            }
            Command::BeginStateCheck {
                operation_id,
                updated_at_ms,
                reply,
            } => {
                let _ = reply.send(sqlite::begin_state_check(
                    &mut connection,
                    operation_id,
                    updated_at_ms,
                ));
            }
            Command::CheckIntegrity { reply } => {
                let _ = reply.send(sqlite::check_integrity(&connection));
            }
            Command::CheckForeignKeys { reply } => {
                let _ = reply.send(sqlite::check_foreign_keys(&connection));
            }
            Command::CancellationRequested {
                operation_id,
                reply,
            } => {
                let _ = reply.send(sqlite::cancellation_requested(&connection, operation_id));
            }
            Command::FinishStateCheck {
                operation_id,
                result,
                completed_at_ms,
                reply,
            } => {
                let _ = reply.send(sqlite::finish_state_check(
                    &mut connection,
                    operation_id,
                    result,
                    completed_at_ms,
                ));
            }
            Command::FinishCancelled {
                operation_id,
                completed_at_ms,
                reply,
            } => {
                let _ = reply.send(sqlite::finish_cancelled(
                    &mut connection,
                    operation_id,
                    completed_at_ms,
                ));
            }
            Command::FinishFailed {
                operation_id,
                error_code,
                diagnostic_id,
                completed_at_ms,
                reply,
            } => {
                let _ = reply.send(sqlite::finish_failed(
                    &mut connection,
                    operation_id,
                    &error_code,
                    &diagnostic_id,
                    completed_at_ms,
                ));
            }
            Command::Recover {
                recovered_at_ms,
                reply,
            } => {
                let _ = reply.send(sqlite::recover(&mut connection, recovered_at_ms));
            }
        }
    }
}
