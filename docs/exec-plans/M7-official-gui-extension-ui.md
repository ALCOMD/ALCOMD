# M7：官方 GUI 与 Portable Extension UI

状态：M7 Phase 0 architecture reset 与 obsolete WebView probe cleanup 已完成；Portable UI contract-first Stop A candidate
已形成并等待项目所有者人工审批；M7 production implementation 未开始，尚未进入 M8。

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
Stop A candidate，不满足 production 完成定义。

## 已完成前置证据

- M6 final `b666e8fcac6fd4153750c401b76f2f61f6d2a34a` / CI `32724915827` 已通过最终人工验收；M6 backend runtime
  acceptance 保持有效。
- M7 WebView evidence 只证明 sandboxed iframe 与 Tauri managed child 不适合作为 v1 产品方向，保留在
  `specs/gui/m7-stop-a.md`，不属于 security/compatibility success。
- Phase 0 architecture reset commit `12089661f7c694f059aaf172344809ff814716f6` 独立保留。
- obsolete test-only WebView harness cleanup commit `c05c5d2c712dd7a791401efc22b8dd19b88023a6` 独立保留；普通 Tauri
  build/no-bundle gate 未删除。

## Stop A frozen candidate

所有下述内容仍是 proposal；没有 production parser、binding、migration、DTO、permission enum、renderer 或 capability。

### Manifest、package 与 surface

Manifest v1 future direct rewrite 使用：

```toml
[entrypoints]
component = "component/extension.wasm"

[ui]
protocol = "portable-v1"
```

`background_component` 直接改名为 `component`，不保留 alias；删除 `ui_entry`。package allowed roots 只剩
`alcomd-extension.toml`、`META-INF/`、`component/`，删除 `ui/` 与 UI quota，保留全部 ZIP/signature/digest 安全规则。

v1 采用一个隐式 stable surface `main`。MCP management 与 Discord Presence synthetic fixture 都可在一个 page 内使用
section/list/form 表达，当前没有引入最多 8 surfaces 所需的真实证据。extension record optional addition 固定为：

```json
{"ui":{"protocol":"portable-v1","surfaces":["main"]}}
```

### Capability 与 consumer completeness

客户端在 hello 请求 `extensions.ui.portable.v1`；daemon 完整实现后才回显。声明支持的 consumer 必须实现全部 v1
node/action。没有 hostId、official GUI identity、framework/renderer name、feature subset/intersection 或 GUI-specific tree。
不支持的 GUI 继续提供 extension management，并把功能页标为 unsupported。unknown v1 node fail closed；新增 node 使用
新的 capability/version。

### WIT/ABI v1 proposal

proposal-only WIT 位于 `specs/extensions/wit/extension-v1-portable-ui-proposal/`，不修改 production bindgen 读取的
active `extension-v1/`。最终仍是 `alcomd:extension/extension-v1@1.0.0`，import host-projects/host-data，export
guest-lifecycle/guest-ui。guest-ui exact sync functions：

```text
open(session-id, surface-id, locale) -> UiDocument
refresh(session-id) -> UiDocument
dispatch(session-id, sequence, request-id, UiAction) -> UiDocument
close(session-id) -> result
```

Host embedding 继续 async；不启用 component-model-async、WASI future/stream、wasmtime-wasi、guest thread 或依赖。
production 获批时 proposal 原子替换 active v1 并删除 proposal directory；不保留 parallel world，不创建 ABI v2，
`extensionApi.major` 仍为 1。

### Lifecycle 与 Host protocol

activation kind 固定 `background|interactive-ui`；stop reason增加 `interactive-ui-idle`，并保留 disabled、permission-revoked、
lease-expired、daemon-shutdown、uninstalling。`background.run` 只授权无 active UI session 时继续后台运行，不授权 UI/
业务/network/filesystem/长调用。UI-only extension 可 install/enable，open按需创建 interactive-ui lease；最后 session
关闭且无 background lease后 5,000 ms 停 Host。已 background-running 时 open不重复 activate。

daemon 为每个 invoke-export签发 `InvocationContextId`，绑定 invocation kind、Extension lease/grant/generation、deadline/
cancel，以及 interactive-ui 的 Client Principal/connection/UiSession/snapshot。capability-call必须回显。well-formed stale
context返回内部 `invocation_context_stale`；unknown/wrong/cross-session context 是 Host protocol violation并终止 Host。
requestId/callId 仍只做 correlation。

### Document、tree、form 与 action

Guest 只返回完整 UiDocument；daemon验证并包装从 1 单调递增的 UiSnapshot revision。flat tree必须恰好一个 page root、
parent-before-child、无 cycle、连续唯一 sibling order、unique node/field/action ID、depth 8。固定 node：

- layout：page、section、stack、group、form、list、list-item；
- display：text、status、key-value、progress、divider；
- input：button、switch、text-field、integer-field、select。

tone 只有 neutral/info/success/warning/danger；progress 为 indeterminate 或 0-10000 basis points；integer 为 JSON-safe
signed integer。禁止 HTML/CSS/JS/Markdown HTML/canvas/image/icon URL/link/URI/font/color/animation/absolute/custom ARIA/
DOM/component/expression/iframe/WebView/navigation。

switch/select/text/integer 只属于 form，GUI本地维护 draft；v1 action只有 `activate` 与 `submit-form`，没有 per-keystroke
或 immediate change。submit按 node order携带完整 typed field set，daemon先验证 current Snapshot、disabled、type、required、
length/range/option/completeness/size，绝不转发任意 JSON。

### Exact limits

机器权威值在 `specs/extensions/portable-ui-limits-v1.json`：surface 1；session per extension/client/daemon 为 8/16/128；
Snapshot/action 262,144/65,536 bytes；nodes 256；depth 8；ID 1-64 lowercase ASCII；single/total text 4,096/65,536
UTF-8 bytes；form 64 fields；select 64 options、label 256 bytes；Host/session concurrency 1；token bucket 60/minute、
burst 10；idle/absolute 300,000/3,600,000 ms；request cache 64；Host idle stop 5,000 ms。

plain text拒绝 NUL、C0/C1/DEL与冻结 bidi controls。text node允许 LF；text-field只有 multiline 时允许 LF；其他字段
单行，CR/TAB始终拒绝。locale只接受 2-15 ASCII bytes 的 canonical language[-Script][-Region] core BCP47 子集。

### Session、replay 与 failure

memory-only session绑定 Client Principal/connection、Extension Principal/instance/package、main surface、grant/lifecycle/
snapshot revisions、locale/deadlines、next sequence和64-entry replay cache。disconnect/restart/disable/revoke/uninstall/
package or generation change/Host crash/quarantine/timeout/protocol violation立即关闭。session不写 DB/Event/data/package/log，
不 crash-recover，不创建 Operation。

dispatch严格 sequence + unique requestId。live duplicate pair返回 current Snapshot、`replayed=true`且不调用 guest；mismatched/
out-of-order/replayed ID fail closed。断线前未知结果不自动重发，新 session通过 Snapshot观察。client invalid action在 guest
前拒绝；60秒第三次关闭该 session但不惩罚 Host。guest invalid document立即关闭 session、终止 Host、计入 existing
crash/quarantine，返回 document-invalid或limit-exceeded，Event/diagnostic只保留 bounded safe classification。

### Authorization 与 RPC

client candidate permission 是 `extensions.ui.use` scoped exact ExtensionId；没有 extension `ui.contribute`。list/get UI
declaration用 extensions.read；open/refresh/dispatch用 extensions.read + extensions.ui.use + exact scope；close返回
`{closed:boolean}`并 best effort，不泄露 owner。

UI invocation 内 Host business capability取 Client Principal与 Extension Principal同一 resource scope交集；host-data
仍由 extension self namespace authority控制，client只需 UI permission。high-impact Plan/credential/Operation继续 host-owned。

RPC compatible addition只有 capability 与 open/refresh/dispatch/close；没有 listSurfaces。DTO closed/bounded，open只接收
extensionId/main/locale，不接收 host/framework/platform。stable error分类由 `m7-portable-ui.schema.json`冻结，不创建宽泛
`extension_ui_failed`，也不公开 Host PID/pipe/lease/path/identity/context。

### State v9 与 renderer

M6 已广告 v8，故 proposal使用 v9：extensions和immutable extension_plans各增 ui_protocol、ui_surfaces_json；canonical
值只允许 `(NULL,[])` 或 `(portable-v1,[main])`。没有 session/Snapshot/action/renderer/browser/cache/workflow table，不创建
production 0009 SQL。additive v8->v9只作为未来实现便利，不作公开 compatibility promise；开发 DB可 reset。

官方 React/MD3 renderer与non-Tauri headless consumer使用同一 public DTO/fixtures。官方 contract冻结 host-owned chrome、
exhaustive match、keyboard/focus/ARIA、200%/320px、reduced motion、无 extension CSS/DOM/Tauri。headless consumer不依赖
GUI/Tauri并输出确定性 semantic summary。Stop A不实现任何 renderer。

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
- 2026-08-25：形成本Portable UI Stop A proposal、synthetic fixtures与contract tests；active/production未修改，等待人工审批。

## 下一停止点

创建独立本地 `docs: freeze M7 portable UI contract candidate` commit，不push。报告精确合同、验证、HEAD/origin/main与
clean worktree后停止。未经项目所有者批准不得开始active replacement、Host/Core/React implementation或M8。
