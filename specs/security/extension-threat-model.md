# Extension Runtime threat model v1

状态：M6 contract-first Stop A candidate；engineering vectors 已冻结，production sandbox 尚未实现。

## Assets 与 trust boundaries

M8 client management uses bounded Core operations
(`../extensions/mcp-management-v1.md`). Test forged or stale leases and UI contexts, missing
permissions on either side, revocation during paging or writes, idempotent replay, lost responses
and credential disclosure. Permission checks, acceptance and replay share one Store transaction.
The first-party guest uses the same interface and sandbox as other extensions.

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
- Core alone canonicalizes public names and derives business permissions/effect from the closed typed Host operations. Signed CLI and MCP declarations are discoverable after enablement. Core revalidates the signed business operations, current extension grants, caller and lease at execution.
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
Observation requires a current grant.

## Install consent and complete permission catalogue

Manifest declarations are disclosure and startup requirements, not a grant ceiling. Explicit owner-controlled grants may include any currently grantable undeclared extension permission. No owner-only permission, missing WIT interface or tool declaration is synthesized. Each Host request still checks the calling Client, Extension, lease, lifecycle and current grant revision. Required tool publication is allowed only with valid signed tool declarations; it does not grant invoking MCP clients business authority.

Author terms are bounded signed plain text, frozen with the Plan and shown as untrusted text. GUI consent order has no separate persisted workflow; Core validates one final consent against the exact Plan fingerprint and caller grant authority. Denial cannot submit Apply. Durable consent is part of the existing idempotent Operation request, not a publisher trust decision. Install registry and initial required grants commit together; crash recovery uses the accepted evidence and must not restore grants revoked after state_committed. Missing, false or changed consent is rejected before Operation acceptance. Existing installations receive no implicit new authority.

## 单扩展源在线准备

清单及下载地址均不可信。Core 要求实际调用主体拥有 extensions.manage；扩展 Host 和来源自报的身份不构成授权。匿名HTTPS策略禁止凭据、Referer、用户信息及降级重定向；合法query保持但不记入错误链或日志。索引实际1MiB/256版本/唯一ID、包实际64MiB、已有代理/全局下载并发与GUI独立2连接池限制资源；清单只是来源证据，不是授权。签名/内容树、ZIP SHA及扩展ID/版本/发布者同时匹配；版本变动在下载前以revision_conflict拒绝，Apply再次验证固定包。来源证据受Plan不可变触发器保护并在重启后保留。取消准备仅丢弃GUI结果，不制造回滚，关闭动画期间也不得自动Apply。测试TLS根限test-kill-gates构造器，不提供生产命令行或环境变量旁路。

## Read-only transport observation bridge

The independent alcomd:mcp-transport/observations@1.0.0 interface exposes only
Core's bounded adapterReported projection. It cannot report, bind identities,
issue tickets, change clients or obtain credentials. Both interactive Client and
Extension permissions are checked in Core on every render/action against the
current ownership, active invocation, deadline, lease, lifecycle and grant revision.
Each unreadable group is omitted; none allowed returns permission_denied. Revocation
invalidates old UI authority and does not turn stale data into successful empty data.
The new subscription-stream read is Global/* and may be required or optional, with
existing explicit consent. An explicit grant is required.
MCP business connections cannot read administrative projections.

## M9 local IPC and session projection

Only the background integration world can link the imports. Core checks the active background invocation and lease before reading Unity sessions or selecting a local IPC target. The Unity capability returns a selected registered project ID, start time and count. The invoking guest receives no path, argv or Unity process handle.

On Windows Core builds local pipe names from the signed target's literal basename set and opens them with identification-only quality of service. It checks the connected server PID, current user SID, login session, executable basename and trusted Authenticode signer organization before sending any byte. Trust verification uses the system verifier with cached retrieval. Guest input supplies a signed target ID or an opaque channel ID, never an OS address. Each instance gets at most one channel. Actual read/write size is capped at 64 KiB, and a write that times out closes the channel to prevent reusing a stream with an unknown prefix. Host messages and diagnostic errors exclude endpoint paths and payloads.

On Unix the address is a non-symlink filesystem socket in the selected Core runtime directory. The kernel provides peer UID and PID. macOS validates the running process against the signed bundle identifier and Team ID under Apple's signing anchor. Linux requires the peer's loaded executable inode to match the declared canonical system path, with every ancestor root-owned and without group or other write access. Per-user and sandbox-container installations outside the declared Linux system path are unsupported. All three platforms fail closed on missing or mismatched identity evidence.

An in-flight IPC operation holds its verified access until it finishes or times out. The Store orders durable revoke, disable and uninstall mutations after that operation, even if the mutation request caller disconnects. A revoked lease cannot use an old handle, and process disposal closes the channel. The single-action activation flow remains follow-up work.
