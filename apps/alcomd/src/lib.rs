//! M2 daemon lifecycle, local RPC transport, and application adapter.

use std::collections::HashSet;
use std::future::Future;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use alcomd_application::{
    AccessContext, Application, ApplicationError, EventRecord as ApplicationEvent, IdempotencyKey,
    M3Application, M4Application, OperationCursor, OperationId, OperationRecord,
    OperationState as DomainState, Revision, StoreErrorKind,
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

static TEST_DATA_SEQUENCE: AtomicU64 = AtomicU64::new(1);

type M2Application = Application<StateStoreHandle>;
type M3ReadApplication = M3Application<StateStoreHandle, alcomd_vpm::VpmReader>;
type M4PackageApplication =
    M4Application<StateStoreHandle, alcomd_vpm::PackageEngine<StateStoreHandle>>;

struct Applications {
    m2: M2Application,
    m3: M3ReadApplication,
    m4: M4PackageApplication,
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
    let cache_root = database
        .parent()
        .ok_or_else(|| BindError::Io(io::Error::other("state path has no parent")))?
        .join("package-cache");
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
    let engine = alcomd_vpm::PackageEngine::new(store.clone(), reader.clone(), cache_root)
        .map_err(|_| BindError::Io(io::Error::other("M4 package engine initialization failed")))?;
    let m4 = M4PackageApplication::new(store.clone(), engine);
    m4.recover()
        .await
        .map_err(|_| BindError::Io(io::Error::other("package transaction recovery failed")))?;
    let applications = Arc::new(Applications {
        m2,
        m3: M3ReadApplication::new(store, reader),
        m4,
    });
    let listener = instance.bind()?;
    run_listener(listener, applications, shutdown)
        .await
        .map_err(BindError::Io)
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
            () = &mut shutdown => break,
        }
    }

    connections.abort_all();
    while connections.join_next().await.is_some() {}
    Ok(())
}

#[derive(Default)]
struct ConnectionState {
    handshake_complete: bool,
    capabilities: HashSet<String>,
}

async fn serve_connection(
    mut stream: IpcStream,
    applications: Arc<Applications>,
) -> io::Result<()> {
    let mut state = ConnectionState::default();
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
        }
        if action.close_after_response {
            return Ok(());
        }
    }
}

struct DispatchAction {
    response: Value,
    complete_handshake: Option<HashSet<String>>,
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
    dispatch_m2(
        request,
        state,
        &applications.m2,
        &applications.m3,
        &applications.m4,
        &access,
    )
    .await
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
        alcomd_protocol::CAPABILITY_REPOSITORIES_READ_V1,
        alcomd_protocol::CAPABILITY_REPOSITORIES_REGISTRY_V1,
        alcomd_protocol::CAPABILITY_PACKAGES_PLAN_V1,
        alcomd_protocol::CAPABILITY_PACKAGES_APPLY_V1,
    ];
    let capabilities = hello
        .capabilities
        .into_iter()
        .filter(|capability| supported.contains(&capability.as_str()))
        .collect::<HashSet<_>>();
    let mut result_capabilities = capabilities.iter().cloned().collect::<Vec<_>>();
    result_capabilities.sort();
    success_action(
        request.id,
        HelloResult::m4(result_capabilities),
        Some(capabilities),
    )
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
    application: &M2Application,
    m3_application: &M3ReadApplication,
    m4_application: &M4PackageApplication,
    access: &AccessContext,
) -> DispatchAction {
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
        _ if request.method.starts_with("packages.") => {
            m4_rpc::dispatch(request, state, m4_application, access).await
        }
        _ => m3_rpc::dispatch(request, state, m3_application, access).await,
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
                alcomd_application::FilesystemPhase::ArchiveReady => {
                    PackageOperationPhase::ArchiveReady
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
        close_after_response: false,
    }
}

fn error_action(id: Option<String>, error: RpcError, close: bool) -> DispatchAction {
    let response = serde_json::to_value(ErrorResponse { id, error })
        .expect("approved error DTO must serialize");
    DispatchAction {
        response,
        complete_handshake: None,
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
