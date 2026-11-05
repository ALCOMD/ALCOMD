//! M5 native Template registry and immutable import/derive Plan use cases.

use std::future::Future;

use serde::{Deserialize, Serialize};

use crate::{
    AccessContext, IdempotencyKey, M3RegistryStore, M4Store, OperationId, PlanId, PrincipalId,
    ProjectId, ProjectRecord, ResolverCatalog, ResourceKey, ResourceLockCoordinator, Revision,
    TemplateId, UnityWriterState, UnityWriterStateKind,
};

/// Durable Template source class. Provenance remains inside the bundle manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateSourceKind {
    Builtin,
    User,
}

/// Public-safe Template registry row.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateRecord {
    pub template_id: TemplateId,
    pub source_kind: TemplateSourceKind,
    pub template_version: String,
    pub display_name: String,
    pub description: Option<String>,
    pub provenance: String,
    pub manifest_json: String,
    pub bundle_sha256: [u8; 32],
    pub manifest_fingerprint: [u8; 32],
    pub favorite: bool,
    pub revision: Revision,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

/// Registry row exactly represented by State Schema v4; presentation fields are derived from the
/// strictly validated canonical manifest by the Template adapter.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredTemplateRecord {
    pub template_id: TemplateId,
    pub source_kind: TemplateSourceKind,
    pub template_version: String,
    pub manifest_json: String,
    pub payload_locator: String,
    pub bundle_sha256: [u8; 32],
    pub favorite: bool,
    pub revision: Revision,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

/// Opaque stable Template page cursor.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TemplateCursor {
    pub updated_at_ms: u64,
    pub template_id: TemplateId,
}

/// Bounded Template registry page.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TemplatePage {
    pub templates: Vec<TemplateRecord>,
    pub next_cursor: Option<TemplateCursor>,
}

/// Strict inspection result plus private adapter evidence used to freeze a Plan.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateBundleEvidence {
    pub template_id: TemplateId,
    pub template_version: String,
    pub display_name: String,
    pub description: Option<String>,
    pub provenance: String,
    pub manifest_json: String,
    pub bundle_sha256: [u8; 32],
    pub manifest_fingerprint: [u8; 32],
    pub payload_tree_sha256: [u8; 32],
    pub entry_count: u64,
    pub total_bytes: u64,
    pub source_path: String,
    pub source_filesystem_identity: Vec<u8>,
}

/// Narrow immutable Template Plan kind in State Schema v5.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TemplatePlanKind {
    Import,
    Derive,
    CreateProject,
}

/// Durable Template Plan lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplatePlanState {
    Unapplied,
    Applied,
}

/// Adapter-produced Plan payload and fingerprint ready for durable insertion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplatePlanDraft {
    pub kind: TemplatePlanKind,
    pub plan_json: String,
    pub plan_fingerprint: [u8; 32],
}

/// Durable immutable Template Plan.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TemplatePlanRecord {
    pub plan_id: PlanId,
    pub owner: PrincipalId,
    pub kind: TemplatePlanKind,
    pub state: TemplatePlanState,
    pub plan_json: String,
    pub plan_fingerprint: [u8; 32],
    pub apply_operation_id: Option<OperationId>,
    pub created_at_ms: u64,
}

/// Accepted or replayed Template apply Operation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TemplateApplyOutcome {
    pub operation_id: OperationId,
    pub replayed: bool,
    pub schedule: bool,
}

/// Result produced only after an object is durably published.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishedTemplate {
    pub record: StoredTemplateRecord,
}

/// Fully validated unpublished project image returned by the Template adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreatedTemplateProject {
    pub project_id: ProjectId,
    pub observation: crate::ProjectObservation,
}

/// Stable safe Template errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum M5TemplateErrorCode {
    InvalidInput,
    PermissionDenied,
    TemplateNotFound,
    TemplateImmutable,
    TemplateConflict,
    TemplateRevisionConflict,
    TemplatePlanNotFound,
    TemplatePlanStale,
    TemplateBundleInvalid,
    TemplateBundleChanged,
    TemplateObjectMissing,
    DependencyUnsatisfied,
    TemplateTargetExists,
    ProjectNotRegistered,
    ProjectRunning,
    ProjectChangedDuringTemplateCreate,
    StoreUnavailable,
    Internal,
}

/// Private technical error; only its stable code and diagnostic identity cross business RPC.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct M5TemplateError {
    code: M5TemplateErrorCode,
    context: crate::TechnicalErrorContext,
    diagnostic_id: Option<String>,
}

impl M5TemplateError {
    #[must_use]
    pub fn new(code: M5TemplateErrorCode) -> Self {
        Self {
            code,
            context: crate::TechnicalErrorContext::default(),
            diagnostic_id: None,
        }
    }
    #[must_use]
    pub const fn code(&self) -> M5TemplateErrorCode {
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
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.context = self.context.with_reason(reason);
        self
    }
    #[must_use]
    pub fn with_diagnostic_id(mut self, id: String) -> Self {
        self.diagnostic_id = Some(id);
        self
    }
    #[must_use]
    pub fn diagnostic_id(&self) -> Option<&str> {
        self.diagnostic_id.as_deref()
    }
    fn ensure_diagnostic_id(mut self) -> (Self, String) {
        let id = self
            .diagnostic_id
            .clone()
            .unwrap_or_else(|| crate::OperationId::new().to_string());
        self.diagnostic_id = Some(id.clone());
        (self, id)
    }
}

impl std::fmt::Display for M5TemplateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "request failed: {:?}", self.code)?;
        if let Some(path) = self.path() {
            write!(formatter, " at {path}")?;
        }
        Ok(())
    }
}
impl std::error::Error for M5TemplateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.context.source_error()
    }
}

/// Authoritative State Schema v5 persistence port.
pub trait M5TemplateStore: Clone + Send + Sync + 'static {
    fn ensure_builtin_templates(
        &self,
        owner: PrincipalId,
        templates: Vec<StoredTemplateRecord>,
        now_ms: u64,
    ) -> impl Future<Output = Result<(), M5TemplateError>> + Send;
    fn list_templates(
        &self,
        owner: PrincipalId,
        cursor: Option<TemplateCursor>,
        limit: u32,
    ) -> impl Future<Output = Result<Vec<StoredTemplateRecord>, M5TemplateError>> + Send;
    fn get_template(
        &self,
        owner: PrincipalId,
        template_id: TemplateId,
    ) -> impl Future<Output = Result<StoredTemplateRecord, M5TemplateError>> + Send;
    fn create_template_plan(
        &self,
        owner: PrincipalId,
        draft: TemplatePlanDraft,
        created_at_ms: u64,
    ) -> impl Future<Output = Result<TemplatePlanRecord, M5TemplateError>> + Send;
    fn get_template_plan(
        &self,
        owner: PrincipalId,
        plan_id: PlanId,
    ) -> impl Future<Output = Result<TemplatePlanRecord, M5TemplateError>> + Send;
    fn accept_template_plan(
        &self,
        owner: PrincipalId,
        plan_id: PlanId,
        key: IdempotencyKey,
        created_at_ms: u64,
    ) -> impl Future<Output = Result<TemplateApplyOutcome, M5TemplateError>> + Send;
    fn begin_template_apply(
        &self,
        operation_id: OperationId,
        updated_at_ms: u64,
    ) -> impl Future<Output = Result<TemplatePlanRecord, M5TemplateError>> + Send;
    fn complete_template_apply(
        &self,
        operation_id: OperationId,
        template: PublishedTemplate,
        completed_at_ms: u64,
    ) -> impl Future<Output = Result<(), M5TemplateError>> + Send;
    fn record_template_checkpoint(
        &self,
        operation_id: OperationId,
        step: u64,
        phase: String,
        updated_at_ms: u64,
    ) -> impl Future<Output = Result<(), M5TemplateError>> + Send;
    fn complete_template_project_create(
        &self,
        operation_id: OperationId,
        project: CreatedTemplateProject,
        completed_at_ms: u64,
    ) -> impl Future<Output = Result<(), M5TemplateError>> + Send;
    fn fail_template_apply(
        &self,
        operation_id: OperationId,
        error_code: String,
        diagnostic_id: String,
        completed_at_ms: u64,
    ) -> impl Future<Output = Result<(), M5TemplateError>> + Send;
    fn recover_template_operations(
        &self,
        recovered_at_ms: u64,
    ) -> impl Future<Output = Result<Vec<OperationId>, M5TemplateError>> + Send;
    fn set_template_favorite(
        &self,
        owner: PrincipalId,
        template_id: TemplateId,
        favorite: bool,
        expected: Revision,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<(StoredTemplateRecord, bool), M5TemplateError>> + Send;
    fn remove_template(
        &self,
        owner: PrincipalId,
        template_id: TemplateId,
        expected: Revision,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<(bool, bool), M5TemplateError>> + Send;
}

/// Filesystem/archive adapter; it owns no registry authority.
pub trait M5TemplateAdapter: Clone + Send + Sync + 'static {
    type PreparedCreateProject: Send + 'static;
    type StagedCreateProject: Send + 'static;

    fn inspect_bundle(
        &self,
        source_path: String,
    ) -> impl Future<Output = Result<TemplateBundleEvidence, M5TemplateError>> + Send;
    fn present_record(
        &self,
        record: StoredTemplateRecord,
    ) -> impl Future<Output = Result<TemplateRecord, M5TemplateError>> + Send;
    fn import_plan(
        &self,
        evidence: TemplateBundleEvidence,
        existing: Option<StoredTemplateRecord>,
        override_existing: bool,
        expected_revision: Option<Revision>,
    ) -> impl Future<Output = Result<TemplatePlanDraft, M5TemplateError>> + Send;
    fn publish_import(
        &self,
        plan: TemplatePlanRecord,
    ) -> impl Future<Output = Result<PublishedTemplate, M5TemplateError>> + Send;
    fn export_bundle(
        &self,
        template: StoredTemplateRecord,
        target_path: String,
    ) -> impl Future<Output = Result<(), M5TemplateError>> + Send;
    #[allow(clippy::too_many_arguments)]
    fn derive_plan(
        &self,
        project: ProjectRecord,
        catalog: ResolverCatalog,
        template_id: TemplateId,
        template_version: String,
        display_name: String,
        description: Option<String>,
        writer_state: UnityWriterStateKind,
    ) -> impl Future<Output = Result<TemplatePlanDraft, M5TemplateError>> + Send;
    fn derive_project_id(&self, plan: &TemplatePlanRecord) -> Result<ProjectId, M5TemplateError>;
    fn publish_derive(
        &self,
        plan: TemplatePlanRecord,
        project: ProjectRecord,
    ) -> impl Future<Output = Result<PublishedTemplate, M5TemplateError>> + Send;
    fn create_project_plan(
        &self,
        template: StoredTemplateRecord,
        catalog: ResolverCatalog,
        target_parent: String,
        target_leaf: String,
        unity_version: String,
    ) -> impl Future<Output = Result<TemplatePlanDraft, M5TemplateError>> + Send;
    fn create_project_resource(
        &self,
        plan: &TemplatePlanRecord,
    ) -> Result<ResourceKey, M5TemplateError>;
    fn prepare_create_project(
        &self,
        plan: TemplatePlanRecord,
        locks: std::sync::Arc<ResourceLockCoordinator>,
    ) -> impl Future<Output = Result<Self::PreparedCreateProject, M5TemplateError>> + Send;
    fn stage_create_project(
        &self,
        operation_id: OperationId,
        prepared: Self::PreparedCreateProject,
    ) -> impl Future<Output = Result<Self::StagedCreateProject, M5TemplateError>> + Send;
    fn discard_staged_create(
        &self,
        staged: Self::StagedCreateProject,
    ) -> impl Future<Output = Result<(), M5TemplateError>> + Send;
    fn publish_create_project(
        &self,
        staged: Self::StagedCreateProject,
    ) -> impl Future<Output = Result<CreatedTemplateProject, M5TemplateError>> + Send;
}

/// Narrow writer-observation port reused by derive without granting `unity.read` to callers.
pub trait M5TemplateWriterGate: Clone + Send + Sync + 'static {
    fn observe_template_source(
        &self,
        access: &AccessContext,
        project_id: ProjectId,
    ) -> impl Future<Output = Result<UnityWriterState, M5TemplateError>> + Send;
}

impl<S, P> M5TemplateWriterGate for crate::M5UnityApplication<S, P>
where
    S: crate::M5UnityStore + M3RegistryStore,
    P: crate::M5UnityPlatform,
{
    async fn observe_template_source(
        &self,
        access: &AccessContext,
        project_id: ProjectId,
    ) -> Result<UnityWriterState, M5TemplateError> {
        self.writer_state_unchecked(access, project_id)
            .await
            .map_err(|cause| error(M5TemplateErrorCode::Internal).with_source(cause))
    }
}

/// Registry/import/export slice. Derive and create-project extend this service in-order.
#[derive(Clone)]
pub struct M5TemplateApplication<S: M5TemplateStore, A: M5TemplateAdapter, W: M5TemplateWriterGate>
{
    store: S,
    adapter: A,
    writer: W,
    locks: std::sync::Arc<ResourceLockCoordinator>,
}

impl<S, A, W> M5TemplateApplication<S, A, W>
where
    S: M5TemplateStore + M3RegistryStore + M4Store + crate::StateStore,
    A: M5TemplateAdapter,
    W: M5TemplateWriterGate,
{
    #[must_use]
    pub fn new(store: S, adapter: A, writer: W) -> Self {
        Self::with_locks(
            store,
            adapter,
            writer,
            std::sync::Arc::new(ResourceLockCoordinator::default()),
        )
    }

    #[must_use]
    pub fn with_locks(
        store: S,
        adapter: A,
        writer: W,
        locks: std::sync::Arc<ResourceLockCoordinator>,
    ) -> Self {
        Self {
            store,
            adapter,
            writer,
            locks,
        }
    }

    pub async fn ensure_builtins(
        &self,
        records: Vec<StoredTemplateRecord>,
    ) -> Result<(), M5TemplateError> {
        self.store
            .ensure_builtin_templates(PrincipalId::local_owner(), records, now_ms()?)
            .await
    }

    pub async fn recover(&self) -> Result<(), M5TemplateError> {
        let operations = self.store.recover_template_operations(now_ms()?).await?;
        for operation_id in operations {
            let recovery = self.locks.project_recovery_guard();
            let application = self.clone();
            tokio::spawn(async move {
                let _recovery = recovery;
                let _ = application.run_operation(operation_id).await;
            });
        }
        Ok(())
    }

    pub async fn list(
        &self,
        access: &AccessContext,
        cursor: Option<TemplateCursor>,
        limit: u32,
    ) -> Result<TemplatePage, M5TemplateError> {
        require(access, crate::Permission::TemplatesRead)?;
        if !(1..=1_000).contains(&limit) {
            return Err(error(M5TemplateErrorCode::InvalidInput));
        }
        let stored = self
            .store
            .list_templates(access.principal().clone(), cursor, limit)
            .await?;
        let mut templates = Vec::with_capacity(stored.len());
        for record in stored {
            templates.push(self.adapter.present_record(record).await?);
        }
        let has_more = templates.len() > limit as usize;
        if has_more {
            templates.pop();
        }
        let next_cursor = has_more.then(|| {
            let last = templates
                .last()
                .expect("non-empty Template page with extra row");
            TemplateCursor {
                updated_at_ms: last.updated_at_ms,
                template_id: last.template_id,
            }
        });
        Ok(TemplatePage {
            templates,
            next_cursor,
        })
    }

    pub async fn get(
        &self,
        access: &AccessContext,
        template_id: TemplateId,
    ) -> Result<TemplateRecord, M5TemplateError> {
        require(access, crate::Permission::TemplatesRead)?;
        let record = self
            .store
            .get_template(access.principal().clone(), template_id)
            .await?;
        self.adapter.present_record(record).await
    }

    pub async fn inspect(
        &self,
        access: &AccessContext,
        source_path: String,
    ) -> Result<TemplateBundleEvidence, M5TemplateError> {
        require(access, crate::Permission::TemplatesRead)?;
        self.adapter.inspect_bundle(source_path).await
    }

    pub async fn plan_import(
        &self,
        access: &AccessContext,
        source_path: String,
        override_existing: bool,
        expected_revision: Option<Revision>,
    ) -> Result<TemplatePlanRecord, M5TemplateError> {
        require_local_owner(access, crate::Permission::TemplatesManage)?;
        let evidence = crate::log_execution_step(
            "templates.import",
            "inspect_bundle",
            Some(crate::log_path("template", source_path.clone())),
            self.adapter.inspect_bundle(source_path),
            |error| error.path().map(str::to_owned),
        )
        .await?;
        let existing = match self
            .store
            .get_template(access.principal().clone(), evidence.template_id)
            .await
        {
            Ok(value) => Some(value),
            Err(value) if value.code() == M5TemplateErrorCode::TemplateNotFound => None,
            Err(value) => return Err(value),
        };
        if existing
            .as_ref()
            .is_some_and(|value| value.source_kind == TemplateSourceKind::Builtin)
        {
            return Err(error(M5TemplateErrorCode::TemplateImmutable));
        }
        let draft = self
            .adapter
            .import_plan(evidence, existing, override_existing, expected_revision)
            .await?;
        self.store
            .create_template_plan(access.principal().clone(), draft, now_ms()?)
            .await
    }

    pub async fn apply_import(
        &self,
        access: &AccessContext,
        plan_id: PlanId,
        key: IdempotencyKey,
    ) -> Result<TemplateApplyOutcome, M5TemplateError> {
        require_local_owner(access, crate::Permission::TemplatesManage)?;
        let plan = self
            .store
            .get_template_plan(access.principal().clone(), plan_id)
            .await?;
        if plan.kind != TemplatePlanKind::Import {
            return Err(error(M5TemplateErrorCode::InvalidInput));
        }
        let outcome = self
            .store
            .accept_template_plan(access.principal().clone(), plan_id, key, now_ms()?)
            .await?;
        if outcome.schedule {
            self.schedule(outcome.operation_id);
        }
        Ok(outcome)
    }

    pub async fn export(
        &self,
        access: &AccessContext,
        template_id: TemplateId,
        expected_revision: Revision,
        target_path: String,
    ) -> Result<(), M5TemplateError> {
        require_local_owner(access, crate::Permission::TemplatesRead)?;
        let template = self
            .store
            .get_template(access.principal().clone(), template_id)
            .await?;
        if template.revision != expected_revision {
            return Err(error(M5TemplateErrorCode::TemplateRevisionConflict));
        }
        crate::log_execution_step(
            "templates.export",
            "write_bundle",
            Some(crate::log_path("template", target_path.clone())),
            self.adapter.export_bundle(template, target_path),
            |error| error.path().map(str::to_owned),
        )
        .await?;
        crate::flush_log_steps().await;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn plan_derive(
        &self,
        access: &AccessContext,
        project_id: ProjectId,
        expected_project_revision: Revision,
        template_id: TemplateId,
        template_version: String,
        display_name: String,
        description: Option<String>,
    ) -> Result<TemplatePlanRecord, M5TemplateError> {
        require_local_owner(access, crate::Permission::TemplatesManage)?;
        require(access, crate::Permission::ProjectsRead)?;
        let project = self
            .store
            .get_project(access.principal().clone(), project_id)
            .await
            .map_err(|cause| error(M5TemplateErrorCode::ProjectNotRegistered).with_source(cause))?;
        if project.revision != expected_project_revision {
            return Err(error(M5TemplateErrorCode::TemplateRevisionConflict));
        }
        if self
            .store
            .get_template(access.principal().clone(), template_id)
            .await
            .is_ok()
        {
            return Err(error(M5TemplateErrorCode::TemplateConflict));
        }
        let writer = self
            .writer
            .observe_template_source(access, project_id)
            .await?;
        if writer.state == UnityWriterStateKind::RunningConfirmed {
            return Err(error(M5TemplateErrorCode::ProjectRunning));
        }
        let catalog = self
            .store
            .resolver_catalog(access.principal().clone(), false)
            .await
            .map_err(|cause| error(M5TemplateErrorCode::Internal).with_source(cause))?;
        if !catalog.complete {
            return Err(error(M5TemplateErrorCode::TemplatePlanStale));
        }
        let target = crate::log_path("project", project.observation.root_path.clone());
        let draft = crate::log_execution_step(
            "templates.derive",
            "inspect_project",
            Some(target),
            self.adapter.derive_plan(
                project,
                catalog,
                template_id,
                template_version,
                display_name,
                description,
                writer.state,
            ),
            |error| error.path().map(str::to_owned),
        )
        .await?;
        self.store
            .create_template_plan(access.principal().clone(), draft, now_ms()?)
            .await
    }

    pub async fn apply_derive(
        &self,
        access: &AccessContext,
        plan_id: PlanId,
        key: IdempotencyKey,
    ) -> Result<TemplateApplyOutcome, M5TemplateError> {
        require_local_owner(access, crate::Permission::TemplatesManage)?;
        require(access, crate::Permission::ProjectsRead)?;
        let plan = self
            .store
            .get_template_plan(access.principal().clone(), plan_id)
            .await?;
        if plan.kind != TemplatePlanKind::Derive {
            return Err(error(M5TemplateErrorCode::InvalidInput));
        }
        let outcome = self
            .store
            .accept_template_plan(access.principal().clone(), plan_id, key, now_ms()?)
            .await?;
        if outcome.schedule {
            self.schedule(outcome.operation_id);
        }
        Ok(outcome)
    }

    pub async fn plan_create_project(
        &self,
        access: &AccessContext,
        template_id: TemplateId,
        expected_template_revision: Revision,
        target_parent: String,
        target_leaf: String,
        unity_version: String,
    ) -> Result<TemplatePlanRecord, M5TemplateError> {
        require_local_owner(access, crate::Permission::TemplatesRead)?;
        require(access, crate::Permission::ProjectsCreate)?;
        require(access, crate::Permission::PackagesRead)?;
        require(access, crate::Permission::RepositoriesRead)?;
        let template = self
            .store
            .get_template(access.principal().clone(), template_id)
            .await?;
        if template.revision != expected_template_revision {
            return Err(error(M5TemplateErrorCode::TemplateRevisionConflict));
        }
        let catalog = self
            .store
            .resolver_catalog(access.principal().clone(), false)
            .await
            .map_err(|cause| error(M5TemplateErrorCode::StoreUnavailable).with_source(cause))?;
        if !catalog.complete {
            return Err(error(M5TemplateErrorCode::TemplatePlanStale));
        }
        let path = std::path::Path::new(&target_parent)
            .join(&target_leaf)
            .to_string_lossy()
            .into_owned();
        let draft = crate::log_execution_step(
            "templates.create-project",
            "check_destination",
            Some(crate::log_path("project", path)),
            self.adapter.create_project_plan(
                template,
                catalog,
                target_parent,
                target_leaf,
                unity_version,
            ),
            |error| error.path().map(str::to_owned),
        )
        .await?;
        self.store
            .create_template_plan(access.principal().clone(), draft, now_ms()?)
            .await
    }

    pub async fn apply_create_project(
        &self,
        access: &AccessContext,
        plan_id: PlanId,
        key: IdempotencyKey,
    ) -> Result<TemplateApplyOutcome, M5TemplateError> {
        require_local_owner(access, crate::Permission::TemplatesRead)?;
        require(access, crate::Permission::ProjectsCreate)?;
        require(access, crate::Permission::PackagesRead)?;
        require(access, crate::Permission::RepositoriesRead)?;
        require(access, crate::Permission::PackagesManage)?;
        let plan = self
            .store
            .get_template_plan(access.principal().clone(), plan_id)
            .await?;
        if plan.kind != TemplatePlanKind::CreateProject {
            return Err(error(M5TemplateErrorCode::InvalidInput));
        }
        if plan.state == TemplatePlanState::Unapplied {
            // Validate the frozen authority before acceptance changes its state.
            // Accepted historical work still recovers from its original evidence.
            self.adapter.create_project_resource(&plan)?;
        }
        let outcome = self
            .store
            .accept_template_plan(access.principal().clone(), plan_id, key, now_ms()?)
            .await?;
        if outcome.schedule {
            self.schedule(outcome.operation_id);
        }
        Ok(outcome)
    }

    pub async fn set_favorite(
        &self,
        access: &AccessContext,
        template_id: TemplateId,
        favorite: bool,
        expected_revision: Revision,
        key: IdempotencyKey,
    ) -> Result<(TemplateRecord, bool), M5TemplateError> {
        require(access, crate::Permission::TemplatesManage)?;
        let (record, replayed) = self
            .store
            .set_template_favorite(
                access.principal().clone(),
                template_id,
                favorite,
                expected_revision,
                key,
                now_ms()?,
            )
            .await?;
        Ok((self.adapter.present_record(record).await?, replayed))
    }

    pub async fn remove(
        &self,
        access: &AccessContext,
        template_id: TemplateId,
        expected_revision: Revision,
        key: IdempotencyKey,
    ) -> Result<(bool, bool), M5TemplateError> {
        require(access, crate::Permission::TemplatesManage)?;
        self.store
            .remove_template(
                access.principal().clone(),
                template_id,
                expected_revision,
                key,
                now_ms()?,
            )
            .await
    }

    fn schedule(&self, operation_id: OperationId) {
        let application = self.clone();
        tokio::spawn(async move {
            let _ = application.run_operation(operation_id).await;
        });
    }

    async fn run_operation(&self, operation_id: OperationId) -> Result<(), M5TemplateError> {
        if self
            .store
            .cancellation_requested(operation_id)
            .await
            .map_err(|cause| error(M5TemplateErrorCode::StoreUnavailable).with_source(cause))?
        {
            self.store
                .finish_cancelled(operation_id, now_ms()?)
                .await
                .map_err(|cause| error(M5TemplateErrorCode::StoreUnavailable).with_source(cause))?;
            return Ok(());
        }
        let plan = self
            .store
            .begin_template_apply(operation_id, now_ms()?)
            .await?;
        let result = match plan.kind {
            TemplatePlanKind::Import => self.adapter.publish_import(plan).await,
            TemplatePlanKind::Derive => self.run_derive(plan).await,
            TemplatePlanKind::CreateProject => {
                return self.run_create_project(operation_id, plan).await;
            }
        };
        match result {
            Ok(template) => {
                if self
                    .store
                    .cancellation_requested(operation_id)
                    .await
                    .map_err(|cause| {
                        error(M5TemplateErrorCode::StoreUnavailable).with_source(cause)
                    })?
                {
                    self.store
                        .finish_cancelled(operation_id, now_ms()?)
                        .await
                        .map_err(|cause| {
                            error(M5TemplateErrorCode::StoreUnavailable).with_source(cause)
                        })?;
                    return Ok(());
                }
                self.store
                    .complete_template_apply(operation_id, template, now_ms()?)
                    .await
            }
            Err(source) => {
                let code = template_error_name(source.code()).to_owned();
                let (source, diagnostic_id) = source.ensure_diagnostic_id();
                crate::with_operation_log_failure(
                    operation_id,
                    crate::operation_log_error(&source, source.path()),
                    self.store
                        .fail_template_apply(operation_id, code, diagnostic_id, now_ms()?),
                )
                .await?;
                Err(source)
            }
        }
    }

    async fn run_create_project(
        &self,
        operation_id: OperationId,
        plan: TemplatePlanRecord,
    ) -> Result<(), M5TemplateError> {
        let result = self.run_create_project_inner(operation_id, plan).await;
        if let Err(source) = result {
            let (source, diagnostic_id) = source.ensure_diagnostic_id();
            let code = template_error_name(source.code()).to_owned();
            self.store
                .fail_template_apply(operation_id, code, diagnostic_id, now_ms()?)
                .await?;
            return Err(source);
        }
        Ok(())
    }

    async fn run_create_project_inner(
        &self,
        operation_id: OperationId,
        plan: TemplatePlanRecord,
    ) -> Result<(), M5TemplateError> {
        let resource = self.adapter.create_project_resource(&plan)?;
        let prepared = self
            .adapter
            .prepare_create_project(plan, std::sync::Arc::clone(&self.locks))
            .await?;
        if self
            .store
            .cancellation_requested(operation_id)
            .await
            .map_err(|cause| error(M5TemplateErrorCode::StoreUnavailable).with_source(cause))?
        {
            self.store
                .finish_cancelled(operation_id, now_ms()?)
                .await
                .map_err(|cause| error(M5TemplateErrorCode::StoreUnavailable).with_source(cause))?;
            return Ok(());
        }
        let _guard = self.locks.acquire(vec![resource]).await;
        let staged = self
            .adapter
            .stage_create_project(operation_id, prepared)
            .await?;
        self.store
            .record_template_checkpoint(operation_id, 2, "staging_complete".to_owned(), now_ms()?)
            .await?;
        if self
            .store
            .cancellation_requested(operation_id)
            .await
            .map_err(|cause| error(M5TemplateErrorCode::StoreUnavailable).with_source(cause))?
        {
            self.adapter.discard_staged_create(staged).await?;
            self.store
                .finish_cancelled(operation_id, now_ms()?)
                .await
                .map_err(|cause| error(M5TemplateErrorCode::StoreUnavailable).with_source(cause))?;
            return Ok(());
        }
        self.store
            .record_template_checkpoint(
                operation_id,
                3,
                "target_publish_intent".to_owned(),
                now_ms()?,
            )
            .await?;
        let project = self.adapter.publish_create_project(staged).await?;
        self.store
            .record_template_checkpoint(operation_id, 4, "target_published".to_owned(), now_ms()?)
            .await?;
        self.store
            .record_template_checkpoint(
                operation_id,
                5,
                "project_registry_commit_intent".to_owned(),
                now_ms()?,
            )
            .await?;
        self.store
            .complete_template_project_create(operation_id, project, now_ms()?)
            .await
    }

    async fn run_derive(
        &self,
        plan: TemplatePlanRecord,
    ) -> Result<PublishedTemplate, M5TemplateError> {
        let project_id = self.adapter.derive_project_id(&plan)?;
        let _guard = self
            .locks
            .acquire(vec![ResourceKey::Project(project_id)])
            .await;
        let project = self
            .store
            .get_project(plan.owner.clone(), project_id)
            .await
            .map_err(|cause| error(M5TemplateErrorCode::ProjectNotRegistered).with_source(cause))?;
        let access = AccessContext::new(plan.owner.clone(), [crate::Permission::ProjectsRead]);
        let writer = self
            .writer
            .observe_template_source(&access, project_id)
            .await?;
        if writer.state == UnityWriterStateKind::RunningConfirmed {
            return Err(error(M5TemplateErrorCode::ProjectRunning));
        }
        self.adapter.publish_derive(plan, project).await
    }
}

fn require(access: &AccessContext, permission: crate::Permission) -> Result<(), M5TemplateError> {
    access
        .require(permission)
        .map_err(|cause| error(M5TemplateErrorCode::PermissionDenied).with_source(cause))
}

fn require_local_owner(
    access: &AccessContext,
    permission: crate::Permission,
) -> Result<(), M5TemplateError> {
    require(access, permission)?;
    if access.principal().as_str() != PrincipalId::LOCAL_OWNER {
        return Err(error(M5TemplateErrorCode::PermissionDenied));
    }
    Ok(())
}

fn now_ms() -> Result<u64, M5TemplateError> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|cause| error(M5TemplateErrorCode::Internal).with_source(cause))
        .and_then(|duration| {
            u64::try_from(duration.as_millis())
                .map_err(|cause| error(M5TemplateErrorCode::Internal).with_source(cause))
        })
}

/// Stable public error code mapping shared with the RPC adapter.
#[must_use]
pub const fn template_error_name(code: M5TemplateErrorCode) -> &'static str {
    match code {
        M5TemplateErrorCode::InvalidInput => "invalid_request",
        M5TemplateErrorCode::PermissionDenied => "permission_denied",
        M5TemplateErrorCode::TemplateNotFound => "template_not_found",
        M5TemplateErrorCode::TemplateImmutable => "template_builtin_immutable",
        M5TemplateErrorCode::TemplateConflict => "template_conflict",
        M5TemplateErrorCode::TemplateRevisionConflict => "revision_conflict",
        M5TemplateErrorCode::TemplatePlanNotFound => "template_plan_stale",
        M5TemplateErrorCode::TemplatePlanStale => "template_plan_stale",
        M5TemplateErrorCode::TemplateBundleInvalid => "template_bundle_invalid",
        M5TemplateErrorCode::TemplateBundleChanged => "template_digest_mismatch",
        M5TemplateErrorCode::TemplateObjectMissing => "template_payload_unavailable",
        M5TemplateErrorCode::DependencyUnsatisfied => "template_dependency_unsatisfied",
        M5TemplateErrorCode::TemplateTargetExists => "template_target_exists",
        M5TemplateErrorCode::ProjectNotRegistered => "project_not_registered",
        M5TemplateErrorCode::ProjectRunning => "unity_project_running",
        M5TemplateErrorCode::ProjectChangedDuringTemplateCreate => {
            "project_changed_during_template_create"
        }
        M5TemplateErrorCode::StoreUnavailable => "store_unavailable",
        M5TemplateErrorCode::Internal => "internal_error",
    }
}

fn error(code: M5TemplateErrorCode) -> M5TemplateError {
    M5TemplateError::new(code)
}
