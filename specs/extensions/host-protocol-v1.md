# Extension Host internal protocol logical contract v1

状态：M6 contract-first Stop A candidate；internal only，不是 public RPC，production 尚未实现。

## Transport 与 framing

daemon 为每个 enabled ExtensionId spawn 一个 Host，并建立 dedicated piped stdin/stdout。没有 listening Named Pipe、
UDS、TCP、daemon RPC forwarding 或 extension-visible endpoint；guest 不继承 Host stdio。Rust `Command`/piped stdio
足够表达当前 contract，不要求新 platform API。

frame 是 `u32 little-endian payload length + UTF-8 JSON`，payload 1-524,288 bytes。零长、oversize、truncated、
invalid UTF-8/JSON 或 sequence violation 立即终止该 Host；不把 malformed Host protocol 转为 public RPC response。

## Bootstrap/channel binding

首帧只能由 daemon 发送 `bootstrap`：protocolVersion=1、一次性 256-bit random nonce、ExtensionInstanceLease 的
opaque leaseId/InstanceId/API world 和 exact limits。Host 必须以 `ready` echo nonce 与 monotonically increasing
sequence=1；不匹配立即终止。

nonce、pipe handle 与 lease 只存在当前 daemon/Host memory；不写 package、extension data、SQLite、log、Event 或
diagnostic。channel 是 daemon-spawned child binding，不接受外部 reconnect，也不是长期 bearer credential。

## Message catalog

daemon -> Host：`bootstrap`、`invoke-export`、`cancel-call`、`revoke-lease`、`shutdown`。

Host -> daemon：`ready`、`capability-call`、`capability-cancelled`、`export-result`、`host-fault`。

每条消息有 protocolVersion=1、strict sequence、requestId/callId 和 bounded payload。`capability-call` 可以携带
leaseId 与 capability input，但不得携带或覆盖 PrincipalId、ExtensionId、publisher、first-party status、permission
或 scope。daemon 以 pipe session 找到 current lease，再从 application authority 解析这些身份。

M6 capability catalog 只有 `host-projects.get-summary` 与 self `host-data.get/set/delete`。unknown capability fail closed；
network/filesystem/clipboard/notification/Discord 不在 catalog。

## Revocation/cancellation

grant revision durable commit 后 daemon 先使 lease invalid，再发送 best-effort `revoke-lease`；正确性不依赖 Host
收到消息。所有新/queued capability call 在 daemon authority check 失败，pending call 被取消。Host 不响应时在
2,000 ms stop deadline 后 terminate；这不回滚已经取得 OperationId 的 core Operation。

## Redaction

protocol payload、fault、log 和 Event 不含完整私密 path、argv/env、token、signature private material、extension-owned
value、trap backtrace 或 raw Wasmtime error。public error 只映射 stable code 与 optional diagnosticId。
