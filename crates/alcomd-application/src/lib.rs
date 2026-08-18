//! Application use cases and orchestration boundaries.
//!
//! Transport adapters call this layer rather than inventing business state in
//! the daemon, CLI, or other entry points.

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_reports_only_the_real_m1_state() {
        assert_eq!(system_status().state(), SystemState::Ready);
        assert_eq!(system_status().state().as_str(), "ready");
    }
}
