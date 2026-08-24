# Extension Host internal protocol logical contract v1

状态：M6 contract-first Stop A candidate；internal only，不是 public RPC，production 尚未实现。

## Transport 与 framing

daemon 为每个 enabled ExtensionId spawn 一个 Host，并建立 dedicated piped stdin/stdout。没有 listening Named Pipe、
UDS、TCP、daemon RPC forwarding 或 extension-visible endpoint；guest 不继承 Host stdio。Rust `Command`/piped stdio
足够表达当前 contract，不要求新 platform API。

daemon 只启动已知可信 sibling Host executable，不经 shell；args 固定并验证。child 使用 `env_clear()`，只有实际证明
必需的最小环境才可逐项加入，默认为空，不能继承 token、credential、proxy 或任意用户/daemon secret。stdout 专用于
binary Host protocol，普通日志不得污染 framing。stderr 是 bounded/redacted diagnostic channel；guest 无 WASI stdio，
不能无限写入。

frame 是 `u32 little-endian payload length + UTF-8 JSON`，payload 1-524,288 bytes。零长、oversize、truncated、
invalid UTF-8/JSON 或 sequence violation 立即终止该 Host；不把 malformed Host protocol 转为 public RPC response。

## Bootstrap/channel binding

首帧只能由 daemon 发送 `bootstrap`：protocolVersion=1、daemon epoch、一次性 256-bit random nonce、
ExtensionInstanceLease 的 opaque leaseId/InstanceId/lifecycle generation/API world 和 exact limits。Host 必须以
`ready` echo nonce 与 monotonically increasing
sequence=1；不匹配立即终止。

nonce、pipe handle 与 lease 只存在当前 daemon/Host memory；不写 package、extension data、SQLite、log、Event 或
diagnostic。channel 是 daemon-spawned child binding，不接受外部 reconnect，也不是长期 bearer credential。

## Message catalog

daemon -> Host：`bootstrap`、`invoke-export`、`cancel-call`、`capability-result`、`revoke-lease`、`shutdown`。

Host -> daemon：`ready`、`capability-call`、`capability-cancelled`、`export-result`、`host-fault`。

每条消息有 protocolVersion=1、daemon epoch、InstanceId、lifecycle generation 与 strict sequence。`invoke-export`/
`export-result` 使用 `requestId` 标识一次 daemon 发起的 guest export；`capability-call`/`capability-result`/
`capability-cancelled` 只使用 `callId` 标识一次 Host 发起的 capability invocation。任何一类消息不得同时携带两个 ID，
两种 ID 不得互换或用于匹配另一类调用。oversize、duplicate/replay ID、wrong instance/generation、stale daemon epoch、malformed message
或 unsolicited authority metadata 均终止 channel。`capability-call` 可以携带
leaseId 与 capability input，但不得携带或覆盖 PrincipalId、ExtensionId、publisher、first-party status、permission
或 scope。daemon 以 pipe session 找到 current lease，再从 application authority 解析这些身份。

M6 capability catalog 只有 `host-projects.get-summary` 与 self `host-data.get/set/delete`。unknown capability fail closed；
network/filesystem/clipboard/notification/Discord 不在 catalog。

### `capability-result`

`capability-result` 是 daemon 对一个 pending `capability-call.callId` 的唯一响应，用于完成已经冻结的同步 WIT
`host-projects.get-summary` 与 `host-data.get/set/delete`。envelope 必须绑定 protocolVersion、daemon epoch、InstanceId、
lifecycle generation、strict sequence 和原 `callId`，并且必须恰好包含 bounded `result` 或 stable `error` 之一。
`result` 与 `error` 互斥；序列化前的 capability-specific value 与序列化后的完整 frame 都必须分别验证上限。

`capability-result` 不得携带 PrincipalId、ExtensionId override、publisher override、first-party status、permission、
resource scope、grant revision、lifecycle authority 或新 lease。authority 只来自 daemon-created current lease 与当前
application/state authority。error 只允许当前 capability 已冻结的 stable machine code 与 `internal_error` 的 optional
diagnosticId；不得包含 Wasmtime trap、Rust/SQLite Debug、filesystem path 或 backtrace。

duplicate response、unknown/already-completed `callId`、wrong InstanceId/generation、stale daemon epoch、invalid sequence、
oversized frame 或 malformed result/error envelope 都是 protocol violation：立即关闭该 Host channel/instance，并进入
既有 crash/protocol-failure 路径，不忽略后继续。Host frame 仍不超过 512 KiB，WIT input/output 各不超过 256 KiB；
Project summary、data key/value 的更窄既有上限优先。

## Revocation/cancellation

grant revision durable commit 后 daemon 先使 lease invalid，再发送 best-effort `revoke-lease`；正确性不依赖 Host
收到消息。所有新/queued capability call 在 daemon authority check 失败，pending call 被取消。Host 不响应时在
2,000 ms stop deadline 后 terminate；这不回滚已经取得 OperationId 的 core Operation。

## Redaction

protocol payload、fault、log 和 Event 不含完整私密 path、argv/env、token、signature private material、extension-owned
value、trap backtrace 或 raw Wasmtime error。public error 只映射 stable code 与 optional diagnosticId。
