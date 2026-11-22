use super::{GuiClientState, connect_if_needed, daemon_unavailable, finish_call};
use alcomd_protocol as rpc;

#[tauri::command]
pub(super) async fn gui_clients_list(
    state: tauri::State<'_, GuiClientState>,
    params: rpc::ClientsListParams,
) -> Result<rpc::ClientsListResult, rpc::RpcError> {
    let mut connection = state.reads.lock().await;
    connect_if_needed(&mut connection).await?;
    let result = connection
        .as_mut()
        .ok_or_else(daemon_unavailable)?
        .clients_list(params)
        .await;
    finish_call(&mut connection, result)
}

#[tauri::command]
pub(super) async fn gui_client_instances_list(
    state: tauri::State<'_, GuiClientState>,
    params: rpc::ClientInstancesListParams,
) -> Result<rpc::ClientInstancesListResult, rpc::RpcError> {
    let mut connection = state.reads.lock().await;
    connect_if_needed(&mut connection).await?;
    let result = connection
        .as_mut()
        .ok_or_else(daemon_unavailable)?
        .client_instances_list(params)
        .await;
    finish_call(&mut connection, result)
}

#[tauri::command]
pub(super) async fn gui_clients_disconnect(
    state: tauri::State<'_, GuiClientState>,
    params: rpc::ClientsDisconnectParams,
) -> Result<rpc::ClientsDisconnectResult, rpc::RpcError> {
    let mut connection = state.control.lock().await;
    connect_if_needed(&mut connection).await?;
    let result = connection
        .as_mut()
        .ok_or_else(daemon_unavailable)?
        .clients_disconnect(params)
        .await;
    finish_call(&mut connection, result)
}
