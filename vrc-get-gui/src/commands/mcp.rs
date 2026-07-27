use crate::activity_log::{
    ActivityImportance, ActivityInput, ActivityKind, ActivityLogState, ActivitySource, operations,
};
use crate::commands::prelude::*;
use tauri::{AppHandle, Manager, State};

#[tauri::command]
#[specta::specta]
pub async fn mcp_status(
    app: AppHandle,
    config: State<'_, GuiConfigState>,
    mcp: State<'_, crate::mcp::McpState>,
) -> Result<crate::mcp::McpStatus, RustError> {
    if let Err(error) = mcp.ensure_running(app).await {
        log::error!("failed to ensure MCP Streamable HTTP server while reading status: {error}");
    }
    Ok(mcp.status(config.get().mcp_enabled).await)
}

#[tauri::command]
#[specta::specta]
pub async fn mcp_set_enabled(
    app: AppHandle,
    config: State<'_, GuiConfigState>,
    mcp: State<'_, crate::mcp::McpState>,
    enabled: bool,
) -> Result<crate::mcp::McpStatus, RustError> {
    let app_for_activity = app.clone();
    let activity = app_for_activity.state::<ActivityLogState>();
    activity
        .track_result(
            Some(&app_for_activity),
            ActivityInput::new(
                ActivitySource::Gui,
                ActivityKind::Write,
                ActivityImportance::Primary,
                operations::MCP_SET_ENABLED,
                if enabled {
                    "Enabling MCP"
                } else {
                    "Disabling MCP"
                },
            ),
            if enabled {
                "MCP enabled"
            } else {
                "MCP disabled"
            },
            Vec::new(),
            async move {
                {
                    let mut config = config.load_mut().await?;
                    config.mcp_enabled = enabled;
                    config.save().await?;
                }

                mcp.set_enabled(app.clone(), enabled).await?;
                Ok(mcp.status(enabled).await)
            },
        )
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn mcp_configure_codex(
    app: AppHandle,
    mcp: State<'_, crate::mcp::McpState>,
    overwrite: bool,
) -> Result<crate::mcp_client_config::McpCodexSetupResult, RustError> {
    mcp.ensure_running(app.clone()).await?;
    let (port, token) = crate::mcp::ensure_mcp_http_config(&app).await?;
    let endpoint = crate::mcp::mcp_http_endpoint(alcomd3_mcp_protocol::MCP_HTTP_BIND_HOST, port);

    let app_for_activity = app.clone();
    let activity = app_for_activity.state::<ActivityLogState>();
    activity
        .track_result(
            Some(&app_for_activity),
            ActivityInput::new(
                ActivitySource::Gui,
                ActivityKind::Write,
                ActivityImportance::Primary,
                operations::MCP_CONFIGURE_CODEX,
                "Configuring Codex MCP",
            ),
            "Codex MCP configuration checked",
            Vec::new(),
            async move {
                Ok(crate::mcp_client_config::configure_codex(&endpoint, &token, overwrite).await?)
            },
        )
        .await
}
