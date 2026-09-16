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
const ACTIVE_UNITY_SCHEMA: &str = include_str!("../../../specs/rpc/m5-unity.schema.json");
const ACTIVE_UNITY_MIGRATION: &str =
    include_str!("../../alcomd-store/migrations/0014_unity_project_version.sql");
const ACTIVE_COPY_MIGRATION: &str =
    include_str!("../../alcomd-store/migrations/0010_project_copy.sql");
const ACTIVE_DELETE_MIGRATION: &str =
    include_str!("../../alcomd-store/migrations/0013_project_directory_delete.sql");
const ROOT_MANIFEST: &str = include_str!("../../../Cargo.toml");
const GUI_MANIFEST: &str = include_str!("../../../apps/alcomd-gui/src-tauri/Cargo.toml");
const ROOT_LOCK: &str = include_str!("../../../Cargo.lock");
const DEPENDENCY_EVALUATION: &str =
    include_str!("../../../docs/exec-plans/M7-project-actions-dependency-evaluation.md");
const CURRENT_PROJECTS_UI: &str = include_str!("../../../apps/alcomd-gui/src/CorePages.tsx");
const CURRENT_ACTIONS_UI: &str = include_str!("../../../apps/alcomd-gui/src/CoreActions.tsx");
const CURRENT_CAPABILITY_GATE: &str = include_str!("../../../apps/alcomd-gui/src/capabilities.tsx");
const CURRENT_APP: &str = include_str!("../../../apps/alcomd-gui/src/App.tsx");
const GUI_RUST_ADAPTER: &str = include_str!("../../../apps/alcomd-gui/src-tauri/src/lib.rs");
const GUI_RPC_ADAPTER: &str = include_str!("../../../apps/alcomd-gui/src/rpc.ts");
const GUI_CAPABILITY: &str =
    include_str!("../../../apps/alcomd-gui/src-tauri/capabilities/main.json");
const P7_DELETE_SCHEMA: &str =
    include_str!("../../../specs/rpc/m7-project-delete.proposal.schema.json");
const P7_STATE_V13: &str =
    include_str!("../../../specs/storage/state-v13-project-delete.proposal.contract.json");
const P7_DELETE_VECTORS: &str =
    include_str!("../../../specs/security/m7-project-delete-path-vectors.json");
const ACTIVE_PERMISSIONS: &str = include_str!("../../../specs/extensions/permissions-v1.md");

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
    assert!(ACTIVE_STORE.contains("const DATA_SCHEMA_VERSION: i64 = 14;"));
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
fn visible_action_gate_separates_m7_completeness_from_m11_release_blockers() {
    let gate: Value = serde_json::from_str(ACTION_GATE).expect("visible action gate");
    assert_eq!(gate["expectedCurrentPermanentFakeCount"], 0);
    assert_eq!(
        gate["releaseBlockerFeatures"],
        json!(["projects.management", "packages.vpm"])
    );
    let visible = gate["visibleActions"].as_array().expect("visible actions");
    assert!(
        visible.len() >= 80,
        "visible action inventory must remain exhaustive"
    );
    let mut ids = BTreeSet::new();
    for action in visible {
        let id = action["id"].as_str().expect("visible action id");
        assert!(ids.insert(id), "duplicate visible action {id}");
        let owner = action["ownerMilestone"].as_str().expect("owner milestone");
        let classification = action["classification"].as_str().expect("classification");
        let release_blocker = action["releaseBlocker"].as_bool().expect("release blocker");
        match owner {
            "M7" => {
                assert!(
                    matches!(classification, "implemented" | "conditional-disabled"),
                    "M7 action {id} has incomplete classification {classification}"
                );
                assert!(
                    !release_blocker,
                    "M7-owned completeness must not hide a release blocker"
                );
            }
            "M11" => {
                assert_eq!(classification, "blocked-future-milestone");
                assert!(
                    release_blocker,
                    "M11 parity entry must remain a release blocker"
                );
            }
            unexpected => panic!("unexpected owner milestone {unexpected}"),
        }
    }
    assert_eq!(gate["verdicts"]["m7OwnedCompleteness"], "PASS");
    assert_eq!(
        gate["verdicts"]["globalReleaseCompleteness"],
        "BLOCKED_BY_M11"
    );
    assert_eq!(gate["verdicts"]["m7OwnedGateMayPass"], true);
    assert_eq!(gate["verdicts"]["globalReleaseGateMayPass"], false);
    assert_eq!(
        gate["verdicts"]["globalBlockers"],
        json!([
            "migration.vcc-import",
            "migration.vcc-migrate",
            "migration.legacy-entry",
            "migration.v3-differential-parity-entry"
        ])
    );
    assert!(!CURRENT_PROJECTS_UI.contains("disabled label=\"Open Project Directory\""));
    assert!(CURRENT_PROJECTS_UI.contains(
        "label={selectingCopyTarget ? \"Choosing Copy Destination…\" : \"Copy Project\"}"
    ));
    for implemented in ["favorite", "remove-directory"] {
        assert!(
            gate["contractFirstProposals"]
                .as_array()
                .expect("contract-first proposals")
                .iter()
                .any(|entry| {
                    entry["id"] == implemented
                        && entry["status"] == "implemented-remote-green"
                        && entry["productionImplemented"] == true
                })
        );
    }
    assert!(
        gate["contractFirstProposals"]
            .as_array()
            .expect("contract-first proposals")
            .iter()
            .any(|entry| {
                entry["id"] == "unity-product-model"
                    && entry["status"] == "implemented-local-candidate"
                    && entry["productionImplemented"] == true
            })
    );
    let unity = &gate["unityProductModel"];
    assert_eq!(unity["automaticEditorSelection"], false);
    assert_eq!(unity["preferredEditorSelection"], false);
    assert_eq!(unity["clearEditorPreference"], false);
    assert_eq!(unity["projectUnitySelector"], "migration");
    assert_eq!(unity["openUnity"], "canonical-exact-match");
    assert_eq!(
        unity["exactMatchCardinality"],
        json!([
            "zero-migrate-or-cancel",
            "one-direct",
            "many-one-shot-chooser"
        ])
    );
    assert_eq!(unity["installationChoicePersistence"], "none");
    assert_eq!(unity["migrationAuthority"], "plan-apply-operation");
    assert_eq!(
        unity["migrateAndOpen"],
        "requery-project-and-launch-options"
    );
    assert_eq!(unity["launchConfig"], "separate-arguments-only");
    for removed in [
        "unity.save-project-editor",
        "unity.clear-project-editor",
        "unity.launch-project",
        "projects.open-unity",
        "projects.detail-unity",
    ] {
        assert!(
            visible.iter().all(|entry| entry["id"] != removed),
            "superseded Unity action remains visible: {removed}"
        );
    }

    for capability in [
        "projects.delete.v1",
        "projects.copy.v1",
        "packages.plan.v2",
        "packages.user-packages.v1",
        "extensions.ui.portable.v1",
    ] {
        assert!(CURRENT_CAPABILITY_GATE.contains(capability));
    }
    assert!(CURRENT_APP.contains("new Set(status.capabilities)"));
    assert!(CURRENT_PROJECTS_UI.contains("capabilityUnavailableTitle"));
    assert!(CURRENT_PROJECTS_UI.contains("disabled={deleting}"));
    assert!(!CURRENT_PROJECTS_UI.contains("Cancel deletion"));
    assert!(CURRENT_ACTIONS_UI.contains("capability_required"));
    assert!(!GUI_RPC_ADAPTER.contains("invokeTyped(\"generic"));
    assert!(!GUI_RPC_ADAPTER.contains("method: string"));
}

#[test]
fn p7_delete_directory_contract_is_implemented_and_active() {
    let schema: Value = serde_json::from_str(P7_DELETE_SCHEMA).expect("P7 delete proposal schema");
    let state: Value = serde_json::from_str(P7_STATE_V13).expect("P7 State v13 proposal");

    assert_eq!(schema["x-alcomd-publication"], "implemented");
    assert_eq!(schema["x-alcomd-approval"], "owner-approved");
    assert_eq!(schema["x-alcomd-active-rpc-modified"], true);
    assert_eq!(schema["x-alcomd-capability"], "projects.delete.v1");
    assert_eq!(
        schema["x-alcomd-operation-kind"],
        "projects.delete-directory"
    );
    assert_eq!(schema["x-alcomd-plan-expiry-ms"], 900_000);
    assert_eq!(schema["x-alcomd-filesystem-writer"], "builtin:local-owner");
    assert_eq!(schema["x-alcomd-extension-grantable"], false);
    assert_eq!(
        schema["x-alcomd-method-permissions"]["projects.planDeleteDirectory"],
        json!(["projects.read", "projects.delete"])
    );
    assert_eq!(
        schema["x-alcomd-method-permissions"]["projects.applyDeleteDirectory"],
        json!(["projects.delete"])
    );

    let plan_required = schema["$defs"]["projectDeletePlan"]["required"]
        .as_array()
        .expect("P7 plan required fields")
        .iter()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    for field in [
        "planId",
        "ownerPrincipalId",
        "projectId",
        "projectRevision",
        "canonicalRootPath",
        "rootFilesystemIdentity",
        "canonicalParentPath",
        "parentFilesystemIdentity",
        "parentIdentitySha256",
        "normalizedLeaf",
        "projectMarkerSha256",
        "writerEvidence",
        "profile",
        "planFingerprint",
        "idempotencyKey",
        "createdAtMs",
        "expiresAtMs",
    ] {
        assert!(
            plan_required.contains(field),
            "missing P7 Plan field {field}"
        );
    }
    assert!(!plan_required.contains("recursiveInventory"));

    assert_eq!(state["status"], "implemented");
    assert_eq!(state["approval"], "owner-approved");
    assert_eq!(state["from"], 12);
    assert_eq!(state["to"], 13);
    assert_eq!(
        state["productionMigration"],
        "crates/alcomd-store/migrations/0013_project_directory_delete.sql"
    );
    assert_eq!(state["productionWiringCreated"], true);
    assert_eq!(
        state["tablesAdded"],
        json!(["project_delete_plans", "project_delete_filesystem_journal"])
    );
    assert_eq!(state["journal"]["appendOnly"], true);
    assert_eq!(
        state["registryCommit"]["event"],
        "project.directory_deleted"
    );
    assert_eq!(state["registryCommit"]["eventCount"], 1);
    assert_eq!(
        state["registryCommit"]["projectUnregisteredEventAlsoWritten"],
        false
    );
    assert_eq!(state["durableProjectReferenceCorrection"]["required"], true);
    assert_eq!(
        state["durableProjectReferenceCorrection"]["tablesToRebuild"],
        json!([
            "package_plans",
            "package_filesystem_journal",
            "project_copy_plans",
            "project_copy_filesystem_journal"
        ])
    );
    assert_eq!(
        state["durableProjectReferenceCorrection"]["ephemeralCurrentProjectRowsRetainingCascade"],
        json!(["project_editor_preferences"])
    );

    assert!(ACTIVE_RPC.contains("projects.planDeleteDirectory"));
    assert!(ACTIVE_RPC.contains("projects.applyDeleteDirectory"));
    assert!(ACTIVE_PROTOCOL.contains("METHOD_PROJECTS_PLAN_DELETE_DIRECTORY"));
    assert!(ACTIVE_PROTOCOL.contains("METHOD_PROJECTS_APPLY_DELETE_DIRECTORY"));
    assert!(ACTIVE_PROTOCOL.contains("CAPABILITY_PROJECTS_DELETE_V1"));
    assert!(ACTIVE_DELETE_MIGRATION.contains("project_delete_plans"));
    assert!(ACTIVE_DELETE_MIGRATION.contains("project_delete_filesystem_journal"));
    assert!(ACTIVE_PERMISSIONS.contains("`projects.delete`"));
    assert!(ACTIVE_STORE.contains("const DATA_SCHEMA_VERSION: i64 = 14;"));
}

#[test]
fn p7_delete_directory_vectors_freeze_fail_closed_recovery_boundaries() {
    let vectors: Value = serde_json::from_str(P7_DELETE_VECTORS).expect("P7 delete path vectors");
    assert_eq!(vectors["status"], "implemented");
    assert_eq!(vectors["approval"], "owner-approved");
    assert_eq!(vectors["mode"], "sibling-quarantine-permanent-v1");
    assert_eq!(vectors["planExpiryMs"], 900_000);
    assert_eq!(vectors["cancelBefore"], "quarantine_intent");
    assert_eq!(vectors["afterBoundary"], "forward-recovery-only");
    assert_eq!(vectors["progress"], "phase-only");
    assert_eq!(
        vectors["writerPolicy"]["running_confirmed"],
        "reject:unity_project_running"
    );
    assert_eq!(
        vectors["writerPolicy"]["not_observed"],
        "allow-with-all-other-revalidation"
    );
    assert_eq!(
        vectors["entryPolicy"]["nested-symlink"],
        "unlink-entry-never-follow"
    );
    assert_eq!(
        vectors["entryPolicy"]["unix-mount-or-bind-mount"],
        "reject-never-cross"
    );

    let cases = vectors["vectors"]
        .as_array()
        .expect("P7 delete vectors")
        .iter()
        .filter_map(|entry| entry["id"].as_str())
        .collect::<BTreeSet<_>>();
    for required in [
        "normal-delete",
        "unregister-only",
        "root-replaced-same-path",
        "writer-confirmed",
        "writer-suspected",
        "writer-unknown",
        "nested-symlink-to-external",
        "windows-junction-to-external",
        "regular-hardlink-in-and-out",
        "unix-cross-device-mount",
        "unix-same-device-bind-mount",
        "kill-after-quarantine-intent-before-rename",
        "kill-after-root-quarantined",
        "kill-after-state-committed",
        "kill-during-deleting",
        "original-path-recreated-after-quarantine",
        "idempotency-apply-replay-after-registry-delete",
        "success-owned-quarantine",
        "recovery-required-owned-quarantine",
    ] {
        assert!(
            cases.contains(required),
            "missing P7 delete vector {required}"
        );
    }

    let forbidden = vectors["forbiddenImplementationClaims"]
        .as_array()
        .expect("forbidden P7 claims")
        .iter()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    for claim in [
        "pre-scan-proves-remove-dir-all-safe",
        "trash-is-portably-recoverable",
        "not-observed-means-unity-definitely-not-running",
        "original-path-may-be-touched-after-quarantine",
    ] {
        assert!(forbidden.contains(claim), "missing forbidden claim {claim}");
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
fn p5_b_preferences_remain_historical_while_state_v14_replaces_editor_selection() {
    let _schema: Value =
        serde_json::from_str(P5_PREFERENCE_SCHEMA).expect("P5 preference historical schema");
    let vectors: Value =
        serde_json::from_str(P5_PREFERENCE_VECTORS).expect("P5 preference vectors");
    let migration: Value = serde_json::from_str(STATE_V11_MIGRATION).expect("State v11 proposal");
    let active_unity: Value =
        serde_json::from_str(ACTIVE_UNITY_SCHEMA).expect("active Unity schema");

    assert_eq!(vectors["status"], "implemented-active-contract-evidence");
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
    assert_eq!(
        vectors["favoriteProposal"]["capability"],
        "projects.registry.v1"
    );
    assert_eq!(
        vectors["favoriteProposal"]["compareRevisionBeforeState"],
        true
    );
    assert_eq!(migration["status"], "implemented-active-contract-evidence");
    assert_eq!(migration["from"], 10);
    assert_eq!(migration["to"], 11);
    assert_eq!(
        migration["productionMigration"],
        "crates/alcomd-store/migrations/0011_project_preferences.sql"
    );
    assert_eq!(migration["productionWiringCreated"], true);
    assert_eq!(migration["userVersionSetLast"], true);
    assert_eq!(migration["rollbackAuthorityOnFailure"], 10);
    assert_eq!(migration["tablesAdded"], json!([]));
    assert!(STATE_V11.contains("`dataSchema: 11`"));

    assert!(ACTIVE_RPC.contains("projects.setFavorite"));
    assert!(ACTIVE_RPC.contains("unity.projectLaunchConfig.get"));
    assert!(ACTIVE_RPC.contains("unity.launchOptions"));
    assert!(ACTIVE_PROTOCOL.contains("METHOD_PROJECTS_SET_FAVORITE"));
    assert!(ACTIVE_PROTOCOL.contains("METHOD_UNITY_PROJECT_LAUNCH_CONFIG_CLEAR"));
    assert!(!ACTIVE_PROTOCOL.contains("METHOD_UNITY_PROJECT_EDITOR_CLEAR"));
    assert!(ACTIVE_STORE.contains("const DATA_SCHEMA_VERSION: i64 = 14;"));
    assert!(ACTIVE_UNITY_MIGRATION.contains("DROP TABLE project_editor_preferences"));
    assert_eq!(
        active_unity["$defs"]["projectUnityLaunchConfig"]["required"],
        json!(["projectId", "arguments", "revision", "updatedAtMs"])
    );
    let active_methods = active_unity["$defs"]["methodName"]["enum"]
        .as_array()
        .expect("active Unity methods");
    for method in [
        "unity.projectLaunchConfig.get",
        "unity.projectLaunchConfig.set",
        "unity.projectLaunchConfig.clear",
        "unity.launchOptions",
    ] {
        assert!(active_methods.iter().any(|candidate| candidate == method));
    }
}
