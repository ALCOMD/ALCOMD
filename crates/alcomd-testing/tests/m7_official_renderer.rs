use std::path::PathBuf;
use std::process::Command;

const RENDERER: &str = include_str!("../../../apps/alcomd-gui/src/PortableUiRenderer.tsx");
const APP: &str = include_str!("../../../apps/alcomd-gui/src/App.tsx");
const RPC: &str = include_str!("../../../apps/alcomd-gui/src/rpc.ts");
const STYLE: &str = include_str!("../../../apps/alcomd-gui/src/styles.css");
const TAURI_CONFIG: &str = include_str!("../../../apps/alcomd-gui/src-tauri/tauri.conf.json");

#[test]
fn official_renderer_executes_shared_portable_ui_semantics() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf();
    let test = workspace.join("apps/alcomd-gui/src/portable-ui.test.ts");
    let output = Command::new("node")
        .arg("--experimental-strip-types")
        .arg(test)
        .current_dir(&workspace)
        .output()
        .expect("Node 24 must be installed by the repository setup gate");
    assert!(
        output.status.success(),
        "Portable UI semantics failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn official_renderer_exhaustively_maps_v1_without_private_authority() {
    for kind in [
        "page",
        "section",
        "stack",
        "group",
        "form",
        "list",
        "list-item",
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
    ] {
        assert!(
            RENDERER.contains(&format!("case \"{kind}\"")),
            "missing renderer for {kind}"
        );
    }
    assert!(RENDERER.contains("aria-describedby"));
    assert!(RENDERER.contains("aria-invalid"));
    assert!(RENDERER.contains("assertNever(node)"));
    assert!(!RENDERER.contains("dangerouslySetInnerHTML"));
    assert!(!APP.contains("localStorage"));
    assert!(!APP.contains("sessionStorage"));
    assert!(!APP.contains("alcomd-extension-host"));
    assert!(!RPC.contains("state.db"));
    for command in [
        "gui_extension_get",
        "gui_extension_ui_open",
        "gui_extension_ui_refresh",
        "gui_extension_ui_dispatch",
        "gui_extension_ui_close",
    ] {
        assert!(RPC.contains(command), "missing typed adapter {command}");
    }
    assert!(!RENDERER.contains("@tauri-apps/api"));
    assert!(!RENDERER.contains("invoke("));
    assert!(!RPC.contains("gui_extension_host"));
}

#[test]
fn official_shell_preserves_host_owned_accessibility_and_responsive_boundaries() {
    assert!(APP.contains("Extension-provided content"));
    assert!(APP.contains("Host-verified extension identity"));
    assert!(!APP.contains("window.confirm("));
    assert!(APP.contains("<MaterialDialog"));
    assert!(APP.contains("Discard unsaved changes?"));
    assert!(APP.contains("Keep editing"));
    assert!(APP.contains("Discard changes"));
    assert!(APP.contains("applyAppearance"));
    assert!(APP.contains("client.settingsGet()"));
    assert!(APP.contains("sourceColorName"));
    assert!(!RENDERER.contains("@alcomd/ui"));
    assert!(!RENDERER.contains("applyAppearance"));
    assert!(STYLE.contains("min-width: 320px"));
    assert!(STYLE.contains("prefers-reduced-motion: reduce"));
    assert!(STYLE.contains(":focus-visible"));
    assert!(STYLE.contains("prefers-color-scheme: dark"));
    assert!(TAURI_CONFIG.contains("\"minWidth\": 320"));
}
