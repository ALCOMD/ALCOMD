# Portable UI v1 threat model candidate

状态：M7 Stop A proposal-only；补充 `extension-threat-model.md`，不表示 production mitigation 已实现。

## Assets and trust boundaries

Extension package/guest/UiDocument/text/action declarations 均不可信。官方与第三方 GUI renderer 也不是业务 authority；
daemon application、Client Principal、Extension Principal/grant/scope/lease、current lifecycle/package identity 和
daemon-issued UI session/invocation context 才构成授权边界。

## Threats and frozen mitigations

- renderer escape/spoof：协议没有 HTML/CSS/JS/DOM/URI/image/font/custom ARIA/navigation/Tauri capability；unknown node
  fail closed；host-owned chrome永远位于 Snapshot 外。
- confused deputy：open/refresh/dispatch同时检查 client `extensions.ui.use` scope 与 extension current lease；UI 内业务
  Host call取双方等价 scope 交集，extension-owned host-data 例外不向 client 暴露 raw data。
- context forgery：InvocationContextId 只由 daemon签发并绑定一个 in-flight export；unknown/wrong context终止 Host。
- stale/replay：session绑定 connection/package/grant/generation/revision；strict sequence、64 remembered request IDs 与
  stale Snapshot check 在 guest 前执行。断线不自动重发。
- resource exhaustion：exact Snapshot/action/node/text/form/session/concurrency/rate/deadline quota；oversize在 renderer前拒绝。
- Unicode spoof：拒绝 NUL/control/bidi marks；security chrome不使用 extension label作为 authority。
- malicious document：结构错误关闭 session并终止 Host，计入 existing crash/quarantine；bounded Event/diagnostic不含内容。
- data exfiltration：无 network/filesystem/browser surface；argv/path/token/Host protocol/context不进入 RPC/Event/log。
- first-party privilege：M8/M9 synthetic fixtures与第三方使用同一 schema、permission、Host 和 renderer contract，无
  private node、page、command 或 permission。

Client invalid action只在 daemon侧拒绝；三次/60秒关闭该 session，不惩罚 Extension Host。Guest invalid document属于
guest protocol violation，立即终止 Host并计入 crash window。这一区分防止恶意 client隔离扩展，也防止恶意 guest把
无效树传给 renderer。

