# ADR: Portable Extension UI

- 状态：Accepted pre-release product direction；M7 Stop A contract candidate frozen for owner review；implementation pending
- 日期：2026-08-25
- Supersedes：ADR 0007 中未发布的 Web UI contribution/container 部分

## 背景

M6 已完成统一 Extension Runtime、WASM/WASI Component Host、Principal、grant/scope、lease/revocation、生命周期、
数据隔离、崩溃隔离和 first-party/third-party parity。M6 同时冻结过一套未发布的 packaged static Web UI、
`ui_entry`、logical Web origin 和 headless UI Bridge 候选，M7 随后研究了 sandboxed cross-origin iframe 与
Tauri managed child WebView。

项目仍处于公开发布前，没有公开 Extension ABI 用户、第三方扩展、第三方 GUI 适配或必须保留的真实数据兼容
包袱。WebView 研究已经证明当前两条物理容器路径不适合作为 M7 v1 基础方向，但没有产生可以提升为 production
security contract 的证据。继续寻找 iframe、child WebView 或 WebviewWindow 容器会把官方 GUI 技术栈错误地
固化为 Core Extension 标准。

## 决策

ALCOMD Extension 是 Core Extension，不是 `alcomd-gui` plugin。Extension Runtime compatibility 与 Extension UI
compatibility 分开版本化和协商。

Extension Backend 继续运行于统一的 `alcomd-extension-host`。扩展 UI 改为 GUI-neutral Portable Extension UI
Surface：扩展只提供 bounded semantic UI tree、state 和 typed action，不携带 HTML、CSS、JavaScript、React、
Tauri 或其他 GUI framework code。

```text
Extension Backend
    -> alcomd-extension-host
    -> Portable Extension UI Surface
    -> alcomd application
    -> ALCOMD RPC v1
        -> alcomd-gui renderer
        -> third-party GUI renderer
        -> headless conformance client
```

`alcomd-gui` 是第一个 renderer，而不是 Extension UI 标准。官方 GUI 在主窗口内容区使用 React、Material Design 3
和自身可访问性体系渲染；不打开额外窗口，不加载扩展网页，不创建 iframe、child WebView 或 WebviewWindow，
也不向扩展授予 Tauri capability。第三方 GUI 使用同一 RPC Surface 和自己的原生组件/设计系统。

GUI 以 `extensions.ui.portable.v1` capability 表达完整 v1 支持，不协商 hostId、renderer identity 或 feature 子集。GUI
不支持 Portable UI 或不提供扩展页面时，只影响功能页面；扩展安装、启用、后台运行、权限、数据和生命周期不依赖
renderer。unknown v1 node fail closed；未来节点使用新 capability/version。

UI Session 和 action 必须同时验证 client Principal authority 与 extension Principal grant/scope/lease，避免 GUI 作为
confused deputy。GUI 只能经 RPC -> `alcomd-application` -> Extension Host；不建立 GUI 到 Host 的直连，也不允许
first-party extension 使用 private React page、private Tauri command、hidden renderer、额外权限或专用 Surface。

Custom Web UI、GUI-specific code surface 和其他高级 Surface 延后到出现真实用例后独立设计，不为它们保留 Web
fallback、兼容字段或双路 renderer。M8 MCP management extension 与 M9 Discord extension 使用同一个 Portable UI。

由于合同尚未公开，implementation 获批后可直接替换 Manifest/package/WIT/ABI v1 与未发布 UI-specific 实现，不创建
v2、alias、dual parser/world/renderer。M6 已真实广告并验收 State Schema v8，因此 Portable UI normalized declaration
使用 v9 proposal；它不恢复旧 Web UI compatibility，开发数据库仍可 reset。

M7 Stop A candidate 进一步冻结：单一隐式 `main` surface；daemon-owned SnapshotRevision 与 memory-only session；17 种
bounded semantic node；只有 activate/submit-form 两种 action；Client `extensions.ui.use` 与 Extension grant/scope 的
双重授权；daemon-issued InvocationContextId；`background`/`interactive-ui` activation；四个 RPC method；官方 MD3 与
non-Tauri headless 两个 consumer contract。所有内容仍是 proposal，必须经下一次人工审批才可替换 active contract或
开始 production。

## 结果

- M6 backend runtime acceptance 保持有效，不重新宣布 M6 失败或未完成。
- packaged static `ui/`、`ui_entry`、HTML/JavaScript contribution、Web UI Bridge physical/container、logical Web origin
  authority 与 headless Web UI ping 作为最终 GUI 模型的假设被 supersede。
- M7 不再以 WebView physical mapping、custom scheme/CSP 或 Tauri IPC negative matrix 作为 product blocker。
- Portable UI Stop A proposal、Schema/WIT proposal、fixtures 与 contract tests 已形成；active contract、Host/Core/React
  implementation 与 capability advertising 均未开始。
