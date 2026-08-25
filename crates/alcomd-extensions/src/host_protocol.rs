use std::io::{self, Read, Write};

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const HOST_PROTOCOL_VERSION: u32 = 1;
pub const MAX_HOST_FRAME_BYTES: usize = 512 * 1024;
pub const MAX_WIT_VALUE_BYTES: usize = 256 * 1024;

#[must_use]
pub fn bootstrap_nonce() -> String {
    let mut digest = Sha256::new();
    digest.update(b"ALCOMD-EXT-BOOTSTRAP-NONCE-V1\0");
    for _ in 0..4 {
        digest.update(
            alcomd_application::OperationId::new()
                .to_string()
                .as_bytes(),
        );
    }
    let bytes: [u8; 32] = digest.finalize().into();
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Issues one daemon-side opaque authority token for exactly one guest export.
#[must_use]
pub fn invocation_context_id() -> String {
    let mut digest = Sha256::new();
    digest.update(b"ALCOMD-EXT-INVOCATION-CONTEXT-V1\0");
    for _ in 0..4 {
        digest.update(
            alcomd_application::OperationId::new()
                .to_string()
                .as_bytes(),
        );
    }
    let bytes: [u8; 32] = digest.finalize().into();
    format!("ictx_{}", base64url_unpadded(&bytes))
}

/// Computes the frozen in-memory Portable UI action replay fingerprint.
#[must_use]
pub fn portable_ui_action_fingerprint(canonical_action: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"ALCOMD-PORTABLE-UI-ACTION-V1\0");
    digest.update(canonical_action);
    digest.finalize().into()
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeLimits {
    pub linear_memory_bytes: u64,
    pub table_elements: u32,
    pub concurrent_guest_calls: u32,
    pub wit_input_bytes: u32,
    pub wit_output_bytes: u32,
    pub host_protocol_frame_bytes: u32,
    pub host_call_window_ms: u64,
    pub host_calls_per_window: u32,
    pub host_call_burst: u32,
    pub fuel_per_guest_call: u64,
    pub epoch_tick_ms: u64,
    pub wall_timeout_ms: u64,
    pub activate_timeout_ms: u64,
    pub deactivate_timeout_ms: u64,
    pub background_lease_ms: u64,
    pub crash_threshold: u32,
    pub crash_window_ms: u64,
    pub restart_delays_ms: [u64; 2],
}

impl Default for RuntimeLimits {
    fn default() -> Self {
        Self {
            linear_memory_bytes: 64 * 1024 * 1024,
            table_elements: 10_000,
            concurrent_guest_calls: 1,
            wit_input_bytes: 256 * 1024,
            wit_output_bytes: 256 * 1024,
            host_protocol_frame_bytes: 512 * 1024,
            host_call_window_ms: 60_000,
            host_calls_per_window: 100,
            host_call_burst: 20,
            fuel_per_guest_call: 10_000_000,
            epoch_tick_ms: 10,
            wall_timeout_ms: 2_000,
            activate_timeout_ms: 5_000,
            deactivate_timeout_ms: 2_000,
            background_lease_ms: 60_000,
            crash_threshold: 3,
            crash_window_ms: 300_000,
            restart_delays_ms: [1_000, 5_000],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostMessage {
    pub protocol_version: u32,
    pub daemon_epoch: String,
    pub instance_id: String,
    pub lifecycle_generation: u64,
    pub sequence: u64,
    #[serde(flatten)]
    pub body: HostMessageBody,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HostMessageWire {
    protocol_version: u32,
    daemon_epoch: String,
    instance_id: String,
    lifecycle_generation: u64,
    sequence: u64,
    #[serde(flatten)]
    body: HostMessageBody,
}

impl<'de> Deserialize<'de> for HostMessage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let original = Value::deserialize(deserializer)?;
        let wire: HostMessageWire =
            serde_json::from_value(original.clone()).map_err(serde::de::Error::custom)?;
        let normalized = serde_json::to_value(&wire).map_err(serde::de::Error::custom)?;
        if original != normalized {
            return Err(serde::de::Error::custom(
                "unknown or non-canonical host field",
            ));
        }
        Ok(Self {
            protocol_version: wire.protocol_version,
            daemon_epoch: wire.daemon_epoch,
            instance_id: wire.instance_id,
            lifecycle_generation: wire.lifecycle_generation,
            sequence: wire.sequence,
            body: wire.body,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum HostMessageBody {
    Bootstrap {
        nonce: String,
        lease_id: String,
        extension_id: String,
        api_world: String,
        limits: RuntimeLimits,
    },
    Ready {
        nonce: String,
    },
    InvokeExport {
        request_id: String,
        invocation_context_id: String,
        export: String,
        input: Value,
    },
    CancelCall {
        request_id: String,
    },
    CapabilityCall {
        call_id: String,
        invocation_context_id: String,
        lease_id: String,
        capability: String,
        input: Value,
    },
    CapabilityResult {
        call_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<HostStableError>,
    },
    CapabilityCancelled {
        call_id: String,
    },
    ExportResult {
        request_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<HostStableError>,
    },
    RevokeLease {
        lease_id: String,
    },
    Shutdown,
    HostFault {
        code: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        diagnostic_id: Option<String>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HostStableError {
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic_id: Option<String>,
}

impl HostMessage {
    pub fn validate_bounds(&self) -> Result<(), HostProtocolError> {
        if self.protocol_version != HOST_PROTOCOL_VERSION
            || self.daemon_epoch.len() != 36
            || self.instance_id.len() != 36
            || self.lifecycle_generation == 0
            || self.sequence == 0
        {
            return Err(HostProtocolError::InvalidEnvelope);
        }
        match &self.body {
            HostMessageBody::Bootstrap {
                nonce,
                lease_id,
                extension_id,
                api_world,
                limits,
            } => {
                if nonce.len() != 64
                    || lease_id.len() != 36
                    || !(3..=255).contains(&extension_id.len())
                    || api_world != "alcomd:extension/extension-v1@1.0.0"
                    || limits != &RuntimeLimits::default()
                {
                    return Err(HostProtocolError::InvalidEnvelope);
                }
            }
            HostMessageBody::Ready { nonce } => {
                if nonce.len() != 64 {
                    return Err(HostProtocolError::InvalidEnvelope);
                }
            }
            HostMessageBody::InvokeExport {
                request_id,
                invocation_context_id,
                export,
                input,
            } => {
                validate_id(request_id)?;
                validate_invocation_context_id(invocation_context_id)?;
                if !matches!(
                    export.as_str(),
                    "activate"
                        | "deactivate"
                        | "ui.open"
                        | "ui.refresh"
                        | "ui.dispatch"
                        | "ui.close"
                ) {
                    return Err(HostProtocolError::InvalidEnvelope);
                }
                validate_value(input)?;
            }
            HostMessageBody::CancelCall { request_id } => validate_id(request_id)?,
            HostMessageBody::CapabilityCall {
                call_id,
                invocation_context_id,
                lease_id,
                capability,
                input,
            } => {
                validate_id(call_id)?;
                validate_invocation_context_id(invocation_context_id)?;
                if lease_id.len() != 36
                    || !matches!(
                        capability.as_str(),
                        "host-projects.get-summary"
                            | "host-data.get"
                            | "host-data.set"
                            | "host-data.delete"
                    )
                {
                    return Err(HostProtocolError::InvalidEnvelope);
                }
                validate_value(input)?;
            }
            HostMessageBody::CapabilityResult {
                call_id,
                result,
                error,
            }
            | HostMessageBody::ExportResult {
                request_id: call_id,
                result,
                error,
            } => {
                validate_id(call_id)?;
                if result.is_some() == error.is_some() {
                    return Err(HostProtocolError::InvalidEnvelope);
                }
                if let Some(result) = result {
                    validate_value(result)?;
                }
                if let Some(error) = error {
                    validate_error(error)?;
                }
            }
            HostMessageBody::CapabilityCancelled { call_id } => validate_id(call_id)?,
            HostMessageBody::RevokeLease { lease_id } => {
                if lease_id.len() != 36 {
                    return Err(HostProtocolError::InvalidEnvelope);
                }
            }
            HostMessageBody::Shutdown => {}
            HostMessageBody::HostFault {
                code,
                diagnostic_id,
            } => {
                if !matches!(
                    code.as_str(),
                    "extension_crashed" | "extension_resource_limit" | "internal_error"
                ) || diagnostic_id
                    .as_ref()
                    .is_some_and(|value| value.len() != 36)
                {
                    return Err(HostProtocolError::InvalidEnvelope);
                }
            }
        }
        let bytes = serde_json::to_vec(self).map_err(|_| HostProtocolError::InvalidEnvelope)?;
        if bytes.is_empty() || bytes.len() > MAX_HOST_FRAME_BYTES {
            return Err(HostProtocolError::FrameLimit);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostProtocolError {
    Io,
    Truncated,
    FrameLimit,
    InvalidEnvelope,
}

pub fn read_host_message(reader: &mut impl Read) -> Result<HostMessage, HostProtocolError> {
    let mut prefix = [0_u8; 4];
    reader
        .read_exact(&mut prefix)
        .map_err(|error| map_read_error(&error))?;
    let length = frame_length(prefix)?;
    let mut payload = vec![0_u8; length];
    reader
        .read_exact(&mut payload)
        .map_err(|error| map_read_error(&error))?;
    decode_message(&payload)
}

pub fn write_host_message(
    writer: &mut impl Write,
    message: &HostMessage,
) -> Result<(), HostProtocolError> {
    let frame = encode_message(message)?;
    writer
        .write_all(&frame)
        .map_err(|_| HostProtocolError::Io)?;
    writer.flush().map_err(|_| HostProtocolError::Io)
}

pub async fn read_host_message_async(
    reader: &mut (impl AsyncRead + Unpin),
) -> Result<HostMessage, HostProtocolError> {
    let mut prefix = [0_u8; 4];
    reader
        .read_exact(&mut prefix)
        .await
        .map_err(|error| map_read_error(&error))?;
    let length = frame_length(prefix)?;
    let mut payload = vec![0_u8; length];
    reader
        .read_exact(&mut payload)
        .await
        .map_err(|error| map_read_error(&error))?;
    decode_message(&payload)
}

pub async fn write_host_message_async(
    writer: &mut (impl AsyncWrite + Unpin),
    message: &HostMessage,
) -> Result<(), HostProtocolError> {
    let frame = encode_message(message)?;
    writer
        .write_all(&frame)
        .await
        .map_err(|_| HostProtocolError::Io)?;
    writer.flush().await.map_err(|_| HostProtocolError::Io)
}

fn encode_message(message: &HostMessage) -> Result<Vec<u8>, HostProtocolError> {
    message.validate_bounds()?;
    let payload = serde_json::to_vec(message).map_err(|_| HostProtocolError::InvalidEnvelope)?;
    let length = u32::try_from(payload.len()).map_err(|_| HostProtocolError::FrameLimit)?;
    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(&length.to_le_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

fn decode_message(payload: &[u8]) -> Result<HostMessage, HostProtocolError> {
    let message = serde_json::from_slice::<HostMessage>(payload)
        .map_err(|_| HostProtocolError::InvalidEnvelope)?;
    message.validate_bounds()?;
    Ok(message)
}

fn frame_length(prefix: [u8; 4]) -> Result<usize, HostProtocolError> {
    let length =
        usize::try_from(u32::from_le_bytes(prefix)).map_err(|_| HostProtocolError::FrameLimit)?;
    if !(1..=MAX_HOST_FRAME_BYTES).contains(&length) {
        return Err(HostProtocolError::FrameLimit);
    }
    Ok(length)
}

fn validate_id(value: &str) -> Result<(), HostProtocolError> {
    if value.is_empty() || value.len() > 128 || !value.is_ascii() {
        Err(HostProtocolError::InvalidEnvelope)
    } else {
        Ok(())
    }
}

fn validate_invocation_context_id(value: &str) -> Result<(), HostProtocolError> {
    if value.len() != 48
        || !value.starts_with("ictx_")
        || !value[5..]
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(HostProtocolError::InvalidEnvelope);
    }
    Ok(())
}

fn base64url_unpadded(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let bits = (u32::from(chunk[0]) << 16)
            | (chunk.get(1).copied().map_or(0, u32::from) << 8)
            | chunk.get(2).copied().map_or(0, u32::from);
        output.push(char::from(ALPHABET[((bits >> 18) & 0x3f) as usize]));
        output.push(char::from(ALPHABET[((bits >> 12) & 0x3f) as usize]));
        if chunk.len() > 1 {
            output.push(char::from(ALPHABET[((bits >> 6) & 0x3f) as usize]));
        }
        if chunk.len() > 2 {
            output.push(char::from(ALPHABET[(bits & 0x3f) as usize]));
        }
    }
    output
}

fn validate_value(value: &Value) -> Result<(), HostProtocolError> {
    let bytes = serde_json::to_vec(value).map_err(|_| HostProtocolError::InvalidEnvelope)?;
    if bytes.len() > MAX_WIT_VALUE_BYTES {
        Err(HostProtocolError::FrameLimit)
    } else {
        Ok(())
    }
}

fn validate_error(error: &HostStableError) -> Result<(), HostProtocolError> {
    if !matches!(
        error.code.as_str(),
        "extension_permission_denied"
            | "extension_scope_denied"
            | "extension_instance_stale"
            | "extension_resource_limit"
            | "extension_crashed"
            | "project_not_found"
            | "revision_conflict"
            | "extension_data_quota_exceeded"
            | "cancelled"
            | "internal_error"
    ) || (error.code == "internal_error") != error.diagnostic_id.is_some()
        || error
            .diagnostic_id
            .as_ref()
            .is_some_and(|value| value.len() != 36)
    {
        Err(HostProtocolError::InvalidEnvelope)
    } else {
        Ok(())
    }
}

fn map_read_error(error: &io::Error) -> HostProtocolError {
    if error.kind() == io::ErrorKind::UnexpectedEof {
        HostProtocolError::Truncated
    } else {
        HostProtocolError::Io
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(body: HostMessageBody) -> HostMessage {
        HostMessage {
            protocol_version: 1,
            daemon_epoch: "00000000-0000-4000-8000-000000000001".to_owned(),
            instance_id: "00000000-0000-4000-8000-000000000002".to_owned(),
            lifecycle_generation: 1,
            sequence: 1,
            body,
        }
    }

    #[test]
    fn capability_result_is_bounded_and_exactly_one_of_result_or_error() {
        let valid = message(HostMessageBody::CapabilityResult {
            call_id: "call-1".to_owned(),
            result: Some(serde_json::json!({"value": null})),
            error: None,
        });
        assert!(valid.validate_bounds().is_ok());
        let invalid = message(HostMessageBody::CapabilityResult {
            call_id: "call-1".to_owned(),
            result: Some(Value::Null),
            error: Some(HostStableError {
                code: "extension_instance_stale".to_owned(),
                diagnostic_id: None,
            }),
        });
        assert_eq!(
            invalid.validate_bounds(),
            Err(HostProtocolError::InvalidEnvelope)
        );
        let cancelled = message(HostMessageBody::CapabilityResult {
            call_id: "call-1".to_owned(),
            result: None,
            error: Some(HostStableError {
                code: "cancelled".to_owned(),
                diagnostic_id: None,
            }),
        });
        assert!(cancelled.validate_bounds().is_ok());
    }

    #[test]
    fn malformed_or_oversized_frames_fail_closed() {
        assert_eq!(
            read_host_message(&mut &0_u32.to_le_bytes()[..]),
            Err(HostProtocolError::FrameLimit)
        );
        let prefix = u32::try_from(MAX_HOST_FRAME_BYTES + 1)
            .expect("bounded test")
            .to_le_bytes();
        assert_eq!(
            read_host_message(&mut &prefix[..]),
            Err(HostProtocolError::FrameLimit)
        );
    }

    #[test]
    fn host_envelope_round_trips_and_rejects_unknown_authority_fields() {
        let valid = message(HostMessageBody::CapabilityResult {
            call_id: "call-1".to_owned(),
            result: Some(serde_json::json!({"summary": {"projectId": "fixture"}})),
            error: None,
        });
        let encoded = serde_json::to_vec(&valid).expect("serialize host envelope");
        let decoded: HostMessage =
            serde_json::from_slice(&encoded).expect("deserialize host envelope");
        assert_eq!(decoded, valid);

        let mut with_authority = serde_json::to_value(&valid).expect("serialize value");
        with_authority
            .as_object_mut()
            .expect("host envelope object")
            .insert(
                "principalId".to_owned(),
                Value::String("local-owner".to_owned()),
            );
        assert!(serde_json::from_value::<HostMessage>(with_authority).is_err());

        let mut with_nested_unknown = serde_json::to_value(&valid).expect("serialize value");
        with_nested_unknown
            .as_object_mut()
            .expect("host envelope object")
            .insert("grantRevision".to_owned(), Value::from(99));
        assert!(serde_json::from_value::<HostMessage>(with_nested_unknown).is_err());
    }

    #[test]
    fn invocation_context_ids_are_exact_and_unforgeable_by_shape() {
        let first = invocation_context_id();
        let second = invocation_context_id();
        assert_eq!(first.len(), 48);
        assert!(first.starts_with("ictx_"));
        assert!(
            first[5..]
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        );
        assert_ne!(first, second);
        assert!(validate_invocation_context_id(&first).is_ok());
        assert_eq!(
            validate_invocation_context_id("ictx_invalid"),
            Err(HostProtocolError::InvalidEnvelope)
        );
    }

    #[test]
    fn portable_ui_fingerprint_uses_the_frozen_domain_separator() {
        let action = br#"{"kind":"activate","actionId":"refresh"}"#;
        let fingerprint = portable_ui_action_fingerprint(action);
        let mut expected = Sha256::new();
        expected.update(b"ALCOMD-PORTABLE-UI-ACTION-V1\0");
        expected.update(action);
        let expected: [u8; 32] = expected.finalize().into();
        assert_eq!(fingerprint, expected);
        assert_ne!(fingerprint, portable_ui_action_fingerprint(b"{}"));
    }
}
