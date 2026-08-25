use alcomd_client::{AlcomdClient, ClientConfig, ClientError};
use alcomd_protocol::{
    ExtensionResult, ExtensionUiCloseParams, ExtensionUiCloseResult, ExtensionUiDispatchParams,
    ExtensionUiDispatchResult, ExtensionUiOpenParams, ExtensionUiOpenResult,
    ExtensionUiRefreshParams, ExtensionUiSnapshotResult, RpcError,
};
use tauri::State;
use tauri::async_runtime::Mutex;

#[derive(Default)]
struct GuiClientState {
    client: Mutex<Option<AlcomdClient>>,
}

#[tauri::command]
async fn gui_extension_get(
    state: State<'_, GuiClientState>,
    extension_id: String,
) -> Result<ExtensionResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.extension_get(extension_id).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_extension_ui_open(
    state: State<'_, GuiClientState>,
    params: ExtensionUiOpenParams,
) -> Result<ExtensionUiOpenResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.extension_ui_open(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_extension_ui_refresh(
    state: State<'_, GuiClientState>,
    params: ExtensionUiRefreshParams,
) -> Result<ExtensionUiSnapshotResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.extension_ui_refresh(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_extension_ui_dispatch(
    state: State<'_, GuiClientState>,
    params: ExtensionUiDispatchParams,
) -> Result<ExtensionUiDispatchResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.extension_ui_dispatch(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_extension_ui_close(
    state: State<'_, GuiClientState>,
    params: ExtensionUiCloseParams,
) -> Result<ExtensionUiCloseResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.extension_ui_close(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

async fn connect_if_needed(client: &mut Option<AlcomdClient>) -> Result<(), RpcError> {
    if client.is_none() {
        *client = Some(
            AlcomdClient::connect(ClientConfig::default())
                .await
                .map_err(client_error)?,
        );
    }
    Ok(())
}

fn finish_call<T>(
    client: &mut Option<AlcomdClient>,
    result: Result<T, ClientError>,
) -> Result<T, RpcError> {
    match result {
        Ok(value) => Ok(value),
        Err(ClientError::Remote(error)) => Err(error),
        Err(error) => {
            *client = None;
            Err(client_error(error))
        }
    }
}

fn client_error(error: ClientError) -> RpcError {
    match error {
        ClientError::Remote(error) => error,
        ClientError::Transport(_)
        | ClientError::StartDaemon(_)
        | ClientError::DaemonPathUnavailable
        | ClientError::StartTimeout
        | ClientError::InvalidResponse => daemon_unavailable(),
    }
}

fn daemon_unavailable() -> RpcError {
    RpcError::extension("daemon_unavailable")
}

/// Starts the official ALCOMD GUI shell.
///
/// Business logic must remain in `alcomd`; this process is only a client and UI host.
pub fn run() {
    tauri::Builder::default()
        .manage(GuiClientState::default())
        .invoke_handler(tauri::generate_handler![
            gui_extension_get,
            gui_extension_ui_open,
            gui_extension_ui_refresh,
            gui_extension_ui_dispatch,
            gui_extension_ui_close
        ])
        .run(tauri::generate_context!())
        .expect("failed to run alcomd-gui");
}
