use std::fmt;
use std::future::Future;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{
    AccessContext, IdempotencyKey, M4Store, OperationId, PackageChangeSet, PackageSourcePin,
    Permission, PrincipalId, ProjectId, ProjectObservation, ResolverCatalog, ResourceKey,
    ResourceLockCoordinator, TechnicalErrorContext,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectCreateInput {
    pub name: String,
    pub target_parent: String,
    pub unity_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<ProjectTemplateRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectTemplateRef {
    pub id: String,
    pub revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectTemplateDeclaration {
    pub id: String,
    pub name: String,
    pub unity: ProjectTemplateUnity,
    pub dependencies: Vec<ProjectTemplateDependency>,
    pub constraints: Vec<ProjectTemplateConstraint>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectTemplateUnity {
    pub series: Vec<String>,
    pub presets: Vec<String>,
}

impl ProjectTemplateUnity {
    #[must_use]
    pub fn allows(&self, version: &str) -> bool {
        if self.series.is_empty() && self.presets.is_empty() {
            return true;
        }
        let series = version.rsplit_once('.').map(|(series, _)| series);
        self.presets.iter().any(|preset| preset == version)
            || self.series.iter().any(|line| Some(line.as_str()) == series)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectTemplateDependency {
    pub package_id: String,
    pub version_range: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectTemplateConstraint {
    pub unity_version: String,
    pub package_id: String,
    pub version_range: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTemplateRecord {
    pub declaration: ProjectTemplateDeclaration,
    pub revision: u64,
    pub locked: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTemplatePage {
    pub templates: Vec<ProjectTemplateRecord>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectCreateAccepted {
    pub operation_id: OperationId,
    pub project_id: ProjectId,
    pub replayed: bool,
}

#[derive(Clone, Debug)]
pub struct ProjectCreateAcceptance {
    pub owner: PrincipalId,
    pub input: ProjectCreateInput,
    pub parent_identity: Vec<u8>,
    pub canonical_parent: String,
    pub fingerprint: String,
    pub key: IdempotencyKey,
    pub allow_package_changes: bool,
}

#[derive(Clone, Debug)]
pub struct ProjectCreateWork {
    pub project_id: ProjectId,
    pub owner: PrincipalId,
    pub input: ProjectCreateInput,
    pub parent_identity: Vec<u8>,
    pub canonical_parent: String,
    pub declaration: Option<ProjectTemplateDeclaration>,
    pub packages: Option<ProjectCreatePackages>,
    pub publishing: bool,
    pub cancel_requested: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectCreatePackages {
    pub change_set: PackageChangeSet,
    pub source_set: Vec<PackageSourcePin>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectCreateErrorCode {
    InvalidInput,
    PermissionDenied,
    IdempotencyConflict,
    TemplateConflict,
    TemplateStale,
    TemplateLocked,
    DependencyUnsatisfied,
    TargetExists,
    DestinationChanged,
    StoreUnavailable,
    Internal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectCreateError {
    code: ProjectCreateErrorCode,
    context: TechnicalErrorContext,
    diagnostic_id: Option<String>,
    recover_on_restart: bool,
}

impl ProjectCreateError {
    #[must_use]
    pub fn new(code: ProjectCreateErrorCode) -> Self {
        Self {
            code,
            context: TechnicalErrorContext::default(),
            diagnostic_id: None,
            recover_on_restart: false,
        }
    }

    #[must_use]
    pub const fn code(&self) -> ProjectCreateErrorCode {
        self.code
    }

    #[must_use]
    pub fn with_source(mut self, source: impl std::error::Error + Send + Sync + 'static) -> Self {
        self.context = self.context.with_source(source);
        self
    }

    #[must_use]
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.context = self.context.with_path(path);
        self
    }

    #[must_use]
    pub fn path(&self) -> Option<&str> {
        self.context.path.as_deref()
    }

    #[must_use]
    pub fn diagnostic_id(&self) -> Option<&str> {
        self.diagnostic_id.as_deref()
    }

    #[must_use]
    pub fn ensure_diagnostic_id(mut self) -> (Self, String) {
        let id = self
            .diagnostic_id
            .get_or_insert_with(|| OperationId::new().to_string())
            .clone();
        (self, id)
    }

    #[must_use]
    pub fn recover_on_restart(mut self) -> Self {
        self.recover_on_restart = true;
        self
    }
}

impl fmt::Display for ProjectCreateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(project_create_error_name(self.code))?;
        if let Some(path) = self.path() {
            write!(formatter, " at {path}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ProjectCreateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.context.source_error()
    }
}

#[must_use]
pub const fn project_create_error_name(code: ProjectCreateErrorCode) -> &'static str {
    match code {
        ProjectCreateErrorCode::InvalidInput => "invalid_request",
        ProjectCreateErrorCode::PermissionDenied => "permission_denied",
        ProjectCreateErrorCode::IdempotencyConflict => "idempotency_conflict",
        ProjectCreateErrorCode::TemplateConflict => "template_conflict",
        ProjectCreateErrorCode::TemplateStale => "revision_conflict",
        ProjectCreateErrorCode::TemplateLocked => "template_builtin_immutable",
        ProjectCreateErrorCode::DependencyUnsatisfied => "template_dependency_unsatisfied",
        ProjectCreateErrorCode::TargetExists => "project_create_target_exists",
        ProjectCreateErrorCode::DestinationChanged => "project_create_destination_changed",
        ProjectCreateErrorCode::StoreUnavailable => "store_unavailable",
        ProjectCreateErrorCode::Internal => "internal_error",
    }
}

pub trait ProjectCreateStore: M4Store + Clone + Send + Sync + 'static {
    fn project_create_replay(
        &self,
        owner: PrincipalId,
        fingerprint: String,
        key: IdempotencyKey,
    ) -> impl Future<Output = Result<Option<ProjectCreateAccepted>, ProjectCreateError>> + Send;

    fn project_create_accept(
        &self,
        acceptance: ProjectCreateAcceptance,
        now_ms: u64,
    ) -> impl Future<Output = Result<ProjectCreateAccepted, ProjectCreateError>> + Send;

    fn project_create_begin(
        &self,
        id: OperationId,
        now_ms: u64,
    ) -> impl Future<Output = Result<Option<ProjectCreateWork>, ProjectCreateError>> + Send;

    fn project_create_save_packages(
        &self,
        id: OperationId,
        packages: ProjectCreatePackages,
        now_ms: u64,
    ) -> impl Future<Output = Result<(), ProjectCreateError>> + Send;

    fn project_create_publish_intent(
        &self,
        id: OperationId,
        now_ms: u64,
    ) -> impl Future<Output = Result<bool, ProjectCreateError>> + Send;

    fn project_create_finish(
        &self,
        id: OperationId,
        observation: ProjectObservation,
        now_ms: u64,
    ) -> impl Future<Output = Result<(), ProjectCreateError>> + Send;

    fn project_create_fail(
        &self,
        id: OperationId,
        error_code: String,
        diagnostic_id: String,
        now_ms: u64,
    ) -> impl Future<Output = Result<(), ProjectCreateError>> + Send;

    fn project_create_cancel(
        &self,
        id: OperationId,
        now_ms: u64,
    ) -> impl Future<Output = Result<(), ProjectCreateError>> + Send;

    fn project_create_pending(
        &self,
        now_ms: u64,
    ) -> impl Future<Output = Result<Vec<OperationId>, ProjectCreateError>> + Send;

    fn project_templates_list(
        &self,
        owner: PrincipalId,
        limit: usize,
        cursor: Option<String>,
    ) -> impl Future<Output = Result<ProjectTemplatePage, ProjectCreateError>> + Send;

    fn project_templates_save(
        &self,
        owner: PrincipalId,
        declaration: ProjectTemplateDeclaration,
        expected_revision: Option<u64>,
        now_ms: u64,
    ) -> impl Future<Output = Result<ProjectTemplateRecord, ProjectCreateError>> + Send;

    fn project_templates_remove(
        &self,
        owner: PrincipalId,
        id: String,
        expected_revision: u64,
        now_ms: u64,
    ) -> impl Future<Output = Result<u64, ProjectCreateError>> + Send;

    fn project_templates_seed(
        &self,
        owner: PrincipalId,
        declarations: Vec<ProjectTemplateDeclaration>,
    ) -> impl Future<Output = Result<(), ProjectCreateError>> + Send;
}

pub trait ProjectCreateAdapter: Clone + Send + Sync + 'static {
    type Stage: Send + Sync;

    fn validate_declaration(
        &self,
        declaration: ProjectTemplateDeclaration,
    ) -> Result<ProjectTemplateDeclaration, ProjectCreateError>;

    fn prepare_destination(
        &self,
        input: &ProjectCreateInput,
    ) -> Result<(String, Vec<u8>), ProjectCreateError>;

    fn fingerprint(&self, input: &ProjectCreateInput) -> Result<String, ProjectCreateError>;

    fn target_resource(&self, work: &ProjectCreateWork) -> ResourceKey;

    fn stage(
        &self,
        id: OperationId,
        work: &ProjectCreateWork,
    ) -> Result<Self::Stage, ProjectCreateError>;

    fn resolve(
        &self,
        stage: &Self::Stage,
        work: &ProjectCreateWork,
        catalog: ResolverCatalog,
    ) -> impl Future<Output = Result<ProjectCreatePackages, ProjectCreateError>> + Send;

    fn materialize(
        &self,
        stage: &Self::Stage,
        packages: ProjectCreatePackages,
        locks: Arc<ResourceLockCoordinator>,
    ) -> impl Future<Output = Result<(), ProjectCreateError>> + Send;

    fn publish(
        &self,
        stage: Self::Stage,
        id: OperationId,
        work: &ProjectCreateWork,
    ) -> impl Future<Output = Result<ProjectObservation, ProjectCreateError>> + Send;

    fn recover_published(
        &self,
        id: OperationId,
        work: &ProjectCreateWork,
    ) -> impl Future<Output = Result<Option<ProjectObservation>, ProjectCreateError>> + Send;

    fn discard(&self, id: OperationId, work: &ProjectCreateWork) -> Result<(), ProjectCreateError>;
}

#[derive(Clone)]
pub struct ProjectCreateApplication<S, A> {
    store: S,
    adapter: A,
    locks: Arc<ResourceLockCoordinator>,
}

impl<S: ProjectCreateStore, A: ProjectCreateAdapter> ProjectCreateApplication<S, A> {
    #[must_use]
    pub fn with_locks(store: S, adapter: A, locks: Arc<ResourceLockCoordinator>) -> Self {
        Self {
            store,
            adapter,
            locks,
        }
    }

    pub async fn seed_builtin_templates(&self) -> Result<(), ProjectCreateError> {
        let declarations = [
            include_str!("../../../specs/templates/vrchat-avatars.json"),
            include_str!("../../../specs/templates/vrchat-worlds.json"),
        ]
        .into_iter()
        .map(|json| {
            let declaration = serde_json::from_str(json).map_err(|cause| {
                ProjectCreateError::new(ProjectCreateErrorCode::Internal).with_source(cause)
            })?;
            self.adapter.validate_declaration(declaration)
        })
        .collect::<Result<Vec<_>, _>>()?;
        self.store
            .project_templates_seed(PrincipalId::local_owner(), declarations)
            .await
    }

    pub async fn create(
        &self,
        access: &AccessContext,
        input: ProjectCreateInput,
        key: IdempotencyKey,
    ) -> Result<ProjectCreateAccepted, ProjectCreateError> {
        require_owner(access)?;
        access
            .require(Permission::ProjectsCreate)
            .map_err(|cause| {
                ProjectCreateError::new(ProjectCreateErrorCode::PermissionDenied).with_source(cause)
            })?;
        crate::canonical_unity_version(&input.unity_version).map_err(|cause| {
            ProjectCreateError::new(ProjectCreateErrorCode::InvalidInput).with_source(cause)
        })?;
        if let Some(template) = &input.template {
            validate_template_reference(&template.id, template.revision)?;
        }
        let owner = access.principal().clone();
        let fingerprint = self.adapter.fingerprint(&input)?;
        if let Some(replayed) = self
            .store
            .project_create_replay(owner.clone(), fingerprint.clone(), key.clone())
            .await?
        {
            return Ok(replayed);
        }
        let (canonical_parent, parent_identity) = match self.adapter.prepare_destination(&input) {
            Ok(destination) => destination,
            Err(cause) => {
                // A concurrent call with the same key may have published since the first read.
                return self
                    .store
                    .project_create_replay(owner, fingerprint, key)
                    .await?
                    .ok_or(cause);
            }
        };
        let allow_packages = [
            Permission::PackagesRead,
            Permission::RepositoriesRead,
            Permission::PackagesManage,
        ]
        .into_iter()
        .all(|permission| access.require(permission).is_ok());
        let accepted = self
            .store
            .project_create_accept(
                ProjectCreateAcceptance {
                    owner,
                    input,
                    parent_identity,
                    canonical_parent,
                    fingerprint,
                    key,
                    allow_package_changes: allow_packages,
                },
                now_ms()?,
            )
            .await?;
        if !accepted.replayed {
            let application = self.clone();
            let id = accepted.operation_id;
            tokio::spawn(async move {
                let _ = application.run(id).await;
            });
        }
        Ok(accepted)
    }

    pub async fn list_templates(
        &self,
        access: &AccessContext,
        limit: Option<u16>,
        cursor: Option<String>,
    ) -> Result<ProjectTemplatePage, ProjectCreateError> {
        require_owner(access)?;
        let limit = limit.unwrap_or(100);
        if !(1..=100).contains(&limit) {
            return Err(ProjectCreateError::new(
                ProjectCreateErrorCode::InvalidInput,
            ));
        }
        self.store
            .project_templates_list(access.principal().clone(), usize::from(limit), cursor)
            .await
    }

    pub async fn save_template(
        &self,
        access: &AccessContext,
        declaration: ProjectTemplateDeclaration,
        expected_revision: Option<u64>,
    ) -> Result<ProjectTemplateRecord, ProjectCreateError> {
        require_owner(access)?;
        if let Some(revision) = expected_revision {
            validate_template_reference(&declaration.id, revision)?;
        }
        let declaration = self.adapter.validate_declaration(declaration)?;
        self.store
            .project_templates_save(
                access.principal().clone(),
                declaration,
                expected_revision,
                now_ms()?,
            )
            .await
    }

    pub async fn remove_template(
        &self,
        access: &AccessContext,
        id: String,
        expected_revision: u64,
    ) -> Result<u64, ProjectCreateError> {
        require_owner(access)?;
        validate_template_reference(&id, expected_revision)?;
        self.store
            .project_templates_remove(access.principal().clone(), id, expected_revision, now_ms()?)
            .await
    }

    pub async fn recover(&self) -> Result<(), ProjectCreateError> {
        for id in self.store.project_create_pending(now_ms()?).await? {
            self.run(id).await?;
        }
        Ok(())
    }

    async fn run(&self, id: OperationId) -> Result<(), ProjectCreateError> {
        let Some(work) = self.store.project_create_begin(id, now_ms()?).await? else {
            return Ok(());
        };
        if let Err(cause) = self.run_work(id, work.clone()).await {
            if cause.recover_on_restart || cause.code() == ProjectCreateErrorCode::StoreUnavailable
            {
                return Err(cause);
            }
            let cause = self.adapter.discard(id, &work).err().unwrap_or(cause);
            let (cause, diagnostic_id) = cause.ensure_diagnostic_id();
            crate::with_operation_log_failure(
                id,
                crate::operation_log_error(&cause, cause.path()),
                self.store.project_create_fail(
                    id,
                    project_create_error_name(cause.code()).to_owned(),
                    diagnostic_id,
                    now_ms()?,
                ),
            )
            .await?;
        }
        Ok(())
    }

    async fn run_work(
        &self,
        id: OperationId,
        mut work: ProjectCreateWork,
    ) -> Result<(), ProjectCreateError> {
        if work.cancel_requested && !work.publishing {
            self.adapter.discard(id, &work)?;
            return self.store.project_create_cancel(id, now_ms()?).await;
        }
        let target_lock = self.adapter.target_resource(&work);
        if work.publishing {
            let _target = self.locks.acquire(vec![target_lock.clone()]).await;
            if let Some(observation) = self.adapter.recover_published(id, &work).await? {
                return self
                    .store
                    .project_create_finish(id, observation, now_ms()?)
                    .await;
            }
        }
        let stage = self.adapter.stage(id, &work)?;
        if work
            .declaration
            .as_ref()
            .is_some_and(|value| !value.dependencies.is_empty())
        {
            if work.packages.is_none() {
                let catalog = self
                    .store
                    .resolver_catalog(work.owner.clone(), false)
                    .await
                    .map_err(|cause| {
                        ProjectCreateError::new(ProjectCreateErrorCode::StoreUnavailable)
                            .with_source(cause)
                    })?;
                let packages = self.adapter.resolve(&stage, &work, catalog).await?;
                self.store
                    .project_create_save_packages(id, packages.clone(), now_ms()?)
                    .await?;
                work.packages = Some(packages);
            }
            if let Some(packages) = &work.packages {
                self.adapter
                    .materialize(&stage, packages.clone(), Arc::clone(&self.locks))
                    .await?;
            }
        }
        let _target = self.locks.acquire(vec![target_lock]).await;
        if !self
            .store
            .project_create_publish_intent(id, now_ms()?)
            .await?
        {
            self.adapter.discard(id, &work)?;
            return self.store.project_create_cancel(id, now_ms()?).await;
        }
        let observation = self.adapter.publish(stage, id, &work).await?;
        self.store
            .project_create_finish(id, observation, now_ms()?)
            .await
    }
}

fn require_owner(access: &AccessContext) -> Result<(), ProjectCreateError> {
    if access.principal().as_str() != PrincipalId::LOCAL_OWNER {
        return Err(ProjectCreateError::new(
            ProjectCreateErrorCode::PermissionDenied,
        ));
    }
    Ok(())
}

#[must_use]
pub fn valid_project_template_id(id: &str) -> bool {
    id.len() <= 128
        && id.contains('.')
        && id.split('.').all(|part| {
            let edge = |byte: &u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
            part.as_bytes().first().is_some_and(edge)
                && part.as_bytes().last().is_some_and(edge)
                && part.bytes().all(|byte| edge(&byte) || byte == b'-')
        })
}

fn validate_template_reference(id: &str, revision: u64) -> Result<(), ProjectCreateError> {
    if !valid_project_template_id(id) || !(1..=9_007_199_254_740_991).contains(&revision) {
        return Err(ProjectCreateError::new(
            ProjectCreateErrorCode::InvalidInput,
        ));
    }
    Ok(())
}

fn now_ms() -> Result<u64, ProjectCreateError> {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|cause| {
            ProjectCreateError::new(ProjectCreateErrorCode::Internal).with_source(cause)
        })?;
    u64::try_from(duration.as_millis()).map_err(|cause| {
        ProjectCreateError::new(ProjectCreateErrorCode::Internal).with_source(cause)
    })
}
