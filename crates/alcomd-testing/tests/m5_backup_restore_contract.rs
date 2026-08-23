use serde_json::{Value, json};

const PLAN_SCHEMA: &str = include_str!("../../../specs/backups/backup-restore-plan-v1.schema.json");
const RPC_SCHEMA: &str = include_str!("../../../specs/rpc/m5-backup-restore.schema.json");
const CLI_CATALOG: &str = include_str!("../../../specs/cli/m5-backup-commands-v1.json");
const ERROR_SCHEMA: &str = include_str!("../../../specs/rpc/rpc-error.schema.json");
const PROFILE: &str = include_str!("../../../specs/backups/backup-profile-v1.json");
const PERMISSIONS: &str = include_str!("../../../specs/extensions/permissions-v1.md");
const STATE_V7: &str = include_str!("../../../specs/storage/state-v7.md");
const VECTORS: &str = include_str!("../fixtures/m5/backup-restore-contract-vectors.json");
const MIGRATION_V7: &str = include_str!("../../alcomd-store/migrations/0007_backup_restore.sql");

#[test]
fn restore_plan_schema_freezes_exact_authority_without_archive_paths() {
    let schema: Value = serde_json::from_str(PLAN_SCHEMA).expect("Restore Plan Schema");
    let required = schema["required"].as_array().expect("required fields");
    for field in [
        "planId",
        "backupId",
        "preallocatedProjectId",
        "backupArchiveSha256",
        "backupFileIdentity",
        "backupByteSize",
        "backupFormatVersion",
        "backupManifestFingerprint",
        "excludeVpmPackages",
        "excludedPackages",
        "targetParentPath",
        "targetParentIdentity",
        "targetLeaf",
        "targetMustBeAbsent",
        "expectedUnityProject",
        "packagesRequireResolve",
        "planFingerprint",
    ] {
        assert!(
            required.iter().any(|candidate| candidate == field),
            "{field}"
        );
    }
    assert_eq!(schema["properties"]["version"]["const"], 1);
    assert_eq!(schema["properties"]["backupFormatVersion"]["const"], 1);
    assert_eq!(schema["properties"]["targetMustBeAbsent"]["const"], true);
    for forbidden in [
        "archivePath",
        "archiveLocator",
        "stagingPath",
        "databaseLocator",
        "journal",
    ] {
        assert!(schema["properties"].get(forbidden).is_none(), "{forbidden}");
    }
}

#[test]
fn restore_rpc_permissions_errors_and_published_cli_are_narrow() {
    let rpc: Value = serde_json::from_str(RPC_SCHEMA).expect("Restore RPC Schema");
    let cli: Value = serde_json::from_str(CLI_CATALOG).expect("Backup CLI catalog");
    let errors: Value = serde_json::from_str(ERROR_SCHEMA).expect("RPC error Schema");

    assert_eq!(rpc["x-alcomd-publication"], "implemented-published");
    assert_eq!(
        rpc["$defs"]["methodName"]["enum"],
        json!(["backups.planRestore", "backups.applyRestore"])
    );
    assert_eq!(rpc["$defs"]["capability"]["const"], "backups.restore.v1");
    assert_eq!(
        rpc["x-alcomd-method-permissions"]["backups.planRestore"],
        json!(["backups.read", "projects.create"])
    );
    assert_eq!(
        rpc["x-alcomd-method-permissions"]["backups.applyRestore"],
        json!(["backups.read", "backups.manage", "projects.create"])
    );
    assert!(PERMISSIONS.contains("`backups.planRestore` 需要 `backups.read + projects.create`"));
    assert!(PERMISSIONS.contains("`backups.applyRestore`"));

    let commands = cli["commands"].as_array().expect("published commands");
    let published = commands
        .iter()
        .find(|command| command["path"] == json!(["backup", "restore"]))
        .expect("published Restore command");
    assert_eq!(published["published"], true);
    assert_eq!(published["nonTtyWithoutYes"], "confirmation_required");
    assert_eq!(
        published["dryRun"],
        "persist-plan-only-no-operation-no-filesystem-write"
    );
    assert_eq!(cli["plannedCommands"], json!([]));

    let codes = errors["properties"]["code"]["enum"]
        .as_array()
        .expect("error codes");
    for code in [
        "backup_restore_plan_not_found",
        "backup_restore_plan_stale",
        "backup_target_exists",
        "backup_target_invalid",
        "backup_archive_invalid",
        "backup_integrity_mismatch",
        "backup_unavailable",
        "backup_restore_recovery_required",
        "permission_denied",
        "idempotency_conflict",
        "operation_cancelled",
        "internal_error",
    ] {
        assert!(codes.iter().any(|candidate| candidate == code), "{code}");
    }
    assert!(!codes.iter().any(|candidate| candidate == "restore_failed"));
}

#[test]
fn restore_profile_hostile_vectors_and_forward_recovery_are_frozen() {
    let rpc: Value = serde_json::from_str(RPC_SCHEMA).expect("Restore RPC Schema");
    let profile: Value = serde_json::from_str(PROFILE).expect("Backup profile");
    let vectors: Value = serde_json::from_str(VECTORS).expect("Restore vectors");

    assert_eq!(vectors["archiveRoots"], json!(["backup.json", "project/"]));
    assert_eq!(profile["archiveRoots"], vectors["archiveRoots"]);
    assert_eq!(profile["compressionMethods"], json!(["stored", "deflate"]));
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
    assert_eq!(
        rpc["$defs"]["progressPhase"]["enum"],
        vectors["progressPhases"]
    );
    assert_eq!(
        vectors["killRestartPoints"]
            .as_array()
            .expect("kill points")
            .len(),
        6
    );
    assert!(
        vectors["hostileEntries"]
            .as_array()
            .expect("hostile entries")
            .len()
            >= 9
    );
    assert!(
        vectors["targetRaces"]
            .as_array()
            .expect("target races")
            .len()
            >= 6
    );
    assert!(
        vectors["artifactRaces"]
            .as_array()
            .expect("artifact races")
            .len()
            >= 4
    );
    assert_eq!(
        vectors["terminalOutcomes"][2],
        "recovery-required-with-user-visible-target-preserved"
    );
}

#[test]
fn schema_v7_is_restore_specific_and_keeps_dispatchers_isolated() {
    assert!(MIGRATION_V7.starts_with("BEGIN IMMEDIATE;"));
    assert!(MIGRATION_V7.ends_with("COMMIT;\n"));
    assert!(MIGRATION_V7.contains("'backups.create', 'backups.restore'"));
    assert!(MIGRATION_V7.contains("CREATE TABLE backup_restore_plans"));
    assert!(MIGRATION_V7.contains("CREATE TABLE backup_restore_filesystem_journal"));
    assert!(!MIGRATION_V7.contains("CREATE TABLE generic"));
    assert!(!MIGRATION_V7.contains("workflow"));
    assert!(!MIGRATION_V7.contains("ALTER TABLE backups"));
    assert!(STATE_V7.contains("`operation_journal.kind='templates.create-project'`"));
    assert!(STATE_V7.contains("`target_published` 后只能"));
    assert!(STATE_V7.contains("v7 Schema 不实现或发布 Restore handler"));
}
