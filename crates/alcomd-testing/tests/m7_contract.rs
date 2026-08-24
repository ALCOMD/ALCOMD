use std::collections::BTreeSet;

use serde_json::Value;

const STOP_A: &str = include_str!("../../../specs/gui/m7-stop-a.md");
const SETTINGS: &str =
    include_str!("../../../specs/gui/m7-settings-appearance-v1.proposal.schema.json");
const VECTORS: &str = include_str!("../fixtures/m7/stop-a-vectors.json");
const DOMAIN: &str = include_str!("../../alcomd-domain/src/lib.rs");
const PROBE_RUST: &str =
    include_str!("../../../apps/alcomd-gui/src-tauri/examples/m7_isolation_probe.rs");
const PROBE_CONFIG: &str =
    include_str!("../../../apps/alcomd-gui/src-tauri/tauri.m7-probe.conf.json");
const PROBE_HOST: &str = include_str!("../../../apps/alcomd-gui/m7-probe-dist/m7-probe-host.html");

#[test]
fn stop_a_keeps_permissions_and_production_boundary_narrow() {
    let vectors: Value = serde_json::from_str(VECTORS).expect("M7 Stop A vectors must be JSON");
    assert_eq!(vectors["activity"]["newPermission"], Value::Null);
    assert_eq!(vectors["diagnostics"]["permission"], Value::Null);
    assert_eq!(vectors["diagnostics"]["rpc"], Value::Null);
    assert!(!DOMAIN.contains("ActivityRead"));
    assert!(!DOMAIN.contains("DiagnosticsRead"));
    assert!(STOP_A.contains("不是 production contract"));
    assert!(STOP_A.contains("不新增 `activity.read`"));
    assert!(STOP_A.contains("不提出或预批准 `diagnostics.read`"));
}

#[test]
fn route_and_tauri_surfaces_are_closed_and_unique() {
    let vectors: Value = serde_json::from_str(VECTORS).expect("M7 Stop A vectors must be JSON");
    let routes = vectors["routes"].as_array().expect("routes array");
    let identities = routes
        .iter()
        .map(|route| route["identity"].as_str().expect("route identity"))
        .collect::<BTreeSet<_>>();
    assert_eq!(identities.len(), routes.len());
    assert_eq!(routes.len(), 18);
    assert_eq!(
        vectors["tauriCommands"],
        serde_json::json!(["gui_query", "gui_command", "gui_select_path"])
    );
    assert_eq!(vectors["extensionPlacement"], "/extensions/:extensionId/ui");
    assert!(STOP_A.contains("不存在 `method: string`"));
    assert!(STOP_A.contains("extension WebView/frame 不匹配任何 Tauri capability"));
}

#[test]
fn settings_proposal_is_exact_but_not_published() {
    let schema: Value =
        serde_json::from_str(SETTINGS).expect("settings proposal schema must be JSON");
    assert_eq!(
        schema["x-alcomd-proposal"]["persistence"],
        "state.db Schema v9 singleton row"
    );
    assert_eq!(
        schema["x-alcomd-proposal"]["capability"],
        "settings.appearance.v1"
    );
    assert_eq!(
        schema["x-alcomd-proposal"]["methodPermissions"]["settings.appearance.get"],
        serde_json::json!(["settings.read"])
    );
    assert_eq!(
        schema["x-alcomd-proposal"]["localStorageMaxUtf8Bytes"],
        4096
    );
    assert_eq!(
        schema["$defs"]["themeSourceColor"]["pattern"],
        "^#[0-9a-f]{6}$"
    );
    assert_eq!(
        schema["$defs"]["updateRequest"]["additionalProperties"],
        false
    );
    assert!(
        schema["description"]
            .as_str()
            .expect("description")
            .contains("not a published RPC")
    );
}

#[test]
fn isolation_vectors_cover_every_owner_required_negative_case() {
    let vectors: Value = serde_json::from_str(VECTORS).expect("M7 Stop A vectors must be JSON");
    let cases = vectors["isolationNegativeCases"]
        .as_array()
        .expect("negative cases")
        .iter()
        .map(|case| case.as_str().expect("negative case"))
        .collect::<BTreeSet<_>>();
    let required = [
        "window.__TAURI__",
        "window.__TAURI_INTERNALS__",
        "raw-tauri-invoke",
        "tauri-event",
        "tauri-channel",
        "parent-opener-access",
        "postmessage-confused-deputy",
        "main-dom-access",
        "daemon-socket",
        "filesystem",
        "clipboard",
        "notification",
        "network",
        "top-level-navigation",
    ];
    for required_case in required {
        assert!(cases.contains(required_case), "missing {required_case}");
    }
    assert_eq!(
        vectors["actualWebviewEvidence"]["containerChoice"],
        "not-yet-frozen"
    );
}

#[test]
fn test_only_probe_has_no_production_capability_and_uses_physical_candidate() {
    let config: Value = serde_json::from_str(PROBE_CONFIG).expect("probe config must be JSON");
    assert_eq!(
        config["app"]["security"]["capabilities"],
        serde_json::json!([
            {
                "identifier": "m7-probe-main-only",
                "description": "Test-only positive control for the M7 WebView isolation probe.",
                "windows": ["m7-isolation-probe"],
                "permissions": ["core:app:allow-name"]
            }
        ])
    );
    assert_eq!(config["app"]["windows"], serde_json::json!([]));
    assert!(PROBE_RUST.contains("register_uri_scheme_protocol(\"alcomd-extension-ui\""));
    assert!(PROBE_RUST.contains("use_https_scheme(true)"));
    assert!(PROBE_RUST.contains("HeaderValue::from_static(\"no-store\")"));
    assert!(PROBE_HOST.contains("sandbox=\"allow-scripts\""));
    assert!(!PROBE_HOST.contains("allow-same-origin"));
    assert!(!PROBE_HOST.contains("allow-top-navigation"));
}
