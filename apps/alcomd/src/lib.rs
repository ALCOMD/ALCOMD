//! M2 daemon lifecycle, local RPC transport, and application adapter.

use std::collections::HashSet;
use std::future::Future;
use std::io;
use std::sync::Arc;

use alcomd_application::{
    AccessContext, Application, ApplicationError, EventRecord as ApplicationEvent, IdempotencyKey,
    M3Application, M4Application, M5BackupApplication as BackupService,
    M5TemplateApplication as TemplateService, M5UnityApplication as M5Application, M6Application,
    M7Application, M7CopyApplication as ProjectCopyService,
    M7DeleteApplication as ProjectDeleteService, M7OfficialApplication,
    M7UnityMigrationApplication as UnityMigrationService, OperationCursor, OperationId,
    OperationRecord, OperationState as DomainState, ResourceLockCoordinator, Revision,
    StoreErrorKind, UserPackageApplication as UserPackageService,
};
use alcomd_platform::{BindError, DaemonInstance, DataConfig, IpcConfig, IpcListener, IpcStream};
use alcomd_protocol::{
    CAPABILITY_EVENTS_REPLAY_V1, CAPABILITY_OPERATIONS_V1, CAPABILITY_STATE_CHECK_V1,
    ErrorResponse, Event, EventsListParams, EventsListResult, HelloParams, HelloResult,
    METHOD_EVENTS_LIST, METHOD_OPERATIONS_CANCEL, METHOD_OPERATIONS_GET, METHOD_OPERATIONS_LIST,
    METHOD_STATE_CHECK, METHOD_SYSTEM_HELLO, METHOD_SYSTEM_STATUS, Operation, OperationAccepted,
    OperationProgress, OperationState, OperationWriteResult, OperationsCancelParams,
    OperationsGetParams, OperationsListCursor, OperationsListParams, OperationsListResult,
    PackageOperationPhase, RPC_VERSION, RequestEnvelope, RpcError, StateCheckParams,
    SuccessResponse, SystemStatusResult, decode_frame_length, encode_frame,
};
use alcomd_store::StateStoreHandle;
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::Instrument;

mod diagnostic_log_rpc;
mod events_rpc;
mod file_log;
mod hub_platform;
mod m3_rpc;
mod m4_rpc;
mod m5_backup_rpc;
mod m5_platform;
mod m5_rpc;
mod m5_template_rpc;
mod m6_rpc;
mod m6_runtime;
mod m7_candidates_rpc;
mod m7_copy_rpc;
mod m7_delete_rpc;
mod m7_official_rpc;
mod m7_rpc;
mod m7_unity_migration_rpc;
mod m7_user_packages_rpc;
mod operation_log;
mod project_discovery_platform;
mod repository_list_rpc;
mod rpc_log;
mod storage_platform;

#[cfg(feature = "test-kill-gates")]
pub mod test_support;

type M2Application = Application<StateStoreHandle>;
type M3ReadApplication = M3Application<StateStoreHandle, alcomd_vpm::VpmReader>;
type ProjectDiscoveryApplication = alcomd_application::ProjectDiscoveryApplication<
    StateStoreHandle,
    alcomd_vpm::VpmReader,
    project_discovery_platform::PlatformProjectDiscovery,
>;
type RepositoryListApplication = alcomd_application::RepositoryListApplication<
    StateStoreHandle,
    alcomd_vpm::RepositoryListFileAdapter,
>;
type M4PackageApplication =
    M4Application<StateStoreHandle, alcomd_vpm::PackageEngine<StateStoreHandle>>;
type M5UnityApplication = M5Application<StateStoreHandle, m5_platform::PlatformUnityAdapter>;
type TemplateApplication =
    TemplateService<StateStoreHandle, alcomd_vpm::TemplateEngine, M5UnityApplication>;
type BackupApplication =
    BackupService<StateStoreHandle, alcomd_vpm::BackupEngine, M5UnityApplication>;
type M6ExtensionApplication = M6Application<
    StateStoreHandle,
    alcomd_extensions::ExtensionEngine<StateStoreHandle>,
    m6_runtime::PlatformExtensionRuntime,
>;
type M7ExtensionApplication = M7Application<
    StateStoreHandle,
    alcomd_extensions::ExtensionEngine<StateStoreHandle>,
    m6_runtime::PlatformExtensionRuntime,
    m7_rpc::ProtocolUiValidator,
>;
type M7OfficialGuiApplication = M7OfficialApplication<StateStoreHandle>;
type ProjectCandidateApplication = alcomd_application::ProjectCandidateApplication<
    StateStoreHandle,
    alcomd_vpm::ProjectCandidateEngine,
>;
type ProjectCopyApplication =
    ProjectCopyService<StateStoreHandle, alcomd_vpm::ProjectCopyEngine, M5UnityApplication>;
type ProjectDeleteApplication =
    ProjectDeleteService<StateStoreHandle, alcomd_vpm::ProjectDeleteEngine, M5UnityApplication>;
type UserPackageApplication = UserPackageService<StateStoreHandle, alcomd_vpm::UserPackageEngine>;
type UnityMigrationApplication = UnityMigrationService<
    StateStoreHandle,
    alcomd_vpm::UnityMigrationEngine<StateStoreHandle>,
    M5UnityApplication,
>;

struct Applications {
    diagnostic_log: alcomd_application::DiagnosticLog,
    _file_log: Option<file_log::FileLog>,
    m2: M2Application,
    m3: M3ReadApplication,
    project_discovery: ProjectDiscoveryApplication,
    repository_lists: RepositoryListApplication,
    m4: M4PackageApplication,
    m5: M5UnityApplication,
    templates: TemplateApplication,
    backups: BackupApplication,
    m6: M6ExtensionApplication,
    m7: M7ExtensionApplication,
    official_gui: M7OfficialGuiApplication,
    candidates: ProjectCandidateApplication,
    project_copy: ProjectCopyApplication,
    project_delete: ProjectDeleteApplication,
    user_packages: UserPackageApplication,
    unity_migration: UnityMigrationApplication,
}

/// Initializes the authoritative store, recovers Operations, then binds IPC.
pub async fn serve_with_data_until<F>(
    ipc: IpcConfig,
    data: DataConfig,
    shutdown: F,
) -> Result<(), BindError>
where
    F: Future<Output = ()>,
{
    let (mut listener, applications) = initialize_daemon(ipc, data).await?;
    run_listener(&mut listener, applications, shutdown)
        .await
        .map_err(BindError::Io)
}

/// Starts Core with the same safe log source used by its tracing subscriber.
pub async fn serve_with_data_and_diagnostics_until<F>(
    ipc: IpcConfig,
    data: DataConfig,
    diagnostic_log: alcomd_application::DiagnosticLog,
    shutdown: F,
) -> Result<(), BindError>
where
    F: Future<Output = ()>,
{
    let (mut listener, applications) = initialize_daemon_with_diagnostics(
        ipc,
        data,
        Arc::new(storage_platform::PlatformStorageDirectories),
        diagnostic_log,
    )
    .await?;
    run_listener(&mut listener, applications, shutdown)
        .await
        .map_err(BindError::Io)
}

async fn initialize_daemon(
    ipc: IpcConfig,
    data: DataConfig,
) -> Result<(IpcListener, Arc<Applications>), BindError> {
    initialize_daemon_with_directories(
        ipc,
        data,
        Arc::new(storage_platform::PlatformStorageDirectories),
    )
    .await
}

async fn initialize_daemon_with_directories(
    ipc: IpcConfig,
    data: DataConfig,
    directory_platform: Arc<dyn alcomd_application::StorageDirectoryPlatform>,
) -> Result<(IpcListener, Arc<Applications>), BindError> {
    initialize_daemon_with_diagnostics(
        ipc,
        data,
        directory_platform,
        alcomd_application::DiagnosticLog::new(),
    )
    .await
}

async fn initialize_daemon_with_diagnostics(
    ipc: IpcConfig,
    data: DataConfig,
    directory_platform: Arc<dyn alcomd_application::StorageDirectoryPlatform>,
    diagnostic_log: alcomd_application::DiagnosticLog,
) -> Result<(IpcListener, Arc<Applications>), BindError> {
    let instance = DaemonInstance::acquire(&ipc)?;
    let database = alcomd_platform::state_database_path(&data)?;
    let data_root = database
        .parent()
        .ok_or_else(|| BindError::Io(io::Error::other("state path has no parent")))?
        .to_path_buf();
    let cache_root = data_root.join("package-cache");
    let file_log = match file_log::FileLog::start(diagnostic_log.clone(), &data_root.join("logs")) {
        Ok(writer) => Some(writer),
        Err(error) => {
            diagnostic_log.record(alcomd_application::DiagnosticLogLevel::Error,
                Some("diagnostic.log_file_failed"), None, None,
                &format!("Could not open the technical log directory: {error}. Logs remain available in memory."),
                "alcomd::logging");
            None
        }
    };
    let store = StateStoreHandle::open(database).map_err(|error| {
        BindError::Io(io::Error::other(format!(
            "state store failed closed: {error}"
        )))
    })?;
    let m2 = M2Application::new(store.clone());
    m2.recover()
        .await
        .map_err(|_| BindError::Io(io::Error::other("Operation recovery failed")))?;
    let reader = alcomd_vpm::VpmReader::new()
        .map_err(|_| BindError::Io(io::Error::other("M3 reader initialization failed")))?;
    let engine = alcomd_vpm::PackageEngine::new(store.clone(), reader.clone(), cache_root.clone())
        .map_err(|_| BindError::Io(io::Error::other("M4 package engine initialization failed")))?;
    let locks = Arc::new(ResourceLockCoordinator::default());
    let m4 = M4PackageApplication::with_locks(store.clone(), engine, Arc::clone(&locks));
    m4.recover()
        .await
        .map_err(|_| BindError::Io(io::Error::other("package transaction recovery failed")))?;
    let official_gui = M7OfficialApplication::new(
        store.clone(),
        Arc::new(alcomd_store::FileConfigStore::new(
            data_root.join("config").join("settings.toml"),
        )),
        Arc::new(hub_platform::PlatformUnityHubAdapter),
        Arc::new(hub_platform::PlatformUnityHubLauncher::default()),
        directory_platform,
    );
    official_gui
        .initialize_settings()
        .await
        .map_err(|_| BindError::Io(io::Error::other("Config initialization failed")))?;
    let unity = M5Application::new(
        store.clone(),
        m5_platform::PlatformUnityAdapter,
        Arc::new(official_gui.clone()),
    );
    let unity_migration_engine = alcomd_vpm::UnityMigrationEngine::new(
        store.clone(),
        reader.clone(),
        cache_root.clone(),
        data_root.join("unity-migration-recovery"),
    )
    .map_err(|_| BindError::Io(io::Error::other("Unity migration initialization failed")))?;
    let unity_migration = UnityMigrationService::with_locks(
        store.clone(),
        unity_migration_engine,
        unity.clone(),
        Arc::clone(&locks),
    );
    unity_migration
        .recover()
        .await
        .map_err(|_| BindError::Io(io::Error::other("Unity migration recovery failed")))?;
    let template_engine = alcomd_vpm::TemplateEngine::with_package_cache(
        data_root.join("template-store"),
        cache_root.clone(),
    )
    .map_err(|_| BindError::Io(io::Error::other("Template engine initialization failed")))?;
    let templates = TemplateService::with_locks(
        store.clone(),
        template_engine.clone(),
        unity.clone(),
        Arc::clone(&locks),
    );
    let builtins = template_engine
        .materialize_builtins(&data_root.join("template-builtin-staging"))
        .map_err(|_| BindError::Io(io::Error::other("builtin Template initialization failed")))?;
    templates
        .ensure_builtins(builtins)
        .await
        .map_err(|_| BindError::Io(io::Error::other("builtin Template registration failed")))?;
    templates
        .recover()
        .await
        .map_err(|_| BindError::Io(io::Error::other("Template recovery failed")))?;
    let project_copy = ProjectCopyService::with_locks(
        store.clone(),
        alcomd_vpm::ProjectCopyEngine,
        unity.clone(),
        Arc::clone(&locks),
    );
    project_copy
        .recover()
        .await
        .map_err(|_| BindError::Io(io::Error::other("Project Copy recovery failed")))?;
    let project_delete = ProjectDeleteService::with_locks(
        store.clone(),
        alcomd_vpm::ProjectDeleteEngine,
        unity.clone(),
        Arc::clone(&locks),
    );
    project_delete
        .recover()
        .await
        .map_err(|_| BindError::Io(io::Error::other("Project Delete recovery failed")))?;
    let user_packages = UserPackageService::new(
        store.clone(),
        alcomd_vpm::UserPackageEngine::new(
            cache_root.clone(),
            data_root.join("user-package-staging"),
        )
        .map_err(|_| BindError::Io(io::Error::other("User Package initialization failed")))?,
    );
    let extension_engine =
        alcomd_extensions::ExtensionEngine::new(store.clone(), data_root.join("extensions"))
            .map_err(|_| {
                BindError::Io(io::Error::other("Extension engine initialization failed"))
            })?;
    let extension_runtime = m6_runtime::PlatformExtensionRuntime::new(store.clone())
        .map_err(|_| BindError::Io(io::Error::other("Extension Host initialization failed")))?;
    let m6 = M6Application::new(
        store.clone(),
        extension_engine,
        extension_runtime,
        Arc::clone(&locks),
    );
    m6.recover(current_time_ms()?)
        .await
        .map_err(|_| BindError::Io(io::Error::other("Extension recovery failed")))?;
    let m7 = M7Application::new(m6.clone(), m7_rpc::ProtocolUiValidator);
    if let Ok(defaults) = official_gui
        .observe_storage_directories(&alcomd_application::ConfigSnapshot::default())
        .await
    {
        let project_directory = defaults.project_directory;
        let prepared = tokio::task::spawn_blocking(move || {
            alcomd_platform::prepare_storage_directory(std::path::Path::new(&project_directory))
        })
        .await;
        match prepared {
            Ok(Ok(_)) => {}
            Ok(Err(error)) => {
                tracing::warn!(%error, "Default project directory could not be prepared")
            }
            Err(error) => {
                tracing::warn!(%error, "Default project directory preparation did not finish")
            }
        }
    } else {
        tracing::warn!(
            "System default project directory is unavailable; configured locations remain authoritative"
        );
    }
    let backups = BackupService::with_locks(
        store.clone(),
        alcomd_vpm::BackupEngine::new(data_root.join("backups"))
            .map_err(|_| BindError::Io(io::Error::other("Backup engine initialization failed")))?,
        unity.clone(),
        Arc::clone(&locks),
    )
    .with_directory_provider(Arc::new(storage_platform::ConfiguredBackupDirectory(
        official_gui.clone(),
    )));
    backups
        .recover()
        .await
        .map_err(|_| BindError::Io(io::Error::other("Backup recovery failed")))?;
    let candidates = ProjectCandidateApplication::new(
        store.clone(),
        alcomd_vpm::ProjectCandidateEngine,
        official_gui.clone(),
    );
    let applications = Arc::new(Applications {
        diagnostic_log,
        _file_log: file_log,
        m2,
        m3: M3ReadApplication::new(store.clone(), reader.clone()),
        project_discovery: ProjectDiscoveryApplication::with_locks(
            store.clone(),
            reader,
            project_discovery_platform::PlatformProjectDiscovery,
            official_gui.clone(),
            Arc::clone(&locks),
        ),
        repository_lists: RepositoryListApplication::new(
            store.clone(),
            alcomd_vpm::RepositoryListFileAdapter::new(data_root.clone()),
        ),
        m4,
        m5: unity,
        templates,
        backups,
        m6,
        m7,
        official_gui,
        candidates,
        project_copy,
        project_delete,
        user_packages,
        unity_migration,
    });
    let listener = instance.bind()?;
    Ok((listener, applications))
}

fn current_time_ms() -> Result<u64, BindError> {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| BindError::Io(io::Error::other("system clock is before Unix epoch")))?;
    u64::try_from(duration.as_millis())
        .map_err(|_| BindError::Io(io::Error::other("system clock exceeds supported range")))
}

async fn run_listener<F>(
    listener: &mut IpcListener,
    applications: Arc<Applications>,
    shutdown: F,
) -> io::Result<()>
where
    F: Future<Output = ()>,
{
    let mut connections = tokio::task::JoinSet::new();
    let mut log_tasks = tokio::task::JoinSet::new();
    let log_subscription = applications
        .m2
        .subscribe_events(&AccessContext::local_owner(), None, 100)
        .map_err(|_| io::Error::other("Task log subscription could not be established"))?;
    log_tasks.spawn(operation_log::follow(
        applications.m2.clone(),
        applications.diagnostic_log.clone(),
        log_subscription,
    ));
    let mut runtime_tick = tokio::time::interval(std::time::Duration::from_millis(250));
    runtime_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut discovery_tick = tokio::time::interval(std::time::Duration::from_secs(30));
    discovery_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let discovery_notify = applications.official_gui.project_discovery_notify();
    let project_recovery_notify = applications.project_discovery.project_recovery_notify();
    let mut discovery_tasks: tokio::task::JoinSet<
        Result<
            alcomd_application::ProjectDiscoveryReport,
            alcomd_application::ProjectDiscoveryError,
        >,
    > = tokio::task::JoinSet::new();
    let mut discovery_requested = false;
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            result = listener.accept() => {
                let stream = result?;
                let applications = Arc::clone(&applications);
                connections.spawn(async move {
                    let _ = serve_connection(stream, applications).await;
                });
            }
            result = connections.join_next(), if !connections.is_empty() => {
                let _ = result;
            }
            result = log_tasks.join_next(), if !log_tasks.is_empty() => {
                if !matches!(result, Some(Ok(Ok(())))) {
                    applications.diagnostic_log.record(alcomd_application::DiagnosticLogLevel::Error,
                        Some("diagnostic.operation_stream_failed"), None, None,
                        "The committed task event stream stopped. New task results cannot be added to the technical log.", "alcomd::operations");
                }
            }
            _ = discovery_tick.tick() => discovery_requested = true,
            () = discovery_notify.notified() => discovery_requested = true,
            () = project_recovery_notify.notified() => discovery_requested = true,
            result = discovery_tasks.join_next(), if !discovery_tasks.is_empty() => {
                match result {
                    Some(Ok(Ok(report))) => {
                        if report.registered > 0 || report.reached_limit {
                            tracing::info!(registered = report.registered, already_registered = report.already_registered,
                                rejected = report.rejected, reached_limit = report.reached_limit,
                                "Default project discovery completed");
                        }
                    }
                    Some(Ok(Err(error))) => tracing::warn!(?error, "Default project discovery did not complete"),
                    Some(Err(error)) => tracing::warn!(%error, "Default project discovery task did not finish"),
                    None => {}
                }
            }
            _ = runtime_tick.tick() => {
                applications
                    .m6
                    .maintain_runtime(current_time_ms().map_err(bind_as_io)?)
                    .await
                    .map_err(|_| io::Error::other("Extension runtime maintenance failed"))?;
                applications.m7.maintain(current_time_ms().map_err(bind_as_io)?).await;
            }
            () = &mut shutdown => break,
        }
        if discovery_requested && discovery_tasks.is_empty() {
            discovery_requested = false;
            let discovery = applications.project_discovery.clone();
            discovery_tasks.spawn(async move { discovery.scan().await });
        }
    }

    discovery_tasks.abort_all();
    log_tasks.abort_all();
    while log_tasks.join_next().await.is_some() {}
    while discovery_tasks.join_next().await.is_some() {}
    connections.abort_all();
    while connections.join_next().await.is_some() {}
    applications
        .m6
        .shutdown_runtime()
        .await
        .map_err(|_| io::Error::other("Extension runtime shutdown failed"))?;
    Ok(())
}

fn bind_as_io(error: BindError) -> io::Error {
    match error {
        BindError::Io(error) => error,
        other => io::Error::other(other.to_string()),
    }
}

struct ConnectionState {
    connection_id: String,
    client_instance_id: Option<String>,
    handshake_complete: bool,
    capabilities: HashSet<String>,
}

async fn serve_connection(
    mut stream: IpcStream,
    applications: Arc<Applications>,
) -> io::Result<()> {
    let mut state = ConnectionState {
        connection_id: OperationId::new().to_string(),
        client_instance_id: None,
        handshake_complete: false,
        capabilities: HashSet::new(),
    };
    let result = async {
        loop {
            let payload = match read_frame(&mut stream).await? {
                Some(payload) => payload,
                None => return Ok(()),
            };
            // Keep the large RPC dispatch future on the heap when it is moved into
            // either the normal request path or the disconnect-aware log read.
            let dispatch = Box::pin(
                dispatch_payload(&payload, &state, &applications)
                    .instrument(tracing::info_span!("rpc_request", gui_toast = false)),
            );
            let action = if diagnostic_log_rpc::is_log_read(&payload) {
                match diagnostic_log_rpc::until_disconnect(&mut stream, dispatch).await? {
                    Some(action) => action,
                    None => return Ok(()),
                }
            } else {
                dispatch.await
            };
            if action.event_subscription.is_some() {
                tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    write_json_frame(&mut stream, &action.response),
                )
                .await
                .map_err(|_| {
                    io::Error::new(io::ErrorKind::TimedOut, "subscription ready timed out")
                })??;
            } else {
                write_json_frame(&mut stream, &action.response).await?;
            }
            if let Some(capabilities) = action.complete_handshake {
                state.handshake_complete = true;
                state.capabilities = capabilities;
                state.client_instance_id = action.client_instance_id;
            }
            if let Some(subscription) = action.event_subscription {
                return events_rpc::serve(&mut stream, subscription).await;
            }
            if action.close_after_response {
                return Ok(());
            }
        }
    }
    .await;
    applications
        .m7
        .close_connection(&state.connection_id, current_time_ms().unwrap_or(0))
        .await;
    result
}

struct DispatchAction {
    response: Value,
    complete_handshake: Option<HashSet<String>>,
    client_instance_id: Option<String>,
    close_after_response: bool,
    event_subscription: Option<alcomd_application::EventSubscription<StateStoreHandle>>,
}

async fn dispatch_payload(
    payload: &[u8],
    state: &ConnectionState,
    applications: &Applications,
) -> DispatchAction {
    let parsed_value: Value = match serde_json::from_slice(payload) {
        Ok(value) => value,
        Err(_) => return error_action(None, RpcError::invalid_request(), false),
    };
    let recovered_id = parsed_value
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty() && id.len() <= alcomd_protocol::MAX_REQUEST_ID_BYTES)
        .map(str::to_owned);
    let request: RequestEnvelope = match serde_json::from_value(parsed_value) {
        Ok(request) => request,
        Err(_) => return error_action(recovered_id, RpcError::invalid_request(), false),
    };
    if request.validate().is_err() {
        return error_action(Some(request.id), RpcError::invalid_request(), false);
    }

    if request.method == METHOD_SYSTEM_HELLO {
        return dispatch_hello(request, state);
    }
    if !state.handshake_complete {
        return error_action(Some(request.id), RpcError::handshake_required(), false);
    }
    let method = request.method.clone();
    let mut action = if m7_candidates_rpc::oversized_candidate_frame(&request.method, payload) {
        m7_candidates_rpc::failure(
            request.id,
            alcomd_application::CandidateQueryError::LimitExceeded,
        )
    } else if request.method == METHOD_SYSTEM_STATUS {
        dispatch_status(request, state)
    } else {
        let access = AccessContext::local_owner();
        dispatch_m2(request, state, applications, &access).await
    };
    rpc_log::record(&applications.diagnostic_log, &method, &mut action.response);
    action
}

fn dispatch_hello(request: RequestEnvelope, state: &ConnectionState) -> DispatchAction {
    if state.handshake_complete {
        return error_action(
            Some(request.id),
            RpcError::handshake_already_completed(),
            false,
        );
    }
    let hello: HelloParams = match serde_json::from_value(request.params) {
        Ok(hello) => hello,
        Err(_) => return error_action(Some(request.id), RpcError::invalid_request(), false),
    };
    if hello.validate().is_err() {
        return error_action(Some(request.id), RpcError::invalid_request(), false);
    }
    if hello.rpc_version != RPC_VERSION {
        return error_action(
            Some(request.id),
            RpcError::rpc_version_unsupported(hello.rpc_version),
            true,
        );
    }
    let supported = [
        alcomd_protocol::CAPABILITY_DIAGNOSTICS_LOG_V1,
        CAPABILITY_STATE_CHECK_V1,
        CAPABILITY_OPERATIONS_V1,
        CAPABILITY_EVENTS_REPLAY_V1,
        alcomd_protocol::CAPABILITY_EVENTS_LIVE_V1,
        alcomd_protocol::CAPABILITY_PROJECTS_READ_V1,
        alcomd_protocol::CAPABILITY_PROJECTS_REGISTRY_V1,
        alcomd_protocol::CAPABILITY_PROJECTS_COPY_V1,
        alcomd_protocol::CAPABILITY_PROJECTS_DELETE_V1,
        alcomd_protocol::CAPABILITY_PROJECTS_UNITY_MIGRATION_V1,
        alcomd_protocol::CAPABILITY_REPOSITORIES_READ_V1,
        alcomd_protocol::CAPABILITY_REPOSITORIES_REGISTRY_V1,
        alcomd_protocol::CAPABILITY_REPOSITORIES_LIST_FILES_V1,
        alcomd_protocol::CAPABILITY_PACKAGES_PLAN_V1,
        alcomd_protocol::CAPABILITY_PACKAGES_PLAN_V2,
        alcomd_protocol::CAPABILITY_PACKAGES_CANDIDATES_V1,
        alcomd_protocol::CAPABILITY_PACKAGES_APPLY_V1,
        alcomd_protocol::CAPABILITY_PACKAGES_USER_PACKAGES_V1,
        alcomd_protocol::CAPABILITY_UNITY_READ_V1,
        alcomd_protocol::CAPABILITY_UNITY_MANAGE_V1,
        alcomd_protocol::CAPABILITY_UNITY_LAUNCH_V1,
        alcomd_protocol::CAPABILITY_UNITY_HUB_OPEN_V1,
        alcomd_protocol::CAPABILITY_TEMPLATES_READ_V1,
        alcomd_protocol::CAPABILITY_TEMPLATES_MANAGE_V1,
        alcomd_protocol::CAPABILITY_TEMPLATES_CREATE_PROJECT_V1,
        alcomd_protocol::CAPABILITY_BACKUPS_READ_V1,
        alcomd_protocol::CAPABILITY_BACKUPS_CREATE_V1,
        alcomd_protocol::CAPABILITY_BACKUPS_RESTORE_V1,
        alcomd_protocol::CAPABILITY_EXTENSIONS_LIFECYCLE_V1,
        alcomd_protocol::CAPABILITY_EXTENSIONS_PERMISSIONS_V1,
        alcomd_protocol::CAPABILITY_EXTENSIONS_PERMISSIONS_READ_V1,
        alcomd_protocol::CAPABILITY_EXTENSIONS_UI_PORTABLE_V1,
    ];
    let client_instance_id = hello.client.instance_id;
    let capabilities = hello
        .capabilities
        .into_iter()
        .filter(|capability| supported.contains(&capability.as_str()))
        .collect::<HashSet<_>>();
    let mut result_capabilities = capabilities.iter().cloned().collect::<Vec<_>>();
    result_capabilities.sort();
    let mut action = success_action(
        request.id,
        HelloResult::m7_official_gui(result_capabilities, alcomd_store::CURRENT_DATA_SCHEMA),
        Some(capabilities),
    );
    action.client_instance_id = Some(client_instance_id);
    action
}

fn dispatch_status(request: RequestEnvelope, state: &ConnectionState) -> DispatchAction {
    if request
        .params
        .as_object()
        .is_none_or(|params| !params.is_empty())
    {
        return error_action(Some(request.id), RpcError::invalid_request(), false);
    }
    let application_status = alcomd_application::system_status();
    let mut capabilities = state.capabilities.iter().cloned().collect::<Vec<_>>();
    capabilities.sort();
    success_action(
        request.id,
        SystemStatusResult {
            product: alcomd_protocol::PRODUCT_FAMILY.to_owned(),
            daemon_version: env!("CARGO_PKG_VERSION").to_owned(),
            rpc_version: RPC_VERSION,
            state: application_status.state().as_str().to_owned(),
            capabilities,
        },
        None,
    )
}

async fn dispatch_m2(
    request: RequestEnvelope,
    state: &ConnectionState,
    applications: &Applications,
    access: &AccessContext,
) -> DispatchAction {
    let application = &applications.m2;
    match request.method.as_str() {
        alcomd_protocol::METHOD_EVENTS_SUBSCRIBE => {
            events_rpc::dispatch(request, state, application, access)
        }
        METHOD_STATE_CHECK => {
            if let Some(action) = require_capability(&request.id, state, CAPABILITY_STATE_CHECK_V1)
            {
                return action;
            }
            let params: StateCheckParams = match serde_json::from_value(request.params) {
                Ok(params) => params,
                Err(_) => return invalid(request.id),
            };
            let key = match IdempotencyKey::parse(params.idempotency_key) {
                Ok(key) => key,
                Err(_) => return invalid(request.id),
            };
            match application.state_check(access, key).await {
                Ok(outcome) => success_action(
                    request.id,
                    OperationAccepted {
                        operation_id: outcome.operation_id.to_string(),
                        replayed: outcome.replayed,
                    },
                    None,
                ),
                Err(error) => application_error(request.id, error),
            }
        }
        METHOD_OPERATIONS_GET => {
            if let Some(action) = require_capability(&request.id, state, CAPABILITY_OPERATIONS_V1) {
                return action;
            }
            let params: OperationsGetParams = match serde_json::from_value(request.params) {
                Ok(params) => params,
                Err(_) => return invalid(request.id),
            };
            let operation_id = match OperationId::parse(&params.operation_id) {
                Ok(value) => value,
                Err(_) => return invalid(request.id),
            };
            match application.get_operation(access, operation_id).await {
                Ok(operation) => match operation_to_rpc(operation) {
                    Ok(operation) => success_action(request.id, operation, None),
                    Err(error) => error_action(Some(request.id), error, false),
                },
                Err(error) => application_error(request.id, error),
            }
        }
        METHOD_OPERATIONS_LIST => {
            if let Some(action) = require_capability(&request.id, state, CAPABILITY_OPERATIONS_V1) {
                return action;
            }
            let params: OperationsListParams = match serde_json::from_value(request.params) {
                Ok(params) => params,
                Err(_) => return invalid(request.id),
            };
            let cursor = match params.cursor.map(cursor_from_rpc).transpose() {
                Ok(cursor) => cursor,
                Err(()) => return invalid(request.id),
            };
            match application
                .list_operations(
                    access,
                    cursor,
                    params.limit.unwrap_or(100),
                    params.active_only,
                )
                .await
            {
                Ok(page) => {
                    let operations = page
                        .operations
                        .into_iter()
                        .map(operation_to_rpc)
                        .collect::<Result<Vec<_>, _>>();
                    match operations {
                        Ok(operations) => success_action(
                            request.id,
                            OperationsListResult {
                                operations,
                                next_cursor: page.next_cursor.map(cursor_to_rpc),
                                active_count: page.active_count,
                            },
                            None,
                        ),
                        Err(error) => error_action(Some(request.id), error, false),
                    }
                }
                Err(error) => application_error(request.id, error),
            }
        }
        METHOD_OPERATIONS_CANCEL => {
            if let Some(action) = require_capability(&request.id, state, CAPABILITY_OPERATIONS_V1) {
                return action;
            }
            let params: OperationsCancelParams = match serde_json::from_value(request.params) {
                Ok(params) => params,
                Err(_) => return invalid(request.id),
            };
            let operation_id = match OperationId::parse(&params.operation_id) {
                Ok(value) => value,
                Err(_) => return invalid(request.id),
            };
            let revision = match Revision::new(params.expected_revision) {
                Some(value) => value,
                None => return invalid(request.id),
            };
            let key = match IdempotencyKey::parse(params.idempotency_key) {
                Ok(value) => value,
                Err(_) => return invalid(request.id),
            };
            match application
                .cancel_operation(access, operation_id, revision, key)
                .await
            {
                Ok((operation, replayed)) => match operation_to_rpc(operation) {
                    Ok(operation) => success_action(
                        request.id,
                        OperationWriteResult {
                            operation,
                            replayed,
                        },
                        None,
                    ),
                    Err(error) => error_action(Some(request.id), error, false),
                },
                Err(error) => application_error(request.id, error),
            }
        }
        METHOD_EVENTS_LIST => {
            if let Some(action) =
                require_capability(&request.id, state, CAPABILITY_EVENTS_REPLAY_V1)
            {
                return action;
            }
            let params: EventsListParams = match serde_json::from_value(request.params) {
                Ok(params) => params,
                Err(_) => return invalid(request.id),
            };
            match application
                .list_events(access, params.after_sequence, params.limit.unwrap_or(100))
                .await
            {
                Ok(page) => {
                    let events = page
                        .events
                        .into_iter()
                        .map(event_to_rpc)
                        .collect::<Result<Vec<_>, _>>();
                    match events {
                        Ok(events) => success_action(
                            request.id,
                            EventsListResult {
                                events,
                                next_sequence: page.next_sequence,
                            },
                            None,
                        ),
                        Err(error) => error_action(Some(request.id), error, false),
                    }
                }
                Err(error) => application_error(request.id, error),
            }
        }
        alcomd_protocol::METHOD_DIAGNOSTICS_LOG_READ => {
            diagnostic_log_rpc::dispatch(request, state, &applications.diagnostic_log, access).await
        }
        _ if matches!(
            request.method.as_str(),
            alcomd_protocol::METHOD_SETTINGS_GET
                | alcomd_protocol::METHOD_SETTINGS_UPDATE
                | alcomd_protocol::METHOD_UNITY_HUB_OPEN
                | alcomd_protocol::METHOD_ACTIVITY_LIST
                | alcomd_protocol::METHOD_DIAGNOSTICS_LIST
        ) =>
        {
            m7_official_rpc::dispatch(request, state, &applications.official_gui, access).await
        }
        alcomd_protocol::METHOD_PACKAGES_QUERY_PROJECT_CANDIDATES => {
            m7_candidates_rpc::dispatch(request, state, &applications.candidates, access).await
        }
        _ if request.method.starts_with("packages.userPackages.") => {
            m7_user_packages_rpc::dispatch(request, state, &applications.user_packages, access)
                .await
        }
        _ if request.method.starts_with("packages.") => {
            m4_rpc::dispatch(request, state, &applications.m4, access).await
        }
        _ if matches!(
            request.method.as_str(),
            alcomd_protocol::METHOD_PROJECTS_PLAN_COPY
                | alcomd_protocol::METHOD_PROJECTS_APPLY_COPY
        ) =>
        {
            m7_copy_rpc::dispatch(request, state, &applications.project_copy, access).await
        }
        _ if matches!(
            request.method.as_str(),
            alcomd_protocol::METHOD_PROJECTS_PLAN_DELETE_DIRECTORY
                | alcomd_protocol::METHOD_PROJECTS_APPLY_DELETE_DIRECTORY
        ) =>
        {
            m7_delete_rpc::dispatch(request, state, &applications.project_delete, access).await
        }
        _ if matches!(
            request.method.as_str(),
            alcomd_protocol::METHOD_PROJECTS_PLAN_UNITY_MIGRATION
                | alcomd_protocol::METHOD_PROJECTS_APPLY_UNITY_MIGRATION
        ) =>
        {
            m7_unity_migration_rpc::dispatch(request, state, &applications.unity_migration, access)
                .await
        }
        _ if request.method.starts_with("unity.") => {
            m5_rpc::dispatch(request, state, &applications.m5, access).await
        }
        _ if request.method.starts_with("templates.") => {
            m5_template_rpc::dispatch(request, state, &applications.templates, access).await
        }
        _ if request.method.starts_with("backups.") => {
            m5_backup_rpc::dispatch(request, state, &applications.backups, access).await
        }
        _ if request.method.starts_with("extensions.") => {
            if matches!(
                request.method.as_str(),
                alcomd_protocol::METHOD_EXTENSIONS_UI_OPEN
                    | alcomd_protocol::METHOD_EXTENSIONS_UI_REFRESH
                    | alcomd_protocol::METHOD_EXTENSIONS_UI_DISPATCH
                    | alcomd_protocol::METHOD_EXTENSIONS_UI_CLOSE
            ) {
                m7_rpc::dispatch(request, state, &applications.m7, access).await
            } else {
                m6_rpc::dispatch(request, state, &applications.m6, access).await
            }
        }
        _ if matches!(
            request.method.as_str(),
            alcomd_protocol::METHOD_REPOSITORIES_INSPECT_LIST_FILE
                | alcomd_protocol::METHOD_REPOSITORIES_PREVIEW_LIST_EXPORT
                | alcomd_protocol::METHOD_REPOSITORIES_EXPORT_LIST_FILE
        ) =>
        {
            repository_list_rpc::dispatch(request, state, &applications.repository_lists, access)
                .await
        }
        _ => m3_rpc::dispatch(request, state, &applications.m3, access).await,
    }
}

fn require_capability(
    id: &str,
    state: &ConnectionState,
    capability: &str,
) -> Option<DispatchAction> {
    (!state.capabilities.contains(capability)).then(|| {
        error_action(
            Some(id.to_owned()),
            RpcError::capability_required(capability),
            false,
        )
    })
}

fn operation_to_rpc(record: OperationRecord) -> Result<Operation, RpcError> {
    let result = record
        .result_json
        .map(|json| serde_json::from_str(&json))
        .transpose()
        .map_err(|_| RpcError::internal(OperationId::new().to_string()))?;
    Ok(Operation {
        operation_id: record.operation_id.to_string(),
        kind: record.kind,
        state: match record.state {
            DomainState::Queued => OperationState::Queued,
            DomainState::Planning => OperationState::Planning,
            DomainState::WaitingForInput => OperationState::WaitingForInput,
            DomainState::Running => OperationState::Running,
            DomainState::Cancelling => OperationState::Cancelling,
            DomainState::Succeeded => OperationState::Succeeded,
            DomainState::Failed => OperationState::Failed,
            DomainState::Cancelled => OperationState::Cancelled,
            DomainState::Interrupted => OperationState::Interrupted,
            DomainState::Recovering => OperationState::Recovering,
        },
        revision: record.revision.get(),
        created_at_ms: record.created_at_ms,
        updated_at_ms: record.updated_at_ms,
        started_at_ms: record.started_at_ms,
        completed_at_ms: record.completed_at_ms,
        result,
        error_code: record.error_code,
        diagnostic_id: record.diagnostic_id,
        progress: record.progress_phase.map(|phase| OperationProgress {
            phase: match phase {
                alcomd_application::FilesystemPhase::Accepted => PackageOperationPhase::Accepted,
                alcomd_application::FilesystemPhase::PreflightComplete => {
                    PackageOperationPhase::PreflightComplete
                }
                alcomd_application::FilesystemPhase::QuarantineIntent => {
                    PackageOperationPhase::QuarantineIntent
                }
                alcomd_application::FilesystemPhase::RootQuarantined => {
                    PackageOperationPhase::RootQuarantined
                }
                alcomd_application::FilesystemPhase::RegistryCommitIntent => {
                    PackageOperationPhase::RegistryCommitIntent
                }
                alcomd_application::FilesystemPhase::InventoryReady => {
                    PackageOperationPhase::InventoryReady
                }
                alcomd_application::FilesystemPhase::Archiving => PackageOperationPhase::Archiving,
                alcomd_application::FilesystemPhase::ArchiveReady => {
                    PackageOperationPhase::ArchiveReady
                }
                alcomd_application::FilesystemPhase::PublishIntent => {
                    PackageOperationPhase::PublishIntent
                }
                alcomd_application::FilesystemPhase::ArchivePublished => {
                    PackageOperationPhase::ArchivePublished
                }
                alcomd_application::FilesystemPhase::ArchiveVerified => {
                    PackageOperationPhase::ArchiveVerified
                }
                alcomd_application::FilesystemPhase::Extracting => {
                    PackageOperationPhase::Extracting
                }
                alcomd_application::FilesystemPhase::Staging => PackageOperationPhase::Staging,
                alcomd_application::FilesystemPhase::StagingComplete => {
                    PackageOperationPhase::StagingComplete
                }
                alcomd_application::FilesystemPhase::TargetPublished => {
                    PackageOperationPhase::TargetPublished
                }
                alcomd_application::FilesystemPhase::ProjectRegistryCommitIntent => {
                    PackageOperationPhase::ProjectRegistryCommitIntent
                }
                alcomd_application::FilesystemPhase::Extracted => PackageOperationPhase::Extracted,
                alcomd_application::FilesystemPhase::Prepared => PackageOperationPhase::Prepared,
                alcomd_application::FilesystemPhase::PackagesReplaced => {
                    PackageOperationPhase::PackagesReplaced
                }
                alcomd_application::FilesystemPhase::VpmManifestCommitted => {
                    PackageOperationPhase::VpmManifestCommitted
                }
                alcomd_application::FilesystemPhase::FilesystemCommitted => {
                    PackageOperationPhase::FilesystemCommitted
                }
                alcomd_application::FilesystemPhase::StateCommitted => {
                    PackageOperationPhase::StateCommitted
                }
                alcomd_application::FilesystemPhase::Deleting => PackageOperationPhase::Deleting,
                alcomd_application::FilesystemPhase::CleanupComplete => {
                    PackageOperationPhase::CleanupComplete
                }
                alcomd_application::FilesystemPhase::RollingBack => {
                    PackageOperationPhase::RollingBack
                }
                alcomd_application::FilesystemPhase::RolledBack => {
                    PackageOperationPhase::RolledBack
                }
                alcomd_application::FilesystemPhase::RecoveryRequired => {
                    PackageOperationPhase::RecoveryRequired
                }
            },
        }),
    })
}

fn event_to_rpc(record: ApplicationEvent) -> Result<Event, RpcError> {
    let payload = serde_json::from_str(&record.payload_json)
        .map_err(|_| RpcError::internal(OperationId::new().to_string()))?;
    Ok(Event {
        sequence: record.sequence,
        event_id: record.event_id,
        kind: record.kind,
        aggregate_kind: record.aggregate_kind,
        aggregate_id: record.aggregate_id,
        aggregate_revision: record.aggregate_revision.get(),
        occurred_at_ms: record.occurred_at_ms,
        payload,
    })
}

fn cursor_from_rpc(cursor: OperationsListCursor) -> Result<OperationCursor, ()> {
    Ok(OperationCursor {
        created_at_ms: cursor.created_at_ms,
        operation_id: OperationId::parse(&cursor.operation_id).map_err(|_| ())?,
    })
}

fn cursor_to_rpc(cursor: OperationCursor) -> OperationsListCursor {
    OperationsListCursor {
        created_at_ms: cursor.created_at_ms,
        operation_id: cursor.operation_id.to_string(),
    }
}

fn application_rpc_error(error: ApplicationError) -> RpcError {
    match error {
        ApplicationError::PermissionDenied => RpcError::permission_denied(),
        ApplicationError::InvalidInput => RpcError::invalid_request(),
        ApplicationError::Store(StoreErrorKind::OperationNotFound) => {
            RpcError::operation_not_found()
        }
        ApplicationError::Store(StoreErrorKind::RevisionConflict) => RpcError::revision_conflict(),
        ApplicationError::Store(StoreErrorKind::IdempotencyConflict) => {
            RpcError::idempotency_conflict()
        }
        ApplicationError::Store(StoreErrorKind::OperationNotCancellable) => {
            RpcError::operation_not_cancellable()
        }
        ApplicationError::Store(StoreErrorKind::Unavailable) => RpcError::store_unavailable(),
        ApplicationError::Store(StoreErrorKind::CorruptState) => {
            RpcError::internal(OperationId::new().to_string())
        }
    }
}

fn application_error(id: String, error: ApplicationError) -> DispatchAction {
    error_action(Some(id), application_rpc_error(error), false)
}

fn invalid(id: String) -> DispatchAction {
    error_action(Some(id), RpcError::invalid_request(), false)
}

fn success_action<T: serde::Serialize>(
    id: String,
    result: T,
    handshake: Option<HashSet<String>>,
) -> DispatchAction {
    let response = serde_json::to_value(SuccessResponse { id, result })
        .expect("approved response DTOs must serialize");
    DispatchAction {
        response,
        complete_handshake: handshake,
        client_instance_id: None,
        close_after_response: false,
        event_subscription: None,
    }
}

fn error_action(id: Option<String>, error: RpcError, close: bool) -> DispatchAction {
    let response = serde_json::to_value(ErrorResponse { id, error })
        .expect("approved error DTO must serialize");
    DispatchAction {
        response,
        complete_handshake: None,
        client_instance_id: None,
        close_after_response: close,
        event_subscription: None,
    }
}

async fn read_frame(stream: &mut IpcStream) -> io::Result<Option<Vec<u8>>> {
    let mut prefix = [0_u8; 4];
    let count = stream.read(&mut prefix[..1]).await?;
    if count == 0 {
        return Ok(None);
    }
    stream.read_exact(&mut prefix[1..]).await?;
    let length = decode_frame_length(prefix)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let mut payload = vec![0_u8; length];
    stream.read_exact(&mut payload).await?;
    Ok(Some(payload))
}

async fn write_json_frame(stream: &mut IpcStream, value: &Value) -> io::Result<()> {
    let payload = serde_json::to_vec(value)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let frame = encode_frame(&payload)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    stream.write_all(&frame).await?;
    stream.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_dispatch_advertises_the_current_store_schema_and_negotiated_capabilities() {
        let state = ConnectionState {
            connection_id: "schema-handshake-test".to_owned(),
            client_instance_id: None,
            handshake_complete: false,
            capabilities: HashSet::new(),
        };
        let requested = [CAPABILITY_STATE_CHECK_V1, "unsupported.test-capability"];
        let action = dispatch_hello(
            RequestEnvelope {
                id: "schema-handshake".to_owned(),
                method: METHOD_SYSTEM_HELLO.to_owned(),
                params: serde_json::json!({
                    "rpcVersion": RPC_VERSION,
                    "client": {
                        "name": "schema-handshake-test",
                        "version": "1.0.0",
                        "instanceId": "schema-handshake-instance"
                    },
                    "capabilities": requested
                }),
            },
            &state,
        );
        let response: SuccessResponse<HelloResult> =
            serde_json::from_value(action.response).expect("successful typed hello response");
        assert_eq!(response.id, "schema-handshake");
        assert_eq!(
            response.result.data_schema,
            Some(alcomd_store::CURRENT_DATA_SCHEMA)
        );
        assert_eq!(
            response.result.config_schema,
            Some(alcomd_protocol::CONFIG_SCHEMA_VERSION)
        );
        assert_eq!(response.result.capabilities, [CAPABILITY_STATE_CHECK_V1]);
        assert_eq!(
            action.complete_handshake,
            Some(HashSet::from([CAPABILITY_STATE_CHECK_V1.to_owned()]))
        );
        assert_eq!(
            action.client_instance_id.as_deref(),
            Some("schema-handshake-instance")
        );
        assert!(!action.close_after_response);
    }
}
