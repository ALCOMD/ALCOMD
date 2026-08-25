use std::fmt;
use std::future::Future;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{
    AccessContext, ApplicationError, IdempotencyKey, OperationId, Permission, PlanId, PrincipalId,
    ResourceKey, ResourceLockCoordinator, Revision,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionSourceKind {
    NotApplicable,
    LocalOwnerSelected,
    FirstPartyPackaged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionTrustDecision {
    Official,
    UserApprovedForExtension,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionDesiredState {
    InstalledDisabled,
    Enabled,
    Uninstalling,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionQuarantineState {
    Clear,
    Quarantined,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionRuntimeState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Crashed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionDataDisposition {
    RetainData,
    DeleteData,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExtensionUiProtocol {
    PortableV1,
}

impl ExtensionUiProtocol {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PortableV1 => "portable-v1",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionPackageEvidence {
    pub source_kind: ExtensionSourceKind,
    pub source_locator: String,
    pub source_identity: Vec<u8>,
    pub extension_id: String,
    pub version: String,
    pub api_major: u32,
    pub profile_version: u32,
    pub package_digest: [u8; 32],
    pub manifest_digest: [u8; 32],
    pub component_digest: [u8; 32],
    pub publisher_fingerprint: String,
    pub required_permissions: Vec<String>,
    pub optional_permissions: Vec<String>,
    pub required_interfaces: Vec<String>,
    pub optional_interfaces: Vec<String>,
    pub ui_protocol: Option<ExtensionUiProtocol>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionRecord {
    pub extension_id: String,
    pub version: String,
    pub api_major: u32,
    pub package_digest: [u8; 32],
    pub publisher_fingerprint: String,
    pub trust_decision: ExtensionTrustDecision,
    pub desired_state: ExtensionDesiredState,
    pub quarantine_state: ExtensionQuarantineState,
    pub runtime_state: ExtensionRuntimeState,
    pub grant_revision: Revision,
    pub lifecycle_generation: Revision,
    pub revision: Revision,
    pub ui_protocol: Option<ExtensionUiProtocol>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionCursor {
    last_extension_id: String,
}

impl ExtensionCursor {
    pub fn new(last_extension_id: String) -> Result<Self, M6Error> {
        if !valid_extension_id(&last_extension_id) {
            return Err(M6Error::new(M6ErrorCode::InvalidInput));
        }
        Ok(Self { last_extension_id })
    }

    #[must_use]
    pub fn last_extension_id(&self) -> &str {
        &self.last_extension_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionPage {
    pub extensions: Vec<ExtensionRecord>,
    pub next_cursor: Option<ExtensionCursor>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionPlanRecord {
    pub plan_id: PlanId,
    pub owner: PrincipalId,
    pub action: String,
    pub state: String,
    pub evidence: ExtensionPackageEvidence,
    pub trust_decision: ExtensionTrustDecision,
    pub expected_revision: Option<Revision>,
    pub data_disposition: Option<ExtensionDataDisposition>,
    pub plan_fingerprint: [u8; 32],
    pub apply_operation_id: Option<OperationId>,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionInstallPlanDraft {
    pub evidence: ExtensionPackageEvidence,
    pub trust_decision: ExtensionTrustDecision,
    pub expected_revision: Option<Revision>,
    pub plan_fingerprint: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionUninstallPlanDraft {
    pub extension: ExtensionRecord,
    pub data_disposition: ExtensionDataDisposition,
    pub plan_fingerprint: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionApplyOutcome {
    pub operation_id: OperationId,
    pub replayed: bool,
    pub schedule: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExtensionJournalState {
    Intent,
    Completed,
}

impl ExtensionJournalState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Intent => "intent",
            Self::Completed => "completed",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExtensionJournalPhase {
    SourceVerified,
    ArchiveVerified,
    StagingComplete,
    PublishIntent,
    PackagePublished,
    GrantsRevoked,
    LeaseRevoked,
    HostStopped,
    PackageBackupIntent,
    PackageMovedToBackup,
    DataDeleteIntent,
    DataDeleted,
    StateCommitIntent,
    StateCommitted,
    CleanupComplete,
}

impl ExtensionJournalPhase {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SourceVerified => "source_verified",
            Self::ArchiveVerified => "archive_verified",
            Self::StagingComplete => "staging_complete",
            Self::PublishIntent => "publish_intent",
            Self::PackagePublished => "package_published",
            Self::GrantsRevoked => "grants_revoked",
            Self::LeaseRevoked => "lease_revoked",
            Self::HostStopped => "host_stopped",
            Self::PackageBackupIntent => "package_backup_intent",
            Self::PackageMovedToBackup => "package_moved_to_backup",
            Self::DataDeleteIntent => "data_delete_intent",
            Self::DataDeleted => "data_deleted",
            Self::StateCommitIntent => "state_commit_intent",
            Self::StateCommitted => "state_committed",
            Self::CleanupComplete => "cleanup_complete",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionFilesystemJournalEntry {
    pub operation_id: OperationId,
    pub step: u64,
    pub plan_id: PlanId,
    pub extension_id: String,
    pub action: String,
    pub phase: ExtensionJournalPhase,
    pub state: ExtensionJournalState,
    pub evidence_json: String,
    pub updated_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionGrantRecord {
    pub extension_id: String,
    pub permission: String,
    pub resource_kind: String,
    pub resource_id: String,
    pub granted: bool,
    pub grant_revision: Revision,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionGrantMutation {
    pub extension_id: String,
    pub permission: String,
    pub resource_kind: String,
    pub resource_id: String,
    pub expected_revision: Revision,
    pub grant: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionDataValue {
    pub value: Vec<u8>,
    pub key_revision: Revision,
    pub namespace_revision: Revision,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionDataWriteResult {
    pub key_revision: Revision,
    pub namespace_revision: Revision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionInstanceLease {
    pub lease_id: String,
    pub extension_id: String,
    pub instance_id: String,
    pub principal_id: PrincipalId,
    pub publisher_fingerprint: String,
    pub grant_revision: Revision,
    pub lifecycle_generation: Revision,
    pub daemon_epoch: String,
    pub expires_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionStartContext {
    pub lease: ExtensionInstanceLease,
    pub component_path: String,
    pub activation_kind: ExtensionActivationKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExtensionActivationKind {
    Background,
    InteractiveUi,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionRuntimePoll {
    pub active: Vec<ExtensionInstanceLease>,
    pub exited: Vec<ExtensionInstanceLease>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionCrashDecision {
    pub extension_id: String,
    pub restart_delay_ms: Option<u64>,
    pub quarantined: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExtensionStopReason {
    Disabled,
    PermissionRevoked,
    LeaseExpired,
    DaemonShutdown,
    Uninstalling,
    InteractiveUiIdle,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionProjectSummary {
    pub project_id: String,
    pub display_name: String,
    pub kind: String,
    pub unity_version: Option<String>,
    pub revision: Revision,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum M6ErrorCode {
    InvalidInput,
    PermissionDenied,
    ManifestInvalid,
    PackageInvalid,
    PackageUntrusted,
    PublisherConfirmationRequired,
    SignatureInvalid,
    AlreadyInstalled,
    NotInstalled,
    ProjectNotFound,
    ScopeDenied,
    ApiUnsupported,
    InstanceStale,
    ResourceLimit,
    Crashed,
    Quarantined,
    PlanStale,
    DataQuotaExceeded,
    DataOwnerMismatch,
    RecoveryRequired,
    RevisionConflict,
    IdempotencyConflict,
    StoreUnavailable,
    Internal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct M6Error {
    code: M6ErrorCode,
}

impl M6Error {
    #[must_use]
    pub const fn new(code: M6ErrorCode) -> Self {
        Self { code }
    }

    #[must_use]
    pub const fn code(&self) -> M6ErrorCode {
        self.code
    }
}

impl fmt::Display for M6Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "extension request failed: {:?}", self.code)
    }
}

impl std::error::Error for M6Error {}

pub trait M6PackageAdapter: Clone + Send + Sync + 'static {
    fn inspect(
        &self,
        source_kind: ExtensionSourceKind,
        path: String,
    ) -> impl Future<Output = Result<ExtensionPackageEvidence, M6Error>> + Send;

    fn verify_installed(
        &self,
        record: ExtensionRecord,
        live_locator: String,
    ) -> impl Future<Output = Result<(), M6Error>> + Send;

    fn install(
        &self,
        operation_id: OperationId,
        plan: ExtensionPlanRecord,
    ) -> impl Future<Output = Result<String, M6Error>> + Send;

    fn uninstall(
        &self,
        operation_id: OperationId,
        plan: ExtensionPlanRecord,
    ) -> impl Future<Output = Result<(), M6Error>> + Send;

    fn cleanup(
        &self,
        operation_id: OperationId,
        plan: ExtensionPlanRecord,
    ) -> impl Future<Output = Result<(), M6Error>> + Send;
}

pub trait M6RuntimeAdapter: Clone + Send + Sync + 'static {
    fn daemon_epoch(&self) -> String;
    fn start(
        &self,
        context: ExtensionStartContext,
    ) -> impl Future<Output = Result<(), M6Error>> + Send;
    fn stop(
        &self,
        extension_id: String,
        reason: ExtensionStopReason,
    ) -> impl Future<Output = Result<(), M6Error>> + Send;
    fn poll(&self) -> impl Future<Output = Result<ExtensionRuntimePoll, M6Error>> + Send;
    fn update_lease(
        &self,
        lease: ExtensionInstanceLease,
    ) -> impl Future<Output = Result<(), M6Error>> + Send;
    fn shutdown(&self) -> impl Future<Output = Result<(), M6Error>> + Send;
}

pub trait M6Store: Clone + Send + Sync + 'static {
    fn list_extensions(
        &self,
        owner: PrincipalId,
        cursor: Option<ExtensionCursor>,
        limit: u32,
    ) -> impl Future<Output = Result<ExtensionPage, M6Error>> + Send;
    fn get_extension(
        &self,
        owner: PrincipalId,
        extension_id: String,
    ) -> impl Future<Output = Result<ExtensionRecord, M6Error>> + Send;
    fn live_package_locator(
        &self,
        owner: PrincipalId,
        extension_id: String,
    ) -> impl Future<Output = Result<String, M6Error>> + Send;
    fn has_background_authority(
        &self,
        owner: PrincipalId,
        extension_id: String,
    ) -> impl Future<Output = Result<bool, M6Error>> + Send;
    fn create_install_plan(
        &self,
        owner: PrincipalId,
        draft: ExtensionInstallPlanDraft,
        now_ms: u64,
    ) -> impl Future<Output = Result<ExtensionPlanRecord, M6Error>> + Send;
    fn create_uninstall_plan(
        &self,
        owner: PrincipalId,
        draft: ExtensionUninstallPlanDraft,
        now_ms: u64,
    ) -> impl Future<Output = Result<ExtensionPlanRecord, M6Error>> + Send;
    fn accept_plan(
        &self,
        owner: PrincipalId,
        plan_id: PlanId,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<ExtensionApplyOutcome, M6Error>> + Send;
    fn get_plan(
        &self,
        plan_id: PlanId,
    ) -> impl Future<Output = Result<ExtensionPlanRecord, M6Error>> + Send;
    fn begin_apply(
        &self,
        operation_id: OperationId,
        now_ms: u64,
    ) -> impl Future<Output = Result<ExtensionPlanRecord, M6Error>> + Send;
    fn append_filesystem_journal(
        &self,
        entry: ExtensionFilesystemJournalEntry,
    ) -> impl Future<Output = Result<(), M6Error>> + Send;
    fn next_filesystem_journal_step(
        &self,
        operation_id: OperationId,
    ) -> impl Future<Output = Result<u64, M6Error>> + Send;
    fn filesystem_journal_has_phase(
        &self,
        operation_id: OperationId,
        phase: ExtensionJournalPhase,
    ) -> impl Future<Output = Result<bool, M6Error>> + Send;
    fn recover_operations(
        &self,
        now_ms: u64,
    ) -> impl Future<Output = Result<Vec<OperationId>, M6Error>> + Send;
    fn finish_install(
        &self,
        operation_id: OperationId,
        live_locator: String,
        now_ms: u64,
    ) -> impl Future<Output = Result<(), M6Error>> + Send;
    fn finish_uninstall(
        &self,
        operation_id: OperationId,
        now_ms: u64,
    ) -> impl Future<Output = Result<(), M6Error>> + Send;
    fn complete_operation(
        &self,
        operation_id: OperationId,
        now_ms: u64,
    ) -> impl Future<Output = Result<(), M6Error>> + Send;
    fn fail_operation(
        &self,
        operation_id: OperationId,
        code: M6ErrorCode,
        now_ms: u64,
    ) -> impl Future<Output = Result<(), M6Error>> + Send;
    fn enable(
        &self,
        owner: PrincipalId,
        extension_id: String,
        expected: Revision,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<ExtensionRecord, M6Error>> + Send;
    fn disable(
        &self,
        owner: PrincipalId,
        extension_id: String,
        expected: Revision,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<ExtensionRecord, M6Error>> + Send;
    fn set_grant(
        &self,
        owner: PrincipalId,
        mutation: ExtensionGrantMutation,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<ExtensionGrantRecord, M6Error>> + Send;
    fn data_get(
        &self,
        lease: ExtensionInstanceLease,
        key: String,
        now_ms: u64,
    ) -> impl Future<Output = Result<Option<ExtensionDataValue>, M6Error>> + Send;
    fn data_set(
        &self,
        lease: ExtensionInstanceLease,
        key: String,
        value: Vec<u8>,
        expected: Option<Revision>,
        now_ms: u64,
    ) -> impl Future<Output = Result<ExtensionDataWriteResult, M6Error>> + Send;
    fn data_delete(
        &self,
        lease: ExtensionInstanceLease,
        key: String,
        expected: Revision,
        now_ms: u64,
    ) -> impl Future<Output = Result<ExtensionDataWriteResult, M6Error>> + Send;
    fn prepare_instance(
        &self,
        owner: PrincipalId,
        extension_id: String,
        daemon_epoch: String,
        now_ms: u64,
    ) -> impl Future<Output = Result<ExtensionStartContext, M6Error>> + Send;
    fn mark_instance_running(
        &self,
        lease: ExtensionInstanceLease,
        now_ms: u64,
    ) -> impl Future<Output = Result<(), M6Error>> + Send;
    fn mark_instance_stopped(
        &self,
        extension_id: String,
        now_ms: u64,
    ) -> impl Future<Output = Result<(), M6Error>> + Send;
    fn renew_instance(
        &self,
        lease: ExtensionInstanceLease,
        now_ms: u64,
    ) -> impl Future<Output = Result<ExtensionInstanceLease, M6Error>> + Send;
    fn record_instance_crash(
        &self,
        lease: ExtensionInstanceLease,
        reason: String,
        now_ms: u64,
    ) -> impl Future<Output = Result<ExtensionCrashDecision, M6Error>> + Send;
    fn recover_instances(
        &self,
        daemon_epoch: String,
        now_ms: u64,
    ) -> impl Future<Output = Result<Vec<String>, M6Error>> + Send;
    fn project_summary(
        &self,
        lease: ExtensionInstanceLease,
        project_id: String,
        now_ms: u64,
    ) -> impl Future<Output = Result<ExtensionProjectSummary, M6Error>> + Send;
}

#[derive(Clone)]
pub struct M6HostApplication<S: M6Store> {
    store: S,
}

impl<S: M6Store> M6HostApplication<S> {
    #[must_use]
    pub fn new(store: S) -> Self {
        Self { store }
    }

    pub async fn project_summary(
        &self,
        lease: ExtensionInstanceLease,
        project_id: String,
        now_ms: u64,
    ) -> Result<ExtensionProjectSummary, M6Error> {
        self.store.project_summary(lease, project_id, now_ms).await
    }

    pub async fn data_get(
        &self,
        lease: ExtensionInstanceLease,
        key: String,
        now_ms: u64,
    ) -> Result<Option<ExtensionDataValue>, M6Error> {
        self.store.data_get(lease, key, now_ms).await
    }

    pub async fn data_set(
        &self,
        lease: ExtensionInstanceLease,
        key: String,
        value: Vec<u8>,
        expected: Option<Revision>,
        now_ms: u64,
    ) -> Result<ExtensionDataWriteResult, M6Error> {
        self.store
            .data_set(lease, key, value, expected, now_ms)
            .await
    }

    pub async fn data_delete(
        &self,
        lease: ExtensionInstanceLease,
        key: String,
        expected: Revision,
        now_ms: u64,
    ) -> Result<ExtensionDataWriteResult, M6Error> {
        self.store.data_delete(lease, key, expected, now_ms).await
    }
}

pub struct M6Application<S: M6Store, P: M6PackageAdapter, R: M6RuntimeAdapter> {
    pub(crate) store: S,
    pub(crate) packages: P,
    pub(crate) runtime: R,
    locks: Arc<ResourceLockCoordinator>,
}

impl<S: M6Store, P: M6PackageAdapter, R: M6RuntimeAdapter> Clone for M6Application<S, P, R> {
    fn clone(&self) -> Self {
        Self {
            store: self.store.clone(),
            packages: self.packages.clone(),
            runtime: self.runtime.clone(),
            locks: Arc::clone(&self.locks),
        }
    }
}

impl<S: M6Store, P: M6PackageAdapter, R: M6RuntimeAdapter> M6Application<S, P, R> {
    #[must_use]
    pub fn new(store: S, packages: P, runtime: R, locks: Arc<ResourceLockCoordinator>) -> Self {
        Self {
            store,
            packages,
            runtime,
            locks,
        }
    }

    pub async fn inspect_install(
        &self,
        access: &AccessContext,
        source_kind: ExtensionSourceKind,
        path: String,
    ) -> Result<ExtensionPackageEvidence, M6Error> {
        access
            .require(Permission::ExtensionsManage)
            .map_err(map_access)?;
        self.packages.inspect(source_kind, path).await
    }

    pub async fn list(
        &self,
        access: &AccessContext,
        cursor: Option<ExtensionCursor>,
        limit: u32,
    ) -> Result<ExtensionPage, M6Error> {
        access
            .require(Permission::ExtensionsRead)
            .map_err(map_access)?;
        if !(1..=1_000).contains(&limit) {
            return Err(M6Error::new(M6ErrorCode::InvalidInput));
        }
        self.store
            .list_extensions(access.principal().clone(), cursor, limit)
            .await
    }

    pub async fn get(
        &self,
        access: &AccessContext,
        extension_id: String,
    ) -> Result<ExtensionRecord, M6Error> {
        access
            .require(Permission::ExtensionsRead)
            .map_err(map_access)?;
        self.store
            .get_extension(access.principal().clone(), extension_id)
            .await
    }

    pub async fn plan_install(
        &self,
        access: &AccessContext,
        evidence: ExtensionPackageEvidence,
        trust_decision: ExtensionTrustDecision,
        expected_revision: Option<Revision>,
        plan_fingerprint: [u8; 32],
        now_ms: u64,
    ) -> Result<ExtensionPlanRecord, M6Error> {
        access
            .require(Permission::ExtensionsManage)
            .map_err(map_access)?;
        self.store
            .create_install_plan(
                access.principal().clone(),
                ExtensionInstallPlanDraft {
                    evidence,
                    trust_decision,
                    expected_revision,
                    plan_fingerprint,
                },
                now_ms,
            )
            .await
    }

    pub async fn plan_uninstall(
        &self,
        access: &AccessContext,
        extension_id: String,
        expected_revision: Revision,
        data_disposition: ExtensionDataDisposition,
        plan_fingerprint: [u8; 32],
        now_ms: u64,
    ) -> Result<ExtensionPlanRecord, M6Error> {
        access
            .require(Permission::ExtensionsManage)
            .map_err(map_access)?;
        let extension = self
            .store
            .get_extension(access.principal().clone(), extension_id)
            .await?;
        if extension.revision != expected_revision {
            return Err(M6Error::new(M6ErrorCode::RevisionConflict));
        }
        self.store
            .create_uninstall_plan(
                access.principal().clone(),
                ExtensionUninstallPlanDraft {
                    extension,
                    data_disposition,
                    plan_fingerprint,
                },
                now_ms,
            )
            .await
    }

    pub async fn enable(
        &self,
        access: &AccessContext,
        extension_id: String,
        expected_revision: Revision,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> Result<ExtensionRecord, M6Error> {
        access
            .require(Permission::ExtensionsManage)
            .map_err(map_access)?;
        let current = self
            .store
            .get_extension(access.principal().clone(), extension_id.clone())
            .await?;
        let locator = self
            .store
            .live_package_locator(access.principal().clone(), extension_id.clone())
            .await?;
        self.packages.verify_installed(current, locator).await?;
        let record = self
            .store
            .enable(
                access.principal().clone(),
                extension_id,
                expected_revision,
                key,
                now_ms,
            )
            .await?;
        self.start_enabled(
            access.principal().clone(),
            record.extension_id.clone(),
            now_ms,
        )
        .await?;
        self.store
            .get_extension(access.principal().clone(), record.extension_id)
            .await
    }

    pub async fn disable(
        &self,
        access: &AccessContext,
        extension_id: String,
        expected_revision: Revision,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> Result<ExtensionRecord, M6Error> {
        access
            .require(Permission::ExtensionsManage)
            .map_err(map_access)?;
        let record = self
            .store
            .disable(
                access.principal().clone(),
                extension_id,
                expected_revision,
                key,
                now_ms,
            )
            .await?;
        self.runtime
            .stop(record.extension_id.clone(), ExtensionStopReason::Disabled)
            .await?;
        self.store
            .mark_instance_stopped(record.extension_id.clone(), now_ms)
            .await?;
        self.store
            .get_extension(access.principal().clone(), record.extension_id)
            .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn set_grant(
        &self,
        access: &AccessContext,
        extension_id: String,
        permission: String,
        resource_kind: String,
        resource_id: String,
        expected_revision: Revision,
        key: IdempotencyKey,
        grant: bool,
        now_ms: u64,
    ) -> Result<ExtensionGrantRecord, M6Error> {
        access
            .require(Permission::ExtensionsPermissionsManage)
            .map_err(map_access)?;
        let result = self
            .store
            .set_grant(
                access.principal().clone(),
                ExtensionGrantMutation {
                    extension_id,
                    permission,
                    resource_kind,
                    resource_id,
                    expected_revision,
                    grant,
                },
                key,
                now_ms,
            )
            .await?;
        self.runtime
            .stop(
                result.extension_id.clone(),
                ExtensionStopReason::PermissionRevoked,
            )
            .await?;
        self.store
            .mark_instance_stopped(result.extension_id.clone(), now_ms)
            .await?;
        let extension = self
            .store
            .get_extension(access.principal().clone(), result.extension_id.clone())
            .await?;
        if extension.desired_state == ExtensionDesiredState::Enabled {
            // The grant mutation is already durable. Runtime restart is a
            // separate lifecycle effect and must not make a successful revoke
            // look as though it failed.
            let _ = self
                .start_enabled(
                    access.principal().clone(),
                    result.extension_id.clone(),
                    now_ms,
                )
                .await;
        }
        Ok(result)
    }

    pub async fn apply(
        &self,
        access: &AccessContext,
        plan_id: PlanId,
        key: IdempotencyKey,
        expected_action: &str,
        now_ms: u64,
    ) -> Result<ExtensionApplyOutcome, M6Error> {
        access
            .require(Permission::ExtensionsManage)
            .map_err(map_access)?;
        let plan = self.store.get_plan(plan_id).await?;
        if plan.action != expected_action {
            return Err(M6Error::new(M6ErrorCode::InvalidInput));
        }
        let _guard = self
            .locks
            .acquire(vec![ResourceKey::Extension(
                plan.evidence.extension_id.clone(),
            )])
            .await;
        let outcome = self
            .store
            .accept_plan(access.principal().clone(), plan_id, key, now_ms)
            .await?;
        if outcome.schedule {
            self.schedule(outcome.operation_id);
        }
        Ok(outcome)
    }

    pub async fn recover(&self, now_ms: u64) -> Result<(), M6Error> {
        for extension_id in self
            .store
            .recover_instances(self.runtime.daemon_epoch(), now_ms)
            .await?
        {
            // One malformed or crashing extension must not prevent the daemon from
            // recovering other extensions or durable Operations.
            let _ = self
                .start_enabled(PrincipalId::local_owner(), extension_id, now_ms)
                .await;
        }
        for operation_id in self.store.recover_operations(now_ms).await? {
            self.schedule(operation_id);
        }
        Ok(())
    }

    pub async fn maintain_runtime(&self, now_ms: u64) -> Result<(), M6Error> {
        let poll = self.runtime.poll().await?;
        for lease in poll.active {
            if lease.expires_at_ms.saturating_sub(now_ms) <= 30_000 {
                match self.store.renew_instance(lease.clone(), now_ms).await {
                    Ok(renewed) => self.runtime.update_lease(renewed).await?,
                    Err(error) if error.code() == M6ErrorCode::InstanceStale => {
                        self.runtime
                            .stop(lease.extension_id, ExtensionStopReason::PermissionRevoked)
                            .await?;
                    }
                    Err(error) => return Err(error),
                }
            }
        }
        for lease in poll.exited {
            let decision = match self
                .store
                .record_instance_crash(lease, "host_exited".to_owned(), now_ms)
                .await
            {
                Ok(decision) => decision,
                Err(error) if error.code() == M6ErrorCode::InstanceStale => continue,
                Err(error) => return Err(error),
            };
            if let Some(delay_ms) = decision.restart_delay_ms {
                self.schedule_restart(decision.extension_id, delay_ms);
            }
        }
        Ok(())
    }

    pub async fn shutdown_runtime(&self) -> Result<(), M6Error> {
        self.runtime.shutdown().await
    }

    fn schedule(&self, operation_id: OperationId) {
        let application = self.clone();
        tokio::spawn(async move {
            let _ = application.run(operation_id).await;
        });
    }

    pub(crate) fn schedule_restart(&self, extension_id: String, delay_ms: u64) {
        let application = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
            let Ok(now_ms) = m6_time_ms() else {
                return;
            };
            let _ = application
                .start_enabled(PrincipalId::local_owner(), extension_id, now_ms)
                .await;
        });
    }

    async fn run(&self, operation_id: OperationId) -> Result<(), M6Error> {
        let now_ms = m6_time_ms()?;
        let plan = self.store.begin_apply(operation_id, now_ms).await?;
        let _guard = self
            .locks
            .acquire(vec![ResourceKey::Extension(
                plan.evidence.extension_id.clone(),
            )])
            .await;
        if plan.action == "uninstall" {
            self.runtime
                .stop(
                    plan.evidence.extension_id.clone(),
                    ExtensionStopReason::Uninstalling,
                )
                .await?;
            self.store
                .mark_instance_stopped(plan.evidence.extension_id.clone(), m6_time_ms()?)
                .await?;
        }
        let action_result = match plan.action.as_str() {
            "install" => match self.packages.install(operation_id, plan.clone()).await {
                Ok(locator) => {
                    self.store
                        .finish_install(operation_id, locator, m6_time_ms()?)
                        .await
                }
                Err(error) => Err(error),
            },
            "uninstall" => match self.packages.uninstall(operation_id, plan.clone()).await {
                Ok(()) => {
                    self.store
                        .finish_uninstall(operation_id, m6_time_ms()?)
                        .await
                }
                Err(error) => Err(error),
            },
            _ => Err(M6Error::new(M6ErrorCode::Internal)),
        };
        if let Err(error) = action_result {
            if error.code() == M6ErrorCode::RecoveryRequired {
                return Err(error);
            }
            self.store
                .fail_operation(operation_id, error.code(), m6_time_ms()?)
                .await?;
            return Ok(());
        }
        m6_test_kill_gate("state_committed")?;
        self.packages.cleanup(operation_id, plan).await?;
        self.store
            .complete_operation(operation_id, m6_time_ms()?)
            .await?;
        Ok(())
    }

    pub(crate) async fn start_enabled(
        &self,
        owner: PrincipalId,
        extension_id: String,
        now_ms: u64,
    ) -> Result<(), M6Error> {
        if !self
            .store
            .has_background_authority(owner.clone(), extension_id.clone())
            .await?
        {
            return Ok(());
        }
        let record = self
            .store
            .get_extension(owner.clone(), extension_id.clone())
            .await?;
        let locator = self
            .store
            .live_package_locator(owner.clone(), extension_id.clone())
            .await?;
        self.packages.verify_installed(record, locator).await?;
        let mut context = self
            .store
            .prepare_instance(owner, extension_id, self.runtime.daemon_epoch(), now_ms)
            .await?;
        context.activation_kind = ExtensionActivationKind::Background;
        if let Err(start_error) = self.runtime.start(context.clone()).await {
            let decision = self
                .store
                .record_instance_crash(context.lease, "host_start_failed".to_owned(), m6_time_ms()?)
                .await?;
            if let Some(delay_ms) = decision.restart_delay_ms {
                self.schedule_restart(decision.extension_id, delay_ms);
            }
            return Err(start_error);
        }
        self.store
            .mark_instance_running(context.lease, m6_time_ms()?)
            .await
    }
}

fn m6_time_ms() -> Result<u64, M6Error> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| M6Error::new(M6ErrorCode::Internal))
        .and_then(|duration| {
            u64::try_from(duration.as_millis()).map_err(|_| M6Error::new(M6ErrorCode::Internal))
        })
}

fn map_access(error: ApplicationError) -> M6Error {
    match error {
        ApplicationError::PermissionDenied => M6Error::new(M6ErrorCode::PermissionDenied),
        _ => M6Error::new(M6ErrorCode::Internal),
    }
}

fn valid_extension_id(value: &str) -> bool {
    value.len() <= 255
        && value.split('.').count() >= 2
        && value.split('.').all(|part| {
            !part.is_empty()
                && !part.starts_with('-')
                && !part.ends_with('-')
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

#[cfg(feature = "test-kill-gates")]
fn m6_test_kill_gate(checkpoint: &str) -> Result<(), M6Error> {
    if std::env::var("ALCOMD_TEST_M6_KILL_GATE").as_deref() != Ok(checkpoint) {
        return Ok(());
    }
    let signal = std::env::var_os("ALCOMD_TEST_M6_KILL_SIGNAL")
        .ok_or_else(|| M6Error::new(M6ErrorCode::Internal))?;
    std::fs::write(signal, checkpoint).map_err(|_| M6Error::new(M6ErrorCode::RecoveryRequired))?;
    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}

#[cfg(not(feature = "test-kill-gates"))]
fn m6_test_kill_gate(_checkpoint: &str) -> Result<(), M6Error> {
    Ok(())
}
