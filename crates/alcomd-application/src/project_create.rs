use std::future::Future;
use std::sync::Arc;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use serde::{Deserialize, Serialize};

use crate::{
    AccessContext, IdempotencyKey, M4Store, OperationId, PackageChangeSet, PackageSourcePin,
    Permission, PrincipalId, ProjectId, ProjectObservation, ProjectRecord, ResolverCatalog,
    ResourceKey, ResourceLockCoordinator, TechnicalErrorContext,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor_version: Option<String>,
    pub archive_sha256: String,
    pub dependencies: Vec<ProjectTemplateDependency>,
    pub locked_versions: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectTemplateDependency {
    pub package_id: String,
    pub version_range: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ProjectTemplateSource {
    File {
        source_file: String,
    },
    Project {
        project_id: ProjectId,
        expected_project_revision: u64,
        id: String,
        name: String,
        include_assets: bool,
        lock_package_versions: bool,
    },
    Manual {
        id: String,
        name: String,
        unity_version: String,
        dependencies: Vec<ProjectTemplateDependency>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ProjectTemplateJobRequest {
    Save {
        source: ProjectTemplateSource,
        expected_revision: Option<u64>,
    },
    Export {
        reference: ProjectTemplateRef,
        destination_file: String,
    },
}

#[derive(Clone, Debug)]
pub struct ProjectTemplateJobWork {
    pub owner: PrincipalId,
    pub request: ProjectTemplateJobRequest,
    pub export_declaration: Option<ProjectTemplateDeclaration>,
    pub cancel_requested: bool,
    pub publishing: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTemplateJobAccepted {
    pub operation_id: OperationId,
    pub replayed: bool,
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
    package_id: Option<String>,
    recover_on_restart: bool,
}

impl ProjectCreateError {
    #[must_use]
    pub fn new(code: ProjectCreateErrorCode) -> Self {
        Self {
            code,
            context: TechnicalErrorContext::default(),
            diagnostic_id: None,
            package_id: None,
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
    pub fn with_package_id(mut self, package_id: String) -> Self {
        self.package_id = Some(package_id);
        self
    }

    #[must_use]
    pub fn package_id(&self) -> Option<&str> {
        self.package_id.as_deref()
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
        if let Some(package_id) = self.package_id() {
            write!(formatter, " ({package_id})")?;
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
        package_id: Option<String>,
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

    fn project_template_archive_references(
        &self,
    ) -> impl Future<Output = Result<BTreeSet<String>, ProjectCreateError>> + Send;

    fn project_templates_source_project(
        &self,
        owner: PrincipalId,
        project_id: ProjectId,
    ) -> impl Future<Output = Result<ProjectRecord, ProjectCreateError>> + Send;

    fn project_template_job_accept(
        &self,
        owner: PrincipalId,
        request: ProjectTemplateJobRequest,
        key: IdempotencyKey,
        fingerprint: String,
        now_ms: u64,
    ) -> impl Future<Output = Result<ProjectTemplateJobAccepted, ProjectCreateError>> + Send;

    fn project_template_job_begin(
        &self,
        id: OperationId,
        now_ms: u64,
    ) -> impl Future<Output = Result<Option<ProjectTemplateJobWork>, ProjectCreateError>> + Send;

    fn project_template_job_publish_intent(
        &self,
        id: OperationId,
        now_ms: u64,
    ) -> impl Future<Output = Result<bool, ProjectCreateError>> + Send;

    fn project_template_job_finish_save(
        &self,
        id: OperationId,
        declaration: ProjectTemplateDeclaration,
        now_ms: u64,
    ) -> impl Future<Output = Result<(), ProjectCreateError>> + Send;

    fn project_template_job_finish_export(
        &self,
        id: OperationId,
        now_ms: u64,
    ) -> impl Future<Output = Result<(), ProjectCreateError>> + Send;

    fn project_template_job_fail(
        &self,
        id: OperationId,
        error_code: String,
        diagnostic_id: String,
        package_id: Option<String>,
        now_ms: u64,
    ) -> impl Future<Output = Result<(), ProjectCreateError>> + Send;

    fn project_template_job_cancel(
        &self,
        id: OperationId,
        now_ms: u64,
    ) -> impl Future<Output = Result<(), ProjectCreateError>> + Send;

    fn project_template_job_pending(
        &self,
        now_ms: u64,
    ) -> impl Future<Output = Result<Vec<OperationId>, ProjectCreateError>> + Send;
}

pub trait ProjectCreateAdapter: Clone + Send + Sync + 'static {
    fn source_settings(
        &self,
    ) -> impl Future<Output = Result<crate::ConfigPackageSettings, ProjectCreateError>> + Send;
    type Stage: Send + Sync;

    fn prepare_template(
        &self,
        source: ProjectTemplateSource,
        project: Option<ProjectRecord>,
    ) -> Result<ProjectTemplateDeclaration, ProjectCreateError>;

    fn builtin_templates(&self) -> Result<Vec<ProjectTemplateDeclaration>, ProjectCreateError>;

    fn prune_template_archives(
        &self,
        referenced: &BTreeSet<String>,
    ) -> Result<(), ProjectCreateError>;

    fn template_job_fingerprint(
        &self,
        request: &ProjectTemplateJobRequest,
    ) -> Result<String, ProjectCreateError>;

    fn export_template(
        &self,
        declaration: &ProjectTemplateDeclaration,
        destination_file: &str,
        id: OperationId,
    ) -> Result<(), ProjectCreateError>;

    fn finish_template_export(
        &self,
        destination_file: &str,
        id: OperationId,
    ) -> Result<(), ProjectCreateError>;

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
    // Archive publication and its catalogue commit must be indivisible to pruning.
    template_archive_writes: Arc<tokio::sync::Mutex<()>>,
}

impl<S: ProjectCreateStore, A: ProjectCreateAdapter> ProjectCreateApplication<S, A> {
    #[must_use]
    pub fn with_locks(store: S, adapter: A, locks: Arc<ResourceLockCoordinator>) -> Self {
        Self {
            store,
            adapter,
            locks,
            template_archive_writes: Arc::default(),
        }
    }

    pub async fn seed_builtin_templates(&self) -> Result<(), ProjectCreateError> {
        let _archive_write = self.template_archive_writes.lock().await;
        let adapter = self.adapter.clone();
        let declarations = template_io(move || adapter.builtin_templates()).await?;
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
        source: ProjectTemplateSource,
        expected_revision: Option<u64>,
        key: IdempotencyKey,
    ) -> Result<ProjectTemplateJobAccepted, ProjectCreateError> {
        require_owner(access)?;
        if let Some(revision) = expected_revision {
            validate_safe_revision(revision)?;
        }
        match &source {
            ProjectTemplateSource::File { source_file } => validate_template_path(source_file)?,
            ProjectTemplateSource::Project {
                project_id,
                expected_project_revision,
                id,
                name,
                ..
            } => {
                access
                    .require_project_read_scope(&project_id.to_string())
                    .map_err(|cause| {
                        ProjectCreateError::new(ProjectCreateErrorCode::PermissionDenied)
                            .with_source(cause)
                    })?;
                validate_safe_revision(*expected_project_revision)?;
                validate_template_metadata(id, name)?;
            }
            ProjectTemplateSource::Manual {
                id,
                name,
                unity_version,
                dependencies,
            } => {
                validate_template_metadata(id, name)?;
                crate::canonical_unity_version(unity_version).map_err(|cause| {
                    ProjectCreateError::new(ProjectCreateErrorCode::InvalidInput).with_source(cause)
                })?;
                if dependencies.len() > 128
                    || dependencies.iter().any(|entry| {
                        entry.package_id.len() > 128 || entry.version_range.len() > 1024
                    })
                {
                    return Err(ProjectCreateError::new(
                        ProjectCreateErrorCode::InvalidInput,
                    ));
                }
            }
        }
        self.start_template_job(
            access,
            ProjectTemplateJobRequest::Save {
                source,
                expected_revision,
            },
            key,
        )
        .await
    }

    pub async fn export_template(
        &self,
        access: &AccessContext,
        reference: ProjectTemplateRef,
        destination_file: String,
        key: IdempotencyKey,
    ) -> Result<ProjectTemplateJobAccepted, ProjectCreateError> {
        require_owner(access)?;
        validate_template_reference(&reference.id, reference.revision)?;
        validate_template_path(&destination_file)?;
        self.start_template_job(
            access,
            ProjectTemplateJobRequest::Export {
                reference,
                destination_file,
            },
            key,
        )
        .await
    }

    async fn start_template_job(
        &self,
        access: &AccessContext,
        request: ProjectTemplateJobRequest,
        key: IdempotencyKey,
    ) -> Result<ProjectTemplateJobAccepted, ProjectCreateError> {
        let serialized = serde_json::to_vec(&request).map_err(|cause| {
            ProjectCreateError::new(ProjectCreateErrorCode::Internal).with_source(cause)
        })?;
        if serialized.len() > 48 * 1024 {
            return Err(ProjectCreateError::new(
                ProjectCreateErrorCode::InvalidInput,
            ));
        }
        let fingerprint = self.adapter.template_job_fingerprint(&request)?;
        let accepted = self
            .store
            .project_template_job_accept(
                access.principal().clone(),
                request,
                key,
                fingerprint,
                now_ms()?,
            )
            .await?;
        if !accepted.replayed {
            let application = self.clone();
            let id = accepted.operation_id;
            tokio::spawn(async move {
                let _ = application.run_template_job(id).await;
            });
        }
        Ok(accepted)
    }

    pub async fn remove_template(
        &self,
        access: &AccessContext,
        id: String,
        expected_revision: u64,
    ) -> Result<u64, ProjectCreateError> {
        require_owner(access)?;
        validate_template_reference(&id, expected_revision)?;
        let revision = self
            .store
            .project_templates_remove(access.principal().clone(), id, expected_revision, now_ms()?)
            .await?;
        self.prune_template_archives().await;
        Ok(revision)
    }

    async fn prune_template_archives(&self) {
        let _archive_write = self.template_archive_writes.lock().await;
        let result = async {
            // A newly accepted create/export can only pin an archive still in the catalogue.
            let referenced = self.store.project_template_archive_references().await?;
            let adapter = self.adapter.clone();
            template_io(move || adapter.prune_template_archives(&referenced)).await
        }
        .await;
        if let Err(cause) = result {
            // Catalogue changes are already committed; startup retries orphan cleanup.
            eprintln!("Could not prune unused template archives: {cause}");
        }
    }

    pub async fn recover(&self) -> Result<(), ProjectCreateError> {
        for id in self.store.project_create_pending(now_ms()?).await? {
            self.run(id).await?;
        }
        for id in self.store.project_template_job_pending(now_ms()?).await? {
            self.run_template_job(id).await?;
        }
        self.prune_template_archives().await;
        Ok(())
    }

    async fn run_template_job(&self, id: OperationId) -> Result<(), ProjectCreateError> {
        let Some(work) = self.store.project_template_job_begin(id, now_ms()?).await? else {
            self.prune_template_archives().await;
            return Ok(());
        };
        let result = if work.cancel_requested && !work.publishing {
            self.store.project_template_job_cancel(id, now_ms()?).await
        } else {
            self.run_template_work(id, work).await
        };
        if let Err(cause) = result {
            if cause.recover_on_restart || cause.code() == ProjectCreateErrorCode::StoreUnavailable
            {
                return Err(cause);
            }
            let (cause, diagnostic_id) = cause.ensure_diagnostic_id();
            crate::with_operation_log_failure(
                id,
                crate::operation_log_error(&cause, cause.path()),
                self.store.project_template_job_fail(
                    id,
                    project_create_error_name(cause.code()).to_owned(),
                    diagnostic_id,
                    cause.package_id().map(str::to_owned),
                    now_ms()?,
                ),
            )
            .await?;
        }
        self.prune_template_archives().await;
        Ok(())
    }

    async fn run_template_work(
        &self,
        id: OperationId,
        work: ProjectTemplateJobWork,
    ) -> Result<(), ProjectCreateError> {
        let adapter = self.adapter.clone();
        match work.request {
            ProjectTemplateJobRequest::Save { source, .. } => {
                let mut project_guard = None;
                let project = if let ProjectTemplateSource::Project {
                    project_id,
                    expected_project_revision,
                    ..
                } = &source
                {
                    project_guard = Some(
                        self.locks
                            .acquire(vec![ResourceKey::Project(*project_id)])
                            .await,
                    );
                    let record = self
                        .store
                        .project_templates_source_project(work.owner, *project_id)
                        .await?;
                    if record.revision.get() != *expected_project_revision {
                        return Err(ProjectCreateError::new(
                            ProjectCreateErrorCode::TemplateStale,
                        ));
                    }
                    Some(record)
                } else {
                    None
                };
                let _archive_write = self.template_archive_writes.lock().await;
                let declaration =
                    template_io(move || adapter.prepare_template(source, project)).await?;
                let result = self
                    .store
                    .project_template_job_finish_save(id, declaration, now_ms()?)
                    .await;
                drop(project_guard);
                result
            }
            ProjectTemplateJobRequest::Export {
                destination_file, ..
            } => {
                let declaration = work
                    .export_declaration
                    .ok_or_else(|| ProjectCreateError::new(ProjectCreateErrorCode::Internal))?;
                if !self
                    .store
                    .project_template_job_publish_intent(id, now_ms()?)
                    .await?
                {
                    return self.store.project_template_job_cancel(id, now_ms()?).await;
                }
                let destination = destination_file.clone();
                template_io(move || adapter.export_template(&declaration, &destination, id))
                    .await?;
                self.store
                    .project_template_job_finish_export(id, now_ms()?)
                    .await?;
                let adapter = self.adapter.clone();
                let _ = template_io(move || adapter.finish_template_export(&destination_file, id))
                    .await;
                Ok(())
            }
        }
    }

    async fn run(&self, id: OperationId) -> Result<(), ProjectCreateError> {
        let Some(work) = self.store.project_create_begin(id, now_ms()?).await? else {
            self.prune_template_archives().await;
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
                    cause.package_id().map(str::to_owned),
                    now_ms()?,
                ),
            )
            .await?;
        }
        if work.declaration.is_some() {
            self.prune_template_archives().await;
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
        if work.declaration.as_ref().is_some_and(|value| {
            !value.dependencies.is_empty() || !value.locked_versions.is_empty()
        }) {
            if work.packages.is_none() {
                let catalog = self
                    .store
                    .resolver_catalog(
                        work.owner.clone(),
                        false,
                        self.adapter.source_settings().await?,
                    )
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
    if !valid_project_template_id(id) {
        return Err(ProjectCreateError::new(
            ProjectCreateErrorCode::InvalidInput,
        ));
    }
    validate_safe_revision(revision)
}

fn validate_safe_revision(revision: u64) -> Result<(), ProjectCreateError> {
    if !(1..=9_007_199_254_740_991).contains(&revision) {
        return Err(ProjectCreateError::new(
            ProjectCreateErrorCode::InvalidInput,
        ));
    }
    Ok(())
}

fn validate_template_metadata(id: &str, name: &str) -> Result<(), ProjectCreateError> {
    if !valid_project_template_id(id)
        || name.trim().is_empty()
        || name.len() > 128
        || name.chars().any(char::is_control)
    {
        return Err(ProjectCreateError::new(
            ProjectCreateErrorCode::InvalidInput,
        ));
    }
    Ok(())
}

fn validate_template_path(path: &str) -> Result<(), ProjectCreateError> {
    if path.is_empty() || path.len() > 32_768 || path.contains('\0') {
        return Err(ProjectCreateError::new(
            ProjectCreateErrorCode::InvalidInput,
        ));
    }
    Ok(())
}

async fn template_io<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, ProjectCreateError> + Send + 'static,
) -> Result<T, ProjectCreateError> {
    tokio::task::spawn_blocking(work).await.map_err(|cause| {
        ProjectCreateError::new(ProjectCreateErrorCode::Internal).with_source(cause)
    })?
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
