# Portable UI v1 threat model candidate

状态：M7 active security contract；Slice B0/B1 已接入 Core/Host/RPC mitigation，official renderer 与完整 Slice D
hostile/fault conformance 尚未完成。

## Assets and trust boundaries

Extension package/guest/UiDocument/text/action declarations 均不可信。官方与第三方 GUI renderer 也不是业务 authority；
daemon application、Client Principal、Extension Principal/grant/scope/lease、current lifecycle/package identity 和
daemon-issued UI session/invocation context 才构成授权边界。

## Threats and frozen mitigations

- renderer escape/spoof：协议没有 HTML/CSS/JS/DOM/URI/image/font/custom ARIA/navigation/Tauri capability；unknown node
  fail closed；host-owned chrome永远位于 Snapshot 外，并显示daemon record中的name、ExtensionId、publisher/trust、
  version、desired/runtime/quarantine与extension-provided标记。
- confused deputy：open/refresh/dispatch同时检查 client `extensions.ui.use` scope 与 extension current lease；UI 内业务
  Host call取双方等价 scope 交集。project summary要求双方对同一 ProjectId 均有 `projects.read`；extension-owned host-data
  同时要求client scoped UI permission与extension self namespace，且不向client暴露raw data。
- render impurity：`interactive-ui-render`只允许project summary与host-data get；render中的set/delete稳定返回
  `extension_permission_denied`，guest可处理后返回有效document，不计作protocol violation、crash或quarantine。
- context forgery：InvocationContextId只由daemon签发并绑定一个in-flight export。grant/lease/session/deadline/generation
  正常推进返回stable stale/permission/cancel并取消调用；unknown/cross-extension/session/invocation、completed reuse或
  authority修改/伪造才终止Host并计入crash/quarantine。
- stale/replay：session绑定 connection/package/grant/generation/revision；strict sequence与64项
  sequence/requestId/expected-revision/action-fingerprint/resulting-Snapshot evidence在guest前验证。exact replay仅在缓存
  resulting revision仍为current时返回；历史exact replay返回 `extension_ui_snapshot_stale`且不泄露旧Snapshot。
  冲突/gap/out-of-order返回 `extension_ui_action_invalid`。断线使session失效。
- session race：每个session的refresh/dispatch/close由窄 coordinator串行化；close先标记closing/closed并拒绝新调用，再以
  无Host capability context best-effort通知guest。close trap不能复活session，Snapshot不得乱序提交。
- resource exhaustion：exact Snapshot/action/node/text/form/session/concurrency/rate/deadline quota；oversize在 renderer前拒绝。
- Unicode spoof：拒绝 NUL/control/bidi marks；security chrome不使用 extension label作为 authority。
- malicious document：结构错误关闭 session并终止 Host，计入 existing crash/quarantine；bounded Event/diagnostic不含内容。
- data exfiltration：无 network/filesystem/browser surface；argv/path/token/Host protocol/context不进入 RPC/Event/log。
- payload disclosure：UiDocument text、draft、action、Snapshot与replay evidence均可能敏感，不进入Event/log/Host stderr/
  crash evidence/Operation/state/telemetry/public internal error；只保留安全ID、stable code、counter与diagnosticId。
- first-party privilege：M8/M9 synthetic fixtures与第三方使用同一 schema、permission、Host 和 renderer contract，无
  private node、page、command 或 permission。
- dirty-draft confusion：draft只在GUI memory并绑定session/revision/form；dirty时不自动refresh/merge，主动refresh/navigation
  使用host-owned discard confirmation，revision/disconnect/stale/close使draft失效。
- locale/appearance confusion：locale在open时规范化并固定；切换locale必须关闭旧session并重新open。theme、density、platform
  与GUI identity不发送给guest，appearance只由renderer决定。
- validation spoof：extension validation message有512-byte上限、只绑定field并由renderer生成ARIA关联；它不能替代任何
  host-owned permission、Plan/Apply、credential、system error或trust confirmation。

Client invalid action只在 daemon侧拒绝；三次/60秒关闭该 session，不惩罚 Extension Host。Guest invalid document属于
guest protocol violation，立即终止 Host并计入 crash window。这一区分防止恶意 client隔离扩展，也防止恶意 guest把
无效树传给 renderer。
