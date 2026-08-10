use crate::activity_log::{
    ActivityImportance, ActivityInput, ActivityKind, ActivityLogState, ActivitySource, operations,
};
use crate::commands::prelude::*;
use crate::extensions::MCP_EXTENSION_ID;
use tauri::{AppHandle, Manager, State};

#[tauri::command]
#[specta::specta]
pub async fn mcp_status(
    app: AppHandle,
    config: State<'_, GuiConfigState>,
    mcp: State<'_, crate::mcp::McpState>,
) -> Result<crate::mcp::McpStatus, RustError> {
    let (extension_enabled, access_enabled) = {
        let config = config.get();
        (
            config.is_extension_enabled(MCP_EXTENSION_ID),
            config.mcp_enabled,
        )
    };
    let status = mcp.status(extension_enabled && access_enabled).await;
    if extension_enabled && !status.is_running() {
        tauri::async_runtime::spawn(async move {
            let mcp = app.state::<crate::mcp::McpState>();
            if let Err(error) = mcp.ensure_running_and_emit_status(app.clone()).await {
                log::error!(
                    "failed to start MCP Streamable HTTP server after reading status: {error}"
                );
            }
        });
    }
    Ok(status)
}

#[tauri::command]
#[specta::specta]
pub async fn mcp_set_enabled(
    app: AppHandle,
    config: State<'_, GuiConfigState>,
    mcp: State<'_, crate::mcp::McpState>,
    enabled: bool,
) -> Result<crate::mcp::McpStatus, RustError> {
    if !config.get().is_extension_enabled(MCP_EXTENSION_ID) {
        return Err(RustError::unrecoverable_str(
            "The MCP extension is disabled",
        ));
    }
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
                    if !config.is_extension_enabled(MCP_EXTENSION_ID) {
                        return Err(RustError::unrecoverable_str(
                            "The MCP extension is disabled",
                        ));
                    }
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
pub async fn mcp_configure_client(
    app: AppHandle,
    config: State<'_, GuiConfigState>,
    mcp: State<'_, crate::mcp::McpState>,
    client: crate::mcp_client_config::McpClient,
    overwrite: bool,
) -> Result<crate::mcp_client_config::McpClientSetupResult, RustError> {
    if !config.get().is_extension_enabled(MCP_EXTENSION_ID) {
        return Err(RustError::unrecoverable_str(
            "The MCP extension is disabled",
        ));
    }
    mcp.ensure_running(app.clone()).await?;
    let (port, token) = crate::mcp::ensure_mcp_http_config(&app).await?;
    let endpoint = crate::mcp::mcp_http_endpoint(crate::mcp::MCP_HTTP_BIND_HOST, port);
    let client_name = client.display_name();

    let app_for_activity = app.clone();
    let activity = app_for_activity.state::<ActivityLogState>();
    activity
        .track_result(
            Some(&app_for_activity),
            ActivityInput::new(
                ActivitySource::Gui,
                ActivityKind::Write,
                ActivityImportance::Primary,
                operations::MCP_CONFIGURE_CLIENT,
                format!("Configuring {client_name} MCP"),
            ),
            format!("{client_name} MCP configuration checked"),
            Vec::new(),
            async move {
                Ok(
                    crate::mcp_client_config::configure_client(
                        client, &endpoint, &token, overwrite,
                    )
                    .await?,
                )
            },
        )
        .await
}
