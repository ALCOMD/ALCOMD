//! Test-only guest that drives every M9 import through one background export.

mod bindings {
    wit_bindgen::generate!({
        path: "../../../../../specs/extensions/wit/extension-v1",
        world: "background-integration-v1",
        generate_all,
    });
    use super::Guest;
    export!(Guest with_types_in self);
}

use bindings::alcomd::extension::{host_local_ipc as ipc, host_unity_sessions as unity, types};
use bindings::exports::alcomd::extension::{guest_background, guest_lifecycle, guest_ui};

struct Guest;

impl guest_background::Guest for Guest {
    fn tick() -> Result<(), types::ExtensionError> {
        let session = unity::current().map_err(|_| failure())?;
        if session.editor_count != 1 || session.current_session.is_none() {
            return Err(failure());
        }
        let connection = ipc::connect("fixture").map_err(|_| failure())?;
        let result = (|| {
            ipc::write(&connection.channel, &[0, 255]).map_err(|_| failure())?;
            let reply = ipc::read(&connection.channel, 2).map_err(|_| failure())?;
            if reply == [42, 7] {
                Ok(())
            } else {
                Err(failure())
            }
        })();
        ipc::close(&connection.channel).map_err(|_| failure())?;
        result
    }
}

fn failure() -> types::ExtensionError {
    types::ExtensionError {
        code: types::ErrorCode::ResourceLimit,
        diagnostic_id: None,
    }
}

impl guest_lifecycle::Guest for Guest {
    fn activate(_: types::ActivationContext) -> Result<(), types::ExtensionError> {
        Ok(())
    }
    fn deactivate(_: types::StopReason) -> Result<(), types::ExtensionError> {
        Ok(())
    }
}

impl guest_ui::Guest for Guest {
    fn open(_: String, _: String) -> guest_ui::UiDocument {
        fixture_document()
    }
    fn refresh(_: String) -> guest_ui::UiDocument {
        fixture_document()
    }
    fn dispatch(_: String, _: guest_ui::UiAction) -> guest_ui::UiDocument {
        fixture_document()
    }
    fn close(_: String) {}
}

fn fixture_document() -> guest_ui::UiDocument {
    guest_ui::UiDocument {
        title: "Background test".into(),
        nodes: vec![],
    }
}
