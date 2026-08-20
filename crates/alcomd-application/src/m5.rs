//! M5 Unity installation, writer observation and launch use cases.

use std::future::Future;

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

/// Project-selected Editor and safe argv array.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectEditorPreference {
    pub project_id: ProjectId,
    pub installation_id: UnityInstallationId,
    pub arguments: Vec<String>,
    pub revision: Revision,
    pub updated_at_ms: u64,
}

/// One short-lived, non-persisted process observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnityProcessObservation {
    pub pid: u32,
    pub start_time: u64,
    pub name: String,
    pub executable_identity: Option<Vec<u8>>,
    pub arguments: Option<Vec<String>>,
}

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

/// Public launch observation state.
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct M5UnityError {
    code: M5UnityErrorCode,
}

impl M5UnityError {
    #[must_use]
    pub const fn new(code: M5UnityErrorCode) -> Self {
        Self { code }
    }

    #[must_use]
    pub const fn code(self) -> M5UnityErrorCode {
        self.code
    }
}

impl std::fmt::Display for M5UnityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Unity request failed")
    }
}

impl std::error::Error for M5UnityError {}

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
    fn get_project_editor(
        &self,
        owner: PrincipalId,
        project_id: ProjectId,
    ) -> impl Future<Output = Result<ProjectEditorPreference, M5UnityError>> + Send;
    #[allow(clippy::too_many_arguments)]
    fn set_project_editor(
        &self,
        owner: PrincipalId,
        project_id: ProjectId,
        installation_id: UnityInstallationId,
        arguments: Vec<String>,
        expected: Option<Revision>,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<(ProjectEditorPreference, bool), M5UnityError>> + Send;
    fn accept_launch(
        &self,
        owner: PrincipalId,
        project: ProjectRecord,
        preference: ProjectEditorPreference,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<(UnityLaunchRecord, bool), M5UnityError>> + Send;
    fn replay_launch(
        &self,
        owner: PrincipalId,
        project: ProjectRecord,
        preference: ProjectEditorPreference,
        key: IdempotencyKey,
    ) -> impl Future<Output = Result<Option<UnityLaunchRecord>, M5UnityError>> + Send;
    fn get_launch(
        &self,
        owner: PrincipalId,
        launch_id: UnityLaunchId,
    ) -> impl Future<Output = Result<UnityLaunchRecord, M5UnityError>> + Send;
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
    fn path_identity(
        &self,
        path: String,
    ) -> impl Future<Output = Result<Vec<u8>, M5UnityError>> + Send;
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
}

impl<S: M5UnityStore + M3RegistryStore, P: M5UnityPlatform> M5UnityApplication<S, P> {
    #[must_use]
    pub const fn new(store: S, platform: P) -> Self {
        Self { store, platform }
    }

    pub async fn list_installations(
        &self,
        access: &AccessContext,
        cursor: Option<UnityInstallationCursor>,
        limit: u32,
    ) -> Result<UnityInstallationPage, M5UnityError> {
        require(access, Permission::UnityRead)?;
        if !(1..=1_000).contains(&limit) {
            return Err(M5UnityError::new(M5UnityErrorCode::InvalidInput));
        }
        self.store
            .list_installations(access.principal().clone(), cursor, limit)
            .await
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
        let observation = self
            .platform
            .validate_installation(executable_path, UnitySourceKind::Manual)
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
        let observations = self.platform.discover_installations().await?;
        self.store
            .synchronize_installations(access.principal().clone(), observations, key, now_ms()?)
            .await
    }

    pub async fn get_project_editor(
        &self,
        access: &AccessContext,
        project_id: ProjectId,
    ) -> Result<ProjectEditorPreference, M5UnityError> {
        require(access, Permission::UnityRead)?;
        self.store
            .get_project_editor(access.principal().clone(), project_id)
            .await
    }

    pub async fn set_project_editor(
        &self,
        access: &AccessContext,
        project_id: ProjectId,
        installation_id: UnityInstallationId,
        arguments: Vec<String>,
        expected: Option<Revision>,
        key: IdempotencyKey,
    ) -> Result<(ProjectEditorPreference, bool), M5UnityError> {
        require(access, Permission::UnityManage)?;
        validate_arguments(&arguments)?;
        self.store
            .set_project_editor(
                access.principal().clone(),
                project_id,
                installation_id,
                arguments,
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

    async fn writer_state_unchecked(
        &self,
        access: &AccessContext,
        project_id: ProjectId,
    ) -> Result<UnityWriterState, M5UnityError> {
        let project = self
            .store
            .get_project(access.principal().clone(), project_id)
            .await
            .map_err(|_| M5UnityError::new(M5UnityErrorCode::ProjectNotRegistered))?;
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

    pub async fn launch(
        &self,
        access: &AccessContext,
        project_id: ProjectId,
        expected_project_revision: Revision,
        key: IdempotencyKey,
    ) -> Result<(UnityLaunchRecord, bool), M5UnityError> {
        require(access, Permission::UnityLaunch)?;
        let project = self
            .store
            .get_project(access.principal().clone(), project_id)
            .await
            .map_err(|_| M5UnityError::new(M5UnityErrorCode::ProjectNotRegistered))?;
        if project.revision != expected_project_revision {
            return Err(M5UnityError::new(M5UnityErrorCode::RevisionConflict));
        }
        let preference = self
            .store
            .get_project_editor(access.principal().clone(), project_id)
            .await?;
        let installation = self
            .store
            .get_installation(access.principal().clone(), preference.installation_id)
            .await?;
        if !compatible_unity_versions(
            &project.observation.unity_version,
            &installation.observation.unity_version,
        ) {
            return Err(M5UnityError::new(M5UnityErrorCode::VersionMismatch));
        }
        if let Some(record) = self
            .store
            .replay_launch(
                access.principal().clone(),
                project.clone(),
                preference.clone(),
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
        let writer = self.writer_state_unchecked(access, project_id).await?;
        match writer.state {
            UnityWriterStateKind::RunningConfirmed => {
                return Err(M5UnityError::new(M5UnityErrorCode::ProjectRunning));
            }
            UnityWriterStateKind::RunningSuspected | UnityWriterStateKind::Unknown => {
                return Err(M5UnityError::new(M5UnityErrorCode::LaunchStateUncertain));
            }
            UnityWriterStateKind::NotObserved => {}
        }
        let (record, replayed) = self
            .store
            .accept_launch(
                access.principal().clone(),
                project.clone(),
                preference.clone(),
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
        if self
            .platform
            .launch(
                installation.observation.executable_path,
                project.observation.root_path,
                preference.arguments,
            )
            .await
            .is_err()
        {
            self.store
                .set_launch_state(
                    access.principal().clone(),
                    record.launch_id,
                    UnityLaunchState::Failed,
                    false,
                )
                .await?;
            return Err(M5UnityError::new(M5UnityErrorCode::LaunchFailed));
        }
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

    pub async fn launch_status(
        &self,
        access: &AccessContext,
        launch_id: UnityLaunchId,
    ) -> Result<UnityLaunchRecord, M5UnityError> {
        require(access, Permission::UnityLaunch)?;
        let record = self
            .store
            .get_launch(access.principal().clone(), launch_id)
            .await?;
        if record.state == UnityLaunchState::Failed {
            return Ok(record);
        }
        let writer = self
            .writer_state_unchecked(access, record.project_id)
            .await?;
        let next = match writer.state {
            UnityWriterStateKind::RunningConfirmed => UnityLaunchState::Open,
            UnityWriterStateKind::NotObserved => UnityLaunchState::Failed,
            UnityWriterStateKind::RunningSuspected | UnityWriterStateKind::Unknown => {
                UnityLaunchState::Opening
            }
        };
        self.store
            .set_launch_state(
                access.principal().clone(),
                launch_id,
                next,
                record.spawn_accepted,
            )
            .await
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
        if !known_executable {
            suspected = true;
            continue;
        }
        let Some(arguments) = process.arguments else {
            suspected = true;
            continue;
        };
        let Some(selector) = project_selector(&arguments) else {
            suspected = true;
            continue;
        };
        match platform.path_identity(selector.to_owned()).await {
            Ok(identity) if identity == project.observation.path_identity_key => {
                return Ok(UnityWriterState {
                    project_id: project.project_id,
                    state: UnityWriterStateKind::RunningConfirmed,
                    evidence: vec![UnityWriterEvidenceKind::ProcessProjectArgument],
                    checked_at_ms: now_ms()?,
                });
            }
            Ok(_) => {}
            Err(_) => suspected = true,
        }
    }
    Ok(UnityWriterState {
        project_id: project.project_id,
        state: if suspected {
            UnityWriterStateKind::RunningSuspected
        } else {
            UnityWriterStateKind::NotObserved
        },
        evidence: suspected
            .then_some(UnityWriterEvidenceKind::ProcessUnreadable)
            .into_iter()
            .collect(),
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

fn validate_arguments(arguments: &[String]) -> Result<(), M5UnityError> {
    let encoded_size = json_arguments_size(arguments)
        .ok_or_else(|| M5UnityError::new(M5UnityErrorCode::InvalidInput))?;
    if arguments.iter().any(|argument| {
        argument.eq_ignore_ascii_case("-projectPath")
            || argument.to_ascii_lowercase().starts_with("-projectpath=")
    }) {
        return Err(M5UnityError::new(
            M5UnityErrorCode::ProjectSelectorForbidden,
        ));
    }
    if arguments.len() > 64
        || encoded_size > 65_536
        || arguments.iter().any(|argument| argument.len() > 4_096)
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

fn compatible_unity_versions(project: &str, installation: &str) -> bool {
    project
        .split('.')
        .take(2)
        .eq(installation.split('.').take(2))
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
    }

    #[test]
    fn unity_compatibility_uses_verified_major_minor() {
        assert!(compatible_unity_versions("2022.3.22f1", "2022.3.40f1"));
        assert!(!compatible_unity_versions("2019.4.31f1", "2022.3.40f1"));
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

        let suspected = classify_writer(
            &FakePlatform {
                fail_identity: false,
            },
            project.clone(),
            std::slice::from_ref(&installation),
            vec![process(None)],
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
        assert_eq!(denied.state, UnityWriterStateKind::RunningSuspected);

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
                observed_at_ms: 1,
            },
            revision: Revision::INITIAL,
            registered_at_ms: 1,
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
            arguments,
        }
    }
}
