//! Sampled, bounded transport facts. Credentials stay private and are used only
//! for a separate Core-authenticated correlation; reporting never drives business.
use alcomd_client::{AlcomdClient, ClientConfig};
use alcomd_protocol::*;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, Weak},
    time::Duration,
};

#[derive(Clone)]
pub(crate) struct Tracker(Arc<Mutex<State>>);
struct State {
    kind: McpTransportKind,
    connections: BTreeMap<String, McpTransportConnectionStage>,
    requests: BTreeMap<String, (McpTransportRequestInput, Option<ClientConfig>)>,
    streams: BTreeMap<String, McpTransportStreamInput>,
    incomplete: bool,
}
pub(crate) struct Connection {
    tracker: Tracker,
    id: String,
}
pub(crate) struct Request {
    tracker: Tracker,
    id: String,
}
pub(crate) struct Stream {
    tracker: Tracker,
    id: String,
}
fn id() -> String {
    // Opaque local correlation only, never an authority or credential. Core
    // separately generates the public IDs and cryptographic binding tickets.
    use std::hash::BuildHasher;
    let state = std::collections::hash_map::RandomState::new();
    let high = state.hash_one(0u8);
    let low = state.hash_one(1u8);
    let bits = (((u128::from(high) << 64) | u128::from(low)) & !(0xfu128 << 76) & !(3u128 << 62))
        | (4u128 << 76)
        | (2u128 << 62);
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        bits >> 96,
        (bits >> 80) & 0xffff,
        (bits >> 64) & 0xffff,
        (bits >> 48) & 0xffff,
        bits & 0xffffffffffff
    )
}
impl Tracker {
    pub fn start(kind: McpTransportKind, config: ClientConfig) -> Self {
        let tracker = Self(Arc::new(Mutex::new(State {
            kind,
            connections: BTreeMap::new(),
            requests: BTreeMap::new(),
            streams: BTreeMap::new(),
            incomplete: false,
        })));
        tokio::spawn(report_loop(
            Arc::downgrade(&tracker.0),
            config.without_daemon_start().with_transport_observer(),
        ));
        tracker
    }
    pub fn connection(&self, stage: McpTransportConnectionStage) -> Option<Connection> {
        let mut state = self.0.lock().ok()?;
        if state.connections.len() >= 128 {
            state.incomplete = true;
            return None;
        }
        let id = id();
        state.connections.insert(id.clone(), stage);
        Some(Connection {
            tracker: self.clone(),
            id,
        })
    }
}
impl Connection {
    pub fn stage(&self, stage: McpTransportConnectionStage) {
        if let Ok(mut s) = self.tracker.0.lock()
            && let Some(current) = s.connections.get_mut(&self.id)
        {
            *current = stage;
        }
    }
    pub fn request(
        &self,
        method: McpTransportMethod,
        identity: Option<ClientConfig>,
    ) -> Option<Request> {
        let mut state = self.tracker.0.lock().ok()?;
        if state.requests.len() >= 128 {
            state.incomplete = true;
            return None;
        }
        state
            .connections
            .insert(self.id.clone(), McpTransportConnectionStage::Active);
        let id = id();
        state.requests.insert(
            id.clone(),
            (
                McpTransportRequestInput {
                    local_ref: id.clone(),
                    connection_ref: self.id.clone(),
                    method,
                    stage: McpTransportRequestStage::Executing,
                },
                identity,
            ),
        );
        Some(Request {
            tracker: self.tracker.clone(),
            id,
        })
    }
}
impl Request {
    pub fn stream(&self) -> Option<Stream> {
        let mut state = self.tracker.0.lock().ok()?;
        if state.streams.len() >= 32 {
            state.incomplete = true;
            return None;
        }
        let (request, _) = state.requests.get_mut(&self.id)?;
        if request.method != McpTransportMethod::Listen {
            return None;
        }
        request.stage = McpTransportRequestStage::Streaming;
        let id = id();
        state.streams.insert(
            id.clone(),
            McpTransportStreamInput {
                local_ref: id.clone(),
                request_ref: self.id.clone(),
                tools_list_changed: true,
            },
        );
        Some(Stream {
            tracker: self.tracker.clone(),
            id,
        })
    }
}
impl Drop for Connection {
    fn drop(&mut self) {
        if let Ok(mut s) = self.tracker.0.lock() {
            s.connections.remove(&self.id);
            let ids: Vec<_> = s
                .requests
                .iter()
                .filter(|(_, r)| r.0.connection_ref == self.id)
                .map(|(id, _)| id.clone())
                .collect();
            for id in ids {
                s.requests.remove(&id);
                s.streams.retain(|_, v| v.request_ref != id);
            }
        }
    }
}
impl Drop for Request {
    fn drop(&mut self) {
        if let Ok(mut s) = self.tracker.0.lock() {
            let connection = s.requests.remove(&self.id).map(|(r, _)| r.connection_ref);
            s.streams.retain(|_, v| v.request_ref != self.id);
            if let Some(connection) = connection
                && !s
                    .requests
                    .values()
                    .any(|(r, _)| r.connection_ref == connection)
                && let Some(stage) = s.connections.get_mut(&connection)
            {
                *stage = McpTransportConnectionStage::Idle;
            }
        }
    }
}
impl Drop for Stream {
    fn drop(&mut self) {
        if let Ok(mut s) = self.tracker.0.lock() {
            s.streams.remove(&self.id);
        }
    }
}
pub(crate) fn method(name: &str) -> Option<McpTransportMethod> {
    match name {
        "server/discover" => Some(McpTransportMethod::Discover),
        "tools/list" => Some(McpTransportMethod::ListTools),
        "tools/call" => Some(McpTransportMethod::CallTool),
        "subscriptions/listen" => Some(McpTransportMethod::Listen),
        _ => None,
    }
}

async fn report_loop(weak: Weak<Mutex<State>>, config: ClientConfig) {
    let mut core = None;
    let mut sequence = 0u64;
    let mut binding_tasks = tokio::task::JoinSet::new();
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        while binding_tasks.try_join_next().is_some() {}
        let Some(shared) = weak.upgrade() else {
            return;
        };
        let report = {
            let Ok(s) = shared.lock() else {
                return;
            };
            sequence += 1;
            McpTransportReport {
                sequence,
                transport: s.kind,
                connections: s
                    .connections
                    .iter()
                    .map(|(id, stage)| McpTransportConnectionInput {
                        local_ref: id.clone(),
                        stage: *stage,
                    })
                    .collect(),
                requests: s.requests.values().map(|(r, _)| r.clone()).collect(),
                subscription_streams: s.streams.values().cloned().collect(),
                coverage_incomplete: s.incomplete,
            }
        };
        drop(shared);
        if core.is_none() {
            core = tokio::time::timeout(
                Duration::from_millis(750),
                AlcomdClient::connect(config.clone()),
            )
            .await
            .ok()
            .and_then(Result::ok);
        }
        let Some(current) = core.as_mut() else {
            continue;
        };
        let result = tokio::time::timeout(
            Duration::from_millis(750),
            current.mcp_transport_report(report),
        )
        .await;
        let Ok(Ok(result)) = result else {
            // This response may have contained one-time binding tickets. Never
            // reconnect/re-report existing requests to obtain replacements.
            // Dropping only this observer makes coverage unavailable; business
            // transport and accepted Core Operations remain independent.
            return;
        };
        // One bounded batch; a lost response is not replayed and cannot retry any business write.
        for binding in result.request_bindings {
            if binding_tasks.len() >= 4 {
                break;
            }
            let weak = weak.clone();
            binding_tasks.spawn(async move {
                let Some(shared) = weak.upgrade() else {
                    return;
                };
                let identity = shared.lock().ok().and_then(|s| {
                    s.requests
                        .get(&binding.request_ref)
                        .and_then(|(_, identity)| identity.clone())
                });
                drop(shared);
                let Some(identity) = identity else {
                    return;
                };
                let _ = tokio::time::timeout(Duration::from_millis(750), async {
                    let mut client = AlcomdClient::connect(identity.without_daemon_start()).await?;
                    client
                        .mcp_transport_bind_request(McpTransportBindParams {
                            ticket: binding.ticket,
                        })
                        .await
                })
                .await;
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn guards_remove_child_rows_and_bound_identity_is_never_serialized() {
        let tracker = Tracker::start(
            McpTransportKind::Http,
            ClientConfig::default().without_daemon_start(),
        );
        let connection = tracker
            .connection(McpTransportConnectionStage::Idle)
            .expect("socket");
        let request = connection
            .request(McpTransportMethod::Listen, None)
            .expect("request");
        let stream = request.stream().expect("accepted stream");
        assert_eq!(tracker.0.lock().expect("state").streams.len(), 1);
        drop(request);
        assert!(tracker.0.lock().expect("state").streams.is_empty());
        drop(stream);
        drop(connection);
        assert!(tracker.0.lock().expect("state").connections.is_empty());
    }
}
