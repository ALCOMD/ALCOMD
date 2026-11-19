use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::Path;

#[test]
fn provider_abi_is_additive_and_core_controls_write_targets() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let wit_root = root.join("specs/extensions/wit/mcp-tools-v1");
    for name in [
        "types.wit",
        "world.wit",
        "host-data.wit",
        "host-projects.wit",
        "guest-lifecycle.wit",
        "guest-ui.wit",
    ] {
        assert_eq!(
            std::fs::read(root.join("specs/extensions/wit/extension-v1").join(name))
                .expect("frozen ABI"),
            std::fs::read(wit_root.join("deps/extension").join(name))
                .expect("unchanged versioned dependency"),
            "{name}"
        );
    }
    let wit = std::fs::read_to_string(wit_root.join("world.wit")).expect("provider ABI");
    assert!(wit.contains("include alcomd:extension/extension-v1@1.0.0"));
    assert_eq!(wit.matches(": func(").count(), 7);
    assert!(wit.contains("apply-plan: func()"));
    assert!(wit.contains("cancel-operation: func()"));
    for forbidden in ["wasi:", "credential", "bearer", "rpc-call", "local-owner"] {
        assert!(!wit.contains(forbidden), "{forbidden}");
    }
    let declarations: Value = serde_json::from_str(include_str!(
        "../../../specs/extensions/mcp-tools-provider-v1.schema.json"
    ))
    .expect("signed declaration schema");
    assert_eq!(declarations["x-max-serialized-bytes"], 65536);
    assert_eq!(declarations["properties"]["tools"]["maxItems"], 32);
    let rpc: Value = serde_json::from_str(include_str!(
        "../../../specs/rpc/m8-extension-tools-v1.schema.json"
    ))
    .expect("RPC schema");
    assert_eq!(rpc["x-capability"], "mcp.extension-tools.v1");
    assert_eq!(rpc["x-methods"].as_object().expect("methods").len(), 2);
    assert_eq!(rpc["x-max-call-json-bytes"], 65536);
    let m6: Value = serde_json::from_str(include_str!(
        "../../../specs/rpc/m6-extension-runtime.schema.json"
    ))
    .expect("extension grants");
    let permissions = &m6["$defs"]["permissionGrant"]["properties"]["permission"]["enum"];
    assert_eq!(
        permissions,
        &json!([
            "background.run",
            "projects.read",
            "mcp.clients.read",
            "mcp.clients.manage",
            "mcp.connections.read",
            "mcp.requests.read",
            "mcp.subscription-streams.read",
            "mcp.tools.provide",
            "repositories.read",
            "packages.read",
            "packages.manage",
            "operations.read",
            "operations.cancel",
            "unity.sessions.read",
            "local-ipc.connect"
        ])
    );
    assert!(
        permissions
            .as_array()
            .expect("permissions")
            .contains(&json!("mcp.tools.provide"))
    );
}

#[test]
fn actual_provider_component_matches_every_recorded_build_input() {
    fn digest(bytes: impl AsRef<[u8]>) -> String {
        Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let record: Value = serde_json::from_str(include_str!(
        "../fixtures/m8/mcp-tools-provider-v1.provenance.json"
    ))
    .expect("provenance");
    for (path, expected) in record["inputs"].as_object().expect("build inputs") {
        assert_eq!(
            digest(std::fs::read(root.join(path)).expect("input")),
            expected.as_str().expect("input digest"),
            "{path}"
        );
    }
    assert_eq!(
        digest(
            std::fs::read(root.join(record["artifact"].as_str().expect("artifact")))
                .expect("component")
        ),
        record["sha256"].as_str().expect("artifact digest")
    );
}
