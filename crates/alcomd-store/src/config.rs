//! File-backed configuration adapter. The application owns permissions, update validation and CAS.
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use alcomd_application::ConfigProxy;

use alcomd_application::{
    BackupCompression, ConfigAppearance, ConfigAppearanceMode, ConfigAppearanceMotion,
    ConfigAppearanceScheme, ConfigBackupSettings, ConfigIntegrationSettings, ConfigLocale,
    ConfigPackageSettings, ConfigPathSettings, ConfigProjectSettings, ConfigRepositoryPackages,
    ConfigSettings, ConfigSnapshot, ConfigStore, ConfigStoreError, ConfigUnitySettings,
    ConfigWindowSettings,
};

const MAX_SETTINGS_BYTES: u64 = 1024 * 1024;
const MAX_BASE_SETTINGS_BYTES: u64 = 16 * 1024;

/// The daemon's bounded, recoverable settings.toml storage adapter.
#[derive(Clone, Debug)]
pub struct FileConfigStore {
    path: PathBuf,
}

impl FileConfigStore {
    pub const SCHEMA_VERSION: u32 = 1;

    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl ConfigStore for FileConfigStore {
    fn initialize(&self) -> Result<ConfigSnapshot, ConfigStoreError> {
        initialize_settings_file(&self.path).map_err(|error| {
            report_config_failure(error, &self.path, "failed to initialize settings")
        })
    }

    fn load(&self) -> Result<ConfigSnapshot, ConfigStoreError> {
        read_settings(&self.path)
            .map_err(|error| report_config_failure(error, &self.path, "failed to read settings"))
    }

    fn publish(&self, snapshot: &ConfigSnapshot) -> Result<(), ConfigStoreError> {
        (|| {
            snapshot
                .settings
                .validate()
                .map_err(|error| ConfigStoreError::Corrupt.with_source(error))?;
            let contents = serialize_settings(snapshot);
            if snapshot.revision == 0 {
                return Err(ConfigStoreError::Corrupt);
            }
            if contents.len() as u64 > MAX_SETTINGS_BYTES
                || serialize_settings_base(snapshot).len() as u64 > MAX_BASE_SETTINGS_BYTES
            {
                return Err(ConfigStoreError::TooLarge);
            }
            publish_settings(&self.path, &contents)
        })()
        .map_err(|error| report_config_failure(error, &self.path, "failed to publish settings"))
    }
}

fn report_config_failure(
    error: ConfigStoreError,
    path: &Path,
    message: &'static str,
) -> ConfigStoreError {
    let error = if std::error::Error::source(&error).is_none() {
        error.with_reason("the settings document or publication state violates the bounded configuration contract")
    } else {
        error
    };
    error
        .with_path(path.to_string_lossy().into_owned())
        .with_reason(message)
}

/// Validates configuration and any interrupted current replacement without writing files.
pub fn preflight_config(path: &Path) -> Result<(), ConfigStoreError> {
    settings_replace_inputs(path)
        .map(|_| ())
        .map_err(|error| report_config_failure(error, path, "failed to preflight settings"))
}

fn initialize_settings_file(path: &Path) -> Result<ConfigSnapshot, ConfigStoreError> {
    if let Some(snapshot) = recover_settings_replace(path)? {
        return Ok(snapshot);
    }
    let snapshot = ConfigSnapshot::default();
    publish_settings(path, &serialize_settings(&snapshot))?;
    Ok(snapshot)
}

fn read_settings(path: &Path) -> Result<ConfigSnapshot, ConfigStoreError> {
    Ok(recover_settings_replace(path)?.unwrap_or_default())
}

fn settings_metadata(path: &Path) -> Result<Option<fs::Metadata>, ConfigStoreError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if !crate::is_regular_state_file(&metadata) => {
            Err(ConfigStoreError::Corrupt.with_reason("settings path is not a regular file"))
        }
        Ok(metadata) if metadata.len() > MAX_SETTINGS_BYTES => {
            Err(ConfigStoreError::Corrupt.with_reason("settings file exceeds the 1 MiB limit"))
        }
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(map_io(error)),
    }
}

fn read_settings_without_recovery(path: &Path) -> Result<ConfigSnapshot, ConfigStoreError> {
    read_optional_settings(path)?.ok_or(ConfigStoreError::Unavailable)
}

fn read_optional_settings(path: &Path) -> Result<Option<ConfigSnapshot>, ConfigStoreError> {
    let Some(metadata) = settings_metadata(path)? else {
        return Ok(None);
    };
    let mut bytes =
        Vec::with_capacity(usize::try_from(metadata.len()).map_err(|_| ConfigStoreError::Corrupt)?);
    File::open(path)
        .map_err(map_io)?
        .take(MAX_SETTINGS_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(map_io)?;
    if bytes.len() as u64 > MAX_SETTINGS_BYTES {
        return Err(ConfigStoreError::Corrupt
            .with_reason("settings file grew beyond the 1 MiB limit while reading"));
    }
    if bytes.contains(&0) {
        return Err(ConfigStoreError::Corrupt.with_reason("settings document contains a NUL byte"));
    }
    parse_settings(
        std::str::from_utf8(&bytes)
            .map_err(|error| ConfigStoreError::Corrupt.with_source(error))?,
    )
    .map(Some)
    .map_err(|error| {
        if error == ConfigStoreError::TooLarge {
            ConfigStoreError::Corrupt.with_source(error)
        } else {
            error
        }
    })
}

// Decode directly to typed integers: TOML's generic Value only represents i64,
// while the settings CAS contract intentionally retains the complete u64 range.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSettings {
    schema: u64,
    revision: u64,
    locale: String,
    appearance: RawAppearance,
    packages: RawPackages,
    unity: Option<RawUnity>,
    paths: Option<RawPaths>,
    window: RawWindow,
    integrations: RawIntegrations,
    network: RawNetwork,
    projects: RawProjects,
    backups: RawBackups,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAppearance {
    mode: String,
    source_color: Option<String>,
    scheme: String,
    motion: String,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPackages {
    hidden_repository_packages: Vec<ConfigRepositoryPackages>,
    show_prerelease: bool,
    prefer_prerelease: bool,
    hidden_repository_ids: Vec<String>,
    hide_local_user_packages: bool,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[derive(Default)]
struct RawUnity {
    hub_path: Option<String>,
    default_launch_arguments: Option<Vec<String>>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[derive(Default)]
struct RawPaths {
    project_directory: Option<String>,
    backup_directory: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWindow {
    remember_state: bool,
    fullscreen: bool,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawIntegrations {
    handle_vcc_links: bool,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNetwork {
    proxy_mode: String,
    proxy_url: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProjects {
    auto_discover: bool,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBackups {
    compression_mode: String,
    include_vpm_packages: bool,
    include_library: bool,
}

fn parse_settings(text: &str) -> Result<ConfigSnapshot, ConfigStoreError> {
    if text.len() as u64 > MAX_SETTINGS_BYTES {
        return Err(ConfigStoreError::TooLarge);
    }
    let raw: RawSettings = toml::from_str(text).map_err(|error: toml::de::Error| {
        // TOML Display and message may quote argv or arbitrary configuration values.
        // Retain the real parser location, never its document or interpolated values.
        let location = error
            .span()
            .map(|span| format!(" at bytes {}..{}", span.start, span.end))
            .unwrap_or_default();
        ConfigStoreError::Corrupt.with_reason(format!(
            "settings TOML does not match the typed document contract{location}"
        ))
    })?;
    if raw.schema != u64::from(FileConfigStore::SCHEMA_VERSION) || raw.revision == 0 {
        return Err(ConfigStoreError::Corrupt);
    }
    let compression_mode = match raw.backups.compression_mode.as_str() {
        "store" => BackupCompression::Store,
        "fast" => BackupCompression::Fast,
        "level3" => BackupCompression::Level3,
        "level5" => BackupCompression::Level5,
        "level7" => BackupCompression::Level7,
        "maximum" => BackupCompression::Maximum,
        _ => return Err(ConfigStoreError::Corrupt),
    };
    let network_proxy = match (raw.network.proxy_mode.as_str(), raw.network.proxy_url) {
        ("system", None) => ConfigProxy::System,
        ("direct", None) => ConfigProxy::Direct,
        ("custom", Some(url)) => {
            ConfigProxy::custom(&url).map_err(|_| ConfigStoreError::Corrupt)?
        }
        _ => return Err(ConfigStoreError::Corrupt),
    };
    let unity = raw.unity.unwrap_or_default();
    let paths = raw.paths.unwrap_or_default();
    let snapshot = ConfigSnapshot {
        revision: raw.revision,
        settings: ConfigSettings {
            projects: ConfigProjectSettings {
                auto_discover: raw.projects.auto_discover,
            },
            backups: ConfigBackupSettings {
                compression_mode,
                include_vpm_packages: raw.backups.include_vpm_packages,
                include_library: raw.backups.include_library,
            },
            integrations: ConfigIntegrationSettings {
                handle_vcc_links: raw.integrations.handle_vcc_links,
            },
            network_proxy,
            window: ConfigWindowSettings {
                fullscreen: raw.window.fullscreen,
                remember_state: raw.window.remember_state,
            },
            appearance: ConfigAppearance {
                mode: parse_mode(&raw.appearance.mode)?,
                source_color: raw.appearance.source_color,
                scheme: parse_scheme(&raw.appearance.scheme)?,
                motion: parse_motion(&raw.appearance.motion)?,
            },
            locale: parse_locale(&raw.locale)?,
            packages: ConfigPackageSettings {
                hidden_repository_packages: raw.packages.hidden_repository_packages,
                show_prerelease: raw.packages.show_prerelease,
                prefer_prerelease: raw.packages.prefer_prerelease,
                hidden_repository_ids: raw.packages.hidden_repository_ids,
                hide_local_user_packages: raw.packages.hide_local_user_packages,
            },
            unity: ConfigUnitySettings {
                hub_path: unity.hub_path,
                default_launch_arguments: unity.default_launch_arguments,
            },
            paths: ConfigPathSettings {
                project_directory: paths.project_directory,
                backup_directory: paths.backup_directory,
            },
        },
    };
    snapshot
        .settings
        .validate()
        .map_err(|_| ConfigStoreError::Corrupt)?;
    if serialize_settings_base(&snapshot).len() as u64 > MAX_BASE_SETTINGS_BYTES {
        return Err(ConfigStoreError::TooLarge);
    }
    Ok(snapshot)
}

fn serialize_settings(snapshot: &ConfigSnapshot) -> Vec<u8> {
    let groups = snapshot
        .settings
        .packages
        .hidden_repository_packages
        .iter()
        .map(|group| {
            let ids = group
                .package_ids
                .iter()
                .map(|id| serde_json::to_string(id).expect("validated package ID"))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "{{ repository_id = \"{}\", package_ids = [{ids}] }}",
                group.repository_id
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    String::from_utf8(serialize_settings_base(snapshot))
        .expect("UTF-8 settings")
        .replacen(
            "[packages]\n",
            &format!("[packages]\nhidden_repository_packages = [{groups}]\n"),
            1,
        )
        .into_bytes()
}

fn serialize_settings_base(snapshot: &ConfigSnapshot) -> Vec<u8> {
    let source = snapshot
        .settings
        .appearance
        .source_color
        .as_ref()
        .map_or_else(String::new, |value| format!("source_color = \"{value}\"\n"));
    let mut contents = format!(
        "schema = {}\nrevision = {}\nlocale = \"{}\"\n\n[appearance]\nmode = \"{}\"\n{}scheme = \"{}\"\nmotion = \"{}\"\n\n[packages]\nshow_prerelease = {}\nprefer_prerelease = {}\nhidden_repository_ids = [{}]\nhide_local_user_packages = {}\n",
        FileConfigStore::SCHEMA_VERSION,
        snapshot.revision,
        locale_str(snapshot.settings.locale),
        mode_str(snapshot.settings.appearance.mode),
        source,
        scheme_str(snapshot.settings.appearance.scheme),
        motion_str(snapshot.settings.appearance.motion),
        snapshot.settings.packages.show_prerelease,
        snapshot.settings.packages.prefer_prerelease,
        snapshot
            .settings
            .packages
            .hidden_repository_ids
            .iter()
            .map(|value| format!("\"{value}\""))
            .collect::<Vec<_>>()
            .join(", "),
        snapshot.settings.packages.hide_local_user_packages,
    );
    let unity = &snapshot.settings.unity;
    if unity.hub_path.is_some() || unity.default_launch_arguments.is_some() {
        contents.push_str("\n[unity]\n");
    }
    if let Some(path) = &unity.hub_path {
        contents.push_str("hub_path = \"");
        for character in path.chars() {
            if matches!(character, '\\' | '"') {
                contents.push('\\');
            }
            contents.push(character);
        }
        contents.push_str("\"\n");
    }
    if let Some(arguments) = &unity.default_launch_arguments {
        contents.push_str("default_launch_arguments = [");
        for (index, argument) in arguments.iter().enumerate() {
            if index > 0 {
                contents.push_str(", ");
            }
            // JSON's canonical string escapes are valid TOML basic-string escapes.
            contents.push_str(
                &serde_json::to_string(argument)
                    .expect("string serialization")
                    .replace('\u{007f}', "\\u007F"),
            );
        }
        contents.push_str("]\n");
    }
    let paths = &snapshot.settings.paths;
    if paths.project_directory.is_some() || paths.backup_directory.is_some() {
        contents.push_str("\n[paths]\n");
        for (key, path) in [
            ("project_directory", &paths.project_directory),
            ("backup_directory", &paths.backup_directory),
        ] {
            if let Some(path) = path {
                contents.push_str(key);
                contents.push_str(" = \"");
                for character in path.chars() {
                    if matches!(character, '\\' | '"') {
                        contents.push('\\');
                    }
                    contents.push(character);
                }
                contents.push_str("\"\n");
            }
        }
    }
    contents.push_str(&format!(
        "\n[window]\nremember_state = {}\nfullscreen = {}\n",
        snapshot.settings.window.remember_state, snapshot.settings.window.fullscreen
    ));
    contents.push_str(&format!(
        "\n[projects]\nauto_discover = {}\n\n[backups]\ncompression_mode = \"{}\"\ninclude_vpm_packages = {}\ninclude_library = {}\n",
        snapshot.settings.projects.auto_discover,
        snapshot.settings.backups.compression_mode.as_str(),
        snapshot.settings.backups.include_vpm_packages,
        snapshot.settings.backups.include_library,
    ));
    contents.push_str(&format!(
        "\n[integrations]\nhandle_vcc_links = {}\n",
        snapshot.settings.integrations.handle_vcc_links
    ));
    contents.push_str("\n[network]\n");
    match &snapshot.settings.network_proxy {
        ConfigProxy::System => contents.push_str("proxy_mode = \"system\"\n"),
        ConfigProxy::Direct => contents.push_str("proxy_mode = \"direct\"\n"),
        ConfigProxy::Custom { url } => {
            contents.push_str("proxy_mode = \"custom\"\nproxy_url = ");
            contents.push_str(&serde_json::to_string(url).expect("string serialization"));
            contents.push('\n');
        }
    }
    contents.into_bytes()
}

fn publish_settings(path: &Path, contents: &[u8]) -> Result<(), ConfigStoreError> {
    if contents.len() as u64 > MAX_SETTINGS_BYTES {
        return Err(ConfigStoreError::TooLarge);
    }
    let parent = path.parent().ok_or(ConfigStoreError::Unavailable)?;
    let had_target = recover_settings_replace(path)?.is_some();
    fs::create_dir_all(parent).map_err(map_io)?;
    let temporary = path.with_extension("toml.new");
    let backup = path.with_extension("toml.bak");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(map_io)?;
    file.write_all(contents).map_err(map_io)?;
    replace_checkpoint("temporary_written")?;
    file.sync_all().map_err(map_io)?;
    drop(file);
    replace_checkpoint("temporary_synced")?;
    alcomd_platform::sync_directory(parent).map_err(map_io)?;
    replace_checkpoint("temporary_directory_synced")?;
    if had_target {
        fs::rename(path, &backup).map_err(map_io)?;
        replace_checkpoint("backup_renamed")?;
        alcomd_platform::sync_directory(parent).map_err(map_io)?;
        replace_checkpoint("backup_directory_synced")?;
    }
    // A failed operation can have an unknown outcome. Keep both the old backup
    // and replacement evidence for the same validation-based recovery path.
    fs::rename(&temporary, path).map_err(map_io)?;
    replace_checkpoint("replacement_renamed")?;
    alcomd_platform::sync_directory(parent).map_err(map_io)?;
    replace_checkpoint("replacement_directory_synced")?;
    let committed = read_settings_without_recovery(path)?;
    if serialize_settings(&committed) != contents {
        return Err(ConfigStoreError::Corrupt);
    }
    if backup.exists() {
        fs::remove_file(backup).map_err(map_io)?;
        replace_checkpoint("backup_removed")?;
        alcomd_platform::sync_directory(parent).map_err(map_io)?;
        replace_checkpoint("cleanup_directory_synced")?;
    }
    Ok(())
}

struct SettingsReplaceInputs {
    current: Option<ConfigSnapshot>,
    backup: Option<ConfigSnapshot>,
    has_temporary: bool,
}

fn settings_replace_inputs(path: &Path) -> Result<SettingsReplaceInputs, ConfigStoreError> {
    let current = read_optional_settings(path)?;
    let backup = read_optional_settings(&path.with_extension("toml.bak"))?;
    let temporary = path.with_extension("toml.new");
    let has_temporary = settings_metadata(&temporary)?.is_some();
    if has_temporary {
        // The writer emits this header first. A short prefix is possible if it
        // stops during write_all. Any other file is unknown and kept intact.
        let header = format!("schema = {}\n", FileConfigStore::SCHEMA_VERSION);
        let mut prefix = Vec::with_capacity(header.len());
        File::open(temporary)
            .map_err(map_io)?
            .take(header.len() as u64)
            .read_to_end(&mut prefix)
            .map_err(map_io)?;
        let has_current_authority = current.is_some() || backup.is_some();
        if !header.as_bytes().starts_with(&prefix)
            || (!has_current_authority && !prefix.is_empty() && prefix.len() < header.len())
        {
            return Err(ConfigStoreError::Corrupt.with_reason("unrecognized settings replacement"));
        }
    }
    Ok(SettingsReplaceInputs {
        current,
        backup,
        has_temporary,
    })
}

fn recover_settings_replace(path: &Path) -> Result<Option<ConfigSnapshot>, ConfigStoreError> {
    let inputs = settings_replace_inputs(path)?;
    let parent = path.parent().ok_or(ConfigStoreError::Unavailable)?;
    let temporary = path.with_extension("toml.new");
    let backup = path.with_extension("toml.bak");
    if let Some(current) = inputs.current {
        if inputs.has_temporary || inputs.backup.is_some() {
            // Establish the observed committed target before discarding replacement evidence.
            alcomd_platform::sync_directory(parent).map_err(map_io)?;
            if inputs.has_temporary {
                fs::remove_file(&temporary).map_err(map_io)?;
            }
            if inputs.backup.is_some() {
                fs::remove_file(&backup).map_err(map_io)?;
            }
            alcomd_platform::sync_directory(parent).map_err(map_io)?;
        }
        return Ok(Some(current));
    }
    if inputs.backup.is_some() {
        fs::rename(&backup, path).map_err(map_io)?;
        alcomd_platform::sync_directory(parent).map_err(map_io)?;
    }
    if inputs.has_temporary {
        fs::remove_file(temporary).map_err(map_io)?;
        alcomd_platform::sync_directory(parent).map_err(map_io)?;
    }
    Ok(inputs.backup)
}

// Fault gates exist only in explicit test builds. Ordinary production settings
// publication has no environment-controlled failure or waiting path.
fn replace_checkpoint(_checkpoint: &str) -> Result<(), ConfigStoreError> {
    #[cfg(test)]
    if REPLACE_FAILURE.with(|failure| failure.get() == Some(_checkpoint)) {
        return Err(ConfigStoreError::Unavailable);
    }
    #[cfg(feature = "test-kill-gates")]
    if std::env::var("ALCOMD_TEST_CONFIG_KILL_GATE").as_deref() == Ok(_checkpoint) {
        let signal = std::env::var_os("ALCOMD_TEST_CONFIG_KILL_SIGNAL")
            .ok_or(ConfigStoreError::Unavailable)?;
        fs::write(signal, _checkpoint).map_err(map_io)?;
        loop {
            std::thread::park();
        }
    }
    Ok(())
}

#[cfg(test)]
thread_local! {
    static REPLACE_FAILURE: std::cell::Cell<Option<&'static str>> = const { std::cell::Cell::new(None) };
}

fn parse_mode(value: &str) -> Result<ConfigAppearanceMode, ConfigStoreError> {
    match value {
        "system" => Ok(ConfigAppearanceMode::System),
        "light" => Ok(ConfigAppearanceMode::Light),
        "dark" => Ok(ConfigAppearanceMode::Dark),
        _ => Err(ConfigStoreError::Corrupt),
    }
}
fn parse_scheme(value: &str) -> Result<ConfigAppearanceScheme, ConfigStoreError> {
    match value {
        "tonal-spot" => Ok(ConfigAppearanceScheme::TonalSpot),
        "fidelity" => Ok(ConfigAppearanceScheme::Fidelity),
        "monochrome" => Ok(ConfigAppearanceScheme::Monochrome),
        "neutral" => Ok(ConfigAppearanceScheme::Neutral),
        "vibrant" => Ok(ConfigAppearanceScheme::Vibrant),
        "expressive" => Ok(ConfigAppearanceScheme::Expressive),
        "content" => Ok(ConfigAppearanceScheme::Content),
        "rainbow" => Ok(ConfigAppearanceScheme::Rainbow),
        "fruit-salad" => Ok(ConfigAppearanceScheme::FruitSalad),
        _ => Err(ConfigStoreError::Corrupt),
    }
}
fn parse_motion(value: &str) -> Result<ConfigAppearanceMotion, ConfigStoreError> {
    match value {
        "system" => Ok(ConfigAppearanceMotion::System),
        "full" => Ok(ConfigAppearanceMotion::Full),
        "reduced" => Ok(ConfigAppearanceMotion::Reduced),
        _ => Err(ConfigStoreError::Corrupt),
    }
}
fn parse_locale(value: &str) -> Result<ConfigLocale, ConfigStoreError> {
    match value {
        "system" => Ok(ConfigLocale::System),
        "en-US" => Ok(ConfigLocale::EnUs),
        "zh-CN" => Ok(ConfigLocale::ZhCn),
        "ja-JP" => Ok(ConfigLocale::JaJp),
        _ => Err(ConfigStoreError::Corrupt),
    }
}
fn mode_str(value: ConfigAppearanceMode) -> &'static str {
    match value {
        ConfigAppearanceMode::System => "system",
        ConfigAppearanceMode::Light => "light",
        ConfigAppearanceMode::Dark => "dark",
    }
}
fn scheme_str(value: ConfigAppearanceScheme) -> &'static str {
    match value {
        ConfigAppearanceScheme::TonalSpot => "tonal-spot",
        ConfigAppearanceScheme::Fidelity => "fidelity",
        ConfigAppearanceScheme::Monochrome => "monochrome",
        ConfigAppearanceScheme::Neutral => "neutral",
        ConfigAppearanceScheme::Vibrant => "vibrant",
        ConfigAppearanceScheme::Expressive => "expressive",
        ConfigAppearanceScheme::Content => "content",
        ConfigAppearanceScheme::Rainbow => "rainbow",
        ConfigAppearanceScheme::FruitSalad => "fruit-salad",
    }
}
fn motion_str(value: ConfigAppearanceMotion) -> &'static str {
    match value {
        ConfigAppearanceMotion::System => "system",
        ConfigAppearanceMotion::Full => "full",
        ConfigAppearanceMotion::Reduced => "reduced",
    }
}
fn locale_str(value: ConfigLocale) -> &'static str {
    match value {
        ConfigLocale::System => "system",
        ConfigLocale::EnUs => "en-US",
        ConfigLocale::ZhCn => "zh-CN",
        ConfigLocale::JaJp => "ja-JP",
    }
}
fn map_io(error: io::Error) -> ConfigStoreError {
    ConfigStoreError::Unavailable.with_source(error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alcomd_application::{
        AccessContext, ConfigAppearanceUpdate, ConfigNullableUpdate, ConfigPackageSettingsUpdate,
        ConfigUpdate, M7OfficialApplication, OfficialGuiError,
    };
    use std::sync::Arc;

    struct TestDirectories;
    impl alcomd_application::StorageDirectoryPlatform for TestDirectories {
        fn default_directories(
            &self,
        ) -> Result<alcomd_application::StorageDirectories, alcomd_application::OfficialGuiError>
        {
            Ok(alcomd_application::StorageDirectories {
                project_directory: "/fixtures/projects".into(),
                backup_directory: "/fixtures/backups".into(),
            })
        }
        fn normalize_directory(
            &self,
            path: &str,
        ) -> Result<String, alcomd_application::OfficialGuiError> {
            if path.starts_with('/') || path.as_bytes().get(1) == Some(&b':') {
                Ok(path.to_owned())
            } else {
                Err(alcomd_application::OfficialGuiError::InvalidInput)
            }
        }
    }

    struct TestHub;

    struct TestHubLauncher;

    async fn issue_settings_client(
        identities: &alcomd_application::McpIdentityApplication<crate::StateStoreHandle>,
    ) -> (alcomd_application::McpClient, [u8; 32], AccessContext) {
        use alcomd_application::McpIdentityWrite;
        let owner = AccessContext::local_owner();
        let client = identities
            .write(
                &owner,
                "create-client".into(),
                McpIdentityWrite::Create {
                    name: "Settings client".into(),
                },
                1,
            )
            .await
            .expect("create caller")
            .client;
        let hash = alcomd_application::mcp_credential_hash(&uuid::Uuid::new_v4().to_string());
        let client = identities
            .write(
                &owner,
                "issue-client".into(),
                McpIdentityWrite::Issue {
                    id: client.client_id,
                    expected: client.revision,
                    hash,
                },
                2,
            )
            .await
            .expect("issue credential")
            .client;
        let caller = identities
            .authenticate(hash)
            .await
            .expect("authenticate caller");
        (client, hash, caller)
    }

    #[tokio::test]
    async fn hub_open_rechecks_the_waiting_mcp_credential_before_launch() {
        use alcomd_application::{
            McpIdentityApplication, McpIdentityWrite, ToolAuthentication,
            UnityHubConfiguredPathStatus, UnityHubLauncher, UnityHubLocation, UnityHubOpenError,
            UnityHubOpenOutcome, UnityHubPlatform, UnityHubSource, UnityHubStatus,
            with_workflow_authentication,
        };
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::task::Poll;

        use tokio::sync::oneshot;
        struct Hub {
            location: UnityHubLocation,
            launches: AtomicUsize,
            inspection: std::sync::Mutex<Option<(oneshot::Sender<()>, oneshot::Receiver<()>)>>,
        }
        impl UnityHubPlatform for Hub {
            fn normalize_path(&self, _: &str) -> Result<String, OfficialGuiError> {
                unreachable!("open only uses the configured target")
            }
            fn inspect(&self, _: Option<&str>) -> UnityHubLocation {
                if let Some((entered, resume)) =
                    self.inspection.lock().expect("inspection gate").take()
                {
                    entered.send(()).expect("signal inspection");
                    resume.blocking_recv().expect("resume inspection");
                }
                self.location.clone()
            }
        }
        impl UnityHubLauncher for Hub {
            fn launch(&self, target: &str) -> Result<(), UnityHubOpenError> {
                assert_eq!(Some(target), self.location.path.as_deref());
                self.launches.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        }
        let directory = crate::events_tests::Directory::new();
        let database = directory.database();
        let path = database.with_file_name("settings.toml");
        let store = crate::StateStoreHandle::open(database).expect("isolated state");
        let identities = McpIdentityApplication::new(store.clone());
        let hub = Arc::new(Hub {
            location: UnityHubLocation {
                status: UnityHubStatus::Available,
                source: UnityHubSource::Configured,
                registered_target: None,
                configured_path_status: UnityHubConfiguredPathStatus::Available,
                path: Some("/fixture/hub".into()),
            },
            launches: AtomicUsize::new(0),
            inspection: Default::default(),
        });
        let application = M7OfficialApplication::new(
            Arc::new(FileConfigStore::new(path)),
            hub.clone(),
            hub.clone(),
            Arc::new(TestDirectories),
        )
        .with_mcp_identity_store(store);
        application.initialize_settings().await.expect("settings");
        let revision = application
            .candidate_settings()
            .await
            .expect("snapshot")
            .revision;
        let (client, hash, caller) = issue_settings_client(&identities).await;
        let open = || {
            with_workflow_authentication(
                ToolAuthentication::Credential(hash),
                application.open_unity_hub(&caller, revision, hub.location.target_token()),
            )
        };
        assert_eq!(open().await, Ok(UnityHubOpenOutcome::LaunchRequested));
        let (entered, inspecting) = oneshot::channel();
        let (resume, gate) = oneshot::channel();
        *hub.inspection.lock().expect("inspection gate") = Some((entered, gate));
        let mut first = std::pin::pin!(open());
        tokio::select! {
            result = &mut first => panic!("completed before inspecting: {result:?}"),
            signal = inspecting => signal.expect("first request is inspecting its target"),
        }
        let mut pending = std::pin::pin!(open());
        std::future::poll_fn(|cx| {
            assert!(pending.as_mut().poll(cx).is_pending());
            Poll::Ready(())
        })
        .await;
        identities
            .write(
                &AccessContext::local_owner(),
                "revoke-hub".into(),
                McpIdentityWrite::Revoke {
                    id: client.client_id,
                    expected: client.revision,
                },
                3,
            )
            .await
            .expect("revoke while open is queued");
        resume.send(()).expect("finish inspecting target");
        assert_eq!(first.await, Err(UnityHubOpenError::PermissionDenied));
        assert_eq!(pending.await, Err(UnityHubOpenError::PermissionDenied));
        assert_eq!(hub.launches.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn settings_update_rechecks_mcp_before_publishing_after_authorization_waits() {
        use alcomd_application as app;
        use tokio::sync::oneshot;

        struct DirectoryCheck {
            gate: std::sync::Mutex<Option<(oneshot::Sender<()>, oneshot::Receiver<()>)>>,
        }
        impl app::StorageDirectoryPlatform for DirectoryCheck {
            fn default_directories(&self) -> Result<app::StorageDirectories, OfficialGuiError> {
                TestDirectories.default_directories()
            }
            fn normalize_directory(&self, path: &str) -> Result<String, OfficialGuiError> {
                let (entered, resume) = self
                    .gate
                    .lock()
                    .expect("validation gate")
                    .take()
                    .expect("one path check");
                entered.send(()).expect("signal validation");
                resume.blocking_recv().expect("finish validation");
                TestDirectories.normalize_directory(path)
            }
        }
        let directory = crate::events_tests::Directory::new();
        let database = directory.database();
        let path = database.with_file_name("settings.toml");
        let store = crate::StateStoreHandle::open(database).expect("isolated identity database");
        let identities = app::McpIdentityApplication::new(store.clone());
        let (client, hash, caller) = issue_settings_client(&identities).await;
        let (entered, validating) = oneshot::channel();
        let (resume, gate) = oneshot::channel();
        let application = M7OfficialApplication::new(
            Arc::new(FileConfigStore::new(path.clone())),
            Arc::new(TestHub),
            Arc::new(TestHubLauncher),
            Arc::new(DirectoryCheck {
                gate: std::sync::Mutex::new(Some((entered, gate))),
            }),
        )
        .with_mcp_identity_store(store);
        application
            .initialize_settings()
            .await
            .expect("initialize settings");
        let update = |revision, patch| {
            app::with_workflow_authentication(
                app::ToolAuthentication::Credential(hash),
                application.update_settings(&caller, revision, patch),
            )
        };
        let initial = application
            .candidate_settings()
            .await
            .expect("initial settings");
        let accepted = update(
            initial.revision,
            ConfigUpdate {
                locale: Some(ConfigLocale::JaJp),
                ..Default::default()
            },
        )
        .await
        .expect("authenticated write");
        assert_eq!(accepted.revision, initial.revision + 1);
        assert_eq!(accepted.settings.locale, ConfigLocale::JaJp);
        let bytes = fs::read(&path).expect("published settings");
        let mut validating_update = std::pin::pin!(update(
            accepted.revision,
            ConfigUpdate {
                paths: Some(app::ConfigPathUpdate {
                    project_directory: ConfigNullableUpdate::Set("/fixtures/other-projects".into()),
                    backup_directory: ConfigNullableUpdate::Unchanged
                }),
                ..Default::default()
            }
        ));
        tokio::select! {
            result = &mut validating_update => panic!("unexpected early update: {result:?}"),
            ready = validating => ready.expect("project directory validation under settings lock"),
        }
        let mut queued = std::pin::pin!(update(
            accepted.revision,
            ConfigUpdate {
                projects: Some(app::ConfigProjectUpdate {
                    auto_discover: false
                }),
                unity: Some(app::ConfigUnityUpdate {
                    hub_path: ConfigNullableUpdate::Unchanged,
                    default_launch_arguments: ConfigNullableUpdate::Set(vec![])
                }),
                ..Default::default()
            }
        ));
        std::future::poll_fn(|cx| {
            assert!(queued.as_mut().poll(cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        identities
            .write(
                &AccessContext::local_owner(),
                "revoke-settings".into(),
                app::McpIdentityWrite::Revoke {
                    id: client.client_id,
                    expected: client.revision,
                },
                3,
            )
            .await
            .expect("revoke while settings requests are pending");
        resume.send(()).expect("resume validation");
        assert_eq!(
            validating_update.await,
            Err(OfficialGuiError::PermissionDenied)
        );
        assert_eq!(queued.await, Err(OfficialGuiError::PermissionDenied));
        assert_eq!(application.candidate_settings().await, Ok(accepted.clone()));
        assert_eq!(fs::read(&path).expect("unchanged file"), bytes);
        assert_eq!(FileConfigStore::new(path).load(), Ok(accepted.clone()));
        let local = application
            .update_settings(
                &AccessContext::local_owner(),
                accepted.revision,
                ConfigUpdate {
                    projects: Some(app::ConfigProjectUpdate {
                        auto_discover: false,
                    }),
                    ..Default::default()
                },
            )
            .await
            .expect("local owner can still update from the unchanged revision");
        assert_eq!(local.revision, accepted.revision + 1);
        assert!(!local.settings.projects.auto_discover);
    }

    impl alcomd_application::UnityHubLauncher for TestHubLauncher {
        fn launch(&self, _: &str) -> Result<(), alcomd_application::UnityHubOpenError> {
            unreachable!("configuration tests must never execute a Hub process")
        }
    }

    impl alcomd_application::UnityHubPlatform for TestHub {
        fn normalize_path(&self, path: &str) -> Result<String, OfficialGuiError> {
            Ok(path.to_owned())
        }

        fn inspect(&self, _: Option<&str>) -> alcomd_application::UnityHubLocation {
            unreachable!("storage tests must not inspect the filesystem through the application")
        }
    }

    fn default_snapshot() -> ConfigSnapshot {
        ConfigSnapshot::default()
    }

    fn validate_hidden_repository_ids(
        values: Vec<String>,
    ) -> Result<Vec<String>, OfficialGuiError> {
        let mut settings = ConfigSnapshot::default().settings;
        settings.packages.hidden_repository_ids = values;
        settings.validate()?;
        Ok(settings.packages.hidden_repository_ids)
    }

    fn temporary_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("alcomd-m7-settings-{}-{name}", std::process::id()))
    }

    #[test]
    fn config_failure_preserves_path_and_parser_position_without_configuration_secrets() {
        let path = temporary_path("真实日志路径-configuration-secret.toml");
        let mut snapshot = default_snapshot();
        snapshot.settings.unity.default_launch_arguments =
            Some(vec!["sensitive-argv-marker".to_owned()]);
        let text = String::from_utf8(serialize_settings(&snapshot))
            .expect("settings UTF-8")
            .replace("schema = 1", "schema = 'sensitive-schema-marker'");
        fs::write(&path, text).expect("isolated malformed settings");
        let error = FileConfigStore::new(path.clone())
            .load()
            .expect_err("typed TOML rejects string schema");
        assert_eq!(
            error.kind(),
            alcomd_application::ConfigStoreErrorKind::Corrupt
        );
        assert_eq!(error.path(), Some(path.to_string_lossy().as_ref()));
        let chain = alcomd_application::technical_error_chain(&error);
        assert!(chain.contains("typed document contract at bytes"));
        assert!(!chain.contains("sensitive-schema-marker"));
        assert!(!chain.contains("sensitive-argv-marker"));
        let propagated = OfficialGuiError::from(error);
        assert_eq!(propagated.path(), Some(path.to_string_lossy().as_ref()));
        assert!(
            alcomd_application::technical_error_chain(&propagated)
                .contains("typed document contract at bytes")
        );
        fs::remove_file(path).expect("remove isolated invalid settings");
    }

    #[test]
    fn config_io_failure_retains_original_cause_and_real_path() {
        let blocked_parent = temporary_path("真实-I-O-settings-parent");
        fs::write(&blocked_parent, b"private parent is a regular file")
            .expect("isolated blocked parent");
        let path = blocked_parent.join("settings.toml");
        let error = FileConfigStore::new(path.clone())
            .publish(&default_snapshot())
            .expect_err("cannot publish over a directory");
        assert_eq!(error.path(), Some(path.to_string_lossy().as_ref()));
        assert!(std::error::Error::source(&error).is_some());
        fs::remove_file(blocked_parent).expect("remove private parent fixture");
    }

    #[test]
    fn deterministic_roundtrip_and_strict_unknown_field() {
        let snapshot = default_snapshot();
        let encoded = serialize_settings(&snapshot);
        assert_eq!(
            parse_settings(std::str::from_utf8(&encoded).expect("utf8")),
            Ok(snapshot)
        );
        assert_eq!(
            parse_settings("schema = 1\nunknown = 2\n"),
            Err(ConfigStoreError::Corrupt)
        );
    }

    #[test]
    fn equivalent_toml_syntax_and_unicode_arrays_preserve_settings() {
        let mut snapshot = default_snapshot();
        snapshot.settings.paths.project_directory = Some("/fixture/项目".to_owned());
        snapshot.settings.unity.default_launch_arguments = Some(vec![
            "-logFile".to_owned(),
            "/fixture/日志 output.txt".to_owned(),
        ]);
        let canonical = String::from_utf8(serialize_settings(&snapshot)).expect("UTF-8");
        let variants = [
            canonical
                .replace("schema = 1", "schema = 1 # current schema")
                .replace("mode = \"system\"", "mode = 'system' # same value"),
            canonical
                .replace("[appearance]", "[\"appearance\"]")
                .replace("locale = \"system\"", "'locale' = 'system'"),
            canonical.replace("\"system\"", "\"syst\\u0065m\"").replace(
                "[\"-logFile\", \"/fixture/日志 output.txt\"]",
                "[\n '-logFile', # argument\n '/fixture/日志 output.txt',\n]",
            ),
        ];
        for variant in variants {
            assert_eq!(parse_settings(&variant), Ok(snapshot.clone()), "{variant}");
        }
        for invalid in [
            canonical.replace("revision = 1", "revision = 1\nrevision = 2"),
            canonical.replace("[appearance]", "[appearance]\nunknown = true"),
            canonical.replace("remember_state = false", "remember_state = 'false'"),
            canonical.replace("[paths]", "[paths]\nunknown = 'ignored'"),
        ] {
            assert_eq!(parse_settings(&invalid), Err(ConfigStoreError::Corrupt));
        }
    }

    #[test]
    fn revision_retains_full_u64_range_in_typed_toml() {
        for revision in [1, i64::MAX as u64, i64::MAX as u64 + 1, u64::MAX] {
            let mut snapshot = default_snapshot();
            snapshot.revision = revision;
            let encoded = serialize_settings(&snapshot);
            assert_eq!(
                parse_settings(std::str::from_utf8(&encoded).expect("UTF-8")),
                Ok(snapshot),
            );
        }
    }

    #[test]
    fn replacement_failure_at_each_boundary_recovers_valid_old_or_new_authority() {
        for (index, checkpoint) in [
            "temporary_written",
            "temporary_synced",
            "temporary_directory_synced",
            "backup_renamed",
            "backup_directory_synced",
            "replacement_renamed",
            "replacement_directory_synced",
            "backup_removed",
            "cleanup_directory_synced",
        ]
        .into_iter()
        .enumerate()
        {
            let root = temporary_path(&format!("replacement-failure-{checkpoint}"));
            fs::create_dir_all(&root).expect("fixture");
            let path = root.join("settings.toml");
            let store = FileConfigStore::new(path.clone());
            let old = store.initialize().expect("old authority");
            let old_bytes = fs::read(&path).expect("old bytes");
            let mut new = old.clone();
            new.revision += 1;
            new.settings.locale = ConfigLocale::ZhCn;
            REPLACE_FAILURE.with(|failure| failure.set(Some(checkpoint)));
            let result = store.publish(&new);
            REPLACE_FAILURE.with(|failure| failure.set(None));
            assert_eq!(result, Err(ConfigStoreError::Unavailable), "{checkpoint}");
            let recovered = store.initialize().expect("recover valid authority");
            assert_eq!(recovered, if index < 5 { old } else { new });
            if index < 5 {
                assert_eq!(fs::read(&path).expect("retained old bytes"), old_bytes);
            }
            assert!(!path.with_extension("toml.new").exists());
            assert!(!path.with_extension("toml.bak").exists());
            fs::remove_dir_all(root).expect("cleanup");
        }
    }

    #[test]
    fn recovery_preserves_invalid_target_and_valid_backup_for_manual_resolution() {
        let root = temporary_path("replacement-invalid-target");
        fs::create_dir_all(&root).expect("fixture");
        let path = root.join("settings.toml");
        let old_bytes = serialize_settings(&default_snapshot());
        fs::write(&path, "invalid replacement").expect("invalid target");
        fs::write(path.with_extension("toml.bak"), &old_bytes).expect("old evidence");
        fs::write(path.with_extension("toml.new"), "unpublished evidence").expect("new evidence");
        assert_eq!(
            FileConfigStore::new(path.clone()).initialize(),
            Err(ConfigStoreError::Corrupt)
        );
        assert_eq!(
            fs::read_to_string(&path).expect("target evidence"),
            "invalid replacement"
        );
        assert_eq!(
            fs::read(path.with_extension("toml.bak")).expect("old evidence"),
            old_bytes
        );
        assert_eq!(
            fs::read_to_string(path.with_extension("toml.new")).expect("new evidence"),
            "unpublished evidence"
        );
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[cfg(feature = "test-kill-gates")]
    #[test]
    fn configuration_process_kill_child() {
        let Some(path) = std::env::var_os("ALCOMD_TEST_CONFIG_CHILD_PATH") else {
            return;
        };
        let store = FileConfigStore::new(PathBuf::from(path));
        let mut snapshot = store.load().expect("child old authority");
        snapshot.revision += 1;
        snapshot.settings.locale = ConfigLocale::ZhCn;
        store
            .publish(&snapshot)
            .expect("child pauses at requested gate");
        panic!("requested process-kill gate was not reached");
    }

    #[cfg(feature = "test-kill-gates")]
    #[test]
    fn killed_settings_writer_recovers_at_each_persistence_boundary() {
        for (index, checkpoint) in [
            "temporary_written",
            "temporary_synced",
            "temporary_directory_synced",
            "backup_renamed",
            "backup_directory_synced",
            "replacement_renamed",
            "replacement_directory_synced",
            "backup_removed",
            "cleanup_directory_synced",
        ]
        .into_iter()
        .enumerate()
        {
            let root = temporary_path(&format!("process-kill-{checkpoint}"));
            fs::create_dir_all(&root).expect("fixture");
            let path = root.join("settings.toml");
            let signal = root.join("checkpoint.txt");
            let store = FileConfigStore::new(path.clone());
            let old = store.initialize().expect("accepted authority");
            let mut child =
                std::process::Command::new(std::env::current_exe().expect("test executable"))
                    .args([
                        "--exact",
                        "config::tests::configuration_process_kill_child",
                        "--nocapture",
                    ])
                    .env("ALCOMD_TEST_CONFIG_CHILD_PATH", &path)
                    .env("ALCOMD_TEST_CONFIG_KILL_GATE", checkpoint)
                    .env("ALCOMD_TEST_CONFIG_KILL_SIGNAL", &signal)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .expect("spawn owned isolated writer");
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            while !signal.exists() && std::time::Instant::now() < deadline {
                if child.try_wait().expect("child status").is_some() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            let reached = signal.exists();
            let _ = child.kill();
            child.wait().expect("reap only owned child");
            assert!(reached, "checkpoint not reached: {checkpoint}");
            let recovered = store
                .initialize()
                .expect("recover after process termination");
            let mut new = old.clone();
            new.revision += 1;
            new.settings.locale = ConfigLocale::ZhCn;
            assert_eq!(recovered, if index < 5 { old } else { new });
            assert!(!path.with_extension("toml.new").exists());
            assert!(!path.with_extension("toml.bak").exists());
            fs::remove_dir_all(root).expect("cleanup");
        }
    }

    #[test]
    fn backup_defaults_and_discovery_roundtrip_reopen_and_reject_invalid_files() {
        let root = temporary_path("config-project-backup-defaults");
        let path = root.join("settings.toml");
        let store = FileConfigStore::new(path.clone());
        let original = store.initialize().expect("current defaults");
        assert!(original.settings.projects.auto_discover);
        assert_eq!(original.settings.backups, ConfigBackupSettings::default());
        let mut expected = original;
        expected.settings.window.remember_state = true;
        expected.settings.unity.default_launch_arguments = Some(Vec::new());
        expected.settings.paths.project_directory = Some("/fixture/retained/projects".into());
        for compression in [
            BackupCompression::Store,
            BackupCompression::Fast,
            BackupCompression::Level3,
            BackupCompression::Level5,
            BackupCompression::Level7,
            BackupCompression::Maximum,
        ] {
            for include_packages in [false, true] {
                for include_library in [false, true] {
                    expected.revision += 1;
                    expected.settings.projects.auto_discover = !include_library;
                    expected.settings.backups = ConfigBackupSettings {
                        compression_mode: compression,
                        include_vpm_packages: include_packages,
                        include_library,
                    };
                    store.publish(&expected).expect("publish settings");
                    assert_eq!(
                        FileConfigStore::new(path.clone()).initialize(),
                        Ok(expected.clone())
                    );
                }
            }
        }
        let current = fs::read_to_string(&path).expect("current settings file");
        for invalid in [
            current.replace("auto_discover = false", "auto_discover = null"),
            current.replace("auto_discover = false", "auto_discover = \"false\""),
            current.replace("auto_discover = false\n", ""),
            current.replace(
                "compression_mode = \"maximum\"",
                "compression_mode = \"normal\"",
            ),
            current.replace("compression_mode = \"maximum\"\n", ""),
            current.replace("include_vpm_packages = true", "include_vpm_packages = 1"),
            current.replace("include_library = true", "include_library = null"),
            current.replace("include_library = true\n", ""),
            current.replace(
                "include_library = true",
                "include_library = true\nunknown = true",
            ),
            current
                .replace("fullscreen = false\n", "")
                .replace("fullscreen = true\n", "")
                .replace("hidden_repository_packages = []\n", "")
                .replace("schema = 1", "schema = 7")
                .replace("\n[network]\nproxy_mode = \"system\"\n", "")
                .replace("\n[integrations]\nhandle_vcc_links = true\n", ""),
        ] {
            fs::write(&path, &invalid).expect("invalid fixture");
            assert_eq!(store.initialize(), Err(ConfigStoreError::Corrupt));
            assert_eq!(
                fs::read_to_string(&path).expect("invalid input preserved"),
                invalid
            );
        }
        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[tokio::test]
    async fn window_preference_is_durable_and_invalid_mixed_updates_leave_accepted_file() {
        use alcomd_application::{ConfigUnityUpdate, ConfigWindowUpdate};
        let root = temporary_path("config-window-save");
        let path = root.join("settings.toml");
        fs::create_dir_all(&root).expect("fixture");
        let mut expected = default_snapshot();
        expected.revision = 168;
        expected.settings.locale = ConfigLocale::JaJp;
        expected.settings.appearance.mode = ConfigAppearanceMode::Dark;
        expected.settings.appearance.source_color = Some("#112233".into());
        expected.settings.appearance.scheme = ConfigAppearanceScheme::FruitSalad;
        expected.settings.appearance.motion = ConfigAppearanceMotion::Full;
        expected.settings.packages.show_prerelease = true;
        expected.settings.packages.hide_local_user_packages = true;
        expected.settings.packages.hidden_repository_ids =
            vec!["00000000-0000-0000-0000-000000000001".into()];
        expected.settings.unity.hub_path = Some("/fixture/Hub".into());
        expected.settings.unity.default_launch_arguments = Some(vec![
            "-logFile".into(),
            r#"C:\用户 Data\a "quoted".log"#.into(),
        ]);
        expected.settings.paths.project_directory = Some("/fixture/projects".into());
        expected.settings.paths.backup_directory = Some("/fixture/backups".into());
        fs::write(&path, serialize_settings(&expected)).expect("current preferences");
        let store = Arc::new(FileConfigStore::new(path.clone()));
        let app = M7OfficialApplication::new(
            store.clone(),
            Arc::new(TestHub),
            Arc::new(TestHubLauncher),
            Arc::new(TestDirectories),
        );
        app.initialize_settings()
            .await
            .expect("load current configuration");
        assert_eq!(store.load(), Ok(expected.clone()));
        for remember_state in [true, false] {
            let saved = app
                .update_settings(
                    &AccessContext::local_owner(),
                    expected.revision,
                    ConfigUpdate {
                        projects: None,
                        backups: None,
                        integrations: None,
                        network_proxy: None,
                        window: Some(ConfigWindowUpdate {
                            remember_state: Some(remember_state),
                            fullscreen: None,
                        }),
                        paths: None,
                        unity: None,
                        appearance: None,
                        locale: None,
                        packages: None,
                    },
                )
                .await
                .expect("save preference");
            expected.revision += 1;
            expected.settings.window.remember_state = remember_state;
            assert_eq!(saved, expected);
            assert_eq!(
                FileConfigStore::new(path.clone()).initialize(),
                Ok(expected.clone())
            );
        }
        let before = fs::read(&path).expect("accepted file");
        assert_eq!(
            app.update_settings(
                &AccessContext::local_owner(),
                expected.revision,
                ConfigUpdate {
                    projects: None,
                    backups: None,
                    integrations: None,
                    network_proxy: None,
                    window: Some(ConfigWindowUpdate {
                        remember_state: Some(true),
                        fullscreen: None
                    }),
                    unity: Some(ConfigUnityUpdate {
                        hub_path: ConfigNullableUpdate::Unchanged,
                        default_launch_arguments: ConfigNullableUpdate::Set(vec![
                            "a".repeat(4096);
                            5
                        ]),
                    }),
                    paths: None,
                    appearance: None,
                    locale: None,
                    packages: None,
                }
            )
            .await,
            Err(OfficialGuiError::InvalidInput)
        );
        assert_eq!(fs::read(&path).expect("retained accepted file"), before);
        assert_eq!(store.load(), Ok(expected));
        let valid = String::from_utf8(before).expect("utf8");
        let mut invalid_files = ["null", "\"true\"", "1", "[]"]
            .map(|value| {
                valid.replace(
                    "remember_state = false",
                    &format!("remember_state = {value}"),
                )
            })
            .to_vec();
        invalid_files.push(valid.replace("\n[window]\nremember_state = false\n", ""));
        invalid_files.push(valid.replace("remember_state = false", ""));
        invalid_files.push(valid.replace(
            "remember_state = false",
            "remember_state = false\nwidth = 1000",
        ));
        invalid_files.push(valid.replace(
            "remember_state = false",
            "remember_state = false\nremember_state = true",
        ));
        for invalid in invalid_files {
            fs::write(&path, &invalid).expect("invalid file");
            assert_eq!(store.initialize(), Err(ConfigStoreError::Corrupt));
            assert_eq!(fs::read_to_string(&path).expect("input retained"), invalid);
        }
        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[tokio::test]
    async fn arguments_reopen_empty_clear_and_atomic_failure() {
        use alcomd_application::ConfigUnityUpdate;
        let root = temporary_path("config-default-arguments");
        let path = root.join("settings.toml");
        let store = Arc::new(FileConfigStore::new(path.clone()));
        let app = M7OfficialApplication::new(
            store.clone(),
            Arc::new(TestHub),
            Arc::new(TestHubLauncher),
            Arc::new(TestDirectories),
        );
        app.initialize_settings().await.expect("initialize");
        let mut expected = store.load().expect("initialized snapshot");
        let update = |arguments| ConfigUpdate {
            projects: None,
            backups: None,
            integrations: None,
            network_proxy: None,
            window: None,
            paths: None,
            appearance: None,
            locale: None,
            packages: None,
            unity: Some(ConfigUnityUpdate {
                hub_path: ConfigNullableUpdate::Unchanged,
                default_launch_arguments: arguments,
            }),
        };
        let literal = vec![
            "-logFile".into(),
            r#"C:\用户 Data\Unity\a "quoted", [file].log"#.into(),
            "one\ntwo\tthree\r\u{0008}\u{000c}\u{007f}\u{0085}😀".into(),
        ];
        for arguments in [
            ConfigNullableUpdate::Set(literal),
            ConfigNullableUpdate::Set(Vec::new()),
            ConfigNullableUpdate::Clear,
        ] {
            expected = app
                .update_settings(
                    &AccessContext::local_owner(),
                    expected.revision,
                    update(arguments.clone()),
                )
                .await
                .expect("save arguments");
            assert_eq!(
                FileConfigStore::new(path.clone()).initialize(),
                Ok(expected.clone())
            );
            let text = fs::read_to_string(&path).expect("saved file");
            assert_eq!(
                text.contains("default_launch_arguments ="),
                !matches!(arguments, ConfigNullableUpdate::Clear)
            );
            assert_eq!(parse_settings(&text), Ok(expected.clone()));
        }
        let before = fs::read(&path).expect("before rejected writes");
        for arguments in [vec!["bad\0argument".into()], vec!["a".repeat(4096); 5]] {
            let mut mixed = update(ConfigNullableUpdate::Set(arguments));
            mixed.locale = Some(ConfigLocale::JaJp);
            assert_eq!(
                app.update_settings(&AccessContext::local_owner(), expected.revision, mixed)
                    .await,
                Err(OfficialGuiError::InvalidInput)
            );
            assert_eq!(fs::read(&path).expect("accepted file retained"), before);
            assert_eq!(store.load(), Ok(expected.clone()));
        }
        for invalid in [
            "null",
            "\"-batchmode\"",
            "[1]",
            "[true]",
            "[\"bad\\q\"]",
            "[\"bad\\/\"]",
            "[\"\\uD800\"]",
            "[\"bad\\u0000arg\"]",
            "[\"a\" \"b\"]",
        ] {
            let invalid = String::from_utf8(before.clone()).expect("utf8")
                + &format!("\n[unity]\ndefault_launch_arguments = {invalid}\n");
            assert_eq!(parse_settings(&invalid), Err(ConfigStoreError::Corrupt));
        }
        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn paths_roundtrip_and_reject_invalid_storage() {
        let root = temporary_path("config-paths");
        let path = root.join("settings.toml");
        fs::create_dir_all(&root).expect("fixture");
        let store = FileConfigStore::new(path.clone());
        let mut original = default_snapshot();
        original.revision = 153;
        original.settings.locale = ConfigLocale::JaJp;
        original.settings.appearance.motion = ConfigAppearanceMotion::Full;
        original.settings.unity.hub_path = Some("/fixtures/Hub".into());
        store
            .publish(&original)
            .expect("publish current configuration");
        let canonical = fs::read_to_string(&path).expect("current config");
        assert!(!canonical.contains("[paths]"));
        for candidate in [r"C:\用户\项目", "/home/测试/备份 \"local\""] {
            let mut saved = original.clone();
            saved.settings.paths.project_directory = Some(candidate.into());
            saved.settings.paths.backup_directory = Some(candidate.into());
            store.publish(&saved).expect("publish");
            assert_eq!(store.load(), Ok(saved.clone()));
            assert_eq!(FileConfigStore::new(path.clone()).initialize(), Ok(saved));
        }
        for suffix in [
            "\n[paths]\nproject_directory = \"\"\n",
            "\n[paths]\nbackup_directory = \"/bad\\q\"\n",
            "\n[paths]\nunknown = \"/path\"\n",
        ] {
            assert_eq!(
                parse_settings(&(canonical.clone() + suffix)),
                Err(ConfigStoreError::Corrupt)
            );
        }
        fs::remove_dir_all(&root).expect("remove fixture");
    }

    #[tokio::test]
    async fn directory_changes_are_durable_and_combined_byte_overflow_keeps_accepted_file() {
        use alcomd_application::ConfigPathUpdate;
        let root = temporary_path("config-durable-directory-update");
        let path = root.join("settings.toml");
        let store = Arc::new(FileConfigStore::new(path.clone()));
        let make_app = || {
            M7OfficialApplication::new(
                store.clone(),
                Arc::new(TestHub),
                Arc::new(TestHubLauncher),
                Arc::new(TestDirectories),
            )
        };
        let app = make_app();
        app.initialize_settings().await.expect("initialize");
        let mut update = ConfigUpdate {
            projects: None,
            backups: None,
            integrations: None,
            network_proxy: None,
            window: None,
            paths: Some(ConfigPathUpdate {
                project_directory: ConfigNullableUpdate::Set("/selected/projects".into()),
                backup_directory: ConfigNullableUpdate::Set("/selected/backups".into()),
            }),
            unity: None,
            appearance: None,
            locale: None,
            packages: None,
        };
        let accepted = app
            .update_settings(&AccessContext::local_owner(), 1, update.clone())
            .await
            .expect("save selected directories");
        let reopened = make_app();
        reopened.initialize_settings().await.expect("reopen");
        assert_eq!(
            reopened.get_settings(&AccessContext::local_owner()).await,
            Ok(accepted.clone())
        );
        assert_eq!(
            reopened
                .effective_directories()
                .await
                .expect("effective selected path")
                .backup_directory,
            "/selected/backups"
        );
        let before = fs::read(&path).expect("accepted bytes");
        update.paths = Some(ConfigPathUpdate {
            project_directory: ConfigNullableUpdate::Set(format!("/{}", "\\".repeat(4000))),
            backup_directory: ConfigNullableUpdate::Set(format!("/{}", "\\".repeat(4000))),
        });
        update.packages = Some(ConfigPackageSettingsUpdate {
            hidden_repository_packages: None,
            show_prerelease: None,
            prefer_prerelease: None,
            hide_local_user_packages: None,
            hidden_repository_ids: Some(
                (0..256)
                    .map(|index| format!("00000000-0000-0000-0000-{index:012x}"))
                    .collect(),
            ),
        });
        assert_eq!(
            reopened
                .update_settings(&AccessContext::local_owner(), 2, update)
                .await,
            Err(OfficialGuiError::InvalidInput)
        );
        assert_eq!(fs::read(&path).expect("unchanged durable bytes"), before);
        assert!(!path.with_extension("toml.new").exists());
        assert!(!path.with_extension("toml.bak").exists());
        assert_eq!(
            reopened.get_settings(&AccessContext::local_owner()).await,
            Ok(accepted)
        );
        let clear = ConfigUpdate {
            projects: None,
            backups: None,
            integrations: None,
            network_proxy: None,
            window: None,
            paths: Some(ConfigPathUpdate {
                project_directory: ConfigNullableUpdate::Clear,
                backup_directory: ConfigNullableUpdate::Clear,
            }),
            unity: None,
            appearance: None,
            locale: None,
            packages: None,
        };
        reopened
            .update_settings(&AccessContext::local_owner(), 2, clear)
            .await
            .expect("clear overrides");
        let cleared = make_app();
        cleared
            .initialize_settings()
            .await
            .expect("restart cleared state");
        assert_eq!(
            cleared
                .effective_directories()
                .await
                .expect("system defaults")
                .project_directory,
            "/fixtures/projects"
        );
        assert_eq!(
            store.load().expect("saved defaults").settings.paths,
            ConfigPathSettings::default()
        );
        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn hub_path_roundtrip_escapes_and_preserves_current_defaults() {
        let original = default_snapshot();
        let original_text = String::from_utf8(serialize_settings(&original)).expect("utf8");
        assert!(!original_text.contains("[unity]"));
        assert_eq!(parse_settings(&original_text), Ok(original.clone()));
        for path in [
            r"C:\Program Files\Unity Hub\Unity Hub.exe",
            "/Applications/Unity Hub.app/Contents/MacOS/Unity Hub",
            "/home/测试/Hub \"local\".AppImage",
        ] {
            let mut snapshot = original.clone();
            snapshot.revision = 9;
            snapshot.settings.unity.hub_path = Some(path.to_owned());
            let text = String::from_utf8(serialize_settings(&snapshot)).expect("utf8");
            assert_eq!(parse_settings(&text), Ok(snapshot));
        }
        for suffix in [
            "\n[unity]\nhub_path = \"\"\n",
            "\n[unity]\nhub_path = \"/tmp/a\\q\"\n",
            "\n[unity]\nhub_path = \"/tmp/a\"b\"\n",
            "\n[unity]\nhub_path = \"/tmp/a\t\"\n",
            "\n[unity]\nunknown = \"/tmp/a\"\n",
        ] {
            assert_eq!(
                parse_settings(&(original_text.clone() + suffix)),
                Err(ConfigStoreError::Corrupt)
            );
        }
        let oversized =
            original_text.clone() + &format!("\n[unity]\nhub_path = \"/{}\"\n", "中".repeat(1400));
        assert_eq!(parse_settings(&oversized), Err(ConfigStoreError::Corrupt));
    }

    #[tokio::test]
    async fn hub_path_is_durable_and_clear_survives_reopen_and_recovery() {
        use alcomd_application::ConfigUnityUpdate;
        let root = temporary_path("hub-path");
        let path = root.join("settings.toml");
        let _ = fs::remove_dir_all(&root);
        let store = Arc::new(FileConfigStore::new(path.clone()));
        let app = M7OfficialApplication::new(
            store.clone(),
            Arc::new(TestHub),
            Arc::new(TestHubLauncher),
            Arc::new(TestDirectories),
        );
        app.initialize_settings().await.expect("initialize");
        let baseline = store.load().expect("baseline");
        let selected = r"C:\用户\Unity Hub\Unity Hub.exe";
        let saved = app
            .update_settings(
                &AccessContext::local_owner(),
                baseline.revision,
                ConfigUpdate {
                    projects: None,
                    backups: None,
                    integrations: None,
                    network_proxy: None,
                    window: None,
                    paths: None,
                    appearance: None,
                    locale: None,
                    packages: None,
                    unity: Some(ConfigUnityUpdate {
                        hub_path: ConfigNullableUpdate::Set(selected.to_owned()),
                        default_launch_arguments: ConfigNullableUpdate::Unchanged,
                    }),
                },
            )
            .await
            .expect("save Hub path");
        assert_eq!(saved.revision, baseline.revision + 1);
        let reopened = FileConfigStore::new(path.clone());
        assert_eq!(reopened.load().expect("reopen"), saved);
        fs::rename(&path, path.with_extension("toml.bak")).expect("interrupted replacement");
        fs::write(
            path.with_extension("toml.new"),
            b"schema = 1
part",
        )
        .expect("partial temporary");
        assert_eq!(reopened.load().expect("recover saved path"), saved);
        let cleared = app
            .update_settings(
                &AccessContext::local_owner(),
                saved.revision,
                ConfigUpdate {
                    projects: None,
                    backups: None,
                    integrations: None,
                    network_proxy: None,
                    window: None,
                    paths: None,
                    appearance: None,
                    locale: None,
                    packages: None,
                    unity: Some(ConfigUnityUpdate {
                        hub_path: ConfigNullableUpdate::Clear,
                        default_launch_arguments: ConfigNullableUpdate::Unchanged,
                    }),
                },
            )
            .await
            .expect("clear path");
        assert_eq!(reopened.load().expect("reopen clear"), cleared);
        assert_eq!(cleared.settings.unity.hub_path, None);
        assert!(!fs::read_to_string(&path).expect("file").contains("[unity]"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn motion_storage_roundtrips_three_canonical_values_and_rejects_unknown_input() {
        let defaults = default_snapshot();
        assert_eq!(
            defaults.settings.appearance.motion,
            ConfigAppearanceMotion::System
        );
        for (motion, text) in [
            (ConfigAppearanceMotion::System, "system"),
            (ConfigAppearanceMotion::Full, "full"),
            (ConfigAppearanceMotion::Reduced, "reduced"),
        ] {
            let mut snapshot = defaults.clone();
            snapshot.settings.appearance.motion = motion;
            let encoded = String::from_utf8(serialize_settings(&snapshot)).expect("utf8");
            assert!(encoded.starts_with("schema = 1\n"));
            assert!(encoded.contains(&format!("motion = \"{text}\"\n")));
            assert_eq!(parse_settings(&encoded), Ok(snapshot));
        }
        let encoded = String::from_utf8(serialize_settings(&defaults)).expect("utf8");
        for invalid in ["Full", "enabled", "disabled", "auto", "unknown", ""] {
            assert_eq!(
                parse_settings(
                    &encoded.replace("motion = \"system\"", &format!("motion = \"{invalid}\""))
                ),
                Err(ConfigStoreError::Corrupt),
                "unknown motion must not become a default: {invalid}"
            );
        }
        assert_eq!(
            parse_settings(&encoded.replace("motion = \"system\"\n", "")),
            Err(ConfigStoreError::Corrupt)
        );
    }

    #[tokio::test]
    async fn motion_only_updates_are_revisioned_durable_and_preserve_other_settings() {
        let root = temporary_path("motion-three-state");
        let path = root.join("settings.toml");
        let _ = fs::remove_dir_all(&root);
        let app = M7OfficialApplication::new(
            Arc::new(FileConfigStore::new(path.clone())),
            Arc::new(TestHub),
            Arc::new(TestHubLauncher),
            Arc::new(TestDirectories),
        );
        let access = AccessContext::local_owner();
        let mut previous = app
            .update_settings(
                &access,
                1,
                ConfigUpdate {
                    projects: None,
                    backups: None,
                    integrations: None,
                    network_proxy: None,
                    window: None,
                    paths: None,
                    unity: None,
                    appearance: Some(ConfigAppearanceUpdate {
                        mode: Some(ConfigAppearanceMode::Dark),
                        source_color: ConfigNullableUpdate::Set("#E040A0".to_owned()),
                        scheme: Some(ConfigAppearanceScheme::Neutral),
                        motion: None,
                    }),
                    locale: Some(ConfigLocale::JaJp),
                    packages: Some(ConfigPackageSettingsUpdate {
                        hidden_repository_packages: None,
                        show_prerelease: Some(true),
                        prefer_prerelease: None,
                        hidden_repository_ids: None,
                        hide_local_user_packages: Some(true),
                    }),
                },
            )
            .await
            .expect("establish unrelated settings");
        for motion in [
            ConfigAppearanceMotion::Full,
            ConfigAppearanceMotion::Reduced,
            ConfigAppearanceMotion::System,
        ] {
            let accepted = app
                .update_settings(
                    &access,
                    previous.revision,
                    ConfigUpdate {
                        projects: None,
                        backups: None,
                        integrations: None,
                        network_proxy: None,
                        window: None,
                        paths: None,
                        unity: None,
                        appearance: Some(ConfigAppearanceUpdate {
                            mode: None,
                            source_color: ConfigNullableUpdate::Unchanged,
                            scheme: None,
                            motion: Some(motion),
                        }),
                        locale: None,
                        packages: None,
                    },
                )
                .await
                .expect("persist a motion-only update");
            let mut expected = previous;
            expected.revision += 1;
            expected.settings.appearance.motion = motion;
            assert_eq!(accepted, expected);
            let reopened = M7OfficialApplication::new(
                Arc::new(FileConfigStore::new(path.clone())),
                Arc::new(TestHub),
                Arc::new(TestHubLauncher),
                Arc::new(TestDirectories),
            );
            assert_eq!(
                reopened
                    .get_settings(&access)
                    .await
                    .expect("reopen durable choice"),
                accepted
            );
            previous = accepted;
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn scheme_storage_is_required_canonical_and_roundtrips_all_nine_values() {
        for (scheme, value) in [
            (ConfigAppearanceScheme::TonalSpot, "tonal-spot"),
            (ConfigAppearanceScheme::Fidelity, "fidelity"),
            (ConfigAppearanceScheme::Monochrome, "monochrome"),
            (ConfigAppearanceScheme::Neutral, "neutral"),
            (ConfigAppearanceScheme::Vibrant, "vibrant"),
            (ConfigAppearanceScheme::Expressive, "expressive"),
            (ConfigAppearanceScheme::Content, "content"),
            (ConfigAppearanceScheme::Rainbow, "rainbow"),
            (ConfigAppearanceScheme::FruitSalad, "fruit-salad"),
        ] {
            let mut snapshot = default_snapshot();
            snapshot.settings.appearance.scheme = scheme;
            let encoded = String::from_utf8(serialize_settings(&snapshot)).expect("utf8");
            assert!(encoded.starts_with("schema = 1\n"));
            assert!(encoded.contains(&format!("scheme = \"{value}\"\n")));
            assert_eq!(parse_settings(&encoded), Ok(snapshot));
        }
        let encoded = String::from_utf8(serialize_settings(&default_snapshot())).expect("utf8");
        assert_eq!(
            parse_settings(&encoded.replace("scheme = \"vibrant\"\n", "")),
            Err(ConfigStoreError::Corrupt)
        );
        for invalid in ["Vibrant", "tonal_spot", "fruitSalad", "unknown", ""] {
            assert_eq!(
                parse_settings(
                    &encoded.replace("scheme = \"vibrant\"", &format!("scheme = \"{invalid}\""))
                ),
                Err(ConfigStoreError::Corrupt),
                "noncanonical or unknown scheme: {invalid}"
            );
        }
    }

    #[tokio::test]
    async fn update_is_revisioned_durable_and_recovers_backup() {
        let root = temporary_path("roundtrip");
        let path = root.join("config/settings.toml");
        let _ = fs::remove_dir_all(&root);
        let app = M7OfficialApplication::new(
            Arc::new(FileConfigStore::new(path.clone())),
            Arc::new(TestHub),
            Arc::new(TestHubLauncher),
            Arc::new(TestDirectories),
        );
        let access = AccessContext::local_owner();
        let updated = app
            .update_settings(
                &access,
                1,
                ConfigUpdate {
                    projects: None,
                    backups: None,
                    integrations: None,
                    network_proxy: None,
                    window: None,
                    paths: None,
                    unity: None,
                    appearance: Some(ConfigAppearanceUpdate {
                        mode: Some(ConfigAppearanceMode::Dark),
                        source_color: ConfigNullableUpdate::Set("#AABBCC".to_owned()),
                        scheme: Some(ConfigAppearanceScheme::Expressive),
                        motion: Some(ConfigAppearanceMotion::Reduced),
                    }),
                    locale: Some(ConfigLocale::ZhCn),
                    packages: Some(ConfigPackageSettingsUpdate {
                        hidden_repository_packages: None,
                        show_prerelease: Some(true),
                        prefer_prerelease: None,
                        hidden_repository_ids: Some(vec![
                            "ffffffff-ffff-ffff-ffff-ffffffffffff".to_owned(),
                            "00000000-0000-0000-0000-000000000000".to_owned(),
                        ]),
                        hide_local_user_packages: Some(true),
                    }),
                },
            )
            .await
            .expect("update");
        assert_eq!(updated.revision, 2);
        assert_eq!(app.get_settings(&access).await.expect("read"), updated);
        assert_eq!(
            app.update_settings(
                &access,
                1,
                ConfigUpdate {
                    projects: None,
                    backups: None,
                    integrations: None,
                    network_proxy: None,
                    window: None,
                    paths: None,
                    unity: None,
                    appearance: None,
                    locale: Some(ConfigLocale::EnUs),
                    packages: None,
                }
            )
            .await,
            Err(OfficialGuiError::RevisionConflict)
        );
        fs::rename(&path, path.with_extension("toml.bak")).expect("simulate backup boundary");
        assert_eq!(app.get_settings(&access).await.expect("recover"), updated);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_noncurrent_configuration_before_recovery_or_publication() {
        let root = temporary_path("reject-unsupported-configuration");
        fs::create_dir_all(&root).expect("fixture");
        let path = root.join("settings.toml");
        let backup = path.with_extension("toml.bak");
        let temporary = path.with_extension("toml.new");
        let store = FileConfigStore::new(path.clone());
        let snapshot = default_snapshot();
        let current = String::from_utf8(serialize_settings(&snapshot)).expect("current settings");
        for (file, other) in [(&path, &backup), (&backup, &temporary), (&temporary, &path)] {
            fs::write(other, &current).expect("current evidence");
            for version in [0, 2, u64::MAX] {
                let old = current.replace("schema = 1", &format!("schema = {version}"));
                fs::write(file, &old).expect("noncurrent evidence");
                assert_eq!(preflight_config(&path), Err(ConfigStoreError::Corrupt));
                assert_eq!(store.initialize(), Err(ConfigStoreError::Corrupt));
                assert_eq!(store.publish(&snapshot), Err(ConfigStoreError::Corrupt));
                assert_eq!(fs::read_to_string(file).expect("original evidence"), old);
                assert_eq!(
                    fs::read_to_string(other).expect("current evidence"),
                    current
                );
            }
            fs::remove_file(file).expect("remove fixture");
            fs::remove_file(other).expect("remove fixture");
        }
        fs::remove_dir_all(&root).expect("cleanup fixture");
    }

    #[test]
    fn rejects_an_unidentified_partial_replacement_without_current_authority() {
        let root = temporary_path("reject-unidentified-replacement");
        fs::create_dir_all(&root).expect("fixture");
        let path = root.join("settings.toml");
        let temporary = path.with_extension("toml.new");
        fs::write(&temporary, b"schema").expect("unidentified temporary content");
        assert_eq!(preflight_config(&path), Err(ConfigStoreError::Corrupt));
        assert_eq!(
            FileConfigStore::new(path.clone()).initialize(),
            Err(ConfigStoreError::Corrupt)
        );
        assert_eq!(fs::read(&temporary).expect("preserved content"), b"schema");
        assert!(!path.exists());
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn invalid_settings_leave_the_complete_file_intact() {
        let root = temporary_path("invalid-settings");
        let path = root.join("settings.toml");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create fixture");
        let store = FileConfigStore::new(path.clone());
        let current = String::from_utf8(serialize_settings(&default_snapshot())).expect("utf8");
        let mut invalid = ["\"default\"", "\"compact\"", "null", "true"]
            .map(|value| {
                current.replace(
                    "motion = \"system\"\n",
                    &format!("unrecognized_key = {value}\nmotion = \"system\"\n"),
                )
            })
            .to_vec();
        invalid.push(current.replace("prefer_prerelease = false\n", ""));
        invalid.push(
            current
                .replace("show_prerelease = true", "show_prerelease = false")
                .replace("prefer_prerelease = false", "prefer_prerelease = true"),
        );
        invalid.push(current.replace("schema = 1", "schema = 2"));
        for invalid in invalid {
            fs::write(&path, &invalid).expect("write unsupported current input");
            assert_eq!(store.initialize(), Err(ConfigStoreError::Corrupt));
            assert_eq!(fs::read_to_string(&path).expect("input retained"), invalid);
            assert!(!path.with_extension("toml.new").exists());
            assert!(!path.with_extension("toml.bak").exists());
        }
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn scheme_only_update_and_three_field_reset_preserve_unrelated_settings() {
        let root = temporary_path("theme-partial-update");
        let path = root.join("settings.toml");
        let _ = fs::remove_dir_all(&root);
        let app = M7OfficialApplication::new(
            Arc::new(FileConfigStore::new(path.clone())),
            Arc::new(TestHub),
            Arc::new(TestHubLauncher),
            Arc::new(TestDirectories),
        );
        let access = AccessContext::local_owner();
        let initial = app
            .update_settings(
                &access,
                1,
                ConfigUpdate {
                    projects: None,
                    backups: None,
                    integrations: None,
                    network_proxy: None,
                    window: None,
                    paths: None,
                    unity: None,
                    appearance: Some(ConfigAppearanceUpdate {
                        mode: Some(ConfigAppearanceMode::Dark),
                        source_color: ConfigNullableUpdate::Set("#112233".to_owned()),
                        scheme: Some(ConfigAppearanceScheme::Neutral),
                        motion: Some(ConfigAppearanceMotion::Reduced),
                    }),
                    locale: Some(ConfigLocale::ZhCn),
                    packages: Some(ConfigPackageSettingsUpdate {
                        hidden_repository_packages: None,
                        show_prerelease: Some(true),
                        prefer_prerelease: None,
                        hidden_repository_ids: Some(vec![
                            "00000000-0000-0000-0000-000000000001".to_owned(),
                        ]),
                        hide_local_user_packages: Some(true),
                    }),
                },
            )
            .await
            .expect("unrelated persisted settings");
        let changed = app
            .update_settings(
                &access,
                initial.revision,
                ConfigUpdate {
                    projects: None,
                    backups: None,
                    integrations: None,
                    network_proxy: None,
                    window: None,
                    paths: None,
                    unity: None,
                    appearance: Some(ConfigAppearanceUpdate {
                        mode: None,
                        source_color: ConfigNullableUpdate::Unchanged,
                        scheme: Some(ConfigAppearanceScheme::FruitSalad),
                        motion: None,
                    }),
                    locale: None,
                    packages: None,
                },
            )
            .await
            .expect("scheme-only partial is not empty");
        let mut expected = initial;
        expected.revision += 1;
        expected.settings.appearance.scheme = ConfigAppearanceScheme::FruitSalad;
        assert_eq!(changed, expected);
        let reset = app
            .update_settings(
                &access,
                changed.revision,
                ConfigUpdate {
                    projects: None,
                    backups: None,
                    integrations: None,
                    network_proxy: None,
                    window: None,
                    paths: None,
                    unity: None,
                    appearance: Some(ConfigAppearanceUpdate {
                        mode: Some(ConfigAppearanceMode::System),
                        source_color: ConfigNullableUpdate::Clear,
                        scheme: Some(ConfigAppearanceScheme::Vibrant),
                        motion: None,
                    }),
                    locale: None,
                    packages: None,
                },
            )
            .await
            .expect("reset three theme fields in one CAS update");
        expected.revision += 1;
        expected.settings.appearance.mode = ConfigAppearanceMode::System;
        expected.settings.appearance.source_color = None;
        expected.settings.appearance.scheme = ConfigAppearanceScheme::Vibrant;
        assert_eq!(reset, expected);
        assert_eq!(app.get_settings(&access).await, Ok(expected.clone()));
        let before = fs::read(&path).expect("before invalid input");
        assert_eq!(
            app.update_settings(
                &access,
                expected.revision,
                ConfigUpdate {
                    projects: None,
                    backups: None,
                    integrations: None,
                    network_proxy: None,
                    window: None,
                    paths: None,
                    unity: None,
                    appearance: Some(ConfigAppearanceUpdate {
                        mode: None,
                        source_color: ConfigNullableUpdate::Set("#bad".to_owned()),
                        scheme: Some(ConfigAppearanceScheme::Fidelity),
                        motion: None,
                    }),
                    locale: None,
                    packages: None,
                }
            )
            .await,
            Err(OfficialGuiError::InvalidInput)
        );
        assert_eq!(fs::read(&path).expect("after invalid input"), before);
        assert_eq!(app.get_settings(&access).await, Ok(expected));
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn failed_config_read_or_update_invalidates_candidate_cache_until_confirmed_read() {
        let root = temporary_path("candidate-cache-failure");
        let path = root.join("settings.toml");
        let _ = fs::remove_dir_all(&root);
        let app = M7OfficialApplication::new(
            Arc::new(FileConfigStore::new(path.clone())),
            Arc::new(TestHub),
            Arc::new(TestHubLauncher),
            Arc::new(TestDirectories),
        );
        let access = AccessContext::local_owner();
        app.initialize_settings().await.expect("initialize");
        let confirmed = app.candidate_settings().await.expect("cached snapshot");
        let original = fs::read(&path).expect("settings bytes");
        fs::write(&path, "not valid settings").expect("inject unreadable Config");
        assert!(app.get_settings(&access).await.is_err());
        assert_eq!(
            app.candidate_settings().await,
            Err(OfficialGuiError::Unavailable)
        );
        fs::write(&path, &original).expect("restore Config");
        assert_eq!(
            app.get_settings(&access).await.expect("confirm restored"),
            confirmed
        );
        // An observed newer disk revision makes a failed update unsuitable as
        // authority for retaining the old cached revision.
        let mut newer = confirmed.clone();
        newer.revision += 1;
        fs::write(&path, serialize_settings(&newer)).expect("new disk revision");
        assert_eq!(
            app.update_settings(
                &access,
                confirmed.revision,
                ConfigUpdate {
                    projects: None,
                    backups: None,
                    integrations: None,
                    network_proxy: None,
                    window: None,
                    paths: None,
                    unity: None,
                    appearance: None,
                    locale: Some(ConfigLocale::EnUs),
                    packages: None,
                }
            )
            .await,
            Err(OfficialGuiError::RevisionConflict)
        );
        assert_eq!(
            app.candidate_settings().await,
            Err(OfficialGuiError::Unavailable)
        );
        assert_eq!(
            app.get_settings(&access).await.expect("confirm newer"),
            newer
        );
        assert_eq!(
            app.candidate_settings()
                .await
                .expect("new cached authority"),
            newer
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn maximum_base_preferences_fit_byte_limit() {
        let mut snapshot = default_snapshot();
        snapshot.settings.appearance.source_color = Some("#FFFFFF".to_owned());
        snapshot.settings.packages.hidden_repository_ids = (0..256)
            .map(|value| format!("00000000-0000-0000-0000-{value:012x}"))
            .collect();
        let encoded = serialize_settings(&snapshot);
        assert!(encoded.len() as u64 <= MAX_BASE_SETTINGS_BYTES);
        assert_eq!(
            parse_settings(std::str::from_utf8(&encoded).expect("utf8")),
            Ok(snapshot)
        );
    }

    #[test]
    fn package_preferences_keep_independent_base_limit_and_total_file_limit() {
        let root = temporary_path("config1-package-preferences");
        let path = root.join("settings.toml");
        fs::create_dir_all(&root).expect("fixture");
        let store = FileConfigStore::new(path.clone());
        let original = store.initialize().expect("initialize");
        let mut saved = original.clone();
        saved.revision += 1;
        saved.settings.packages.hidden_repository_packages = vec![ConfigRepositoryPackages {
            repository_id: "00000000-0000-4000-8000-000000000102".into(),
            package_ids: (0..4096)
                .map(|n| format!("com.example.{n:04}.{}", "a".repeat(105)))
                .collect(),
        }];
        store
            .publish(&saved)
            .expect("bounded preferences exceed former file budget");
        assert!(fs::metadata(&path).expect("file").len() > MAX_BASE_SETTINGS_BYTES);
        assert_eq!(
            FileConfigStore::new(path.clone()).initialize(),
            Ok(saved.clone())
        );
        let bytes = fs::read_to_string(&path).expect("saved");
        let invalid = bytes.replace("schema = 1", "schema = 12");
        assert!(parse_settings(&invalid).is_err());
        let mut extra = saved.clone();
        extra.settings.unity.default_launch_arguments = Some(vec!["x".repeat(4096); 5]);
        assert_eq!(store.publish(&extra), Err(ConfigStoreError::TooLarge));
        assert_eq!(fs::read_to_string(&path).expect("unchanged"), bytes);
        let current = String::from_utf8(serialize_settings(&original)).expect("UTF-8");
        assert_eq!(
            parse_settings(&current.replace("hidden_repository_packages = []\n", "")),
            Err(ConfigStoreError::Corrupt)
        );
        let mut padded = current.clone();
        padded.push('#');
        padded.push_str(&" ".repeat(MAX_SETTINGS_BYTES as usize - padded.len()));
        assert_eq!(parse_settings(&padded), Ok(original));
        padded.push(' ');
        fs::write(&path, padded).expect("oversized file");
        assert_eq!(store.load(), Err(ConfigStoreError::Corrupt));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn escaped_hub_path_cannot_exceed_file_bound_or_replace_accepted_settings() {
        let root = temporary_path("hub-encoded-size");
        let path = root.join("settings.toml");
        let _ = fs::remove_dir_all(&root);
        let store = FileConfigStore::new(path.clone());
        let original = store.initialize().expect("initialize");
        let original_bytes = fs::read(&path).expect("original bytes");
        let mut oversized = original.clone();
        oversized.revision += 1;
        oversized.settings.packages.hidden_repository_ids = (0..256)
            .map(|value| format!("00000000-0000-0000-0000-{value:012x}"))
            .collect();
        oversized.settings.unity.hub_path = Some(format!("/{}", "\\".repeat(4095)));
        assert!(serialize_settings_base(&oversized).len() as u64 > MAX_BASE_SETTINGS_BYTES);
        assert_eq!(store.publish(&oversized), Err(ConfigStoreError::TooLarge));
        assert_eq!(fs::read(&path).expect("accepted bytes"), original_bytes);
        assert_eq!(store.load().expect("accepted revision"), original);
        assert!(!path.with_extension("toml.new").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn hidden_repository_ids_reject_duplicates_and_noncanonical_storage() {
        assert_eq!(
            validate_hidden_repository_ids(vec![
                "00000000-0000-0000-0000-000000000000".to_owned(),
                "00000000-0000-0000-0000-000000000000".to_owned(),
            ]),
            Err(OfficialGuiError::InvalidInput)
        );
        let canonical =
            String::from_utf8(serialize_settings(&default_snapshot())).expect("current settings");
        let unsorted = canonical.replace("hidden_repository_ids = []", "hidden_repository_ids = [\"ffffffff-ffff-ffff-ffff-ffffffffffff\", \"00000000-0000-0000-0000-000000000000\"]");
        assert_eq!(parse_settings(&unsorted), Err(ConfigStoreError::Corrupt));
    }

    #[test]
    fn hidden_repository_ids_reject_more_than_256_entries() {
        let values = (0..257)
            .map(|value| format!("00000000-0000-0000-0000-{value:012x}"))
            .collect();
        assert_eq!(
            validate_hidden_repository_ids(values),
            Err(OfficialGuiError::InvalidInput)
        );
    }

    #[test]
    fn fullscreen_roundtrips_and_is_required() {
        let root = temporary_path("config-fullscreen");
        let path = root.join("settings.toml");
        fs::create_dir_all(&root).expect("fixture");
        let store = FileConfigStore::new(path.clone());
        let mut saved = default_snapshot();
        saved.revision = 41;
        saved.settings.window.remember_state = true;
        saved.settings.window.fullscreen = true;
        store.publish(&saved).expect("publish fullscreen");
        assert_eq!(store.initialize(), Ok(saved.clone()));
        let canonical = fs::read_to_string(&path).expect("canonical");
        assert!(canonical.contains("fullscreen = true\n"));
        for value in ["null", "\"true\"", "1"] {
            let invalid = canonical.replace("fullscreen = true", &format!("fullscreen = {value}"));
            fs::write(&path, &invalid).expect("invalid current");
            assert_eq!(store.initialize(), Err(ConfigStoreError::Corrupt));
            assert_eq!(fs::read_to_string(&path).expect("retained input"), invalid);
        }
        let missing = canonical.replace("fullscreen = true\n", "");
        fs::write(&path, &missing).expect("missing current field");
        assert_eq!(store.initialize(), Err(ConfigStoreError::Corrupt));
        fs::remove_dir_all(root).expect("cleanup fixture");
    }

    #[test]
    fn vcc_opt_out_is_strict_and_durable() {
        let root = temporary_path("config-vcc-links");
        let path = root.join("settings.toml");
        fs::create_dir_all(&root).expect("fixture");
        let store = FileConfigStore::new(path.clone());
        let mut expected = default_snapshot();
        expected.revision = 42;
        expected.settings.window.fullscreen = true;
        let section = "\n[integrations]\nhandle_vcc_links = true\n";
        let current = String::from_utf8(serialize_settings(&expected)).expect("canonical");
        store.publish(&expected).expect("publish current settings");
        assert_eq!(store.initialize(), Ok(expected.clone()));
        expected.settings.integrations.handle_vcc_links = false;
        store.publish(&expected).expect("save opt-out");
        assert_eq!(
            FileConfigStore::new(path.clone()).initialize(),
            Ok(expected)
        );
        for invalid in [
            current.replace(section, ""),
            current.replace("handle_vcc_links = true", "handle_vcc_links = \"true\""),
        ] {
            fs::write(&path, &invalid).expect("invalid fixture");
            assert_eq!(store.initialize(), Err(ConfigStoreError::Corrupt));
            assert_eq!(fs::read_to_string(&path).expect("retained file"), invalid);
        }
        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn global_proxy_is_strict_and_durable() {
        let root = temporary_path("config-proxy");
        let path = root.join("settings.toml");
        fs::create_dir_all(&root).expect("fixture");
        let store = FileConfigStore::new(path.clone());
        let mut snapshot = default_snapshot();
        snapshot.revision = 47;
        snapshot.settings.integrations.handle_vcc_links = false;
        let canonical = String::from_utf8(serialize_settings(&snapshot)).expect("canonical");
        let section = "\n[network]\nproxy_mode = \"system\"\n";
        store.publish(&snapshot).expect("publish current settings");
        assert_eq!(store.initialize(), Ok(snapshot.clone()));
        for proxy in [
            ConfigProxy::Direct,
            ConfigProxy::custom("http://[::1]:7890").expect("IPv6 proxy"),
        ] {
            snapshot.settings.network_proxy = proxy;
            store.publish(&snapshot).expect("save proxy");
            assert_eq!(
                FileConfigStore::new(path.clone()).initialize(),
                Ok(snapshot.clone())
            );
        }
        for invalid in [
            canonical.replace(section, ""),
            canonical.replace("proxy_mode = \"system\"", "proxy_mode = \"custom\""),
            canonical.replace(section, "\n[network]\nproxy_mode = \"custom\"\nproxy_url = \"http://user:fixture-secret@proxy.test\"\n"),
        ] {
            fs::write(&path, invalid).expect("invalid fixture");
            assert_eq!(store.initialize(), Err(ConfigStoreError::Corrupt));
        }
        fs::remove_dir_all(root).expect("remove fixture");
    }
}
