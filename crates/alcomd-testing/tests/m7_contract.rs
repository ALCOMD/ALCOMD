const WEBVIEW_EVIDENCE: &str = include_str!("../../../specs/gui/m7-stop-a.md");

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
