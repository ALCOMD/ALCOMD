# M7：官方 GUI 与 Portable Extension UI

状态：**M7 IN_PROGRESS / H7 IN_PROGRESS / H6 STAGE_ACCEPTED_WITH_FOLLOWUPS / H5 STAGE_ACCEPTED_WITH_FOLLOWUPS / H4 STAGE_ACCEPTED_WITH_FOLLOWUPS / H3 STAGE_ACCEPTED_WITH_FOLLOWUPS / H2-B STAGE_ACCEPTED_WITH_FOLLOWUPS / H2-A ACCEPTED / Visual Gate 2 PASS**。唯一工作目录为标准 `ALCOMD`，
活动分支为 `main`；当前阶段与验证结果统一见 `docs/status.md`。已通过的 M6、Portable UI B0-D、
Project/Package P0-P8 和 exact Unity 技术检查点保持其原始 SHA/证据，不替代本轮 GUI 人工验收。

## 当前有效合同

- GUI 继承 A-033 的 v3 用户模型，使用真实 Material Web 控件和共享组件。Projects、包管理及
  已获准的其他既有页面基础可用性修正已在 H2-A 验收；H2–H6 原签收保持，当前只推进已批准 H7 最终整体验收。
  Open Unity 保留单个 leading play 图标和 Material 按钮的 18px 图标尺寸。Projects 的视图入口
  显示当前模式并保留视图偏好；内容包装透明，卡片和表格使用自己的表面。
- `packages.queryProjectCandidates` / `packages.candidates.v1` 按已批准的
  `M7-package-candidate-evidence-stop-a.md` 提供完整、来源保真的候选证据。GUI 不解析
  SemVer/Unity，不以可选元数据首页推断来源缺失或可操作性；行内版本、条件更新与批量操作仍
  复用原有单次 Plan、显式 Apply 和一个 Operation，不更改普通 resolver/来源/恢复策略。
- 当前数据基线为 **State v16**。v15 引入独立的 `lastModifiedAtMs`；v16 移除项目观察时间，
  保留安装扫描、writer 检查及迁移专有恢复证据。候选/迁移的权威投影显式白名单排除展示时间，
  保持原冻结字节与 hash；旧未 Apply 的 Unity 迁移计划按合同过期，不重签原审批。
- 按 A-034 保留 Schema 识别机制，hello 报告成功初始化后的真实存储版本。未发布开发库不
  承诺长期兼容，仅为明确需要保留的数据维护迁移；当前不降回 v1、不新增版本、不删除现有迁移。
  发布前初始 Schema 整理是独立审计任务，不自动清空数据库、Plan、Operation 或恢复证据。
  本轮 hello 修复只纠正已有 `dataSchema` 事实值与内部构造参数，不改 wire DTO/Schema 形状、
  RPC major 或 capability，属于非破坏性修正；由 dispatcher、兼容形状和实际 SQLite 回归验证。
- 当前设置合同为 **Config4**，按 2026-11-02 明确批准彻底删除 GUI 紧凑模式及 `appearance.density`。
  application 持有权限、CAS、字段校验、候选缓存和取消一致性；
  `ConfigStore` 窄 port 的文件适配器位于 store，负责 strict 16 KiB TOML、Config1–3→4 和
  原子替换/恢复。普通设置修改即保存，以当前连接内的串行窄 PATCH 协调；失败保留意图、
  显式 Retry 先重读，未知已提交不重复写，换连接停止旧队列，不保留全局 Save/Discard 栏。
- 主题编辑器保持独立 Save/Cancel、实时预览和冲突三方合并；仅明确保存 sourceColor/mode/scheme。
  默认种子 `#6CB6FF`、Vibrant、系统明暗及九方案不变。共享 Material HEX/拾色器/HCT/分段/
  Radio 使用标准控件；主题表单填满公共内容盒、卡片四周 24px，弹窗 padding 由 Material 提供。
  HCT 数值使用 Slider 自带 labeled 提示；短请求保持动作名称，不插入瞬时状态行。
- 字体只引用本机 Noto Sans/Mono 及中日 CJK fallback，不打包或联网加载字体。三语言为
  en-US/zh-CN/ja-JP，使用静态词条和 React 文本插值；外部内容不翻译、不解释 HTML。
  动画三态为 system/full/reduced，默认 system；共享适配器处理 CSS/Shadow DOM/弹层及滚动条，
  不恢复已撤除的切页动画。
- 公共 DesktopList 统一列、排序、截断和稳定正文行盒；PageHeader、SplitButton、Dialog、
  ScrollArea 和 SegmentedButtons 统一持有布局/生命周期。分段控件按真实字体预留最长选中态，
  不写死文案宽度。滚动条只在实际滚动或轨道悬停时唤出，沿用库计时和销毁语义；textarea
  内部编辑滚动保持原生。离线页面恢复同时重新协商当前连接能力，不依赖切页刷新能力。
- 异步 Plan 的触发控件通过稳定本地 origin ref 返回焦点；共享 Dialog 等待公开 closed、
  业务控件恢复可用及 Material updateComplete，不因修复焦点而放宽 disabled/eligibility。
  MenuItem 统一消费公开 click/Enter/Space 选择并只在 closed 后派发一次；Escape 不派发。
  Select 由真实 busy 状态控制禁用，选中标签按公开生命周期同步，不增加输入/设置写入。
- Unity Hub 位置复用 settings.get/update、settings.read/manage 与 Config4 `unity.hubPath`；
  有界只读发现和路径规范化由 platform 提供，不加注册表/PATH/Hub CLI 扫描。路径草稿 Enter
  或离开操作组保存，Esc 撤销，IME/同组焦点切换/无修改不写；选择和自动查找共享串行 CAS。
- `unity.hub.open` / `unity.hub.open.v1` 要求 settings.read 和独立 unity.hub.open，
  初期仅认证 local-owner；application 在协调锁内重验 revision/有效路径，独立 Launcher
  预建有界 worker 后 spawn 并回收自己的 Child，最多四个，不 kill 用户进程或自动重试未知结果。
  仅无显式偏好且确实未找到时返回 download_required，由闭合无参数 Tauri adapter 打开固定
  官方下载页。Auto detect/Open 位于卡片右上角、Select 留在编辑行；未确认草稿阻止启动。
  No shell 指应用不构造、调用或通过 shell 执行命令，不拼接/解释 shell 参数；保留标准库与
  OS 的 ENOEXEC/解释器回退语义。spawn 成功仅为 OS 接受，不代表 Hub 已正常运行、厂商
  身份、退出成功或窗口就绪；同步拒绝、后续退出与资源回收分别验证。
- 正式合同与证据分别见 `specs/config/settings-v4.*`（Config1–3 仅为迁移历史）、
  `specs/config/theme-controls.proposal.contract.json`、
  `specs/config/unity-hub-location.proposal.contract.json`、
  `specs/storage/state-v15-project-last-modified.contract.json`、
  `specs/storage/state-v16-remove-project-observed.contract.json` 和对应 testing 文档。
  名为 proposal 的已批准合同仍由这些既有文件直接维护，不建立第二份有效合同。

本轮 Schema 策略与架构审查修复的逐轮证据见
[修复验证记录](../testing/m7-architecture-review-fixes-20261031.md)；当前阶段仍统一见 `docs/status.md`。
测试门禁使用 ready 且由测试 handle 持有端点的 daemon，Windows 占用时在创建客户端前失败；
生产初始化顺序和每用户唯一端点保持。Chromium suite 将三个现有 fixture 静态预构建，保留
development React/StrictMode、原断言、零重试与 30 秒门槛；不改变生产 Vite 配置、依赖版本
或系统 TCP 参数。H2-B 资源加载专项仅在测试浏览器回移 Playwright 官方的
`TcpPortRandomizationWin` 修正，完整保留原 16 项默认禁用列表及其他参数，并用版本 guard
要求升级时复核。fixture 以 `Connection: close` 在响应完成后关闭连接，减少 context 销毁
留下的客户端 TIME_WAIT；所有资源继续通过同一 Vite fixture 的真实 HTTP 加载。专项的
原失败轮、连接对照与该候选独立完整 292/0 结果见
[H2-B 验证记录](../testing/m7-h2-b-20261101.md#chromium-资源加载专项)。前置完整结果和真实桌面范围见
[测试门禁与桌面验证](../testing/m7-test-gate-closure-20261031.md)。
H2-A 本轮真实 Apply、失效 Plan、焦点、筛选、偏好重启及签收图集见
[Visual Gate 2 材料](../testing/m7-visual-gate-2-20261031.md)。

项目所有者于 2026-10-31 明确批准 P6 wire 修复：P6 source selector、reinstall selection 和
bulk intent 的 Rust enum variant fields 使用已冻结的 camelCase；候选查询使用独立公开
source DTO，保持已冻结的 snake_case Schema/vectors 不变。该修复会替换当前错误接受的
P6 snake_case 字段，属于实际 wire 的破坏性纠正；按此次未发布合同审批直接修复，RPC v1、
State16、Config3、权限和依赖保持。不添加旧字段别名、双格式支持或 GUI 回退。

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
4. 实现获批的 Config3、settings/activity/diagnostics、候选读取与 Unity Hub 窄合同，并保持各自权限和版本边界；
5. Chromium browser-level DOM/component accessibility automation，以及 A-033 所要求的 v3 user-model continuity、Material component
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

### Portable UI 存储与 renderer

M6 的 v8 后，Portable UI 在 v9 为 extensions 和 immutable extension_plans 各增 nullable `ui_protocol`，合法值 NULL 或
`portable-v1`。不保存页面常量，不建session/Snapshot/replay/action/draft/renderer/browser/cache/workflow table。v8->v9
单事务 migration 已保留 rows/FK/revision/Event sequence，existing 默认 NULL，future schema fail closed；B1 完整接线后
当前 hello 广告实际 State v16 与 `extensions.ui.portable.v1`；v9 是该字段的历史引入版本。

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
3. Slice F1：当前 Config3、`settings.toml` 单 writer durability、ConfigStore port、settings permission/RPC；
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
- `docs/gui/m7-material-component-audit.md`；
- `docs/gui/m7-user-model-continuity.md`；
- `docs/gui/m7-information-architecture-candidates.md`（historical exploration only）。

Official GUI 默认继承 v3 用户模型：Projects、package-centric Project workflow、Packages & Templates、Extensions、Settings、
Log 与 footer/progress utilities。v4 的首要目标是在同一用户结构中提供 MD3 现代化和更可靠、安全、可观察的实现，而不是为
daemon、RPC、Operation、Extension Host、Portable UI、permission/scope 或 State Schema 建立新产品领域。

只有 v3 无对应用户用例、用户需要独立工作区、无法自然容纳于既有概念且独立入口明确提升可用性时，才考虑新增 user-level
surface。Candidate A/B/C 因仍从“如何给 v4 domain 找位置”出发，现只保留为 historical exploration。

以下是当前仍有效的 implementation route/capability inventory，不是 proposed sidebar hierarchy；route 可以保留作 deep link、
secondary surface 或 context workspace：

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

当前 minimum-change proposal 将 Projects 恢复为 home-equivalent，将 Repositories/Templates移回Packages & Templates，将
project Unity/Backups保留在Project context，将global Unity放入Settings，将Operations表达为context progress与secondary Task
Center，将Activity/Diagnostics放入同一Log utility，并把About/version放入footer/Settings。Extensions是v3既有用户概念，保持
其utility身份和大致归属；permission/quarantine/Portable UI进入Extension detail。Portable UI v1不提供sidebar/menu/toolbar/
arbitrary navigation contribution。该proposal已获项目所有者批准。

### Visual architecture and component policy

`19267230507071dc61ba306b98c8cfdd113e9ea2` 的功能接线保持有效，但状态固定为：

```text
functional_implementation_candidate
but
rejected_for_visual_and_information_architecture_acceptance
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

### Settings Config4 与存储权威

当前活动格式为 `specs/config/settings-v4.md` / `.schema.json`。closed 字段为：
appearance mode/sourceColor/scheme/motion、locale、packages 三项偏好及可清除的 unity.hubPath；
精确枚举、边界和 null 语义以现有机器合同为准。所有非 nullable PATCH 字段显式 null 整次拒绝，
sourceColor:null 和 hubPath:null 分别表示清除颜色和恢复 Hub 默认发现。

settings.get/update 要求原 settings.read/manage，返回 Config4、revision、完整归一化设置及
派生 unityHub 证据；revision checked monotonic u64，文件 strict UTF-8/unknown-field fail closed，
最多 16 KiB，确定性序列化、可恢复原子替换。派生文件状态不推进 revision，保存后状态读取
失败不把已提交设置伪报为失败。application 只经 ConfigStore port 保存归一化快照，Store
负责文件格式和恢复，不增加配置数据库表、通用 key/value、GUI 直写或第二套权威。

主题确认、普通即时设置和 Hub 配置共用 UI 串行 CAS；状态读取、能力协商和故障恢复仍通过
typed client/RPC。当前 hello.configSchema=4。Config1–3 只作为保留原数据的迁移输入，不是当前写入或
返回格式；旧 density 校验后丢弃，保留原 revision 与其他设置，Config4 不接受该字段。
开发兼容策略按 A-034，发布前另行审计这些历史输入的保留需求。

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
crates/alcomd-domain/
crates/alcomd-platform/
crates/alcomd-vpm/
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

当前获批依赖/资产范围：

- 唯一前端浏览器测试依赖 `@playwright/test = 1.62.1`，不进入 production 图。
- `@alcomd/ui` 直接 production 依赖为原有 `@material/web = 2.5.0`、配色引擎
  `@material/material-color-utilities = 0.4.0`（Apache-2.0）和
  `overlayscrollbars = 2.17.0`（MIT）；两项新增库均无额外 runtime dependency/native addon。
- Material Symbols 为固定 Google upstream commit 的 only-used vendored SVG；新增 palette
  只使用同一 upstream/样式并登记 path/hash，不存在 Material Symbols npm 包或第二图标库。
- npm lock 的已批准开发安全补丁为 Vite 7.3.7 和 source-map-js 1.2.2，不加新库/override。
  许可和维护风险见 THIRD_PARTY_NOTICES.md、当前合同及依赖评估记录。

上述限定支持相应 `packages/alcomd-ui/package.json`、GUI check script 与根 package-lock
改动；不构成后续依赖升级许可。候选读取允许窄 VPM primitive、domain/Store read snapshot
及 typed DTO 接线；已批准的 Hub 发现/启动在现有 platform adapter 内使用既有 std API。
不新增 production crate、Cargo 依赖、unsafe 文件或未批准 native API；不扩大 Tauri
capability、不启用 unstable、不创建 iframe/child WebView/WebviewWindow 或 M8/M9 接线。

## 明确排除

- M8 MCP backend/management product logic、M9 Discord IPC/Presence product logic；fixtures只证明UI表达能力；
- custom Web UI、iframe/child WebView/WebviewWindow、asset/origin/CSP/Tauri capability；
- Local API、migration/import/bootstrap/updater/installer/signing/dist、update channel 与 full external-client credential pairing；
- WASI 0.3、component-model-async、wasmtime-wasi、guest thread，以及下述清单外的新 dependency；
- M8/M9 production wiring，以及 Portable UI 合同外的新 public RPC/Permission/error/node/action/surface。

## 验证与 release blockers

验证按风险和开发阶段安排，不以小修改或独立提交为完整门禁的触发点：

| 时机 | 验证安排 |
| --- | --- |
| 日常修改 | 受影响模块的定向测试及必要的 fmt/Clippy、前端 check；仅文档修改执行 metadata、引用与 diff check |
| 功能模块完成 | 集中覆盖该模块及相关回归路径；H2-B 按模板分页/创建、Plan review/终态刷新、资源生命周期分组，涉及共享组件时覆盖 H2-A 对应的焦点、权限、重连和包管理路径 |
| 稳定候选与阶段收尾 | 合并相关修改后集中运行适用完整门禁；跨平台、安全、依赖和重要架构变更覆盖相应平台与安全回归，避免反复编译同一批依赖 |

高风险问题或回归失败及时执行必要的额外测试，不机械等待模块或阶段结束。最终候选基于其实际
SHA 运行 format/Clippy/Workspace tests、xtask、metadata、baseline freeze、npm check/build、
完整 Playwright suite、Tauri release no-bundle 及 diff check；不逐项或逐提交重复完整测试与构建。
三平台 Hosted CI 和 CodeQL 集中用于获准的重要技术检查点、阶段收尾及最终验收候选；
在已有授权范围内批量提交、集中推送，不新增推送授权，不仅因本次流程调整取消运行中的 CI。
跨平台与依赖安全技术检查点已在 `97c761f` 取得同一 SHA 三平台完整 required jobs 和 CodeQL 成功。
H2-B `0ef5b08` 已按项目所有者本轮明确授权推送，当前三平台 CI/CodeQL 成功；停止在 Resources
人工验收。未经新的推送授权不推送额外提交，不因状态文档整理重跑既有完整验证。

本地完整验证优先复用 `scripts/check.ps1` / `scripts/check.sh`，其已包含 fmt、Clippy、
Workspace tests、xtask、前端 check/build 与 metadata；不在其后逐项重跑，也不叠加另一个
包含相同完整检查的聚合入口。CLI auto-start、冻结基线、浏览器及 release 构建等未覆盖部分
按现有独立入口补齐，保留各自实际结果与锁文件校验。

依赖变化仅允许上一节精确清单，三份 lockfile 不得产生无法解释的增量；production Vite 图
不含 Playwright 或完整 icon corpus，Cargo/unsafe/native API 边界保持。各轮失败、定向重试和
完整 suite 分别记录，不拼接不同提交的通过结果，不将未执行标记为 PASS；保留全部测试、
断言和安全门禁。运行中的真实环境未经授权不为测试而停止。
Schema 的机器形状检查与 daemon 实际版本一致性、页面完整来源证据、离线恢复和公共列表
混排行盒须有执行回归，不能以静态门禁通过替代。

后续 first-party private node/page/command/permission、partial v1 renderer、GUI-to-Host direct channel、renderer作为business
authority均是M7 blocker。
native interactive control 在 Material Web 已有对应组件时继续以 custom CSS 模拟、Core/Portable UI 使用不同 component
foundation、违反已批准 minimum-change user model/A-033 continuity 或包含 `UNJUSTIFIED_DEVIATION`、跳过任一 Visual Gate，
同样是 M7 blocker。
M11真实v3 fixture缺失继续阻塞GUI differential parity，但不阻塞 `gui.m7-core-surfaces`。M12继续承担Win10/Win11安装/启动/
WebView2/update/uninstall，以及 Narrator/VoiceOver/Linux screen-reader 和真实平台 accessibility runtime validation。

## Visual realignment slices

2026-09-19 owner-authorized supplement: before Visual Gate 2 acceptance, correct basic layout and usability
of the other existing GUI pages, referencing frozen v3 design to the same degree as Projects. This explicitly
allows resource tables/segmented navigation, grouped Settings with its Unity entry, Logs Activity/Diagnostics,
existing task/extension/about/backup/detail presentation and normal return paths. Reuse current Material
components and RPC contracts; do not copy v3 source. Preserve Projects/package workspace and confirmed icons,
avoid narrow-window adaptation, new dependencies, contracts, migrations and production platform APIs. This is
not blanket approval of the remaining H2/H3-H7 roadmap or visual signoff. Browser fixtures may establish frontend
evidence; Computer Use interruption or missing Tauri evidence must remain separate. Continue straightforward
fixes and defer uncertain contract-dependent work without blocking unrelated corrections.

1. **H0 Material foundation（已批准）**：`@alcomd/ui` 封装审计中真实需要的 Material Web components；接通真实 MD3 color/type/shape/
   elevation/state tokens；验证 React 19 integration、interaction/ripple、component accessibility 和Core/Portable共用层。
2. **H1 v3-reference Projects shell/navigation（Visual Gate 1 已通过）**：首次实现因 global top app bar、admin-style sidebar grouping、
   centered max-width page、hero typography、giant empty/form cards 与过低桌面密度被标记为
   `rejected_for_visual_direction`。重新执行必须先完成 `docs/gui/alcomd3-v3-layout-baseline.md` 的直接视觉拆解，再只实现
   wide desktop 的 recognizable sidebar/content canvas、Projects page toolbar、sort/filter 与 dense content。narrow adaptive
   不能反向决定本 gate 的宽屏结构；完成 first runnable prototype 后立即停止 Visual Gate 1。
3. **H2 Projects/Packages & Templates（H2-A 与上述既有页面修正已验收）**：完成 package-centric workspace、
   候选读取/行内版本/批量、统一公共组件及已批准的资源/模板/设置/日志/扩展/备份基础交互。
   只实现本文件列明的窄公开合同。项目所有者于 2026-10-31 签收 Visual Gate 2；H2-B 已获下述技术收尾通过后的条件批准，其他产品范围仍未批准。
4. **H3 contextual v4 enhancements（阶段性人工验收通过，保留非阻塞体验问题）**：把 global/project Unity、Backups 与 Operations 增强嵌入既有 Settings/Project/progress workflow，不改
   Plan/Apply/Operation；完成 Visual Gate 3 的第一部分。项目所有者于 2026-11-02 明确签收本轮，并确认 Unity 自动检测、同记录刷新和路径显示；不表示体验完善或整个 Visual Gate 3/M7 完成。项目上下文、共享提醒和任务机制保持，不重复验收已检查页面。
5. **H4 Extensions/Portable UI（阶段性人工验收通过，保留非阻塞体验问题）**：保留Extensions既有用户概念和utility归属；permission/quarantine/Portable UI位于detail，host
   chrome与renderer全部使用共同Material components；不改 Portable UI protocol。所有者于 2026-11-02 认可安装/启停/卸载、权限管理、同窗口功能页与共享错误可读性；保留表单、超时、外部断线和 M8/M9 正式业务未覆盖项，不表示整个 Visual Gate 4/M7 完成。
6. **H5 utilities/observability（阶段性人工验收通过，保留非阻塞体验问题）**：保留 grouped Settings、Log内Activity/Diagnostics与footer About/version，
   只补真实缺失功能及必要整改，保留 A-026 权限分离，复用共享 Material、错误显示与只读 `displayPath`；当前 Config4 窄变更见下方批准记录。
   所有者于 2026-11-02 明确签收，认可删除紧凑模式；保留一般文案、布局与过度设计待办，不表示体验完善或 M7 完成。
7. **H6 responsive/a11y/Playwright regression（阶段性人工验收通过，保留非阻塞体验问题）**：覆盖已批准 minimum-change user model 的 structural continuity、grouping/workspace/
   action placement、Material element presence和observable interaction、theme propagation、wide/narrow/200%/reduced motion；
   不测试Material shadow DOM私有细节。所有者于 2026-11-02 确认窄窗、键盘、主题与主要辅助功能符合预期；默认间距保持，不恢复 density，不建立第二套组件。
8. **H7 manual visual signoff（已批准，实施中）**：整理精简最终清单，核对历史失败的后续修复并集中稳定候选完整本地验证；只修实际功能、操作、重要布局与回归，不重验已签收页面。最终 Hosted CI/CodeQL 的推送授权另行确认。

H0-H7 只重组 shell、composition、component rendering 和 visual hierarchy；不得重新设计 RPC、Plan/Apply、Operation、Settings、
Activity、Diagnostics 或 Portable UI authority。

### H5 已批准切片（2026-11-02）

H4 阶段签收及实际环境恢复后直接推进 H5，不另设规划停止点。先核对 Settings/Config3、Activity/Diagnostics、
About/version 已有接线、反馈、分页与权限，只补真实缺口；正常状态简洁，错误/风险准确保留，短暂反馈不撑开布局。
只读路径统一 `displayPath`，不修改实际路径或 RPC/文件参数；公共合同确有缺口才提交最小方案并只暂停相关部分。
日常定向检查，稳定候选集中必要验证与真实界面入口；不重复全量 Rust/Chromium/release/三平台 CI，不以旧结果替代新代码。
已有 H2-B/H3/H4 非阻塞待办保留，不批量升为 H5 阻塞。不进入 H6/H7，不推送，不宣布 M7 完成。

复用审计已确认全部 Config3 字段、普通设置串行 CAS/未知结果核对、主题预览/三方合并、日志分段/搜索/已加载项筛选/
keyset 分页及关于/许可证入口已有实现。只修设置丢失安全错误原因、隐藏仓库 256 项界限、日志任务详情不刷新、
缺少 operations capability 时的操作接线、诊断原因与现有摘要翻译，以及 GUI/Core 版本区分和断线时本地信息可见性。
复用现有 OperationFollow 与 Reminder；GUI 构建版本取已由 metadata 检查对齐产品/Tauri 的 package 元数据。
不新增公共 RPC、State Schema、权限、依赖或通知/保存/任务机制。真实窗口只经已有自身进程启动方式准备，桌面控制保持停用。

移除紧凑模式前的 H5 候选 `c768289` 已完成 GUI check（1030 条消息）与一次 release no-bundle 构建；该轮 Core 为 `e2b7ddd`。
关联浏览器首轮 101/2、定位修复后的三个模块 56/0 分别保留，184 项输入逐轮未变，不拼成全套通过。
原 Core 持续运行，新 GUI 已实际打开；构建前后五组公开状态一致，期间更早的 Unity 记录变化单独保留。
交付 Settings、Log、About 三项真实窗口清单并等待 H5 人工确认；未执行完整 Rust/浏览器套件或远端验证，不进入 H6/H7、不推送。
证据与未覆盖项见 [H5 候选记录](../testing/m7-h5-utilities-20261102.md)。

#### H5 已批准差异：移除紧凑模式与 Config4（2026-11-02）

项目所有者已批准完整删除 GUI 紧凑模式及公开 `settings.get/update` 的 `appearance.density`，
当前设置合同升级为 Config Schema 4；不保留隐藏开关、旧字段别名或两套当前模型。Config4 文件、完整响应及
partial update 均不含 density，显式提供该字段作为未知字段拒绝；现有 settings.read/manage、串行 CAS、
16 KiB 文件上限、原子发布及恢复纪律保持。无需新 RPC、capability、权限、State Schema 或生产依赖。

Config1–3 仅作为既有启动迁移输入：按各自历史合同校验后丢弃旧 density，保留其余有效设置和原 revision；
v1/v2 仍仅补各自原先缺少的 packages/scheme 默认值。迁移在 ready 前完成，不算用户更新，不推进 revision；
Config4 当前写入与公开响应只使用新的唯一模型。当前规范及 Schema 原位转向 settings-v4，旧版本明确标为迁移历史。
验证只覆盖字段拒绝、旧格式迁移/其余设置及 revision 保留、GUI 紧凑入口与样式移除及相关回归；不重复无关全量或远端检查。

当前候选为 `6739705`：Store 19/0、application 17/0、协议合同 7/0、hello/映射/真实隔离 RPC 各 1/0，
GUI/UI check、保存队列 15 场景、浏览器 76/0 及补充 1/0 已通过；Clippy 初轮测试初始化失败与窄修复验分别保留。
一次 Core/GUI 必要构建后已实际恢复原环境，Config4 迁移只删除 density，revision 166 和其他偏好不变；新窗口保持打开。
以上为 `6739705` 交付时的历史状态；所有者随后已阶段性签收 H5 并批准 H6，当前授权见下节。
旧轮测试不冒充本轮全套，新增代码无远端验证；H7 与推送仍未授权。

## 技术收尾与 H2-B 顺序授权（2026-10-31）

### 当前 H6 授权（2026-11-02）

H5 已阶段性人工签收，紧凑模式移除与 Config4 方向获认可；H2–H4 原签收保持，不重验已检查流程。
直接推进既定 H6：先核对已有实现/测试，只修桌面宽度变化、较窄窗口、200% 可用性、键盘焦点与弹窗、
明暗主题和减少动画的实际缺口，复用共享 Material/ScrollArea/Reminder，不重做 GUI 或新增适配体系。
旧测试若只因可读错误/已删除密度/既有名称变化失配，应保留安全与交互断言并改为当前可观察合同，不能删断言掩盖产品缺陷。
定向诊断与修复后集中必要浏览器回归，准确区分等效缩放与真实平台/辅助技术结果；未覆盖项留待明确验收。
只读核对 Config4 现状复用迁移证据，不重跑 Rust/迁移/三平台 CI。H6 候选交付精简结果与人工入口，不进入 H7、不推送、不宣布 M7 完成。

本地安装的 Tauri 2.12.5 配置/类型确认缩放快捷键默认关闭。H6 复用其 `zoomHotkeysEnabled`，
仅为本地主窗口增加 Tauri 已有的 `core:webview:allow-set-webview-zoom` UI 权限以支持 macOS/Linux 的既有缩放适配；
Windows 使用 WebView2 原生缩放。不新增 ALCOMD RPC/权限或产品设置。自动化以 720×450 CSS + DPR2 核对
1440×900 物理窗口的 200% 等效布局，不把改根字号或浏览器结果冒充真实平台快捷键/读屏验收。

#### H6 本地候选与实际交付（2026-11-02）

代码候选 `a4f12a19d5fca0692053bb281fefab472fd60b65` 修正窄窗工具/表单/扩展卡片、Portable 正文滚动、
主题分段聚焦、Skip 锚点和 SplitButton 真实点击区域。宽屏保持原布局，日语主题保留 intrinsic 宽度，
沿用 860px 断点在窄窗单列；默认间距和 Config4 保持，不改 Core/RPC/State/扩展合同。
独立定向 27/11、8/4、13/1、1/0；一次集中 Chromium **383/12**（395 项，16.3m）。
旧断言、模板公开闭合时序与真实日语列宽分别修正后，失败及受影响路径 **53/1**，剩余新测试定位窄修后仅该项 **1/0**。
不拼成全套零失败；各轮 186 项输入及失败证据保留，最终输入与一次 GUI release no-bundle 构建一致。
GUI/UI check、metadata/diff 通过；不重跑 Rust 全量、Core release 或三平台 CI，新增代码仍无远端验证。

原 Core **143516** 持续使用原目录；仅关闭已记录旧 GUI，新 GUI **139052**（窗口 **8393506**，可响应）已打开。
构建前后项目、Editor、Operation、settings、status 五组完整公开状态全部一致，Config4/revision166 及文件摘要不变。
没有恢复桌面控制或伪造原生截图，原生缩放/实际读屏和跨平台辅助技术仍未覆盖。
以上为 H6 候选交付时的历史状态，所有者随后已阶段性签收 H6 并批准 H7；
各轮结果及 12 项对应关系见 [H6 验收记录](../testing/m7-h6-accessibility-20261102.md)。H2–H6 原签收保持，推送仍未授权，M7 进行中。

### 当前 H7 授权与收束（2026-11-02）

直接执行既有 H7，不另设规划审批。先核对现有实现与证据，仅修真实使用缺陷；普通文案、一般视觉和过度设计待办继续非阻塞。
原完整 Chromium 383/12 与后续 53/1、最后新增日语用例 1/0 独立保留，逐项对应不等于合并通过。
稳定候选集中执行适用的完整 Rust、前端、Chromium 与构建门禁，绑定实际执行 SHA 和输入摘要；聚合脚本重叠项不重复执行。
Windows 完整 IPC 测试必须使用隔离数据；沿用已批准流程，在原任务全终态、身份核对后暂停记录中的原 GUI/Core，终态后实际恢复并比较五组完整公开状态。
真实读屏器、跨平台辅助技术和客户端发行环境验证保持 M12 归属，Portable UI 未覆盖原生状态单独记录，不虚报或扩大至 M8/M9 业务。
最终人工清单原位整理在 `docs/testing/m7-official-gui-manual-checklist.md`，合并历史重复项，保留 H2–H6 签收引用；
桌面控制仍遵守此前停止边界，没有新增原生交互或截图时如实标记。形成准确本地候选后交所有者检查，再单独确认远端安排；不自动推送、不自行宣布 M7 完成。

项目所有者批准先处理当前跨平台测试/启动缺陷与依赖安全问题，再实施既有 H2-B，最后形成
真实桌面候选并停止人工验收；不重开 H2-A/Visual Gate 2，不重规划 M7 或重做 Projects/Packages。
开始时现场核对唯一工作区、实际 Git 状态与当前 HEAD 的 CI/CodeQL；`736f55a`、
CI `41191370985` 和 CodeQL `41191370892` 仅为审阅基线，不要求回退或重复修复。

- Hub 先区分 OS 同步 spawn 错误、OS 已接受和 child 随后退出，证明容量、owned child 回收与
  接收方取消/断线边界；按证据选择 fixture-only 或窄生产修复，保持 shell/参数、权限和进程边界。
- 安全评估基于实际 Cargo.lock、features、平台与调用链，逐项核对上游和 RustSec。先在既有
  依赖评估写明选定版本与理由，再升级当前兼容维护线内必要补丁及精确约束/锁文件；传递变化
  逐项解释。保持 Wasmtime Host-only、ABI/WIT、TLS 校验、ZIP 防御与 Plan/Apply 语义。
  跨不兼容版本、新直接生产依赖、feature 扩大、unsafe 或 native API 变更必须先停受影响项审批。
- 范围内 CI 后续失败可诊断并窄修；不得跳平台/测试、放宽断言、单纯延长超时、无诊断重跑或
  干扰用户进程来取得绿色。逐轮保留失败与修复证据。
- 技术收尾单独本地提交，通过适用本地门禁后明确允许普通 fast-forward 推送 `origin/main`，
  仅覆盖上述修复、获准安全补丁与对应证据。不得强推、改历史、改仓库设置或绕过工具审批；
  工具若仍要求具体 SHA 额外授权，报告 SHA 与范围后停止该推送。
- 同一最终技术 SHA 必须取得 Windows、Ubuntu、macOS 完整 required jobs 及 CodeQL 成功，
  且适用安全问题已修、其他公告有可检查结论。未满足时不宣称 PASS，也不提前改 H2-B 产品。
- 本轮已完成所选 Wasmtime/rustls/webpki/ZIP 维护及 ZIP 类型防御，完整 Rust 491/0/8、前端、
  冻结基线、CLI 并发启动与 release/no-bundle 通过。首个技术提交 Chromium 237/10 的每项均在
  文档或必需静态 JS 请求遇到 ERR_NO_BUFFER_SPACE、业务未到达；该门禁仍失败，不能拼接
  前轮或定向结果作为通过。项目所有者已明确批准仅 SHA
  `73f465b4cd0ab28b90794fcaac94e21db5be0e72` 在保留本机失败记录时普通快进推送，并以同 SHA
  三平台完整 required jobs 与 CodeQL 全绿作为本轮技术收尾通过条件。该 SHA 已核实位于
  `origin/main`，CodeQL 四语言已成功；Windows 完整成功、browser 247/0，macOS Hub 已通过但后续
  候选 RPC fixture 长路径失败，Ubuntu browser 245/2。后续修复沿用既有 macOS 短临时根、修正
  Select 内容宽度及表格测试的剩余空间前提，保留全部原断言；完整本地 Rust 491/0/8、布局专项
  3/3、根前端检查/构建与 release no-bundle 通过。后续候选完整 Chromium 为 246/2，分别为
  30 秒预算与标题等待失败，150 个输入未变，不能拼为 PASS。项目所有者另行明确批准后续精确
  SHA `02b6f0c3e81b8eedb711123ef9de2edc228792ab` 在保留失败记录时普通快进推送，该 SHA 已在
  `origin/main` 核实；CI `41209763063` 的 Windows/macOS 完整成功、browser 各 248/0，Ubuntu
  为 247/1，唯一失败是 257 目标的 5 秒标题检查。CodeQL `41209762434` 四语言成功；
  大列表候选仅按需创建行菜单、给内容一次绘制机会后初始化滚动容器。相关包管理/滚动条/导航
  63/0、前端 check/build、release no-bundle、xtask 与 metadata 通过；独立完整本地 Chromium
  247/2（18.4 分钟）、150 个输入未变。两项失败为必需 JS 加载与四宽度网格累计预算，未通过
  范围如实保留。所有者另行明确批准精确 SHA `1932140a815dd12712cd00108d70acebe51a3560`
  保留失败后普通快进推送，已在 `origin/main` 核实；CodeQL `41221092450` 四语言成功，
  CI `41221092743` 的 Ubuntu/macOS 完整成功、browser 各 249/0；Windows browser 248/1，
  唯一失败是正常动画分支的一次性滚动条样式读取。后续测试候选在默认断言时限内等待实际样式，
  增加 property 校验并保留原阈值；动画专项 9/0、前端与 metadata 通过，独立完整本地 Chromium
  248/1（12.6 分钟），150 个输入未变。唯一失败为必需 JS 的 ERR_ADDRESS_IN_USE，Hub 业务
  未到达；未重跑、改 Hub 或扩大预算。所有者已明确批准精确 SHA
  `97c761f76613ea68ebc7a93e4695940dabb969dd` 保留本机失败后普通快进推送，已核实位于
  `origin/main`；CodeQL `41228566315` 四语言成功，CI `41228566567` 三平台完整 required jobs
  全部成功、browser 各 249/0。同一最终技术 SHA 的集中收尾检查点已通过，H2-B 按既有授权
  开始实施；前述本机失败记录保持。详细证据见
  `../testing/m7-technical-closure-20261031.md`。
- 满足前提后 H2-B 自动获准开始，无需再询问。H2-B 允许本地候选提交，禁止自动推送；
  当前 release GUI、匹配 daemon 和隔离样例必须走实际写入闭环，最后交截图/清单等待人工验收。
- 项目所有者于 2026-11-01 明确要求将本轮 Chromium 门禁修复与相关文档用真实 Windows
  用户环境提交并推送；允许将包含既有 H2-B 基线的当前候选普通快进更新到 `origin/main`。
  该显式授权只覆盖本轮候选，H2-B 与 M7 人工验收仍待定。

## H2-B 资源与模板范围及验收方案（条件批准）

状态：**阶段性人工验收通过，保留体验改进项；本轮体验整改结束**，2026-11-01。
实施前提与推送边界以上节为准；H2-A 检查点为
`23f545245f613f47f8def1e16418c4d12303404e`，Visual Gate 2 已签收。
本节直接细化原 H2 的剩余范围，不新建产品里程碑，不重复实现已存在的入口。
当前阶段仍以 `../status.md` 为唯一入口；技术收尾已在 `97c761f` 完成同 SHA 三平台 CI/CodeQL 检查点。
H2-B 前次运行时代码候选为 `cbb4c4198a21519efb24d8f0a04a509664e83b50`，完整 Rust 492/0/8、
前端/冻结基线/release 构建通过。原两轮完整 Chromium 290/2、283/9 与仅上游缓解的 291/1
独立保留；fixture 连接关闭修正经有界对照后，工作区独立完整 Chromium 为 292/0，11.2 分钟、
652 项输入前后相同，不拼接不同轮次。专项验证基于 `173c956`，测试基础设施修复与相关
文档已提交为 `0ef5b088dfdb7a8eb2b7a88ebef94ae436c90f50` 并普通快进推送 `origin/main`。
隔离 native 已核对 stale 零接受副作用、资源/模板写入、两入口创建、持久化、
Retry 与原环境恢复。builtin confirmed CLI remove 先前自动审批拒绝的历史保持；项目所有者随后
明确授权在既有可丢弃隔离环境补齐：正式 CLI 带 --yes 的唯一请求返回 template_builtin_immutable，
exit 1，七组公开状态与 63 个文件的路径/大小/hash 完全一致，无新增 Operation。
收尾仅补新建项目结果链接与既有键盘合同：真实窗口 `/projects/d138da13-ad72-4de8-b438-d99b70c43f66`
与 Operation 返回 ProjectId、公开查询一致；Tab/Shift+Tab、Enter/Space 符合分组按钮合同，
方向键未改变焦点或路由，无须增加新的交互模式。前次收尾已读回五组原样例状态一致并恢复开发窗口。
完整矩阵与截图身份见 `../testing/m7-h2-b-20261101.md`。同一远端 SHA `0ef5b08` 的
CI `41287607183` attempt 1 三平台完整成功、Chromium 各 292/0；CodeQL `41287607108`
attempt 1 四语言成功。只收集已有运行；运行时、测试与构建输入未改，不重复完整本地门禁。
后续实际候选为 `2a163b0`（生产输入 `ceef00f`），已沿用此前成功方式启动匹配 release GUI/Core。
项目所有者本次确认 Resources 仓库/列表交换、用户包、模板和键盘/弹窗体验的阶段性人工验收
通过，允许结束本轮整改；文案不准确、冗余说明、信息位置和过度设计保留为现有 status 待办。
不宣称体验完善、所有问题解决或整个 Visual Gate 3/M7 完成，H2-A 原签收保持。
隔离 11 个任务已完整分页确认终态后，按授权关闭本轮窗口并实际恢复原开发 Core/GUI；五组
公开结果与暂停前一致、settings revision 155。新代码的远端验证仍未取得，不借旧 SHA 替代。
本次仅允许本地文档提交，不新增推送授权；不将此前截图/工具缺口追写为已执行。

### 本轮人工反馈整改与仓库列表窄合同

项目所有者实际查看后确认：流程可用，但 Resources 提示挤动布局、提醒位置混乱，体验未通过；
仓库列表导入/导出是冻结 v3 用户能力的实际缺口，本轮补齐，不另立 M7 规划或进入后续阶段。
先在真实 release 窗口保存完整正常/成功/失败状态截图，再修共享提醒框与调用位置。
短暂成功使用不占布局的轻提示；字段错误靠近字段，操作错误留在弹窗/对应上下文，风险与部分
失败持久显示；进度仍由既有机制负责。关闭、取消、重试清理旧状态，长内容可换行/展开/滚动。
共享组件影响其他页面时补关联定向回归，不重做已签收的 H2-A。

实施前复用审计：冻结 v3 `4aa98ae` 的仓库列表规范为 UTF-8 文本、每行 URL、`#` 注释、空白忽略，
支持 http(s) 与 `vcc://vpm/addRepo?url=...&headers[]=...`。当时 Core/client 已有 inspect、分页
list、register、refresh、unregister，尚无列表文件读写合同。GUI 已依赖 dialog 插件，已有目录
选择与 Hub 文件打开，可复用为薄打开/保存适配器，不在 Tauri 或 GUI 中实现仓库登记权威。

公共差异（**项目所有者已批准以下修订合同，已在本地实现并取得本轮阶段性人工签收**）：

| RPC | 输入 / 输出 | 权限及边界 |
| --- | --- | --- |
| repositories.inspectListFile | `{path}` → 有界条目 `{line, source?, status, reason?}` 与忽略行数 | local-owner + 既有 repositories.read；只读 UTF-8，最多 1 MiB / 4096 条，超限整体拒绝，不写 registry，不自动下载 |
| repositories.previewListExport | `{}` → 全量登记快照的可导出数量、排除项及 fingerprint | 既有 repositories.read；Core 从一致只读快照取得全量，不受 GUI 当前页限制；返回排除原因，不返回认证秘密 |
| repositories.exportListFile | `{targetPath, expectedFingerprint}` → 实际导出数量及摘要 | 认证 local-owner + 既有 repositories.read；重验预览内容与范围的全量快照，变化则在创建文件前拒绝；仅创建全新文件，拒绝 Core 数据/已登记项目内目标，取消选择不调用 |

新增 capability `repositories.list-files.v1`，沿用既有权限；补 RPC Schema/SDK/兼容快照，
不改 State Schema，不引入新生产依赖、批量事务或通用工作流引擎。错误用有界稳定 code，
不将原始无效行、headers 或 URL 凭据写入日志/普通错误。

最小用户行为与文件边界：

- 工具栏 Add repository 的 split menu 提供添加、导入列表、导出列表，不新增导航。
- 复用 v3 普通 UTF-8 URL 列表，支持 CRLF/LF、可选 UTF-8 BOM、注释及不带 headers 的 vcc 添加链接；
  不是 repository manifest、package 导入或旧数据库迁移。带 headers/userinfo 的条目明确列为不支持，
  不降级为匿名条目，不显示其秘密；其他 scheme、无效行都给行号和原因，不静默忽略。
- 导入先预览并选择，再调用既有 repositories.inspect 检查可访问性和现有来源规则；重复 URL/ID、
  无效或不可访问项留在结果中。显式确认后按所选项逐条调用既有 register，默认追加，不清空或替换。
  不是原子事务；取消停止调度新条目，在途请求的实际完成仍计入，分别展示成功/失败/未处理/未知。
- 导出预览和实际文件覆盖 Core 全量登记快照，不能只导出 GUI 已加载的一页。保留 Core 已验证的
  完整 http(s) URL，包括 query 的字节顺序与内容，不删除、重排或因存在 query 而排除。
  预览明确提示文件包含完整地址，地址自身可能携带凭据、分享前需检查，显式确认后才导出。
  不读取凭据库、不附带 headers/密码/token，不承诺 URL 已被认证为无秘密，不建立按参数名猜测
  所有秘密的系统。当前无法表达的认证来源和本地来源明确排除，逐项说明，不称作完整备份。
- 当前 Core 没有内置仓库记录类型或预置仓库注入；导出当前实际登记的远程来源，不按 URL 猜测
  builtin，也不凭空加入 v3 官方/精选默认仓库。本地来源不伪装为 URL。
- 原生打开/保存只选择路径，Core 负责格式、全量查询和文件写入。保存只创建新文件，已有文件
  明确报冲突并允许另选路径，保存弹窗不承诺覆盖。失败不留下可被当作成功的半成品，且不能清理
  非本次创建的文件。普通成功轻提示，排除项/部分失败持续保留供用户处理。

实现为 application 的 RepositoryListApplication、Store 的一致只读投影与独立 VPM 文件适配器；
通过现有 RPC/client/SDK 接入 GUI。当前 dialog 插件不能关闭系统保存框的覆盖确认，保存界面因此
使用新文件名字段和已有原生目录选择器，Tauri 仅组合用户选定路径；打开复用原生文件选择器。
保存不显示系统的替换文件承诺，最终目标、冲突及路径保护仍由 Core 判断。列表和错误正文滚动，
操作按钮使用 Material Dialog 公开 actions slot。未增加生产依赖或持久任务。

该修订合同已明确获准，先同步本节及 RPC Schema，再实施，不再追加纯文档审批。
独立共享提醒框组件同样已批准；各信息性质使用合适承载，避免一律浮动或重复反馈。

### 目标与现状证据

建议 H2-B 收口 Resources 内 Repositories、User Packages、Templates 三个既有分区，以及它们
连接到 Projects 的模板创建流程。保持现有导航、1400×800 桌面基线、已签收密度与共享 Material
组件，不重做 Projects/Packages，也不以资源子系统划分新的一级工作区。

| 现状 | 已核对证据 | H2-B 处理 |
| --- | --- | --- |
| 仓库 inspect/register/refresh/unregister、包预览与分页已接线 | `CorePages.tsx` 的 RepositoriesPage/RepositoryPackageList；`CoreActions.tsx` 的 RegisterRepositoryPanel/RepositoryActions | 复用现有功能，补齐异常、并发 revision 与真实桌面闭环证据 |
| 用户包登记、刷新、取消登记及行内详情已接线 | `CorePages.tsx` 的 UserPackageEnroll/UserPackageManager/UserPackageDetails；`rpc.ts` 的既有 userPackageGet | 详情读取当前 Core 记录，关闭隔离旧响应；资源与相关包来源 18/0；当前 release 的真实 chooser、详情、显式刷新、取消/确认移除与重启持久化已记录 |
| 模板导入、派生、收藏、导出、移除、创建项目都已有入口 | `CoreActions.tsx` 的 TemplateImportPanel/TemplateDeriveDialog/TemplateActions；`ProjectCreationDialogs.tsx` | 修正当前流程缺口，不把这些功能重新列为“从零实现” |
| 模板列表与创建选择器已接通后页 | `rpc.ts` 保持 limit=100 并透传 cursor；TemplatesPage 复用 PagedItems，CreateProjectDialog 保留后页失败时的数据/选择并隔离关闭后的响应 | 101 项、创建导航、失败重试、旧响应隔离和延迟 Retry 的模块回归 14/0；证据见 `../testing/m7-h2-b-20261101.md`，不替代最终候选或 native 验收 |
| 模板任务由当前页面持有并在成功终态刷新 | 可控延迟已复现关闭失去跟踪/初读错误停止等缺口；复用 OperationFollow，行 controller 位于 PagedItems 外 | 保留多个显式接受 ID；成功才刷新，旧任务不覆盖新草稿；完整整轮与定向结果分别见 testing 记录 |
| 三类模板 review 已使用现有白名单 evidence | `TemplatePlanReview.tsx` 校验顶层字段并绑定读取/请求上下文；两个创建入口共享规则 | 缺失、错误类型、身份/digest 不匹配不开放 Apply；不显示 filesystem identity，不扩充公共字段 |
| 浏览器已补真实 wire、分页、任务交错及资源生命周期 | 新模板/资源/shared-follow specs 和原 harness；规范 UUID、顶层 Plan、受控 Running/terminal 和有状态 CAS fixture | 保留全部既有断言；模块 14/0、资源 18/0、普通模板 6/0 分别记录；34 项扩展组的 30/4 不合并为完整通过 |
| H2-A 真实桌面材料没有这些资源写入闭环 | `../testing/m7-visual-gate-2-20261031.md` 的实际范围 | 新建 H2-B 独立证据，不用 H2-A 的 GUI Apply 或 CLI backend 测试冒充 |

上述 GUI 缺口以浏览器受控场景复现和验证，尚不作为 native 故障或真实写入通过证据。
v3 行为仅使用冻结审计和布局对照，不复制源码。

### 建议实施范围与顺序

1. **模板读取与页面状态**。只为 existing `templates.list` 接通 cursor，使用既有 PagedItems
   或等价的现有分页交互，覆盖 Templates 列表和 Projects 的模板选择器；保留 Core 顺序与
   opaque cursor，不解析游标、不提高 limit 规避分页，也不静默把未加载项当作不存在。
   失败保留已取得的数据与选择，明确提供再次读取；切页/关闭后不接受旧请求结果。
   TemplateDeriveDialog 已有项目分页，验证它能选择后页项目即可，不另建项目列表。
2. **模板确认、任务与结果**。复用当前 import/derive/create Plan/Apply、OperationFollow 和
   shared Dialog。review 至少展示 action、TemplateId；import/override 显示 expectedRevision
   与是否替换；derive 显示源 ProjectId/revision；create 显示 templateRevision、targetLeaf
   与 targetMustBeAbsent。都从该 Plan 现有 evidence 读取，不显示 filesystem identity 的原始字节。
   名称/版本只可使用与 Plan digest/identity 对齐的读取快照作为上下文，不能伪造为 Plan 返回字段。
   TS 对已知 evidence 做窄类型校验，不增加任意 JSON viewer；关键证据缺失时给有界错误并阻止 Apply。
   Apply 后跟随同一个 OperationId，成功终态再刷新实际列表/项目；失败、取消、断线不能报成功。
   stale/conflict 明确显示，不自动重新 Plan 或重发 Apply。两个创建入口保持同一 Core 用例与结果身份，
   用户可定位 Core 注册的新项目，不建立第二套项目创建/Operation engine。
3. **资源动作闭环**。仓库继续使用既有 URL/local manifest 输入、inspect→确认→register、
   refresh、unregister 与包预览分页；用户包继续使用既有目录选择器、enroll、refresh 和确认后 remove。
   用户包详情在同一 Resources 工作区的行内弹窗内读取 get，显示名称、packageId/version 和来源目录，
   revision/hash 只作次要详情；不暴露 object-store/cache 路径或改成新一级导航。
   取消选择/确认不写入；并发变化要求显式重读，删除仅取消登记，不删除源文件或已安装包。
4. **验收与材料**。完成下面的自动化及真实桌面矩阵，沿用共享 loading/last-known-good/error/
   reconnect/capability、focus/menu/select 规则。只有因范围内失败而需要的共享组件窄修正可进入本 slice，
   并回归已签收的 Project/Package 路径；不借此重构全部页面。

资源 URI、RPC capability、权限和存储版本保持。相关 method 分组为：仓库既有七项
inspect/list/get/packages/register/refresh/unregister，用户包既有五项 list/get/enroll/refresh/remove，
模板既有十二项 list/get/inspectBundle/planImport/applyImport/planDerive/applyDerive/export/
setFavorite/remove/planCreateProject/applyCreateProject。Operation 查询/取消仍用原合同；
低影响 command 不创造 GUI Plan 或新 Operation。

GUI production 改动为 `apps/alcomd-gui/src/CorePages.tsx`、`CoreActions.tsx`、
`ProjectCreationDialogs.tsx`、`TemplatePlanReview.tsx`、`rpc.ts`、必要的本地 `core-models.ts` 与三语言词条；
tests/browser 的现有 harness/spec 同步。`UtilityWorkspace.tsx`、`Material.tsx` 等共享层只允许
为上述可复现问题作必要修正。已有 Tauri templates adapter 已接收完整参数，无需新增平台 command。
本轮同时收口真实桌面已复现的模板 Apply 接受边界：`crates/alcomd-store/src/m5_template.rs`
在原有接受事务内重验 import/derive/create-project 的登记 revision、digest 和对象存在性，
先处理已接受请求的幂等重放，再拒绝已知失效且尚未接受的 Plan；最终提交继续复用同一重验。
`crates/alcomd-testing/tests/m5_template.rs` 证明拒绝不产生 Operation/Event/目标目录，不消耗
幂等键，显式新 Plan 仍可创建，已完成任务在模板 revision 改变后仍按原 ID 重放。
这项修复不改变冻结字段、错误码或恢复格式，不删除旧失败任务、目标目录或恢复证据。
本方案不提出新依赖、public DTO/wire、RPC、权限、Schema、migration 或 native API。

### 自动化验收矩阵

| 场景 | 必须证明的结果 |
| --- | --- |
| 模板分页与完整选择 | 至少 101 项、两页不同数据；后页模板可见且可用于创建。请求沿用真实 cursor/limit；翻页不创建 Plan。后页失败保留前页及选择，重读不重复项；空列表与读取失败分开 |
| 模板派生项目分页 | 从源项目后页选择，Plan 使用该 ProjectId 与重读的 revision；关闭/重新打开不继承前次草稿或迟到响应 |
| 三类 Plan review | import/override、derive、create 各使用 Core 实际投影的 evidence 形状；展示与返回对象一致。非法/缺失关键证据不开放 Apply，不读未经批准的字段；不加大公共合同 |
| 模板任务终态 | 可控 Running→Succeeded 后才核对新增/替换对象与版本；失败/取消不显示成功；同一 OperationId、一次显式 Apply。接受阶段的早读不能覆盖终态重读 |
| stale 与未知结果 | Plan 后推进模板或项目 revision；原 Apply 明确拒绝，前后无额外 Operation/文件变化。断线重连跟随原任务，未知提交结果不自动重试 |
| 模板普通动作与边界 | favorite 双向持久化；真实 export 返回成功并有外部 artifact；remove 用户模板及 builtin remove/override 拒绝。导入相同 digest、不同 digest 和明确 override 分别按已有合同处理 |
| 仓库生命周期 | inspect 不登记，确认后 register；重复/无效源有稳定错误。refresh 失败保留 last-known-good，成功后展示当前包；预览后页可达。取消 unregister 无写入，确认只取消 registry |
| 用户包生命周期 | 选择取消不 enroll；有效目录登记、get 当前详情、源变更后显式 refresh；revision conflict 不覆写；remove 只取消登记，源文件不变。缺 capability 不调用业务 RPC |
| 页面与交互 | 各分区 loading/empty/refreshing/error/disconnected；Retry 恢复数据和能力。键盘分段、弹窗 Tab/Shift+Tab/Escape/返回原控件、三语言/明暗及既有 reduced-motion 行为保持 |

现有 browser helper 不得用提前 succeeded、固定一项返回或 synthetic 错误码跳过真实竞态。
同一测试场景的请求序列和数据结果均须可检查；使用公开 Material 生命周期，不访问私有 shadow DOM。
测试先补足能证明缺口的失败用例再修实现，不放宽 30 秒、单 worker、零重试或原断言。

开发中按本文件“验证与 release blockers”的风险分级安排定向及模块回归；相关修改稳定后，
在实际本地候选 SHA 上集中完成主 ExecPlan 的完整本地门禁：fmt、Clippy、Workspace tests、
xtask、metadata、冻结基线、npm check/build、完整 Chromium、Tauri release no-bundle 与 diff check。
该清单不按单个修改或提交重复触发，H2-B 也不自动触发 Hosted CI/CodeQL 或推送。
日志逐轮保存；旧 SHA 的通过、定向复核与完整运行不得拼接，未执行项保持未执行。
仅修改本方案或验证安排时运行文档检查，不重复执行或宣称取得 H2-B 的 Rust/browser/native 验收结果。

### 真实桌面验收方案与停止点

真实验收使用嵌入 production 前端的 release GUI、独立临时 Core 数据目录及 WebView2 profile，
在正式应用用例层内操作。准备只含 JSON、文本和最小项目结构的合成 source、两个有效的同 ID
不同 digest 用户模板 bundle、一个 local 和一个仅监听 loopback 的 anonymous remote 仓库。
它们必须与用户样例、正式仓库、Unity/Hub 安装隔离；模板只用 native `.alcomdtemplate`。
外部输入可在登记前构造，登记后只经正式 CLI/GUI/RPC 修改；不直接写数据库、cache 或已注册项目。

真实 GUI 至少走完：

1. 仓库 preview/确认添加→包预览→refresh 成功与失败的 last-known-good→取消/确认移除；
2. 用户包 chooser 取消与选中→详情→外部合成 source 改动后 refresh→取消/确认 remove，
   用只读 hash 核对 source 未被删除或改写；
3. 用户模板 import→Running/terminal→列表出现，favorite→刷新与重启仍保持；
4. 用户模板 export 到全新路径→Core inspect 验证，直接 export 的 bundle bytes/digest 与已验证对象一致；
5. 同 ID 不同 digest 的导入先拒绝未确认覆盖，再显式 override；builtin 的覆盖/移除仍拒绝；
6. 从合成项目 derive→成功后列表更新，从 Projects 及模板行分别创建两个新项目，
   跟随各自 Operation 并核对 Core 注册的 ProjectId、目标文件与排除项；不启动 Unity；
7. GUI review 后由另一正式客户端推进临时 revision，原 Apply 拒绝；保留前后 Project/Template、
   Operation 列表及文件摘要，证明没有额外副作用；
8. 代表性的深浅色、键盘/focus、断线/Retry 与重启持久化。native 未覆盖的场景逐项记未执行，
   browser 多页/错误注入不得冒充 native 动作。

共享 Windows endpoint 必须先核对归属与可用性，不能误连原样例 Core；占用/初始化失败即停止。
需要暂停既有样例进程时按已有授权核实身份及无活动 Operation，并记录、恢复完整原状态；
不停止用户 Unity/Hub。临时运行时最后关闭，原 Projects/Editors/Operations/Settings 与 revision
前后比对；截图仓库外保存，逐张记实际 GUI hash/构建，技术日志留在 ignored target。

完成后停止在 **H2-B 候选与真实桌面签收**，交付修改摘要、逐项矩阵、日志/输入 hash、
截图和未覆盖项，等待项目所有者验收。这些是 Visual Gate 3 remaining core pages 的资源/模板证据，
不自动签收整个 Visual Gate 3，不重开已通过的 Visual Gate 2，也不自动开始 H3-H7 或 Hosted CI/push。

### 排除项与必须停止审批的条件

- 仓库列表 import/export 已由本轮人工反馈明确纳入 H2-B，新增窄公共合同按上节审批。
  仓库凭据 enrollment/revocation、header/proxy、deep link handler、reorder/cleanup/cache
  等其他缺口保持原边界，不静默加入，也不标记为已完成。
- v3 `.alcomtemplate`/tar.gz、v3 模板编辑/依赖与 UnityPackage parity、真实脱敏 differential fixture
  保持 M11 归属；不在正常 runtime 加回旧解析器。H2-B 只验收现有 native Template Bundle v1。
- 不新增 template 文件打开/保存选择器。现有导入/导出路径输入仍可完成流程；新增 native picker、
  文件关联或系统 opener 是独立方案与审批点，不因已依赖 dialog plugin 就自动获批。
- 不新增 source renderer、workflow engine、通用数据表框架；不扩大窄窗口/窗口记忆、H3 Unity/Backups/
  Operations 增强、H4 Portable UI、H5 utilities、M8/M9、安装更新或三平台辅助技术范围。
- 若现有 evidence/Operation 结果不足以安全完成上述确认或定位，或实现需要 public DTO/Schema、RPC、
  capability/permission、持久格式、依赖或 native API 变更，先停止对应步骤并提交具体补充方案；
  不能根据表单内容伪造 Core 证据、直接读项目文件、加别名/双格式或放宽权限来绕过。
- 聚合 feature 仍保持实际 in_progress；M11/M12 未完成项与 Hub 下载/native Portable UI 的旧未覆盖
  范围不被本次提案或资源验收提升为通过。提交、推送、Hosted CI 和发布仍按各自授权执行。

### H3 已批准切片：已有能力复核与直接接线

项目所有者在 H2-B 阶段签收后明确批准现有 H3 直接实施；上节 H2-B 的范围排除不再阻止本切片。
交接已完成：原开发 Core/GUI 已实际恢复，Projects/Editors/Operations/Settings/Status 五组完整公开结果一致；
仓库列表三个 RPC 已补入既有 GUI 矩阵，公开方法 92/92 对齐。无需另设纯规划或交接人工停止点。

| 已有流程 | 本轮只补的缺口 |
| --- | --- |
| Settings 已有 Unity Hub、安装列表、登记/发现/移除；项目已有精确启动、启动参数、Unity 迁移 Plan/Apply | 接入共享成功轻提示、上下文错误与请求生命周期；本页接受启动后从现有更多菜单按需读取/刷新该启动记录，校验 ID/项目/安装身份，清理迟到结果；不重做启动或 Hub |
| 项目已有备份创建、备份详情与恢复入口 | 创建接受后将同一 Operation 留在项目备份上下文，终态刷新历史；接通现有备份 cursor；详情复用 Projects 恢复弹窗与所选备份，移除重复恢复面板 |
| 已有 OperationFollow、任务中心、任务详情、协作取消 | 任务列表接通现有 cursor；详情复用同一跟踪组件持续读取、明确失败/取消状态及 Core 结果入口；取消错误留在确认上下文 |
| Projects 已有托管备份恢复 Plan/Apply | 复用已有 OperationFollow，已知 ID 读取失败持续跟踪，不能再次 Apply；未知写入结果保留未知、禁止重复提交；补全备份选择分页 |

不新增公共 RPC、Schema、权限、依赖、持久任务或通用工作流；GUI 不读写数据库或项目文件。
先运行受影响 GUI 检查与定向回归，稳定候选再集中验证和真实隔离桌面查看。主体 `ef47c25` 已取得
30/0 与真实隔离备份/恢复/反馈证据；启动状态补齐后的最终代码 `ab5d183` 已取得独立 **34/0**、
GUI check（918 条翻译）与更新后的 release GUI。备份/任务等 16 个相关函数不变的比对证据保留，
原生截图仍绑定实际 `ef47c25` 构建；Esc 后新增启动状态弹窗未做原生操作/截图。原环境已实际恢复，
五组完整公开状态一致、revision 155。详见 [H3 候选与桌面证据](../testing/m7-h3-contextual-20261101.md)。H2-B 新增代码及 H3
仍没有对应远端 CI/CodeQL，旧 `0ef5b08` 的通过只对应旧输入。H3 的停止点是本地候选、证据、真实窗口
和简短清单交由项目所有者验收；当时 H4-H7、推送与 M7 整体完成均未授权。当前 H4 的后续批准见下节，H5-H7 与推送仍未授权。

2026-11-02 所有者暂不签收 H3，要求优先修复当前 Windows Editor 检测。现场原开发数据的登记为空，
但默认 Hub 安装根存在 `2022.3.22f1`；真实 Package Manager 清单不含 Editor 顶层版本，旧验证却依赖人造
`{"version":...}` 形状并静默丢弃失败。此轮只修现有验证/发现链与直接误报：Windows 从 Editor 本身的
ProductName/ProductVersion 资源读取，复用现有 `windows-sys` FileSystem 功能，不执行 Editor 获取版本，
不以目录名推断、不新增依赖或公开合同。窄 version.dll FFI 只在 platform 私有模块局部允许 unsafe，
每处说明指针、缓冲区、对齐与生命周期，输出仍经 Core canonical version 校验。发现失败明确返回既有错误，
技术日志保留完整路径和原因，GUI 不把版本无法验证说成未安装；先补真实元数据形状的失败回归。
Hub 自定义根尚未覆盖的能力不作为本机故障原因，也不借此扩大实现。既有 exact launch、Plan/Apply、
无持久 Editor 偏好与不干扰用户 Unity 的边界不变，其他非阻塞体验只保留现有待办。

该修复已本地提交为 `efdb133`：platform 8/0、daemon 3/0、exact version 1/0、RPC 1/0、Unity 浏览器
6/0 与受影响 Clippy、GUI check、Core/GUI release 构建通过。真实原环境的正式 CLI 自动发现及再次刷新
均得到同一 verified `2022.3.22f1`，两个样例的 exact candidates 匹配；没有手动登记、启动或迁移实际工程。
原数据 Core 已恢复，更新前后五组公开状态一致；发现后 Editor 登记按预期从 0 到 1，其他四组保持、revision 155。
physical Escape 后未再进行桌面输入/原生截图，新 GUI 已启动可响应，真实安装页显示仍待所有者确认。
本轮停止点只核对这一项，不重新验收所有 H3 页面，不以 CLI 成功替代原生 GUI 验证或 H3 签收。

所有者随后确认 Unity 已正常检测/显示、刷新保持同一条，检测功能项通过；H3 整体仍未签收。
新反馈是 Windows `\\?\` 内部路径前缀直接显示。项目列表已有私有格式化函数，其余只读路径未复用。
本轮把它收束为 GUI 的单一 `displayPath`，接入安装列表/完整路径 tooltip、启动选择与已有项目/来源/目标审阅。
仅格式化已知盘符和 UNC 展示，保留 Volume GUID/设备等特殊命名空间；RPC、编辑状态、文件操作与 Core 身份
继续用原值，URL 不按文件路径处理。不修改 Core、Schema、权限、导航或页面布局，只补路径和参数分离的定向回归。

路径整改已本地提交为 `fec8117`：独立定向浏览器 10/0（1 worker、0 retries）、GUI check 及 GUI release 通过，
历史 8/2、9/1 保留。真实窗口已保存前后完整截图，刷新后仍为同一条 `2022.3.22f1`，显示普通盘符路径；
前后完整安装 RPC 结果一致，Core 原进程/数据未变，没有操作用户 Unity。GUI 新产物为 `cc4f…9808a`，
Core 继续使用 `efdb133` 的 `ef371…1b5ca`，不重复 Rust/浏览器全套/三平台 CI。
所有者随后明确确认“路径显示正常了”，并进一步给出 H3 阶段性人工签收，保留现有非阻塞体验问题；不重复验收。

### H4 已批准切片：复用审计与实施边界

所有者在上述 H3 签收后明确批准 H4 直接实施，不另设纯规划停止点。交接时原开发 Core/验收 GUI 在原数据环境，
五组公开状态已完整核对，任务全终态，无隔离占用需要恢复。现有扩展卡片、安装/卸载 Plan、启停、权限变更与
17-node Portable UI renderer 均已有接线，继续保留；只补管理流程、分页、反馈/进度和异步 session 生命周期。
安装输入的手填 registry revision 说明与 Core 不符：现有安装只接受新 ExtensionId，`expectedRevision=0` 表示
预期缺席，已有安装由 Core 拒绝；按该既有语义修正 GUI，不新增 registry 读接口或绕过版本校验。
权限/作用范围与安装审阅如发现公开只读信息不足，提交具体最小合同审批，仅暂停该部分；其余流程继续。
Portable UI 的文档、草稿、action 与 session authority 保持原合同；不增加页面发现、额外窗口或第二套任务机制。
稳定候选集中做受影响浏览器/GUI 检查与必要构建，真实窗口交付实际范围、未覆盖项与简短清单；H4 人工签收由所有者确认。

#### H4 权限只读缺口：已批准的最小公共合同

现有 `extensions.get` 只有 grantRevision，不返回权限声明或已授权 scope；公开接口只有 set/revoke，GUI 无法
如实展示“已授予哪些项目”。安装 Plan 内已有冻结的 required/optional permissions，但 RPC 投影未返回，无法审阅。
已按人工批准增加以下只读信息，复用既有 Core 表与权限权威，不改 State Schema、Portable UI 协议或运行时：

- `ExtensionPlan` 兼容增加 `requiredPermissions` / `optionalPermissions` 数组，直接投影冻结 Plan evidence；每组沿用
  Manifest 的最多 64 项、每项 128 字符约束，不以 GUI 表单/猜测代替。新 GUI 缺少这些证据时明确提示审阅不可用。
- 新增 `extensions.permissions.list({extensionId,cursor?,limit?})` 与 capability `extensions.permissions.read.v1`，
  需要现有 `extensions.read`、当前 Principal 对该 ExtensionId 的所有权；不新增 permission 或扩大 grant 能力。
  返回 `{extensionId,grantRevision,requiredPermissions,optionalPermissions,grants,nextCursor?}`；grants 只列当前有效的
  `{permission,resourceKind,resourceId}`，不返回历史撤销、凭据、Host 身份或数据。默认 100、最大 1000 项，按三字段
  keyset 分页；游标绑定 ExtensionId、grantRevision 及已安装包快照，读取同一事务内的声明和有效 grants，授权或包变化用既有
  `revision_conflict` 拒绝后续页，要求重读，不能拼接不同 revision 的授权集合。
- GUI 在既有详情/权限弹窗显示声明、实际权限和具体项目范围；set/revoke 继续使用返回的 grantRevision 显式提交，
  不自动授权、不伪造空集合、不提供通配范围。本提案不增加 marketplace、升级、外部服务权限或 M8/M9 产品功能。

所有者已明确批准直接实施本合同，先同步 Schema/测试，不再停一轮纯文档审批。声明来自已冻结 Plan evidence，
不在审阅时从可变来源重读；实际 grants 只来自 Core 当前权威记录。GUI 用清楚名称和实际范围区分“申请什么”与
“已允许什么”，不堆原始 JSON/revision/权限字符串，不增加持久查询会话或缓存；失效读取不能覆盖新状态。
定向覆盖无/部分授权、项目 scope、未授权访问、分页中撤销或包变化、Plan 声明不漂移，以及 GUI 读取失败/刷新/变更后展示。
新增代码没有 Hosted CI/CodeQL 结果，不复用旧 SHA 作为本轮通过；不推送、不进入 H5-H7。

#### H4 本地候选与实际证据

Core/公开合同提交 `0f8c9f4`、GUI 提交 `88c3f69` 已完成范围内接线，最终代码候选为 `e2b7ddd`。安装只审阅冻结权限申请，权限管理读取
当前 grants 并展示具体项目，撤销/包变化使分页快照失效；写入仍用既有接口及调用时校验。列表分页保留已加载项，
安装/卸载跟踪同一 Operation；隔离恢复显式执行，不误报成功。共享 Material Switch 修复受控状态恢复时原生勾选不同步，
关联焦点和 H3 路径回归通过；Portable UI 保留原协议，按 client/session 隔离异步结果与草稿，不重放未知写入。

GUI 独立浏览器 36/0，155 项输入不变；相关 dispatcher/协议/契约、七 crate Clippy、SDK/GUI check、
xtask、初次 Core release 与一次 Tauri release no-bundle 通过。旧失败与修复定向运行独立保留。
真实隔离 Core/Host 验证了签名安装、授权/撤销、UI 同连接操作、崩溃隔离及 retain-data 卸载，样例四文件未变。
初次跨连接 UI 操作的 session_not_found、证据 helper 误写旧授权方法名的 method_not_found 均保留，修正测试入口后
单独验证；生产 GUI/client 原有方法名没有该错误。故意 trap 的恢复仍为 extension_crashed，保持隔离，未宣称健康。

真实 Core 反例暴露 required 未授权仍成功写 enabled；另一定向反例证明 setGrant 能借用历史包已不再有效的声明。
两者均属功能正确性缺陷，`e2b7ddd` 在原 Store 事务复用当前安装包的冻结证据，required 缺失在写入前拒绝，
授权也只认当前声明，不改 Runtime/公共合同。修前 1/2、0/1 和中间 12/0 独立保留，最终 M6 Store **13/0**、
Store/应用 Clippy、静态 xtask 通过；因实际生产输入变化仅再构建 Core。新隔离 Core 验证 permission_denied、
完整公开记录不变、拒绝不消耗 key、未声明授权拒绝、UI-only 无自动授权及同连接 UI 正常。
GUI/客户端/协议输入不变，复用原 GUI 构建和 36/0，不重复 Tauri/浏览器或完整 Workspace。

前轮原环境已恢复到当时 Core/GUI，五组完整公开状态一致、revision 155；当轮隔离进程均已收束，无用户 Unity/不明进程被处理。
Esc 中断后未完成新增原生 GUI 交互及整改后截图，不将浏览器/Core 结果冒充 native PASS。当前状态为本地候选待人工验收，
H2-A/H2-B/H3 签收保持，M7 继续进行中。完整构建摘要、历史失败、实际窗口及四项清单见
[H4 本地验证记录](../testing/m7-h4-extensions-20261102.md)。最终文档整理只运行 metadata/diff，不重新构建或全量测试。

所有者随后要求补齐实际可操作的隔离样例，并明确保持 Esc 后桌面控制停用。本轮复用同一签名正常包、已有隔离项目和
`data-native`；经正式 Core 完成正常包 retain-data 卸载/重装/启停准备，最终留在未安装状态供人工从安装审阅开始。
固定本地 `H4-Review.ps1` 已实际 Start→Restore 核对五组一致，再 Start 交付隔离 GUI；不调用桌面输入接口，
不向原环境装样例、不改产品代码或协议、不重新构建/运行测试套件。原环境当前为人工查看临时暂停，验收结束必须
实际 Restore 并核对，不能继续沿用前轮“原窗口已打开”冒充隔离候选。入口、短清单、现有权限证据与未制造状态见同一 testing 记录。

随后截图中的 `extension_publisher_confirmation_required` 暴露共享错误缺少用户说明，所有者要求处理多处原始返回显示。
`52ed23b` 只修 GUI 提示：有限稳定码映射、中英日文字、默认折叠技术标识及必要公共/资源接线；原始 message/data 不进入正文，
取消、重试、进度、未知结果和信任/签名/权限逻辑保持。关联首轮 78/1 后，唯一测试焦点顺序问题定向 6/0，逐轮保留；
check 和必要 GUI 构建通过，Core/协议/依赖未变，不重跑 Rust、浏览器全套或远端 CI。当前隔离窗口已更新，原生交互仍待所有者查看。

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
- 2026-08-26：项目所有者纠正“参照 v3”被机械解释为固定导航树的问题。A-033 修订为 recognizable structural/product
  continuity；新增 divergence budget 与 Candidate A（project-first）、B（three workspace hubs）、C（bounded Overview）IA
  synthesis。Material Web事实结论、`192672…`功能证据与M7边界不变；尚未选择production target，H0/H1未开始。
- 2026-08-26：项目所有者进一步冻结“v4默认继承v3用户模型”的前提。A/B/C降为historical exploration；建立v3/v4
  user-facing capability map，将M1-M7绝大多数变化归为same/enhanced existing capability，仅把durable task recovery/history与
  extension permission/quarantine视为嵌入既有区域的有限新用户价值。形成单一minimum-change proposal；production未开始。
- 2026-08-26：项目所有者批准minimum-change IA baseline与H0/H1 production，并冻结Project tree只表达语义归属、Portable UI
  无navigation contribution、Task Center为secondary utility、Update仅在真实capability存在时显示。H1后停止Visual Gate 1。
- 2026-08-26：H0以`@alcomd/ui`集中注册Material Web 2.5.0、共享MD3 tokens和窄React 19 facade完成；H1将一级用户区域
  收敛为Projects、Packages & Templates、Extensions、Settings、Log及footer utilities，默认进入Projects并保留全部既有route。
  npm check/build和Playwright 15项通过；等待完整repository/Tauri gate与真实GUI Visual Gate 1签收。
- 2026-08-26：项目所有者在真实 GUI 中拒绝首次 H1 的视觉方向，分类为 `rejected_for_visual_direction`；技术 gate 不冒充
  visual acceptance。停止对该 shell 的补丁式修改，改为以 v3 发布图和冻结 GUI 源码为直接 reference，先形成 Projects 的
  relative geometry、其他主要页面、共同 component/layout vocabulary 与 MD3 translation，再仅重建 wide Projects prototype。
- 2026-08-26：项目所有者以真实 v3 Projects reference 和真实 release GUI 对照，人工确认新的 wide desktop shell、sidebar/
  main canvas、Projects toolbar、桌面密度及 Material Web Button/TextField/Select 的 interaction、ripple/state layer 与 keyboard
  focus；Visual Gate 1 为 `PASS`。独立检查点 `7bb325b78a74518881fb65e03de7f79902e72cd3`，该结论不代表整个 M7 visual
  acceptance 完成。获批下一步仅为 H2-A Project workspace / Packages。
- 2026-08-26：项目所有者批准唯一 production UI asset dependency `@material-symbols/svg-400 = 0.47.0`，冻结
  Material Symbols Rounded/weight 400、default fill 0、selected navigation fill 1、`@alcomd/ui` ownership、显式
  `?url` import 与 CSS mask。动态字符串 registry、remote font/CDN、SVGR 和第二套通用 icon language 保持禁止。
- 2026-08-26：icon foundation 以独立提交 `ea3312b1c10be2f021a5c516cec61449778be761` 落盘；lock closure 只有
  一个零传递 package record。H2-A Project package workspace 已完成本地候选并只增加当前可见 action 所需的十个
  named Rounded glyph。production bundle 精确包含二十个 Material Symbols data URL（十个 navigation regular/filled 与
  十个 H2 action），不含完整 icon corpus、Outlined/Sharp、远程字体或 emitted SVG。npm check/build、Playwright 17项、
  Tauri release no-bundle、fmt/clippy/Workspace tests、xtask、metadata、baseline freeze 与 diff gate 已通过；Visual Gate 2
  尚未人工验收。
- 2026-08-26：项目所有者查看 `1eddee436fcd0343e57d85f300312dab1c97e120` real release GUI 后拒绝首轮
  Visual Gate 2：Material text button 使 sidebar icon/label group 居中，Projects toolbar 的 refresh、view switch 与排序方向仍以
  纯文字表达，偏离 v3 reference 和已冻结 icon contract。修正只改为 left-aligned semantic navigation + MD3 state layer，
  并为实际 toolbar action 接入 named Material Symbols；不改变页面、RPC、状态或业务流程。
- 2026-08-26：首轮 Visual Gate 2 修正候选已完成本地技术验证：npm check/build、Playwright 18 项、Tauri release
  no-bundle、xtask、metadata、baseline freeze 与 diff gate 均通过；shipping bundle 包含 30 个实际使用的 SVG data URL，
  不含 Google Fonts 请求或第三方 package path。Visual Gate 2 仍等待项目所有者复看 real release GUI，不因自动化通过而关闭。
- 2026-08-26：项目所有者复看 `6dd463852227f565b055fdd8ada21575ee9ba6a8` 后确认左对齐问题已修复，但拒绝继续用
  native control + custom CSS 模拟 Material Web component。正式执行规则修正为：只要 `@material/web 2.5.0` 存在适用组件，
  official GUI 和 Portable UI 必须经 `@alcomd/ui` 的窄 facade 渲染真实 `md-*` element；native button/input/select/
  textarea/progress/dialog 均由 source gate 禁止。2.5.0 没有完整 Navigation Drawer/Rail，且 `md-list-item` 不能提供本导航
  所需的 button/`aria-current` 合同，因此 primary navigation 是唯一窄例外：保留 semantic native button，并组合真实
  `md-ripple`/`md-focus-ring`，不再用伪元素或 JavaScript 模拟 Material state layer。该例外不得扩展到其他控件。
- 2026-08-27：项目所有者要求导航实现以 Material Web 官方页面源码而非截图近似为准。只读核对官方仓库
  `cac97678831d48d4eb4a606ca50f92673a1dc20c` 后，确认 catalog drawer 的真实 composition 是 custom `nav-drawer`
  shell + `md-list` + `md-list-item`，并以 12px list/item spacing、28px shape 和 `surface-container-highest` selected
  container 实现。Primary Navigation 因此移除最后一个 native button exception，改由 `@alcomd/ui` 集中注册并由 GUI 窄
  facade 渲染真实 Material List/ListItem；semantic aside/nav shell保留，未采用labs drawer、未新增dependency，也未改变
  route/RPC/state/permission。Material 2.5.0 的 delegated ARIA行为由integration test固定，不以shadow DOM私有结构作为合同。
- 2026-08-27：项目所有者复看后指出连续 navigation items 仍过于松散。进一步核对 Material 3 Navigation Drawer
  component contract，确认 item shape top/bottom inset 为 0dp；Material Web catalog site 的 12px block margin 是文档站局部
  layout，并非必须继承的MD3导航间距。ALCOMD移除连续item之间的额外margin，保留56px component height、28px shape、
  12px drawer inline inset、Material state layer以及独立区域之间的语义分隔。
- 2026-08-27：项目所有者最终选择 v3 final 的实际尺寸作为导航密度基线，并明确包含总宽、高度和间距。只读审计
  `SideBar.tsx` 冻结：260px sidebar、12px padding、4px item gap、48px item height、16px item inline padding、
  20px icon、16px icon-label gap、full-pill shape、14px/20px weight-500 label和
  `surface-container-highest` selected/hover container。v4只覆盖Material List tokens与host spacing，仍使用真实
  `md-list`/`md-list-item`、Material Symbols和既有route/RPC边界，不复制v3 React/Tailwind实现。
- 2026-08-27：项目所有者纠正 icon infrastructure：`@material-symbols/svg-400`/`svg-500` static SVG 固定
  `opsz=48`，均标记 `rejected_as_final_icon_source` 并从 manifest/lock 移除。最终只从 Google 官方
  `google/material-design-icons@e083cc60a0828fdd3b404cea0cb8a5b900e9c23e` vendoring 当前实际使用的 Rounded / weight
  400 / grade 0 / fill 0 / opsz 20 或 24 SVG；`@alcomd/ui` 按 exact optical size 提供闭合 named export，mask 保留
  官方 geometry/currentColor 并删除 120% 裁边及单图标补丁。selected navigation 继续复用 fill-0 symbol，只由
  Material selected container/state 表达选择。
- 2026-08-27：项目所有者在真实 release GUI 中观察后判定 primary navigation 的真实 20px/opsz20 symbols 视觉过小。
  primary navigation 因此整体使用真实 24px/opsz24 SVG，并把 host icon-label gap 从 16px 调整为 12px，使标签起点、
  48px item height 与其他 v3-derived shell geometry 保持不变；没有裁边、scale、weight override 或 per-icon 特例。
- 2026-08-27：项目所有者批准 Projects 列表移除低价值 package count，并以 v3 的添加日期体验恢复真实注册时间。
  RPC v1 只兼容增加可选 `ProjectSnapshot.registeredAtMs`：注册项目返回 store 中既有的 registration timestamp，
  未注册 `projects.inspect` snapshot 省略；不新增 method、capability、permission 或 State Schema，GUI 不得以
  `observedAtMs` 伪造添加日期。
- 2026-08-28：项目所有者在真实 release GUI 中逐项复看 Projects 列表后认可当前局部实现。列表模式只通过表头排序，
  Project 列保留可见最小宽度并在宽屏伸展，其余可预测列与 action region 按最长冻结文案锁定宽度；Added 使用
  `YYYY-MM-DD`，Last observed 使用相对时间。表头与单元格统一左对齐、单行截断，Type 的 24px symbol 与文本基线对齐；
  Open Unity、Manage、Backups 与 Material overflow menu 均保留固定 action footprint。Primary navigation 不提供收起状态，
  窄宽度直接裁切而不产生额外滚动条。本次局部认可不关闭完整 Visual Gate 2，也不批准其他 H2、H3-H7。
- 2026-08-28：Project Actions Functional Parity Audit 经项目所有者接受。确认当前 permanent-disabled visible fake
  entries 只有 Open Project Directory 与 Copy Project，但 Add/Register chooser、Create、Restore、Favorite、Unity
  preference clear、package reinstall/bulk/source/local/prerelease/hidden/docs/changelog、Remove Directory 与 M11 VCC migration
  等真实缺口不能被 fake-entry grep 覆盖。H2 visual work暂停，形成独立 Project/Package Functional Closure Stop A proposal、
  State v10 Copy-only migration contract、fixtures/tests、dependency evaluation 与 milestone ownership；active RPC、production
  migration/implementation和依赖均未改变。
- 2026-08-28：项目所有者附合同修正批准 Functional Closure P0-P4 production。exact opener/dialog dependency、closed
  ProjectId opener/native picker、active Project Copy RPC/State v10、durable inventory/two-pass content verification、forward
  recovery、CLI 与 list/grid/workspace GUI flow 已形成 local candidate；每个 durable Copy phase 的真实 kill/restart matrix 已通过。
  H2 visual 与 P5-P8 继续暂停，完整聚合 feature 状态未提升。
- 2026-08-30：P6 remote checkpoint 已由项目所有者验收为 PASS，sealed HEAD
  `0f83536bdeb74116e4f8d533c9fc3bf5bea529fb`。随后只创建 P7 Delete Project Directory proposal-only Stop A：完成 v3
  exact behavior、现有 v4 primitive、Windows/Linux/macOS delete primitive 与 Rust `remove_dir_all` 审计，提出 ProjectId-only
  Plan/Apply、独立 `projects.delete`、sibling quarantine permanent-delete、State v13 durable evidence 与 recovery/path vectors；
  active RPC、State、Permission、production source、dependency、unsafe 与平台 API 均未改变。
- 2026-08-31：Visual Gate 2 期间重新冻结 Project Unity Version / Open Unity 产品模型。v3 exact behavior 与当前 v4
  Automatic/Explicit preference model 已完成源码审计；Stop A 提出 canonical exact version、0/1/>1 Open Unity、one-shot
  installation chooser、独立 migration Plan/Apply/Operation、State v14 preference removal/launch-config split 与 crash recovery。
  VRChat 2019 -> 2022 当时识别为 secondary preparation 缺口，未扩展 P6。P8 historical PASS
  保留，但当前 completeness 为 `REOPENED_BY_PRODUCT_MODEL_CHANGE`；H2-A 暂停而非拒绝。
- 2026-08-31：Unity model proposal chain `a3faa7b…`、`baa4768…` 的最终 owner review 为
  `PASS WITH IMPLEMENTATION AMENDMENTS`；State v14、exact launch、migration Plan/Operation、private preparation 与窄平台进程
  合同已 seal。contract-only remote checkpoint 全绿后 production 自动获准开始；H2 visual 仍暂停。final amendment 补齐
  Launch Config/launchOptions/required one-shot launch、single-authority migration Plan、downgrade fail-closed、
  `vrchat-2019-to-2022-v1` bounded private preparation、final phases 与窄 UnityMigrationProcess platform contract；
  H2/P8/global 状态不变。
- 2026-08-31：Unity production sealed HEAD `e9349fd0f4cc9c18d9174dfc5826d48654aeb6f5` 已通过
  Hosted CI run `33352406804` 与 CodeQL run `33352406214`，更新后的 P8 M7-owned completeness 为 PASS；
  global 仍为 `BLOCKED_BY_M11`。H2-A 从该 sealed HEAD 恢复 Visual Gate 2；`24b03916…` 只保留为
  pre-Unity-model local checkpoint。

- 2026-09-06：H2-A local candidate 的 shared icon renderer 改为官方支持的 `md-icon` + SVG，
  继续使用 pinned Google Rounded 400 资源；移除 mask 与 wrapper 的 button icon size 特例。
  真实 Windows GUI 操作发现并修复同路由 reconnect 后 capability snapshot 未刷新的问题；
  final browser suite 44/44 与 Tauri no-bundle build 通过。真实 package review / Copy review 的
  布局和 Escape 取消已检查，未执行复制或包移除。窄窗口 sticky action 遮盖数据列仍是未通过项，
  不以自动测试通过替代视觉验收；详见 `docs/testing/m7-icon-reconnect-local-acceptance.md`。

- 2026-09-06：项目所有者纠正 icon integration 范围：不要求窄窗口适配，不授权更换按钮图标。
  撤销 Open Unity 的 `open_in_new`/trailing 替换，恢复原 `play_arrow`/leading；仅保留 `md-icon`
  与官方 SVG 接入修正，不引入响应式菜单或其他布局改变。

- 2026-09-06：项目所有者接受普通 Material Web button 的 18px icon；移除临时双图标比较，
  保持单个原始 leading play。最终前端 check、44/44 browser tests 与 debug no-bundle build 通过。
  computer-use 在隔离 synthetic fixture 中完成真实 Copy 的 review/Apply/成功反馈/列表刷新与
  副本 Manage；源文件未变，副本 5 文件 SHA-256 全部一致。缺失精确 Editor 提示与 Escape 通过。
  本轮没有真实 Unity launch、migration、Hosted CI 或 Visual Gate 2 人工通过结论。

- 2026-09-06：按项目所有者要求继续对照 v3 实际源码，补齐 H2-A 包管理工具栏与筛选入口：
  shared Material search、icon Refresh、maintenance overflow、repository/local/prerelease filters，
  复用既有 settings revision/RPC。48/48 browser tests、frontend check/build、debug no-bundle build、
  xtask 与 metadata 检查通过。桌面新筛选交互尚未完成验收（电脑控制检测到用户输入，已暂停点击）；
  不把浏览器测试冒充 real Tauri evidence。版本选择器、条件更新与批量入口仍是未完成项，详见
  `docs/testing/m7-h2-a-package-workspace-continuity.md`。Visual Gate 2 保持开放。

- 2026-09-17：按项目所有者要求收敛为标准目录单工作树。原 main 的 9 个未提交文件保存为历史快照
  `b4547d5`；整合提交 `9d0ddbb` 保留双方历史且采用 `9307572` 精确代码树，不复活 Automatic Editor。
  两个额外 worktree 已移除，本地材料保存在标准目录恢复归档，旧分支统一归为 `codex/archive/*`。
  本次只做工作空间与记录整合，没有 GUI 新功能、合同调整、push 或视觉验收结论。

## 下一停止点

P6 实际 wire 字段修复与候选 DTO 分离已按项目所有者批准落实，其完整 Rust 483/0/8、公开
RPC 8/8 和六种 Plan review 证据保持。本轮 H2-A 已形成真实桌面签收候选，完成 GUI
Apply/同一 Operation、失效 Plan 拒绝、焦点、筛选、语言/主题持久化及环境恢复。
最终完整 Chromium 在同一输入下独立通过 247/0，零重试；首轮 246/1 网络加载失败及各构建
截图身份独立记录于 `docs/testing/m7-visual-gate-2-20261031.md`，不拼接定向结果。
本地门禁与材料已收口，项目所有者于 2026-10-31 以“可以，接下来做什么”确认签收 Visual Gate 2。
H2-A 验收检查点为 `23f545245f613f47f8def1e16418c4d12303404e`。
技术收尾已在 `97c761f` 取得同一 SHA 三平台完整 CI/CodeQL 成功，H2-B 已按既有授权实施。
前次 H2-B 运行时代码候选为 `cbb4c41`：完整 Rust/构建通过。资源加载专项只改测试基础设施，
独立完整 Chromium 已通过 292/0，修复与相关文档已用真实 Windows 用户环境提交并普通快进
推送为 `0ef5b088dfdb7a8eb2b7a88ebef94ae436c90f50`；原失败轮与网络/root/NetLog 证据保持。
该 SHA 的 CI `41287607183` 三平台及 CodeQL `41287607108` 四语言均在 attempt 1 完整成功。
隔离桌面的结果链接精确导航、既有键盘合同、构建/截图 hash、未覆盖项与原环境恢复已记录。
只补明确缺口并收集既有运行，没有重复完整本地测试或触发 workflow。先前被审批拒绝的 builtin
confirmed CLI remove 已在新明确授权后通过正式 CLI 补齐，Core 拒绝及零副作用证据独立保存，
历史未执行记录保留。项目所有者随后实际查看并拒绝 H2-B 体验签收，指出提示布局和列表交换缺口。
本轮按上述批准合同整改，本地提示提交为 `348dbef`、列表功能为 `ceef00f`；`2a163b0` 只修测试
类型检查，不改变运行时代码。旧 release 构建已独立保留，新候选已在同一隔离数据环境启动。
该候选集中定向浏览器 47/0，相关 Rust/协议/Clippy 及 release 构建通过；未复跑旧完整门禁。
此前因 Computer Use 中断未取得 Agent 的新原生交互/整改后截图证据，历史缺口保留；本次
项目所有者明确确认“阶段性人工验收通过，保留体验改进项”，允许结束 H2-B 本轮体验整改。
未将该签收提升为体验完善、整个 Visual Gate 3 或 M7 完成。按授权实际恢复原 Core/开发窗口，
五组公开状态与暂停前一致、revision 155，不再仅记录守候等待。
获批 G3 的本轮完成度核对已完成，现有 RPC 覆盖矩阵与公开方法 92/92 对齐；方法归属不冒充独立 GUI 动作全部完成。
项目所有者随后明确批准既有 H3，上轮形成 `ab5d183` 本地代码候选：同 SHA 定向回归 34/0、GUI check
与 release 构建通过，主体备份/恢复的原生证据仍绑定实际 `ef47c25` 构建，新增启动状态弹窗未完成原生查看。
原开发环境再次实际恢复，五组完整公开状态一致、revision 155。`efdb133` 修复 Windows Editor 版本读取后，
项目所有者已确认检测、显示及重复刷新；该轮 Esc 后未取得原生查看的历史记录保留。
随后路径显示反馈已由 `fec8117` 共享格式化窄修，10/0 与当前原生安装页/刷新、前后截图和 RPC 不变均已记录。
所有者已确认本次路径显示正常，Unity 检测、显示和重复刷新确认保持，不重复要求查看已通过项。
所有者已进一步确认 H3 阶段性人工验收通过，保留非阻塞体验问题；原环境/验收窗口核对完成，直接推进获批 H4。
H4 已形成本地代码候选 `e2b7ddd`，两处权限权威缺陷已窄修；最终 Store 13/0 和隔离 Core 复验通过。
GUI 的 155 项输入仍与 `88c3f69` 一致，复用该构建和 36/0；原环境已实际恢复。
新增原生交互/整改后截图因 Esc 尚未覆盖，下一停止点为窗口、清单与 H4 人工确认，不重验 H2/H3，不宣布 M7 完成。
以上为 H4 候选交付时的历史状态。所有者随后已于 2026-11-02 阶段性签收 H4，并批准直接实施 H5。
隔离任务已全终态，原 Core/GUI 已实际恢复；五组公开状态与切换前完整快照一致，详见现有 status/testing 文档。
H2-A、H2-B、H3 签收保持；H2-B 新增代码、H3 与 H4 仍无对应远端验证，不用旧 CI 替代。
不重规划 M7，不重做已有页面；只推进已批准 H5，不进入 H6/H7、M8/M9/M11，不推送或宣布 M7 完成。
已有候选读取、exact Unity、Config3、Hub 和 State16 的合同继续有效。完整 GUI/Rust/三平台
技术门禁与人工视觉结果分别记录，不把旧 SHA 或定向通过冒充当前候选全绿。
