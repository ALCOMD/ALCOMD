//! Official M1 Rust client for the per-user ALCOMD daemon.

use std::io;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use alcomd_platform::{DataConfig, IpcConfig, IpcStream};
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

mod events;
mod m8_oauth;
#[doc(hidden)]
pub mod sdk_stdio;
pub use events::EventSubscription;
pub use m8_oauth::OAuthRejection;

const START_TIMEOUT: Duration = Duration::from_secs(5);
const RETRY_INTERVAL: Duration = Duration::from_millis(50);
const MCP_CREDENTIAL_ENVIRONMENT_KEY: &str = "ALCOMD_MCP_CLIENT_CREDENTIAL";
static REPORTED_PROCESS_INSTANCE: OnceLock<String> = OnceLock::new();

/// Client connection settings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientConfig {
    client_name: String,
    runtime_directory: Option<PathBuf>,
    data_directory: Option<PathBuf>,
    start_daemon: bool,
    require_installation: bool,
    daemon_path: Option<PathBuf>,
    authentication: Option<alcomd_protocol::McpAuthentication>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            client_name: "alcomd-client".to_owned(),
            runtime_directory: None,
            data_directory: None,
            start_daemon: true,
            require_installation: false,
            daemon_path: None,
            authentication: None,
        }
    }
}

impl ClientConfig {
    /// Connects to the current user's Core and starts its registered installation when needed.
    #[must_use]
    pub fn for_installed_core() -> Self {
        Self {
            require_installation: true,
            ..Self::default()
        }
    }

    /// Dedicated protected-IPC reporting role; never native owner or business authority.
    #[must_use]
    pub fn with_transport_observer(mut self) -> Self {
        self.authentication = Some(alcomd_protocol::McpAuthentication {
            kind: "mcp-transport-observer".into(),
            credential: String::new(),
            resource: None,
        });
        self
    }
    pub fn with_mcp_credential(mut self, credential: String) -> Self {
        self.authentication = Some(alcomd_protocol::McpAuthentication {
            kind: "mcp-client".into(),
            credential,
            resource: None,
        });
        self
    }

    /// Identifies this entry point in the existing hello metadata.
    #[must_use]
    pub fn with_client_name(mut self, name: &str) -> Self {
        self.client_name = name.to_owned();
        self
    }

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
    hello_result: HelloResult,
    oauth_rejection: Option<OAuthRejection>,
    stream: IpcStream,
    next_request_id: u64,
    usable: bool,
}

impl AlcomdClient {
    pub async fn mcp_transport_report(
        &mut self,
        params: alcomd_protocol::McpTransportReport,
    ) -> Result<alcomd_protocol::McpTransportReportResult, ClientError> {
        self.call(alcomd_protocol::METHOD_MCP_TRANSPORT_REPORT, params)
            .await
    }
    pub async fn mcp_transport_bind_request(
        &mut self,
        params: alcomd_protocol::McpTransportBindParams,
    ) -> Result<alcomd_protocol::McpTransportBindResult, ClientError> {
        self.call(alcomd_protocol::METHOD_MCP_TRANSPORT_BIND_REQUEST, params)
            .await
    }
    pub async fn mcp_transport_snapshot(
        &mut self,
        params: alcomd_protocol::McpTransportQuery,
    ) -> Result<alcomd_protocol::McpTransportSnapshot, ClientError> {
        self.call(alcomd_protocol::METHOD_MCP_TRANSPORT_SNAPSHOT, params)
            .await
    }
    pub fn supports_capability(&self, capability: &str) -> bool {
        self.hello_result
            .capabilities
            .iter()
            .any(|accepted| accepted == capability)
    }

    /// Details negotiated with the connected daemon, including the shared capabilities.
    #[must_use]
    pub fn hello_result(&self) -> &HelloResult {
        &self.hello_result
    }
    pub async fn extension_cli_commands_list(
        &mut self,
        params: alcomd_protocol::ExtensionCliCommandsListParams,
    ) -> Result<alcomd_protocol::ExtensionCliCommandsListResult, ClientError> {
        self.call(alcomd_protocol::METHOD_EXTENSIONS_CLI_COMMANDS_LIST, params)
            .await
    }
    pub async fn extension_cli_commands_invoke(
        &mut self,
        params: alcomd_protocol::ExtensionCliCommandsInvokeParams,
    ) -> Result<alcomd_protocol::ExtensionMcpToolsCallResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_EXTENSIONS_CLI_COMMANDS_INVOKE,
            params,
        )
        .await
    }
    pub async fn mcp_tools_catalog(
        &mut self,
        params: alcomd_protocol::McpToolsCatalogParams,
    ) -> Result<alcomd_protocol::McpToolsCatalogResult, ClientError> {
        self.call(alcomd_protocol::METHOD_MCP_TOOLS_CATALOG, params)
            .await
    }
    pub async fn extension_mcp_tools_call(
        &mut self,
        params: alcomd_protocol::ExtensionMcpToolsCallParams,
    ) -> Result<alcomd_protocol::ExtensionMcpToolsCallResult, ClientError> {
        self.call(alcomd_protocol::METHOD_EXTENSION_MCP_TOOLS_CALL, params)
            .await
    }

    /// Whether the last exchange completed and this stream can serve another request.
    /// Cancelled or interrupted exchanges must be discarded rather than pooled.
    #[must_use]
    pub fn is_usable(&self) -> bool {
        self.usable
    }

    /// Connects to the per-user daemon, optionally starting its registered installation.
    pub async fn connect(config: ClientConfig) -> Result<Self, ClientError> {
        let stream = connect_with_policy(&config).await?;
        let mut client = Self {
            hello_result: HelloResult::ready(Vec::new(), 1),
            oauth_rejection: None,
            stream,
            next_request_id: 1,
            usable: true,
        };
        client
            .hello(&config.client_name, config.authentication.clone())
            .await?;
        Ok(client)
    }

    pub async fn mcp_runtime_snapshot(
        &mut self,
        params: alcomd_protocol::McpRuntimeSnapshotParams,
    ) -> Result<alcomd_protocol::McpRuntimeSnapshotResult, ClientError> {
        self.call(alcomd_protocol::METHOD_MCP_RUNTIME_SNAPSHOT, params)
            .await
    }

    pub async fn mcp_client_request_create(
        &mut self,
        params: alcomd_protocol::McpClientRequestCreateParams,
    ) -> Result<alcomd_protocol::ClientManagementReview, ClientError> {
        self.call(alcomd_protocol::METHOD_MCP_CLIENT_REQUESTS_CREATE, params)
            .await
    }

    pub async fn mcp_client_request_get(
        &mut self,
        params: alcomd_protocol::McpClientRequestGetParams,
    ) -> Result<alcomd_protocol::ClientManagementReview, ClientError> {
        self.call(alcomd_protocol::METHOD_MCP_CLIENT_REQUESTS_GET, params)
            .await
    }

    pub async fn mcp_client_request_list(
        &mut self,
        params: alcomd_protocol::McpClientsListParams,
    ) -> Result<alcomd_protocol::ClientRequestPage, ClientError> {
        self.call(alcomd_protocol::METHOD_MCP_CLIENT_REQUESTS_LIST, params)
            .await
    }

    pub async fn mcp_client_request_decide(
        &mut self,
        params: alcomd_protocol::McpClientRequestDecideParams,
    ) -> Result<alcomd_protocol::ClientManagementDecision, ClientError> {
        self.call(alcomd_protocol::METHOD_MCP_CLIENT_REQUESTS_DECIDE, params)
            .await
    }

    pub async fn mcp_clients_list(
        &mut self,
        params: alcomd_protocol::McpClientsListParams,
    ) -> Result<alcomd_protocol::McpClientsListResult, ClientError> {
        self.call(alcomd_protocol::METHOD_MCP_CLIENTS_LIST, params)
            .await
    }
    pub async fn clients_list(
        &mut self,
        params: alcomd_protocol::ClientsListParams,
    ) -> Result<alcomd_protocol::ClientsListResult, ClientError> {
        self.call(alcomd_protocol::METHOD_CLIENTS_LIST, params)
            .await
    }
    pub async fn client_instances_list(
        &mut self,
        params: alcomd_protocol::ClientInstancesListParams,
    ) -> Result<alcomd_protocol::ClientInstancesListResult, ClientError> {
        self.call(alcomd_protocol::METHOD_CLIENT_INSTANCES_LIST, params)
            .await
    }
    pub async fn clients_disconnect(
        &mut self,
        params: alcomd_protocol::ClientsDisconnectParams,
    ) -> Result<alcomd_protocol::ClientsDisconnectResult, ClientError> {
        self.call(alcomd_protocol::METHOD_CLIENTS_DISCONNECT, params)
            .await
    }

    pub async fn desktop_focus_project_unity(
        &mut self,
        params: alcomd_protocol::DesktopFocusProjectUnityParams,
    ) -> Result<alcomd_protocol::DesktopActionResult, ClientError> {
        self.call(alcomd_protocol::METHOD_DESKTOP_FOCUS_PROJECT_UNITY, params)
            .await
    }

    pub async fn desktop_open_project_directory(
        &mut self,
        params: alcomd_protocol::DesktopOpenProjectDirectoryParams,
    ) -> Result<alcomd_protocol::DesktopActionResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_DESKTOP_OPEN_PROJECT_DIRECTORY,
            params,
        )
        .await
    }

    pub async fn desktop_open_unity_hub_download(
        &mut self,
        params: alcomd_protocol::DesktopOpenUnityHubDownloadParams,
    ) -> Result<alcomd_protocol::DesktopActionResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_DESKTOP_OPEN_UNITY_HUB_DOWNLOAD,
            params,
        )
        .await
    }

    pub async fn mcp_client_create(
        &mut self,
        params: alcomd_protocol::McpClientsCreateParams,
    ) -> Result<alcomd_protocol::McpClientRecord, ClientError> {
        self.call(alcomd_protocol::METHOD_MCP_CLIENTS_CREATE, params)
            .await
    }
    pub async fn mcp_client_issue_credential(
        &mut self,
        params: alcomd_protocol::McpClientMutationParams,
    ) -> Result<alcomd_protocol::McpClientCredentialResult, ClientError> {
        self.call(alcomd_protocol::METHOD_MCP_CLIENTS_ISSUE_CREDENTIAL, params)
            .await
    }
    pub async fn mcp_client_revoke(
        &mut self,
        params: alcomd_protocol::McpClientMutationParams,
    ) -> Result<alcomd_protocol::McpClientRecord, ClientError> {
        self.call(alcomd_protocol::METHOD_MCP_CLIENTS_REVOKE, params)
            .await
    }

    /// Performs the mandatory M1 handshake.
    async fn hello(
        &mut self,
        client_name: &str,
        authentication: Option<alcomd_protocol::McpAuthentication>,
    ) -> Result<(), ClientError> {
        let reported_instance = REPORTED_PROCESS_INSTANCE.get_or_init(|| {
            let started = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
            format!("{}-{}", std::process::id(), started.as_nanos())
        });
        let params = HelloParams {
            authentication,
            rpc_version: RPC_VERSION,
            client: ClientInfo {
                name: client_name.to_owned(),
                version: env!("CARGO_PKG_VERSION").to_owned(),
                instance_id: reported_instance.clone(),
            },
            capabilities: vec![
                alcomd_protocol::CAPABILITY_MCP_EXTENSION_TOOLS_V1.to_owned(),
                alcomd_protocol::CAPABILITY_EXTENSION_CLI_COMMANDS_V1.to_owned(),
                alcomd_protocol::CAPABILITY_MCP_TOOLS_NOTIFICATIONS_V1.to_owned(),
                alcomd_protocol::CAPABILITY_MCP_PACKAGES_CATALOG_PLAN_V1.to_owned(),
                alcomd_protocol::CAPABILITY_PROJECT_INVENTORY_V1.to_owned(),
                alcomd_protocol::CAPABILITY_TEMPLATE_EDIT_V1.to_owned(),
                alcomd_protocol::CAPABILITY_MCP_TRANSPORT_REPORT_V1.to_owned(),
                alcomd_protocol::CAPABILITY_MCP_TRANSPORT_READ_V1.to_owned(),
                alcomd_protocol::CAPABILITY_MCP_OAUTH_V1.to_owned(),
                alcomd_protocol::CAPABILITY_MCP_CLIENTS_V1.to_owned(),
                alcomd_protocol::CAPABILITY_CLIENTS_REGISTRY_V1.to_owned(),
                alcomd_protocol::CAPABILITY_DESKTOP_ACTIONS_V1.to_owned(),
                alcomd_protocol::CAPABILITY_MCP_RUNTIME_READ_V1.to_owned(),
                CAPABILITY_STATE_CHECK_V1.to_owned(),
                CAPABILITY_OPERATIONS_V1.to_owned(),
                CAPABILITY_EVENTS_REPLAY_V1.to_owned(),
                alcomd_protocol::CAPABILITY_EVENTS_LIVE_V1.to_owned(),
                alcomd_protocol::CAPABILITY_LOGS_READ_V1.to_owned(),
                alcomd_protocol::CAPABILITY_PROJECTS_CREATE_V1.to_owned(),
                alcomd_protocol::CAPABILITY_PROJECT_TEMPLATES_V1.to_owned(),
                alcomd_protocol::CAPABILITY_PROJECTS_READ_V1.to_owned(),
                alcomd_protocol::CAPABILITY_PROJECTS_REGISTRY_V1.to_owned(),
                alcomd_protocol::CAPABILITY_PROJECTS_COPY_V1.to_owned(),
                alcomd_protocol::CAPABILITY_PROJECTS_DELETE_V1.to_owned(),
                alcomd_protocol::CAPABILITY_PROJECTS_UNITY_MIGRATION_V1.to_owned(),
                alcomd_protocol::CAPABILITY_REPOSITORIES_READ_V1.to_owned(),
                alcomd_protocol::CAPABILITY_REPOSITORIES_REGISTRY_V1.to_owned(),
                alcomd_protocol::CAPABILITY_REPOSITORIES_LIST_FILES_V1.to_owned(),
                alcomd_protocol::CAPABILITY_REPOSITORIES_LIST_IMPORT_V1.to_owned(),
                alcomd_protocol::CAPABILITY_PACKAGES_PLAN_V1.to_owned(),
                alcomd_protocol::CAPABILITY_PACKAGES_CANDIDATES_V1.to_owned(),
                alcomd_protocol::CAPABILITY_PACKAGES_APPLY_V1.to_owned(),
                alcomd_protocol::CAPABILITY_PACKAGES_USER_PACKAGES_V1.to_owned(),
                alcomd_protocol::CAPABILITY_UNITY_READ_V1.to_owned(),
                alcomd_protocol::CAPABILITY_UNITY_MANAGE_V1.to_owned(),
                alcomd_protocol::CAPABILITY_UNITY_LAUNCH_V1.to_owned(),
                alcomd_protocol::CAPABILITY_UNITY_HUB_OPEN_V1.to_owned(),
                alcomd_protocol::CAPABILITY_BACKUPS_CREATE_V1.to_owned(),
                alcomd_protocol::CAPABILITY_BACKUPS_RESTORE_V1.to_owned(),
                alcomd_protocol::CAPABILITY_EXTENSIONS_LIFECYCLE_V1.to_owned(),
                alcomd_protocol::CAPABILITY_EXTENSIONS_REMOTE_INSTALL_V1.to_owned(),
                alcomd_protocol::CAPABILITY_EXTENSIONS_PERMISSIONS_V1.to_owned(),
                alcomd_protocol::CAPABILITY_EXTENSIONS_PERMISSIONS_READ_V1.to_owned(),
                alcomd_protocol::CAPABILITY_EXTENSIONS_UI_PORTABLE_V1.to_owned(),
            ],
        };
        let mut hello: HelloResult = self.call(METHOD_SYSTEM_HELLO, &params).await?;
        if hello.rpc_version != params.rpc_version || hello.validate().is_err() {
            self.usable = false;
            return Err(ClientError::InvalidResponse);
        }
        hello
            .capabilities
            .retain(|capability| params.capabilities.contains(capability));
        self.hello_result = hello;
        Ok(())
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
    pub async fn logs_read(
        &mut self,
        params: alcomd_protocol::LogsReadParams,
    ) -> Result<alcomd_protocol::LogsReadResult, ClientError> {
        self.call(alcomd_protocol::METHOD_LOGS_READ, params).await
    }

    pub async fn operations_list(
        &mut self,
        cursor: Option<OperationsListCursor>,
        limit: Option<u32>,
        active_only: bool,
    ) -> Result<OperationsListResult, ClientError> {
        self.call(
            METHOD_OPERATIONS_LIST,
            OperationsListParams {
                cursor,
                limit,
                active_only,
            },
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

    /// Reads the durable Config Schema 1 settings snapshot.
    pub async fn settings_get(
        &mut self,
    ) -> Result<alcomd_protocol::SettingsGetResult, ClientError> {
        self.call(alcomd_protocol::METHOD_SETTINGS_GET, json!({}))
            .await
    }

    /// Applies an optimistic-concurrency settings update.
    pub async fn settings_update(
        &mut self,
        params: alcomd_protocol::SettingsUpdateParams,
    ) -> Result<alcomd_protocol::SettingsGetResult, ClientError> {
        self.call(alcomd_protocol::METHOD_SETTINGS_UPDATE, params)
            .await
    }

    /// Opens the Core-resolved local Unity Hub, guarded by accepted settings evidence.
    pub async fn unity_hub_open(
        &mut self,
        params: alcomd_protocol::UnityHubOpenParams,
    ) -> Result<alcomd_protocol::UnityHubOpenResult, ClientError> {
        self.call(alcomd_protocol::METHOD_UNITY_HUB_OPEN, params)
            .await
    }

    pub async fn extensions_list(
        &mut self,
        params: alcomd_protocol::ExtensionsListParams,
    ) -> Result<alcomd_protocol::ExtensionsListResult, ClientError> {
        self.call(alcomd_protocol::METHOD_EXTENSIONS_LIST, params)
            .await
    }

    pub async fn extension_get(
        &mut self,
        extension_id: String,
    ) -> Result<alcomd_protocol::ExtensionResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_EXTENSIONS_GET,
            alcomd_protocol::ExtensionIdParams { extension_id },
        )
        .await
    }

    pub async fn extension_permissions_list(
        &mut self,
        params: alcomd_protocol::ExtensionPermissionsListParams,
    ) -> Result<alcomd_protocol::ExtensionPermissionsListResult, ClientError> {
        self.call(alcomd_protocol::METHOD_EXTENSIONS_PERMISSIONS_LIST, params)
            .await
    }

    pub async fn extension_inspect_source(
        &mut self,
        params: alcomd_protocol::ExtensionInspectSourceParams,
    ) -> Result<alcomd_protocol::ExtensionSourcePreview, ClientError> {
        self.call(alcomd_protocol::METHOD_EXTENSIONS_INSPECT_SOURCE, params)
            .await
    }
    pub async fn extension_plan_install_from_source(
        &mut self,
        params: alcomd_protocol::ExtensionPlanInstallFromSourceParams,
    ) -> Result<alcomd_protocol::ExtensionPlanResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_EXTENSIONS_PLAN_INSTALL_FROM_SOURCE,
            params,
        )
        .await
    }
    pub async fn extension_plan_install(
        &mut self,
        params: alcomd_protocol::ExtensionPlanInstallParams,
    ) -> Result<alcomd_protocol::ExtensionPlanResult, ClientError> {
        self.call(alcomd_protocol::METHOD_EXTENSIONS_PLAN_INSTALL, params)
            .await
    }

    pub async fn extension_apply_install(
        &mut self,
        params: alcomd_protocol::ExtensionInstallApplyParams,
    ) -> Result<alcomd_protocol::ExtensionOperationResult, ClientError> {
        self.call(alcomd_protocol::METHOD_EXTENSIONS_APPLY_INSTALL, params)
            .await
    }

    pub async fn extension_enable(
        &mut self,
        params: alcomd_protocol::ExtensionLifecycleParams,
    ) -> Result<alcomd_protocol::ExtensionResult, ClientError> {
        self.call(alcomd_protocol::METHOD_EXTENSIONS_ENABLE, params)
            .await
    }

    pub async fn extension_enable_with_consent(
        &mut self,
        params: alcomd_protocol::ExtensionEnableParams,
    ) -> Result<alcomd_protocol::ExtensionResult, ClientError> {
        self.call(alcomd_protocol::METHOD_EXTENSIONS_ENABLE, params)
            .await
    }

    pub async fn extension_disable(
        &mut self,
        params: alcomd_protocol::ExtensionLifecycleParams,
    ) -> Result<alcomd_protocol::ExtensionResult, ClientError> {
        self.call(alcomd_protocol::METHOD_EXTENSIONS_DISABLE, params)
            .await
    }

    pub async fn extension_plan_uninstall(
        &mut self,
        params: alcomd_protocol::ExtensionPlanUninstallParams,
    ) -> Result<alcomd_protocol::ExtensionPlanResult, ClientError> {
        self.call(alcomd_protocol::METHOD_EXTENSIONS_PLAN_UNINSTALL, params)
            .await
    }

    pub async fn extension_apply_uninstall(
        &mut self,
        params: alcomd_protocol::ExtensionApplyParams,
    ) -> Result<alcomd_protocol::ExtensionOperationResult, ClientError> {
        self.call(alcomd_protocol::METHOD_EXTENSIONS_APPLY_UNINSTALL, params)
            .await
    }

    pub async fn extension_set_grant(
        &mut self,
        params: alcomd_protocol::ExtensionGrantParams,
    ) -> Result<alcomd_protocol::ExtensionGrantResult, ClientError> {
        self.call(alcomd_protocol::METHOD_EXTENSIONS_SET_GRANT, params)
            .await
    }

    pub async fn extension_revoke_grant(
        &mut self,
        params: alcomd_protocol::ExtensionGrantParams,
    ) -> Result<alcomd_protocol::ExtensionGrantResult, ClientError> {
        self.call(alcomd_protocol::METHOD_EXTENSIONS_REVOKE_GRANT, params)
            .await
    }

    pub async fn extension_ui_open(
        &mut self,
        params: alcomd_protocol::ExtensionUiOpenParams,
    ) -> Result<alcomd_protocol::ExtensionUiOpenResult, ClientError> {
        self.call(alcomd_protocol::METHOD_EXTENSIONS_UI_OPEN, params)
            .await
    }

    pub async fn extension_ui_refresh(
        &mut self,
        params: alcomd_protocol::ExtensionUiRefreshParams,
    ) -> Result<alcomd_protocol::ExtensionUiSnapshotResult, ClientError> {
        self.call(alcomd_protocol::METHOD_EXTENSIONS_UI_REFRESH, params)
            .await
    }

    pub async fn extension_ui_dispatch(
        &mut self,
        params: alcomd_protocol::ExtensionUiDispatchParams,
    ) -> Result<alcomd_protocol::ExtensionUiDispatchResult, ClientError> {
        self.call(alcomd_protocol::METHOD_EXTENSIONS_UI_DISPATCH, params)
            .await
    }

    pub async fn extension_ui_close(
        &mut self,
        params: alcomd_protocol::ExtensionUiCloseParams,
    ) -> Result<alcomd_protocol::ExtensionUiCloseResult, ClientError> {
        self.call(alcomd_protocol::METHOD_EXTENSIONS_UI_CLOSE, params)
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

    pub async fn package_plan_reinstall(
        &mut self,
        params: alcomd_protocol::PackagePlanReinstallParams,
    ) -> Result<alcomd_protocol::PackagePlan, ClientError> {
        self.call(alcomd_protocol::METHOD_PACKAGES_PLAN_REINSTALL, params)
            .await
    }

    pub async fn mcp_package_query(
        &mut self,
        params: alcomd_protocol::McpPackageQuery,
    ) -> Result<alcomd_protocol::McpPackagePage, ClientError> {
        self.call(alcomd_protocol::METHOD_MCP_PACKAGES_QUERY, params)
            .await
    }

    pub async fn mcp_package_plan(
        &mut self,
        params: alcomd_protocol::McpPackagePlanRequest,
    ) -> Result<alcomd_protocol::McpPackagePlan, ClientError> {
        self.call(alcomd_protocol::METHOD_MCP_PACKAGES_PLAN, params)
            .await
    }

    pub async fn package_query_project_candidates(
        &mut self,
        params: alcomd_protocol::ProjectCandidateRequest,
    ) -> Result<alcomd_protocol::ProjectCandidatePage, ClientError> {
        self.call(
            alcomd_protocol::METHOD_PACKAGES_QUERY_PROJECT_CANDIDATES,
            params,
        )
        .await
    }

    pub async fn package_plan_bulk(
        &mut self,
        params: alcomd_protocol::PackagePlanBulkParams,
    ) -> Result<alcomd_protocol::PackagePlan, ClientError> {
        self.call(alcomd_protocol::METHOD_PACKAGES_PLAN_BULK, params)
            .await
    }

    pub async fn package_apply_plan(
        &mut self,
        params: alcomd_protocol::PackageApplyPlanParams,
    ) -> Result<alcomd_protocol::PackageApplyPlanResult, ClientError> {
        self.call(alcomd_protocol::METHOD_PACKAGES_APPLY_PLAN, params)
            .await
    }

    pub async fn user_packages_list(
        &mut self,
        params: alcomd_protocol::UserPackagesListParams,
    ) -> Result<alcomd_protocol::UserPackagesListResult, ClientError> {
        self.call(alcomd_protocol::METHOD_PACKAGES_USER_PACKAGES_LIST, params)
            .await
    }

    pub async fn user_package_get(
        &mut self,
        params: alcomd_protocol::UserPackageGetParams,
    ) -> Result<alcomd_protocol::UserPackageResult, ClientError> {
        self.call(alcomd_protocol::METHOD_PACKAGES_USER_PACKAGES_GET, params)
            .await
    }

    pub async fn user_package_enroll(
        &mut self,
        params: alcomd_protocol::UserPackageEnrollParams,
    ) -> Result<alcomd_protocol::UserPackageWriteResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_PACKAGES_USER_PACKAGES_ENROLL,
            params,
        )
        .await
    }

    pub async fn user_package_refresh(
        &mut self,
        params: alcomd_protocol::UserPackageMutationParams,
    ) -> Result<alcomd_protocol::UserPackageWriteResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_PACKAGES_USER_PACKAGES_REFRESH,
            params,
        )
        .await
    }

    pub async fn user_package_remove(
        &mut self,
        params: alcomd_protocol::UserPackageMutationParams,
    ) -> Result<alcomd_protocol::UserPackageRemoveResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_PACKAGES_USER_PACKAGES_REMOVE,
            params,
        )
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

    pub async fn unity_project_launch_config_get(
        &mut self,
        project_id: String,
    ) -> Result<alcomd_protocol::ProjectUnityLaunchConfigResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_UNITY_PROJECT_LAUNCH_CONFIG_GET,
            alcomd_protocol::UnityProjectIdParams { project_id },
        )
        .await
    }

    pub async fn unity_project_launch_config_set(
        &mut self,
        params: alcomd_protocol::ProjectUnityLaunchConfigSetParams,
    ) -> Result<alcomd_protocol::ProjectUnityLaunchConfigMutationResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_UNITY_PROJECT_LAUNCH_CONFIG_SET,
            params,
        )
        .await
    }

    pub async fn unity_project_launch_config_clear(
        &mut self,
        params: alcomd_protocol::ProjectUnityLaunchConfigClearParams,
    ) -> Result<alcomd_protocol::ProjectUnityLaunchConfigMutationResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_UNITY_PROJECT_LAUNCH_CONFIG_CLEAR,
            params,
        )
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

    pub async fn unity_launch_options(
        &mut self,
        params: alcomd_protocol::UnityLaunchOptionsParams,
    ) -> Result<alcomd_protocol::UnityLaunchOptionsResult, ClientError> {
        self.call(alcomd_protocol::METHOD_UNITY_LAUNCH_OPTIONS, params)
            .await
    }

    pub async fn projects_create(
        &mut self,
        params: alcomd_protocol::ProjectsCreateParams,
    ) -> Result<alcomd_protocol::ProjectsCreateResult, ClientError> {
        self.call(alcomd_protocol::METHOD_PROJECTS_CREATE, params)
            .await
    }

    pub async fn project_inventory(
        &mut self,
        params: alcomd_protocol::ProjectInventoryParams,
    ) -> Result<alcomd_protocol::ProjectInventoryResult, ClientError> {
        self.call(alcomd_protocol::METHOD_PROJECT_INVENTORY, params)
            .await
    }

    pub async fn template_get(
        &mut self,
        params: alcomd_protocol::ProjectTemplateGetParams,
    ) -> Result<alcomd_protocol::ProjectTemplateGetResult, ClientError> {
        self.call(alcomd_protocol::METHOD_TEMPLATE_GET, params)
            .await
    }

    pub async fn template_edit(
        &mut self,
        params: alcomd_protocol::ProjectTemplateEditParams,
    ) -> Result<alcomd_protocol::ProjectTemplatesSaveResult, ClientError> {
        self.call(alcomd_protocol::METHOD_TEMPLATE_EDIT, params)
            .await
    }

    pub async fn project_templates_list(
        &mut self,
        params: alcomd_protocol::ProjectTemplatesListParams,
    ) -> Result<alcomd_protocol::ProjectTemplatesListResult, ClientError> {
        self.call(alcomd_protocol::METHOD_PROJECT_TEMPLATES_LIST, params)
            .await
    }

    pub async fn project_templates_save(
        &mut self,
        params: alcomd_protocol::ProjectTemplatesSaveParams,
    ) -> Result<alcomd_protocol::ProjectTemplatesSaveResult, ClientError> {
        self.call(alcomd_protocol::METHOD_PROJECT_TEMPLATES_SAVE, params)
            .await
    }

    pub async fn project_templates_remove(
        &mut self,
        params: alcomd_protocol::ProjectTemplatesRemoveParams,
    ) -> Result<alcomd_protocol::ProjectTemplatesRemoveResult, ClientError> {
        self.call(alcomd_protocol::METHOD_PROJECT_TEMPLATES_REMOVE, params)
            .await
    }

    pub async fn project_templates_export(
        &mut self,
        params: alcomd_protocol::ProjectTemplatesExportParams,
    ) -> Result<alcomd_protocol::ProjectTemplatesSaveResult, ClientError> {
        self.call(alcomd_protocol::METHOD_PROJECT_TEMPLATES_EXPORT, params)
            .await
    }

    pub async fn backup_create(
        &mut self,
        params: alcomd_protocol::BackupCreateParams,
    ) -> Result<OperationAccepted, ClientError> {
        self.call(alcomd_protocol::METHOD_BACKUPS_CREATE, params)
            .await
    }

    pub async fn backup_restore(
        &mut self,
        params: alcomd_protocol::BackupRestoreParams,
    ) -> Result<OperationAccepted, ClientError> {
        self.call(alcomd_protocol::METHOD_BACKUPS_RESTORE, params)
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

    pub async fn get_plan(
        &mut self,
        params: alcomd_protocol::PlansGetParams,
    ) -> Result<alcomd_protocol::PlansGetResult, ClientError> {
        self.call(alcomd_protocol::METHOD_PLANS_GET, params).await
    }

    pub async fn project_plan_copy(
        &mut self,
        params: alcomd_protocol::ProjectsPlanCopyParams,
    ) -> Result<alcomd_protocol::ProjectsPlanCopyResult, ClientError> {
        self.call(alcomd_protocol::METHOD_PROJECTS_PLAN_COPY, params)
            .await
    }

    pub async fn project_apply_copy(
        &mut self,
        params: alcomd_protocol::ProjectsApplyCopyParams,
    ) -> Result<alcomd_protocol::ProjectsApplyCopyResult, ClientError> {
        self.call(alcomd_protocol::METHOD_PROJECTS_APPLY_COPY, params)
            .await
    }

    pub async fn project_plan_delete_directory(
        &mut self,
        params: alcomd_protocol::ProjectsPlanDeleteDirectoryParams,
    ) -> Result<alcomd_protocol::ProjectsPlanDeleteDirectoryResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_PROJECTS_PLAN_DELETE_DIRECTORY,
            params,
        )
        .await
    }

    pub async fn project_apply_delete_directory(
        &mut self,
        params: alcomd_protocol::ProjectsApplyDeleteDirectoryParams,
    ) -> Result<alcomd_protocol::ProjectsApplyDeleteDirectoryResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_PROJECTS_APPLY_DELETE_DIRECTORY,
            params,
        )
        .await
    }

    pub async fn project_plan_unity_migration(
        &mut self,
        params: alcomd_protocol::ProjectsPlanUnityMigrationParams,
    ) -> Result<alcomd_protocol::ProjectsPlanUnityMigrationResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_PROJECTS_PLAN_UNITY_MIGRATION,
            params,
        )
        .await
    }

    pub async fn project_apply_unity_migration(
        &mut self,
        params: alcomd_protocol::ProjectsApplyUnityMigrationParams,
    ) -> Result<alcomd_protocol::ProjectsApplyUnityMigrationResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_PROJECTS_APPLY_UNITY_MIGRATION,
            params,
        )
        .await
    }

    pub async fn projects_list(
        &mut self,
        cursor: Option<alcomd_protocol::ProjectRegistryCursor>,
        limit: Option<u32>,
    ) -> Result<alcomd_protocol::ProjectsListResult, ClientError> {
        self.projects_list_with_view(alcomd_protocol::ProjectsListParams {
            cursor,
            limit,
            ..Default::default()
        })
        .await
    }

    pub async fn projects_list_with_view(
        &mut self,
        params: alcomd_protocol::ProjectsListParams,
    ) -> Result<alcomd_protocol::ProjectsListResult, ClientError> {
        self.call(alcomd_protocol::METHOD_PROJECTS_LIST, params)
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
        params: alcomd_protocol::ProjectRegisterParams,
    ) -> Result<alcomd_protocol::ProjectWriteResult, ClientError> {
        self.call(alcomd_protocol::METHOD_PROJECTS_REGISTER, params)
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

    pub async fn project_set_favorite(
        &mut self,
        params: alcomd_protocol::ProjectSetFavoriteParams,
    ) -> Result<alcomd_protocol::ProjectWriteResult, ClientError> {
        self.call(alcomd_protocol::METHOD_PROJECTS_SET_FAVORITE, params)
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
    ) -> Result<alcomd_protocol::RepositoryInspectResult, ClientError> {
        self.repository_inspect_with_credentials(source, None).await
    }

    pub async fn repository_inspect_with_credentials(
        &mut self,
        source: alcomd_protocol::RepositorySource,
        credentials: Option<alcomd_protocol::RepositoryCredentials>,
    ) -> Result<alcomd_protocol::RepositoryInspectResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_REPOSITORIES_INSPECT,
            alcomd_protocol::RepositoryInspectParams {
                source,
                credentials,
            },
        )
        .await
    }

    pub async fn repository_list_inspect_file(
        &mut self,
        path: String,
    ) -> Result<alcomd_protocol::RepositoryListInspection, ClientError> {
        self.call(
            alcomd_protocol::METHOD_REPOSITORIES_INSPECT_LIST_FILE,
            alcomd_protocol::RepositoryListInspectParams { path },
        )
        .await
    }

    pub async fn repository_list_preview_export(
        &mut self,
    ) -> Result<alcomd_protocol::RepositoryListPreview, ClientError> {
        self.repository_configuration_preview_export(alcomd_protocol::RepositoryListFormat::Native)
            .await
    }

    pub async fn repository_configuration_preview_export(
        &mut self,
        format: alcomd_protocol::RepositoryListFormat,
    ) -> Result<alcomd_protocol::RepositoryListPreview, ClientError> {
        self.call(
            alcomd_protocol::METHOD_REPOSITORIES_PREVIEW_LIST_EXPORT,
            alcomd_protocol::RepositoryListPreviewParams { format },
        )
        .await
    }

    pub async fn repository_configuration_export_file(
        &mut self,
        target_path: String,
        expected_fingerprint: String,
        format: alcomd_protocol::RepositoryListFormat,
        idempotency_key: String,
    ) -> Result<alcomd_protocol::RepositoryListExportResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_REPOSITORIES_EXPORT_LIST_FILE,
            alcomd_protocol::RepositoryListExportParams {
                idempotency_key,
                target_path,
                expected_fingerprint,
                format,
            },
        )
        .await
    }

    pub async fn repository_list_import_file(
        &mut self,
        params: alcomd_protocol::RepositoryListImportParams,
    ) -> Result<alcomd_protocol::RepositoryListImportResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_REPOSITORIES_IMPORT_LIST_FILE,
            params,
        )
        .await
    }

    pub async fn repository_list_cancel_registration(
        &mut self,
        params: alcomd_protocol::RepositoryListCancelParams,
    ) -> Result<alcomd_protocol::RepositoryListCancelResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_REPOSITORIES_CANCEL_LIST_REGISTRATION,
            params,
        )
        .await
    }

    pub async fn repository_list_export_file(
        &mut self,
        target_path: String,
        expected_fingerprint: String,
        idempotency_key: String,
    ) -> Result<alcomd_protocol::RepositoryListExportResult, ClientError> {
        self.repository_configuration_export_file(
            target_path,
            expected_fingerprint,
            alcomd_protocol::RepositoryListFormat::Native,
            idempotency_key,
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
        preview_id: String,
        idempotency_key: String,
    ) -> Result<alcomd_protocol::RepositoryWriteResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_REPOSITORIES_REGISTER,
            alcomd_protocol::RepositoryRegisterParams {
                preview_id,
                idempotency_key,
            },
        )
        .await
    }

    pub async fn repository_set_credentials(
        &mut self,
        params: alcomd_protocol::RepositorySetCredentialsParams,
    ) -> Result<alcomd_protocol::RepositoryWriteResult, ClientError> {
        self.call(alcomd_protocol::METHOD_REPOSITORIES_SET_CREDENTIALS, params)
            .await
    }

    pub async fn repository_move(
        &mut self,
        params: alcomd_protocol::RepositoryMoveParams,
    ) -> Result<alcomd_protocol::RepositoryMoveResult, ClientError> {
        self.call(alcomd_protocol::METHOD_REPOSITORIES_MOVE, params)
            .await
    }

    pub async fn repository_set_display_name(
        &mut self,
        params: alcomd_protocol::RepositorySetDisplayNameParams,
    ) -> Result<alcomd_protocol::RepositoryWriteResult, ClientError> {
        self.call(
            alcomd_protocol::METHOD_REPOSITORIES_SET_DISPLAY_NAME,
            params,
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

    pub async fn repositories_refresh_all(
        &mut self,
        params: alcomd_protocol::RepositoryRefreshAllParams,
    ) -> Result<alcomd_protocol::RepositoryRefreshAllResult, ClientError> {
        self.call(alcomd_protocol::METHOD_REPOSITORIES_REFRESH_ALL, params)
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

    pub(crate) async fn call<P, T>(&mut self, method: &str, params: P) -> Result<T, ClientError>
    where
        P: Serialize,
        T: DeserializeOwned,
    {
        let deadline = read_deadline(method);
        self.call_with_deadline(method, params, deadline).await
    }
    async fn call_with_deadline<P: Serialize, T: DeserializeOwned>(
        &mut self,
        method: &str,
        params: P,
        deadline: Option<Duration>,
    ) -> Result<T, ClientError> {
        self.oauth_rejection = None;
        if !self.usable {
            return Err(ClientError::InvalidResponse);
        }
        // If a caller cancels while a frame is in flight, this connection must
        // not be reused with an unread response or a partially consumed frame.
        self.usable = false;
        let result = match deadline {
            Some(duration) => tokio::time::timeout(duration, self.exchange(method, params))
                .await
                .unwrap_or(Err(ClientError::ReadTimeout)),
            None => self.exchange(method, params).await,
        };
        self.usable = result.is_ok() || matches!(&result, Err(ClientError::Remote(_)));
        if let Err(ClientError::Remote(error)) = &result
            && let Some(tag) = error.data.as_ref().and_then(|v| v["oauthError"].as_str())
        {
            self.oauth_rejection = match tag {
                "invalid_grant" | "invalid_client" => Some(OAuthRejection::InvalidToken),
                _ => None,
            };
        }
        result
    }

    async fn exchange<P, T>(&mut self, method: &str, params: P) -> Result<T, ClientError>
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
        decode_response(&payload, &request.id)
    }
}

fn decode_response<T: DeserializeOwned>(
    payload: &[u8],
    request_id: &str,
) -> Result<T, ClientError> {
    let response: Response<T> =
        serde_json::from_slice(payload).map_err(|_| ClientError::InvalidResponse)?;
    match response {
        Response::Success(success) if success.id == request_id => Ok(success.result),
        Response::Error(error) if error.id.as_deref() == Some(request_id) => {
            Err(ClientError::Remote(error.error))
        }
        Response::Success(_) | Response::Error(_) => Err(ClientError::InvalidResponse),
    }
}

fn read_deadline(method: &str) -> Option<Duration> {
    use alcomd_protocol::*;
    if matches!(method, METHOD_SYSTEM_HELLO | METHOD_EVENTS_SUBSCRIBE) {
        return Some(Duration::from_secs(5));
    }
    matches!(
        method,
        METHOD_SYSTEM_STATUS
            | METHOD_OPERATIONS_GET
            | METHOD_OPERATIONS_LIST
            | METHOD_MCP_TOOLS_CATALOG
            | METHOD_MCP_CLIENTS_LIST
            | METHOD_MCP_RUNTIME_SNAPSHOT
            | METHOD_LOGS_READ
            | METHOD_EVENTS_LIST
            | METHOD_PROJECTS_LIST
            | METHOD_PROJECTS_GET
            | METHOD_REPOSITORIES_LIST
            | METHOD_REPOSITORIES_GET
            | METHOD_REPOSITORIES_PACKAGES
            | METHOD_PACKAGES_QUERY_PROJECT_CANDIDATES
            | METHOD_MCP_PACKAGES_QUERY
            | METHOD_PACKAGES_USER_PACKAGES_LIST
            | METHOD_PACKAGES_USER_PACKAGES_GET
            | METHOD_UNITY_INSTALLATIONS_LIST
            | METHOD_UNITY_INSTALLATIONS_GET
            | METHOD_UNITY_PROJECT_LAUNCH_CONFIG_GET
            | METHOD_UNITY_WRITER_STATE
            | METHOD_UNITY_LAUNCH_OPTIONS
            | METHOD_PROJECT_TEMPLATES_LIST
            | alcomd_protocol::METHOD_PROJECT_INVENTORY
            | alcomd_protocol::METHOD_TEMPLATE_GET
            | METHOD_EXTENSIONS_LIST
            | METHOD_EXTENSIONS_GET
            | METHOD_EXTENSIONS_PERMISSIONS_LIST
            | METHOD_SETTINGS_GET
    )
    .then_some(Duration::from_secs(10))
}

async fn connect_with_policy(config: &ClientConfig) -> Result<IpcStream, ClientError> {
    let ipc = config.ipc_config();
    let deadline = Instant::now() + START_TIMEOUT;
    let mut started = false;
    #[cfg(windows)]
    let mut startup_guard = None;
    loop {
        match alcomd_platform::connect(&ipc).await {
            Ok(stream) => return Ok(stream),
            Err(error) if is_transient(&error) => {
                if Instant::now() >= deadline {
                    return Err(ClientError::StartTimeout);
                }
                if is_endpoint_absent(&error) && config.start_daemon && !started {
                    #[cfg(windows)]
                    if startup_guard.is_none() {
                        startup_guard = alcomd_platform::CoreStartupGuard::try_acquire()
                            .map_err(ClientError::StartDaemon)?;
                        if startup_guard.is_none() {
                            tokio::time::sleep(RETRY_INTERVAL).await;
                        }
                        // Recheck the endpoint after claiming the right to start Core.
                        continue;
                    }
                    spawn_daemon(config)?;
                    started = true;
                }
                tokio::time::sleep(RETRY_INTERVAL).await;
            }
            Err(error) => return Err(ClientError::Transport(error)),
        }
    }
}

fn spawn_daemon(config: &ClientConfig) -> Result<(), ClientError> {
    let command = daemon_command(config)?;
    #[cfg(windows)]
    let started = alcomd_platform::start_core_process(
        std::path::Path::new(command.get_program()),
        command.get_args(),
        command.get_current_dir(),
        MCP_CREDENTIAL_ENVIRONMENT_KEY,
    );
    #[cfg(not(windows))]
    let started = {
        let mut command = command;
        command.spawn().map(drop)
    };
    started.map_err(ClientError::StartDaemon)
}

fn daemon_command(config: &ClientConfig) -> Result<Command, ClientError> {
    let mut command = if let Some(path) = &config.daemon_path {
        Command::new(path)
    } else if config.data_directory.is_some() && !config.require_installation {
        // The development data override can live under a shared temporary
        // ancestor. It selects the sibling Core without trusting that
        // directory as an installation registration source.
        Command::new(sibling_daemon_path()?)
    } else {
        let data = config
            .data_directory
            .clone()
            .map(DataConfig::isolated)
            .unwrap_or_default();
        match alcomd_platform::registered_core_executable(&data) {
            Ok(Some(daemon)) => {
                let mut command = Command::new(&daemon);
                command.current_dir(daemon.parent().ok_or(ClientError::DaemonPathUnavailable)?);
                command
            }
            Ok(None) if config.require_installation => {
                return Err(ClientError::DaemonPathUnavailable);
            }
            Ok(None) => default_daemon_command()?,
            #[cfg(target_os = "linux")]
            Err(error)
                if !config.require_installation && error.kind() == io::ErrorKind::InvalidData =>
            {
                let entry_point = std::env::current_exe().map_err(ClientError::StartDaemon)?;
                let bootstrap =
                    alcomd_platform::linux_retired_deb_installation_bootstrap(&entry_point, &data)
                        .map_err(ClientError::StartDaemon)?
                        .ok_or(ClientError::StartDaemon(error))?;
                installation_daemon_command(&bootstrap)?
            }
            Err(error) => return Err(ClientError::StartDaemon(error)),
        }
    };
    command.env_remove(MCP_CREDENTIAL_ENVIRONMENT_KEY);
    if let Some(runtime_directory) = &config.runtime_directory {
        command.arg("--runtime-dir").arg(runtime_directory);
    }
    if let Some(data_directory) = &config.data_directory {
        command.arg("--data-dir").arg(data_directory);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    Ok(command)
}

fn default_daemon_command() -> Result<Command, ClientError> {
    #[cfg(target_os = "linux")]
    use alcomd_platform::linux_installation_bootstrap as installation_bootstrap;
    #[cfg(windows)]
    use alcomd_platform::windows_installation_bootstrap as installation_bootstrap;

    #[cfg(any(windows, target_os = "linux"))]
    if let Some(bootstrap) =
        installation_bootstrap(&std::env::current_exe().map_err(ClientError::StartDaemon)?)
            .map_err(ClientError::StartDaemon)?
    {
        return installation_daemon_command(&bootstrap);
    }
    Ok(Command::new(sibling_daemon_path()?))
}

#[cfg(any(windows, target_os = "linux"))]
fn installation_daemon_command(bootstrap: &std::path::Path) -> Result<Command, ClientError> {
    let runtime = bootstrap
        .parent()
        .ok_or(ClientError::DaemonPathUnavailable)?;
    let mut command = Command::new(bootstrap);
    command.arg("start-installed-core").current_dir(runtime);
    Ok(command)
}

fn sibling_daemon_path() -> Result<PathBuf, ClientError> {
    let executable = std::env::current_exe().map_err(ClientError::StartDaemon)?;
    let directory = executable
        .parent()
        .ok_or(ClientError::DaemonPathUnavailable)?;
    Ok(directory.join(alcomd_platform::CORE_EXECUTABLE_NAME))
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
    /// A bounded read/handshake did not finish; discard the interrupted connection.
    #[error("the ALCOMD read did not finish within its deadline")]
    ReadTimeout,
    /// The daemon returned a stable public RPC error.
    #[error("daemon request failed")]
    Remote(RpcError),
    /// The daemon response violated the frozen M1 contract.
    #[error("the ALCOMD daemon returned an invalid RPC response")]
    InvalidResponse,
}

impl ClientError {
    /// Only transient transport/start failures may restart a live subscription.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Transport(error) => !matches!(
                error.kind(),
                io::ErrorKind::PermissionDenied
                    | io::ErrorKind::InvalidInput
                    | io::ErrorKind::InvalidData
                    | io::ErrorKind::Unsupported
            ),
            Self::StartTimeout | Self::ReadTimeout => true,
            Self::Remote(error) => matches!(
                error.code.as_str(),
                "daemon_unavailable" | "store_unavailable" | "runtime_unavailable"
            ),
            Self::StartDaemon(_) | Self::DaemonPathUnavailable | Self::InvalidResponse => false,
        }
    }
}

#[cfg(test)]
mod connection_reuse_tests {
    use super::*;

    #[test]
    fn mcp_startup_secret_is_redacted_and_excluded_from_daemon_environment() {
        let marker = "startup-secret-must-not-be-in-debug".repeat(2);
        let config = ClientConfig::default()
            .with_daemon_path(PathBuf::from("not-executed"))
            .with_mcp_credential(marker.clone());
        assert!(!format!("{config:?}").contains(&marker));
        let command = daemon_command(&config).expect("construct, never spawn");
        let environment: Vec<_> = command.get_envs().collect();
        assert_eq!(
            environment,
            [(std::ffi::OsStr::new(MCP_CREDENTIAL_ENVIRONMENT_KEY), None)]
        );
    }

    #[test]
    fn development_data_override_uses_sibling_without_reading_installation_registration() {
        // A relative temporary location would fail registration validation.
        // Command construction alone never reads or modifies its contents.
        let data = PathBuf::from("isolated-cli-fixture");
        let development = ClientConfig::default().with_data_directory(data.clone());
        let command = daemon_command(&development).expect("development sibling");
        assert_eq!(
            command.get_program(),
            sibling_daemon_path()
                .expect("sibling executable")
                .as_os_str()
        );
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [std::ffi::OsStr::new("--data-dir"), data.as_os_str()]
        );

        let explicit_daemon = PathBuf::from("explicit-daemon");
        let command = daemon_command(&development.with_daemon_path(explicit_daemon.clone()))
            .expect("explicit development executable");
        assert_eq!(command.get_program(), explicit_daemon.as_os_str());

        let registration_error =
            alcomd_platform::core_installation_registration(&DataConfig::isolated(data.clone()))
                .expect_err("relative data directory must fail installation validation");
        let installed = ClientConfig::for_installed_core().with_data_directory(data);
        assert!(matches!(
            daemon_command(&installed),
            Err(ClientError::StartDaemon(error)) if error.kind() == registration_error.kind()
        ));
    }

    #[cfg(unix)]
    async fn pair() -> (IpcStream, IpcStream) {
        IpcStream::pair().expect("private stream pair")
    }

    #[cfg(windows)]
    async fn pair() -> (IpcStream, IpcStream) {
        use std::sync::atomic::{AtomicU64, Ordering};
        use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};
        static INSTANCE_SEQUENCE: AtomicU64 = AtomicU64::new(1);
        let sequence = INSTANCE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let pipe = format!(
            r"\\.\pipe\alcomd-client-reuse-test-{}-{sequence}",
            std::process::id()
        );
        let server = ServerOptions::new()
            .first_pipe_instance(true)
            .reject_remote_clients(true)
            .create(&pipe)
            .expect("private server");
        let client = ClientOptions::new().open(&pipe).expect("private client");
        server.connect().await.expect("accept private client");
        (IpcStream::Client(client), IpcStream::Server(server))
    }

    fn client(stream: IpcStream) -> AlcomdClient {
        AlcomdClient {
            hello_result: HelloResult::ready(Vec::new(), 1),
            oauth_rejection: None,
            stream,
            next_request_id: 1,
            usable: true,
        }
    }

    async fn hello_daemon(mut stream: IpcStream, result: serde_json::Value) {
        let request: RequestEnvelope =
            serde_json::from_slice(&read_frame(&mut stream).await.expect("hello request"))
                .expect("decode hello");
        assert_eq!(request.method, METHOD_SYSTEM_HELLO);
        assert_eq!(request.params["rpcVersion"], RPC_VERSION);
        let payload =
            serde_json::to_vec(&json!({ "id": request.id, "result": result })).expect("JSON");
        stream
            .write_all(&encode_frame(&payload).expect("frame"))
            .await
            .expect("hello response");
    }

    #[tokio::test]
    async fn hello_keeps_only_offered_capabilities_and_exposes_ready_versions() {
        let (stream, server) = pair().await;
        let mut client = client(stream);
        let task = tokio::spawn(hello_daemon(
            server,
            json!({
                "rpcVersion": RPC_VERSION,
                "daemonVersion": "4.0.0-alpha.0",
                "capabilities": [CAPABILITY_OPERATIONS_V1, "future.capability.v1"],
                "dataSchema": 1,
                "configSchema": 1,
                "extensionApi": {"major":1,"world":"alcomd:extension/extension-v1@1.0.0"},
                "futureOptional": true
            }),
        ));
        client.hello("sdk-test", None).await.expect("valid hello");
        assert!(client.is_usable());
        assert!(client.supports_capability(CAPABILITY_OPERATIONS_V1));
        assert!(!client.supports_capability("future.capability.v1"));
        assert_eq!(client.hello_result().data_schema, 1);
        assert_eq!(client.hello_result().config_schema, 1);
        task.await.expect("server task");
    }

    #[tokio::test]
    async fn incomplete_handshake_prevents_business_calls() {
        let mut reply = serde_json::to_value(alcomd_protocol::HelloResult::ready(Vec::new(), 1))
            .expect("valid handshake fixture");
        reply
            .as_object_mut()
            .expect("handshake fields")
            .remove("extensionApi");
        let (stream, server) = pair().await;
        let mut client = client(stream);
        let task = tokio::spawn(hello_daemon(server, reply));
        assert!(matches!(
            client.hello("sdk-test", None).await,
            Err(ClientError::InvalidResponse)
        ));
        assert!(!client.is_usable());
        task.await.expect("server task");
    }

    #[tokio::test]
    async fn hello_rejects_wrong_major_and_malformed_capabilities() {
        for (field, invalid) in [
            ("rpcVersion", json!(RPC_VERSION + 1)),
            ("capabilities", json!(["invalid name"])),
            ("capabilities", json!(["future.v1", "future.v1"])),
        ] {
            let mut result =
                serde_json::to_value(alcomd_protocol::HelloResult::ready(Vec::new(), 1))
                    .expect("valid handshake fixture");
            result[field] = invalid;
            let (stream, server) = pair().await;
            let mut client = client(stream);
            let task = tokio::spawn(hello_daemon(server, result));
            assert!(matches!(
                client.hello("sdk-test", None).await,
                Err(ClientError::InvalidResponse)
            ));
            assert!(!client.is_usable());
            task.await.expect("server task");
        }
    }

    #[tokio::test]
    async fn an_invalid_or_truncated_response_makes_the_connection_unusable() {
        let malformed = [
            encode_frame(br#"{"id":"2","result":{}}"#).expect("wrong ID success"),
            encode_frame(br#"{"id":"2","error":{"code":"denied","message":"denied"}}"#)
                .expect("wrong ID error"),
            encode_frame(br#"{"id":"1","result":{},"error":null}"#).expect("ambiguous response"),
            0u32.to_le_bytes().to_vec(),
            (u32::try_from(alcomd_protocol::MAX_FRAME_PAYLOAD_BYTES).expect("bounded") + 1)
                .to_le_bytes()
                .to_vec(),
        ];
        let truncated = [vec![20, 0], vec![20, 0, 0, 0, b'{']];
        for (frame, is_truncated) in malformed
            .into_iter()
            .map(|frame| (frame, false))
            .chain(truncated.into_iter().map(|frame| (frame, true)))
        {
            let (stream, mut server) = pair().await;
            let mut client = client(stream);
            let task = tokio::spawn(async move {
                let _ = read_frame(&mut server).await.expect("request frame");
                server.write_all(&frame).await.expect("response bytes");
            });
            let result = client
                .call::<_, serde_json::Value>(METHOD_SYSTEM_STATUS, json!({}))
                .await;
            if is_truncated {
                assert!(
                    matches!(result, Err(ClientError::Transport(error)) if error.kind() == io::ErrorKind::UnexpectedEof)
                );
            } else {
                assert!(matches!(result, Err(ClientError::InvalidResponse)));
            }
            assert!(!client.is_usable());
            task.await.expect("server task");
        }
    }

    #[tokio::test]
    async fn completed_success_and_remote_error_keep_the_connection_reusable() {
        let (stream, mut server) = pair().await;
        let mut client = client(stream);
        let task = tokio::spawn(async move {
            for index in 0..3 {
                let request: serde_json::Value =
                    serde_json::from_slice(&read_frame(&mut server).await.expect("frame"))
                        .expect("request");
                let response = if index == 1 {
                    json!({"id":request["id"],"error":{"code":"permission_denied","message":"denied"}})
                } else {
                    json!({"id":request["id"],"result":{"index":index}})
                };
                server
                    .write_all(
                        &encode_frame(&serde_json::to_vec(&response).expect("JSON"))
                            .expect("encode"),
                    )
                    .await
                    .expect("response");
            }
        });
        for index in 0..3 {
            let result: Result<serde_json::Value, _> = client.call("reuse-test", json!({})).await;
            if index == 1 {
                assert!(matches!(result, Err(ClientError::Remote(_))));
            } else {
                assert_eq!(result.expect("success")["index"], index);
            }
            assert!(client.is_usable());
        }
        task.await.expect("server task");
    }

    #[tokio::test]
    async fn abandoning_an_in_flight_frame_marks_the_connection_unusable() {
        let (stream, mut server) = pair().await;
        let mut client = client(stream);
        let mut request = Box::pin(client.call::<_, serde_json::Value>("blocked-test", json!({})));
        tokio::time::timeout(Duration::from_secs(2), async {
            tokio::select! {
                result = &mut request => panic!("no response was sent: {result:?}"),
                frame = read_frame(&mut server) => { assert!(frame.is_ok()); }
            }
        })
        .await
        .expect("request reaches private server");
        drop(request);
        assert!(!client.is_usable());
        assert!(matches!(
            client
                .call::<_, serde_json::Value>("next-test", json!({}))
                .await,
            Err(ClientError::InvalidResponse)
        ));
    }
}
