use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

const MANIFEST_SCHEMA: &str = include_str!("../../../specs/extensions/manifest-v1.schema.json");
const PACKAGE_PROFILE: &str = include_str!("../../../specs/extensions/package-profile-v1.json");
const SIGNATURE_SCHEMA: &str = include_str!("../../../specs/extensions/signature-v1.schema.json");
const ABI_MATRIX: &str = include_str!("../../../specs/extensions/abi-compatibility-v1.json");
const RUNTIME_LIMITS: &str = include_str!("../../../specs/extensions/runtime-limits-v1.json");
const UI_SCHEMA: &str = include_str!("../../../specs/extensions/ui-bridge-v1.schema.json");
const RPC_SCHEMA: &str = include_str!("../../../specs/rpc/m6-extension-runtime.schema.json");
const HELLO_SCHEMA: &str = include_str!("../../../specs/rpc/system-hello.response.schema.json");
const ERROR_SCHEMA: &str = include_str!("../../../specs/rpc/rpc-error.schema.json");
const STATE_CONTRACT: &str =
    include_str!("../../../specs/storage/state-v8-migration.contract.json");
const PERMISSIONS: &str = include_str!("../../../specs/extensions/permissions-v1.md");
const LIFECYCLE: &str = include_str!("../../../specs/extensions/lifecycle-v1.md");
const HOST_PROTOCOL: &str = include_str!("../../../specs/extensions/host-protocol-v1.md");
const ABI_SPEC: &str = include_str!("../../../specs/extensions/abi-v1.md");
const TYPES_WIT: &str = include_str!("../../../specs/extensions/wit/extension-v1/types.wit");
const PROJECTS_WIT: &str =
    include_str!("../../../specs/extensions/wit/extension-v1/host-projects.wit");
const DATA_WIT: &str = include_str!("../../../specs/extensions/wit/extension-v1/host-data.wit");
const LIFECYCLE_WIT: &str =
    include_str!("../../../specs/extensions/wit/extension-v1/guest-lifecycle.wit");
const WORLD_WIT: &str = include_str!("../../../specs/extensions/wit/extension-v1/world.wit");
const VECTORS: &str = include_str!("../fixtures/m6/extension-contract-vectors.json");
const HOSTILE: &str = include_str!("../fixtures/m6/hostile-package-vectors.json");
const COMPATIBILITY: &str = include_str!("../fixtures/m6/old-guest-new-host-v1.json");
const UI_VECTORS: &str = include_str!("../fixtures/m6/ui-bridge-vectors.json");

#[test]
fn manifest_package_signature_and_digest_are_frozen() {
    let manifest: Value = serde_json::from_str(MANIFEST_SCHEMA).expect("Manifest Schema");
    let profile: Value = serde_json::from_str(PACKAGE_PROFILE).expect("package profile");
    let signature: Value = serde_json::from_str(SIGNATURE_SCHEMA).expect("signature Schema");
    let vectors: Value = serde_json::from_str(VECTORS).expect("contract vectors");

    assert_eq!(manifest["additionalProperties"], false);
    assert_eq!(manifest["properties"]["schema"]["const"], 1);
    assert_eq!(manifest["properties"]["api"]["const"], 1);
    assert_eq!(
        manifest["properties"]["entrypoints"]["properties"]["background_component"]["const"],
        "component/extension.wasm"
    );
    assert_eq!(profile["container"], "zip");
    assert_eq!(profile["zip64"], false);
    assert_eq!(profile["compressionMethods"], json!(["stored", "deflate"]));
    assert_eq!(profile["limits"]["archiveBytes"], 67_108_864_u64);
    assert_eq!(profile["limits"]["entries"], 1_024);
    assert_eq!(profile["limits"]["totalUncompressedBytes"], 134_217_728_u64);
    assert_eq!(profile["limits"]["pathDepth"], 16);
    assert_eq!(profile["limits"]["expansionRatio"], 100);
    assert_eq!(signature["properties"]["algorithm"]["const"], "ed25519");
    assert_eq!(
        signature["properties"]["publisherFingerprint"]["pattern"],
        "^ed25519-sha256:[0-9a-f]{64}$"
    );
    assert!(manifest["properties"].get("first_party").is_none());

    let digest = &vectors["canonicalDigest"];
    let mut entries = digest["entries"]
        .as_array()
        .expect("digest entries")
        .clone();
    entries.sort_by(|left, right| {
        left["path"]
            .as_str()
            .expect("left path")
            .as_bytes()
            .cmp(right["path"].as_str().expect("right path").as_bytes())
    });
    let mut hasher = Sha256::new();
    hasher.update(b"ALCOMD-EXT-CONTENT-SHA256-V1\0");
    for entry in entries {
        let path = entry["path"].as_str().expect("entry path").as_bytes();
        let content = if let Some(value) = entry.get("contentUtf8") {
            value.as_str().expect("UTF-8 content").as_bytes().to_vec()
        } else {
            decode_hex(entry["contentHex"].as_str().expect("hex content"))
        };
        hasher.update(
            u32::try_from(path.len())
                .expect("path length")
                .to_le_bytes(),
        );
        hasher.update(path);
        hasher.update(
            u64::try_from(content.len())
                .expect("content length")
                .to_le_bytes(),
        );
        hasher.update(content);
    }
    assert_eq!(
        hex(&hasher.finalize()),
        digest["expectedSha256"].as_str().expect("expected digest")
    );
    assert_eq!(vectors["signatureGolden"]["algorithm"], "ed25519");
    assert_eq!(
        vectors["signatureGolden"]["packageDigest"],
        digest["expectedSha256"]
    );
    assert_eq!(
        vectors["signatureGolden"]["publicKey"]
            .as_str()
            .expect("public key")
            .len(),
        64
    );
    assert_eq!(
        vectors["signatureGolden"]["signature"]
            .as_str()
            .expect("signature")
            .len(),
        128
    );
}

#[test]
fn hostile_archive_profile_has_each_frozen_negative_class() {
    let hostile: Value = serde_json::from_str(HOSTILE).expect("hostile vectors");
    let names = hostile["rejected"]
        .as_array()
        .expect("rejected cases")
        .iter()
        .map(|case| case["name"].as_str().expect("case name"))
        .collect::<Vec<_>>();
    for required in [
        "traversal",
        "absolute",
        "windows-device",
        "unc",
        "symlink",
        "reparse",
        "duplicate",
        "case-collision",
        "unicode-collision",
        "file-directory-collision",
        "special-entry",
        "unsupported-codec",
        "encrypted",
        "too-many-entries",
        "too-deep",
        "path-too-long",
        "ratio-too-large",
    ] {
        assert!(names.contains(&required), "missing hostile case {required}");
    }
}

#[test]
fn exact_wit_world_has_no_ambient_wasi_and_shape_changes_are_breaking() {
    let matrix: Value = serde_json::from_str(ABI_MATRIX).expect("ABI matrix");
    let fixture: Value = serde_json::from_str(COMPATIBILITY).expect("compatibility fixture");
    assert_eq!(matrix["abiMajor"], 1);
    assert_eq!(matrix["world"], "alcomd:extension/extension-v1@1.0.0");
    assert_eq!(matrix["ambientWasiImports"], json!([]));
    assert_eq!(fixture["newHost"]["result"], "compatible");
    assert_eq!(fixture["oldGuest"]["requiredImports"], matrix["imports"]);
    for breaking in [
        "record-field-add",
        "record-field-remove",
        "record-field-change",
        "variant-or-enum-semantic-change",
        "parameter-type-change",
        "result-type-change",
        "existing-required-import-change",
    ] {
        assert!(
            matrix["breakingChanges"]
                .as_array()
                .expect("breaking changes")
                .iter()
                .any(|value| value == breaking),
            "missing breaking rule {breaking}"
        );
    }
    assert!(WORLD_WIT.contains("world extension-v1"));
    assert!(WORLD_WIT.contains("import host-projects;"));
    assert!(WORLD_WIT.contains("import host-data;"));
    assert!(WORLD_WIT.contains("export guest-lifecycle;"));
    for wit in [TYPES_WIT, PROJECTS_WIT, DATA_WIT, LIFECYCLE_WIT, WORLD_WIT] {
        assert!(wit.contains("package alcomd:extension@1.0.0;"));
        assert!(!wit.contains("wasi:"));
    }
    for denied in [
        "inherit_env",
        "inherit_args",
        "inherit_stdio",
        "inherit_network",
    ] {
        assert!(ABI_SPEC.contains(denied), "missing ambient deny {denied}");
    }
}

#[test]
fn lease_revocation_data_and_runtime_limits_are_exact() {
    let limits: Value = serde_json::from_str(RUNTIME_LIMITS).expect("runtime limits");
    let vectors: Value = serde_json::from_str(VECTORS).expect("contract vectors");
    assert_eq!(limits["hostProcessesPerEnabledExtensionId"], 1);
    assert_eq!(limits["activeComponentInstancesPerHost"], 1);
    assert_eq!(limits["linearMemoryBytes"], 67_108_864_u64);
    assert_eq!(limits["tableElements"], 10_000);
    assert_eq!(limits["concurrentGuestCalls"], 1);
    assert_eq!(limits["fuelPerGuestCall"], 10_000_000);
    assert_eq!(limits["epochTickMs"], 10);
    assert_eq!(limits["wallTimeoutMs"], 2_000);
    assert_eq!(limits["backgroundLeaseMs"], 60_000);
    assert_eq!(limits["crashLoop"]["threshold"], 3);
    assert_eq!(limits["crashLoop"]["windowMs"], 300_000);
    assert_eq!(limits["instructionPolicy"], "fuel-and-epoch");
    assert_eq!(
        vectors["lifecycle"]["legalComposite"],
        json!({"desired": "enabled", "quarantine": "quarantined", "runtime": "stopped"})
    );
    assert_eq!(vectors["lifecycle"]["crashEvidencePerExtension"], 16);
    assert_eq!(
        vectors["retainedData"]["owner"],
        json!(["ExtensionId", "publisherFingerprint"])
    );
    assert_eq!(vectors["retainedData"]["grantsRestoredOnReinstall"], false);
    assert!(LIFECYCLE.contains("grant revision durable update 是 revoke linearization point"));
    assert!(LIFECYCLE.contains("一个独立 `alcomd-extension-host` OS process"));
    assert!(HOST_PROTOCOL.contains("dedicated piped stdin/stdout"));
    assert!(HOST_PROTOCOL.contains("1-524,288 bytes"));
    assert!(HOST_PROTOCOL.contains("不得携带或覆盖 PrincipalId、ExtensionId"));
    assert!(PERMISSIONS.contains("specific ProjectId"));
    assert!(PERMISSIONS.contains("`mcp.sessions.read` 已由 A-023 拒绝"));
}

#[test]
fn state_rpc_errors_and_publication_boundary_are_frozen() {
    let state: Value = serde_json::from_str(STATE_CONTRACT).expect("state contract");
    let rpc: Value = serde_json::from_str(RPC_SCHEMA).expect("RPC contract");
    let hello: Value = serde_json::from_str(HELLO_SCHEMA).expect("hello Schema");
    let errors: Value = serde_json::from_str(ERROR_SCHEMA).expect("error Schema");
    assert_eq!(state["from"], 7);
    assert_eq!(state["to"], 8);
    assert_eq!(
        state["productionMigration"],
        "crates/alcomd-store/migrations/0008_extension_runtime.sql"
    );
    assert_eq!(state["tablesAdded"].as_array().expect("tables").len(), 8);
    assert_eq!(
        state["operationKindsAdded"],
        json!(["extensions.install", "extensions.uninstall"])
    );
    assert_eq!(
        state["desiredStates"],
        json!(["installed_disabled", "enabled", "uninstalling"])
    );
    assert_eq!(state["quarantineStates"], json!(["clear", "quarantined"]));
    assert_eq!(state["crashEvidencePerExtension"], 16);
    assert_eq!(state["extensionApiMajor"], 1);
    assert_eq!(state["packageProfileVersion"], 1);
    assert_eq!(
        state["planAuthorityFields"],
        json!([
            "extension_id",
            "version",
            "api_major",
            "profile_version",
            "package_digest",
            "manifest_digest",
            "component_digest",
            "publisher_fingerprint",
            "permissions",
            "interfaces",
            "source_evidence"
        ])
    );
    assert_eq!(
        state["installSourceKinds"],
        json!(["local_owner_selected", "first_party_packaged"])
    );
    assert_eq!(
        rpc["$defs"]["capability"]["enum"],
        json!(["extensions.lifecycle.v1", "extensions.permissions.v1"])
    );
    assert_eq!(
        rpc["$defs"]["methodName"]["enum"]
            .as_array()
            .expect("methods")
            .len(),
        10
    );
    for (method, result) in [
        ("extensions.list", "#/$defs/listResult"),
        ("extensions.get", "#/$defs/extensionResult"),
        ("extensions.planInstall", "#/$defs/planResult"),
        ("extensions.applyInstall", "#/$defs/operationResult"),
        ("extensions.enable", "#/$defs/extensionResult"),
        ("extensions.disable", "#/$defs/extensionResult"),
        ("extensions.planUninstall", "#/$defs/planResult"),
        ("extensions.applyUninstall", "#/$defs/operationResult"),
        ("extensions.setGrant", "#/$defs/grantResult"),
        ("extensions.revokeGrant", "#/$defs/grantResult"),
    ] {
        assert_eq!(rpc["x-alcomd-method-results"][method], result, "{method}");
    }
    assert_eq!(
        rpc["x-alcomd-list-pagination"]["order"],
        "extensionId ascending"
    );
    assert_eq!(
        rpc["$defs"]["planRecord"]["properties"]["apiMajor"]["const"],
        1
    );
    assert_eq!(
        rpc["$defs"]["planRecord"]["properties"]["profileVersion"]["const"],
        1
    );
    assert_eq!(
        rpc["$defs"]["listResult"]["properties"]["extensions"]["maxItems"],
        1_000
    );
    assert_eq!(
        rpc["$defs"]["listResult"]["properties"]["nextCursor"]["maxLength"],
        1_024
    );
    assert!(HOST_PROTOCOL.contains(
        "daemon -> Host：`bootstrap`、`invoke-export`、`cancel-call`、`capability-result`"
    ));
    assert!(HOST_PROTOCOL.contains("只使用 `callId`"));
    assert!(HOST_PROTOCOL.contains("必须恰好包含 bounded `result` 或 stable `error` 之一"));
    assert!(HOST_PROTOCOL.contains("两种 ID 不得互换"));
    assert_eq!(
        hello["properties"]["result"]["properties"]["extensionApi"]["properties"]["world"]["const"],
        "alcomd:extension/extension-v1@1.0.0"
    );
    for code in [
        "extension_manifest_invalid",
        "extension_package_untrusted",
        "extension_publisher_confirmation_required",
        "extension_signature_invalid",
        "extension_permission_denied",
        "extension_scope_denied",
        "extension_api_unsupported",
        "extension_instance_stale",
        "extension_resource_limit",
        "extension_quarantined",
        "extension_plan_stale",
        "extension_data_quota_exceeded",
        "extension_data_owner_mismatch",
        "extension_recovery_required",
    ] {
        assert!(
            errors["properties"]["code"]["enum"]
                .as_array()
                .expect("error enum")
                .iter()
                .any(|value| value == code),
            "missing error {code}"
        );
    }
    assert_production_does_not_advertise_m6();
}

#[test]
fn ui_bridge_is_headless_only_and_bounded() {
    let schema: Value = serde_json::from_str(UI_SCHEMA).expect("UI Bridge Schema");
    let vectors: Value = serde_json::from_str(UI_VECTORS).expect("UI vectors");
    assert_eq!(schema["oneOf"].as_array().expect("envelopes").len(), 3);
    assert_eq!(vectors["contribution"], "headless/test contribution");
    assert_eq!(vectors["publishedProductPlacements"], json!([]));
    assert_eq!(vectors["illustrativeUrlMappingIsPublicContract"], false);
    assert_eq!(
        vectors["logicalOrigin"]["extensionId"],
        "dev.example.project-summary"
    );
    assert_eq!(vectors["validRequest"]["bridgeVersion"], 1);
    for negative in [
        "origin-spoof",
        "sequence-replay",
        "request-id-collision",
        "oversized-message",
        "rate-flood",
        "dom-access",
        "tauri-ipc",
        "private-channel",
    ] {
        assert!(
            vectors["negative"]
                .as_array()
                .expect("negative vectors")
                .iter()
                .any(|value| value == negative),
            "missing UI abuse vector {negative}"
        );
    }
}

fn decode_hex(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0);
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).expect("hex UTF-8");
            u8::from_str_radix(text, 16).expect("hex byte")
        })
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[(byte >> 4) as usize]));
        output.push(char::from(HEX[(byte & 0x0f) as usize]));
    }
    output
}

fn assert_production_does_not_advertise_m6() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("Workspace root");
    for relative in ["apps", "crates", "packages"] {
        scan_source_tree(&root.join(relative));
    }
}

fn scan_source_tree(path: &Path) {
    if path.is_dir() {
        for entry in fs::read_dir(path).expect("scan source directory") {
            let child = entry.expect("source entry").path();
            let name = child
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if !matches!(name, "target" | "node_modules" | "dist") {
                scan_source_tree(&child);
            }
        }
        return;
    }
    if !matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("rs" | "ts" | "tsx" | "js" | "jsx")
    ) {
        return;
    }
    let relative = path.to_string_lossy();
    if relative.ends_with("m6_contract.rs") {
        return;
    }
    let content = fs::read_to_string(path).expect("read production source");
    for unpublished in [
        "extensions.lifecycle.v1",
        "extensions.permissions.v1",
        "extensions.planInstall",
        "alcomd:extension/extension-v1@1.0.0",
    ] {
        assert!(
            !content.contains(unpublished),
            "M6 contract advertised in production source: {} contains {unpublished}",
            path.display()
        );
    }
}
