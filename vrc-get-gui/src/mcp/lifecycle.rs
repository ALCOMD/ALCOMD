use super::*;

pub struct McpState {
    inner: Mutex<McpStateInner>,
    pub(super) project_operations: Arc<StdMutex<McpProjectOperationStore>>,
    limiter: tools::ToolInvocationLimiter,
    protocol_tasks: tasks::McpTaskManager,
}

struct McpStateInner {
    active: Option<http::ActiveMcpHttpServer>,
    recent_clients: VecDeque<McpRecentClientStatus>,
    last_client_status_emit_unix_ms: u64,
}

impl McpState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(McpStateInner {
                active: None,
                recent_clients: VecDeque::new(),
                last_client_status_emit_unix_ms: 0,
            }),
            project_operations: Arc::new(StdMutex::new(McpProjectOperationStore::default())),
            limiter: tools::ToolInvocationLimiter::new(tools::ToolInvocationLimits::production()),
            protocol_tasks: tasks::McpTaskManager::new(),
        }
    }

    pub async fn status(&self, enabled: bool) -> McpStatus {
        let mut inner = self.inner.lock().await;
        if inner
            .active
            .as_ref()
            .is_some_and(|active| active.is_finished())
        {
            log::error!("embedded MCP Streamable HTTP server exited unexpectedly");
            inner.active.take();
        }
        let http = inner.active.as_ref();
        let now_unix_ms = now_unix_ms();

        McpStatus {
            enabled,
            running: http.is_some(),
            protocol_version: MCP_PROTOCOL_VERSION.to_string(),
            transport: "streamable-http".to_string(),
            host: http.map(|http| http.host.clone()),
            port: http.map(|http| http.port),
            mcp_endpoint: http.map(|http| mcp_http_endpoint(&http.host, http.port)),
            authorization_token: http.map(|http| http.token.clone()),
            authorization_token_environment_variable: MCP_HTTP_TOKEN_ENV.to_string(),
            quick_setup_supported: crate::mcp_client_config::quick_setup_supported(),
            recent_clients: inner
                .recent_clients
                .iter()
                .filter(|client| is_recent_client_activity(client.last_seen_unix_ms, now_unix_ms))
                .cloned()
                .collect(),
        }
    }

    pub async fn ensure_running(&self, app: AppHandle) -> io::Result<()> {
        self.start(app).await
    }

    pub async fn ensure_running_and_emit_status(&self, app: AppHandle) -> io::Result<()> {
        self.start(app.clone()).await?;
        let enabled = app.state::<GuiConfigState>().get().mcp_enabled;
        self.emit_status(app, enabled).await;
        Ok(())
    }

    pub async fn set_enabled(&self, app: AppHandle, enabled: bool) -> io::Result<()> {
        if let Err(e) = self.start(app.clone()).await {
            log::error!("failed to ensure embedded MCP HTTP server while setting MCP access: {e}");
        }
        self.emit_status(app, enabled).await;
        Ok(())
    }

    pub async fn synchronize_extension_state(&self, app: AppHandle) -> io::Result<()> {
        loop {
            let enabled = app
                .state::<GuiConfigState>()
                .get()
                .is_extension_enabled(crate::extensions::MCP_EXTENSION_ID);
            let result = if enabled {
                self.start(app.clone()).await
            } else {
                self.stop(Some(&app)).await
            };
            let latest_enabled = app
                .state::<GuiConfigState>()
                .get()
                .is_extension_enabled(crate::extensions::MCP_EXTENSION_ID);

            if latest_enabled != enabled {
                if let Err(error) = result {
                    log::error!("failed to apply superseded MCP extension enabled status: {error}");
                }
                continue;
            }

            let access_enabled = latest_enabled && app.state::<GuiConfigState>().get().mcp_enabled;
            self.emit_status(app, access_enabled).await;
            return result;
        }
    }

    async fn start(&self, app: AppHandle) -> io::Result<()> {
        let (http_port, http_token) = ensure_mcp_http_config(&app).await?;
        let mut inner = self.inner.lock().await;
        if let Some(active) = inner.active.as_ref()
            && active.port == http_port
            && active.token == http_token
            && !active.is_finished()
        {
            return Ok(());
        }
        if let Some(active) = inner.active.take() {
            active.stop().await;
        }

        let http = http::start(app, http_port, http_token, self.limiter.clone()).await?;

        inner.recent_clients.clear();
        inner.last_client_status_emit_unix_ms = 0;
        inner.active = Some(http);
        Ok(())
    }

    async fn stop(&self, app: Option<&AppHandle>) -> io::Result<()> {
        let mut inner = self.inner.lock().await;
        if let Some(active) = inner.active.take() {
            active.stop().await;
        }
        inner.recent_clients.clear();
        inner.last_client_status_emit_unix_ms = 0;
        let tool_calls = self.project_operations.lock().unwrap().abort_all();
        self.protocol_tasks.shutdown();
        if let Some(app) = app {
            for tool_call in tool_calls {
                cancel_tracked_mcp_tool_call_activity(app, &tool_call);
                emit_tracked_mcp_tool_call_event(app, &tool_call, McpToolCallPhase::Failed);
            }
        }
        Ok(())
    }

    pub async fn shutdown(&self, app: &AppHandle) -> io::Result<()> {
        self.stop(Some(app)).await
    }

    pub(super) async fn emit_status(&self, app: AppHandle, enabled: bool) {
        let status = self.status(enabled).await;
        if let Err(e) = app.emit(MCP_STATUS_CHANGED_EVENT, status) {
            log::error!("failed to emit MCP status change: {e}");
        }
    }

    pub(super) async fn record_client(&self, client: &ClientIdentity) -> bool {
        let mut inner = self.inner.lock().await;
        let now_unix_ms = now_unix_ms();
        let McpStateInner {
            recent_clients,
            last_client_status_emit_unix_ms,
            ..
        } = &mut *inner;
        record_client_activity(
            recent_clients,
            client,
            now_unix_ms,
            last_client_status_emit_unix_ms,
        )
    }

    pub(super) fn protocol_tasks(&self) -> tasks::McpTaskManager {
        self.protocol_tasks.clone()
    }
}

pub(crate) async fn ensure_mcp_http_config(app: &AppHandle) -> io::Result<(u16, String)> {
    let state = app.state::<GuiConfigState>();
    let current = state.get();
    if current.mcp_http_port != 0 && current.mcp_http_token.len() >= 32 {
        return Ok((current.mcp_http_port, current.mcp_http_token.clone()));
    }
    drop(current);

    let mut config = state.load_mut().await?;
    let changed = config.ensure_mcp_http_config();
    let result = (config.mcp_http_port, config.mcp_http_token.clone());
    if changed {
        config.save().await?;
    }
    Ok(result)
}

pub(crate) fn mcp_http_endpoint(host: &str, port: u16) -> String {
    format!("http://{host}:{port}{MCP_HTTP_PATH}")
}
