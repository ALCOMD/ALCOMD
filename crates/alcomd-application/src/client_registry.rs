//! The live client catalogue and disconnect use case. Installation and process
//! evidence are observations supplied by the platform, never authority.

use std::collections::{BTreeMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tokio::sync::watch;

use crate::{AccessContext, IdempotencyKey, OperationId};

pub const CLIENT_CONNECTION_LIMIT: usize = 1024;
const PAGE_LIFETIME: Duration = Duration::from_secs(60);
const SNAPSHOT_LIMIT: usize = 32;
const RECEIPT_LIMIT: usize = 4096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryProcess {
    pub pid: u32,
    pub creation: u64,
    pub image: Vec<u8>,
    pub name: String,
    pub shared_runtime: bool,
}
pub struct RegistryInstallation {
    pub program_id: String,
    pub kind: String,
    pub name: String,
    pub scope: String,
    pub version: String,
    pub image: Vec<u8>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryReported {
    pub name: String,
    pub version: String,
    pub instance_id: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryConnection {
    pub connection_id: String,
    pub connected_at_ms: u64,
    pub reported: RegistryReported,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryInstalled {
    pub scope: String,
    pub product_version: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryProgram {
    pub program_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program_id: Option<String>,
    pub display_name: String,
    pub kind: String,
    pub installations: Vec<RegistryInstalled>,
    pub active_instance_count: usize,
    pub active_connection_count: usize,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryInstance {
    pub instance_id: String,
    pub connections: Vec<RegistryConnection>,
}
#[derive(Debug, Eq, PartialEq)]
pub enum RegistryError {
    Denied,
    Invalid,
    Stale,
    NotFound,
    Conflict,
    Unavailable,
}
pub enum RegistryTarget<'a> {
    Connection(&'a str),
    Instance(&'a str),
}

struct Live {
    process: Option<RegistryProcess>,
    parent: Option<RegistryProcess>,
    instance: String,
    parent_instance: String,
    program: String,
    connection: RegistryConnection,
    close: watch::Sender<bool>,
}
struct Receipt {
    principal: String,
    key: String,
    target: String,
    result: Vec<String>,
}
#[derive(Clone)]
enum Rows {
    Programs(Vec<RegistryProgram>, bool),
    Instances(Vec<RegistryInstance>),
}
struct Snapshot {
    id: String,
    principal: String,
    query: String,
    created: Instant,
    rows: Rows,
}
#[derive(Default)]
struct State {
    connections: BTreeMap<String, Live>,
    snapshots: VecDeque<Snapshot>,
    receipts: VecDeque<Receipt>,
}
pub struct ClientRegistry {
    pub core_instance_id: String,
    state: Mutex<State>,
}
fn fresh() -> String {
    OperationId::new().to_string()
}

impl Default for ClientRegistry {
    fn default() -> Self {
        Self {
            core_instance_id: fresh(),
            state: Mutex::new(State::default()),
        }
    }
}
impl ClientRegistry {
    pub fn connect(
        &self,
        process: Option<RegistryProcess>,
        parent: Option<RegistryProcess>,
        connection: RegistryConnection,
        close: watch::Sender<bool>,
    ) {
        let mut state = self.state.lock().expect("client registry lock");
        let instance_for = |evidence: &Option<RegistryProcess>| {
            evidence
                .as_ref()
                .and_then(|process| {
                    state.connections.values().find_map(|live| {
                        if live.process.as_ref() == Some(process) {
                            Some(live.instance.clone())
                        } else if live.parent.as_ref() == Some(process) {
                            Some(live.parent_instance.clone())
                        } else {
                            None
                        }
                    })
                })
                .unwrap_or_else(fresh)
        };
        let instance = instance_for(&process);
        let parent_instance = instance_for(&parent);
        // An unattributed helper cannot combine unrelated GUI programs.
        let program = process
            .as_ref()
            .and_then(|p| {
                state.connections.values().find(|v| {
                    v.process.as_ref().is_some_and(|other| {
                        if p.shared_runtime {
                            other == p
                        } else {
                            other.image == p.image
                        }
                    })
                })
            })
            .map_or_else(|| format!("live:{}", fresh()), |v| v.program.clone());
        state.connections.insert(
            connection.connection_id.clone(),
            Live {
                process,
                parent,
                instance,
                parent_instance,
                program,
                connection,
                close,
            },
        );
    }
    pub fn closed(&self, connection_id: &str) {
        self.state
            .lock()
            .expect("client registry lock")
            .connections
            .remove(connection_id);
    }
    fn principal(access: &AccessContext) -> Result<&str, RegistryError> {
        if access.is_interactive_client() {
            Ok(access.principal().as_str())
        } else {
            Err(RegistryError::Denied)
        }
    }
    pub fn list(
        &self,
        access: &AccessContext,
        installed: &[RegistryInstallation],
        incomplete: bool,
        cursor: Option<&str>,
        limit: Option<u32>,
    ) -> Result<(Vec<RegistryProgram>, Option<String>, bool), RegistryError> {
        let principal = Self::principal(access)?;
        let mut state = self.state.lock().map_err(|_| RegistryError::Unavailable)?;
        let (rows, next, start, end) =
            page(&mut state, principal, "programs", cursor, limit, |state| {
                let (programs, _) = views(state, installed);
                Ok(Rows::Programs(programs.into_values().collect(), incomplete))
            })?;
        let Rows::Programs(rows, coverage) = rows else {
            return Err(RegistryError::Invalid);
        };
        Ok((rows[start..end].to_vec(), next, coverage))
    }
    pub fn instances(
        &self,
        access: &AccessContext,
        installed: &[RegistryInstallation],
        key: &str,
        cursor: Option<&str>,
        limit: Option<u32>,
    ) -> Result<(Vec<RegistryInstance>, Option<String>), RegistryError> {
        let principal = Self::principal(access)?;
        if key.is_empty() || key.len() > 160 {
            return Err(RegistryError::Invalid);
        }
        let mut state = self.state.lock().map_err(|_| RegistryError::Unavailable)?;
        let query = format!("instances:{key}");
        let (rows, next, start, end) =
            page(&mut state, principal, &query, cursor, limit, |state| {
                let (programs, mut instances) = views(state, installed);
                if !programs.contains_key(key) {
                    return Err(RegistryError::NotFound);
                }
                Ok(Rows::Instances(
                    instances
                        .remove(key)
                        .unwrap_or_default()
                        .into_values()
                        .collect(),
                ))
            })?;
        let Rows::Instances(rows) = rows else {
            return Err(RegistryError::Invalid);
        };
        Ok((rows[start..end].to_vec(), next))
    }
    pub async fn disconnect(
        &self,
        access: &AccessContext,
        core: &str,
        target: RegistryTarget<'_>,
        key: &str,
        installed: &[RegistryInstallation],
        store: &impl crate::OAuthStore,
    ) -> Result<Vec<String>, RegistryError> {
        let principal = Self::principal(access)?;
        let (instance, id) = match target {
            RegistryTarget::Instance(id) => (true, id),
            RegistryTarget::Connection(id) => (false, id),
        };
        if access.is_mcp_client() {
            let authentication = crate::workflow_authentication().ok_or(RegistryError::Denied)?;
            let current = authentication
                .authenticate(store)
                .await
                .map_err(|_| RegistryError::Denied)?;
            if current.principal() != access.principal() {
                return Err(RegistryError::Denied);
            }
        }
        if OperationId::parse(core).is_err()
            || OperationId::parse(id).is_err()
            || IdempotencyKey::parse(key.to_owned()).is_err()
        {
            return Err(RegistryError::Invalid);
        }
        if core != self.core_instance_id {
            return Err(RegistryError::Stale);
        }
        let target = format!("{}:{id}", if instance { "instance" } else { "connection" });
        let mut state = self.state.lock().map_err(|_| RegistryError::Unavailable)?;
        if let Some(receipt) = state
            .receipts
            .iter()
            .find(|r| r.principal == principal && r.key == key)
        {
            return if receipt.target == target {
                Ok(receipt.result.clone())
            } else {
                Err(RegistryError::Conflict)
            };
        }
        let ids = state
            .connections
            .iter()
            .filter_map(|(conn, live)| {
                let matches = if instance {
                    attribution(live, installed).1 == id
                } else {
                    conn == id
                };
                matches.then(|| conn.clone())
            })
            .collect::<Vec<_>>();
        if ids.is_empty() {
            return Err(RegistryError::NotFound);
        }
        for conn in &ids {
            if let Some(live) = state.connections.remove(conn) {
                live.close.send_replace(true);
            }
        }
        if state.receipts.len() >= RECEIPT_LIMIT {
            state.receipts.pop_front();
        }
        state.receipts.push_back(Receipt {
            principal: principal.to_owned(),
            key: key.to_owned(),
            target,
            result: ids.clone(),
        });
        Ok(ids)
    }
}

fn attribution<'a>(
    live: &'a Live,
    installed: &'a [RegistryInstallation],
) -> (Option<&'a RegistryInstallation>, &'a str) {
    for (process, instance) in [
        (&live.parent, &live.parent_instance),
        (&live.process, &live.instance),
    ] {
        let Some(process) = process else {
            continue;
        };
        if process.shared_runtime {
            continue;
        }
        let mut matches = installed.iter().filter(|item| item.image == process.image);
        if let Some(item) = matches.next()
            && matches.all(|other| other.program_id == item.program_id)
        {
            return (Some(item), instance);
        }
    }
    (None, &live.instance)
}
type InstanceMap = BTreeMap<String, BTreeMap<String, RegistryInstance>>;
fn views(
    state: &State,
    installed: &[RegistryInstallation],
) -> (BTreeMap<String, RegistryProgram>, InstanceMap) {
    let mut programs = BTreeMap::new();
    let mut instances = InstanceMap::new();
    for item in installed {
        let row = programs
            .entry(item.program_id.clone())
            .or_insert_with(|| RegistryProgram {
                program_key: item.program_id.clone(),
                program_id: Some(item.program_id.clone()),
                display_name: item.name.clone(),
                kind: item.kind.clone(),
                installations: vec![],
                active_instance_count: 0,
                active_connection_count: 0,
            });
        row.installations.push(RegistryInstalled {
            scope: item.scope.clone(),
            product_version: item.version.clone(),
        });
    }
    for live in state.connections.values() {
        let (installed, instance) = attribution(live, installed);
        let key = installed.map_or(&live.program, |item| &item.program_id);
        let row = programs
            .entry(key.clone())
            .or_insert_with(|| RegistryProgram {
                program_key: key.clone(),
                program_id: None,
                display_name: live
                    .process
                    .as_ref()
                    .map_or_else(|| "Unknown client".to_owned(), |p| p.name.clone()),
                kind: "unknown".into(),
                installations: vec![],
                active_instance_count: 0,
                active_connection_count: 0,
            });
        let instance = instances
            .entry(key.clone())
            .or_default()
            .entry(instance.to_owned())
            .or_insert_with(|| RegistryInstance {
                instance_id: instance.to_owned(),
                connections: vec![],
            });
        if instance.connections.is_empty() {
            row.active_instance_count += 1;
        }
        instance.connections.push(live.connection.clone());
        row.active_connection_count += 1;
    }
    (programs, instances)
}

fn page(
    state: &mut State,
    principal: &str,
    query: &str,
    cursor: Option<&str>,
    limit: Option<u32>,
    read: impl FnOnce(&State) -> Result<Rows, RegistryError>,
) -> Result<(Rows, Option<String>, usize, usize), RegistryError> {
    let limit = limit.unwrap_or(32);
    if !(1..=128).contains(&limit) {
        return Err(RegistryError::Invalid);
    }
    state
        .snapshots
        .retain(|s| s.created.elapsed() < PAGE_LIFETIME);
    let (id, start, rows) = if let Some(cursor) = cursor {
        let (id, offset) = cursor.split_once(':').ok_or(RegistryError::Invalid)?;
        let start = offset
            .parse::<usize>()
            .map_err(|_| RegistryError::Invalid)?;
        let snapshot = state
            .snapshots
            .iter()
            .find(|s| s.id == id && s.principal == principal && s.query == query)
            .ok_or(RegistryError::Stale)?;
        (id.to_owned(), start, snapshot.rows.clone())
    } else {
        (fresh(), 0, read(state)?)
    };
    let len = match &rows {
        Rows::Programs(v, _) => v.len(),
        Rows::Instances(v) => v.len(),
    };
    if start > len {
        return Err(RegistryError::Invalid);
    }
    let end = start.saturating_add(limit as usize).min(len);
    let next = (end < len).then(|| format!("{id}:{end}"));
    if cursor.is_none() && next.is_some() {
        if state.snapshots.len() >= SNAPSHOT_LIMIT {
            state.snapshots.pop_front();
        }
        state.snapshots.push_back(Snapshot {
            id,
            principal: principal.to_owned(),
            query: query.to_owned(),
            created: Instant::now(),
            rows: rows.clone(),
        });
    }
    Ok((rows, next, start, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process(pid: u32, image: u8, name: &str, shared_runtime: bool) -> RegistryProcess {
        RegistryProcess {
            pid,
            creation: u64::from(pid),
            image: vec![image],
            name: name.into(),
            shared_runtime,
        }
    }
    fn installed(id: &str, kind: &str, image: u8) -> RegistryInstallation {
        RegistryInstallation {
            program_id: id.into(),
            kind: kind.into(),
            name: id.into(),
            scope: "user".into(),
            version: "1.0".into(),
            image: vec![image],
        }
    }
    fn connected(
        registry: &ClientRegistry,
        peer: RegistryProcess,
        parent: Option<RegistryProcess>,
    ) -> String {
        let id = fresh();
        registry.connect(
            Some(peer),
            parent,
            RegistryConnection {
                connection_id: id.clone(),
                connected_at_ms: 10,
                reported: RegistryReported {
                    name: "untrusted diagnostic".into(),
                    version: "1".into(),
                    instance_id: "identical claim".into(),
                },
            },
            watch::channel(false).0,
        );
        id
    }
    #[test]
    fn ten_cli_processes_one_program_and_registered_gui_bridges_share_one_instance() {
        let registry = ClientRegistry::default();
        let owner = AccessContext::local_owner();
        let installs = vec![
            installed("example.cli", "cli", 1),
            installed("example.gui", "gui", 2),
            installed("example.offline", "gui", 3),
            installed("example.runtime", "gui", 9),
        ];
        for pid in 1..=10 {
            connected(&registry, process(pid, 1, "cli", false), None);
        }
        let gui = process(20, 2, "gui", false);
        connected(&registry, gui.clone(), None);
        connected(
            &registry,
            process(21, 4, "alcomd-sdk-bridge", true),
            Some(gui),
        );
        let anonymous = connected(
            &registry,
            process(31, 4, "alcomd-sdk-bridge", true),
            Some(process(30, 9, "node", true)),
        );
        let (programs, _, _) = registry
            .list(&owner, &installs, false, None, None)
            .expect("list programs");
        let cli = programs
            .iter()
            .find(|p| p.program_key == "example.cli")
            .expect("cli");
        assert_eq!(
            (cli.active_instance_count, cli.active_connection_count),
            (10, 10)
        );
        let gui = registry
            .instances(&owner, &installs, "example.gui", None, None)
            .expect("gui instances")
            .0;
        assert_eq!(gui.len(), 1);
        assert_eq!(gui[0].connections.len(), 2);
        assert!(
            programs
                .iter()
                .any(|p| p.program_key == "example.offline" && p.active_instance_count == 0)
        );
        assert!(
            programs
                .iter()
                .any(|p| p.program_key == "example.runtime" && p.active_instance_count == 0)
        );
        let unknown = programs
            .iter()
            .find(|p| p.program_id.is_none())
            .expect("unidentified script");
        registry.closed(&anonymous);
        assert!(matches!(
            registry.instances(&owner, &installs, &unknown.program_key, None, None),
            Err(RegistryError::NotFound)
        ));
    }
    #[test]
    fn page_remains_bound_to_query_and_snapshot_after_connections_change() {
        let registry = ClientRegistry::default();
        let owner = AccessContext::local_owner();
        let ids = [
            connected(&registry, process(1, 1, "a", false), None),
            connected(&registry, process(2, 2, "b", false), None),
        ];
        let (first, cursor, coverage) = registry
            .list(&owner, &[], true, None, Some(1))
            .expect("first page");
        assert!(coverage);
        let cursor = cursor.expect("next page");
        registry.closed(&ids[0]);
        registry.closed(&ids[1]);
        let (next, done, coverage) = registry
            .list(&owner, &[], false, Some(&cursor), Some(1))
            .expect("stable snapshot");
        assert!(coverage && done.is_none());
        assert_ne!(first[0].program_key, next[0].program_key);
        assert!(matches!(
            registry.instances(&owner, &[], &first[0].program_key, Some(&cursor), Some(1)),
            Err(RegistryError::Stale)
        ));
        let restarted = ClientRegistry::default();
        assert!(matches!(
            restarted.list(&owner, &[], false, Some(&cursor), Some(1)),
            Err(RegistryError::Stale)
        ));
    }
}
