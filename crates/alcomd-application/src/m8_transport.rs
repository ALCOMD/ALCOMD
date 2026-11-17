//! Bounded, transient adapter reports. Only Core authentication can attach a client.
use crate::m8_transport_types::*;
use crate::{
    AccessContext, ManagementError, ManagementErrorCode as E, McpClientId, OperationId, Permission,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub const MCP_TRANSPORT_INTERFACE: &str = "alcomd:mcp-transport/observations@1.0.0";
pub const MCP_TRANSPORT_BYTES: usize = 65_536;
pub const MCP_TRANSPORT_ROWS: usize = 4096;

#[derive(Clone, Copy, Debug, Default)]
pub struct TransportReadAccess {
    pub connections: bool,
    pub requests: bool,
    pub streams: bool,
}
impl TransportReadAccess {
    pub fn from_client(access: &AccessContext) -> Self {
        if access.is_mcp_client() {
            return Self::default();
        }
        Self {
            connections: access.require(Permission::McpConnectionsRead).is_ok(),
            requests: access.require(Permission::McpRequestsRead).is_ok(),
            streams: access
                .require(Permission::McpSubscriptionStreamsRead)
                .is_ok(),
        }
    }
}
pub trait McpTransportAuthorityStore: Clone + Send + Sync + 'static {
    fn mcp_transport_access(
        &self,
        lease: crate::ExtensionInstanceLease,
        authority: crate::ExtensionUiInvocationAuthority,
        now_ms: u64,
    ) -> impl Future<Output = Result<TransportReadAccess, ManagementError>> + Send;
}

#[derive(Clone)]
pub struct McpTransportObservations {
    instance: String,
    state: Arc<Mutex<BTreeMap<String, Observer>>>,
}
struct Observer {
    id: String,
    kind: McpTransportKind,
    sequence: u64,
    last: Instant,
    last_ms: u64,
    incomplete: bool,
    connections: BTreeMap<String, McpTransportConnectionRow>,
    requests: BTreeMap<String, Request>,
    streams: BTreeMap<String, McpTransportStreamRow>,
}
struct Request {
    input: McpTransportRequestInput,
    row: McpTransportRequestRow,
    ticket: Option<([u8; 32], Instant)>,
}
fn id(value: &str) -> bool {
    OperationId::parse(value).is_ok_and(|id| id.to_string() == value)
}
fn ids_unique<'a>(values: impl Iterator<Item = &'a str>) -> bool {
    let mut seen = BTreeSet::new();
    values.into_iter().all(|v| id(v) && seen.insert(v))
}
fn valid(report: &McpTransportReport) -> bool {
    (1..=9_007_199_254_740_991).contains(&report.sequence)
        && report.connections.len() <= 128
        && report.requests.len() <= 128
        && report.subscription_streams.len() <= 32
        && serde_json::to_vec(report).is_ok_and(|v| v.len() <= MCP_TRANSPORT_BYTES)
        && ids_unique(report.connections.iter().map(|r| r.local_ref.as_str()))
        && ids_unique(report.requests.iter().map(|r| r.local_ref.as_str()))
        && ids_unique(
            report
                .subscription_streams
                .iter()
                .map(|r| r.local_ref.as_str()),
        )
        && report.requests.iter().all(|r| {
            report
                .connections
                .iter()
                .any(|c| c.local_ref == r.connection_ref)
        })
        && report.subscription_streams.iter().all(|s| {
            s.tools_list_changed
                && report.requests.iter().any(|r| {
                    r.local_ref == s.request_ref
                        && r.method == McpTransportMethod::Listen
                        && r.stage == McpTransportRequestStage::Streaming
                })
        })
}
fn expire_rows(state: &mut BTreeMap<String, Observer>, now: Instant) {
    for observer in state
        .values_mut()
        .filter(|o| now.saturating_duration_since(o.last) >= Duration::from_secs(5))
    {
        observer.connections.clear();
        observer.requests.clear();
        observer.streams.clear();
    }
}
impl Default for McpTransportObservations {
    fn default() -> Self {
        Self::new()
    }
}
impl McpTransportObservations {
    pub fn new() -> Self {
        Self {
            instance: OperationId::new().to_string(),
            state: Arc::default(),
        }
    }
    pub fn forget(&self, channel: &str) {
        if let Ok(mut state) = self.state.lock() {
            state.remove(channel);
        }
    }
    pub fn report(
        &self,
        channel: &str,
        report: McpTransportReport,
        now_ms: u64,
        now: Instant,
        mut entropy: impl FnMut(&mut [u8; 32]) -> Result<(), ()>,
    ) -> Result<McpTransportReportResult, ManagementError> {
        if !valid(&report) {
            return Err(E::InvalidArgument.into());
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| ManagementError::from(E::Unavailable))?;
        expire_rows(&mut state, now);
        let old = state.get(channel);
        if let Some(old) = old {
            if report.sequence <= old.sequence {
                return Err(E::RevisionConflict.into());
            }
            if old.kind != report.transport
                || now.saturating_duration_since(old.last) < Duration::from_secs(1)
            {
                return Err(E::InvalidArgument.into());
            }
            for input in &report.requests {
                if let Some(previous) = old.requests.get(&input.local_ref)
                    && (previous.input.method != input.method
                        || previous.input.connection_ref != input.connection_ref
                        || input.stage < previous.input.stage)
                {
                    return Err(E::InvalidArgument.into());
                }
            }
        } else if state.len() >= 64 {
            return Err(E::ResourceLimit.into());
        }
        let observer_id = old
            .map(|v| v.id.clone())
            .unwrap_or_else(|| OperationId::new().to_string());
        let other_count: usize = state
            .iter()
            .filter(|(key, _)| key.as_str() != channel)
            .filter(|(_, v)| now.saturating_duration_since(v.last) < Duration::from_secs(5))
            .map(|(_, v)| v.connections.len() + v.requests.len() + v.streams.len())
            .sum();
        let over_capacity = other_count
            + report.connections.len()
            + report.requests.len()
            + report.subscription_streams.len()
            > MCP_TRANSPORT_ROWS;
        let mut next = Observer {
            id: observer_id.clone(),
            kind: report.transport,
            sequence: report.sequence,
            last: now,
            last_ms: now_ms,
            incomplete: report.coverage_incomplete || over_capacity,
            connections: BTreeMap::new(),
            requests: BTreeMap::new(),
            streams: BTreeMap::new(),
        };
        let mut bindings = Vec::new();
        if !over_capacity {
            for input in report.connections {
                let mut row = old
                    .and_then(|o| o.connections.get(&input.local_ref))
                    .cloned()
                    .unwrap_or_else(|| McpTransportConnectionRow {
                        connection_id: OperationId::new().to_string(),
                        observer_id: observer_id.clone(),
                        source: McpTransportSource::AdapterReported,
                        first_observed_at_ms: now_ms,
                        stage: input.stage,
                    });
                row.stage = input.stage;
                next.connections.insert(input.local_ref, row);
            }
            for input in report.requests {
                let connection_id = next.connections[&input.connection_ref]
                    .connection_id
                    .clone();
                let (mut row, ticket) =
                    if let Some(previous) = old.and_then(|o| o.requests.get(&input.local_ref)) {
                        (previous.row.clone(), previous.ticket)
                    } else {
                        let mut bytes = [0; 32];
                        entropy(&mut bytes).map_err(|_| ManagementError::from(E::Unavailable))?;
                        let ticket = crate::oauth_hex(&bytes);
                        bindings.push(McpTransportBinding {
                            request_ref: input.local_ref.clone(),
                            ticket: ticket.clone(),
                        });
                        (
                            McpTransportRequestRow {
                                request_id: OperationId::new().to_string(),
                                connection_id,
                                observer_id: observer_id.clone(),
                                source: McpTransportSource::AdapterReported,
                                first_observed_at_ms: now_ms,
                                method: input.method,
                                stage: input.stage,
                                client_id: None,
                            },
                            Some((crate::mcp_credential_hash(&ticket), now)),
                        )
                    };
                row.stage = input.stage;
                next.requests
                    .insert(input.local_ref.clone(), Request { input, row, ticket });
            }
            for input in report.subscription_streams {
                let request = &next.requests[&input.request_ref];
                let row = McpTransportStreamRow {
                    stream_id: old
                        .and_then(|o| o.streams.get(&input.local_ref))
                        .filter(|r| r.request_id == request.row.request_id)
                        .map(|r| r.stream_id.clone())
                        .unwrap_or_else(|| OperationId::new().to_string()),
                    request_id: request.row.request_id.clone(),
                    observer_id: observer_id.clone(),
                    source: McpTransportSource::AdapterReported,
                    first_observed_at_ms: old
                        .and_then(|o| o.streams.get(&input.local_ref))
                        .filter(|s| s.request_id == request.row.request_id)
                        .map_or(now_ms, |s| s.first_observed_at_ms),
                    tools_list_changed: true,
                    client_id: request.row.client_id.clone(),
                };
                next.streams.insert(input.local_ref, row);
            }
        }
        let incomplete = next.incomplete;
        state.insert(channel.to_owned(), next);
        Ok(McpTransportReportResult {
            observer_id,
            request_bindings: bindings,
            coverage_incomplete: incomplete,
        })
    }
    pub fn bind(
        &self,
        ticket: &str,
        access: &AccessContext,
        now: Instant,
    ) -> Result<McpTransportBindResult, ManagementError> {
        let client = access
            .principal()
            .as_str()
            .strip_prefix("mcp:")
            .and_then(|s| McpClientId::parse(s).ok())
            .ok_or_else(|| ManagementError::from(E::PermissionDenied))?;
        if ticket.len() != 64
            || !ticket
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(E::PermissionDenied.into());
        }
        let hash = crate::mcp_credential_hash(ticket);
        let mut state = self
            .state
            .lock()
            .map_err(|_| ManagementError::from(E::Unavailable))?;
        expire_rows(&mut state, now);
        for observer in state.values_mut() {
            if now.saturating_duration_since(observer.last) >= Duration::from_secs(5) {
                continue;
            }
            for request in observer.requests.values_mut() {
                if let Some((expected, issued)) = request.ticket
                    && now.saturating_duration_since(issued) < Duration::from_secs(10)
                    && hash
                        .iter()
                        .zip(expected)
                        .fold(0u8, |different, (a, b)| different | (*a ^ b))
                        == 0
                {
                    request.ticket = None;
                    request.row.client_id = Some(client.to_string());
                    for stream in observer
                        .streams
                        .values_mut()
                        .filter(|s| s.request_id == request.row.request_id)
                    {
                        stream.client_id = Some(client.to_string());
                    }
                    return Ok(McpTransportBindResult { bound: true });
                }
            }
        }
        Err(E::PermissionDenied.into())
    }
    pub fn snapshot(
        &self,
        query: McpTransportQuery,
        access: TransportReadAccess,
        now_ms: u64,
        now: Instant,
    ) -> Result<McpTransportSnapshot, ManagementError> {
        if !access.connections && !access.requests && !access.streams {
            return Err(E::PermissionDenied.into());
        }
        let limit = query.limit.unwrap_or(25) as usize;
        if !(1..=100).contains(&limit)
            || query
                .client_id
                .as_ref()
                .is_some_and(|id| McpClientId::parse(id).is_err())
        {
            return Err(E::InvalidArgument.into());
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| ManagementError::from(E::Unavailable))?;
        expire_rows(&mut state, now);
        let mut observers = Vec::new();
        let mut connections = Vec::new();
        let mut requests = Vec::new();
        let mut streams = Vec::new();
        let mut fresh = false;
        let mut stale = false;
        let mut incomplete = false;
        for observer in state.values() {
            let age = now.saturating_duration_since(observer.last);
            let status = if age >= Duration::from_secs(5) {
                McpTransportAvailability::Unavailable
            } else if age > Duration::from_secs(2) {
                McpTransportAvailability::Stale
            } else {
                McpTransportAvailability::Available
            };
            fresh |= status == McpTransportAvailability::Available;
            stale |= status == McpTransportAvailability::Stale;
            incomplete |= observer.incomplete || status != McpTransportAvailability::Available;
            observers.push(McpTransportObserverStatus {
                observer_id: observer.id.clone(),
                transport: observer.kind,
                last_report_at_ms: observer.last_ms,
                status,
            });
            if status != McpTransportAvailability::Available {
                continue;
            }
            let matches = |id: &Option<String>| {
                query
                    .client_id
                    .as_ref()
                    .is_none_or(|filter| id.as_ref() == Some(filter))
            };
            let selected: Vec<_> = observer
                .requests
                .values()
                .filter(|r| matches(&r.row.client_id))
                .collect();
            for connection in observer.connections.values().filter(|c| {
                query.client_id.is_none()
                    || selected
                        .iter()
                        .any(|r| r.row.connection_id == c.connection_id)
            }) {
                connections.push(connection.clone());
            }
            requests.extend(selected.into_iter().map(|r| r.row.clone()));
            streams.extend(
                observer
                    .streams
                    .values()
                    .filter(|s| matches(&s.client_id))
                    .cloned(),
            );
        }
        connections.sort_by(|a, b| {
            (a.first_observed_at_ms, &a.connection_id)
                .cmp(&(b.first_observed_at_ms, &b.connection_id))
        });
        requests.sort_by(|a, b| {
            (a.first_observed_at_ms, &a.request_id).cmp(&(b.first_observed_at_ms, &b.request_id))
        });
        streams.sort_by(|a, b| {
            (a.first_observed_at_ms, &a.stream_id).cmp(&(b.first_observed_at_ms, &b.stream_id))
        });
        let page = |count: usize| McpTransportRows::<()> {
            items: Vec::new(),
            total_tracked: count as u32,
            truncated: count > limit,
        };
        let c = page(connections.len());
        let r = page(requests.len());
        let s = page(streams.len());
        connections.truncate(limit);
        requests.truncate(limit);
        streams.truncate(limit);
        let mut snapshot = McpTransportSnapshot {
            instance_id: self.instance.clone(),
            observed_at_ms: now_ms,
            status: if fresh {
                McpTransportAvailability::Available
            } else if stale {
                McpTransportAvailability::Stale
            } else {
                McpTransportAvailability::Unavailable
            },
            can_read_connections: access.connections,
            can_read_requests: access.requests,
            can_read_subscription_streams: access.streams,
            coverage_incomplete: incomplete || state.is_empty(),
            observers,
            connections: access.connections.then_some(McpTransportRows {
                items: connections,
                total_tracked: c.total_tracked,
                truncated: c.truncated,
            }),
            requests: access.requests.then_some(McpTransportRows {
                items: requests,
                total_tracked: r.total_tracked,
                truncated: r.truncated,
            }),
            subscription_streams: access.streams.then_some(McpTransportRows {
                items: streams,
                total_tracked: s.total_tracked,
                truncated: s.truncated,
            }),
        };
        while serde_json::to_vec(&snapshot)
            .map_err(|_| ManagementError::from(E::Unavailable))?
            .len()
            > MCP_TRANSPORT_BYTES
        {
            // Never truncate a row or expose data from an unreadable group.
            if let Some(page) = snapshot.requests.as_mut().filter(|p| !p.items.is_empty()) {
                page.items.pop();
                page.truncated = true;
            } else if let Some(page) = snapshot
                .subscription_streams
                .as_mut()
                .filter(|p| !p.items.is_empty())
            {
                page.items.pop();
                page.truncated = true;
            } else if let Some(page) = snapshot
                .connections
                .as_mut()
                .filter(|p| !p.items.is_empty())
            {
                page.items.pop();
                page.truncated = true;
            } else {
                return Err(E::ResourceLimit.into());
            }
        }
        Ok(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> McpTransportReport {
        let c = OperationId::new().to_string();
        let r = OperationId::new().to_string();
        McpTransportReport {
            sequence: 1,
            transport: McpTransportKind::Http,
            connections: vec![McpTransportConnectionInput {
                local_ref: c.clone(),
                stage: McpTransportConnectionStage::Active,
            }],
            requests: vec![McpTransportRequestInput {
                local_ref: r,
                connection_ref: c,
                method: McpTransportMethod::CallTool,
                stage: McpTransportRequestStage::Executing,
            }],
            subscription_streams: vec![],
            coverage_incomplete: false,
        }
    }
    fn all() -> TransportReadAccess {
        TransportReadAccess {
            connections: true,
            requests: true,
            streams: true,
        }
    }
    fn client() -> AccessContext {
        AccessContext::new(
            crate::PrincipalId::parse(format!("mcp:{}", McpClientId::new())).expect("identity"),
            [],
        )
    }
    fn report(
        registry: &McpTransportObservations,
        channel: &str,
        value: McpTransportReport,
        now: Instant,
    ) -> McpTransportReportResult {
        let mut n = 0u64;
        registry
            .report(channel, value, 100, now, |bytes| {
                n += 1;
                bytes[..8].copy_from_slice(&n.to_le_bytes());
                Ok(())
            })
            .expect("report")
    }
    #[test]
    fn only_current_core_identity_binds_once_and_tcp_has_no_owner() {
        let registry = McpTransportObservations::new();
        let now = Instant::now();
        let mut value = fixture();
        let mut second = value.requests[0].clone();
        second.local_ref = OperationId::new().to_string();
        value.requests.push(second);
        let response = report(&registry, "one", value.clone(), now);
        let a = client();
        let b = client();
        assert_eq!(
            registry
                .bind(
                    &response.request_bindings[0].ticket,
                    &AccessContext::local_owner(),
                    now
                )
                .expect_err("owner cannot claim identity")
                .code,
            E::PermissionDenied
        );
        registry
            .bind(&response.request_bindings[0].ticket, &a, now)
            .expect("actual A");
        registry
            .bind(&response.request_bindings[1].ticket, &b, now)
            .expect("actual B");
        assert!(
            registry
                .bind(&response.request_bindings[0].ticket, &b, now)
                .is_err()
        );
        let snapshot = registry
            .snapshot(Default::default(), all(), 100, now)
            .expect("snapshot");
        assert_eq!(
            snapshot
                .connections
                .as_ref()
                .expect("connections")
                .items
                .len(),
            1
        );
        let rows = &snapshot.requests.as_ref().expect("requests").items;
        assert_eq!(
            rows.iter()
                .filter(|r| r.client_id.as_deref()
                    == Some(a.principal().as_str().strip_prefix("mcp:").expect("A")))
                .count(),
            1
        );
        assert_eq!(
            rows.iter()
                .filter(|r| r.client_id.as_deref()
                    == Some(b.principal().as_str().strip_prefix("mcp:").expect("B")))
                .count(),
            1
        );
        let raw = serde_json::to_string(&snapshot).expect("projection");
        for binding in response.request_bindings {
            assert!(!raw.contains(&binding.ticket));
            assert!(!format!("{binding:?}").contains(&binding.ticket));
        }
        let mut forged = serde_json::to_value(value).expect("input");
        forged["requests"][0]["clientId"] = serde_json::json!("claimed-owner");
        assert!(serde_json::from_value::<McpTransportReport>(forged).is_err());
    }
    #[test]
    fn order_validation_and_entropy_failure_do_not_partially_replace_state() {
        let registry = McpTransportObservations::new();
        let now = Instant::now();
        let mut value = fixture();
        report(&registry, "one", value.clone(), now);
        assert_eq!(
            registry
                .report(
                    "one",
                    value.clone(),
                    200,
                    now + Duration::from_secs(1),
                    |_| Ok(())
                )
                .expect_err("sequence")
                .code,
            E::RevisionConflict
        );
        value.sequence = 2;
        value.requests[0].local_ref = OperationId::new().to_string();
        assert!(
            registry
                .report(
                    "one",
                    value.clone(),
                    200,
                    now + Duration::from_secs(1),
                    |_| Err(())
                )
                .is_err()
        );
        let snapshot = registry
            .snapshot(Default::default(), all(), 200, now + Duration::from_secs(1))
            .expect("original snapshot");
        assert_eq!(snapshot.requests.expect("requests").total_tracked, 1);
        value.requests[0].connection_ref = OperationId::new().to_string();
        assert!(
            registry
                .report("one", value, 200, now + Duration::from_secs(1), |_| Ok(()))
                .is_err()
        );
    }
    #[test]
    fn stale_unavailable_disconnect_and_restart_never_mean_zero_activity_success() {
        let registry = McpTransportObservations::new();
        let now = Instant::now();
        report(&registry, "one", fixture(), now);
        let initial = registry
            .snapshot(Default::default(), all(), 100, now)
            .expect("snapshot");
        let stale = registry
            .snapshot(
                Default::default(),
                all(),
                3100,
                now + Duration::from_secs(3),
            )
            .expect("stale");
        assert_eq!(stale.status, McpTransportAvailability::Stale);
        assert!(stale.coverage_incomplete);
        assert!(stale.requests.expect("requests").items.is_empty());
        assert_eq!(
            registry
                .snapshot(
                    Default::default(),
                    all(),
                    6100,
                    now + Duration::from_secs(6)
                )
                .expect("expired")
                .status,
            McpTransportAvailability::Unavailable
        );
        registry.forget("one");
        assert_eq!(
            registry
                .snapshot(Default::default(), all(), 100, now)
                .expect("disconnected")
                .status,
            McpTransportAvailability::Unavailable
        );
        assert_ne!(
            McpTransportObservations::new()
                .snapshot(Default::default(), all(), 100, now)
                .expect("restart")
                .instance_id,
            initial.instance_id
        );
    }
    #[test]
    fn expired_tickets_and_unreadable_groups_are_denied() {
        let registry = McpTransportObservations::new();
        let now = Instant::now();
        let mut value = fixture();
        let first = report(&registry, "one", value.clone(), now);
        for second in 1..=9 {
            value.sequence += 1;
            assert!(
                report(
                    &registry,
                    "one",
                    value.clone(),
                    now + Duration::from_secs(second)
                )
                .request_bindings
                .is_empty()
            );
        }
        assert!(
            registry
                .bind(
                    &first.request_bindings[0].ticket,
                    &client(),
                    now + Duration::from_secs(11)
                )
                .is_err()
        );
        assert!(
            registry
                .snapshot(Default::default(), TransportReadAccess::default(), 100, now)
                .is_err()
        );
        assert!(!TransportReadAccess::from_client(&client()).connections);
        let only = registry
            .snapshot(
                Default::default(),
                TransportReadAccess {
                    requests: true,
                    ..Default::default()
                },
                100,
                now + Duration::from_secs(9),
            )
            .expect("one group");
        assert!(only.connections.is_none());
        assert!(only.subscription_streams.is_none());
        assert!(only.requests.is_some());
        assert!(
            registry
                .snapshot(
                    McpTransportQuery {
                        limit: Some(101),
                        ..Default::default()
                    },
                    all(),
                    100,
                    now
                )
                .is_err()
        );
    }
}
