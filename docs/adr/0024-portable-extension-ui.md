# ADR: Portable Extension UI

- 状态：Accepted as pre-release product direction; contract and implementation pending
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

GUI 不支持 Portable UI、只支持部分 required feature，或不提供扩展页面时，只影响该扩展的功能页面；扩展安装、
启用、后台运行、权限、数据和生命周期不依赖 GUI renderer。renderer 不得静默丢弃 Surface 声明的 required feature。

UI Session 和 action 必须同时验证 client Principal authority 与 extension Principal grant/scope/lease，避免 GUI 作为
confused deputy。GUI 只能经 RPC -> `alcomd-application` -> Extension Host；不建立 GUI 到 Host 的直连，也不允许
first-party extension 使用 private React page、private Tauri command、hidden renderer、额外权限或专用 Surface。

Custom Web UI、GUI-specific code surface 和其他高级 Surface 延后到出现真实用例后独立设计，不为它们保留 Web
fallback、兼容字段或双路 renderer。M8 MCP management extension 与 M9 Discord extension 使用同一个 Portable UI。

由于合同尚未公开，下一轮 contract-first 可以直接替换 Manifest v1、package profile v1、Extension WIT/ABI v1、
UI contract v1、未发布的 State Schema v8 及 M6 UI-specific 实现/测试。不创建 Manifest/ABI v2、deprecated
`ui_entry`、old/new parser/world alias、UI Bridge compatibility alias、State v9 compatibility migration 或旧开发
数据库迁移；需要时开发者清空本地 `state.db` 后重新初始化。

本 ADR 只接受产品方向。Portable UI Snapshot/Node/Action Schema、UI Session、Host protocol、RPC method/capability/error、
client permission、State v8 调整、renderer contract、third-party GUI descriptor、threat model、资源限制和 conformance
tests 必须在 M7 Portable UI contract-first Stop A 单独冻结并再次人工审批。

## 结果

- M6 backend runtime acceptance 保持有效，不重新宣布 M6 失败或未完成。
- packaged static `ui/`、`ui_entry`、HTML/JavaScript contribution、Web UI Bridge physical/container、logical Web origin
  authority 与 headless Web UI ping 作为最终 GUI 模型的假设被 supersede。
- M7 不再以 WebView physical mapping、custom scheme/CSP 或 Tauri IPC negative matrix 作为 product blocker。
- Portable UI 与 renderer 的实际合同和实现仍未开始；任何候选字段、节点、权限和 RPC 名称都不是已发布合同。
