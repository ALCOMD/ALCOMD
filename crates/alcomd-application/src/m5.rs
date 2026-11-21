//! M5 Unity installation, writer observation and launch use cases.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::{
    AccessContext, IdempotencyKey, M3RegistryStore, Permission, PrincipalId, ProjectId,
    ProjectRecord, Revision, UnityInstallationId, UnityLaunchId,
};

/// Verified or intentionally unknown Unity executable architecture.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnityArchitecture {
    X86_64,
    Arm64,
    Universal,
    Unknown,
}

/// Source that produced a candidate before executable validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitySourceKind {
    Manual,
    HubConfig,
    KnownInstallRoot,
    UnityCliHint,
}

/// Validated input ready for persistence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UnityInstallationObservation {
    pub executable_path: String,
    pub filesystem_identity: Vec<u8>,
    pub unity_version: String,
    pub architecture: UnityArchitecture,
    pub source_kind: UnitySourceKind,
    pub observed_at_ms: u64,
}

/// Durable Unity installation registry row.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UnityInstallationRecord {
    pub installation_id: UnityInstallationId,
    pub observation: UnityInstallationObservation,
    pub revision: Revision,
    pub updated_at_ms: u64,
}

/// Stable tuple cursor for installation listing.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UnityInstallationCursor {
    pub updated_at_ms: u64,
    pub installation_id: UnityInstallationId,
}

/// Bounded installation page.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UnityInstallationPage {
    pub installations: Vec<UnityInstallationRecord>,
    pub next_cursor: Option<UnityInstallationCursor>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnityVersionOptions {
    pub versions: Vec<String>,
    pub default_version: Option<String>,
}

/// Per-project safe Unity launch arguments. A missing row is represented by a
/// revision-zero sentinel and never selects an Editor installation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectUnityLaunchConfig {
    pub project_id: ProjectId,
    pub arguments: Vec<String>,
    pub revision: Option<Revision>,
    pub updated_at_ms: u64,
}

/// Stored project override and the currently resolved launch arguments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectUnityLaunchConfigView {
    pub config: ProjectUnityLaunchConfig,
    pub default_arguments: Vec<String>,
    pub effective_arguments: Vec<String>,
}

/// Core-owned, coherent snapshot of global launch defaults.
pub trait UnityLaunchDefaultsSource: Send + Sync {
    fn arguments(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<String>, M5UnityError>> + Send + '_>>;
}

impl UnityLaunchDefaultsSource for crate::M7OfficialApplication {
    fn arguments(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<String>, M5UnityError>> + Send + '_>> {
        Box::pin(async {
            self.candidate_settings()
                .await
                .map(|snapshot| {
                    snapshot
                        .settings
                        .unity
                        .default_launch_arguments
                        .unwrap_or_default()
                })
                .map_err(|_| M5UnityError::new(M5UnityErrorCode::Internal))
        })
    }
}

/// Exact one-shot launch candidates for the project's observed Unity version.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UnityLaunchOptions {
    pub project_id: ProjectId,
    pub project_revision: Revision,
    pub project_unity_version: String,
    pub exact_matching_installations: Vec<UnityInstallationRecord>,
    pub migration_version_options: Option<Vec<UnityMigrationVersionOption>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UnityMigrationVersionOption {
    pub unity_version: String,
    pub installations: Vec<UnityMigrationInstallationOption>,
    pub default_installation_id: Option<UnityInstallationId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UnityMigrationInstallationOption {
    pub installation_id: UnityInstallationId,
    pub architecture: UnityArchitecture,
}

/// One short-lived, non-persisted process observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnityProcessObservation {
    pub pid: u32,
    pub start_time: u64,
    pub name: String,
    pub executable_identity: Option<Vec<u8>>,
    /// Stable executable metadata proved a different product, not a failed read.
    pub verified_non_editor: bool,
    pub arguments: Option<Vec<String>>,
}

/// One selected, registered Unity editing session without process or path details.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnitySession {
    pub project_id: ProjectId,
    pub started_at_ms: u64,
}

/// Safe projection of one complete observation for the local Core user.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnitySessionObservation {
    pub current_session: Option<UnitySession>,
    pub editor_count: u32,
}

struct CachedUnitySessions {
    sampled_at: Instant,
    observation: Result<UnitySessionObservation, M5UnityError>,
}

const UNITY_SESSION_SAMPLE_INTERVAL: Duration = Duration::from_secs(5);
const UNITY_SESSION_SAMPLE_TIMEOUT: Duration = Duration::from_millis(1_500);

/// Public writer state is deliberately not a boolean.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnityWriterStateKind {
    RunningConfirmed,
    RunningSuspected,
    NotObserved,
    Unknown,
}

/// Safe evidence kind that never contains argv, PID or a private path.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnityWriterEvidenceKind {
    ProcessProjectArgument,
    ProcessUnreadable,
    InspectionError,
}

/// Safe bounded writer result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UnityWriterState {
    pub project_id: ProjectId,
    pub state: UnityWriterStateKind,
    pub evidence: Vec<UnityWriterEvidenceKind>,
    pub checked_at_ms: u64,
}

/// State carried by a durable launch result and idempotent replay.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnityLaunchState {
    Opening,
    Open,
    Failed,
}

/// Durable, non-sensitive launch record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UnityLaunchRecord {
    pub launch_id: UnityLaunchId,
    pub project_id: ProjectId,
    pub installation_id: UnityInstallationId,
    pub state: UnityLaunchState,
    pub spawn_accepted: bool,
    pub created_at_ms: u64,
}

/// Stable M5 Unity error code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum M5UnityErrorCode {
    InvalidInput,
    PermissionDenied,
    RevisionConflict,
    IdempotencyConflict,
    ProjectNotRegistered,
    InstallationNotFound,
    InstallationInvalid,
    InstallationInUse,
    EditorSelectionRequired,
    VersionUnverified,
    VersionMismatch,
    ProjectSelectorForbidden,
    ProjectRunning,
    LaunchStateUncertain,
    LaunchFailed,
    LaunchNotFound,
    StoreUnavailable,
    Internal,
}

/// Safe M5 Unity error with no platform detail.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct M5UnityError {
    code: M5UnityErrorCode,
    context: Option<crate::TechnicalErrorContext>,
}

impl M5UnityError {
    #[must_use]
    pub const fn new(code: M5UnityErrorCode) -> Self {
        Self {
            code,
            context: None,
        }
    }

    #[must_use]
    pub const fn code(&self) -> M5UnityErrorCode {
        self.code
    }
}

impl std::fmt::Display for M5UnityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Unity request failed ({:?})", self.code)
    }
}

impl M5UnityError {
    pub fn with_source(mut self, error: impl std::error::Error + Send + Sync + 'static) -> Self {
        self.context = Some(self.context.take().unwrap_or_default().with_source(error));
        self
    }
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.context = Some(self.context.take().unwrap_or_default().with_path(path));
        self
    }

    pub fn diagnostic_id(&self) -> Option<&str> {
        self.context
            .as_ref()
            .and_then(|c| c.diagnostic_id.as_deref())
    }
    pub fn path(&self) -> Option<&str> {
        self.context.as_ref().and_then(|c| c.path.as_deref())
    }
}
impl std::error::Error for M5UnityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.context
            .as_ref()
            .and_then(crate::TechnicalErrorContext::source_error)
    }
}

/// Authoritative persistence port for the M5 Unity registry.
pub trait M5UnityStore: Clone + Send + Sync + 'static {
    fn register_installation(
        &self,
        owner: PrincipalId,
        observation: UnityInstallationObservation,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<(UnityInstallationRecord, bool), M5UnityError>> + Send;
    fn get_installation(
        &self,
        owner: PrincipalId,
        id: UnityInstallationId,
    ) -> impl Future<Output = Result<UnityInstallationRecord, M5UnityError>> + Send;
    fn list_installations(
        &self,
        owner: PrincipalId,
        cursor: Option<UnityInstallationCursor>,
        limit: u32,
    ) -> impl Future<Output = Result<UnityInstallationPage, M5UnityError>> + Send;
    fn remove_installation(
        &self,
        owner: PrincipalId,
        id: UnityInstallationId,
        expected: Revision,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<(bool, bool), M5UnityError>> + Send;
    fn synchronize_installations(
        &self,
        owner: PrincipalId,
        observations: Vec<UnityInstallationObservation>,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<(UnityInstallationPage, bool), M5UnityError>> + Send;
    fn get_project_launch_config(
        &self,
        owner: PrincipalId,
        project_id: ProjectId,
    ) -> impl Future<Output = Result<ProjectUnityLaunchConfig, M5UnityError>> + Send;
    fn set_project_launch_config(
        &self,
        owner: PrincipalId,
        project_id: ProjectId,
        arguments: Vec<String>,
        expected: Option<Revision>,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<(ProjectUnityLaunchConfig, bool, bool), M5UnityError>> + Send;
    fn clear_project_launch_config(
        &self,
        owner: PrincipalId,
        project_id: ProjectId,
        expected: Option<Revision>,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<(ProjectUnityLaunchConfig, bool, bool), M5UnityError>> + Send;
    fn accept_launch(
        &self,
        owner: PrincipalId,
        project: ProjectRecord,
        config: ProjectUnityLaunchConfig,
        installation_id: UnityInstallationId,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<(UnityLaunchRecord, bool), M5UnityError>> + Send;
    fn replay_launch(
        &self,
        owner: PrincipalId,
        project: ProjectRecord,
        config: ProjectUnityLaunchConfig,
        installation_id: UnityInstallationId,
        key: IdempotencyKey,
    ) -> impl Future<Output = Result<Option<UnityLaunchRecord>, M5UnityError>> + Send;
    fn set_launch_state(
        &self,
        owner: PrincipalId,
        launch_id: UnityLaunchId,
        state: UnityLaunchState,
        spawn_accepted: bool,
    ) -> impl Future<Output = Result<UnityLaunchRecord, M5UnityError>> + Send;
}

/// Platform port. Implementations expose no sysinfo or OS-specific types.
pub trait M5UnityPlatform: Clone + Send + Sync + 'static {
    fn validate_installation(
        &self,
        executable_path: String,
        source_kind: UnitySourceKind,
    ) -> impl Future<Output = Result<UnityInstallationObservation, M5UnityError>> + Send;
    fn discover_installations(
        &self,
    ) -> impl Future<Output = Result<Vec<UnityInstallationObservation>, M5UnityError>> + Send;
    fn observe_processes(
        &self,
    ) -> impl Future<Output = Result<Vec<UnityProcessObservation>, M5UnityError>> + Send;
    /// Observes processes for a bounded background sample without blocking its executor.
    fn observe_session_processes(
        &self,
    ) -> impl Future<Output = Result<Vec<UnityProcessObservation>, M5UnityError>> + Send {
        self.observe_processes()
    }
    /// Current foreground process, when the platform can reliably observe it.
    fn foreground_process(&self) -> Option<u32> {
        None
    }
    fn path_identity(
        &self,
        path: String,
    ) -> impl Future<Output = Result<Vec<u8>, M5UnityError>> + Send;
    /// Resolves a project selection made by an observed editor without blocking sampling.
    fn session_project_identity(
        &self,
        path: String,
    ) -> impl Future<Output = Result<Vec<u8>, M5UnityError>> + Send {
        self.path_identity(path)
    }
    fn launch(
        &self,
        executable_path: String,
        project_root: String,
        arguments: Vec<String>,
    ) -> impl Future<Output = Result<(), M5UnityError>> + Send;
}

/// M5 Unity application service.
#[derive(Clone)]
pub struct M5UnityApplication<S: M5UnityStore + M3RegistryStore, P: M5UnityPlatform> {
    store: S,
    platform: P,
    defaults: Arc<dyn UnityLaunchDefaultsSource>,
    unity_sessions: Arc<tokio::sync::Mutex<Option<CachedUnitySessions>>>,
}

impl<S: M5UnityStore + M3RegistryStore, P: M5UnityPlatform> M5UnityApplication<S, P> {
    #[must_use]
    pub fn new(store: S, platform: P, defaults: Arc<dyn UnityLaunchDefaultsSource>) -> Self {
        Self {
            store,
            platform,
            defaults,
            unity_sessions: Arc::default(),
        }
    }

    /// Core-only projection. The calling adapter must authorize the current invocation.
    pub async fn observe_local_unity_sessions(
        &self,
    ) -> Result<UnitySessionObservation, M5UnityError> {
        let mut cache = self.unity_sessions.lock().await;
        if let Some(cached) = cache.as_ref()
            && cached.sampled_at.elapsed() < UNITY_SESSION_SAMPLE_INTERVAL
        {
            return cached.observation.clone();
        }

        let sampled_at = Instant::now();
        let observation = tokio::time::timeout(
            UNITY_SESSION_SAMPLE_TIMEOUT,
            self.sample_local_unity_sessions(),
        )
        .await
        .unwrap_or_else(|_| Err(M5UnityError::new(M5UnityErrorCode::Internal)));
        *cache = Some(CachedUnitySessions {
            sampled_at,
            observation: observation.clone(),
        });
        observation
    }

    async fn sample_local_unity_sessions(&self) -> Result<UnitySessionObservation, M5UnityError> {
        let foreground_before = self.platform.foreground_process();
        let processes = self.platform.observe_session_processes().await?;
        let foreground_after = self.platform.foreground_process();
        let foreground = foreground_before.filter(|pid| Some(*pid) == foreground_after);
        let owner = PrincipalId::local_owner();

        let mut installations = BTreeSet::new();
        let mut cursor = None;
        loop {
            let page = self
                .store
                .list_installations(owner.clone(), cursor, 1_000)
                .await?;
            installations.extend(
                page.installations
                    .into_iter()
                    .map(|record| record.observation.filesystem_identity),
            );
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }

        let confirmed =
            confirm_session_processes(&self.platform, &installations, processes).await?;

        let mut projects = BTreeMap::new();
        if !confirmed.is_empty() {
            let mut cursor = None;
            loop {
                let page = self
                    .store
                    .list_projects(owner.clone(), cursor, 1_000)
                    .await
                    .map_err(|_| M5UnityError::new(M5UnityErrorCode::StoreUnavailable))?;
                projects.extend(
                    page.projects
                        .into_iter()
                        .map(|record| (record.observation.path_identity_key, record.project_id)),
                );
                cursor = page.next_cursor;
                if cursor.is_none() {
                    break;
                }
            }
        }

        project_unity_sessions(confirmed, &projects, foreground)
    }

    pub async fn list_installations(
        &self,
        access: &AccessContext,
        cursor: Option<UnityInstallationCursor>,
        limit: u32,
        include_version_options: bool,
    ) -> Result<(UnityInstallationPage, Option<UnityVersionOptions>), M5UnityError> {
        require(access, Permission::UnityRead)?;
        if !(1..=1_000).contains(&limit) || (include_version_options && cursor.is_some()) {
            return Err(M5UnityError::new(M5UnityErrorCode::InvalidInput));
        }
        let page = self
            .store
            .list_installations(access.principal().clone(), cursor, limit)
            .await?;
        if !include_version_options {
            return Ok((page, None));
        }
        let mut versions: BTreeSet<_> = page
            .installations
            .iter()
            .filter_map(|row| canonical_unity_version(&row.observation.unity_version).ok())
            .collect();
        if let Some(next) = page.next_cursor.clone() {
            self.visit_installations(access, Some(next), |row| {
                if let Ok(version) = canonical_unity_version(&row.observation.unity_version) {
                    versions.insert(version);
                }
            })
            .await?;
        }
        let default_version = if versions.len() == 1 {
            versions.first().cloned()
        } else {
            None
        };
        Ok((
            page,
            Some(UnityVersionOptions {
                versions: versions.into_iter().collect(),
                default_version,
            }),
        ))
    }

    async fn visit_installations(
        &self,
        access: &AccessContext,
        mut cursor: Option<UnityInstallationCursor>,
        mut visit: impl FnMut(UnityInstallationRecord) + Send,
    ) -> Result<(), M5UnityError> {
        loop {
            let page = self
                .store
                .list_installations(access.principal().clone(), cursor, 1_000)
                .await?;
            page.installations.into_iter().for_each(&mut visit);
            cursor = page.next_cursor;
            if cursor.is_none() {
                return Ok(());
            }
        }
    }

    pub async fn get_installation(
        &self,
        access: &AccessContext,
        id: UnityInstallationId,
    ) -> Result<UnityInstallationRecord, M5UnityError> {
        require(access, Permission::UnityRead)?;
        self.store
            .get_installation(access.principal().clone(), id)
            .await
    }

    pub async fn register_installation(
        &self,
        access: &AccessContext,
        executable_path: String,
        key: IdempotencyKey,
    ) -> Result<(UnityInstallationRecord, bool), M5UnityError> {
        require(access, Permission::UnityManage)?;
        let observation = crate::log_execution_step(
            "unity.installations.register",
            "check_editor",
            Some(crate::log_path("unity", executable_path.clone())),
            self.platform
                .validate_installation(executable_path, UnitySourceKind::Manual),
            |error| error.path().map(str::to_owned),
        )
        .await?;
        self.store
            .register_installation(access.principal().clone(), observation, key, now_ms()?)
            .await
    }

    pub async fn remove_installation(
        &self,
        access: &AccessContext,
        id: UnityInstallationId,
        expected: Revision,
        key: IdempotencyKey,
    ) -> Result<(bool, bool), M5UnityError> {
        require(access, Permission::UnityManage)?;
        self.store
            .remove_installation(access.principal().clone(), id, expected, key, now_ms()?)
            .await
    }

    pub async fn refresh_installations(
        &self,
        access: &AccessContext,
        key: IdempotencyKey,
    ) -> Result<(UnityInstallationPage, bool), M5UnityError> {
        require(access, Permission::UnityManage)?;
        let observations = crate::log_execution_step(
            "unity.installations.refresh",
            "find_editors",
            None,
            self.platform.discover_installations(),
            |error| error.path().map(str::to_owned),
        )
        .await?;
        self.store
            .synchronize_installations(access.principal().clone(), observations, key, now_ms()?)
            .await
    }

    pub async fn get_project_launch_config(
        &self,
        access: &AccessContext,
        project_id: ProjectId,
    ) -> Result<ProjectUnityLaunchConfigView, M5UnityError> {
        require(access, Permission::ProjectsRead)?;
        require(access, Permission::UnityRead)?;
        let config = self
            .store
            .get_project_launch_config(access.principal().clone(), project_id)
            .await?;
        let default_arguments = self.defaults.arguments().await?;
        let effective_arguments = if config.revision.is_some() {
            config.arguments.clone()
        } else {
            default_arguments.clone()
        };
        Ok(ProjectUnityLaunchConfigView {
            config,
            default_arguments,
            effective_arguments,
        })
    }

    pub async fn set_project_launch_config(
        &self,
        access: &AccessContext,
        project_id: ProjectId,
        arguments: Vec<String>,
        expected: Option<Revision>,
        key: IdempotencyKey,
    ) -> Result<(ProjectUnityLaunchConfig, bool, bool), M5UnityError> {
        require(access, Permission::ProjectsRead)?;
        require(access, Permission::UnityManage)?;
        validate_arguments(&arguments)?;
        self.store
            .set_project_launch_config(
                access.principal().clone(),
                project_id,
                arguments,
                expected,
                key,
                now_ms()?,
            )
            .await
    }

    pub async fn clear_project_launch_config(
        &self,
        access: &AccessContext,
        project_id: ProjectId,
        expected: Option<Revision>,
        key: IdempotencyKey,
    ) -> Result<(ProjectUnityLaunchConfig, bool, bool), M5UnityError> {
        require(access, Permission::ProjectsRead)?;
        require(access, Permission::UnityManage)?;
        self.store
            .clear_project_launch_config(
                access.principal().clone(),
                project_id,
                expected,
                key,
                now_ms()?,
            )
            .await
    }

    pub async fn writer_state(
        &self,
        access: &AccessContext,
        project_id: ProjectId,
    ) -> Result<UnityWriterState, M5UnityError> {
        require(access, Permission::UnityRead)?;
        self.writer_state_unchecked(access, project_id).await
    }

    pub(crate) async fn writer_state_unchecked(
        &self,
        access: &AccessContext,
        project_id: ProjectId,
    ) -> Result<UnityWriterState, M5UnityError> {
        let project = self
            .store
            .get_project(access.principal().clone(), project_id)
            .await
            .map_err(project_read_error)?;
        let installations = self
            .store
            .list_installations(access.principal().clone(), None, 1_000)
            .await?;
        let processes = match self.platform.observe_processes().await {
            Ok(value) => value,
            Err(_) => {
                return unknown_writer(project_id);
            }
        };
        classify_writer(
            &self.platform,
            project,
            &installations.installations,
            processes,
        )
        .await
    }

    pub async fn launch_options(
        &self,
        access: &AccessContext,
        project_id: ProjectId,
        expected_project_revision: Revision,
        include_migration_version_options: bool,
    ) -> Result<UnityLaunchOptions, M5UnityError> {
        require(access, Permission::ProjectsRead)?;
        require(access, Permission::UnityRead)?;
        require(access, Permission::UnityLaunch)?;
        let project = self
            .store
            .get_project(access.principal().clone(), project_id)
            .await
            .map_err(project_read_error)?;
        if project.revision != expected_project_revision {
            return Err(M5UnityError::new(M5UnityErrorCode::RevisionConflict));
        }
        let canonical = canonical_unity_version(&project.observation.unity_version)?;
        let mut matches = Vec::new();
        let mut migration_groups = BTreeMap::<String, Vec<UnityInstallationRecord>>::new();
        self.visit_installations(access, None, |installation| {
            let Ok(version) = canonical_unity_version(&installation.observation.unity_version)
            else {
                return;
            };
            if version == canonical {
                matches.push(installation);
            } else if include_migration_version_options {
                migration_groups
                    .entry(version)
                    .or_default()
                    .push(installation);
            }
        })
        .await?;
        order_installation_options(&mut matches);
        let migration_version_options = include_migration_version_options.then(|| {
            migration_groups
                .into_iter()
                .map(|(unity_version, mut group)| {
                    order_installation_options(&mut group);
                    let default_installation_id =
                        (group.len() == 1).then(|| group[0].installation_id);
                    UnityMigrationVersionOption {
                        unity_version,
                        default_installation_id,
                        installations: group
                            .into_iter()
                            .map(|row| UnityMigrationInstallationOption {
                                installation_id: row.installation_id,
                                architecture: row.observation.architecture,
                            })
                            .collect(),
                    }
                })
                .collect()
        });
        Ok(UnityLaunchOptions {
            project_id,
            project_revision: project.revision,
            project_unity_version: canonical,
            exact_matching_installations: matches,
            migration_version_options,
        })
    }

    pub async fn launch(
        &self,
        access: &AccessContext,
        project_id: ProjectId,
        installation_id: UnityInstallationId,
        expected_project_revision: Revision,
        key: IdempotencyKey,
    ) -> Result<(UnityLaunchRecord, bool), M5UnityError> {
        require(access, Permission::UnityLaunch)?;
        let project = self
            .store
            .get_project(access.principal().clone(), project_id)
            .await
            .map_err(project_read_error)?;
        if project.revision != expected_project_revision {
            return Err(M5UnityError::new(M5UnityErrorCode::RevisionConflict));
        }
        let mut config = self
            .store
            .get_project_launch_config(access.principal().clone(), project_id)
            .await?;
        if config.revision.is_none() {
            // This transient copy binds the accepted launch to its effective argv;
            // the stored project override remains absent.
            config.arguments = self.defaults.arguments().await?;
        }
        validate_arguments(&config.arguments)?;
        if let Some(record) = self
            .store
            .replay_launch(
                access.principal().clone(),
                project.clone(),
                config.clone(),
                installation_id,
                key.clone(),
            )
            .await?
        {
            return if record.state == UnityLaunchState::Failed {
                Err(M5UnityError::new(M5UnityErrorCode::LaunchFailed))
            } else {
                Ok((record, true))
            };
        }
        let installation = self
            .store
            .get_installation(access.principal().clone(), installation_id)
            .await?;
        let project_version = canonical_unity_version(&project.observation.unity_version)?;
        if !exact_unity_version(&project_version, &installation.observation.unity_version) {
            return Err(M5UnityError::new(M5UnityErrorCode::VersionMismatch));
        }
        let writer = self.writer_state_unchecked(access, project_id).await?;
        match writer.state {
            UnityWriterStateKind::RunningConfirmed => {
                return Err(M5UnityError::new(M5UnityErrorCode::ProjectRunning));
            }
            UnityWriterStateKind::RunningSuspected => {
                return Err(M5UnityError::new(M5UnityErrorCode::LaunchStateUncertain));
            }
            // An unattributed observation cannot prohibit opening every project.
            UnityWriterStateKind::NotObserved | UnityWriterStateKind::Unknown => {}
        }
        let (record, replayed) = self
            .store
            .accept_launch(
                access.principal().clone(),
                project.clone(),
                config.clone(),
                installation_id,
                key,
                now_ms()?,
            )
            .await?;
        if replayed && record.state == UnityLaunchState::Failed {
            return Err(M5UnityError::new(M5UnityErrorCode::LaunchFailed));
        }
        if replayed && record.spawn_accepted {
            return Ok((record, true));
        }
        let executable_path = installation.observation.executable_path;
        let project_path = project.observation.root_path;
        if let Err(error) = crate::log_execution_step(
            "unity.launch",
            "launch_requested",
            Some(crate::log_path("project", project_path.clone())),
            self.platform.launch(
                executable_path.clone(),
                project_path.clone(),
                config.arguments,
            ),
            |error| error.path().map(str::to_owned),
        )
        .await
        {
            self.store
                .set_launch_state(
                    access.principal().clone(),
                    record.launch_id,
                    UnityLaunchState::Failed,
                    false,
                )
                .await?;
            return Err(error);
        }
        crate::flush_log_steps().await;
        let accepted = self
            .store
            .set_launch_state(
                access.principal().clone(),
                record.launch_id,
                UnityLaunchState::Opening,
                true,
            )
            .await?;
        Ok((accepted, replayed))
    }
}

fn unknown_writer(project_id: ProjectId) -> Result<UnityWriterState, M5UnityError> {
    Ok(UnityWriterState {
        project_id,
        state: UnityWriterStateKind::Unknown,
        evidence: vec![UnityWriterEvidenceKind::InspectionError],
        checked_at_ms: now_ms()?,
    })
}

async fn classify_writer<P: M5UnityPlatform>(
    platform: &P,
    project: ProjectRecord,
    installations: &[UnityInstallationRecord],
    processes: Vec<UnityProcessObservation>,
) -> Result<UnityWriterState, M5UnityError> {
    let mut suspected = false;
    let mut unresolved = false;
    for process in processes {
        if process.pid == 0 || process.start_time == 0 {
            continue;
        }
        let unity_by_name = process.name.eq_ignore_ascii_case("unity")
            || process.name.eq_ignore_ascii_case("unity.exe");
        let known_executable = process
            .executable_identity
            .as_ref()
            .is_some_and(|identity| {
                installations
                    .iter()
                    .any(|installation| installation.observation.filesystem_identity == *identity)
            });
        if !unity_by_name && !known_executable {
            continue;
        }
        if !known_executable && process.verified_non_editor && process.executable_identity.is_some()
        {
            continue;
        }
        let Some(arguments) = process.arguments else {
            unresolved = true;
            continue;
        };
        let Some(selector) = project_selector(&arguments) else {
            unresolved = true;
            continue;
        };
        match platform.path_identity(selector.to_owned()).await {
            Ok(identity)
                if identity == project.observation.path_identity_key && !known_executable =>
            {
                suspected = true;
            }
            Ok(identity) if identity == project.observation.path_identity_key => {
                return Ok(UnityWriterState {
                    project_id: project.project_id,
                    state: UnityWriterStateKind::RunningConfirmed,
                    evidence: vec![UnityWriterEvidenceKind::ProcessProjectArgument],
                    checked_at_ms: now_ms()?,
                });
            }
            Ok(_) => {}
            Err(_) => unresolved = true,
        }
    }
    Ok(UnityWriterState {
        project_id: project.project_id,
        state: if suspected {
            UnityWriterStateKind::RunningSuspected
        } else if unresolved {
            UnityWriterStateKind::Unknown
        } else {
            UnityWriterStateKind::NotObserved
        },
        evidence: if suspected {
            vec![UnityWriterEvidenceKind::ProcessProjectArgument]
        } else if unresolved {
            vec![UnityWriterEvidenceKind::ProcessUnreadable]
        } else {
            Vec::new()
        },
        checked_at_ms: now_ms()?,
    })
}

fn project_selector(arguments: &[String]) -> Option<&str> {
    arguments.windows(2).find_map(|pair| {
        pair[0]
            .eq_ignore_ascii_case("-projectPath")
            .then_some(pair[1].as_str())
    })
}

async fn confirm_session_processes<P: M5UnityPlatform>(
    platform: &P,
    installations: &BTreeSet<Vec<u8>>,
    processes: Vec<UnityProcessObservation>,
) -> Result<BTreeMap<(u64, u32), Vec<u8>>, M5UnityError> {
    let mut confirmed = BTreeMap::new();
    for process in processes {
        let key = (process.start_time, process.pid);
        if process.pid == 0 || process.start_time == 0 || process.verified_non_editor {
            continue;
        }
        let verified_editor = process
            .executable_identity
            .as_ref()
            .is_some_and(|executable| installations.contains(executable));
        if !verified_editor || confirmed.contains_key(&key) {
            continue;
        }
        let arguments = process
            .arguments
            .ok_or_else(|| M5UnityError::new(M5UnityErrorCode::Internal))?;
        if arguments
            .iter()
            .any(|argument| argument.eq_ignore_ascii_case("-batchmode"))
        {
            continue;
        }
        if let Some(project_path) = project_selector(&arguments) {
            let identity = platform
                .session_project_identity(project_path.to_owned())
                .await?;
            confirmed.insert(key, identity);
        }
    }
    Ok(confirmed)
}

fn project_unity_sessions(
    confirmed: BTreeMap<(u64, u32), Vec<u8>>,
    projects: &BTreeMap<Vec<u8>, ProjectId>,
    foreground: Option<u32>,
) -> Result<UnitySessionObservation, M5UnityError> {
    let mut latest = None;
    let mut foreground_session = None;
    let mut editor_count = 0;
    for ((started_at_secs, pid), identity) in confirmed.into_iter().rev() {
        let Some(project_id) = projects.get(&identity) else {
            continue;
        };
        let started_at_ms = started_at_secs
            .checked_mul(1_000)
            .ok_or_else(|| M5UnityError::new(M5UnityErrorCode::Internal))?;
        let session = UnitySession {
            project_id: *project_id,
            started_at_ms,
        };
        editor_count += 1;
        latest.get_or_insert(session);
        if foreground == Some(pid) {
            foreground_session.get_or_insert(session);
        }
    }
    Ok(UnitySessionObservation {
        current_session: foreground_session.or(latest),
        editor_count,
    })
}

pub(crate) fn validate_arguments(arguments: &[String]) -> Result<(), M5UnityError> {
    let encoded_size = json_arguments_size(arguments)
        .ok_or_else(|| M5UnityError::new(M5UnityErrorCode::InvalidInput))?;
    if arguments.iter().any(|argument| {
        let name = argument
            .split_once('=')
            .map_or(argument.as_str(), |(name, _)| name);
        ["-projectPath", "-openfile", "-createProject"]
            .iter()
            .any(|selector| name.eq_ignore_ascii_case(selector))
    }) {
        return Err(M5UnityError::new(
            M5UnityErrorCode::ProjectSelectorForbidden,
        ));
    }
    if arguments.len() > 64
        || encoded_size > 65_536
        || arguments
            .iter()
            .any(|argument| argument.len() > 4_096 || argument.contains('\0'))
    {
        return Err(M5UnityError::new(M5UnityErrorCode::InvalidInput));
    }
    Ok(())
}

fn json_arguments_size(arguments: &[String]) -> Option<usize> {
    let mut size = 2_usize.checked_add(arguments.len().saturating_sub(1))?;
    for argument in arguments {
        size = size.checked_add(2)?;
        for character in argument.chars() {
            let encoded = match character {
                '"' | '\\' | '\u{0008}' | '\u{0009}' | '\n' | '\u{000c}' | '\r' => 2,
                '\u{0000}'..='\u{001f}' => 6,
                value => value.len_utf8(),
            };
            size = size.checked_add(encoded)?;
        }
    }
    Some(size)
}

/// Validates the exact canonical Editor version used by registration and launch.
pub fn canonical_unity_version(value: &str) -> Result<String, M5UnityError> {
    if value.len() < 8 || value.len() > 64 || !value.is_ascii() {
        return Err(M5UnityError::new(M5UnityErrorCode::VersionUnverified));
    }
    let bytes = value.as_bytes();
    let mut index = 0;
    for component in 0..3 {
        index = parse_decimal(bytes, index, true)?;
        if component < 2 {
            if index >= bytes.len() || bytes[index] != b'.' {
                return Err(M5UnityError::new(M5UnityErrorCode::VersionUnverified));
            }
            index += 1;
        } else if !bytes
            .get(index)
            .is_some_and(|value| matches!(value, b'a' | b'b' | b'f' | b'p' | b'x'))
        {
            return Err(M5UnityError::new(M5UnityErrorCode::VersionUnverified));
        }
    }
    if index >= bytes.len() || !matches!(bytes[index], b'a' | b'b' | b'f' | b'p' | b'x') {
        return Err(M5UnityError::new(M5UnityErrorCode::VersionUnverified));
    }
    index += 1;
    index = parse_decimal(bytes, index, false)?;
    if index < bytes.len() {
        if bytes[index] != b'c' {
            return Err(M5UnityError::new(M5UnityErrorCode::VersionUnverified));
        }
        index = parse_decimal(bytes, index + 1, false)?;
    }
    if index != bytes.len() {
        return Err(M5UnityError::new(M5UnityErrorCode::VersionUnverified));
    }
    Ok(value.to_owned())
}

fn parse_decimal(bytes: &[u8], start: usize, allow_zero: bool) -> Result<usize, M5UnityError> {
    let Some(first) = bytes.get(start).copied() else {
        return Err(M5UnityError::new(M5UnityErrorCode::VersionUnverified));
    };
    if !first.is_ascii_digit() || (!allow_zero && first == b'0') {
        return Err(M5UnityError::new(M5UnityErrorCode::VersionUnverified));
    }
    let mut end = start + 1;
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
    }
    if first == b'0' && end > start + 1 {
        return Err(M5UnityError::new(M5UnityErrorCode::VersionUnverified));
    }
    Ok(end)
}

fn exact_unity_version(project: &str, installation: &str) -> bool {
    canonical_unity_version(installation).is_ok_and(|value| value == project)
}

fn order_installation_options(installations: &mut [UnityInstallationRecord]) {
    installations.sort_by(|left, right| {
        left.observation
            .unity_version
            .as_bytes()
            .cmp(right.observation.unity_version.as_bytes())
            .then_with(|| {
                source_order(left.observation.source_kind)
                    .cmp(&source_order(right.observation.source_kind))
            })
            .then_with(|| {
                left.installation_id
                    .to_string()
                    .cmp(&right.installation_id.to_string())
            })
    });
}

const fn source_order(source: UnitySourceKind) -> u8 {
    match source {
        UnitySourceKind::Manual => 0,
        UnitySourceKind::HubConfig => 1,
        UnitySourceKind::UnityCliHint => 2,
        UnitySourceKind::KnownInstallRoot => 3,
    }
}

fn project_read_error(error: crate::M3Error) -> M5UnityError {
    M5UnityError::new(match error.code() {
        crate::M3ErrorCode::PermissionDenied => M5UnityErrorCode::PermissionDenied,
        crate::M3ErrorCode::ProjectNotRegistered => M5UnityErrorCode::ProjectNotRegistered,
        crate::M3ErrorCode::StoreUnavailable => M5UnityErrorCode::StoreUnavailable,
        _ => M5UnityErrorCode::Internal,
    })
}

fn require(access: &AccessContext, permission: Permission) -> Result<(), M5UnityError> {
    access
        .require(permission)
        .map_err(|_| M5UnityError::new(M5UnityErrorCode::PermissionDenied))
}

fn now_ms() -> Result<u64, M5UnityError> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| M5UnityError::new(M5UnityErrorCode::Internal))
        .and_then(|duration| {
            u64::try_from(duration.as_millis())
                .map_err(|_| M5UnityError::new(M5UnityErrorCode::Internal))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    struct FakePlatform {
        fail_identity: bool,
    }

    impl M5UnityPlatform for FakePlatform {
        async fn validate_installation(
            &self,
            _: String,
            _: UnitySourceKind,
        ) -> Result<UnityInstallationObservation, M5UnityError> {
            Err(M5UnityError::new(M5UnityErrorCode::Internal))
        }

        async fn discover_installations(
            &self,
        ) -> Result<Vec<UnityInstallationObservation>, M5UnityError> {
            Ok(Vec::new())
        }

        async fn observe_processes(&self) -> Result<Vec<UnityProcessObservation>, M5UnityError> {
            Ok(Vec::new())
        }

        async fn path_identity(&self, _: String) -> Result<Vec<u8>, M5UnityError> {
            if self.fail_identity {
                Err(M5UnityError::new(M5UnityErrorCode::Internal))
            } else {
                Ok(vec![7])
            }
        }

        async fn launch(&self, _: String, _: String, _: Vec<String>) -> Result<(), M5UnityError> {
            Ok(())
        }
    }

    #[test]
    fn session_projection_prefers_a_confirmed_foreground_editor() {
        let foreground_project = ProjectId::new();
        let latest_project = ProjectId::new();
        let projects = BTreeMap::from([(vec![1], foreground_project), (vec![2], latest_project)]);
        let processes = BTreeMap::from([
            ((10, 100), vec![1]),
            ((20, 200), vec![2]),
            ((30, 300), vec![9]),
            ((15, 150), vec![1]),
        ]);

        let foreground = project_unity_sessions(processes.clone(), &projects, Some(100))
            .expect("confirmed foreground");
        assert_eq!(foreground.editor_count, 3);
        assert_eq!(
            foreground.current_session,
            Some(UnitySession {
                project_id: foreground_project,
                started_at_ms: 10_000
            }),
        );

        let fallback = project_unity_sessions(processes, &projects, Some(300))
            .expect("unregistered foreground does not affect selection");
        assert_eq!(fallback.editor_count, 3);
        assert_eq!(
            fallback.current_session,
            Some(UnitySession {
                project_id: latest_project,
                started_at_ms: 20_000
            }),
        );
    }

    #[test]
    fn session_projection_distinguishes_empty_observation_from_invalid_evidence() {
        let projects = BTreeMap::from([(vec![1], ProjectId::new())]);
        let empty = project_unity_sessions(BTreeMap::from([((1, 1), vec![2])]), &projects, None)
            .expect("unregistered editor");
        assert_eq!(
            empty,
            UnitySessionObservation {
                current_session: None,
                editor_count: 0
            }
        );

        let invalid =
            project_unity_sessions(BTreeMap::from([((u64::MAX, 1), vec![1])]), &projects, None);
        assert_eq!(
            invalid.expect_err("timestamp overflow").code(),
            M5UnityErrorCode::Internal
        );
    }

    #[tokio::test]
    async fn session_sampling_filters_batch_helpers_and_unverified_editors() {
        let known_installation = vec![1];
        let editor = UnityProcessObservation {
            pid: 1,
            start_time: 2,
            name: "Unity".to_owned(),
            executable_identity: Some(known_installation.clone()),
            verified_non_editor: false,
            arguments: Some(vec!["-projectPath".to_owned(), "X:/project".to_owned()]),
        };
        let mut batch = UnityProcessObservation {
            pid: 2,
            ..editor.clone()
        };
        batch
            .arguments
            .as_mut()
            .expect("arguments")
            .push("-batchmode".to_owned());
        let helper = UnityProcessObservation {
            pid: 3,
            verified_non_editor: true,
            ..editor.clone()
        };
        let unverified = UnityProcessObservation {
            pid: 4,
            executable_identity: Some(vec![5]),
            ..editor.clone()
        };
        let platform = FakePlatform {
            fail_identity: false,
        };
        let installations = BTreeSet::from([known_installation]);
        let confirmed = confirm_session_processes(
            &platform,
            &installations,
            vec![editor.clone(), editor.clone(), batch, helper, unverified],
        )
        .await
        .expect("confirmed process");
        assert_eq!(confirmed, BTreeMap::from([((2, 1), vec![7])]));

        let unreadable = UnityProcessObservation {
            arguments: None,
            ..editor
        };
        assert!(
            confirm_session_processes(&platform, &installations, vec![unreadable])
                .await
                .is_err()
        );
    }

    #[test]
    fn project_selector_is_independent_and_user_override_is_rejected() {
        let arguments = vec![
            "-batchmode".to_owned(),
            "-projectPath".to_owned(),
            "X:/project".to_owned(),
        ];
        assert_eq!(project_selector(&arguments), Some("X:/project"));
        assert_eq!(
            validate_arguments(&arguments)
                .expect_err("selector override")
                .code(),
            M5UnityErrorCode::ProjectSelectorForbidden
        );
        assert_eq!(
            validate_arguments(&["-PROJECTPATH=X:/other".to_owned()])
                .expect_err("inline selector override")
                .code(),
            M5UnityErrorCode::ProjectSelectorForbidden
        );
        assert!(validate_arguments(&vec!["x".repeat(4_096); 17]).is_err());
        for selector in [
            "-openfile",
            "-OPENFILE=Scene.unity",
            "-createProject",
            "-CREATEPROJECT=X:/other",
        ] {
            assert_eq!(
                validate_arguments(&[selector.to_owned()])
                    .expect_err("alternate project selector")
                    .code(),
                M5UnityErrorCode::ProjectSelectorForbidden
            );
        }
        assert_eq!(
            validate_arguments(&["embedded\0nul".to_owned()])
                .expect_err("NUL cannot be passed to an OS process")
                .code(),
            M5UnityErrorCode::InvalidInput
        );
        assert!(
            validate_arguments(&[String::new(), "literal \"quotes\" \\ spaces ".to_owned()])
                .is_ok()
        );
    }

    #[test]
    fn unity_launch_uses_canonical_exact_versions() {
        assert!(exact_unity_version("2022.3.22f1", "2022.3.22f1"));
        assert!(exact_unity_version("2022.3.22f1c1", "2022.3.22f1c1"));
        assert!(!exact_unity_version("2022.3.22f1", "2022.3.40f1"));
        assert!(!exact_unity_version("2022.3.22f1", "2022.3.22f1c1"));
        for invalid in [
            "2022.3",
            "2022.3.22",
            "2022.3.22f0",
            "2022.3.22f1c0",
            "02022.3.22f1",
        ] {
            assert_eq!(
                canonical_unity_version(invalid)
                    .expect_err("invalid Unity version")
                    .code(),
                M5UnityErrorCode::VersionUnverified
            );
        }
    }

    #[tokio::test]
    async fn fake_process_evidence_preserves_all_four_writer_states() {
        let project = project();
        let installation = installation();
        let confirmed = classify_writer(
            &FakePlatform {
                fail_identity: false,
            },
            project.clone(),
            std::slice::from_ref(&installation),
            vec![process(Some(vec![
                "Unity".to_owned(),
                "-projectPath".to_owned(),
                "fixture-project".to_owned(),
            ]))],
        )
        .await
        .expect("confirmed state");
        assert_eq!(confirmed.state, UnityWriterStateKind::RunningConfirmed);

        let mut unregistered = process(Some(vec![
            "Unity".to_owned(),
            "-projectPath".to_owned(),
            "fixture-project".to_owned(),
        ]));
        unregistered.executable_identity = Some(vec![10]);
        let suspected = classify_writer(
            &FakePlatform {
                fail_identity: false,
            },
            project.clone(),
            std::slice::from_ref(&installation),
            vec![unregistered],
        )
        .await
        .expect("partial state");
        assert_eq!(suspected.state, UnityWriterStateKind::RunningSuspected);

        let denied = classify_writer(
            &FakePlatform {
                fail_identity: true,
            },
            project.clone(),
            &[installation],
            vec![process(Some(vec![
                "Unity".to_owned(),
                "-projectPath".to_owned(),
                "fixture-project".to_owned(),
            ]))],
        )
        .await
        .expect("permission denied state");
        assert_eq!(denied.state, UnityWriterStateKind::Unknown);

        let absent = classify_writer(
            &FakePlatform {
                fail_identity: false,
            },
            project.clone(),
            &[],
            Vec::new(),
        )
        .await
        .expect("complete empty state");
        assert_eq!(absent.state, UnityWriterStateKind::NotObserved);
        assert_eq!(
            unknown_writer(project.project_id)
                .expect("unknown state")
                .state,
            UnityWriterStateKind::Unknown
        );
    }

    #[tokio::test]
    async fn verified_non_editor_does_not_block_another_project() {
        let platform = FakePlatform {
            fail_identity: false,
        };
        let mut project = project();
        project.observation.path_identity_key = vec![8];
        let mut helper = process(Some(vec!["unity.exe".to_owned(), "serve".to_owned()]));
        helper.executable_identity = Some(vec![10]);
        helper.verified_non_editor = true;
        let other_editor = process(Some(vec![
            "Unity.exe".to_owned(),
            "-projectPath".to_owned(),
            "other-project".to_owned(),
        ]));
        for processes in [vec![helper.clone()], vec![other_editor, helper]] {
            let state = classify_writer(&platform, project.clone(), &[installation()], processes)
                .await
                .expect("read-only writer observation");
            assert_eq!(state.state, UnityWriterStateKind::NotObserved);
            assert!(state.evidence.is_empty());
        }
    }

    #[tokio::test]
    async fn unattributed_same_name_remains_unknown_without_claiming_project_usage() {
        let mut helper = process(Some(vec!["unity.exe".to_owned(), "serve".to_owned()]));
        helper.executable_identity = Some(vec![10]);
        let mut unreadable_identity = helper.clone();
        unreadable_identity.executable_identity = None;
        unreadable_identity.verified_non_editor = true;
        for process in [helper, unreadable_identity] {
            let state = classify_writer(
                &FakePlatform {
                    fail_identity: false,
                },
                project(),
                &[installation()],
                vec![process],
            )
            .await
            .expect("uncertain writer observation");
            assert_eq!(state.state, UnityWriterStateKind::Unknown);
        }
    }

    #[tokio::test]
    async fn unregistered_editor_for_another_project_does_not_claim_this_project() {
        let mut project = project();
        project.observation.path_identity_key = vec![8];
        let mut editor = process(Some(vec![
            "Unity.exe".to_owned(),
            "-projectPath".to_owned(),
            "other-project".to_owned(),
        ]));
        editor.executable_identity = None;
        let state = classify_writer(
            &FakePlatform {
                fail_identity: false,
            },
            project,
            &[],
            vec![editor],
        )
        .await
        .expect("other project observation");
        assert_eq!(state.state, UnityWriterStateKind::NotObserved);
    }

    #[tokio::test]
    async fn matching_registered_editor_is_confirmed_despite_non_editor_helper() {
        let mut helper = process(None);
        helper.executable_identity = Some(vec![10]);
        helper.verified_non_editor = true;
        let mut editor = process(Some(vec![
            "Unity.exe".to_owned(),
            "-projectPath".to_owned(),
            "fixture-project".to_owned(),
        ]));
        // A registered executable match remains authoritative even if metadata
        // evidence supplied by an adapter were contradictory.
        editor.verified_non_editor = true;
        for processes in [vec![helper.clone(), editor.clone()], vec![editor, helper]] {
            let state = classify_writer(
                &FakePlatform {
                    fail_identity: false,
                },
                project(),
                &[installation()],
                processes,
            )
            .await
            .expect("confirmed Editor observation");
            assert_eq!(state.state, UnityWriterStateKind::RunningConfirmed);
        }
    }

    fn project() -> ProjectRecord {
        ProjectRecord {
            project_id: ProjectId::new(),
            observation: crate::ProjectObservation {
                root_path: "fixture-project".to_owned(),
                path_identity_key: vec![7],
                project_type: crate::ProjectType::Unknown,
                unity_version: "2022.3.22f1".to_owned(),
                unity_revision: None,
                vpm_manifest: crate::ManifestState::Missing,
                upm_manifest: crate::ManifestState::Missing,
                direct_dependencies: Vec::new(),
                locked_dependencies: Vec::new(),
                issues: Vec::new(),
            },
            revision: Revision::INITIAL,
            registered_at_ms: 1,
            last_modified_at_ms: None,
            favorite: false,
        }
    }

    fn installation() -> UnityInstallationRecord {
        UnityInstallationRecord {
            installation_id: UnityInstallationId::new(),
            observation: UnityInstallationObservation {
                executable_path: "fixture-unity".to_owned(),
                filesystem_identity: vec![9],
                unity_version: "2022.3.40f1".to_owned(),
                architecture: UnityArchitecture::Unknown,
                source_kind: UnitySourceKind::Manual,
                observed_at_ms: 1,
            },
            revision: Revision::INITIAL,
            updated_at_ms: 1,
        }
    }

    fn process(arguments: Option<Vec<String>>) -> UnityProcessObservation {
        UnityProcessObservation {
            pid: 7,
            start_time: 8,
            name: "Unity".to_owned(),
            executable_identity: Some(vec![9]),
            verified_non_editor: false,
            arguments,
        }
    }
}
