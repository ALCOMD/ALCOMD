# M7：官方 GUI 与 Portable Extension UI

状态：Portable UI B0-D production candidate `aa1323430252ed21995284a7b36dd36e45a15e0a` 已通过 Hosted CI
`32877438910`。Official GUI functional candidate `19267230507071dc61ba306b98c8cfdd113e9ea2` 完成 E1-G1/G3
生产实现与本地自动化验收，但项目所有者在正式 checklist 开始前拒绝其 visual/layout acceptance：其宏观布局没有以 v3 为
基线，且 `@material/web` dependency 存在但 component system 未被采用。当前只完成 visual realignment audit；H0-H7 尚未获
production approval。M7 仍未完成，尚未进入 M8/M9。

## 目标与完成定义

M7 在 M1-M6 已完成的 daemon/RPC/application/Extension Host 基础上交付官方 React/Material Design 3 客户端，并建立
GUI-neutral Portable Extension UI v1：

```text
Extension Component
    -> alcomd-extension-host
    -> alcomd-application authority
    -> ALCOMD RPC v1 Portable UI
        -> official React/MD3 renderer
        -> non-Tauri headless conformance consumer
        -> future third-party GUI renderer
```

官方 GUI 与 extension renderer 都只经 typed client/RPC/application 访问权威状态；不直接读 state.db、项目、repository、
package cache、extension data 或 Host protocol。first-party extension 与 third-party extension 使用相同 package、WIT、
permission、Host、session、RPC 与 renderer contract。

M7 完成需要：

1. 保持已经通过 Hosted CI 的 Portable UI B0-D contract、Core/Host、official renderer、headless conformance 与
   security/fault evidence；
2. official GUI 完整覆盖 M1-M7 已真实发布的 GUI-relevant application/RPC use case；
3. 统一实现 navigation、route、responsive shell、typed adapter、页面状态、Plan review、Apply、Operation follow 与恢复；
4. 实现获批的 Config Schema v1、`settings.get`/`settings.update`、`activity.list` 与 `diagnostics.list`；
5. Chromium browser-level DOM/component accessibility automation，以及 A-033 所要求的 v3 macro layout、Material component
   behavior、分阶段截图和一次真实交互桌面的视觉/流程/键盘焦点签收完成；
6. 本地完整 gate 和最终提交自身的 Windows Server 2025、Ubuntu 22.04、macOS 15 arm64 Hosted CI 通过；
7. 项目所有者完成人工验收。

`gui.v3-entry-parity` 不属于 M7 completion blocker：它保持 blocked 并由 M11 在真实脱敏 v3 Fixture 到位后完成。
Narrator、VoiceOver 与 Linux screen-reader smoke 由 M12 在真实目标客户端/辅助技术 runtime 上完成，M7 不把它们标记为通过。

## 已完成前置证据

- M6 final `b666e8fcac6fd4153750c401b76f2f61f6d2a34a` / CI `32724915827` 已通过最终人工验收；M6 backend runtime
  acceptance 保持有效。
- M7 WebView evidence 只证明 sandboxed iframe 与 Tauri managed child 不适合作为 v1 产品方向，保留在
  `specs/gui/m7-stop-a.md`，不属于 security/compatibility success。
- Phase 0 architecture reset commit `12089661f7c694f059aaf172344809ff814716f6` 独立保留。
- obsolete test-only WebView harness cleanup commit `c05c5d2c712dd7a791401efc22b8dd19b88023a6` 独立保留；普通 Tauri
  build/no-bundle gate 未删除。

## Stop A frozen contract

以下合同已获项目所有者批准。Slice B0 已原子替换 active Manifest/package/WIT，加入 State v9 migration、typed
`ui_protocol`、`extensions.ui.use` 与 RPC DTO foundation；Slice B1 已完成 Session runtime 与 capability advertising；
renderer 已在 Slice C 完成；B0/B1 状态没有被用于冒充 renderer evidence。

### Manifest、package 与单一 Portable UI

active Manifest v1 已直接改写为：

```toml
[entrypoints]
component = "component/extension.wasm"

[ui]
protocol = "portable-v1"
```

`background_component` 直接改名为 `component`，不保留 alias；删除 `ui_entry`。package allowed roots 只剩
`alcomd-extension.toml`、`META-INF/`、`component/`，删除 `ui/` 与 UI quota，保留全部 ZIP/signature/digest 安全规则。

v1 每个 extension 最多公开一个隐式 Portable UI；其逻辑身份 `main` 不保存或传输。MCP management 与 Discord Presence
synthetic fixture 都可在一个 page 内使用 section/list/form 表达。Manifest、State、WIT、RPC与Snapshot均没有页面数组、
ID参数、dynamic discovery或兼容槽位。extension record optional addition固定为：

```json
{"ui":{"protocol":"portable-v1"}}
```

### Capability 与 consumer completeness

客户端在 hello 请求 `extensions.ui.portable.v1`；daemon 完整实现后才回显。声明支持的 consumer 必须实现全部 v1
node/action。没有 hostId、official GUI identity、framework/renderer name、feature subset/intersection 或 GUI-specific tree。
不支持的 GUI 继续提供 extension management，并把功能页标为 unsupported。unknown v1 node fail closed；新增 node 使用
新的 capability/version。

### Active WIT/ABI v1

active WIT 位于 `specs/extensions/wit/extension-v1/`，production bindgen 与真实 Extension Host 使用
`alcomd:extension/extension-v1@1.0.0`，import host-projects/host-data，required export guest-lifecycle/guest-ui。所有 ABI v1
Component 都必须实现 guest-ui；Manifest 无 `[ui]` 时 daemon 不调用，reference fixture 提供 empty stub，Host instantiate
仍验证完整 world。guest-ui exact sync functions：

```text
open(guest-session-id, locale) -> UiDocument
refresh(guest-session-id) -> UiDocument
dispatch(guest-session-id, UiAction) -> UiDocument
close(guest-session-id)
```

Host embedding 继续 async；没有启用 component-model-async、WASI future/stream、wasmtime-wasi、guest thread 或新依赖。
旧 proposal directory 已删除；没有保留 parallel world，没有创建 ABI v2，`extensionApi.major` 仍为 1。

### Lifecycle 与 Host protocol

activation kind 固定 `background|interactive-ui`；stop reason增加 `interactive-ui-idle`，并保留 disabled、permission-revoked、
lease-expired、daemon-shutdown、uninstalling。`extensions.ui.open`不隐式enable；disabled/quarantined/no-ui分别返回
`extension_not_enabled`/`extension_quarantined`/`extension_ui_not_available`。检查client、package/digest、
grant/lifecycle/quarantine后创建interactive-ui lease，按需启动并仅在本次Host activation尚未发生时activate。
多个session共用activation，background-running Host不重复activate。最后session关闭且无background lease时调用
`deactivate(interactive-ui-idle)`并在5,000 ms内停Host；关闭route不修改desired state。
`background.run`不自动加入Manifest required permissions。扩展若主动列为required且未授权，enable失败；列为optional且
未授权时UI仍可用，但最后一个interactive session关闭后Host停止。Portable UI不产生隐式background lease。

daemon 为每个 invoke-export签发 `InvocationContextId`，绑定 invocation kind、Extension lease/grant/generation、deadline/
cancel，以及 interactive-ui 的 Client Principal/connection/UiSession/snapshot。capability-call必须回显。grant/lease/
session/deadline/generation正常竞态返回stable stale/permission/cancel并取消invocation，不计Host crash。unknown/cross
extension/session/invocation、completed reuse或authority修改/伪造才是Host protocol violation并终止Host。export返回后
context立即失效；requestId/callId仍只做correlation。

### Document、tree、form 与 action

Guest 只返回完整 UiDocument；daemon验证并包装从 1 单调递增的 UiSnapshot revision。flat tree必须恰好一个 page root、
parent-before-child、无 cycle/unreachable node、连续唯一 sibling order、exact parent/child matrix、unique
node/field/action/option ID、depth 8。ID grammar固定 `^[a-z][a-z0-9._-]{0,63}$`，form nodeId即formId。固定 node：

- layout：page、section、stack、group、form、list、list-item；
- display：text、status、key-value、progress、divider；
- input：button、switch、text-field、integer-field、select。

tone 只有 neutral/info/success/warning/danger；progress 为 indeterminate 或 0-10000 basis points；integer 为 JSON-safe
signed integer。禁止 HTML/CSS/JS/Markdown HTML/canvas/image/icon URL/link/URI/font/color/animation/absolute/custom ARIA/
DOM/component/expression/iframe/WebView/navigation。

switch/select/text/integer拥有且只拥有一个form ancestor，禁止nested form；v1 action只有 `activate` 与 `submit-form`。
submit按node order只携带完整editable field set；disabled/read-only值来自Snapshot，client提交它们即invalid。daemon验证
form/action归属、type/required/length/range/option/completeness/size。field可带valid或invalid+最多512 UTF-8 bytes纯文本；
renderer设置invalid与host-generated `aria-describedby`，该文本不是security confirmation。

### Exact limits

机器权威值在 `specs/extensions/portable-ui-limits-v1.json`：session per extension/client/daemon 为 8/16/128；
Snapshot/action 262,144/65,536 bytes；nodes 256；depth 8；ID 1-64 lowercase ASCII；single/total text 4,096/65,536
UTF-8 bytes；validation 512 bytes；form 64 fields；select 64 options、label 256 bytes；Host/session concurrency 1；token bucket 60/minute、
burst 10；idle/absolute 300,000/3,600,000 ms；request cache 64；Host idle stop 5,000 ms。

plain text拒绝 NUL、C0/C1/DEL与冻结 bidi controls。text node允许 LF；text-field只有 multiline 时允许 LF；其他字段
单行，CR/TAB始终拒绝。locale只接受 2-15 ASCII bytes 的 canonical language[-Script][-Region] core BCP47 子集。

### Session、replay 与 failure

memory-only session绑定 Client Principal/connection、Extension Principal/instance/package、grant/lifecycle/
snapshot revisions、locale/deadlines、next sequence和64-entry replay cache。disconnect/restart/disable/revoke/uninstall/
package or generation change/Host crash/quarantine/timeout/protocol violation立即关闭。session不写 DB/Event/data/package/log，
不 crash-recover，不创建 Operation。

dispatch每session内存保存最近64项sequence/requestId/expected revision/canonical action fingerprint/resulting Snapshot。
新连续请求调用guest一次；exact replay不调用guest并返回原resulting Snapshot、`replayed=true`；相同ID/sequence但
revision/fingerprint冲突或gap/out-of-order统一返回 `extension_ui_action_invalid`。Client错误不计Host crash；Guest invalid
document关闭session、终止Host并计入既有crash/quarantine。Portable UI无server push；draft只在GUI memory并绑定
session/revision/form，dirty时禁止自动refresh/merge，主动刷新/导航需host-owned discard confirmation。

### Authorization 与 RPC

client candidate permission 是 `extensions.ui.use` scoped exact ExtensionId；没有 extension `ui.contribute`。list/get UI
declaration用 extensions.read；open/refresh/dispatch用 extensions.read + extensions.ui.use + exact scope；close返回
`{closed:boolean}`并 best effort，不泄露 owner。

UI invocation 内 Host business capability取 Client Principal与 Extension Principal同一 resource scope交集；host-data
仍由 extension self namespace authority控制，client只需 UI permission。high-impact Plan/credential/Operation继续 host-owned。

RPC compatible addition只有 capability 与 open/refresh/dispatch/close。DTO closed/bounded，open只接收extensionId/locale，
不接收页面ID、host/framework/platform。固定route为 `/extensions/:extensionId/ui`。stable error分类由
`m7-portable-ui.schema.json`冻结，删除 `extension_ui_surface_not_found`，不创建宽泛
`extension_ui_failed`，也不公开 Host PID/pipe/lease/path/identity/context。

### State v9 与 renderer

M6 曾广告 v8；active v9 只为 extensions 和 immutable extension_plans 各增 nullable `ui_protocol`，合法值 NULL 或
`portable-v1`。不保存页面常量，不建session/Snapshot/replay/action/draft/renderer/browser/cache/workflow table。v8->v9
单事务 migration 已保留 rows/FK/revision/Event sequence，existing 默认 NULL，future schema fail closed；B1 完整接线后
hello 已广告 dataSchema 9 与 `extensions.ui.portable.v1`。

官方 React/MD3 renderer与non-Tauri headless consumer使用同一 public DTO/fixtures。官方 contract冻结 host-owned
name/ExtensionId/publisher-trust/version/desired/runtime/quarantine/extension-provided chrome、exhaustive match、
keyboard/focus/ARIA、200%/320px、reduced motion、无 extension CSS/DOM/Tauri。headless consumer不依赖GUI/Tauri并输出
确定性 semantic summary。B0/B1 不实现任何 renderer；Slice C 已实现 official renderer，Slice D 已实现独立 headless
consumer 并用相同 public DTO/Fixture 验证全部 node/action 与 fail-closed 行为。

## Contract artifacts

- ADR refinement：`docs/adr/0024-portable-extension-ui.md`；
- Portable UI contract/schema/limits：`specs/extensions/portable-ui-v1.*`、`portable-ui-limits-v1.*`；
- active Manifest/package/ABI/permission/Host protocol：`specs/extensions/`、
  `specs/extensions/host-protocol-invocation-context-v1.md`；
- active WIT：`specs/extensions/wit/extension-v1/`；
- RPC contract：`specs/rpc/m7-portable-ui.schema.json`、`specs/rpc/alcomd-rpc-v1.md`；
- State v9/migration：`specs/storage/state-v9*`、`crates/alcomd-store/migrations/0009_portable_extension_ui.sql`；
- threat model/consumer contract：`specs/security/extension-portable-ui-threat-model.md`、
  `specs/gui/portable-ui-renderer-v1.md`；
- MCP/Discord/adversarial/headless fixtures：`crates/alcomd-testing/fixtures/m7/`；
- contract tests：`crates/alcomd-testing/tests/m7_contract.rs`。

## 已批准 production slices

1. Slice B0（已完成）：原子替换 active Manifest/package/WIT/ABI/permission/State contract。
2. Slice B1（已完成）：接入 Host/Core/RPC memory-only session、InvocationContext、interactive lifecycle、render purity、
   current-only replay、validation、capability advertising 与 invalidation。
3. Slice C（已完成）：官方 shell/typed adapter/React/MD3 renderer、forms、host-owned chrome 与
   loading/error/disconnected/reconnect，仍只调用 public client/RPC。
4. Slice D（本地实现与定向验证已完成）：non-Tauri consumer、first/third parity、malformed/revoke/crash/reconnect、
   InvocationContext binding、dual-authority route、payload redaction 与 limits；最终三平台/a11y验收仍待 push 后执行。

## Restored Official GUI Completion

Portable UI architecture reset 只替换旧 Web UI/iframe/isolated WebView 方向，不删除 official GUI 对当前 Core use case 的覆盖。
completion audit 后恢复以下批准切片：

1. Slice E1：Core read surfaces、stable route identity、wide rail/drawer、narrow modal drawer 与统一 data-route state；
2. Slice E2：当前既有 mutation、typed Plan review、explicit Apply、Operation progress/cancel/reconnect；
3. Slice F1：Config Schema v1、`settings.toml` 单 writer durability、settings permission/RPC；
4. Slice F2：Event/Operation 的 safe Activity projection 与 redacted Diagnostics projection；
5. Slice F3：Settings、Activity、Diagnostics 与 About GUI；
6. Slice G1：Chromium browser-level DOM/component accessibility automation；
7. Slice G2：一次真实交互桌面的 manual screenshot/flow/keyboard-focus evidence preparation；
8. Slice G3：metadata/completion audit、完整本地 gate 与最终本地候选。

### Information architecture

Official GUI 的宏观 information architecture 受 A-033 约束。冻结参考与映射分别位于：

- `docs/gui/alcomd3-v3-layout-baseline.md`；
- `docs/gui/m7-layout-mapping.md`；
- `docs/gui/m7-current-layout-gap.md`；
- `docs/gui/m7-material-component-audit.md`。

v3 是 layout/navigation/page grouping/action placement 的设计基线，不是源码上游。以下 route identity 与当前 Core capability
继续有效，但最终 navigation grouping/composition 必须按 mapping 经项目所有者批准后落地：

```text
Home
Projects
  -> Project Detail
       -> Packages
       -> Unity
       -> Backups
Repositories
Templates
Operations
Extensions
  -> Extension Detail
       -> Permissions
       -> Portable UI
Settings
Activity
Diagnostics
About
```

route identity 使用稳定 ASCII ID 和 opaque resource ID，不使用翻译文案或完整路径作为 key。不存在 Core/RPC capability 的
业务不显示假按钮或 disabled fake page。M8 MCP、M9 Discord、migration/import、updater/distribution、Local API、Custom
Web UI 与 full external-client credential pairing/revocation 不进入 M7。

`Home`、独立 Unity/Backups/Operations、Activity/Diagnostics 分离和 About 等 v4-specific surface 不能仅因 route 已存在就视为
布局获批。它们必须保留已实现业务能力，同时在 v3 context/workflow 中找到可识别位置；重大偏离继续是 `pending`。

### Visual architecture and component policy

`19267230507071dc61ba306b98c8cfdd113e9ea2` 的功能接线保持有效，但状态固定为：

```text
technically_valid
but
rejected_for_m7_visual_design_acceptance
```

源码审计确认 `@material/web 2.5.0` production import 为 0、rendered `md-*` element 为 0；Core 与 Portable UI 使用 native
button/input/select/textarea/progress 和 custom CSS。该事实分类为
`material_web_dependency_present_but_component_system_not_adopted`，主题分类为
`material_theme_not_actually_wired`。

Material Web 成为 interactive component foundation。只要 2.5.0 提供对应组件，Core 与 Portable UI 都必须经
`@alcomd/ui` 的窄 typed wrapper 使用真实 Material behavior；不得自行模拟 ripple/state layer。应用 shell、data table、page
grid、split pane 等 Material Web 未提供的结构，使用 semantic HTML + 最小 `@alcomd/ui` layout primitive + 同一 MD3 tokens。
`apps/alcomd-gui` 原则上不散布 direct Material imports。

React 19 custom-element properties/events/boolean/form/ref/typing/validation 必须在 H0 用真实 2.5.0 component test 冻结；优先使用
React 19 原生支持，不增加第三方 adapter。新增 dependency 仍是人工停止点。

### RPC coverage authority

`docs/exec-plans/M7-gui-rpc-coverage.md` 是 M1-M7 public RPC 到 official GUI 的枚举矩阵。每个 GUI-relevant method 必须有
page、user action、read/write、confirmation、Operation、state handling 与 test owner；transport/internal method 显式标记
`not_applicable`。矩阵由 protocol constants 与 official client 自动枚举结果核对，不凭记忆删减。

### Typed GUI adapter

所有业务保持 `React -> typed Tauri adapter -> alcomd-client -> RPC -> alcomd-application`。允许使用 app-private closed
`gui_query`/`gui_command` discriminated variants 收敛 adapter，但 Rust/TypeScript 必须 exhaustive match，unknown variant
fail closed，且不能成为 arbitrary method/JSON passthrough。GUI 不直接连接 daemon socket、SQLite、project filesystem、
repository HTTP、package cache 或 Extension Host。

### Common states and mutation semantics

每个 data route 实现 `initial-loading`、保留 last-known-good 的 `refreshing`、`empty`、`error` 与 `disconnected`；mutation
route 另实现 `confirmation-required`、`operation-running`、`success`、`failed`、`cancelled`。permission denial 不是 empty；
`internal_error` 只展示 stable code 与 `diagnosticId`。

高影响写入保持 `Plan -> review daemon ChangeSet/safe risk summary -> explicit Apply -> OperationId -> progress -> terminal
result`。stale/revision conflict 回到明确 review，不静默重新 Plan；route/window close 不等于取消 Operation。

### Settings Config Schema v1

M7 获准使用 `config/settings.toml`，由 daemon 单写；不建立 State Schema v10。公开 permission 只有 `settings.read`、
`settings.manage`，公开 RPC 只有 `settings.get`、`settings.update`。Config v1 的 closed fields 为：

- `appearance.mode = system|light|dark`；
- `appearance.sourceColor = null|canonical #RRGGBB`；
- `appearance.density = default|compact`；
- `appearance.motion = system|reduced`；
- `locale = system|canonical supported locale`。

`settings.get` 返回 `configSchema`、revision、full settings；`settings.update` 接收 expectedRevision 与 closed partial update，
返回新 revision 和 normalized full settings。revision 为 checked monotonic u64，stale 复用既有 revision-conflict family。
文件必须 strict/unknown-field fail closed、bounded UTF-8、deterministic serialization、crash-safe replacement。只有 storage/RPC
完整接线后 hello 才广告 `configSchema: 1`。不得新增 updater/MCP/Discord/migration/extension arbitrary key/value 设置。

### Activity and Diagnostics

公开 permission 限定 `activity.read`、`diagnostics.read`，公开 RPC 限定 `activity.list`、`diagnostics.list`。Activity 是现有
durable Event/Operation 的 bounded、deterministic、keyset-paginated safe projection，不创建第二份 history；不得返回 raw
request/result/error、完整路径、token、extension payload 或 UI form value。前端不自行 join `events.list`/`operations.list`。

Diagnostics v1 只返回 redacted structured items：occurredAt、severity、subsystem、stable code、optional diagnosticId 与 bounded
safe summary；不提供 raw log export，不建立第二个 durable log table，不暴露 argv/env/SQL/Debug/backtrace/extension-owned value
或 Portable UI payload。若现有 safe evidence 无法形成 read model 且需要新 durable logging subsystem，立即停止审批。

### Accessibility and manual ownership

M7 automation 使用唯一获批 devDependency `@playwright/test = 1.62.1` 与其匹配 Chromium revision，覆盖 semantic HTML、
landmark/heading/name/label/validation/live region、drawer/dialog focus trap/restore、route focus、keyboard-only flow、Tab/arrow/Escape、
visible focus、reduced motion、light/dark contrast、200% deterministic layout evidence、320 CSS px、error/disabled/progress 与全部
17 Portable UI node 的真实 DOM semantics。自动化不复活 iframe/child WebView probe，也不宣称真实平台辅助技术认证。

M7 manual owner 是一次真实交互桌面的 visual screenshot、flow、keyboard/focus smoke 与项目所有者签收。M11 owner 是真实 v3
entry/flow/migration-state/screenshot differential parity。M12 owner 是 Windows Narrator、macOS VoiceOver、Linux screen reader
及真实平台 client accessibility runtime validation。

visual work 不再等到全部页面完成后一次性查看。H1-H5 期间设置四个阻塞 gate：

1. Visual Gate 1：main shell/navigation；
2. Visual Gate 2：Projects/packages/repositories；
3. Visual Gate 3：remaining core pages；
4. Visual Gate 4：Portable UI/settings/diagnostics。

每个 gate 必须启动真实 GUI、采集仓库外 bounded screenshot 并取得项目所有者签收后才能进入下一大块。最终 H7 还要检查
Material hover/pressed/focus/ripple/disabled behavior，不以自动测试或截图替代真实交互。

不得同时保留旧 Web UI和Portable UI parser/world/renderer，不建立通用 UI/workflow engine。

## 已批准 production 修改范围

```text
apps/alcomd/
apps/alcomd-extension-host/
apps/alcomd-gui/
crates/alcomd-application/
crates/alcomd-client/
crates/alcomd-extensions/
crates/alcomd-protocol/
crates/alcomd-store/
crates/alcomd-testing/
packages/alcomd-sdk/
packages/alcomd-ui/
.github/workflows/ci.yml
docs/adr/ docs/exec-plans/ docs/status.md docs/testing/
feature-parity.toml
specs/config/ specs/extensions/ specs/gui/ specs/rpc/ specs/security/ specs/storage/
```

唯一 manifest/lock 例外是获批的 `apps/alcomd-gui/package.json` exact devDependency `@playwright/test = 1.62.1` 与根
`package-lock.json` 正常 Playwright closure。其余 Cargo/npm manifest/lock、dependency graph、unsafe whitelist、platform API、
Tauri unstable/capability、iframe/child WebView/WebviewWindow 以及任何 M8/M9 production wiring 仍禁止。

## 明确排除

- M8 MCP backend/management product logic、M9 Discord IPC/Presence product logic；fixtures只证明UI表达能力；
- custom Web UI、iframe/child WebView/WebviewWindow、asset/origin/CSP/Tauri capability；
- Local API、migration/import/bootstrap/updater/installer/signing/dist、update channel 与 full external-client credential pairing；
- WASI 0.3、component-model-async、wasmtime-wasi、guest thread、新 dependency；
- M8/M9 production wiring，以及 Portable UI 合同外的新 public RPC/Permission/error/node/action/surface。

## 验证与 release blockers

每个 production slice 运行 targeted tests；最终候选运行 format/clippy/workspace tests、xtask、metadata、baseline freeze、
npm check/build、Playwright Chromium browser suite、Tauri no-bundle 与 diff check。必须证明除获批 Playwright dev-only closure
外 Cargo/npm manifests与三份 lockfile依赖图不变、production Vite bundle不含 Playwright、unsafe/platform API 不变。

后续 first-party private node/page/command/permission、partial v1 renderer、GUI-to-Host direct channel、renderer作为business
authority均是M7 blocker。
native interactive control 在 Material Web 已有对应组件时继续以 custom CSS 模拟、Core/Portable UI 使用不同 component
foundation、未批准的 v3 macro layout deviation 或跳过任一 Visual Gate，同样是 M7 blocker。
M11真实v3 fixture缺失继续阻塞GUI differential parity，但不阻塞 `gui.m7-core-surfaces`。M12继续承担Win10/Win11安装/启动/
WebView2/update/uninstall，以及 Narrator/VoiceOver/Linux screen-reader 和真实平台 accessibility runtime validation。

## Planned visual realignment slices（未批准 production）

1. **H0 Material foundation**：`@alcomd/ui` 封装审计中真实需要的 Material Web components；接通真实 MD3 color/type/shape/
   elevation/state tokens；验证 React 19 integration、interaction/ripple、component accessibility 和Core/Portable共用层。
2. **H1 v3-faithful shell/navigation**：wide persistent sidebar、single content canvas、page toolbar 与 narrow adaptive drawer；完成
   Visual Gate 1。
3. **H2 Projects/packages/repositories**：恢复 Projects header/list-grid/create、Project package-centric workspace、Packages &
   Templates grouping、repository table/actions；完成 Visual Gate 2。
4. **H3 Templates/Unity/backups/Operations**：在 v3 context workflow中安置 v4 durable surfaces，不改 Plan/Apply/Operation；完成
   Visual Gate 3 的第一部分。
5. **H4 Extensions/Portable UI**：Extensions utility placement、host chrome、Portable node renderer全部使用共同Material components；
   不改 Portable UI protocol；完成 Visual Gate 4 的第一部分。
6. **H5 Settings/Activity/Diagnostics/About**：恢复 grouped Settings 与 observability utility关系，保留 Config v1/A-026权限分离；
   完成 Visual Gate 3/4。
7. **H6 responsive/a11y/Playwright regression**：覆盖 v3 macro navigation/page grouping/action placement、Material element presence和
   observable interaction、theme propagation、wide/narrow/200%/reduced motion；不测试Material shadow DOM私有细节。
8. **H7 manual visual signoff**：执行更新后的真实GUI checklist和最终screenshots；再进入候选Hosted CI/push审批。

H0-H7 只重组 shell、composition、component rendering 和 visual hierarchy；不得重新设计 RPC、Plan/Apply、Operation、Settings、
Activity、Diagnostics 或 Portable UI authority。

## Progress log

- 2026-08-24：M6完成；M7最初研究WebView-based UI。
- 2026-08-24至2026-08-25：iframe CI `32752875840` 与Windows managed-child diagnostic形成rejected-design evidence，
  没有production security/compatibility success。
- 2026-08-25：项目所有者批准GUI-neutral Portable UI architecture reset；commit
  `12089661f7c694f059aaf172344809ff814716f6`。
- 2026-08-25：独立cleanup commit `c05c5d2c712dd7a791401efc22b8dd19b88023a6`删除obsolete test-only harness与CI
  probe，保留历史evidence与普通Tauri gates。
- 2026-08-25：形成本Portable UI Stop A proposal、synthetic fixtures与contract tests；active/production未修改。
- 2026-08-25：项目所有者认可Stop A总体架构方向，并要求完成单一隐式页面、required guest-ui、exact replay/form/tree、
  InvocationContext分类、State v9最小列与EOF格式的窄review closure；production仍未批准。
- 2026-08-25：项目所有者最终批准 Portable UI Stop A production implementation；独立执行语义补充 commit
  `41f049365894f347d59b44eb7a41ac41c95e64df` 冻结 InvocationContext、render purity、close/replay/race/locale 与脱敏边界。
- 2026-08-25：Slice B0 完成 active Manifest/package profile/WIT ABI v1 direct replacement、State v9、immutable typed
  `ui_protocol`、`extensions.ui.use` 与四个 RPC DTO foundation；backend fixture 使用同一 mandatory guest-ui world，未保留
  Web UI compatibility path。
- 2026-08-26：Slice B1 完成 daemon-memory-only `UiSessionCoordinator`、InvocationContext exact echo/capability matrix、
  real guest-ui Host binding、interactive lifecycle、dual authority、render purity、current-only replay、monotonic snapshot、
  disconnect/disable/digest/grant/generation invalidation 与 schema/capability advertising。真实 daemon/RPC/Host 测试证明
  UI-only open/close、background Host 保留、render write 不落库、action write 落库、cross-connection denial、disconnect cleanup、
  malformed document terminate/crash/quarantine；未开始 official renderer。
- 2026-08-26：Slice C 完成 official main-shell route、五个窄 typed Tauri-to-client adapter、全部 17 node/2 action 的
  exhaustive React renderer、memory-only revision-bound form draft、host-owned identity/error/discard confirmation、
  light/dark/system/source-color/density/locale、320 CSS px/reduced-motion/focus/ARIA 边界。共享 MCP/Discord Fixture 在 Node 24
  真实 consumer test 中通过；未增加 dependency、lockfile、unsafe、platform API、Tauri capability 或 private Host channel。
- 2026-08-26：Slice D 完成独立 non-Tauri headless consumer与共享 Fixture semantic conformance、hostile document/action
  matrix、session exact cap/rate/timeout/u64 overflow、Host binding forge/stale rejection、dual-authority route永久门禁和敏感
  payload无日志sink/close后不可访问证据；复用真实 daemon/RPC/Host lifecycle、Store scope/revoke及first/third parity测试，
  未增加 dependency、lockfile、unsafe、platform API或公共合同。
- 2026-08-26：最终完整本地 fmt/clippy/workspace tests、xtask、metadata、baseline freeze、npm check/build、Tauri release
  no-bundle 与 diff/lock/authority gate 全部通过，形成等待项目所有者明确 push 批准的本地技术候选；尚无 Hosted CI 或
  最终人工验收证据。
- 2026-08-26：Portable UI candidate `aa1323430252ed21995284a7b36dd36e45a15e0a` 正常 push 后由 Hosted CI
  `32877438910` 在 Windows Server 2025、Ubuntu 22.04 与 macOS 15 arm64 全部验证成功；completion audit 未将其冒充完整
  M7 acceptance。
- 2026-08-26：项目所有者确认 architecture reset 误删 official GUI scope，批准恢复 E1-G3、四个新 RPC、四个 permission、
  Config Schema v1，并将 v3 differential parity 与三平台 screen-reader smoke 分别归属 M11 与 M12。
- 2026-08-26：唯一新增前端测试依赖 `@playwright/test = 1.62.1` 获批；Chromium-only browser automation 不进入 production
  bundle/runtime，也不作为 WebView2/WebKitGTK/WKWebView compatibility certification。
- 2026-08-26：Official GUI Completion E1-F3 完成 M1-M7 GUI-relevant Core routes、typed Tauri/client/RPC adapter、
  Plan/Apply/Operation flows、Config Schema v1 durable settings、safe Activity/Diagnostics projection 与统一页面状态；只新增已批准
  的四个 RPC 和四个 permission，没有 State DB 新表、generic RPC passthrough、新 Cargo dependency、unsafe 或平台 API。
- 2026-08-26：G1 Chromium automation 使用 `@playwright/test 1.62.1` 匹配的 Chromium revision `1234`，12 项 browser tests
  全部通过，覆盖 keyboard/focus/dialog/form/live region、320 CSS px、deterministic 200% layout、reduced motion、light/dark targeted
  contrast、全部 17 个 Portable UI node、dirty discard 及 Core success/stale/failed/cancelled flows。该证据只证明 Web frontend
  semantics，不替代 M12 的真实 WebView/辅助技术验证。
- 2026-08-26：G3 本地 fmt/clippy/locked Workspace tests、xtask、metadata、baseline freeze、npm ci/check/build、Playwright、
  Tauri release no-bundle 与 diff/lock/authority gates 通过；production Vite bundle 不含 Playwright，production npm graph 不含
  Playwright。当时 G2 checklist 已准备但尚未执行，最终三平台 Hosted CI 与项目所有者人工验收仍待 push 后取得。
- 2026-08-26：项目所有者实际查看 functional candidate `19267230507071dc61ba306b98c8cfdd113e9ea2` 后，在正式
  checklist 开始前拒绝 visual/layout acceptance；候选未 push。功能、RPC、Config/Activity/Diagnostics、Portable UI、
  Plan/Apply与自动化证据保留，official GUI shell/rendering进入realignment。
- 2026-08-26：只读审计冻结 v3 final source/reference 的 macro layout baseline、v3->v4 mapping、当前逐页 gap、Material Web
  2.5.0 control/theme/integration inventory；新增 A-033 约束 A-020。H0-H7 仅完成规划，production未开始。

## 下一停止点

本 visual realignment audit 形成独立未 push planning commit 后停止。等待项目所有者批准 v3 layout baseline、v4 MD3 mapping、
Material component policy、所有 `pending` deviation 与 H0-H7，再开始任何 GUI production redesign。不得 push，不得开始 M8/M9。
