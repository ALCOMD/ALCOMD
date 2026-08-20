//! Public ALCOMD RPC v1 data-transfer objects and framing contract.
//!
//! The protocol is JSON-RPC-inspired, but it is not JSON-RPC 2.0 compatible.
//! Internal domain and application types must not leak into this crate.

use std::collections::HashSet;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

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
pub const METHOD_PACKAGES_APPLY_PLAN: &str = "packages.applyPlan";
pub const METHOD_UNITY_INSTALLATIONS_LIST: &str = "unity.installations.list";
pub const METHOD_UNITY_INSTALLATIONS_GET: &str = "unity.installations.get";
pub const METHOD_UNITY_INSTALLATIONS_REGISTER: &str = "unity.installations.register";
pub const METHOD_UNITY_INSTALLATIONS_REMOVE: &str = "unity.installations.remove";
pub const METHOD_UNITY_INSTALLATIONS_REFRESH: &str = "unity.installations.refresh";
pub const METHOD_UNITY_PROJECT_EDITOR_GET: &str = "unity.projectEditor.get";
pub const METHOD_UNITY_PROJECT_EDITOR_SET: &str = "unity.projectEditor.set";
pub const METHOD_UNITY_WRITER_STATE: &str = "unity.writerState";
pub const METHOD_UNITY_LAUNCH: &str = "unity.launch";
pub const METHOD_UNITY_LAUNCH_STATUS: &str = "unity.launchStatus";

/// Capability required by `state.check`.
pub const CAPABILITY_STATE_CHECK_V1: &str = "state.check.v1";

/// Capability required by operation methods.
pub const CAPABILITY_OPERATIONS_V1: &str = "operations.v1";

/// Capability required by `events.list`.
pub const CAPABILITY_EVENTS_REPLAY_V1: &str = "events.replay.v1";
pub const CAPABILITY_PROJECTS_READ_V1: &str = "projects.read.v1";
pub const CAPABILITY_PROJECTS_REGISTRY_V1: &str = "projects.registry.v1";
pub const CAPABILITY_REPOSITORIES_READ_V1: &str = "repositories.read.v1";
pub const CAPABILITY_REPOSITORIES_REGISTRY_V1: &str = "repositories.registry.v1";
pub const CAPABILITY_PACKAGES_PLAN_V1: &str = "packages.plan.v1";
pub const CAPABILITY_PACKAGES_APPLY_V1: &str = "packages.apply.v1";
pub const CAPABILITY_UNITY_READ_V1: &str = "unity.read.v1";
pub const CAPABILITY_UNITY_MANAGE_V1: &str = "unity.manage.v1";
pub const CAPABILITY_UNITY_LAUNCH_V1: &str = "unity.launch.v1";

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
    pub const UNITY_VERSION_UNVERIFIED: &str = "unity_version_unverified";
    pub const UNITY_VERSION_MISMATCH: &str = "unity_version_mismatch";
    pub const UNITY_ARCHITECTURE_UNSUPPORTED: &str = "unity_architecture_unsupported";
    pub const UNITY_PROJECT_RUNNING: &str = "unity_project_running";
    pub const UNITY_LAUNCH_STATE_UNCERTAIN: &str = "unity_launch_state_uncertain";
    pub const UNITY_PROJECT_SELECTOR_FORBIDDEN: &str = "unity_project_selector_forbidden";
    pub const UNITY_LAUNCH_FAILED: &str = "unity_launch_failed";
    pub const UNITY_LAUNCH_NOT_FOUND: &str = "unity_launch_not_found";
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
        }
    }

    /// Creates the M5 hello result after Schema v5 and local workflow services are ready.
    #[must_use]
    pub fn m5(capabilities: Vec<String>) -> Self {
        Self {
            rpc_version: RPC_VERSION,
            daemon_version: env!("CARGO_PKG_VERSION").to_owned(),
            capabilities,
            data_schema: Some(5),
        }
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
    ArchiveReady,
    Extracted,
    Prepared,
    PackagesReplaced,
    VpmManifestCommitted,
    FilesystemCommitted,
    StateCommitted,
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
pub struct PackageSourcePin {
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackageApplyPlanParams {
    pub plan_id: String,
    pub expected_revision: u64,
    pub idempotency_key: String,
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
pub struct ProjectEditorPreference {
    pub project_id: String,
    pub installation_id: String,
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
pub struct ProjectEditorSetParams {
    pub project_id: String,
    pub installation_id: String,
    pub arguments: Vec<String>,
    pub expected_revision: u64,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectEditorResult {
    pub preference: ProjectEditorPreference,
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
    pub expected_project_revision: u64,
    pub idempotency_key: String,
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
}
