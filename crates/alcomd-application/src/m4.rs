use std::collections::BTreeSet;
use std::future::Future;

use serde::{Deserialize, Serialize};

use crate::{
    AccessContext, IdempotencyKey, M3RegistryStore, OperationId, PlanId, PrincipalId, ProjectId,
    ProjectObservation, ProjectRecord, ResourceKey, ResourceLockCoordinator, Revision, StateStore,
    StoreErrorKind, UserPackageId,
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
    Reinstall,
    Bulk,
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
            Self::Reinstall => "reinstall",
            Self::Bulk => "bulk",
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
pub struct RepositoryPackageSourcePin {
    pub repository_id: String,
    pub repository_revision: u64,
    pub source_identity: String,
    #[serde(with = "sha256_hex")]
    pub manifest_fingerprint: [u8; 32],
    pub package_id: String,
    pub version: String,
    pub artifact_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_headers: Option<PackageDownloadHeaders>,
    #[serde(
        default,
        with = "optional_sha256_hex",
        skip_serializing_if = "Option::is_none"
    )]
    pub archive_sha256: Option<[u8; 32]>,
}

/// Locator only. The exact package fingerprint authorizes a transient read.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageDownloadHeaders {
    pub local_path: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPackageSourcePin {
    pub user_package_id: UserPackageId,
    pub source_revision: u64,
    pub source_identity: String,
    #[serde(with = "sha256_hex")]
    pub manifest_fingerprint: [u8; 32],
    pub package_id: String,
    pub version: String,
    #[serde(with = "sha256_hex")]
    pub archive_sha256: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PackageSourcePin {
    Repository(RepositoryPackageSourcePin),
    UserPackage(UserPackageSourcePin),
}

impl PackageSourcePin {
    #[must_use]
    pub fn package_id(&self) -> &str {
        match self {
            Self::Repository(value) => &value.package_id,
            Self::UserPackage(value) => &value.package_id,
        }
    }

    #[must_use]
    pub fn version(&self) -> &str {
        match self {
            Self::Repository(value) => &value.version,
            Self::UserPackage(value) => &value.version,
        }
    }

    #[must_use]
    pub const fn archive_sha256(&self) -> Option<[u8; 32]> {
        match self {
            Self::Repository(value) => value.archive_sha256,
            Self::UserPackage(value) => Some(value.archive_sha256),
        }
    }

    #[must_use]
    pub fn artifact_url(&self) -> Option<&str> {
        match self {
            Self::Repository(value) => Some(&value.artifact_url),
            Self::UserPackage(_) => None,
        }
    }

    #[must_use]
    pub const fn is_user_package(&self) -> bool {
        matches!(self, Self::UserPackage(_))
    }

    #[must_use]
    pub fn deterministic_source_key(&self) -> String {
        match self {
            Self::Repository(value) => format!("repository:{}", value.repository_id),
            Self::UserPackage(value) => format!("user-package:{}", value.user_package_id),
        }
    }
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackageRemovalConflict {
    pub package_id: String,
    pub required_by: Vec<PackageRemovalRequirement>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackageRemovalRequirement {
    pub package_id: String,
    pub range: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LegacyRemovalKind {
    File,
    Directory,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyRemoval {
    pub path: String,
    pub kind: LegacyRemovalKind,
    pub package_id: String,
    pub identity: Vec<u8>,
    #[serde(with = "sha256_hex")]
    pub fingerprint: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageChangeSet {
    pub format_version: u32,
    pub mutations: Vec<PackageMutation>,
    pub dependency_edges: Vec<PackageDependencyEdge>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removal_conflicts: Vec<PackageRemovalConflict>,
    // Internal frozen filesystem evidence; never projected to the public DTO.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub absent_removal_packages: Vec<String>,
    // Internal frozen reinstall intent; never projected to the public DTO.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub legacy_reinstall_packages: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub legacy_removals: Vec<LegacyRemoval>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub legacy_packages: Vec<String>,
    #[serde(with = "sha256_hex")]
    pub vpm_manifest_sha256: [u8; 32],
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unity_compatibility_overrides: Vec<PackageUnityCompatibilityOverride>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackageUnityCompatibilityOverride {
    pub package_id: String,
    pub version: String,
}

impl PackageChangeSet {
    pub fn validate_bounds(&self) -> Result<(), M4Error> {
        if !matches!(self.format_version, 1..=4)
            || self.mutations.len() > MAX_PACKAGE_MUTATIONS
            || self.dependency_edges.len() > MAX_DEPENDENCY_EDGES
            || !self.valid_removal_conflicts()
            || self.absent_removal_packages.len() > MAX_PACKAGE_MUTATIONS
            || (!self.absent_removal_packages.is_empty() && self.format_version != 4)
            || self
                .absent_removal_packages
                .windows(2)
                .any(|ids| ids[0] >= ids[1])
            || self.absent_removal_packages.iter().any(|id| {
                !self.mutations.iter().any(|mutation| {
                    mutation.package_id == *id
                        && mutation.kind == PackageMutationKind::Remove
                        && mutation.source.is_none()
                })
            })
            || self.legacy_reinstall_packages.len() > MAX_PACKAGE_MUTATIONS
            || (!self.legacy_reinstall_packages.is_empty() && self.format_version != 4)
            || self
                .legacy_reinstall_packages
                .windows(2)
                .any(|ids| ids[0] >= ids[1])
            || self.legacy_reinstall_packages.iter().any(|id| {
                !self.mutations.iter().any(|mutation| {
                    mutation.package_id == *id
                        && matches!(
                            mutation.kind,
                            PackageMutationKind::Install | PackageMutationKind::Replace
                        )
                        && mutation.source.is_some()
                        && mutation.to_version.is_some()
                })
            })
            || self.legacy_removals.len() > 4096
            || self.legacy_packages.len() > MAX_PACKAGE_MUTATIONS
            || self.unity_compatibility_overrides.len() > 1
            || (!self.unity_compatibility_overrides.is_empty() && self.format_version != 4)
            || self.unity_compatibility_overrides.iter().any(|item| {
                item.package_id.is_empty()
                    || item.package_id.len() > 128
                    || item.version.is_empty()
                    || item.version.len() > 256
                    || !self.mutations.iter().any(|mutation| {
                        mutation.package_id == item.package_id
                            && mutation.to_version.as_ref() == Some(&item.version)
                            && mutation.source.is_some()
                    })
            })
            || (self.format_version < 4
                && (!self.legacy_removals.is_empty() || !self.legacy_packages.is_empty()))
            || (self.format_version == 1
                && self.mutations.iter().any(|mutation| {
                    mutation
                        .source
                        .as_ref()
                        .is_some_and(PackageSourcePin::is_user_package)
                }))
            || (self.format_version == 2
                && !self.mutations.iter().any(|mutation| {
                    mutation
                        .source
                        .as_ref()
                        .is_some_and(PackageSourcePin::is_user_package)
                }))
        {
            return Err(M4Error::new(M4ErrorCode::PlanTooLarge));
        }
        Ok(())
    }

    fn valid_removal_conflicts(&self) -> bool {
        if self.removal_conflicts.is_empty() {
            return true;
        }
        if self.format_version != 4
            || self.removal_conflicts.len() > MAX_PACKAGE_MUTATIONS
            || self.removal_conflicts.iter().fold(0usize, |count, item| {
                count.saturating_add(item.required_by.len())
            }) > MAX_DEPENDENCY_EDGES
            || self
                .mutations
                .iter()
                .any(|item| item.kind != PackageMutationKind::Remove)
            || self
                .removal_conflicts
                .windows(2)
                .any(|items| items[0].package_id >= items[1].package_id)
        {
            return false;
        }
        let removed = self
            .mutations
            .iter()
            .map(|item| item.package_id.as_str())
            .collect::<BTreeSet<_>>();
        let edges = self
            .dependency_edges
            .iter()
            .filter(|edge| !edge.direct)
            .map(|edge| {
                (
                    edge.from_package_id.as_str(),
                    edge.to_package_id.as_str(),
                    edge.range.as_str(),
                )
            })
            .collect::<BTreeSet<_>>();
        self.removal_conflicts.iter().all(|item| {
            !item.required_by.is_empty()
                && !item.package_id.is_empty()
                && item.package_id.len() <= 128
                && removed.contains(item.package_id.as_str())
                && !item
                    .required_by
                    .windows(2)
                    .any(|items| items[0] >= items[1])
                && item.required_by.iter().all(|required| {
                    !required.package_id.is_empty()
                        && required.package_id.len() <= 128
                        && !required.range.is_empty()
                        && required.range.len() <= 1_024
                        && !removed.contains(required.package_id.as_str())
                        && edges.contains(&(
                            required.package_id.as_str(),
                            item.package_id.as_str(),
                            required.range.as_str(),
                        ))
                })
        })
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
pub enum ResolverCatalogSource {
    Repository {
        repository_locator: String,
        repository_id: String,
        repository_revision: u64,
        repository_priority: u64,
        source_identity: String,
        artifact_url: String,
        credential_reference: Option<String>,
        download_headers_present: bool,
        zip_sha256: Option<String>,
        local_zip_sha256: Option<[u8; 32]>,
        manifest_fingerprint: [u8; 32],
    },
    UserPackage {
        user_package_id: UserPackageId,
        source_revision: u64,
        source_identity: String,
        archive_sha256: [u8; 32],
        manifest_fingerprint: [u8; 32],
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolverCatalogEntry {
    pub source: ResolverCatalogSource,
    pub package_id: String,
    pub version: String,
    pub yanked: bool,
    pub unity: Option<String>,
    pub author_name: Option<String>,
    pub author_email: Option<String>,
    pub unity_release: Option<String>,
    pub dependencies_json: String,
    pub legacy_metadata_present: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolverCatalog {
    pub entries: Vec<ResolverCatalogEntry>,
    pub complete: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackagePlanRequest {
    pub allow_unity_incompatible: bool,
    pub action: PlanAction,
    pub project_id: ProjectId,
    pub expected_revision: Revision,
    pub package_id: Option<String>,
    pub version_range: Option<String>,
    pub repository_id: Option<String>,
    pub include_prerelease: bool,
    pub plan_version: PackagePlanVersion,
    pub source_selector: Option<PackageSourceSelector>,
    pub reinstall_selection: Option<ReinstallSelection>,
    pub reinstall_sources: Vec<(String, PackageSourceSelector)>,
    pub bulk_intents: Vec<PackageBulkIntent>,
}

impl PackagePlanRequest {
    #[must_use]
    pub fn is_removal_only(&self) -> bool {
        self.action == PlanAction::Remove
            || (self.action == PlanAction::Bulk
                && !self.bulk_intents.is_empty()
                && self
                    .bulk_intents
                    .iter()
                    .all(|intent| matches!(intent, PackageBulkIntent::Remove { .. })))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackagePlanVersion {
    V1,
    V2,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PackageSourceSelector {
    Repository { repository_id: String },
    UserPackage { user_package_id: UserPackageId },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReinstallSelection {
    Packages(Vec<String>),
    All,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PackageBulkIntent {
    Install {
        package_id: String,
        version_range: Option<String>,
        source: Option<PackageSourceSelector>,
        include_prerelease: bool,
    },
    Upgrade {
        package_id: String,
        version_range: Option<String>,
        source: Option<PackageSourceSelector>,
        include_prerelease: bool,
    },
    Remove {
        package_id: String,
    },
    Reinstall {
        package_id: String,
        source: Option<PackageSourceSelector>,
    },
}

impl PackageBulkIntent {
    #[must_use]
    pub fn package_id(&self) -> &str {
        match self {
            Self::Install { package_id, .. }
            | Self::Upgrade { package_id, .. }
            | Self::Remove { package_id }
            | Self::Reinstall { package_id, .. } => package_id,
        }
    }
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
    PreflightComplete,
    QuarantineIntent,
    RootQuarantined,
    RegistryCommitIntent,
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
    Deleting,
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
            Self::PreflightComplete => "preflight_complete",
            Self::QuarantineIntent => "quarantine_intent",
            Self::RootQuarantined => "root_quarantined",
            Self::RegistryCommitIntent => "registry_commit_intent",
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
            Self::Deleting => "deleting",
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
    PackageNotInstalled,
    PackageIntentConflict,
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
            Self::PackageNotInstalled => "package_not_installed",
            Self::PackageIntentConflict => "package_intent_conflict",
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
    context: crate::TechnicalErrorContext,
    diagnostic_id: Option<String>,
}

impl M4Error {
    #[must_use]
    pub fn new(code: M4ErrorCode) -> Self {
        Self {
            code,
            subreason: None,
            context: crate::TechnicalErrorContext::default(),
            diagnostic_id: None,
        }
    }
    #[must_use]
    pub const fn code(&self) -> M4ErrorCode {
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

    #[must_use]
    pub fn with_subreason(code: M4ErrorCode, subreason: impl Into<String>) -> Self {
        let mut error = Self::new(code);
        error.subreason = Some(subreason.into());
        error
    }
    #[must_use]
    pub fn subreason(&self) -> Option<&str> {
        self.subreason.as_deref()
    }
}

impl std::fmt::Display for M4Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "request failed: {:?}", self.code)?;
        if let Some(reason) = self.subreason.as_deref() {
            write!(formatter, " ({reason})")?;
        }
        if let Some(path) = self.path() {
            write!(formatter, " at {path}")?;
        }
        Ok(())
    }
}
impl std::error::Error for M4Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.context.source_error()
    }
}

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

pub trait RepositoryArchivePinStore: Clone + Send + Sync + 'static {
    fn pin_repository_archive(
        &self,
        owner: PrincipalId,
        pin: RepositoryPackageSourcePin,
    ) -> impl Future<Output = Result<(), M4Error>> + Send;
}

pub trait M4Store: RepositoryArchivePinStore {
    fn resolver_catalog(
        &self,
        owner: PrincipalId,
        include_user_packages: bool,
        settings: crate::ConfigPackageSettings,
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
    fn source_settings(
        &self,
    ) -> impl Future<Output = Result<crate::ConfigPackageSettings, M4Error>> + Send;
    fn prepare_plan(
        &self,
        owner: PrincipalId,
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

/// Revalidate current source authority even when a frozen user archive can outlive enrollment.
pub fn validate_package_source_settings(
    settings: &crate::ConfigPackageSettings,
    sources: &[PackageSourcePin],
) -> Result<(), M4Error> {
    let disabled = sources.iter().any(|source| {
        (!settings.show_prerelease
            && source
                .version()
                .split('+')
                .next()
                .unwrap_or_default()
                .contains('-'))
            || match source {
                PackageSourcePin::Repository(pin) => {
                    settings.repository_package_hidden(&pin.repository_id, &pin.package_id)
                }
                PackageSourcePin::UserPackage(_) => settings.hide_local_user_packages,
            }
    });
    if disabled {
        Err(M4Error::with_subreason(
            M4ErrorCode::PlanStale,
            "source_authority_changed",
        ))
    } else {
        Ok(())
    }
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
        if request.allow_unity_incompatible
            && (!matches!(
                request.action,
                PlanAction::Install | PlanAction::Upgrade | PlanAction::Downgrade
            ) || request.plan_version != PackagePlanVersion::V2
                || request.package_id.is_none()
                || !request
                    .version_range
                    .as_deref()
                    .is_some_and(|range| range.starts_with('=') && range.len() > 1))
        {
            return Err(M4Error::new(M4ErrorCode::InvalidInput));
        }
        let project = if access.is_mcp_client() {
            access
                .require_project_read_scope(&request.project_id.to_string())
                .map_err(|_| M4Error::new(M4ErrorCode::PermissionDenied))?;
            self.store
                .get_mcp_project(access.principal().clone(), request.project_id)
                .await
                .map_err(map_m3_error)?
        } else {
            self.store
                .get_project(access.principal().clone(), request.project_id)
                .await
                .map_err(map_m3_error)?
        };
        if project.revision != request.expected_revision {
            return Err(M4Error::new(M4ErrorCode::RevisionConflict));
        }
        let catalog = if request.is_removal_only() {
            ResolverCatalog {
                entries: Vec::new(),
                complete: true,
            }
        } else {
            self.store
                .resolver_catalog(
                    access.principal().clone(),
                    request.plan_version == PackagePlanVersion::V2,
                    self.adapter.source_settings().await?,
                )
                .await?
        };
        if request.action != PlanAction::Remove && !catalog.complete {
            return Err(M4Error::new(M4ErrorCode::RepositoryRefreshRequired));
        }
        let action = format!("packages.{}", request.action.as_str());
        let requested_override =
            request
                .allow_unity_incompatible
                .then(|| PackageUnityCompatibilityOverride {
                    package_id: request.package_id.clone().unwrap_or_default(),
                    version: request
                        .version_range
                        .as_deref()
                        .and_then(|range| range.strip_prefix('='))
                        .unwrap_or_default()
                        .to_owned(),
                });
        let removal_only = request.is_removal_only();
        let target = crate::log_path("project", project.observation.root_path.clone());
        let draft = crate::log_execution_step(
            &action,
            "resolve_dependencies",
            Some(target),
            self.adapter
                .prepare_plan(access.principal().clone(), project, catalog, request),
            |error| error.path().map(str::to_owned),
        )
        .await?;
        if !removal_only && !draft.change_set.removal_conflicts.is_empty() {
            return Err(M4Error::new(M4ErrorCode::Internal));
        }
        if draft.change_set.unity_compatibility_overrides
            != requested_override.into_iter().collect::<Vec<_>>()
        {
            return Err(M4Error::new(M4ErrorCode::Internal));
        }
        validate_package_source_settings(
            &self.adapter.source_settings().await?,
            &draft.source_set,
        )?;
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
        let catalog = if plan.source_set.is_empty() {
            ResolverCatalog {
                entries: Vec::new(),
                complete: true,
            }
        } else {
            self.store
                .resolver_catalog(
                    access.principal().clone(),
                    plan.source_set
                        .iter()
                        .any(PackageSourcePin::is_user_package),
                    self.adapter.source_settings().await?,
                )
                .await?
        };
        let target = crate::log_path("project", project.observation.root_path.clone());
        let action = format!("packages.{}", plan.action.as_str());
        crate::log_execution_step(
            &action,
            "check_plan",
            Some(target.clone()),
            self.adapter.revalidate_plan(project, catalog, plan),
            |error| error.path().map(str::to_owned),
        )
        .await?;
        let outcome = crate::log_transaction_step(
            &action,
            "accepted",
            Some(target),
            self.store.accept_package_plan(
                access.principal().clone(),
                plan_id,
                expected_revision,
                idempotency_key,
                m4_time_ms()?,
            ),
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
                let (error, diagnostic_id) = error.ensure_diagnostic_id();
                if error.code() == M4ErrorCode::RecoveryRequired {
                    return Err(error);
                }
                crate::with_operation_log_failure(
                    operation_id,
                    crate::operation_log_error(&error, error.path()),
                    self.store.fail_package_apply(
                        operation_id,
                        error.code().as_str().to_owned(),
                        diagnostic_id,
                        m4_time_ms()?,
                    ),
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
        .map_err(|cause| M4Error::new(M4ErrorCode::PermissionDenied).with_source(cause))
}

fn map_m3_error(error: crate::M3Error) -> M4Error {
    let path = error.technical_path().map(str::to_owned);
    let mapped = match error.code() {
        crate::M3ErrorCode::ProjectNotRegistered => M4Error::new(M4ErrorCode::ProjectNotRegistered),
        crate::M3ErrorCode::PermissionDenied => M4Error::new(M4ErrorCode::PermissionDenied),
        crate::M3ErrorCode::StoreUnavailable => M4Error::new(M4ErrorCode::StoreUnavailable),
        _ => M4Error::new(M4ErrorCode::Internal),
    };
    let mapped = mapped.with_source(error);
    if let Some(path) = path {
        mapped.with_path(path)
    } else {
        mapped
    }
}

fn m4_time_ms() -> Result<u64, M4Error> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|cause| M4Error::new(M4ErrorCode::Internal).with_source(cause))
        .and_then(|duration| {
            u64::try_from(duration.as_millis())
                .map_err(|cause| M4Error::new(M4ErrorCode::Internal).with_source(cause))
        })
}

mod optional_sha256_hex {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(
        value: &Option<[u8; 32]>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(digest) => super::sha256_hex::serialize(digest, serializer),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<[u8; 32]>, D::Error> {
        #[derive(Deserialize)]
        struct Digest(#[serde(with = "super::sha256_hex")] [u8; 32]);
        Option::<Digest>::deserialize(deserializer).map(|value| value.map(|digest| digest.0))
    }
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
            unity_compatibility_overrides: Vec::new(),
            removal_conflicts: Vec::new(),
            absent_removal_packages: Vec::new(),
            legacy_reinstall_packages: Vec::new(),
            format_version: 1,
            mutations: Vec::new(),
            dependency_edges: Vec::new(),
            legacy_removals: Vec::new(),
            legacy_packages: Vec::new(),
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

#[cfg(test)]
mod error_identity_tests {
    use super::*;
    #[test]
    fn operation_failure_preserves_causes_and_only_a_new_failure_gets_a_new_identity() {
        let path = "C:/ALCOMD-Test/项目 A/Packages/vpm-manifest.json";
        let failure = || {
            M4Error::new(M4ErrorCode::ProjectChangedDuringApply)
                .with_path(path)
                .with_source(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "test underlying file permission failure",
                ))
        };
        let (source, first) = failure().ensure_diagnostic_id();
        let (propagated, again) = source.ensure_diagnostic_id();
        assert_eq!(first, again);
        assert_eq!(propagated.diagnostic_id(), Some(first.as_str()));
        assert_eq!(propagated.path(), Some(path));
        assert!(std::error::Error::source(&propagated).is_some());
        assert!(
            crate::technical_error_chain(&propagated)
                .contains("test underlying file permission failure")
        );
        let (_, retry) = failure().ensure_diagnostic_id();
        assert_ne!(first, retry);
    }
}
