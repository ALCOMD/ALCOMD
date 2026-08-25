use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

const WEBVIEW_EVIDENCE: &str = include_str!("../../../specs/gui/m7-stop-a.md");
const ACTIVE_MANIFEST: &str = include_str!("../../../specs/extensions/manifest-v1.schema.json");
const ACTIVE_MANIFEST_TEXT: &str = include_str!("../../../specs/extensions/manifest-v1.md");
const ACTIVE_PACKAGE: &str = include_str!("../../../specs/extensions/package-profile-v1.json");
const ACTIVE_ABI: &str = include_str!("../../../specs/extensions/abi-compatibility-v1.json");
const PORTABLE_SCHEMA: &str = include_str!("../../../specs/extensions/portable-ui-v1.schema.json");
const PORTABLE_CONTRACT: &str = include_str!("../../../specs/extensions/portable-ui-v1.md");
const LIMITS: &str = include_str!("../../../specs/extensions/portable-ui-limits-v1.json");
const ACTIVE_RPC: &str = include_str!("../../../specs/rpc/m7-portable-ui.schema.json");
const RPC_ERROR: &str = include_str!("../../../specs/rpc/rpc-error.schema.json");
const ACTIVE_STATE: &str = include_str!("../../../specs/storage/state-v9-migration.contract.json");
const ACTIVE_PERMISSIONS: &str = include_str!("../../../specs/extensions/permissions-v1.md");
const HOST_PROTOCOL_PROPOSAL: &str =
    include_str!("../../../specs/extensions/proposals/host-protocol-invocation-context-v1.md");
const RENDERER_PROPOSAL: &str = include_str!("../../../specs/gui/portable-ui-renderer-v1.md");
const THREAT_MODEL: &str =
    include_str!("../../../specs/security/extension-portable-ui-threat-model.md");
const ACTIVE_WORLD: &str = include_str!("../../../specs/extensions/wit/extension-v1/world.wit");
const ACTIVE_TYPES: &str = include_str!("../../../specs/extensions/wit/extension-v1/types.wit");
const ACTIVE_LIFECYCLE: &str =
    include_str!("../../../specs/extensions/wit/extension-v1/guest-lifecycle.wit");
const ACTIVE_UI: &str = include_str!("../../../specs/extensions/wit/extension-v1/guest-ui.wit");
const MCP_FIXTURE: &str = include_str!("../fixtures/m7/mcp-management-snapshot.json");
const DISCORD_FIXTURE: &str = include_str!("../fixtures/m7/discord-presence-snapshot.json");
const HEADLESS_FIXTURE: &str = include_str!("../fixtures/m7/headless-renderer-conformance.json");
const ADVERSARIAL: &str = include_str!("../fixtures/m7/portable-ui-adversarial-vectors.json");

#[test]
fn superseded_webview_evidence_remains_rejected_and_non_production() {
    assert!(WEBVIEW_EVIDENCE.contains("superseded by Portable Extension UI direction reset"));
    assert!(WEBVIEW_EVIDENCE.contains("sandboxed_cross_origin_iframe = rejected_for_m7_v1"));
    assert!(WEBVIEW_EVIDENCE.contains("child_webview_navigation_unavailable"));
    assert!(WEBVIEW_EVIDENCE.contains("isolated_managed_child_webview = rejected_for_m7_v1"));
    assert!(WEBVIEW_EVIDENCE.contains("没有 production evidence"));
    assert!(WEBVIEW_EVIDENCE.contains(
        "不能标记为 product security、compatibility 或 production implementation success"
    ));
}

#[test]
fn active_manifest_and_package_are_direct_rewrites_without_web_assets() {
    let manifest: Value = serde_json::from_str(ACTIVE_MANIFEST).expect("Manifest");
    let package: Value = serde_json::from_str(ACTIVE_PACKAGE).expect("package profile");

    assert_eq!(manifest["properties"]["schema"]["const"], 1);
    assert_eq!(manifest["properties"]["api"]["const"], 1);
    assert_eq!(
        manifest["properties"]["entrypoints"]["properties"]["component"]["const"],
        "component/extension.wasm"
    );
    assert!(
        manifest["properties"]["entrypoints"]["properties"]
            .get("background_component")
            .is_none()
    );
    assert!(
        manifest["properties"]["entrypoints"]["properties"]
            .get("ui_entry")
            .is_none()
    );
    assert_eq!(
        manifest["properties"]["ui"]["properties"]["protocol"]["const"],
        "portable-v1"
    );
    assert_eq!(
        package["allowedRoots"],
        json!(["alcomd-extension.toml", "META-INF/", "component/"])
    );
    assert!(package["limits"].get("uiTotalBytes").is_none());
    assert!(
        package["rejected"]
            .as_array()
            .expect("rejected")
            .iter()
            .any(|value| value == "ui-root")
    );
}

#[test]
fn active_wit_keeps_abi_major_one_and_requires_portable_ui() {
    let abi: Value = serde_json::from_str(ACTIVE_ABI).expect("ABI contract");
    assert_eq!(abi["abiMajor"], 1);
    assert_eq!(abi["world"], "alcomd:extension/extension-v1@1.0.0");
    assert_eq!(abi["ambientWasiImports"], json!([]));
    assert_eq!(abi["hostEmbedding"], "async");
    assert_eq!(abi["witFunctions"], "sync");
    assert_eq!(abi["componentModelAsync"], false);
    assert_eq!(abi["wasmtimeWasi"], false);
    assert_eq!(abi["guestUiRequiredForEveryAbiV1Component"], true);
    assert_eq!(abi["manifestUiControlsPublicationNotWorldShape"], true);
    assert_eq!(abi["noUiGuestUsesDefaultEmptyStub"], true);
    assert_eq!(
        abi["preReleaseDirectRewrite"]["parallelWorldRetained"],
        false
    );

    assert!(ACTIVE_WORLD.contains("world extension-v1"));
    assert!(ACTIVE_WORLD.contains("import host-projects;"));
    assert!(ACTIVE_WORLD.contains("import host-data;"));
    assert!(ACTIVE_WORLD.contains("export guest-lifecycle;"));
    assert!(ACTIVE_WORLD.contains("export guest-ui;"));
    assert!(ACTIVE_TYPES.contains("type guest-session-id = string;"));
    assert!(!ACTIVE_TYPES.contains("ui-session-id"));
    assert!(
        ACTIVE_UI
            .contains("open: func(session-id: guest-session-id, locale: string) -> ui-document;")
    );
    assert!(ACTIVE_UI.contains("refresh: func(session-id: guest-session-id) -> ui-document;"));
    assert!(ACTIVE_UI.contains(
        "dispatch: func(session-id: guest-session-id, action: ui-action) -> ui-document;"
    ));
    assert!(ACTIVE_UI.contains("close: func(session-id: guest-session-id);"));
    for forbidden in [
        "surface-id",
        "sequence:",
        "request-id:",
        "snapshot-revision",
        "principal",
        "grant-revision",
        "package-digest",
        "invocation-context",
    ] {
        assert!(
            !ACTIVE_UI.contains(forbidden),
            "guest-ui leaked {forbidden}"
        );
    }
    assert!(ACTIVE_TYPES.contains("enum activation-kind"));
    assert!(ACTIVE_TYPES.contains("background,"));
    assert!(ACTIVE_TYPES.contains("interactive-ui,"));
    assert!(ACTIVE_TYPES.contains("interactive-ui-idle,"));
    assert!(ACTIVE_LIFECYCLE.contains("activate: func"));
    for wit in [ACTIVE_TYPES, ACTIVE_LIFECYCLE, ACTIVE_UI, ACTIVE_WORLD] {
        assert!(!wit.contains("wasi:"));
    }
}

#[test]
fn review_closure_freezes_manifest_lifecycle_draft_and_security_responsibility() {
    assert!(ACTIVE_MANIFEST_TEXT.contains("所有 ABI v1 Component 都必须实现 `guest-ui`"));
    assert!(ACTIVE_MANIFEST_TEXT.contains("官方 SDK/reference guest提供空桩"));
    assert!(ACTIVE_MANIFEST_TEXT.contains("`background.run` 不由 `[ui]` 隐式添加"));
    assert!(PORTABLE_CONTRACT.contains("`/extensions/:extensionId/ui`"));
    assert!(PORTABLE_CONTRACT.contains("仍等于 session 当前 revision"));
    assert!(PORTABLE_CONTRACT.contains("不返回历史 Snapshot"));
    assert!(PORTABLE_CONTRACT.contains("不写 localStorage、state.db"));
    assert!(PORTABLE_CONTRACT.contains("discard confirmation"));
    assert!(PORTABLE_CONTRACT.contains("disabled/read-only field 不出现在 wire values"));
    assert!(PORTABLE_CONTRACT.contains("Guest/client failure responsibility"));
    assert!(PORTABLE_CONTRACT.contains("deactivate(interactive-ui-idle)"));
    assert!(PORTABLE_CONTRACT.contains("Portable UI 不产生"));
    assert!(RENDERER_PROPOSAL.contains("publisher-trust/version/desired/runtime/quarantine"));
    assert!(RENDERER_PROPOSAL.contains("aria-describedby"));
    assert!(RENDERER_PROPOSAL.contains("512 UTF-8"));
    assert!(HOST_PROTOCOL_PROPOSAL.contains("Normal authority races"));
    assert!(HOST_PROTOCOL_PROPOSAL.contains("`permission-denied`"));
    assert!(HOST_PROTOCOL_PROPOSAL.contains("`lease-revoked`"));
    assert!(HOST_PROTOCOL_PROPOSAL.contains("`invocation_context_stale`"));
    assert!(HOST_PROTOCOL_PROPOSAL.contains("`cancelled`"));
    assert!(HOST_PROTOCOL_PROPOSAL.contains("completed context reuse"));
    assert!(HOST_PROTOCOL_PROPOSAL.contains("立即完成并失效"));
    assert!(HOST_PROTOCOL_PROPOSAL.contains("Invocation kind 与 capability matrix"));
    assert!(HOST_PROTOCOL_PROPOSAL.contains("interactive-ui-render"));
    assert!(HOST_PROTOCOL_PROPOSAL.contains("interactive-ui-action"));
    assert!(HOST_PROTOCOL_PROPOSAL.contains("interactive-ui-close"));
    assert!(HOST_PROTOCOL_PROPOSAL.contains("extension_permission_denied"));
    assert!(THREAT_MODEL.contains("render impurity"));
    assert!(THREAT_MODEL.contains("历史exact replay"));
    assert!(THREAT_MODEL.contains("session race"));
    assert!(THREAT_MODEL.contains("payload disclosure"));
}

#[test]
fn rpc_state_permissions_and_limits_are_exact_and_closed() {
    let schema: Value = serde_json::from_str(PORTABLE_SCHEMA).expect("Portable UI Schema");
    let limits: Value = serde_json::from_str(LIMITS).expect("limits");
    let rpc: Value = serde_json::from_str(ACTIVE_RPC).expect("active RPC contract");
    let rpc_error: Value = serde_json::from_str(RPC_ERROR).expect("RPC errors");
    let state: Value = serde_json::from_str(ACTIVE_STATE).expect("active State contract");

    assert_eq!(rpc["x-alcomd-publication"], "active-production-contract");

    assert!(limits.get("surfacesPerExtension").is_none());
    assert!(limits.get("surfaceId").is_none());
    assert_eq!(limits["activeSessionsPerExtension"], 8);
    assert_eq!(limits["activeSessionsPerClientConnection"], 16);
    assert_eq!(limits["totalActiveSessions"], 128);
    assert_eq!(limits["encodedSnapshotBytes"], 262_144);
    assert_eq!(limits["encodedDispatchRequestBytes"], 65_536);
    assert_eq!(limits["nodesPerSnapshot"], 256);
    assert_eq!(limits["treeDepth"], 8);
    assert_eq!(limits["individualPlainTextUtf8Bytes"], 4_096);
    assert_eq!(limits["totalPlainTextUtf8BytesPerSnapshot"], 65_536);
    assert_eq!(limits["formFieldsPerSnapshot"], 64);
    assert_eq!(limits["selectOptionsPerField"], 64);
    assert_eq!(limits["validationMessageUtf8Bytes"], 512);
    assert_eq!(limits["concurrentGuestUiCallsPerExtensionHost"], 1);
    assert_eq!(limits["concurrentActionsPerSession"], 1);
    assert_eq!(limits["requestRatePerMinute"], 60);
    assert_eq!(limits["requestBurst"], 10);
    assert_eq!(limits["idleSessionMs"], 300_000);
    assert_eq!(limits["absoluteSessionLifetimeMs"], 3_600_000);
    assert_eq!(limits["rememberedRequestIdsPerSession"], 64);
    assert_eq!(limits["interactiveHostIdleStopMs"], 5_000);

    assert_eq!(
        schema["$defs"]["node"]["properties"]["kind"]["enum"]
            .as_array()
            .expect("node kinds")
            .len(),
        17
    );
    assert_eq!(
        schema["$defs"]["action"]["oneOf"]
            .as_array()
            .expect("action union")
            .len(),
        2
    );
    assert_eq!(
        schema["$defs"]["identifier"]["pattern"],
        "^[a-z][a-z0-9._-]{0,63}$"
    );
    assert!(
        schema["$defs"]["uiDocument"]["properties"]
            .get("surfaceId")
            .is_none()
    );
    let matrix = schema["x-alcomd-parent-child-matrix"]
        .as_object()
        .expect("parent/child matrix");
    assert_eq!(matrix.len(), 17);
    for (parent, children) in matrix {
        let actual = children
            .as_array()
            .expect("child array")
            .iter()
            .map(|child| child.as_str().expect("child kind"))
            .collect::<Vec<_>>();
        assert_eq!(actual, allowed_children(parent), "matrix row {parent}");
    }
    assert_eq!(rpc["x-alcomd-capability"], "extensions.ui.portable.v1");
    assert_eq!(
        rpc["$defs"]["methodName"]["enum"],
        json!([
            "extensions.ui.open",
            "extensions.ui.refresh",
            "extensions.ui.dispatch",
            "extensions.ui.close"
        ])
    );
    assert!(!rpc.to_string().contains("listSurfaces"));
    assert!(!rpc.to_string().contains("surfaceId"));
    assert_eq!(
        rpc["$defs"]["uiDeclaration"]["required"],
        json!(["protocol"])
    );
    assert_eq!(rpc["x-alcomd-route"], "/extensions/:extensionId/ui");
    assert_eq!(
        rpc["x-alcomd-dispatch-replay"]["exactReplay"]["result"],
        "original-resulting-snapshot-only-if-current"
    );
    assert_eq!(
        rpc["x-alcomd-dispatch-replay"]["exactReplay"]["currentRevisionRequired"],
        true
    );
    assert_eq!(
        rpc["x-alcomd-dispatch-replay"]["historicalExactReplay"]["error"],
        "extension_ui_snapshot_stale"
    );
    assert_eq!(rpc["x-alcomd-snapshot-monotonicity"]["openRevision"], 1);
    assert_eq!(
        rpc["x-alcomd-session-coordination"]["states"],
        json!(["active", "closing", "closed"])
    );
    assert_eq!(
        rpc["x-alcomd-invocation-capability-matrix"]["interactive-ui-render"],
        json!(["host-projects.get-summary", "host-data.get"])
    );
    assert_eq!(
        rpc["x-alcomd-invocation-capability-matrix"]["interactive-ui-close"],
        json!([])
    );
    assert_eq!(rpc["x-alcomd-locale"]["immutablePerSession"], true);
    assert_eq!(
        rpc["x-alcomd-sensitive-payload"]["secureZeroizationClaimed"],
        false
    );
    assert_eq!(
        rpc["x-alcomd-dispatch-replay"]["conflictOrOutOfOrder"]["error"],
        "extension_ui_action_invalid"
    );
    for code in [
        "extension_not_enabled",
        "extension_ui_not_available",
        "extension_ui_protocol_unsupported",
        "extension_ui_session_not_found",
        "extension_ui_session_stale",
        "extension_ui_snapshot_stale",
        "extension_ui_document_invalid",
        "extension_ui_action_invalid",
        "extension_ui_limit_exceeded",
    ] {
        assert!(
            rpc["x-alcomd-stable-errors"]
                .as_array()
                .expect("stable errors")
                .iter()
                .any(|value| value == code),
            "missing stable error {code}"
        );
        assert!(
            rpc_error["properties"]["code"]["enum"]
                .as_array()
                .expect("global RPC errors")
                .iter()
                .any(|value| value == code),
            "global RPC error contract is missing {code}"
        );
    }
    assert!(
        !rpc["x-alcomd-stable-errors"]
            .as_array()
            .expect("stable errors")
            .contains(&json!("extension_ui_surface_not_found"))
    );

    assert_eq!(state["from"], 8);
    assert_eq!(state["to"], 9);
    assert_eq!(
        state["productionMigration"],
        "crates/alcomd-store/migrations/0009_portable_extension_ui.sql"
    );
    assert_eq!(state["tablesAdded"], json!([]));
    assert_eq!(state["planFieldsImmutable"], json!(["ui_protocol"]));
    assert_eq!(state["columnsAdded"]["extensions"], json!(["ui_protocol"]));
    assert_eq!(
        state["columnsAdded"]["extension_plans"],
        json!(["ui_protocol"])
    );
    assert!(!state.to_string().contains("ui_surfaces_json"));
    assert!(ACTIVE_PERMISSIONS.contains("`extensions.ui.use`"));
    assert!(ACTIVE_PERMISSIONS.contains("不存在 `ui.contribute`"));
    assert!(ACTIVE_PERMISSIONS.contains("Client Principal 与 Extension Principal"));
    assert!(ACTIVE_PERMISSIONS.contains("任一侧不能扩大另一侧 authority"));
    assert!(HOST_PROTOCOL_PROPOSAL.contains("InvocationContextId"));
    assert!(HOST_PROTOCOL_PROPOSAL.contains("Normal authority races"));
    assert!(HOST_PROTOCOL_PROPOSAL.contains("completed context reuse"));
    assert!(HOST_PROTOCOL_PROPOSAL.contains("Host protocol violation"));
}

#[test]
fn synthetic_mcp_and_discord_documents_cover_the_complete_node_set() {
    let mcp: Value = serde_json::from_str(MCP_FIXTURE).expect("MCP fixture");
    let discord: Value = serde_json::from_str(DISCORD_FIXTURE).expect("Discord fixture");
    let conformance: Value = serde_json::from_str(HEADLESS_FIXTURE).expect("headless fixture");

    let mcp_summary = validate_document(&mcp);
    let discord_summary = validate_document(&discord);
    assert_eq!(mcp_summary.node_count, 15);
    assert_eq!(discord_summary.node_count, 15);
    assert_eq!(
        mcp_summary.action_ids,
        btreeset(["apply-client-action", "refresh-connections"])
    );
    assert_eq!(
        mcp_summary.field_ids,
        btreeset(["client-action", "client-id"])
    );
    assert_eq!(discord_summary.action_ids, btreeset(["save-presence"]));
    assert_eq!(
        discord_summary.field_ids,
        btreeset([
            "presence-enabled",
            "presence-mode",
            "presence-text",
            "refresh-interval",
        ])
    );

    let all_kinds = mcp_summary
        .kinds
        .union(&discord_summary.kinds)
        .cloned()
        .collect::<BTreeSet<_>>();
    let required_kinds = conformance["requiredNodeKinds"]
        .as_array()
        .expect("required node kinds")
        .iter()
        .map(|value| value.as_str().expect("node kind").to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(all_kinds, required_kinds);
    assert_eq!(
        conformance["requiredActionKinds"],
        json!(["activate", "submit-form"])
    );
    assert_eq!(
        conformance["requiredFieldSemantics"],
        json!(["disabled", "readOnly", "validation"])
    );
    assert_eq!(
        conformance["formDraftBinding"],
        json!(["sessionId", "snapshotRevision", "formNodeId"])
    );
    assert_eq!(conformance["dirtyDraftAutomaticRefresh"], false);
    assert_eq!(conformance["draftPersistentStorage"], false);
    assert_eq!(
        conformance["invalidFieldAriaDescribedBy"],
        "host-generated-validation-message-id"
    );
    assert_eq!(conformance["sameSemanticResultRequired"], true);
    assert_eq!(conformance["tauriDependency"], false);
    assert_eq!(conformance["guiDependency"], false);
}

#[test]
fn adversarial_vectors_freeze_guest_client_replay_and_session_failure_classes() {
    let vectors: Value = serde_json::from_str(ADVERSARIAL).expect("adversarial vectors");
    let guest = vector_ids(&vectors["guestDocumentCases"]);
    let client = vector_ids(&vectors["clientActionCases"]);
    let replay = vector_ids(&vectors["replayCases"]);

    for required in [
        "missing-page-root",
        "duplicate-node-id",
        "duplicate-action-id",
        "duplicate-field-id",
        "missing-or-later-parent",
        "cycle",
        "unknown-node",
        "tree-depth-nine",
        "nodes-257",
        "snapshot-over-262144-bytes",
        "nul-control-or-bidi",
        "input-outside-form",
        "nested-form",
        "list-direct-non-item-child",
        "list-item-outside-list",
        "leaf-with-child",
        "unreachable-node",
        "invalid-identifier-grammar",
        "validation-message-over-512-utf8-bytes",
        "submit-action-bound-to-other-form",
    ] {
        assert!(guest.contains(required), "missing guest vector {required}");
    }
    for required in [
        "malformed-rpc-envelope",
        "snapshot-stale",
        "unknown-action",
        "disabled-action",
        "missing-form-field",
        "wrong-field-type",
        "unknown-select-option",
        "extra-form-field",
        "submitted-disabled-field",
        "submitted-read-only-field",
        "future-sequence",
        "same-sequence-new-request-id",
        "replayed-request-id-new-sequence",
        "third-invalid-action-in-60000ms",
    ] {
        assert!(
            client.contains(required),
            "missing client vector {required}"
        );
    }
    assert!(replay.contains("exact-accepted-request-current-revision"));
    assert!(replay.contains("exact-accepted-request-historical-after-refresh"));
    assert!(replay.contains("same-request-id-different-revision"));
    assert!(replay.contains("same-request-id-different-action-fingerprint"));
    assert!(replay.contains("same-sequence-different-revision"));
    assert!(replay.contains("sequence-gap-or-out-of-order"));
    assert!(replay.contains("connection-lost-before-response"));
    let exact = vectors["replayCases"]
        .as_array()
        .expect("replay cases")
        .iter()
        .find(|case| case["id"] == "exact-accepted-request-current-revision")
        .expect("exact replay");
    assert_eq!(exact["cachedRevisionIsCurrent"], true);
    assert_eq!(exact["invokeGuest"], false);
    assert_eq!(exact["returnOriginalResultingSnapshot"], true);
    assert_eq!(exact["returnCurrentSnapshot"], true);
    assert_eq!(exact["replayed"], true);
    let historical = vectors["replayCases"]
        .as_array()
        .expect("replay cases")
        .iter()
        .find(|case| case["id"] == "exact-accepted-request-historical-after-refresh")
        .expect("historical exact replay");
    assert_eq!(historical["cachedRevisionIsCurrent"], false);
    assert_eq!(historical["expectedError"], "extension_ui_snapshot_stale");
    assert_eq!(historical["invokeGuest"], false);
    assert_eq!(historical["returnOriginalResultingSnapshot"], false);

    let capabilities = vector_ids(&vectors["invocationCapabilityCases"]);
    for required in [
        "background-existing-m6-authority",
        "render-project-summary",
        "render-host-data-get",
        "render-host-data-set-denied",
        "render-host-data-delete-denied",
        "action-host-data-write",
        "close-no-capability",
        "project-dual-principal",
        "host-data-dual-authority",
    ] {
        assert!(
            capabilities.contains(required),
            "missing capability vector {required}"
        );
    }
    for denied_id in [
        "render-host-data-set-denied",
        "render-host-data-delete-denied",
    ] {
        let denied = vectors["invocationCapabilityCases"]
            .as_array()
            .expect("capability cases")
            .iter()
            .find(|case| case["id"] == denied_id)
            .expect("render write denial");
        assert_eq!(denied["expectedError"], "extension_permission_denied");
        assert_eq!(denied["protocolViolation"], false);
        assert_eq!(denied["countCrash"], false);
        assert_eq!(denied["quarantine"], false);
    }

    let contexts = vectors["invocationContextCases"]
        .as_array()
        .expect("context cases");
    assert!(contexts.iter().any(|case| {
        case["id"] == "grant-revision-race"
            && case["classification"] == "normal-authority-race"
            && case["terminateHost"] == false
    }));
    assert!(contexts.iter().any(|case| {
        case["id"] == "completed-context-reuse"
            && case["classification"] == "host-protocol-violation"
            && case["terminateHost"] == true
    }));
    let drafts = vector_ids(&vectors["rendererDraftCases"]);
    assert!(drafts.contains("dirty-form-no-automatic-refresh"));
    assert!(drafts.contains("new-revision-invalidates-draft"));
    let monotonic = vector_ids(&vectors["snapshotMonotonicityCases"]);
    for required in [
        "open-starts-at-one",
        "successful-refresh-increments",
        "successful-dispatch-increments",
        "revision-overflow",
        "renderer-newer-revision",
        "renderer-equal-exact-replay",
        "renderer-lower-revision",
    ] {
        assert!(monotonic.contains(required));
    }
    let coordination = vector_ids(&vectors["sessionCoordinationCases"]);
    for required in [
        "one-in-flight-refresh-dispatch-close",
        "close-marks-closed-before-guest",
        "pending-invoke-cancelled-by-close",
        "close-context-has-no-capability",
        "close-trap-session-remains-closed",
        "out-of-order-snapshot-commit",
    ] {
        assert!(coordination.contains(required));
    }
    let locales = vector_ids(&vectors["localeCases"]);
    for required in [
        "open-canonicalizes-locale",
        "locale-change-clean",
        "locale-change-dirty",
        "guest-appearance-inputs",
    ] {
        assert!(locales.contains(required));
    }
    let lifecycle = vector_ids(&vectors["lifecycleCases"]);
    for required in [
        "open-does-not-enable",
        "quarantined-open",
        "ui-protocol-null-open",
        "first-ui-session-activates-interactive-ui-once",
        "additional-ui-session-reuses-activation",
        "background-host-open-does-not-reactivate",
        "last-session-no-background-deactivates",
        "route-close-keeps-desired-state",
        "required-background-ungranted-blocks-enable",
        "optional-background-ungranted-allows-ui",
    ] {
        assert!(
            lifecycle.contains(required),
            "missing lifecycle vector {required}"
        );
    }
    let last_close = vectors["lifecycleCases"]
        .as_array()
        .expect("lifecycle cases")
        .iter()
        .find(|case| case["id"] == "last-session-no-background-deactivates")
        .expect("last close");
    assert_eq!(last_close["reason"], "interactive-ui-idle");
    assert_eq!(last_close["stopDeadlineMs"], 5_000);

    for case in vectors["guestDocumentCases"]
        .as_array()
        .expect("guest cases")
    {
        assert_eq!(case["closeSession"], true);
        assert_eq!(case["terminateHost"], true);
        assert_eq!(case["countCrash"], true);
    }
    for case in vectors["clientActionCases"]
        .as_array()
        .expect("client cases")
    {
        assert_eq!(case["invokeGuest"], false);
        assert_eq!(case["terminateHost"], false);
    }
    assert_eq!(
        vector_ids(&vectors["guestExecutionCases"]),
        ["guest-fuel-or-timeout", "guest-trap"]
            .into_iter()
            .collect()
    );
    assert_eq!(
        vectors["diagnosticPayloadForbidden"],
        json!([
            "document",
            "field-values",
            "filesystem-path",
            "host-frame",
            "guest-trap"
        ])
    );
    assert_eq!(
        vectors["sensitivePayloadForbiddenSinks"],
        json!([
            "event",
            "log",
            "host-stderr",
            "crash-evidence",
            "operation",
            "state-db",
            "telemetry",
            "public-internal-error"
        ])
    );
    assert_eq!(
        vectors["replayEvidenceLifetime"],
        "session-memory-until-close"
    );
    assert_eq!(vectors["secureZeroizationClaimed"], false);
}

struct DocumentSummary {
    node_count: usize,
    kinds: BTreeSet<String>,
    action_ids: BTreeSet<String>,
    field_ids: BTreeSet<String>,
}

fn validate_document(document: &Value) -> DocumentSummary {
    assert_eq!(document["protocol"], "portable-v1");
    assert!(document.get("surfaceId").is_none());
    let encoded = serde_json::to_vec(document).expect("encode fixture");
    assert!(encoded.len() <= 262_144);
    let nodes = document["nodes"].as_array().expect("nodes");
    assert!(!nodes.is_empty());
    assert!(nodes.len() <= 256);

    let mut node_ids = HashSet::new();
    let mut parent_by_id = HashMap::<String, Option<String>>::new();
    let mut kind_by_id = HashMap::<String, String>::new();
    let mut sibling_orders = BTreeMap::<String, BTreeSet<u64>>::new();
    let mut kinds = BTreeSet::new();
    let mut action_ids = BTreeSet::new();
    let mut field_ids = BTreeSet::new();
    let mut roots = 0_u8;

    for node in nodes {
        let node_id = node["nodeId"].as_str().expect("nodeId");
        assert!(is_identifier(node_id));
        assert!(node_ids.insert(node_id.to_owned()), "duplicate {node_id}");
        let kind = node["kind"].as_str().expect("kind");
        kinds.insert(kind.to_owned());

        let parent = node.get("parentId").map(|value| {
            let parent_id = value.as_str().expect("parentId");
            assert!(node_ids.contains(parent_id), "parent must precede child");
            parent_id.to_owned()
        });
        if parent.is_none() {
            roots += 1;
            assert_eq!(kind, "page");
        }
        let sibling_key = parent.clone().unwrap_or_else(|| "<root>".to_owned());
        let order = node["order"].as_u64().expect("order");
        assert!(
            sibling_orders.entry(sibling_key).or_default().insert(order),
            "duplicate sibling order"
        );
        parent_by_id.insert(node_id.to_owned(), parent);
        kind_by_id.insert(node_id.to_owned(), kind.to_owned());

        let payload = &node["payload"];
        if let Some(action_id) = payload
            .get("actionId")
            .or_else(|| payload.get("submitActionId"))
        {
            let action_id = action_id.as_str().expect("action ID");
            assert!(is_identifier(action_id));
            assert!(action_ids.insert(action_id.to_owned()));
        }
        if let Some(field_id) = payload.get("fieldId") {
            let field_id = field_id.as_str().expect("field ID");
            assert!(is_identifier(field_id));
            assert!(field_ids.insert(field_id.to_owned()));
            assert!(payload["disabled"].is_boolean());
            assert!(payload["readOnly"].is_boolean());
            if let Some(validation) = payload.get("validation") {
                let state = validation["state"].as_str().expect("validation state");
                assert!(matches!(state, "valid" | "invalid"));
                if state == "invalid" {
                    assert!(
                        validation["message"]
                            .as_str()
                            .expect("validation message")
                            .len()
                            <= 512
                    );
                }
            }
        }
        assert_safe_strings(node);
    }
    assert_eq!(roots, 1);
    assert_eq!(nodes[0]["kind"], "page");
    assert!(nodes[0].get("parentId").is_none());

    for orders in sibling_orders.values() {
        let expected =
            (0_u64..u64::try_from(orders.len()).expect("orders length")).collect::<BTreeSet<_>>();
        assert_eq!(orders, &expected);
    }

    let containers = [
        "page",
        "section",
        "stack",
        "group",
        "form",
        "list",
        "list-item",
    ];
    for (node_id, parent) in &parent_by_id {
        if let Some(parent_id) = parent {
            let parent_kind = kind_by_id.get(parent_id).expect("parent kind");
            let child_kind = kind_by_id.get(node_id).expect("child kind");
            assert!(
                allowed_children(parent_kind).contains(&child_kind.as_str()),
                "invalid parent/child pair {parent_kind}/{child_kind}"
            );
        }
        if kind_by_id
            .get(node_id)
            .is_some_and(|kind| kind == "list-item")
        {
            let direct_parent = parent.as_ref().expect("list-item parent");
            assert_eq!(kind_by_id.get(direct_parent).expect("list parent"), "list");
        }
        if kind_by_id.get(node_id).is_some_and(|kind| kind == "form") {
            let mut ancestor = parent.as_ref();
            while let Some(parent_id) = ancestor {
                assert_ne!(kind_by_id.get(parent_id).expect("ancestor kind"), "form");
                ancestor = parent_by_id.get(parent_id).expect("ancestor").as_ref();
            }
        }
        let mut depth = 1_u8;
        let mut cursor = parent.as_ref();
        while let Some(parent_id) = cursor {
            depth += 1;
            assert!(depth <= 8);
            let parent_kind = kind_by_id.get(parent_id).expect("parent kind");
            assert!(containers.contains(&parent_kind.as_str()));
            cursor = parent_by_id.get(parent_id).expect("parent record").as_ref();
        }
        if ["switch", "text-field", "integer-field", "select"]
            .contains(&kind_by_id.get(node_id).expect("node kind").as_str())
        {
            let mut form_count = 0_u8;
            let mut ancestor = parent.as_ref();
            while let Some(parent_id) = ancestor {
                if kind_by_id.get(parent_id).is_some_and(|kind| kind == "form") {
                    form_count += 1;
                }
                ancestor = parent_by_id.get(parent_id).expect("ancestor").as_ref();
            }
            assert_eq!(form_count, 1);
        }
    }

    DocumentSummary {
        node_count: nodes.len(),
        kinds,
        action_ids,
        field_ids,
    }
}

fn assert_safe_strings(value: &Value) {
    match value {
        Value::String(text) => {
            assert!(text.len() <= 4_096);
            assert!(!text.chars().any(is_forbidden_character));
        }
        Value::Array(values) => {
            for value in values {
                assert_safe_strings(value);
            }
        }
        Value::Object(values) => {
            for value in values.values() {
                assert_safe_strings(value);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn is_forbidden_character(character: char) -> bool {
    matches!(
        character,
        '\0'..='\u{0009}'
            | '\u{000B}'..='\u{001F}'
            | '\u{007F}'..='\u{009F}'
            | '\u{061C}'
            | '\u{200E}'
            | '\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2066}'..='\u{2069}'
    )
}

fn is_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase()
                || (index > 0
                    && (byte.is_ascii_digit() || byte == b'.' || byte == b'_' || byte == b'-'))
        })
}

fn allowed_children(parent_kind: &str) -> &'static [&'static str] {
    const PAGE: &[&str] = &[
        "section",
        "stack",
        "group",
        "form",
        "list",
        "text",
        "status",
        "key-value",
        "progress",
        "divider",
        "button",
    ];
    const FLEX: &[&str] = &[
        "section",
        "stack",
        "group",
        "form",
        "list",
        "text",
        "status",
        "key-value",
        "progress",
        "divider",
        "button",
        "switch",
        "text-field",
        "integer-field",
        "select",
    ];
    const FORM: &[&str] = &[
        "section",
        "stack",
        "group",
        "text",
        "status",
        "key-value",
        "progress",
        "divider",
        "switch",
        "text-field",
        "integer-field",
        "select",
    ];
    const LIST: &[&str] = &["list-item"];
    const LIST_ITEM: &[&str] = &[
        "section",
        "stack",
        "group",
        "form",
        "list",
        "text",
        "status",
        "key-value",
        "progress",
        "divider",
        "button",
    ];

    match parent_kind {
        "page" => PAGE,
        "section" | "stack" | "group" => FLEX,
        "form" => FORM,
        "list" => LIST,
        "list-item" => LIST_ITEM,
        _ => &[],
    }
}

fn vector_ids(value: &Value) -> BTreeSet<&str> {
    value
        .as_array()
        .expect("vector array")
        .iter()
        .map(|item| item["id"].as_str().expect("vector id"))
        .collect()
}

fn btreeset<const N: usize>(values: [&str; N]) -> BTreeSet<String> {
    values.into_iter().map(str::to_owned).collect()
}
