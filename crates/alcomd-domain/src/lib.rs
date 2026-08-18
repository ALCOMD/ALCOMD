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

    /// Returns the underlying UUID.
    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
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
        }
    }
}

/// Error returned when a bounded public domain value is invalid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DomainValueError {
    /// Operation ID was not a valid UUID.
    InvalidOperationId,
    /// Principal ID was empty, non-ASCII, or exceeded its frozen limit.
    InvalidPrincipalId,
    /// Idempotency key was empty, non-ASCII, or exceeded its frozen limit.
    InvalidIdempotencyKey,
}

impl fmt::Display for DomainValueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidOperationId => formatter.write_str("invalid Operation identifier"),
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
    }

    #[test]
    fn principal_and_permission_names_are_frozen() {
        assert_eq!(PrincipalId::local_owner().as_str(), "builtin:local-owner");
        assert_eq!(Permission::EventsRead.as_str(), "events.read");
        assert!(IdempotencyKey::parse("check-once").is_ok());
        assert_eq!(
            IdempotencyKey::parse("界"),
            Err(DomainValueError::InvalidIdempotencyKey)
        );
    }
}
