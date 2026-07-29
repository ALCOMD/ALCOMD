use crate::extensions::{
    LOG_EXTENSION_ID, MCP_EXTENSION_ID, THEME_EXTENSION_ID, built_in_extension_can_disable,
    built_in_extension_definition, built_in_extension_definitions,
};
use crate::logging::LogLevel;
use indexmap::IndexSet;
use serde::{Deserialize, Serialize, Serializer};
use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiConfig {
    #[serde(default)]
    pub gui_hidden_repositories: IndexSet<String>,
    #[serde(default)]
    pub hide_local_user_packages: bool,
    #[serde(default)]
    pub window_size: WindowSize,
    #[serde(default)]
    pub fullscreen: bool,
    #[serde(default = "language_default")]
    pub language: String,
    #[serde(default = "backup_default")]
    pub backup_format: String,
    #[serde(default = "project_sorting_default")]
    pub project_sorting: String,
    #[serde(default = "release_channel_default")]
    // "stable" or "beta"
    pub release_channel: String,
    #[serde(default = "automatic_update_default")]
    pub automatic_update: bool,
    #[serde(default = "use_alcom_for_vcc_protocol_default")]
    pub use_alcom_for_vcc_protocol: bool,
    #[serde(default)]
    pub setup_process_progress: u32,
    #[serde(default)]
    pub default_unity_arguments: Option<Vec<String>>,
    #[serde(default = "log_level_default")]
    pub logs_level: Vec<LogLevel>,
    #[serde(default = "gui_animation_default")]
    pub gui_animation: bool,
    #[serde(default = "gui_compact_default")]
    pub gui_compact: bool,
    #[serde(default)]
    pub mcp_enabled: bool,
    #[serde(default = "mcp_http_port_default")]
    pub mcp_http_port: u16,
    #[serde(default)]
    pub mcp_http_token: String,
    #[serde(default = "project_view_mode_default")]
    pub project_view_mode: String,
    #[serde(default)]
    pub unity_hub_access_method: UnityHubAccessMethod,
    // last element is the most recent one
    // 8 paths are saved
    #[serde(default)]
    pub recent_project_locations: Vec<String>,
    /// the list of favorite templates by id
    /// those templates will be shown at the top of template selection on project creation
    /// or derived templates
    #[serde(default)]
    pub favorite_templates: Vec<String>,
    /// The lastly used template, this will be the initially selected template
    #[serde(default)]
    pub last_used_template: Option<String>,
    #[serde(default)]
    pub update_reminder: Option<UpdateReminderConfig>,
    #[serde(default, serialize_with = "serialize_sidebar_extensions")]
    pub sidebar_extensions: Vec<SidebarExtension>,
    #[serde(default)]
    pub extensions: BTreeMap<String, ExtensionUserConfig>,
}

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SidebarExtension {
    pub id: String,
    #[serde(default = "default_true")]
    pub installed: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub visible: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionUserConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn serialize_sidebar_extensions<S>(
    extensions: &[SidebarExtension],
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct SidebarLayout<'a> {
        id: &'a str,
        visible: bool,
    }

    extensions
        .iter()
        .map(|extension| SidebarLayout {
            id: &extension.id,
            visible: extension.visible,
        })
        .collect::<Vec<_>>()
        .serialize(serializer)
}

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateReminderConfig {
    pub latest_version: String,
    pub remind_after: f64,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, Default, specta::Type)]
pub enum UnityHubAccessMethod {
    /// Reads config files of Unity Hub
    #[default]
    ReadConfig,
    /// Launches headless Unity Hub in background
    CallHub,
}

impl Default for GuiConfig {
    fn default() -> Self {
        GuiConfig {
            gui_hidden_repositories: IndexSet::new(),
            hide_local_user_packages: false,
            window_size: WindowSize::default(),
            fullscreen: false,
            language: language_default(),
            backup_format: backup_default(),
            project_sorting: project_sorting_default(),
            release_channel: release_channel_default(),
            automatic_update: automatic_update_default(),
            use_alcom_for_vcc_protocol: use_alcom_for_vcc_protocol_default(),
            setup_process_progress: 0,
            default_unity_arguments: None,
            logs_level: log_level_default(),
            gui_animation: true,
            gui_compact: gui_compact_default(),
            mcp_enabled: false,
            mcp_http_port: mcp_http_port_default(),
            mcp_http_token: String::new(),
            project_view_mode: project_view_mode_default(),
            unity_hub_access_method: UnityHubAccessMethod::ReadConfig,
            recent_project_locations: Vec::new(),
            favorite_templates: vec![],
            last_used_template: None,
            update_reminder: None,
            sidebar_extensions: default_sidebar_extensions(),
            extensions: default_extension_configs(),
        }
    }
}

impl GuiConfig {
    pub(crate) fn fix_defaults(&mut self) {
        if self.language.is_empty() {
            self.language = language_default();
        }
        if self.language == "zh_cn" {
            self.language = "zh_hans".to_string();
        }
        if self.backup_format.is_empty() {
            self.backup_format = backup_default();
        }
        if self.project_sorting.is_empty() {
            self.project_sorting = project_sorting_default();
        }
        if self.sidebar_extensions.is_empty() {
            self.sidebar_extensions = default_sidebar_extensions();
        } else {
            self.sidebar_extensions = normalize_sidebar_extensions(self.sidebar_extensions.clone());
        }
        for definition in built_in_extension_definitions() {
            if !self.extensions.contains_key(definition.id) {
                let enabled = self
                    .sidebar_extensions
                    .iter()
                    .find(|extension| extension.id == definition.id)
                    .is_none_or(|extension| extension.enabled);
                self.extensions
                    .insert(definition.id.to_string(), ExtensionUserConfig { enabled });
            }
        }
        self.synchronize_sidebar_extension_states();
        if !self.is_extension_enabled(MCP_EXTENSION_ID) {
            self.mcp_enabled = false;
        }
    }

    pub(crate) fn ensure_mcp_http_config(&mut self) -> bool {
        let mut changed = false;
        if self.mcp_http_port == 0 {
            self.mcp_http_port = mcp_http_port_default();
            changed = true;
        }
        if self.mcp_http_token.len() < 32 {
            self.mcp_http_token = uuid::Uuid::new_v4().simple().to_string();
            changed = true;
        }
        changed
    }

    pub(crate) fn set_extension_enabled(&mut self, id: &str, enabled: bool) {
        self.extensions
            .insert(id.to_string(), ExtensionUserConfig { enabled });
        if id == MCP_EXTENSION_ID && !enabled {
            self.mcp_enabled = false;
        }
        self.synchronize_sidebar_extension_states();
    }

    pub(crate) fn is_extension_enabled(&self, id: &str) -> bool {
        self.extensions.get(id).is_none_or(|state| state.enabled)
    }

    pub(crate) fn synchronize_sidebar_extension_states(&mut self) {
        for extension in &mut self.sidebar_extensions {
            if built_in_extension_definition(&extension.id).is_some() {
                extension.installed = true;
                extension.enabled = self
                    .extensions
                    .get(&extension.id)
                    .is_none_or(|state| state.enabled);
            }
        }
    }
}

fn language_default() -> String {
    for locale in sys_locale::get_locales() {
        if locale.starts_with("en") {
            return "en".to_string();
        }
        if locale.starts_with("de") {
            return "de".to_string();
        }
        if locale.starts_with("ja") {
            return "ja".to_string();
        }
        if locale.starts_with("zh") {
            return "zh_hans".to_string();
        }
    }

    "en".to_string()
}

fn theme_default() -> String {
    "system".to_string()
}

fn backup_default() -> String {
    "default".to_string()
}

fn project_sorting_default() -> String {
    "lastModified".to_string()
}

fn release_channel_default() -> String {
    "stable".to_string()
}

fn automatic_update_default() -> bool {
    true
}

fn use_alcom_for_vcc_protocol_default() -> bool {
    true
}

fn mcp_http_port_default() -> u16 {
    alcomd3_mcp_protocol::MCP_HTTP_DEFAULT_PORT
}

fn log_level_default() -> Vec<LogLevel> {
    vec![
        LogLevel::Debug,
        LogLevel::Error,
        LogLevel::Warn,
        LogLevel::Info,
    ]
}

fn gui_animation_default() -> bool {
    true
}

fn gui_compact_default() -> bool {
    false
}

fn project_view_mode_default() -> String {
    "List".to_string()
}

fn default_true() -> bool {
    true
}

const LOCKED_SIDEBAR_ITEM_IDS: &[&str] = &["extensions"];
const ALWAYS_VISIBLE_SIDEBAR_EXTENSION_IDS: &[&str] = &["projects", "packages", "settings"];
const BUILT_IN_SIDEBAR_EXTENSION_IDS: &[&str] = &[
    "projects",
    "packages",
    MCP_EXTENSION_ID,
    THEME_EXTENSION_ID,
    "settings",
    LOG_EXTENSION_ID,
];

fn is_configurable_sidebar_extension(id: &str) -> bool {
    !LOCKED_SIDEBAR_ITEM_IDS.contains(&id)
}

pub(crate) fn is_builtin_sidebar_extension(id: &str) -> bool {
    BUILT_IN_SIDEBAR_EXTENSION_IDS.contains(&id)
}

pub(crate) fn is_sidebar_extension_always_visible(id: &str) -> bool {
    ALWAYS_VISIBLE_SIDEBAR_EXTENSION_IDS.contains(&id)
}

fn default_sidebar_extensions() -> Vec<SidebarExtension> {
    vec![
        SidebarExtension {
            id: "projects".to_string(),
            installed: true,
            enabled: true,
            visible: true,
        },
        SidebarExtension {
            id: "packages".to_string(),
            installed: true,
            enabled: true,
            visible: true,
        },
        SidebarExtension {
            id: MCP_EXTENSION_ID.to_string(),
            installed: true,
            enabled: true,
            visible: true,
        },
        SidebarExtension {
            id: THEME_EXTENSION_ID.to_string(),
            installed: true,
            enabled: true,
            visible: true,
        },
        SidebarExtension {
            id: "settings".to_string(),
            installed: true,
            enabled: true,
            visible: true,
        },
        SidebarExtension {
            id: LOG_EXTENSION_ID.to_string(),
            installed: true,
            enabled: true,
            visible: true,
        },
    ]
}

fn default_extension_configs() -> BTreeMap<String, ExtensionUserConfig> {
    built_in_extension_definitions()
        .iter()
        .map(|definition| {
            (
                definition.id.to_string(),
                ExtensionUserConfig { enabled: true },
            )
        })
        .collect()
}

pub(crate) fn normalize_sidebar_extensions(
    existing: Vec<SidebarExtension>,
) -> Vec<SidebarExtension> {
    let mut seen = HashSet::<String>::new();
    let mut updated = Vec::new();
    for mut extension in existing {
        if extension.id.is_empty() || !is_configurable_sidebar_extension(&extension.id) {
            continue;
        }
        if is_builtin_sidebar_extension(&extension.id) {
            extension.installed = true;
            if !built_in_extension_can_disable(&extension.id) {
                extension.enabled = true;
            }
        }
        if is_sidebar_extension_always_visible(&extension.id) {
            extension.visible = true;
        }
        if seen.insert(extension.id.clone()) {
            updated.push(extension);
        }
    }

    for default_extension in default_sidebar_extensions() {
        if seen.insert(default_extension.id.clone()) {
            updated.push(default_extension);
        }
    }

    updated
}

pub(crate) fn apply_sidebar_extension_layout(
    existing: Vec<SidebarExtension>,
    requested: Vec<SidebarExtension>,
) -> Vec<SidebarExtension> {
    let mut seen = HashSet::<String>::new();
    let mut updated = Vec::new();

    for requested_extension in requested {
        if !seen.insert(requested_extension.id.clone()) {
            continue;
        }
        if let Some(existing_extension) = existing
            .iter()
            .find(|extension| extension.id == requested_extension.id)
        {
            updated.push(SidebarExtension {
                id: existing_extension.id.clone(),
                installed: existing_extension.installed,
                enabled: existing_extension.enabled,
                visible: requested_extension.visible,
            });
        }
    }

    for existing_extension in existing {
        if seen.insert(existing_extension.id.clone()) {
            updated.push(existing_extension);
        }
    }

    normalize_sidebar_extensions(updated)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeConfig {
    #[serde(default = "theme_default")]
    pub theme: String,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            theme: theme_default(),
        }
    }
}

impl ThemeConfig {
    pub(crate) fn fix_defaults(&mut self) {
        if self.theme.is_empty() {
            self.theme = theme_default();
        }
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct WindowSize {
    pub width: u32,
    pub height: u32,
}

impl Default for WindowSize {
    fn default() -> Self {
        WindowSize {
            width: 1400,
            height: 800,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        GuiConfig, SidebarExtension, apply_sidebar_extension_layout, normalize_sidebar_extensions,
    };
    use crate::extensions::{LOG_EXTENSION_ID, MCP_EXTENSION_ID, THEME_EXTENSION_ID};

    #[test]
    fn automatic_updates_default_to_enabled_for_existing_configs() {
        let config: GuiConfig = serde_json::from_str("{}").unwrap();
        assert!(config.automatic_update);
    }

    #[test]
    fn automatic_updates_can_be_disabled() {
        let config: GuiConfig = serde_json::from_str(r#"{"automaticUpdate":false}"#).unwrap();
        assert!(!config.automatic_update);
    }

    #[test]
    fn missing_mcp_http_config_is_generated_once() {
        let mut config: GuiConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(
            config.mcp_http_port,
            alcomd3_mcp_protocol::MCP_HTTP_DEFAULT_PORT
        );
        assert!(config.mcp_http_token.is_empty());

        assert!(config.ensure_mcp_http_config());
        let token = config.mcp_http_token.clone();
        assert_eq!(token.len(), 32);
        assert!(!config.ensure_mcp_http_config());
        assert_eq!(config.mcp_http_token, token);
    }

    #[test]
    fn sidebar_extensions_use_expected_default_order() {
        let config = GuiConfig::default();
        let ids = config
            .sidebar_extensions
            .iter()
            .map(|extension| extension.id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            ids,
            ["projects", "packages", "mcp", "theme", "settings", "log"]
        );
    }

    #[test]
    fn missing_sidebar_extensions_are_added_without_changing_existing_order() {
        let extensions = vec![
            SidebarExtension {
                id: "log".to_string(),
                installed: true,
                enabled: true,
                visible: true,
            },
            SidebarExtension {
                id: "projects".to_string(),
                installed: true,
                enabled: true,
                visible: true,
            },
        ];

        let normalized = normalize_sidebar_extensions(extensions);
        let ids = normalized
            .iter()
            .map(|extension| extension.id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            ids,
            ["log", "projects", "packages", "mcp", "theme", "settings"]
        );
        let theme = normalized
            .iter()
            .find(|extension| extension.id == "theme")
            .unwrap();
        assert!(theme.installed);
        assert!(theme.enabled);
        assert!(theme.visible);
    }

    #[test]
    fn existing_sidebar_extensions_default_to_enabled() {
        let extension: SidebarExtension =
            serde_json::from_str(r#"{"id":"theme","installed":true,"visible":true}"#).unwrap();

        assert!(extension.enabled);
    }

    #[test]
    fn legacy_extension_state_migrates_to_backend_extension_config() {
        let mut config: GuiConfig = serde_json::from_str(
            r#"{"sidebarExtensions":[{"id":"theme","enabled":false,"visible":true}]}"#,
        )
        .unwrap();

        config.fix_defaults();

        assert!(!config.extensions.get(THEME_EXTENSION_ID).unwrap().enabled);
        let serialized = serde_json::to_value(config).unwrap();
        assert_eq!(
            serialized["extensions"][THEME_EXTENSION_ID]["enabled"],
            false
        );
        assert_eq!(
            serialized["sidebarExtensions"][0],
            serde_json::json!({"id": "theme", "visible": true})
        );
    }

    #[test]
    fn enableable_built_in_extensions_are_always_installed_but_can_be_disabled() {
        for id in [MCP_EXTENSION_ID, THEME_EXTENSION_ID, LOG_EXTENSION_ID] {
            let normalized = normalize_sidebar_extensions(vec![SidebarExtension {
                id: id.to_string(),
                installed: false,
                enabled: false,
                visible: false,
            }]);
            let extension = normalized
                .iter()
                .find(|extension| extension.id == id)
                .unwrap();

            assert!(extension.installed);
            assert!(!extension.enabled);
            assert!(!extension.visible);
        }
    }

    #[test]
    fn disabled_mcp_extension_disables_mcp_access() {
        let mut config: GuiConfig =
            serde_json::from_str(r#"{"mcpEnabled":true,"extensions":{"mcp":{"enabled":false}}}"#)
                .unwrap();

        config.fix_defaults();

        assert!(!config.is_extension_enabled(MCP_EXTENSION_ID));
        assert!(!config.mcp_enabled);
    }

    #[test]
    fn disabling_mcp_extension_clears_access_until_explicitly_enabled() {
        let mut config = GuiConfig {
            mcp_enabled: true,
            ..GuiConfig::default()
        };

        config.set_extension_enabled(MCP_EXTENSION_ID, false);
        assert!(!config.is_extension_enabled(MCP_EXTENSION_ID));
        assert!(!config.mcp_enabled);

        config.set_extension_enabled(MCP_EXTENSION_ID, true);
        assert!(config.is_extension_enabled(MCP_EXTENSION_ID));
        assert!(!config.mcp_enabled);
    }

    #[test]
    fn required_sidebar_extensions_remain_visible() {
        for id in ["projects", "packages", "settings"] {
            let normalized = normalize_sidebar_extensions(vec![SidebarExtension {
                id: id.to_string(),
                installed: true,
                enabled: true,
                visible: false,
            }]);
            let extension = normalized
                .iter()
                .find(|extension| extension.id == id)
                .unwrap();

            assert!(extension.visible);
        }
    }

    #[test]
    fn sidebar_layout_changes_cannot_change_extension_runtime_state() {
        let existing = vec![
            SidebarExtension {
                id: "theme".to_string(),
                installed: true,
                enabled: false,
                visible: true,
            },
            SidebarExtension {
                id: "log".to_string(),
                installed: true,
                enabled: true,
                visible: true,
            },
        ];
        let requested = vec![
            SidebarExtension {
                id: "log".to_string(),
                installed: false,
                enabled: false,
                visible: false,
            },
            SidebarExtension {
                id: "theme".to_string(),
                installed: false,
                enabled: true,
                visible: true,
            },
        ];

        let updated = apply_sidebar_extension_layout(existing, requested);

        assert_eq!(updated[0].id, "log");
        assert!(updated[0].installed);
        assert!(updated[0].enabled);
        assert!(!updated[0].visible);
        assert_eq!(updated[1].id, "theme");
        assert!(updated[1].installed);
        assert!(!updated[1].enabled);
        assert!(updated[1].visible);
    }
}
