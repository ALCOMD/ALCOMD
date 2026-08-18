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

/// `system.hello` method name.
pub const METHOD_SYSTEM_HELLO: &str = "system.hello";

/// `system.status` method name.
pub const METHOD_SYSTEM_STATUS: &str = "system.status";

/// Stable M1 error codes.
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
}

impl HelloResult {
    /// Creates the minimal M1 hello result.
    #[must_use]
    pub fn m1() -> Self {
        Self {
            rpc_version: RPC_VERSION,
            daemon_version: env!("CARGO_PKG_VERSION").to_owned(),
            capabilities: Vec::new(),
        }
    }
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
    matches!(bytes.next(), Some(b'a'..=b'z'))
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
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
