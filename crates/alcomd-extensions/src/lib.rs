//! Extension runtime.
//!
//! Manifest validation, permissions, lifecycle, data isolation, and host capabilities.

mod engine;
mod host_protocol;
mod package;

pub use engine::ExtensionEngine;
pub use host_protocol::*;
pub use package::{
    ExtensionManifest, ManifestUi, PackageError, PackageErrorCode, VerifiedExtensionPackage,
    extract_extension_package, inspect_extension_directory, inspect_extension_package,
};

/// Stable crate identifier used by scaffold checks.
pub const CRATE_NAME: &str = "alcomd-extensions";
