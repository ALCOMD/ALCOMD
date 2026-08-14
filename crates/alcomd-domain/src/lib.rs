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
}

impl Default for OperationId {
    fn default() -> Self {
        Self::new()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_ids_are_unique() {
        assert_ne!(ProjectId::new(), ProjectId::new());
    }
}
