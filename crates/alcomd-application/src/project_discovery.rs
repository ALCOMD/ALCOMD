//! Bounded default-directory discovery. Only the Core invokes this internal use case.

use std::sync::Arc;

use crate::{
    AccessContext, IdempotencyKey, M3ErrorCode, M3ReadAdapter, M3RegistryStore,
    M7OfficialApplication, PrincipalId, ProjectDiscoveryMode, ProjectId, ResourceKey,
    ResourceLockCoordinator, m3_time_ms,
};

/// Maximum directory entries observed during one default-root scan, including non-projects.
pub const PROJECT_DISCOVERY_ENTRY_LIMIT: usize = 4096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectDiscoveryRoot {
    pub path: String,
    pub identity: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectDiscoveryCandidate {
    pub path: String,
    pub identity: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectDiscoveryEntries {
    pub root: ProjectDiscoveryRoot,
    pub candidates: Vec<ProjectDiscoveryCandidate>,
    pub reached_limit: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProjectDiscoveryErrorKind {
    Configuration,
    Directory,
    Registration,
}

#[derive(Clone, Debug)]
pub struct ProjectDiscoveryError {
    kind: ProjectDiscoveryErrorKind,
    context: Option<crate::TechnicalErrorContext>,
}

impl PartialEq for ProjectDiscoveryError {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
    }
}
impl Eq for ProjectDiscoveryError {}

#[allow(non_upper_case_globals)]
impl ProjectDiscoveryError {
    pub const ConfigurationUnavailable: Self = Self {
        kind: ProjectDiscoveryErrorKind::Configuration,
        context: None,
    };
    pub const DirectoryUnavailable: Self = Self {
        kind: ProjectDiscoveryErrorKind::Directory,
        context: None,
    };
    pub const RegistrationUnavailable: Self = Self {
        kind: ProjectDiscoveryErrorKind::Registration,
        context: None,
    };

    #[must_use]
    pub fn with_source(mut self, source: impl std::error::Error + Send + Sync + 'static) -> Self {
        self.context = Some(self.context.take().unwrap_or_default().with_source(source));
        self
    }
    #[must_use]
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.context = Some(self.context.take().unwrap_or_default().with_path(path));
        self
    }
    #[must_use]
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.context = Some(self.context.take().unwrap_or_default().with_reason(reason));
        self
    }
    #[must_use]
    pub fn path(&self) -> Option<&str> {
        self.context
            .as_ref()
            .and_then(|context| context.path.as_deref())
    }
    #[must_use]
    pub fn diagnostic_id(&self) -> Option<&str> {
        self.context
            .as_ref()
            .and_then(|context| context.diagnostic_id.as_deref())
    }
}

impl std::fmt::Display for ProjectDiscoveryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self.kind {
            ProjectDiscoveryErrorKind::Configuration => {
                "project discovery configuration is unavailable"
            }
            ProjectDiscoveryErrorKind::Directory => "project discovery directory is unavailable",
            ProjectDiscoveryErrorKind::Registration => "project discovery registration failed",
        })
    }
}
impl std::error::Error for ProjectDiscoveryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.context
            .as_ref()
            .and_then(crate::TechnicalErrorContext::source_error)
    }
}

fn configuration_error(source: crate::OfficialGuiError) -> ProjectDiscoveryError {
    let mut error = ProjectDiscoveryError::ConfigurationUnavailable;
    if let Some(path) = source.path() {
        error = error.with_path(path);
    }
    let id = source.diagnostic_id().map(str::to_owned);
    error = error.with_source(source);
    error
        .context
        .get_or_insert_with(Default::default)
        .diagnostic_id = id;
    error
}

fn registration_error(source: crate::M3Error) -> ProjectDiscoveryError {
    let mut error = ProjectDiscoveryError::RegistrationUnavailable;
    if let Some(path) = source.technical_path() {
        error = error.with_path(path);
    }
    error.with_source(source)
}

/// The OS adapter enumerates only plain immediate child directories and never writes sources.
pub trait ProjectDiscoveryPlatform: Clone + Send + Sync + 'static {
    fn creation_resource(
        &self,
        root: &ProjectDiscoveryRoot,
        candidate: &ProjectDiscoveryCandidate,
    ) -> Result<ResourceKey, ProjectDiscoveryError>;

    fn entries(
        &self,
        root: String,
    ) -> impl std::future::Future<Output = Result<ProjectDiscoveryEntries, ProjectDiscoveryError>> + Send;

    /// Rechecks the root/child object identities, immediate-parent relationship and link policy.
    fn still_current(
        &self,
        root: ProjectDiscoveryRoot,
        candidate: ProjectDiscoveryCandidate,
    ) -> impl std::future::Future<Output = Result<bool, ProjectDiscoveryError>> + Send;
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProjectDiscoveryReport {
    pub registered: u32,
    pub already_registered: u32,
    pub rejected: u32,
    pub reached_limit: bool,
    pub configuration_changed: bool,
    pub startup_recovery_pending: bool,
}

#[derive(Clone)]
pub struct ProjectDiscoveryApplication<S, R, P> {
    store: S,
    reader: R,
    platform: P,
    settings: M7OfficialApplication,
    locks: Arc<ResourceLockCoordinator>,
}

impl<S, R, P> ProjectDiscoveryApplication<S, R, P>
where
    S: M3RegistryStore,
    R: M3ReadAdapter,
    P: ProjectDiscoveryPlatform,
{
    #[must_use]
    pub fn new(store: S, reader: R, platform: P, settings: M7OfficialApplication) -> Self {
        Self::with_locks(
            store,
            reader,
            platform,
            settings,
            Arc::new(ResourceLockCoordinator::default()),
        )
    }

    #[must_use]
    pub fn with_locks(
        store: S,
        reader: R,
        platform: P,
        settings: M7OfficialApplication,
        locks: Arc<ResourceLockCoordinator>,
    ) -> Self {
        Self {
            store,
            reader,
            platform,
            settings,
            locks,
        }
    }

    #[must_use]
    pub fn project_recovery_notify(&self) -> Arc<tokio::sync::Notify> {
        self.locks.project_recovery_notify()
    }

    /// One bounded scan. Scheduling and the single-in-flight task belong to the daemon lifecycle.
    pub async fn scan(&self) -> Result<ProjectDiscoveryReport, ProjectDiscoveryError> {
        self.scan_inner().await
    }

    async fn scan_inner(&self) -> Result<ProjectDiscoveryReport, ProjectDiscoveryError> {
        let mut report = ProjectDiscoveryReport::default();
        if self.locks.project_recovery_pending() {
            report.startup_recovery_pending = true;
            return Ok(report);
        }
        let snapshot = match self.settings.candidate_settings().await {
            Ok(snapshot) => snapshot,
            Err(_) => self
                .settings
                .get_settings(&AccessContext::local_owner())
                .await
                .map_err(configuration_error)?,
        };
        if !snapshot.settings.projects.auto_discover {
            return Ok(report);
        }
        let directories = self
            .settings
            .observe_storage_directories(&snapshot)
            .await
            .map_err(configuration_error)?;
        let entries = self.platform.entries(directories.project_directory).await?;
        report.reached_limit =
            entries.reached_limit || entries.candidates.len() >= PROJECT_DISCOVERY_ENTRY_LIMIT;
        for candidate in entries
            .candidates
            .into_iter()
            .take(PROJECT_DISCOVERY_ENTRY_LIMIT)
        {
            if self.locks.project_recovery_pending() {
                report.startup_recovery_pending = true;
                break;
            }
            // Do not keep the configuration lock while reading project files.
            let Some(fence) = self
                .settings
                .project_discovery_fence(snapshot.revision)
                .await
                .map_err(configuration_error)?
            else {
                report.configuration_changed = true;
                break;
            };
            drop(fence);
            let resource = self.platform.creation_resource(&entries.root, &candidate)?;
            let _creation_guard = self.locks.acquire(vec![resource]).await;
            if !self
                .platform
                .still_current(entries.root.clone(), candidate.clone())
                .await?
            {
                report.rejected += 1;
                continue;
            }
            let observation = match self
                .reader
                .inspect_project(candidate.path.clone(), ProjectDiscoveryMode::ExactRoot)
                .await
            {
                Ok(observation)
                    if observation.root_path == candidate.path
                        && observation.path_identity_key == candidate.identity =>
                {
                    observation
                }
                Ok(_) | Err(_) => {
                    report.rejected += 1;
                    continue;
                }
            };
            if !self
                .platform
                .still_current(entries.root.clone(), candidate)
                .await?
            {
                report.rejected += 1;
                continue;
            }
            let Some(_fence) = self
                .settings
                .project_discovery_fence(snapshot.revision)
                .await
                .map_err(configuration_error)?
            else {
                report.configuration_changed = true;
                break;
            };
            let key = IdempotencyKey::parse(format!("project-discovery:{}", ProjectId::new()))
                .map_err(|source| {
                    ProjectDiscoveryError::RegistrationUnavailable.with_source(source)
                })?;
            let now = m3_time_ms().map_err(registration_error)?;
            // Share the exact registry identity/transaction/event rules of explicit registration.
            // The short configuration fence orders acceptance before any disabling/path update.
            match self
                .store
                .register_project(PrincipalId::local_owner(), observation, key, now)
                .await
            {
                Ok(_) => report.registered += 1,
                Err(error) if error.code() == M3ErrorCode::ProjectAlreadyRegistered => {
                    report.already_registered += 1;
                }
                Err(error) => return Err(registration_error(error)),
            }
        }
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_failure_keeps_its_cause_path_and_identity() {
        let id = crate::OperationId::new().to_string();
        let path = std::env::temp_dir()
            .join("ALCOMD 设置")
            .join("settings.toml")
            .display()
            .to_string();
        let source = crate::OfficialGuiError::Unavailable
            .with_path(path.clone())
            .with_source(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "controlled configuration read failure",
            ))
            .with_diagnostic_id(Some(id.clone()));
        let failure = configuration_error(source);
        assert_eq!(failure.diagnostic_id(), Some(id.as_str()));
        assert_eq!(failure.path(), Some(path.as_str()));
        assert!(
            crate::technical_error_chain(&failure)
                .contains("controlled configuration read failure")
        );
        assert_eq!(failure.diagnostic_id(), Some(id.as_str()));
    }
}
