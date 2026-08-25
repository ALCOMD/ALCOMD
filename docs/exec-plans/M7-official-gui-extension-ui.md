# M7：官方 GUI 与 Portable Extension UI

状态：M7 architecture direction reset in progress；WebView-based Extension UI 已在 production 前拒绝；Portable UI contract-first 尚未开始；M7 production implementation 未开始

## 目标

在 M1-M6 已完成的 daemon、RPC、Operation、Event、项目/仓库、VPM transaction、Unity、模板、备份与
Extension Runtime 基础上，把 M0 GUI 壳收敛为官方客户端，并建立与 GUI 技术栈无关的 Portable Extension UI：

```text
React / Material Design 3 official shell
    -> typed Tauri adapter
        -> alcomd-client
            -> ALCOMD RPC v1
                -> alcomd-application

Extension Backend
    -> alcomd-extension-host
    -> Portable Extension UI Surface
    -> alcomd application
    -> ALCOMD RPC v1
        -> official React / Material Design 3 renderer
        -> third-party GUI renderer
        -> headless conformance client
```

ALCOMD Extension 是 Core Extension，不是 `alcomd-gui` plugin。Extension Runtime compatibility 与 Extension UI
compatibility 相互独立。Portable UI 只携带 bounded semantic UI tree、state 和 typed action，不携带 HTML、CSS、
JavaScript、React、Tauri 或其他 GUI framework code。`alcomd-gui` 是第一个 renderer，不是 Extension UI 标准。

M7 不建立第二个业务 authority。GUI 不直接读取 `state.db`、项目文件、repository、package cache、extension-owned
data 或 Extension Host private protocol；所有业务结果、revision、Plan、Operation、权限和错误以 daemon 为准。

## 前置条件与本轮边界

- M6 最终候选 `b666e8fcac6fd4153750c401b76f2f61f6d2a34a` 与 CI run `32724915827` 已通过三平台
  Hosted CI 和项目所有者最终人工验收；M6 backend runtime acceptance 保持有效。
- M6 acceptance closure `577af7d4cd5746ea259c047e9b80189ca698db70` 对应 CI run `32733276048` 三平台成功。
- M1-M6 RPC v1 继续只能兼容增加；下一轮对未发布 Extension UI-specific v1 合同的直接替换必须在统一
  Portable UI Stop A 明确冻结，不能从本计划候选直接推导 production 授权。
- 当前没有公开 Extension ABI 用户、第三方扩展、第三方 GUI 适配或需保留的真实数据兼容包袱；获批后允许直接
  替换未发布合同，不设计 v2/compatibility layer/dual parser/dual renderer/旧开发数据库 migration。
- Phase 0 只修改架构、ADR、ExecPlan、状态和必要 metadata；不修改 Manifest/WIT/RPC/Permission/State Schema、
  production Rust/TypeScript、dependency、lockfile、测试实现、Tauri capability、CI 或平台 API。

## M6 保持有效与被替代的边界

以下 M6 能力继续有效，不重新宣布 M6 整体失败或未完成：

- Extension package/signature validation；
- Wasmtime/WASI Component Host；
- one ExtensionId per Host OS process；
- Principal、grant/scope、`ExtensionInstanceLease` 与 revocation；
- lifecycle、quarantine、extension-owned data 与 crash isolation；
- first-party/third-party parity。

以下未发布的 UI-specific pre-release contract 被 Portable UI 方向 supersede：

- packaged static `ui/` assets 与 `ui_entry`；
- HTML/CSS/JavaScript UI contribution；
- Web UI Bridge physical/container assumptions；
- logical Web origin 作为 UI authority；
- headless Web UI ping 作为最终 GUI integration model。

不建立 deprecated `ui_entry`、Manifest/ABI v2、old/new parser/world alias、UI Bridge compatibility alias、Web UI
fallback、双路 renderer 或 State v8 -> v9 compatibility migration。开发期不兼容数据库允许清空后重新初始化。

## 产品体验与 GUI renderer 边界

官方 GUI 保持单一主窗口 shell。核心页面继续规划为首页、项目、包、仓库、模板、Unity、备份、Operation、扩展、
设置、活动和关于；尚未存在的 external access、technical diagnostics、migration、updater 和 distribution 用例不以
占位成功页面冒充实现。

Portable Extension UI 的官方 route 候选为：

```text
/extensions/:extensionId/ui/:surfaceId
```

页面位于主窗口内容区，由官方 React/Material Design 3 组件渲染。官方 GUI 负责 layout、theme、typography、shape、
spacing、density、motion、focus、keyboard、ARIA/accessibility，以及 loading/empty/error/progress/disconnected state。
扩展只能提供语义内容和 typed action；不能提供 CSS、颜色、字体、absolute positioning、animation 或 GUI component。

因此 M7 基础 Extension UI：

- 不打开第二窗口；
- 不创建 iframe、child WebView 或 WebviewWindow；
- 不加载第三方网页或扩展 JavaScript；
- 不授予 extension Tauri IPC/capability；
- 深色、浅色和主题色由 GUI renderer 统一控制。

GUI 不支持某个 UI Surface 时，扩展安装、启用、后台运行、权限、数据和生命周期仍正常。支持 `portable-v1` 的 GUI
使用自己的 native renderer；只支持部分 feature 的 GUI 只有在满足 Surface required features 时才显示，不能静默
丢弃关键节点；完全不支持的 GUI 仍显示标准 extension management，并把功能页面明确标为 unsupported。

## Portable UI proposed design（不是已发布合同）

以下内容只是下一轮 Stop A 的候选输入。本轮不创建或修改真实 Schema、WIT、RPC、Permission 或 State Schema。

### Manifest v1

未来候选直接删除 `ui_entry` 与 `ui/` static asset semantics，并可加入：

```toml
[ui]
protocol = "portable-v1"
```

没有 `[ui]` 的扩展仍可正常安装、启用和运行后台。不要增加 Manifest v2。

### Package profile v1

未来候选删除 `ui/` allowed root 与 UI static asset quota，把包根收窄为：

```text
alcomd-extension.toml
META-INF/
component/
```

### Extension ABI/WIT v1

未来候选直接更新当前 v1 world，增加 guest UI export；没有 UI 的扩展返回 empty surface list。不增加并行 v2
world。具体 WIT type/function、sync/async 调用、limits 和 cancellation 必须由 Stop A 冻结。

### Portable UI Snapshot/Node/Action v1

候选采用 bounded、semantic、flat node tree。候选最小 node family：

- layout：`page`、`section`、`stack`、`group`；
- display：`text`、`status`、`key-value`、`progress`、`divider`；
- input：`button`、`switch`、`text-field`、`number-field`、`select`。

明确排除 HTML、CSS、JavaScript、arbitrary Markdown HTML、Canvas、arbitrary image/network URL、custom font/color、
absolute positioning、expression language、GUI framework component 和 extension-defined animation。具体 node、field、
required feature、tree/depth/text/action/state quota 与 stable error 必须在 Stop A 冻结。

### RPC v1 compatible additions

候选 capability：

```text
extensions.ui.portable.v1
```

候选 methods：

```text
extensions.ui.listSurfaces
extensions.ui.open
extensions.ui.refresh
extensions.ui.dispatch
extensions.ui.close
```

所有调用保持 `GUI -> RPC -> alcomd-application -> Extension Host`，不建立 GUI 到 Host 直连。method/capability/DTO、
pagination、bounds、revision、cancellation 和 stable errors 尚未冻结。

### Authority 与 UI Session

候选 UI Session 至少绑定：

- `UiSessionId`；
- Client `PrincipalId`、connection/instance；
- `ExtensionId`、Extension `PrincipalId`、`ExtensionInstanceId`；
- `PackageDigest`、`SurfaceId`；
- `GrantRevision`、`LifecycleGeneration`、`SnapshotRevision`。

UI Action 同时验证 Client Principal authority 与 Extension Principal grant/scope/lease，避免 GUI 通过扩展页面成为
confused deputy。候选客户端权限为 `extensions.ui.use`，可 scope 到 ExtensionId；是否采用必须在 Stop A 审批。
提供 UI 本身不需要 extension permission，不增加 `ui.contribute`。

### State Schema v8

未来候选直接调整未发布的 State Schema v8，例如记录 `ui_protocol` 并同步 immutable install Plan。不增加 v9
compatibility migration；不兼容的开发数据库要求清空后重新初始化。具体 table/column/check/migration bootstrap 仍待
Stop A 冻结。

### Renderer portability

同一 Snapshot/Action contract 至少由两个 consumer 验证：

1. `alcomd-gui` React/Material Design 3 renderer；
2. 非 Tauri headless reference consumer。

第二个 consumer 证明协议属于 Core/RPC，而不是 `alcomd-gui` 私有 API。未来第三方 GUI 自己实现 renderer。

## M7 阶段与停止点

### Phase 0：Architecture direction reset

本轮只完成：

- 接受 ADR 0024 并把 ADR 0007 UI 部分标为 partially superseded；
- 同步 Architecture、M6/M7 ExecPlan、状态、旧 Stop A evidence 和必要 metadata；
- 记录并清理尚未提交的 test-only WebView diagnostic WIP；
- 不开始 Portable UI contract-first 或 production implementation。

### Stop A：Portable UI contract-first

下一轮必须形成并统一人工审批：

1. Manifest v1 direct rewrite；
2. package profile v1 direct rewrite；
3. Extension WIT world v1 direct rewrite；
4. Portable UI Snapshot/Node/Action Schema；
5. UI Session authority；
6. Host protocol；
7. RPC capability/methods/errors；
8. client permission；
9. dual Principal authorization；
10. State Schema v8 direct adjustment；
11. official renderer contract；
12. third-party GUI host descriptor；
13. threat model；
14. exact limits；
15. renderer/headless conformance fixtures/tests。

Stop A 通过仍不等于 production 完成；任何公共合同偏离、新 dependency、unsafe 或 platform API 都必须重新审批。

### Slice B：Core and Extension Host implementation

- 直接替换未发布的 M6 UI-specific Manifest/package/WIT/State/UI Bridge 实现与测试；
- 实现 bounded Surface discovery/open/refresh/action/close、UI Session、dual Principal authority 和 Host export；
- 不建立 GUI 到 Host 直连、第二套 business authority 或通用 UI/workflow engine。

### Slice C：Official React/MD3 renderer

- 完成 shell、typed Tauri/RPC adapter、connection/recovery state、官方 route 和 semantic node renderer；
- 使用 `@alcomd/ui`/Material Design 3 统一主题、layout、focus、keyboard、ARIA、loading/error/progress；
- 核心项目/包/仓库/模板/Unity/备份/Operation/扩展管理继续只调用已存在的 typed RPC/application use case。

### Slice D：Cross-GUI conformance proof

- official renderer 与非 Tauri headless consumer 使用同一 Snapshot/Action fixtures；
- 覆盖 required feature negotiation、unknown optional field、unsupported surface、dual Principal、revoke、stale snapshot、
  malformed/oversized tree、action replay、crash/disconnect 和 first-party/third-party parity；
- 完成本地与三平台 Hosted CI、GUI/a11y/截图签收后停止在 M8 前。

## 测试与验收规划

Portable UI contract-first 将规划：

- Schema/WIT/Manifest/package/State/RPC/permission snapshot 与 compatibility tests；
- bounded tree、duplicate ID、depth/node/text/action/state quota 和 malformed input；
- SnapshotRevision/action idempotency、stale session、revoke-in-flight、lifecycle/package replacement invalidation；
- client Principal + extension Principal 双 authority 与 confused-deputy denial；
- official React/MD3 renderer component/a11y tests；
- headless reference consumer conformance；
- first-party/third-party 使用同一 public Surface，不存在 hidden renderer/private command；
- Windows Server 2025、Ubuntu 22.04、macOS 15 arm64 的完整 repository/GUI build 和 test gate。

真实 v3 GUI、template、backup、Unity differential parity 继续 blocked 到 M11 Fixture。Windows 10/11 安装、启动、
WebView2、更新和卸载继续 deferred 到 M12。synthetic/headless fixture 不能冒充真实 v3 或客户端发行证据。

## 允许修改范围

Phase 0 只允许：

```text
docs/architecture/ALCOMD-V4.md
docs/adr/0007-extension-sandbox.md
docs/adr/0024-portable-extension-ui.md
docs/exec-plans/M6-extension-runtime.md
docs/exec-plans/M7-official-gui-extension-ui.md
docs/status.md
specs/gui/m7-stop-a.md
docs/testing/test-plan.toml           # 仅规划状态
feature-parity.toml                   # 仅影响清单/规划状态
```

Stop A 和 production 的实际修改范围必须在下一轮重新批准。本列表不授权修改 Manifest/WIT/RPC/Permission/State、
production Rust/TypeScript、Tauri capability、dependency/lockfile、test implementation、CI、unsafe 或 platform API。

## 明确排除

- Custom Web UI、GUI-specific code Surface、iframe、child WebView、WebviewWindow、direct Wry、WebView2 COM 或其他
  native WebView container search；
- M8 MCP 协议/管理产品逻辑与 M9 Discord Presence 产品逻辑；
- Local API、M10 SDK 完整硬化、M11 migration/bootstrap/updater、M12 installer/signing/dist；
- new background capability、native extension、WASI 0.3、通用 service locator、dependency injection 或 workflow engine；
- 本轮任何 Portable UI parser/session/renderer 或 React production implementation。

## WebView 研究的历史证据

### Sandboxed cross-origin iframe

CI run `32752875840` 在 Windows Server 2025、Ubuntu 22.04、macOS 15 arm64 均成功创建主 WebView，但 extension
document 超时，custom protocol handler 未触发。结论为 `rejected_for_m7_v1`。这只证明当前 iframe + physical
mapping 不可实施，不能声称 private IPC security 已通过或已被证明失败。

### Tauri managed child WebView

Windows 本地环境为 Tauri `2.11.5`、WebView2 `151.0.4129.101`。`Window::add_child` 返回成功，但已知
`WebviewUrl::App("m7-child-control.html")` 页面仍不可用：`on_navigation`、`on_page_load`、title callback 均未触发；
`Webview::url()` 返回 `runtime error: failed to receive message from webview`；`eval_with_callback` 没有回调。分类为
`child_webview_navigation_unavailable`，结论为 `isolated_managed_child_webview rejected_for_m7_v1`。

因此不继续 Stage 2 custom protocol、Stage 3 initial custom URL、Ubuntu/macOS child probe、WebviewWindow、Tauri
unstable production adoption、direct Wry、native platform API、WebView2 COM 或新的容器搜索。两类证据都是
rejected design evidence，不是 production security/compatibility success。

## Release blockers 与风险

- Portable UI Manifest/WIT/State/RPC/permission/limits 尚未冻结；在 Stop A 前不能实现或广告 capability。
- Renderer 不得成为业务 authority、绕过 permission/lease，或静默丢弃 required node/action。
- first-party extension 若使用 private page/command/renderer/permission，或协议只由 Tauri consumer 验证，是 blocker。
- 当前没有真实 v3 GUI screenshot/flow Fixture；它阻塞 differential parity，不阻塞明确标注的 engineering work。
- hosted GUI/build evidence不替代 M12 Windows 10/11 完整客户端发行验证。

## 与 M0、M6、M8/M9、M11/M12 的关系

- M0 提供固定 Tauri/React/Vite/Material shell 和三平台 no-bundle build；M7 不改变完整产品发行模型。
- M6 backend runtime acceptance 保持有效；仅未发布 UI-specific contract 被 Portable UI 方向替换。
- M8 MCP management extension 与 M9 Discord extension 使用同一 Portable UI，不能获得 private React page、Tauri
  command、hidden renderer、extra permission 或 first-party-only Surface。
- M11 提供真实 v3 migration/Fixture；其缺失继续阻塞真实 differential parity。
- M12 承担 installer/updater/dist 和 Win10/Win11 完整客户端验证。

## 验证命令

Phase 0 至少运行：

```text
cargo xtask check
python scripts/validate-metadata.py
pwsh -NoProfile -File scripts/freeze-baselines.ps1 -Check
git diff --check
```

并确认 Cargo/npm manifests、三份 lockfile、production source、unsafe、platform API、CI 和 test implementation 无变化，
工作树只包含获批规划文档。

## 下一停止条件

1. Phase 0 创建独立本地规划提交，不 push，`origin/main` 保持当前已推送基线。
2. Portable UI contract-first 尚未开始；等待项目所有者下一轮人工审批。
3. 未获批前不修改 Manifest/package profile/WIT/UI Bridge/Host protocol/RPC/Permission/State Schema 或生产代码。
4. 不自动开始 Slice B/C/D，也不进入 M8。

## 进度日志

- 2026-08-24：M6 正式完成并通过最终人工验收；M7 初始草案规划官方 GUI 与 WebView-based Extension UI container。
- 2026-08-24 至 2026-08-25：形成旧 Stop A proposal 和 test-only iframe/managed child WebView evidence；所有
  physical mapping、custom scheme、Tauri unstable 和 Web UI container 均未进入 production。
- 2026-08-25：iframe CI run `32752875840` 得到 `rejected_for_m7_v1`；Windows managed child 诊断得到
  `child_webview_navigation_unavailable` 和 `isolated_managed_child_webview rejected_for_m7_v1`。实验完成架构决策
  价值，停止继续搜索 WebView container。
- 2026-08-25：项目所有者在无公开兼容对象的前提下，将基础 Extension UI 重置为 Core/RPC Portable UI。Phase 0
  只同步 architecture/ADR/ExecPlan/status/evidence/metadata；Portable UI contract-first 与 production 均未开始。
