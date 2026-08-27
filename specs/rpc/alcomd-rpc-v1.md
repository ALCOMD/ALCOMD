# ALCOMD RPC v1

状态：M1-M4 已完成并通过人工验收；M5 CLI/Unity/Template 已实现，Backup Create 合同已冻结但未实现

## 1. M1 合同范围

M1 只冻结并实现：

- 每用户本地 IPC 与单实例所有权。
- 长度前缀帧。
- JSON-RPC-inspired request/response/error envelope。
- 每连接 `system.hello` 握手。
- 只读 `system.status`。

本协议受 JSON-RPC 2.0 的 request ID、method、params、result/error 分离启发，但没有
`jsonrpc: "2.0"` 字段，也不采用 JSON-RPC 2.0 的全部错误码、通知、批处理或传输规则。因此
ALCOMD RPC v1 **不是 JSON-RPC 2.0 兼容实现**。

M2 以兼容增加方式冻结最小 Operation/Event/Revision/幂等/Principal 合同；不改变 M1 framing、
envelope、握手、status 或错误行为。

## 2. 传输与端点

官方客户端使用当前用户范围的本地 IPC，不通过 TCP：

```text
Windows: \\.\pipe\CQMHV.ALCOMD.<current-user-SID>.rpc-v1
Linux:  $XDG_RUNTIME_DIR/alcomd/rpc-v1.sock
macOS:  经过所有权检查的短 per-user runtime/temp 目录中的 rpc-v1.sock
```

Windows Named Pipe 的 DACL 必须明确限制为当前用户，不授予 Everyone 或 Anonymous。SID 只用
于端点隔离，不是应用层 Principal。

Linux 的 `alcomd` runtime 目录为 `0700`，socket 为 `0600`。实现必须使用 `lstat`/等价的
不跟随 symlink 检查验证 runtime directory 与既有 socket 的类型和所有权。若
`XDG_RUNTIME_DIR` 缺失或不安全，只能使用经过相同检查的短 per-user fallback，不能把 socket
直接置于共享可写目录。

macOS 使用满足 `sockaddr_un` 长度限制的短 per-user 路径，父目录为 `0700`、socket 为 `0600`，
并执行所有权和 symlink 检查；不得使用可能过长的 Application Support 路径。

## 3. 帧

每条消息是：

```text
u32 little-endian payload_length
payload_length bytes UTF-8 JSON payload
```

- payload 最大为 `4 MiB`（`4 * 1024 * 1024 = 4_194_304` bytes）。
- 长度是 UTF-8 JSON payload 的字节数，不包含 4-byte 前缀。
- 零长度、超过上限、EOF 导致的前缀/正文截断，或无法取得完整 payload，均为 framing error。
- framing error 立即关闭当前连接；服务器不得猜测 request ID 或伪造 RPC error response。
- 完整 payload 的非 UTF-8、JSON 语法、envelope 或 params 错误是 RPC-level
  `invalid_request`，服务器返回结构化 error 后可继续处理该连接。
- 服务器必须在按声明长度分配 payload 前检查上限。
- M1 不支持 batch、compression、notification 或 server-initiated request。

## 4. Envelope

请求：

```json
{
    "id": "request-1",
    "method": "system.status",
    "params": {}
}
```

成功响应：

```json
{
    "id": "request-1",
    "result": {}
}
```

错误响应：

```json
{
    "id": "request-1",
    "error": {
        "code": "method_not_found",
        "message": "The requested method is not available."
    }
}
```

完整 payload 尚不能取得合法 request ID 时，错误响应的 `id` 为 `null`。成功响应的 `id` 必须
是对应请求的原值。响应必须且只能包含 `result` 或 `error` 之一。

请求限制：

| 字段 | 合同 |
|---|---|
| `id` | 非空 UTF-8 字符串，最多 64 bytes |
| `method` | ASCII 方法名，最多 128 bytes，格式为 `segment.segment` |
| `params` | JSON object；具体方法 Schema 决定字段 |

所有公开错误使用稳定字符串 `error.code` 作为机器可读事实来源。`message` 是稳定、安全的简短
说明，客户端不得解析它判断错误类型。`internal_error` 必须携带非敏感 `diagnosticId`；普通响应
不得包含堆栈、完整路径、环境变量、凭据或原始内部错误。

客户端必须忽略响应中的未知可选字段和未知 capability。兼容增加新 method、capability 或可选
响应字段不提升 RPC major；删除字段、改变既有字段类型/语义或改变既有方法语义属于破坏性
变化，必须提升 major 或经过批准提供明确兼容路径。

权威 Schema：

- `request-envelope.schema.json`
- `response-envelope.schema.json`
- `rpc-error.schema.json`
- `system-hello.request.schema.json`
- `system-hello.response.schema.json`
- `system-status.request.schema.json`
- `system-status.response.schema.json`

## 5. 握手状态机

每条连接的第一条有效请求必须为 `system.hello`：

- M1 只支持 `rpcVersion = 1`。
- 不支持的 major 返回 `rpc_version_unsupported`，随后关闭连接。
- hello 前调用其他方法返回 `handshake_required`；连接保持可用，客户端仍可发送 hello。
- 握手完成后再次 hello 返回 `handshake_already_completed`；连接保持可用。
- capability 是稳定、唯一的 ASCII 字符串集合。客户端最多声明 64 项，每项最多 128 bytes。
- 服务器只返回双方实际接受且由服务端支持的 capability；未知 capability 安全忽略。
- `client.name`、`client.version`、`client.instanceId` 仅用于诊断和能力协商，不参与授权。

M1 hello 请求示例：

```json
{
    "id": "hello-1",
    "method": "system.hello",
    "params": {
        "rpcVersion": 1,
        "client": {
            "name": "alcomd-cli",
            "version": "4.0.0-alpha.0",
            "instanceId": "e0d74a54-0fd9-4b7d-91eb-a408658f0253"
        },
        "capabilities": []
    }
}
```

hello 结果只包含已经真实存在的值：

```json
{
    "id": "hello-1",
    "result": {
        "rpcVersion": 1,
        "daemonVersion": "4.0.0-alpha.0",
        "capabilities": []
    }
}
```

`dataSchema`、`configSchema` 和 `extensionApi` 不属于 M1 hello 合同；这些子系统真正实现后才能
以兼容可选字段方式增加。

客户端 metadata 限制：

| 字段 | 最小 | 最大 UTF-8 bytes |
|---|---:|---:|
| `client.name` | 1 | 64 |
| `client.version` | 1 | 64 |
| `client.instanceId` | 1 | 128 |

## 6. system.status

`system.status` 是握手后的无参数只读方法：

```json
{
    "id": "status-1",
    "method": "system.status",
    "params": {}
}
```

M1 结果：

```json
{
    "id": "status-1",
    "result": {
        "product": "ALCOMD",
        "daemonVersion": "4.0.0-alpha.0",
        "rpcVersion": 1,
        "state": "ready",
        "capabilities": []
    }
}
```

不得返回 PID、完整本地路径、环境变量、凭据、虚构的子系统版本或尚未存在的 Operation/Store
状态。

## 7. M1 错误合同

| `error.code` | 语义 | 连接行为 |
|---|---|---|
| `invalid_request` | 完整 payload 的 UTF-8、JSON、envelope 或 params 无效 | 保持连接 |
| `method_not_found` | 握手后请求未知 method | 保持连接 |
| `handshake_required` | hello 前调用其他 method | 保持连接 |
| `handshake_already_completed` | 同一连接重复 hello | 保持连接 |
| `rpc_version_unsupported` | hello 请求不支持的 major | 响应后关闭 |
| `component_upgrade_required` | 对端组件版本不能满足已冻结的调用要求 | 保持连接，除非方法另有合同 |
| `internal_error` | 未知内部错误；必须包含 `diagnosticId` | 默认保持连接 |

`rpc_version_unsupported` 的非敏感 `data` 固定包含 `requestedVersion` 与
`supportedVersions`。其他 error data 必须由对应方法 Schema 明确允许。

## 8. 单实例与 CLI 按需启动

- 单实例使用与进程生命周期绑定的 OS advisory lock/所有权机制；lock 文件存在本身不是存活
  证据。本锁只拥有 daemon 生命周期，不是 M2 业务 Resource Lock。
- Unix stale socket 只有在已取得唯一实例锁、`lstat` 确认为 Unix socket、所有者为当前用户且
  位于已验证 runtime directory 后才能删除。
- `alcomd-cli system status` 在 endpoint not found 或 connection refused 时默认启动 sibling
  `alcomd`，总等待和重试上限为 5 秒。
- permission denied、协议版本或其他错误不得触发自动启动来掩盖问题。
- 并发 CLI 可以竞争启动，但最终只有一个 daemon 获得权威实例；daemon 生命周期不依赖 CLI。
- `--no-start-daemon` 禁止自动启动，供脚本、诊断和测试使用。
- M1 不建立 daemon supervisor、system service、插件式 launcher 或正式安装布局；M12 再验证
  完整产品中的启动与生命周期。

## 9. M2 capability 与 hello 兼容增加

M2 capability 固定为：

| capability | 允许的方法 |
|---|---|
| `state.check.v1` | `state.check` |
| `operations.v1` | `operations.get`、`operations.list`、`operations.cancel` |
| `events.replay.v1` | `events.list` |

客户端必须在本连接的 `system.hello.params.capabilities` 中请求，server 也实际支持并返回对应值后
才能调用。缺少协商结果返回 `capability_required`。capability 只对当前连接有效，不是 Principal
或 permission。

store 成功初始化且 daemon ready 后，hello result 兼容增加可选 `dataSchema: 1`：

```json
{
    "id": "hello-1",
    "result": {
        "rpcVersion": 1,
        "daemonVersion": "4.0.0-alpha.0",
        "capabilities": ["state.check.v1", "operations.v1", "events.replay.v1"],
        "dataSchema": 1
    }
}
```

`dataSchema` 不替代 RPC version/capability 协商。M2 不增加 `configSchema` 或 `extensionApi`。旧 M1
client 必须忽略该字段并继续工作。

## 10. M2 Principal 与 permission

M2 内建 Principal ID 是 `builtin:local-owner`，权限固定为：

- `state.check`
- `operations.read`
- `operations.cancel`
- `events.read`

每个 application 用例复验 permission 和 owner/visibility。client metadata、SID、pipe 名、
OperationId 与 capability 都不能证明 Principal。`builtin:local-owner` 是官方本地客户端的 M2
bootstrap Principal，不代表任意同用户进程已经完成可信认证，也不自动获得未来业务权限。

## 11. M2 method

### state.check

需要 capability `state.check.v1` 与 permission `state.check`。

```json
{
    "id": "check-1",
    "method": "state.check",
    "params": {
        "idempotencyKey": "check-2026-08-18"
    }
}
```

成功接受结果：

```json
{
    "id": "check-1",
    "result": {
        "operationId": "00000000-0000-4000-8000-000000000001",
        "replayed": false
    }
}
```

它只读执行有严格数量/大小上限的 `PRAGMA integrity_check` 与
`PRAGMA foreign_key_check`，不 repair/VACUUM/REINDEX。公开 Operation 结果只包含安全分类。

### operations.get

需要 capability `operations.v1` 与 permission `operations.read`。params 只有 `operationId`；只
返回当前 Principal 可见的 Operation。结果字段由 `operation.schema.json` 冻结。

### operations.list

需要 capability `operations.v1` 与 permission `operations.read`。默认 limit 100、最大 1000，
按 `(createdAtMs DESC, operationId DESC)` 排序。可选 cursor 是 opaque DTO，包含上一页最后一项
的 createdAtMs/operationId；下一页使用严格 tuple comparison。客户端不得构造或解释 cursor，
只应原样回传。新 Operation 插入不改变已继续向后的分页边界。

### operations.cancel

需要 capability `operations.v1` 与 permission `operations.cancel`。params 固定包含 operationId、
expectedRevision 与 idempotencyKey。接受取消意图不承诺最终 cancelled；合作式取消与完成竞态可
得到 succeeded/failed/cancelled。返回最新 Operation 与 replayed。

### events.list

需要 capability `events.replay.v1` 与 permission `events.read`。`afterSequence` 是 exclusive
cursor，limit 默认 100、最大 1000，结果严格 sequence ASC。非空页 nextSequence 是本页最后
Event sequence；空页等于输入。下一页直接把 nextSequence 作为 afterSequence，不做 `+1`。

M2 不删除 Event，因此不会产生 `event_cursor_expired`；该错误名只为未来 retention 保留。

## 12. Operation、revision 与 recovery

Operation 状态为：queued、planning、waiting_for_input、running、cancelling、succeeded、failed、
cancelled、interrupted、recovering。M2 `state.check` 不进入 planning/waiting_for_input。终态不可变。

- revision 从 1 开始，公开变化递增；no-op 与幂等重放不递增。
- 既有 Operation 写命令必须携带 expectedRevision；stale 返回 `revision_conflict`。
- Event aggregateRevision 等于提交后的 revision。
- 公开 u64 在 store 限制为 SQLite signed i64 正整数范围。
- queued 重启保持 queued 并重新调度。
- running/cancelling/recovering 重启后依次进入 interrupted、recovering，每步增加 revision/Event。
- interrupted/recovering 再次崩溃遵守同一规则。
- state.check 在检查前、两个 pragma 之间与检查后观察 cancellation；不承诺中断单条 pragma。

## 13. Event sequence

Event sequence 是数据库全局 AUTOINCREMENT 正整数，允许空洞。Event 与聚合状态在同一 transaction
提交。客户端按 sequence 去重并在断线后从最后成功处理的 sequence 继续。M2 不提供 notification、
server-initiated request、无限流或 retention。

## 14. 幂等

scope 固定为 `(PrincipalId, method, idempotencyKey)`。key 是非空 ASCII，最多 128 bytes。
同 key + 同 method-specific、版本化、类型化、非敏感 canonical fingerprint 返回原 Operation/响应；
不同 fingerprint 返回 `idempotency_conflict`。M2 不删除记录、不按时间过期，也不允许 key 因时间
经过重新使用。fingerprint 不使用 Rust DefaultHasher 或随机 hash。

## 15. M2 稳定错误

除 M1 错误外，M2 冻结：

| `error.code` | 语义 |
|---|---|
| `capability_required` | 当前连接未协商 method 所需 capability |
| `permission_denied` | Principal 缺少 permission 或 owner visibility |
| `revision_conflict` | expectedRevision 与当前值不一致 |
| `idempotency_conflict` | 同 scope key 对应不同 fingerprint |
| `operation_not_found` | 当前 Principal 可见范围中不存在该 Operation |
| `operation_not_cancellable` | Operation 已终态或当前转换不能取消 |
| `event_cursor_expired` | 未来 retention 后 cursor 过旧；M2 当前不产生 |
| `data_schema_unsupported` | store Schema 高于当前支持版本；启动期 fail closed |
| `store_unavailable` | daemon ready 后 store 操作失败 |

公开错误不包含 SQL、数据库路径、完整 SQLite 文本、堆栈或凭据。启动期 Schema 不支持不会把
daemon 暴露成 half-ready，也不改变 `system.status state="ready"` 的成功合同。

## 16. M2 仍不支持

- notification、batch、server-initiated request、新 transport。
- planning/waiting_for_input 的公开 input/approve/reject/resume。
- 外部 credential/pairing/revocation 与未来业务权限。
- Event retention/compaction 与实际 `event_cursor_expired`。
- 项目、VPM、文件事务或通用 workflow/job API。

## 17. M3 兼容增加：项目与 Repository

M3 在 RPC major 1 上增加四项 capability：`projects.read.v1`、`projects.registry.v1`、
`repositories.read.v1`、`repositories.registry.v1`。

| method | capability | permission | 语义 |
|---|---|---|---|
| `projects.inspect` | `projects.read.v1` | `projects.read` | 无状态读取绝对路径；显式 exact-root/search-parents |
| `projects.list` | `projects.read.v1` | `projects.read` | 注册项目稳定分页 |
| `projects.get` | `projects.read.v1` | `projects.read` | 读取一个已注册 snapshot |
| `projects.register` | `projects.registry.v1` | `projects.manage` | exact root 读取成功后写 registry |
| `projects.refresh` | `projects.registry.v1` | `projects.manage` | 刷新 normalized snapshot，不写项目 |
| `projects.unregister` | `projects.registry.v1` | `projects.manage` | 只删 registry row，不删目录 |
| `repositories.inspect` | `repositories.read.v1` | `repositories.read` | 无状态读取 local/remote source |
| `repositories.list` | `repositories.read.v1` | `repositories.read` | 注册 source 稳定分页 |
| `repositories.get` | `repositories.read.v1` | `repositories.read` | 读取一个 last-known-good snapshot |
| `repositories.packages` | `repositories.read.v1` | `repositories.read` | raw package/version identity 分页 |
| `repositories.register` | `repositories.registry.v1` | `repositories.manage` | 首次完整读取成功后写 registry/cache |
| `repositories.refresh` | `repositories.registry.v1` | `repositories.manage` | 条件刷新并保留 last-known-good |
| `repositories.unregister` | `repositories.registry.v1` | `repositories.manage` | 只删 registry/cache，不删 local source |

`projects.inspect/register` 的 RPC path 必须绝对；CLI 可先解析相对路径。local repository path 同样
必须绝对，remote URL 只允许无 userinfo 的 HTTP(S)。DTO、枚举、长度、cursor 与可选字段由
`m3-project-repository.schema.json` 冻结。列表默认 limit 100、最大 1000；registry page 按
`(registeredAtMs DESC, id DESC)`，package page 按 `(packageId ASC, version ASC)`，cursor exclusive。
注册项目的 `ProjectSnapshot` 以兼容可选字段返回 `registeredAtMs`（Unix epoch
milliseconds）；`projects.inspect` 产生的未注册 snapshot 不返回该字段。客户端必须继续接受字段缺失，
不得使用 `observedAtMs` 推断或伪造注册时间。

同步写命令使用 M2 永久幂等 scope，但成功响应不需要 OperationId。`register` 返回注册后的
aggregate 与 `replayed`；`refresh` 返回最新 aggregate 与 `replayed`；`unregister` 返回资源 ID、
删除前 revision + 1、`unregistered: true` 与 `replayed`。同 Principal、method、key、fingerprint
重放 durable response；不同 fingerprint 返回 `idempotency_conflict`。外部 fetch/parse 失败发生在
reservation 前，不消耗 key。

Project/Repository Event kind 固定为 `project.registered/refreshed/unregistered` 与
`repository.registered/refreshed/unregistered`。no-op、HTTP 304、validator-only update 与失败不增加
revision、不写 Event。unregister Event 保留在历史中，重新注册生成新 ID。

M3 增加稳定错误：`path_encoding_unsupported`、`project_not_found`、
`project_not_registered`、`project_already_registered`、`project_inaccessible`、
`project_version_missing`、`project_version_invalid`、`project_manifest_invalid`、
`repository_not_found`、`repository_not_registered`、`repository_already_registered`、
`repository_source_invalid`、`repository_inaccessible`、`repository_unavailable`、
`repository_document_invalid`、`repository_document_too_large` 与
`repository_credentials_unsupported`。普通错误不返回完整路径、URL userinfo、原始文档、parser
debug、HTTP header、SQL 或 credential；未知错误继续使用 `internal_error + diagnosticId`。

M3 仍不增加 notification、background refresh、credential、package payload、SemVer、resolver、
Plan/Apply 或项目写入。新增 method/capability/可选响应字段是兼容增加；既有 M1/M2 方法语义不变。

## 18. M4 兼容增加：Package Plan/Apply

M4 在 RPC major 1 上冻结两个 capability：

| capability | method |
|---|---|
| `packages.plan.v1` | `packages.planInstall`、`packages.planRemove`、`packages.planUpgrade`、`packages.planDowngrade`、`packages.planResolve` |
| `packages.apply.v1` | `packages.applyPlan` |

本节同时约束已接入的 M4 最小生产垂直切片、hello capability 广告与 State Schema v3 自动
migration。完整 GUI/CLI/MCP、credential、hashless/legacy VPM 和 v3 parity 仍未实现，不得因这些
method 已接通而描述为完整 package manager。

所有 Plan 方法是 bounded 同步 RPC，需要 `projects.read + repositories.read + packages.read` 及目标
resource scope。它们可以保存 immutable durable Plan，但不得写 Unity 项目、下载 package、访问
网络或触发 repository refresh。只使用最后一次显式 refresh 产生的 resolver-ready snapshot；否则
返回 `repository_refresh_required`。

Plan 的 action 与 method 一一对应，返回 `planId`、`state=unapplied`、project revision、byte-stable
ChangeSet 与 SHA-256 fingerprint。ChangeSet 最多 1,024 package mutation 与 4,096 dependency edge，
同时受 4 MiB RPC frame 限制；超限返回 `plan_too_large`，不得截断。Plan 无 TTL/expiry/自动清理，
状态只有 `unapplied`/`applied`，一个 Plan 最多绑定一个 Apply Operation。

`packages.applyPlan` 参数固定为：

```json
{
    "planId": "00000000-0000-4000-8000-000000000401",
    "expectedRevision": 7,
    "idempotencyKey": "apply-package-plan-once"
}
```

Apply 需要 `packages.apply.v1`、`packages.manage` 与目标 project write scope，并重新验证 read 权限、
Plan owner、project identity/revision/fingerprint，以及每个 pinned repository ID/revision、source
identity、manifest fingerprint、package version、artifact URL 与 SHA-256。任一前提变化返回
`plan_stale` 与 `m4-package-transaction.schema.json#/$defs/planStaleReason` 中的稳定 subreason；Apply
不得重新 resolve、fallback source、改变版本或产生新 ChangeSet。

成功接受返回 `{operationId,replayed}`。永久幂等 scope 仍为
`(PrincipalId, packages.applyPlan, idempotencyKey)`；fingerprint 包含 planId 与 expectedRevision。
请求连接断开不取消已返回的 OperationId。Apply Operation kind 是 `packages.apply`，复用 M2 state、
revision、Event、cancel/recovery 语义；公开 progress 只暴露脱敏 phase，不含完整 path/URL/entry。

M4 remote archive 只接受 64-lower-hex SHA-256，缺失/畸形均为 `package_hash_required`。这是 M4
安全子集而非完整 VPM hashless 兼容。M4 不提供 yanked override 或 legacy cleanup；yanked 不参与
新选择，legacy metadata 需要变更时返回 `package_legacy_cleanup_required`。

M4 新增的稳定错误由 `rpc-error.schema.json` 枚举冻结，包括 package manifest/range/dependency/
Unity/source/hash/cache/archive/path、Plan、refresh 与 project transaction 错误。普通 `error.data` 只
允许安全 subreason、opaque ID 和 revision，不得包含完整路径、credential/header、raw manifest、
archive、SQL 或 parser/OS debug。未知错误仍为 `internal_error + diagnosticId`。

完整 DTO、长度与枚举由 `m4-package-transaction.schema.json` 冻结。新增 method/capability/可选
Operation progress 是兼容增加；M1-M3 字段与方法语义不变。

## 19. M5 contract-first 兼容增加：Unity registry 与 launch

M5 在 RPC major 1 上兼容增加并实现三项 capability：

| capability | method | permission |
|---|---|---|
| `unity.read.v1` | `unity.installations.list/get`、`unity.projectEditor.get`、`unity.writerState` | `unity.read` |
| `unity.manage.v1` | `unity.installations.register/remove/refresh`、`unity.projectEditor.set` | `unity.manage` |
| `unity.launch.v1` | `unity.launch`、`unity.launchStatus` | `unity.launch` |

manual register 与 discovery candidate 必须经同一 executable validator。`unity.installations.register`
只接受绝对 executable path 与 idempotencyKey；path、Hub/CLI 声明或显示版本本身不能证明 identity。
`refresh` 是 bounded registry synchronization，可返回显式 partial diagnostics，但 malformed/不存在的
candidate 不得注册为有效 installation。Hub CLI 不成为权威依赖。

project Editor preference 保存 InstallationId 与 bounded argv array。`expectedRevision=0` 表示调用方
要求 preference 尚不存在，正整数要求精确 revision。参数不得包含 `-projectPath` 或等价重复 project
selector；daemon 固定把已验证 absolute project root 作为独立 `-projectPath` argv 传给已验证 Editor，
不得经 shell。

`unity.writerState` 返回 `running_confirmed`、`running_suspected`、`not_observed` 或 `unknown` 及有界、
脱敏 evidence。process/path/argv 检查失败必须为 unknown；not_observed 不表示 definitely not running。
package/template/backup mutation 对 confirmed 返回 `unity_project_running`，suspected/unknown 仅 advisory
并继续 live fingerprint gate。Unity launch 对 confirmed 拒绝第二实例，对 suspected/unknown 返回
`unity_launch_state_uncertain`，只有 not_observed 允许 spawn。

`unity.launch` 复验 project/installation identity、project revision、permission 与永久 idempotency；成功
只返回 opaque launch record 且 `state=opening`/`spawnAccepted=true`，不宣称 Unity 已完全打开。
`unity.launchStatus` 后续只能观察为 opening/open/failed。客户端断开不终止 Unity；M5 不建立通用
supervisor。foreground/activation 不在本合同中。

新增稳定错误为 `confirmation_required`、`unity_installation_not_found`、
`unity_installation_invalid`、`unity_installation_in_use`、`unity_version_unverified`、
`unity_version_mismatch`、`unity_architecture_unsupported`、`unity_project_running`、
`unity_launch_state_uncertain`、`unity_project_selector_forbidden`、`unity_launch_failed` 与
`unity_launch_not_found`。普通 error 不包含完整进程命令行、私密路径、PID 列表或 OS debug。

完整 DTO、enum 与上限由 `m5-unity.schema.json` 冻结。State Schema v5 已接入自动 migration；daemon
在 store 成功初始化后通过 hello 广告当前 `dataSchema: 7` 和客户端实际协商的已实现 M5 capability。此兼容增加不
改变 M1-M4 方法语义，也不表示 Template、Backup 或完整 CLI 已实现。

## 20. M5 contract-only 兼容增加：Template Bundle v1

Template production adapter 已实现以下三项 capability；`system.hello` 只在实际 dispatcher、application
use case 与持久状态初始化成功后广告：

| capability | method |
|---|---|
| `templates.read.v1` | `templates.list/get/inspectBundle/export` |
| `templates.manage.v1` | `templates.planImport/applyImport/planDerive/applyDerive/setFavorite/remove` |
| `templates.create-project.v1` | `templates.planCreateProject/applyCreateProject` |

完整 DTO、collection/field bounds、immutable Plan、Operation progress 与 permission matrix 由
`m5-template.schema.json` 冻结。这些 method 已进入 dispatcher；Backup 仍未实现或发布。

Template Plan authority 使用 State Schema v5 的窄 `template_plans` 表。Plan JSON 固定 `version: 1`
并按 import/derive/create-project 使用严格 DTO；唯一允许的持久更新是从 unapplied 绑定到精确匹配的
`templates.import`、`templates.derive` 或 `templates.create-project` Operation。M4 `package_plans`
保持不变且不得承载 Template Plan。Schema v5 只使这些 Operation kind 可持久化，不等同于 capability
或 dispatcher 已发布。

inspect 纯只读。Import/override 和 derive 先产生 immutable Plan；Apply 返回 OperationId 并复验相同
bundle/project identity、digest、revision、fingerprint 与 idempotency。新 ID import 可创建；user 同 ID
同 digest 为 no-op；user 同 ID 异 digest为 `template_conflict`；builtin ID 永远
`template_builtin_immutable`。favorite 是低影响 revision/idempotency command；remove 只删除 user registry
binding，不执行 object GC。

create-project Plan 固定 parent filesystem identity + normalized target leaf、target absent、template
revision/digest、manifest/payload/resource fingerprint 和 M4 exact package ChangeSet/source pins。Plan 不刷新
repository、不写目标目录。Apply 返回 OperationId，只能创建全新目录；不 overwrite、merge、删除既有
内容或重新 resolve。package dependency 复用 M4 authority 和 transaction primitive，不创建 nested child
Operation。state commit 前不得 succeeded，重启必须复用同一 OperationId/Plan/idempotency/ProjectId。

普通错误只返回稳定 code、opaque ID/revision 和安全 subreason，不返回 bundle/source/target 完整私密
路径、locator、raw manifest、ZIP entry、credential 或 SQL/OS debug。新增 Template errors 由
`rpc-error.schema.json` 冻结。CLI 名称在 `m5-template-commands-v1.json`，并已加入实际 command
catalog/help。

## 21. M5 兼容增加：Backup Create

Backup Create 在 RPC major 1 上冻结并发布两项 capability：`backups.read.v1` 与
`backups.create.v1`。完整 DTO 与 bounds 由 `m5-backup-create.schema.json` 冻结：`backups.list/get`
要求 `backups.read`；`backups.create` 要求 `backups.manage` 与目标 Project read scope，并直接返回预分配
且可幂等重放的 `operationId`/`backupId`。它没有 Plan/Apply，不接收任意 output path。

请求固定 `projectId`、`expectedRevision`、`compressionMode`、`excludeVpmPackages`、`idempotencyKey`。
Operation phase 固定为 `accepted`、`inventory_ready`、`archiving`、`archive_ready`、`publish_intent`、
`archive_published`、`state_committed`。进入 publish intent 后，取消不能产生 ambiguous final artifact；
恢复必须复用原 OperationId、BackupId 和幂等请求。

新增稳定错误为 `backup_not_found`、`backup_unavailable`、`backup_source_unsafe`、
`backup_archive_limit_exceeded`、`backup_integrity_mismatch` 与 `project_changed_during_backup`。普通错误、
Event 和 activity 不得包含完整 source/archive path、内部 locator、原始 argv、credential 或 journal。
Backup Archive v1/profile 由 `specs/backups/` 冻结。Create method 已进入 dispatcher、official client 默认
capability 与 CLI help。

Backup Restore contract-first 在 RPC major 1 上兼容冻结 `backups.planRestore`、
`backups.applyRestore` 与 `backups.restore.v1`，完整 DTO 由 `m5-backup-restore.schema.json` 冻结。Plan
要求 `backups.read + projects.create`；Apply 另需 `backups.manage`，返回固定 OperationId 与 Plan 预分配的
ProjectId。公共 Plan/result 只含 target/Backup 安全摘要，不返回 archive/staging path、DB locator 或 journal
detail。Restore dispatcher/worker/client/CLI 尚未实现，daemon 不广告 capability，也不接受 method；Schema v7
只证明 durable authority 可存储，不代表 production Restore 已发布。

## 22. M6 兼容增加：Extension Runtime

M6 在 RPC major 1 上实现并广告两项 capability：

| capability | method | permission |
|---|---|---|
| `extensions.lifecycle.v1` | `extensions.list/get/planInstall/applyInstall/enable/disable/planUninstall/applyUninstall` | read 使用 `extensions.read`；mutation 使用 `extensions.manage` |
| `extensions.permissions.v1` | `extensions.setGrant/revokeGrant` | `extensions.permissions.manage` |

完整 DTO 与 bounds 由 `m6-extension-runtime.schema.json` 冻结。install/uninstall 先创建永久 immutable Plan；Apply
复验 package/source/signature/publisher/trust/revision/digest 并返回 OperationId，不重新 Plan。uninstall 默认
`retain_data`；`delete_data` 必须在 immutable Plan 中显式选择。

enable/disable 是 revisioned、永久幂等的窄 lifecycle command，不创建 Operation kind。enable 在
package/API/grant 重验后创建 daemon-owned `ExtensionInstanceLease`；disable/revoke 的 durable grant revision commit
是 authority linearization point。State v8 只增加 `extensions.install` 与 `extensions.uninstall` 两个 Operation kind。
Host/guest 不能自报 PrincipalId、ExtensionId、publisher、first-party status 或 scope。desired state 与 runtime
state 分离，公开 record 不返回 PID、Host pipe、lease nonce、安装路径、trap/backtrace 或 extension data。

`extensions.setGrant/revokeGrant` 的 M6 business scope 只允许 `background.run + ExtensionId self` 与
`projects.read + specific ProjectId`。permission/grant update 使用 expected grant revision 与永久幂等；wildcard、path、
URL 和 Manifest selector 全部拒绝。

install source kind 只允许 `local_owner_selected` 与 `first_party_packaged`，不接受 URL、registry、marketplace、remote
catalog 或任意网络 fetch。retained data namespace 绑定 ExtensionId + publisher fingerprint；publisher 不同返回
`extension_data_owner_mismatch`。uninstall 总是撤销全部 grants/lease/session/handle，reinstall 不恢复旧 grant。

`system.hello.result.extensionApi` 是兼容 optional field；daemon 在 data Schema v8 migration、Host/WIT/runtime 与
对应 capability 全部生产可用后返回 `{major:1,world:"alcomd:extension/extension-v1@1.0.0"}`，并广告
`dataSchema: 8`。旧 client 必须忽略未知 optional field。

M6 stable errors 由 `rpc-error.schema.json` 冻结。普通 error 只返回安全 code、opaque ID/revision 与必要 subreason，
不返回 package/source/data 完整路径、public key material 以外的 credential、Host protocol、trap、WIT parser debug、
SQLite 或 OS debug。未知错误仍为 `internal_error + diagnosticId`。

## 23. M7 兼容增加：Portable Extension UI

M7 在 RPC major 1 上兼容增加 `extensions.ui.portable.v1` 与四个 method：

| method | params | result | client authority |
|---|---|---|---|
| `extensions.ui.open` | `extensionId`, `locale` | `session`, `snapshot` | `extensions.read` + `extensions.ui.use` scoped exact ExtensionId |
| `extensions.ui.refresh` | `sessionId`, `expectedSnapshotRevision` | `snapshot` | 每次重验当前 Session owner 与上述双权限 |
| `extensions.ui.dispatch` | `sessionId`, `expectedSnapshotRevision`, `sequence`, `requestId`, `action` | `snapshot`, `replayed` | 每次重验当前 Session owner 与上述双权限 |
| `extensions.ui.close` | `sessionId` | `closed` | 当前 Session owner best-effort close |

完整闭合 DTO、tagged unions、bounds、replay、revision、Session coordination 与安全错误由
`m7-portable-ui.schema.json`、`../extensions/portable-ui-v1.schema.json` 和
`../extensions/portable-ui-v1.md` 冻结。没有 `listSurfaces`、surface identity、filesystem locator、Host PID、pipe、
lease secret、InvocationContextId、private Principal metadata 或 raw Wasmtime error。

State v9 只为 `extensions` 与 immutable `extension_plans` 增加 nullable checked `ui_protocol`；UI Session、Snapshot、
action、replay 与 renderer state 都不持久化。B1 daemon/Host/RPC wiring 完成后，hello 广告 `dataSchema: 9`，并只在
client 请求时回显已实现的 `extensions.ui.portable.v1` capability。

## 24. M7 兼容增加：Official GUI settings/activity/diagnostics

M7 在 RPC major 1 上兼容增加四个 base method；不增加 capability：

| method | params | result | permission |
|---|---|---|---|
| `settings.get` | `{}` | `configSchema`, `revision`, full normalized `settings` | `settings.read` |
| `settings.update` | `expectedRevision`, closed partial `update` | new `revision`, full normalized `settings` | `settings.manage` |
| `activity.list` | optional tuple `cursor`, optional `limit` | closed Event/Operation projection and next cursor | `activity.read` |
| `diagnostics.list` | optional tuple `cursor`, optional `limit` | closed redacted diagnostics and next cursor | `diagnostics.read` |

`settings.update` 不接受 idempotency key、generic key/value、路径或 extension setting；revision conflict 使用现有
`revision_conflict`。只有 `config/settings.toml` production storage 和四个 RPC 全部接线后，hello 才以兼容
optional field 广告 `configSchema: 1`。

Activity/Diagnostics page size 默认 100、最大 200，使用确定性 tuple keyset cursor。Activity 不返回 Event payload
或 Operation request/result；Diagnostics v1 只投影已有 Operation failure 与 safe Event evidence，不新建 durable log
table，也不提供 raw export。完整 DTO、redaction denylist 与 bounds 由 `m7-official-gui.schema.json`、
`../config/settings-v1.schema.json` 和 `../security/official-gui-read-model-threat-model.md` 冻结。

## 25. M7 兼容增加：Project Copy

M7 在 RPC major 1 上实现并广告 `projects.copy.v1`：

| method | params | result | permission |
|---|---|---|---|
| `projects.planCopy` | source ProjectId/revision、target parent/leaf、idempotency key | immutable bounded Copy Plan、`replayed` | `projects.read + projects.create` |
| `projects.applyCopy` | PlanId、expected source revision、idempotency key | durable OperationId、预分配 target ProjectId、`replayed` | `projects.read + projects.create` |

外部 filesystem mutation 仍只允许 `builtin:local-owner`。Plan 有效期固定为 900000ms，只做 bounded
identity/writer/target preflight，不遍历 source、不计算 inventory 或内容 digest。Apply 返回
`projects.copy` Operation；完整 inventory 是 Operation-owned、versioned JSON Lines 私有文件，不进入 RPC、Event、日志或
SQLite inventory table。

Operation phase、Copy Profile v1、quota、source 两遍 SHA-256 一致性、sibling staging、forward recovery、取消边界、
Resource Lock 与稳定错误由 `m7-project-copy.proposal.schema.json` 和 `../storage/state-v10.md` 冻结。State v10 只增加
`project_copy_plans`、`project_copy_filesystem_journal` 与 `operations.kind='projects.copy'`；完整 wiring 后 hello 广告
`dataSchema: 10`。新增 method/capability/字段是 RPC v1 兼容增加，既有 Project/Package 方法语义不变。
