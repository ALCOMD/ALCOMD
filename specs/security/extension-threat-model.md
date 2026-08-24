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

## M7 Stop A Extension UI proposal（尚未成为 production contract）

M7 只候选 host-owned `/extensions/:extensionId/ui`，不增加 Manifest contribution。候选 container 是 main WebView
内 `sandbox="allow-scripts"` 的 cross-origin iframe；isolated Tauri-managed child WebView 仍保留比较项。最终选择必须由
WebView2、WebKitGTK 与 WKWebView actual in-app harness 证明，静态源码检查或 DOM automation 不构成证明。

| threat | proposed mitigation | required actual negative evidence |
|---|---|---|
| extension 获得 Tauri private IPC | extension frame 不匹配 capability；iframe 不运行 main-frame initialization；无通用 invoke | `__TAURI__` 与 `__TAURI_INTERNALS__` absent，raw invoke/event/channel 在 app command 前失败 |
| frame 读取 host DOM/parent/opener | cross-origin + opaque sandbox origin；无 `allow-same-origin`/popup/top navigation | parent/opener/main DOM read/write 全失败 |
| confused-deputy `postMessage` | exact `contentWindow` + current session/generation + strict Bridge schema/sequence；不以 `event.origin=null` 授权 | sibling/parent/old frame/forged session/replay 全拒绝 |
| asset path/package confusion | verified package digest、random per-open token、normalized `ui/` regular file、exact MIME、no listing/fallback | traversal/link/collision/unknown MIME/cross digest/token 全拒绝 |
| network/exfiltration | CSP `connect-src 'none'`、no forms/popup/worker/top navigation；M7 不增加 `network.request` | fetch/WebSocket/beacon/navigation/download/form blocked |
| direct local authority | 无 daemon socket、filesystem、clipboard、notification、Node 或 Tauri plugin capability | 每项 actual probe unavailable/denied |
| package replacement keeps old authority | digest/lifecycle generation change atomically closes Bridge, token, pending, queue and cache binding | old frame/request/asset URL 均失败，新 package 使用新 session |
| sensitive diagnostic reaches WebView | daemon/application boundary 先产生 safe DTO；M7 不创建 diagnostics read model | token/Authorization/credential/full private path/argv/stack/arbitrary internal string 不出现在 result/console/artifact |

当前 actual evidence 尚未取得，container choice 与 physical mapping 均为 `not yet frozen`。在三平台 probe 完成并经人工
审批前，不得实现 production asset protocol、Bridge transport 或 Tauri capability。
