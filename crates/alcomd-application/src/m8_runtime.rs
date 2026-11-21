//! Ephemeral Core IPC observations; never a credential, session or Operation owner.
use crate::{
    AccessContext, ExtensionInstanceLease, ExtensionUiInvocationAuthority, ManagementError,
    ManagementErrorCode as E, McpClientId, OperationId, Permission,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    future::Future,
    sync::{Arc, Mutex},
};

pub const MCP_RUNTIME_BYTES: usize = 65_536;
pub const MCP_RUNTIME_CONNECTION_LIMIT: usize = 4096;
pub const MCP_RUNTIME_INTERFACE: &str = "alcomd:mcp-runtime/observations@1.0.0";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeSnapshotQuery {
    pub client_id: Option<String>,
    pub limit: Option<u32>,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct RuntimeReadAccess {
    pub connections: bool,
    pub requests: bool,
}
impl RuntimeReadAccess {
    pub fn from_client(access: &AccessContext) -> Self {
        Self {
            connections: access.require(Permission::McpConnectionsRead).is_ok(),
            requests: access.require(Permission::McpRequestsRead).is_ok(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeConnection {
    pub connection_id: String,
    pub client_id: McpClientId,
    pub connected_at_ms: u64,
    pub active_call_count: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeCall {
    pub call_id: String,
    pub connection_id: String,
    pub client_id: McpClientId,
    pub method: ObservedCoreMethod,
    pub started_at_ms: u64,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ObservedCoreMethod {
    #[serde(rename = "mcp.tools.catalog")]
    ToolCatalog,
    #[serde(rename = "extensions.mcpTools.call")]
    ExtensionTool,
    #[serde(rename = "system.status")]
    Status,
    #[serde(rename = "projects.get")]
    ProjectGet,
    #[serde(rename = "projects.list")]
    ProjectList,
    #[serde(rename = "state.check")]
    StateCheck,
    #[serde(rename = "operations.get")]
    OperationGet,
    #[serde(rename = "operations.list")]
    OperationList,
    #[serde(rename = "operations.cancel")]
    OperationCancel,
    #[serde(rename = "events.list")]
    EventList,
    #[serde(rename = "packages.planRemove")]
    PackagePlanRemove,
    #[serde(rename = "packages.applyPlan")]
    PackageApply,
}
impl ObservedCoreMethod {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ToolCatalog => "mcp.tools.catalog",
            Self::ExtensionTool => "extensions.mcpTools.call",
            Self::Status => "system.status",
            Self::ProjectGet => "projects.get",
            Self::ProjectList => "projects.list",
            Self::StateCheck => "state.check",
            Self::OperationGet => "operations.get",
            Self::OperationList => "operations.list",
            Self::OperationCancel => "operations.cancel",
            Self::EventList => "events.list",
            Self::PackagePlanRemove => "packages.planRemove",
            Self::PackageApply => "packages.applyPlan",
        }
    }
    pub fn parse_static(method: &str) -> Option<Self> {
        Some(match method {
            "mcp.tools.catalog" => Self::ToolCatalog,
            "extensions.mcpTools.call" => Self::ExtensionTool,
            "system.status" => Self::Status,
            "projects.get" => Self::ProjectGet,
            "projects.list" => Self::ProjectList,
            "state.check" => Self::StateCheck,
            "operations.get" => Self::OperationGet,
            "operations.list" => Self::OperationList,
            "operations.cancel" => Self::OperationCancel,
            "events.list" => Self::EventList,
            "packages.planRemove" => Self::PackagePlanRemove,
            "packages.applyPlan" => Self::PackageApply,
            _ => return None,
        })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeRows<T> {
    pub items: Vec<T>,
    pub total_tracked: u32,
    pub truncated: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeSnapshot {
    pub instance_id: String,
    pub observed_at_ms: u64,
    pub can_read_connections: bool,
    pub can_read_requests: bool,
    pub coverage_incomplete: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connections: Option<RuntimeRows<RuntimeConnection>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requests: Option<RuntimeRows<RuntimeCall>>,
}
pub trait McpRuntimeAuthorityStore: Clone + Send + Sync + 'static {
    fn mcp_runtime_access(
        &self,
        lease: ExtensionInstanceLease,
        authority: ExtensionUiInvocationAuthority,
        now_ms: u64,
    ) -> impl Future<Output = Result<RuntimeReadAccess, ManagementError>> + Send;
}

#[derive(Clone)]
pub struct McpRuntimeObservations {
    instance_id: String,
    state: Arc<Mutex<RuntimeState>>,
}
#[derive(Default)]
struct RuntimeState {
    connections: BTreeMap<String, LiveConnection>,
    omitted_connections: usize,
    omitted_calls: usize,
}
struct LiveConnection {
    record: RuntimeConnection,
    call: Option<RuntimeCall>,
}
pub struct RuntimeConnectionGuard {
    state: Arc<Mutex<RuntimeState>>,
    id: String,
    tracked: bool,
}
pub struct RuntimeCallGuard {
    state: Arc<Mutex<RuntimeState>>,
    connection: String,
    id: Option<String>,
    omitted: bool,
}
impl Default for McpRuntimeObservations {
    fn default() -> Self {
        Self::new()
    }
}
impl McpRuntimeObservations {
    #[must_use]
    pub fn new() -> Self {
        Self {
            instance_id: OperationId::new().to_string(),
            state: Arc::default(),
        }
    }
    /// Only Core's successfully authenticated connection path supplies these identities.
    pub fn connect(
        &self,
        id: String,
        client_id: McpClientId,
        now_ms: u64,
    ) -> RuntimeConnectionGuard {
        let tracked = if let Ok(mut state) = self.state.lock() {
            if state.connections.len() < MCP_RUNTIME_CONNECTION_LIMIT
                && !state.connections.contains_key(&id)
            {
                state.connections.insert(
                    id.clone(),
                    LiveConnection {
                        record: RuntimeConnection {
                            connection_id: id.clone(),
                            client_id,
                            connected_at_ms: now_ms,
                            active_call_count: 0,
                        },
                        call: None,
                    },
                );
                true
            } else {
                state.omitted_connections = state.omitted_connections.saturating_add(1);
                false
            }
        } else {
            false
        };
        RuntimeConnectionGuard {
            state: Arc::clone(&self.state),
            id,
            tracked,
        }
    }
    pub fn begin_call(
        &self,
        connection: &str,
        method: ObservedCoreMethod,
        now_ms: u64,
    ) -> RuntimeCallGuard {
        let mut id = None;
        let mut omitted = false;
        if let Ok(mut state) = self.state.lock()
            && let Some(live) = state.connections.get_mut(connection)
        {
            if live.call.is_none() {
                let call_id = OperationId::new().to_string();
                live.call = Some(RuntimeCall {
                    call_id: call_id.clone(),
                    connection_id: connection.into(),
                    client_id: live.record.client_id,
                    method,
                    started_at_ms: now_ms,
                });
                live.record.active_call_count = 1;
                id = Some(call_id);
            } else {
                state.omitted_calls = state.omitted_calls.saturating_add(1);
                omitted = true;
            }
        }
        RuntimeCallGuard {
            state: Arc::clone(&self.state),
            connection: connection.into(),
            id,
            omitted,
        }
    }
    pub fn snapshot(
        &self,
        query: RuntimeSnapshotQuery,
        access: RuntimeReadAccess,
        now_ms: u64,
    ) -> Result<RuntimeSnapshot, ManagementError> {
        if !access.connections && !access.requests {
            return Err(E::PermissionDenied.into());
        }
        let limit = query.limit.unwrap_or(25);
        if !(1..=100).contains(&limit) || now_ms > i64::MAX as u64 {
            return Err(E::InvalidArgument.into());
        }
        let filter = query
            .client_id
            .map(|id| {
                McpClientId::parse(&id)
                    .ok()
                    .filter(|parsed| parsed.to_string() == id)
                    .ok_or_else(|| ManagementError::from(E::InvalidArgument))
            })
            .transpose()?;
        let state = self
            .state
            .lock()
            .map_err(|_| ManagementError::from(E::Unavailable))?;
        let matching = || {
            state.connections.values().filter(|live| {
                filter
                    .as_ref()
                    .is_none_or(|id| *id == live.record.client_id)
            })
        };
        let mut connections: Vec<_> = if access.connections {
            matching().map(|live| live.record.clone()).collect()
        } else {
            vec![]
        };
        let mut calls: Vec<_> = if access.requests {
            matching().filter_map(|live| live.call.clone()).collect()
        } else {
            vec![]
        };
        connections.sort_by(|a, b| {
            (a.connected_at_ms, &a.connection_id).cmp(&(b.connected_at_ms, &b.connection_id))
        });
        calls.sort_by(|a, b| (a.started_at_ms, &a.call_id).cmp(&(b.started_at_ms, &b.call_id)));
        let mut result = RuntimeSnapshot {
            instance_id: self.instance_id.clone(),
            observed_at_ms: now_ms,
            can_read_connections: access.connections,
            can_read_requests: access.requests,
            coverage_incomplete: state.omitted_connections > 0 || state.omitted_calls > 0,
            connections: access
                .connections
                .then(|| rows(connections, limit as usize)),
            requests: access.requests.then(|| rows(calls, limit as usize)),
        };
        drop(state);
        while serde_json::to_vec(&result)
            .map_err(|_| ManagementError::from(E::InternalError))?
            .len()
            > MCP_RUNTIME_BYTES
        {
            if let Some(rows) = result.requests.as_mut()
                && rows.items.pop().is_some()
            {
                rows.truncated = true;
                continue;
            }
            if let Some(rows) = result.connections.as_mut()
                && rows.items.pop().is_some()
            {
                rows.truncated = true;
                continue;
            }
            return Err(E::ResourceLimit.into());
        }
        Ok(result)
    }
}
fn rows<T>(mut items: Vec<T>, limit: usize) -> RuntimeRows<T> {
    let total_tracked = items.len() as u32;
    let truncated = items.len() > limit;
    items.truncate(limit);
    RuntimeRows {
        items,
        total_tracked,
        truncated,
    }
}
impl Drop for RuntimeConnectionGuard {
    fn drop(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            if self.tracked {
                state.connections.remove(&self.id);
            } else {
                state.omitted_connections = state.omitted_connections.saturating_sub(1);
            }
        }
    }
}
impl Drop for RuntimeCallGuard {
    fn drop(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            if self.omitted {
                state.omitted_calls = state.omitted_calls.saturating_sub(1);
            }
            if let Some(live) = state.connections.get_mut(&self.connection)
                && live
                    .call
                    .as_ref()
                    .is_some_and(|call| Some(&call.call_id) == self.id.as_ref())
            {
                live.call = None;
                live.record.active_call_count = 0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn all() -> RuntimeReadAccess {
        RuntimeReadAccess {
            connections: true,
            requests: true,
        }
    }
    #[test]
    fn observations_are_ephemeral_scoped_and_do_not_turn_calls_into_operations() {
        let runtime = McpRuntimeObservations::new();
        let first = McpClientId::new();
        let second = McpClientId::new();
        let id = OperationId::new().to_string();
        let connection = runtime.connect(id.clone(), first, 1);
        let other = runtime.connect(OperationId::new().to_string(), second, 2);
        let call = runtime.begin_call(&id, ObservedCoreMethod::PackageApply, 3);
        let view = runtime
            .snapshot(
                RuntimeSnapshotQuery {
                    client_id: Some(first.to_string()),
                    limit: None,
                },
                all(),
                4,
            )
            .expect("runtime observation fixture");
        assert_eq!(
            view.connections.expect("runtime observation fixture").items[0].active_call_count,
            1
        );
        assert_eq!(
            view.requests
                .expect("runtime observation fixture")
                .total_tracked,
            1
        );
        drop(call);
        assert_eq!(
            runtime
                .snapshot(Default::default(), all(), 5)
                .expect("runtime observation fixture")
                .requests
                .expect("runtime observation fixture")
                .total_tracked,
            0
        );
        drop(connection);
        drop(other);
        assert_eq!(
            runtime
                .snapshot(Default::default(), all(), 6)
                .expect("runtime observation fixture")
                .connections
                .expect("runtime observation fixture")
                .total_tracked,
            0
        );
        let restarted = McpRuntimeObservations::new();
        assert_ne!(runtime.instance_id, restarted.instance_id);
    }
    #[test]
    fn unavailable_permissions_and_unknown_names_never_expose_rows() {
        let runtime = McpRuntimeObservations::new();
        let _connection = runtime.connect(OperationId::new().to_string(), McpClientId::new(), 1);
        assert!(matches!(
            runtime
                .snapshot(Default::default(), Default::default(), 2)
                .expect_err("ungranted observation read must fail")
                .code,
            E::PermissionDenied
        ));
        let only = runtime
            .snapshot(
                Default::default(),
                RuntimeReadAccess {
                    connections: true,
                    requests: false,
                },
                2,
            )
            .expect("runtime observation fixture");
        assert!(only.requests.is_none());
        assert!(!only.can_read_requests);
        let value = serde_json::to_value(only).expect("runtime observation fixture");
        assert!(value.get("requests").is_none());
        for key in [
            "credential",
            "hash",
            "params",
            "headers",
            "requestId",
            "response",
        ] {
            assert!(!value.to_string().contains(key));
        }
        assert!(ObservedCoreMethod::parse_static("secret-user-controlled-method").is_none());
        assert!(
            runtime
                .snapshot(
                    RuntimeSnapshotQuery {
                        client_id: Some("invalid".into()),
                        limit: None
                    },
                    all(),
                    2
                )
                .is_err()
        );
        assert!(
            runtime
                .snapshot(
                    RuntimeSnapshotQuery {
                        client_id: None,
                        limit: Some(0)
                    },
                    all(),
                    2
                )
                .is_err()
        );
    }
    #[test]
    fn connected_client_can_observe_runtime() {
        let id = crate::McpClientId::new();
        let client = crate::McpClient {
            client_id: id,
            principal_id: alcomd_domain::PrincipalId::parse(format!("mcp:{id}"))
                .expect("principal"),
            display_name: "test".into(),
            revision: 1,
            created_at_ms: 0,
            revoked_at_ms: None,
        };
        let access = crate::mcp_access(&client).expect("client");
        for permission in [
            alcomd_domain::Permission::McpConnectionsRead,
            alcomd_domain::Permission::McpRequestsRead,
            alcomd_domain::Permission::McpSubscriptionStreamsRead,
        ] {
            assert!(access.require(permission).is_ok());
        }
    }
    #[test]
    fn overflow_is_explicit_bounded_and_clears_when_untracked_connection_closes() {
        let runtime = McpRuntimeObservations::new();
        let client = McpClientId::new();
        let connections: Vec<_> = (0..MCP_RUNTIME_CONNECTION_LIMIT)
            .map(|_| runtime.connect(OperationId::new().to_string(), client, 1))
            .collect();
        let excess = runtime.connect(OperationId::new().to_string(), client, 1);
        let view = runtime
            .snapshot(
                RuntimeSnapshotQuery {
                    client_id: None,
                    limit: Some(1),
                },
                all(),
                2,
            )
            .expect("runtime observation fixture");
        assert!(view.coverage_incomplete);
        assert_eq!(
            view.connections
                .as_ref()
                .expect("runtime observation fixture")
                .total_tracked,
            4096
        );
        assert!(
            view.connections
                .as_ref()
                .expect("runtime observation fixture")
                .truncated
        );
        assert_eq!(
            view.connections
                .as_ref()
                .expect("runtime observation fixture")
                .items
                .len(),
            1
        );
        assert!(
            serde_json::to_vec(&view)
                .expect("runtime observation fixture")
                .len()
                <= MCP_RUNTIME_BYTES
        );
        drop(excess);
        assert!(
            !runtime
                .snapshot(Default::default(), all(), 3)
                .expect("runtime observation fixture")
                .coverage_incomplete
        );
        drop(connections);
    }
}
