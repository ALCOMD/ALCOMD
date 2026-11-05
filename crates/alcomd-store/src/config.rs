//! File-backed configuration adapter. The application owns permissions, update validation and CAS.
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use alcomd_application::{
    BackupCompression, ConfigAppearance, ConfigAppearanceMode, ConfigAppearanceMotion,
    ConfigAppearanceScheme, ConfigBackupSettings, ConfigLocale, ConfigPackageSettings,
    ConfigPathSettings, ConfigProjectSettings, ConfigSettings, ConfigSnapshot, ConfigStore,
    ConfigStoreError, ConfigUnitySettings, ConfigWindowSettings,
};

const MAX_SETTINGS_BYTES: u64 = 16 * 1024;

/// The daemon's bounded, recoverable settings.toml storage adapter.
#[derive(Clone, Debug)]
pub struct FileConfigStore {
    path: PathBuf,
}

impl FileConfigStore {
    pub const SCHEMA_VERSION: u32 = 10;

    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl ConfigStore for FileConfigStore {
    fn initialize(&self) -> Result<ConfigSnapshot, ConfigStoreError> {
        initialize_settings_file(&self.path)?;
        read_settings(&self.path)
    }

    fn load(&self) -> Result<ConfigSnapshot, ConfigStoreError> {
        read_settings(&self.path)
    }

    fn publish(&self, snapshot: &ConfigSnapshot) -> Result<(), ConfigStoreError> {
        snapshot
            .settings
            .validate()
            .map_err(|_| ConfigStoreError::Corrupt)?;
        let contents = serialize_settings(snapshot);
        if snapshot.revision == 0 {
            return Err(ConfigStoreError::Corrupt);
        }
        if contents.len() as u64 > MAX_SETTINGS_BYTES {
            return Err(ConfigStoreError::TooLarge);
        }
        publish_settings(&self.path, &contents)
    }
}

fn initialize_settings_file(path: &Path) -> Result<(), ConfigStoreError> {
    recover_settings_replace(path)?;
    if !path.exists() {
        publish_settings(path, &serialize_settings(&ConfigSnapshot::default()))?;
        return Ok(());
    }
    let (snapshot, schema) = read_settings_with_schema(path)?;
    if schema < u64::from(FileConfigStore::SCHEMA_VERSION) {
        publish_settings(path, &serialize_settings(&snapshot))?;
    }
    Ok(())
}

fn read_settings(path: &Path) -> Result<ConfigSnapshot, ConfigStoreError> {
    recover_settings_replace(path)?;
    if !path.exists() {
        return Ok(ConfigSnapshot::default());
    }
    read_settings_without_recovery(path)
}

fn read_settings_with_schema(path: &Path) -> Result<(ConfigSnapshot, u64), ConfigStoreError> {
    let metadata = fs::symlink_metadata(path).map_err(map_io)?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_SETTINGS_BYTES {
        return Err(ConfigStoreError::Corrupt);
    }
    let mut bytes =
        Vec::with_capacity(usize::try_from(metadata.len()).map_err(|_| ConfigStoreError::Corrupt)?);
    File::open(path)
        .map_err(map_io)?
        .take(MAX_SETTINGS_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(map_io)?;
    if bytes.len() as u64 > MAX_SETTINGS_BYTES || bytes.contains(&0) {
        return Err(ConfigStoreError::Corrupt);
    }
    parse_settings_with_schema(std::str::from_utf8(&bytes).map_err(|_| ConfigStoreError::Corrupt)?)
}

#[cfg(test)]
fn parse_settings(text: &str) -> Result<ConfigSnapshot, ConfigStoreError> {
    parse_settings_with_schema(text).map(|(snapshot, _)| snapshot)
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
    packages: Option<RawPackages>,
    unity: Option<RawUnity>,
    paths: Option<RawPaths>,
    window: Option<RawWindow>,
    projects: Option<RawProjects>,
    backups: Option<RawBackups>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAppearance {
    mode: String,
    source_color: Option<String>,
    scheme: Option<String>,
    density: Option<String>,
    motion: String,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPackages {
    show_prerelease: bool,
    hidden_repository_ids: Vec<String>,
    hide_local_user_packages: bool,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawUnity {
    hub_path: Option<String>,
    default_launch_arguments: Option<Vec<String>>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPaths {
    project_directory: Option<String>,
    backup_directory: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWindow {
    remember_state: Option<bool>,
    fullscreen: Option<bool>,
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

fn parse_settings_with_schema(text: &str) -> Result<(ConfigSnapshot, u64), ConfigStoreError> {
    if text.len() as u64 > MAX_SETTINGS_BYTES {
        return Err(ConfigStoreError::TooLarge);
    }
    let raw: RawSettings = toml::from_str(text).map_err(|_| ConfigStoreError::Corrupt)?;
    let schema = raw.schema;
    let revision = Some(raw.revision);
    let locale = Some(parse_locale(&raw.locale)?);
    let mode = Some(parse_mode(&raw.appearance.mode)?);
    let source_color = raw.appearance.source_color;
    let scheme = raw
        .appearance
        .scheme
        .as_deref()
        .map(parse_scheme)
        .transpose()?;
    let legacy_density = raw.appearance.density.is_some();
    if let Some(density) = raw.appearance.density {
        validate_legacy_density(&density)?;
    }
    let motion = Some(parse_motion(&raw.appearance.motion)?);
    let show_prerelease = raw.packages.as_ref().map(|value| value.show_prerelease);
    let hidden_repository_ids = raw
        .packages
        .as_ref()
        .map(|value| value.hidden_repository_ids.clone());
    let hide_local_user_packages = raw
        .packages
        .as_ref()
        .map(|value| value.hide_local_user_packages);
    let hub_path = raw.unity.as_ref().and_then(|value| value.hub_path.clone());
    let default_launch_arguments = raw
        .unity
        .as_ref()
        .and_then(|value| value.default_launch_arguments.clone());
    let project_directory = raw
        .paths
        .as_ref()
        .and_then(|value| value.project_directory.clone());
    let backup_directory = raw
        .paths
        .as_ref()
        .and_then(|value| value.backup_directory.clone());
    let remember_state = raw.window.as_ref().and_then(|value| value.remember_state);
    let fullscreen = raw.window.as_ref().and_then(|value| value.fullscreen);
    let auto_discover = raw.projects.as_ref().map(|value| value.auto_discover);
    let compression_mode = raw
        .backups
        .as_ref()
        .map(|value| match value.compression_mode.as_str() {
            "store" => Ok(BackupCompression::Store),
            "fast" => Ok(BackupCompression::Fast),
            "level3" => Ok(BackupCompression::Level3),
            "level5" => Ok(BackupCompression::Level5),
            "level7" => Ok(BackupCompression::Level7),
            "maximum" => Ok(BackupCompression::Maximum),
            _ => Err(ConfigStoreError::Corrupt),
        })
        .transpose()?;
    let include_vpm_packages = raw.backups.as_ref().map(|value| value.include_vpm_packages);
    let include_library = raw.backups.as_ref().map(|value| value.include_library);
    if !(1..=u64::from(FileConfigStore::SCHEMA_VERSION)).contains(&schema)
        || legacy_density != (schema < 4)
    {
        return Err(ConfigStoreError::Corrupt);
    }
    if schema < 3 && raw.unity.is_some() {
        return Err(ConfigStoreError::Corrupt);
    }
    if schema < 5 && raw.paths.is_some() {
        return Err(ConfigStoreError::Corrupt);
    }
    if schema < 6 && default_launch_arguments.is_some() {
        return Err(ConfigStoreError::Corrupt);
    }
    if schema < 7 && raw.window.is_some() {
        return Err(ConfigStoreError::Corrupt);
    }
    if schema < 10 && fullscreen.is_some() {
        return Err(ConfigStoreError::Corrupt);
    }
    if schema < 8 && (raw.projects.is_some() || raw.backups.is_some()) {
        return Err(ConfigStoreError::Corrupt);
    }
    if schema < 9
        && matches!(
            compression_mode,
            Some(BackupCompression::Level3 | BackupCompression::Level5 | BackupCompression::Level7)
        )
    {
        return Err(ConfigStoreError::Corrupt);
    }
    if schema < 3 && motion == Some(ConfigAppearanceMotion::Full) {
        return Err(ConfigStoreError::Corrupt);
    }
    let scheme = if schema < 3 {
        if scheme.is_some() {
            return Err(ConfigStoreError::Corrupt);
        }
        ConfigAppearanceScheme::Vibrant
    } else {
        scheme.ok_or(ConfigStoreError::Corrupt)?
    };
    let revision = revision
        .filter(|value| *value > 0)
        .ok_or(ConfigStoreError::Corrupt)?;
    let packages = if schema == 1 {
        if show_prerelease.is_some()
            || hidden_repository_ids.is_some()
            || hide_local_user_packages.is_some()
            || raw.packages.is_some()
        {
            return Err(ConfigStoreError::Corrupt);
        }
        ConfigPackageSettings {
            show_prerelease: false,
            hidden_repository_ids: Vec::new(),
            hide_local_user_packages: false,
        }
    } else {
        if raw.packages.is_none() {
            return Err(ConfigStoreError::Corrupt);
        }
        ConfigPackageSettings {
            show_prerelease: show_prerelease.ok_or(ConfigStoreError::Corrupt)?,
            hidden_repository_ids: hidden_repository_ids.ok_or(ConfigStoreError::Corrupt)?,
            hide_local_user_packages: hide_local_user_packages.ok_or(ConfigStoreError::Corrupt)?,
        }
    };
    let snapshot = ConfigSnapshot {
        revision,
        settings: ConfigSettings {
            projects: if schema < 8 {
                ConfigProjectSettings::default()
            } else {
                ConfigProjectSettings {
                    auto_discover: auto_discover.ok_or(ConfigStoreError::Corrupt)?,
                }
            },
            backups: if schema < 8 {
                ConfigBackupSettings::default()
            } else {
                ConfigBackupSettings {
                    compression_mode: compression_mode.ok_or(ConfigStoreError::Corrupt)?,
                    include_vpm_packages: include_vpm_packages.ok_or(ConfigStoreError::Corrupt)?,
                    include_library: include_library.ok_or(ConfigStoreError::Corrupt)?,
                }
            },
            window: ConfigWindowSettings {
                fullscreen: if schema < 10 {
                    false
                } else {
                    fullscreen.ok_or(ConfigStoreError::Corrupt)?
                },
                remember_state: if schema < 7 {
                    false
                } else {
                    remember_state.ok_or(ConfigStoreError::Corrupt)?
                },
            },
            appearance: ConfigAppearance {
                mode: mode.ok_or(ConfigStoreError::Corrupt)?,
                source_color,
                scheme,
                motion: motion.ok_or(ConfigStoreError::Corrupt)?,
            },
            locale: locale.ok_or(ConfigStoreError::Corrupt)?,
            packages,
            unity: ConfigUnitySettings {
                hub_path,
                default_launch_arguments,
            },
            paths: ConfigPathSettings {
                project_directory,
                backup_directory,
            },
        },
    };
    snapshot
        .settings
        .validate()
        .map_err(|_| ConfigStoreError::Corrupt)?;
    Ok((snapshot, schema))
}

fn serialize_settings(snapshot: &ConfigSnapshot) -> Vec<u8> {
    let source = snapshot
        .settings
        .appearance
        .source_color
        .as_ref()
        .map_or_else(String::new, |value| format!("source_color = \"{value}\"\n"));
    let mut contents = format!(
        "schema = {}\nrevision = {}\nlocale = \"{}\"\n\n[appearance]\nmode = \"{}\"\n{}scheme = \"{}\"\nmotion = \"{}\"\n\n[packages]\nshow_prerelease = {}\nhidden_repository_ids = [{}]\nhide_local_user_packages = {}\n",
        FileConfigStore::SCHEMA_VERSION,
        snapshot.revision,
        locale_str(snapshot.settings.locale),
        mode_str(snapshot.settings.appearance.mode),
        source,
        scheme_str(snapshot.settings.appearance.scheme),
        motion_str(snapshot.settings.appearance.motion),
        snapshot.settings.packages.show_prerelease,
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
    contents.into_bytes()
}

fn publish_settings(path: &Path, contents: &[u8]) -> Result<(), ConfigStoreError> {
    if contents.len() as u64 > MAX_SETTINGS_BYTES {
        return Err(ConfigStoreError::TooLarge);
    }
    let parent = path.parent().ok_or(ConfigStoreError::Unavailable)?;
    fs::create_dir_all(parent).map_err(map_io)?;
    recover_settings_replace(path)?;
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
    let had_target = path.exists();
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

fn recover_settings_replace(path: &Path) -> Result<(), ConfigStoreError> {
    let parent = path.parent().ok_or(ConfigStoreError::Unavailable)?;
    let temporary = path.with_extension("toml.new");
    let backup = path.with_extension("toml.bak");
    if path.exists() {
        let _ = read_settings_without_recovery(path)?;
        if !temporary.exists() && !backup.exists() {
            return Ok(());
        }
        // Establish the observed valid target before discarding its old backup.
        alcomd_platform::sync_directory(parent).map_err(map_io)?;
        if temporary.exists() {
            fs::remove_file(&temporary).map_err(map_io)?;
        }
        if backup.exists() {
            fs::remove_file(&backup).map_err(map_io)?;
        }
        alcomd_platform::sync_directory(parent).map_err(map_io)?;
        return Ok(());
    }
    if backup.exists() {
        let _ = read_settings_without_recovery(&backup)?;
        fs::rename(&backup, path).map_err(map_io)?;
        alcomd_platform::sync_directory(parent).map_err(map_io)?;
    }
    if temporary.exists() {
        fs::remove_file(temporary).map_err(map_io)?;
        alcomd_platform::sync_directory(parent).map_err(map_io)?;
    }
    Ok(())
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

fn read_settings_without_recovery(path: &Path) -> Result<ConfigSnapshot, ConfigStoreError> {
    read_settings_with_schema(path).map(|(snapshot, _)| snapshot)
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
fn validate_legacy_density(value: &str) -> Result<(), ConfigStoreError> {
    match value {
        "default" | "compact" => Ok(()),
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
fn map_io(_: io::Error) -> ConfigStoreError {
    ConfigStoreError::Unavailable
}

#[cfg(test)]
mod tests {
    use super::*;
    use alcomd_application::{
        AccessContext, ConfigAppearanceUpdate, ConfigNullableUpdate, ConfigPackageSettingsUpdate,
        ConfigUpdate, M7OfficialApplication, OfficialActivityCursor, OfficialActivityPage,
        OfficialGuiError, OfficialGuiStore,
        PrincipalId, StoreError,
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

    const CONFIG8_DEFAULTS_TOML: &str = "\n[projects]\nauto_discover = true\n\n[backups]\ncompression_mode = \"fast\"\ninclude_vpm_packages = true\ninclude_library = false\n";

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
                .replace("schema = 10", "schema = 10 # current schema")
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
    fn config9_backup_defaults_and_discovery_roundtrip_reopen_and_reject_invalid_files() {
        let root = temporary_path("config8-project-backup-defaults");
        let path = root.join("settings.toml");
        let store = FileConfigStore::new(path.clone());
        let original = store.initialize().expect("default Config8");
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
        let current = fs::read_to_string(&path).expect("Config8 file");
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
                .replace("schema = 10", "schema = 7"),
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

    #[test]
    fn config9_migration_respects_file_limit_and_retains_valid_historical_input_on_overflow() {
        let root = temporary_path("config8-migration-size-bound");
        let path = root.join("settings.toml");
        fs::create_dir_all(&root).expect("fixture");
        let mut expected = default_snapshot();
        expected.settings.unity.default_launch_arguments = Some(vec!["x".repeat(2500); 5]);
        let legacy_file = |snapshot: &ConfigSnapshot| {
            String::from_utf8(serialize_settings(snapshot))
                .expect("UTF-8")
                .replace(CONFIG8_DEFAULTS_TOML, "")
                .replace("fullscreen = false\n", "")
                .replace("fullscreen = true\n", "")
                .replace("schema = 10", "schema = 7")
        };
        let mut remaining =
            usize::try_from(MAX_SETTINGS_BYTES).expect("file limit") - legacy_file(&expected).len();
        for argument in expected
            .settings
            .unity
            .default_launch_arguments
            .as_mut()
            .expect("arguments")
        {
            let count = remaining.min(4096 - argument.len());
            argument.push_str(&"x".repeat(count));
            remaining -= count;
        }
        assert_eq!(remaining, 0);
        let legacy = legacy_file(&expected);
        assert_eq!(legacy.len() as u64, MAX_SETTINGS_BYTES);
        assert_eq!(parse_settings(&legacy), Ok(expected));
        fs::write(&path, &legacy).expect("valid Config7 at byte bound");
        assert_eq!(
            FileConfigStore::new(path.clone()).initialize(),
            Err(ConfigStoreError::TooLarge)
        );
        assert_eq!(
            fs::read_to_string(&path).expect("original authority"),
            legacy
        );
        assert!(!path.with_extension("toml.new").exists());
        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn config9_migrates_all_historical_vectors_with_retained_settings() {
        let root = temporary_path("config7-window-migration");
        let path = root.join("settings.toml");
        fs::create_dir_all(&root).expect("fixture");
        let vectors: serde_json::Value = serde_json::from_str(include_str!(
            "../../../specs/config/settings-v10.contract-vectors.json"
        ))
        .expect("Config10 vectors");
        for vector in vectors["legacyInputs"]
            .as_array()
            .expect("historical vectors")
        {
            let source = vector["toml"].as_str().expect("TOML source");
            let expected = parse_settings(source).expect("valid historical input");
            assert_eq!(
                expected.revision,
                vector["expectedRevision"].as_u64().expect("revision")
            );
            assert_eq!(
                expected.settings.window.remember_state,
                vector["expectedSettings"]["window"]["rememberState"]
                    .as_bool()
                    .expect("window preference")
            );
            assert_eq!(
                expected.settings.projects.auto_discover,
                vector["expectedSettings"]["projects"]["autoDiscover"]
                    .as_bool()
                    .expect("project preference")
            );
            let expected_backups = &vector["expectedSettings"]["backups"];
            assert_eq!(
                expected.settings.backups.compression_mode.as_str(),
                expected_backups["compressionMode"]
                    .as_str()
                    .expect("compression")
            );
            assert_eq!(
                expected.settings.backups.include_vpm_packages,
                expected_backups["includeVpmPackages"]
                    .as_bool()
                    .expect("package preference")
            );
            assert_eq!(
                expected.settings.backups.include_library,
                expected_backups["includeLibrary"]
                    .as_bool()
                    .expect("Library preference")
            );
            fs::write(&path, source).expect("historical input");
            let store = FileConfigStore::new(path.clone());
            assert_eq!(store.initialize(), Ok(expected.clone()));
            let accepted = fs::read_to_string(&path).expect("migrated file");
            assert!(accepted.starts_with("schema = 10\n"));
            assert!(accepted.contains(&format!(
                "\n[window]\nremember_state = {}\n",
                expected.settings.window.remember_state
            )));
            if vector["schema"].as_u64().expect("historical schema") < 8 {
                assert!(accepted.contains(CONFIG8_DEFAULTS_TOML));
            }
            assert_eq!(
                FileConfigStore::new(path.clone()).initialize(),
                Ok(expected)
            );
            assert_eq!(fs::read_to_string(&path).expect("stable reopen"), accepted);
        }
        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn config9_rejects_new_levels_in_historical_schemas_without_rewriting_the_input() {
        let root = temporary_path("config9-historical-levels");
        let path = root.join("settings.toml");
        fs::create_dir_all(&root).expect("fixture");
        let mut snapshot = default_snapshot();
        snapshot.revision = 168;
        snapshot.settings.projects.auto_discover = false;
        snapshot.settings.window.remember_state = true;
        snapshot.settings.backups.include_vpm_packages = false;
        snapshot.settings.backups.include_library = true;
        for compression in [
            BackupCompression::Level3,
            BackupCompression::Level5,
            BackupCompression::Level7,
        ] {
            snapshot.settings.backups.compression_mode = compression;
            let current = String::from_utf8(serialize_settings(&snapshot)).expect("Config9 UTF-8");
            assert_eq!(parse_settings(&current), Ok(snapshot.clone()));
            for schema in 1..=8 {
                let historical = current
                    .replace("fullscreen = false\n", "")
                    .replace("fullscreen = true\n", "")
                    .replace("schema = 10", &format!("schema = {schema}"));
                fs::write(&path, &historical).expect("historical schema with unapproved level");
                assert_eq!(
                    FileConfigStore::new(path.clone()).initialize(),
                    Err(ConfigStoreError::Corrupt)
                );
                assert_eq!(
                    fs::read_to_string(&path).expect("input unchanged"),
                    historical
                );
            }
        }
        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[tokio::test]
    async fn config7_window_preference_is_durable_and_invalid_mixed_updates_leave_accepted_file() {
        use alcomd_application::{ConfigUnityUpdate, ConfigWindowUpdate};
        let root = temporary_path("config7-window-save");
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
        let legacy = String::from_utf8(serialize_settings(&expected))
            .expect("utf8")
            .replace("\n[window]\nremember_state = false\n", "")
            .replace(CONFIG8_DEFAULTS_TOML, "")
            .replace("fullscreen = false\n", "")
            .replace("fullscreen = true\n", "")
            .replace("schema = 10", "schema = 6");
        fs::write(&path, &legacy).expect("Config6 with retained preferences");
        let store = Arc::new(FileConfigStore::new(path.clone()));
        let app = M7OfficialApplication::new(
            NoStore,
            store.clone(),
            Arc::new(TestHub),
            Arc::new(TestHubLauncher),
            Arc::new(TestDirectories),
        );
        app.initialize_settings().await.expect("migrate Config6");
        assert_eq!(store.load(), Ok(expected.clone()));
        for remember_state in [true, false] {
            let saved = app
                .update_settings(
                    &AccessContext::local_owner(),
                    expected.revision,
                    ConfigUpdate {
                        projects: None,
                        backups: None,
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
        invalid_files.push(
            valid
                .replace(CONFIG8_DEFAULTS_TOML, "")
                .replace("fullscreen = false\n", "")
                .replace("fullscreen = true\n", "")
                .replace("schema = 10", "schema = 6"),
        );
        for invalid in invalid_files {
            fs::write(&path, &invalid).expect("invalid file");
            assert_eq!(store.initialize(), Err(ConfigStoreError::Corrupt));
            assert_eq!(fs::read_to_string(&path).expect("input retained"), invalid);
        }
        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn config6_migrates_config5_without_changing_revision_or_existing_preferences() {
        let root = temporary_path("config6-default-arguments-migration");
        let path = root.join("settings.toml");
        fs::create_dir_all(&root).expect("fixture");
        let mut expected = default_snapshot();
        expected.revision = 166;
        expected.settings.locale = ConfigLocale::JaJp;
        expected.settings.appearance.mode = ConfigAppearanceMode::Dark;
        expected.settings.appearance.source_color = Some("#112233".into());
        expected.settings.appearance.scheme = ConfigAppearanceScheme::FruitSalad;
        expected.settings.appearance.motion = ConfigAppearanceMotion::Full;
        expected.settings.packages.show_prerelease = true;
        expected.settings.packages.hide_local_user_packages = true;
        expected.settings.packages.hidden_repository_ids =
            vec!["00000000-0000-0000-0000-000000000001".into()];
        expected.settings.unity.hub_path = Some(r"C:\用户\Unity Hub\Unity Hub.exe".into());
        expected.settings.paths.project_directory = Some("/fixture/projects".into());
        expected.settings.paths.backup_directory = Some("/fixture/backups".into());
        let current = String::from_utf8(serialize_settings(&expected)).expect("utf8");
        let legacy = current
            .replace("\n[window]\nremember_state = false\n", "")
            .replace(CONFIG8_DEFAULTS_TOML, "")
            .replace("fullscreen = false\n", "")
            .replace("fullscreen = true\n", "")
            .replace("schema = 10", "schema = 5");
        fs::write(&path, &legacy).expect("Config5");
        let store = FileConfigStore::new(path.clone());
        assert_eq!(store.load(), Ok(expected.clone()));
        assert_eq!(fs::read_to_string(&path).expect("read-only load"), legacy);
        assert_eq!(store.initialize(), Ok(expected.clone()));
        assert_eq!(fs::read_to_string(&path).expect("migrated"), current);
        assert_eq!(
            FileConfigStore::new(path.clone()).initialize(),
            Ok(expected)
        );
        for schema in 3..=5 {
            let mut invalid = current
                .replace("\n[window]\nremember_state = false\n", "")
                .replace(CONFIG8_DEFAULTS_TOML, "")
                .replace("fullscreen = false\n", "")
                .replace("fullscreen = true\n", "")
                .replace("schema = 10", &format!("schema = {schema}"));
            if schema < 5 {
                invalid = invalid
                    .split("[paths]")
                    .next()
                    .expect("before paths")
                    .into();
            }
            if schema == 3 {
                invalid = invalid.replace(
                    "motion = \"full\"",
                    "density = \"default\"\nmotion = \"full\"",
                );
            }
            invalid = invalid.replace("[unity]\n", "[unity]\ndefault_launch_arguments = []\n");
            fs::write(&path, &invalid).expect("unsupported historical field");
            assert_eq!(store.initialize(), Err(ConfigStoreError::Corrupt));
            assert_eq!(fs::read_to_string(&path).expect("retained input"), invalid);
        }
        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[tokio::test]
    async fn config6_arguments_reopen_empty_clear_and_atomic_failure() {
        use alcomd_application::ConfigUnityUpdate;
        let root = temporary_path("config6-default-arguments");
        let path = root.join("settings.toml");
        let store = Arc::new(FileConfigStore::new(path.clone()));
        let app = M7OfficialApplication::new(
            NoStore,
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
    fn config5_paths_roundtrip_and_config4_migration_preserves_all_preferences() {
        let root = temporary_path("config5-paths");
        let path = root.join("settings.toml");
        fs::create_dir_all(&root).expect("fixture");
        let store = FileConfigStore::new(path.clone());
        let mut original = default_snapshot();
        original.revision = 153;
        original.settings.locale = ConfigLocale::JaJp;
        original.settings.appearance.motion = ConfigAppearanceMotion::Full;
        original.settings.unity.hub_path = Some("/fixtures/Hub".into());
        let legacy = String::from_utf8(serialize_settings(&original))
            .expect("utf8")
            .replace("\n[window]\nremember_state = false\n", "")
            .replace(CONFIG8_DEFAULTS_TOML, "")
            .replace("fullscreen = false\n", "")
            .replace("fullscreen = true\n", "")
            .replace("schema = 10", "schema = 4");
        fs::write(&path, &legacy).expect("legacy Config4");
        assert_eq!(store.initialize(), Ok(original.clone()));
        let migrated = fs::read_to_string(&path).expect("migrated current config");
        assert!(migrated.starts_with("schema = 10\n"));
        assert!(!migrated.contains("[paths]"));
        for candidate in [r"C:\用户\项目", "/home/测试/备份 \"local\""] {
            let mut saved = original.clone();
            saved.settings.paths.project_directory = Some(candidate.into());
            saved.settings.paths.backup_directory = Some(candidate.into());
            store.publish(&saved).expect("publish");
            assert_eq!(store.load(), Ok(saved.clone()));
            assert_eq!(FileConfigStore::new(path.clone()).initialize(), Ok(saved));
        }
        for schema in 1..=4 {
            let invalid = format!(
                "{}\n[paths]\nproject_directory = \"/unexpected\"\n",
                legacy.replace("schema = 4", &format!("schema = {schema}"))
            );
            assert_eq!(parse_settings(&invalid), Err(ConfigStoreError::Corrupt));
        }
        for suffix in [
            "\n[paths]\nproject_directory = \"\"\n",
            "\n[paths]\nbackup_directory = \"/bad\\q\"\n",
            "\n[paths]\nunknown = \"/path\"\n",
        ] {
            assert_eq!(
                parse_settings(&(migrated.clone() + suffix)),
                Err(ConfigStoreError::Corrupt)
            );
        }
        fs::remove_dir_all(&root).expect("remove fixture");
    }

    #[tokio::test]
    async fn config5_directory_changes_are_durable_and_combined_byte_overflow_keeps_accepted_file()
    {
        use alcomd_application::ConfigPathUpdate;
        let root = temporary_path("config5-durable-directory-update");
        let path = root.join("settings.toml");
        let store = Arc::new(FileConfigStore::new(path.clone()));
        let make_app = || {
            M7OfficialApplication::new(
                NoStore,
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
            show_prerelease: None,
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
    fn hub_path_roundtrip_escapes_and_preserves_legacy_defaults() {
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
        for schema in [1, 2] {
            let mut legacy = original_text
                .replace("\n[window]\nremember_state = false\n", "")
                .replace(CONFIG8_DEFAULTS_TOML, "")
                .replace("fullscreen = false\n", "")
                .replace("fullscreen = true\n", "")
                .replace("schema = 10", &format!("schema = {schema}"))
                .replace("scheme = \"vibrant\"\n", "density = \"default\"\n");
            if schema == 1 {
                legacy = legacy
                    .split("[packages]")
                    .next()
                    .expect("legacy root")
                    .to_owned();
            }
            let migrated = parse_settings(&legacy).expect("legacy defaults");
            assert_eq!(migrated.settings.unity.hub_path, None);
            assert_eq!(migrated.revision, original.revision);
            assert_eq!(
                parse_settings(&(legacy + "\n[unity]\nhub_path = \"/tmp/hub\"\n")),
                Err(ConfigStoreError::Corrupt)
            );
        }
    }

    #[tokio::test]
    async fn hub_path_is_durable_and_clear_survives_reopen_and_recovery() {
        use alcomd_application::ConfigUnityUpdate;
        let root = temporary_path("hub-path");
        let path = root.join("settings.toml");
        let _ = fs::remove_dir_all(&root);
        let store = Arc::new(FileConfigStore::new(path.clone()));
        let app = M7OfficialApplication::new(
            NoStore,
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
        fs::write(path.with_extension("toml.new"), b"partial").expect("partial temporary");
        assert_eq!(reopened.load().expect("recover saved path"), saved);
        let cleared = app
            .update_settings(
                &AccessContext::local_owner(),
                saved.revision,
                ConfigUpdate {
                    projects: None,
                    backups: None,
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
            assert!(encoded.starts_with("schema = 10\n"));
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
        for schema in [1, 2] {
            let legacy = encoded
                .replace("\n[window]\nremember_state = false\n", "")
                .replace(CONFIG8_DEFAULTS_TOML, "")
                .replace("fullscreen = false\n", "")
                .replace("fullscreen = true\n", "")
                .replace("schema = 10", &format!("schema = {schema}"))
                .replace("scheme = \"vibrant\"\n", "density = \"default\"\n");
            let legacy = if schema == 1 {
                legacy
                    .split("[packages]")
                    .next()
                    .expect("root and appearance")
                    .to_owned()
            } else {
                legacy
            };
            for (text, expected) in [
                ("system", ConfigAppearanceMotion::System),
                ("reduced", ConfigAppearanceMotion::Reduced),
            ] {
                let parsed = parse_settings(
                    &legacy.replace("motion = \"system\"", &format!("motion = \"{text}\"")),
                )
                .expect("preserve frozen historical motion");
                assert_eq!(parsed.settings.appearance.motion, expected);
            }
            assert_eq!(
                parse_settings(&legacy.replace("motion = \"system\"", "motion = \"full\"")),
                Err(ConfigStoreError::Corrupt)
            );
        }
    }

    #[tokio::test]
    async fn motion_only_updates_are_revisioned_durable_and_preserve_other_settings() {
        let root = temporary_path("motion-three-state");
        let path = root.join("settings.toml");
        let _ = fs::remove_dir_all(&root);
        let app = M7OfficialApplication::new(
            NoStore,
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
                        show_prerelease: Some(true),
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
                NoStore,
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
            assert!(encoded.starts_with("schema = 10\n"));
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
        for schema in [1, 2] {
            assert_eq!(
                parse_settings(
                    &encoded
                        .replace("\n[window]\nremember_state = false\n", "")
                        .replace(CONFIG8_DEFAULTS_TOML, "")
                        .replace("fullscreen = false\n", "")
                        .replace("fullscreen = true\n", "")
                        .replace("schema = 10", &format!("schema = {schema}"))
                        .replace(
                            "scheme = \"vibrant\"\n",
                            "scheme = \"vibrant\"\ndensity = \"default\"\n"
                        )
                ),
                Err(ConfigStoreError::Corrupt),
                "a newer field is invalid in an older schema"
            );
        }
    }

    #[tokio::test]
    async fn update_is_revisioned_durable_and_recovers_backup() {
        let root = temporary_path("roundtrip");
        let path = root.join("config/settings.toml");
        let _ = fs::remove_dir_all(&root);
        let app = M7OfficialApplication::new(
            NoStore,
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
                        show_prerelease: Some(true),
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

    #[tokio::test]
    async fn startup_migrates_v1_without_incrementing_revision() {
        let root = temporary_path("migration");
        let path = root.join("config/settings.toml");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(path.parent().expect("parent")).expect("create config directory");
        fs::write(
            &path,
            "schema = 1\nrevision = 7\nlocale = \"en-US\"\n\n[appearance]\nmode = \"dark\"\ndensity = \"compact\"\nmotion = \"reduced\"\n",
        )
        .expect("write v1");
        let app = M7OfficialApplication::new(
            NoStore,
            Arc::new(FileConfigStore::new(path.clone())),
            Arc::new(TestHub),
            Arc::new(TestHubLauncher),
            Arc::new(TestDirectories),
        );
        app.initialize_settings().await.expect("migrate v1");
        let migrated = app
            .get_settings(&AccessContext::local_owner())
            .await
            .expect("read current settings");
        assert_eq!(migrated.revision, 7);
        assert!(!migrated.settings.packages.show_prerelease);
        assert!(migrated.settings.packages.hidden_repository_ids.is_empty());
        assert!(!migrated.settings.packages.hide_local_user_packages);
        assert_eq!(
            migrated.settings.appearance.scheme,
            ConfigAppearanceScheme::Vibrant
        );
        assert_eq!(
            migrated.settings.appearance.mode,
            ConfigAppearanceMode::Dark
        );
        assert_eq!(
            migrated.settings.appearance.motion,
            ConfigAppearanceMotion::Reduced
        );
        assert_eq!(migrated.settings.locale, ConfigLocale::EnUs);
        assert!(
            fs::read_to_string(&path)
                .expect("read file")
                .starts_with("schema = 10\n")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn startup_migrates_v2_preserving_retained_settings_and_revision() {
        let root = temporary_path("theme-v2-migration");
        let path = root.join("settings.toml");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create fixture");
        let legacy = "schema = 2\nrevision = 37\nlocale = \"ja-JP\"\n\n[appearance]\nmode = \"light\"\nsource_color = \"#AABBCC\"\ndensity = \"compact\"\nmotion = \"reduced\"\n\n[packages]\nshow_prerelease = true\nhidden_repository_ids = [\"00000000-0000-0000-0000-000000000001\", \"ffffffff-ffff-ffff-ffff-ffffffffffff\"]\nhide_local_user_packages = true\n";
        fs::write(&path, legacy).expect("write v2");
        let expected = parse_settings(legacy).expect("supported v2 input");
        let app = M7OfficialApplication::new(
            NoStore,
            Arc::new(FileConfigStore::new(path.clone())),
            Arc::new(TestHub),
            Arc::new(TestHubLauncher),
            Arc::new(TestDirectories),
        );
        // Reading old configuration is supported before the startup publication boundary.
        assert_eq!(
            app.get_settings(&AccessContext::local_owner()).await,
            Ok(expected.clone())
        );
        app.initialize_settings()
            .await
            .expect("publish v4 migration");
        assert_eq!(
            app.get_settings(&AccessContext::local_owner()).await,
            Ok(expected.clone())
        );
        assert_eq!(expected.revision, 37);
        assert_eq!(
            expected.settings.appearance.scheme,
            ConfigAppearanceScheme::Vibrant
        );
        assert_eq!(
            expected.settings.appearance.source_color.as_deref(),
            Some("#AABBCC")
        );
        assert_eq!(
            expected.settings.appearance.mode,
            ConfigAppearanceMode::Light
        );
        assert_eq!(
            expected.settings.appearance.motion,
            ConfigAppearanceMotion::Reduced
        );
        assert_eq!(expected.settings.locale, ConfigLocale::JaJp);
        assert!(expected.settings.packages.show_prerelease);
        assert_eq!(expected.settings.packages.hidden_repository_ids.len(), 2);
        assert!(expected.settings.packages.hide_local_user_packages);
        assert_eq!(
            fs::read(&path).expect("canonical v4"),
            serialize_settings(&expected)
        );
        assert!(!path.with_extension("toml.new").exists());
        assert!(!path.with_extension("toml.bak").exists());
        let restarted = M7OfficialApplication::new(
            NoStore,
            Arc::new(FileConfigStore::new(path.clone())),
            Arc::new(TestHub),
            Arc::new(TestHubLauncher),
            Arc::new(TestDirectories),
        );
        restarted.initialize_settings().await.expect("restart");
        assert_eq!(restarted.candidate_settings().await, Ok(expected));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn startup_migrates_v3_removing_only_density_and_preserving_revision_and_settings() {
        let root = temporary_path("density-v3-migration");
        let path = root.join("settings.toml");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create fixture");
        let store = FileConfigStore::new(path.clone());
        let mut expected = default_snapshot();
        expected.revision = 91;
        expected.settings.appearance.mode = ConfigAppearanceMode::Dark;
        expected.settings.appearance.scheme = ConfigAppearanceScheme::FruitSalad;
        expected.settings.appearance.source_color = Some("#AABBCC".to_owned());
        expected.settings.appearance.motion = ConfigAppearanceMotion::Full;
        expected.settings.locale = ConfigLocale::JaJp;
        expected.settings.packages.show_prerelease = true;
        expected.settings.packages.hide_local_user_packages = true;
        expected.settings.packages.hidden_repository_ids = vec![
            "00000000-0000-0000-0000-000000000001".to_owned(),
            "ffffffff-ffff-ffff-ffff-ffffffffffff".to_owned(),
        ];
        expected.settings.unity.hub_path = Some(r"C:\用户\Unity Hub\Unity Hub.exe".to_owned());
        let canonical = String::from_utf8(serialize_settings(&expected)).expect("utf8");
        assert!(canonical.starts_with("schema = 10\n"));
        assert!(!canonical.contains("density"));
        for density in ["default", "compact"] {
            let legacy = canonical
                .replace("\n[window]\nremember_state = false\n", "")
                .replace(CONFIG8_DEFAULTS_TOML, "")
                .replace("fullscreen = false\n", "")
                .replace("fullscreen = true\n", "")
                .replace("schema = 10", "schema = 3")
                .replace(
                    "motion = \"full\"\n",
                    &format!("density = \"{density}\"\nmotion = \"full\"\n"),
                );
            fs::write(&path, &legacy).expect("write v3");
            assert_eq!(store.load(), Ok(expected.clone()));
            assert_eq!(fs::read_to_string(&path).expect("read-only load"), legacy);
            assert_eq!(store.initialize(), Ok(expected.clone()));
            assert_eq!(
                fs::read_to_string(&path).expect("migrated current settings"),
                canonical
            );
            assert_eq!(store.initialize(), Ok(expected.clone()));
            assert_eq!(
                fs::read_to_string(&path).expect("stable restart"),
                canonical
            );
            assert!(!path.with_extension("toml.new").exists());
            assert!(!path.with_extension("toml.bak").exists());
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_density_is_required_and_validated_before_discarding_without_rewriting_bad_inputs() {
        let root = temporary_path("density-legacy-validation");
        let path = root.join("settings.toml");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create fixture");
        let store = FileConfigStore::new(path.clone());
        let current = String::from_utf8(serialize_settings(&default_snapshot())).expect("utf8");
        for schema in 1..=3 {
            let mut legacy = current
                .replace("\n[window]\nremember_state = false\n", "")
                .replace(CONFIG8_DEFAULTS_TOML, "")
                .replace("fullscreen = false\n", "")
                .replace("fullscreen = true\n", "")
                .replace("schema = 10", &format!("schema = {schema}"));
            legacy = if schema < 3 {
                legacy.replace("scheme = \"vibrant\"\n", "density = \"default\"\n")
            } else {
                legacy.replace(
                    "motion = \"system\"\n",
                    "density = \"default\"\nmotion = \"system\"\n",
                )
            };
            if schema == 1 {
                legacy = legacy
                    .split("[packages]")
                    .next()
                    .expect("legacy appearance")
                    .to_owned();
            }
            assert_eq!(parse_settings(&legacy), Ok(default_snapshot()));
            assert_eq!(
                parse_settings(&legacy.replace("density = \"default\"", "density = \"compact\"")),
                Ok(default_snapshot())
            );
            let mut invalid = vec![
                legacy.replace("density = \"default\"\n", ""),
                legacy.replace("density = \"default\"", "density = \"unknown\""),
                legacy.replace("density = \"default\"", "density = true"),
                legacy.replace(
                    "density = \"default\"",
                    "density = \"default\"\ndensity = \"compact\"",
                ),
                legacy.replace("mode = \"system\"\n", ""),
                legacy.replace("motion = \"system\"\n", ""),
                legacy.replace("locale = \"system\"\n", ""),
                legacy.replace("revision = 1", "revision = 0"),
            ];
            if schema > 1 {
                invalid.push(legacy.replace("show_prerelease = false\n", ""));
            }
            if schema == 3 {
                invalid.push(legacy.replace("scheme = \"vibrant\"\n", ""));
            }
            for invalid in invalid {
                fs::write(&path, &invalid).expect("write malformed legacy input");
                assert_eq!(
                    store.initialize(),
                    Err(ConfigStoreError::Corrupt),
                    "schema {schema}: {invalid}"
                );
                assert_eq!(fs::read_to_string(&path).expect("input retained"), invalid);
                assert!(!path.with_extension("toml.new").exists());
                assert!(!path.with_extension("toml.bak").exists());
            }
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn current_config_rejects_removed_density_and_future_versions_without_rewriting() {
        let root = temporary_path("density-current-rejection");
        let path = root.join("settings.toml");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create fixture");
        let store = FileConfigStore::new(path.clone());
        let current = String::from_utf8(serialize_settings(&default_snapshot())).expect("utf8");
        let mut invalid = ["\"default\"", "\"compact\"", "null", "true"]
            .map(|value| {
                current.replace(
                    "motion = \"system\"\n",
                    &format!("density = {value}\nmotion = \"system\"\n"),
                )
            })
            .to_vec();
        invalid.push(current.replace("schema = 10", "schema = 11"));
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
            NoStore,
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
                        show_prerelease: Some(true),
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
    async fn theme_schema_migration_recovers_old_backup_and_rejects_invalid_without_replacement() {
        let root = temporary_path("theme-migration-recovery");
        let path = root.join("settings.toml");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create fixture");
        let legacy = "schema = 2\nrevision = 19\nlocale = \"en-US\"\n\n[appearance]\nmode = \"dark\"\ndensity = \"compact\"\nmotion = \"reduced\"\n\n[packages]\nshow_prerelease = true\nhidden_repository_ids = []\nhide_local_user_packages = true\n";
        fs::write(path.with_extension("toml.bak"), legacy).expect("old backup after rename");
        fs::write(path.with_extension("toml.new"), "incomplete replacement")
            .expect("interrupted write");
        let app = M7OfficialApplication::new(
            NoStore,
            Arc::new(FileConfigStore::new(path.clone())),
            Arc::new(TestHub),
            Arc::new(TestHubLauncher),
            Arc::new(TestDirectories),
        );
        app.initialize_settings()
            .await
            .expect("recover and migrate old backup");
        let expected = parse_settings(legacy).expect("expected preserved choices");
        assert_eq!(app.candidate_settings().await, Ok(expected.clone()));
        let valid = String::from_utf8(serialize_settings(&expected)).expect("utf8");
        for invalid in [
            valid.replace("scheme = \"vibrant\"", "scheme = \"unapproved\""),
            valid.replace("scheme = \"vibrant\"\n", ""),
        ] {
            fs::write(&path, &invalid).expect("invalid current file");
            assert_eq!(
                app.initialize_settings().await,
                Err(OfficialGuiError::Corrupt)
            );
            assert_eq!(
                fs::read_to_string(&path).expect("unchanged invalid file"),
                invalid
            );
            assert_eq!(
                app.candidate_settings().await,
                Err(OfficialGuiError::Unavailable)
            );
            assert!(!path.with_extension("toml.new").exists());
            assert!(!path.with_extension("toml.bak").exists());
        }
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn failed_config_read_or_update_invalidates_candidate_cache_until_confirmed_read() {
        let root = temporary_path("candidate-cache-failure");
        let path = root.join("settings.toml");
        let _ = fs::remove_dir_all(&root);
        let app = M7OfficialApplication::new(
            NoStore,
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
    fn maximum_legacy_preferences_fit_existing_byte_limit() {
        let mut snapshot = default_snapshot();
        snapshot.settings.appearance.source_color = Some("#FFFFFF".to_owned());
        snapshot.settings.packages.hidden_repository_ids = (0..256)
            .map(|value| format!("00000000-0000-0000-0000-{value:012x}"))
            .collect();
        let encoded = serialize_settings(&snapshot);
        assert!(encoded.len() as u64 <= MAX_SETTINGS_BYTES);
        assert_eq!(
            parse_settings(std::str::from_utf8(&encoded).expect("utf8")),
            Ok(snapshot)
        );
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
        assert!(serialize_settings(&oversized).len() as u64 > MAX_SETTINGS_BYTES);
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
        let unsorted = "schema = 2\nrevision = 1\nlocale = \"system\"\n\n[appearance]\nmode = \"system\"\ndensity = \"default\"\nmotion = \"system\"\n\n[packages]\nshow_prerelease = false\nhidden_repository_ids = [\"ffffffff-ffff-ffff-ffff-ffffffffffff\", \"00000000-0000-0000-0000-000000000000\"]\nhide_local_user_packages = false\n";
        assert_eq!(parse_settings(unsorted), Err(ConfigStoreError::Corrupt));
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

    #[derive(Clone)]
    struct NoStore;
    impl OfficialGuiStore for NoStore {
        async fn get_official_activity(&self, _: PrincipalId, _: String) -> Result<alcomd_application::OfficialActivityItem, StoreError> { panic!("this fixture must not read Activity") }
        async fn record_failed_activity(&self, _: PrincipalId, _: alcomd_application::NewFailedActivity) -> Result<alcomd_application::OfficialActivityItem, StoreError> { panic!("this fixture must not write Activity") }

        async fn list_official_activity(
            &self,
            _: PrincipalId,
            _: Option<OfficialActivityCursor>,
            _: u32,
        ) -> Result<OfficialActivityPage, StoreError> {
            unreachable!()
        }

    }

    #[test]
    fn config10_fullscreen_roundtrips_and_legacy_inputs_cannot_carry_the_new_field() {
        let root = temporary_path("config10-fullscreen");
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
        for schema in 7..=9 {
            let invalid = canonical.replace("schema = 10", &format!("schema = {schema}"));
            fs::write(&path, &invalid).expect("unapproved historical field");
            assert_eq!(store.initialize(), Err(ConfigStoreError::Corrupt));
            assert_eq!(
                fs::read_to_string(&path).expect("historical authority"),
                invalid
            );
        }
        let legacy9 = missing.replace("schema = 10", "schema = 9");
        fs::write(&path, legacy9).expect("valid Config9");
        saved.settings.window.fullscreen = false;
        assert_eq!(store.initialize(), Ok(saved));
        assert!(
            fs::read_to_string(&path)
                .expect("migrated")
                .starts_with("schema = 10\n")
        );
        fs::remove_dir_all(root).expect("cleanup fixture");
    }
}
