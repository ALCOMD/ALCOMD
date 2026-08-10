use super::dispatch::invoke_gui as dispatch_gui;
use super::tasks::McpTaskManager;
use super::types::{ClientIdentity, ListRepositoriesOutput};
use super::{MCP_OPERATION_CANCEL_METHOD, MCP_OPERATION_GET_METHOD, MCP_OPERATION_START_METHOD};
use anyhow::Result;
use rmcp::{
    ErrorData as McpError, Json, Peer, RoleServer, ServerHandler,
    handler::server::{router::tool::ToolRouter, tool::ToolCallContext, wrapper::Parameters},
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, CancelTaskParams,
        CreateTaskResult, GetTaskParams, GetTaskResult, Implementation, JsonObject,
        ProgressNotificationParam, ProgressToken, ProtocolVersion, ServerCapabilities, ServerInfo,
        UpdateTaskParams,
    },
    schemars,
    service::RequestContext,
    task_manager::{TaskContext, TaskExit, TaskOptions},
    tool, tool_handler, tool_router,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::borrow::Cow;
use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::AppHandle;
use uuid::Uuid;

const TOOL_INVOCATION_MAX_CONCURRENT: usize = 64;
const TOOL_INVOCATION_MAX_STARTED_PER_WINDOW: usize = 600;
const TOOL_INVOCATION_RATE_WINDOW: Duration = Duration::from_secs(60);
const PROJECT_TASK_DEFAULT_POLL_INTERVAL_MS: u64 = 500;
const PROJECT_TASK_MIN_POLL_INTERVAL_MS: u64 = 100;
const SUPPORTED_PROTOCOL_VERSIONS: &[ProtocolVersion] =
    &[ProtocolVersion::V_2026_07_28, ProtocolVersion::V_2025_11_25];

type McpJsonResult = std::result::Result<Json<JsonObject>, Json<JsonObject>>;
type ListRepositoriesMcpResult =
    std::result::Result<Json<ListRepositoriesOutput>, Json<JsonObject>>;

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
struct ProjectDetailsArgs {
    project_path: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct TemplateIdArgs {
    template_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct CreateTemplateArgs {
    display_name: String,
    base_template_id: String,
    unity_version_range: String,
    vpm_dependencies: BTreeMap<String, String>,
    unitypackage_paths: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct EditTemplateArgs {
    template_id: String,
    display_name: String,
    base_template_id: String,
    unity_version_range: String,
    vpm_dependencies: BTreeMap<String, String>,
    unitypackage_paths: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct SetTemplatePackageArgs {
    template_id: String,
    package_name: String,
    version_range: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct RemoveTemplatePackageArgs {
    template_id: String,
    package_name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct TemplateUnityPackageArgs {
    template_id: String,
    unitypackage_path: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
struct BackupProjectArgs {
    project_path: String,
    backup_name: Option<String>,
    #[serde(default)]
    exclude_vpm_packages: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
struct CopyProjectArgs {
    source_project_path: String,
    new_project_path: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
struct RestoreProjectFromBackupArgs {
    backup_path: String,
    project_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
struct CreateProjectArgs {
    project_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    base_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    template_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    unity_version: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
struct AddExistingProjectArgs {
    project_path: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
struct InstallProjectPackageArgs {
    project_path: String,
    package_name: String,
    version_selector: ProjectPackageVersionSelectorArg,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<ProjectPackageSourceArg>,
    #[serde(skip_serializing_if = "Option::is_none")]
    allow_conflicts: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
struct ProjectPackageArgs {
    project_path: String,
    package_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    allow_conflicts: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ProjectPackageVersionSelectorArg {
    LatestGuiVisible,
    Exact { version: String },
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct ProjectPackageSourceArg {
    repository_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectTaskSnapshot {
    task_id: String,
    #[allow(dead_code)]
    kind: ProjectTaskKind,
    status: ProjectTaskStatus,
    status_message: Option<String>,
    poll_interval: Option<u64>,
    progress: Option<ProjectTaskProgress>,
    result: Option<Value>,
    error: Option<ProjectTaskError>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ProjectTaskKind {
    Create,
    Backup,
    Copy,
    Restore,
    InstallPackage,
    UninstallPackage,
    ReinstallPackage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ProjectTaskStatus {
    Working,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectTaskProgress {
    total: usize,
    proceed: usize,
    last_proceed: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectTaskError {
    code: String,
    message: String,
    data: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
struct PackageListArgs {
    offset: Option<usize>,
    limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct RepositoryPackagesArgs {
    repository_id: String,
    offset: Option<usize>,
    limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct AddRepositoryArgs {
    repository_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    headers: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct RemoveRepositoryArgs {
    repository_url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct PackageDetailsArgs {
    package_name: String,
    version: Option<String>,
    repository_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
enum ActivityLogSourceArg {
    Gui,
    Mcp,
    DeepLink,
    System,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
enum ActivityLogKindArg {
    Read,
    Write,
    Passive,
    Open,
    Maintenance,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
enum ActivityLogStatusArg {
    Started,
    Succeeded,
    Failed,
    Cancelled,
    Info,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
enum ActivityLogVisibilityArg {
    Important,
    Primary,
    Secondary,
    Technical,
    All,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
enum ActivityLogOrderArg {
    Newest,
    Oldest,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
enum ActivityLogGroupByArg {
    Source,
    Kind,
    Status,
    Operation,
    ToolName,
    ClientName,
    Day,
    Hour,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
struct ActivityLogSearchArgs {
    search: Option<String>,
    sources: Option<Vec<ActivityLogSourceArg>>,
    kinds: Option<Vec<ActivityLogKindArg>>,
    statuses: Option<Vec<ActivityLogStatusArg>>,
    visibility: Option<ActivityLogVisibilityArg>,
    operations: Option<Vec<String>>,
    tool_names: Option<Vec<String>>,
    request_id: Option<String>,
    target: Option<String>,
    since: Option<String>,
    until: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
    order: Option<ActivityLogOrderArg>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
struct ActivityLogEntryArgs {
    id: String,
    include_details: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
struct ActivityLogSummaryArgs {
    search: Option<String>,
    sources: Option<Vec<ActivityLogSourceArg>>,
    kinds: Option<Vec<ActivityLogKindArg>>,
    statuses: Option<Vec<ActivityLogStatusArg>>,
    visibility: Option<ActivityLogVisibilityArg>,
    operations: Option<Vec<String>>,
    tool_names: Option<Vec<String>>,
    request_id: Option<String>,
    target: Option<String>,
    since: Option<String>,
    until: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
    order: Option<ActivityLogOrderArg>,
    group_by: Option<ActivityLogGroupByArg>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
struct ActivityLogContextArgs {
    id: String,
    before: Option<usize>,
    after: Option<usize>,
    include_details: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
enum TechnicalLogLevelArg {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
enum TechnicalLogScopeArg {
    Memory,
    RecentFiles,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
enum TechnicalLogGroupByArg {
    Level,
    Target,
    File,
    Hour,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
struct TechnicalLogSearchArgs {
    search: Option<String>,
    levels: Option<Vec<TechnicalLogLevelArg>>,
    targets: Option<Vec<String>>,
    scope: Option<TechnicalLogScopeArg>,
    since: Option<String>,
    until: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
    max_message_chars: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
struct TechnicalLogEntryArgs {
    id: String,
    max_message_chars: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
struct TechnicalLogSummaryArgs {
    search: Option<String>,
    levels: Option<Vec<TechnicalLogLevelArg>>,
    targets: Option<Vec<String>>,
    scope: Option<TechnicalLogScopeArg>,
    since: Option<String>,
    until: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
    max_message_chars: Option<usize>,
    group_by: Option<TechnicalLogGroupByArg>,
}

#[derive(Debug)]
enum InvokeOutcome {
    Success(Value),
    ToolError(Value),
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ToolInvocationLimits {
    max_concurrent: usize,
    max_started_per_window: usize,
    window: Duration,
}

impl ToolInvocationLimits {
    pub(super) fn production() -> Self {
        Self {
            max_concurrent: TOOL_INVOCATION_MAX_CONCURRENT,
            max_started_per_window: TOOL_INVOCATION_MAX_STARTED_PER_WINDOW,
            window: TOOL_INVOCATION_RATE_WINDOW,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolRateLimitReason {
    TooManyConcurrent,
    TooManyStartedInWindow,
}

impl ToolRateLimitReason {
    fn message(self) -> &'static str {
        match self {
            ToolRateLimitReason::TooManyConcurrent => {
                "Too many ALCOMD3 MCP tool calls are already running"
            }
            ToolRateLimitReason::TooManyStartedInWindow => {
                "ALCOMD3 MCP tool call rate limit exceeded"
            }
        }
    }
}

#[derive(Clone)]
pub(super) struct ToolInvocationLimiter {
    inner: Arc<ToolInvocationLimiterInner>,
}

struct ToolInvocationLimiterInner {
    limits: ToolInvocationLimits,
    state: Mutex<ToolInvocationLimiterState>,
}

#[derive(Default)]
struct ToolInvocationLimiterState {
    active: usize,
    started_at: VecDeque<Instant>,
}

struct ToolInvocationPermit {
    inner: Arc<ToolInvocationLimiterInner>,
}

impl ToolInvocationLimiter {
    pub(super) fn new(limits: ToolInvocationLimits) -> Self {
        assert!(limits.max_concurrent > 0);
        assert!(limits.max_started_per_window > 0);
        assert!(!limits.window.is_zero());

        Self {
            inner: Arc::new(ToolInvocationLimiterInner {
                limits,
                state: Mutex::new(ToolInvocationLimiterState::default()),
            }),
        }
    }

    fn try_start(
        &self,
        now: Instant,
    ) -> std::result::Result<ToolInvocationPermit, ToolRateLimitReason> {
        let mut state = self.inner.state.lock().unwrap();
        prune_started_at(&mut state.started_at, now, self.inner.limits.window);

        if state.active >= self.inner.limits.max_concurrent {
            return Err(ToolRateLimitReason::TooManyConcurrent);
        }
        if state.started_at.len() >= self.inner.limits.max_started_per_window {
            return Err(ToolRateLimitReason::TooManyStartedInWindow);
        }

        state.active += 1;
        state.started_at.push_back(now);

        Ok(ToolInvocationPermit {
            inner: Arc::clone(&self.inner),
        })
    }
}

impl Drop for ToolInvocationPermit {
    fn drop(&mut self) {
        let mut state = self.inner.state.lock().unwrap();
        state.active = state.active.saturating_sub(1);
    }
}

fn prune_started_at(started_at: &mut VecDeque<Instant>, now: Instant, window: Duration) {
    while started_at
        .front()
        .and_then(|started| now.checked_duration_since(*started))
        .is_some_and(|elapsed| elapsed >= window)
    {
        started_at.pop_front();
    }
}

#[derive(Clone)]
pub(super) struct Alcomd3Mcp {
    app: AppHandle,
    client: Arc<Mutex<ClientIdentity>>,
    limiter: ToolInvocationLimiter,
    tasks: McpTaskManager,
    tool_router: ToolRouter<Alcomd3Mcp>,
}

impl Alcomd3Mcp {
    pub(super) fn new(
        app: AppHandle,
        limiter: ToolInvocationLimiter,
        tasks: McpTaskManager,
    ) -> Self {
        Self {
            app,
            client: Arc::new(Mutex::new(ClientIdentity {
                name: "MCP client".to_string(),
                version: None,
            })),
            limiter,
            tasks,
            tool_router: Self::tool_router(),
        }
    }

    fn update_client(&self, peer: &Peer<RoleServer>) {
        let Some(info) = peer.peer_info() else {
            return;
        };
        let mut client = self.client.lock().unwrap();
        client.name = info.client_info.name.clone();
        client.version = Some(info.client_info.version.clone());
    }

    fn client_for_peer(&self, peer: &Peer<RoleServer>) -> ClientIdentity {
        self.update_client(peer);
        self.client.lock().unwrap().clone()
    }

    async fn invoke<T: Serialize>(
        &self,
        method: &str,
        params: T,
        peer: Peer<RoleServer>,
    ) -> McpJsonResult {
        let _permit = match self.limiter.try_start(Instant::now()) {
            Ok(permit) => permit,
            Err(reason) => return format_rate_limit_result(reason),
        };
        self.update_client(&peer);
        let client = self.client.lock().unwrap().clone();
        let mut params = serde_json::to_value(params).unwrap_or(Value::Null);
        remove_null_object_fields(&mut params);
        format_invoke_result(invoke_gui(&self.app, method, params, &client).await)
    }

    async fn invoke_typed<TParams, TOutput>(
        &self,
        method: &str,
        params: TParams,
        peer: Peer<RoleServer>,
    ) -> std::result::Result<Json<TOutput>, Json<JsonObject>>
    where
        TParams: Serialize,
        TOutput: DeserializeOwned,
    {
        let _permit = match self.limiter.try_start(Instant::now()) {
            Ok(permit) => permit,
            Err(reason) => return Err(format_rate_limit_error(reason)),
        };
        self.update_client(&peer);
        let client = self.client.lock().unwrap().clone();
        let mut params = serde_json::to_value(params).unwrap_or(Value::Null);
        remove_null_object_fields(&mut params);
        format_typed_invoke_result(invoke_gui(&self.app, method, params, &client).await)
    }

    async fn invoke_project_tool_sync<T: Serialize>(
        &self,
        method: &str,
        params: T,
        context: RequestContext<RoleServer>,
    ) -> McpJsonResult {
        let _permit = match self.limiter.try_start(Instant::now()) {
            Ok(permit) => permit,
            Err(reason) => return format_rate_limit_result(reason),
        };

        let client = self.client_for_peer(&context.peer);
        let task_id = Uuid::new_v4().to_string();
        let params = serde_json::to_value(params).unwrap_or(Value::Null);
        let mut snapshot = match invoke_gui_value(
            &self.app,
            MCP_OPERATION_START_METHOD,
            json!({
                "taskId": task_id,
                "method": method,
                "params": params,
            }),
            &client,
        )
        .await
        .and_then(project_task_snapshot_from_value)
        {
            Ok(snapshot) => snapshot,
            Err(error) => return format_mcp_error_result(error),
        };

        let progress_token = context.meta.get_progress_token();
        let mut last_progress = -1.0;
        notify_project_progress_if_needed(
            &context.peer,
            &snapshot,
            progress_token.clone(),
            &mut last_progress,
        )
        .await;

        loop {
            match snapshot.status {
                ProjectTaskStatus::Working => {
                    tokio::select! {
                        _ = context.ct.cancelled() => {
                            let cancelled = invoke_gui_value(
                                &self.app,
                                MCP_OPERATION_CANCEL_METHOD,
                                json!({ "taskId": snapshot.task_id }),
                                &client,
                            )
                            .await
                            .and_then(project_task_snapshot_from_value);
                            return match cancelled {
                                Ok(snapshot) => project_task_snapshot_to_tool_result(snapshot),
                                Err(error) => format_mcp_error_result(error),
                            };
                        }
                        _ = tokio::time::sleep(project_task_poll_interval(&snapshot)) => {}
                    }

                    snapshot = match invoke_gui_value(
                        &self.app,
                        MCP_OPERATION_GET_METHOD,
                        json!({ "taskId": snapshot.task_id }),
                        &client,
                    )
                    .await
                    .and_then(project_task_snapshot_from_value)
                    {
                        Ok(snapshot) => snapshot,
                        Err(error) => return format_mcp_error_result(error),
                    };
                    notify_project_progress_if_needed(
                        &context.peer,
                        &snapshot,
                        progress_token.clone(),
                        &mut last_progress,
                    )
                    .await;
                }
                ProjectTaskStatus::Completed
                | ProjectTaskStatus::Failed
                | ProjectTaskStatus::Cancelled => {
                    return project_task_snapshot_to_tool_result(snapshot);
                }
            }
        }
    }
}

fn remove_null_object_fields(value: &mut Value) {
    match value {
        Value::Object(object) => {
            object.retain(|_, value| {
                remove_null_object_fields(value);
                !value.is_null()
            });
        }
        Value::Array(values) => {
            for value in values {
                remove_null_object_fields(value);
            }
        }
        _ => {}
    }
}

fn format_rate_limit_result(reason: ToolRateLimitReason) -> McpJsonResult {
    Err(format_rate_limit_error(reason))
}

fn format_rate_limit_error(reason: ToolRateLimitReason) -> Json<JsonObject> {
    Json(value_as_object(json!({
        "ok": false,
        "error": {
            "code": "rate_limited",
            "message": reason.message(),
        }
    })))
}

fn rate_limit_mcp_error(reason: ToolRateLimitReason) -> McpError {
    McpError::invalid_request(
        reason.message(),
        Some(json!({
            "code": "rate_limited",
        })),
    )
}

fn format_mcp_error_result(error: McpError) -> McpJsonResult {
    let data = error.data;
    if let Some(value) = data.as_ref()
        && value.pointer("/ok") == Some(&Value::Bool(false))
        && value.pointer("/error").is_some()
    {
        return Err(Json(value_as_object(value.clone())));
    }

    Err(Json(value_as_object(json!({
        "ok": false,
        "error": {
            "code": "mcp_task_error",
            "message": error.message,
            "data": data,
        }
    }))))
}

fn project_task_snapshot_to_tool_result(snapshot: ProjectTaskSnapshot) -> McpJsonResult {
    match snapshot.status {
        ProjectTaskStatus::Completed => {
            let result = snapshot.result.unwrap_or_else(|| json!({ "ok": true }));
            Ok(Json(value_as_object(result)))
        }
        ProjectTaskStatus::Failed | ProjectTaskStatus::Cancelled => {
            let error = snapshot.error.unwrap_or(ProjectTaskError {
                code: "project_task_error".to_string(),
                message: "MCP project task did not complete successfully".to_string(),
                data: None,
            });
            Err(Json(value_as_object(json!({
                "ok": false,
                "error": error,
            }))))
        }
        ProjectTaskStatus::Working => Err(Json(value_as_object(json!({
            "ok": false,
            "error": {
                "code": "project_task_incomplete",
                "message": "MCP project task is still running",
            }
        })))),
    }
}

fn project_tool_method(tool_name: &str) -> std::result::Result<&'static str, McpError> {
    match tool_name {
        "alcomd3_create_project" => Ok("create_project"),
        "alcomd3_backup_project" => Ok("backup_project"),
        "alcomd3_copy_project" => Ok("copy_project"),
        "alcomd3_restore_project_from_backup" => Ok("restore_project_from_backup"),
        "alcomd3_install_project_package" => Ok("install_project_package"),
        "alcomd3_uninstall_project_package" => Ok("uninstall_project_package"),
        "alcomd3_reinstall_project_package" => Ok("reinstall_project_package"),
        _ => Err(McpError::invalid_params(
            format!("tool does not support task-based invocation: {tool_name}"),
            None,
        )),
    }
}

fn project_task_snapshot_from_value(
    value: Value,
) -> std::result::Result<ProjectTaskSnapshot, McpError> {
    serde_json::from_value(value)
        .map_err(|e| McpError::internal_error(format!("invalid task response: {e}"), None))
}

fn project_task_poll_interval(snapshot: &ProjectTaskSnapshot) -> Duration {
    Duration::from_millis(
        snapshot
            .poll_interval
            .unwrap_or(PROJECT_TASK_DEFAULT_POLL_INTERVAL_MS)
            .max(PROJECT_TASK_MIN_POLL_INTERVAL_MS),
    )
}

async fn invoke_gui_value(
    app: &AppHandle,
    method: &str,
    params: Value,
    client: &ClientIdentity,
) -> std::result::Result<Value, McpError> {
    match invoke_gui(app, method, params, client).await {
        Ok(InvokeOutcome::Success(value)) => Ok(value),
        Ok(InvokeOutcome::ToolError(value)) => Err(mcp_error_from_tool_error(value)),
        Err(error) => Err(McpError::internal_error(
            format!("ALCOMD3 MCP dispatch failed: {error}"),
            None,
        )),
    }
}

fn mcp_error_from_tool_error(value: Value) -> McpError {
    let message = value
        .pointer("/error/message")
        .and_then(Value::as_str)
        .unwrap_or("ALCOMD3 returned a tool error")
        .to_string();
    let code = value.pointer("/error/code").and_then(Value::as_str);
    match code {
        Some("invalid_params" | "project_task_not_found" | "project_task_already_finished") => {
            McpError::invalid_params(message, Some(value))
        }
        Some("unknown_method" | "unsupported_project_task_method") => {
            McpError::invalid_request(message, Some(value))
        }
        _ => McpError::invalid_request(message, Some(value)),
    }
}

async fn run_project_task(
    app: AppHandle,
    method: String,
    params: Value,
    client: ClientIdentity,
    context: TaskContext,
) -> std::result::Result<CallToolResult, TaskExit> {
    let task_id = context.task_id().to_string();
    let mut snapshot = invoke_gui_value(
        &app,
        MCP_OPERATION_START_METHOD,
        json!({
            "taskId": task_id,
            "method": method,
            "params": params,
        }),
        &client,
    )
    .await
    .and_then(project_task_snapshot_from_value)
    .map_err(TaskExit::Error)?;

    loop {
        if let Some(message) = snapshot.status_message.as_deref() {
            context.set_status_message(message);
        }

        match snapshot.status {
            ProjectTaskStatus::Working => {
                tokio::select! {
                    _ = context.cancelled() => {
                        let _ = invoke_gui_value(
                            &app,
                            MCP_OPERATION_CANCEL_METHOD,
                            json!({ "taskId": snapshot.task_id }),
                            &client,
                        )
                        .await;
                        return Err(TaskExit::Cancelled);
                    }
                    _ = tokio::time::sleep(project_task_poll_interval(&snapshot)) => {}
                }

                snapshot = invoke_gui_value(
                    &app,
                    MCP_OPERATION_GET_METHOD,
                    json!({ "taskId": snapshot.task_id }),
                    &client,
                )
                .await
                .and_then(project_task_snapshot_from_value)
                .map_err(TaskExit::Error)?;
            }
            ProjectTaskStatus::Completed => {
                let result = snapshot.result.unwrap_or_else(|| json!({ "ok": true }));
                return Ok(CallToolResult::structured(result));
            }
            ProjectTaskStatus::Cancelled => return Err(TaskExit::Cancelled),
            ProjectTaskStatus::Failed => {
                let error = snapshot.error.unwrap_or(ProjectTaskError {
                    code: "project_task_error".to_string(),
                    message: "MCP project task did not complete successfully".to_string(),
                    data: None,
                });
                return Err(TaskExit::Error(mcp_error_from_tool_error(json!({
                    "ok": false,
                    "error": error,
                }))));
            }
        }
    }
}

async fn notify_project_progress_if_needed(
    peer: &Peer<RoleServer>,
    snapshot: &ProjectTaskSnapshot,
    progress_token: Option<ProgressToken>,
    last_progress: &mut f64,
) {
    let Some(progress_token) = progress_token else {
        return;
    };
    let Some(notification) = project_progress_notification(snapshot, progress_token, last_progress)
    else {
        return;
    };

    let _ = peer.notify_progress(notification).await;
}

fn project_progress_notification(
    snapshot: &ProjectTaskSnapshot,
    progress_token: ProgressToken,
    last_progress: &mut f64,
) -> Option<ProgressNotificationParam> {
    let progress = snapshot.progress.as_ref()?;
    let current = progress.proceed as f64;
    if current <= *last_progress {
        return None;
    }

    *last_progress = current;
    let mut notification = ProgressNotificationParam::new(progress_token, current);
    if progress.total > 0 {
        notification = notification.with_total(progress.total as f64);
    }
    if let Some(message) = &snapshot.status_message {
        notification = notification.with_message(message.clone());
    }

    Some(notification)
}

#[tool_router]
impl Alcomd3Mcp {
    #[tool(
        description = "List Unity projects registered in ALCOMD3",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn alcomd3_list_projects(&self, peer: Peer<RoleServer>) -> McpJsonResult {
        self.invoke("list_projects", json!({}), peer).await
    }

    #[tool(
        description = "List the environment-level templates currently available in ALCOMD3. These templates are choices for alcomd3_create_project, not data owned by a registered project. Use a returned template ID and Unity version with alcomd3_create_project.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn alcomd3_list_templates(&self, peer: Peer<RoleServer>) -> McpJsonResult {
        self.invoke("list_templates", json!({}), peer).await
    }

    #[tool(
        description = "Read one environment-level ALCOMD3 template selected by template_id. This does not read template information from a registered project. Derived templates include their editable definition and UnityPackage paths; template storage paths are not returned.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn alcomd3_get_template(
        &self,
        Parameters(args): Parameters<TemplateIdArgs>,
        peer: Peer<RoleServer>,
    ) -> McpJsonResult {
        self.invoke("get_template", args, peer).await
    }

    #[tool(
        description = "Create a derived environment-level ALCOMD3 template. The base template ID must come from alcomd3_list_templates with usableAsBase=true. Every unitypackage_paths entry must be an existing absolute .unitypackage file.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn alcomd3_create_template(
        &self,
        Parameters(args): Parameters<CreateTemplateArgs>,
        peer: Peer<RoleServer>,
    ) -> McpJsonResult {
        self.invoke("create_template", args, peer).await
    }

    #[tool(
        description = "Replace the editable definition of one derived environment-level ALCOMD3 template. The template ID and storage location remain unchanged. Built-in and project-archive templates cannot be edited.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn alcomd3_edit_template(
        &self,
        Parameters(args): Parameters<EditTemplateArgs>,
        peer: Peer<RoleServer>,
    ) -> McpJsonResult {
        self.invoke("edit_template", args, peer).await
    }

    #[tool(
        description = "Set one direct VPM package dependency on a derived ALCOMD3 template. This stores package_name and version_range in the template definition; it does not install or resolve the package. Repeating the same value is a no-op.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn alcomd3_set_template_package(
        &self,
        Parameters(args): Parameters<SetTemplatePackageArgs>,
        peer: Peer<RoleServer>,
    ) -> McpJsonResult {
        self.invoke("set_template_package", args, peer).await
    }

    #[tool(
        description = "Remove one direct VPM package dependency from a derived ALCOMD3 template. This only edits the template definition and does not uninstall anything from existing projects.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn alcomd3_remove_template_package(
        &self,
        Parameters(args): Parameters<RemoveTemplatePackageArgs>,
        peer: Peer<RoleServer>,
    ) -> McpJsonResult {
        self.invoke("remove_template_package", args, peer).await
    }

    #[tool(
        description = "Set one UnityPackage attachment on a derived ALCOMD3 template. unitypackage_path must identify an existing absolute .unitypackage file. The file is referenced, not copied; repeating the same canonical path is a no-op.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn alcomd3_set_template_unitypackage(
        &self,
        Parameters(args): Parameters<TemplateUnityPackageArgs>,
        peer: Peer<RoleServer>,
    ) -> McpJsonResult {
        self.invoke("set_template_unitypackage", args, peer).await
    }

    #[tool(
        description = "Remove one UnityPackage attachment reference from a derived ALCOMD3 template. unitypackage_path should be copied from alcomd3_get_template. The referenced .unitypackage file is not deleted.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn alcomd3_remove_template_unitypackage(
        &self,
        Parameters(args): Parameters<TemplateUnityPackageArgs>,
        peer: Peer<RoleServer>,
    ) -> McpJsonResult {
        self.invoke("remove_template_unitypackage", args, peer)
            .await
    }

    #[tool(
        description = "Remove one user-defined environment-level ALCOMD3 template by template_id. Built-in templates cannot be removed. Referenced UnityPackage files are not deleted.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn alcomd3_remove_template(
        &self,
        Parameters(args): Parameters<TemplateIdArgs>,
        peer: Peer<RoleServer>,
    ) -> McpJsonResult {
        self.invoke("remove_template", args, peer).await
    }

    #[tool(
        description = "Get details for a project registered in ALCOMD3. project_path must match an ALCOMD3 registered project path.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn alcomd3_get_project_details(
        &self,
        Parameters(_args): Parameters<ProjectDetailsArgs>,
        peer: Peer<RoleServer>,
    ) -> McpJsonResult {
        self.invoke("get_project_details", _args, peer).await
    }

    #[tool(
        description = "List VPM repositories available in ALCOMD3, including default and user repositories. Use the returned id to select a repository in package-reading tools, or the returned url to remove a user-added repository.",
        output_schema = rmcp::handler::server::tool::schema_for_type::<ListRepositoriesOutput>(),
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn alcomd3_list_repositories(&self, peer: Peer<RoleServer>) -> ListRepositoriesMcpResult {
        self.invoke_typed("list_repositories", json!({}), peer)
            .await
    }

    #[tool(
        description = "Add a VPM repository URL to ALCOMD3 and refresh package cache visibility. repository_url must be a valid repository URL. headers can provide optional HTTP headers for the repository request. The returned repository.url is the handle for later removal; duplicate repository URLs or declared IDs are rejected.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn alcomd3_add_repository(
        &self,
        Parameters(args): Parameters<AddRepositoryArgs>,
        peer: Peer<RoleServer>,
    ) -> McpJsonResult {
        self.invoke("add_repository", args, peer).await
    }

    #[tool(
        description = "Remove one user-added VPM repository from ALCOMD3. repository_url is required and should be copied from alcomd3_list_repositories. Default repositories cannot be removed.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn alcomd3_remove_repository(
        &self,
        Parameters(args): Parameters<RemoveRepositoryArgs>,
        peer: Peer<RoleServer>,
    ) -> McpJsonResult {
        self.invoke("remove_repository", args, peer).await
    }

    #[tool(
        description = "Get detailed package metadata for GUI-visible ALCOMD3 packages selected by package_name, optional version, and optional repository_id.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn alcomd3_get_package_details(
        &self,
        Parameters(args): Parameters<PackageDetailsArgs>,
        peer: Peer<RoleServer>,
    ) -> McpJsonResult {
        self.invoke("get_package_details", args, peer).await
    }

    #[tool(
        description = "List lightweight package summaries visible in the ALCOMD3 GUI package list. Use alcomd3_get_package_details for dependencies, description, keywords, and URLs.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn alcomd3_list_packages(
        &self,
        Parameters(args): Parameters<PackageListArgs>,
        peer: Peer<RoleServer>,
    ) -> McpJsonResult {
        self.invoke("list_packages", args, peer).await
    }

    #[tool(
        description = "List lightweight package summaries from one ALCOMD3 remote repository selected by repository_id. Use alcomd3_list_repositories to discover repository IDs.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn alcomd3_list_repository_packages(
        &self,
        Parameters(args): Parameters<RepositoryPackagesArgs>,
        peer: Peer<RoleServer>,
    ) -> McpJsonResult {
        self.invoke("list_repository_packages", args, peer).await
    }

    #[tool(
        description = "Get ALCOMD3 environment settings including registered Unity installations, default Unity launch arguments, and default project and backup paths.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn alcomd3_get_environment_settings(&self, peer: Peer<RoleServer>) -> McpJsonResult {
        self.invoke("get_environment_settings", json!({}), peer)
            .await
    }

    #[tool(
        description = "Search ALCOMD3 user-readable activity logs with bounded filters and pagination. Use this before alcomd3_get_activity_log_entry; do not raise limit to pull all logs.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn alcomd3_search_activity_logs(
        &self,
        Parameters(args): Parameters<ActivityLogSearchArgs>,
        peer: Peer<RoleServer>,
    ) -> McpJsonResult {
        self.invoke("search_activity_logs", args, peer).await
    }

    #[tool(
        description = "Get one ALCOMD3 activity log entry by id. Obtain ids from alcomd3_search_activity_logs or alcomd3_summarize_activity_logs.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn alcomd3_get_activity_log_entry(
        &self,
        Parameters(args): Parameters<ActivityLogEntryArgs>,
        peer: Peer<RoleServer>,
    ) -> McpJsonResult {
        self.invoke("get_activity_log_entry", args, peer).await
    }

    #[tool(
        description = "Summarize ALCOMD3 activity logs by source, kind, status, operation, tool name, client name, day, or hour. Use this to decide which activity log details to inspect.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn alcomd3_summarize_activity_logs(
        &self,
        Parameters(args): Parameters<ActivityLogSummaryArgs>,
        peer: Peer<RoleServer>,
    ) -> McpJsonResult {
        self.invoke("summarize_activity_logs", args, peer).await
    }

    #[tool(
        description = "Get nearby ALCOMD3 activity log entries around one activity id to reconstruct an operation chain without reading all logs.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn alcomd3_get_activity_log_context(
        &self,
        Parameters(args): Parameters<ActivityLogContextArgs>,
        peer: Peer<RoleServer>,
    ) -> McpJsonResult {
        self.invoke("get_activity_log_context", args, peer).await
    }

    #[tool(
        description = "Search ALCOMD3 technical logs with bounded filters and previews. Defaults to Error/Warn memory logs; use alcomd3_get_technical_log_entry for a selected id.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn alcomd3_search_technical_logs(
        &self,
        Parameters(args): Parameters<TechnicalLogSearchArgs>,
        peer: Peer<RoleServer>,
    ) -> McpJsonResult {
        self.invoke("search_technical_logs", args, peer).await
    }

    #[tool(
        description = "Get one ALCOMD3 technical log entry by id with message redaction and truncation. Obtain ids from alcomd3_search_technical_logs.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn alcomd3_get_technical_log_entry(
        &self,
        Parameters(args): Parameters<TechnicalLogEntryArgs>,
        peer: Peer<RoleServer>,
    ) -> McpJsonResult {
        self.invoke("get_technical_log_entry", args, peer).await
    }

    #[tool(
        description = "Summarize ALCOMD3 technical logs by level, target, file, or hour. Use this before inspecting individual technical log entries.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn alcomd3_summarize_technical_logs(
        &self,
        Parameters(args): Parameters<TechnicalLogSummaryArgs>,
        peer: Peer<RoleServer>,
    ) -> McpJsonResult {
        self.invoke("summarize_technical_logs", args, peer).await
    }

    #[tool(
        description = "Create a new Unity project, register it in ALCOMD3, and resolve project packages. project_name is required. base_path defaults to the ALCOMD3 default project path. template_id and unity_version default to the current GUI template selection rules when omitted.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn alcomd3_create_project(
        &self,
        Parameters(args): Parameters<CreateProjectArgs>,
        context: RequestContext<RoleServer>,
    ) -> McpJsonResult {
        self.invoke_project_tool_sync("create_project", args, context)
            .await
    }

    #[tool(
        description = "Add an existing Unity project folder to ALCOMD3. project_path must be an absolute path to a valid Unity project directory.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn alcomd3_add_existing_project(
        &self,
        Parameters(args): Parameters<AddExistingProjectArgs>,
        peer: Peer<RoleServer>,
    ) -> McpJsonResult {
        self.invoke("add_existing_project", args, peer).await
    }

    #[tool(
        description = "Create a zip backup archive for a Unity project registered in ALCOMD3. project_path must match an ALCOMD3 registered project path. backup_name optionally overrides the generated archive name without the .zip extension. exclude_vpm_packages omits installed VPM package contents when true and defaults to false.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn alcomd3_backup_project(
        &self,
        Parameters(args): Parameters<BackupProjectArgs>,
        context: RequestContext<RoleServer>,
    ) -> McpJsonResult {
        self.invoke_project_tool_sync("backup_project", args, context)
            .await
    }

    #[tool(
        description = "Copy a Unity project registered in ALCOMD3 to a new project directory and register the copied project. source_project_path must match an ALCOMD3 registered project path, and new_project_path must not already exist.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn alcomd3_copy_project(
        &self,
        Parameters(args): Parameters<CopyProjectArgs>,
        context: RequestContext<RoleServer>,
    ) -> McpJsonResult {
        self.invoke_project_tool_sync("copy_project", args, context)
            .await
    }

    #[tool(
        description = "Restore a Unity project from an ALCOMD3 zip backup archive into the configured default project directory and register the restored project. project_name optionally overrides the restored folder name.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn alcomd3_restore_project_from_backup(
        &self,
        Parameters(args): Parameters<RestoreProjectFromBackupArgs>,
        context: RequestContext<RoleServer>,
    ) -> McpJsonResult {
        self.invoke_project_tool_sync("restore_project_from_backup", args, context)
            .await
    }

    #[tool(
        description = "Install one GUI-visible VPM package into a Unity project registered in ALCOMD3. project_path must match a registered project path. version_selector is required: use {\"type\":\"latest_gui_visible\"} to install the same latest compatible version the GUI exposes, or {\"type\":\"exact\",\"version\":\"x.y.z\"}. Optional source selects a remote repository by the repository_id returned from alcomd3_list_repositories. Conflicts or legacy file removals are blocked unless allow_conflicts is true.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn alcomd3_install_project_package(
        &self,
        Parameters(args): Parameters<InstallProjectPackageArgs>,
        context: RequestContext<RoleServer>,
    ) -> McpJsonResult {
        self.invoke_project_tool_sync("install_project_package", args, context)
            .await
    }

    #[tool(
        description = "Uninstall one installed package from a Unity project registered in ALCOMD3. project_path must match a registered project path. Conflicts or legacy file removals are blocked unless allow_conflicts is true.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn alcomd3_uninstall_project_package(
        &self,
        Parameters(args): Parameters<ProjectPackageArgs>,
        context: RequestContext<RoleServer>,
    ) -> McpJsonResult {
        self.invoke_project_tool_sync("uninstall_project_package", args, context)
            .await
    }

    #[tool(
        description = "Reinstall one installed package in a Unity project registered in ALCOMD3. project_path must match a registered project path. Conflicts or legacy file removals are blocked unless allow_conflicts is true.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn alcomd3_reinstall_project_package(
        &self,
        Parameters(args): Parameters<ProjectPackageArgs>,
        context: RequestContext<RoleServer>,
    ) -> McpJsonResult {
        self.invoke_project_tool_sync("reinstall_project_package", args, context)
            .await
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Alcomd3Mcp {
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResponse, McpError> {
        let client_supports_tasks = context
            .client_capabilities()
            .is_some_and(|capabilities| capabilities.supports_tasks());

        if client_supports_tasks && let Ok(method) = project_tool_method(&request.name) {
            let permit = self
                .limiter
                .try_start(Instant::now())
                .map_err(rate_limit_mcp_error)?;
            let client = self.client_for_peer(&context.peer);
            let app = self.app.clone();
            let method = method.to_string();
            let mut params = Value::Object(request.arguments.unwrap_or_default());
            remove_null_object_fields(&mut params);
            let task = self.tasks.spawn(
                TaskOptions::new()
                    .with_ttl_ms(super::MCP_PROJECT_TASK_TTL_MS)
                    .with_poll_interval_ms(PROJECT_TASK_DEFAULT_POLL_INTERVAL_MS)
                    .with_status_message(format!("Running {method}")),
                move |task_context| {
                    Box::pin(async move {
                        let _permit = permit;
                        run_project_task(app, method, params, client, task_context).await
                    })
                },
            );
            return Ok(CallToolResponse::Task(CreateTaskResult::new(task)));
        }

        let tool_context = ToolCallContext::new(self, request, context);
        self.tool_router.call(tool_context).await
    }

    fn get_info(&self) -> ServerInfo {
        server_info()
    }

    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Borrowed(SUPPORTED_PROTOCOL_VERSIONS)
    }

    async fn get_task(
        &self,
        request: GetTaskParams,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<GetTaskResult, McpError> {
        Ok(GetTaskResult::new(self.tasks.get_task(&request.task_id)?))
    }

    async fn update_task(
        &self,
        request: UpdateTaskParams,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<(), McpError> {
        self.tasks
            .update_task(&request.task_id, request.input_responses)
    }

    async fn cancel_task(
        &self,
        request: CancelTaskParams,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<(), McpError> {
        self.tasks.cancel_task(&request.task_id)
    }
}

fn server_info() -> ServerInfo {
    ServerInfo::new(
        ServerCapabilities::builder()
            .enable_tools()
            .enable_tasks()
            .build(),
    )
    .with_server_info(Implementation::new(
        "alcomd3-mcp",
        env!("CARGO_PKG_VERSION"),
    ))
    .with_instructions(
        "Use the MCP server built into ALCOMD3 GUI. Some tools create or add projects, add repositories, create project backups, copies, restores, or package changes. ALCOMD3 must remain running while tools are used.",
    )
}

async fn invoke_gui(
    app: &AppHandle,
    method: &str,
    params: Value,
    client: &ClientIdentity,
) -> Result<InvokeOutcome> {
    match dispatch_gui(app.clone(), method, params, client.clone()).await {
        Ok(value) => {
            let value = match value {
                Value::Object(mut object) => {
                    object.insert("ok".to_string(), Value::Bool(true));
                    Value::Object(object)
                }
                value => json!({
                    "ok": true,
                    "result": value,
                }),
            };
            Ok(InvokeOutcome::Success(value))
        }
        Err(error) => Ok(InvokeOutcome::ToolError(json!({
            "ok": false,
            "error": error,
        }))),
    }
}

fn format_invoke_result(result: Result<InvokeOutcome>) -> McpJsonResult {
    match result {
        Ok(InvokeOutcome::Success(value)) => Ok(Json(value_as_object(value))),
        Ok(InvokeOutcome::ToolError(value)) => Err(Json(value_as_object(value))),
        Err(error) => Err(Json(value_as_object(json!({
            "ok": false,
            "error": {
                "code": "mcp_internal_error",
                "message": format!("ALCOMD3 MCP dispatch failed: {error}"),
            }
        })))),
    }
}

fn format_typed_invoke_result<T: DeserializeOwned>(
    result: Result<InvokeOutcome>,
) -> std::result::Result<Json<T>, Json<JsonObject>> {
    match result {
        Ok(InvokeOutcome::Success(value)) => {
            serde_json::from_value(value).map(Json).map_err(|error| {
                Json(value_as_object(json!({
                    "ok": false,
                    "error": {
                        "code": "invalid_alcomd3_response",
                        "message": format!("ALCOMD3 returned an invalid tool response: {error}"),
                    }
                })))
            })
        }
        Ok(InvokeOutcome::ToolError(value)) => Err(Json(value_as_object(value))),
        Err(error) => Err(Json(value_as_object(json!({
            "ok": false,
            "error": {
                "code": "mcp_internal_error",
                "message": format!("ALCOMD3 MCP dispatch failed: {error}"),
            }
        })))),
    }
}

fn value_as_object(value: Value) -> JsonObject {
    match value {
        Value::Object(object) => object,
        value => {
            let mut object = JsonObject::new();
            object.insert("result".to_string(), value);
            object
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::{DetailedTask, ErrorCode, InputRequest, TaskPayload, TaskStatus};

    const EXPECTED_TOOL_NAMES: &[&str] = &[
        "alcomd3_add_existing_project",
        "alcomd3_add_repository",
        "alcomd3_backup_project",
        "alcomd3_copy_project",
        "alcomd3_create_project",
        "alcomd3_create_template",
        "alcomd3_edit_template",
        "alcomd3_get_activity_log_context",
        "alcomd3_get_activity_log_entry",
        "alcomd3_get_environment_settings",
        "alcomd3_get_package_details",
        "alcomd3_get_project_details",
        "alcomd3_get_technical_log_entry",
        "alcomd3_get_template",
        "alcomd3_install_project_package",
        "alcomd3_list_packages",
        "alcomd3_list_projects",
        "alcomd3_list_repositories",
        "alcomd3_list_repository_packages",
        "alcomd3_list_templates",
        "alcomd3_reinstall_project_package",
        "alcomd3_remove_repository",
        "alcomd3_remove_template",
        "alcomd3_remove_template_package",
        "alcomd3_remove_template_unitypackage",
        "alcomd3_restore_project_from_backup",
        "alcomd3_search_activity_logs",
        "alcomd3_search_technical_logs",
        "alcomd3_set_template_package",
        "alcomd3_set_template_unitypackage",
        "alcomd3_summarize_activity_logs",
        "alcomd3_summarize_technical_logs",
        "alcomd3_uninstall_project_package",
    ];

    #[test]
    fn tool_catalog_preserves_all_33_public_tools_and_metadata() {
        let tools = Alcomd3Mcp::tool_router().list_all();
        let names = tools
            .iter()
            .map(|tool| tool.name.as_ref())
            .collect::<Vec<_>>();

        assert_eq!(names, EXPECTED_TOOL_NAMES);
        for tool in tools {
            assert!(
                tool.description
                    .as_ref()
                    .is_some_and(|value| !value.is_empty()),
                "{} is missing its description",
                tool.name
            );
            assert!(
                tool.annotations.is_some(),
                "{} is missing its annotations",
                tool.name
            );
            assert_eq!(
                tool.input_schema.get("type"),
                Some(&Value::String("object".to_string())),
                "{} must retain an object input schema",
                tool.name
            );
        }
    }

    #[test]
    fn server_identity_capabilities_and_protocol_versions_are_explicit() {
        let info = server_info();

        assert_eq!(info.server_info.name, "alcomd3-mcp");
        assert_eq!(info.server_info.version, env!("CARGO_PKG_VERSION"));
        assert!(info.capabilities.tools.is_some());
        assert!(info.capabilities.supports_tasks());
        assert_eq!(
            SUPPORTED_PROTOCOL_VERSIONS,
            &[ProtocolVersion::V_2026_07_28, ProtocolVersion::V_2025_11_25,]
        );
        assert_eq!(ProtocolVersion::LATEST, ProtocolVersion::V_2025_11_25);
        assert_ne!(SUPPORTED_PROTOCOL_VERSIONS[0], ProtocolVersion::LATEST);
    }

    #[test]
    fn production_limits_remain_64_concurrent_and_600_per_minute() {
        let limits = ToolInvocationLimits::production();

        assert_eq!(limits.max_concurrent, 64);
        assert_eq!(limits.max_started_per_window, 600);
        assert_eq!(limits.window, Duration::from_secs(60));
    }

    #[test]
    fn tool_invocation_limiter_enforces_window_capacity() {
        let limiter = ToolInvocationLimiter::new(ToolInvocationLimits {
            max_concurrent: 8,
            max_started_per_window: 2,
            window: Duration::from_secs(60),
        });
        let now = Instant::now();

        drop(limiter.try_start(now).unwrap());
        drop(limiter.try_start(now + Duration::from_secs(1)).unwrap());
        assert!(matches!(
            limiter.try_start(now + Duration::from_secs(2)),
            Err(ToolRateLimitReason::TooManyStartedInWindow)
        ));
        assert!(limiter.try_start(now + Duration::from_secs(61)).is_ok());
    }

    #[test]
    fn tool_invocation_limiter_enforces_concurrency_capacity() {
        let limiter = ToolInvocationLimiter::new(ToolInvocationLimits {
            max_concurrent: 2,
            max_started_per_window: 16,
            window: Duration::from_secs(60),
        });
        let now = Instant::now();
        let first = limiter.try_start(now).unwrap();
        let _second = limiter.try_start(now).unwrap();

        assert!(matches!(
            limiter.try_start(now),
            Err(ToolRateLimitReason::TooManyConcurrent)
        ));
        drop(first);
        assert!(limiter.try_start(now).is_ok());
    }

    #[test]
    fn success_and_business_errors_keep_structured_tool_results() {
        let success = match format_invoke_result(Ok(InvokeOutcome::Success(json!({
            "ok": true,
            "value": 1,
        })))) {
            Ok(value) => value,
            Err(_) => panic!("success result was converted into a tool error"),
        };
        assert_eq!(success.0["ok"], true);
        assert_eq!(success.0["value"], 1);

        let error = match format_invoke_result(Ok(InvokeOutcome::ToolError(json!({
            "ok": false,
            "error": {
                "code": "mcp_disabled",
                "message": "MCP is disabled",
            }
        })))) {
            Ok(_) => panic!("business error was converted into a successful tool result"),
            Err(value) => value,
        };
        assert_eq!(error.0["ok"], false);
        assert_eq!(error.0["error"]["code"], "mcp_disabled");
    }

    #[test]
    fn typed_repository_result_preserves_the_canonical_shape() {
        let result = match format_typed_invoke_result::<ListRepositoriesOutput>(Ok(
            InvokeOutcome::Success(json!({
                "ok": true,
                "repositories": [{
                    "id": "com.example.repository",
                    "url": "https://example.com/index.json",
                    "name": "Example Repository",
                    "displayName": "My Repository",
                    "kind": "user",
                    "hidden": false,
                }],
                "packageVisibility": {
                    "hideLocalUserPackages": false,
                    "showPrereleasePackages": true,
                },
            })),
        )) {
            Ok(value) => value,
            Err(_) => panic!("canonical repository response was rejected"),
        };
        let value = serde_json::to_value(result.0).unwrap();

        assert_eq!(value["repositories"][0]["kind"], "user");
        assert_eq!(value["packageVisibility"]["showPrereleasePackages"], true);
    }

    #[test]
    fn remove_null_object_fields_keeps_gui_defaults_available() {
        let mut value = json!({
            "visibility": null,
            "limit": 10,
            "nested": {
                "order": null,
                "search": "mcp"
            },
            "items": [{
                "scope": null,
                "levels": ["error"]
            }]
        });

        remove_null_object_fields(&mut value);
        assert!(value.get("visibility").is_none());
        assert_eq!(value["limit"], 10);
        assert!(value["nested"].get("order").is_none());
        assert_eq!(value["nested"]["search"], "mcp");
        assert!(value["items"][0].get("scope").is_none());
    }

    #[test]
    fn operation_lookup_errors_map_to_invalid_params() {
        let error = mcp_error_from_tool_error(json!({
            "ok": false,
            "error": {
                "code": "project_task_not_found",
                "message": "MCP project operation was not found: task-1"
            }
        }));

        assert_eq!(error.code, ErrorCode::INVALID_PARAMS);
    }

    #[test]
    fn project_operation_poll_interval_defaults_and_clamps_low_values() {
        let mut snapshot = project_task_snapshot_from_value(json!({
            "taskId": "task-1",
            "kind": "backup",
            "status": "working",
            "createdAt": "2026-06-25T00:00:00Z",
            "lastUpdatedAt": "2026-06-25T00:00:01Z"
        }))
        .unwrap();

        assert_eq!(
            project_task_poll_interval(&snapshot),
            Duration::from_millis(PROJECT_TASK_DEFAULT_POLL_INTERVAL_MS)
        );
        snapshot.poll_interval = Some(0);
        assert_eq!(
            project_task_poll_interval(&snapshot),
            Duration::from_millis(PROJECT_TASK_MIN_POLL_INTERVAL_MS)
        );
    }

    #[tokio::test]
    async fn task_manager_state_survives_transport_clones_and_completes() {
        let manager = McpTaskManager::new();
        let first_transport = manager.clone();
        let task = first_transport.spawn(
            TaskOptions::new()
                .with_ttl_ms(60_000)
                .with_poll_interval_ms(100),
            |_context| {
                Box::pin(async {
                    Ok(CallToolResult::structured(json!({
                        "ok": true,
                        "value": 1,
                    })))
                })
            },
        );
        drop(first_transport);

        let restarted_transport = manager.clone();
        let detailed =
            wait_for_task_status(&restarted_transport, &task.task_id, TaskStatus::Completed).await;
        assert_eq!(detailed.task.ttl_ms, Some(60_000));
        assert_eq!(detailed.task.poll_interval_ms, Some(100));
        assert!(matches!(detailed.payload, TaskPayload::Completed { .. }));
    }

    #[tokio::test]
    async fn task_update_delivers_input_and_task_cancel_is_cooperative() {
        let manager = McpTaskManager::new();
        let input_task = manager.spawn(TaskOptions::default(), |context| {
            Box::pin(async move {
                let request: InputRequest = serde_json::from_value(json!({
                    "method": "elicitation/create",
                    "params": {
                        "message": "Continue?",
                        "requestedSchema": {
                            "type": "object",
                            "properties": {}
                        }
                    }
                }))
                .map_err(|error| McpError::internal_error(error.to_string(), None))?;
                let response = context.request_input("confirmation", request).await?;
                Ok(CallToolResult::structured(json!({
                    "ok": true,
                    "answer": response,
                })))
            })
        });
        wait_for_task_status(&manager, &input_task.task_id, TaskStatus::InputRequired).await;
        manager
            .update_task(
                &input_task.task_id,
                [("confirmation".to_string(), json!({ "accepted": true }))],
            )
            .unwrap();
        wait_for_task_status(&manager, &input_task.task_id, TaskStatus::Completed).await;

        let cancelled_task = manager.spawn(TaskOptions::default(), |context| {
            Box::pin(async move {
                context.cancelled().await;
                Err(TaskExit::Cancelled)
            })
        });
        manager.cancel_task(&cancelled_task.task_id).unwrap();
        wait_for_task_status(&manager, &cancelled_task.task_id, TaskStatus::Cancelled).await;
    }

    #[tokio::test]
    async fn task_failures_are_observable_and_shutdown_clears_tasks() {
        let manager = McpTaskManager::new();
        let failed_task = manager.spawn(TaskOptions::default(), |_context| {
            Box::pin(async {
                Err(TaskExit::Error(McpError::invalid_request(
                    "project operation failed",
                    Some(json!({ "code": "project_operation_failed" })),
                )))
            })
        });
        let failed = wait_for_task_status(&manager, &failed_task.task_id, TaskStatus::Failed).await;
        assert!(matches!(failed.payload, TaskPayload::Failed { .. }));

        let running_task = manager.spawn(TaskOptions::default(), |context| {
            Box::pin(async move {
                context.cancelled().await;
                Err(TaskExit::Cancelled)
            })
        });
        assert_eq!(manager.running_task_count(), 1);
        manager.shutdown();
        assert_eq!(manager.running_task_count(), 0);
        assert!(manager.get_task(&running_task.task_id).is_err());
    }

    async fn wait_for_task_status(
        manager: &McpTaskManager,
        task_id: &str,
        expected: TaskStatus,
    ) -> DetailedTask {
        for _ in 0..100 {
            let detailed = manager.get_task(task_id).unwrap();
            if detailed.status() == expected {
                return detailed;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("task {task_id} did not reach {expected:?}");
    }
}
