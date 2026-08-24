//! Pure domain types for ALCOMD.
//!
//! This crate must not depend on Tauri, SQLite, HTTP, MCP, or operating-system APIs.

use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Stable identifier for a registered Unity project.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectId(Uuid);

impl ProjectId {
    /// Creates a new random project identifier.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Parses a canonical UUID representation.
    pub fn parse(value: &str) -> Result<Self, DomainValueError> {
        Uuid::parse_str(value)
            .map(Self)
            .map_err(|_| DomainValueError::InvalidProjectId)
    }

    /// Returns the underlying UUID.
    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

/// Stable identifier for a registered VPM repository source.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RepositoryId(Uuid);

impl RepositoryId {
    /// Creates a new random repository identifier.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Parses a canonical UUID representation.
    pub fn parse(value: &str) -> Result<Self, DomainValueError> {
        Uuid::parse_str(value)
            .map(Self)
            .map_err(|_| DomainValueError::InvalidRepositoryId)
    }

    /// Returns the underlying UUID.
    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for RepositoryId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for RepositoryId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Stable identifier for a validated Unity Editor installation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UnityInstallationId(Uuid);

impl UnityInstallationId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn parse(value: &str) -> Result<Self, DomainValueError> {
        Uuid::parse_str(value)
            .map(Self)
            .map_err(|_| DomainValueError::InvalidUnityInstallationId)
    }

    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for UnityInstallationId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for UnityInstallationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Opaque identifier for one accepted Unity launch attempt.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UnityLaunchId(Uuid);

impl UnityLaunchId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn parse(value: &str) -> Result<Self, DomainValueError> {
        Uuid::parse_str(value)
            .map(Self)
            .map_err(|_| DomainValueError::InvalidUnityLaunchId)
    }
}

impl Default for UnityLaunchId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for UnityLaunchId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl Default for ProjectId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ProjectId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Stable identifier for a builtin or user Template.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TemplateId(Uuid);

impl TemplateId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn parse(value: &str) -> Result<Self, DomainValueError> {
        Uuid::parse_str(value)
            .ok()
            .filter(|parsed| parsed.to_string() == value)
            .map(Self)
            .ok_or(DomainValueError::InvalidTemplateId)
    }

    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for TemplateId {
    fn default() -> Self {
        Self::new()
    }
}

/// Stable identifier for one managed native backup archive.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BackupId(Uuid);

impl BackupId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn parse(value: &str) -> Result<Self, DomainValueError> {
        Uuid::parse_str(value)
            .ok()
            .filter(|parsed| parsed.to_string() == value)
            .map(Self)
            .ok_or(DomainValueError::InvalidBackupId)
    }
}

impl Default for BackupId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for BackupId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl fmt::Display for TemplateId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Stable identifier for an immutable package transaction plan.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PlanId(Uuid);

impl PlanId {
    /// Creates a new random plan identifier.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Parses a canonical UUID representation.
    pub fn parse(value: &str) -> Result<Self, DomainValueError> {
        Uuid::parse_str(value)
            .map(Self)
            .map_err(|_| DomainValueError::InvalidPlanId)
    }

    /// Returns the underlying UUID.
    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for PlanId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for PlanId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Stable identifier for a long-running operation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OperationId(Uuid);

impl OperationId {
    /// Creates a new random operation identifier.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Constructs an Operation identifier from an already validated UUID.
    #[must_use]
    pub const fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }

    /// Parses a canonical UUID representation.
    pub fn parse(value: &str) -> Result<Self, DomainValueError> {
        Uuid::parse_str(value)
            .map(Self)
            .map_err(|_| DomainValueError::InvalidOperationId)
    }

    /// Returns the underlying UUID.
    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for OperationId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for OperationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Lifecycle state of a long-running operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationState {
    /// Waiting for a worker or resource lock.
    Queued,
    /// Producing a non-mutating change plan.
    Planning,
    /// Waiting for a user or policy decision.
    WaitingForInput,
    /// Applying changes.
    Running,
    /// Cancellation has been requested.
    Cancelling,
    /// Completed successfully.
    Succeeded,
    /// Completed with an error.
    Failed,
    /// Cancelled before completion.
    Cancelled,
    /// Interrupted by process or system termination.
    Interrupted,
    /// Being recovered after interruption.
    Recovering,
}

impl OperationState {
    /// Returns whether this state is immutable and terminal.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }

    /// Returns whether the frozen M2 state machine permits this public transition.
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Queued, Self::Running | Self::Cancelled)
                | (
                    Self::Running,
                    Self::Cancelling | Self::Succeeded | Self::Failed | Self::Interrupted
                )
                | (
                    Self::Cancelling,
                    Self::Succeeded | Self::Failed | Self::Cancelled | Self::Interrupted
                )
                | (Self::Interrupted, Self::Recovering)
                | (
                    Self::Recovering,
                    Self::Running
                        | Self::Cancelling
                        | Self::Failed
                        | Self::Cancelled
                        | Self::Interrupted
                )
        )
    }

    /// Returns the deterministic M2 restart treatment for a persisted state.
    #[must_use]
    pub const fn recovery_action(self) -> RecoveryAction {
        match self {
            Self::Queued => RecoveryAction::RescheduleQueued,
            Self::Running | Self::Cancelling | Self::Recovering => {
                RecoveryAction::InterruptThenRecover
            }
            Self::Succeeded | Self::Failed | Self::Cancelled => RecoveryAction::LeaveTerminal,
            Self::Interrupted => RecoveryAction::ResumeRecovery,
            Self::Planning | Self::WaitingForInput => RecoveryAction::ReservedStateNotCreatedByM2,
        }
    }
}

/// Frozen M2 restart action for a persisted Operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryAction {
    /// Keep a queued Operation queued and dispatch it again.
    RescheduleQueued,
    /// Commit `interrupted`, then commit `recovering`, incrementing revision and Event each time.
    InterruptThenRecover,
    /// Continue the second half of a previously committed recovery transition.
    ResumeRecovery,
    /// Preserve an immutable terminal state.
    LeaveTerminal,
    /// M2 does not create this persisted state and must not invent workflow behavior for it.
    ReservedStateNotCreatedByM2,
}

/// Positive optimistic-concurrency revision bounded by SQLite signed integer storage.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Revision(i64);

impl Revision {
    /// Initial revision for a newly visible aggregate.
    pub const INITIAL: Self = Self(1);

    /// Validates and constructs a public revision.
    pub const fn new(value: u64) -> Option<Self> {
        if value == 0 || value > i64::MAX as u64 {
            None
        } else {
            Some(Self(value as i64))
        }
    }

    /// Returns the public unsigned representation.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0 as u64
    }

    /// Returns the next revision, or `None` at the SQLite integer boundary.
    pub const fn checked_next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

/// Bootstrap Principal identifier used by M2 official same-user clients.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PrincipalId(String);

impl PrincipalId {
    /// Frozen M2 built-in Principal.
    pub const LOCAL_OWNER: &'static str = "builtin:local-owner";

    /// Constructs the M2 built-in Principal.
    #[must_use]
    pub fn local_owner() -> Self {
        Self(Self::LOCAL_OWNER.to_owned())
    }

    /// Creates a bounded Principal identifier for synthetic isolation tests.
    pub fn parse(value: impl Into<String>) -> Result<Self, DomainValueError> {
        let value = value.into();
        if value.is_empty() || value.len() > 128 || !value.is_ascii() {
            return Err(DomainValueError::InvalidPrincipalId);
        }
        Ok(Self(value))
    }

    /// Returns the stable identifier string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Bounded caller-supplied command idempotency key.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct IdempotencyKey(String);

impl IdempotencyKey {
    /// Frozen maximum UTF-8 byte length.
    pub const MAX_BYTES: usize = 128;

    /// Validates the M2 non-empty ASCII key contract.
    pub fn parse(value: impl Into<String>) -> Result<Self, DomainValueError> {
        let value = value.into();
        if value.is_empty() || value.len() > Self::MAX_BYTES || !value.is_ascii() {
            return Err(DomainValueError::InvalidIdempotencyKey);
        }
        Ok(Self(value))
    }

    /// Returns the caller-supplied key.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Permissions frozen for M2 application use cases.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum Permission {
    /// Start a read-only state integrity check.
    #[serde(rename = "state.check")]
    StateCheck,
    /// Read visible Operation records.
    #[serde(rename = "operations.read")]
    OperationsRead,
    /// Request cancellation of an owned Operation.
    #[serde(rename = "operations.cancel")]
    OperationsCancel,
    /// Replay visible durable Events.
    #[serde(rename = "events.read")]
    EventsRead,
    /// Read project paths and normalized snapshots.
    #[serde(rename = "projects.read")]
    ProjectsRead,
    /// Manage only the ALCOMD project registry and snapshot cache.
    #[serde(rename = "projects.manage")]
    ProjectsManage,
    /// Read repository sources and normalized catalogs.
    #[serde(rename = "repositories.read")]
    RepositoriesRead,
    /// Manage only the ALCOMD repository registry and metadata cache.
    #[serde(rename = "repositories.manage")]
    RepositoriesManage,
    /// Read package catalogs and produce immutable plans.
    #[serde(rename = "packages.read")]
    PackagesRead,
    /// Apply package plans to owned projects.
    #[serde(rename = "packages.manage")]
    PackagesManage,
    /// Read Unity installations, preferences and writer state.
    #[serde(rename = "unity.read")]
    UnityRead,
    /// Manage Unity installation registry and project Editor preferences.
    #[serde(rename = "unity.manage")]
    UnityManage,
    /// Launch and observe a validated Unity Editor.
    #[serde(rename = "unity.launch")]
    UnityLaunch,
    /// Create a new project at an explicit nonexistent destination.
    #[serde(rename = "projects.create")]
    ProjectsCreate,
    /// Read Template registry records and inspect/export bundles.
    #[serde(rename = "templates.read")]
    TemplatesRead,
    /// Import, derive, favorite, and remove user Templates.
    #[serde(rename = "templates.manage")]
    TemplatesManage,
    /// Read public-safe managed Backup metadata.
    #[serde(rename = "backups.read")]
    BackupsRead,
    /// Create managed native Backup archives.
    #[serde(rename = "backups.manage")]
    BackupsManage,
    /// Read installed extension metadata.
    #[serde(rename = "extensions.read")]
    ExtensionsRead,
    /// Manage extension install and lifecycle.
    #[serde(rename = "extensions.manage")]
    ExtensionsManage,
    /// Grant and revoke scoped extension permissions.
    #[serde(rename = "extensions.permissions.manage")]
    ExtensionsPermissionsManage,
    /// Permit a verified extension Component to run in its Host.
    #[serde(rename = "background.run")]
    BackgroundRun,
}

impl Permission {
    /// Returns the frozen public permission name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StateCheck => "state.check",
            Self::OperationsRead => "operations.read",
            Self::OperationsCancel => "operations.cancel",
            Self::EventsRead => "events.read",
            Self::ProjectsRead => "projects.read",
            Self::ProjectsManage => "projects.manage",
            Self::RepositoriesRead => "repositories.read",
            Self::RepositoriesManage => "repositories.manage",
            Self::PackagesRead => "packages.read",
            Self::PackagesManage => "packages.manage",
            Self::UnityRead => "unity.read",
            Self::UnityManage => "unity.manage",
            Self::UnityLaunch => "unity.launch",
            Self::ProjectsCreate => "projects.create",
            Self::TemplatesRead => "templates.read",
            Self::TemplatesManage => "templates.manage",
            Self::BackupsRead => "backups.read",
            Self::BackupsManage => "backups.manage",
            Self::ExtensionsRead => "extensions.read",
            Self::ExtensionsManage => "extensions.manage",
            Self::ExtensionsPermissionsManage => "extensions.permissions.manage",
            Self::BackgroundRun => "background.run",
        }
    }
}

/// Resource keys supported by the M2 in-process lock coordinator.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum ResourceKey {
    /// Serializes state-store integrity checks.
    StateStore,
    /// Serializes mutation of one Operation.
    Operation(OperationId),
    /// Serializes mutation of one registered project.
    Project(ProjectId),
    /// Serializes mutation of one registered repository.
    Repository(RepositoryId),
    /// Serializes publication of one content-addressed package cache object.
    PackageCache([u8; 32]),
    /// Serializes creation of one absent child beneath a validated parent object.
    ProjectCreate {
        parent_identity_sha256: [u8; 32],
        target_leaf: String,
    },
    /// Serializes mutation of one extension identity.
    Extension(String),
}

impl ResourceKey {
    /// Returns the canonical byte sequence used for deterministic multi-lock ordering.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        match self {
            Self::StateStore => b"state-store".to_vec(),
            Self::Operation(operation_id) => {
                let mut bytes = b"operation:".to_vec();
                bytes.extend_from_slice(operation_id.as_uuid().as_bytes());
                bytes
            }
            Self::Project(project_id) => {
                let mut bytes = b"project:".to_vec();
                bytes.extend_from_slice(project_id.as_uuid().as_bytes());
                bytes
            }
            Self::Repository(repository_id) => {
                let mut bytes = b"repository:".to_vec();
                bytes.extend_from_slice(repository_id.as_uuid().as_bytes());
                bytes
            }
            Self::PackageCache(digest) => {
                let mut bytes = b"package-cache:".to_vec();
                bytes.extend_from_slice(digest);
                bytes
            }
            Self::ProjectCreate {
                parent_identity_sha256,
                target_leaf,
            } => {
                let mut bytes = b"project-create:".to_vec();
                bytes.extend_from_slice(parent_identity_sha256);
                bytes.push(b':');
                bytes.extend_from_slice(target_leaf.as_bytes());
                bytes
            }
            Self::Extension(extension_id) => {
                let mut bytes = b"extension:".to_vec();
                bytes.extend_from_slice(extension_id.as_bytes());
                bytes
            }
        }
    }
}

/// Error returned when a bounded public domain value is invalid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DomainValueError {
    /// Project ID was not a valid UUID.
    InvalidProjectId,
    /// Repository ID was not a valid UUID.
    InvalidRepositoryId,
    /// Unity installation ID was not a valid UUID.
    InvalidUnityInstallationId,
    /// Unity launch ID was not a valid UUID.
    InvalidUnityLaunchId,
    /// Operation ID was not a valid UUID.
    InvalidOperationId,
    /// Plan ID was not a valid UUID.
    InvalidPlanId,
    /// Template ID was not a valid UUID.
    InvalidTemplateId,
    /// Backup ID was not a canonical UUID.
    InvalidBackupId,
    /// Principal ID was empty, non-ASCII, or exceeded its frozen limit.
    InvalidPrincipalId,
    /// Idempotency key was empty, non-ASCII, or exceeded its frozen limit.
    InvalidIdempotencyKey,
}

impl fmt::Display for DomainValueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProjectId => formatter.write_str("invalid Project identifier"),
            Self::InvalidRepositoryId => formatter.write_str("invalid Repository identifier"),
            Self::InvalidUnityInstallationId => {
                formatter.write_str("invalid Unity installation identifier")
            }
            Self::InvalidUnityLaunchId => formatter.write_str("invalid Unity launch identifier"),
            Self::InvalidOperationId => formatter.write_str("invalid Operation identifier"),
            Self::InvalidPlanId => formatter.write_str("invalid Plan identifier"),
            Self::InvalidTemplateId => formatter.write_str("invalid Template identifier"),
            Self::InvalidBackupId => formatter.write_str("invalid Backup identifier"),
            Self::InvalidPrincipalId => formatter.write_str("invalid Principal identifier"),
            Self::InvalidIdempotencyKey => formatter.write_str("invalid idempotency key"),
        }
    }
}

impl std::error::Error for DomainValueError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_ids_are_unique() {
        assert_ne!(ProjectId::new(), ProjectId::new());
    }

    #[test]
    fn operation_transition_table_freezes_m2_edges_and_terminal_immutability() {
        let states = [
            OperationState::Queued,
            OperationState::Planning,
            OperationState::WaitingForInput,
            OperationState::Running,
            OperationState::Cancelling,
            OperationState::Succeeded,
            OperationState::Failed,
            OperationState::Cancelled,
            OperationState::Interrupted,
            OperationState::Recovering,
        ];
        let allowed = [
            (OperationState::Queued, OperationState::Running),
            (OperationState::Queued, OperationState::Cancelled),
            (OperationState::Running, OperationState::Cancelling),
            (OperationState::Running, OperationState::Succeeded),
            (OperationState::Running, OperationState::Failed),
            (OperationState::Running, OperationState::Interrupted),
            (OperationState::Cancelling, OperationState::Succeeded),
            (OperationState::Cancelling, OperationState::Failed),
            (OperationState::Cancelling, OperationState::Cancelled),
            (OperationState::Cancelling, OperationState::Interrupted),
            (OperationState::Interrupted, OperationState::Recovering),
            (OperationState::Recovering, OperationState::Running),
            (OperationState::Recovering, OperationState::Cancelling),
            (OperationState::Recovering, OperationState::Failed),
            (OperationState::Recovering, OperationState::Cancelled),
            (OperationState::Recovering, OperationState::Interrupted),
        ];
        for current in states {
            for next in states {
                assert_eq!(
                    current.can_transition_to(next),
                    allowed.contains(&(current, next)),
                    "unexpected transition {current:?} -> {next:?}"
                );
            }
        }
        for state in states {
            assert_eq!(
                state.is_terminal(),
                matches!(
                    state,
                    OperationState::Succeeded | OperationState::Failed | OperationState::Cancelled
                )
            );
        }
    }

    #[test]
    fn recovery_table_preserves_queued_and_terminal_states() {
        assert_eq!(
            OperationState::Queued.recovery_action(),
            RecoveryAction::RescheduleQueued
        );
        for state in [
            OperationState::Running,
            OperationState::Cancelling,
            OperationState::Recovering,
        ] {
            assert_eq!(
                state.recovery_action(),
                RecoveryAction::InterruptThenRecover
            );
        }
        assert_eq!(
            OperationState::Interrupted.recovery_action(),
            RecoveryAction::ResumeRecovery
        );
        for state in [
            OperationState::Succeeded,
            OperationState::Failed,
            OperationState::Cancelled,
        ] {
            assert_eq!(state.recovery_action(), RecoveryAction::LeaveTerminal);
        }
    }

    #[test]
    fn revision_is_positive_and_sqlite_bounded() {
        assert_eq!(Revision::new(0), None);
        assert_eq!(Revision::INITIAL.get(), 1);
        assert_eq!(
            Revision::new(i64::MAX as u64).map(Revision::get),
            Some(i64::MAX as u64)
        );
        assert_eq!(Revision::new(i64::MAX as u64 + 1), None);
        assert_eq!(
            Revision::new(i64::MAX as u64).and_then(Revision::checked_next),
            None
        );
    }

    #[test]
    fn resource_lock_order_is_canonical_and_scope_is_minimal() {
        let first = ResourceKey::Operation(OperationId::new());
        let second = ResourceKey::Operation(OperationId::new());
        assert_ne!(first.canonical_bytes(), second.canonical_bytes());
        assert_eq!(ResourceKey::StateStore.canonical_bytes(), b"state-store");
        let create = ResourceKey::ProjectCreate {
            parent_identity_sha256: [7; 32],
            target_leaf: "Example".to_owned(),
        };
        assert_eq!(
            create.canonical_bytes(),
            [b"project-create:".as_slice(), &[7; 32], b":Example"].concat()
        );
    }

    #[test]
    fn principal_and_permission_names_are_frozen() {
        assert_eq!(PrincipalId::local_owner().as_str(), "builtin:local-owner");
        assert_eq!(Permission::EventsRead.as_str(), "events.read");
        assert_eq!(Permission::UnityRead.as_str(), "unity.read");
        assert_eq!(Permission::UnityManage.as_str(), "unity.manage");
        assert_eq!(Permission::UnityLaunch.as_str(), "unity.launch");
        assert_eq!(Permission::ProjectsCreate.as_str(), "projects.create");
        assert!(IdempotencyKey::parse("check-once").is_ok());
        assert_eq!(
            IdempotencyKey::parse("界"),
            Err(DomainValueError::InvalidIdempotencyKey)
        );
    }
}
