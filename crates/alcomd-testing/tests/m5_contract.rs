use serde_json::{Value, json};

const CLI_SCHEMA: &str = include_str!("../../../specs/cli/alcomd-cli-v1.schema.json");
const COMMAND_CATALOG: &str = include_str!("../../../specs/cli/command-catalog-v1.json");
const UNITY_SCHEMA: &str = include_str!("../../../specs/rpc/m5-unity.schema.json");
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
