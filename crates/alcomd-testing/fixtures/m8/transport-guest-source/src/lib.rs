//! Test-only component. Generated canonical ABI follows the frozen generator.
mod bindings {
    wit_bindgen::generate!({path: "../../../../../specs/extensions/wit/mcp-transport-v1", world: "transport-v1",generate_all,});
    #[cfg(target_arch = "wasm32")]
    use super::Guest;
    #[cfg(target_arch = "wasm32")]
    export!(Guest with_types_in self);
}
use bindings::alcomd::extension::types;
use bindings::alcomd::mcp_transport::observations as host;
use bindings::exports::alcomd::extension::{guest_lifecycle, guest_ui as ui};
struct Guest;
impl guest_lifecycle::Guest for Guest {
    fn activate(_: types::ActivationContext) -> Result<(), types::ExtensionError> {
        Ok(())
    }
    fn deactivate(_: types::StopReason) -> Result<(), types::ExtensionError> {
        Ok(())
    }
}
fn document() -> ui::UiDocument {
    let text = match host::snapshot(None, Some(25)) {
        Ok(s) => format!(
            "connections={} requests={} streams={} coverageIncomplete={}",
            s.can_read_connections,
            s.can_read_requests,
            s.can_read_subscription_streams,
            s.coverage_incomplete
        ),
        Err(e) => {
            if matches!(
                e.code,
                bindings::alcomd::mcp_management::clients::ErrorCode::PermissionDenied
            ) {
                "transport_error=permission_denied".into()
            } else {
                format!("transport_error={:?}", e.code)
            }
        }
    };
    ui::UiDocument {
        title: "Transport test".into(),
        nodes: vec![
            ui::UiNode {
                node_id: "root".into(),
                parent_id: None,
                order: 0,
                payload: ui::NodePayload::Page(ui::PageNode {
                    title: "Transport test".into(),
                }),
            },
            ui::UiNode {
                node_id: "facts".into(),
                parent_id: Some("root".into()),
                order: 0,
                payload: ui::NodePayload::Text(ui::TextNode {
                    text,
                    tone: ui::Tone::Neutral,
                }),
            },
        ],
    }
}
impl ui::Guest for Guest {
    fn open(_: String, _: String) -> ui::UiDocument {
        document()
    }
    fn refresh(_: String) -> ui::UiDocument {
        document()
    }
    fn dispatch(_: String, _: ui::UiAction) -> ui::UiDocument {
        document()
    }
    fn close(_: String) {}
}
