use serde_json::{Value, json};

#[test]
fn extension_source_rpc_and_index_contracts_match_typed_dtos_and_limits() {
    use alcomd_protocol::{
        ExtensionInspectSourceParams, ExtensionPlanInstallFromSourceParams, ExtensionSourcePreview,
        ExtensionSourceVersion,
    };
    let contract: Value = serde_json::from_str(include_str!(
        "../../../specs/rpc/m6-extension-runtime.schema.json"
    ))
    .expect("RPC Schema");
    for method in [
        "extensions.inspectSource",
        "extensions.planInstallFromSource",
    ] {
        assert_eq!(
            contract["x-alcomd-method-permissions"][method],
            json!(["extensions.manage"])
        );
    }
    assert!(
        contract["$defs"]["capability"]["enum"]
            .as_array()
            .expect("caps")
            .contains(&json!("extensions.remote-install.v1"))
    );
    let url = "https://example.invalid/index?b=2&a=1&a=3".to_string();
    for (definition, value) in [
        (
            "inspectSourceParams",
            serde_json::to_value(ExtensionInspectSourceParams { url: url.clone() }).expect("DTO"),
        ),
        (
            "planInstallFromSourceParams",
            serde_json::to_value(ExtensionPlanInstallFromSourceParams {
                url: url.clone(),
                version: "1.2.0".into(),
                expected_source_fingerprint: "a".repeat(64),
            })
            .expect("DTO"),
        ),
        (
            "sourcePreview",
            serde_json::to_value(ExtensionSourcePreview {
                url: url.clone(),
                extension_id: "dev.example.extension".into(),
                display_name: "Example".into(),
                publisher_fingerprint: format!("ed25519-sha256:{}", "a".repeat(64)),
                description: None,
                versions: vec![ExtensionSourceVersion {
                    version: "1.2.0".into(),
                    api_major: 1,
                }],
                source_fingerprint: "b".repeat(64),
            })
            .expect("DTO"),
        ),
    ] {
        let mut schema = contract.clone();
        schema["$ref"] = json!(format!("#/$defs/{definition}"));
        let validator = jsonschema::validator_for(&schema).expect("valid RPC Schema");
        assert!(validator.is_valid(&value), "{definition}: {value}");
        let mut unknown = value.clone();
        unknown["publisherApproval"] = true.into();
        assert!(!validator.is_valid(&unknown), "strict {definition}");
    }
    let index: Value = serde_json::from_str(include_str!(
        "../../../specs/extensions/source-v1.schema.json"
    ))
    .expect("index Schema");
    assert_eq!(index["properties"]["packages"]["minProperties"], 1);
    assert_eq!(index["properties"]["packages"]["maxProperties"], 1);
    let package = &index["properties"]["packages"]["additionalProperties"];
    assert_eq!(package["properties"]["versions"]["maxProperties"], 256);
    let validator = jsonschema::validator_for(&index).expect("valid index Schema");
    let mut value = json!({"format":"alcomd-extension-source-v1","packages":{"dev.example.extension":{"displayName":"Example","publisherFingerprint":format!("ed25519-sha256:{}","a".repeat(64)),"versions":{"1.2.0":{"apiMajor":1,"url":url,"zipSHA256":"b".repeat(64)}}}}});
    assert!(validator.is_valid(&value));
    value["packages"]["dev.example.extension"]["versions"]["1.2.0"]["zipSHA256"] =
        "B".repeat(64).into();
    assert!(!validator.is_valid(&value));
}
