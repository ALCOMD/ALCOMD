use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::{AccessContext, Permission, PrincipalId, StoreError};

const MAX_SETTINGS_BYTES: u64 = 16 * 1024;
const DEFAULT_PAGE_LIMIT: u32 = 100;
const MAX_PAGE_LIMIT: u32 = 200;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigAppearanceMode {
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigAppearanceDensity {
    Default,
    Compact,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigAppearanceMotion {
    System,
    Reduced,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigLocale {
    System,
    EnUs,
    ZhCn,
    JaJp,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigAppearance {
    pub mode: ConfigAppearanceMode,
    pub source_color: Option<String>,
    pub density: ConfigAppearanceDensity,
    pub motion: ConfigAppearanceMotion,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigSettings {
    pub appearance: ConfigAppearance,
    pub locale: ConfigLocale,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigSnapshot {
    pub revision: u64,
    pub settings: ConfigSettings,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfigNullableUpdate<T> {
    Unchanged,
    Clear,
    Set(T),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigAppearanceUpdate {
    pub mode: Option<ConfigAppearanceMode>,
    pub source_color: ConfigNullableUpdate<String>,
    pub density: Option<ConfigAppearanceDensity>,
    pub motion: Option<ConfigAppearanceMotion>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigUpdate {
    pub appearance: Option<ConfigAppearanceUpdate>,
    pub locale: Option<ConfigLocale>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfficialActivityCursor {
    pub occurred_at_ms: u64,
    pub source_rank: u8,
    pub stable_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OfficialActivityKind {
    Operation,
    Event,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfficialActivityItem {
    pub occurred_at_ms: u64,
    pub kind: OfficialActivityKind,
    pub summary_code: String,
    pub operation_id: Option<String>,
    pub event_sequence: Option<u64>,
    pub resource_kind: Option<String>,
    pub resource_id: Option<String>,
    pub state: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfficialActivityPage {
    pub items: Vec<OfficialActivityItem>,
    pub next_cursor: Option<OfficialActivityCursor>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfficialDiagnosticCursor {
    pub occurred_at_ms: u64,
    pub operation_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfficialDiagnosticItem {
    pub occurred_at_ms: u64,
    pub subsystem: String,
    pub code: String,
    pub diagnostic_id: Option<String>,
    pub operation_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfficialDiagnosticPage {
    pub items: Vec<OfficialDiagnosticItem>,
    pub next_cursor: Option<OfficialDiagnosticCursor>,
}

pub trait OfficialGuiStore: Clone + Send + Sync + 'static {
    fn list_official_activity(
        &self,
        owner: PrincipalId,
        cursor: Option<OfficialActivityCursor>,
        limit: u32,
    ) -> impl std::future::Future<Output = Result<OfficialActivityPage, StoreError>> + Send;

    fn list_official_diagnostics(
        &self,
        owner: PrincipalId,
        cursor: Option<OfficialDiagnosticCursor>,
        limit: u32,
    ) -> impl std::future::Future<Output = Result<OfficialDiagnosticPage, StoreError>> + Send;
}

#[derive(Clone)]
pub struct M7OfficialApplication<S> {
    store: S,
    settings_path: Arc<PathBuf>,
    settings_lock: Arc<Mutex<()>>,
}

impl<S: OfficialGuiStore> M7OfficialApplication<S> {
    #[must_use]
    pub fn new(store: S, settings_path: PathBuf) -> Self {
        Self {
            store,
            settings_path: Arc::new(settings_path),
            settings_lock: Arc::new(Mutex::new(())),
        }
    }

    pub async fn get_settings(
        &self,
        access: &AccessContext,
    ) -> Result<ConfigSnapshot, OfficialGuiError> {
        access
            .require(Permission::SettingsRead)
            .map_err(|_| OfficialGuiError::PermissionDenied)?;
        let _guard = self.settings_lock.lock().await;
        let path = Arc::clone(&self.settings_path);
        tokio::task::spawn_blocking(move || read_settings(&path))
            .await
            .map_err(|_| OfficialGuiError::Unavailable)?
    }

    pub async fn update_settings(
        &self,
        access: &AccessContext,
        expected_revision: u64,
        update: ConfigUpdate,
    ) -> Result<ConfigSnapshot, OfficialGuiError> {
        access
            .require(Permission::SettingsManage)
            .map_err(|_| OfficialGuiError::PermissionDenied)?;
        if expected_revision == 0 || update_is_empty(&update) {
            return Err(OfficialGuiError::InvalidInput);
        }
        let _guard = self.settings_lock.lock().await;
        let path = Arc::clone(&self.settings_path);
        tokio::task::spawn_blocking(move || update_settings_file(&path, expected_revision, update))
            .await
            .map_err(|_| OfficialGuiError::Unavailable)?
    }

    pub async fn list_activity(
        &self,
        access: &AccessContext,
        cursor: Option<OfficialActivityCursor>,
        limit: Option<u32>,
    ) -> Result<OfficialActivityPage, OfficialGuiError> {
        access
            .require(Permission::ActivityRead)
            .map_err(|_| OfficialGuiError::PermissionDenied)?;
        let limit = validate_limit(limit)?;
        validate_activity_cursor(cursor.as_ref())?;
        self.store
            .list_official_activity(access.principal().clone(), cursor, limit)
            .await
            .map_err(Into::into)
    }

    pub async fn list_diagnostics(
        &self,
        access: &AccessContext,
        cursor: Option<OfficialDiagnosticCursor>,
        limit: Option<u32>,
    ) -> Result<OfficialDiagnosticPage, OfficialGuiError> {
        access
            .require(Permission::DiagnosticsRead)
            .map_err(|_| OfficialGuiError::PermissionDenied)?;
        let limit = validate_limit(limit)?;
        if cursor.as_ref().is_some_and(|value| {
            value.occurred_at_ms > i64::MAX as u64 || !safe_identity(&value.operation_id)
        }) {
            return Err(OfficialGuiError::InvalidInput);
        }
        self.store
            .list_official_diagnostics(access.principal().clone(), cursor, limit)
            .await
            .map_err(Into::into)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OfficialGuiError {
    InvalidInput,
    PermissionDenied,
    RevisionConflict,
    Unavailable,
    Corrupt,
}

impl From<StoreError> for OfficialGuiError {
    fn from(_: StoreError) -> Self {
        Self::Unavailable
    }
}

fn default_snapshot() -> ConfigSnapshot {
    ConfigSnapshot {
        revision: 1,
        settings: ConfigSettings {
            appearance: ConfigAppearance {
                mode: ConfigAppearanceMode::System,
                source_color: None,
                density: ConfigAppearanceDensity::Default,
                motion: ConfigAppearanceMotion::System,
            },
            locale: ConfigLocale::System,
        },
    }
}

fn read_settings(path: &Path) -> Result<ConfigSnapshot, OfficialGuiError> {
    recover_settings_replace(path)?;
    if !path.exists() {
        return Ok(default_snapshot());
    }
    let metadata = fs::symlink_metadata(path).map_err(map_io)?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_SETTINGS_BYTES {
        return Err(OfficialGuiError::Corrupt);
    }
    let mut bytes =
        Vec::with_capacity(usize::try_from(metadata.len()).map_err(|_| OfficialGuiError::Corrupt)?);
    File::open(path)
        .map_err(map_io)?
        .take(MAX_SETTINGS_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(map_io)?;
    if bytes.len() as u64 > MAX_SETTINGS_BYTES || bytes.contains(&0) {
        return Err(OfficialGuiError::Corrupt);
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| OfficialGuiError::Corrupt)?;
    parse_settings(text)
}

fn update_settings_file(
    path: &Path,
    expected_revision: u64,
    update: ConfigUpdate,
) -> Result<ConfigSnapshot, OfficialGuiError> {
    let current = read_settings(path)?;
    if current.revision != expected_revision {
        return Err(OfficialGuiError::RevisionConflict);
    }
    let revision = current
        .revision
        .checked_add(1)
        .ok_or(OfficialGuiError::Corrupt)?;
    let settings = apply_update(current.settings, update)?;
    let next = ConfigSnapshot { revision, settings };
    publish_settings(path, &serialize_settings(&next))?;
    Ok(next)
}

fn apply_update(
    mut settings: ConfigSettings,
    update: ConfigUpdate,
) -> Result<ConfigSettings, OfficialGuiError> {
    if let Some(appearance) = update.appearance {
        if let Some(mode) = appearance.mode {
            settings.appearance.mode = mode;
        }
        match appearance.source_color {
            ConfigNullableUpdate::Unchanged => {}
            ConfigNullableUpdate::Clear => settings.appearance.source_color = None,
            ConfigNullableUpdate::Set(value) => {
                if !valid_source_color(&value) {
                    return Err(OfficialGuiError::InvalidInput);
                }
                settings.appearance.source_color = Some(value);
            }
        }
        if let Some(density) = appearance.density {
            settings.appearance.density = density;
        }
        if let Some(motion) = appearance.motion {
            settings.appearance.motion = motion;
        }
    }
    if let Some(locale) = update.locale {
        settings.locale = locale;
    }
    Ok(settings)
}

fn parse_settings(text: &str) -> Result<ConfigSnapshot, OfficialGuiError> {
    let mut section = "root";
    let mut seen = HashSet::<String>::new();
    let mut schema = None;
    let mut revision = None;
    let mut locale = None;
    let mut mode = None;
    let mut source_color = None;
    let mut density = None;
    let mut motion = None;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            if line != "[appearance]"
                || section != "root"
                || !seen.insert("section:appearance".to_owned())
            {
                return Err(OfficialGuiError::Corrupt);
            }
            section = "appearance";
            continue;
        }
        let (key, value) = line.split_once('=').ok_or(OfficialGuiError::Corrupt)?;
        let key = key.trim();
        let value = value.trim();
        let identity = format!("{section}:{key}");
        if !seen.insert(identity) {
            return Err(OfficialGuiError::Corrupt);
        }
        match (section, key) {
            ("root", "schema") => schema = Some(parse_u64(value)?),
            ("root", "revision") => revision = Some(parse_u64(value)?),
            ("root", "locale") => locale = Some(parse_locale(parse_string(value)?)?),
            ("appearance", "mode") => mode = Some(parse_mode(parse_string(value)?)?),
            ("appearance", "source_color") => {
                let value = parse_string(value)?.to_owned();
                if !valid_source_color(&value) {
                    return Err(OfficialGuiError::Corrupt);
                }
                source_color = Some(value);
            }
            ("appearance", "density") => density = Some(parse_density(parse_string(value)?)?),
            ("appearance", "motion") => motion = Some(parse_motion(parse_string(value)?)?),
            _ => return Err(OfficialGuiError::Corrupt),
        }
    }
    if schema != Some(1) {
        return Err(OfficialGuiError::Corrupt);
    }
    let revision = revision
        .filter(|value| *value > 0)
        .ok_or(OfficialGuiError::Corrupt)?;
    Ok(ConfigSnapshot {
        revision,
        settings: ConfigSettings {
            appearance: ConfigAppearance {
                mode: mode.ok_or(OfficialGuiError::Corrupt)?,
                source_color,
                density: density.ok_or(OfficialGuiError::Corrupt)?,
                motion: motion.ok_or(OfficialGuiError::Corrupt)?,
            },
            locale: locale.ok_or(OfficialGuiError::Corrupt)?,
        },
    })
}

fn serialize_settings(snapshot: &ConfigSnapshot) -> Vec<u8> {
    let source = snapshot
        .settings
        .appearance
        .source_color
        .as_ref()
        .map_or_else(String::new, |value| format!("source_color = \"{value}\"\n"));
    format!(
        "schema = 1\nrevision = {}\nlocale = \"{}\"\n\n[appearance]\nmode = \"{}\"\n{}density = \"{}\"\nmotion = \"{}\"\n",
        snapshot.revision,
        locale_str(snapshot.settings.locale),
        mode_str(snapshot.settings.appearance.mode),
        source,
        density_str(snapshot.settings.appearance.density),
        motion_str(snapshot.settings.appearance.motion),
    ).into_bytes()
}

fn publish_settings(path: &Path, contents: &[u8]) -> Result<(), OfficialGuiError> {
    let parent = path.parent().ok_or(OfficialGuiError::Unavailable)?;
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
        return Err(OfficialGuiError::Unavailable);
    }
    let committed = read_settings_without_recovery(path)?;
    if committed.revision == 0 {
        return Err(OfficialGuiError::Corrupt);
    }
    if backup.exists() {
        fs::remove_file(backup).map_err(map_io)?;
    }
    Ok(())
}

fn recover_settings_replace(path: &Path) -> Result<(), OfficialGuiError> {
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

fn read_settings_without_recovery(path: &Path) -> Result<ConfigSnapshot, OfficialGuiError> {
    let metadata = fs::symlink_metadata(path).map_err(map_io)?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_SETTINGS_BYTES {
        return Err(OfficialGuiError::Corrupt);
    }
    let bytes = fs::read(path).map_err(map_io)?;
    if bytes.len() as u64 > MAX_SETTINGS_BYTES || bytes.contains(&0) {
        return Err(OfficialGuiError::Corrupt);
    }
    parse_settings(std::str::from_utf8(&bytes).map_err(|_| OfficialGuiError::Corrupt)?)
}

fn update_is_empty(update: &ConfigUpdate) -> bool {
    update.locale.is_none()
        && update.appearance.as_ref().is_none_or(|value| {
            value.mode.is_none()
                && value.density.is_none()
                && value.motion.is_none()
                && matches!(value.source_color, ConfigNullableUpdate::Unchanged)
        })
}

fn validate_limit(limit: Option<u32>) -> Result<u32, OfficialGuiError> {
    let value = limit.unwrap_or(DEFAULT_PAGE_LIMIT);
    (value > 0 && value <= MAX_PAGE_LIMIT)
        .then_some(value)
        .ok_or(OfficialGuiError::InvalidInput)
}

fn validate_activity_cursor(
    cursor: Option<&OfficialActivityCursor>,
) -> Result<(), OfficialGuiError> {
    if cursor.is_some_and(|value| {
        value.occurred_at_ms > i64::MAX as u64
            || value.source_rank > 1
            || !safe_identity(&value.stable_id)
    }) {
        Err(OfficialGuiError::InvalidInput)
    } else {
        Ok(())
    }
}

fn safe_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 255
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphanumeric() || index > 0 && matches!(byte, b'.' | b'_' | b':' | b'-')
        })
}

fn parse_string(value: &str) -> Result<&str, OfficialGuiError> {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .filter(|value| !value.contains('"') && !value.contains('\\'))
        .ok_or(OfficialGuiError::Corrupt)
}
fn parse_u64(value: &str) -> Result<u64, OfficialGuiError> {
    value.parse().map_err(|_| OfficialGuiError::Corrupt)
}
fn valid_source_color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value[1..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'A'..=b'F').contains(&byte))
}
fn parse_mode(value: &str) -> Result<ConfigAppearanceMode, OfficialGuiError> {
    match value {
        "system" => Ok(ConfigAppearanceMode::System),
        "light" => Ok(ConfigAppearanceMode::Light),
        "dark" => Ok(ConfigAppearanceMode::Dark),
        _ => Err(OfficialGuiError::Corrupt),
    }
}
fn parse_density(value: &str) -> Result<ConfigAppearanceDensity, OfficialGuiError> {
    match value {
        "default" => Ok(ConfigAppearanceDensity::Default),
        "compact" => Ok(ConfigAppearanceDensity::Compact),
        _ => Err(OfficialGuiError::Corrupt),
    }
}
fn parse_motion(value: &str) -> Result<ConfigAppearanceMotion, OfficialGuiError> {
    match value {
        "system" => Ok(ConfigAppearanceMotion::System),
        "reduced" => Ok(ConfigAppearanceMotion::Reduced),
        _ => Err(OfficialGuiError::Corrupt),
    }
}
fn parse_locale(value: &str) -> Result<ConfigLocale, OfficialGuiError> {
    match value {
        "system" => Ok(ConfigLocale::System),
        "en-US" => Ok(ConfigLocale::EnUs),
        "zh-CN" => Ok(ConfigLocale::ZhCn),
        "ja-JP" => Ok(ConfigLocale::JaJp),
        _ => Err(OfficialGuiError::Corrupt),
    }
}
fn mode_str(value: ConfigAppearanceMode) -> &'static str {
    match value {
        ConfigAppearanceMode::System => "system",
        ConfigAppearanceMode::Light => "light",
        ConfigAppearanceMode::Dark => "dark",
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
fn map_io(_: io::Error) -> OfficialGuiError {
    OfficialGuiError::Unavailable
}

#[cfg(test)]
mod tests {
    use super::*;

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
            Err(OfficialGuiError::Corrupt)
        );
    }

    #[tokio::test]
    async fn update_is_revisioned_durable_and_recovers_backup() {
        let root = temporary_path("roundtrip");
        let path = root.join("config/settings.toml");
        let _ = fs::remove_dir_all(&root);
        let app = M7OfficialApplication::new(NoStore, path.clone());
        let access = AccessContext::local_owner();
        let updated = app
            .update_settings(
                &access,
                1,
                ConfigUpdate {
                    appearance: Some(ConfigAppearanceUpdate {
                        mode: Some(ConfigAppearanceMode::Dark),
                        source_color: ConfigNullableUpdate::Set("#AABBCC".to_owned()),
                        density: Some(ConfigAppearanceDensity::Compact),
                        motion: Some(ConfigAppearanceMotion::Reduced),
                    }),
                    locale: Some(ConfigLocale::ZhCn),
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
                    appearance: None,
                    locale: Some(ConfigLocale::EnUs)
                }
            )
            .await,
            Err(OfficialGuiError::RevisionConflict)
        );
        fs::rename(&path, path.with_extension("toml.bak")).expect("simulate backup boundary");
        assert_eq!(app.get_settings(&access).await.expect("recover"), updated);
        let _ = fs::remove_dir_all(root);
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
