# M7：官方 GUI 与扩展 UI 容器

状态：M7 总体方向已获批；仅执行 contract-first Stop A，production implementation 尚未获得批准

## 目标

在 M1-M6 已完成的 daemon、RPC、Operation、Event、项目/仓库、VPM transaction、Unity、模板、备份与
Extension Runtime 基础上，把当前 M0 GUI 壳收敛为可运行的官方客户端和受控 Extension UI 容器：

```text
React / Material Design 3 official shell
    -> main-WebView-only typed Tauri adapter
        -> alcomd-client
            -> ALCOMD RPC v1
                -> alcomd-application

verified extension static UI
    -> isolated WebView or sandboxed frame（Stop A 冻结）
        -> Extension UI Bridge v1
            -> public permission / scope / lease authority
```

M7 不建立第二个业务 authority。GUI 不直接读取 `state.db`、项目文件、repository、package cache 或 Extension
私有数据；Tauri command 只做窗口、文件选择、通知、RPC client 和隔离 UI 容器适配。所有业务结果、revision、
Plan、Operation、权限与错误均以 daemon 返回为准。

## 前置条件

- M6 最终候选 `b666e8fcac6fd4153750c401b76f2f61f6d2a34a` 与 CI run `32724915827` 已通过三平台
  Hosted CI 和项目所有者最终人工验收。
- M6 acceptance closure `577af7d4cd5746ea259c047e9b80189ca698db70` 对应 CI run `32733276048`：
  Windows Server 2025、Ubuntu 22.04、macOS 15 arm64 均成功。
- M1-M6 RPC v1 只能兼容增加；M7 不改变已发布方法、字段、错误或 capability 的既有语义。
- M6 已冻结 logical `ExtensionUiOrigin { extension_id, package_digest }` 与 UI Bridge v1 headless envelope，
  但实际 WebView origin/scheme、placement、CSP 和 Tauri 隔离尚未冻结。
- A-020 要求入口、用例、数据结果、错误、进度和可访问性等价，不要求像素复刻；真实 v3 GUI 截图与流程
  Fixture 尚未建立，因此 `gui.v3-entry-parity` 继续 blocked。

## 最小交付物与完成定义

M7 完成必须同时具备：

1. 官方 GUI shell 具有稳定信息架构、响应式导航、路由和可恢复的 daemon connection 状态。
2. 项目、包、仓库、模板、Unity、备份、Operation 与扩展管理页面通过 typed Tauri/RPC client 调用已经真实
   存在的 M3-M6 用例；不得由前端重算 resolver、revision、权限或 writer safety。
3. Plan/Apply、高影响确认、Operation progress/cancel、结构化错误、stale/revision conflict 和重试语义在 GUI
   中保持可见且不静默重新规划。
4. M7 contract-first Stop A 冻结最小 appearance/settings、activity/diagnostics、GUI adapter 与 Extension UI
   placement/origin/CSP 合同；所有公共 RPC、permission 或 State/Config Schema 变化先取得人工审批。
5. 至少一个 synthetic、签名有效且无 first-party privilege 的 extension static UI 在真实 Tauri/WebView 中经
   UI Bridge v1 工作；spoof、replay、跨扩展访问、Tauri IPC、daemon socket、filesystem 和 top-level navigation
   尝试全部 fail closed。
6. light/dark/system appearance、loading/empty/error/progress/disconnected 状态、键盘导航、焦点恢复、语义化
   标记、缩放和 reduced-motion 通过自动化与人工验收。
7. public/synthetic GUI 工程测试可以按真实证据标记 implemented；没有真实脱敏 v3 GUI Fixture 时，
   `gui.v3-entry-parity` 必须保持 blocked。
8. 本地完整门禁和 Windows Server 2025、Ubuntu 22.04、macOS 15 arm64 Hosted CI 全部成功；真实截图流程经
   项目所有者签收，随后停止在 M8 前。

M7 milestone 完成不等于 M8 MCP、M9 Discord、M11 migration/updater 或 M12 distribution/client validation 完成，
也不自动把 `gui.v3-surfaces` 聚合 feature 提升为 implemented。

## 信息架构与页面边界

### App shell

主窗口保持一个 host-owned shell：

```text
首次设置（仅 M7 已有能力）
主应用
├─ 首页 / 状态摘要
├─ 项目
│  ├─ 项目列表
│  └─ 项目详情
│     ├─ 包
│     ├─ Unity
│     └─ 备份
├─ 仓库
├─ 模板
├─ Operation 中心
├─ 扩展
│  ├─ 安装与生命周期
│  ├─ 权限与 scope
│  └─ 扩展 UI（若存在 ui_entry）
├─ 设置
├─ 活动
└─ 关于 / 许可证
```

- 宽窗口使用 navigation rail/drawer；窄窗口使用 modal drawer。路由身份不能依赖翻译文本。
- 详情页使用稳定 opaque ID，而不是完整路径或显示名作为 route key。
- MCP 管理和 Discord 不成为 host-owned 业务页面。M8/M9 第一方扩展只能在获准的通用扩展 UI 入口中运行。
- 外部访问管理、技术日志、migration、updater 和 distribution 页面只有对应 application/RPC 能力真实存在后才
  可启用；M7 不以假数据或“成功”占位冒充功能。
- 首次设置只覆盖本里程碑真实可配置项。VCC/v3 导入、更新频道、安装器和系统关联步骤继续属于 M11/M12。

### 核心页面到 RPC 的映射

| 页面 | M7 可使用的权威入口 | 禁止的前端行为 |
|---|---|---|
| 首页 | `system.status`、最近 Operation/Event | 推断 daemon PID、路径或未来子系统版本 |
| 项目 | `projects.*` | 直接扫描或写项目目录 |
| 包 | `repositories.packages`、`packages.plan*`、`packages.applyPlan` | 前端 resolver、静默重新 Plan |
| 仓库 | `repositories.*` | 浏览器直接 fetch repository URL |
| 模板 | `templates.*` | 浏览器直接读写 bundle 或 project |
| Unity | `unity.*` | 前端进程枚举、shell launch 或 writer 判断 |
| 备份 | `backups.*` | 普通 unzip、直接 target 写入 |
| Operation | `operations.*`、`events.list` | 把连接断开当作 Operation 取消 |
| 扩展 | `extensions.*` | 读取 package 目录、伪造 grant/lease |

尚未存在的 settings/activity/diagnostics 方法不在草案中伪装成已发布 RPC；其候选合同只在 Stop A 冻结。

## 用户流程与状态模型

每个 route 必须显式支持：

- `initial-loading`：保留页面结构与可访问名称，避免只显示无标签 spinner。
- `refreshing`：保留 last-known-good 内容，显示非阻塞刷新状态。
- `empty`：解释空状态原因和唯一主要行动，不把权限拒绝伪装为空列表。
- `error`：显示稳定 `error.code` 的用户语义、可安全展示的 `diagnostic_id` 和适用的重试动作；不得展示堆栈、
  token、Authorization、完整私密路径或任意内部错误字符串。
- `disconnected`：明确 daemon 不可达、重连中或版本不兼容；保留未提交的纯表单视图状态，但禁用会产生歧义的写入。
- `operation-running`：展示 OperationId、安全 phase、进度、cancel availability 和 detach/follow 状态。
- `confirmation-required`：展示冻结 Plan/ChangeSet、来源、风险与 stale 说明；确认只提交原 PlanId/expectedRevision。
- `success` / `failed` / `cancelled`：以 daemon 终态为准，不能因 UI 请求返回或窗口关闭虚构成功/取消。

同一请求的 stale/revision conflict 必须回到显式 review；GUI 不允许后台重新 Plan 后替用户 Apply 不同结果。

## Material Design 3、主题与 appearance

- 复用现有 React、Material Web 与 `@alcomd/ui`，由 `@alcomd/ui` 承载最小 design token、layout primitive、
  field/dialog/banner/progress/error-state 包装，业务 DTO 不进入 UI package。
- 以 Material Design 3 color/type/shape/elevation/motion token 构建，不复制 v3 像素布局。
- M7 最小 appearance 为 `system | light | dark`、主题 source color、density/compact preference、motion preference
  与 locale。九种 v3 scheme 是否全部进入 M7 由 A-020 流程对照在 Stop A 冻结，不从审计文字直接推断实现。
- 权威 appearance 与 locale 必须由 Stop A 获批后的 daemon settings use case 持久化；`localStorage` 只能保存可丢失的
  drawer 开合、最后 tab、列表列宽等 view state，并必须可在清空后无损恢复业务状态。
- 系统主题变化在 `system` 模式即时生效；用户明确 light/dark 时不被系统变化覆盖。
- reduced motion 来自系统偏好和用户设置的更严格结果；不得用动画作为唯一状态或进度表达。
- 所有颜色组合、focus ring、disabled/error/success 状态必须经过对比度检查；高对比度平台行为在三平台人工验收。

## 可访问性与键盘导航

- 所有功能必须可以仅用键盘完成；navigation、dialog、menu、tabs、data table 和 list/grid 具有确定 tab/arrow/escape
  行为。
- route 切换后焦点进入页面标题；dialog 关闭后回到触发控件；Operation/错误更新使用适当且不刷屏的 live region。
- 图标按钮具有可翻译 accessible name；颜色、位置、hover 或 motion 不能成为唯一提示。
- 支持 200% 文本缩放和至少 320 CSS px 内容宽度，不隐藏关键确认或取消入口。
- 页面使用 heading/landmark/list/table 等原生语义；只在 Material Web 缺少语义时补充 ARIA。
- 自动化覆盖 axe-compatible 规则、焦点顺序、dialog trap/restore、键盘主要流程；人工覆盖 Windows Narrator、
  macOS VoiceOver 与 Linux 可用屏幕阅读器的最小 smoke。自动规则通过不得描述为完整无障碍认证。

## Typed client、Tauri adapter 与 daemon 生命周期

### 主 WebView

- Rust 侧持有 `alcomd_client::AlcomdClient`，执行 `system.hello`、capability negotiation、按需启动和有界重连。
- 前端只调用 main-window capability 允许的 typed command。command 参数使用封闭的 discriminated request/response
  DTO 或逐用例 command；不得提供任意 `method: string + JSON` 的通用 RPC passthrough。
- Tauri adapter 只做 DTO 转换、连接生命周期、窗口/选择器/通知适配，不实现 resolver、permission、Plan、
  revision、project writer 或 extension lifecycle 规则。
- 浏览器 TypeScript 类型必须从已冻结 Schema 生成或以 contract snapshot 验证；当前 `@alcomd/sdk` 仍是早期 scaffold，
  不得把其过时 `system.hello` 字段当作权威。是否在 M7 修正共享 package，或把 M7 类型保持 app-private 直到 M10，
  属于 Stop A 人工审批点。

### 连接状态

连接状态机最小为：

```text
starting -> connecting -> ready
                     \-> incompatible
                     \-> disconnected -> reconnecting -> ready
```

- 只有 endpoint not found / connection refused 可以触发既有 daemon auto-start；permission denied、协议不兼容和
  handshake 错误不得通过反复重启掩盖。
- 重连使用有界 exponential backoff + jitter 和明确“立即重试”动作；窗口隐藏/后台时降低轮询频率。
- 重连后重新 hello、核对 capability，并通过 `events.list`/`operations.list` 从 durable cursor/state 恢复；不能依赖
  WebView 内存中的“上次成功”。cursor 过期或不可用时执行有界全量刷新并明确刷新原因。
- 前端请求使用 view generation/cancellation 丢弃过时响应，但取消 view task 不等于取消 daemon Operation。
- 多窗口不在 M7 最小切片；不预建通用 window/session manager。

## Extension 管理 GUI

host-owned 扩展页面提供：

- installed extension list/detail、publisher/signature/source/trust 的安全摘要；
- desired/runtime/quarantine 状态与最近 crash evidence；
- install Plan/Apply、enable、disable、uninstall Plan/Apply；
- required/optional permission、resource scope、grant revision 与 revoke；
- 后台 lease 的只读状态；
- 仅当已验证 package 含 `ui_entry` 时显示“打开扩展 UI”。

第一方与第三方使用完全相同的 `extensions.*` RPC、grant、scope、lease 和 UI Bridge。GUI 不根据 first-party
标记调用隐藏 command，也不允许 first-party frame 获得额外 Tauri capability。

M7 最小 placement 方案候选为 host-owned `/extensions/:extensionId/ui` 详情子页，不允许 Manifest 自定义 sidebar、
toolbar、context menu 或任意 top-level navigation。该方案能支持 M8/M9 的第一方扩展 UI，同时不提前实现其业务。
placement 在 Stop A 审批后冻结；未经审批不修改 Manifest v1。

## Extension UI Bridge 的真实 WebView/Tauri 集成

### 必须冻结的 physical mapping

M6 logical origin 保持不变。M7 Stop A 必须在实际 Tauri 2 / WebView2 / WebKitGTK / WKWebView 上比较并冻结：

1. 无 Tauri capability 的 isolated child WebView；
2. main WebView 内、带严格 sandbox/CSP 的 cross-origin iframe。

优先选择能够在三平台证明以下性质的最小方案，不为统一 DOM 布局牺牲隔离：

- URL/scheme 只由 host 根据已验证 ExtensionId、package digest 和 `ui_entry` 构造；页面输入不能指定 authority。
- physical origin 确定映射到 logical `ExtensionUiOrigin`，package 更新后旧 digest origin/session 失效。
- 只读取 immutable verified package 的 `ui/` regular files；路径 normalization、MIME、size、fallback 与 cache policy
  fail closed，不暴露 package/root 真实路径。
- 每个 extension origin 相互隔离，禁止 same-origin 合并、目录 listing、跨 digest cache 混用和 symlink/reparse 跟随。
- CSP 默认拒绝网络、frame ancestor、object、worker、form 和 top navigation；允许项必须由真实 UI 需求和公开
  permission/Host capability 单独批准。M7 不新增 `network.request`。
- extension context 没有 `window.__TAURI__`、Tauri invoke/event/channel、Node、filesystem、clipboard、notification、
  daemon socket 或 main DOM authority。
- Bridge 仍使用 v1 session/origin/generation/grant revision/sequence/requestId/rate/quota 校验；WebView transport 不能
  绕过 `crates/alcomd-extensions` 的 admission authority。
- disable/revoke/uninstall/package replacement/navigation/crash 后关闭 session、拒绝 pending request 并清空 queue。

如果 Tauri/WebView 的实际注入模型无法证明 iframe 不接触 private IPC，则 M7 必须使用独立无 capability WebView；
不得用约定“扩展不要调用”代替技术隔离。physical URL/scheme 只是宿主实现细节，不升级为 Extension ABI v1 永久 URL。

### Bridge 方法边界

M7 只把当前真实公开能力映射为明确 allowlist。不得把任意 RPC method、Tauri command 或 application service 透传给
extension UI。新增 Bridge method 必须同时有：public permission、resource scope、stable DTO/error、revocation test
与 threat-model 更新。`headless.test.ping` 继续仅用于测试，不进入 production catalog。

## M7 contract-first Stop A

生产实现前必须形成并由项目所有者统一审批：

1. GUI route/flow matrix：页面、入口、RPC、permission、loading/empty/error/progress/confirmation 与 deferred 项。
2. appearance/settings v1：权威字段、revision/concurrency、State 或 Config 二选一的 persistence、RPC
   method/capability/permission 候选和
   localStorage 白名单。
3. activity/diagnostics read model：Activity 默认只组合 `events.list` 与 `operations.*`，分别复用
   `events.read` 与 `operations.read`；不默认新增 `activity.read`。当前 Permission baseline 没有
   `diagnostics.read`，Stop A 只有在证明现有 Event/Operation read model 存在真实缺口并冻结 exact safe DTO、
   daemon/application redaction boundary、pagination/bounds、stable errors、capability、permission requirement 与
   Principal availability 后，才可把新的 read model/permission 作为候选提交审批。原始诊断导出继续排除。
4. main-WebView typed Tauri command schema、capability 文件和 extension frame 无 private IPC 的 negative contract。
5. Extension UI placement、physical scheme/origin、asset serving、MIME/cache/CSP/sandbox 与 Bridge transport。
6. public/synthetic flow fixtures、a11y matrix、截图清单和三平台真实 WebView 验收方案。
7. 所有新增 frontend/dev/runtime dependency 的精确版本、features、license、维护状态、Node 兼容、native/build
   script、bundle/lock diff 与替代方案。

Stop A 前不得修改 public RPC、permission name、Config/State Schema、Manifest/WIT/UI Bridge contract 或 production
代码。Stop A 通过不代表 M7 完成，只允许按获批 slice 实施。

Stop A proposal 已集中记录在 `specs/gui/m7-stop-a.md`；appearance exact wire shape 位于
`specs/gui/m7-settings-appearance-v1.proposal.schema.json`，synthetic vectors 位于
`crates/alcomd-testing/fixtures/m7/stop-a-vectors.json`，依赖候选位于
`docs/exec-plans/M7-dependency-evaluation.md`。这些文件名中的 proposal 含义是“等待下一轮人工审批”，不构成
published contract。三平台 actual WebView evidence 尚未取得，因此 container choice 与 physical mapping 保持
`not yet frozen`；不得从 preferred probe candidate 推导 production 授权。

## 实施顺序

### Slice 0：contract-first 与 test harness

- 冻结 Stop A 工件、snapshot、synthetic fixtures、typed adapter contract 和三平台 WebView harness。
- 证明 extension context 无 Tauri/private IPC，再开始真实 extension UI wiring。

### Slice 1：shell、connection 与 design system

- 替换 M0 hero scaffold；完成 app shell、route、connection state、error boundary、MD3 token 和 a11y primitives。
- 接通 `system.status`、Operation/Event 恢复，不接业务写入。

### Slice 2：只读核心页面

- 项目、仓库/package catalog、模板、Unity installation、备份、Operation 与扩展 list/detail。
- 先完成 loading/empty/error/disconnected，再增加 mutation action。

### Slice 3：Plan/Apply 与核心写入口

- 项目 registry、repository registry、package Plan/Apply、Unity registry/launch、template、backup 和 extension lifecycle。
- 每一类写入复用现有 revision/idempotency/Operation/writer gate，不建立 GUI 专用业务路径。

### Slice 4：settings 与 activity

- 只实现 Stop A 获批的最小 State/Config/RPC；无真实 daemon use case 的设置不显示为可保存。
- Activity 只组合现有 Event/Operation authority；Stop A 未证明 diagnostics 独立缺口，因此 production slice
  默认不包含 diagnostics。技术详情不能落入普通前端日志或 telemetry。

### Slice 5：真实 Extension UI 容器

- 接通 approved physical origin/CSP/transport 与 generic host-owned placement。
- synthetic third-party 与 synthetic first-party package 走相同路径；MCP/Discord 产品逻辑仍不实现。

### Slice 6：收敛与人工截图签收

- 完成 flow matrix、键盘/a11y、主题、断线/恢复、三平台 actual WebView smoke 和截图签收。
- 只按真实证据更新 feature/test metadata，停止在 M8 前。

## 允许修改范围（获批 production 后）

contract-first 与后续 production 的候选允许范围为：

```text
apps/alcomd-gui/
packages/alcomd-ui/
crates/alcomd-client/                 # 仅 GUI 所需的 typed client/reconnect 增量
crates/alcomd-protocol/               # 仅 Stop A 获批的兼容 RPC 增量
crates/alcomd-application/            # 仅 settings/activity/diagnostics use case
crates/alcomd-store/                  # 仅获批的最小权威持久状态
crates/alcomd-extensions/             # UI Bridge/WebView transport adapter，不改 ABI/WIT
apps/alcomd/                           # 仅获批 RPC dispatcher/hello capability
packages/alcomd-sdk/                  # 仅在 Stop A 明确批准共享 TS contract 时
specs/config/
specs/rpc/
specs/extensions/ui-bridge-v1.*
specs/security/extension-threat-model.md
crates/alcomd-testing/fixtures/m7/
crates/alcomd-testing/tests/
docs/testing/test-plan.toml
feature-parity.toml
scripts/、xtask/、.github/workflows/   # 仅 M7 test/build gate
docs/exec-plans/M7-official-gui-extension-ui.md
docs/status.md
package.json / package-lock.json       # 仅获批 dependency/script 变化
Cargo.toml / Cargo.lock                # 仅获批 Tauri adapter dependency/feature 变化
```

实际获批范围应按 slice 收缩；本列表不构成新增生产依赖、公共合同或平台 API 的 blanket approval。

## 依赖方向与架构约束

```text
React pages -> app-private typed GUI client -> main-only Tauri command
    -> alcomd-client -> alcomd-protocol -> local RPC -> alcomd

extension static UI -> UI Bridge transport -> alcomd-extensions admission
    -> public application capability
```

禁止：

```text
React / Tauri -> state.db
React / Tauri -> project/repository/package/cache filesystem
React -> arbitrary daemon RPC method
Extension frame -> main Tauri command / daemon socket / main DOM
first-party extension -> hidden command or permission bypass
@alcomd/ui -> business DTO / RPC / Tauri
```

`alcomd-gui` 可以依赖 `alcomd-client`/`alcomd-protocol`；`@alcomd/ui` 只提供视觉与交互 primitive。不要引入
service locator、通用 dependency injection、Redux 式全局业务副本、通用 workflow engine 或第二套 Operation store。

## Frontend/runtime dependency evaluation

当前已存在且优先复用：React 19.2.8、Material Web 2.5.0、Tauri API 2.11.1、Tauri CLI 2.11.4、TypeScript
6.0.3、Vite 7.3.6。草案不安装或批准任何新依赖。

Stop A 应分别评估：

- 文件/目录选择：优先 Tauri 官方 dialog plugin；精确版本与 capability 必须和当前 Tauri 版本兼容。
- MD3 source color/HCT：先确认 Material Web/现有代码是否足够；不足时再评估 Material 官方 color utilities。
- component/a11y test：Vitest、Testing Library、user-event、axe-compatible 工具。
- real WebView/e2e：优先 Tauri 官方支持的 WebDriver/harness；macOS 若无等价自动驱动，明确人工与 test-only
  in-app harness 边界，不宣称跨平台等价自动化。

只有候选的 exact version、features、license、维护状态、Node 24 支持、transitive/native/build script、bundle
影响和 `package-lock.json`/`Cargo.lock` diff 获批后才可安装。优先不用 router/data-cache/global-state 生产依赖；
若原生 History API + React state 足以满足有限 route，不引入 React Router 或通用 query framework。

## 单元、集成、安全与三平台验收

### Unit / component / contract

- route/parser、typed DTO snapshot、unknown optional field/capability、stable error mapping。
- loading/empty/error/disconnected/refreshing/Operation/confirmation 全状态 component tests。
- keyboard、focus restore、live region、theme/system change、reduced motion、200% zoom layout。
- localStorage allowlist 与“清空 view state 不丢业务状态”。
- main Tauri command allowlist；任意 method string、unknown variant、oversize 和 malformed payload fail closed。
- Extension asset path/MIME/cache/CSP/origin mapping 与 Bridge v1 sequence/replay/quota/revocation。

### Integration / fault / security

- 真实 daemon auto-start、hello、capability、断开、重启、版本不兼容和 cursor recovery。
- 所有 M3-M6 页面走真实 RPC；写流程验证 Plan review、stale、revision conflict、idempotency、Operation
  follow/cancel/restart recovery。
- synthetic extension 在真实 WebView 发起允许 Bridge method；origin spoof、cross-extension/digest、replay、flood、
  top navigation、network、Tauri invoke/event/channel、filesystem、clipboard 与 daemon socket 尝试均拒绝。
- disable/revoke/uninstall/package replacement 和 WebView crash 后 session/pending/event queue 失效。
- 错误、前端 console、activity 与 screenshot fixture 不含 token、Authorization、完整私密路径、argv 或凭据。

不执行攻击性公网、真实 credential 或第三方服务测试；使用 local deterministic fixtures 验证 GUI 自身边界。

### 三平台边界

- Windows Server 2025 hosted、Ubuntu 22.04 hosted、macOS 15 arm64 hosted 均运行 workspace/frontend checks、
  component/contract/integration tests、release executables、Tauri `build --no-bundle`、锁文件与 final diff gate。
- 三个平台都必须实际加载 synthetic extension UI 并验证 physical origin、CSP、Bridge 和 Tauri/private IPC denial；
  若 hosted runner 无可靠交互桌面，使用获批 test-only in-app harness，并明确它不是截图/辅助技术人工验收。
- Ubuntu 继续检查最高 `GLIBC_2.35` ceiling；macOS 继续检查 arm64 / minos 11.0。
- Windows Server 2025 不是 Windows 10/11 客户端兼容证据。Win10 22H2/Win11 的完整安装、启动、WebView2、
  用户路径、更新和卸载仍 deferred 到 M12，不因 M7 WebView smoke 提前标记通过。
- A-020 截图/流程签收至少覆盖 light/dark、窄/宽布局、关键 empty/error/progress/confirmation、键盘焦点和
  extension UI isolation state；像素差异不单独构成失败，缺失用户入口或状态构成失败。

## Fixture 与 parity

- M7 创建的 fixtures 只能是 public/synthetic project/repository/package/template/backup/extension/UI 数据。
- 可以新增独立的 M7 engineering test IDs，例如 GUI route/state contract、typed RPC integration、a11y、
  extension WebView isolation；只有真实测试落地后才标记 implemented。
- `gui.v3-entry-parity` 在真实冻结截图/流程 Fixture 与差分证据形成前保持 blocked。synthetic screenshot、审计表或
  人工记忆不能冒充 v3 differential parity。
- `projects.v3-parity`、`templates.v3-parity`、`backups.v3-parity`、`unity.v3-vrc-parity` 继续由 M11 Fixture 阻塞。

## 明确排除

- M8 MCP protocol、tool implementation、client credential 和 MCP 管理产品逻辑。
- M9 Discord Presence、Discord settings/preview/background capability 产品逻辑。
- Local API、Python/.NET SDK、M10 公共 SDK 完整硬化。
- v3/VCC/ALCOM migration、bootstrap、updater、deep-link registration 和 legacy cleanup。
- installer、签名、公证、DMG/AppImage/DEB/Inno Setup、完整产品 dist 和 Windows 10/11 客户端发行验收。
- 新的 Extension background capability、`network.request`、filesystem、clipboard、notification 或 Discord 权限。
- Manifest v2、Extension ABI v2、WASI 0.3、WIT 变化和 native extension。
- arbitrary extension sidebar/toolbar/context-menu/top-level navigation contribution。
- 多窗口框架、通用 desktop automation、service locator、dependency injection 或 workflow engine。

## Release blockers 与风险

- 真实 WebView 中的 Tauri IPC 注入、custom scheme origin 和 CSP 行为存在平台差异；headless M6 test 不能替代 M7
  实机证明。任一平台无法 fail closed 时阻塞 Extension UI 容器发布。
- current `@alcomd/sdk` 是早期 scaffold，含与冻结 M1 hello 不一致字段；M7 不修正或绕过会造成 typed client 漂移。
- Material Web 与 React custom element interop、focus/ARIA 行为需实际测试；视觉可用不等于无障碍通过。
- 当前没有真实 v3 GUI screenshot Fixture；它阻塞 differential parity，但不阻塞有明确标注的 synthetic/public M7
  engineering implementation。
- activity/diagnostics 与 settings 尚缺冻结 production contract；在 Stop A 前不得由 localStorage、前端日志或私有
  Tauri command 临时替代。
- hosted GUI automation 不能证明 Windows 10/11 发行生命周期；该风险继续由 M12 接收。
- M8/M9 第一方 extension UI 尚未实现；M7 只能证明通用容器和 synthetic first/third parity，不能宣称这些产品完成。

## 人工审批点

1. Stop A 的 route/flow matrix、M7 completion boundary 与哪些 v3 flow 明确 deferred。
2. appearance/settings 的 RPC、capability、permission 与 Config/State Schema，以及 Activity 复用
   `events.read`/`operations.read` 的边界；任何 diagnostics 候选必须另行证明并审批。
3. `@alcomd/sdk` 在 M7 的修改边界，或 app-private typed contract 到 M10 的临时边界。
4. isolated WebView 与 sandboxed iframe 的最终选择，以及 physical scheme/origin、CSP、asset serving 和 placement。
5. 所有新增 frontend/dev/runtime dependency 与 lockfile diff。
6. 三平台 real WebView harness、人工截图与 accessibility 签收矩阵。
7. M7 最终 feature/test metadata 提升范围和进入 M8 前的人工验收。

任何新增 production dependency、public RPC、permission、Config/State Schema、Extension Manifest/UI Bridge contract、
Tauri capability 或平台 API 都必须在相应审批点停止；普通获批 M7 UI 实现问题可自主处理。

## 与 M0、M6、M8/M9、M11/M12 的关系

- M0 提供固定 Tauri/React/Vite/Material 壳和三平台 no-bundle build；M7 替换其展示性 scaffold，不改变完整产品
  发行模型。
- M6 提供 extension package/runtime/grant/lease 和 logical UI Bridge 安全合同；M7 只完成真实 UI transport/
  placement，不给予 first-party 隐藏权限。
- M8/M9 分别实现 MCP 与 Discord 第一方扩展业务；它们使用 M7 通用扩展 UI 容器，但 M7 不实现其内容。
- M11 提供真实 v3 migration/Fixture；其缺失继续阻塞 GUI 与各业务面的真实 v3 parity。
- M12 承担安装器、updater/dist 和 Windows 10/11 完整客户端验证；M7 hosted/actual WebView smoke 不替代 M12。

## 验证命令

规划草案至少运行：

```text
cargo xtask check
python scripts/validate-metadata.py
pwsh -NoProfile -File scripts/freeze-baselines.ps1 -Check
git diff --check
```

获批生产实现后按实际依赖运行：

```text
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo xtask check
npm run check
npm run build
python scripts/validate-metadata.py
pwsh -NoProfile -File scripts/freeze-baselines.ps1 -Check
git diff --check
```

并取得最终提交自身的三平台 Hosted CI、真实 Extension WebView isolation 结果和项目所有者截图/流程签收。

## M8 前停止条件

1. 本草案取得人工审批后，先执行统一 contract-first Stop A；Stop A 未批准前不得开始 M7 production。
2. M7 production、contract/component/integration/security/a11y tests、三平台 Hosted CI 和人工截图签收全部完成。
3. 只按真实证据更新 feature/test metadata；所有 v3 parity、M8/M9、M11/M12 状态保持真实。
4. 最终提交、origin/main、remote main 与 CI head SHA 一致，工作树干净。
5. 项目所有者完成人工验收后 M7 才可正式完成；随后停止在 M8 前，不自动开始 MCP 实现。

## 进度日志

- 2026-08-24：M6 已通过最终人工验收并正式完成；acceptance closure
  `577af7d4cd5746ea259c047e9b80189ca698db70` 对应 CI run `32733276048` 三平台全部成功。
- 2026-08-24：创建本 M7 ExecPlan 草案。只规划官方 GUI、MD3/appearance、typed RPC client、connection state、
  accessibility、Extension UI WebView/Tauri 隔离和三平台验收；未修改 production code、RPC、State/Config Schema、
  Manifest/WIT/UI Bridge、permission、dependency、lockfile、unsafe 或 platform API。
- 2026-08-24：项目所有者批准仅进入 contract-first Stop A，并纠正权限基线：Activity 优先复用
  `events.read`/`operations.read`，`activity.read` 不默认新增；`diagnostics.read` 当前不是已批准 Permission。
  Stop A 只形成提案、fixtures、threat model、test plan 与 test-only isolation evidence，不开始 production。
- 2026-08-24：形成 Stop A route/settings/activity/typed adapter/placement/physical mapping/CSP/sandbox proposals、
  threat-model 增量、synthetic contract vectors 与精确依赖候选审计。选择 app-private generated/snapshot-verified
  TypeScript contract 到 M10；未安装依赖或修改 lockfile。三平台 actual WebView evidence 尚未取得，故 iframe 与
  isolated child WebView 的最终选择保持 `not yet frozen`。test-only Tauri example 已通过 compile check；本机 Windows
  probe 在进入 `main` 前被 loader `STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139)` 阻止，不能记为 WebView2 security
  evidence，等待项目所有者审批 hosted harness 后再取得三平台结果。
- 2026-08-24：M7 focused contract tests（5 项）、`cargo fmt --all -- --check`、Workspace all-target Clippy、
  `cargo xtask check`、metadata validation、baseline freeze check 与 `git diff --check` 通过；三份 lockfile 无变化，
  未新增 production dependency、public contract、unsafe 或 platform API。Stop A 停在 container/physical mapping 的
  三平台 actual WebView evidence 与下一轮人工审批之前。
- 2026-08-24：项目所有者批准仅执行 test-only 三平台 WebView evidence。为 iframe candidate 增加真实
  WebView2/WebKitGTK/WKWebView probe、A/X-B/X-A/Y origin/session matrix、`headless.test.ping` positive Bridge
  control、main-WebView-only Tauri command control、CSP/IPC/DOM/network/filesystem/clipboard/notification denial
  与 fail-closed CI glue；production Extension UI container 继续 `not yet frozen`，未开始 M7 production。
