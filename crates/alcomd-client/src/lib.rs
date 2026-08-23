//! Official M1 Rust client for the per-user ALCOMD daemon.

use std::io;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use alcomd_platform::{IpcConfig, IpcStream};
use alcomd_protocol::{
    CAPABILITY_EVENTS_REPLAY_V1, CAPABILITY_OPERATIONS_V1, CAPABILITY_STATE_CHECK_V1, ClientInfo,
    EventsListParams, EventsListResult, HelloParams, HelloResult, METHOD_EVENTS_LIST,
    METHOD_OPERATIONS_CANCEL, METHOD_OPERATIONS_GET, METHOD_OPERATIONS_LIST, METHOD_STATE_CHECK,
    METHOD_SYSTEM_HELLO, METHOD_SYSTEM_STATUS, Operation, OperationAccepted, OperationWriteResult,
    OperationsCancelParams, OperationsGetParams, OperationsListCursor, OperationsListParams,
    OperationsListResult, RPC_VERSION, RequestEnvelope, Response, RpcError, StateCheckParams,
    SystemStatusResult, decode_frame_length, encode_frame,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::json;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const START_TIMEOUT: Duration = Duration::from_secs(5);
const RETRY_INTERVAL: Duration = Duration::from_millis(50);
static INSTANCE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Client connection settings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientConfig {
    runtime_directory: Option<PathBuf>,
    data_directory: Option<PathBuf>,
    start_daemon: bool,
    daemon_path: Option<PathBuf>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            runtime_directory: None,
            data_directory: None,
            start_daemon: true,
            daemon_path: None,
        }
    }
}

impl ClientConfig {
    /// Disables automatic daemon startup.
    #[must_use]
    pub fn without_daemon_start(mut self) -> Self {
        self.start_daemon = false;
        self
    }

    /// Uses an isolated Unix runtime directory.
    #[must_use]
    pub fn with_runtime_directory(mut self, path: PathBuf) -> Self {
        self.runtime_directory = Some(path);
        self
    }

    /// Uses an isolated daemon data directory when auto-starting in tests.
    #[must_use]
    pub fn with_data_directory(mut self, path: PathBuf) -> Self {
        self.data_directory = Some(path);
        self
    }

    /// Overrides the daemon executable for an isolated integration test.
    #[must_use]
    pub fn with_daemon_path(mut self, path: PathBuf) -> Self {
        self.daemon_path = Some(path);
        self
    }

    fn ipc_config(&self) -> IpcConfig {
        self.runtime_directory
            .clone()
            .map(IpcConfig::isolated)
            .unwrap_or_default()
    }
}

/// A connection to the per-user `alcomd` process.
pub struct AlcomdClient {
    stream: IpcStream,
    next_request_id: u64,
}

impl AlcomdClient {
    /// Connects to the per-user daemon, optionally starting its sibling binary.
    pub async fn connect(config: ClientConfig) -> Result<Self, ClientError> {
        let stream = connect_with_policy(&config).await?;
        let mut client = Self {
            stream,
            next_request_id: 1,
        };
        let _ = client.hello().await?;
        Ok(client)
    }

    /// Performs the mandatory M1 handshake.
    async fn hello(&mut self) -> Result<HelloResult, ClientError> {
        let sequence = INSTANCE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let params = HelloParams {
            rpc_version: RPC_VERSION,
            client: ClientInfo {
                name: "alcomd-client".to_owned(),
                version: env!("CARGO_PKG_VERSION").to_owned(),
                instance_id: format!("{}-{sequence}", std::process::id()),
            },
            capabilities: vec![
                CAPABILITY_STATE_CHECK_V1.to_owned(),
                CAPABILITY_OPERATIONS_V1.to_owned(),
                CAPABILITY_EVENTS_REPLAY_V1.to_owned(),
                alcomd_protocol::CAPABILITY_PROJECTS_READ_V1.to_owned(),
                alcomd_protocol::CAPABILITY_PROJECTS_REGISTRY_V1.to_owned(),
                alcomd_protocol::CAPABILITY_REPOSITORIES_READ_V1.to_owned(),
                alcomd_protocol::CAPABILITY_REPOSITORIES_REGISTRY_V1.to_owned(),
                alcomd_protocol::CAPABILITY_PACKAGES_PLAN_V1.to_owned(),
                alcomd_protocol::CAPABILITY_PACKAGES_APPLY_V1.to_owned(),
                alcomd_protocol::CAPABILITY_UNITY_READ_V1.to_owned(),
                alcomd_protocol::CAPABILITY_UNITY_MANAGE_V1.to_owned(),
                alcomd_protocol::CAPABILITY_UNITY_LAUNCH_V1.to_owned(),
                alcomd_protocol::CAPABILITY_TEMPLATES_READ_V1.to_owned(),
                alcomd_protocol::CAPABILITY_TEMPLATES_MANAGE_V1.to_owned(),
                alcomd_protocol::CAPABILITY_TEMPLATES_CREATE_PROJECT_V1.to_owned(),
                alcomd_protocol::CAPABILITY_BACKUPS_READ_V1.to_owned(),
                alcomd_protocol::CAPABILITY_BACKUPS_CREATE_V1.to_owned(),
            ],
        };
        self.call(METHOD_SYSTEM_HELLO, params).await
    }

    /// Queries the truthful minimal daemon status.
    pub async fn system_status(&mut self) -> Result<SystemStatusResult, ClientError> {
        self.call(METHOD_SYSTEM_STATUS, json!({})).await
    }

    /// Starts or idempotently replays the read-only state integrity check.
    pub async fn state_check(
        &mut self,
        idempotency_key: String,
    ) -> Result<OperationAccepted, ClientError> {
        self.call(METHOD_STATE_CHECK, StateCheckParams { idempotency_key })
            .await
    }

    /// Returns one visible Operation.
    pub async fn operation_get(&mut self, operation_id: String) -> Result<Operation, ClientError> {
        self.call(METHOD_OPERATIONS_GET, OperationsGetParams { operation_id })
            .await
    }

    /// Lists visible Operations using the frozen tuple cursor.
    pub async fn operations_list(
        &mut self,
        cursor: Option<OperationsListCursor>,
        limit: Option<u32>,
    ) -> Result<OperationsListResult, ClientError> {
        self.call(
            METHOD_OPERATIONS_LIST,
            OperationsListParams { cursor, limit },
        )
        .await
    }

    /// Requests idempotent cooperative cancellation.
    pub async fn operation_cancel(
        &mut self,
        operation_id: String,
        expected_revision: u64,
        idempotency_key: String,
    ) -> Result<OperationWriteResult, ClientError> {
        self.call(
            METHOD_OPERATIONS_CANCEL,
            OperationsCancelParams {
                operation_id,
                expected_revision,
                idempotency_key,
            },
        )
        .await
    }

    /// Replays visible durable Events after an exclusive sequence.
    pub async fn events_list(
        &mut self,
        after_sequence: u64,
        limit: Option<u32>,
    ) -> Result<EventsListResult, ClientError> {
        self.call(
            METHOD_EVENTS_LIST,
            EventsListParams {
                after_sequence,
                limit,
            },
        )
        .await
    }

    pub async fn package_plan_install(
        &mut self,
        params: alcomd_protocol::PackagePlanInstallParams,
    ) -> Result<alcomd_protocol::PackagePlan, ClientError> {
        self.call(alcomd_protocol::METHOD_PACKAGES_PLAN_INSTALL, params)
            .await
    }

    pub async fn package_plan_remove(
        &mut self,
        params: alcomd_protocol::PackagePlanRemoveParams,
    ) -> Result<alcomd_protocol::PackagePlan, ClientError> {
        self.call(alcomd_protocol::METHOD_PACKAGES_PLAN_REMOVE, params)
            .await
    }

    pub async fn package_plan_upgrade(
        &mut self,
        params: alcomd_protocol::PackagePlanUpgradeParams,
    ) -> Result<alcomd_protocol::PackagePlan, ClientError> {
        self.call(alcomd_protocol::METHOD_PACKAGES_PLAN_UPGRADE, params)
            .await
    }

    pub async fn package_plan_downgrade(
        &mut self,
        params: alcomd_protocol::PackagePlanDowngradeParams,
    ) -> Result<alcomd_protocol::PackagePlan, ClientError> {
        self.call(alcomd_protocol::METHOD_PACKAGES_PLAN_DOWNGRADE, params)
            .await
    }

    pub async fn package_plan_resolve(
        &mut self,
        params: alcomd_protocol::PackagePlanResolveParams,
    ) -> Result<alcomd_protocol::PackagePlan, ClientError> {
        self.call(alcomd_protocol::METHOD_PACKAGES_PLAN_RESOLVE, params)
            .await
    }

    pub async fn package_apply_plan(
        &mut self,
        params: alcomd_protocol::PackageApplyPlanParams,
    ) -> Result<alcomd_protocol::PackageApplyPlanResult, ClientError> {
        self.call(alcomd_protocol::METHOD_PACKAGES_APPLY_PLAN, params)
            .await
    }

    pub async fn unity_installations_list(
        &mut self,
        params: alcomd_protocol::UnityInstallationsListParams,
    ) -> Result<alcomd_protocol::UnityInstallationsListResult, ClientError> {
        self.call(alcomd_protocol::METHOD_UNITY_INSTALLATIONS_LIST, params)
            .await
    }

    pub async fn unity_installation_get(
        &mut self,
        installation_id: String,
    ) -> Result<alcomd_protocol::UnityInstallationResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_UNITY_INSTALLATIONS_GET,
            alcomd_protocol::UnityInstallationIdParams { installation_id },
        )
        .await
    }

    pub async fn unity_installation_register(
        &mut self,
        params: alcomd_protocol::UnityInstallationRegisterParams,
    ) -> Result<alcomd_protocol::UnityInstallationResult, ClientError> {
        self.call(alcomd_protocol::METHOD_UNITY_INSTALLATIONS_REGISTER, params)
            .await
    }

    pub async fn unity_installation_remove(
        &mut self,
        params: alcomd_protocol::UnityInstallationRemoveParams,
    ) -> Result<alcomd_protocol::UnityInstallationRemoveResult, ClientError> {
        self.call(alcomd_protocol::METHOD_UNITY_INSTALLATIONS_REMOVE, params)
            .await
    }

    pub async fn unity_installations_refresh(
        &mut self,
        idempotency_key: String,
    ) -> Result<alcomd_protocol::UnityInstallationsListResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_UNITY_INSTALLATIONS_REFRESH,
            alcomd_protocol::UnityInstallationRefreshParams { idempotency_key },
        )
        .await
    }

    pub async fn unity_project_editor_get(
        &mut self,
        project_id: String,
    ) -> Result<alcomd_protocol::ProjectEditorResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_UNITY_PROJECT_EDITOR_GET,
            alcomd_protocol::UnityProjectIdParams { project_id },
        )
        .await
    }

    pub async fn unity_project_editor_set(
        &mut self,
        params: alcomd_protocol::ProjectEditorSetParams,
    ) -> Result<alcomd_protocol::ProjectEditorResult, ClientError> {
        self.call(alcomd_protocol::METHOD_UNITY_PROJECT_EDITOR_SET, params)
            .await
    }

    pub async fn unity_writer_state(
        &mut self,
        project_id: String,
    ) -> Result<alcomd_protocol::UnityWriterState, ClientError> {
        self.call(
            alcomd_protocol::METHOD_UNITY_WRITER_STATE,
            alcomd_protocol::UnityProjectIdParams { project_id },
        )
        .await
    }

    pub async fn unity_launch(
        &mut self,
        params: alcomd_protocol::UnityLaunchParams,
    ) -> Result<alcomd_protocol::UnityLaunchResult, ClientError> {
        self.call(alcomd_protocol::METHOD_UNITY_LAUNCH, params)
            .await
    }

    pub async fn unity_launch_status(
        &mut self,
        launch_id: String,
    ) -> Result<alcomd_protocol::UnityLaunchResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_UNITY_LAUNCH_STATUS,
            alcomd_protocol::UnityLaunchStatusParams { launch_id },
        )
        .await
    }

    pub async fn templates_list(
        &mut self,
        params: alcomd_protocol::TemplatesListParams,
    ) -> Result<alcomd_protocol::TemplatesListResult, ClientError> {
        self.call(alcomd_protocol::METHOD_TEMPLATES_LIST, params)
            .await
    }

    pub async fn template_get(
        &mut self,
        template_id: String,
    ) -> Result<alcomd_protocol::TemplateRecordResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_TEMPLATES_GET,
            alcomd_protocol::TemplateIdParams { template_id },
        )
        .await
    }

    pub async fn template_inspect_bundle(
        &mut self,
        bundle_path: String,
    ) -> Result<alcomd_protocol::TemplateBundleInspection, ClientError> {
        self.call(
            alcomd_protocol::METHOD_TEMPLATES_INSPECT_BUNDLE,
            alcomd_protocol::TemplateInspectBundleParams { bundle_path },
        )
        .await
    }

    pub async fn template_plan_import(
        &mut self,
        params: alcomd_protocol::TemplatePlanImportParams,
    ) -> Result<alcomd_protocol::TemplatePlan, ClientError> {
        self.call(alcomd_protocol::METHOD_TEMPLATES_PLAN_IMPORT, params)
            .await
    }

    pub async fn template_apply_import(
        &mut self,
        params: alcomd_protocol::TemplateApplyPlanParams,
    ) -> Result<alcomd_protocol::TemplateApplyResult, ClientError> {
        self.call(alcomd_protocol::METHOD_TEMPLATES_APPLY_IMPORT, params)
            .await
    }

    pub async fn template_plan_derive(
        &mut self,
        params: alcomd_protocol::TemplatePlanDeriveParams,
    ) -> Result<alcomd_protocol::TemplatePlan, ClientError> {
        self.call(alcomd_protocol::METHOD_TEMPLATES_PLAN_DERIVE, params)
            .await
    }

    pub async fn template_apply_derive(
        &mut self,
        params: alcomd_protocol::TemplateApplyPlanParams,
    ) -> Result<alcomd_protocol::TemplateApplyResult, ClientError> {
        self.call(alcomd_protocol::METHOD_TEMPLATES_APPLY_DERIVE, params)
            .await
    }

    pub async fn template_export(
        &mut self,
        params: alcomd_protocol::TemplateExportParams,
    ) -> Result<alcomd_protocol::TemplateExportResult, ClientError> {
        self.call(alcomd_protocol::METHOD_TEMPLATES_EXPORT, params)
            .await
    }

    pub async fn template_set_favorite(
        &mut self,
        params: alcomd_protocol::TemplateSetFavoriteParams,
    ) -> Result<alcomd_protocol::TemplateRecordResult, ClientError> {
        self.call(alcomd_protocol::METHOD_TEMPLATES_SET_FAVORITE, params)
            .await
    }

    pub async fn template_remove(
        &mut self,
        params: alcomd_protocol::TemplateRemoveParams,
    ) -> Result<alcomd_protocol::TemplateRemoveResult, ClientError> {
        self.call(alcomd_protocol::METHOD_TEMPLATES_REMOVE, params)
            .await
    }

    pub async fn template_plan_create_project(
        &mut self,
        params: alcomd_protocol::TemplatePlanCreateProjectParams,
    ) -> Result<alcomd_protocol::TemplatePlan, ClientError> {
        self.call(
            alcomd_protocol::METHOD_TEMPLATES_PLAN_CREATE_PROJECT,
            params,
        )
        .await
    }

    pub async fn template_apply_create_project(
        &mut self,
        params: alcomd_protocol::TemplateApplyPlanParams,
    ) -> Result<alcomd_protocol::TemplateApplyResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_TEMPLATES_APPLY_CREATE_PROJECT,
            params,
        )
        .await
    }

    pub async fn backups_list(
        &mut self,
        params: alcomd_protocol::BackupsListParams,
    ) -> Result<alcomd_protocol::BackupsListResult, ClientError> {
        self.call(alcomd_protocol::METHOD_BACKUPS_LIST, params)
            .await
    }

    pub async fn backup_get(
        &mut self,
        backup_id: String,
    ) -> Result<alcomd_protocol::BackupRecord, ClientError> {
        self.call(
            alcomd_protocol::METHOD_BACKUPS_GET,
            alcomd_protocol::BackupGetParams { backup_id },
        )
        .await
    }

    pub async fn backup_create(
        &mut self,
        params: alcomd_protocol::BackupCreateParams,
    ) -> Result<alcomd_protocol::BackupCreateResult, ClientError> {
        self.call(alcomd_protocol::METHOD_BACKUPS_CREATE, params)
            .await
    }

    pub async fn project_inspect(
        &mut self,
        path: String,
        discovery_mode: alcomd_protocol::ProjectDiscoveryMode,
    ) -> Result<alcomd_protocol::ProjectResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_PROJECTS_INSPECT,
            alcomd_protocol::ProjectsInspectParams {
                path,
                discovery_mode,
            },
        )
        .await
    }

    pub async fn projects_list(
        &mut self,
        cursor: Option<alcomd_protocol::RegistryCursor>,
        limit: Option<u32>,
    ) -> Result<alcomd_protocol::ProjectsListResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_PROJECTS_LIST,
            alcomd_protocol::RegistryListParams { cursor, limit },
        )
        .await
    }

    pub async fn project_get(
        &mut self,
        project_id: String,
    ) -> Result<alcomd_protocol::ProjectResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_PROJECTS_GET,
            alcomd_protocol::ProjectIdParams { project_id },
        )
        .await
    }

    pub async fn project_register(
        &mut self,
        path: String,
        idempotency_key: String,
    ) -> Result<alcomd_protocol::ProjectWriteResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_PROJECTS_REGISTER,
            alcomd_protocol::ProjectRegisterParams {
                path,
                idempotency_key,
            },
        )
        .await
    }

    pub async fn project_refresh(
        &mut self,
        project_id: String,
        expected_revision: u64,
        idempotency_key: String,
    ) -> Result<alcomd_protocol::ProjectWriteResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_PROJECTS_REFRESH,
            alcomd_protocol::ProjectMutationParams {
                project_id,
                expected_revision,
                idempotency_key,
            },
        )
        .await
    }

    pub async fn project_unregister(
        &mut self,
        project_id: String,
        expected_revision: u64,
        idempotency_key: String,
    ) -> Result<alcomd_protocol::ProjectUnregisterResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_PROJECTS_UNREGISTER,
            alcomd_protocol::ProjectMutationParams {
                project_id,
                expected_revision,
                idempotency_key,
            },
        )
        .await
    }

    pub async fn repository_inspect(
        &mut self,
        source: alcomd_protocol::RepositorySource,
    ) -> Result<alcomd_protocol::RepositoryResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_REPOSITORIES_INSPECT,
            alcomd_protocol::RepositoryInspectParams { source },
        )
        .await
    }

    pub async fn repositories_list(
        &mut self,
        cursor: Option<alcomd_protocol::RegistryCursor>,
        limit: Option<u32>,
    ) -> Result<alcomd_protocol::RepositoriesListResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_REPOSITORIES_LIST,
            alcomd_protocol::RegistryListParams { cursor, limit },
        )
        .await
    }

    pub async fn repository_get(
        &mut self,
        repository_id: String,
    ) -> Result<alcomd_protocol::RepositoryResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_REPOSITORIES_GET,
            alcomd_protocol::RepositoryIdParams { repository_id },
        )
        .await
    }

    pub async fn repository_packages(
        &mut self,
        repository_id: String,
        cursor: Option<alcomd_protocol::PackageCursor>,
        limit: Option<u32>,
    ) -> Result<alcomd_protocol::RepositoryPackagesResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_REPOSITORIES_PACKAGES,
            alcomd_protocol::RepositoryPackagesParams {
                repository_id,
                cursor,
                limit,
            },
        )
        .await
    }

    pub async fn repository_register(
        &mut self,
        source: alcomd_protocol::RepositorySource,
        idempotency_key: String,
    ) -> Result<alcomd_protocol::RepositoryWriteResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_REPOSITORIES_REGISTER,
            alcomd_protocol::RepositoryRegisterParams {
                source,
                idempotency_key,
            },
        )
        .await
    }

    pub async fn repository_refresh(
        &mut self,
        repository_id: String,
        expected_revision: u64,
        idempotency_key: String,
    ) -> Result<alcomd_protocol::RepositoryWriteResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_REPOSITORIES_REFRESH,
            alcomd_protocol::RepositoryMutationParams {
                repository_id,
                expected_revision,
                idempotency_key,
            },
        )
        .await
    }

    pub async fn repository_unregister(
        &mut self,
        repository_id: String,
        expected_revision: u64,
        idempotency_key: String,
    ) -> Result<alcomd_protocol::RepositoryUnregisterResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_REPOSITORIES_UNREGISTER,
            alcomd_protocol::RepositoryMutationParams {
                repository_id,
                expected_revision,
                idempotency_key,
            },
        )
        .await
    }

    async fn call<P, T>(&mut self, method: &str, params: P) -> Result<T, ClientError>
    where
        P: Serialize,
        T: DeserializeOwned,
    {
        let id = self.next_request_id.to_string();
        self.next_request_id = self.next_request_id.saturating_add(1);
        let request = RequestEnvelope {
            id,
            method: method.to_owned(),
            params: serde_json::to_value(params).map_err(|_| ClientError::InvalidResponse)?,
        };
        let payload = serde_json::to_vec(&request).map_err(|_| ClientError::InvalidResponse)?;
        let frame = encode_frame(&payload).map_err(|_| ClientError::InvalidResponse)?;
        self.stream
            .write_all(&frame)
            .await
            .map_err(ClientError::Transport)?;
        self.stream.flush().await.map_err(ClientError::Transport)?;

        let payload = read_frame(&mut self.stream).await?;
        let response: Response<T> =
            serde_json::from_slice(&payload).map_err(|_| ClientError::InvalidResponse)?;
        match response {
            Response::Success(success) if success.id == request.id => Ok(success.result),
            Response::Success(_) => Err(ClientError::InvalidResponse),
            Response::Error(error) => Err(ClientError::Remote(error.error)),
        }
    }
}

async fn connect_with_policy(config: &ClientConfig) -> Result<IpcStream, ClientError> {
    let ipc = config.ipc_config();
    match alcomd_platform::connect(&ipc).await {
        Ok(stream) => return Ok(stream),
        Err(error) if is_endpoint_absent(&error) && config.start_daemon => spawn_daemon(config)?,
        Err(error) if is_transient(&error) => {}
        Err(error) => return Err(ClientError::Transport(error)),
    }

    let deadline = Instant::now() + START_TIMEOUT;
    loop {
        match alcomd_platform::connect(&ipc).await {
            Ok(stream) => return Ok(stream),
            Err(error) if is_transient(&error) && Instant::now() < deadline => {
                tokio::time::sleep(RETRY_INTERVAL).await;
            }
            Err(error) if is_transient(&error) => return Err(ClientError::StartTimeout),
            Err(error) => return Err(ClientError::Transport(error)),
        }
    }
}

fn spawn_daemon(config: &ClientConfig) -> Result<(), ClientError> {
    let daemon = match &config.daemon_path {
        Some(path) => path.clone(),
        None => sibling_daemon_path()?,
    };
    let mut command = Command::new(daemon);
    if let Some(runtime_directory) = &config.runtime_directory {
        command.arg("--runtime-dir").arg(runtime_directory);
    }
    if let Some(data_directory) = &config.data_directory {
        command.arg("--data-dir").arg(data_directory);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(ClientError::StartDaemon)?;
    Ok(())
}

fn sibling_daemon_path() -> Result<PathBuf, ClientError> {
    let executable = std::env::current_exe().map_err(ClientError::StartDaemon)?;
    let directory = executable
        .parent()
        .ok_or(ClientError::DaemonPathUnavailable)?;
    #[cfg(windows)]
    let name = "alcomd.exe";
    #[cfg(not(windows))]
    let name = "alcomd";
    Ok(directory.join(name))
}

fn is_endpoint_absent(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
    )
}

fn is_transient(error: &io::Error) -> bool {
    is_endpoint_absent(error)
        || error.kind() == io::ErrorKind::WouldBlock
        || cfg!(windows) && error.raw_os_error() == Some(231)
}

async fn read_frame(stream: &mut IpcStream) -> Result<Vec<u8>, ClientError> {
    let mut prefix = [0_u8; 4];
    stream
        .read_exact(&mut prefix)
        .await
        .map_err(ClientError::Transport)?;
    let length = decode_frame_length(prefix).map_err(|_| ClientError::InvalidResponse)?;
    let mut payload = vec![0_u8; length];
    stream
        .read_exact(&mut payload)
        .await
        .map_err(ClientError::Transport)?;
    Ok(payload)
}

/// Errors produced by the RPC client.
#[derive(Debug, Error)]
pub enum ClientError {
    /// Local IPC failed. Callers should avoid exposing the source path verbatim.
    #[error("failed to communicate with the ALCOMD daemon")]
    Transport(#[source] io::Error),
    /// Starting the sibling daemon failed.
    #[error("failed to start the ALCOMD daemon")]
    StartDaemon(#[source] io::Error),
    /// The process layout does not contain a sibling daemon location.
    #[error("the ALCOMD daemon executable location is unavailable")]
    DaemonPathUnavailable,
    /// The daemon did not become reachable within the approved five-second bound.
    #[error("the ALCOMD daemon did not become ready within five seconds")]
    StartTimeout,
    /// The daemon returned a stable public RPC error.
    #[error("daemon request failed")]
    Remote(RpcError),
    /// The daemon response violated the frozen M1 contract.
    #[error("the ALCOMD daemon returned an invalid RPC response")]
    InvalidResponse,
}
