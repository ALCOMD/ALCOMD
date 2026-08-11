use crate::activity_log::ActivityLogState;
use crate::config::{ExtensionUserConfig, SidebarExtension, is_builtin_sidebar_extension};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use tauri::{AppHandle, Emitter, Manager};

pub const EXTENSION_STATE_CHANGED_EVENT: &str = "extension-state-changed";
pub const MCP_EXTENSION_ID: &str = "mcp";
pub const THEME_EXTENSION_ID: &str = "theme";
pub const LOG_EXTENSION_ID: &str = "log";
pub const UNITY_DISCORD_STATUS_EXTENSION_ID: &str = "unity-discord-status";

#[derive(Clone, Copy, Debug)]
pub struct BuiltInExtensionDefinition {
    pub id: &'static str,
    pub display_name: &'static str,
    pub can_disable: bool,
    pub can_install: bool,
    pub can_uninstall: bool,
    pub default_enabled: bool,
    lifecycle: BuiltInExtensionLifecycle,
}

#[derive(Clone, Copy, Debug)]
enum BuiltInExtensionLifecycle {
    PresentationOnly,
    Logs,
    DiscordStatus,
}

const BUILT_IN_EXTENSION_DEFINITIONS: &[BuiltInExtensionDefinition] = &[
    BuiltInExtensionDefinition {
        id: MCP_EXTENSION_ID,
        display_name: "MCP",
        can_disable: true,
        can_install: false,
        can_uninstall: false,
        default_enabled: true,
        lifecycle: BuiltInExtensionLifecycle::PresentationOnly,
    },
    BuiltInExtensionDefinition {
        id: THEME_EXTENSION_ID,
        display_name: "Theme",
        can_disable: true,
        can_install: false,
        can_uninstall: false,
        default_enabled: true,
        lifecycle: BuiltInExtensionLifecycle::PresentationOnly,
    },
    BuiltInExtensionDefinition {
        id: LOG_EXTENSION_ID,
        display_name: "Logs",
        can_disable: true,
        can_install: false,
        can_uninstall: false,
        default_enabled: true,
        lifecycle: BuiltInExtensionLifecycle::Logs,
    },
    BuiltInExtensionDefinition {
        id: UNITY_DISCORD_STATUS_EXTENSION_ID,
        display_name: "Discord",
        can_disable: true,
        can_install: false,
        can_uninstall: false,
        default_enabled: true,
        lifecycle: BuiltInExtensionLifecycle::DiscordStatus,
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExtensionOrigin {
    BuiltIn,
    ThirdParty,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionManifest {
    pub id: String,
    pub display_name: String,
    pub version: String,
    pub origin: ExtensionOrigin,
    pub can_disable: bool,
    pub can_install: bool,
    pub can_uninstall: bool,
}

pub trait ExtensionLifecycle: Send + Sync {
    fn apply_enabled_state(&self, app: &AppHandle, enabled: bool);
}

struct PresentationOnlyLifecycle;

impl ExtensionLifecycle for PresentationOnlyLifecycle {
    fn apply_enabled_state(&self, _app: &AppHandle, _enabled: bool) {}
}

struct LogLifecycle;

impl ExtensionLifecycle for LogLifecycle {
    fn apply_enabled_state(&self, app: &AppHandle, enabled: bool) {
        app.state::<ActivityLogState>().set_enabled(enabled);
        crate::logging::set_technical_logging_enabled(enabled);
    }
}

struct DiscordStatusLifecycle;

impl ExtensionLifecycle for DiscordStatusLifecycle {
    fn apply_enabled_state(&self, app: &AppHandle, enabled: bool) {
        app.state::<crate::discord_presence::DiscordPresenceState>()
            .set_enabled(enabled);
    }
}

#[derive(Clone)]
struct RegisteredExtension {
    manifest: ExtensionManifest,
    installed: bool,
    default_enabled: bool,
    lifecycle: Arc<dyn ExtensionLifecycle>,
}

pub struct InstalledThirdPartyExtension {
    pub manifest: ExtensionManifest,
    pub lifecycle: Arc<dyn ExtensionLifecycle>,
}

pub struct ExtensionRegistry {
    entries: RwLock<IndexMap<String, RegisteredExtension>>,
}

impl Default for ExtensionRegistry {
    fn default() -> Self {
        Self::from_sources(Vec::new(), Vec::new())
            .expect("built-in extension manifests must be valid")
    }
}

impl ExtensionRegistry {
    pub fn from_sources(
        catalog: Vec<ExtensionManifest>,
        installed: Vec<InstalledThirdPartyExtension>,
    ) -> Result<Self, ExtensionRegistryError> {
        let registry = Self {
            entries: RwLock::new(IndexMap::new()),
        };
        for definition in BUILT_IN_EXTENSION_DEFINITIONS {
            let lifecycle: Arc<dyn ExtensionLifecycle> = match definition.lifecycle {
                BuiltInExtensionLifecycle::PresentationOnly => Arc::new(PresentationOnlyLifecycle),
                BuiltInExtensionLifecycle::Logs => Arc::new(LogLifecycle),
                BuiltInExtensionLifecycle::DiscordStatus => Arc::new(DiscordStatusLifecycle),
            };
            registry.register(
                ExtensionManifest {
                    id: definition.id.to_string(),
                    display_name: definition.display_name.to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    origin: ExtensionOrigin::BuiltIn,
                    can_disable: definition.can_disable,
                    can_install: definition.can_install,
                    can_uninstall: definition.can_uninstall,
                },
                true,
                definition.default_enabled,
                lifecycle,
            )?;
        }
        for extension in installed {
            registry.register_installed_third_party(extension)?;
        }
        for manifest in catalog {
            registry.register_third_party_catalog_entry(manifest)?;
        }
        Ok(registry)
    }

    pub fn register_third_party_catalog_entry(
        &self,
        manifest: ExtensionManifest,
    ) -> Result<(), ExtensionRegistryError> {
        if manifest.origin != ExtensionOrigin::ThirdParty {
            return Err(ExtensionRegistryError::InvalidOrigin);
        }
        if let Some(existing) = self.entries.read().unwrap().get(&manifest.id)
            && existing.manifest.origin == ExtensionOrigin::ThirdParty
            && existing.installed
        {
            validate_manifest(&manifest, false)?;
            return Ok(());
        }
        self.register(manifest, false, true, Arc::new(PresentationOnlyLifecycle))
    }

    pub fn register_installed_third_party(
        &self,
        extension: InstalledThirdPartyExtension,
    ) -> Result<(), ExtensionRegistryError> {
        if extension.manifest.origin != ExtensionOrigin::ThirdParty {
            return Err(ExtensionRegistryError::InvalidOrigin);
        }
        validate_manifest(&extension.manifest, true)?;
        let mut entries = self.entries.write().unwrap();
        if let Some(existing) = entries.get(&extension.manifest.id)
            && (existing.manifest.origin != ExtensionOrigin::ThirdParty || existing.installed)
        {
            return Err(ExtensionRegistryError::DuplicateId);
        }
        entries.shift_remove(&extension.manifest.id);
        entries.insert(
            extension.manifest.id.clone(),
            RegisteredExtension {
                manifest: extension.manifest,
                installed: true,
                default_enabled: true,
                lifecycle: extension.lifecycle,
            },
        );
        Ok(())
    }

    fn register(
        &self,
        manifest: ExtensionManifest,
        installed: bool,
        default_enabled: bool,
        lifecycle: Arc<dyn ExtensionLifecycle>,
    ) -> Result<(), ExtensionRegistryError> {
        validate_manifest(&manifest, installed)?;
        let mut entries = self.entries.write().unwrap();
        if entries.contains_key(&manifest.id) {
            return Err(ExtensionRegistryError::DuplicateId);
        }
        entries.insert(
            manifest.id.clone(),
            RegisteredExtension {
                manifest,
                installed,
                default_enabled,
                lifecycle,
            },
        );
        Ok(())
    }

    fn get(&self, id: &str) -> Option<RegisteredExtension> {
        self.entries.read().unwrap().get(id).cloned()
    }

    pub fn enablement_capability(&self, id: &str) -> Option<(bool, bool)> {
        self.get(id)
            .map(|extension| (extension.installed, extension.manifest.can_disable))
    }

    pub fn is_installed(&self, id: &str) -> Option<bool> {
        self.get(id).map(|extension| extension.installed)
    }

    pub fn sidebar_info(
        &self,
        sidebar_layout: &[SidebarExtension],
        extension_configs: &BTreeMap<String, ExtensionUserConfig>,
    ) -> Vec<SidebarExtension> {
        sidebar_layout
            .iter()
            .map(|layout| {
                let (installed, enabled) = self.get(&layout.id).map_or_else(
                    || {
                        let built_in = is_builtin_sidebar_extension(&layout.id);
                        (built_in, built_in)
                    },
                    |extension| {
                        (
                            extension.installed,
                            extension.installed
                                && extension_configs
                                    .get(&extension.manifest.id)
                                    .map_or(extension.default_enabled, |state| state.enabled),
                        )
                    },
                );
                SidebarExtension {
                    id: layout.id.clone(),
                    installed,
                    enabled,
                    visible: layout.visible,
                }
            })
            .collect()
    }

    pub fn management_info(
        &self,
        extension_configs: &BTreeMap<String, ExtensionUserConfig>,
    ) -> Vec<ExtensionManagementInfo> {
        self.entries
            .read()
            .unwrap()
            .values()
            .map(|extension| {
                let state = extension_configs.get(&extension.manifest.id);
                ExtensionManagementInfo {
                    id: extension.manifest.id.clone(),
                    display_name: extension.manifest.display_name.clone(),
                    version: extension.manifest.version.clone(),
                    installed: extension.installed,
                    enabled: extension.installed
                        && state.map_or(extension.default_enabled, |extension| extension.enabled),
                    built_in: extension.manifest.origin == ExtensionOrigin::BuiltIn,
                    can_disable: extension.manifest.can_disable,
                    can_install: extension.manifest.can_install,
                    can_uninstall: extension.manifest.can_uninstall,
                }
            })
            .collect()
    }

    pub fn restore_runtime_states(
        &self,
        app: &AppHandle,
        extension_configs: &BTreeMap<String, ExtensionUserConfig>,
    ) {
        let extensions = self
            .entries
            .read()
            .unwrap()
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for extension in extensions {
            if !extension.installed {
                continue;
            }
            let enabled = extension_configs
                .get(&extension.manifest.id)
                .map_or(extension.default_enabled, |extension| extension.enabled);
            extension.lifecycle.apply_enabled_state(app, enabled);
        }
    }

    pub fn apply_enabled_state(&self, app: &AppHandle, id: &str, enabled: bool) {
        let Some(extension) = self.get(id) else {
            return;
        };
        extension.lifecycle.apply_enabled_state(app, enabled);
        if let Err(error) = app.emit(
            EXTENSION_STATE_CHANGED_EVENT,
            ExtensionStateChanged {
                id: id.to_string(),
                enabled,
            },
        ) {
            log::error!("failed to emit extension state change: {error}");
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtensionRegistryError {
    DuplicateId,
    InvalidId,
    InvalidDisplayName,
    InvalidVersion,
    InvalidOrigin,
    InvalidInstallationCapabilities,
}

fn validate_manifest(
    manifest: &ExtensionManifest,
    installed: bool,
) -> Result<(), ExtensionRegistryError> {
    let mut characters = manifest.id.chars();
    let Some(first) = characters.next() else {
        return Err(ExtensionRegistryError::InvalidId);
    };
    if !first.is_ascii_lowercase()
        || !characters.all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '.' | '-' | '_')
        })
    {
        return Err(ExtensionRegistryError::InvalidId);
    }
    if manifest.display_name.trim().is_empty() || manifest.display_name.len() > 128 {
        return Err(ExtensionRegistryError::InvalidDisplayName);
    }
    if semver::Version::parse(&manifest.version).is_err() {
        return Err(ExtensionRegistryError::InvalidVersion);
    }
    if manifest.origin != ExtensionOrigin::ThirdParty && !installed {
        return Err(ExtensionRegistryError::InvalidOrigin);
    }
    if installed && manifest.can_install {
        return Err(ExtensionRegistryError::InvalidInstallationCapabilities);
    }
    if !installed && manifest.can_uninstall {
        return Err(ExtensionRegistryError::InvalidInstallationCapabilities);
    }
    Ok(())
}

pub fn built_in_extension_definitions() -> &'static [BuiltInExtensionDefinition] {
    BUILT_IN_EXTENSION_DEFINITIONS
}

pub fn built_in_extension_definition(id: &str) -> Option<&'static BuiltInExtensionDefinition> {
    BUILT_IN_EXTENSION_DEFINITIONS
        .iter()
        .find(|definition| definition.id == id)
}

pub fn built_in_extension_can_disable(id: &str) -> bool {
    built_in_extension_definition(id).is_some_and(|definition| definition.can_disable)
}

#[derive(Clone, Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionManagementInfo {
    pub id: String,
    pub display_name: String,
    pub version: String,
    pub installed: bool,
    pub enabled: bool,
    pub built_in: bool,
    pub can_disable: bool,
    pub can_install: bool,
    pub can_uninstall: bool,
}

#[derive(Clone, Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionStateChanged {
    pub id: String,
    pub enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn third_party_manifest(
        id: &str,
        can_disable: bool,
        can_install: bool,
        can_uninstall: bool,
    ) -> ExtensionManifest {
        ExtensionManifest {
            id: id.to_string(),
            display_name: match id {
                "example.available" => "Available Extension",
                "example.installed" => "Installed Extension",
                _ => "Test Extension",
            }
            .to_string(),
            version: "1.0.0".to_string(),
            origin: ExtensionOrigin::ThirdParty,
            can_disable,
            can_install,
            can_uninstall,
        }
    }

    #[test]
    fn management_capabilities_come_from_the_backend_registry() {
        let registry = ExtensionRegistry::from_sources(
            vec![third_party_manifest("example.available", true, true, false)],
            Vec::new(),
        )
        .unwrap();
        let states = BTreeMap::from([
            (
                THEME_EXTENSION_ID.to_string(),
                ExtensionUserConfig { enabled: false },
            ),
            (
                MCP_EXTENSION_ID.to_string(),
                ExtensionUserConfig { enabled: true },
            ),
            (
                LOG_EXTENSION_ID.to_string(),
                ExtensionUserConfig { enabled: false },
            ),
        ]);

        let extensions = registry.management_info(&states);
        let available = extensions
            .iter()
            .find(|extension| extension.id == "example.available")
            .unwrap();
        let theme = extensions
            .iter()
            .find(|extension| extension.id == THEME_EXTENSION_ID)
            .unwrap();
        let mcp = extensions
            .iter()
            .find(|extension| extension.id == MCP_EXTENSION_ID)
            .unwrap();
        let log = extensions
            .iter()
            .find(|extension| extension.id == LOG_EXTENSION_ID)
            .unwrap();
        let discord_status = extensions
            .iter()
            .find(|extension| extension.id == UNITY_DISCORD_STATUS_EXTENSION_ID)
            .unwrap();

        assert!(!available.built_in);
        assert!(!available.installed);
        assert!(available.can_install);
        assert!(theme.built_in);
        assert!(theme.can_disable);
        assert!(!theme.enabled);
        assert!(mcp.can_disable);
        assert!(mcp.enabled);
        assert!(log.can_disable);
        assert!(!log.enabled);
        assert!(discord_status.can_disable);
        assert!(discord_status.enabled);
        assert_eq!(
            extensions
                .iter()
                .map(|extension| extension.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                MCP_EXTENSION_ID,
                THEME_EXTENSION_ID,
                LOG_EXTENSION_ID,
                UNITY_DISCORD_STATUS_EXTENSION_ID,
                "example.available"
            ]
        );
    }

    #[test]
    fn third_party_registration_rejects_invalid_or_duplicate_ids() {
        assert!(matches!(
            ExtensionRegistry::from_sources(
                vec![third_party_manifest("../invalid", true, true, false)],
                Vec::new(),
            ),
            Err(ExtensionRegistryError::InvalidId)
        ));
        assert!(matches!(
            ExtensionRegistry::from_sources(
                vec![third_party_manifest(THEME_EXTENSION_ID, true, true, false,)],
                Vec::new(),
            ),
            Err(ExtensionRegistryError::DuplicateId)
        ));
    }

    #[test]
    fn installed_third_party_state_comes_from_the_registry_source() {
        let registry = ExtensionRegistry::from_sources(
            vec![third_party_manifest("example.installed", true, true, false)],
            vec![InstalledThirdPartyExtension {
                manifest: third_party_manifest("example.installed", true, false, true),
                lifecycle: Arc::new(PresentationOnlyLifecycle),
            }],
        )
        .unwrap();
        let states = BTreeMap::from([(
            "example.installed".to_string(),
            ExtensionUserConfig { enabled: false },
        )]);

        let extensions = registry.management_info(&states);
        let extension = extensions
            .iter()
            .find(|extension| extension.id == "example.installed")
            .unwrap();

        assert!(extension.installed);
        assert!(!extension.enabled);
        assert!(!extension.built_in);
        assert!(extension.can_disable);
        assert!(extension.can_uninstall);
        assert!(!extension.can_install);
        assert_eq!(
            extensions
                .iter()
                .map(|extension| extension.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                MCP_EXTENSION_ID,
                THEME_EXTENSION_ID,
                LOG_EXTENSION_ID,
                UNITY_DISCORD_STATUS_EXTENSION_ID,
                "example.installed"
            ]
        );
    }

    #[test]
    fn sidebar_state_is_projected_from_the_registry_and_user_config() {
        let registry = ExtensionRegistry::from_sources(
            vec![third_party_manifest("example.available", true, true, false)],
            vec![InstalledThirdPartyExtension {
                manifest: third_party_manifest("example.installed", true, false, true),
                lifecycle: Arc::new(PresentationOnlyLifecycle),
            }],
        )
        .unwrap();
        let layout = vec![
            SidebarExtension {
                id: "example.available".to_string(),
                installed: true,
                enabled: true,
                visible: true,
            },
            SidebarExtension {
                id: "example.installed".to_string(),
                installed: false,
                enabled: true,
                visible: true,
            },
        ];
        let states = BTreeMap::from([(
            "example.installed".to_string(),
            ExtensionUserConfig { enabled: false },
        )]);

        let sidebar = registry.sidebar_info(&layout, &states);

        assert!(!sidebar[0].installed);
        assert!(!sidebar[0].enabled);
        assert!(sidebar[1].installed);
        assert!(!sidebar[1].enabled);
    }

    #[test]
    fn third_party_entries_append_in_registration_and_installation_order() {
        let registry = ExtensionRegistry::default();
        registry
            .register_third_party_catalog_entry(third_party_manifest(
                "example.second",
                true,
                true,
                false,
            ))
            .unwrap();
        registry
            .register_third_party_catalog_entry(third_party_manifest(
                "example.first",
                true,
                true,
                false,
            ))
            .unwrap();

        assert_eq!(
            registry
                .management_info(&BTreeMap::new())
                .iter()
                .map(|extension| extension.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                MCP_EXTENSION_ID,
                THEME_EXTENSION_ID,
                LOG_EXTENSION_ID,
                UNITY_DISCORD_STATUS_EXTENSION_ID,
                "example.second",
                "example.first"
            ]
        );

        registry
            .register_installed_third_party(InstalledThirdPartyExtension {
                manifest: third_party_manifest("example.second", true, false, true),
                lifecycle: Arc::new(PresentationOnlyLifecycle),
            })
            .unwrap();

        assert_eq!(
            registry
                .management_info(&BTreeMap::new())
                .iter()
                .map(|extension| extension.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                MCP_EXTENSION_ID,
                THEME_EXTENSION_ID,
                LOG_EXTENSION_ID,
                UNITY_DISCORD_STATUS_EXTENSION_ID,
                "example.first",
                "example.second"
            ]
        );
    }
}
