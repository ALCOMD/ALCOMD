# ALCOMD Portable UI v1 contract candidate

状态：M7 contract-first Stop A；proposal-only，尚未被 daemon、Host、RPC client 或 renderer 实现/广告。

## 协议与 surface

Portable UI 是 Core/RPC contract，不是 Tauri API。客户端在 `system.hello.params.capabilities` 请求
`extensions.ui.portable.v1`；daemon 只有完整实现 v1 后才回显。声明该 capability 的 consumer 必须完整支持 v1，
unknown node/action fail closed。v1 不协商 hostId、framework、renderer、平台、官方 GUI 身份或 feature 子集。

每个声明 `[ui]` 的 extension 只有一个隐式 `main` surface。M8 MCP management 和 M9 Discord Presence fixtures 都可
在单一 page 中表达，因此不增加 list-surfaces、surface array 或 guest dynamic discovery。

## Daemon authority

Guest 每次只返回完整 `UiDocument`。daemon 验证成功后包装：

```text
UiSnapshot { sessionId, snapshotRevision, document }
```

revision 从 1 开始，仅由 daemon 单调递增。Guest 不决定 session、client Principal、revision、grant revision、
lifecycle generation、package digest 或 authority scope。v1 没有 patch/diff、client reducer、expression、durable
UI session 或 guest revision。

## Flat tree

每个 node 是 `{nodeId,parentId?,order,kind,payload}`。必须恰好一个没有 parent 的 `page`，且它是首个 node；其他
node 必须引用此前出现的 parent。nodeId、fieldId、actionId 分别在 Snapshot 内唯一。每组 siblings 的 order 必须从
0 连续递增且不重复。tree depth 把 page 计为 1，最大 8；cycle、missing/later parent、第二个 root 均无效。

只有 page/section/stack/group/form/list/list-item 可含 child；leaf 不可含 child。list 的直接 child 只能是 list-item，
list-item 不得嵌套 list-item。form 不得嵌套；每个 switch/text-field/integer-field/select 必须有且只有一个最近 form
ancestor。disabled field 仍出现在 submit values 中并必须等于 Snapshot initial value。

固定 node 集合：

- layout：page、section、stack、group、form、list、list-item；
- display：text、status、key-value、progress、divider；
- input：button、switch、text-field、integer-field、select。

tone 只有 neutral/info/success/warning/danger。progress 只有 indeterminate 或 0-10000 basis points。integer 使用
JSON-safe signed integer。禁止 HTML、CSS、JavaScript、Markdown HTML、Canvas、image/icon URL、external link、URI、
font、color、animation、absolute positioning、custom ARIA/DOM/component、script/expression、iframe/WebView 和 GUI
navigation contribution。

## Forms 与 actions

v1 动作只有：

- `activate { actionId }`：只对应 enabled button；
- `submit-form { actionId, values[] }`：只对应 enabled form 的 submitActionId。

switch、select、text-field、integer-field 仅是 form field，没有 immediate change action。GUI 在内存维护 draft，绝不按
keystroke 调 RPC。submit 必须按 document node order 提交该 form 的每个 field 恰好一次；类型、required、length、
integer min/max、select option、disabled initial value 和完整性均由 daemon 对当前 Snapshot 验证。GUI 不能发送任意
JSON。button 不在 form 中承担 submit；form payload 明确给出 host renderer 的 submit button。

## Sequence、replay 与断线

session 的首个 sequence 是 1，之后严格 `+1`。每个 requestId 是 1-64 printable ASCII bytes 且 session 内唯一。
daemon 记住最近 64 个已完成 pair 与 result revision，不缓存 64 份 document：同一 `(sequence,requestId)` 的
live-session duplicate 不再调用 guest，而返回当前 Snapshot 并标记 `replayed=true`。同 sequence/different ID、
同 ID/different sequence、future
sequence 或已淘汰 sequence 返回 `extension_ui_action_invalid`，绝不调用 guest。

expectedSnapshotRevision 必须等于当前 revision，否则 `extension_ui_snapshot_stale`。一次 session 只允许一个 action；
并发第二个 action返回 `extension_ui_action_invalid`。v1 不承诺 durable exactly-once。连接在 response 前断开会关闭
session；客户端不得自动重发，必须重新 open 并从新 Snapshot 观察结果。Core durable write 继续使用自己的
Plan/Operation/idempotency。

## Session lifecycle

daemon memory session 至少绑定 UiSessionId、Client PrincipalId、client connection/instance、ExtensionId、Extension
PrincipalId、Extension InstanceId、PackageDigest、`main`、GrantRevision、LifecycleGeneration、SnapshotRevision、locale、
created/last-activity/idle/absolute deadline、next sequence、64-entry request cache。它不写 state.db、Event payload、
extension data、package 或 log。

client disconnect、daemon restart、disable、grant revoke、uninstall、package digest change、lifecycle generation change、
Host crash、quarantine、timeout 或 protocol violation立即关闭 session。旧 ID/sequence/requestId 不可用于新 session。
refresh/dispatch 对从未由当前 connection 获得的 ID返回 `extension_ui_session_not_found`，对已知但失效的 ID返回
`extension_ui_session_stale`。close 返回 `{closed:boolean}`：live session 被关闭时 true，已经 absent/stale/unknown 时 false；它不抛 session-not-found，
也不泄露 session owner。close 不调用 guest action，只 best-effort 通知 guest。

## Invalid input 与 guest failure

client 的 stale/malformed/unknown/disabled/action constraint failure在调用 guest 前返回稳定错误。每个 session 在滚动
60,000 ms 内第三次 `extension_ui_action_invalid` 或 request limit violation时立即关闭该 session；不终止 Host，也不计
入 crash window。

Guest 在 open/refresh/dispatch 返回 malformed tree、duplicate/cycle/unknown node、oversize、invalid Unicode 或非法
action declaration时：当前 session 立即关闭，Host instance 终止，记录现有 crash/quarantine window 的 safe reason
`ui-protocol-violation`。结构错误返回 `extension_ui_document_invalid`，quota 错误返回
`extension_ui_limit_exceeded`。Event 只可包含 ExtensionId、InstanceId、surfaceId、failure class、diagnosticId 与时间；
不得包含 document、field value、path、Host frame 或 guest trap。Host timeout/trap 继续映射现有
`extension_resource_limit`/`extension_crashed`。malformed document 永不交给 renderer。

## Host-owned chrome 与 high-impact boundary

Renderer 在 Snapshot 外显示 extension name、ExtensionId、publisher/trust summary、enabled/runtime/quarantine state和
“extension-provided content”。Extension text 不能创建/伪造 permission grant、Plan/Apply、credential、daemon error、
file picker、notification permission、core navigation 或 updater/install confirmation。未来 typed high-impact interaction
必须由 daemon authority 与 host-owned GUI 渲染；v1 不预建 HostInteraction framework。

所有 exact quota、locale 与 Unicode 规则见 `portable-ui-limits-v1.*`。
