//! Read-only projection of real Core IPC connections and dispatch work.
use crate::{
    Applications, ConnectionState, DispatchAction, current_time_ms, error_action,
    require_capability, success_action,
};
use alcomd_application::{
    AccessContext, ManagementError, ManagementErrorCode as E, RuntimeReadAccess,
    RuntimeSnapshotQuery,
};
use alcomd_protocol::{CAPABILITY_MCP_RUNTIME_READ_V1, RequestEnvelope, RpcError};

pub(super) fn dispatch(
    request: RequestEnvelope,
    state: &ConnectionState,
    applications: &Applications,
    access: &AccessContext,
) -> DispatchAction {
    if let Some(action) = require_capability(&request.id, state, CAPABILITY_MCP_RUNTIME_READ_V1) {
        return action;
    }
    let result = (|| {
        let query: RuntimeSnapshotQuery = serde_json::from_value(request.params)
            .map_err(|_| ManagementError::from(E::InvalidArgument))?;
        let now = current_time_ms().map_err(|_| ManagementError::from(E::Unavailable))?;
        applications
            .mcp_runtime
            .snapshot(query, RuntimeReadAccess::from_client(access), now)
    })();
    match result {
        Ok(snapshot) => success_action(request.id, snapshot, None),
        Err(error) => error_action(
            Some(request.id),
            match error.code {
                E::PermissionDenied | E::ScopeDenied => RpcError::permission_denied(),
                E::InvalidArgument => RpcError::invalid_request(),
                E::Unavailable => RpcError::store_unavailable(),
                _ => RpcError::internal(alcomd_application::OperationId::new().to_string()),
            },
            false,
        ),
    }
}
