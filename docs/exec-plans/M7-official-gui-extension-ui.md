# M7：官方 GUI 与 Portable Extension UI

状态：M7 Portable UI Stop A 与执行语义补充已通过最终人工审批；Slice B0 active contract replacement 已完成本地实现与
验证，正在进入 Slice B1 Core + Extension Host；M7 尚未完成，尚未进入 M8。

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

M7 完成需要：Stop A 人工批准；Core/Host implementation；官方 renderer；headless conformance consumer；本地完整 gate；
Windows Server 2025、Ubuntu 22.04、macOS 15 arm64 Hosted CI；GUI/a11y evidence；项目所有者最终人工验收。当前只完成
Stop A approved candidate contract evidence，不满足 production 完成定义。

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
`ui_protocol`、`extensions.ui.use` 与 RPC DTO foundation；Session runtime、capability advertising 和 renderer 分别留在
B1/C，不能由 B0 状态冒充完成。

### Manifest、package 与单一 Portable UI

Manifest v1 future direct rewrite 使用：

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

### WIT/ABI v1 proposal

proposal-only WIT 位于 `specs/extensions/wit/extension-v1-portable-ui-proposal/`，不修改 production bindgen 读取的
active `extension-v1/`。最终仍是 `alcomd:extension/extension-v1@1.0.0`，import host-projects/host-data，required export
guest-lifecycle/guest-ui。所有ABI v1 Component都必须实现guest-ui；Manifest无 `[ui]` 时daemon不调用，SDK/reference
提供empty stub，Host instantiate仍验证完整world。guest-ui exact sync functions：

```text
open(guest-session-id, locale) -> UiDocument
refresh(guest-session-id) -> UiDocument
dispatch(guest-session-id, UiAction) -> UiDocument
close(guest-session-id)
```

Host embedding 继续 async；不启用 component-model-async、WASI future/stream、wasmtime-wasi、guest thread 或依赖。
production 获批时 proposal 原子替换 active v1 并删除 proposal directory；不保留 parallel world，不创建 ABI v2，
`extensionApi.major` 仍为 1。

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

M6 已广告v8，故proposal使用v9：extensions和immutable extension_plans各只增nullable `ui_protocol`，合法值NULL或
`portable-v1`。不保存页面常量，不建session/Snapshot/replay/action/draft/renderer/browser/cache/workflow table。v8->v9
必须单事务保留rows/FK/revision/Event sequence，existing默认NULL，future schema fail closed；production wiring前hello继续
广告v8，不创建production 0009 SQL。

官方 React/MD3 renderer与non-Tauri headless consumer使用同一 public DTO/fixtures。官方 contract冻结 host-owned
name/ExtensionId/publisher-trust/version/desired/runtime/quarantine/extension-provided chrome、exhaustive match、
keyboard/focus/ARIA、200%/320px、reduced motion、无 extension CSS/DOM/Tauri。headless consumer不依赖GUI/Tauri并输出
确定性 semantic summary。Stop A不实现任何 renderer。

## Contract artifacts

- ADR refinement：`docs/adr/0024-portable-extension-ui.md`；
- Portable UI contract/schema/limits：`specs/extensions/portable-ui-v1.*`、`portable-ui-limits-v1.*`；
- Manifest/package/ABI/permission/Host protocol proposals：`specs/extensions/proposals/`；
- WIT proposal：`specs/extensions/wit/extension-v1-portable-ui-proposal/`；
- RPC proposal：`specs/rpc/m7-portable-ui.schema.json`；
- State v9/migration proposal：`specs/storage/state-v9*`；
- threat model/consumer contract：`specs/security/extension-portable-ui-threat-model.md`、
  `specs/gui/portable-ui-renderer-v1.md`；
- MCP/Discord/adversarial/headless fixtures：`crates/alcomd-testing/fixtures/m7/`；
- contract tests：`crates/alcomd-testing/tests/m7_contract.rs`。

## 后续 production slices（未批准）

1. Slice B：原子替换 active Manifest/package/WIT/ABI/permission/State contract，接入 Host/Core/RPC session与dual authority。
2. Slice C：官方 shell/typed adapter/React/MD3 renderer与既有 Core pages，仍只调用 public client/RPC。
3. Slice D：non-Tauri consumer、first/third parity、malformed/revoke/crash/reconnect与三平台/a11y验收。

不得同时保留旧 Web UI和Portable UI parser/world/renderer，不建立通用 UI/workflow engine。

## Stop A 允许修改范围

```text
docs/adr/0024-portable-extension-ui.md
docs/exec-plans/M7-official-gui-extension-ui.md
docs/status.md
docs/testing/test-plan.toml
feature-parity.toml
specs/extensions/proposals/
specs/extensions/portable-ui-v1.*
specs/extensions/portable-ui-limits-v1.*
specs/extensions/wit/extension-v1-portable-ui-proposal/
specs/gui/portable-ui-renderer-v1.md
specs/rpc/m7-portable-ui.schema.json
specs/security/extension-threat-model.md
specs/security/extension-portable-ui-threat-model.md
specs/storage/state-v9*
crates/alcomd-testing/fixtures/m7/
crates/alcomd-testing/tests/m7_contract.rs
```

cleanup commit另允许删除obsolete probe与CI probe steps。Stop A禁止 active Manifest/package/WIT、production Host/daemon/
application/protocol/store/Permission/React/Tauri、Cargo/npm manifests/locks、dependency、unsafe、platform API。

## 明确排除

- M8 MCP backend/management product logic、M9 Discord IPC/Presence product logic；fixtures只证明UI表达能力；
- custom Web UI、iframe/child WebView/WebviewWindow、asset/origin/CSP/Tauri capability；
- Local API、migration/bootstrap/updater/installer/signing/dist与M12 Windows client validation；
- WASI 0.3、component-model-async、wasmtime-wasi、guest thread、新 dependency；
- production GUI/Core/Host/State/RPC wiring与M8进入。

## 验证与 release blockers

Stop A运行 format/clippy/workspace tests、xtask、metadata、baseline freeze、diff check。必须证明 Cargo/npm manifests与三份
lockfile不变、active WIT不变、production source不变、unsafe/platform API不变。

Stop A人工批准前，任何 active contract replacement或production capability都是blocker。后续 first-party private node/page/
command/permission、partial v1 renderer、GUI-to-Host direct channel、renderer作为business authority均是M7 blocker。
M11真实v3 fixture缺失继续阻塞GUI differential parity；M12继续承担Win10/Win11安装/启动/WebView2/update/uninstall。

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
  Web UI compatibility path。B1 尚未完成，因此 hello 继续广告 dataSchema 8且不回显Portable UI capability。

## 下一停止点

继续 Slice B1 Core + Extension Host，完成 UiSessionCoordinator、InvocationContext、guest-ui binding、interactive lifecycle、
replay、dual authorization、validation 与 invalidation/recovery。保持职责清晰的本地提交且不 push；遇到已列 stop condition
立即停止。不得开始 M8/M9。
