//! Signed third-party fixture for the public client and observation interfaces.
mod bindings {
    wit_bindgen::generate!({
        path: ["../../../../../specs/extensions/wit/mcp-runtime-v1", "../../../../../specs/extensions/wit/mcp-transport-v1/transport.wit"],
        inline: "package fixtures:management; world fixture { include alcomd:mcp-runtime/runtime-v1@1.0.0; import alcomd:mcp-transport/observations@1.0.0; }",
        world: "fixture", generate_all,
    });
    use super::Guest;
    export!(Guest with_types_in self);
}
use bindings::alcomd::{
    extension::types, mcp_management::clients, mcp_runtime::observations as runtime,
    mcp_transport::observations as transport,
};
use bindings::exports::alcomd::extension::{guest_lifecycle, guest_ui as ui};
use std::{cell::RefCell, collections::BTreeMap};

struct Guest;
#[derive(Default)]
struct View {
    activity: bool,
    selected: Option<String>,
    confirming: bool,
    next_request: u64,
}
thread_local! { static VIEWS: RefCell<BTreeMap<String, View>> = RefCell::default(); }
impl guest_lifecycle::Guest for Guest {
    fn activate(_: types::ActivationContext) -> Result<(), types::ExtensionError> {
        Ok(())
    }
    fn deactivate(_: types::StopReason) -> Result<(), types::ExtensionError> {
        Ok(())
    }
}

fn node(nodes: &mut Vec<ui::UiNode>, id: &str, parent: Option<&str>, payload: ui::NodePayload) {
    let order = nodes
        .iter()
        .filter(|n| n.parent_id.as_deref() == parent)
        .count() as u32;
    nodes.push(ui::UiNode {
        node_id: id.into(),
        parent_id: parent.map(str::to_owned),
        order,
        payload,
    });
}
fn button(nodes: &mut Vec<ui::UiNode>, id: &str) {
    node(
        nodes,
        id,
        Some("page"),
        ui::NodePayload::Button(ui::ButtonNode {
            action_id: id.into(),
            label: id.into(),
            disabled: false,
        }),
    );
}
fn text(nodes: &mut Vec<ui::UiNode>, id: &str, value: String) {
    node(
        nodes,
        id,
        Some("page"),
        ui::NodePayload::Text(ui::TextNode {
            text: value,
            tone: ui::Tone::Neutral,
        }),
    );
}
fn render(view: &View) -> ui::UiDocument {
    let title = "External management fixture";
    let mut nodes = Vec::new();
    node(
        &mut nodes,
        "page",
        None,
        ui::NodePayload::Page(ui::PageNode {
            title: title.into(),
        }),
    );
    match clients::list(None, Some(50)) {
        Err(_) => text(&mut nodes, "read", "clients unavailable".into()),
        Ok(page) => {
            text(
                &mut nodes,
                "read",
                format!(
                    "clients: {}",
                    page.items
                        .iter()
                        .map(|c| c.display_name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            );
            if page.can_manage {
                node(
                    &mut nodes,
                    "create-form",
                    Some("page"),
                    ui::NodePayload::Form(ui::FormNode {
                        submit_action_id: "create".into(),
                        submit_label: "create client".into(),
                        disabled: false,
                    }),
                );
                node(
                    &mut nodes,
                    "name-field",
                    Some("create-form"),
                    ui::NodePayload::TextField(ui::TextFieldNode {
                        field_id: "name".into(),
                        label: "Name".into(),
                        initial_value: String::new(),
                        required: true,
                        min_length: 1,
                        max_length: 80,
                        multiline: false,
                        disabled: false,
                        read_only: false,
                        validation: None,
                    }),
                );
                if view.selected.is_some() {
                    if view.confirming {
                        button(&mut nodes, "confirm");
                        button(&mut nodes, "cancel");
                    } else {
                        button(&mut nodes, "revoke");
                    }
                }
            }
        }
    }
    let live = runtime::snapshot(None, Some(25)).ok();
    let reported = transport::snapshot(None, Some(25)).ok();
    if live.is_some()
        || reported
            .as_ref()
            .is_some_and(|r| r.can_read_subscription_streams)
    {
        button(&mut nodes, "activity");
        if view.activity {
            button(&mut nodes, "runtime-refresh");
            if let Some(snapshot) = live {
                if let Some(page) = snapshot.connections {
                    text(
                        &mut nodes,
                        "connections",
                        format!(
                            "connections={} {}",
                            page.total_tracked,
                            page.items
                                .iter()
                                .map(|r| r.client_id.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                    );
                }
                if let Some(page) = snapshot.requests {
                    text(
                        &mut nodes,
                        "requests",
                        format!(
                            "requests={} {}",
                            page.total_tracked,
                            page.items
                                .iter()
                                .map(|r| r.method.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                    );
                }
            }
            if let Some(snapshot) = reported.filter(|r| r.can_read_subscription_streams) {
                if matches!(snapshot.status, transport::Availability::Unavailable) {
                    text(&mut nodes, "streams", "streams unavailable".into());
                } else if let Some(page) = snapshot.subscription_streams {
                    text(
                        &mut nodes,
                        "streams",
                        format!(
                            "streams={} {}",
                            page.total_tracked,
                            page.items
                                .iter()
                                .map(|r| format!(
                                    "{} changed={}",
                                    r.client_id.as_deref().unwrap_or("unattributed"),
                                    r.tools_list_changed
                                ))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                    );
                }
            }
        }
    }
    ui::UiDocument {
        title: title.into(),
        nodes,
    }
}
impl ui::Guest for Guest {
    fn open(session: String, _: String) -> ui::UiDocument {
        VIEWS.with_borrow_mut(|views| render(views.entry(session).or_default()))
    }
    fn refresh(session: String) -> ui::UiDocument {
        VIEWS.with_borrow_mut(|views| render(views.entry(session).or_default()))
    }
    fn dispatch(session: String, action: ui::UiAction) -> ui::UiDocument {
        VIEWS.with_borrow_mut(|views| {
            let view = views.entry(session.clone()).or_default();
            view.next_request += 1;
            let key = format!("fixture-{session}-{}", view.next_request);
            match action {
                ui::UiAction::Activate(action) if action == "activity" => view.activity = true,
                ui::UiAction::Activate(action) if action == "revoke" => view.confirming = true,
                ui::UiAction::Activate(action) if action == "cancel" => view.confirming = false,
                ui::UiAction::Activate(action) if action == "confirm" && view.confirming => {
                    if let (Some(id), Ok(page)) = (&view.selected, clients::list(None, Some(50)))
                        && let Some(client) = page.items.into_iter().find(|c| &c.client_id == id)
                    {
                        let _ = clients::revoke(id, client.revision, &key);
                    }
                    view.confirming = false;
                }
                ui::UiAction::SubmitForm(form) if form.action_id == "create" => {
                    if let Some(ui::FieldValue::Text(name)) = form
                        .values
                        .into_iter()
                        .find(|f| f.field_id == "name")
                        .map(|f| f.value)
                        && let Ok(record) = clients::create(&name, &key)
                    {
                        view.selected = Some(record.client_id);
                    }
                }
                _ => {}
            }
            render(view)
        })
    }
    fn close(session: String) {
        VIEWS.with_borrow_mut(|v| v.remove(&session));
    }
}
