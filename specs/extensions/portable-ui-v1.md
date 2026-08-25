# ALCOMD Portable UI v1 contract candidate

状态：M7 contract-first Stop A review closure；proposal-only，尚未被 daemon、Host、RPC client 或 renderer 实现或广告。

## 协议与单一页面

Portable UI 是 Core/RPC contract，不是 Tauri API。客户端在 `system.hello.params.capabilities` 请求
`extensions.ui.portable.v1`；daemon 只有完整实现 v1 后才回显。声明该 capability 的 consumer 必须完整支持 v1，
unknown node/action fail closed。v1 不协商 hostId、framework、renderer、平台、官方 GUI 身份或 feature 子集。

每个声明 `[ui]` 的 extension 最多公开一个 Portable UI，逻辑身份隐式为 `main`，但 `main` 不进入 Manifest、WIT、RPC、
State 或 Snapshot。固定入口 route 是 `/extensions/:extensionId/ui`。v1 没有页面列表、identity 参数、guest discovery 或为第二个
页面预留的兼容槽位；若未来出现真实的第二页面需求，必须通过新的 Portable UI protocol capability/version 设计。

## Daemon authority

Guest 每次只返回完整 `UiDocument`。daemon 验证成功后包装：

```text
UiSnapshot { sessionId, snapshotRevision, document }
```

revision 从 1 开始，仅由 daemon 单调递增。Guest 不决定 session、client Principal、revision、grant revision、lifecycle
generation、package digest 或 authority scope。v1 没有 patch/diff、client reducer、expression、durable UI session 或 guest
revision。

WIT `guest-session-id` 是daemon生成的opaque correlation token，只允许guest区分自己的多个UI session；它不产生Host
Capability authority，不能选择Client Principal、不能替代InvocationContextId，也不能跨Host lifecycle generation重用。
RPC/session的sequence、requestId、revision与replay完全由daemon验证，只有验证通过的新action才调用一次guest dispatch。

## Invocation kinds 与 render purity

M7 只允许以下四种显式 invocation kind，不建立通用 policy engine：

| invocation kind | guest export / lifecycle | 允许的 Host capability |
| --- | --- | --- |
| `background` | 既有 M6 background 行为 | 既有 M6 grant/scope 所允许的能力 |
| `interactive-ui-render` | `activate(interactive-ui)`、`guest-ui.open`、`guest-ui.refresh` | `host-projects.get-summary`、`host-data.get` |
| `interactive-ui-action` | `guest-ui.dispatch` | `host-projects.get-summary`、`host-data.get`、`host-data.set`、`host-data.delete` |
| `interactive-ui-close` | `guest-ui.close`、`deactivate(interactive-ui-idle)` | 无 |

render invocation 必须保持纯读取。若 guest 在 render 中请求 `host-data.set` 或 `host-data.delete`，daemon 使用既有稳定
`extension_permission_denied` 拒绝该 capability call；这不是 Host protocol violation、guest crash 或 quarantine evidence。
guest 可以处理该拒绝并继续返回有效 document。项目读取同时要求 Client Principal 与 Extension Principal 对同一 ProjectId
均具备 `projects.read`；extension-owned host-data 同时要求 Client Principal 的 scoped `extensions.ui.use` 与 Extension Principal
只能访问自身 ExtensionId namespace。client metadata、guest-session-id 与 InvocationContextId 都不替代这组双重授权。

## Flat tree 与 exact parent/child matrix

所有 node/field/action/option identity token 必须匹配 `^[a-z][a-z0-9._-]{0,63}$`。对 form 而言，form node 的 `nodeId`
就是其 `formId`；不引入第二个 form identity。空白、control、Unicode、slash、colon 与 display text 均不得作为 ID。

每个 node 是 `{nodeId,parentId?,order,kind,payload}`。必须恰好一个没有 parent 的 `page`，且它是首个 node；其他 node
必须引用此前出现的 parent。nodeId、fieldId、actionId、optionId 分别在 Snapshot 内唯一。每组 siblings 的 order 必须从
0 连续递增且不重复。tree depth 把 page 计为 1，最大 8；cycle、unreachable node、missing/later parent 或第二个 root
均无效。

合法直接 child 冻结如下；未列出的组合全部拒绝：

| parent | direct children |
| --- | --- |
| page | section, stack, group, form, list, text, status, key-value, progress, divider, button |
| section | section, stack, group, form, list, text, status, key-value, progress, divider, button, switch, text-field, integer-field, select |
| stack | section, stack, group, form, list, text, status, key-value, progress, divider, button, switch, text-field, integer-field, select |
| group | section, stack, group, form, list, text, status, key-value, progress, divider, button, switch, text-field, integer-field, select |
| form | section, stack, group, text, status, key-value, progress, divider, switch, text-field, integer-field, select |
| list | list-item |
| list-item | section, stack, group, form, list, text, status, key-value, progress, divider, button |

`page` 不能作为 child；`list-item` 只能直接属于 `list`；form 不可嵌套。每个 switch/text-field/integer-field/select 必须
拥有且只拥有一个 form ancestor。text/status/key-value/progress/divider/button/switch/text-field/integer-field/select 是 leaf，
不得拥有 children。button 只声明无任意 payload 的 `activate`；form payload 产生的 submit control 只绑定所在 form。

固定 node 集合：

- layout：page、section、stack、group、form、list、list-item；
- display：text、status、key-value、progress、divider；
- input：button、switch、text-field、integer-field、select。

tone 只有 neutral/info/success/warning/danger。progress 只有 indeterminate 或 0-10000 basis points。integer 使用
JSON-safe signed integer。禁止 HTML、CSS、JavaScript、Markdown HTML、Canvas、image/icon URL、external link、URI、font、
color、animation、absolute positioning、custom ARIA/DOM/component、script/expression、iframe/WebView 和 GUI navigation
contribution。

## Forms、validation 与 actions

v1 动作只有：

- `activate { actionId }`：只对应 enabled button，不携带任意 payload；
- `submit-form { actionId, values[] }`：只对应 enabled form 的 `submitActionId`。

switch、select、text-field、integer-field 没有 immediate change action。GUI 在内存维护 draft，绝不按 keystroke 调 RPC。
submit 必须按 document node order 提交该 form 的完整 editable field set，恰好一次且无额外 field。editable 定义为
`disabled=false && readOnly=false`。disabled/read-only field 不出现在 wire values；daemon 从当前 Snapshot 取得其值，客户端
若提交它们则 action invalid。daemon 对当前 Snapshot 验证 form/action 归属、field 唯一性与完整性、类型、required、text
length、integer min/max、select option 和 encoded request quota后才调用 guest。

每个 field 可携带 `validation`：`valid`，或 `invalid` 加最多 512 UTF-8 bytes 的 plain-text message；该对象属于 field
payload，因此只绑定该 field。Renderer 对 invalid field设置原生 invalid state，并以 host-generated message element ID建立
`aria-describedby`；extension 不提供 ARIA ID。validation text 只是 extension content，不能替代 permission、Plan/Apply、
credential 或其他 host-owned security confirmation。

## Refresh 与本地 draft

v1 没有 server-push UI event。refresh 是 client 主动请求的 read-like session action。Renderer 只可由用户主动刷新，或在
页面可见且没有 dirty form 时有界轮询；存在 dirty form 时不得自动 refresh 或静默合并。

Form draft 严格绑定 UiSessionId、SnapshotRevision 与 Form nodeId，只存在 GUI process memory，不写 localStorage、state.db、
extension data、Event 或 log。新 revision 不得自动套用旧 draft。用户刷新或导航时若 draft dirty，必须先显示 host-owned
discard confirmation。disconnect、stale 或 close 使 draft 失效；不得按 field name/type进行 heuristic merge。所有支持 v1
的 GUI consumer 都必须遵守这些 renderer conformance 规则。

locale 在 `extensions.ui.open` 时规范化并固定到 session；refresh/dispatch 不接受 locale 变化。切换 locale 时，renderer 必须
先处理 dirty-form discard confirmation，再关闭旧 session 并用新 locale 打开新 session。theme、density、platform 与官方
GUI identity 不发送给 guest；appearance 始终由 renderer 控制。

## Sequence、exact replay 与断线

session 的首个 accepted sequence 是 1，之后严格 `+1`。每个 requestId 是 1-64 printable ASCII bytes。daemon 为每个
session 在内存保存最近 64 个 accepted dispatch evidence：sequence、requestId、expectedSnapshotRevision、canonical action
fingerprint 与 resulting Snapshot。

action fingerprint 固定为 SHA-256：输入依次为 ASCII domain separator `ALCOMD-PORTABLE-UI-ACTION-V1`、一个 NUL byte，
以及 action 的 schema-order canonical compact UTF-8 JSON（无 insignificant whitespace；object member按 action schema顺序，
form values按 document node order）。该 fingerprint 只做 replay equality，不是授权或持久 identity。

- 新请求必须满足 `sequence == lastAcceptedSequence + 1` 且 requestId 未使用；guest 只调用一次，验证新 document，分配新
  revision，缓存 resulting Snapshot，返回 `replayed=false`。
- sequence、requestId、expected revision 与 action fingerprint 全部匹配已缓存项，并且该项 resulting Snapshot revision
  仍等于 session 当前 revision 时，不调用 guest，返回该项 resulting Snapshot，返回 `replayed=true`。
- exact evidence 匹配但该项 resulting Snapshot 已因后续 refresh 或 dispatch 不再是当前 revision 时，稳定返回
  `extension_ui_snapshot_stale`，不调用 guest，也不返回历史 Snapshot。
- 相同 requestId 或 sequence但 revision/fingerprint 不同，稳定返回 `extension_ui_action_invalid`，不调用 guest。
- sequence gap、out-of-order 或已淘汰 evidence 的重放稳定返回 `extension_ui_action_invalid`，不调用 guest。

expectedSnapshotRevision 对新请求必须等于当前 revision，否则 `extension_ui_snapshot_stale`。一次 session 只允许一个
action；并发第二个 action返回 `extension_ui_action_invalid`。evidence 只在 daemon memory；client connection断开后 session
失效，不承诺跨连接或 daemon restart replay。需要 durable exactly-once 的业务操作继续使用既有 idempotency/Plan/Operation。

open 成功分配 revision 1。每次实际调用 guest 且 document 验证成功的 refresh/dispatch 都以 checked `u64` 严格增加一次；
溢出返回安全 `internal_error`，不得 wrap。每个 session 对 refresh、dispatch 和 close coordination 只允许一个 in-flight，
由窄的 `UiSessionCoordinator` 串行化；closing/closed 阻止新调用，已有调用遵循既有 cancellation/deadline，且不得乱序提交
Snapshot。Renderer 只接受高于当前 revision 的新结果，或 revision 等于当前值的 exact replay；绝不接受更低 revision。

## Session 与 interactive-ui lifecycle

memory-only session 绑定 UiSessionId、Client PrincipalId、client connection/instance、ExtensionId、Extension PrincipalId、
Extension InstanceId、PackageDigest、GrantRevision、LifecycleGeneration、SnapshotRevision、locale、deadline、next sequence 与
64-entry replay evidence。它不写 state.db、Event、extension data、package 或 log。

`extensions.ui.open` 不隐式 enable。desired state 不是 enabled 返回 `extension_not_enabled`；quarantined 返回
`extension_quarantined`；`ui_protocol=NULL` 返回 `extension_ui_not_available`。open 顺序固定为：检查 Client Principal；检查
package/Manifest/digest；检查 grant/lifecycle/quarantine；创建 interactive-ui lease；按需启动 Host；若本次 Host activation 尚未
发生则调用 `guest-lifecycle.activate(interactive-ui)`；最后调用 `guest-ui.open`。多个 UI session 共用一个 activated Host；
background Host 不重复 activate。

最后一个 UI session 关闭且没有 background lease时，daemon 调用 `deactivate(interactive-ui-idle)`，并在 5,000 ms deadline
内停止 Host；timeout 进入既有 bounded termination/crash分类。关闭窗口或 route不改变 desired state。Manifest required
permissions 不自动加入 `background.run`：UI-only extension 无该权限仍可 install/enable/open；扩展若自行将其列为 required，
未授权时 enable失败；列为 optional但未授权时 UI可用，最后一个 interactive session关闭后 Host停止。Portable UI 不产生
隐式 background lease。

client disconnect、daemon restart、disable、grant revoke、uninstall、package digest/generation change、Host crash、quarantine、
timeout 或 protocol violation立即关闭 session。refresh/dispatch 对非当前 connection的 ID返回
`extension_ui_session_not_found`，对已知但失效的 ID返回 `extension_ui_session_stale`。close 返回 `{closed:boolean}`；它不泄露
session owner，且只 best-effort通知 guest。

close 的权威顺序固定为：daemon 先将 session 标为 `closing`/`closed`，使新的 refresh/dispatch 失败，再在
`interactive-ui-close` context 中 best-effort 调用 `guest-ui.close`。该 context 没有 Host capability；guest 不能恢复 session、
写 host-data、修改 desired state 或延长 lease。close trap 不重新打开 session；如果 Host 因其他 UI session 或 background
lease 继续存在，该 trap 仍按真实 guest crash 进入既有 crash/quarantine policy。

## Guest/client failure responsibility

Guest 返回 malformed document、duplicate ID、unknown node、cycle、oversize、forbidden Unicode、invalid parent/child 或
自相矛盾 action/field declaration，属于 guest protocol violation：关闭 session、终止 Host、记录 bounded safe reason并进入
既有 crash/restart/quarantine evidence。结构/Unicode错误返回 `extension_ui_document_invalid`，quota错误返回
`extension_ui_limit_exceeded`；malformed document绝不交给 renderer。Event 只可包含 ExtensionId、InstanceId、failure class、
diagnosticId 与时间，不得包含 document、field value、path、Host frame、argv 或 guest trap。

Client malformed envelope使用既有 `invalid_request`；stale revision使用 `extension_ui_snapshot_stale`；unknown/wrong/extra/missing
field/action、invalid option、replay conflict、gap/out-of-order使用 `extension_ui_action_invalid`；request quota使用
`extension_ui_limit_exceeded`。这些都在 guest前拒绝，不算 Host crash且不触发 quarantine。滚动 60,000 ms内第三次 action
invalid/limit violation关闭当前 session。Guest正常 trap/fuel/timeout继续映射既有 `extension_crashed`/
`extension_resource_limit`并进入已有 crash/quarantine policy。

UiDocument text、form draft、action values、Snapshot 与 replay evidence 都视为潜在敏感数据。原始 payload 不得进入 Event、
普通日志、Host stderr、crash evidence、Operation、state.db、telemetry 或 public `internal_error`。允许记录的只有 ExtensionId、
安全的 node/action/field ID、稳定 error code、计数与 diagnosticId。replay evidence 只存在 session memory 并在 close 时释放；
本合同不宣称 secure zeroization。

## Host-owned chrome 与 high-impact boundary

Renderer 在 Snapshot 外显示 daemon Extension record中的 extension name、ExtensionId、publisher/trust、version、desired/runtime/
quarantine state与“extension-provided content”身份。UiDocument 不能创建或伪造 permission approval、Plan/Apply confirmation、
credential prompt、OS file picker、daemon/system error dialog、updater/install confirmation、core navigation、native notification
permission或 security trust badge。Extension text 永远不能替代 host-owned confirmation。

所有 exact quota、locale 与 Unicode 规则见 `portable-ui-limits-v1.*`。
