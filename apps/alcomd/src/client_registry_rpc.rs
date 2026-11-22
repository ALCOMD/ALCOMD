//! Native IPC projection. OS observations never become authority.
use crate::{
    Applications, ConnectionState, DispatchAction, error_action, require_capability, success_action,
};
use alcomd_application as app;
use alcomd_platform::client_installation::{self, ClientKind, InstallScope};
use alcomd_protocol as rpc;
use serde_json::json;

pub(super) fn process(
    value: alcomd_platform::client_peer::ProcessIdentity,
) -> app::RegistryProcess {
    app::RegistryProcess {
        pid: value.pid,
        creation: value.creation,
        image: value.image,
        name: value.name,
        shared_runtime: value.shared_runtime,
    }
}

pub(super) fn official_actor(
    data: &alcomd_platform::DataConfig,
    peer: &alcomd_platform::client_peer::ClientPeer,
) -> &'static str {
    let Some(process) = &peer.process else {
        return "Local IPC";
    };
    let catalogue = client_installation::scan(data);
    for process in peer.bridge_parent.iter().chain(std::iter::once(process)) {
        if process.shared_runtime {
            continue;
        }
        if let Some(label) = catalogue.items.iter().find_map(|i| {
            (i.executable_identity == process.image)
                .then(|| i.record.official_actor())
                .flatten()
        }) {
            return label;
        }
    }
    "Local IPC"
}

enum Action {
    Programs(rpc::ClientsListParams),
    Instances(rpc::ClientInstancesListParams),
    Disconnect(rpc::ClientsDisconnectParams),
}
pub(super) async fn dispatch(
    request: rpc::RequestEnvelope,
    state: &ConnectionState,
    apps: &Applications,
    access: &app::AccessContext,
) -> DispatchAction {
    if let Some(error) = require_capability(&request.id, state, rpc::CAPABILITY_CLIENTS_REGISTRY_V1)
    {
        return error;
    }
    let action = match request.method.as_str() {
        rpc::METHOD_CLIENTS_LIST => serde_json::from_value(request.params).map(Action::Programs),
        rpc::METHOD_CLIENT_INSTANCES_LIST => {
            serde_json::from_value(request.params).map(Action::Instances)
        }
        rpc::METHOD_CLIENTS_DISCONNECT => {
            serde_json::from_value(request.params).map(Action::Disconnect)
        }
        _ => return error_action(Some(request.id), rpc::RpcError::method_not_found(), false),
    };
    let Ok(action) = action else {
        return error_action(Some(request.id), rpc::RpcError::invalid_request(), false);
    };
    let data = apps.client_installation_data.clone();
    let catalogue = tokio::task::spawn_blocking(move || client_installation::scan(&data)).await;
    let Ok(catalogue) = catalogue else {
        return error_action(Some(request.id), rpc::RpcError::store_unavailable(), false);
    };
    let installed = catalogue
        .items
        .into_iter()
        .map(|item| app::RegistryInstallation {
            program_id: item.record.program_id,
            name: item.record.display_name,
            version: item.record.product_version,
            image: item.executable_identity,
            scope: match item.scope {
                InstallScope::User => "user",
                InstallScope::Machine => "machine",
            }
            .into(),
            kind: match item.record.kind {
                ClientKind::Gui => "gui",
                ClientKind::Cli => "cli",
                ClientKind::Mcp => "mcp",
            }
            .into(),
        })
        .collect::<Vec<_>>();
    let registry = &apps.client_registry;
    let core = &registry.core_instance_id;
    let result = match action {
        Action::Programs(p) => registry
            .list(
                access,
                &installed,
                catalogue.incomplete,
                p.cursor.as_deref(),
                p.limit,
            )
            .map(|(items, next, coverage)| {
                let mut value =
                    json!({"coreInstanceId":core,"items":items,"coverageIncomplete":coverage});
                if let Some(cursor) = next {
                    value["nextCursor"] = json!(cursor);
                }
                value
            }),
        Action::Instances(p) => registry
            .instances(
                access,
                &installed,
                &p.program_key,
                p.cursor.as_deref(),
                p.limit,
            )
            .map(|(items, next)| {
                let mut value = json!({"coreInstanceId":core,"items":items});
                if let Some(cursor) = next {
                    value["nextCursor"] = json!(cursor);
                }
                value
            }),
        Action::Disconnect(p) => {
            let target = match &p.target {
                rpc::ClientDisconnectTarget::Connection { id } => {
                    app::RegistryTarget::Connection(id)
                }
                rpc::ClientDisconnectTarget::Instance { id } => app::RegistryTarget::Instance(id),
            };
            registry
                .disconnect(
                    access,
                    &p.expected_core_instance_id,
                    target,
                    &p.idempotency_key,
                    &installed,
                    &apps.store,
                )
                .await
                .map(|ids| json!({"disconnectedConnectionIds":ids}))
        }
    };
    match result {
        Ok(value) => success_action(request.id, value, None),
        Err(err) => {
            let rpc = match err {
                app::RegistryError::Denied => rpc::RpcError::permission_denied(),
                app::RegistryError::Invalid => rpc::RpcError::invalid_request(),
                app::RegistryError::Stale => rpc::RpcError::client_registry_stale(),
                app::RegistryError::NotFound => rpc::RpcError::client_registry_not_found(),
                app::RegistryError::Conflict => rpc::RpcError::idempotency_conflict(),
                app::RegistryError::Unavailable => rpc::RpcError::store_unavailable(),
            };
            error_action(Some(request.id), rpc, false)
        }
    }
}
