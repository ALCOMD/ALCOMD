//! Independent read-only transport metadata bridge; no credential-bearing API.
use super::HostState;
use alcomd_application::{
    MCP_TRANSPORT_BYTES, ManagementError, ManagementErrorCode as E, McpTransportSnapshot,
};
use serde_json::json;
mod bindings {
    wasmtime::component::bindgen!({
        path: "../../specs/extensions/wit/mcp-transport-v1", world: "transport-v1",
        imports: { default: async }, exports: { default: async },
    });
}
use bindings::alcomd::{mcp_management::clients as errors, mcp_transport::observations as wit};
pub(super) fn add_to_linker(
    linker: &mut wasmtime::component::Linker<HostState>,
) -> anyhow::Result<()> {
    wit::add_to_linker::<_, wasmtime::component::HasSelf<_>>(linker, |s| s)
        .map_err(|_| anyhow::anyhow!("Transport observation interface link failed"))
}
fn failure(error: ManagementError) -> errors::ManagementError {
    errors::ManagementError {
        code: match error.code {
            E::InvalidArgument => errors::ErrorCode::InvalidArgument,
            E::PermissionDenied => errors::ErrorCode::PermissionDenied,
            E::ScopeDenied => errors::ErrorCode::ScopeDenied,
            E::LeaseExpired => errors::ErrorCode::LeaseExpired,
            E::LeaseRevoked => errors::ErrorCode::LeaseRevoked,
            E::RevisionConflict => errors::ErrorCode::RevisionConflict,
            E::IdempotencyConflict => errors::ErrorCode::IdempotencyConflict,
            E::ResourceLimit => errors::ErrorCode::ResourceLimit,
            E::Cancelled => errors::ErrorCode::Cancelled,
            E::Unavailable => errors::ErrorCode::Unavailable,
            E::WriteOutcomeUnknown => errors::ErrorCode::WriteOutcomeUnknown,
            E::InternalError => errors::ErrorCode::InternalError,
        },
        diagnostic_id: error.diagnostic_id,
    }
}

fn availability(value: alcomd_application::McpTransportAvailability) -> wit::Availability {
    use alcomd_application::McpTransportAvailability as A;
    match value {
        A::Available => wit::Availability::Available,
        A::Stale => wit::Availability::Stale,
        A::Unavailable => wit::Availability::Unavailable,
    }
}
impl wit::Host for HostState {
    async fn snapshot(
        &mut self,
        client_id: Option<String>,
        limit: Option<u32>,
    ) -> Result<wit::TransportSnapshot, errors::ManagementError> {
        let response = self
            .protocol
            .capability(
                "host-mcp-transport.snapshot",
                json!({"clientId":client_id,"limit":limit}),
            )
            .map_err(|error| {
                use super::types::ErrorCode;
                failure(
                    match error.code {
                        ErrorCode::PermissionDenied => E::PermissionDenied,
                        ErrorCode::ScopeDenied => E::ScopeDenied,
                        ErrorCode::LeaseExpired => E::LeaseExpired,
                        ErrorCode::LeaseRevoked => E::LeaseRevoked,
                        ErrorCode::Cancelled => E::Cancelled,
                        ErrorCode::ResourceLimit => E::ResourceLimit,
                        _ => E::Unavailable,
                    }
                    .into(),
                )
            })?;
        if let Some(error) = response.get("error") {
            return Err(failure(
                serde_json::from_value(error.clone()).unwrap_or_else(|_| E::InternalError.into()),
            ));
        }
        let value = response
            .get("ok")
            .ok_or_else(|| failure(E::InternalError.into()))?;
        if serde_json::to_vec(value).map_or(true, |v| v.len() > MCP_TRANSPORT_BYTES) {
            return Err(failure(E::ResourceLimit.into()));
        }
        let snapshot: McpTransportSnapshot =
            serde_json::from_value(value.clone()).map_err(|_| failure(E::InternalError.into()))?;
        Ok(project(snapshot))
    }
}
fn project(s: McpTransportSnapshot) -> wit::TransportSnapshot {
    use alcomd_application::{
        McpTransportConnectionStage as C, McpTransportKind as K, McpTransportMethod as M,
        McpTransportRequestStage as R,
    };
    wit::TransportSnapshot {
        instance_id: s.instance_id,
        observed_at_ms: s.observed_at_ms,
        status: availability(s.status),
        can_read_connections: s.can_read_connections,
        can_read_requests: s.can_read_requests,
        can_read_subscription_streams: s.can_read_subscription_streams,
        coverage_incomplete: s.coverage_incomplete,
        observers: s
            .observers
            .into_iter()
            .map(|o| wit::ObserverStatus {
                observer_id: o.observer_id,
                transport: match o.transport {
                    K::Stdio => wit::TransportKind::Stdio,
                    K::Http => wit::TransportKind::Http,
                    K::HttpsOauth => wit::TransportKind::HttpsOauth,
                },
                last_report_at_ms: o.last_report_at_ms,
                status: availability(o.status),
            })
            .collect(),
        connections: s.connections.map(|p| wit::ConnectionPage {
            total_tracked: p.total_tracked,
            truncated: p.truncated,
            items: p
                .items
                .into_iter()
                .map(|r| wit::ConnectionRow {
                    connection_id: r.connection_id,
                    observer_id: r.observer_id,
                    source: wit::ReportSource::AdapterReported,
                    first_observed_at_ms: r.first_observed_at_ms,
                    stage: match r.stage {
                        C::Accepted => wit::ConnectionStage::Accepted,
                        C::Tls => wit::ConnectionStage::Tls,
                        C::Idle => wit::ConnectionStage::Idle,
                        C::Active => wit::ConnectionStage::Active,
                    },
                })
                .collect(),
        }),
        requests: s.requests.map(|p| wit::RequestPage {
            total_tracked: p.total_tracked,
            truncated: p.truncated,
            items: p
                .items
                .into_iter()
                .map(|r| wit::RequestRow {
                    request_id: r.request_id,
                    connection_id: r.connection_id,
                    observer_id: r.observer_id,
                    source: wit::ReportSource::AdapterReported,
                    first_observed_at_ms: r.first_observed_at_ms,
                    client_id: r.client_id,
                    method: match r.method {
                        M::Discover => wit::RequestMethod::Discover,
                        M::ListTools => wit::RequestMethod::ListTools,
                        M::CallTool => wit::RequestMethod::CallTool,
                        M::Listen => wit::RequestMethod::Listen,
                    },
                    stage: match r.stage {
                        R::Receiving => wit::RequestStage::Receiving,
                        R::Executing => wit::RequestStage::Executing,
                        R::Streaming => wit::RequestStage::Streaming,
                    },
                })
                .collect(),
        }),
        subscription_streams: s.subscription_streams.map(|p| wit::StreamPage {
            total_tracked: p.total_tracked,
            truncated: p.truncated,
            items: p
                .items
                .into_iter()
                .map(|r| wit::StreamRow {
                    stream_id: r.stream_id,
                    request_id: r.request_id,
                    observer_id: r.observer_id,
                    source: wit::ReportSource::AdapterReported,
                    first_observed_at_ms: r.first_observed_at_ms,
                    client_id: r.client_id,
                    tools_list_changed: r.tools_list_changed,
                })
                .collect(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_projection_and_wit_omit_denied_groups_and_secrets() {
        let registry = alcomd_application::McpTransportObservations::new();
        let snapshot = registry
            .snapshot(
                Default::default(),
                alcomd_application::TransportReadAccess {
                    connections: false,
                    requests: false,
                    streams: true,
                },
                10,
                std::time::Instant::now(),
            )
            .expect("stream-only projection");
        let value = serde_json::to_value(&snapshot).expect("metadata");
        for source in [
            include_str!("../../../specs/rpc/m8-mcp-transport-v1.schema.json"),
            include_str!("../../../specs/extensions/mcp-transport-v1.schema.json"),
        ] {
            let mut schema: serde_json::Value =
                serde_json::from_str(source).expect("closed schema");
            schema["$ref"] = json!("#/$defs/snapshot");
            let validator = jsonschema::options()
                .with_draft(jsonschema::Draft::Draft202012)
                .build(&schema)
                .expect("local schema");
            assert!(validator.is_valid(&value));
            for forbidden in ["ticket", "credential", "params", "headers"] {
                let mut forged = value.clone();
                forged[forbidden] = json!("forbidden");
                assert!(!validator.is_valid(&forged));
            }
            let mut denied = value.clone();
            denied["requests"] = json!({"items":[],"totalTracked":0,"truncated":false});
            assert!(
                !validator.is_valid(&denied),
                "denied is omitted, never an empty readable group"
            );
        }
        let projected = project(snapshot);
        assert!(projected.connections.is_none() && projected.requests.is_none());
        assert!(projected.subscription_streams.is_some());
        assert!(matches!(projected.status, wit::Availability::Unavailable));
        assert!(projected.coverage_incomplete);
    }
}
