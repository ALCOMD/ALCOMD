use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

const WEBVIEW_EVIDENCE: &str = include_str!("../../../specs/gui/m7-stop-a.md");
const MANIFEST_PROPOSAL: &str =
    include_str!("../../../specs/extensions/proposals/manifest-v1-portable-ui.schema.json");
const PACKAGE_PROPOSAL: &str =
    include_str!("../../../specs/extensions/proposals/package-profile-v1-portable-ui.json");
const ABI_PROPOSAL: &str =
    include_str!("../../../specs/extensions/proposals/abi-compatibility-v1-portable-ui.json");
const PORTABLE_SCHEMA: &str = include_str!("../../../specs/extensions/portable-ui-v1.schema.json");
const LIMITS: &str = include_str!("../../../specs/extensions/portable-ui-limits-v1.json");
const RPC_PROPOSAL: &str = include_str!("../../../specs/rpc/m7-portable-ui.schema.json");
const STATE_PROPOSAL: &str =
    include_str!("../../../specs/storage/state-v9-migration.contract.proposal.json");
const PERMISSION_PROPOSAL: &str =
    include_str!("../../../specs/extensions/proposals/permissions-portable-ui-v1.md");
const HOST_PROTOCOL_PROPOSAL: &str =
    include_str!("../../../specs/extensions/proposals/host-protocol-invocation-context-v1.md");
const ACTIVE_WORLD: &str = include_str!("../../../specs/extensions/wit/extension-v1/world.wit");
const PROPOSAL_TYPES: &str =
    include_str!("../../../specs/extensions/wit/extension-v1-portable-ui-proposal/types.wit");
const PROPOSAL_LIFECYCLE: &str = include_str!(
    "../../../specs/extensions/wit/extension-v1-portable-ui-proposal/guest-lifecycle.wit"
);
const PROPOSAL_UI: &str =
    include_str!("../../../specs/extensions/wit/extension-v1-portable-ui-proposal/guest-ui.wit");
const PROPOSAL_WORLD: &str =
    include_str!("../../../specs/extensions/wit/extension-v1-portable-ui-proposal/world.wit");
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
fn manifest_and_package_proposals_are_direct_rewrites_without_web_assets() {
    let manifest: Value = serde_json::from_str(MANIFEST_PROPOSAL).expect("Manifest proposal");
    let package: Value = serde_json::from_str(PACKAGE_PROPOSAL).expect("package proposal");

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
fn wit_proposal_keeps_abi_major_one_and_active_world_untouched() {
    let abi: Value = serde_json::from_str(ABI_PROPOSAL).expect("ABI proposal");
    assert_eq!(abi["abiMajor"], 1);
    assert_eq!(abi["world"], "alcomd:extension/extension-v1@1.0.0");
    assert_eq!(abi["ambientWasiImports"], json!([]));
    assert_eq!(abi["hostEmbedding"], "async");
    assert_eq!(abi["witFunctions"], "sync");
    assert_eq!(abi["componentModelAsync"], false);
    assert_eq!(abi["wasmtimeWasi"], false);
    assert_eq!(
        abi["preReleaseDirectRewrite"]["parallelWorldRetained"],
        false
    );

    assert!(!ACTIVE_WORLD.contains("guest-ui"));
    assert!(PROPOSAL_WORLD.contains("world extension-v1"));
    assert!(PROPOSAL_WORLD.contains("import host-projects;"));
    assert!(PROPOSAL_WORLD.contains("import host-data;"));
    assert!(PROPOSAL_WORLD.contains("export guest-lifecycle;"));
    assert!(PROPOSAL_WORLD.contains("export guest-ui;"));
    for function in [
        "open: func",
        "refresh: func",
        "dispatch: func",
        "close: func",
    ] {
        assert!(
            PROPOSAL_UI.contains(function),
            "missing guest-ui {function}"
        );
    }
    assert!(PROPOSAL_TYPES.contains("enum activation-kind"));
    assert!(PROPOSAL_TYPES.contains("background,"));
    assert!(PROPOSAL_TYPES.contains("interactive-ui,"));
    assert!(PROPOSAL_TYPES.contains("interactive-ui-idle,"));
    assert!(PROPOSAL_LIFECYCLE.contains("activate: func"));
    for wit in [
        PROPOSAL_TYPES,
        PROPOSAL_LIFECYCLE,
        PROPOSAL_UI,
        PROPOSAL_WORLD,
    ] {
        assert!(!wit.contains("wasi:"));
    }
}

#[test]
fn rpc_state_permissions_and_limits_are_exact_and_closed() {
    let schema: Value = serde_json::from_str(PORTABLE_SCHEMA).expect("Portable UI Schema");
    let limits: Value = serde_json::from_str(LIMITS).expect("limits");
    let rpc: Value = serde_json::from_str(RPC_PROPOSAL).expect("RPC proposal");
    let state: Value = serde_json::from_str(STATE_PROPOSAL).expect("State proposal");

    assert_eq!(limits["surfacesPerExtension"], 1);
    assert_eq!(limits["surfaceId"], "main");
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
    assert_eq!(
        rpc["$defs"]["uiDeclaration"]["properties"]["surfaces"]["maxItems"],
        1
    );
    for code in [
        "extension_not_enabled",
        "extension_ui_not_available",
        "extension_ui_surface_not_found",
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
    }

    assert_eq!(state["from"], 8);
    assert_eq!(state["to"], 9);
    assert!(state["productionMigration"].is_null());
    assert_eq!(state["tablesAdded"], json!([]));
    assert_eq!(state["surfacesPerExtension"], 1);
    assert_eq!(
        state["planFieldsImmutable"],
        json!(["ui_protocol", "ui_surfaces_json"])
    );
    assert!(PERMISSION_PROPOSAL.contains("`extensions.ui.use`"));
    assert!(PERMISSION_PROPOSAL.contains("不新增 `ui.contribute`"));
    assert!(PERMISSION_PROPOSAL.contains("Client Principal 等价业务"));
    assert!(PERMISSION_PROPOSAL.contains("permission/scope 的交集"));
    assert!(HOST_PROTOCOL_PROPOSAL.contains("InvocationContextId"));
    assert!(HOST_PROTOCOL_PROPOSAL.contains("invocation_context_stale"));
    assert!(HOST_PROTOCOL_PROPOSAL.contains("unknown ID"));
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
    ] {
        assert!(guest.contains(required), "missing guest vector {required}");
    }
    for required in [
        "snapshot-stale",
        "unknown-action",
        "disabled-action",
        "missing-form-field",
        "wrong-field-type",
        "unknown-select-option",
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
    assert!(replay.contains("duplicate-live-pair"));
    assert!(replay.contains("connection-lost-before-response"));

    for case in vectors["guestDocumentCases"]
        .as_array()
        .expect("guest cases")
    {
        assert_eq!(case["closeSession"], true);
        assert_eq!(case["terminateHost"], true);
        assert_eq!(case["countCrash"], true);
    }
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
}

struct DocumentSummary {
    node_count: usize,
    kinds: BTreeSet<String>,
    action_ids: BTreeSet<String>,
    field_ids: BTreeSet<String>,
}

fn validate_document(document: &Value) -> DocumentSummary {
    assert_eq!(document["protocol"], "portable-v1");
    assert_eq!(document["surfaceId"], "main");
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
        if kind_by_id
            .get(node_id)
            .is_some_and(|kind| kind == "list-item")
        {
            let direct_parent = parent.as_ref().expect("list-item parent");
            assert_eq!(kind_by_id.get(direct_parent).expect("list parent"), "list");
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
            byte.is_ascii_lowercase() || (index > 0 && (byte.is_ascii_digit() || byte == b'-'))
        })
        && !value.ends_with('-')
        && !value.contains("--")
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
