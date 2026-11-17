use super::HostState;
use alcomd_application::{
    MCP_RUNTIME_BYTES, ManagementError, ManagementErrorCode as E, RuntimeSnapshot,
};
use serde_json::json;
mod bindings {
    wasmtime::component::bindgen!({
        path: "../../specs/extensions/wit/mcp-runtime-v1",
        world: "runtime-v1",
        imports: { default: async }, exports: { default: async },
    });
}
use bindings::alcomd::{mcp_management::clients as errors, mcp_runtime::observations as wit};
pub(super) fn add_to_linker(
    linker: &mut wasmtime::component::Linker<HostState>,
) -> anyhow::Result<()> {
    wit::add_to_linker::<_, wasmtime::component::HasSelf<_>>(linker, |state| state)
        .map_err(|_| anyhow::anyhow!("Core observation interface link failed"))
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
impl wit::Host for HostState {
    async fn snapshot(
        &mut self,
        client_id: Option<String>,
        limit: Option<u32>,
    ) -> Result<wit::RuntimeSnapshot, errors::ManagementError> {
        let response = self
            .protocol
            .capability(
                "host-mcp-runtime.snapshot",
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
        if serde_json::to_vec(value).map_or(true, |bytes| bytes.len() > MCP_RUNTIME_BYTES) {
            return Err(failure(E::ResourceLimit.into()));
        }
        let snapshot: RuntimeSnapshot =
            serde_json::from_value(value.clone()).map_err(|_| failure(E::InternalError.into()))?;
        Ok(wit::RuntimeSnapshot {
            instance_id: snapshot.instance_id,
            observed_at_ms: snapshot.observed_at_ms,
            can_read_connections: snapshot.can_read_connections,
            can_read_requests: snapshot.can_read_requests,
            coverage_incomplete: snapshot.coverage_incomplete,
            connections: snapshot.connections.map(|page| wit::ConnectionPage {
                items: page
                    .items
                    .into_iter()
                    .map(|row| wit::ConnectionRow {
                        connection_id: row.connection_id,
                        client_id: row.client_id.to_string(),
                        connected_at_ms: row.connected_at_ms,
                        active_call_count: row.active_call_count,
                    })
                    .collect(),
                total_tracked: page.total_tracked,
                truncated: page.truncated,
            }),
            requests: snapshot.requests.map(|page| wit::RequestPage {
                items: page
                    .items
                    .into_iter()
                    .map(|row| wit::RequestRow {
                        call_id: row.call_id,
                        connection_id: row.connection_id,
                        client_id: row.client_id.to_string(),
                        method: row.method.as_str().into(),
                        started_at_ms: row.started_at_ms,
                    })
                    .collect(),
                total_tracked: page.total_tracked,
                truncated: page.truncated,
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_projection_matches_public_schemas_and_rejects_secret_fields() {
        use alcomd_application::{
            McpClientId, McpRuntimeObservations, ObservedCoreMethod, OperationId, RuntimeReadAccess,
        };
        let runtime = McpRuntimeObservations::new();
        let id = OperationId::new().to_string();
        let _connection = runtime.connect(id.clone(), McpClientId::new(), 1);
        let _call = runtime.begin_call(&id, ObservedCoreMethod::PackageApply, 2);
        let value = serde_json::to_value(
            runtime
                .snapshot(
                    Default::default(),
                    RuntimeReadAccess {
                        connections: true,
                        requests: true,
                    },
                    3,
                )
                .expect("real Core projection"),
        )
        .expect("serializable metadata");
        for source in [
            include_str!("../../../specs/rpc/m8-mcp-runtime-v1.schema.json"),
            include_str!("../../../specs/extensions/mcp-runtime-v1.schema.json"),
        ] {
            let mut schema: serde_json::Value =
                serde_json::from_str(source).expect("public Schema");
            schema["$ref"] = json!("#/$defs/snapshot");
            let validator = jsonschema::options()
                .with_draft(jsonschema::Draft::Draft202012)
                .build(&schema)
                .expect("closed local Schema");
            assert!(validator.is_valid(&value));
            let mut secret = value.clone();
            secret["credential"] = json!("synthetic-forbidden-field");
            assert!(!validator.is_valid(&secret));
            let mut inconsistent = value.clone();
            inconsistent["canReadRequests"] = json!(false);
            assert!(
                !validator.is_valid(&inconsistent),
                "a denied group must be omitted"
            );
        }
    }
}
