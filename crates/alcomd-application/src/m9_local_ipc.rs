use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const UNITY_SESSION_PERMISSION: &str = "unity.sessions.read";
pub const LOCAL_IPC_PERMISSION: &str = "local-ipc.connect";
pub const LOCAL_IPC_RESOURCE_KIND: &str = "LocalIpcTarget";
pub const BACKGROUND_INTEGRATION_INTERFACES: &[&str] = &[
    "alcomd:extension/guest-background@1.0.0",
    "alcomd:extension/host-local-ipc@1.0.0",
    "alcomd:extension/host-unity-sessions@1.0.0",
];

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ExtensionBackgroundIntegration {
    pub enabled: bool,
    pub local_ipc_targets: BTreeMap<String, LocalIpcTarget>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionActivationConsent {
    pub package_digest: String,
    pub targets: Vec<String>,
}

pub struct VerifiedExtensionActivation {
    pub package_digest: [u8; 32],
    pub target_scopes: Vec<String>,
}

impl ExtensionBackgroundIntegration {
    pub fn consent_preview(&self, package_digest: [u8; 32]) -> Option<ExtensionActivationConsent> {
        (!self.local_ipc_targets.is_empty()).then(|| ExtensionActivationConsent {
            package_digest: lower_hex(package_digest),
            targets: self
                .local_ipc_targets
                .iter()
                .map(|(id, target)| target.consent_label(id))
                .collect(),
        })
    }

    pub(crate) fn activation(&self, package_digest: [u8; 32]) -> VerifiedExtensionActivation {
        VerifiedExtensionActivation {
            package_digest,
            target_scopes: self
                .local_ipc_targets
                .iter()
                .map(|(id, target)| target.scope_key(id))
                .collect(),
        }
    }
}

/// A signed local target declaration. Names are tokens, never native path input.
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LocalIpcTarget {
    pub label: String,
    pub windows: Option<WindowsLocalIpcTarget>,
    pub macos: Option<MacLocalIpcTarget>,
    pub linux: Option<LinuxLocalIpcTarget>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WindowsLocalIpcTarget {
    pub pipe_names: Vec<String>,
    pub executable_name: String,
    pub publisher: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MacLocalIpcTarget {
    pub socket_names: Vec<String>,
    pub bundle_identifier: String,
    pub team_identifier: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LinuxLocalIpcTarget {
    pub socket_names: Vec<String>,
    pub system_executable: String,
}

impl LocalIpcTarget {
    fn consent_label(&self, id: &str) -> String {
        #[cfg(windows)]
        let identity = self
            .windows
            .as_ref()
            .map(|value| format!("{} · {}", value.executable_name, value.publisher));
        #[cfg(target_os = "macos")]
        let identity = self
            .macos
            .as_ref()
            .map(|value| format!("{} · {}", value.bundle_identifier, value.team_identifier));
        #[cfg(target_os = "linux")]
        let identity = self
            .linux
            .as_ref()
            .map(|value| value.system_executable.clone());
        #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
        let identity: Option<String> = None;
        match identity {
            Some(identity) => format!("{} ({id} · {identity})", self.label),
            None => format!("{} ({id})", self.label),
        }
    }

    pub fn valid(&self, target_id: &str) -> bool {
        token(target_id, 64)
            && text(&self.label, 100)
            && (self.windows.is_some() || self.macos.is_some() || self.linux.is_some())
            && self.windows.as_ref().is_none_or(|windows| {
                endpoint_names(&windows.pipe_names)
                    && windows
                        .executable_name
                        .strip_suffix(".exe")
                        .is_some_and(|name| token(name, 76))
                    && text(&windows.publisher, 120)
            })
            && self.macos.as_ref().is_none_or(|macos| {
                endpoint_names(&macos.socket_names)
                    && valid_mac_identity(&macos.bundle_identifier, &macos.team_identifier)
            })
            && self.linux.as_ref().is_none_or(|linux| {
                endpoint_names(&linux.socket_names)
                    && valid_system_executable(&linux.system_executable)
            })
    }

    /// Stable scope key over the canonical signed declaration and its target ID.
    pub fn scope_key(&self, target_id: &str) -> String {
        let mut hash = Sha256::new();
        hash.update(b"alcomd-local-ipc-target-v1\0");
        let mut field = |field: &str| {
            hash.update(field.as_bytes());
            hash.update([0]);
        };
        field(target_id);
        field(&self.label);
        for (platform, fields, names) in [
            self.windows.as_ref().map(|target| {
                (
                    "windows",
                    vec![target.executable_name.as_str(), &target.publisher],
                    &target.pipe_names,
                )
            }),
            self.macos.as_ref().map(|target| {
                (
                    "macos",
                    vec![target.bundle_identifier.as_str(), &target.team_identifier],
                    &target.socket_names,
                )
            }),
            self.linux.as_ref().map(|target| {
                (
                    "linux",
                    vec![target.system_executable.as_str()],
                    &target.socket_names,
                )
            }),
        ]
        .into_iter()
        .flatten()
        {
            field(platform);
            for value in fields.into_iter().chain(names.iter().map(String::as_str)) {
                field(value);
            }
            // A zero-length field terminates the platform’s non-empty field list.
            field("");
        }
        lower_hex(hash.finalize().into())
    }
}

fn lower_hex(bytes: [u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    bytes
        .into_iter()
        .flat_map(|byte| {
            [
                char::from(HEX[usize::from(byte >> 4)]),
                char::from(HEX[usize::from(byte & 15)]),
            ]
        })
        .collect()
}

fn endpoint_names(names: &[String]) -> bool {
    (1..=10).contains(&names.len())
        && names.windows(2).all(|pair| pair[0] < pair[1])
        && names.iter().all(|name| token(name, 80))
}

fn valid_mac_identity(bundle: &str, team: &str) -> bool {
    bundle.len() <= 160
        && bundle.contains('.')
        && bundle.split('.').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
        && team.len() == 10
        && team
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
}

fn valid_system_executable(path: &str) -> bool {
    path.len() <= 512
        && path.strip_prefix('/').is_some_and(|relative| {
            relative.split('/').all(|part| {
                !part.is_empty()
                    && part != "."
                    && part != ".."
                    && part
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
            })
        })
}

pub fn valid_local_ipc_scope_key(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn token(value: &str, limit: usize) -> bool {
    !value.is_empty()
        && value.len() <= limit
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn text(value: &str, limit: usize) -> bool {
    !value.trim().is_empty() && value.len() <= limit && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_is_bound_to_the_exact_label_platform_identities_and_endpoints() {
        let declaration = LocalIpcTarget {
            label: "Fixture application".into(),
            windows: Some(WindowsLocalIpcTarget {
                pipe_names: vec!["fixture-ipc-0".into(), "fixture-ipc-1".into()],
                executable_name: "fixture.exe".into(),
                publisher: "Fixture publisher".into(),
            }),
            macos: Some(MacLocalIpcTarget {
                socket_names: vec!["fixture-ipc-0".into()],
                bundle_identifier: "dev.example.fixture".into(),
                team_identifier: "TEST012345".into(),
            }),
            linux: Some(LinuxLocalIpcTarget {
                socket_names: vec!["fixture-ipc-0".into()],
                system_executable: "/opt/fixture/Fixture".into(),
            }),
        };
        assert!(declaration.valid("fixture"));
        let scope = declaration.scope_key("fixture");
        assert!(valid_local_ipc_scope_key(&scope));
        assert_ne!(scope, declaration.scope_key("same-app"));

        let mut changed = declaration.clone();
        changed
            .windows
            .as_mut()
            .expect("Windows profile")
            .publisher
            .push('!');
        assert_ne!(scope, changed.scope_key("fixture"));
        changed
            .windows
            .as_mut()
            .expect("Windows profile")
            .pipe_names
            .reverse();
        assert!(!changed.valid("fixture"));

        let mut changed = declaration;
        changed
            .linux
            .as_mut()
            .expect("Linux profile")
            .system_executable = "/opt/../run/Fixture".into();
        assert!(!changed.valid("fixture"));
    }
}
