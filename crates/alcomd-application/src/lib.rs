//! Application use cases and orchestration boundaries.
//!
//! Transport adapters call this layer rather than inventing business state in
//! the daemon, CLI, or other entry points.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

pub use alcomd_domain::{
    BackupId, IdempotencyKey, OperationId, OperationState, Permission, PlanId, PrincipalId,
    ProjectId, RepositoryId, ResourceKey, Revision, UnityInstallationId, UnityLaunchId,
    UserPackageId,
};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, Notify, OwnedMutexGuard};

mod error_context;
mod events;
mod m4;
mod m5;
mod m5_backup;
mod m6;
mod m7;
mod m7_candidate_bulk;
mod m7_candidates;
mod m7_candidates_types;
mod m7_copy;
mod m7_delete;
mod m7_official;
mod m7_presentation;
mod m7_unity_migration;
mod m7_user_packages;
mod m8_mcp;
mod operation_logs;
mod project_authority;
mod project_create;
mod project_discovery;
mod repository_list;
mod repository_refresh;

pub use error_context::*;
pub use events::*;
pub use m4::*;
pub use m5::*;
pub use m5_backup::*;
pub use m6::*;
pub use m7::*;
pub use m7_candidates::*;
pub use m7_candidates_types::*;
pub use m7_copy::*;
pub use m7_delete::*;
pub use m7_official::*;
pub use m7_presentation::*;
pub use m7_unity_migration::*;
pub use m7_user_packages::*;
pub use m8_mcp::*;
pub use operation_logs::*;
pub use project_authority::*;
pub use project_create::*;
pub use project_discovery::*;
pub use repository_list::*;
pub use repository_refresh::*;

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
}

/// The leaf name used by project lists and details.
pub fn project_display_name(root_path: &str) -> Option<&str> {
    root_path
        .trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
}

/// Durable registered project record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectRecord {
    pub project_id: ProjectId,
    pub observation: ProjectObservation,
    pub revision: Revision,
    pub registered_at_ms: u64,
    /// Core-maintained user activity time, independent of observation and semantic revision.
    /// Historical records without reliable activity evidence remain unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_modified_at_ms: Option<u64>,
    /// User-controlled registry metadata; absent in durable v10 JSON means false.
    #[serde(default)]
    pub favorite: bool,
}

/// Local file or remote repository source.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RepositorySource {
    Local { path: String },
    Remote { url: String },
}

/// Write-only credentials. Debug deliberately omits all user-supplied fields.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryCredentials {
    pub headers: std::collections::BTreeMap<String, String>,
}

impl std::fmt::Debug for RepositoryCredentials {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RepositoryCredentials([redacted])")
    }
}

pub const VRCHAT_OFFICIAL_REPOSITORY_ID: &str = "7e2233c8-0b3f-4cf2-aeb4-57d3d240b101";
pub const VRCHAT_CURATED_REPOSITORY_ID: &str = "7e2233c8-0b3f-4cf2-aeb4-57d3d240b102";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuiltinRepository {
    Official,
    Curated,
}

pub fn builtin_repository(id: RepositoryId) -> Option<BuiltinRepository> {
    match id.to_string().as_str() {
        VRCHAT_OFFICIAL_REPOSITORY_ID => Some(BuiltinRepository::Official),
        VRCHAT_CURATED_REPOSITORY_ID => Some(BuiltinRepository::Curated),
        _ => None,
    }
}

pub fn can_remove_repository(access: &AccessContext, id: RepositoryId) -> bool {
    builtin_repository(id).is_none() && access.require(Permission::RepositoriesManage).is_ok()
}

pub fn can_edit_repository_authentication(
    access: &AccessContext,
    record: &RepositoryRecord,
) -> bool {
    builtin_repository(record.repository_id).is_none()
        && repository_supports_authentication(&record.observation.source)
        && require_repository_credentials(access).is_ok()
}

pub fn repository_supports_authentication(source: &RepositorySource) -> bool {
    matches!(source, RepositorySource::Remote { .. })
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation: Option<PackageDisplayMetadata>,
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
    pub author_name: Option<String>,
    pub author_email: Option<String>,
    pub artifact_url: String,
    pub zip_sha256: Option<String>,
    pub unity_release: Option<String>,
    pub dependencies_json: String,
    pub manifest_fingerprint: Vec<u8>,
    pub legacy_metadata_present: bool,
    pub download_headers_present: bool,
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
    pub display_name: Option<String>,
    pub revision: Revision,
    pub registered_at_ms: u64,
    #[serde(default)]
    pub credentials_configured: bool,
    #[serde(skip)]
    pub resolver_indexed: bool,
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
    /// Read-time filesystem projection; never persisted with registered projects.
    #[serde(skip)]
    pub directory_states: HashMap<ProjectId, ProjectDirectoryState>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectDirectoryState {
    Available,
    Missing,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectDirectoryView {
    pub project: ProjectRecord,
    pub directory_state: ProjectDirectoryState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectListView {
    pub search: Option<String>,
    pub sort: ProjectListSort,
    pub descending: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectListSort {
    Name,
    Type,
    Unity,
    Added,
    Modified,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectViewPage {
    pub page: ProjectPage,
    pub sequence: u64,
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
    InvalidInput,
    RevisionConflict,
    IdempotencyConflict,
    PermissionDenied,
    StoreUnavailable,
    Internal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct M3Error {
    code: M3ErrorCode,
    context: Option<TechnicalErrorContext>,
}

impl M3Error {
    #[must_use]
    pub const fn new(code: M3ErrorCode) -> Self {
        Self {
            code,
            context: None,
        }
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

impl M3Error {
    pub fn with_source(mut self, error: impl std::error::Error + Send + Sync + 'static) -> Self {
        self.context = Some(self.context.take().unwrap_or_default().with_source(error));
        self
    }
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.context = Some(self.context.take().unwrap_or_default().with_path(path));
        self
    }
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.context = Some(self.context.take().unwrap_or_default().with_reason(reason));
        self
    }
    pub fn technical_path(&self) -> Option<&str> {
        self.context.as_ref().and_then(|c| c.path.as_deref())
    }
}
impl std::error::Error for M3Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.context
            .as_ref()
            .and_then(TechnicalErrorContext::source_error)
    }
}

/// External read adapter. Implementations perform bounded I/O but never mutate sources.
pub trait M3ReadAdapter: Clone + Send + Sync + 'static {
    fn normalize_remote_repository_url(&self, url: &str) -> Result<String, M3Error>;

    fn project_directory_state(
        &self,
        path: String,
    ) -> impl Future<Output = ProjectDirectoryState> + Send;

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

    fn inspect_repository_authenticated(
        &self,
        source: RepositorySource,
        validators: Option<RepositoryValidators>,
        credentials: Option<RepositoryCredentials>,
    ) -> impl Future<Output = Result<RepositoryReadOutcome, M3Error>> + Send {
        async move {
            if credentials.is_some() {
                return Err(M3Error::new(M3ErrorCode::RepositoryCredentialsUnsupported));
            }
            self.inspect_repository(source, validators).await
        }
    }

    fn save_repository_credentials(
        &self,
        _reference: String,
        _credentials: RepositoryCredentials,
    ) -> impl Future<Output = Result<(), M3Error>> + Send {
        async { Err(M3Error::new(M3ErrorCode::RepositoryCredentialsUnsupported)) }
    }

    fn load_repository_credentials(
        &self,
        _reference: String,
    ) -> impl Future<Output = Result<RepositoryCredentials, M3Error>> + Send {
        async { Err(M3Error::new(M3ErrorCode::RepositoryCredentialsUnsupported)) }
    }

    fn delete_repository_credentials(
        &self,
        _reference: String,
    ) -> impl Future<Output = Result<(), M3Error>> + Send {
        async { Err(M3Error::new(M3ErrorCode::RepositoryCredentialsUnsupported)) }
    }
}

/// M3 persistence port implemented only by the authoritative state store.
pub trait M3RegistryStore: Clone + Send + Sync + 'static {
    fn queue_repository_credential_cleanup(
        &self,
        _reference: String,
    ) -> impl Future<Output = Result<(), M3Error>> + Send {
        async { Err(M3Error::new(M3ErrorCode::RepositoryCredentialsUnsupported)) }
    }

    fn finish_repository_credential_cleanup(
        &self,
        _reference: String,
    ) -> impl Future<Output = Result<(), M3Error>> + Send {
        async { Ok(()) }
    }

    fn pending_repository_credential_cleanup(
        &self,
    ) -> impl Future<Output = Result<Vec<String>, M3Error>> + Send {
        async { Ok(Vec::new()) }
    }
    fn repository_credential_reference(
        &self,
        _owner: PrincipalId,
        _id: RepositoryId,
    ) -> impl Future<Output = Result<Option<String>, M3Error>> + Send {
        async { Ok(None) }
    }

    fn register_repository_authenticated(
        &self,
        _owner: PrincipalId,
        _observation: RepositoryObservation,
        _reference: String,
        _key: IdempotencyKey,
        _now_ms: u64,
    ) -> impl Future<Output = Result<SyncWrite<RepositoryRecord>, M3Error>> + Send {
        async { Err(M3Error::new(M3ErrorCode::RepositoryCredentialsUnsupported)) }
    }

    fn set_repository_credential_reference(
        &self,
        _owner: PrincipalId,
        _id: RepositoryId,
        _expected: Revision,
        _reference: Option<String>,
        _key: IdempotencyKey,
        _now_ms: u64,
    ) -> impl Future<Output = Result<SyncWrite<RepositoryRecord>, M3Error>> + Send {
        async { Err(M3Error::new(M3ErrorCode::RepositoryCredentialsUnsupported)) }
    }
    fn register_project(
        &self,
        owner: PrincipalId,
        observation: ProjectObservation,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<SyncWrite<ProjectRecord>, M3Error>> + Send;
    fn get_mcp_project(
        &self,
        _principal: PrincipalId,
        _id: ProjectId,
    ) -> impl Future<Output = Result<ProjectRecord, M3Error>> + Send {
        std::future::ready(Err(M3Error::new(M3ErrorCode::PermissionDenied)))
    }
    fn list_mcp_projects(
        &self,
        _principal: PrincipalId,
        _cursor: Option<RegistryCursor<ProjectId>>,
        _limit: u32,
    ) -> impl Future<Output = Result<ProjectPage, M3Error>> + Send {
        std::future::ready(Err(M3Error::new(M3ErrorCode::PermissionDenied)))
    }
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
    fn list_project_view(
        &self,
        owner: PrincipalId,
        cursor: Option<RegistryCursor<ProjectId>>,
        limit: u32,
        view: ProjectListView,
        expected_sequence: Option<u64>,
    ) -> impl Future<Output = Result<ProjectViewPage, M3Error>> + Send;
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
    fn set_repository_display_name(
        &self,
        owner: PrincipalId,
        id: RepositoryId,
        expected: Revision,
        display_name: Option<String>,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<SyncWrite<RepositoryRecord>, M3Error>> + Send;
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
    /// Total visible non-terminal Operations, independently of this page.
    pub active_count: u64,
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoreError {
    kind: StoreErrorKind,
    context: Option<TechnicalErrorContext>,
}

impl StoreError {
    /// Creates a safe store error category.
    #[must_use]
    pub const fn new(kind: StoreErrorKind) -> Self {
        Self {
            kind,
            context: None,
        }
    }

    /// Returns the stable application-visible category.
    #[must_use]
    pub const fn kind(&self) -> StoreErrorKind {
        self.kind
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "state store request failed ({:?})", self.kind)
    }
}

impl StoreError {
    pub fn with_source(mut self, error: impl std::error::Error + Send + Sync + 'static) -> Self {
        self.context = Some(self.context.take().unwrap_or_default().with_source(error));
        self
    }
}
impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.context
            .as_ref()
            .and_then(TechnicalErrorContext::source_error)
    }
}

/// Narrow asynchronous persistence port required by the M2 vertical slice.
pub trait StateStore: Clone + Send + Sync + 'static {
    /// Whether any Principal owns a durable nonterminal Operation.
    fn has_unfinished_operations(&self) -> impl Future<Output = Result<bool, StoreError>> + Send;

    /// Subscribes to actual committed event watermarks.
    fn event_commits(&self) -> tokio::sync::watch::Receiver<EventCommitWatermark>;

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
        active_only: bool,
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
    actor: String,
    permissions: HashSet<Permission>,
    project_read_scopes: AccessScopes,
    project_package_write_scopes: AccessScopes,
    extension_ui_scopes: AccessScopes,
    mcp_identity: bool,
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
            actor: "Core".to_owned(),
            permissions: permissions.into_iter().collect(),
            project_read_scopes: AccessScopes::None,
            project_package_write_scopes: AccessScopes::None,
            extension_ui_scopes: AccessScopes::None,
            mcp_identity: false,
        }
    }

    /// Creates the M2 built-in official-client context.
    #[must_use]
    pub fn local_owner() -> Self {
        let mut access = Self::new(
            PrincipalId::local_owner(),
            [
                Permission::McpClientsRead,
                Permission::McpClientsManage,
                Permission::StateCheck,
                Permission::OperationsRead,
                Permission::OperationsCancel,
                Permission::EventsRead,
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
                Permission::UnityHubOpen,
                Permission::ProjectsCreate,
                Permission::ProjectsDelete,
                Permission::ProjectsUnityMigrate,
                Permission::BackupsRead,
                Permission::BackupsManage,
                Permission::ExtensionsRead,
                Permission::ExtensionsManage,
                Permission::ExtensionsPermissionsManage,
                Permission::ExtensionsUiUse,
            ],
        );
        access.project_read_scopes = AccessScopes::All;
        access.project_package_write_scopes = AccessScopes::All;
        access.extension_ui_scopes = AccessScopes::All;
        access
    }

    pub(crate) fn with_mcp_identity(mut self) -> Self {
        self.mcp_identity = true;
        self
    }
    pub fn is_mcp_client(&self) -> bool {
        self.mcp_identity
    }

    /// Returns the authenticated transport-neutral Principal.
    #[must_use]
    pub fn principal(&self) -> &PrincipalId {
        &self.principal
    }

    /// Binds the invoking entry point as metadata, independently of permissions.
    #[must_use]
    pub fn with_actor(mut self, actor: String) -> Self {
        self.actor = actor;
        self
    }

    #[must_use]
    pub fn actor(&self) -> &str {
        &self.actor
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

    pub(crate) fn with_project_package_write_scopes(mut self, ids: Vec<String>) -> Self {
        self.project_package_write_scopes = AccessScopes::Exact(ids.into_iter().collect());
        self
    }

    pub fn require_project_package_write_scope(
        &self,
        project_id: &str,
    ) -> Result<(), ApplicationError> {
        self.require(Permission::PackagesManage)?;
        require_scope(&self.project_package_write_scopes, project_id)
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApplicationError {
    /// Permission or owner visibility was denied.
    PermissionDenied,
    /// Input was outside a frozen bound.
    InvalidInput,
    /// The persistence adapter returned a safe error category.
    Store(StoreError),
}

impl From<StoreError> for ApplicationError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

impl fmt::Display for ApplicationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PermissionDenied => f.write_str("permission denied"),
            Self::InvalidInput => f.write_str("invalid input"),
            Self::Store(_) => f.write_str("persistence request failed"),
        }
    }
}
impl std::error::Error for ApplicationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Store(e) => Some(e),
            _ => None,
        }
    }
}

/// Minimal deterministic in-process async exclusive lock coordinator.
#[derive(Default)]
pub struct ResourceLockCoordinator {
    entries: Mutex<HashMap<ResourceKey, Weak<Mutex<()>>>>,
    project_recoveries: Arc<AtomicUsize>,
    project_recovery_notify: Arc<Notify>,
}

impl ResourceLockCoordinator {
    /// Reserves startup project recovery before its asynchronous task can be scheduled.
    /// Ordinary Apply operations use their existing per-target resource locks instead.
    #[must_use = "retain the guard until startup project recovery finishes"]
    pub fn project_recovery_guard(&self) -> ProjectRecoveryGuard {
        self.project_recoveries.fetch_add(1, Ordering::AcqRel);
        ProjectRecoveryGuard {
            pending: Arc::clone(&self.project_recoveries),
            notify: Arc::clone(&self.project_recovery_notify),
        }
    }

    #[must_use]
    pub fn project_recovery_pending(&self) -> bool {
        self.project_recoveries.load(Ordering::Acquire) != 0
    }

    #[must_use]
    pub fn project_recovery_notify(&self) -> Arc<Notify> {
        Arc::clone(&self.project_recovery_notify)
    }

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

/// Startup-only barrier for default-directory registration; it neither blocks ready nor persists.
#[must_use = "retain the guard until the scheduled startup recovery finishes"]
pub struct ProjectRecoveryGuard {
    pending: Arc<AtomicUsize>,
    notify: Arc<Notify>,
}

impl Drop for ProjectRecoveryGuard {
    fn drop(&mut self) {
        if self.pending.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.notify.notify_one();
        }
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
    /// Core lifecycle query across all Principals, independent of client visibility.
    pub async fn has_unfinished_operations(&self) -> Result<bool, ApplicationError> {
        self.store
            .has_unfinished_operations()
            .await
            .map_err(Into::into)
    }

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
        active_only: bool,
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
            .list_operations(access.principal().clone(), cursor, limit, active_only)
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
        self.store
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
    repository_import: Arc<Mutex<()>>,
    repository_previews: Arc<Mutex<VecDeque<RepositoryPreview>>>,
}

const REPOSITORY_PREVIEW_TTL: Duration = Duration::from_secs(60);
const REPOSITORY_PREVIEW_LIMIT: usize = 8;

struct RepositoryPreview {
    connection_id: String,
    owner: PrincipalId,
    expires_at: Instant,
    observation: RepositoryObservation,
    credentials: Option<RepositoryCredentials>,
}

impl<S: M3RegistryStore, R: M3ReadAdapter> M3Application<S, R> {
    #[must_use]
    pub fn new(store: S, reader: R) -> Self {
        Self {
            store,
            reader,
            locks: Arc::new(ResourceLockCoordinator::default()),
            repository_import: Arc::new(Mutex::new(())),
            repository_previews: Arc::new(Mutex::new(VecDeque::new())),
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
        let mut page = if access.is_mcp_client() {
            self.store
                .list_mcp_projects(access.principal().clone(), cursor, limit)
                .await?
        } else {
            self.store
                .list_projects(access.principal().clone(), cursor, limit)
                .await?
        };
        self.observe_project_directories(&mut page).await;
        Ok(page)
    }

    pub async fn list_project_view(
        &self,
        access: &AccessContext,
        cursor: Option<RegistryCursor<ProjectId>>,
        limit: u32,
        view: ProjectListView,
        expected_sequence: Option<u64>,
    ) -> Result<ProjectViewPage, M3Error> {
        require_m3(access, Permission::ProjectsRead)?;
        if access.is_mcp_client() {
            return Err(M3Error::new(M3ErrorCode::PermissionDenied));
        }
        validate_m3_limit(limit)?;
        if cursor.is_some() != expected_sequence.is_some()
            || expected_sequence.is_some_and(|value| value > i64::MAX as u64)
            || view.search.as_ref().is_some_and(|search| {
                search.chars().count() > 256 || search.chars().any(char::is_control)
            })
        {
            return Err(M3Error::new(M3ErrorCode::InvalidInput));
        }
        let mut view_page = self
            .store
            .list_project_view(
                access.principal().clone(),
                cursor,
                limit,
                view,
                expected_sequence,
            )
            .await?;
        self.observe_project_directories(&mut view_page.page).await;
        Ok(view_page)
    }

    async fn observe_project_directories(&self, page: &mut ProjectPage) {
        for project in &page.projects {
            let state = self
                .reader
                .project_directory_state(project.observation.root_path.clone())
                .await;
            page.directory_states.insert(project.project_id, state);
        }
    }

    pub async fn get_project(
        &self,
        access: &AccessContext,
        id: ProjectId,
    ) -> Result<ProjectDirectoryView, M3Error> {
        require_m3(access, Permission::ProjectsRead)?;
        let project = if access.is_mcp_client() {
            access
                .require_project_read_scope(&id.to_string())
                .map_err(|_| M3Error::new(M3ErrorCode::PermissionDenied))?;
            self.store
                .get_mcp_project(access.principal().clone(), id)
                .await?
        } else {
            self.store
                .get_project(access.principal().clone(), id)
                .await?
        };
        Ok(self.project_directory_view(project).await)
    }

    /// Enriches a record returned by an authorized owner-scoped lookup or write.
    pub async fn project_directory_view(&self, project: ProjectRecord) -> ProjectDirectoryView {
        let directory_state = self
            .reader
            .project_directory_state(project.observation.root_path.clone())
            .await;
        ProjectDirectoryView {
            project,
            directory_state,
        }
    }

    pub async fn register_project(
        &self,
        access: &AccessContext,
        path: String,
        key: IdempotencyKey,
    ) -> Result<SyncWrite<ProjectRecord>, M3Error> {
        require_m3(access, Permission::ProjectsManage)?;
        let observation = log_execution_step(
            "projects.register",
            "inspect_project",
            Some(log_path("project", path.clone())),
            self.reader
                .inspect_project(path, ProjectDiscoveryMode::ExactRoot),
            |error| error.technical_path().map(str::to_owned),
        )
        .await?;
        let target = log_path("project", observation.root_path.clone());
        log_transaction_step(
            "projects.register",
            "registered",
            Some(target),
            self.store.register_project(
                access.principal().clone(),
                observation,
                key,
                m3_time_ms()?,
            ),
        )
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
        let path = current.observation.root_path;
        let observation = log_execution_step(
            "projects.refresh",
            "inspect_project",
            Some(log_path("project", path.clone())),
            self.reader
                .inspect_project(path, ProjectDiscoveryMode::ExactRoot),
            |error| error.technical_path().map(str::to_owned),
        )
        .await?;
        let result = self
            .store
            .refresh_project(
                access.principal().clone(),
                id,
                expected,
                observation,
                key,
                m3_time_ms()?,
            )
            .await?;
        Ok(result)
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
        let result = self
            .store
            .set_project_favorite(
                access.principal().clone(),
                id,
                favorite,
                expected,
                key,
                m3_time_ms()?,
            )
            .await?;
        Ok(result)
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
        let result = self
            .store
            .unregister_project(access.principal().clone(), id, expected, key, m3_time_ms()?)
            .await?;
        Ok(result)
    }

    pub async fn inspect_repository(
        &self,
        access: &AccessContext,
        connection_id: &str,
        source: RepositorySource,
        credentials: Option<RepositoryCredentials>,
    ) -> Result<RepositoryObservation, M3Error> {
        require_m3(access, Permission::RepositoriesRead)?;
        if credentials.is_some() {
            require_repository_credentials(access)?;
        }
        self.forget_repository_preview(connection_id).await;
        let mut observation = self
            .read_repository_preview(source, credentials.clone())
            .await?;
        flush_log_steps().await;
        // Retain only remote observations that this connection can register.
        // Local registration still rechecks the file at the selected path.
        if matches!(observation.source, RepositorySource::Remote { .. })
            && access.require(Permission::RepositoriesManage).is_ok()
        {
            let packages = std::mem::take(&mut observation.packages);
            let mut previews = self.repository_previews.lock().await;
            let now = Instant::now();
            previews.retain(|preview| preview.expires_at > now);
            if previews.len() == REPOSITORY_PREVIEW_LIMIT {
                previews.pop_front();
            }
            previews.push_back(RepositoryPreview {
                connection_id: connection_id.to_owned(),
                owner: access.principal().clone(),
                expires_at: now + REPOSITORY_PREVIEW_TTL,
                observation: RepositoryObservation {
                    packages,
                    ..observation.clone()
                },
                credentials,
            });
        }
        Ok(observation)
    }

    pub async fn forget_repository_preview(&self, connection_id: &str) {
        let now = Instant::now();
        self.repository_previews
            .lock()
            .await
            .retain(|preview| preview.connection_id != connection_id && preview.expires_at > now);
    }

    async fn read_repository_preview(
        &self,
        source: RepositorySource,
        credentials: Option<RepositoryCredentials>,
    ) -> Result<RepositoryObservation, M3Error> {
        match self
            .reader
            .inspect_repository_authenticated(source, None, credentials)
            .await?
        {
            RepositoryReadOutcome::Fresh(observation) => Ok(observation),
            RepositoryReadOutcome::NotModified(_) => {
                Err(M3Error::new(M3ErrorCode::RepositoryUnavailable))
            }
        }
    }

    async fn persist_authenticated_repository(
        &self,
        access: &AccessContext,
        observation: RepositoryObservation,
        credentials: RepositoryCredentials,
        key: IdempotencyKey,
    ) -> Result<SyncWrite<RepositoryRecord>, M3Error> {
        let now = m3_time_ms()?;
        let reference = self.stage_repository_credentials(credentials).await?;
        let result = self
            .store
            .register_repository_authenticated(
                access.principal().clone(),
                observation,
                reference.clone(),
                key,
                now,
            )
            .await;
        if unused_credential_reference(&result) {
            self.discard_credential_reference(reference).await;
        }
        result
    }

    pub async fn set_repository_display_name(
        &self,
        access: &AccessContext,
        id: RepositoryId,
        expected: Revision,
        display_name: Option<String>,
        key: IdempotencyKey,
    ) -> Result<SyncWrite<RepositoryRecord>, M3Error> {
        require_m3(access, Permission::RepositoriesManage)?;
        if display_name
            .as_ref()
            .is_some_and(|name| name.chars().count() > 128 || name.chars().any(char::is_control))
        {
            return Err(M3Error::new(M3ErrorCode::InvalidInput));
        }
        let display_name = display_name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_owned);
        let _guard = self.locks.acquire(vec![ResourceKey::Repository(id)]).await;
        self.store
            .set_repository_display_name(
                access.principal().clone(),
                id,
                expected,
                display_name,
                key,
                m3_time_ms()?,
            )
            .await
    }

    pub async fn set_repository_credentials(
        &self,
        access: &AccessContext,
        id: RepositoryId,
        expected: Revision,
        credentials: Option<RepositoryCredentials>,
        key: IdempotencyKey,
    ) -> Result<SyncWrite<RepositoryRecord>, M3Error> {
        require_repository_credentials(access)?;
        if builtin_repository(id).is_some() {
            return Err(M3Error::new(M3ErrorCode::PermissionDenied));
        }
        let _guard = self.locks.acquire(vec![ResourceKey::Repository(id)]).await;
        let owner = access.principal().clone();
        let now = m3_time_ms()?;
        let old = self
            .store
            .repository_credential_reference(owner.clone(), id)
            .await?;
        let reference = match credentials {
            Some(credentials) => {
                let record = self.store.get_repository(owner.clone(), id).await?;
                self.read_repository_preview(record.observation.source, Some(credentials.clone()))
                    .await?;
                Some(self.stage_repository_credentials(credentials).await?)
            }
            None => None,
        };
        let result = self
            .store
            .set_repository_credential_reference(owner, id, expected, reference.clone(), key, now)
            .await;
        if result.as_ref().is_ok_and(|result| !result.replayed) {
            if let Some(old) = old {
                self.discard_credential_reference(old).await;
            }
        } else if let Some(reference) = reference.filter(|_| unused_credential_reference(&result)) {
            self.discard_credential_reference(reference).await;
        }
        result
    }

    async fn stage_repository_credentials(
        &self,
        credentials: RepositoryCredentials,
    ) -> Result<String, M3Error> {
        let reference = OperationId::new().to_string();
        self.store
            .queue_repository_credential_cleanup(reference.clone())
            .await?;
        if let Err(error) = self
            .reader
            .save_repository_credentials(reference.clone(), credentials)
            .await
        {
            self.discard_credential_reference(reference).await;
            return Err(error);
        }
        Ok(reference)
    }

    async fn discard_credential_reference(&self, reference: String) {
        if self
            .reader
            .delete_repository_credentials(reference.clone())
            .await
            .is_err()
            || self
                .store
                .finish_repository_credential_cleanup(reference)
                .await
                .is_err()
        {
            eprintln!("could not remove an unreferenced repository credential");
        }
    }

    /// Take this snapshot at startup, before any new references can be staged.
    pub async fn recover_repository_credentials(&self) -> Result<(), M3Error> {
        let references = self.store.pending_repository_credential_cleanup().await?;
        if !references.is_empty() {
            let application = self.clone();
            tokio::spawn(async move {
                for reference in references {
                    application.discard_credential_reference(reference).await;
                }
            });
        }
        Ok(())
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
        let target = match &source {
            RepositorySource::Local { path } => Some(log_path("repository", path.clone())),
            RepositorySource::Remote { .. } => None,
        };
        let observation = match log_execution_step(
            "repositories.register",
            "fetch_repository",
            target,
            self.reader.inspect_repository(source, None),
            |error| error.technical_path().map(str::to_owned),
        )
        .await?
        {
            RepositoryReadOutcome::Fresh(value) => value,
            RepositoryReadOutcome::NotModified(_) => {
                return Err(M3Error::new(M3ErrorCode::RepositoryUnavailable));
            }
        };
        self.persist_repository(access, observation, key).await
    }

    pub async fn register_repository_for_connection(
        &self,
        access: &AccessContext,
        connection_id: &str,
        source: RepositorySource,
        key: IdempotencyKey,
        credentials: Option<RepositoryCredentials>,
    ) -> Result<SyncWrite<RepositoryRecord>, M3Error> {
        require_m3(access, Permission::RepositoriesManage)?;
        if credentials.is_some() {
            require_repository_credentials(access)?;
        }
        let preview = {
            let mut previews = self.repository_previews.lock().await;
            let now = Instant::now();
            previews.retain(|preview| preview.expires_at > now);
            previews
                .iter()
                .position(|preview| {
                    preview.connection_id == connection_id && &preview.owner == access.principal()
                })
                .and_then(|index| previews.remove(index))
        };
        let observation = if let Some(preview) = preview
            && preview.credentials == credentials
            && let RepositorySource::Remote { url } = &source
            && self
                .reader
                .normalize_remote_repository_url(url)
                .is_ok_and(|url| preview.observation.source == RepositorySource::Remote { url })
        {
            preview.observation
        } else if credentials.is_some() {
            self.read_repository_preview(source, credentials.clone())
                .await?
        } else {
            return self.register_repository(access, source, key).await;
        };
        if let Some(credentials) = credentials {
            self.persist_authenticated_repository(access, observation, credentials, key)
                .await
        } else {
            self.persist_repository(access, observation, key).await
        }
    }

    async fn persist_repository(
        &self,
        access: &AccessContext,
        observation: RepositoryObservation,
        key: IdempotencyKey,
    ) -> Result<SyncWrite<RepositoryRecord>, M3Error> {
        let target = match &observation.source {
            RepositorySource::Local { path } => Some(log_path("repository", path.clone())),
            RepositorySource::Remote { .. } => None,
        };
        log_transaction_step(
            "repositories.register",
            "registered",
            target,
            self.store.register_repository(
                access.principal().clone(),
                observation,
                key,
                m3_time_ms()?,
            ),
        )
        .await
    }

    /// Downloads a full list through a rolling window, then persists in list order.
    /// Stopping closes admission and lets the already admitted work finish.
    pub async fn register_repository_list(
        &self,
        access: &AccessContext,
        urls: Vec<String>,
        key: IdempotencyKey,
        stop: Option<tokio::sync::watch::Receiver<bool>>,
    ) -> Result<Vec<RepositoryBatchItem>, M3Error>
    where
        S: RepositoryListStore,
    {
        self.register_repository_list_with_credentials(
            access,
            urls.into_iter().map(|url| (url, None)).collect(),
            key,
            stop,
        )
        .await
    }

    pub async fn register_repository_list_with_credentials(
        &self,
        access: &AccessContext,
        urls: Vec<(String, Option<RepositoryCredentials>)>,
        key: IdempotencyKey,
        mut stop: Option<tokio::sync::watch::Receiver<bool>>,
    ) -> Result<Vec<RepositoryBatchItem>, M3Error>
    where
        S: RepositoryListStore,
    {
        const REORDER_WINDOW: usize = 2 * DOWNLOAD_CONCURRENCY;

        require_m3(access, Permission::RepositoriesManage)?;
        if access.principal() != &PrincipalId::local_owner() {
            return Err(M3Error::new(M3ErrorCode::PermissionDenied));
        }
        let invalid = || M3Error::new(M3ErrorCode::RepositorySourceInvalid);
        if urls.is_empty() || urls.len() > REPOSITORY_LIST_MAX_ENTRIES || key.as_str().len() > 100 {
            return Err(invalid());
        }
        let mut distinct = HashSet::with_capacity(urls.len());
        let entries = urls
            .into_iter()
            .enumerate()
            .map(|(index, (url, credentials))| {
                if url.len() > 2048 {
                    return Err(invalid());
                }
                let url = self.reader.normalize_remote_repository_url(&url)?;
                if url.len() > 2048 || !distinct.insert(url.clone()) {
                    return Err(invalid());
                }
                let key = IdempotencyKey::parse(format!("list:{}:{index}", key.as_str()))
                    .map_err(|_| invalid())?;
                Ok((url, key, credentials))
            })
            .collect::<Result<Vec<_>, _>>()?;

        // One ordered import owns the bounded reorder window at a time.
        let _import = if let Some(signal) = &mut stop {
            tokio::select! {
                biased;
                _ = signal.wait_for(|stopped| *stopped) => {
                    return Ok(vec![RepositoryBatchItem::Unprocessed; entries.len()]);
                }
                guard = self.repository_import.lock() => guard,
            }
        } else {
            self.repository_import.lock().await
        };
        let known = self
            .store
            .repository_list_snapshot(access.principal().clone())
            .await?
            .repositories
            .into_iter()
            .filter_map(|entry| match entry.source {
                RepositorySource::Remote { url } => Some(url),
                RepositorySource::Local { .. } => None,
            })
            .collect::<HashSet<_>>();
        let mut downloads = tokio::task::JoinSet::new();
        let mut download_indices = HashMap::new();
        let mut observations = HashMap::new();
        let mut admitted = 0;
        let mut outcomes = Vec::with_capacity(entries.len());
        let mut commit = None;
        while outcomes.len() < entries.len() {
            while admitted < entries.len()
                && admitted - outcomes.len() < REORDER_WINDOW
                && downloads.len() < DOWNLOAD_CONCURRENCY
                && !stop
                    .as_ref()
                    .is_some_and(|stop| *stop.borrow() || stop.has_changed().is_err())
            {
                let (url, _, credentials) = &entries[admitted];
                if known.contains(url) {
                    observations.insert(admitted, None);
                } else {
                    let reader = self.reader.clone();
                    let source = RepositorySource::Remote { url: url.clone() };
                    let credentials = credentials.clone();
                    let task = downloads.spawn(async move {
                        reader
                            .inspect_repository_authenticated(source, None, credentials)
                            .await
                    });
                    download_indices.insert(task.id(), admitted);
                }
                admitted += 1;
            }

            if commit.is_none()
                && let Some(observation) = observations.remove(&outcomes.len())
            {
                commit = Some(Box::pin(self.persist_repository_list_item(
                    access,
                    entries[outcomes.len()].1.clone(),
                    entries[outcomes.len()].2.clone(),
                    observation,
                )));
            }
            if commit.is_none() && downloads.is_empty() {
                break;
            }
            // Refill download slots while the next ordered registration is in
            // the store, with no more than one write in progress.
            tokio::select! {
                joined = downloads.join_next_with_id(), if !downloads.is_empty() => {
                    let (id, observation) = match joined.ok_or_else(|| M3Error::new(M3ErrorCode::Internal))? {
                        Ok(joined) => joined,
                        Err(error) => (error.id(), Err(M3Error::new(M3ErrorCode::Internal).with_source(error))),
                    };
                    let index = download_indices.remove(&id).ok_or_else(|| M3Error::new(M3ErrorCode::Internal))?;
                    observations.insert(index, Some(observation));
                }
                outcome = async {
                    match &mut commit {
                        Some(commit) => commit.await,
                        None => std::future::pending().await,
                    }
                } => {
                    commit = None;
                    outcomes.push(outcome);
                    if matches!(outcome, RepositoryBatchItem::Unknown(_)) {
                        // Remaining jobs are read-only. An ambiguous store result
                        // prevents any further registrations for this import.
                        downloads.shutdown().await;
                        break;
                    }
                }
            }
        }
        outcomes.resize(entries.len(), RepositoryBatchItem::Unprocessed);
        Ok(outcomes)
    }

    async fn persist_repository_list_item(
        &self,
        access: &AccessContext,
        key: IdempotencyKey,
        credentials: Option<RepositoryCredentials>,
        observation: Option<Result<RepositoryReadOutcome, M3Error>>,
    ) -> RepositoryBatchItem {
        let Some(observation) = observation else {
            return RepositoryBatchItem::Existing;
        };
        match log_execution_step(
            "repositories.register",
            "fetch_repository",
            None,
            async { observation },
            |error| error.technical_path().map(str::to_owned),
        )
        .await
        {
            Ok(RepositoryReadOutcome::Fresh(value)) => {
                let result = match credentials {
                    Some(credentials) => {
                        self.persist_authenticated_repository(access, value, credentials, key)
                            .await
                    }
                    None => self.persist_repository(access, value, key).await,
                };
                match result {
                    Ok(_) => RepositoryBatchItem::Added,
                    Err(error) if error.code() == M3ErrorCode::RepositoryAlreadyRegistered => {
                        RepositoryBatchItem::Existing
                    }
                    Err(error)
                        if matches!(
                            error.code(),
                            M3ErrorCode::Internal | M3ErrorCode::StoreUnavailable
                        ) =>
                    {
                        RepositoryBatchItem::Unknown(error.code())
                    }
                    Err(error) => RepositoryBatchItem::Failed(error.code()),
                }
            }
            Ok(RepositoryReadOutcome::NotModified(_)) => {
                RepositoryBatchItem::Failed(M3ErrorCode::RepositoryUnavailable)
            }
            Err(error) => RepositoryBatchItem::Failed(error.code()),
        }
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
        self.refresh_repository_locked(access, id, expected, key)
            .await
    }

    /// Separate Core maintenance command; ordinary queries remain side-effect free.
    pub async fn refresh_repository_if_stale(
        &self,
        access: &AccessContext,
        id: RepositoryId,
        key: IdempotencyKey,
    ) -> Result<bool, M3Error> {
        require_m3(access, Permission::RepositoriesManage)?;
        let _guard = self.locks.acquire(vec![ResourceKey::Repository(id)]).await;
        let current = self
            .store
            .get_repository(access.principal().clone(), id)
            .await?;
        let now_ms = m3_time_ms()?;
        if current.resolver_indexed
            && !repository_refresh_due(current.observation.refreshed_at_ms, now_ms)
        {
            return Ok(false);
        }
        self.refresh_repository_locked(access, id, current.revision, key)
            .await?;
        Ok(true)
    }

    async fn refresh_repository_locked(
        &self,
        access: &AccessContext,
        id: RepositoryId,
        expected: Revision,
        key: IdempotencyKey,
    ) -> Result<SyncWrite<RepositoryRecord>, M3Error> {
        let current = self
            .store
            .get_repository(access.principal().clone(), id)
            .await?;
        let target = match &current.observation.source {
            RepositorySource::Local { path } => Some(log_path("repository", path.clone())),
            RepositorySource::Remote { .. } => None,
        };
        match log_execution_step(
            "repositories.refresh",
            "fetch_repository",
            target,
            async {
                let credentials = if current.credentials_configured {
                    let reference = self
                        .store
                        .repository_credential_reference(access.principal().clone(), id)
                        .await?
                        .ok_or_else(|| M3Error::new(M3ErrorCode::RepositoryUnavailable))?;
                    Some(self.reader.load_repository_credentials(reference).await?)
                } else {
                    None
                };
                self.reader
                    .inspect_repository_authenticated(
                        current.observation.source,
                        (current.resolver_indexed
                            && !current
                                .observation
                                .issues
                                .iter()
                                .any(|issue| issue.code == "resolver_metadata_invalid"))
                        .then_some(current.observation.validators),
                        credentials,
                    )
                    .await
            },
            |error| error.technical_path().map(str::to_owned),
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

    pub async fn refresh_all_repositories(
        &self,
        access: &AccessContext,
        key: IdempotencyKey,
    ) -> Result<Vec<(RepositoryId, Result<(), M3ErrorCode>)>, M3Error>
    where
        S: RepositoryListStore + OperationLogStore,
    {
        require_m3(access, Permission::RepositoriesRead)?;
        require_m3(access, Permission::RepositoriesManage)?;
        let invalid = || M3Error::new(M3ErrorCode::RepositorySourceInvalid);
        if key.as_str().len() > 75 {
            return Err(invalid());
        }
        let snapshot = self
            .store
            .repository_list_snapshot(access.principal().clone())
            .await?;
        if snapshot.repositories.len() > REPOSITORY_LIST_MAX_ENTRIES {
            return Err(M3Error::new(M3ErrorCode::RepositoryDocumentTooLarge));
        }
        let invalid_snapshot = || M3Error::new(M3ErrorCode::Internal);
        let jobs = snapshot
            .repositories
            .into_iter()
            .map(|record| {
                let id =
                    RepositoryId::parse(&record.repository_id).map_err(|_| invalid_snapshot())?;
                let revision = Revision::new(record.revision).ok_or_else(invalid_snapshot)?;
                let prefix = format!("refresh-all:{}:{id}", key.as_str());
                let first = IdempotencyKey::parse(format!("{prefix}:0")).map_err(|_| invalid())?;
                let retry = IdempotencyKey::parse(format!("{prefix}:1")).map_err(|_| invalid())?;
                Ok((id, revision, first, retry))
            })
            .collect::<Result<Vec<_>, M3Error>>()?;
        let mut outcomes = jobs
            .iter()
            .map(|(id, ..)| (*id, Ok(())))
            .collect::<Vec<_>>();
        let mut pending = jobs.into_iter().enumerate();
        let mut running = tokio::task::JoinSet::new();
        let mut indices = HashMap::new();
        loop {
            while running.len() < DOWNLOAD_CONCURRENCY {
                let Some((index, (id, revision, first, retry))) = pending.next() else {
                    break;
                };
                let application = self.clone();
                let access = access.clone();
                let task = running.spawn(async move {
                    with_log_caller(application.store.clone(), &access, async {
                        match application
                            .refresh_repository(&access, id, revision, first)
                            .await
                        {
                            Err(error) if error.code() == M3ErrorCode::RevisionConflict => {
                                let latest = application.get_repository(&access, id).await?;
                                application
                                    .refresh_repository(&access, id, latest.revision, retry)
                                    .await
                            }
                            result => result,
                        }
                    })
                    .await
                    .map(|_| ())
                    .map_err(|error| error.code())
                });
                indices.insert(task.id(), index);
            }
            let Some(joined) = running.join_next_with_id().await else {
                break;
            };
            let (id, result) = match joined {
                Ok(result) => result,
                Err(error) => (error.id(), Err(M3ErrorCode::Internal)),
            };
            let index = indices.remove(&id).ok_or_else(invalid_snapshot)?;
            outcomes[index].1 = result;
        }
        Ok(outcomes)
    }

    pub async fn unregister_repository(
        &self,
        access: &AccessContext,
        id: RepositoryId,
        expected: Revision,
        key: IdempotencyKey,
    ) -> Result<UnregisterResult<RepositoryId>, M3Error> {
        if !can_remove_repository(access, id) {
            return Err(M3Error::new(M3ErrorCode::PermissionDenied));
        }
        let _guard = self.locks.acquire(vec![ResourceKey::Repository(id)]).await;
        let old = match self
            .store
            .repository_credential_reference(access.principal().clone(), id)
            .await
        {
            Ok(reference) => reference,
            Err(error) if error.code() == M3ErrorCode::RepositoryNotRegistered => None,
            Err(error) => return Err(error),
        };
        let result = self
            .store
            .unregister_repository(access.principal().clone(), id, expected, key, m3_time_ms()?)
            .await?;
        if let Some(reference) = old.filter(|_| !result.replayed) {
            self.discard_credential_reference(reference).await;
        }
        Ok(result)
    }
}

fn require_repository_credentials(access: &AccessContext) -> Result<(), M3Error> {
    require_m3(access, Permission::RepositoriesManage)?;
    if access.principal() != &PrincipalId::local_owner() {
        return Err(M3Error::new(M3ErrorCode::PermissionDenied));
    }
    Ok(())
}

fn unused_credential_reference(result: &Result<SyncWrite<RepositoryRecord>, M3Error>) -> bool {
    match result {
        Ok(result) => result.replayed,
        Err(error) => !matches!(
            error.code(),
            M3ErrorCode::Internal | M3ErrorCode::StoreUnavailable
        ),
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
    async fn project_recovery_barrier_is_reserved_before_poll_and_released_on_task_cancellation() {
        let locks = ResourceLockCoordinator::default();
        let first = locks.project_recovery_guard();
        let scheduled = locks.project_recovery_guard();
        let notify = locks.project_recovery_notify();
        assert!(locks.project_recovery_pending());
        drop(first);
        assert!(locks.project_recovery_pending());
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), notify.notified())
                .await
                .is_err()
        );
        let task = tokio::spawn(async move {
            let _recovery = scheduled;
            std::future::pending::<()>().await;
        });
        task.abort();
        assert!(
            task.await
                .expect_err("cancelled recovery task")
                .is_cancelled()
        );
        assert!(!locks.project_recovery_pending());
        tokio::time::timeout(std::time::Duration::from_secs(1), notify.notified())
            .await
            .expect("last recovery release wakes discovery");
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
