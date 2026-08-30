//! M2 daemon lifecycle, local RPC transport, and application adapter.

use std::collections::HashSet;
use std::future::Future;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use alcomd_application::{
    AccessContext, Application, ApplicationError, EventRecord as ApplicationEvent, IdempotencyKey,
    M3Application, M4Application, M5BackupApplication as BackupService,
    M5TemplateApplication as TemplateService, M5UnityApplication as M5Application, M6Application,
    M7Application, M7CopyApplication as ProjectCopyService,
    M7DeleteApplication as ProjectDeleteService, M7OfficialApplication, OperationCursor,
    OperationId, OperationRecord, OperationState as DomainState, ResourceLockCoordinator, Revision,
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

mod m3_rpc;
mod m4_rpc;
mod m5_backup_rpc;
mod m5_platform;
mod m5_rpc;
mod m5_template_rpc;
mod m6_rpc;
mod m6_runtime;
mod m7_copy_rpc;
mod m7_delete_rpc;
mod m7_official_rpc;
mod m7_rpc;
mod m7_user_packages_rpc;

static TEST_DATA_SEQUENCE: AtomicU64 = AtomicU64::new(1);

type M2Application = Application<StateStoreHandle>;
type M3ReadApplication = M3Application<StateStoreHandle, alcomd_vpm::VpmReader>;
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
type ProjectCopyApplication =
    ProjectCopyService<StateStoreHandle, alcomd_vpm::ProjectCopyEngine, M5UnityApplication>;
type ProjectDeleteApplication =
    ProjectDeleteService<StateStoreHandle, alcomd_vpm::ProjectDeleteEngine, M5UnityApplication>;
type UserPackageApplication = UserPackageService<StateStoreHandle, alcomd_vpm::UserPackageEngine>;

struct Applications {
    m2: M2Application,
    m3: M3ReadApplication,
    m4: M4PackageApplication,
    m5: M5UnityApplication,
    templates: TemplateApplication,
    backups: BackupApplication,
    m6: M6ExtensionApplication,
    m7: M7ExtensionApplication,
    official_gui: M7OfficialGuiApplication,
    project_copy: ProjectCopyApplication,
    project_delete: ProjectDeleteApplication,
    user_packages: UserPackageApplication,
}

/// Runs the daemon with an ephemeral isolated data directory.
///
/// This compatibility entry point exists for M1 integration tests. Production
/// callers must use [`serve_with_data_until`] with the formal platform config.
pub async fn serve_until<F>(ipc: IpcConfig, shutdown: F) -> Result<(), BindError>
where
    F: Future<Output = ()>,
{
    let sequence = TEST_DATA_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "alcomd-daemon-test-{}-{sequence}",
        std::process::id()
    ));
    let result =
        serve_with_data_until(ipc, DataConfig::isolated(directory.clone()), shutdown).await;
    for _ in 0..20 {
        if std::fs::remove_dir_all(&directory).is_ok() || !directory.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    result
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
    let instance = DaemonInstance::acquire(&ipc)?;
    let database = alcomd_platform::state_database_path(&data)?;
    let data_root = database
        .parent()
        .ok_or_else(|| BindError::Io(io::Error::other("state path has no parent")))?
        .to_path_buf();
    let cache_root = data_root.join("package-cache");
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
    let unity = M5Application::new(store.clone(), m5_platform::PlatformUnityAdapter);
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
    let backups = BackupService::with_locks(
        store.clone(),
        alcomd_vpm::BackupEngine::new(data_root.join("backups"))
            .map_err(|_| BindError::Io(io::Error::other("Backup engine initialization failed")))?,
        unity.clone(),
        Arc::clone(&locks),
    );
    backups
        .recover()
        .await
        .map_err(|_| BindError::Io(io::Error::other("Backup recovery failed")))?;
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
    let official_gui = M7OfficialApplication::new(
        store.clone(),
        data_root.join("config").join("settings.toml"),
    );
    official_gui
        .initialize_settings()
        .await
        .map_err(|_| BindError::Io(io::Error::other("Config initialization failed")))?;
    let applications = Arc::new(Applications {
        m2,
        m3: M3ReadApplication::new(store.clone(), reader),
        m4,
        m5: unity,
        templates,
        backups,
        m6,
        m7,
        official_gui,
        project_copy,
        project_delete,
        user_packages,
    });
    let listener = instance.bind()?;
    run_listener(listener, applications, shutdown)
        .await
        .map_err(BindError::Io)
}

fn current_time_ms() -> Result<u64, BindError> {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| BindError::Io(io::Error::other("system clock is before Unix epoch")))?;
    u64::try_from(duration.as_millis())
        .map_err(|_| BindError::Io(io::Error::other("system clock exceeds supported range")))
}

async fn run_listener<F>(
    mut listener: IpcListener,
    applications: Arc<Applications>,
    shutdown: F,
) -> io::Result<()>
where
    F: Future<Output = ()>,
{
    let mut connections = tokio::task::JoinSet::new();
    let mut runtime_tick = tokio::time::interval(std::time::Duration::from_millis(250));
    runtime_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
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
    }

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
            let action = dispatch_payload(&payload, &state, &applications).await;
            write_json_frame(&mut stream, &action.response).await?;
            if let Some(capabilities) = action.complete_handshake {
                state.handshake_complete = true;
                state.capabilities = capabilities;
                state.client_instance_id = action.client_instance_id;
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
    if request.method == METHOD_SYSTEM_STATUS {
        return dispatch_status(request, state);
    }

    let access = AccessContext::local_owner();
    dispatch_m2(request, state, applications, &access).await
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
        CAPABILITY_STATE_CHECK_V1,
        CAPABILITY_OPERATIONS_V1,
        CAPABILITY_EVENTS_REPLAY_V1,
        alcomd_protocol::CAPABILITY_PROJECTS_READ_V1,
        alcomd_protocol::CAPABILITY_PROJECTS_REGISTRY_V1,
        alcomd_protocol::CAPABILITY_PROJECTS_COPY_V1,
        alcomd_protocol::CAPABILITY_PROJECTS_DELETE_V1,
        alcomd_protocol::CAPABILITY_REPOSITORIES_READ_V1,
        alcomd_protocol::CAPABILITY_REPOSITORIES_REGISTRY_V1,
        alcomd_protocol::CAPABILITY_PACKAGES_PLAN_V1,
        alcomd_protocol::CAPABILITY_PACKAGES_PLAN_V2,
        alcomd_protocol::CAPABILITY_PACKAGES_APPLY_V1,
        alcomd_protocol::CAPABILITY_PACKAGES_USER_PACKAGES_V1,
        alcomd_protocol::CAPABILITY_UNITY_READ_V1,
        alcomd_protocol::CAPABILITY_UNITY_MANAGE_V1,
        alcomd_protocol::CAPABILITY_UNITY_LAUNCH_V1,
        alcomd_protocol::CAPABILITY_TEMPLATES_READ_V1,
        alcomd_protocol::CAPABILITY_TEMPLATES_MANAGE_V1,
        alcomd_protocol::CAPABILITY_TEMPLATES_CREATE_PROJECT_V1,
        alcomd_protocol::CAPABILITY_BACKUPS_READ_V1,
        alcomd_protocol::CAPABILITY_BACKUPS_CREATE_V1,
        alcomd_protocol::CAPABILITY_BACKUPS_RESTORE_V1,
        alcomd_protocol::CAPABILITY_EXTENSIONS_LIFECYCLE_V1,
        alcomd_protocol::CAPABILITY_EXTENSIONS_PERMISSIONS_V1,
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
        HelloResult::m7_official_gui(result_capabilities),
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
                .list_operations(access, cursor, params.limit.unwrap_or(100))
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
        _ if matches!(
            request.method.as_str(),
            alcomd_protocol::METHOD_SETTINGS_GET
                | alcomd_protocol::METHOD_SETTINGS_UPDATE
                | alcomd_protocol::METHOD_ACTIVITY_LIST
                | alcomd_protocol::METHOD_DIAGNOSTICS_LIST
        ) =>
        {
            m7_official_rpc::dispatch(request, &applications.official_gui, access).await
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

fn application_error(id: String, error: ApplicationError) -> DispatchAction {
    let error = match error {
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
    };
    error_action(Some(id), error, false)
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
