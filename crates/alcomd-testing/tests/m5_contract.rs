use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const CLI_SCHEMA: &str = include_str!("../../../specs/cli/alcomd-cli-v1.schema.json");
const COMMAND_CATALOG: &str = include_str!("../../../specs/cli/command-catalog-v1.json");
const UNITY_SCHEMA: &str = include_str!("../../../specs/rpc/m5-unity.schema.json");
const TEMPLATE_SCHEMA: &str =
    include_str!("../../../specs/templates/template-bundle-v1.schema.json");
const TEMPLATE_RPC_SCHEMA: &str = include_str!("../../../specs/rpc/m5-template.schema.json");
const TEMPLATE_CLI: &str = include_str!("../../../specs/cli/m5-template-commands-v1.json");
const TEMPLATE_FIXTURE: &str = include_str!("../fixtures/m5/template-native-v1.json");
const TEMPLATE_VECTORS: &str = include_str!("../fixtures/m5/template-contract-vectors.json");
const BUILTIN_INVENTORY: &str = include_str!("../../../specs/templates/builtin-inventory-v1.toml");
const BLANK_SCAFFOLD: &[u8] =
    include_bytes!("../../../specs/templates/builtin-scaffolds/blank-v1.json");
const AVATARS_SCAFFOLD: &[u8] =
    include_bytes!("../../../specs/templates/builtin-scaffolds/avatars-v1.json");
const WORLDS_SCAFFOLD: &[u8] =
    include_bytes!("../../../specs/templates/builtin-scaffolds/worlds-v1.json");
const PROTOCOL_SOURCE: &str = include_str!("../../alcomd-protocol/src/lib.rs");
const MIGRATION_V4: &str = include_str!("../../alcomd-store/migrations/0004_local_workflows.sql");
const PERMISSIONS: &str = include_str!("../../../specs/extensions/permissions-v1.md");
const STATE_V4: &str = include_str!("../../../specs/storage/state-v4.md");

#[test]
fn cli_machine_contract_freezes_modes_options_records_and_exit_codes() {
    let schema: Value = serde_json::from_str(CLI_SCHEMA).expect("CLI Schema JSON");
    let definitions = schema["$defs"].as_object().expect("CLI definitions");
    assert_eq!(
        definitions["outputMode"]["enum"],
        json!(["human", "json", "ndjson"])
    );
    assert_eq!(definitions["exitCode"]["enum"], json!([0, 1, 2, 3, 130]));
    assert_eq!(
        definitions["globalOption"]["enum"],
        json!([
            "--json",
            "--ndjson",
            "--quiet",
            "--yes",
            "--dry-run",
            "--no-wait",
            "--no-start-daemon"
        ])
    );
    assert_eq!(
        definitions["ndjsonRecord"]["properties"]["type"]["enum"],
        json!(["operation", "progress", "event", "result", "error"])
    );
    assert_eq!(
        definitions["resultDocument"]["required"],
        json!(["type", "command", "result"])
    );
    assert_eq!(
        definitions["errorDocument"]["required"],
        json!(["type", "error"])
    );
}

#[test]
fn command_catalog_has_unique_names_and_freezes_core_aliases() {
    let catalog: Value = serde_json::from_str(COMMAND_CATALOG).expect("command catalog JSON");
    assert_eq!(catalog["contractVersion"], 1);
    assert_eq!(
        catalog["publicationRule"],
        "publish-only-when-backend-capability-is-implemented"
    );
    let groups = catalog["groups"].as_array().expect("command groups");
    let mut group_names = std::collections::BTreeSet::new();
    for group in groups {
        let name = group["name"].as_str().expect("group name");
        assert!(group_names.insert(name), "duplicate group {name}");
        let mut names = std::collections::BTreeSet::new();
        for command in group["commands"].as_array().expect("commands") {
            let command_name = command["name"].as_str().expect("command name");
            assert!(
                names.insert(command_name),
                "duplicate command {name} {command_name}"
            );
        }
    }
    let repository = groups
        .iter()
        .find(|group| group["name"] == "repository")
        .expect("repository group");
    assert_eq!(repository["aliases"], json!(["repo"]));
    let package = groups
        .iter()
        .find(|group| group["name"] == "package")
        .expect("package group");
    assert!(
        package["commands"]
            .as_array()
            .expect("package commands")
            .iter()
            .any(|command| command["name"] == "install" && command["aliases"] == json!(["i"]))
    );
    assert!(
        package["commands"]
            .as_array()
            .expect("package commands")
            .iter()
            .any(|command| command["name"] == "remove" && command["aliases"] == json!(["rm"]))
    );
}

#[test]
fn unity_contract_keeps_permissions_capabilities_and_writer_states_narrow() {
    let schema: Value = serde_json::from_str(UNITY_SCHEMA).expect("Unity Schema JSON");
    assert_eq!(
        schema["$defs"]["capability"]["enum"],
        json!(["unity.read.v1", "unity.manage.v1", "unity.launch.v1"])
    );
    assert_eq!(
        schema["$defs"]["writerStateKind"]["enum"],
        json!([
            "running_confirmed",
            "running_suspected",
            "not_observed",
            "unknown"
        ])
    );
    for permission in [
        "unity.read",
        "unity.manage",
        "unity.launch",
        "projects.create",
    ] {
        assert!(
            PERMISSIONS.lines().any(|line| line.trim() == permission),
            "{permission}"
        );
    }
    assert!(STATE_V4.contains("广告 `dataSchema: 4`"));
    assert!(STATE_V4.contains("不增加通用 settings/property/value"));
}

#[test]
fn native_template_bundle_freezes_manifest_layout_and_independent_quota() {
    let schema: Value = serde_json::from_str(TEMPLATE_SCHEMA).expect("Template manifest Schema");
    let fixture: Value = serde_json::from_str(TEMPLATE_FIXTURE).expect("native Template fixture");
    let vectors: Value = serde_json::from_str(TEMPLATE_VECTORS).expect("Template vectors");

    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(schema["properties"]["formatVersion"]["const"], 1);
    assert_eq!(fixture["formatVersion"], 1);
    assert_eq!(fixture["payload"]["root"], "payload/");
    assert_eq!(fixture["provenance"]["createdBy"], "authored");
    assert_eq!(schema["x-alcomd-archive-quota"], vectors["archiveQuota"]);
    assert_eq!(
        schema["$defs"]["additionalResource"]["additionalProperties"],
        false
    );
    assert_eq!(
        schema["$defs"]["payload"]["properties"]["entryCount"]["maximum"],
        100_000
    );
    assert_eq!(
        schema["$defs"]["payload"]["properties"]["totalBytes"]["maximum"],
        8_589_934_592_u64
    );
    assert_eq!(
        vectors["archiveQuota"]["compressedBytes"],
        2_147_483_648_u64
    );
    assert_eq!(vectors["archiveQuota"]["entryCount"], 100_000);
    assert_eq!(
        vectors["archiveQuota"]["singleEntryBytes"],
        2_147_483_648_u64
    );
    assert_eq!(
        vectors["archiveQuota"]["totalUncompressedBytes"],
        8_589_934_592_u64
    );
    assert_eq!(vectors["archiveQuota"]["pathDepth"], 64);
    assert_eq!(vectors["archiveQuota"]["pathBytes"], 1_024);
    assert_eq!(vectors["archiveQuota"]["expansionRatio"], 1_000);

    let rejection_cases = vectors["bundleRejections"]
        .as_array()
        .expect("bundle rejection vectors");
    for required in [
        "unknown-manifest-field",
        "bundle-digest-mismatch",
        "case-collision",
        "unicode-nfc-collision",
        "symlink-entry",
        "special-file-entry",
        "quota-exceeded",
    ] {
        assert!(
            rejection_cases.iter().any(|case| case["case"] == required),
            "missing {required}"
        );
    }
}

#[test]
fn builtin_inventory_is_stable_native_and_contains_no_sdk_bytes() {
    assert_eq!(BUILTIN_INVENTORY.matches("[[template]]").count(), 3);
    for family in ["blank", "vrchat-avatars", "vrchat-worlds"] {
        assert!(BUILTIN_INVENTORY.contains(&format!("family = \"{family}\"")));
    }
    for suffix in ["b001", "b002", "b003"] {
        assert!(BUILTIN_INVENTORY.contains(suffix));
    }
    assert!(BUILTIN_INVENTORY.contains("inventory_license = \"AGPL-3.0-only\""));
    assert!(BUILTIN_INVENTORY.contains("embedded_third_party_assets = false"));
    assert!(BUILTIN_INVENTORY.contains("no SDK bytes are embedded"));
    assert!(BUILTIN_INVENTORY.contains("package_id = \"com.vrchat.avatars\""));
    assert!(BUILTIN_INVENTORY.contains("package_id = \"com.vrchat.worlds\""));
    assert_eq!(
        BUILTIN_INVENTORY
            .matches("version_range = \">=3.0.0\"")
            .count(),
        2
    );
    assert_eq!(
        BUILTIN_INVENTORY.matches("payload_tree_sha256 =").count(),
        3
    );
    assert_eq!(
        BUILTIN_INVENTORY.matches("payload_source_sha256 =").count(),
        3
    );
    for (source, source_digest) in [
        (
            BLANK_SCAFFOLD,
            "b63d2e689363103003f59b5e2859ceb7ee16f49b0693f99d8aae96789ee4005f",
        ),
        (
            AVATARS_SCAFFOLD,
            "423a9f5fac566d22067ce73d36c8723b5ecea8b00c51ef4444d9a3ed5519dee5",
        ),
        (
            WORLDS_SCAFFOLD,
            "f9dabcb62e40cbaee79e71c4772cf9cc127f7e01624403e40fe950dd0b4a8d58",
        ),
    ] {
        assert_eq!(hex_digest(source), source_digest);
        assert_eq!(
            scaffold_tree_digest(source),
            "827604d760a376315abfa5a8f2fdbd633c145e991b6d8cb97c49d756f325d1a4"
        );
        assert!(BUILTIN_INVENTORY.contains(source_digest));
    }
}

#[test]
fn template_rpc_permissions_and_planned_cli_do_not_publish_production_capability() {
    let schema: Value = serde_json::from_str(TEMPLATE_RPC_SCHEMA).expect("Template RPC Schema");
    let cli: Value = serde_json::from_str(TEMPLATE_CLI).expect("Template planned CLI");
    let methods = schema["$defs"]["methodName"]["enum"]
        .as_array()
        .expect("Template methods");
    assert_eq!(methods.len(), 12);
    assert_eq!(
        schema["$defs"]["capability"]["enum"],
        json!([
            "templates.read.v1",
            "templates.manage.v1",
            "templates.create-project.v1"
        ])
    );
    assert_eq!(cli["status"], "planned-not-published");
    assert_eq!(
        cli["commands"].as_array().expect("planned commands").len(),
        9
    );
    assert_eq!(
        schema["x-alcomd-method-permissions"]["templates.applyCreateProject"],
        json!([
            "templates.read",
            "projects.create",
            "packages.read",
            "repositories.read",
            "packages.manage"
        ])
    );
    assert!(!PROTOCOL_SOURCE.contains("templates.read.v1"));
    assert!(!COMMAND_CATALOG.contains("\"name\": \"template\""));
    for permission in [
        "templates.read",
        "templates.manage",
        "projects.create",
        "packages.read",
        "packages.manage",
        "repositories.read",
    ] {
        assert!(PERMISSIONS.contains(permission), "{permission}");
    }
}

#[test]
fn template_contract_fits_state_v4_without_migration_change() {
    assert!(MIGRATION_V4.contains("source_kind IN ('builtin', 'user')"));
    assert!(!MIGRATION_V4.contains("'imported'"));
    assert!(!MIGRATION_V4.contains("'derived'"));
    assert!(MIGRATION_V4.contains("length(manifest_json) <= 1048576"));
    assert!(MIGRATION_V4.contains("length(payload_sha256) = 32"));
    assert!(STATE_V4.contains("不修改 migration"));
    assert!(STATE_V4.contains("sha256:<64-lower-hex>"));
    assert!(STATE_V4.contains("不增加 Schema v5"));
}

fn hex_digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn scaffold_tree_digest(source: &[u8]) -> String {
    let document: Value = serde_json::from_slice(source).expect("scaffold descriptor JSON");
    let mut files = document["files"]
        .as_array()
        .expect("scaffold files")
        .iter()
        .map(|file| {
            (
                file["path"].as_str().expect("path"),
                file["utf8"].as_str().expect("UTF-8 content"),
            )
        })
        .collect::<Vec<_>>();
    files.sort_by_key(|(path, _)| *path);
    let mut digest = Sha256::new();
    for (path, content) in files {
        let path = path.as_bytes();
        let content = content.as_bytes();
        digest.update(
            u32::try_from(path.len())
                .expect("bounded path")
                .to_le_bytes(),
        );
        digest.update(path);
        digest.update(
            u64::try_from(content.len())
                .expect("bounded content")
                .to_le_bytes(),
        );
        digest.update(content);
    }
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
