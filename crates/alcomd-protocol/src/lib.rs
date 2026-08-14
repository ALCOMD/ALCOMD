//! Public ALCOMD RPC data-transfer objects.
//!
//! Internal domain objects must not leak into this crate without an explicit compatibility review.

use serde::{Deserialize, Serialize};

/// Current ALCOMD RPC major version.
pub const RPC_VERSION: u32 = 1;

/// Stable product family.
pub const PRODUCT_FAMILY: &str = "ALCOMD";

/// Stable technical root name.
pub const TECHNICAL_NAME: &str = "alcomd";

/// Request sent before normal RPC calls.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HelloRequest {
    /// RPC major version requested by the client.
    pub rpc_version: u32,
    /// Client identity and build information.
    pub client: ClientInfo,
    /// Optional client capabilities.
    #[serde(default)]
    pub capabilities: Vec<String>,
}

/// Client identity included in a handshake.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    /// Stable client program name such as `alcomd-cli`.
    pub name: String,
    /// Client semantic version.
    pub version: String,
    /// Per-process instance identifier.
    pub instance_id: String,
}

/// Successful handshake response.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HelloResponse {
    /// Negotiated RPC major version.
    pub rpc_version: u32,
    /// Running daemon version.
    pub daemon_version: String,
    /// Current data schema.
    pub data_schema: u32,
    /// Current config schema.
    pub config_schema: u32,
    /// Current extension API major.
    pub extension_api: u32,
}

impl HelloResponse {
    /// Returns the scaffold protocol response.
    #[must_use]
    pub fn scaffold() -> Self {
        Self {
            rpc_version: RPC_VERSION,
            daemon_version: env!("CARGO_PKG_VERSION").to_owned(),
            data_schema: 1,
            config_schema: 1,
            extension_api: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_response_uses_camel_case_json() {
        let value = serde_json::to_value(HelloResponse::scaffold()).expect("serialize response");
        assert_eq!(value["rpcVersion"], RPC_VERSION);
        assert_eq!(value["dataSchema"], 1);
    }
}
