//! Application use cases and orchestration boundaries.
//!
//! Transport adapters call this layer rather than inventing business state in
//! the daemon, CLI, or other entry points.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::future::Future;
use std::sync::{Arc, Weak};

pub use alcomd_domain::{
    BackupId, IdempotencyKey, OperationId, OperationState, Permission, PlanId, PrincipalId,
    ProjectId, RepositoryId, ResourceKey, Revision, TemplateId, UnityInstallationId, UnityLaunchId,
    UserPackageId,
};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, OwnedMutexGuard};

mod m4;
mod m5;
mod m5_backup;
mod m5_template;
mod m6;
mod m7;
mod m7_copy;
mod m7_delete;
mod m7_official;
mod m7_user_packages;

pub use m4::*;
pub use m5::*;
pub use m5_backup::*;
pub use m5_template::*;
pub use m6::*;
pub use m7::*;
pub use m7_copy::*;
pub use m7_delete::*;
pub use m7_official::*;
pub use m7_user_packages::*;

/// Minimal truthful daemon status for the M1 read-only vertical slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SystemStatus {
    state: SystemState,
}

impl SystemStatus {
    /// Returns the state exposed by the running daemon.
    #[must_use]
    pub const fn state(self) -> SystemState {
        self.state
    }
}

/// States that exist in the M1 system-status use case.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SystemState {
    /// The daemon owns its endpoint and can serve M1 queries.
    Ready,
}

impl SystemState {
    /// Stable transport-neutral representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
        }
    }
}

/// Executes the M1 `system.status` query.
#[must_use]
pub const fn system_status() -> SystemStatus {
    SystemStatus {
        state: SystemState::Ready,
    }
}

/// Explicit project discovery mode frozen for M3.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ProjectDiscoveryMode {
    ExactRoot,
    SearchParents,
}

/// Frozen marker-based M3 project type.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectType {
    Avatars,
    Worlds,
    VpmStarter,
    UpmAvatars,
    UpmWorlds,
    UpmStarter,
    LegacySdk2,
    LegacyWorlds,
    LegacyAvatars,
    Unknown,
}

/// State of one optional project manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ManifestState {
    Missing,
    Valid,
}

/// Raw dependency identity/value pair with no version semantics.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DependencyIdentity {
    pub package_id: String,
    pub value: String,
}

/// Bounded parse issue safe for public DTOs and state.db.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ReadIssue {
    pub code: String,
    pub component: String,
    pub item: String,
    pub line: Option<u64>,
    pub column: Option<u64>,
}

/// Normalized project observation produced without modifying the project.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectObservation {
    pub root_path: String,
    pub path_identity_key: Vec<u8>,
    pub project_type: ProjectType,
    pub unity_version: String,
    pub unity_revision: Option<String>,
    pub vpm_manifest: ManifestState,
    pub upm_manifest: ManifestState,
    pub direct_dependencies: Vec<DependencyIdentity>,
    pub locked_dependencies: Vec<DependencyIdentity>,
    pub issues: Vec<ReadIssue>,
    pub observed_at_ms: u64,
}

/// Durable registered project record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectRecord {
    pub project_id: ProjectId,
    pub observation: ProjectObservation,
    pub revision: Revision,
    pub registered_at_ms: u64,
    /// User-controlled registry metadata; absent in durable v10 JSON means false.
    #[serde(default)]
    pub favorite: bool,
}

/// Local file or anonymous remote repository source.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RepositorySource {
    Local { path: String },
    Remote { url: String },
}

/// Conditional HTTP validators stored separately from semantic state.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct RepositoryValidators {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

/// Raw package/version display row; no SemVer meaning is implied.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RepositoryPackageVersion {
    pub package_id: String,
    pub version: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub yanked: bool,
    pub unity: Option<String>,
    /// Sanitized optional presentation links.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub links: Option<RepositoryPackageLinks>,
    /// Strict M4 resolver metadata; absent for legacy/raw snapshots.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolver: Option<ResolverPackageMetadata>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RepositoryPackageLinks {
    pub documentation: Option<String>,
    pub changelog: Option<String>,
}

/// Resolver-ready package metadata persisted only after a complete strict parse.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ResolverPackageMetadata {
    pub semantic_version: String,
    pub author_name: String,
    pub author_email: String,
    pub artifact_url: String,
    pub zip_sha256: String,
    pub unity_release: Option<String>,
    pub dependencies_json: String,
    pub manifest_fingerprint: Vec<u8>,
    pub legacy_metadata_present: bool,
}

/// Completely parsed repository observation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RepositoryObservation {
    pub source: RepositorySource,
    pub source_identity_key: Vec<u8>,
    pub declared_id: Option<String>,
    pub name: Option<String>,
    pub declared_url: Option<String>,
    pub issues: Vec<ReadIssue>,
    pub packages: Vec<RepositoryPackageVersion>,
    pub validators: RepositoryValidators,
    pub refreshed_at_ms: u64,
}

/// Result of a conditional repository read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RepositoryReadOutcome {
    Fresh(RepositoryObservation),
    NotModified(RepositoryValidators),
}

/// Durable registered repository record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RepositoryRecord {
    pub repository_id: RepositoryId,
    pub observation: RepositoryObservation,
    pub revision: Revision,
    pub registered_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RegistryCursor<I> {
    pub registered_at_ms: u64,
    pub id: I,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PackageCursor {
    pub package_id: String,
    pub version: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectPage {
    pub projects: Vec<ProjectRecord>,
    pub next_cursor: Option<RegistryCursor<ProjectId>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RepositoryPage {
    pub repositories: Vec<RepositoryRecord>,
    pub next_cursor: Option<RegistryCursor<RepositoryId>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PackagePage {
    pub packages: Vec<RepositoryPackageVersion>,
    pub next_cursor: Option<PackageCursor>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SyncWrite<T> {
    pub value: T,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UnregisterResult<I> {
    pub id: I,
    pub revision: Revision,
    pub replayed: bool,
}

/// Stable M3 failure code; messages and technical sources stay adapter-private.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum M3ErrorCode {
    PathEncodingUnsupported,
    ProjectNotFound,
    ProjectNotRegistered,
    ProjectAlreadyRegistered,
    ProjectInaccessible,
    ProjectVersionMissing,
    ProjectVersionInvalid,
    ProjectManifestInvalid,
    RepositoryNotFound,
    RepositoryNotRegistered,
    RepositoryAlreadyRegistered,
    RepositorySourceInvalid,
    RepositoryInaccessible,
    RepositoryUnavailable,
    RepositoryDocumentInvalid,
    RepositoryDocumentTooLarge,
    RepositoryCredentialsUnsupported,
    RevisionConflict,
    IdempotencyConflict,
    PermissionDenied,
    StoreUnavailable,
    Internal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct M3Error {
    code: M3ErrorCode,
}

impl M3Error {
    #[must_use]
    pub const fn new(code: M3ErrorCode) -> Self {
        Self { code }
    }

    #[must_use]
    pub const fn code(&self) -> M3ErrorCode {
        self.code
    }
}

impl fmt::Display for M3Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "M3 request failed: {:?}", self.code)
    }
}

impl std::error::Error for M3Error {}

/// External read adapter. Implementations perform bounded I/O but never mutate sources.
pub trait M3ReadAdapter: Clone + Send + Sync + 'static {
    fn inspect_project(
        &self,
        path: String,
        mode: ProjectDiscoveryMode,
    ) -> impl Future<Output = Result<ProjectObservation, M3Error>> + Send;

    fn inspect_repository(
        &self,
        source: RepositorySource,
        validators: Option<RepositoryValidators>,
    ) -> impl Future<Output = Result<RepositoryReadOutcome, M3Error>> + Send;
}

/// M3 persistence port implemented only by the authoritative state store.
pub trait M3RegistryStore: Clone + Send + Sync + 'static {
    fn register_project(
        &self,
        owner: PrincipalId,
        observation: ProjectObservation,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<SyncWrite<ProjectRecord>, M3Error>> + Send;
    fn get_project(
        &self,
        owner: PrincipalId,
        id: ProjectId,
    ) -> impl Future<Output = Result<ProjectRecord, M3Error>> + Send;
    fn list_projects(
        &self,
        owner: PrincipalId,
        cursor: Option<RegistryCursor<ProjectId>>,
        limit: u32,
    ) -> impl Future<Output = Result<ProjectPage, M3Error>> + Send;
    fn refresh_project(
        &self,
        owner: PrincipalId,
        id: ProjectId,
        expected: Revision,
        observation: ProjectObservation,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<SyncWrite<ProjectRecord>, M3Error>> + Send;
    fn set_project_favorite(
        &self,
        owner: PrincipalId,
        id: ProjectId,
        favorite: bool,
        expected: Revision,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<SyncWrite<ProjectRecord>, M3Error>> + Send;
    fn unregister_project(
        &self,
        owner: PrincipalId,
        id: ProjectId,
        expected: Revision,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<UnregisterResult<ProjectId>, M3Error>> + Send;
    fn register_repository(
        &self,
        owner: PrincipalId,
        observation: RepositoryObservation,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<SyncWrite<RepositoryRecord>, M3Error>> + Send;
    fn get_repository(
        &self,
        owner: PrincipalId,
        id: RepositoryId,
    ) -> impl Future<Output = Result<RepositoryRecord, M3Error>> + Send;
    fn list_repositories(
        &self,
        owner: PrincipalId,
        cursor: Option<RegistryCursor<RepositoryId>>,
        limit: u32,
    ) -> impl Future<Output = Result<RepositoryPage, M3Error>> + Send;
    fn list_repository_packages(
        &self,
        owner: PrincipalId,
        id: RepositoryId,
        cursor: Option<PackageCursor>,
        limit: u32,
    ) -> impl Future<Output = Result<PackagePage, M3Error>> + Send;
    fn refresh_repository(
        &self,
        owner: PrincipalId,
        id: RepositoryId,
        expected: Revision,
        observation: RepositoryObservation,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<SyncWrite<RepositoryRecord>, M3Error>> + Send;
    fn update_repository_validators(
        &self,
        owner: PrincipalId,
        id: RepositoryId,
        expected: Revision,
        validators: RepositoryValidators,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<SyncWrite<RepositoryRecord>, M3Error>> + Send;
    fn unregister_repository(
        &self,
        owner: PrincipalId,
        id: RepositoryId,
        expected: Revision,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<UnregisterResult<RepositoryId>, M3Error>> + Send;
}

/// Versioned, typed, non-sensitive canonical fingerprint for `state.check`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StateCheckFingerprintV1;

impl StateCheckFingerprintV1 {
    /// Returns the exact permanent M2 fingerprint representation.
    #[must_use]
    pub const fn canonical_json(self) -> &'static str {
        r#"{"version":1}"#
    }
}

/// Versioned, typed, non-sensitive canonical fingerprint for `operations.cancel`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CancelOperationFingerprintV1 {
    operation_id: OperationId,
    expected_revision: Revision,
}

impl CancelOperationFingerprintV1 {
    /// Creates the typed fingerprint DTO after public input validation.
    #[must_use]
    pub const fn new(operation_id: OperationId, expected_revision: Revision) -> Self {
        Self {
            operation_id,
            expected_revision,
        }
    }

    /// Serializes fields in the frozen order without a randomized process hash.
    #[must_use]
    pub fn canonical_json(&self) -> String {
        format!(
            r#"{{"expectedRevision":{},"operationId":"{}","version":1}}"#,
            self.expected_revision.get(),
            self.operation_id
        )
    }
}

/// Safe classification returned by one read-only database check phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckClassification {
    /// The check reported no issue.
    Ok,
    /// One or more issues were found within the bounded result scan.
    IssuesDetected,
    /// The bounded scan stopped before consuming all issue rows.
    IssuesTruncated,
}

/// Safe public result of the M2 state integrity check.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateCheckResult {
    /// `PRAGMA integrity_check` classification.
    pub integrity: CheckClassification,
    /// `PRAGMA foreign_key_check` classification.
    pub foreign_keys: CheckClassification,
}

/// Transport-neutral persisted Operation record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OperationRecord {
    /// Stable identifier.
    pub operation_id: OperationId,
    /// Stable kind; M2 only creates `state.check`.
    pub kind: String,
    /// Current lifecycle state.
    pub state: OperationState,
    /// Current positive revision.
    pub revision: Revision,
    /// Owning Principal.
    pub owner: PrincipalId,
    /// Whether cooperative cancellation was requested.
    pub cancel_requested: bool,
    /// Creation time in Unix epoch milliseconds.
    pub created_at_ms: u64,
    /// Last public update time in Unix epoch milliseconds.
    pub updated_at_ms: u64,
    /// Optional start time.
    pub started_at_ms: Option<u64>,
    /// Optional completion time.
    pub completed_at_ms: Option<u64>,
    /// Safe method-specific result JSON.
    pub result_json: Option<String>,
    /// Stable public error code.
    pub error_code: Option<String>,
    /// Safe diagnostic correlation identifier.
    pub diagnostic_id: Option<String>,
    /// Latest durable package filesystem phase, absent for non-package Operations.
    pub progress_phase: Option<FilesystemPhase>,
}

/// Opaque Operation pagination cursor used inside the application layer.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OperationCursor {
    /// Creation time of the previous page's final record.
    pub created_at_ms: u64,
    /// Identifier of the previous page's final record.
    pub operation_id: OperationId,
}

/// One page of visible Operations.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OperationPage {
    /// Records in stable descending tuple order.
    pub operations: Vec<OperationRecord>,
    /// Final record cursor, absent for an empty page.
    pub next_cursor: Option<OperationCursor>,
}

/// Transport-neutral persisted Event record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EventRecord {
    /// Positive global sequence.
    pub sequence: u64,
    /// Stable Event UUID string.
    pub event_id: String,
    /// Stable event kind.
    pub kind: String,
    /// Aggregate kind.
    pub aggregate_kind: String,
    /// Aggregate identifier.
    pub aggregate_id: String,
    /// Post-commit aggregate revision.
    pub aggregate_revision: Revision,
    /// Event time in Unix epoch milliseconds.
    pub occurred_at_ms: u64,
    /// Safe Event payload JSON.
    pub payload_json: String,
}

/// One page of visible Events.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EventPage {
    /// Records in ascending global sequence order.
    pub events: Vec<EventRecord>,
    /// Last returned sequence, or the input cursor for an empty page.
    pub next_sequence: u64,
}

/// Result of atomically creating or replaying `state.check`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CreateOperationOutcome {
    /// Stable Operation identifier.
    pub operation_id: OperationId,
    /// Whether an existing idempotent result was replayed.
    pub replayed: bool,
    /// Whether a worker must be scheduled for this request.
    pub schedule: bool,
}

/// Store failure category safe for application decisions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreErrorKind {
    /// The store cannot currently complete the request.
    Unavailable,
    /// The Operation does not exist or is not visible.
    OperationNotFound,
    /// The expected revision is stale.
    RevisionConflict,
    /// The idempotency key conflicts with a different request fingerprint.
    IdempotencyConflict,
    /// The current state cannot accept cancellation.
    OperationNotCancellable,
    /// Persisted state violates the frozen contract.
    CorruptState,
}

/// Safe persistence-port error without SQLite text or local paths.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoreError {
    kind: StoreErrorKind,
}

impl StoreError {
    /// Creates a safe store error category.
    #[must_use]
    pub const fn new(kind: StoreErrorKind) -> Self {
        Self { kind }
    }

    /// Returns the stable application-visible category.
    #[must_use]
    pub const fn kind(self) -> StoreErrorKind {
        self.kind
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("state store request failed")
    }
}

impl std::error::Error for StoreError {}

/// Narrow asynchronous persistence port required by the M2 vertical slice.
pub trait StateStore: Clone + Send + Sync + 'static {
    /// Creates or replays the sole M2 Operation kind atomically.
    fn create_state_check(
        &self,
        owner: PrincipalId,
        idempotency_key: IdempotencyKey,
        created_at_ms: u64,
    ) -> impl Future<Output = Result<CreateOperationOutcome, StoreError>> + Send;

    /// Loads one owned Operation.
    fn get_operation(
        &self,
        owner: PrincipalId,
        operation_id: OperationId,
    ) -> impl Future<Output = Result<OperationRecord, StoreError>> + Send;

    /// Lists owned Operations with a strict descending tuple cursor.
    fn list_operations(
        &self,
        owner: PrincipalId,
        cursor: Option<OperationCursor>,
        limit: u32,
    ) -> impl Future<Output = Result<OperationPage, StoreError>> + Send;

    /// Persists an idempotent cooperative cancellation request.
    fn cancel_operation(
        &self,
        owner: PrincipalId,
        operation_id: OperationId,
        expected_revision: Revision,
        idempotency_key: IdempotencyKey,
        updated_at_ms: u64,
    ) -> impl Future<Output = Result<(OperationRecord, bool), StoreError>> + Send;

    /// Lists visible durable Events after an exclusive sequence cursor.
    fn list_events(
        &self,
        owner: PrincipalId,
        after_sequence: u64,
        limit: u32,
    ) -> impl Future<Output = Result<EventPage, StoreError>> + Send;

    /// Starts or resumes one queued/recovering state check.
    fn begin_state_check(
        &self,
        operation_id: OperationId,
        updated_at_ms: u64,
    ) -> impl Future<Output = Result<OperationRecord, StoreError>> + Send;

    /// Runs only the bounded integrity-check phase.
    fn check_integrity(
        &self,
    ) -> impl Future<Output = Result<CheckClassification, StoreError>> + Send;

    /// Runs only the bounded foreign-key-check phase.
    fn check_foreign_keys(
        &self,
    ) -> impl Future<Output = Result<CheckClassification, StoreError>> + Send;

    /// Reloads cancellation state between check phases.
    fn cancellation_requested(
        &self,
        operation_id: OperationId,
    ) -> impl Future<Output = Result<bool, StoreError>> + Send;

    /// Completes an Operation and commits its terminal Event atomically.
    fn finish_state_check(
        &self,
        operation_id: OperationId,
        result: StateCheckResult,
        completed_at_ms: u64,
    ) -> impl Future<Output = Result<OperationRecord, StoreError>> + Send;

    /// Completes an Operation cooperatively as cancelled.
    fn finish_cancelled(
        &self,
        operation_id: OperationId,
        completed_at_ms: u64,
    ) -> impl Future<Output = Result<OperationRecord, StoreError>> + Send;

    /// Completes an Operation with a stable safe error and diagnostic ID.
    fn finish_failed(
        &self,
        operation_id: OperationId,
        error_code: String,
        diagnostic_id: String,
        completed_at_ms: u64,
    ) -> impl Future<Output = Result<OperationRecord, StoreError>> + Send;

    /// Applies frozen restart transitions and returns work that must be scheduled.
    fn recover(
        &self,
        recovered_at_ms: u64,
    ) -> impl Future<Output = Result<Vec<OperationId>, StoreError>> + Send;
}

/// Principal and granted permissions supplied independently of RPC client metadata.
#[derive(Clone, Debug)]
pub struct AccessContext {
    principal: PrincipalId,
    permissions: HashSet<Permission>,
    project_read_scopes: AccessScopes,
    extension_ui_scopes: AccessScopes,
}

#[derive(Clone, Debug, Default)]
enum AccessScopes {
    #[default]
    None,
    All,
    Exact(HashSet<String>),
}

impl AccessContext {
    /// Creates an explicit access context.
    #[must_use]
    pub fn new(principal: PrincipalId, permissions: impl IntoIterator<Item = Permission>) -> Self {
        Self {
            principal,
            permissions: permissions.into_iter().collect(),
            project_read_scopes: AccessScopes::None,
            extension_ui_scopes: AccessScopes::None,
        }
    }

    /// Creates the M2 built-in official-client context.
    #[must_use]
    pub fn local_owner() -> Self {
        let mut access = Self::new(
            PrincipalId::local_owner(),
            [
                Permission::StateCheck,
                Permission::OperationsRead,
                Permission::OperationsCancel,
                Permission::EventsRead,
                Permission::ActivityRead,
                Permission::DiagnosticsRead,
                Permission::SettingsRead,
                Permission::SettingsManage,
                Permission::ProjectsRead,
                Permission::ProjectsManage,
                Permission::RepositoriesRead,
                Permission::RepositoriesManage,
                Permission::PackagesRead,
                Permission::PackagesManage,
                Permission::UnityRead,
                Permission::UnityManage,
                Permission::UnityLaunch,
                Permission::ProjectsCreate,
                Permission::ProjectsDelete,
                Permission::TemplatesRead,
                Permission::TemplatesManage,
                Permission::BackupsRead,
                Permission::BackupsManage,
                Permission::ExtensionsRead,
                Permission::ExtensionsManage,
                Permission::ExtensionsPermissionsManage,
                Permission::ExtensionsUiUse,
            ],
        );
        access.project_read_scopes = AccessScopes::All;
        access.extension_ui_scopes = AccessScopes::All;
        access
    }

    /// Returns the authenticated transport-neutral Principal.
    #[must_use]
    pub fn principal(&self) -> &PrincipalId {
        &self.principal
    }

    /// Revalidates one application permission at the use-case boundary.
    pub fn require(&self, permission: Permission) -> Result<(), ApplicationError> {
        self.permissions
            .contains(&permission)
            .then_some(())
            .ok_or(ApplicationError::PermissionDenied)
    }

    /// Narrows a caller to exact project-read resources for isolation tests or
    /// a future authenticated external-client adapter.
    #[must_use]
    pub fn with_project_read_scopes(
        mut self,
        project_ids: impl IntoIterator<Item = String>,
    ) -> Self {
        self.project_read_scopes = AccessScopes::Exact(project_ids.into_iter().collect());
        self
    }

    /// Narrows a caller to exact Portable UI extension resources.
    #[must_use]
    pub fn with_extension_ui_scopes(
        mut self,
        extension_ids: impl IntoIterator<Item = String>,
    ) -> Self {
        self.extension_ui_scopes = AccessScopes::Exact(extension_ids.into_iter().collect());
        self
    }

    /// Revalidates `projects.read` for one exact project resource.
    pub fn require_project_read_scope(&self, project_id: &str) -> Result<(), ApplicationError> {
        self.require(Permission::ProjectsRead)?;
        require_scope(&self.project_read_scopes, project_id)
    }

    /// Revalidates `extensions.ui.use` for one exact ExtensionId resource.
    pub fn require_extension_ui_scope(&self, extension_id: &str) -> Result<(), ApplicationError> {
        self.require(Permission::ExtensionsUiUse)?;
        require_scope(&self.extension_ui_scopes, extension_id)
    }
}

fn require_scope(scopes: &AccessScopes, resource_id: &str) -> Result<(), ApplicationError> {
    match scopes {
        AccessScopes::All => Ok(()),
        AccessScopes::Exact(values) if values.contains(resource_id) => Ok(()),
        AccessScopes::None | AccessScopes::Exact(_) => Err(ApplicationError::PermissionDenied),
    }
}

/// Stable application error categories mapped to public RPC errors by adapters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApplicationError {
    /// Permission or owner visibility was denied.
    PermissionDenied,
    /// Input was outside a frozen bound.
    InvalidInput,
    /// The persistence adapter returned a safe error category.
    Store(StoreErrorKind),
}

impl From<StoreError> for ApplicationError {
    fn from(error: StoreError) -> Self {
        Self::Store(error.kind())
    }
}

/// Minimal deterministic in-process async exclusive lock coordinator.
#[derive(Default)]
pub struct ResourceLockCoordinator {
    entries: Mutex<HashMap<ResourceKey, Weak<Mutex<()>>>>,
}

impl ResourceLockCoordinator {
    /// Acquires unique keys in canonical byte order and returns lifetime-bound guards.
    pub async fn acquire(&self, mut keys: Vec<ResourceKey>) -> ResourceLockGuards {
        keys.sort_by_key(ResourceKey::canonical_bytes);
        keys.dedup();
        let locks = {
            let mut entries = self.entries.lock().await;
            keys.into_iter()
                .map(|key| {
                    entries
                        .get(&key)
                        .and_then(Weak::upgrade)
                        .unwrap_or_else(|| {
                            let lock = Arc::new(Mutex::new(()));
                            entries.insert(key, Arc::downgrade(&lock));
                            lock
                        })
                })
                .collect::<Vec<_>>()
        };
        let mut guards = Vec::with_capacity(locks.len());
        for lock in locks {
            guards.push(lock.lock_owned().await);
        }
        ResourceLockGuards { guards }
    }
}

/// RAII owner for all acquired M2 resource locks.
pub struct ResourceLockGuards {
    guards: Vec<OwnedMutexGuard<()>>,
}

/// Minimal M2 application service shared by every transport adapter.
pub struct Application<S: StateStore> {
    store: S,
    locks: Arc<ResourceLockCoordinator>,
}

impl<S: StateStore> Clone for Application<S> {
    fn clone(&self) -> Self {
        Self {
            store: self.store.clone(),
            locks: Arc::clone(&self.locks),
        }
    }
}

impl<S: StateStore> Application<S> {
    /// Creates the M2 use-case service over one persistence port.
    #[must_use]
    pub fn new(store: S) -> Self {
        Self {
            store,
            locks: Arc::new(ResourceLockCoordinator::default()),
        }
    }

    /// Applies recovery transitions and schedules durable pending work before ready.
    pub async fn recover(&self) -> Result<(), ApplicationError> {
        let operation_ids = self.store.recover(unix_time_ms()?).await?;
        for operation_id in operation_ids {
            self.schedule(operation_id);
        }
        Ok(())
    }

    /// Starts or replays the sole M2 Operation kind.
    pub async fn state_check(
        &self,
        access: &AccessContext,
        idempotency_key: IdempotencyKey,
    ) -> Result<CreateOperationOutcome, ApplicationError> {
        access.require(Permission::StateCheck)?;
        let _guard = self.locks.acquire(vec![ResourceKey::StateStore]).await;
        let outcome = self
            .store
            .create_state_check(access.principal().clone(), idempotency_key, unix_time_ms()?)
            .await?;
        if outcome.schedule {
            self.schedule(outcome.operation_id);
        }
        Ok(outcome)
    }

    /// Returns one Operation visible to the caller.
    pub async fn get_operation(
        &self,
        access: &AccessContext,
        operation_id: OperationId,
    ) -> Result<OperationRecord, ApplicationError> {
        access.require(Permission::OperationsRead)?;
        self.store
            .get_operation(access.principal().clone(), operation_id)
            .await
            .map_err(Into::into)
    }

    /// Lists visible Operations with the frozen M2 cursor semantics.
    pub async fn list_operations(
        &self,
        access: &AccessContext,
        cursor: Option<OperationCursor>,
        limit: u32,
    ) -> Result<OperationPage, ApplicationError> {
        access.require(Permission::OperationsRead)?;
        validate_limit(limit)?;
        if cursor
            .as_ref()
            .is_some_and(|cursor| cursor.created_at_ms > i64::MAX as u64)
        {
            return Err(ApplicationError::InvalidInput);
        }
        self.store
            .list_operations(access.principal().clone(), cursor, limit)
            .await
            .map_err(Into::into)
    }

    /// Requests idempotent cooperative cancellation with optimistic concurrency.
    pub async fn cancel_operation(
        &self,
        access: &AccessContext,
        operation_id: OperationId,
        expected_revision: Revision,
        idempotency_key: IdempotencyKey,
    ) -> Result<(OperationRecord, bool), ApplicationError> {
        access.require(Permission::OperationsCancel)?;
        let _guard = self
            .locks
            .acquire(vec![ResourceKey::Operation(operation_id)])
            .await;
        self.store
            .cancel_operation(
                access.principal().clone(),
                operation_id,
                expected_revision,
                idempotency_key,
                unix_time_ms()?,
            )
            .await
            .map_err(Into::into)
    }

    /// Lists durable visible Events after an exclusive sequence cursor.
    pub async fn list_events(
        &self,
        access: &AccessContext,
        after_sequence: u64,
        limit: u32,
    ) -> Result<EventPage, ApplicationError> {
        access.require(Permission::EventsRead)?;
        validate_limit(limit)?;
        if after_sequence > i64::MAX as u64 {
            return Err(ApplicationError::InvalidInput);
        }
        self.store
            .list_events(access.principal().clone(), after_sequence, limit)
            .await
            .map_err(Into::into)
    }

    fn schedule(&self, operation_id: OperationId) {
        let application = self.clone();
        tokio::spawn(async move {
            let _ = application.run_state_check(operation_id).await;
        });
    }

    async fn run_state_check(&self, operation_id: OperationId) -> Result<(), ApplicationError> {
        let _guard = self.locks.acquire(vec![ResourceKey::StateStore]).await;
        if self.store.cancellation_requested(operation_id).await? {
            let _ = self
                .store
                .finish_cancelled(operation_id, unix_time_ms()?)
                .await?;
            return Ok(());
        }
        let _ = self
            .store
            .begin_state_check(operation_id, unix_time_ms()?)
            .await?;
        let integrity = match self.store.check_integrity().await {
            Ok(value) => value,
            Err(error) => return self.persist_worker_failure(operation_id, error).await,
        };
        if self.store.cancellation_requested(operation_id).await? {
            let _ = self
                .store
                .finish_cancelled(operation_id, unix_time_ms()?)
                .await?;
            return Ok(());
        }
        let foreign_keys = match self.store.check_foreign_keys().await {
            Ok(value) => value,
            Err(error) => return self.persist_worker_failure(operation_id, error).await,
        };
        if self.store.cancellation_requested(operation_id).await? {
            let _ = self
                .store
                .finish_cancelled(operation_id, unix_time_ms()?)
                .await?;
            return Ok(());
        }
        let _ = self
            .store
            .finish_state_check(
                operation_id,
                StateCheckResult {
                    integrity,
                    foreign_keys,
                },
                unix_time_ms()?,
            )
            .await?;
        Ok(())
    }

    async fn persist_worker_failure(
        &self,
        operation_id: OperationId,
        error: StoreError,
    ) -> Result<(), ApplicationError> {
        let error_code = match error.kind() {
            StoreErrorKind::Unavailable => "store_unavailable",
            _ => "internal_error",
        };
        let diagnostic_id = OperationId::new().to_string();
        let _ = self
            .store
            .finish_failed(
                operation_id,
                error_code.to_owned(),
                diagnostic_id,
                unix_time_ms()?,
            )
            .await?;
        Ok(())
    }
}

/// M3 project/repository read and registry use cases.
#[derive(Clone)]
pub struct M3Application<S: M3RegistryStore, R: M3ReadAdapter> {
    store: S,
    reader: R,
    locks: Arc<ResourceLockCoordinator>,
}

impl<S: M3RegistryStore, R: M3ReadAdapter> M3Application<S, R> {
    #[must_use]
    pub fn new(store: S, reader: R) -> Self {
        Self {
            store,
            reader,
            locks: Arc::new(ResourceLockCoordinator::default()),
        }
    }

    pub async fn inspect_project(
        &self,
        access: &AccessContext,
        path: String,
        mode: ProjectDiscoveryMode,
    ) -> Result<ProjectObservation, M3Error> {
        require_m3(access, Permission::ProjectsRead)?;
        self.reader.inspect_project(path, mode).await
    }

    pub async fn list_projects(
        &self,
        access: &AccessContext,
        cursor: Option<RegistryCursor<ProjectId>>,
        limit: u32,
    ) -> Result<ProjectPage, M3Error> {
        require_m3(access, Permission::ProjectsRead)?;
        validate_m3_limit(limit)?;
        self.store
            .list_projects(access.principal().clone(), cursor, limit)
            .await
    }

    pub async fn get_project(
        &self,
        access: &AccessContext,
        id: ProjectId,
    ) -> Result<ProjectRecord, M3Error> {
        require_m3(access, Permission::ProjectsRead)?;
        self.store.get_project(access.principal().clone(), id).await
    }

    pub async fn register_project(
        &self,
        access: &AccessContext,
        path: String,
        key: IdempotencyKey,
    ) -> Result<SyncWrite<ProjectRecord>, M3Error> {
        require_m3(access, Permission::ProjectsManage)?;
        let observation = self
            .reader
            .inspect_project(path, ProjectDiscoveryMode::ExactRoot)
            .await?;
        self.store
            .register_project(access.principal().clone(), observation, key, m3_time_ms()?)
            .await
    }

    pub async fn refresh_project(
        &self,
        access: &AccessContext,
        id: ProjectId,
        expected: Revision,
        key: IdempotencyKey,
    ) -> Result<SyncWrite<ProjectRecord>, M3Error> {
        require_m3(access, Permission::ProjectsManage)?;
        let _guard = self.locks.acquire(vec![ResourceKey::Project(id)]).await;
        let current = self
            .store
            .get_project(access.principal().clone(), id)
            .await?;
        let observation = self
            .reader
            .inspect_project(
                current.observation.root_path,
                ProjectDiscoveryMode::ExactRoot,
            )
            .await?;
        self.store
            .refresh_project(
                access.principal().clone(),
                id,
                expected,
                observation,
                key,
                m3_time_ms()?,
            )
            .await
    }

    pub async fn set_project_favorite(
        &self,
        access: &AccessContext,
        id: ProjectId,
        favorite: bool,
        expected: Revision,
        key: IdempotencyKey,
    ) -> Result<SyncWrite<ProjectRecord>, M3Error> {
        require_m3(access, Permission::ProjectsManage)?;
        let _guard = self.locks.acquire(vec![ResourceKey::Project(id)]).await;
        self.store
            .set_project_favorite(
                access.principal().clone(),
                id,
                favorite,
                expected,
                key,
                m3_time_ms()?,
            )
            .await
    }

    pub async fn unregister_project(
        &self,
        access: &AccessContext,
        id: ProjectId,
        expected: Revision,
        key: IdempotencyKey,
    ) -> Result<UnregisterResult<ProjectId>, M3Error> {
        require_m3(access, Permission::ProjectsManage)?;
        let _guard = self.locks.acquire(vec![ResourceKey::Project(id)]).await;
        self.store
            .unregister_project(access.principal().clone(), id, expected, key, m3_time_ms()?)
            .await
    }

    pub async fn inspect_repository(
        &self,
        access: &AccessContext,
        source: RepositorySource,
    ) -> Result<RepositoryObservation, M3Error> {
        require_m3(access, Permission::RepositoriesRead)?;
        match self.reader.inspect_repository(source, None).await? {
            RepositoryReadOutcome::Fresh(observation) => Ok(observation),
            RepositoryReadOutcome::NotModified(_) => {
                Err(M3Error::new(M3ErrorCode::RepositoryUnavailable))
            }
        }
    }

    pub async fn list_repositories(
        &self,
        access: &AccessContext,
        cursor: Option<RegistryCursor<RepositoryId>>,
        limit: u32,
    ) -> Result<RepositoryPage, M3Error> {
        require_m3(access, Permission::RepositoriesRead)?;
        validate_m3_limit(limit)?;
        self.store
            .list_repositories(access.principal().clone(), cursor, limit)
            .await
    }

    pub async fn get_repository(
        &self,
        access: &AccessContext,
        id: RepositoryId,
    ) -> Result<RepositoryRecord, M3Error> {
        require_m3(access, Permission::RepositoriesRead)?;
        self.store
            .get_repository(access.principal().clone(), id)
            .await
    }

    pub async fn list_repository_packages(
        &self,
        access: &AccessContext,
        id: RepositoryId,
        cursor: Option<PackageCursor>,
        limit: u32,
    ) -> Result<PackagePage, M3Error> {
        require_m3(access, Permission::RepositoriesRead)?;
        validate_m3_limit(limit)?;
        self.store
            .list_repository_packages(access.principal().clone(), id, cursor, limit)
            .await
    }

    pub async fn register_repository(
        &self,
        access: &AccessContext,
        source: RepositorySource,
        key: IdempotencyKey,
    ) -> Result<SyncWrite<RepositoryRecord>, M3Error> {
        require_m3(access, Permission::RepositoriesManage)?;
        let observation = match self.reader.inspect_repository(source, None).await? {
            RepositoryReadOutcome::Fresh(value) => value,
            RepositoryReadOutcome::NotModified(_) => {
                return Err(M3Error::new(M3ErrorCode::RepositoryUnavailable));
            }
        };
        self.store
            .register_repository(access.principal().clone(), observation, key, m3_time_ms()?)
            .await
    }

    pub async fn refresh_repository(
        &self,
        access: &AccessContext,
        id: RepositoryId,
        expected: Revision,
        key: IdempotencyKey,
    ) -> Result<SyncWrite<RepositoryRecord>, M3Error> {
        require_m3(access, Permission::RepositoriesManage)?;
        let _guard = self.locks.acquire(vec![ResourceKey::Repository(id)]).await;
        let current = self
            .store
            .get_repository(access.principal().clone(), id)
            .await?;
        match self
            .reader
            .inspect_repository(
                current.observation.source,
                Some(current.observation.validators),
            )
            .await?
        {
            RepositoryReadOutcome::Fresh(observation) => {
                self.store
                    .refresh_repository(
                        access.principal().clone(),
                        id,
                        expected,
                        observation,
                        key,
                        m3_time_ms()?,
                    )
                    .await
            }
            RepositoryReadOutcome::NotModified(validators) => {
                if current.revision != expected {
                    return Err(M3Error::new(M3ErrorCode::RevisionConflict));
                }
                let record = self
                    .store
                    .update_repository_validators(
                        access.principal().clone(),
                        id,
                        expected,
                        validators,
                        key,
                        m3_time_ms()?,
                    )
                    .await?;
                Ok(record)
            }
        }
    }

    pub async fn unregister_repository(
        &self,
        access: &AccessContext,
        id: RepositoryId,
        expected: Revision,
        key: IdempotencyKey,
    ) -> Result<UnregisterResult<RepositoryId>, M3Error> {
        require_m3(access, Permission::RepositoriesManage)?;
        let _guard = self.locks.acquire(vec![ResourceKey::Repository(id)]).await;
        self.store
            .unregister_repository(access.principal().clone(), id, expected, key, m3_time_ms()?)
            .await
    }
}

fn require_m3(access: &AccessContext, permission: Permission) -> Result<(), M3Error> {
    access
        .require(permission)
        .map_err(|_| M3Error::new(M3ErrorCode::PermissionDenied))
}

fn validate_m3_limit(limit: u32) -> Result<(), M3Error> {
    if (1..=1_000).contains(&limit) {
        Ok(())
    } else {
        Err(M3Error::new(M3ErrorCode::Internal))
    }
}

fn m3_time_ms() -> Result<u64, M3Error> {
    unix_time_ms().map_err(|_| M3Error::new(M3ErrorCode::Internal))
}

fn validate_limit(limit: u32) -> Result<(), ApplicationError> {
    if (1..=1_000).contains(&limit) {
        Ok(())
    } else {
        Err(ApplicationError::InvalidInput)
    }
}

fn unix_time_ms() -> Result<u64, ApplicationError> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| ApplicationError::InvalidInput)
        .and_then(|duration| {
            u64::try_from(duration.as_millis()).map_err(|_| ApplicationError::InvalidInput)
        })
}

impl ResourceLockGuards {
    /// Number of unique held resource locks.
    #[must_use]
    pub fn len(&self) -> usize {
        self.guards.len()
    }

    /// Returns whether no lock is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.guards.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_reports_only_the_real_m1_state() {
        assert_eq!(system_status().state(), SystemState::Ready);
        assert_eq!(system_status().state().as_str(), "ready");
    }

    #[test]
    fn m2_fingerprints_are_versioned_typed_and_deterministic() {
        assert_eq!(StateCheckFingerprintV1.canonical_json(), r#"{"version":1}"#);
        let operation_id =
            OperationId::parse("00000000-0000-4000-8000-000000000001").expect("fixed UUID");
        let fingerprint = CancelOperationFingerprintV1::new(operation_id, Revision::INITIAL);
        assert_eq!(
            fingerprint.canonical_json(),
            r#"{"expectedRevision":1,"operationId":"00000000-0000-4000-8000-000000000001","version":1}"#
        );
    }

    #[test]
    fn synthetic_principal_permissions_are_revalidated_per_use_case() {
        let access = AccessContext::new(
            PrincipalId::parse("synthetic:reader").expect("synthetic Principal"),
            [Permission::OperationsRead],
        );
        assert!(access.require(Permission::OperationsRead).is_ok());
        assert_eq!(
            access.require(Permission::OperationsCancel),
            Err(ApplicationError::PermissionDenied)
        );
    }

    #[tokio::test]
    async fn resource_locks_deduplicate_and_serialize_the_same_key() {
        let coordinator = Arc::new(ResourceLockCoordinator::default());
        let operation = OperationId::new();
        let first = coordinator
            .acquire(vec![
                ResourceKey::Operation(operation),
                ResourceKey::Operation(operation),
            ])
            .await;
        assert_eq!(first.len(), 1);
        let waiting = {
            let coordinator = Arc::clone(&coordinator);
            tokio::spawn(async move {
                coordinator
                    .acquire(vec![ResourceKey::Operation(operation)])
                    .await
            })
        };
        tokio::task::yield_now().await;
        assert!(!waiting.is_finished());
        drop(first);
        let acquired = waiting.await.expect("join lock waiter");
        assert_eq!(acquired.len(), 1);
    }

    #[tokio::test]
    async fn different_resource_keys_do_not_block_each_other() {
        let coordinator = Arc::new(ResourceLockCoordinator::default());
        let _first = coordinator
            .acquire(vec![ResourceKey::Operation(OperationId::new())])
            .await;
        let other = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            coordinator.acquire(vec![ResourceKey::StateStore]),
        )
        .await
        .expect("different key must not block");
        assert_eq!(other.len(), 1);
    }

    #[tokio::test]
    async fn package_project_writes_serialize_per_project_but_not_globally() {
        let coordinator = Arc::new(ResourceLockCoordinator::default());
        let first_project = ProjectId::new();
        let second_project = ProjectId::new();
        let held = coordinator
            .acquire(vec![ResourceKey::Project(first_project)])
            .await;
        let same_project = {
            let coordinator = Arc::clone(&coordinator);
            tokio::spawn(async move {
                coordinator
                    .acquire(vec![ResourceKey::Project(first_project)])
                    .await
            })
        };
        tokio::task::yield_now().await;
        assert!(!same_project.is_finished());
        let other_project = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            coordinator.acquire(vec![ResourceKey::Project(second_project)]),
        )
        .await
        .expect("different projects must remain parallel");
        assert_eq!(other_project.len(), 1);
        drop(held);
        assert_eq!(same_project.await.expect("same-project waiter").len(), 1);
    }

    #[tokio::test]
    async fn package_cache_locks_serialize_per_digest_but_not_globally() {
        let coordinator = Arc::new(ResourceLockCoordinator::default());
        let first_digest = [1_u8; 32];
        let second_digest = [2_u8; 32];
        let held = coordinator
            .acquire(vec![ResourceKey::PackageCache(first_digest)])
            .await;
        let same_digest = {
            let coordinator = Arc::clone(&coordinator);
            tokio::spawn(async move {
                coordinator
                    .acquire(vec![ResourceKey::PackageCache(first_digest)])
                    .await
            })
        };
        tokio::task::yield_now().await;
        assert!(!same_digest.is_finished());
        let other_digest = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            coordinator.acquire(vec![ResourceKey::PackageCache(second_digest)]),
        )
        .await
        .expect("different package digests must remain parallel");
        assert_eq!(other_digest.len(), 1);
        drop(held);
        assert_eq!(same_digest.await.expect("same-digest waiter").len(), 1);
    }

    #[tokio::test]
    async fn project_create_locks_serialize_only_the_same_parent_and_leaf() {
        let coordinator = Arc::new(ResourceLockCoordinator::default());
        let first = ResourceKey::ProjectCreate {
            parent_identity_sha256: [3; 32],
            target_leaf: "Example".to_owned(),
        };
        let held = coordinator.acquire(vec![first.clone()]).await;
        let same = {
            let coordinator = Arc::clone(&coordinator);
            tokio::spawn(async move { coordinator.acquire(vec![first]).await })
        };
        tokio::task::yield_now().await;
        assert!(!same.is_finished());
        let other = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            coordinator.acquire(vec![ResourceKey::ProjectCreate {
                parent_identity_sha256: [3; 32],
                target_leaf: "Other".to_owned(),
            }]),
        )
        .await
        .expect("different target leaf must remain parallel");
        assert_eq!(other.len(), 1);
        drop(held);
        assert_eq!(same.await.expect("same target waiter").len(), 1);
    }

    #[tokio::test]
    async fn cancelled_lock_waiter_leaves_no_owner() {
        let coordinator = Arc::new(ResourceLockCoordinator::default());
        let operation = OperationId::new();
        let held = coordinator
            .acquire(vec![ResourceKey::Operation(operation)])
            .await;
        let waiting = {
            let coordinator = Arc::clone(&coordinator);
            tokio::spawn(async move {
                coordinator
                    .acquire(vec![ResourceKey::Operation(operation)])
                    .await
            })
        };
        tokio::task::yield_now().await;
        waiting.abort();
        let _ = waiting.await;
        drop(held);
        let reacquired = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            coordinator.acquire(vec![ResourceKey::Operation(operation)]),
        )
        .await
        .expect("cancelled waiter must not retain the lock");
        assert_eq!(reacquired.len(), 1);
    }
}
