use std::fmt;
use std::future::Future;

use serde::{Deserialize, Serialize};

use crate::{
    AccessContext, IdempotencyKey, M3RegistryStore, OperationId, PlanId, PrincipalId, ProjectId,
    ProjectObservation, ProjectRecord, ResourceKey, ResourceLockCoordinator, Revision, StateStore,
    StoreErrorKind,
};

pub const MAX_PACKAGE_MUTATIONS: usize = 1_024;
pub const MAX_DEPENDENCY_EDGES: usize = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlanAction {
    Install,
    Remove,
    Upgrade,
    Downgrade,
    Resolve,
}

impl PlanAction {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Remove => "remove",
            Self::Upgrade => "upgrade",
            Self::Downgrade => "downgrade",
            Self::Resolve => "resolve",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlanState {
    Unapplied,
    Applied,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageSourcePin {
    pub repository_id: String,
    pub repository_revision: u64,
    pub source_identity: String,
    #[serde(with = "sha256_hex")]
    pub manifest_fingerprint: [u8; 32],
    pub package_id: String,
    pub version: String,
    pub artifact_url: String,
    #[serde(with = "sha256_hex")]
    pub archive_sha256: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageMutationKind {
    Install,
    Remove,
    Replace,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageMutation {
    pub kind: PackageMutationKind,
    pub package_id: String,
    pub from_version: Option<String>,
    pub to_version: Option<String>,
    pub source: Option<PackageSourcePin>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageDependencyEdge {
    pub from_package_id: String,
    pub to_package_id: String,
    pub range: String,
    pub direct: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageChangeSet {
    pub format_version: u32,
    pub mutations: Vec<PackageMutation>,
    pub dependency_edges: Vec<PackageDependencyEdge>,
    #[serde(with = "sha256_hex")]
    pub vpm_manifest_sha256: [u8; 32],
}

impl PackageChangeSet {
    pub fn validate_bounds(&self) -> Result<(), M4Error> {
        if self.format_version != 1
            || self.mutations.len() > MAX_PACKAGE_MUTATIONS
            || self.dependency_edges.len() > MAX_DEPENDENCY_EDGES
        {
            return Err(M4Error::new(M4ErrorCode::PlanTooLarge));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackagePlanDraft {
    pub project_id: ProjectId,
    pub action: PlanAction,
    pub project_revision: Revision,
    pub project_snapshot_fingerprint: [u8; 32],
    pub change_set_fingerprint: [u8; 32],
    pub change_set: PackageChangeSet,
    pub source_set: Vec<PackageSourcePin>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackagePlanRecord {
    pub plan_id: PlanId,
    pub owner: PrincipalId,
    pub project_id: ProjectId,
    pub action: PlanAction,
    pub state: PlanState,
    pub project_revision: Revision,
    pub project_snapshot_fingerprint: [u8; 32],
    pub change_set_fingerprint: [u8; 32],
    pub change_set: PackageChangeSet,
    pub source_set: Vec<PackageSourcePin>,
    pub apply_operation_id: Option<OperationId>,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolverCatalogEntry {
    pub repository_id: String,
    pub repository_revision: u64,
    pub repository_priority: u64,
    pub source_identity: String,
    pub package_id: String,
    pub version: String,
    pub yanked: bool,
    pub unity: Option<String>,
    pub author_name: String,
    pub author_email: String,
    pub artifact_url: String,
    pub zip_sha256: String,
    pub unity_release: Option<String>,
    pub dependencies_json: String,
    pub manifest_fingerprint: [u8; 32],
    pub legacy_metadata_present: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolverCatalog {
    pub entries: Vec<ResolverCatalogEntry>,
    pub complete: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackagePlanRequest {
    pub action: PlanAction,
    pub project_id: ProjectId,
    pub expected_revision: Revision,
    pub package_id: Option<String>,
    pub version_range: Option<String>,
    pub repository_id: Option<String>,
    pub include_prerelease: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageApplyCompletion {
    pub project_observation: ProjectObservation,
    pub result_json: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyPlanOutcome {
    pub operation_id: OperationId,
    pub replayed: bool,
    #[serde(skip)]
    pub schedule: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilesystemPhase {
    Accepted,
    InventoryReady,
    Archiving,
    ArchiveReady,
    PublishIntent,
    ArchivePublished,
    ArchiveVerified,
    Extracting,
    Staging,
    StagingComplete,
    TargetPublished,
    ProjectRegistryCommitIntent,
    Extracted,
    Prepared,
    PackagesReplaced,
    VpmManifestCommitted,
    FilesystemCommitted,
    StateCommitted,
    CleanupComplete,
    RollingBack,
    RolledBack,
    RecoveryRequired,
}

impl FilesystemPhase {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::InventoryReady => "inventory_ready",
            Self::Archiving => "archiving",
            Self::ArchiveReady => "archive_ready",
            Self::PublishIntent => "publish_intent",
            Self::ArchivePublished => "archive_published",
            Self::ArchiveVerified => "archive_verified",
            Self::Extracting => "extracting",
            Self::Staging => "staging",
            Self::StagingComplete => "staging_complete",
            Self::TargetPublished => "target_published",
            Self::ProjectRegistryCommitIntent => "project_registry_commit_intent",
            Self::Extracted => "extracted",
            Self::Prepared => "prepared",
            Self::PackagesReplaced => "packages_replaced",
            Self::VpmManifestCommitted => "vpm_manifest_committed",
            Self::FilesystemCommitted => "filesystem_committed",
            Self::StateCommitted => "state_committed",
            Self::CleanupComplete => "cleanup_complete",
            Self::RollingBack => "rolling_back",
            Self::RolledBack => "rolled_back",
            Self::RecoveryRequired => "recovery_required",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JournalState {
    Intent,
    Completed,
}

impl JournalState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Intent => "intent",
            Self::Completed => "completed",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilesystemJournalEntry {
    pub operation_id: OperationId,
    pub step: u64,
    pub plan_id: PlanId,
    pub project_id: ProjectId,
    pub phase: FilesystemPhase,
    pub state: JournalState,
    pub project_identity_key: Vec<u8>,
    pub change_set_fingerprint: [u8; 32],
    pub evidence_json: String,
    pub updated_at_ms: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum M4ErrorCode {
    InvalidInput,
    PermissionDenied,
    ProjectNotRegistered,
    RepositoryRefreshRequired,
    PackageNotFound,
    PackageVersionInvalid,
    PackageRangeInvalid,
    PackageDependencyMissing,
    PackageDependencyConflict,
    PackageUnityIncompatible,
    PackageSourceAmbiguous,
    PackageManifestInvalid,
    PackageHashRequired,
    PackageLegacyCleanupRequired,
    PackageVersionYanked,
    PlanNotFound,
    PlanStale,
    PlanTooLarge,
    IdempotencyConflict,
    RevisionConflict,
    ProjectChangedDuringApply,
    PackageIntegrityMismatch,
    PackageDownloadTooLarge,
    OfflineCacheMiss,
    PackageCacheCorrupt,
    PackageCacheQuotaExceeded,
    PackageArchiveInvalid,
    PackageArchiveUnsafePath,
    PackagePathCollision,
    PackageArchiveUnsupportedCompression,
    PackageArchiveLimitExceeded,
    RecoveryRequired,
    OperationCancelled,
    StoreUnavailable,
    Internal,
}

impl M4ErrorCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::PermissionDenied => "permission_denied",
            Self::ProjectNotRegistered => "project_not_registered",
            Self::RepositoryRefreshRequired => "repository_refresh_required",
            Self::PackageNotFound => "package_not_found",
            Self::PackageVersionInvalid => "package_version_invalid",
            Self::PackageRangeInvalid => "package_range_invalid",
            Self::PackageDependencyMissing => "package_dependency_missing",
            Self::PackageDependencyConflict => "package_dependency_conflict",
            Self::PackageUnityIncompatible => "package_unity_incompatible",
            Self::PackageSourceAmbiguous => "package_source_ambiguous",
            Self::PackageManifestInvalid => "package_manifest_invalid",
            Self::PackageHashRequired => "package_hash_required",
            Self::PackageLegacyCleanupRequired => "package_legacy_cleanup_required",
            Self::PackageVersionYanked => "package_version_yanked",
            Self::PlanNotFound => "plan_not_found",
            Self::PlanStale => "plan_stale",
            Self::PlanTooLarge => "plan_too_large",
            Self::IdempotencyConflict => "idempotency_conflict",
            Self::RevisionConflict => "revision_conflict",
            Self::ProjectChangedDuringApply => "project_changed_during_apply",
            Self::PackageIntegrityMismatch => "package_integrity_mismatch",
            Self::PackageDownloadTooLarge => "package_download_too_large",
            Self::OfflineCacheMiss => "offline_cache_miss",
            Self::PackageCacheCorrupt => "package_cache_corrupt",
            Self::PackageCacheQuotaExceeded => "package_cache_quota_exceeded",
            Self::PackageArchiveInvalid => "package_archive_invalid",
            Self::PackageArchiveUnsafePath => "package_path_invalid",
            Self::PackagePathCollision => "package_path_collision",
            Self::PackageArchiveUnsupportedCompression => "package_archive_unsupported_compression",
            Self::PackageArchiveLimitExceeded => "package_archive_limit_exceeded",
            Self::RecoveryRequired => "project_transaction_recovery_required",
            Self::OperationCancelled => "operation_cancelled",
            Self::StoreUnavailable => "store_unavailable",
            Self::Internal => "internal_error",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct M4Error {
    code: M4ErrorCode,
    subreason: Option<String>,
}

impl M4Error {
    #[must_use]
    pub const fn new(code: M4ErrorCode) -> Self {
        Self {
            code,
            subreason: None,
        }
    }

    #[must_use]
    pub fn with_subreason(code: M4ErrorCode, subreason: impl Into<String>) -> Self {
        Self {
            code,
            subreason: Some(subreason.into()),
        }
    }

    #[must_use]
    pub const fn code(&self) -> M4ErrorCode {
        self.code
    }

    #[must_use]
    pub fn subreason(&self) -> Option<&str> {
        self.subreason.as_deref()
    }
}

impl fmt::Display for M4Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "M4 request failed: {:?}", self.code)
    }
}

impl std::error::Error for M4Error {}

impl From<StoreErrorKind> for M4Error {
    fn from(value: StoreErrorKind) -> Self {
        let code = match value {
            StoreErrorKind::RevisionConflict => M4ErrorCode::RevisionConflict,
            StoreErrorKind::IdempotencyConflict => M4ErrorCode::IdempotencyConflict,
            StoreErrorKind::Unavailable => M4ErrorCode::StoreUnavailable,
            StoreErrorKind::CorruptState
            | StoreErrorKind::OperationNotFound
            | StoreErrorKind::OperationNotCancellable => M4ErrorCode::Internal,
        };
        Self::new(code)
    }
}

pub trait M4Store: Clone + Send + Sync + 'static {
    fn resolver_catalog(
        &self,
        owner: PrincipalId,
    ) -> impl Future<Output = Result<ResolverCatalog, M4Error>> + Send;

    fn create_package_plan(
        &self,
        owner: PrincipalId,
        draft: PackagePlanDraft,
        created_at_ms: u64,
    ) -> impl Future<Output = Result<PackagePlanRecord, M4Error>> + Send;

    fn get_package_plan(
        &self,
        owner: PrincipalId,
        plan_id: PlanId,
    ) -> impl Future<Output = Result<PackagePlanRecord, M4Error>> + Send;

    fn accept_package_plan(
        &self,
        owner: PrincipalId,
        plan_id: PlanId,
        expected_revision: Revision,
        idempotency_key: IdempotencyKey,
        created_at_ms: u64,
    ) -> impl Future<Output = Result<ApplyPlanOutcome, M4Error>> + Send;

    fn append_filesystem_journal(
        &self,
        entry: FilesystemJournalEntry,
    ) -> impl Future<Output = Result<(), M4Error>> + Send;

    fn next_filesystem_journal_step(
        &self,
        operation_id: OperationId,
    ) -> impl Future<Output = Result<u64, M4Error>> + Send;

    fn begin_package_apply(
        &self,
        operation_id: OperationId,
        updated_at_ms: u64,
    ) -> impl Future<Output = Result<PackagePlanRecord, M4Error>> + Send;

    fn complete_package_apply(
        &self,
        operation_id: OperationId,
        completion: PackageApplyCompletion,
        completed_at_ms: u64,
    ) -> impl Future<Output = Result<(), M4Error>> + Send;

    fn fail_package_apply(
        &self,
        operation_id: OperationId,
        error_code: String,
        diagnostic_id: String,
        completed_at_ms: u64,
    ) -> impl Future<Output = Result<(), M4Error>> + Send;

    fn recover_package_operations(
        &self,
        recovered_at_ms: u64,
    ) -> impl Future<Output = Result<Vec<OperationId>, M4Error>> + Send;
}

pub trait M4PackageAdapter: Clone + Send + Sync + 'static {
    fn prepare_plan(
        &self,
        project: ProjectRecord,
        catalog: ResolverCatalog,
        request: PackagePlanRequest,
    ) -> impl Future<Output = Result<PackagePlanDraft, M4Error>> + Send;

    fn revalidate_plan(
        &self,
        project: ProjectRecord,
        catalog: ResolverCatalog,
        plan: PackagePlanRecord,
    ) -> impl Future<Output = Result<(), M4Error>> + Send;

    fn execute_plan(
        &self,
        operation_id: OperationId,
        project: ProjectRecord,
        plan: PackagePlanRecord,
        locks: std::sync::Arc<ResourceLockCoordinator>,
    ) -> impl Future<Output = Result<PackageApplyCompletion, M4Error>> + Send;
}

#[derive(Clone)]
pub struct M4Application<S, A> {
    store: S,
    adapter: A,
    locks: std::sync::Arc<ResourceLockCoordinator>,
}

impl<S, A> M4Application<S, A>
where
    S: M3RegistryStore + M4Store + StateStore,
    A: M4PackageAdapter,
{
    #[must_use]
    pub fn new(store: S, adapter: A) -> Self {
        Self::with_locks(
            store,
            adapter,
            std::sync::Arc::new(ResourceLockCoordinator::default()),
        )
    }

    #[must_use]
    pub fn with_locks(
        store: S,
        adapter: A,
        locks: std::sync::Arc<ResourceLockCoordinator>,
    ) -> Self {
        Self {
            store,
            adapter,
            locks,
        }
    }

    pub async fn recover(&self) -> Result<(), M4Error> {
        let operations = self.store.recover_package_operations(m4_time_ms()?).await?;
        for operation_id in operations {
            self.schedule(operation_id);
        }
        Ok(())
    }

    pub async fn plan(
        &self,
        access: &AccessContext,
        request: PackagePlanRequest,
    ) -> Result<PackagePlanRecord, M4Error> {
        require(access, crate::Permission::ProjectsRead)?;
        require(access, crate::Permission::RepositoriesRead)?;
        require(access, crate::Permission::PackagesRead)?;
        let project = self
            .store
            .get_project(access.principal().clone(), request.project_id)
            .await
            .map_err(map_m3_error)?;
        if project.revision != request.expected_revision {
            return Err(M4Error::new(M4ErrorCode::RevisionConflict));
        }
        let catalog = self
            .store
            .resolver_catalog(access.principal().clone())
            .await?;
        if request.action != PlanAction::Remove && !catalog.complete {
            return Err(M4Error::new(M4ErrorCode::RepositoryRefreshRequired));
        }
        let draft = self.adapter.prepare_plan(project, catalog, request).await?;
        self.store
            .create_package_plan(access.principal().clone(), draft, m4_time_ms()?)
            .await
    }

    pub async fn apply_plan(
        &self,
        access: &AccessContext,
        plan_id: PlanId,
        expected_revision: Revision,
        idempotency_key: IdempotencyKey,
    ) -> Result<ApplyPlanOutcome, M4Error> {
        require(access, crate::Permission::ProjectsRead)?;
        require(access, crate::Permission::RepositoriesRead)?;
        require(access, crate::Permission::PackagesRead)?;
        require(access, crate::Permission::PackagesManage)?;
        if access.principal().as_str() != PrincipalId::LOCAL_OWNER {
            return Err(M4Error::new(M4ErrorCode::PermissionDenied));
        }
        let plan = self
            .store
            .get_package_plan(access.principal().clone(), plan_id)
            .await?;
        if plan.state == PlanState::Applied {
            return self
                .store
                .accept_package_plan(
                    access.principal().clone(),
                    plan_id,
                    expected_revision,
                    idempotency_key,
                    m4_time_ms()?,
                )
                .await;
        }
        let _guard = self
            .locks
            .acquire(vec![ResourceKey::Project(plan.project_id)])
            .await;
        let project = self
            .store
            .get_project(access.principal().clone(), plan.project_id)
            .await
            .map_err(map_m3_error)?;
        let catalog = self
            .store
            .resolver_catalog(access.principal().clone())
            .await?;
        self.adapter.revalidate_plan(project, catalog, plan).await?;
        let outcome = self
            .store
            .accept_package_plan(
                access.principal().clone(),
                plan_id,
                expected_revision,
                idempotency_key,
                m4_time_ms()?,
            )
            .await?;
        if outcome.schedule {
            self.schedule(outcome.operation_id);
        }
        Ok(outcome)
    }

    fn schedule(&self, operation_id: OperationId) {
        let application = self.clone();
        tokio::spawn(async move {
            let _ = application.run(operation_id).await;
        });
    }

    async fn run(&self, operation_id: OperationId) -> Result<(), M4Error> {
        if self
            .store
            .cancellation_requested(operation_id)
            .await
            .map_err(|error| M4Error::from(error.kind()))?
        {
            let _ = self
                .store
                .finish_cancelled(operation_id, m4_time_ms()?)
                .await
                .map_err(|error| M4Error::from(error.kind()))?;
            return Ok(());
        }
        let plan = self
            .store
            .begin_package_apply(operation_id, m4_time_ms()?)
            .await?;
        let project = self
            .store
            .get_project(plan.owner.clone(), plan.project_id)
            .await
            .map_err(map_m3_error)?;
        match self
            .adapter
            .execute_plan(
                operation_id,
                project,
                plan,
                std::sync::Arc::clone(&self.locks),
            )
            .await
        {
            Ok(completion) => {
                self.store
                    .complete_package_apply(operation_id, completion, m4_time_ms()?)
                    .await
            }
            Err(error) => {
                if error.code() == M4ErrorCode::OperationCancelled {
                    let _ = self
                        .store
                        .finish_cancelled(operation_id, m4_time_ms()?)
                        .await
                        .map_err(|error| M4Error::from(error.kind()))?;
                    return Ok(());
                }
                if error.code() == M4ErrorCode::RecoveryRequired {
                    return Err(error);
                }
                self.store
                    .fail_package_apply(
                        operation_id,
                        error.code().as_str().to_owned(),
                        OperationId::new().to_string(),
                        m4_time_ms()?,
                    )
                    .await?;
                Ok(())
            }
        }
    }
}

fn require(access: &AccessContext, permission: crate::Permission) -> Result<(), M4Error> {
    access
        .require(permission)
        .map_err(|_| M4Error::new(M4ErrorCode::PermissionDenied))
}

fn map_m3_error(error: crate::M3Error) -> M4Error {
    match error.code() {
        crate::M3ErrorCode::ProjectNotRegistered => M4Error::new(M4ErrorCode::ProjectNotRegistered),
        crate::M3ErrorCode::PermissionDenied => M4Error::new(M4ErrorCode::PermissionDenied),
        crate::M3ErrorCode::StoreUnavailable => M4Error::new(M4ErrorCode::StoreUnavailable),
        _ => M4Error::new(M4ErrorCode::Internal),
    }
}

fn m4_time_ms() -> Result<u64, M4Error> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| M4Error::new(M4ErrorCode::Internal))
        .and_then(|duration| {
            u64::try_from(duration.as_millis()).map_err(|_| M4Error::new(M4ErrorCode::Internal))
        })
}

mod sha256_hex {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(value: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error> {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut result = String::with_capacity(64);
        for byte in value {
            result.push(char::from(HEX[(byte >> 4) as usize]));
            result.push(char::from(HEX[(byte & 0x0f) as usize]));
        }
        serializer.serialize_str(&result)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[u8; 32], D::Error> {
        let value = String::deserialize(deserializer)?;
        if value.len() != 64
            || value
                .bytes()
                .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
        {
            return Err(serde::de::Error::custom("invalid SHA-256"));
        }
        let mut digest = [0_u8; 32];
        for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
            digest[index] = (nibble(pair[0]) << 4) | nibble(pair[1]);
        }
        Ok(digest)
    }

    fn nibble(value: u8) -> u8 {
        match value {
            b'0'..=b'9' => value - b'0',
            b'a'..=b'f' => value - b'a' + 10,
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn change_set_limits_are_not_truncating() {
        let mut change_set = PackageChangeSet {
            format_version: 1,
            mutations: Vec::new(),
            dependency_edges: Vec::new(),
            vpm_manifest_sha256: [0; 32],
        };
        assert!(change_set.validate_bounds().is_ok());
        change_set
            .mutations
            .resize_with(MAX_PACKAGE_MUTATIONS + 1, || PackageMutation {
                kind: PackageMutationKind::Remove,
                package_id: "com.example.package".to_owned(),
                from_version: Some("1.0.0".to_owned()),
                to_version: None,
                source: None,
            });
        assert_eq!(
            change_set.validate_bounds().expect_err("too large").code(),
            M4ErrorCode::PlanTooLarge
        );
    }
}
