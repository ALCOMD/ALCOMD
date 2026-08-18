//! SQLite persistence adapter for the M2 state/Operation vertical slice.
//!
//! One dedicated worker thread owns the only SQLite connection. Async callers
//! submit bounded commands and never run synchronous SQLite work on Tokio.

use std::fmt;
use std::path::PathBuf;
use std::thread;

use alcomd_application::{
    CheckClassification, CreateOperationOutcome, EventPage, IdempotencyKey, OperationCursor,
    OperationId, OperationPage, OperationRecord, PrincipalId, Revision, StateCheckResult,
    StateStore, StoreError,
};
use tokio::sync::{mpsc, oneshot};

mod sqlite;

/// Stable crate identifier used by repository checks.
pub const CRATE_NAME: &str = "alcomd-store";

/// Current supported SQLite data schema.
pub const CURRENT_DATA_SCHEMA: u32 = 1;

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
    commands: mpsc::Sender<Command>,
}

impl StateStoreHandle {
    /// Opens, migrates, verifies, and starts the single SQLite worker.
    pub fn open(path: PathBuf) -> Result<Self, StoreOpenError> {
        let (commands, receiver) = mpsc::channel(128);
        let (ready_sender, ready_receiver) = std::sync::mpsc::sync_channel(1);
        thread::Builder::new()
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
        Ok(Self { commands })
    }

    async fn request<T>(
        &self,
        create: impl FnOnce(oneshot::Sender<Result<T, StoreError>>) -> Command,
    ) -> Result<T, StoreError> {
        let (sender, receiver) = oneshot::channel();
        self.commands
            .send(create(sender))
            .await
            .map_err(|_| sqlite::unavailable())?;
        receiver.await.map_err(|_| sqlite::unavailable())?
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
