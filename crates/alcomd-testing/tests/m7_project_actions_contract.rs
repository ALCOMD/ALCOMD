use serde_json::{Value, json};
use std::collections::BTreeSet;

const COPY_SCHEMA: &str = include_str!("../../../specs/rpc/m7-project-copy.proposal.schema.json");
const STATE_V10: &str = include_str!("../../../specs/storage/state-v10.md");
const STATE_V10_MIGRATION: &str =
    include_str!("../../../specs/storage/state-v10-migration.proposal.contract.json");
const COPY_VECTORS: &str = include_str!("../fixtures/m7/project-copy-contract-vectors.json");
const ACTION_GATE: &str = include_str!("../fixtures/m7/visible-action-completeness-v1.json");
const P5_PREFERENCE_SCHEMA: &str =
    include_str!("../../../specs/rpc/m7-project-preferences.proposal.schema.json");
const P5_PREFERENCE_VECTORS: &str =
    include_str!("../fixtures/m7/project-preferences-contract-vectors.json");
const STATE_V11: &str = include_str!("../../../specs/storage/state-v11.md");
const STATE_V11_MIGRATION: &str =
    include_str!("../../../specs/storage/state-v11-migration.proposal.contract.json");
const ACTIVE_RPC: &str = include_str!("../../../specs/rpc/alcomd-rpc-v1.md");
const ACTIVE_STORE: &str = include_str!("../../alcomd-store/src/sqlite.rs");
const ACTIVE_PROTOCOL: &str = include_str!("../../alcomd-protocol/src/lib.rs");
const ACTIVE_COPY_MIGRATION: &str =
    include_str!("../../alcomd-store/migrations/0010_project_copy.sql");
const ROOT_MANIFEST: &str = include_str!("../../../Cargo.toml");
const GUI_MANIFEST: &str = include_str!("../../../apps/alcomd-gui/src-tauri/Cargo.toml");
const ROOT_LOCK: &str = include_str!("../../../Cargo.lock");
const DEPENDENCY_EVALUATION: &str =
    include_str!("../../../docs/exec-plans/M7-project-actions-dependency-evaluation.md");
const CURRENT_PROJECTS_UI: &str = include_str!("../../../apps/alcomd-gui/src/CorePages.tsx");
const GUI_RUST_ADAPTER: &str = include_str!("../../../apps/alcomd-gui/src-tauri/src/lib.rs");
const GUI_RPC_ADAPTER: &str = include_str!("../../../apps/alcomd-gui/src/rpc.ts");
const GUI_CAPABILITY: &str =
    include_str!("../../../apps/alcomd-gui/src-tauri/capabilities/main.json");

#[test]
fn implemented_project_copy_contract_is_bounded_and_active() {
    let schema: Value = serde_json::from_str(COPY_SCHEMA).expect("copy proposal schema");
    assert_eq!(schema["x-alcomd-publication"], "implemented");
    assert_eq!(schema["x-alcomd-active-rpc-modified"], true);
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

    assert!(ACTIVE_RPC.contains("projects.planCopy"));
    assert!(ACTIVE_RPC.contains("projects.applyCopy"));
    assert!(ACTIVE_PROTOCOL.contains("METHOD_PROJECTS_PLAN_COPY"));
    assert!(ACTIVE_PROTOCOL.contains("METHOD_PROJECTS_APPLY_COPY"));
    assert!(ACTIVE_PROTOCOL.contains("CAPABILITY_PROJECTS_COPY_V1"));
    assert!(ACTIVE_STORE.contains("const DATA_SCHEMA_VERSION: i64 = 10;"));
}

#[test]
fn project_copy_profile_phases_and_recovery_are_exact() {
    let vectors: Value = serde_json::from_str(COPY_VECTORS).expect("copy vectors");
    assert_eq!(vectors["status"], "implemented");
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
    assert_eq!(vectors["privateInventoryManifest"]["publicRpc"], false);
    assert_eq!(vectors["privateInventoryManifest"]["logged"], false);
    assert_eq!(
        vectors["privateInventoryManifest"]["syncBeforePhase"],
        "inventory_ready"
    );
    assert_eq!(vectors["sourceConsistency"]["copyCalculatesSha256"], true);
    assert_eq!(
        vectors["sourceConsistency"]["secondPassBefore"],
        "publish_intent"
    );
    assert_eq!(
        vectors["stagingLayout"]["permanentMarkerInFinalProject"],
        false
    );

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
fn state_v10_remains_copy_only_and_is_implemented() {
    let migration: Value =
        serde_json::from_str(STATE_V10_MIGRATION).expect("state v10 migration proposal");
    assert_eq!(migration["status"], "implemented");
    assert_eq!(migration["from"], 9);
    assert_eq!(migration["to"], 10);
    assert_eq!(
        migration["productionMigration"],
        "crates/alcomd-store/migrations/0010_project_copy.sql"
    );
    assert_eq!(migration["productionWiringCreated"], true);
    assert_eq!(migration["operationKindsAdded"], json!(["projects.copy"]));
    assert_eq!(
        migration["tablesAdded"],
        json!(["project_copy_plans", "project_copy_filesystem_journal"])
    );
    assert_eq!(migration["plan"]["fullInventoryStored"], false);
    assert_eq!(migration["plan"]["expiryMs"], 900_000);
    assert_eq!(migration["journal"]["appendOnly"], true);
    assert_eq!(
        migration["privateInventoryManifest"]["storedInStateDb"],
        false
    );
    assert!(STATE_V10.contains("daemon 广告 `dataSchema: 10`"));
    assert!(STATE_V10.contains("不包含 Favorite"));
    assert!(ACTIVE_COPY_MIGRATION.contains("PRAGMA user_version = 10;"));
    assert!(ACTIVE_COPY_MIGRATION.contains("project_copy_plans"));
    assert!(ACTIVE_COPY_MIGRATION.contains("project_copy_filesystem_journal"));
}

#[test]
fn dependency_decisions_are_exact_and_opener_plugin_stays_rejected() {
    assert!(!ROOT_MANIFEST.contains("tauri-plugin-opener"));
    assert!(!GUI_MANIFEST.contains("tauri-plugin-opener"));
    assert!(!ROOT_LOCK.contains("name = \"tauri-plugin-opener\""));
    assert!(DEPENDENCY_EVALUATION.contains("`tauri-plugin-opener`：**rejected**"));
    assert!(DEPENDENCY_EVALUATION.contains("`open = 5.4.2`：**approved**"));
    assert!(DEPENDENCY_EVALUATION.contains("`tauri-plugin-dialog = 2.7.2`：**approved**"));
}

#[test]
fn visible_action_gate_records_real_current_gaps_instead_of_hiding_them() {
    let gate: Value = serde_json::from_str(ACTION_GATE).expect("visible action gate");
    assert_eq!(gate["productionGateMayPass"], false);
    assert_eq!(gate["expectedCurrentPermanentFakeCount"], 0);
    assert_eq!(
        gate["releaseBlockerFeatures"],
        json!(["projects.management", "packages.vpm"])
    );
    let visible = gate["visibleActions"].as_array().expect("visible actions");
    for implemented in [
        "projects.create-from-template",
        "projects.restore-managed-backup",
    ] {
        assert!(visible.iter().any(|action| {
            action["id"] == implemented && action["classification"] == "implemented"
        }));
    }
    let permanent = visible
        .iter()
        .filter(|action| action["classification"] == "permanent-disabled-release-blocker")
        .collect::<Vec<_>>();
    assert!(permanent.is_empty());
    assert!(!CURRENT_PROJECTS_UI.contains("disabled label=\"Open Project Directory\""));
    assert!(CURRENT_PROJECTS_UI.contains(
        "label={selectingCopyTarget ? \"Choosing Copy Destination…\" : \"Copy Project\"}"
    ));
    assert!(
        gate["knownParityGaps"]
            .as_array()
            .expect("known gaps")
            .len()
            > 2
    );
    assert!(
        !gate["knownParityGaps"]
            .as_array()
            .expect("known gaps")
            .iter()
            .any(|gap| gap == "create-entry" || gap == "restore-entry")
    );
    for proposal in ["favorite", "clear-unity-preference"] {
        assert!(
            gate["contractFirstProposals"]
                .as_array()
                .expect("contract-first proposals")
                .iter()
                .any(|entry| {
                    entry["id"] == proposal
                        && entry["status"] == "proposal-only-owner-approval-required"
                        && entry["productionImplemented"] == false
                })
        );
    }
}

#[test]
fn official_gui_local_project_affordances_remain_closed() {
    assert!(GUI_RUST_ADAPTER.contains("async fn gui_open_project_directory("));
    assert!(GUI_RUST_ADAPTER.contains("project_id: String"));
    assert!(GUI_RUST_ADAPTER.contains("client.project_get(project_id).await"));
    assert!(GUI_RUST_ADAPTER.contains("open::that(root)"));
    assert!(!GUI_RUST_ADAPTER.contains("open::with"));
    assert!(GUI_RUST_ADAPTER.contains("async fn gui_select_directory(app: tauri::AppHandle)"));
    assert!(GUI_RPC_ADAPTER.contains("gui_open_project_directory"));
    assert!(GUI_RPC_ADAPTER.contains("gui_select_directory"));
    assert!(!GUI_CAPABILITY.contains("dialog:"));
    assert!(!GUI_CAPABILITY.contains("opener:"));
}

#[test]
fn p5_b_project_preferences_are_exact_proposals_not_production_wiring() {
    let schema: Value =
        serde_json::from_str(P5_PREFERENCE_SCHEMA).expect("P5 preference proposal schema");
    let vectors: Value =
        serde_json::from_str(P5_PREFERENCE_VECTORS).expect("P5 preference vectors");
    let migration: Value = serde_json::from_str(STATE_V11_MIGRATION).expect("State v11 proposal");

    assert_eq!(
        schema["x-alcomd-publication"],
        "proposal-only-owner-approval-required"
    );
    assert_eq!(schema["x-alcomd-active-rpc-modified"], false);
    assert_eq!(schema["x-alcomd-state-schema"], 11);
    assert_eq!(
        schema["properties"]["methods"]["const"],
        json!([
            "projects.setFavorite",
            "unity.projectEditor.selection.get",
            "unity.projectEditor.clear"
        ])
    );
    assert_eq!(
        schema["properties"]["permissions"]["const"],
        json!({
            "projects.setFavorite": ["projects.manage"],
            "unity.projectEditor.selection.get": ["unity.read"],
            "unity.projectEditor.clear": ["unity.manage"]
        })
    );
    assert!(
        schema["$defs"]["registeredProjectFavoriteExtension"]["required"]
            .as_array()
            .expect("registered project favorite fields")
            .iter()
            .any(|field| field == "favorite")
    );
    assert_eq!(
        schema["properties"]["stableErrors"]["const"]["automaticLaunchResolution"],
        json!([
            "unity_installation_not_found",
            "unity_editor_selection_required"
        ])
    );

    assert_eq!(vectors["v3Favorite"]["favoriteOnlyFilter"], false);
    assert_eq!(vectors["v3Favorite"]["survivesUnregister"], false);
    assert_eq!(
        vectors["favoriteProposal"]["event"],
        "project.favorite_changed"
    );
    assert_eq!(
        vectors["favoriteProposal"]["coreListOrderingChanges"],
        false
    );
    assert_eq!(vectors["favoriteProposal"]["cursorChanges"], false);
    assert_eq!(vectors["v3UnityClear"]["preservesArguments"], true);
    assert_eq!(
        vectors["unityProposal"]["clearMethod"],
        "unity.projectEditor.clear"
    );
    assert_eq!(vectors["unityProposal"]["projectRevisionChanges"], false);
    assert_eq!(
        vectors["unityProposal"]["automaticMultipleMatchError"],
        "unity_editor_selection_required"
    );

    assert_eq!(migration["status"], "proposal-only-owner-approval-required");
    assert_eq!(migration["from"], 10);
    assert_eq!(migration["to"], 11);
    assert_eq!(migration["productionMigration"], Value::Null);
    assert_eq!(migration["productionWiringCreated"], false);
    assert_eq!(migration["tablesAdded"], json!([]));
    assert_eq!(
        migration["projectEditorPreference"]["argumentsPreservedOnClear"],
        true
    );
    assert!(STATE_V11.contains("尚无 production migration"));
    assert!(STATE_V11.contains("daemon 仍广告 `dataSchema: 10`"));

    assert!(!ACTIVE_RPC.contains("projects.setFavorite"));
    assert!(!ACTIVE_RPC.contains("unity.projectEditor.selection.get"));
    assert!(!ACTIVE_RPC.contains("unity.projectEditor.clear"));
    assert!(!ACTIVE_PROTOCOL.contains("METHOD_PROJECTS_SET_FAVORITE"));
    assert!(!ACTIVE_PROTOCOL.contains("METHOD_UNITY_PROJECT_EDITOR_CLEAR"));
    assert!(ACTIVE_STORE.contains("const DATA_SCHEMA_VERSION: i64 = 10;"));
    assert!(
        !std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../alcomd-store/migrations/0011_project_preferences.sql")
            .exists()
    );
}
