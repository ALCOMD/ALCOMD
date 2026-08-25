# M7 WebView Stop A historical evidence

状态：**superseded by Portable Extension UI direction reset**

本文件保留旧 M7 WebView Stop A 的研究事实和失败分类。它不再是 active proposal，不发布 RPC、Permission、
State Schema、Tauri capability、physical URL、Web UI Bridge 或 Extension UI container contract，也不授权
production implementation。

新的 M7 方向与下一审批点见 `docs/exec-plans/M7-official-gui-extension-ui.md`：Portable UI contract-first Stop A。

## 1. Sandboxed cross-origin iframe evidence

最终用于架构判断的 test-only Hosted CI run：`32752875840`。

- Windows Server 2025、Ubuntu 22.04、macOS 15 arm64 均成功创建主 WebView。
- extension document 在三个环境均超时。
- custom protocol handler 未触发。
- 没有取得能够证明 iframe 中 private Tauri IPC presence 或 denial 的完整 evidence matrix。

分类与结论：

```text
sandboxed_cross_origin_iframe = rejected_for_m7_v1
```

该结论只说明当前 iframe + physical mapping 不可实施。不得据此声称 private IPC security 已通过，也不得声称
private IPC security 已被证明失败。旧 physical scheme、custom origin、CSP、sandbox 和 asset serving proposal
没有升级为 production contract。

## 2. Tauri managed child WebView evidence

Windows-only local diagnostic 环境：

```text
Tauri = 2.11.5
WebView2 Runtime = 151.0.4129.101
API = Window::add_child
initial URL = WebviewUrl::App("m7-child-control.html")
```

实际结果：

- `Window::add_child` 返回成功；
- `on_navigation` 未触发；
- `on_page_load` 未触发；
- document title callback 未触发；
- `Webview::url()` 返回 `runtime error: failed to receive message from webview`；
- `eval_with_callback` 调用被接受，但没有 callback；
- 未取得 `document.readyState`、title 或 marker。

分类与结论：

```text
child_webview_navigation_unavailable
isolated_managed_child_webview = rejected_for_m7_v1
```

Stage 1 失败后按门禁没有执行 post-attach custom protocol navigation，也没有执行 initial
`WebviewUrl::CustomProtocol`。没有运行 Ubuntu/macOS child probe。

## 3. 明确取消的 WebView 后续方向

M7/4.0.0 基础 Extension UI 不再继续：

- final iframe/child/WebviewWindow container selection；
- physical custom scheme/origin/CSP/asset mapping；
- Hosted WebView isolation evidence 作为产品 blocker；
- Tauri unstable production adoption 或精确 pin 作为 Extension UI 方案；
- Stage 2/3 child custom protocol；
- Ubuntu/macOS child probe；
- WebviewWindow candidate；
- direct Wry、native platform API 或 WebView2 COM；
- 新的 iframe/child/WebviewWindow container search。

已经推送的 probe 和 CI 历史保留，不 force push、amend 或删除历史提交。test-only evidence 只能标记为 rejected
design evidence，不能标记为 product security、compatibility 或 production implementation success。

## 4. 没有 production evidence

WebView 研究没有修改或冻结 production Manifest、package profile、WIT、UI Bridge、Host protocol、RPC、Permission、
State Schema、Tauri capability 或 dependency policy。没有 Extension UI WebView production wiring，也没有第一方或
第三方 Web UI compatibility 承诺。

旧 Stop A 中的 route/settings/activity/typed adapter 草案仅作为历史 planning input；Portable UI 方向下如仍有真实
需求，必须在新的 Stop A 重新提出、冻结并人工审批，不能从本文件推导授权。

## 5. 新方向

基础 Extension UI 改为：

```text
Extension Backend
    -> alcomd-extension-host
    -> Portable Extension UI Surface
    -> alcomd application
    -> ALCOMD RPC v1
        -> official GUI renderer
        -> third-party GUI renderer
        -> headless conformance client
```

本文件不定义 Portable UI Schema。下一审批点是 `docs/exec-plans/M7-official-gui-extension-ui.md` 中列出的
Portable UI contract-first Stop A。
