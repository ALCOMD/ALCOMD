use std::collections::HashSet;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::{AccessContext, Permission, PrincipalId, StoreError};

const DEFAULT_PAGE_LIMIT: u32 = 100;
const MAX_PAGE_LIMIT: u32 = 200;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigAppearanceMode {
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigAppearanceScheme {
    TonalSpot,
    Fidelity,
    Monochrome,
    Neutral,
    Vibrant,
    Expressive,
    Content,
    Rainbow,
    FruitSalad,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigAppearanceDensity {
    Default,
    Compact,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigAppearanceMotion {
    System,
    Full,
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
    pub scheme: ConfigAppearanceScheme,
    pub density: ConfigAppearanceDensity,
    pub motion: ConfigAppearanceMotion,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigSettings {
    pub appearance: ConfigAppearance,
    pub locale: ConfigLocale,
    pub packages: ConfigPackageSettings,
    pub unity: ConfigUnitySettings,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ConfigUnitySettings {
    pub hub_path: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigUnityUpdate {
    pub hub_path: ConfigNullableUpdate<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnityHubStatus {
    Available,
    NotFound,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnityHubSource {
    Configured,
    Discovered,
    None,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnityHubConfiguredPathStatus {
    Unset,
    Available,
    Invalid,
    Unavailable,
}

/// Derived executable evidence. It is never persisted as configuration authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnityHubLocation {
    pub status: UnityHubStatus,
    pub source: UnityHubSource,
    pub path: Option<String>,
    pub configured_path_status: UnityHubConfiguredPathStatus,
}

impl UnityHubLocation {
    #[must_use]
    pub fn unavailable(configured: bool) -> Self {
        Self {
            status: UnityHubStatus::Unavailable,
            source: UnityHubSource::None,
            path: None,
            configured_path_status: if configured {
                UnityHubConfiguredPathStatus::Unavailable
            } else {
                UnityHubConfiguredPathStatus::Unset
            },
        }
    }
}

/// Blocking filesystem inspection port. Implementations must not execute Hub or write files.
pub trait UnityHubPlatform: Send + Sync + 'static {
    fn normalize_path(&self, path: &str) -> Result<String, OfficialGuiError>;
    fn inspect(&self, configured_path: Option<&str>) -> UnityHubLocation;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnityHubOpenOutcome {
    /// The operating system accepted process creation; no window readiness is implied.
    Spawned,
    /// No configured or discovered executable exists. No process was started.
    DownloadRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnityHubOpenError {
    InvalidInput,
    PermissionDenied,
    RevisionConflict,
    PathChanged,
    PathInvalid,
    Unavailable,
    Busy,
    OpenFailed,
    Internal,
}

/// Command port separate from read-only location inspection. Implementations revalidate
/// the Core-resolved target, start it without arguments and retain child-reaping ownership.
pub trait UnityHubLauncher: Send + Sync + 'static {
    fn launch(&self, path: &str) -> Result<(), UnityHubOpenError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigPackageSettings {
    pub show_prerelease: bool,
    pub hidden_repository_ids: Vec<String>,
    pub hide_local_user_packages: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigSnapshot {
    pub revision: u64,
    pub settings: ConfigSettings,
}

/// Synchronous typed configuration port. The caller serializes access and offloads blocking work.
/// Implementations own decoding, storage migration and recoverable publication, never user CAS.
pub trait ConfigStore: Send + Sync + 'static {
    fn initialize(&self) -> Result<ConfigSnapshot, ConfigStoreError>;
    fn load(&self) -> Result<ConfigSnapshot, ConfigStoreError>;
    /// Failure may follow publication; callers must invalidate any previously cached authority.
    fn publish(&self, snapshot: &ConfigSnapshot) -> Result<(), ConfigStoreError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigStoreError {
    Unavailable,
    Corrupt,
}

impl From<ConfigStoreError> for OfficialGuiError {
    fn from(error: ConfigStoreError) -> Self {
        match error {
            ConfigStoreError::Unavailable => Self::Unavailable,
            ConfigStoreError::Corrupt => Self::Corrupt,
        }
    }
}

impl Default for ConfigSnapshot {
    fn default() -> Self {
        default_snapshot()
    }
}

impl ConfigSettings {
    /// Validates normalized settings independently of their transport or storage representation.
    pub fn validate(&self) -> Result<(), OfficialGuiError> {
        if self
            .appearance
            .source_color
            .as_ref()
            .is_some_and(|value| !valid_source_color(value))
            || validate_hidden_repository_ids(self.packages.hidden_repository_ids.clone())?
                != self.packages.hidden_repository_ids
            || self
                .unity
                .hub_path
                .as_ref()
                .is_some_and(|path| !valid_hub_path_text(path))
        {
            return Err(OfficialGuiError::InvalidInput);
        }
        Ok(())
    }
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
    pub scheme: Option<ConfigAppearanceScheme>,
    pub density: Option<ConfigAppearanceDensity>,
    pub motion: Option<ConfigAppearanceMotion>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigUpdate {
    pub appearance: Option<ConfigAppearanceUpdate>,
    pub locale: Option<ConfigLocale>,
    pub packages: Option<ConfigPackageSettingsUpdate>,
    pub unity: Option<ConfigUnityUpdate>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigPackageSettingsUpdate {
    pub show_prerelease: Option<bool>,
    pub hidden_repository_ids: Option<Vec<String>>,
    pub hide_local_user_packages: Option<bool>,
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
    config: Arc<dyn ConfigStore>,
    hub_platform: Arc<dyn UnityHubPlatform>,
    hub_launcher: Arc<dyn UnityHubLauncher>,
    settings_lock: Arc<Mutex<Option<ConfigSnapshot>>>,
}

impl<S: OfficialGuiStore> M7OfficialApplication<S> {
    #[must_use]
    pub fn new(
        store: S,
        config: Arc<dyn ConfigStore>,
        hub_platform: Arc<dyn UnityHubPlatform>,
        hub_launcher: Arc<dyn UnityHubLauncher>,
    ) -> Self {
        Self {
            store,
            config,
            hub_platform,
            hub_launcher,
            settings_lock: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn get_settings(
        &self,
        access: &AccessContext,
    ) -> Result<ConfigSnapshot, OfficialGuiError> {
        access
            .require(Permission::SettingsRead)
            .map_err(|_| OfficialGuiError::PermissionDenied)?;
        let mut guard = Arc::clone(&self.settings_lock).lock_owned().await;
        let config = Arc::clone(&self.config);
        tokio::task::spawn_blocking(move || {
            *guard = None;
            let snapshot = config.load().map_err(OfficialGuiError::from)?;
            *guard = Some(snapshot.clone());
            Ok(snapshot)
        })
        .await
        .map_err(|_| OfficialGuiError::Unavailable)?
    }

    pub async fn initialize_settings(&self) -> Result<(), OfficialGuiError> {
        let mut guard = Arc::clone(&self.settings_lock).lock_owned().await;
        let config = Arc::clone(&self.config);
        tokio::task::spawn_blocking(move || {
            *guard = None;
            *guard = Some(config.initialize().map_err(OfficialGuiError::from)?);
            Ok(())
        })
        .await
        .map_err(|_| OfficialGuiError::Unavailable)?
    }

    /// Immutable initialized configuration for read-only evidence; never opens a file.
    pub async fn candidate_settings(&self) -> Result<ConfigSnapshot, OfficialGuiError> {
        self.settings_lock
            .lock()
            .await
            .clone()
            .ok_or(OfficialGuiError::Unavailable)
    }

    /// Observes the exact snapshot already returned by an authorized settings get/update.
    /// Inspection failure cannot turn a committed settings update into a failed command.
    pub async fn observe_unity_hub(&self, snapshot: &ConfigSnapshot) -> UnityHubLocation {
        let configured = snapshot.settings.unity.hub_path.clone();
        let has_configured = configured.is_some();
        let platform = Arc::clone(&self.hub_platform);
        tokio::task::spawn_blocking(move || platform.inspect(configured.as_deref()))
            .await
            .unwrap_or_else(|_| UnityHubLocation::unavailable(has_configured))
    }

    /// Opens only the current authoritative Hub target for the authenticated local owner.
    /// expected_path is a comparison guard, never an executable or argument instruction.
    pub async fn open_unity_hub(
        &self,
        access: &AccessContext,
        expected_revision: u64,
        expected_path: Option<String>,
    ) -> Result<UnityHubOpenOutcome, UnityHubOpenError> {
        if access.principal() != &PrincipalId::local_owner()
            || access.require(Permission::SettingsRead).is_err()
            || access.require(Permission::UnityHubOpen).is_err()
        {
            return Err(UnityHubOpenError::PermissionDenied);
        }
        if expected_revision == 0
            || expected_path
                .as_ref()
                .is_some_and(|path| !valid_hub_path_text(path))
        {
            return Err(UnityHubOpenError::InvalidInput);
        }
        let mut guard = Arc::clone(&self.settings_lock).lock_owned().await;
        let config = Arc::clone(&self.config);
        let platform = Arc::clone(&self.hub_platform);
        let launcher = Arc::clone(&self.hub_launcher);
        // Cancellation must not release settings coordination between validation and
        // process-creation acknowledgement. Child lifetime belongs to the launcher.
        tokio::task::spawn_blocking(move || {
            *guard = None;
            let snapshot = config.load().map_err(|_| UnityHubOpenError::Unavailable)?;
            *guard = Some(snapshot.clone());
            if snapshot.revision != expected_revision {
                return Err(UnityHubOpenError::RevisionConflict);
            }
            let location = platform.inspect(snapshot.settings.unity.hub_path.as_deref());
            if location.status == UnityHubStatus::Unavailable {
                return Err(UnityHubOpenError::Unavailable);
            }
            if location.path != expected_path {
                return Err(UnityHubOpenError::PathChanged);
            }
            match location.status {
                UnityHubStatus::Available => {
                    let path = location.path.ok_or(UnityHubOpenError::Unavailable)?;
                    launcher.launch(&path)?;
                    Ok(UnityHubOpenOutcome::Spawned)
                }
                UnityHubStatus::NotFound if snapshot.settings.unity.hub_path.is_none() => {
                    Ok(UnityHubOpenOutcome::DownloadRequired)
                }
                UnityHubStatus::NotFound => Err(UnityHubOpenError::PathInvalid),
                UnityHubStatus::Unavailable => Err(UnityHubOpenError::Unavailable),
            }
        })
        .await
        .map_err(|_| UnityHubOpenError::Internal)?
    }

    pub async fn update_settings(
        &self,
        access: &AccessContext,
        expected_revision: u64,
        mut update: ConfigUpdate,
    ) -> Result<ConfigSnapshot, OfficialGuiError> {
        access
            .require(Permission::SettingsManage)
            .map_err(|_| OfficialGuiError::PermissionDenied)?;
        if expected_revision == 0
            || update_is_empty(&update)
            || update
                .unity
                .as_ref()
                .is_some_and(|value| matches!(value.hub_path, ConfigNullableUpdate::Unchanged))
        {
            return Err(OfficialGuiError::InvalidInput);
        }
        // Move the owned guard into the disk task: cancellation cannot publish a new
        // file revision while leaving the candidate snapshot at the old revision.
        let mut guard = Arc::clone(&self.settings_lock).lock_owned().await;
        let config = Arc::clone(&self.config);
        let hub_platform = Arc::clone(&self.hub_platform);
        tokio::task::spawn_blocking(move || {
            // A publication failure can occur after rename. Never retain an older
            // confirmed snapshot when the resulting disk revision is uncertain.
            *guard = None;
            let current = config.load().map_err(OfficialGuiError::from)?;
            if current.revision != expected_revision {
                return Err(OfficialGuiError::RevisionConflict);
            }
            let revision = current
                .revision
                .checked_add(1)
                .ok_or(OfficialGuiError::Corrupt)?;
            if let Some(ConfigUnityUpdate {
                hub_path: ConfigNullableUpdate::Set(path),
            }) = &mut update.unity
            {
                if !valid_hub_path_text(path) {
                    return Err(OfficialGuiError::InvalidInput);
                }
                *path = hub_platform.normalize_path(path)?;
            }
            let settings = apply_update(current.settings, update)?;
            let snapshot = ConfigSnapshot { revision, settings };
            config.publish(&snapshot).map_err(OfficialGuiError::from)?;
            *guard = Some(snapshot.clone());
            Ok(snapshot)
        })
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
                scheme: ConfigAppearanceScheme::Vibrant,
                density: ConfigAppearanceDensity::Default,
                motion: ConfigAppearanceMotion::System,
            },
            locale: ConfigLocale::System,
            packages: ConfigPackageSettings {
                show_prerelease: false,
                hidden_repository_ids: Vec::new(),
                hide_local_user_packages: false,
            },
            unity: ConfigUnitySettings::default(),
        },
    }
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
        if let Some(scheme) = appearance.scheme {
            settings.appearance.scheme = scheme;
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
    if let Some(packages) = update.packages {
        if let Some(value) = packages.show_prerelease {
            settings.packages.show_prerelease = value;
        }
        if let Some(value) = packages.hidden_repository_ids {
            settings.packages.hidden_repository_ids = validate_hidden_repository_ids(value)?;
        }
        if let Some(value) = packages.hide_local_user_packages {
            settings.packages.hide_local_user_packages = value;
        }
    }
    if let Some(unity) = update.unity {
        match unity.hub_path {
            ConfigNullableUpdate::Unchanged => {}
            ConfigNullableUpdate::Clear => settings.unity.hub_path = None,
            ConfigNullableUpdate::Set(path) => settings.unity.hub_path = Some(path),
        }
    }
    settings.validate()?;
    Ok(settings)
}

fn update_is_empty(update: &ConfigUpdate) -> bool {
    update.locale.is_none()
        && update
            .unity
            .as_ref()
            .is_none_or(|value| matches!(value.hub_path, ConfigNullableUpdate::Unchanged))
        && update.appearance.as_ref().is_none_or(|value| {
            value.mode.is_none()
                && value.scheme.is_none()
                && value.density.is_none()
                && value.motion.is_none()
                && matches!(value.source_color, ConfigNullableUpdate::Unchanged)
        })
        && update.packages.as_ref().is_none_or(|value| {
            value.show_prerelease.is_none()
                && value.hidden_repository_ids.is_none()
                && value.hide_local_user_packages.is_none()
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

fn validate_hidden_repository_ids(
    mut values: Vec<String>,
) -> Result<Vec<String>, OfficialGuiError> {
    if values.len() > 256 || values.iter().any(|value| !canonical_uuid(value)) {
        return Err(OfficialGuiError::InvalidInput);
    }
    let mut seen = HashSet::with_capacity(values.len());
    if values.iter().any(|value| !seen.insert(value.clone())) {
        return Err(OfficialGuiError::InvalidInput);
    }
    values.sort_unstable();
    Ok(values)
}
fn canonical_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte),
        })
}
fn valid_source_color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value[1..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'A'..=b'F').contains(&byte))
}

fn valid_hub_path_text(value: &str) -> bool {
    !value.is_empty() && value.len() <= 4096 && !value.bytes().any(|byte| byte < 32 || byte == 127)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Condvar, Mutex as StdMutex};
    use tokio::sync::Notify;

    #[derive(Default)]
    struct RecordingHubLauncher {
        calls: StdMutex<Vec<String>>,
        failure: StdMutex<Option<UnityHubOpenError>>,
        blocked: StdMutex<bool>,
        release: Condvar,
        entered: Notify,
        panic_after_dispatch: AtomicBool,
    }

    impl UnityHubLauncher for RecordingHubLauncher {
        fn launch(&self, path: &str) -> Result<(), UnityHubOpenError> {
            self.calls
                .lock()
                .expect("launch calls")
                .push(path.to_owned());
            self.entered.notify_one();
            assert!(
                !self.panic_after_dispatch.load(Ordering::SeqCst),
                "unexpected launcher failure"
            );
            let mut blocked = self.blocked.lock().expect("launch gate");
            while *blocked {
                blocked = self.release.wait(blocked).expect("release launch");
            }
            self.failure
                .lock()
                .expect("launch error")
                .map_or(Ok(()), Err)
        }
    }

    struct OpenHubPlatform {
        location: UnityHubLocation,
        inspections: AtomicUsize,
    }

    impl UnityHubPlatform for OpenHubPlatform {
        fn normalize_path(&self, _: &str) -> Result<String, OfficialGuiError> {
            panic!("open must use the Core-inspected target, not normalize expectedPath")
        }

        fn inspect(&self, _: Option<&str>) -> UnityHubLocation {
            self.inspections.fetch_add(1, Ordering::SeqCst);
            self.location.clone()
        }
    }

    fn open_fixture(
        location: UnityHubLocation,
    ) -> (
        M7OfficialApplication<NoStore>,
        Arc<MemoryConfig>,
        Arc<OpenHubPlatform>,
        Arc<RecordingHubLauncher>,
    ) {
        let config = Arc::new(MemoryConfig::default());
        let platform = Arc::new(OpenHubPlatform {
            location,
            inspections: AtomicUsize::new(0),
        });
        let launcher = Arc::new(RecordingHubLauncher::default());
        let app =
            M7OfficialApplication::new(NoStore, config.clone(), platform.clone(), launcher.clone());
        (app, config, platform, launcher)
    }

    fn no_hub() -> UnityHubLocation {
        UnityHubLocation {
            status: UnityHubStatus::NotFound,
            source: UnityHubSource::None,
            path: None,
            configured_path_status: UnityHubConfiguredPathStatus::Unset,
        }
    }

    fn available_hub(
        source: UnityHubSource,
        configured_path_status: UnityHubConfiguredPathStatus,
    ) -> UnityHubLocation {
        UnityHubLocation {
            status: UnityHubStatus::Available,
            source,
            path: Some("/core/resolved-hub".into()),
            configured_path_status,
        }
    }

    #[tokio::test]
    async fn hub_open_requires_local_owner_and_both_permissions_before_any_io() {
        let (app, config, platform, launcher) = open_fixture(no_hub());
        let owner = PrincipalId::local_owner();
        for access in [
            AccessContext::new(owner.clone(), []),
            AccessContext::new(owner.clone(), [Permission::SettingsRead]),
            AccessContext::new(owner, [Permission::UnityHubOpen]),
            AccessContext::new(
                PrincipalId::parse("external:test").expect("principal"),
                [Permission::SettingsRead, Permission::UnityHubOpen],
            ),
        ] {
            assert_eq!(
                app.open_unity_hub(&access, 1, None).await,
                Err(UnityHubOpenError::PermissionDenied)
            );
        }
        assert_eq!(config.loads.load(Ordering::SeqCst), 0);
        assert_eq!(platform.inspections.load(Ordering::SeqCst), 0);
        assert!(launcher.calls.lock().expect("calls").is_empty());
    }

    #[tokio::test]
    async fn hub_open_rejects_bad_request_and_revision_or_path_drift_without_launching() {
        let (app, config, platform, launcher) = open_fixture(available_hub(
            UnityHubSource::Discovered,
            UnityHubConfiguredPathStatus::Unset,
        ));
        let access = AccessContext::local_owner();
        assert_eq!(
            app.open_unity_hub(&access, 0, None).await,
            Err(UnityHubOpenError::InvalidInput)
        );
        for path in [String::new(), "x\nHub".into(), "界".repeat(1366)] {
            assert_eq!(
                app.open_unity_hub(&access, 1, Some(path)).await,
                Err(UnityHubOpenError::InvalidInput)
            );
        }
        assert_eq!(config.loads.load(Ordering::SeqCst), 0);
        assert_eq!(
            app.open_unity_hub(&access, 2, platform.location.path.clone())
                .await,
            Err(UnityHubOpenError::RevisionConflict)
        );
        assert_eq!(platform.inspections.load(Ordering::SeqCst), 0);
        assert_eq!(
            app.open_unity_hub(&access, 1, Some("/client/other-hub".into()))
                .await,
            Err(UnityHubOpenError::PathChanged)
        );
        assert_eq!(
            app.open_unity_hub(&access, 1, None).await,
            Err(UnityHubOpenError::PathChanged)
        );
        assert!(launcher.calls.lock().expect("calls").is_empty());
        assert_eq!(config.publishes.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn hub_open_accepts_configured_discovered_and_invalid_manual_usable_fallback_targets() {
        for (source, status, configured) in [
            (
                UnityHubSource::Configured,
                UnityHubConfiguredPathStatus::Available,
                Some("/saved/hub"),
            ),
            (
                UnityHubSource::Discovered,
                UnityHubConfiguredPathStatus::Unset,
                None,
            ),
            (
                UnityHubSource::Discovered,
                UnityHubConfiguredPathStatus::Invalid,
                Some("/saved/missing-hub"),
            ),
        ] {
            let (app, config, platform, launcher) = open_fixture(available_hub(source, status));
            config
                .snapshot
                .lock()
                .expect("config")
                .settings
                .unity
                .hub_path = configured.map(str::to_owned);
            let original = config.snapshot.lock().expect("config").clone();
            assert_eq!(
                app.open_unity_hub(
                    &AccessContext::local_owner(),
                    1,
                    platform.location.path.clone()
                )
                .await,
                Ok(UnityHubOpenOutcome::Spawned)
            );
            assert_eq!(
                launcher.calls.lock().expect("calls").as_slice(),
                &["/core/resolved-hub".to_owned()]
            );
            assert_eq!(*config.snapshot.lock().expect("config"), original);
            assert_eq!(app.candidate_settings().await, Ok(original));
            assert_eq!(config.publishes.load(Ordering::SeqCst), 0);
        }
    }

    #[tokio::test]
    async fn hub_download_is_only_for_confirmed_automatic_absence() {
        let (app, config, _, launcher) = open_fixture(no_hub());
        assert_eq!(
            app.open_unity_hub(&AccessContext::local_owner(), 1, None)
                .await,
            Ok(UnityHubOpenOutcome::DownloadRequired)
        );
        config
            .snapshot
            .lock()
            .expect("config")
            .settings
            .unity
            .hub_path = Some("/saved/missing-hub".into());
        assert_eq!(
            app.open_unity_hub(&AccessContext::local_owner(), 1, None)
                .await,
            Err(UnityHubOpenError::PathInvalid)
        );
        assert_eq!(
            app.open_unity_hub(
                &AccessContext::local_owner(),
                1,
                Some("/old/visible-hub".into())
            )
            .await,
            Err(UnityHubOpenError::PathChanged)
        );
        assert!(launcher.calls.lock().expect("calls").is_empty());
        let (app, _, _, launcher) = open_fixture(UnityHubLocation::unavailable(false));
        assert_eq!(
            app.open_unity_hub(&AccessContext::local_owner(), 1, None)
                .await,
            Err(UnityHubOpenError::Unavailable)
        );
        assert!(launcher.calls.lock().expect("calls").is_empty());
    }

    #[tokio::test]
    async fn hub_launcher_errors_do_not_mutate_configuration_or_trigger_another_launch() {
        let (app, config, platform, launcher) = open_fixture(available_hub(
            UnityHubSource::Discovered,
            UnityHubConfiguredPathStatus::Unset,
        ));
        let original = config.snapshot.lock().expect("config").clone();
        for error in [
            UnityHubOpenError::Busy,
            UnityHubOpenError::OpenFailed,
            UnityHubOpenError::PathInvalid,
            UnityHubOpenError::Unavailable,
        ] {
            let previous_calls = launcher.calls.lock().expect("calls").len();
            *launcher.failure.lock().expect("failure") = Some(error);
            assert_eq!(
                app.open_unity_hub(
                    &AccessContext::local_owner(),
                    1,
                    platform.location.path.clone()
                )
                .await,
                Err(error)
            );
            assert_eq!(
                launcher.calls.lock().expect("calls").len(),
                previous_calls + 1,
                "no implicit retry"
            );
            assert_eq!(*config.snapshot.lock().expect("config"), original);
            assert_eq!(app.candidate_settings().await, Ok(original.clone()));
        }
        assert_eq!(config.publishes.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn unexpected_hub_dispatch_panic_is_internal_without_retry_or_configuration_change() {
        let (app, config, platform, launcher) = open_fixture(available_hub(
            UnityHubSource::Discovered,
            UnityHubConfiguredPathStatus::Unset,
        ));
        let original = config.snapshot.lock().expect("config").clone();
        launcher.panic_after_dispatch.store(true, Ordering::SeqCst);
        assert_eq!(
            app.open_unity_hub(
                &AccessContext::local_owner(),
                1,
                platform.location.path.clone()
            )
            .await,
            Err(UnityHubOpenError::Internal)
        );
        assert_eq!(
            launcher.calls.lock().expect("calls").len(),
            1,
            "uncertain dispatch must not be repeated"
        );
        assert_eq!(*config.snapshot.lock().expect("config"), original);
        assert_eq!(app.candidate_settings().await, Ok(original));
        assert_eq!(config.publishes.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn cancelled_hub_open_keeps_settings_coordination_until_process_acknowledgement() {
        let (app, config, platform, launcher) = open_fixture(available_hub(
            UnityHubSource::Discovered,
            UnityHubConfiguredPathStatus::Unset,
        ));
        *launcher.blocked.lock().expect("launch gate") = true;
        let application = app.clone();
        let path = platform.location.path.clone();
        let request = tokio::spawn(async move {
            application
                .open_unity_hub(&AccessContext::local_owner(), 1, path)
                .await
        });
        launcher.entered.notified().await;
        request.abort();
        assert!(request.await.expect_err("cancelled request").is_cancelled());
        let reader_app = app.clone();
        let reader = tokio::spawn(async move { reader_app.candidate_settings().await });
        tokio::task::yield_now().await;
        assert!(
            !reader.is_finished(),
            "coordination must remain owned until launch acknowledgement"
        );
        *launcher.blocked.lock().expect("launch gate") = false;
        launcher.release.notify_all();
        let confirmed = tokio::time::timeout(std::time::Duration::from_secs(2), reader)
            .await
            .expect("acknowledged")
            .expect("reader task")
            .expect("cache");
        assert_eq!(confirmed, *config.snapshot.lock().expect("config"));
        assert_eq!(launcher.calls.lock().expect("calls").len(), 1);
        assert_eq!(config.publishes.load(Ordering::SeqCst), 0);
    }

    #[derive(Default)]
    struct FakeHub {
        normalizations: AtomicUsize,
        reject: AtomicBool,
        panic_inspection: AtomicBool,
        inspected: StdMutex<Vec<Option<String>>>,
    }

    impl UnityHubPlatform for FakeHub {
        fn normalize_path(&self, path: &str) -> Result<String, OfficialGuiError> {
            self.normalizations.fetch_add(1, Ordering::SeqCst);
            if self.reject.load(Ordering::SeqCst) {
                Err(OfficialGuiError::InvalidInput)
            } else {
                Ok(format!("{path}.normalized"))
            }
        }

        fn inspect(&self, configured_path: Option<&str>) -> UnityHubLocation {
            assert!(
                !self.panic_inspection.load(Ordering::SeqCst),
                "inspection failure"
            );
            self.inspected
                .lock()
                .expect("inspections")
                .push(configured_path.map(str::to_owned));
            match configured_path {
                Some(path) => UnityHubLocation {
                    status: UnityHubStatus::Available,
                    source: UnityHubSource::Configured,
                    path: Some(path.to_owned()),
                    configured_path_status: UnityHubConfiguredPathStatus::Available,
                },
                None => UnityHubLocation {
                    status: UnityHubStatus::NotFound,
                    source: UnityHubSource::None,
                    path: None,
                    configured_path_status: UnityHubConfiguredPathStatus::Unset,
                },
            }
        }
    }

    #[derive(Default)]
    struct MemoryConfig {
        snapshot: StdMutex<ConfigSnapshot>,
        loads: AtomicUsize,
        publishes: AtomicUsize,
        fail_after_publication: AtomicBool,
        blocked: StdMutex<bool>,
        release: Condvar,
        entered: Notify,
    }

    impl ConfigStore for MemoryConfig {
        fn initialize(&self) -> Result<ConfigSnapshot, ConfigStoreError> {
            self.load()
        }
        fn load(&self) -> Result<ConfigSnapshot, ConfigStoreError> {
            self.loads.fetch_add(1, Ordering::SeqCst);
            Ok(self.snapshot.lock().expect("memory config").clone())
        }
        fn publish(&self, snapshot: &ConfigSnapshot) -> Result<(), ConfigStoreError> {
            self.publishes.fetch_add(1, Ordering::SeqCst);
            self.entered.notify_one();
            let mut blocked = self.blocked.lock().expect("publication gate");
            while *blocked {
                blocked = self.release.wait(blocked).expect("release publication");
            }
            *self.snapshot.lock().expect("memory config") = snapshot.clone();
            if self.fail_after_publication.load(Ordering::SeqCst) {
                Err(ConfigStoreError::Unavailable)
            } else {
                Ok(())
            }
        }
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

    fn locale_update(locale: ConfigLocale) -> ConfigUpdate {
        ConfigUpdate {
            appearance: None,
            locale: Some(locale),
            packages: None,
            unity: None,
        }
    }

    fn hub_update(hub_path: ConfigNullableUpdate<String>) -> ConfigUpdate {
        ConfigUpdate {
            appearance: None,
            locale: None,
            packages: None,
            unity: Some(ConfigUnityUpdate { hub_path }),
        }
    }

    #[tokio::test]
    async fn hub_permissions_cas_and_empty_container_reject_before_inspection() {
        let config = Arc::new(MemoryConfig::default());
        let hub = Arc::new(FakeHub::default());
        let app = M7OfficialApplication::new(
            NoStore,
            config.clone(),
            hub.clone(),
            Arc::new(RecordingHubLauncher::default()),
        );
        let update = hub_update(ConfigNullableUpdate::Set("/fixture/hub".into()));
        assert_eq!(
            app.update_settings(
                &AccessContext::new(PrincipalId::local_owner(), []),
                1,
                update.clone()
            )
            .await,
            Err(OfficialGuiError::PermissionDenied)
        );
        assert_eq!(config.loads.load(Ordering::SeqCst), 0);
        let mut empty = hub_update(ConfigNullableUpdate::Unchanged);
        empty.locale = Some(ConfigLocale::JaJp);
        assert_eq!(
            app.update_settings(&AccessContext::local_owner(), 1, empty)
                .await,
            Err(OfficialGuiError::InvalidInput)
        );
        assert_eq!(config.loads.load(Ordering::SeqCst), 0);
        assert_eq!(
            app.update_settings(&AccessContext::local_owner(), 2, update)
                .await,
            Err(OfficialGuiError::RevisionConflict)
        );
        assert_eq!(hub.normalizations.load(Ordering::SeqCst), 0);
        assert!(hub.inspected.lock().expect("inspections").is_empty());
        assert_eq!(config.publishes.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn invalid_hub_selection_rejects_mixed_update_atomically() {
        let config = Arc::new(MemoryConfig::default());
        let original = config.snapshot.lock().expect("config").clone();
        let hub = Arc::new(FakeHub::default());
        hub.reject.store(true, Ordering::SeqCst);
        let app = M7OfficialApplication::new(
            NoStore,
            config.clone(),
            hub.clone(),
            Arc::new(RecordingHubLauncher::default()),
        );
        let mut update = hub_update(ConfigNullableUpdate::Set("/fixture/missing".into()));
        update.locale = Some(ConfigLocale::JaJp);
        assert_eq!(
            app.update_settings(&AccessContext::local_owner(), 1, update)
                .await,
            Err(OfficialGuiError::InvalidInput)
        );
        assert_eq!(*config.snapshot.lock().expect("config"), original);
        assert_eq!(config.publishes.load(Ordering::SeqCst), 0);
        assert_eq!(hub.normalizations.load(Ordering::SeqCst), 1);
        for path in [String::new(), "a\0b".into(), "界".repeat(1366)] {
            assert_eq!(
                app.update_settings(
                    &AccessContext::local_owner(),
                    1,
                    hub_update(ConfigNullableUpdate::Set(path))
                )
                .await,
                Err(OfficialGuiError::InvalidInput)
            );
        }
        assert_eq!(
            hub.normalizations.load(Ordering::SeqCst),
            1,
            "invalid text never reaches filesystem port"
        );
    }

    #[tokio::test]
    async fn hub_save_normalizes_and_unrelated_updates_never_revalidate_removed_executable() {
        let config = Arc::new(MemoryConfig::default());
        let hub = Arc::new(FakeHub::default());
        let app = M7OfficialApplication::new(
            NoStore,
            config.clone(),
            hub.clone(),
            Arc::new(RecordingHubLauncher::default()),
        );
        let first = app
            .update_settings(
                &AccessContext::local_owner(),
                1,
                hub_update(ConfigNullableUpdate::Set("/fixture/hub".into())),
            )
            .await
            .expect("save");
        assert_eq!(
            first.settings.unity.hub_path.as_deref(),
            Some("/fixture/hub.normalized")
        );
        hub.reject.store(true, Ordering::SeqCst);
        let second = app
            .update_settings(
                &AccessContext::local_owner(),
                2,
                locale_update(ConfigLocale::JaJp),
            )
            .await
            .expect("unrelated save");
        assert_eq!(second.settings.unity, first.settings.unity);
        let cleared = app
            .update_settings(
                &AccessContext::local_owner(),
                3,
                hub_update(ConfigNullableUpdate::Clear),
            )
            .await
            .expect("clear");
        assert_eq!(cleared.revision, 4);
        assert_eq!(cleared.settings.unity.hub_path, None);
        assert_eq!(cleared.settings.locale, ConfigLocale::JaJp);
        assert_eq!(hub.normalizations.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn hub_evidence_uses_exact_response_snapshot_without_storage_or_authority_changes() {
        let config = Arc::new(MemoryConfig::default());
        let hub = Arc::new(FakeHub::default());
        let app = M7OfficialApplication::new(
            NoStore,
            config.clone(),
            hub.clone(),
            Arc::new(RecordingHubLauncher::default()),
        );
        let accepted = app
            .update_settings(
                &AccessContext::local_owner(),
                1,
                hub_update(ConfigNullableUpdate::Set("/fixture/old".into())),
            )
            .await
            .expect("first save");
        let current = app
            .update_settings(
                &AccessContext::local_owner(),
                2,
                hub_update(ConfigNullableUpdate::Clear),
            )
            .await
            .expect("second save");
        let loads = config.loads.load(Ordering::SeqCst);
        let publishes = config.publishes.load(Ordering::SeqCst);
        let evidence = app.observe_unity_hub(&accepted).await;
        assert_eq!(evidence.path, accepted.settings.unity.hub_path);
        assert_eq!(
            hub.inspected.lock().expect("inspections").as_slice(),
            std::slice::from_ref(&accepted.settings.unity.hub_path)
        );
        assert_eq!(config.loads.load(Ordering::SeqCst), loads);
        assert_eq!(config.publishes.load(Ordering::SeqCst), publishes);
        assert_eq!(app.candidate_settings().await, Ok(current));
    }

    #[tokio::test]
    async fn hub_inspector_failure_after_commit_returns_unavailable_evidence_only() {
        let config = Arc::new(MemoryConfig::default());
        let hub = Arc::new(FakeHub::default());
        let app = M7OfficialApplication::new(
            NoStore,
            config.clone(),
            hub.clone(),
            Arc::new(RecordingHubLauncher::default()),
        );
        let accepted = app
            .update_settings(
                &AccessContext::local_owner(),
                1,
                hub_update(ConfigNullableUpdate::Set("/fixture/hub".into())),
            )
            .await
            .expect("committed");
        hub.panic_inspection.store(true, Ordering::SeqCst);
        assert_eq!(
            app.observe_unity_hub(&accepted).await,
            UnityHubLocation::unavailable(true)
        );
        assert_eq!(config.publishes.load(Ordering::SeqCst), 1);
        assert_eq!(app.candidate_settings().await, Ok(accepted));
    }

    #[tokio::test]
    async fn settings_permissions_and_empty_updates_are_checked_before_storage() {
        let config = Arc::new(MemoryConfig::default());
        let app = M7OfficialApplication::new(
            NoStore,
            config.clone(),
            Arc::new(FakeHub::default()),
            Arc::new(RecordingHubLauncher::default()),
        );
        let denied = AccessContext::new(PrincipalId::local_owner(), []);
        assert_eq!(
            app.get_settings(&denied).await,
            Err(OfficialGuiError::PermissionDenied)
        );
        assert_eq!(
            app.update_settings(&denied, 1, locale_update(ConfigLocale::JaJp))
                .await,
            Err(OfficialGuiError::PermissionDenied)
        );
        assert_eq!(
            app.update_settings(
                &AccessContext::local_owner(),
                0,
                locale_update(ConfigLocale::JaJp)
            )
            .await,
            Err(OfficialGuiError::InvalidInput)
        );
        assert_eq!(
            app.update_settings(
                &AccessContext::local_owner(),
                1,
                ConfigUpdate {
                    appearance: None,
                    locale: None,
                    packages: None,
                    unity: None,
                }
            )
            .await,
            Err(OfficialGuiError::InvalidInput)
        );
        assert_eq!(config.loads.load(Ordering::SeqCst), 0);
        assert_eq!(config.publishes.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn competing_cas_updates_publish_once_and_preserve_unrelated_settings() {
        let config = Arc::new(MemoryConfig::default());
        let original = config.snapshot.lock().expect("memory config").clone();
        let app = M7OfficialApplication::new(
            NoStore,
            config.clone(),
            Arc::new(FakeHub::default()),
            Arc::new(RecordingHubLauncher::default()),
        );
        let access = AccessContext::local_owner();
        let (first, second) = tokio::join!(
            app.update_settings(&access, 1, locale_update(ConfigLocale::JaJp)),
            app.update_settings(&access, 1, locale_update(ConfigLocale::ZhCn))
        );
        let accepted = match (first, second) {
            (Ok(value), Err(OfficialGuiError::RevisionConflict))
            | (Err(OfficialGuiError::RevisionConflict), Ok(value)) => value,
            outcomes => panic!("expected one accepted CAS: {outcomes:?}"),
        };
        assert_eq!(accepted.revision, 2);
        assert_eq!(accepted.settings.appearance, original.settings.appearance);
        assert_eq!(accepted.settings.packages, original.settings.packages);
        assert_eq!(config.publishes.load(Ordering::SeqCst), 1);
        // A rejected CAS invalidates old candidate authority until a confirmed read.
        assert_eq!(
            app.candidate_settings().await,
            Err(OfficialGuiError::Unavailable)
        );
        assert_eq!(app.get_settings(&access).await, Ok(accepted.clone()));
        assert_eq!(app.candidate_settings().await, Ok(accepted));
    }

    #[tokio::test]
    async fn invalid_values_and_revision_overflow_never_publish() {
        let config = Arc::new(MemoryConfig::default());
        let original = config.snapshot.lock().expect("memory config").clone();
        let app = M7OfficialApplication::new(
            NoStore,
            config.clone(),
            Arc::new(FakeHub::default()),
            Arc::new(RecordingHubLauncher::default()),
        );
        let bad = ConfigUpdate {
            appearance: Some(ConfigAppearanceUpdate {
                mode: Some(ConfigAppearanceMode::Dark),
                source_color: ConfigNullableUpdate::Set("#bad".into()),
                scheme: None,
                density: None,
                motion: None,
            }),
            locale: None,
            packages: None,
            unity: None,
        };
        assert_eq!(
            app.update_settings(&AccessContext::local_owner(), 1, bad)
                .await,
            Err(OfficialGuiError::InvalidInput)
        );
        assert_eq!(*config.snapshot.lock().expect("memory config"), original);
        config.snapshot.lock().expect("memory config").revision = u64::MAX;
        assert_eq!(
            app.update_settings(
                &AccessContext::local_owner(),
                u64::MAX,
                locale_update(ConfigLocale::JaJp)
            )
            .await,
            Err(OfficialGuiError::Corrupt)
        );
        assert_eq!(config.publishes.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn uncertain_publication_invalidates_cache_until_read_confirms_actual_authority() {
        let config = Arc::new(MemoryConfig::default());
        let app = M7OfficialApplication::new(
            NoStore,
            config.clone(),
            Arc::new(FakeHub::default()),
            Arc::new(RecordingHubLauncher::default()),
        );
        app.initialize_settings()
            .await
            .expect("initialize authority");
        config.fail_after_publication.store(true, Ordering::SeqCst);
        assert_eq!(
            app.update_settings(
                &AccessContext::local_owner(),
                1,
                locale_update(ConfigLocale::JaJp)
            )
            .await,
            Err(OfficialGuiError::Unavailable)
        );
        assert_eq!(
            app.candidate_settings().await,
            Err(OfficialGuiError::Unavailable)
        );
        let confirmed = app
            .get_settings(&AccessContext::local_owner())
            .await
            .expect("confirm outcome");
        assert_eq!(confirmed.revision, 2);
        assert_eq!(confirmed.settings.locale, ConfigLocale::JaJp);
        let loads = config.loads.load(Ordering::SeqCst);
        assert_eq!(app.candidate_settings().await, Ok(confirmed));
        assert_eq!(
            config.loads.load(Ordering::SeqCst),
            loads,
            "candidate reads never invoke storage"
        );
    }

    #[tokio::test]
    async fn cancelling_caller_keeps_owned_lock_until_publication_and_cache_finish() {
        let config = Arc::new(MemoryConfig::default());
        *config.blocked.lock().expect("publication gate") = true;
        let app = M7OfficialApplication::new(
            NoStore,
            config.clone(),
            Arc::new(FakeHub::default()),
            Arc::new(RecordingHubLauncher::default()),
        );
        app.initialize_settings()
            .await
            .expect("initialize authority");
        let writer = app.clone();
        let task = tokio::spawn(async move {
            writer
                .update_settings(
                    &AccessContext::local_owner(),
                    1,
                    locale_update(ConfigLocale::JaJp),
                )
                .await
        });
        config.entered.notified().await;
        task.abort();
        assert!(task.await.expect_err("caller cancelled").is_cancelled());
        let reader = app.clone();
        let read = tokio::spawn(async move { reader.candidate_settings().await });
        tokio::task::yield_now().await;
        assert!(
            !read.is_finished(),
            "owned publication lock must outlive caller"
        );
        *config.blocked.lock().expect("publication gate") = false;
        config.release.notify_all();
        let confirmed = tokio::time::timeout(std::time::Duration::from_secs(2), read)
            .await
            .expect("publication finishes")
            .expect("reader task")
            .expect("confirmed cached authority");
        assert_eq!(confirmed.revision, 2);
        assert_eq!(confirmed.settings.locale, ConfigLocale::JaJp);
        assert_eq!(
            config.loads.load(Ordering::SeqCst),
            2,
            "candidate read does not reload storage"
        );
        assert_eq!(*config.snapshot.lock().expect("memory config"), confirmed);
    }
}
