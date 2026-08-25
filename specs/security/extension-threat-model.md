# Extension Runtime threat model v1

状态：M6 contract-first Stop A candidate；engineering vectors 已冻结，production sandbox 尚未实现。

## Assets 与 trust boundaries

- authority：ExtensionId/Principal/grant/scope/revision、official publisher policy、Operation/Plan authority；
- confidentiality：project/private path、extension-owned data、credential、argv/environment、Host internal channel；
- integrity：`.alcomdext` content、Manifest/WIT/component、state.db、Project files、registry、journal；
- availability：daemon unique writer、其他 Extension Host、bounded CPU/memory/storage/call capacity。

Boundaries：untrusted ZIP/publisher、untrusted Component、isolated UI origin、per-ExtensionId Host OS process、private
Host pipe、daemon/application authority、SQLite/object store。第一方 package 在进入 runtime 后不获得更高 trust boundary。

## Threats 与 mandatory mitigations

| threat | mitigation | negative evidence |
|---|---|---|
| traversal/absolute/device/UNC/link/collision/special ZIP entry | Extension-specific preflight quota、existing hostile path stack、staging、no convenience extract | hostile package vectors |
| package replacement / Manifest says first-party | canonical content SHA-256、strict Ed25519、fingerprint/trust/source layering、Apply revalidation | digest/signature/trust vectors |
| unknown publisher silently trusted | explicit high-impact per-extension approval in immutable Plan; uninstall clears approval | confirmation vectors |
| guest imports ambient OS authority | no inherit env/argv/stdio/network/preopen; exact WIT linker allowlist; no WASI imports in first slice | import-set golden |
| guest forges Principal/Extension/scope | daemon-created lease bound to spawned pipe session; guest authority metadata ignored | stale/forged lease vectors |
| revoke race / cached authorization | durable grant revision commit linearizes; every call rechecks; queue/session/handle cancellation | revoke-in-flight matrix |
| guest reads project files/DB | only extension-safe Project summary application use case | scope/capability vectors |
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
