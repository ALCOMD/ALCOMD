# M7 retired WebView design evidence

状态：**superseded；test-only harness 已退役**

M7 曾用 test-only Tauri harness 评估 sandboxed cross-origin iframe 与 managed child WebView。两条候选均在
production 前退出 M7 v1 产品方向；相关脚本、assets、example、配置和 CI runtime probe 已删除，普通 Tauri GUI
build / no-bundle gate 保持不变。

历史事实、CI run、失败分类和“不得冒充 production security success”的边界只由
`specs/gui/m7-stop-a.md` 维护。本文不再描述可运行 harness、physical mapping 或 machine-readable result，也不作为
`gui.m7-webview-isolation` 的 metadata evidence。

新的产品方向和下一审批点见 `docs/exec-plans/M7-official-gui-extension-ui.md` 中的 Portable UI contract-first Stop A。
