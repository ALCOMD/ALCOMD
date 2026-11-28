use std::io;

use alcomd_platform::read_windows_installation_registry_string;

pub(super) use alcomd_platform::{
    confirm_windows_installation as confirm, notify_windows_installation as notify,
};

use super::Scope;

pub(super) fn registry_value(scope: Scope, key: &str, name: &str) -> io::Result<Option<String>> {
    read_windows_installation_registry_string(scope == Scope::AllUsers, key, name)
}
