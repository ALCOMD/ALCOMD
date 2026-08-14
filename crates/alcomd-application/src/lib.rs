//! Application use cases and orchestration boundaries.
//!
//! Transport adapters must call this layer rather than implementing business rules themselves.

use alcomd_domain::OperationState;
use serde::{Deserialize, Serialize};

/// Minimal health snapshot used by the initial vertical slice.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthSnapshot {
    /// Human-readable component state.
    pub status: String,
    /// Version of the running component.
    pub version: String,
    /// Current operation subsystem state.
    pub operation_state: Option<OperationState>,
}

impl HealthSnapshot {
    /// Creates a scaffold health response.
    #[must_use]
    pub fn scaffold() -> Self {
        Self {
            status: "scaffold".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            operation_state: None,
        }
    }
}

/// Port implemented by a component that can report health.
pub trait HealthProvider: Send + Sync {
    /// Returns the current health snapshot.
    fn health(&self) -> HealthSnapshot;
}
