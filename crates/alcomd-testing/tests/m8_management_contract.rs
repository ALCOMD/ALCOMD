use serde_json::{Value, json};
use std::path::Path;

#[test]
fn management_interface_is_additive_and_schema_preserves_bounded_secret_free_shapes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let old = root.join("specs/extensions/wit/extension-v1");
    let new = root.join("specs/extensions/wit/mcp-management-v1");
    for name in [
        "types.wit",
        "world.wit",
        "host-data.wit",
        "host-projects.wit",
        "guest-lifecycle.wit",
        "guest-ui.wit",
    ] {
        assert_eq!(
            std::fs::read(old.join(name)).expect("frozen original"),
            std::fs::read(new.join("deps/extension").join(name))
                .expect("exact versioned dependency")
        );
    }
    let wit = std::fs::read_to_string(new.join("management.wit")).expect("new WIT");
    assert!(wit.contains("include alcomd:extension/extension-v1@1.0.0"));
    assert_eq!(wit.matches(": func(").count(), 6);
    assert!(!wit.contains("issue-credential"));
    assert!(!wit.contains("wasi:"));
    assert!(wit.contains("can-manage: bool"));
    let schema: Value = serde_json::from_str(include_str!(
        "../../../specs/extensions/mcp-management-v1.schema.json"
    ))
    .expect("schema");
    assert_eq!(schema["x-max-serialized-bytes"], 65536);
    assert_eq!(
        schema["$defs"]["clientPage"]["properties"]["canManage"]["type"],
        "boolean"
    );
    assert_eq!(
        schema["$defs"]["request"]["oneOf"]
            .as_array()
            .expect("methods")
            .len(),
        6
    );
    assert_eq!(
        schema["$defs"]["grantPage"]["properties"]["items"]["maxItems"],
        128
    );
    assert_eq!(
        schema["$defs"]["client"]["properties"]["grantCount"]["maximum"],
        4096
    );
    for field in [
        "credential",
        "credentialId",
        "tokenHash",
        "principalId",
        "rootPath",
    ] {
        assert!(schema["$defs"]["client"]["properties"].get(field).is_none());
    }
    let m6: Value = serde_json::from_str(include_str!(
        "../../../specs/rpc/m6-extension-runtime.schema.json"
    ))
    .expect("existing grant schema");
    assert_eq!(
        m6["$defs"]["permissionGrant"]["properties"]["permission"]["enum"],
        json!([
            "background.run",
            "projects.read",
            "mcp.clients.read",
            "mcp.clients.manage"
        ])
    );
    assert_eq!(
        m6["$defs"]["permissionGrant"]["properties"]["resourceKind"]["enum"],
        json!(["Extension", "Project", "Global"])
    );
    assert_eq!(
        m6["$defs"]["permissionGrant"]["properties"]["resourceId"]["minLength"],
        1
    );
}

#[test]
fn actual_component_fixture_matches_its_build_inputs() {
    use sha2::{Digest, Sha256};
    fn digest(bytes: impl AsRef<[u8]>) -> String {
        Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let record: Value = serde_json::from_str(include_str!(
        "../fixtures/m8/mcp-management-v1.provenance.json"
    ))
    .expect("provenance");
    for (path, expected) in record["inputs"].as_object().expect("build inputs") {
        let actual = digest(std::fs::read(root.join(path)).expect("input"));
        assert_eq!(actual, expected.as_str().expect("input hash"), "{path}");
    }
    let bytes = std::fs::read(root.join(record["artifact"].as_str().expect("artifact")))
        .expect("actual component");
    assert_eq!(
        digest(bytes),
        record["sha256"].as_str().expect("binary hash")
    );
}
