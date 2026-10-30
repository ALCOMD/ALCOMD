//! File-backed configuration adapter. The application owns permissions, update validation and CAS.
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use alcomd_application::{
    ConfigAppearance, ConfigAppearanceDensity, ConfigAppearanceMode, ConfigAppearanceMotion,
    ConfigAppearanceScheme, ConfigLocale, ConfigPackageSettings, ConfigSettings, ConfigSnapshot,
    ConfigStore, ConfigStoreError, ConfigUnitySettings,
};

const MAX_SETTINGS_BYTES: u64 = 16 * 1024;

/// The daemon's bounded, recoverable settings.toml storage adapter.
#[derive(Clone, Debug)]
pub struct FileConfigStore {
    path: PathBuf,
}

impl FileConfigStore {
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
        if snapshot.revision == 0 || contents.len() as u64 > MAX_SETTINGS_BYTES {
            return Err(ConfigStoreError::Corrupt);
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
    if schema < 3 {
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

fn parse_settings_with_schema(text: &str) -> Result<(ConfigSnapshot, u64), ConfigStoreError> {
    let mut section = "root";
    let mut seen = HashSet::<String>::new();
    let mut schema = None;
    let mut revision = None;
    let mut locale = None;
    let mut mode = None;
    let mut source_color = None;
    let mut scheme = None;
    let mut density = None;
    let mut motion = None;
    let mut show_prerelease = None;
    let mut hidden_repository_ids = None;
    let mut hide_local_user_packages = None;
    let mut hub_path = None;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            let next = match line {
                "[appearance]" => "appearance",
                "[packages]" => "packages",
                "[unity]" => "unity",
                _ => return Err(ConfigStoreError::Corrupt),
            };
            if !seen.insert(format!("section:{next}")) {
                return Err(ConfigStoreError::Corrupt);
            }
            section = next;
            continue;
        }
        let (key, value) = line.split_once('=').ok_or(ConfigStoreError::Corrupt)?;
        let key = key.trim();
        let value = value.trim();
        let identity = format!("{section}:{key}");
        if !seen.insert(identity) {
            return Err(ConfigStoreError::Corrupt);
        }
        match (section, key) {
            ("root", "schema") => schema = Some(parse_u64(value)?),
            ("root", "revision") => revision = Some(parse_u64(value)?),
            ("root", "locale") => locale = Some(parse_locale(parse_string(value)?)?),
            ("unity", "hub_path") => hub_path = Some(parse_path_string(value)?),
            ("appearance", "mode") => mode = Some(parse_mode(parse_string(value)?)?),
            ("appearance", "source_color") => {
                let value = parse_string(value)?.to_owned();
                source_color = Some(value);
            }
            ("appearance", "scheme") => scheme = Some(parse_scheme(parse_string(value)?)?),
            ("appearance", "density") => density = Some(parse_density(parse_string(value)?)?),
            ("appearance", "motion") => motion = Some(parse_motion(parse_string(value)?)?),
            ("packages", "show_prerelease") => show_prerelease = Some(parse_bool(value)?),
            ("packages", "hidden_repository_ids") => {
                hidden_repository_ids = Some(parse_uuid_array(value)?)
            }
            ("packages", "hide_local_user_packages") => {
                hide_local_user_packages = Some(parse_bool(value)?)
            }
            _ => return Err(ConfigStoreError::Corrupt),
        }
    }
    let schema = schema.ok_or(ConfigStoreError::Corrupt)?;
    if !matches!(schema, 1..=3) {
        return Err(ConfigStoreError::Corrupt);
    }
    if schema < 3 && seen.contains("section:unity") {
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
            || seen.contains("section:packages")
        {
            return Err(ConfigStoreError::Corrupt);
        }
        ConfigPackageSettings {
            show_prerelease: false,
            hidden_repository_ids: Vec::new(),
            hide_local_user_packages: false,
        }
    } else {
        if !seen.contains("section:packages") {
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
            appearance: ConfigAppearance {
                mode: mode.ok_or(ConfigStoreError::Corrupt)?,
                source_color,
                scheme,
                density: density.ok_or(ConfigStoreError::Corrupt)?,
                motion: motion.ok_or(ConfigStoreError::Corrupt)?,
            },
            locale: locale.ok_or(ConfigStoreError::Corrupt)?,
            packages,
            unity: ConfigUnitySettings { hub_path },
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
        "schema = 3\nrevision = {}\nlocale = \"{}\"\n\n[appearance]\nmode = \"{}\"\n{}scheme = \"{}\"\ndensity = \"{}\"\nmotion = \"{}\"\n\n[packages]\nshow_prerelease = {}\nhidden_repository_ids = [{}]\nhide_local_user_packages = {}\n",
        snapshot.revision,
        locale_str(snapshot.settings.locale),
        mode_str(snapshot.settings.appearance.mode),
        source,
        scheme_str(snapshot.settings.appearance.scheme),
        density_str(snapshot.settings.appearance.density),
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
    if let Some(path) = &snapshot.settings.unity.hub_path {
        contents.push_str("\n[unity]\nhub_path = \"");
        for character in path.chars() {
            if matches!(character, '\\' | '"') {
                contents.push('\\');
            }
            contents.push(character);
        }
        contents.push_str("\"\n");
    }
    contents.into_bytes()
}

fn publish_settings(path: &Path, contents: &[u8]) -> Result<(), ConfigStoreError> {
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
    file.sync_all().map_err(map_io)?;
    drop(file);
    let had_target = path.exists();
    if had_target {
        fs::rename(path, &backup).map_err(map_io)?;
    }
    if fs::rename(&temporary, path).is_err() {
        if had_target {
            let _ = fs::rename(&backup, path);
        }
        let _ = fs::remove_file(&temporary);
        return Err(ConfigStoreError::Unavailable);
    }
    let committed = read_settings_without_recovery(path)?;
    if committed.revision == 0 {
        return Err(ConfigStoreError::Corrupt);
    }
    if backup.exists() {
        fs::remove_file(backup).map_err(map_io)?;
    }
    Ok(())
}

fn recover_settings_replace(path: &Path) -> Result<(), ConfigStoreError> {
    let temporary = path.with_extension("toml.new");
    let backup = path.with_extension("toml.bak");
    if path.exists() {
        let _ = read_settings_without_recovery(path)?;
        if temporary.exists() {
            fs::remove_file(&temporary).map_err(map_io)?;
        }
        if backup.exists() {
            fs::remove_file(&backup).map_err(map_io)?;
        }
        return Ok(());
    }
    if backup.exists() {
        let _ = read_settings_without_recovery(&backup)?;
        fs::rename(&backup, path).map_err(map_io)?;
    }
    if temporary.exists() {
        fs::remove_file(temporary).map_err(map_io)?;
    }
    Ok(())
}

fn read_settings_without_recovery(path: &Path) -> Result<ConfigSnapshot, ConfigStoreError> {
    read_settings_with_schema(path).map(|(snapshot, _)| snapshot)
}

fn parse_string(value: &str) -> Result<&str, ConfigStoreError> {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .filter(|value| !value.contains('"') && !value.contains('\\'))
        .ok_or(ConfigStoreError::Corrupt)
}

fn parse_path_string(value: &str) -> Result<String, ConfigStoreError> {
    let inner = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .ok_or(ConfigStoreError::Corrupt)?;
    let mut path = String::with_capacity(inner.len());
    let mut characters = inner.chars();
    while let Some(character) = characters.next() {
        match character {
            '\\' => match characters.next() {
                Some(escaped @ ('\\' | '"')) => path.push(escaped),
                _ => return Err(ConfigStoreError::Corrupt),
            },
            '"' => return Err(ConfigStoreError::Corrupt),
            character => path.push(character),
        }
    }
    Ok(path)
}
fn parse_u64(value: &str) -> Result<u64, ConfigStoreError> {
    value.parse().map_err(|_| ConfigStoreError::Corrupt)
}
fn parse_bool(value: &str) -> Result<bool, ConfigStoreError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(ConfigStoreError::Corrupt),
    }
}
fn parse_uuid_array(value: &str) -> Result<Vec<String>, ConfigStoreError> {
    let inner = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .ok_or(ConfigStoreError::Corrupt)?
        .trim();
    let values = if inner.is_empty() {
        Vec::new()
    } else {
        inner
            .split(',')
            .map(|value| parse_string(value.trim()).map(str::to_owned))
            .collect::<Result<Vec<_>, _>>()?
    };
    Ok(values)
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
fn parse_density(value: &str) -> Result<ConfigAppearanceDensity, ConfigStoreError> {
    match value {
        "default" => Ok(ConfigAppearanceDensity::Default),
        "compact" => Ok(ConfigAppearanceDensity::Compact),
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
fn density_str(value: ConfigAppearanceDensity) -> &'static str {
    match value {
        ConfigAppearanceDensity::Default => "default",
        ConfigAppearanceDensity::Compact => "compact",
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
        OfficialDiagnosticCursor, OfficialDiagnosticPage, OfficialGuiError, OfficialGuiStore,
        PrincipalId, StoreError,
    };
    use std::sync::Arc;

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
                .replace("schema = 3", &format!("schema = {schema}"))
                .replace("scheme = \"vibrant\"\n", "");
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
        );
        app.initialize_settings().await.expect("initialize");
        let baseline = store.load().expect("baseline");
        let selected = r"C:\用户\Unity Hub\Unity Hub.exe";
        let saved = app
            .update_settings(
                &AccessContext::local_owner(),
                baseline.revision,
                ConfigUpdate {
                    appearance: None,
                    locale: None,
                    packages: None,
                    unity: Some(ConfigUnityUpdate {
                        hub_path: ConfigNullableUpdate::Set(selected.to_owned()),
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
                    appearance: None,
                    locale: None,
                    packages: None,
                    unity: Some(ConfigUnityUpdate {
                        hub_path: ConfigNullableUpdate::Clear,
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
            assert!(encoded.starts_with("schema = 3\n"));
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
                .replace("schema = 3", &format!("schema = {schema}"))
                .replace("scheme = \"vibrant\"\n", "");
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
        );
        let access = AccessContext::local_owner();
        let mut previous = app
            .update_settings(
                &access,
                1,
                ConfigUpdate {
                    unity: None,
                    appearance: Some(ConfigAppearanceUpdate {
                        mode: Some(ConfigAppearanceMode::Dark),
                        source_color: ConfigNullableUpdate::Set("#E040A0".to_owned()),
                        scheme: Some(ConfigAppearanceScheme::Neutral),
                        density: Some(ConfigAppearanceDensity::Compact),
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
                        unity: None,
                        appearance: Some(ConfigAppearanceUpdate {
                            mode: None,
                            source_color: ConfigNullableUpdate::Unchanged,
                            scheme: None,
                            density: None,
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
            assert!(encoded.starts_with("schema = 3\n"));
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
                parse_settings(&encoded.replace("schema = 3", &format!("schema = {schema}"))),
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
        );
        let access = AccessContext::local_owner();
        let updated = app
            .update_settings(
                &access,
                1,
                ConfigUpdate {
                    unity: None,
                    appearance: Some(ConfigAppearanceUpdate {
                        mode: Some(ConfigAppearanceMode::Dark),
                        source_color: ConfigNullableUpdate::Set("#AABBCC".to_owned()),
                        scheme: Some(ConfigAppearanceScheme::Expressive),
                        density: Some(ConfigAppearanceDensity::Compact),
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
        );
        app.initialize_settings().await.expect("migrate v1");
        let migrated = app
            .get_settings(&AccessContext::local_owner())
            .await
            .expect("read v3");
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
            migrated.settings.appearance.density,
            ConfigAppearanceDensity::Compact
        );
        assert_eq!(
            migrated.settings.appearance.motion,
            ConfigAppearanceMotion::Reduced
        );
        assert_eq!(migrated.settings.locale, ConfigLocale::EnUs);
        assert!(
            fs::read_to_string(&path)
                .expect("read file")
                .starts_with("schema = 3\n")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn startup_migrates_v2_preserving_every_setting_and_revision() {
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
        );
        // Reading old configuration is supported before the startup publication boundary.
        assert_eq!(
            app.get_settings(&AccessContext::local_owner()).await,
            Ok(expected.clone())
        );
        app.initialize_settings()
            .await
            .expect("publish v3 migration");
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
            expected.settings.appearance.density,
            ConfigAppearanceDensity::Compact
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
            fs::read(&path).expect("canonical v3"),
            serialize_settings(&expected)
        );
        assert!(!path.with_extension("toml.new").exists());
        assert!(!path.with_extension("toml.bak").exists());
        let restarted = M7OfficialApplication::new(
            NoStore,
            Arc::new(FileConfigStore::new(path.clone())),
            Arc::new(TestHub),
            Arc::new(TestHubLauncher),
        );
        restarted.initialize_settings().await.expect("restart");
        assert_eq!(restarted.candidate_settings().await, Ok(expected));
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
        );
        let access = AccessContext::local_owner();
        let initial = app
            .update_settings(
                &access,
                1,
                ConfigUpdate {
                    unity: None,
                    appearance: Some(ConfigAppearanceUpdate {
                        mode: Some(ConfigAppearanceMode::Dark),
                        source_color: ConfigNullableUpdate::Set("#112233".to_owned()),
                        scheme: Some(ConfigAppearanceScheme::Neutral),
                        density: Some(ConfigAppearanceDensity::Compact),
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
                    unity: None,
                    appearance: Some(ConfigAppearanceUpdate {
                        mode: None,
                        source_color: ConfigNullableUpdate::Unchanged,
                        scheme: Some(ConfigAppearanceScheme::FruitSalad),
                        density: None,
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
                    unity: None,
                    appearance: Some(ConfigAppearanceUpdate {
                        mode: Some(ConfigAppearanceMode::System),
                        source_color: ConfigNullableUpdate::Clear,
                        scheme: Some(ConfigAppearanceScheme::Vibrant),
                        density: None,
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
                    unity: None,
                    appearance: Some(ConfigAppearanceUpdate {
                        mode: None,
                        source_color: ConfigNullableUpdate::Set("#bad".to_owned()),
                        scheme: Some(ConfigAppearanceScheme::Fidelity),
                        density: None,
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
    fn maximum_v3_snapshot_fits_existing_byte_limit() {
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
        assert_eq!(store.publish(&oversized), Err(ConfigStoreError::Corrupt));
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
        async fn list_official_activity(
            &self,
            _: PrincipalId,
            _: Option<OfficialActivityCursor>,
            _: u32,
        ) -> Result<OfficialActivityPage, StoreError> {
            unreachable!()
        }
        async fn list_official_diagnostics(
            &self,
            _: PrincipalId,
            _: Option<OfficialDiagnosticCursor>,
            _: u32,
        ) -> Result<OfficialDiagnosticPage, StoreError> {
            unreachable!()
        }
    }
}
