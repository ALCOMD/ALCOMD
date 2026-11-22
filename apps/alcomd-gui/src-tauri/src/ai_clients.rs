use super::{GuiClientState, connect_if_needed, daemon_unavailable, finish_call};
use alcomd_protocol as rpc;

#[tauri::command]
pub(super) async fn gui_mcp_clients_list(
    state: tauri::State<'_, GuiClientState>,
    params: rpc::McpClientsListParams,
) -> Result<rpc::McpClientsListResult, rpc::RpcError> {
    let mut connection = state.reads.lock().await;
    connect_if_needed(&mut connection).await?;
    let result = connection
        .as_mut()
        .ok_or_else(daemon_unavailable)?
        .mcp_clients_list(params)
        .await;
    finish_call(&mut connection, result)
}

#[tauri::command]
pub(super) async fn gui_mcp_client_create(
    state: tauri::State<'_, GuiClientState>,
    params: rpc::McpClientsCreateParams,
) -> Result<rpc::McpClientRecord, rpc::RpcError> {
    let mut connection = state.control.lock().await;
    connect_if_needed(&mut connection).await?;
    let result = connection
        .as_mut()
        .ok_or_else(daemon_unavailable)?
        .mcp_client_create(params)
        .await;
    finish_call(&mut connection, result)
}

#[tauri::command]
pub(super) async fn gui_mcp_client_issue_credential(
    state: tauri::State<'_, GuiClientState>,
    params: rpc::McpClientMutationParams,
) -> Result<rpc::McpClientCredentialResult, rpc::RpcError> {
    let mut connection = state.control.lock().await;
    connect_if_needed(&mut connection).await?;
    let result = connection
        .as_mut()
        .ok_or_else(daemon_unavailable)?
        .mcp_client_issue_credential(params)
        .await;
    finish_call(&mut connection, result)
}

#[tauri::command]
pub(super) async fn gui_mcp_client_revoke(
    state: tauri::State<'_, GuiClientState>,
    params: rpc::McpClientMutationParams,
) -> Result<rpc::McpClientRecord, rpc::RpcError> {
    let mut connection = state.control.lock().await;
    connect_if_needed(&mut connection).await?;
    let result = connection
        .as_mut()
        .ok_or_else(daemon_unavailable)?
        .mcp_client_revoke(params)
        .await;
    finish_call(&mut connection, result)
}

#[tauri::command]
pub(super) async fn gui_oauth_registrations_list(
    state: tauri::State<'_, GuiClientState>,
    params: rpc::McpOAuthPageParams,
) -> Result<rpc::McpOAuthRegistrationPage, rpc::RpcError> {
    let mut connection = state.reads.lock().await;
    connect_if_needed(&mut connection).await?;
    let client = connection.as_mut().ok_or_else(daemon_unavailable)?;
    let result = client.oauth_registrations_list(params).await;
    finish_call(&mut connection, result)
}

#[tauri::command]
pub(super) async fn gui_oauth_registration_set(
    state: tauri::State<'_, GuiClientState>,
    params: rpc::McpOAuthRegistrationSetParams,
) -> Result<rpc::McpOAuthRegistrationResult, rpc::RpcError> {
    let mut connection = state.control.lock().await;
    connect_if_needed(&mut connection).await?;
    let client = connection.as_mut().ok_or_else(daemon_unavailable)?;
    let result = client.oauth_registration_set(params).await;
    finish_call(&mut connection, result)
}

#[tauri::command]
pub(super) async fn gui_oauth_registration_revoke(
    state: tauri::State<'_, GuiClientState>,
    params: rpc::McpOAuthRegistrationRevokeParams,
) -> Result<rpc::McpOAuthRegistration, rpc::RpcError> {
    let mut connection = state.control.lock().await;
    connect_if_needed(&mut connection).await?;
    let client = connection.as_mut().ok_or_else(daemon_unavailable)?;
    let result = client.oauth_registration_revoke(params).await;
    finish_call(&mut connection, result)
}

#[tauri::command]
pub(super) async fn gui_oauth_request_get(
    state: tauri::State<'_, GuiClientState>,
    params: rpc::McpOAuthRequestGetParams,
) -> Result<rpc::McpOAuthRequestReview, rpc::RpcError> {
    let mut connection = state.reads.lock().await;
    connect_if_needed(&mut connection).await?;
    let client = connection.as_mut().ok_or_else(daemon_unavailable)?;
    let result = client.oauth_request_get(params).await;
    finish_call(&mut connection, result)
}

#[tauri::command]
pub(super) async fn gui_oauth_request_decide(
    state: tauri::State<'_, GuiClientState>,
    params: rpc::McpOAuthRequestDecideParams,
) -> Result<rpc::McpOAuthRequestDecision, rpc::RpcError> {
    let mut connection = state.control.lock().await;
    connect_if_needed(&mut connection).await?;
    let client = connection.as_mut().ok_or_else(daemon_unavailable)?;
    let result = client.oauth_request_decide(params).await;
    finish_call(&mut connection, result)
}

#[tauri::command]
pub(super) async fn gui_mcp_client_request_create(
    state: tauri::State<'_, GuiClientState>,
    params: rpc::McpClientRequestCreateParams,
) -> Result<rpc::ClientManagementReview, rpc::RpcError> {
    let mut connection = state.control.lock().await;
    connect_if_needed(&mut connection).await?;
    let client = connection.as_mut().ok_or_else(daemon_unavailable)?;
    let result = client.mcp_client_request_create(params).await;
    finish_call(&mut connection, result)
}

#[tauri::command]
pub(super) async fn gui_mcp_client_request_get(
    state: tauri::State<'_, GuiClientState>,
    params: rpc::McpClientRequestGetParams,
) -> Result<rpc::ClientManagementReview, rpc::RpcError> {
    let mut connection = state.reads.lock().await;
    connect_if_needed(&mut connection).await?;
    let client = connection.as_mut().ok_or_else(daemon_unavailable)?;
    let result = client.mcp_client_request_get(params).await;
    finish_call(&mut connection, result)
}

#[tauri::command]
pub(super) async fn gui_mcp_client_request_list(
    state: tauri::State<'_, GuiClientState>,
    params: rpc::McpClientsListParams,
) -> Result<rpc::ClientRequestPage, rpc::RpcError> {
    let mut connection = state.reads.lock().await;
    connect_if_needed(&mut connection).await?;
    let client = connection.as_mut().ok_or_else(daemon_unavailable)?;
    let result = client.mcp_client_request_list(params).await;
    finish_call(&mut connection, result)
}

#[tauri::command]
pub(super) async fn gui_mcp_client_request_decide(
    state: tauri::State<'_, GuiClientState>,
    params: rpc::McpClientRequestDecideParams,
) -> Result<rpc::ClientManagementDecision, rpc::RpcError> {
    let mut connection = state.control.lock().await;
    connect_if_needed(&mut connection).await?;
    let client = connection.as_mut().ok_or_else(daemon_unavailable)?;
    let result = client.mcp_client_request_decide(params).await;
    finish_call(&mut connection, result)
}

#[tauri::command]
pub(super) async fn gui_mcp_runtime_snapshot(
    state: tauri::State<'_, GuiClientState>,
    params: rpc::McpRuntimeSnapshotParams,
) -> Result<rpc::McpRuntimeSnapshotResult, rpc::RpcError> {
    let mut connection = state.reads.lock().await;
    connect_if_needed(&mut connection).await?;
    let client = connection.as_mut().ok_or_else(daemon_unavailable)?;
    let result = client.mcp_runtime_snapshot(params).await;
    finish_call(&mut connection, result)
}

#[tauri::command]
pub(super) async fn gui_mcp_transport_snapshot(
    state: tauri::State<'_, GuiClientState>,
    params: rpc::McpTransportQuery,
) -> Result<rpc::McpTransportSnapshot, rpc::RpcError> {
    let mut connection = state.reads.lock().await;
    connect_if_needed(&mut connection).await?;
    let client = connection.as_mut().ok_or_else(daemon_unavailable)?;
    let result = client.mcp_transport_snapshot(params).await;
    finish_call(&mut connection, result)
}
