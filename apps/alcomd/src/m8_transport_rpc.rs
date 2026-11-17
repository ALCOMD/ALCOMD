//! Observer reports have no owner or business authority. Binding always uses current Core auth.
use crate::{
    Applications, ConnectionState, DispatchAction, current_time_ms, error_action,
    require_capability, success_action,
};
use alcomd_application::{
    AccessContext, ManagementError, ManagementErrorCode as E, TransportReadAccess,
};
use alcomd_protocol::*;

pub(super) fn dispatch(
    request: RequestEnvelope,
    state: &ConnectionState,
    apps: &Applications,
    access: &AccessContext,
) -> DispatchAction {
    let capability = if request.method == METHOD_MCP_TRANSPORT_SNAPSHOT {
        CAPABILITY_MCP_TRANSPORT_READ_V1
    } else {
        CAPABILITY_MCP_TRANSPORT_REPORT_V1
    };
    if let Some(action) = require_capability(&request.id, state, capability) {
        return action;
    }
    let result: Result<serde_json::Value, ManagementError> = (|| {
        let now = current_time_ms().map_err(|_| ManagementError::from(E::Unavailable))?;
        let instant = std::time::Instant::now();
        let output = match request.method.as_str() {
            METHOD_MCP_TRANSPORT_REPORT if state.transport_observer => {
                if serde_json::to_vec(&request.params)
                    .map_or(true, |v| v.len() > alcomd_application::MCP_TRANSPORT_BYTES)
                {
                    return Err(E::ResourceLimit.into());
                }
                let input = serde_json::from_value::<alcomd_application::McpTransportReport>(
                    request.params,
                )
                .map_err(|_| ManagementError::from(E::InvalidArgument))?;
                serde_json::to_value(apps.mcp_transport.report(
                    &state.connection_id,
                    input,
                    now,
                    instant,
                    |bytes| getrandom::fill(bytes).map_err(|_| ()),
                )?)
            }
            METHOD_MCP_TRANSPORT_BIND_REQUEST
                if !state.transport_observer && access.is_mcp_client() =>
            {
                let input: McpTransportBindParams = serde_json::from_value(request.params)
                    .map_err(|_| ManagementError::from(E::InvalidArgument))?;
                serde_json::to_value(apps.mcp_transport.bind(&input.ticket, access, instant)?)
            }
            METHOD_MCP_TRANSPORT_SNAPSHOT if !state.transport_observer => {
                let input =
                    serde_json::from_value::<alcomd_application::McpTransportQuery>(request.params)
                        .map_err(|_| ManagementError::from(E::InvalidArgument))?;
                serde_json::to_value(apps.mcp_transport.snapshot(
                    input,
                    TransportReadAccess::from_client(access),
                    now,
                    instant,
                )?)
            }
            _ => return Err(E::PermissionDenied.into()),
        };
        output.map_err(|_| E::InternalError.into())
    })();
    match result {
        Ok(value) => success_action(request.id, value, None),
        Err(error) => error_action(
            Some(request.id),
            match error.code {
                E::PermissionDenied | E::ScopeDenied => RpcError::permission_denied(),
                E::RevisionConflict => RpcError::revision_conflict(),
                E::InvalidArgument | E::ResourceLimit => RpcError::invalid_request(),
                E::Unavailable => RpcError::store_unavailable(),
                _ => RpcError::internal(alcomd_application::OperationId::new().to_string()),
            },
            false,
        ),
    }
}
