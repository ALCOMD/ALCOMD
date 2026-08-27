use serde_json::{Value, json};
use std::collections::BTreeSet;

const COPY_SCHEMA: &str = include_str!("../../../specs/rpc/m7-project-copy.proposal.schema.json");
const STATE_V10: &str = include_str!("../../../specs/storage/state-v10.md");
const STATE_V10_MIGRATION: &str =
    include_str!("../../../specs/storage/state-v10-migration.proposal.contract.json");
const COPY_VECTORS: &str = include_str!("../fixtures/m7/project-copy-contract-vectors.json");
const ACTION_GATE: &str = include_str!("../fixtures/m7/visible-action-completeness-v1.json");
const ACTIVE_RPC: &str = include_str!("../../../specs/rpc/alcomd-rpc-v1.md");
const ACTIVE_STORE: &str = include_str!("../../alcomd-store/src/sqlite.rs");
const ROOT_MANIFEST: &str = include_str!("../../../Cargo.toml");
const GUI_MANIFEST: &str = include_str!("../../../apps/alcomd-gui/src-tauri/Cargo.toml");
const ROOT_LOCK: &str = include_str!("../../../Cargo.lock");
const CURRENT_PROJECTS_UI: &str = include_str!("../../../apps/alcomd-gui/src/CorePages.tsx");

#[test]
fn project_copy_proposal_is_bounded_and_not_active() {
    let schema: Value = serde_json::from_str(COPY_SCHEMA).expect("copy proposal schema");
    assert_eq!(schema["x-alcomd-publication"], "proposal-only-not-active");
    assert_eq!(schema["x-alcomd-active-rpc-modified"], false);
    assert_eq!(schema["x-alcomd-capability"], "projects.copy.v1");
    assert_eq!(schema["x-alcomd-operation-kind"], "projects.copy");
    assert_eq!(schema["x-alcomd-plan-expiry-ms"], 900_000);
    assert_eq!(
        schema["x-alcomd-method-permissions"]["projects.planCopy"],
        json!(["projects.read", "projects.create"])
    );
    assert_eq!(
        schema["x-alcomd-method-permissions"]["projects.applyCopy"],
        json!(["projects.read", "projects.create"])
    );

    let required = schema["$defs"]["projectCopyPlan"]["required"]
        .as_array()
        .expect("plan required")
        .iter()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    for field in [
        "planId",
        "ownerPrincipalId",
        "sourceProjectId",
        "sourceProjectRevision",
        "sourceCanonicalRootPath",
        "sourceFilesystemIdentity",
        "writerEvidence",
        "targetParentCanonicalPath",
        "targetParentFilesystemIdentity",
        "normalizedTargetLeaf",
        "targetMustNotExist",
        "targetProjectId",
        "profile",
        "planFingerprint",
        "idempotencyKey",
        "createdAtMs",
        "expiresAtMs",
    ] {
        assert!(
            required.contains(field),
            "missing bounded Plan field {field}"
        );
    }
    assert!(!required.contains("inventory"));
    assert!(!required.contains("treeFingerprint"));

    assert!(!ACTIVE_RPC.contains("projects.planCopy"));
    assert!(!ACTIVE_RPC.contains("projects.applyCopy"));
    assert!(ACTIVE_STORE.contains("const DATA_SCHEMA_VERSION: i64 = 9;"));
}

#[test]
fn project_copy_profile_phases_and_recovery_are_exact() {
    let vectors: Value = serde_json::from_str(COPY_VECTORS).expect("copy vectors");
    assert_eq!(vectors["status"], "proposal-only-not-active");
    assert_eq!(vectors["planExpiryMs"], 900_000);
    assert_eq!(vectors["planClaimsFullInventory"], false);
    assert_eq!(vectors["profile"]["quota"]["maxEntries"], 500_000);
    assert_eq!(
        vectors["profile"]["quota"]["maxSingleFileBytes"],
        34_359_738_368_u64
    );
    assert_eq!(
        vectors["profile"]["quota"]["maxTotalRegularFileBytes"],
        137_438_953_472_u64
    );
    assert_eq!(vectors["profile"]["quota"]["maxDepth"], 128);
    assert_eq!(
        vectors["profile"]["quota"]["maxNormalizedPathUtf8Bytes"],
        1024
    );
    assert_eq!(
        vectors["phases"],
        json!([
            "accepted",
            "inventory_ready",
            "staging",
            "staging_complete",
            "publish_intent",
            "target_published",
            "project_registry_commit_intent",
            "state_committed",
            "cleanup_complete"
        ])
    );
    assert_eq!(vectors["cancelBefore"], "publish_intent");
    assert_eq!(vectors["afterPublishIntent"], "forward-recovery-only");
    assert_eq!(vectors["lockOrdering"], "ResourceKey::canonical_bytes");
    assert_eq!(vectors["forbiddenError"], "project_copy_failed");

    let rejects = vectors["profile"]["reject"]
        .as_array()
        .expect("rejects")
        .iter()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    for rejected in [
        "symlink",
        "junction",
        "reparse-point",
        "hard-linked-regular-file",
        "special-file",
        "non-utf8-name",
        "unicode-collision",
        "case-collision",
        "target-inside-source",
        "source-inside-target",
        "overwrite",
        "merge",
    ] {
        assert!(rejects.contains(rejected), "missing rejection {rejected}");
    }
}

#[test]
fn state_v10_remains_copy_only_and_proposal_only() {
    let migration: Value =
        serde_json::from_str(STATE_V10_MIGRATION).expect("state v10 migration proposal");
    assert_eq!(migration["status"], "proposal-only-not-active");
    assert_eq!(migration["from"], 9);
    assert_eq!(migration["to"], 10);
    assert_eq!(migration["productionMigration"], Value::Null);
    assert_eq!(migration["productionWiringCreated"], false);
    assert_eq!(migration["operationKindsAdded"], json!(["projects.copy"]));
    assert_eq!(
        migration["tablesAdded"],
        json!(["project_copy_plans", "project_copy_filesystem_journal"])
    );
    assert_eq!(migration["plan"]["fullInventoryStored"], false);
    assert_eq!(migration["plan"]["expiryMs"], 900_000);
    assert_eq!(migration["journal"]["appendOnly"], true);
    assert!(STATE_V10.contains("daemon 继续广告 `dataSchema: 9`"));
    assert!(STATE_V10.contains("不包含 Favorite"));
}

#[test]
fn dependency_candidates_have_not_entered_production() {
    for candidate in ["tauri-plugin-opener", "tauri-plugin-dialog"] {
        assert!(!ROOT_MANIFEST.contains(candidate));
        assert!(!GUI_MANIFEST.contains(candidate));
        assert!(!ROOT_LOCK.contains(&format!("name = \"{candidate}\"")));
    }
}

#[test]
fn visible_action_gate_records_real_current_gaps_instead_of_hiding_them() {
    let gate: Value = serde_json::from_str(ACTION_GATE).expect("visible action gate");
    assert_eq!(gate["productionGateMayPass"], false);
    assert_eq!(gate["expectedCurrentPermanentFakeCount"], 2);
    assert_eq!(
        gate["releaseBlockerFeatures"],
        json!(["projects.management", "packages.vpm"])
    );
    let visible = gate["visibleActions"].as_array().expect("visible actions");
    let permanent = visible
        .iter()
        .filter(|action| action["classification"] == "permanent-disabled-release-blocker")
        .collect::<Vec<_>>();
    assert_eq!(permanent.len(), 2);
    assert!(
        permanent
            .iter()
            .any(|action| action["id"] == "projects.open-directory")
    );
    assert!(
        permanent
            .iter()
            .any(|action| action["id"] == "projects.copy")
    );
    assert!(CURRENT_PROJECTS_UI.contains("disabled label=\"Open Project Directory\""));
    assert!(CURRENT_PROJECTS_UI.contains("disabled label=\"Copy Project\""));
    assert!(
        gate["knownParityGaps"]
            .as_array()
            .expect("known gaps")
            .len()
            > 2
    );
}
