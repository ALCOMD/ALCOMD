//! Discord extension backend ABI placeholder.
//!
//! This crate is deliberately standalone until Extension ABI v1 is accepted.

/// Returns the stable extension identifier.
#[must_use]
pub const fn extension_id() -> &'static str {
    "com.cqmhv.alcomd.extension.discord"
}
