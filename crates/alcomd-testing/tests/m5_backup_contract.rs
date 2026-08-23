use serde_json::{Value, json};

const MANIFEST_SCHEMA: &str = include_str!("../../../specs/backups/backup-v1.schema.json");
const PROFILE: &str = include_str!("../../../specs/backups/backup-profile-v1.json");
const RPC_SCHEMA: &str = include_str!("../../../specs/rpc/m5-backup-create.schema.json");
const CLI_CATALOG: &str = include_str!("../../../specs/cli/m5-backup-commands-v1.json");
const ERROR_SCHEMA: &str = include_str!("../../../specs/rpc/rpc-error.schema.json");
const VECTORS: &str = include_str!("../fixtures/m5/backup-contract-vectors.json");
const PERMISSIONS: &str = include_str!("../../../specs/extensions/permissions-v1.md");
const MIGRATION_V6: &str = include_str!("../../alcomd-store/migrations/0006_backup_create.sql");

#[test]
fn native_backup_manifest_and_quota_are_frozen() {
    let schema: Value = serde_json::from_str(MANIFEST_SCHEMA).expect("Backup manifest Schema");
    let profile: Value = serde_json::from_str(PROFILE).expect("Backup profile");
    let vectors: Value = serde_json::from_str(VECTORS).expect("Backup vectors");

    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(schema["properties"]["formatVersion"]["const"], 1);
    assert_eq!(
        schema["properties"]["packagesRequireResolve"]["const"],
        true
    );
    assert_eq!(vectors["manifest"]["formatVersion"], 1);
    assert_eq!(vectors["manifest"]["packagesRequireResolve"], true);
    assert_eq!(profile["archiveRoots"], json!(["backup.json", "project/"]));
    assert_eq!(profile["zip64"], true);
    assert_eq!(profile["compressionMethods"], json!(["stored", "deflate"]));
    assert_eq!(
        profile["compressionModes"],
        json!(["store", "fast", "maximum"])
    );
    assert_eq!(profile["limits"]["archiveBytes"], 68_719_476_736_u64);
    assert_eq!(profile["limits"]["entries"], 500_000);
    assert_eq!(
        profile["limits"]["singleRegularFileBytes"],
        34_359_738_368_u64
    );
    assert_eq!(
        profile["limits"]["totalUncompressedBytes"],
        137_438_953_472_u64
    );
    assert_eq!(profile["limits"]["pathDepth"], 128);
    assert_eq!(profile["limits"]["normalizedPathUtf8Bytes"], 1_024);
    assert_eq!(profile["limits"]["expansionRatio"], 10_000);
}

#[test]
fn exclusion_table_and_vpm_semantics_are_exact() {
    let profile: Value = serde_json::from_str(PROFILE).expect("Backup profile");
    let vectors: Value = serde_json::from_str(VECTORS).expect("Backup vectors");
    assert_eq!(
        profile["pathPolicy"]["rootDirectoriesExcluded"],
        json!(["Logs", "Obj", "Temp"])
    );
    assert_eq!(
        profile["pathPolicy"]["directoriesExcludedAtAnyDepth"],
        json!([".git"])
    );
    assert_eq!(
        profile["pathPolicy"]["rootDirectoryPrefixesExcludedAsciiCaseInsensitive"],
        json!(["Library"])
    );
    assert_eq!(
        profile["pathPolicy"]["directFileExceptions"][0],
        json!({
            "underRootPrefix": "Library",
            "relativeFile": "LastSceneManagerSetup.txt",
            "recursive": false
        })
    );
    for required in [
        "ProjectSettings/ProjectVersion.txt",
        "Packages/vpm-manifest.json",
        "Packages/manifest.json",
    ] {
        assert!(
            profile["pathPolicy"]["alwaysIncludedFiles"]
                .as_array()
                .expect("always included")
                .iter()
                .any(|value| value == required),
            "missing {required}"
        );
    }
    let cases = vectors["pathCases"].as_array().expect("path cases");
    for required in [
        "Logs/Editor.log",
        "Obj/cache.bin",
        "Temp/work",
        "Assets/Nested/.git/config",
        "Library/cache",
        "Library-foo/cache",
        "Library/LastSceneManagerSetup.txt",
        "Library/Sub/LastSceneManagerSetup.txt",
        "MemoryCaptures/capture.bin",
        "UserSettings/EditorUserSettings.asset",
        ".vscode/settings.json",
        ".idea/workspace.xml",
    ] {
        assert!(
            cases.iter().any(|case| case["path"] == required),
            "missing {required}"
        );
    }
    assert_eq!(profile["vpmExclusion"]["networkAccess"], false);
    assert_eq!(profile["vpmExclusion"]["manifestFilesAlwaysIncluded"], true);
    assert_eq!(
        profile["vpmExclusion"]["preserveUnlockedEmbeddedAndUnknownChildren"],
        true
    );
    assert_eq!(profile["vpmExclusion"]["mismatch"], "fail-closed");
}

#[test]
fn rpc_permissions_errors_phases_and_cli_publication_are_narrow() {
    let rpc: Value = serde_json::from_str(RPC_SCHEMA).expect("Backup RPC Schema");
    let cli: Value = serde_json::from_str(CLI_CATALOG).expect("Backup CLI catalog");
    let errors: Value = serde_json::from_str(ERROR_SCHEMA).expect("RPC error Schema");
    let vectors: Value = serde_json::from_str(VECTORS).expect("Backup vectors");

    assert_eq!(
        rpc["$defs"]["capability"]["enum"],
        json!(["backups.read.v1", "backups.create.v1"])
    );
    assert_eq!(
        rpc["$defs"]["methodName"]["enum"],
        json!(["backups.list", "backups.get", "backups.create"])
    );
    assert_eq!(
        rpc["$defs"]["progressPhase"]["enum"],
        vectors["progressPhases"]
    );
    assert_eq!(
        rpc["$defs"]["createResult"]["required"],
        json!(["operationId", "backupId", "replayed"])
    );
    assert_eq!(cli["status"], "implemented");
    assert_eq!(cli["commands"][2]["planApply"], false);
    assert_eq!(
        cli["commands"][2]["forbiddenOptions"],
        json!(["--yes", "--dry-run", "--output"])
    );
    assert!(PERMISSIONS.contains("`backups.manage` 与目标 Project 的 `projects.read` scope"));
    for code in [
        "backup_not_found",
        "backup_unavailable",
        "backup_source_unsafe",
        "backup_archive_limit_exceeded",
        "backup_integrity_mismatch",
        "project_changed_during_backup",
    ] {
        assert!(
            errors["properties"]["code"]["enum"]
                .as_array()
                .expect("error enum")
                .iter()
                .any(|value| value == code),
            "missing {code}"
        );
    }
}

#[test]
fn schema_v6_is_only_the_strict_backup_create_kind() {
    assert!(MIGRATION_V6.starts_with("BEGIN IMMEDIATE;"));
    assert!(MIGRATION_V6.ends_with("COMMIT;\n"));
    assert!(MIGRATION_V6.contains("'backups.create'"));
    for forbidden in [
        "CREATE TABLE backups",
        "backup_plans",
        "backups.restore",
        "workflow",
    ] {
        assert!(!MIGRATION_V6.contains(forbidden), "unexpected {forbidden}");
    }
}
