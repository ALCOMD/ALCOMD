use super::*;

pub(super) async fn invoke_gui(
    app: AppHandle,
    method: &str,
    params: Value,
    client: ClientIdentity,
) -> Result<Value, McpToolError> {
    let request = McpInvocation {
        request_id: Uuid::new_v4(),
        client,
        method: method.to_string(),
        params,
    };

    let mcp = app.state::<McpState>();
    let enabled = app.state::<GuiConfigState>().get().mcp_enabled;
    if mcp.record_client(&request.client).await {
        mcp.emit_status(app.clone(), enabled).await;
    }

    if !enabled && !mcp_request_allowed_when_disabled(&request.method) {
        record_disabled_mcp_tool_call(&app, &request);
        return Err(McpToolError::new("mcp_disabled", MCP_DISABLED_MESSAGE));
    }

    let mut tool_call =
        mcp_tool_call_for_request(&request.method, &request.params, request.request_id);
    if let Some(tool_call) = &tool_call {
        emit_tracked_mcp_tool_call_event(&app, tool_call, McpToolCallPhase::Started);
    }
    if let Some(tool_call) = &mut tool_call {
        tool_call.activity = start_mcp_tool_call_activity(&app, &request, tool_call);
    }

    let result = dispatch_gui_request(
        app.clone(),
        &request.method,
        request.params,
        tool_call.clone(),
    )
    .await;
    if let Some(tool_call) = &tool_call
        && mcp_tool_call_finishes_with_response(&request.method, &result)
    {
        finish_tracked_mcp_tool_call_activity(
            &app,
            tool_call,
            mcp_tool_call_finished_phase(&result),
            mcp_tool_call_error_message(&result),
        );
        emit_tracked_mcp_tool_call_event(&app, tool_call, mcp_tool_call_finished_phase(&result));
    }

    result
}

#[derive(Debug, Clone, Serialize)]
pub struct McpToolError {
    pub code: String,
    pub message: String,
    pub data: Option<Value>,
}

impl McpToolError {
    pub(super) fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            data: None,
        }
    }

    pub(super) fn with_data(
        code: impl Into<String>,
        message: impl Into<String>,
        data: Value,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            data: Some(data),
        }
    }

    pub(super) fn from_error(code: impl Into<String>, error: impl std::error::Error) -> Self {
        Self::new(code, error.to_string())
    }

    pub(super) fn from_rust_error(code: impl Into<String>, error: RustError) -> Self {
        Self::new(code, error.into_message())
    }
}

async fn dispatch_gui_request(
    app: AppHandle,
    method: &str,
    params: Value,
    tool_call: Option<McpTrackedToolCall>,
) -> Result<Value, McpToolError> {
    match method {
        "list_projects" => list_projects(app).await,
        "list_templates" => list_templates(app).await,
        "get_template" => get_template(app, params).await,
        "create_template" => create_template(app, params).await,
        "edit_template" => edit_template(app, params).await,
        "set_template_package" => set_template_package(app, params).await,
        "remove_template_package" => remove_template_package(app, params).await,
        "set_template_unitypackage" => set_template_unitypackage(app, params).await,
        "remove_template_unitypackage" => remove_template_unitypackage(app, params).await,
        "remove_template" => remove_template(app, params).await,
        "get_project_details" => get_project_details(app, params).await,
        "list_repositories" => list_repositories(app).await,
        "add_repository" => add_repository(app, params).await,
        "remove_repository" => remove_repository(app, params).await,
        "get_package_details" => get_package_details(app, params).await,
        "list_packages" => list_packages(app, params).await,
        "list_repository_packages" => list_repository_packages(app, params).await,
        "get_environment_settings" => get_environment_settings(app).await,
        "search_activity_logs" => search_activity_logs(app, params).await,
        "get_activity_log_entry" => get_activity_log_entry(app, params).await,
        "summarize_activity_logs" => summarize_activity_logs(app, params).await,
        "get_activity_log_context" => get_activity_log_context(app, params).await,
        "search_technical_logs" => search_technical_logs(app, params).await,
        "get_technical_log_entry" => get_technical_log_entry(app, params).await,
        "summarize_technical_logs" => summarize_technical_logs(app, params).await,
        "create_project" => create_project(app, params).await,
        "add_existing_project" => add_existing_project(app, params).await,
        "backup_project" => backup_project(app, params).await,
        "copy_project" => copy_project(app, params).await,
        "restore_project_from_backup" => restore_project_from_backup(app, params).await,
        "install_project_package" => install_project_package(app, params).await,
        "uninstall_project_package" => uninstall_project_package(app, params).await,
        "reinstall_project_package" => reinstall_project_package(app, params).await,
        MCP_OPERATION_START_METHOD => project_operation_start(app, params, tool_call).await,
        MCP_OPERATION_GET_METHOD => project_operation_get(app, params).await,
        MCP_OPERATION_CANCEL_METHOD => project_operation_cancel(app, params).await,
        "search_packages" => list_packages(app, params).await,
        _ => Err(McpToolError::new(
            "unknown_method",
            format!("MCP method is not implemented: {method}"),
        )),
    }
}
