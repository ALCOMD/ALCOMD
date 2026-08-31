//! Public ALCOMD RPC v1 data-transfer objects and framing contract.
//!
//! The protocol is JSON-RPC-inspired, but it is not JSON-RPC 2.0 compatible.
//! Internal domain and application types must not leak into this crate.

use std::collections::HashSet;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

mod m7;

pub use m7::*;

/// Current ALCOMD RPC major version.
pub const RPC_VERSION: u32 = 1;

/// Stable product family.
pub const PRODUCT_FAMILY: &str = "ALCOMD";

/// Stable technical root name.
pub const TECHNICAL_NAME: &str = "alcomd";

/// Maximum UTF-8 JSON payload size for one frame.
pub const MAX_FRAME_PAYLOAD_BYTES: usize = 4 * 1024 * 1024;

/// Maximum request ID length in UTF-8 bytes.
pub const MAX_REQUEST_ID_BYTES: usize = 64;

/// Maximum method length in ASCII bytes.
pub const MAX_METHOD_BYTES: usize = 128;

/// Maximum capability count in one hello message.
pub const MAX_CAPABILITIES: usize = 64;

/// Maximum capability length in ASCII bytes.
pub const MAX_CAPABILITY_BYTES: usize = 128;

/// Maximum idempotency-key length in UTF-8 bytes.
pub const MAX_IDEMPOTENCY_KEY_BYTES: usize = 128;

/// Default page size for M2 list methods.
pub const DEFAULT_PAGE_LIMIT: u32 = 100;

/// Maximum page size for M2 list methods.
pub const MAX_PAGE_LIMIT: u32 = 1_000;

/// Maximum public integer representable by SQLite's signed integer storage.
pub const MAX_SQLITE_PUBLIC_INTEGER: u64 = i64::MAX as u64;

/// `system.hello` method name.
pub const METHOD_SYSTEM_HELLO: &str = "system.hello";

/// `system.status` method name.
pub const METHOD_SYSTEM_STATUS: &str = "system.status";

/// `state.check` method name.
pub const METHOD_STATE_CHECK: &str = "state.check";

/// `operations.get` method name.
pub const METHOD_OPERATIONS_GET: &str = "operations.get";

/// `operations.list` method name.
pub const METHOD_OPERATIONS_LIST: &str = "operations.list";

/// `operations.cancel` method name.
pub const METHOD_OPERATIONS_CANCEL: &str = "operations.cancel";

/// `events.list` method name.
pub const METHOD_EVENTS_LIST: &str = "events.list";
pub const METHOD_PROJECTS_INSPECT: &str = "projects.inspect";
pub const METHOD_PROJECTS_LIST: &str = "projects.list";
pub const METHOD_PROJECTS_GET: &str = "projects.get";
pub const METHOD_PROJECTS_REGISTER: &str = "projects.register";
pub const METHOD_PROJECTS_REFRESH: &str = "projects.refresh";
pub const METHOD_PROJECTS_SET_FAVORITE: &str = "projects.setFavorite";
pub const METHOD_PROJECTS_UNREGISTER: &str = "projects.unregister";
pub const METHOD_REPOSITORIES_INSPECT: &str = "repositories.inspect";
pub const METHOD_REPOSITORIES_LIST: &str = "repositories.list";
pub const METHOD_REPOSITORIES_GET: &str = "repositories.get";
pub const METHOD_REPOSITORIES_PACKAGES: &str = "repositories.packages";
pub const METHOD_REPOSITORIES_REGISTER: &str = "repositories.register";
pub const METHOD_REPOSITORIES_REFRESH: &str = "repositories.refresh";
pub const METHOD_REPOSITORIES_UNREGISTER: &str = "repositories.unregister";
pub const METHOD_PACKAGES_PLAN_INSTALL: &str = "packages.planInstall";
pub const METHOD_PACKAGES_PLAN_REMOVE: &str = "packages.planRemove";
pub const METHOD_PACKAGES_PLAN_UPGRADE: &str = "packages.planUpgrade";
pub const METHOD_PACKAGES_PLAN_DOWNGRADE: &str = "packages.planDowngrade";
pub const METHOD_PACKAGES_PLAN_RESOLVE: &str = "packages.planResolve";
pub const METHOD_PACKAGES_PLAN_REINSTALL: &str = "packages.planReinstall";
pub const METHOD_PACKAGES_PLAN_BULK: &str = "packages.planBulk";
pub const METHOD_PACKAGES_APPLY_PLAN: &str = "packages.applyPlan";
pub const METHOD_PACKAGES_USER_PACKAGES_LIST: &str = "packages.userPackages.list";
pub const METHOD_PACKAGES_USER_PACKAGES_GET: &str = "packages.userPackages.get";
pub const METHOD_PACKAGES_USER_PACKAGES_ENROLL: &str = "packages.userPackages.enroll";
pub const METHOD_PACKAGES_USER_PACKAGES_REFRESH: &str = "packages.userPackages.refresh";
pub const METHOD_PACKAGES_USER_PACKAGES_REMOVE: &str = "packages.userPackages.remove";
pub const METHOD_UNITY_INSTALLATIONS_LIST: &str = "unity.installations.list";
pub const METHOD_UNITY_INSTALLATIONS_GET: &str = "unity.installations.get";
pub const METHOD_UNITY_INSTALLATIONS_REGISTER: &str = "unity.installations.register";
pub const METHOD_UNITY_INSTALLATIONS_REMOVE: &str = "unity.installations.remove";
pub const METHOD_UNITY_INSTALLATIONS_REFRESH: &str = "unity.installations.refresh";
pub const METHOD_UNITY_PROJECT_LAUNCH_CONFIG_GET: &str = "unity.projectLaunchConfig.get";
pub const METHOD_UNITY_PROJECT_LAUNCH_CONFIG_SET: &str = "unity.projectLaunchConfig.set";
pub const METHOD_UNITY_PROJECT_LAUNCH_CONFIG_CLEAR: &str = "unity.projectLaunchConfig.clear";
pub const METHOD_UNITY_WRITER_STATE: &str = "unity.writerState";
pub const METHOD_UNITY_LAUNCH_OPTIONS: &str = "unity.launchOptions";
pub const METHOD_UNITY_LAUNCH: &str = "unity.launch";
pub const METHOD_UNITY_LAUNCH_STATUS: &str = "unity.launchStatus";
pub const METHOD_TEMPLATES_LIST: &str = "templates.list";
pub const METHOD_TEMPLATES_GET: &str = "templates.get";
pub const METHOD_TEMPLATES_INSPECT_BUNDLE: &str = "templates.inspectBundle";
pub const METHOD_TEMPLATES_PLAN_IMPORT: &str = "templates.planImport";
pub const METHOD_TEMPLATES_APPLY_IMPORT: &str = "templates.applyImport";
pub const METHOD_TEMPLATES_PLAN_DERIVE: &str = "templates.planDerive";
pub const METHOD_TEMPLATES_APPLY_DERIVE: &str = "templates.applyDerive";
pub const METHOD_TEMPLATES_EXPORT: &str = "templates.export";
pub const METHOD_TEMPLATES_SET_FAVORITE: &str = "templates.setFavorite";
pub const METHOD_TEMPLATES_REMOVE: &str = "templates.remove";
pub const METHOD_TEMPLATES_PLAN_CREATE_PROJECT: &str = "templates.planCreateProject";
pub const METHOD_TEMPLATES_APPLY_CREATE_PROJECT: &str = "templates.applyCreateProject";
pub const METHOD_BACKUPS_LIST: &str = "backups.list";
pub const METHOD_BACKUPS_GET: &str = "backups.get";
pub const METHOD_BACKUPS_CREATE: &str = "backups.create";
pub const METHOD_BACKUPS_PLAN_RESTORE: &str = "backups.planRestore";
pub const METHOD_BACKUPS_APPLY_RESTORE: &str = "backups.applyRestore";
pub const METHOD_PROJECTS_PLAN_COPY: &str = "projects.planCopy";
pub const METHOD_PROJECTS_APPLY_COPY: &str = "projects.applyCopy";
pub const METHOD_PROJECTS_PLAN_DELETE_DIRECTORY: &str = "projects.planDeleteDirectory";
pub const METHOD_PROJECTS_APPLY_DELETE_DIRECTORY: &str = "projects.applyDeleteDirectory";
pub const METHOD_PROJECTS_PLAN_UNITY_MIGRATION: &str = "projects.planUnityMigration";
pub const METHOD_PROJECTS_APPLY_UNITY_MIGRATION: &str = "projects.applyUnityMigration";
pub const METHOD_EXTENSIONS_LIST: &str = "extensions.list";
pub const METHOD_EXTENSIONS_GET: &str = "extensions.get";
pub const METHOD_EXTENSIONS_PLAN_INSTALL: &str = "extensions.planInstall";
pub const METHOD_EXTENSIONS_APPLY_INSTALL: &str = "extensions.applyInstall";
pub const METHOD_EXTENSIONS_ENABLE: &str = "extensions.enable";
pub const METHOD_EXTENSIONS_DISABLE: &str = "extensions.disable";
pub const METHOD_EXTENSIONS_PLAN_UNINSTALL: &str = "extensions.planUninstall";
pub const METHOD_EXTENSIONS_APPLY_UNINSTALL: &str = "extensions.applyUninstall";
pub const METHOD_EXTENSIONS_SET_GRANT: &str = "extensions.setGrant";
pub const METHOD_EXTENSIONS_REVOKE_GRANT: &str = "extensions.revokeGrant";
pub const METHOD_EXTENSIONS_UI_OPEN: &str = "extensions.ui.open";
pub const METHOD_EXTENSIONS_UI_REFRESH: &str = "extensions.ui.refresh";
pub const METHOD_EXTENSIONS_UI_DISPATCH: &str = "extensions.ui.dispatch";
pub const METHOD_EXTENSIONS_UI_CLOSE: &str = "extensions.ui.close";
pub const METHOD_SETTINGS_GET: &str = "settings.get";
pub const METHOD_SETTINGS_UPDATE: &str = "settings.update";
pub const METHOD_ACTIVITY_LIST: &str = "activity.list";
pub const METHOD_DIAGNOSTICS_LIST: &str = "diagnostics.list";

/// Capability required by `state.check`.
pub const CAPABILITY_STATE_CHECK_V1: &str = "state.check.v1";

/// Capability required by operation methods.
pub const CAPABILITY_OPERATIONS_V1: &str = "operations.v1";

/// Capability required by `events.list`.
pub const CAPABILITY_EVENTS_REPLAY_V1: &str = "events.replay.v1";
pub const CAPABILITY_PROJECTS_READ_V1: &str = "projects.read.v1";
pub const CAPABILITY_PROJECTS_REGISTRY_V1: &str = "projects.registry.v1";
pub const CAPABILITY_PROJECTS_COPY_V1: &str = "projects.copy.v1";
pub const CAPABILITY_PROJECTS_DELETE_V1: &str = "projects.delete.v1";
pub const CAPABILITY_PROJECTS_UNITY_MIGRATION_V1: &str = "projects.unity-migration.v1";
pub const CAPABILITY_REPOSITORIES_READ_V1: &str = "repositories.read.v1";
pub const CAPABILITY_REPOSITORIES_REGISTRY_V1: &str = "repositories.registry.v1";
pub const CAPABILITY_PACKAGES_PLAN_V1: &str = "packages.plan.v1";
pub const CAPABILITY_PACKAGES_PLAN_V2: &str = "packages.plan.v2";
pub const CAPABILITY_PACKAGES_APPLY_V1: &str = "packages.apply.v1";
pub const CAPABILITY_PACKAGES_USER_PACKAGES_V1: &str = "packages.user-packages.v1";
pub const CAPABILITY_UNITY_READ_V1: &str = "unity.read.v1";
pub const CAPABILITY_UNITY_MANAGE_V1: &str = "unity.manage.v1";
pub const CAPABILITY_UNITY_LAUNCH_V1: &str = "unity.launch.v1";
pub const CAPABILITY_TEMPLATES_READ_V1: &str = "templates.read.v1";
pub const CAPABILITY_TEMPLATES_MANAGE_V1: &str = "templates.manage.v1";
pub const CAPABILITY_TEMPLATES_CREATE_PROJECT_V1: &str = "templates.create-project.v1";
pub const CAPABILITY_BACKUPS_READ_V1: &str = "backups.read.v1";
pub const CAPABILITY_BACKUPS_CREATE_V1: &str = "backups.create.v1";
pub const CAPABILITY_BACKUPS_RESTORE_V1: &str = "backups.restore.v1";
pub const CAPABILITY_EXTENSIONS_LIFECYCLE_V1: &str = "extensions.lifecycle.v1";
pub const CAPABILITY_EXTENSIONS_PERMISSIONS_V1: &str = "extensions.permissions.v1";
pub const CAPABILITY_EXTENSIONS_UI_PORTABLE_V1: &str = "extensions.ui.portable.v1";

/// Stable RPC v1 error codes implemented through M2.
pub mod error_code {
    /// A complete payload could not be parsed or validated as an RPC request.
    pub const INVALID_REQUEST: &str = "invalid_request";
    /// The requested method is unavailable.
    pub const METHOD_NOT_FOUND: &str = "method_not_found";
    /// A method was called before `system.hello`.
    pub const HANDSHAKE_REQUIRED: &str = "handshake_required";
    /// `system.hello` was repeated on an initialized connection.
    pub const HANDSHAKE_ALREADY_COMPLETED: &str = "handshake_already_completed";
    /// The requested RPC major is unsupported.
    pub const RPC_VERSION_UNSUPPORTED: &str = "rpc_version_unsupported";
    /// A peer component must be upgraded before the request can run.
    pub const COMPONENT_UPGRADE_REQUIRED: &str = "component_upgrade_required";
    /// An unknown internal failure occurred.
    pub const INTERNAL_ERROR: &str = "internal_error";
    /// The connection did not negotiate the capability required by the method.
    pub const CAPABILITY_REQUIRED: &str = "capability_required";
    /// The current Principal lacks the required permission or visibility.
    pub const PERMISSION_DENIED: &str = "permission_denied";
    /// The supplied expected revision is stale.
    pub const REVISION_CONFLICT: &str = "revision_conflict";
    /// An idempotency key was reused with a different canonical fingerprint.
    pub const IDEMPOTENCY_CONFLICT: &str = "idempotency_conflict";
    /// The requested operation does not exist or is not visible.
    pub const OPERATION_NOT_FOUND: &str = "operation_not_found";
    /// The requested operation can no longer accept cancellation.
    pub const OPERATION_NOT_CANCELLABLE: &str = "operation_not_cancellable";
    /// A future retained-event implementation no longer has the requested cursor.
    pub const EVENT_CURSOR_EXPIRED: &str = "event_cursor_expired";
    /// The data schema is newer than this daemon supports.
    pub const DATA_SCHEMA_UNSUPPORTED: &str = "data_schema_unsupported";
    /// The ready daemon could not complete a state-store operation.
    pub const STORE_UNAVAILABLE: &str = "store_unavailable";
    pub const PATH_ENCODING_UNSUPPORTED: &str = "path_encoding_unsupported";
    pub const PROJECT_NOT_FOUND: &str = "project_not_found";
    pub const PROJECT_NOT_REGISTERED: &str = "project_not_registered";
    pub const PROJECT_ALREADY_REGISTERED: &str = "project_already_registered";
    pub const PROJECT_INACCESSIBLE: &str = "project_inaccessible";
    pub const PROJECT_VERSION_MISSING: &str = "project_version_missing";
    pub const PROJECT_VERSION_INVALID: &str = "project_version_invalid";
    pub const PROJECT_MANIFEST_INVALID: &str = "project_manifest_invalid";
    pub const REPOSITORY_NOT_FOUND: &str = "repository_not_found";
    pub const REPOSITORY_NOT_REGISTERED: &str = "repository_not_registered";
    pub const REPOSITORY_ALREADY_REGISTERED: &str = "repository_already_registered";
    pub const REPOSITORY_SOURCE_INVALID: &str = "repository_source_invalid";
    pub const REPOSITORY_INACCESSIBLE: &str = "repository_inaccessible";
    pub const REPOSITORY_UNAVAILABLE: &str = "repository_unavailable";
    pub const REPOSITORY_DOCUMENT_INVALID: &str = "repository_document_invalid";
    pub const REPOSITORY_DOCUMENT_TOO_LARGE: &str = "repository_document_too_large";
    pub const REPOSITORY_CREDENTIALS_UNSUPPORTED: &str = "repository_credentials_unsupported";
    pub const REPOSITORY_REFRESH_REQUIRED: &str = "repository_refresh_required";
    pub const PACKAGE_NOT_FOUND: &str = "package_not_found";
    pub const PACKAGE_VERSION_INVALID: &str = "package_version_invalid";
    pub const PACKAGE_RANGE_INVALID: &str = "package_range_invalid";
    pub const PACKAGE_DEPENDENCY_MISSING: &str = "package_dependency_missing";
    pub const PACKAGE_DEPENDENCY_CONFLICT: &str = "package_dependency_conflict";
    pub const PACKAGE_UNITY_INCOMPATIBLE: &str = "package_unity_incompatible";
    pub const PACKAGE_SOURCE_AMBIGUOUS: &str = "package_source_ambiguous";
    pub const PACKAGE_MANIFEST_INVALID: &str = "package_manifest_invalid";
    pub const PACKAGE_HASH_REQUIRED: &str = "package_hash_required";
    pub const PACKAGE_LEGACY_CLEANUP_REQUIRED: &str = "package_legacy_cleanup_required";
    pub const PACKAGE_VERSION_YANKED: &str = "package_version_yanked";
    pub const PLAN_NOT_FOUND: &str = "plan_not_found";
    pub const PLAN_STALE: &str = "plan_stale";
    pub const PLAN_TOO_LARGE: &str = "plan_too_large";
    pub const PROJECT_CHANGED_DURING_APPLY: &str = "project_changed_during_apply";
    pub const PACKAGE_CACHE_CORRUPT: &str = "package_cache_corrupt";
    pub const PACKAGE_CACHE_QUOTA_EXCEEDED: &str = "package_cache_quota_exceeded";
    pub const PACKAGE_INTEGRITY_MISMATCH: &str = "package_integrity_mismatch";
    pub const PACKAGE_DOWNLOAD_TOO_LARGE: &str = "package_download_too_large";
    pub const OFFLINE_CACHE_MISS: &str = "offline_cache_miss";
    pub const PACKAGE_ARCHIVE_INVALID: &str = "package_archive_invalid";
    pub const PACKAGE_ARCHIVE_UNSUPPORTED_COMPRESSION: &str =
        "package_archive_unsupported_compression";
    pub const PACKAGE_ARCHIVE_LIMIT_EXCEEDED: &str = "package_archive_limit_exceeded";
    pub const PACKAGE_PATH_INVALID: &str = "package_path_invalid";
    pub const PACKAGE_PATH_COLLISION: &str = "package_path_collision";
    pub const PROJECT_TRANSACTION_RECOVERY_REQUIRED: &str = "project_transaction_recovery_required";
    pub const RECOVERY_REQUIRED: &str = "recovery_required";
    pub const UNITY_INSTALLATION_NOT_FOUND: &str = "unity_installation_not_found";
    pub const UNITY_INSTALLATION_INVALID: &str = "unity_installation_invalid";
    pub const UNITY_INSTALLATION_IN_USE: &str = "unity_installation_in_use";
    pub const UNITY_EDITOR_SELECTION_REQUIRED: &str = "unity_editor_selection_required";
    pub const UNITY_VERSION_UNVERIFIED: &str = "unity_version_unverified";
    pub const UNITY_VERSION_MISMATCH: &str = "unity_version_mismatch";
    pub const UNITY_ARCHITECTURE_UNSUPPORTED: &str = "unity_architecture_unsupported";
    pub const UNITY_PROJECT_RUNNING: &str = "unity_project_running";
    pub const UNITY_LAUNCH_STATE_UNCERTAIN: &str = "unity_launch_state_uncertain";
    pub const UNITY_PROJECT_SELECTOR_FORBIDDEN: &str = "unity_project_selector_forbidden";
    pub const PROJECT_UNITY_MIGRATION_PLAN_NOT_FOUND: &str =
        "project_unity_migration_plan_not_found";
    pub const PROJECT_UNITY_MIGRATION_PLAN_STALE: &str = "project_unity_migration_plan_stale";
    pub const PROJECT_UNITY_MIGRATION_UNSUPPORTED: &str = "project_unity_migration_unsupported";
    pub const PROJECT_UNITY_MIGRATION_SOURCE_CHANGED: &str =
        "project_unity_migration_source_changed";
    pub const PROJECT_UNITY_MIGRATION_RECOVERY_REQUIRED: &str =
        "project_unity_migration_recovery_required";
    pub const BACKUP_NOT_FOUND: &str = "backup_not_found";
    pub const BACKUP_UNAVAILABLE: &str = "backup_unavailable";
    pub const BACKUP_SOURCE_UNSAFE: &str = "backup_source_unsafe";
    pub const BACKUP_ARCHIVE_LIMIT_EXCEEDED: &str = "backup_archive_limit_exceeded";
    pub const PROJECT_CHANGED_DURING_BACKUP: &str = "project_changed_during_backup";
    pub const BACKUP_INTEGRITY_MISMATCH: &str = "backup_integrity_mismatch";
    pub const BACKUP_RESTORE_PLAN_NOT_FOUND: &str = "backup_restore_plan_not_found";
    pub const BACKUP_RESTORE_PLAN_STALE: &str = "backup_restore_plan_stale";
    pub const BACKUP_TARGET_EXISTS: &str = "backup_target_exists";
    pub const BACKUP_TARGET_INVALID: &str = "backup_target_invalid";
    pub const BACKUP_RESTORE_RECOVERY_REQUIRED: &str = "backup_restore_recovery_required";
    pub const PROJECT_COPY_PLAN_NOT_FOUND: &str = "project_copy_plan_not_found";
    pub const PROJECT_COPY_PLAN_STALE: &str = "project_copy_plan_stale";
    pub const PROJECT_COPY_TARGET_EXISTS: &str = "project_copy_target_exists";
    pub const PROJECT_COPY_TARGET_UNSAFE: &str = "project_copy_target_unsafe";
    pub const PROJECT_COPY_SOURCE_UNSAFE: &str = "project_copy_source_unsafe";
    pub const PROJECT_COPY_SOURCE_CHANGED: &str = "project_copy_source_changed";
    pub const PROJECT_COPY_LIMIT_EXCEEDED: &str = "project_copy_limit_exceeded";
    pub const PROJECT_COPY_RECOVERY_REQUIRED: &str = "project_copy_recovery_required";
    pub const PROJECT_DELETE_PLAN_NOT_FOUND: &str = "project_delete_plan_not_found";
    pub const PROJECT_DELETE_PLAN_STALE: &str = "project_delete_plan_stale";
    pub const PROJECT_DELETE_SOURCE_MISSING: &str = "project_delete_source_missing";
    pub const PROJECT_DELETE_SOURCE_UNSAFE: &str = "project_delete_source_unsafe";
    pub const PROJECT_DELETE_SOURCE_CHANGED: &str = "project_delete_source_changed";
    pub const PROJECT_DELETE_RECOVERY_REQUIRED: &str = "project_delete_recovery_required";
    pub const UNITY_LAUNCH_FAILED: &str = "unity_launch_failed";
    pub const UNITY_LAUNCH_NOT_FOUND: &str = "unity_launch_not_found";
    pub const TEMPLATE_NOT_FOUND: &str = "template_not_found";
    pub const TEMPLATE_BUILTIN_IMMUTABLE: &str = "template_builtin_immutable";
    pub const TEMPLATE_CONFLICT: &str = "template_conflict";
    pub const TEMPLATE_PLAN_STALE: &str = "template_plan_stale";
    pub const TEMPLATE_BUNDLE_INVALID: &str = "template_bundle_invalid";
    pub const TEMPLATE_DIGEST_MISMATCH: &str = "template_digest_mismatch";
    pub const TEMPLATE_PAYLOAD_UNAVAILABLE: &str = "template_payload_unavailable";
    pub const TEMPLATE_TARGET_EXISTS: &str = "template_target_exists";
    pub const PROJECT_CHANGED_DURING_TEMPLATE_CREATE: &str =
        "project_changed_during_template_create";
    pub const EXTENSION_MANIFEST_INVALID: &str = "extension_manifest_invalid";
    pub const EXTENSION_PACKAGE_INVALID: &str = "extension_package_invalid";
    pub const EXTENSION_PACKAGE_UNTRUSTED: &str = "extension_package_untrusted";
    pub const EXTENSION_PUBLISHER_CONFIRMATION_REQUIRED: &str =
        "extension_publisher_confirmation_required";
    pub const EXTENSION_SIGNATURE_INVALID: &str = "extension_signature_invalid";
    pub const EXTENSION_ALREADY_INSTALLED: &str = "extension_already_installed";
    pub const EXTENSION_NOT_INSTALLED: &str = "extension_not_installed";
    pub const EXTENSION_NOT_ENABLED: &str = "extension_not_enabled";
    pub const EXTENSION_PERMISSION_DENIED: &str = "extension_permission_denied";
    pub const EXTENSION_SCOPE_DENIED: &str = "extension_scope_denied";
    pub const EXTENSION_API_UNSUPPORTED: &str = "extension_api_unsupported";
    pub const EXTENSION_INSTANCE_STALE: &str = "extension_instance_stale";
    pub const EXTENSION_RESOURCE_LIMIT: &str = "extension_resource_limit";
    pub const EXTENSION_CRASHED: &str = "extension_crashed";
    pub const EXTENSION_QUARANTINED: &str = "extension_quarantined";
    pub const EXTENSION_PLAN_STALE: &str = "extension_plan_stale";
    pub const EXTENSION_DATA_QUOTA_EXCEEDED: &str = "extension_data_quota_exceeded";
    pub const EXTENSION_DATA_OWNER_MISMATCH: &str = "extension_data_owner_mismatch";
    pub const EXTENSION_RECOVERY_REQUIRED: &str = "extension_recovery_required";
    pub const EXTENSION_UI_NOT_AVAILABLE: &str = "extension_ui_not_available";
    pub const EXTENSION_UI_PROTOCOL_UNSUPPORTED: &str = "extension_ui_protocol_unsupported";
    pub const EXTENSION_UI_SESSION_NOT_FOUND: &str = "extension_ui_session_not_found";
    pub const EXTENSION_UI_SESSION_STALE: &str = "extension_ui_session_stale";
    pub const EXTENSION_UI_SNAPSHOT_STALE: &str = "extension_ui_snapshot_stale";
    pub const EXTENSION_UI_DOCUMENT_INVALID: &str = "extension_ui_document_invalid";
    pub const EXTENSION_UI_ACTION_INVALID: &str = "extension_ui_action_invalid";
    pub const EXTENSION_UI_LIMIT_EXCEEDED: &str = "extension_ui_limit_exceeded";
}

/// JSON-RPC-inspired request envelope.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RequestEnvelope {
    /// Non-empty request correlation identifier.
    pub id: String,
    /// Stable method name.
    pub method: String,
    /// Method-specific parameters.
    pub params: Value,
}

impl RequestEnvelope {
    /// Validates the envelope limits that JSON Schema cannot express in UTF-8 bytes.
    pub fn validate(&self) -> Result<(), ContractViolation> {
        validate_non_empty_utf8("id", &self.id, MAX_REQUEST_ID_BYTES)?;
        validate_method(&self.method)?;
        if !self.params.is_object() {
            return Err(ContractViolation::ParamsMustBeObject);
        }
        Ok(())
    }
}

/// Successful response envelope.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuccessResponse<T> {
    /// Request identifier copied from the request.
    pub id: String,
    /// Method-specific result.
    pub result: T,
}

/// Error response envelope.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    /// Request identifier, or `null` when no valid identifier could be recovered.
    pub id: Option<String>,
    /// Stable public error.
    pub error: RpcError,
}

/// A typed RPC response.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Response<T> {
    /// Successful response.
    Success(SuccessResponse<T>),
    /// Structured public error.
    Error(ErrorResponse),
}

/// Stable, non-sensitive public RPC error.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcError {
    /// Stable machine-readable code.
    pub code: String,
    /// Safe human-readable summary. Clients must not parse this field.
    pub message: String,
    /// Non-sensitive diagnostic correlation ID, required for `internal_error`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic_id: Option<String>,
    /// Error-specific non-sensitive structured data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl RpcError {
    /// Creates an `invalid_request` error.
    #[must_use]
    pub fn invalid_request() -> Self {
        Self::simple(
            error_code::INVALID_REQUEST,
            "The request payload is invalid.",
        )
    }

    /// Creates a `method_not_found` error.
    #[must_use]
    pub fn method_not_found() -> Self {
        Self::simple(
            error_code::METHOD_NOT_FOUND,
            "The requested method is not available.",
        )
    }

    /// Creates a `handshake_required` error.
    #[must_use]
    pub fn handshake_required() -> Self {
        Self::simple(
            error_code::HANDSHAKE_REQUIRED,
            "system.hello must complete before calling this method.",
        )
    }

    /// Creates a `handshake_already_completed` error.
    #[must_use]
    pub fn handshake_already_completed() -> Self {
        Self::simple(
            error_code::HANDSHAKE_ALREADY_COMPLETED,
            "system.hello has already completed on this connection.",
        )
    }

    /// Creates an `rpc_version_unsupported` error.
    #[must_use]
    pub fn rpc_version_unsupported(requested_version: u32) -> Self {
        Self {
            code: error_code::RPC_VERSION_UNSUPPORTED.to_owned(),
            message: "The requested RPC major version is not supported.".to_owned(),
            diagnostic_id: None,
            data: Some(json!({
                "requestedVersion": requested_version,
                "supportedVersions": [RPC_VERSION]
            })),
        }
    }

    /// Creates a `component_upgrade_required` error.
    #[must_use]
    pub fn component_upgrade_required() -> Self {
        Self::simple(
            error_code::COMPONENT_UPGRADE_REQUIRED,
            "A component upgrade is required to complete this request.",
        )
    }

    /// Creates a `capability_required` error naming the missing capability.
    #[must_use]
    pub fn capability_required(capability: &str) -> Self {
        Self {
            code: error_code::CAPABILITY_REQUIRED.to_owned(),
            message: "The method requires a capability not negotiated by this connection."
                .to_owned(),
            diagnostic_id: None,
            data: Some(json!({"requiredCapability": capability})),
        }
    }

    /// Creates a non-enumerating `permission_denied` error.
    #[must_use]
    pub fn permission_denied() -> Self {
        Self::simple(
            error_code::PERMISSION_DENIED,
            "The current Principal cannot access this resource.",
        )
    }

    /// Creates a `revision_conflict` error.
    #[must_use]
    pub fn revision_conflict() -> Self {
        Self::simple(
            error_code::REVISION_CONFLICT,
            "The Operation revision no longer matches the request.",
        )
    }

    /// Creates an `idempotency_conflict` error.
    #[must_use]
    pub fn idempotency_conflict() -> Self {
        Self::simple(
            error_code::IDEMPOTENCY_CONFLICT,
            "The idempotency key was already used for a different request.",
        )
    }

    /// Creates a non-enumerating `operation_not_found` error.
    #[must_use]
    pub fn operation_not_found() -> Self {
        Self::simple(
            error_code::OPERATION_NOT_FOUND,
            "The Operation was not found.",
        )
    }

    /// Creates an `operation_not_cancellable` error.
    #[must_use]
    pub fn operation_not_cancellable() -> Self {
        Self::simple(
            error_code::OPERATION_NOT_CANCELLABLE,
            "The Operation cannot accept cancellation in its current state.",
        )
    }

    /// Creates a ready-daemon `store_unavailable` error.
    #[must_use]
    pub fn store_unavailable() -> Self {
        Self::simple(
            error_code::STORE_UNAVAILABLE,
            "The state store could not complete the request.",
        )
    }

    /// Creates one of the frozen non-sensitive M3 resource errors.
    #[must_use]
    pub fn m3_resource(code: &str) -> Self {
        let message = match code {
            error_code::PATH_ENCODING_UNSUPPORTED => "The path encoding is unsupported.",
            error_code::PROJECT_NOT_FOUND => "The Unity project was not found.",
            error_code::PROJECT_NOT_REGISTERED => "The project is not registered.",
            error_code::PROJECT_ALREADY_REGISTERED => "The project is already registered.",
            error_code::PROJECT_INACCESSIBLE => "The project cannot be read.",
            error_code::PROJECT_VERSION_MISSING => "The Unity project version file is missing.",
            error_code::PROJECT_VERSION_INVALID => "The Unity project version is invalid.",
            error_code::PROJECT_MANIFEST_INVALID => "A project manifest is invalid.",
            error_code::REPOSITORY_NOT_FOUND => "The repository source was not found.",
            error_code::REPOSITORY_NOT_REGISTERED => "The repository is not registered.",
            error_code::REPOSITORY_ALREADY_REGISTERED => "The repository is already registered.",
            error_code::REPOSITORY_SOURCE_INVALID => "The repository source is invalid.",
            error_code::REPOSITORY_INACCESSIBLE => "The repository source cannot be read.",
            error_code::REPOSITORY_UNAVAILABLE => "The remote repository is unavailable.",
            error_code::REPOSITORY_DOCUMENT_INVALID => "The repository document is invalid.",
            error_code::REPOSITORY_DOCUMENT_TOO_LARGE => "The repository document is too large.",
            error_code::REPOSITORY_CREDENTIALS_UNSUPPORTED => {
                "Repository credentials are not supported in M3."
            }
            _ => "The resource request failed.",
        };
        Self::simple(code, message)
    }

    /// Creates a stable, non-sensitive M4 package transaction error.
    #[must_use]
    pub fn m4_resource(code: &str, subreason: Option<&str>) -> Self {
        Self {
            code: code.to_owned(),
            message: "The package transaction could not be completed.".to_owned(),
            diagnostic_id: None,
            data: subreason.map(|value| json!({"subreason": value})),
        }
    }

    /// Creates an `internal_error` with a non-sensitive diagnostic ID.
    #[must_use]
    pub fn internal(diagnostic_id: impl Into<String>) -> Self {
        Self {
            code: error_code::INTERNAL_ERROR.to_owned(),
            message: "An internal error occurred.".to_owned(),
            diagnostic_id: Some(diagnostic_id.into()),
            data: None,
        }
    }

    /// Creates a stable, non-sensitive M5 Unity error.
    #[must_use]
    pub fn unity(code: &str) -> Self {
        Self::simple(code, "The Unity request could not be completed.")
    }

    /// Creates a stable, non-sensitive M5 Template error.
    #[must_use]
    pub fn template(code: &str) -> Self {
        Self::simple(code, "The Template request could not be completed.")
    }

    /// Creates a stable, non-sensitive M5 Backup error.
    #[must_use]
    pub fn backup(code: &str) -> Self {
        Self::simple(code, "The Backup request could not be completed.")
    }

    /// Creates a stable, non-sensitive M7 Project Copy error.
    #[must_use]
    pub fn project_copy(code: &str) -> Self {
        Self::simple(code, "The Project Copy request could not be completed.")
    }

    /// Creates a stable, non-sensitive M7 Project Directory Delete error.
    #[must_use]
    pub fn project_delete(code: &str) -> Self {
        Self::simple(
            code,
            "The Project Directory Delete request could not be completed.",
        )
    }

    /// Creates a stable, non-sensitive M7 Project Unity migration error.
    #[must_use]
    pub fn unity_migration(code: &str) -> Self {
        Self::simple(code, "The Project Unity migration could not be completed.")
    }

    /// Creates a stable, non-sensitive M6 Extension Runtime error.
    #[must_use]
    pub fn extension(code: &str) -> Self {
        Self::simple(code, "The Extension request could not be completed.")
    }

    fn simple(code: &str, message: &str) -> Self {
        Self {
            code: code.to_owned(),
            message: message.to_owned(),
            diagnostic_id: None,
            data: None,
        }
    }
}

/// Parameters sent as the first valid request on every connection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HelloParams {
    /// RPC major version requested by the client.
    pub rpc_version: u32,
    /// Diagnostic client metadata. This is not a security identity.
    pub client: ClientInfo,
    /// Optional capabilities requested by the client.
    pub capabilities: Vec<String>,
}

impl HelloParams {
    /// Validates metadata and capability limits.
    pub fn validate(&self) -> Result<(), ContractViolation> {
        self.client.validate()?;
        validate_capabilities(&self.capabilities)
    }
}

/// Diagnostic client metadata included in `system.hello`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientInfo {
    /// Stable client program name such as `alcomd-cli`.
    pub name: String,
    /// Client semantic version.
    pub version: String,
    /// Per-process client instance identifier.
    pub instance_id: String,
}

impl ClientInfo {
    /// Validates diagnostic metadata limits.
    pub fn validate(&self) -> Result<(), ContractViolation> {
        validate_non_empty_utf8("client.name", &self.name, 64)?;
        validate_non_empty_utf8("client.version", &self.version, 64)?;
        validate_non_empty_utf8("client.instanceId", &self.instance_id, 128)
    }
}

/// Successful `system.hello` result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HelloResult {
    /// Negotiated RPC major version.
    pub rpc_version: u32,
    /// Running daemon version.
    pub daemon_version: String,
    /// Capabilities accepted by both peers.
    pub capabilities: Vec<String>,
    /// Ready state-store schema version. Absent when the store is unavailable or not initialized.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_schema: Option<u32>,
    /// Public settings Config Schema version, present only once durable config RPC is ready.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_schema: Option<u32>,
    /// Extension ABI information, present only once the M6 runtime is available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extension_api: Option<ExtensionApiInfo>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionApiInfo {
    pub major: u32,
    pub world: String,
}

impl HelloResult {
    /// Creates the minimal M1 hello result.
    #[must_use]
    pub fn m1() -> Self {
        Self {
            rpc_version: RPC_VERSION,
            daemon_version: env!("CARGO_PKG_VERSION").to_owned(),
            capabilities: Vec::new(),
            data_schema: None,
            config_schema: None,
            extension_api: None,
        }
    }

    /// Creates the M2 hello result after the state store is ready.
    #[must_use]
    pub fn m2(capabilities: Vec<String>) -> Self {
        Self {
            rpc_version: RPC_VERSION,
            daemon_version: env!("CARGO_PKG_VERSION").to_owned(),
            capabilities,
            data_schema: Some(1),
            config_schema: None,
            extension_api: None,
        }
    }

    /// Creates the M3 hello result after Schema v2 and read services are ready.
    #[must_use]
    pub fn m3(capabilities: Vec<String>) -> Self {
        Self {
            rpc_version: RPC_VERSION,
            daemon_version: env!("CARGO_PKG_VERSION").to_owned(),
            capabilities,
            data_schema: Some(2),
            config_schema: None,
            extension_api: None,
        }
    }

    /// Creates the M4 hello result after Schema v3 and package transactions are ready.
    #[must_use]
    pub fn m4(capabilities: Vec<String>) -> Self {
        Self {
            rpc_version: RPC_VERSION,
            daemon_version: env!("CARGO_PKG_VERSION").to_owned(),
            capabilities,
            data_schema: Some(3),
            config_schema: None,
            extension_api: None,
        }
    }

    /// Creates the M5 hello result after Schema v5 and local workflow services are ready.
    #[must_use]
    pub fn m5(capabilities: Vec<String>) -> Self {
        Self {
            rpc_version: RPC_VERSION,
            daemon_version: env!("CARGO_PKG_VERSION").to_owned(),
            capabilities,
            data_schema: Some(6),
            config_schema: None,
            extension_api: None,
        }
    }

    /// Creates the M6 hello result after Schema v8 and Extension ABI v1 are ready.
    #[must_use]
    pub fn m6(capabilities: Vec<String>) -> Self {
        Self {
            rpc_version: RPC_VERSION,
            daemon_version: env!("CARGO_PKG_VERSION").to_owned(),
            capabilities,
            data_schema: Some(8),
            config_schema: None,
            extension_api: Some(ExtensionApiInfo {
                major: 1,
                world: "alcomd:extension/extension-v1@1.0.0".to_owned(),
            }),
        }
    }

    /// Creates the M7 hello result after Schema v11 and Project preference wiring are ready.
    #[must_use]
    pub fn m7(capabilities: Vec<String>) -> Self {
        Self {
            rpc_version: RPC_VERSION,
            daemon_version: env!("CARGO_PKG_VERSION").to_owned(),
            capabilities,
            data_schema: Some(11),
            config_schema: None,
            extension_api: Some(ExtensionApiInfo {
                major: 1,
                world: "alcomd:extension/extension-v1@1.0.0".to_owned(),
            }),
        }
    }

    /// Creates the current M7 hello result after State Schema 13 and Config Schema 2 are ready.
    #[must_use]
    pub fn m7_official_gui(capabilities: Vec<String>) -> Self {
        let mut result = Self::m7(capabilities);
        result.data_schema = Some(13);
        result.config_schema = Some(2);
        result
    }
}

/// Public Operation lifecycle state.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationState {
    /// Waiting for dispatch.
    Queued,
    /// Producing a plan; reserved for a future milestone.
    Planning,
    /// Waiting for input; reserved for a future milestone.
    WaitingForInput,
    /// Running the operation.
    Running,
    /// Cancellation was requested.
    Cancelling,
    /// Completed successfully.
    Succeeded,
    /// Completed with an error.
    Failed,
    /// Cancelled before completion.
    Cancelled,
    /// Interrupted by process termination.
    Interrupted,
    /// Recovering from an interruption.
    Recovering,
}

/// Parameters for `state.check`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StateCheckParams {
    /// Caller-supplied idempotency key scoped to Principal and method.
    pub idempotency_key: String,
}

/// Result returned when a new or replayed Operation is accepted.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationAccepted {
    /// Stable Operation UUID.
    pub operation_id: String,
    /// Whether this result came from an idempotency replay.
    pub replayed: bool,
}

/// Parameters for `operations.get`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationsGetParams {
    /// Operation UUID visible to the current Principal.
    pub operation_id: String,
}

/// Public Operation representation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    /// Stable Operation UUID.
    pub operation_id: String,
    /// Stable operation kind. M2 only creates `state.check`.
    pub kind: String,
    /// Current lifecycle state.
    pub state: OperationState,
    /// Positive public revision bounded by signed SQLite integer range.
    pub revision: u64,
    /// Creation time as Unix epoch milliseconds.
    pub created_at_ms: u64,
    /// Last public update time as Unix epoch milliseconds.
    pub updated_at_ms: u64,
    /// Start time, when execution began.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at_ms: Option<u64>,
    /// Completion time for a terminal Operation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at_ms: Option<u64>,
    /// Safe method-specific result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Stable public failure code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    /// Non-sensitive diagnostic correlation ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic_id: Option<String>,
    /// Latest durable, non-sensitive package transaction phase.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<OperationProgress>,
}

/// Bounded progress exposed for a package transaction Operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationProgress {
    /// Latest durable filesystem transaction phase.
    pub phase: PackageOperationPhase,
}

/// Public package transaction phases. These names are stable RPC values.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageOperationPhase {
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

/// Opaque stable cursor for `operations.list`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationsListCursor {
    /// Creation time of the final item in the previous page.
    pub created_at_ms: u64,
    /// Operation UUID of the final item in the previous page.
    pub operation_id: String,
}

/// Parameters for `operations.list`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationsListParams {
    /// Exclusive tuple cursor for the next page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<OperationsListCursor>,
    /// Page size; defaults to 100 and cannot exceed 1000.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// Result for `operations.list`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationsListResult {
    /// Operations ordered by `(createdAtMs DESC, operationId DESC)`.
    pub operations: Vec<Operation>,
    /// Cursor for the final returned item, or absent when no item was returned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<OperationsListCursor>,
}

/// Parameters for `operations.cancel`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationsCancelParams {
    /// Operation UUID visible to the current Principal.
    pub operation_id: String,
    /// Required optimistic-concurrency revision.
    pub expected_revision: u64,
    /// Caller-supplied idempotency key scoped to Principal and method.
    pub idempotency_key: String,
}

/// Result for an idempotent Operation write.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationWriteResult {
    /// Current public Operation state.
    pub operation: Operation,
    /// Whether this result came from an idempotency replay.
    pub replayed: bool,
}

/// Parameters for `events.list`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EventsListParams {
    /// Exclusive global Event sequence cursor.
    pub after_sequence: u64,
    /// Page size; defaults to 100 and cannot exceed 1000.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// Public durable Event representation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    /// Positive global sequence allocated by SQLite AUTOINCREMENT.
    pub sequence: u64,
    /// Stable Event UUID.
    pub event_id: String,
    /// Stable Event kind.
    pub kind: String,
    /// Stable aggregate type.
    pub aggregate_kind: String,
    /// Aggregate identifier.
    pub aggregate_id: String,
    /// Aggregate revision after the state change committed with this Event.
    pub aggregate_revision: u64,
    /// Event time as Unix epoch milliseconds.
    pub occurred_at_ms: u64,
    /// Safe event-specific payload.
    pub payload: Value,
}

/// Result for `events.list`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventsListResult {
    /// Events ordered by strictly increasing sequence.
    pub events: Vec<Event>,
    /// Last returned sequence, or the input `afterSequence` for an empty page.
    pub next_sequence: u64,
}

/// Explicit Unity project discovery policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectDiscoveryMode {
    /// Inspect only the supplied directory.
    ExactRoot,
    /// Walk parent directories until a Unity root is found.
    SearchParents,
}

/// Frozen M3 project classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectType {
    Avatars,
    Worlds,
    VpmStarter,
    UpmAvatars,
    UpmWorlds,
    UpmStarter,
    LegacySdk2,
    LegacyWorlds,
    LegacyAvatars,
    Unknown,
}

/// Read state of an optional project manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManifestState {
    Missing,
    Valid,
}

/// Raw package identity/value pair; no SemVer meaning is implied.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyIdentity {
    pub package_id: String,
    pub value: String,
}

/// Bounded non-sensitive parse issue.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadIssue {
    pub code: String,
    pub component: String,
    pub item: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u64>,
}

/// Public normalized project snapshot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSnapshot {
    pub project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registered_at_ms: Option<u64>,
    pub root_path: String,
    pub project_type: ProjectType,
    pub unity_version: String,
    pub unity_revision: Option<String>,
    pub vpm_manifest: ManifestState,
    pub upm_manifest: ManifestState,
    pub direct_dependencies: Vec<DependencyIdentity>,
    pub locked_dependencies: Vec<DependencyIdentity>,
    pub issues: Vec<ReadIssue>,
    pub observed_at_ms: u64,
    pub revision: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub favorite: Option<bool>,
}

/// Local or anonymous remote repository source.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum RepositorySource {
    Local { path: String },
    Remote { url: String },
}

/// Public normalized repository metadata.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySnapshot {
    pub repository_id: Option<String>,
    pub source: RepositorySource,
    pub declared_id: Option<String>,
    pub name: Option<String>,
    pub declared_url: Option<String>,
    pub issues: Vec<ReadIssue>,
    pub revision: Option<u64>,
    pub refreshed_at_ms: u64,
}

/// M3 raw package/version display model.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryPackageVersion {
    pub package_id: String,
    pub version: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub yanked: bool,
    pub unity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prerelease: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub links: Option<RepositoryPackageLinks>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryPackageLinks {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documentation: Option<RepositoryPackageLink>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changelog: Option<RepositoryPackageLink>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryPackageLink {
    pub url: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegistryCursor {
    pub registered_at_ms: u64,
    pub id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackageCursor {
    pub package_id: String,
    pub version: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectsInspectParams {
    pub path: String,
    pub discovery_mode: ProjectDiscoveryMode,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegistryListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<RegistryCursor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectIdParams {
    pub project_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRegisterParams {
    pub path: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectMutationParams {
    pub project_id: String,
    pub expected_revision: u64,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectSetFavoriteParams {
    pub project_id: String,
    pub favorite: bool,
    pub expected_revision: u64,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryInspectParams {
    pub source: RepositorySource,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryIdParams {
    pub repository_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryPackagesParams {
    pub repository_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<PackageCursor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryRegisterParams {
    pub source: RepositorySource,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryMutationParams {
    pub repository_id: String,
    pub expected_revision: u64,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectResult {
    pub project: ProjectSnapshot,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectsListResult {
    pub projects: Vec<ProjectSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<RegistryCursor>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWriteResult {
    pub project: ProjectSnapshot,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectUnregisterResult {
    pub project_id: String,
    pub revision: u64,
    pub unregistered: bool,
    pub replayed: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryResult {
    pub repository: RepositorySnapshot,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoriesListResult {
    pub repositories: Vec<RepositorySnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<RegistryCursor>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryWriteResult {
    pub repository: RepositorySnapshot,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryUnregisterResult {
    pub repository_id: String,
    pub revision: u64,
    pub unregistered: bool,
    pub replayed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackagePlanAction {
    Install,
    Remove,
    Upgrade,
    Downgrade,
    Resolve,
    Reinstall,
    Bulk,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackagePlanState {
    Unapplied,
    Applied,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageMutationKind {
    Install,
    Remove,
    Replace,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryPackageSourcePin {
    pub repository_id: String,
    pub repository_revision: u64,
    pub source_identity: String,
    pub manifest_fingerprint: String,
    pub package_id: String,
    pub version: String,
    pub artifact_url: String,
    pub archive_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UserPackageSourcePin {
    pub user_package_id: String,
    pub source_revision: u64,
    pub source_identity: String,
    pub manifest_fingerprint: String,
    pub package_id: String,
    pub version: String,
    pub archive_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PackageSourcePin {
    Repository(RepositoryPackageSourcePin),
    UserPackage(UserPackageSourcePin),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackageMutation {
    pub kind: PackageMutationKind,
    pub package_id: String,
    pub from_version: Option<String>,
    pub to_version: Option<String>,
    pub source: Option<PackageSourcePin>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackageDependencyEdge {
    pub from_package_id: String,
    pub to_package_id: String,
    pub range: String,
    pub direct: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackageChangeSet {
    pub format_version: u32,
    pub mutations: Vec<PackageMutation>,
    pub dependency_edges: Vec<PackageDependencyEdge>,
    pub vpm_manifest_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackagePlan {
    pub plan_id: String,
    pub action: PackagePlanAction,
    pub state: PackagePlanState,
    pub project_id: String,
    pub project_revision: u64,
    pub change_set_fingerprint: String,
    pub change_set: PackageChangeSet,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackagePlanInstallParams {
    pub project_id: String,
    pub expected_revision: u64,
    pub package_id: String,
    #[serde(default)]
    pub version_range: Option<String>,
    #[serde(default)]
    pub repository_id: Option<String>,
    #[serde(default)]
    pub source: Option<PackageSourceSelector>,
    #[serde(default)]
    pub include_prerelease: bool,
}

pub type PackagePlanUpgradeParams = PackagePlanInstallParams;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackagePlanRemoveParams {
    pub project_id: String,
    pub expected_revision: u64,
    pub package_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackagePlanDowngradeParams {
    pub project_id: String,
    pub expected_revision: u64,
    pub package_id: String,
    pub version: String,
    #[serde(default)]
    pub repository_id: Option<String>,
    #[serde(default)]
    pub source: Option<PackageSourceSelector>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackagePlanResolveParams {
    pub project_id: String,
    pub expected_revision: u64,
    #[serde(default)]
    pub include_prerelease: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PackageSourceSelector {
    Repository { repository_id: String },
    UserPackage { user_package_id: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PackageReinstallSelection {
    Packages { package_ids: Vec<String> },
    All,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackageReinstallSource {
    pub package_id: String,
    pub source: PackageSourceSelector,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackagePlanReinstallParams {
    pub project_id: String,
    pub expected_revision: u64,
    pub selection: PackageReinstallSelection,
    #[serde(default)]
    pub sources: Vec<PackageReinstallSource>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PackageBulkIntent {
    Install {
        package_id: String,
        #[serde(default)]
        version_range: Option<String>,
        #[serde(default)]
        source: Option<PackageSourceSelector>,
        include_prerelease: bool,
    },
    Upgrade {
        package_id: String,
        #[serde(default)]
        version_range: Option<String>,
        #[serde(default)]
        source: Option<PackageSourceSelector>,
        include_prerelease: bool,
    },
    Remove {
        package_id: String,
    },
    Reinstall {
        package_id: String,
        #[serde(default)]
        source: Option<PackageSourceSelector>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackagePlanBulkParams {
    pub project_id: String,
    pub expected_revision: u64,
    pub intents: Vec<PackageBulkIntent>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackageApplyPlanParams {
    pub plan_id: String,
    pub expected_revision: u64,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UserPackageCursor {
    pub updated_at_ms: u64,
    pub user_package_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UserPackagesListParams {
    #[serde(default)]
    pub cursor: Option<UserPackageCursor>,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UserPackageGetParams {
    pub user_package_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UserPackageEnrollParams {
    pub source_path: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UserPackageMutationParams {
    pub user_package_id: String,
    pub expected_revision: u64,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UserPackageRecord {
    pub user_package_id: String,
    pub source_root_path: String,
    pub package_id: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    pub revision: u64,
    pub archive_sha256: String,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UserPackagesListResult {
    pub user_packages: Vec<UserPackageRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<UserPackageCursor>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UserPackageResult {
    pub user_package: UserPackageRecord,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UserPackageWriteResult {
    pub user_package: UserPackageRecord,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UserPackageRemoveResult {
    pub user_package_id: String,
    pub revision: u64,
    pub removed: bool,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageApplyPlanResult {
    pub operation_id: String,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryPackagesResult {
    pub packages: Vec<RepositoryPackageVersion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<PackageCursor>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnityArchitecture {
    X86_64,
    Arm64,
    Universal,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitySourceKind {
    Manual,
    HubConfig,
    KnownInstallRoot,
    UnityCliHint,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnityInstallation {
    pub installation_id: String,
    pub executable_path: String,
    pub filesystem_identity: String,
    pub unity_version: String,
    pub architecture: UnityArchitecture,
    pub source_kind: UnitySourceKind,
    pub revision: u64,
    pub observed_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UnityInstallationsListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnityInstallationsListResult {
    pub installations: Vec<UnityInstallation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UnityInstallationIdParams {
    pub installation_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UnityInstallationRegisterParams {
    pub executable_path: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UnityInstallationRemoveParams {
    pub installation_id: String,
    pub expected_revision: u64,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UnityInstallationRefreshParams {
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnityInstallationResult {
    pub installation: UnityInstallation,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnityInstallationRemoveResult {
    pub installation_id: String,
    pub removed: bool,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectUnityLaunchConfig {
    pub project_id: String,
    pub arguments: Vec<String>,
    pub revision: u64,
    pub updated_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UnityProjectIdParams {
    pub project_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectUnityLaunchConfigSetParams {
    pub project_id: String,
    pub arguments: Vec<String>,
    pub expected_revision: u64,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectUnityLaunchConfigResult {
    pub config: ProjectUnityLaunchConfig,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectUnityLaunchConfigClearParams {
    pub project_id: String,
    pub expected_revision: u64,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectUnityLaunchConfigMutationResult {
    pub config: ProjectUnityLaunchConfig,
    pub changed: bool,
    pub replayed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnityWriterStateKind {
    RunningConfirmed,
    RunningSuspected,
    NotObserved,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnityWriterEvidenceKind {
    ProcessProjectArgument,
    ProcessUnreadable,
    InspectionError,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnityWriterEvidence {
    pub kind: UnityWriterEvidenceKind,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnityWriterState {
    pub project_id: String,
    pub state: UnityWriterStateKind,
    pub evidence: Vec<UnityWriterEvidence>,
    pub checked_at_ms: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnityLaunchState {
    Opening,
    Open,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnityLaunchRecord {
    pub launch_id: String,
    pub project_id: String,
    pub installation_id: String,
    pub state: UnityLaunchState,
    pub spawn_accepted: bool,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UnityLaunchParams {
    pub project_id: String,
    pub installation_id: String,
    pub expected_project_revision: u64,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UnityLaunchOptionsParams {
    pub project_id: String,
    pub expected_project_revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnityLaunchOptionsResult {
    pub project_id: String,
    pub project_revision: u64,
    pub project_unity_version: String,
    pub exact_matching_installations: Vec<UnityInstallation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UnityLaunchStatusParams {
    pub launch_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnityLaunchResult {
    pub launch: UnityLaunchRecord,
    pub replayed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateSourceKind {
    Builtin,
    User,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateRecord {
    pub template_id: String,
    pub source_kind: TemplateSourceKind,
    pub template_version: String,
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub provenance: String,
    pub favorite: bool,
    pub bundle_sha256: String,
    pub manifest_fingerprint: String,
    pub revision: u64,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TemplatesListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplatesListResult {
    pub templates: Vec<TemplateRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TemplateIdParams {
    pub template_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateRecordResult {
    pub template: TemplateRecord,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TemplateInspectBundleParams {
    pub bundle_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateBundleInspection {
    pub format_version: u32,
    pub template_id: String,
    pub template_version: String,
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub provenance: String,
    pub bundle_sha256: String,
    pub manifest_fingerprint: String,
    pub payload_tree_sha256: String,
    pub entry_count: u64,
    pub total_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TemplatePlanImportParams {
    pub bundle_path: String,
    #[serde(rename = "override")]
    pub override_existing: bool,
    pub expected_revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TemplatePlanDeriveParams {
    pub project_id: String,
    pub expected_project_revision: u64,
    pub template_id: String,
    pub template_version: String,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TemplateApplyPlanParams {
    pub plan_id: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplatePlan {
    pub plan_id: String,
    pub action: String,
    pub state: String,
    pub plan_fingerprint: String,
    #[serde(flatten)]
    pub evidence: serde_json::Map<String, Value>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateApplyResult {
    pub operation_id: String,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TemplateExportParams {
    pub template_id: String,
    pub expected_revision: u64,
    pub target_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateExportResult {
    pub exported: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TemplateSetFavoriteParams {
    pub template_id: String,
    pub favorite: bool,
    pub expected_revision: u64,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TemplateRemoveParams {
    pub template_id: String,
    pub expected_revision: u64,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateRemoveResult {
    pub template_id: String,
    pub removed: bool,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TemplatePlanCreateProjectParams {
    pub template_id: String,
    pub expected_template_revision: u64,
    pub target_parent: String,
    pub target_leaf: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupCompression {
    Store,
    Fast,
    Maximum,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRecord {
    pub backup_id: String,
    pub source_project_id: String,
    pub archive_sha256: String,
    pub archive_bytes: u64,
    pub format_version: u32,
    pub created_at_ms: u64,
    pub compression_mode: BackupCompression,
    pub exclude_vpm_packages: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackupsListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupsListResult {
    pub backups: Vec<BackupRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackupGetParams {
    pub backup_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackupCreateParams {
    pub project_id: String,
    pub expected_revision: u64,
    pub compression_mode: BackupCompression,
    pub exclude_vpm_packages: bool,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupCreateResult {
    pub operation_id: String,
    pub backup_id: String,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackupPlanRestoreParams {
    pub backup_id: String,
    pub target_parent: String,
    pub target_leaf: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRestoreExcludedPackage {
    pub package_id: String,
    pub version: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRestoreTarget {
    pub parent: String,
    pub leaf: String,
    pub must_be_absent: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRestorePlan {
    pub plan_id: String,
    pub project_id: String,
    pub backup_id: String,
    pub target: BackupRestoreTarget,
    pub archive_sha256: String,
    pub packages_require_resolve: bool,
    pub excluded_packages: Vec<BackupRestoreExcludedPackage>,
    pub plan_fingerprint: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackupApplyRestoreParams {
    pub plan_id: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupApplyRestoreResult {
    pub operation_id: String,
    pub project_id: String,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectsPlanCopyParams {
    pub source_project_id: String,
    pub expected_revision: u64,
    pub target_parent_path: String,
    pub target_leaf: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCopyWriterEvidence {
    pub state: String,
    pub observed_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCopyQuota {
    pub max_entries: u64,
    pub max_single_file_bytes: u64,
    pub max_total_regular_file_bytes: u64,
    pub max_depth: u32,
    pub max_normalized_path_utf8_bytes: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCopyProfile {
    pub id: String,
    pub version: u32,
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
    pub rejects: Vec<String>,
    pub quota: ProjectCopyQuota,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCopyPlan {
    pub plan_id: String,
    pub owner_principal_id: String,
    pub source_project_id: String,
    pub source_project_revision: u64,
    pub source_canonical_root_path: String,
    pub source_filesystem_identity: String,
    pub source_project_kind: String,
    pub expected_unity_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_unity_revision: Option<String>,
    pub writer_evidence: ProjectCopyWriterEvidence,
    pub target_parent_canonical_path: String,
    pub target_parent_filesystem_identity: String,
    pub normalized_target_leaf: String,
    pub target_must_not_exist: bool,
    pub target_project_id: String,
    pub profile: ProjectCopyProfile,
    pub safe_exclusion_summary: Vec<String>,
    pub plan_fingerprint: String,
    pub idempotency_key: String,
    pub created_at_ms: u64,
    pub expires_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectsPlanCopyResult {
    pub plan: ProjectCopyPlan,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectsApplyCopyParams {
    pub plan_id: String,
    pub expected_revision: u64,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectsApplyCopyResult {
    pub operation_id: String,
    pub target_project_id: String,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectsPlanDeleteDirectoryParams {
    pub project_id: String,
    pub expected_revision: u64,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDeleteWriterEvidence {
    pub state: String,
    pub observed_at_ms: u64,
    pub safe_evidence: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDeleteProfile {
    pub id: String,
    pub version: u32,
    pub mode: String,
    pub protected_root_profile_version: u32,
    pub progress: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDeletePlan {
    pub plan_id: String,
    pub owner_principal_id: String,
    pub project_id: String,
    pub project_revision: u64,
    pub canonical_root_path: String,
    pub root_filesystem_identity: String,
    pub canonical_parent_path: String,
    pub parent_filesystem_identity: String,
    pub parent_identity_sha256: String,
    pub normalized_leaf: String,
    pub project_marker_sha256: String,
    pub expected_unity_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_unity_revision: Option<String>,
    pub writer_evidence: ProjectDeleteWriterEvidence,
    pub profile: ProjectDeleteProfile,
    pub plan_fingerprint: String,
    pub idempotency_key: String,
    pub created_at_ms: u64,
    pub expires_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectsPlanDeleteDirectoryResult {
    pub plan: ProjectDeletePlan,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectsApplyDeleteDirectoryParams {
    pub plan_id: String,
    pub expected_revision: u64,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectsApplyDeleteDirectoryResult {
    pub operation_id: String,
    pub project_id: String,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectsPlanUnityMigrationParams {
    pub project_id: String,
    pub target_installation_id: String,
    pub expected_project_revision: u64,
    pub idempotency_key: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectUnityMigrationClassificationKind {
    PatchOrMinorUpgrade,
    MajorUpgrade,
    PatchOrMinorDowngrade,
    MajorDowngrade,
    ChinaVariantChange,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectUnityMigrationClassification {
    pub kind: ProjectUnityMigrationClassificationKind,
    pub supported_for_apply: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectUnityMigrationPlan {
    pub plan_id: String,
    pub project_id: String,
    pub source_unity_version: String,
    pub target_unity_version: String,
    pub target_installation_id: String,
    pub classification: ProjectUnityMigrationClassification,
    pub expires_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ProjectsPlanUnityMigrationResult {
    NoChange {
        current_version: String,
    },
    Planned {
        plan: ProjectUnityMigrationPlan,
        replayed: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectsApplyUnityMigrationParams {
    pub plan_id: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectsApplyUnityMigrationResult {
    pub operation_id: String,
    pub replayed: bool,
}

pub const CONFIG_SCHEMA_VERSION: u32 = 2;
pub const MAX_OFFICIAL_GUI_PAGE_LIMIT: u32 = 200;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppearanceMode {
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppearanceDensity {
    Default,
    Compact,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppearanceMotion {
    System,
    Reduced,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SettingsLocale {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "en-US")]
    EnUs,
    #[serde(rename = "zh-CN")]
    ZhCn,
    #[serde(rename = "ja-JP")]
    JaJp,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppearanceSettings {
    pub mode: AppearanceMode,
    pub source_color: Option<String>,
    pub density: AppearanceDensity,
    pub motion: AppearanceMotion,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub appearance: AppearanceSettings,
    pub locale: SettingsLocale,
    pub packages: PackageSettings,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackageSettings {
    pub show_prerelease: bool,
    pub hidden_repository_ids: Vec<String>,
    pub hide_local_user_packages: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum NullableUpdate<T> {
    #[default]
    Unchanged,
    Clear,
    Set(T),
}

impl<T> NullableUpdate<T> {
    #[must_use]
    pub const fn is_unchanged(&self) -> bool {
        matches!(self, Self::Unchanged)
    }
}

impl<T: Serialize> Serialize for NullableUpdate<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Unchanged | Self::Clear => serializer.serialize_none(),
            Self::Set(value) => value.serialize(serializer),
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for NullableUpdate<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Option::<T>::deserialize(deserializer).map(|value| match value {
            Some(value) => Self::Set(value),
            None => Self::Clear,
        })
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppearanceSettingsUpdate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<AppearanceMode>,
    #[serde(default, skip_serializing_if = "NullableUpdate::is_unchanged")]
    pub source_color: NullableUpdate<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub density: Option<AppearanceDensity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion: Option<AppearanceMotion>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsUpdate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub appearance: Option<AppearanceSettingsUpdate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<SettingsLocale>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub packages: Option<PackageSettingsUpdate>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackageSettingsUpdate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_prerelease: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden_repository_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hide_local_user_packages: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsGetResult {
    pub config_schema: u32,
    pub revision: u64,
    pub settings: Settings,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsUpdateParams {
    pub expected_revision: u64,
    pub update: SettingsUpdate,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityCursor {
    pub occurred_at_ms: u64,
    pub source_rank: u8,
    pub stable_id: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<ActivityCursor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityItemType {
    Operation,
    Event,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityItem {
    pub occurred_at_ms: u64,
    #[serde(rename = "type")]
    pub item_type: ActivityItemType,
    pub summary_code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_sequence: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityListResult {
    pub items: Vec<ActivityItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<ActivityCursor>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticCursor {
    pub occurred_at_ms: u64,
    pub operation_id: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticsListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<DiagnosticCursor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Warning,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticItem {
    pub occurred_at_ms: u64,
    pub severity: DiagnosticSeverity,
    pub subsystem: String,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    pub summary: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsListResult {
    pub items: Vec<DiagnosticItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<DiagnosticCursor>,
}

/// Successful `system.status` result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemStatusResult {
    /// Stable product family.
    pub product: String,
    /// Running daemon version.
    pub daemon_version: String,
    /// Negotiated RPC major version.
    pub rpc_version: u32,
    /// Current minimal daemon readiness state.
    pub state: String,
    /// Non-sensitive capabilities available on this connection.
    pub capabilities: Vec<String>,
}

impl SystemStatusResult {
    /// Creates the minimal truthful M1 status result.
    #[must_use]
    pub fn ready() -> Self {
        Self {
            product: PRODUCT_FAMILY.to_owned(),
            daemon_version: env!("CARGO_PKG_VERSION").to_owned(),
            rpc_version: RPC_VERSION,
            state: "ready".to_owned(),
            capabilities: Vec::new(),
        }
    }
}

/// Framing failures close the connection without an RPC response.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameError {
    /// A zero-length frame is invalid.
    ZeroLength,
    /// The declared payload exceeds the 4 MiB contract limit.
    TooLarge,
}

impl fmt::Display for FrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroLength => formatter.write_str("RPC frame payload length is zero"),
            Self::TooLarge => formatter.write_str("RPC frame payload exceeds 4 MiB"),
        }
    }
}

impl std::error::Error for FrameError {}

/// Validates and decodes a little-endian frame prefix.
pub fn decode_frame_length(prefix: [u8; 4]) -> Result<usize, FrameError> {
    let length = u32::from_le_bytes(prefix) as usize;
    validate_frame_length(length)?;
    Ok(length)
}

/// Prepends the approved little-endian frame length to a complete payload.
pub fn encode_frame(payload: &[u8]) -> Result<Vec<u8>, FrameError> {
    validate_frame_length(payload.len())?;
    let length = u32::try_from(payload.len()).map_err(|_| FrameError::TooLarge)?;
    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(&length.to_le_bytes());
    frame.extend_from_slice(payload);
    Ok(frame)
}

fn validate_frame_length(length: usize) -> Result<(), FrameError> {
    if length == 0 {
        return Err(FrameError::ZeroLength);
    }
    if length > MAX_FRAME_PAYLOAD_BYTES {
        return Err(FrameError::TooLarge);
    }
    Ok(())
}

/// Contract validation failure for a complete RPC payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContractViolation {
    /// A required string is empty.
    Empty(&'static str),
    /// A string exceeds its UTF-8 byte limit.
    TooLong(&'static str),
    /// A method name does not follow the stable ASCII format.
    InvalidMethod,
    /// Params must be a JSON object.
    ParamsMustBeObject,
    /// Too many capabilities were declared.
    TooManyCapabilities,
    /// A capability is empty, too long, malformed, or duplicated.
    InvalidCapability,
}

impl fmt::Display for ContractViolation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty(field) => write!(formatter, "{field} must not be empty"),
            Self::TooLong(field) => write!(formatter, "{field} exceeds its byte limit"),
            Self::InvalidMethod => formatter.write_str("method has an invalid format"),
            Self::ParamsMustBeObject => formatter.write_str("params must be an object"),
            Self::TooManyCapabilities => formatter.write_str("too many capabilities"),
            Self::InvalidCapability => formatter.write_str("invalid capability set"),
        }
    }
}

impl std::error::Error for ContractViolation {}

fn validate_non_empty_utf8(
    field: &'static str,
    value: &str,
    max_bytes: usize,
) -> Result<(), ContractViolation> {
    if value.is_empty() {
        return Err(ContractViolation::Empty(field));
    }
    if value.len() > max_bytes {
        return Err(ContractViolation::TooLong(field));
    }
    Ok(())
}

fn validate_method(method: &str) -> Result<(), ContractViolation> {
    if method.is_empty() || method.len() > MAX_METHOD_BYTES || !method.is_ascii() {
        return Err(ContractViolation::InvalidMethod);
    }
    let mut segments = method.split('.');
    let Some(first) = segments.next() else {
        return Err(ContractViolation::InvalidMethod);
    };
    let Some(second) = segments.next() else {
        return Err(ContractViolation::InvalidMethod);
    };
    if !valid_method_segment(first)
        || !valid_method_segment(second)
        || !segments.all(valid_method_segment)
    {
        return Err(ContractViolation::InvalidMethod);
    }
    Ok(())
}

fn valid_method_segment(segment: &str) -> bool {
    let mut bytes = segment.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z')) && bytes.all(|byte| byte.is_ascii_alphanumeric())
}

fn validate_capabilities(capabilities: &[String]) -> Result<(), ContractViolation> {
    if capabilities.len() > MAX_CAPABILITIES {
        return Err(ContractViolation::TooManyCapabilities);
    }
    let mut unique = HashSet::with_capacity(capabilities.len());
    for capability in capabilities {
        if capability.is_empty()
            || capability.len() > MAX_CAPABILITY_BYTES
            || !capability.is_ascii()
            || !capability.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'_' | b'-')
            })
            || !unique.insert(capability)
        {
            return Err(ContractViolation::InvalidCapability);
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionsListParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionIdParams {
    pub extension_id: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionSourceKind {
    LocalOwnerSelected,
    FirstPartyPackaged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionPublisherApproval {
    None,
    ApproveForExtension,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionPlanInstallParams {
    pub source_kind: ExtensionSourceKind,
    pub package_path: String,
    pub expected_revision: u64,
    pub publisher_approval: ExtensionPublisherApproval,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionDataDisposition {
    RetainData,
    DeleteData,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionPlanUninstallParams {
    pub extension_id: String,
    pub expected_revision: u64,
    pub data_disposition: ExtensionDataDisposition,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionApplyParams {
    pub plan_id: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionLifecycleParams {
    pub extension_id: String,
    pub expected_revision: u64,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionGrantParams {
    pub extension_id: String,
    pub permission: String,
    pub resource_kind: String,
    pub resource_id: String,
    pub expected_grant_revision: u64,
    pub idempotency_key: String,
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionRecord {
    pub extension_id: String,
    pub version: String,
    pub api_major: u32,
    pub package_digest: String,
    pub publisher_fingerprint: String,
    pub trust_decision: ExtensionTrustDecision,
    pub desired_state: ExtensionDesiredState,
    pub quarantine_state: ExtensionQuarantineState,
    pub runtime_state: ExtensionRuntimeState,
    pub grant_revision: u64,
    pub lifecycle_generation: u64,
    pub revision: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui: Option<ExtensionUiDeclaration>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionPlan {
    pub plan_id: String,
    pub action: String,
    pub state: String,
    pub source_kind: String,
    pub extension_id: String,
    pub version: String,
    pub api_major: u32,
    pub profile_version: u32,
    pub package_digest: String,
    pub publisher_fingerprint: String,
    pub trust_decision: ExtensionTrustDecision,
    pub data_disposition: String,
    pub plan_fingerprint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui_protocol: Option<ExtensionUiProtocol>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionsListResult {
    pub extensions: Vec<ExtensionRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionResult {
    pub extension: ExtensionRecord,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionPlanResult {
    pub plan: ExtensionPlan,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionOperationResult {
    pub operation_id: String,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionGrantResult {
    pub extension_id: String,
    pub grant_revision: u64,
    pub state: String,
    pub replayed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_prefix_is_little_endian() {
        let frame = encode_frame(b"{}").expect("encode frame");
        assert_eq!(&frame[..4], &[2, 0, 0, 0]);
        assert_eq!(decode_frame_length([2, 0, 0, 0]), Ok(2));
    }

    #[test]
    fn frame_length_rejects_zero_and_over_limit() {
        assert_eq!(decode_frame_length([0; 4]), Err(FrameError::ZeroLength));
        let over_limit = (MAX_FRAME_PAYLOAD_BYTES as u32 + 1).to_le_bytes();
        assert_eq!(decode_frame_length(over_limit), Err(FrameError::TooLarge));
    }

    #[test]
    fn request_limits_use_utf8_bytes() {
        let request = RequestEnvelope {
            id: "界".repeat(22),
            method: METHOD_SYSTEM_STATUS.to_owned(),
            params: json!({}),
        };
        assert_eq!(request.validate(), Err(ContractViolation::TooLong("id")));
    }

    #[test]
    fn method_requires_two_ascii_segments() {
        for method in ["status", "System.status", "system.", "system.sta-tus"] {
            let request = RequestEnvelope {
                id: "1".to_owned(),
                method: method.to_owned(),
                params: json!({}),
            };
            assert_eq!(request.validate(), Err(ContractViolation::InvalidMethod));
        }
        let request = RequestEnvelope {
            id: "1".to_owned(),
            method: METHOD_PACKAGES_APPLY_PLAN.to_owned(),
            params: json!({}),
        };
        assert!(request.validate().is_ok());
    }

    #[test]
    fn hello_rejects_duplicate_and_unknown_format_capabilities() {
        let mut hello = HelloParams {
            rpc_version: RPC_VERSION,
            client: ClientInfo {
                name: "alcomd-cli".to_owned(),
                version: "4.0.0-alpha.0".to_owned(),
                instance_id: "test-instance".to_owned(),
            },
            capabilities: vec!["events".to_owned(), "events".to_owned()],
        };
        assert_eq!(hello.validate(), Err(ContractViolation::InvalidCapability));
        hello.capabilities = vec!["Unsafe Capability".to_owned()];
        assert_eq!(hello.validate(), Err(ContractViolation::InvalidCapability));
    }

    #[test]
    fn hello_and_status_do_not_advertise_future_subsystems() {
        let hello = serde_json::to_value(HelloResult::m1()).expect("serialize hello");
        let status = serde_json::to_value(SystemStatusResult::ready()).expect("serialize status");
        for forbidden in ["dataSchema", "configSchema", "extensionApi", "pid"] {
            assert!(hello.get(forbidden).is_none());
            assert!(status.get(forbidden).is_none());
        }
    }

    #[test]
    fn m7_official_gui_hello_advertises_current_schemas() {
        let hello = HelloResult::m7_official_gui(Vec::new());
        assert_eq!(hello.data_schema, Some(13));
        assert_eq!(hello.config_schema, Some(2));
    }

    #[test]
    fn internal_error_has_only_safe_diagnostic_id() {
        let error = RpcError::internal("00000000-0000-4000-8000-000000000000");
        let value = serde_json::to_value(error).expect("serialize error");
        assert_eq!(value["code"], error_code::INTERNAL_ERROR);
        assert_eq!(
            value["diagnosticId"],
            "00000000-0000-4000-8000-000000000000"
        );
        assert!(value.get("data").is_none());
    }

    #[test]
    fn response_ignores_unknown_optional_fields() {
        let value = json!({
            "id": "hello-1",
            "result": {
                "rpcVersion": 1,
                "daemonVersion": "4.0.0-alpha.0",
                "capabilities": ["future.capability"],
                "futureOptional": true
            },
            "futureEnvelopeField": true
        });
        let response: Response<HelloResult> =
            serde_json::from_value(value).expect("ignore compatible response additions");
        let Response::Success(success) = response else {
            panic!("expected success response");
        };
        assert_eq!(success.result.capabilities, ["future.capability"]);
    }

    #[test]
    fn package_source_pin_preserves_v1_repository_wire_shape_and_adds_unambiguous_v2_user_shape() {
        let repository = PackageSourcePin::Repository(RepositoryPackageSourcePin {
            repository_id: "00000000-0000-4000-8000-000000000001".to_owned(),
            repository_revision: 2,
            source_identity: "repository-source".to_owned(),
            manifest_fingerprint: "11".repeat(32),
            package_id: "com.example.package".to_owned(),
            version: "1.2.3".to_owned(),
            artifact_url: "https://example.invalid/package.zip".to_owned(),
            archive_sha256: "22".repeat(32),
        });
        assert_eq!(
            serde_json::to_value(repository).expect("repository pin"),
            json!({
                "repositoryId": "00000000-0000-4000-8000-000000000001",
                "repositoryRevision": 2,
                "sourceIdentity": "repository-source",
                "manifestFingerprint": "11".repeat(32),
                "packageId": "com.example.package",
                "version": "1.2.3",
                "artifactUrl": "https://example.invalid/package.zip",
                "archiveSha256": "22".repeat(32)
            })
        );

        let user = PackageSourcePin::UserPackage(UserPackageSourcePin {
            user_package_id: "00000000-0000-4000-8000-000000000002".to_owned(),
            source_revision: 3,
            source_identity: "opaque-user-source".to_owned(),
            manifest_fingerprint: "33".repeat(32),
            package_id: "com.example.package".to_owned(),
            version: "1.2.3".to_owned(),
            archive_sha256: "44".repeat(32),
        });
        let value = serde_json::to_value(user).expect("user pin");
        assert_eq!(value["sourceRevision"], 3);
        assert!(value.get("userPackageId").is_some());
        assert!(value.get("repositoryId").is_none());
        assert!(value.get("artifactUrl").is_none());
        assert!(value.get("kind").is_none());
    }
}
