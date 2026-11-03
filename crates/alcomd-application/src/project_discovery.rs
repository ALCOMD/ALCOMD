//! Bounded default-directory discovery. Only the Core invokes this internal use case.

use std::sync::Arc;

use crate::{
    AccessContext, IdempotencyKey, M3ErrorCode, M3ReadAdapter, M3RegistryStore,
    M7OfficialApplication, OfficialGuiStore, PrincipalId, ProjectDiscoveryMode, ProjectId,
    ResourceKey, ResourceLockCoordinator, m3_time_ms,
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
pub enum ProjectDiscoveryError {
    ConfigurationUnavailable,
    DirectoryUnavailable,
    RegistrationUnavailable,
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
    settings: M7OfficialApplication<S>,
    locks: Arc<ResourceLockCoordinator>,
}

impl<S, R, P> ProjectDiscoveryApplication<S, R, P>
where
    S: M3RegistryStore + OfficialGuiStore,
    R: M3ReadAdapter,
    P: ProjectDiscoveryPlatform,
{
    #[must_use]
    pub fn new(store: S, reader: R, platform: P, settings: M7OfficialApplication<S>) -> Self {
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
        settings: M7OfficialApplication<S>,
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
                .map_err(|_| ProjectDiscoveryError::ConfigurationUnavailable)?,
        };
        if !snapshot.settings.projects.auto_discover {
            return Ok(report);
        }
        let directories = self
            .settings
            .observe_storage_directories(&snapshot)
            .await
            .map_err(|_| ProjectDiscoveryError::ConfigurationUnavailable)?;
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
                .map_err(|_| ProjectDiscoveryError::ConfigurationUnavailable)?
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
                .map_err(|_| ProjectDiscoveryError::ConfigurationUnavailable)?
            else {
                report.configuration_changed = true;
                break;
            };
            let key = IdempotencyKey::parse(format!("project-discovery:{}", ProjectId::new()))
                .map_err(|_| ProjectDiscoveryError::RegistrationUnavailable)?;
            let now = m3_time_ms().map_err(|_| ProjectDiscoveryError::RegistrationUnavailable)?;
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
                Err(_) => return Err(ProjectDiscoveryError::RegistrationUnavailable),
            }
        }
        Ok(report)
    }
}
