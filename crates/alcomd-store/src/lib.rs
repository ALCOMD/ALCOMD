//! SQLite persistence adapter for the M2 state/Operation vertical slice.
//!
//! One dedicated worker thread owns the only SQLite connection. Async callers
//! submit bounded commands and never run synchronous SQLite work on Tokio.

use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use alcomd_application::{
    ApplyPlanOutcome, BackupArchiveEvidence, BackupCreateOutcome, BackupCreateRequest,
    BackupCursor, BackupId, BackupOperationRecord, BackupPhase, BackupRestoreApplyOutcome,
    BackupRestoreOperationRecord, BackupRestorePhase, BackupRestorePlanDraft,
    BackupRestorePlanRecord, CheckClassification, CreateOperationOutcome, CreatedTemplateProject,
    EventPage, ExtensionApplyOutcome, ExtensionDataValue, ExtensionDataWriteResult,
    ExtensionFilesystemJournalEntry, ExtensionGrantMutation, ExtensionGrantRecord,
    ExtensionInstallPlanDraft, ExtensionInstanceLease, ExtensionPlanRecord, ExtensionRecord,
    ExtensionUninstallPlanDraft, FilesystemJournalEntry, IdempotencyKey, M3Error, M3RegistryStore,
    M4Error, M4Store, M5BackupError, M5BackupStore, M5TemplateError, M5TemplateStore, M5UnityError,
    M5UnityStore, M6Error, M6Store, M7CopyError, M7CopyStore, OperationCursor, OperationId,
    OperationPage, OperationRecord, PackageApplyCompletion, PackageCursor, PackagePage,
    PackagePlanDraft, PackagePlanRecord, PlanId, PrincipalId, ProjectCopyApplyOutcome,
    ProjectCopyInventoryEvidence, ProjectCopyOperationRecord, ProjectCopyPhase,
    ProjectCopyPlanDraft, ProjectCopyPlanOutcome, ProjectCopyPlanRecord, ProjectEditorPreference,
    ProjectEditorSelectionState, ProjectId, ProjectObservation, ProjectPage, ProjectRecord,
    PublishedProjectCopy, PublishedTemplate, RegistryCursor, RepositoryId, RepositoryObservation,
    RepositoryPage, RepositoryRecord, RepositoryValidators, ResolverCatalog, RestoredProject,
    Revision, StateCheckResult, StateStore, StoreError, StoredBackupRecord, StoredTemplateRecord,
    SyncWrite, TemplateApplyOutcome, TemplateCursor, TemplateId, TemplatePlanDraft,
    TemplatePlanRecord, UnityInstallationCursor, UnityInstallationId, UnityInstallationObservation,
    UnityInstallationPage, UnityInstallationRecord, UnityLaunchId, UnityLaunchRecord,
    UnityLaunchState, UnregisterResult,
};
use tokio::sync::{mpsc, oneshot};

mod m3;
mod m4;
mod m5;
mod m5_backup;
mod m5_backup_restore;
mod m5_template;
mod m6;
mod m7_copy;
mod m7_official;
mod sqlite;

/// Stable crate identifier used by repository checks.
pub const CRATE_NAME: &str = "alcomd-store";

/// Current supported SQLite data schema.
pub const CURRENT_DATA_SCHEMA: u32 = 12;

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

    async fn set_project_favorite(
        &self,
        owner: PrincipalId,
        id: ProjectId,
        favorite: bool,
        expected: Revision,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> Result<SyncWrite<ProjectRecord>, M3Error> {
        self.request_m3(move |connection| {
            m3::set_project_favorite(connection, &owner, id, favorite, expected, &key, now_ms)
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

impl M6Store for StateStoreHandle {
    async fn list_extensions(
        &self,
        owner: PrincipalId,
        cursor: Option<alcomd_application::ExtensionCursor>,
        limit: u32,
    ) -> Result<alcomd_application::ExtensionPage, M6Error> {
        self.request_worker(
            move |connection| m6::list_extensions(connection, &owner, cursor.as_ref(), limit),
            m6::unavailable,
        )
        .await
    }

    async fn get_extension(
        &self,
        owner: PrincipalId,
        extension_id: String,
    ) -> Result<ExtensionRecord, M6Error> {
        self.request_worker(
            move |connection| m6::get_extension(connection, &owner, &extension_id),
            m6::unavailable,
        )
        .await
    }

    async fn live_package_locator(
        &self,
        owner: PrincipalId,
        extension_id: String,
    ) -> Result<String, M6Error> {
        self.request_worker(
            move |connection| m6::live_package_locator(connection, &owner, &extension_id),
            m6::unavailable,
        )
        .await
    }

    async fn has_background_authority(
        &self,
        owner: PrincipalId,
        extension_id: String,
    ) -> Result<bool, M6Error> {
        self.request_worker(
            move |connection| m6::has_background_authority(connection, &owner, &extension_id),
            m6::unavailable,
        )
        .await
    }

    async fn create_install_plan(
        &self,
        owner: PrincipalId,
        draft: ExtensionInstallPlanDraft,
        now_ms: u64,
    ) -> Result<ExtensionPlanRecord, M6Error> {
        self.request_worker(
            move |connection| m6::create_install_plan(connection, &owner, draft, now_ms),
            m6::unavailable,
        )
        .await
    }

    async fn create_uninstall_plan(
        &self,
        owner: PrincipalId,
        draft: ExtensionUninstallPlanDraft,
        now_ms: u64,
    ) -> Result<ExtensionPlanRecord, M6Error> {
        self.request_worker(
            move |connection| m6::create_uninstall_plan(connection, &owner, draft, now_ms),
            m6::unavailable,
        )
        .await
    }

    async fn accept_plan(
        &self,
        owner: PrincipalId,
        plan_id: PlanId,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> Result<ExtensionApplyOutcome, M6Error> {
        self.request_worker(
            move |connection| m6::accept_plan(connection, &owner, plan_id, &key, now_ms),
            m6::unavailable,
        )
        .await
    }

    async fn get_plan(&self, plan_id: PlanId) -> Result<ExtensionPlanRecord, M6Error> {
        self.request_worker(
            move |connection| m6::get_plan(connection, plan_id),
            m6::unavailable,
        )
        .await
    }

    async fn begin_apply(
        &self,
        operation_id: OperationId,
        now_ms: u64,
    ) -> Result<ExtensionPlanRecord, M6Error> {
        self.request_worker(
            move |connection| m6::begin_apply(connection, operation_id, now_ms),
            m6::unavailable,
        )
        .await
    }

    async fn append_filesystem_journal(
        &self,
        entry: ExtensionFilesystemJournalEntry,
    ) -> Result<(), M6Error> {
        self.request_worker(
            move |connection| m6::append_filesystem_journal(connection, entry),
            m6::unavailable,
        )
        .await
    }

    async fn next_filesystem_journal_step(
        &self,
        operation_id: OperationId,
    ) -> Result<u64, M6Error> {
        self.request_worker(
            move |connection| m6::next_filesystem_journal_step(connection, operation_id),
            m6::unavailable,
        )
        .await
    }

    async fn filesystem_journal_has_phase(
        &self,
        operation_id: OperationId,
        phase: alcomd_application::ExtensionJournalPhase,
    ) -> Result<bool, M6Error> {
        self.request_worker(
            move |connection| m6::filesystem_journal_has_phase(connection, operation_id, phase),
            m6::unavailable,
        )
        .await
    }

    async fn recover_operations(&self, now_ms: u64) -> Result<Vec<OperationId>, M6Error> {
        self.request_worker(
            move |connection| m6::recover_operations(connection, now_ms),
            m6::unavailable,
        )
        .await
    }

    async fn finish_install(
        &self,
        operation_id: OperationId,
        live_locator: String,
        now_ms: u64,
    ) -> Result<(), M6Error> {
        self.request_worker(
            move |connection| m6::finish_install(connection, operation_id, &live_locator, now_ms),
            m6::unavailable,
        )
        .await
    }

    async fn finish_uninstall(
        &self,
        operation_id: OperationId,
        now_ms: u64,
    ) -> Result<(), M6Error> {
        self.request_worker(
            move |connection| m6::finish_uninstall(connection, operation_id, now_ms),
            m6::unavailable,
        )
        .await
    }

    async fn complete_operation(
        &self,
        operation_id: OperationId,
        now_ms: u64,
    ) -> Result<(), M6Error> {
        self.request_worker(
            move |connection| m6::complete_operation(connection, operation_id, now_ms),
            m6::unavailable,
        )
        .await
    }

    async fn fail_operation(
        &self,
        operation_id: OperationId,
        code: alcomd_application::M6ErrorCode,
        now_ms: u64,
    ) -> Result<(), M6Error> {
        self.request_worker(
            move |connection| m6::fail_operation(connection, operation_id, code, now_ms),
            m6::unavailable,
        )
        .await
    }

    async fn enable(
        &self,
        owner: PrincipalId,
        extension_id: String,
        expected: Revision,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> Result<ExtensionRecord, M6Error> {
        self.request_worker(
            move |connection| {
                m6::set_desired(
                    connection,
                    &owner,
                    &extension_id,
                    expected,
                    &key,
                    true,
                    now_ms,
                )
            },
            m6::unavailable,
        )
        .await
    }

    async fn disable(
        &self,
        owner: PrincipalId,
        extension_id: String,
        expected: Revision,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> Result<ExtensionRecord, M6Error> {
        self.request_worker(
            move |connection| {
                m6::set_desired(
                    connection,
                    &owner,
                    &extension_id,
                    expected,
                    &key,
                    false,
                    now_ms,
                )
            },
            m6::unavailable,
        )
        .await
    }

    async fn set_grant(
        &self,
        owner: PrincipalId,
        mutation: ExtensionGrantMutation,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> Result<ExtensionGrantRecord, M6Error> {
        self.request_worker(
            move |connection| {
                m6::set_grant(
                    connection,
                    &owner,
                    &mutation.extension_id,
                    &mutation.permission,
                    &mutation.resource_kind,
                    &mutation.resource_id,
                    mutation.expected_revision,
                    &key,
                    mutation.grant,
                    now_ms,
                )
            },
            m6::unavailable,
        )
        .await
    }

    async fn data_get(
        &self,
        lease: ExtensionInstanceLease,
        key: String,
        now_ms: u64,
    ) -> Result<Option<ExtensionDataValue>, M6Error> {
        self.request_worker(
            move |connection| m6::data_get(connection, &lease, &key, now_ms),
            m6::unavailable,
        )
        .await
    }

    async fn data_set(
        &self,
        lease: ExtensionInstanceLease,
        key: String,
        value: Vec<u8>,
        expected: Option<Revision>,
        now_ms: u64,
    ) -> Result<ExtensionDataWriteResult, M6Error> {
        self.request_worker(
            move |connection| m6::data_set(connection, &lease, &key, &value, expected, now_ms),
            m6::unavailable,
        )
        .await
    }

    async fn data_delete(
        &self,
        lease: ExtensionInstanceLease,
        key: String,
        expected: Revision,
        now_ms: u64,
    ) -> Result<ExtensionDataWriteResult, M6Error> {
        self.request_worker(
            move |connection| m6::data_delete(connection, &lease, &key, expected, now_ms),
            m6::unavailable,
        )
        .await
    }

    async fn prepare_instance(
        &self,
        owner: PrincipalId,
        extension_id: String,
        daemon_epoch: String,
        now_ms: u64,
    ) -> Result<alcomd_application::ExtensionStartContext, M6Error> {
        self.request_worker(
            move |connection| {
                m6::prepare_instance(connection, &owner, &extension_id, &daemon_epoch, now_ms)
            },
            m6::unavailable,
        )
        .await
    }

    async fn mark_instance_running(
        &self,
        lease: ExtensionInstanceLease,
        now_ms: u64,
    ) -> Result<(), M6Error> {
        self.request_worker(
            move |connection| m6::mark_instance_running(connection, &lease, now_ms),
            m6::unavailable,
        )
        .await
    }

    async fn mark_instance_stopped(
        &self,
        extension_id: String,
        _now_ms: u64,
    ) -> Result<(), M6Error> {
        self.request_worker(
            move |connection| m6::mark_instance_stopped(connection, &extension_id),
            m6::unavailable,
        )
        .await
    }

    async fn renew_instance(
        &self,
        lease: ExtensionInstanceLease,
        now_ms: u64,
    ) -> Result<ExtensionInstanceLease, M6Error> {
        self.request_worker(
            move |connection| m6::renew_instance(connection, &lease, now_ms),
            m6::unavailable,
        )
        .await
    }

    async fn record_instance_crash(
        &self,
        lease: ExtensionInstanceLease,
        reason: String,
        now_ms: u64,
    ) -> Result<alcomd_application::ExtensionCrashDecision, M6Error> {
        self.request_worker(
            move |connection| m6::record_instance_crash(connection, &lease, &reason, now_ms),
            m6::unavailable,
        )
        .await
    }

    async fn recover_instances(
        &self,
        daemon_epoch: String,
        now_ms: u64,
    ) -> Result<Vec<String>, M6Error> {
        self.request_worker(
            move |connection| m6::recover_instances(connection, &daemon_epoch, now_ms),
            m6::unavailable,
        )
        .await
    }

    async fn project_summary(
        &self,
        lease: ExtensionInstanceLease,
        project_id: String,
        now_ms: u64,
    ) -> Result<alcomd_application::ExtensionProjectSummary, M6Error> {
        self.request_worker(
            move |connection| m6::project_summary(connection, &lease, &project_id, now_ms),
            m6::unavailable,
        )
        .await
    }
}

impl M5UnityStore for StateStoreHandle {
    async fn register_installation(
        &self,
        owner: PrincipalId,
        observation: UnityInstallationObservation,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> Result<(UnityInstallationRecord, bool), M5UnityError> {
        self.request_worker(
            move |connection| {
                m5::register_installation(connection, &owner, observation, &key, now_ms)
            },
            m5::unavailable,
        )
        .await
    }

    async fn get_installation(
        &self,
        owner: PrincipalId,
        id: UnityInstallationId,
    ) -> Result<UnityInstallationRecord, M5UnityError> {
        self.request_worker(
            move |connection| m5::get_installation(connection, &owner, id),
            m5::unavailable,
        )
        .await
    }

    async fn list_installations(
        &self,
        owner: PrincipalId,
        cursor: Option<UnityInstallationCursor>,
        limit: u32,
    ) -> Result<UnityInstallationPage, M5UnityError> {
        self.request_worker(
            move |connection| m5::list_installations(connection, &owner, cursor, limit),
            m5::unavailable,
        )
        .await
    }

    async fn remove_installation(
        &self,
        owner: PrincipalId,
        id: UnityInstallationId,
        expected: Revision,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> Result<(bool, bool), M5UnityError> {
        self.request_worker(
            move |connection| {
                m5::remove_installation(connection, &owner, id, expected, &key, now_ms)
            },
            m5::unavailable,
        )
        .await
    }

    async fn synchronize_installations(
        &self,
        owner: PrincipalId,
        observations: Vec<UnityInstallationObservation>,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> Result<(UnityInstallationPage, bool), M5UnityError> {
        self.request_worker(
            move |connection| {
                m5::synchronize_installations(connection, &owner, observations, &key, now_ms)
            },
            m5::unavailable,
        )
        .await
    }

    async fn get_project_editor(
        &self,
        owner: PrincipalId,
        project_id: ProjectId,
    ) -> Result<ProjectEditorPreference, M5UnityError> {
        self.request_worker(
            move |connection| m5::get_project_editor(connection, &owner, project_id),
            m5::unavailable,
        )
        .await
    }

    async fn get_project_editor_selection(
        &self,
        owner: PrincipalId,
        project_id: ProjectId,
    ) -> Result<ProjectEditorSelectionState, M5UnityError> {
        self.request_worker(
            move |connection| m5::get_project_editor_selection(connection, &owner, project_id),
            m5::unavailable,
        )
        .await
    }

    async fn set_project_editor(
        &self,
        owner: PrincipalId,
        project_id: ProjectId,
        installation_id: UnityInstallationId,
        arguments: Vec<String>,
        expected: Option<Revision>,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> Result<(ProjectEditorPreference, bool), M5UnityError> {
        self.request_worker(
            move |connection| {
                m5::set_project_editor(
                    connection,
                    &owner,
                    project_id,
                    installation_id,
                    arguments,
                    expected,
                    &key,
                    now_ms,
                )
            },
            m5::unavailable,
        )
        .await
    }

    async fn clear_project_editor(
        &self,
        owner: PrincipalId,
        project_id: ProjectId,
        expected: Option<Revision>,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> Result<(ProjectEditorSelectionState, bool), M5UnityError> {
        self.request_worker(
            move |connection| {
                m5::clear_project_editor(connection, &owner, project_id, expected, &key, now_ms)
            },
            m5::unavailable,
        )
        .await
    }

    async fn accept_launch(
        &self,
        owner: PrincipalId,
        project: ProjectRecord,
        selection: ProjectEditorSelectionState,
        resolved_installation_id: UnityInstallationId,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> Result<(UnityLaunchRecord, bool), M5UnityError> {
        self.request_worker(
            move |connection| {
                m5::accept_launch(
                    connection,
                    &owner,
                    project,
                    selection,
                    resolved_installation_id,
                    &key,
                    now_ms,
                )
            },
            m5::unavailable,
        )
        .await
    }

    async fn replay_launch(
        &self,
        owner: PrincipalId,
        project: ProjectRecord,
        selection: ProjectEditorSelectionState,
        key: IdempotencyKey,
    ) -> Result<Option<UnityLaunchRecord>, M5UnityError> {
        self.request_worker(
            move |connection| m5::replay_launch(connection, &owner, &project, &selection, &key),
            m5::unavailable,
        )
        .await
    }

    async fn get_launch(
        &self,
        owner: PrincipalId,
        launch_id: UnityLaunchId,
    ) -> Result<UnityLaunchRecord, M5UnityError> {
        self.request_worker(
            move |connection| m5::get_launch(connection, &owner, launch_id),
            m5::unavailable,
        )
        .await
    }

    async fn set_launch_state(
        &self,
        owner: PrincipalId,
        launch_id: UnityLaunchId,
        state: UnityLaunchState,
        spawn_accepted: bool,
    ) -> Result<UnityLaunchRecord, M5UnityError> {
        self.request_worker(
            move |connection| {
                m5::set_launch_state(connection, &owner, launch_id, state, spawn_accepted)
            },
            m5::unavailable,
        )
        .await
    }
}

impl M5BackupStore for StateStoreHandle {
    async fn list_backups(
        &self,
        owner: PrincipalId,
        project_id: Option<ProjectId>,
        cursor: Option<BackupCursor>,
        limit: u32,
    ) -> Result<Vec<StoredBackupRecord>, M5BackupError> {
        self.request_worker(
            move |connection| {
                m5_backup::list_backups(connection, &owner, project_id, cursor, limit)
            },
            m5_backup::unavailable,
        )
        .await
    }

    async fn get_backup(
        &self,
        owner: PrincipalId,
        backup_id: BackupId,
    ) -> Result<StoredBackupRecord, M5BackupError> {
        self.request_worker(
            move |connection| m5_backup::get_backup(connection, &owner, backup_id),
            m5_backup::unavailable,
        )
        .await
    }

    async fn accept_backup_create(
        &self,
        owner: PrincipalId,
        request: BackupCreateRequest,
        key: IdempotencyKey,
    ) -> Result<BackupCreateOutcome, M5BackupError> {
        self.request_worker(
            move |connection| m5_backup::accept_backup_create(connection, &owner, request, &key),
            m5_backup::unavailable,
        )
        .await
    }

    async fn begin_backup_create(
        &self,
        operation_id: OperationId,
        updated_at_ms: u64,
    ) -> Result<BackupOperationRecord, M5BackupError> {
        self.request_worker(
            move |connection| {
                m5_backup::begin_backup_create(connection, operation_id, updated_at_ms)
            },
            m5_backup::unavailable,
        )
        .await
    }

    async fn record_backup_checkpoint(
        &self,
        operation_id: OperationId,
        phase: BackupPhase,
        evidence: Option<BackupArchiveEvidence>,
        updated_at_ms: u64,
    ) -> Result<(), M5BackupError> {
        self.request_worker(
            move |connection| {
                m5_backup::record_backup_checkpoint(
                    connection,
                    operation_id,
                    phase,
                    evidence,
                    updated_at_ms,
                )
            },
            m5_backup::unavailable,
        )
        .await
    }

    async fn complete_backup_create(
        &self,
        operation_id: OperationId,
        backup: StoredBackupRecord,
        completed_at_ms: u64,
    ) -> Result<(), M5BackupError> {
        self.request_worker(
            move |connection| {
                m5_backup::complete_backup_create(connection, operation_id, backup, completed_at_ms)
            },
            m5_backup::unavailable,
        )
        .await
    }

    async fn fail_backup_create(
        &self,
        operation_id: OperationId,
        error_code: String,
        diagnostic_id: String,
        completed_at_ms: u64,
    ) -> Result<(), M5BackupError> {
        self.request_worker(
            move |connection| {
                m5_backup::fail_backup_create(
                    connection,
                    operation_id,
                    &error_code,
                    &diagnostic_id,
                    completed_at_ms,
                )
            },
            m5_backup::unavailable,
        )
        .await
    }

    async fn defer_backup_recovery(
        &self,
        operation_id: OperationId,
        diagnostic_id: String,
        updated_at_ms: u64,
    ) -> Result<(), M5BackupError> {
        self.request_worker(
            move |connection| {
                m5_backup::defer_backup_recovery(
                    connection,
                    operation_id,
                    &diagnostic_id,
                    updated_at_ms,
                )
            },
            m5_backup::unavailable,
        )
        .await
    }

    async fn recover_backup_operations(
        &self,
        recovered_at_ms: u64,
    ) -> Result<Vec<OperationId>, M5BackupError> {
        self.request_worker(
            move |connection| m5_backup::recover_backup_operations(connection, recovered_at_ms),
            m5_backup::unavailable,
        )
        .await
    }

    async fn create_backup_restore_plan(
        &self,
        owner: PrincipalId,
        draft: BackupRestorePlanDraft,
        created_at_ms: u64,
    ) -> Result<BackupRestorePlanRecord, M5BackupError> {
        self.request_worker(
            move |connection| {
                m5_backup_restore::create_plan(connection, &owner, draft, created_at_ms)
            },
            m5_backup_restore::unavailable,
        )
        .await
    }

    async fn accept_backup_restore(
        &self,
        owner: PrincipalId,
        plan_id: PlanId,
        key: IdempotencyKey,
        created_at_ms: u64,
    ) -> Result<BackupRestoreApplyOutcome, M5BackupError> {
        self.request_worker(
            move |connection| {
                m5_backup_restore::accept(connection, &owner, plan_id, &key, created_at_ms)
            },
            m5_backup_restore::unavailable,
        )
        .await
    }

    async fn begin_backup_restore(
        &self,
        operation_id: OperationId,
        updated_at_ms: u64,
    ) -> Result<BackupRestoreOperationRecord, M5BackupError> {
        self.request_worker(
            move |connection| m5_backup_restore::begin(connection, operation_id, updated_at_ms),
            m5_backup_restore::unavailable,
        )
        .await
    }

    async fn record_backup_restore_checkpoint(
        &self,
        operation_id: OperationId,
        phase: BackupRestorePhase,
        restored: Option<RestoredProject>,
        updated_at_ms: u64,
    ) -> Result<(), M5BackupError> {
        self.request_worker(
            move |connection| {
                m5_backup_restore::checkpoint(
                    connection,
                    operation_id,
                    phase,
                    restored,
                    updated_at_ms,
                )
            },
            m5_backup_restore::unavailable,
        )
        .await
    }

    async fn complete_backup_restore(
        &self,
        operation_id: OperationId,
        restored: RestoredProject,
        completed_at_ms: u64,
    ) -> Result<(), M5BackupError> {
        self.request_worker(
            move |connection| {
                m5_backup_restore::complete(connection, operation_id, restored, completed_at_ms)
            },
            m5_backup_restore::unavailable,
        )
        .await
    }

    async fn finish_backup_restore_success(
        &self,
        operation_id: OperationId,
        completed_at_ms: u64,
    ) -> Result<(), M5BackupError> {
        self.request_worker(
            move |connection| {
                m5_backup_restore::finish_success(connection, operation_id, completed_at_ms)
            },
            m5_backup_restore::unavailable,
        )
        .await
    }

    async fn fail_backup_restore(
        &self,
        operation_id: OperationId,
        error_code: String,
        diagnostic_id: String,
        completed_at_ms: u64,
    ) -> Result<(), M5BackupError> {
        self.request_worker(
            move |connection| {
                m5_backup_restore::fail(
                    connection,
                    operation_id,
                    &error_code,
                    &diagnostic_id,
                    completed_at_ms,
                )
            },
            m5_backup_restore::unavailable,
        )
        .await
    }

    async fn recover_backup_restore_operations(
        &self,
        recovered_at_ms: u64,
    ) -> Result<Vec<OperationId>, M5BackupError> {
        self.request_worker(
            move |connection| m5_backup_restore::recover(connection, recovered_at_ms),
            m5_backup_restore::unavailable,
        )
        .await
    }

    async fn completed_backup_restores(
        &self,
    ) -> Result<Vec<(OperationId, BackupRestorePlanRecord)>, M5BackupError> {
        self.request_worker(
            |connection| m5_backup_restore::completed(connection),
            m5_backup_restore::unavailable,
        )
        .await
    }
}

impl M7CopyStore for StateStoreHandle {
    async fn create_project_copy_plan(
        &self,
        owner: PrincipalId,
        draft: ProjectCopyPlanDraft,
    ) -> Result<ProjectCopyPlanOutcome, M7CopyError> {
        self.request_worker(
            move |connection| m7_copy::create_plan(connection, &owner, draft),
            m7_copy::unavailable,
        )
        .await
    }

    async fn get_project_copy_plan(
        &self,
        owner: PrincipalId,
        plan_id: PlanId,
    ) -> Result<ProjectCopyPlanRecord, M7CopyError> {
        self.request_worker(
            move |connection| m7_copy::get_plan(connection, &owner, plan_id),
            m7_copy::unavailable,
        )
        .await
    }

    async fn replay_project_copy_apply(
        &self,
        owner: PrincipalId,
        plan_id: PlanId,
        expected_revision: Revision,
        key: IdempotencyKey,
    ) -> Result<Option<ProjectCopyApplyOutcome>, M7CopyError> {
        self.request_worker(
            move |connection| {
                m7_copy::replay_apply(connection, &owner, plan_id, expected_revision, &key)
            },
            m7_copy::unavailable,
        )
        .await
    }

    async fn accept_project_copy(
        &self,
        owner: PrincipalId,
        plan_id: PlanId,
        expected_revision: Revision,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> Result<ProjectCopyApplyOutcome, M7CopyError> {
        self.request_worker(
            move |connection| {
                m7_copy::accept(connection, &owner, plan_id, expected_revision, &key, now_ms)
            },
            m7_copy::unavailable,
        )
        .await
    }

    async fn begin_project_copy(
        &self,
        operation_id: OperationId,
        now_ms: u64,
    ) -> Result<ProjectCopyOperationRecord, M7CopyError> {
        self.request_worker(
            move |connection| m7_copy::begin_operation(connection, operation_id, now_ms),
            m7_copy::unavailable,
        )
        .await
    }

    async fn record_project_copy_checkpoint(
        &self,
        operation_id: OperationId,
        phase: ProjectCopyPhase,
        inventory: Option<ProjectCopyInventoryEvidence>,
        published: Option<PublishedProjectCopy>,
        now_ms: u64,
    ) -> Result<(), M7CopyError> {
        self.request_worker(
            move |connection| {
                m7_copy::checkpoint(
                    connection,
                    operation_id,
                    phase,
                    inventory,
                    published,
                    now_ms,
                )
            },
            m7_copy::unavailable,
        )
        .await
    }

    async fn complete_project_copy(
        &self,
        operation_id: OperationId,
        published: PublishedProjectCopy,
        now_ms: u64,
    ) -> Result<(), M7CopyError> {
        self.request_worker(
            move |connection| m7_copy::complete(connection, operation_id, published, now_ms),
            m7_copy::unavailable,
        )
        .await
    }

    async fn finish_project_copy_success(
        &self,
        operation_id: OperationId,
        now_ms: u64,
    ) -> Result<(), M7CopyError> {
        self.request_worker(
            move |connection| m7_copy::finish_success(connection, operation_id, now_ms),
            m7_copy::unavailable,
        )
        .await
    }

    async fn fail_project_copy(
        &self,
        operation_id: OperationId,
        code: String,
        diagnostic_id: String,
        now_ms: u64,
    ) -> Result<(), M7CopyError> {
        self.request_worker(
            move |connection| {
                m7_copy::fail(connection, operation_id, &code, &diagnostic_id, now_ms)
            },
            m7_copy::unavailable,
        )
        .await
    }

    async fn recover_project_copy_operations(
        &self,
        now_ms: u64,
    ) -> Result<Vec<OperationId>, M7CopyError> {
        self.request_worker(
            move |connection| m7_copy::recover(connection, now_ms),
            m7_copy::unavailable,
        )
        .await
    }

    async fn project_copy_cancel_requested(
        &self,
        operation_id: OperationId,
    ) -> Result<bool, M7CopyError> {
        self.request_worker(
            move |connection| m7_copy::cancellation_requested(connection, operation_id),
            m7_copy::unavailable,
        )
        .await
    }

    async fn finish_project_copy_cancelled(
        &self,
        operation_id: OperationId,
        now_ms: u64,
    ) -> Result<(), M7CopyError> {
        self.request_worker(
            move |connection| m7_copy::finish_cancelled(connection, operation_id, now_ms),
            m7_copy::unavailable,
        )
        .await
    }
}

impl M5TemplateStore for StateStoreHandle {
    async fn ensure_builtin_templates(
        &self,
        owner: PrincipalId,
        templates: Vec<StoredTemplateRecord>,
        now_ms: u64,
    ) -> Result<(), M5TemplateError> {
        self.request_worker(
            move |connection| {
                m5_template::ensure_builtin_templates(connection, &owner, templates, now_ms)
            },
            || M5TemplateError::new(alcomd_application::M5TemplateErrorCode::StoreUnavailable),
        )
        .await
    }

    async fn list_templates(
        &self,
        owner: PrincipalId,
        cursor: Option<TemplateCursor>,
        limit: u32,
    ) -> Result<Vec<StoredTemplateRecord>, M5TemplateError> {
        self.request_worker(
            move |connection| m5_template::list_templates(connection, &owner, cursor, limit),
            || M5TemplateError::new(alcomd_application::M5TemplateErrorCode::StoreUnavailable),
        )
        .await
    }

    async fn get_template(
        &self,
        owner: PrincipalId,
        template_id: TemplateId,
    ) -> Result<StoredTemplateRecord, M5TemplateError> {
        self.request_worker(
            move |connection| m5_template::get_template(connection, &owner, template_id),
            || M5TemplateError::new(alcomd_application::M5TemplateErrorCode::StoreUnavailable),
        )
        .await
    }

    async fn create_template_plan(
        &self,
        owner: PrincipalId,
        draft: TemplatePlanDraft,
        created_at_ms: u64,
    ) -> Result<TemplatePlanRecord, M5TemplateError> {
        self.request_worker(
            move |connection| {
                m5_template::create_template_plan(connection, &owner, draft, created_at_ms)
            },
            || M5TemplateError::new(alcomd_application::M5TemplateErrorCode::StoreUnavailable),
        )
        .await
    }

    async fn get_template_plan(
        &self,
        owner: PrincipalId,
        plan_id: PlanId,
    ) -> Result<TemplatePlanRecord, M5TemplateError> {
        self.request_worker(
            move |connection| m5_template::get_template_plan(connection, &owner, plan_id),
            || M5TemplateError::new(alcomd_application::M5TemplateErrorCode::StoreUnavailable),
        )
        .await
    }

    async fn accept_template_plan(
        &self,
        owner: PrincipalId,
        plan_id: PlanId,
        key: IdempotencyKey,
        created_at_ms: u64,
    ) -> Result<TemplateApplyOutcome, M5TemplateError> {
        self.request_worker(
            move |connection| {
                m5_template::accept_template_plan(connection, &owner, plan_id, &key, created_at_ms)
            },
            || M5TemplateError::new(alcomd_application::M5TemplateErrorCode::StoreUnavailable),
        )
        .await
    }

    async fn begin_template_apply(
        &self,
        operation_id: OperationId,
        updated_at_ms: u64,
    ) -> Result<TemplatePlanRecord, M5TemplateError> {
        self.request_worker(
            move |connection| {
                m5_template::begin_template_apply(connection, operation_id, updated_at_ms)
            },
            || M5TemplateError::new(alcomd_application::M5TemplateErrorCode::StoreUnavailable),
        )
        .await
    }

    async fn complete_template_apply(
        &self,
        operation_id: OperationId,
        template: PublishedTemplate,
        completed_at_ms: u64,
    ) -> Result<(), M5TemplateError> {
        self.request_worker(
            move |connection| {
                m5_template::complete_template_apply(
                    connection,
                    operation_id,
                    template,
                    completed_at_ms,
                )
            },
            || M5TemplateError::new(alcomd_application::M5TemplateErrorCode::StoreUnavailable),
        )
        .await
    }

    async fn record_template_checkpoint(
        &self,
        operation_id: OperationId,
        step: u64,
        phase: String,
        updated_at_ms: u64,
    ) -> Result<(), M5TemplateError> {
        self.request_worker(
            move |connection| {
                m5_template::record_template_checkpoint(
                    connection,
                    operation_id,
                    step,
                    &phase,
                    updated_at_ms,
                )
            },
            || M5TemplateError::new(alcomd_application::M5TemplateErrorCode::StoreUnavailable),
        )
        .await
    }

    async fn complete_template_project_create(
        &self,
        operation_id: OperationId,
        project: CreatedTemplateProject,
        completed_at_ms: u64,
    ) -> Result<(), M5TemplateError> {
        self.request_worker(
            move |connection| {
                m5_template::complete_template_project_create(
                    connection,
                    operation_id,
                    project,
                    completed_at_ms,
                )
            },
            || M5TemplateError::new(alcomd_application::M5TemplateErrorCode::StoreUnavailable),
        )
        .await
    }

    async fn fail_template_apply(
        &self,
        operation_id: OperationId,
        error_code: String,
        diagnostic_id: String,
        completed_at_ms: u64,
    ) -> Result<(), M5TemplateError> {
        self.request_worker(
            move |connection| {
                m5_template::fail_template_apply(
                    connection,
                    operation_id,
                    &error_code,
                    &diagnostic_id,
                    completed_at_ms,
                )
            },
            || M5TemplateError::new(alcomd_application::M5TemplateErrorCode::StoreUnavailable),
        )
        .await
    }

    async fn recover_template_operations(
        &self,
        recovered_at_ms: u64,
    ) -> Result<Vec<OperationId>, M5TemplateError> {
        self.request_worker(
            move |connection| m5_template::recover_template_operations(connection, recovered_at_ms),
            || M5TemplateError::new(alcomd_application::M5TemplateErrorCode::StoreUnavailable),
        )
        .await
    }

    async fn set_template_favorite(
        &self,
        owner: PrincipalId,
        template_id: TemplateId,
        favorite: bool,
        expected: Revision,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> Result<(StoredTemplateRecord, bool), M5TemplateError> {
        self.request_worker(
            move |connection| {
                m5_template::set_template_favorite(
                    connection,
                    &owner,
                    template_id,
                    favorite,
                    expected,
                    &key,
                    now_ms,
                )
            },
            || M5TemplateError::new(alcomd_application::M5TemplateErrorCode::StoreUnavailable),
        )
        .await
    }

    async fn remove_template(
        &self,
        owner: PrincipalId,
        template_id: TemplateId,
        expected: Revision,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> Result<(bool, bool), M5TemplateError> {
        self.request_worker(
            move |connection| {
                m5_template::remove_template(
                    connection,
                    &owner,
                    template_id,
                    expected,
                    &key,
                    now_ms,
                )
            },
            || M5TemplateError::new(alcomd_application::M5TemplateErrorCode::StoreUnavailable),
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
