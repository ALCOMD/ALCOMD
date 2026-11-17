# Extension Runtime threat model v1

状态：M6 contract-first Stop A candidate；engineering vectors 已冻结，production sandbox 尚未实现。

## Assets 与 trust boundaries

M8 approved management bridge adds only bounded MCP client administration through Core use cases
(`../extensions/mcp-management-v1.md`). It adds no ambient WASI authority. Mandatory negative vectors:
forged/stale lease and UI context; one-sided permissions; project capability escalation; grant revocation
and package changes during paging or write acceptance; oversized grants; idempotency replay after
revocation; accepted-write response loss; accidental credential/log disclosure. Grant checks and
acceptance/replay share one Store transaction. First-party guests use the same interface and sandbox.

- authority：ExtensionId/Principal/grant/scope/revision、受控安装来源、Operation/Plan authority；
- confidentiality：project/private path、extension-owned data、credential、argv/environment、Host internal channel；
- integrity：`.alcomdextension` content、Manifest/WIT/component、state.db、Project files、registry、journal；
- availability：daemon unique writer、其他 Extension Host、bounded CPU/memory/storage/call capacity。

Boundaries：untrusted ZIP/publisher、untrusted Component、isolated UI origin、per-ExtensionId Host OS process、private
Host pipe、daemon/application authority、SQLite/object store。第一方 package 在进入 runtime 后不获得更高 trust boundary。

## Threats 与 mandatory mitigations

| threat | mitigation | negative evidence |
|---|---|---|
| traversal/absolute/device/UNC/link/collision/special ZIP entry | Extension-specific preflight quota、existing hostile path stack、staging、no convenience extract | hostile package vectors |
| package replacement / Manifest says first-party | canonical content SHA-256、strict Ed25519、fingerprint/source identity binding、Apply revalidation | digest/signature/source vectors |
| installation or a valid signature is mistaken for publisher certification | owner-selected installation has no publisher approval/trust flag; signatures prove integrity/identity only; install never grants capabilities | valid self-signed install without approval, invalid signature rejection, default-zero grants |
| guest imports ambient OS authority | no inherit env/argv/stdio/network/preopen; exact WIT linker allowlist; no WASI imports in first slice | import-set golden |
| guest forges Principal/Extension/scope | daemon-created lease bound to spawned pipe session; guest authority metadata ignored | stale/forged lease vectors |
| revoke race / cached authorization | durable grant revision commit linearizes; every call rechecks; queue/session/handle cancellation | revoke-in-flight matrix |
| guest reads project files/DB | only extension-safe Project summary application use case | capability and target-identity vectors |
| cross-extension data read | lease-selected namespace, no ExtensionId argument, transaction/quota constraints | cross-read vectors |
| publisher reuses retained ExtensionId data | namespace owner binds ExtensionId + publisher fingerprint; mismatch fails closed; no publisher transfer | owner-mismatch vectors |
| infinite loop/memory/table/call flood | exact fuel+epoch/wall/memory/table/instance/concurrency/rate limits | resource-limit vectors |
| Host crash/hang/OOM affects core/peer | one ExtensionId per Host process, bounded stop, crash-loop quarantine | topology/crash vectors |
| phantom running after restart | desired/runtime split, daemon epoch + live child handle required | lifecycle recovery vectors |
| quarantine erases user intent | desired/quarantine/runtime are independent; crash loop changes quarantine only | lifecycle recovery vectors |
| install/uninstall half commit | immutable Plan, resource lock, staging/backup, append-only journal, forward recovery | phase kill matrix |
| malicious UI origin/replay/flood/private channel | isolated origin/session, sequence/request ID, revoke, exact limits, no Tauri/DOM/Node | headless UI vectors |
| logs leak data | stable safe errors, diagnostic ID, no raw path/argv/trap/token/value | redaction tests |
| first-party hidden API | same package/WIT/permission/Host/quota; repository scan and parity fixture | first/third parity test |

## Revocation boundary

grant revision durable update 是 authority linearization point。之后尚未被 `alcomd-application` 接受的调用失败，
Host queue 取消，session/handle 失效。已经正式接受并获得 OperationId 的高影响 Operation 继续遵循既有 recovery，
但不能进行任何新的 Host Capability call。

## Explicitly absent authority

M6 first slice 没有 network、filesystem、clipboard、notification、Discord、raw socket、shell/process control、
daemon RPC socket、Host listener、OS credential store、M7 product placement 或 native library。测试只用 synthetic/local
fixture，不执行攻击性公网、真实 credential 或凭据传播测试。

## M7 Portable UI proposal（尚未成为 production contract）

旧 iframe/managed-child WebView 设计已在 production 前拒绝，其 evidence 保留于 `specs/gui/m7-stop-a.md`。M7 Stop A
不再加载 extension HTML/CSS/JavaScript、asset、origin 或 Tauri capability。当前完整 proposal threat model 位于
`specs/security/extension-portable-ui-threat-model.md`，覆盖 daemon-owned session/revision、Client/Extension dual
authority、InvocationContext、防 replay、resource/Unicode limits、host-owned chrome、malformed guest fail-closed 与
first/third-party parity。

该 proposal 未接入 active Manifest/WIT/Host/RPC/Permission/State 或 renderer。只有项目所有者批准 Stop A 后才能开始
production replacement；M6 backend runtime 已验收的 sandbox、lease、data 与 crash isolation 保持有效。


## Extension MCP tools (owner-approved 2026-11-16)

- A signed provider catalogue is untrusted data. Host-only JSON Schema validation rejects external/dynamic/circular references and header mappings; a deny retriever prevents network/file resolution. Preflight type-checks the provider ABI without guest execution. Native schema work, compilation and guest calls run in the supervised Host process with existing quotas and deadlines.
- Core alone canonicalizes public names and derives business permissions/effect from the closed typed Host operations. Publication uses `mcp.tools.provide`; it may be required (explicit install consent) or optional (default off). MCP clients retain existing global business capabilities; no project or per-extension client whitelists exist.
- Every Host call reauthenticates current MCP credentials/OAuth and extension grants, package and lease. Writes check again inside acceptance transactions. Guest receives neither client credential nor an owner context. It cannot invent an RPC method, alter the captured Apply/cancel target or confirm for the caller.
- Core-owned effect evidence cannot be forged by guest data. One persistent action is recorded atomically with business acceptance; lost replies, traps and protocol cancellation do not cancel or repeat accepted Operations. Explicit same-key recovery checks current caller/business authority and restores original evidence only; it never invokes a removed or revoked provider.
- Catalogue continuations bind current authentication, client revision, provider packages/grant revisions and position. HTTP resolves Core effect metadata before calling; reserved write controls conservatively preserve unknown-write semantics across source changes. No new listener/session or credential forwarding to Wasm is introduced.
- Native schema dependencies include third-party SIMD/unsafe code. Disabling default resolver features is not a claim of zero unsafe; dependency and feature evidence is recorded in M8.


## M8 Core observation read boundary (approved 2026-11-17)

The additive observations@1.0.0 Host interface exposes only the Core-generated
connection/call projection. The authoritative registry is in Core memory and is
populated after real MCP authentication; clientInfo, adapter text and guest input
cannot select a Principal or create a row. Method names come from the static Core
allowlist. Parameters, responses, credentials/hashes, headers, request IDs, paths
and URLs are excluded by construction, including diagnostic/Portable UI output.

The two optional Global/* read permissions reveal cross-client activity metadata
and must be explicitly granted. Native MCP business credentials and OAuth scopes
do not include these administration permissions. Each Host read validates both
interactive Client and Extension authority plus declaration, scope, current lease,
grant/lifecycle state, UI state and deadline; canRead flags are presentation only.
Grant revocation must invalidate the previous UI result, never preserve it as live.

The4096-connection tracking budget and64KiB projections cannot reject business.
Overflow/truncation is explicit; read errors cannot masquerade as zero activity.
Drop/restart clears ephemeral records and changes the Core instance binding.
A displayed IPC connection is not a current-authorization or online assertion;
HTTP sockets, adapter queues and protocol subscriptions remain unobserved.
Observation cleanup and request cancellation never cancel accepted Operations.
State40 preserves all old rows/constraints and grants nothing automatically.

## State41 install consent and complete permission catalogue

Manifest declarations are disclosure and startup requirements, not a grant ceiling. Explicit owner-controlled grants may include any currently grantable undeclared extension permission. No owner-only permission, missing WIT interface or tool declaration is synthesized. Each Host request still checks the calling Client, Extension, lease, lifecycle and current grant revision. Required tool publication is allowed only with valid signed tool declarations; it does not grant invoking MCP clients business authority.

Author terms are bounded signed plain text, frozen with the Plan and shown as untrusted text. GUI consent order has no separate persisted workflow; Core validates one final consent against the exact Plan fingerprint and caller grant authority. Denial cannot submit Apply. Durable consent is part of the existing idempotent Operation request, not a publisher trust decision. Install registry and initial required grants commit together; crash recovery uses the accepted evidence and must not restore grants revoked after state_committed. Missing, false or changed consent is rejected before Operation acceptance. Existing installations receive no implicit new authority.

## 单扩展源在线准备（M8 / State42）

清单及下载地址均不可信。仅认证local-owner＋extensions.manage可触发，不能由扩展Host、MCP客户端或来源自报身份提升。匿名HTTPS策略禁止凭据、Referer、用户信息及降级重定向；合法query保持但不记入错误链或日志。索引实际1MiB/256版本/唯一ID、包实际64MiB、已有代理/全局下载并发与GUI独立2连接池限制资源；清单只是来源证据，不是授权。签名/内容树、ZIP SHA及扩展ID/版本/发布者同时匹配；版本变动在下载前以revision_conflict拒绝，Apply再次验证固定包。来源证据受Plan不可变触发器保护并在重启后保留；State42按连接临时停用外键重建，提交前完整检查并恢复外键。取消准备仅丢弃GUI结果，不制造回滚，关闭动画期间也不得自动Apply。测试TLS根限test-kill-gates构造器，不提供生产命令行或环境变量旁路。

## Read-only transport observation bridge

The independent alcomd:mcp-transport/observations@1.0.0 interface exposes only
Core's bounded adapterReported projection. It cannot report, bind identities,
issue tickets, change clients or obtain credentials. Both interactive Client and
Extension permissions are checked in Core on every render/action against the
current ownership, active invocation, deadline, lease, lifecycle and grant revision.
Each unreadable group is omitted; none allowed returns permission_denied. Revocation
invalidates old UI authority and does not turn stale data into successful empty data.
The new subscription-stream read is Global/* and may be required or optional, with
existing explicit consent. State43 only extends the CHECK and never grants it.
External MCP business permission grants cannot read administrative projections.
