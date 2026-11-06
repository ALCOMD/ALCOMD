# ALCOMD RPC v1

状态：M1-M6 已完成人工验收；当前活动合同含 M7 Portable UI、候选读取、Config12 设置和 Unity Hub 打开。M7 仍在进行中，当前人工签收范围以 `docs/status.md` 为准。

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
Windows: \\.\pipe\vrclearn.ALCOMD.<current-user-SID>.rpc-v1
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
变化。已发布合同必须提升 major 或经过批准提供明确兼容路径；未发布合同的直接修订同样
必须经过项目所有者明确批准，并同步 Schema、调用方和测试，不自动增加别名或回退。
A-034 的数据库开发兼容策略本身不授权 RPC、Extension API 或权限变更。

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

`dataSchema` 表示 daemon 成功初始化后的实际数据库结构版本。当前 State v28 由 daemon 的存储
组合处提供，protocol 只序列化该事实，不能独立硬编码旧版本。当前 `configSchema` 为 12，
`extensionApi.major` 为 1。这些内部格式版本不等于产品发布版本。
按 A-034，未发布开发库不承诺长期兼容；当前保留 v16 及既有迁移，发布前 Schema 整理另行审计，
不因此自动清空数据。首次正式发布后维护正式升级与恢复链。

`dataSchema` 不替代 RPC version/capability 协商。M2 本身不增加 `configSchema` 或 `extensionApi`。旧 M1
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

M7 已批准的 `activeOnly` 默认为 false；true 只返回非终态任务，仍使用同一排序与独立分页。
`activeCount` 是当前 Principal 可见的全部非终态任务数量，不受分页或 activeOnly 过滤影响，
与本次列表来自一致读取依据。首次/重新同步分别读取近期历史和全部活跃任务，不能用 count
代替活跃任务内容，也不能把活跃分页游标作为历史继续位置。

### operations.cancel

需要 capability `operations.v1` 与 permission `operations.cancel`。params 固定包含 operationId、
expectedRevision 与 idempotencyKey。接受取消意图不承诺最终 cancelled；合作式取消与完成竞态可
得到 succeeded/failed/cancelled。返回最新 Operation 与 replayed。

### events.list

需要 capability `events.replay.v1` 与 permission `events.read`。`afterSequence` 是 exclusive
cursor，limit 默认 100、最大 1000，结果严格 sequence ASC。非空页 nextSequence 是本页最后
Event sequence；空页等于输入。下一页直接把 nextSequence 作为 afterSequence，不做 `+1`。
事件读取同时受4MiB字节预算限制，可在数量上限之前返回有界前缀；nextSequence仍只推进到
已返回事件。单条事件无法容纳时明确失败，不静默跳过。订阅发送另检查完整编码帧大小。

M2 不删除 Event，因此不会产生 `event_cursor_expired`；该错误名只为未来 retention 保留。

### events.subscribe（M7 已批准增量）

完整合同见 [m7-events-live.schema.json](m7-events-live.schema.json) 与独立的
[notification-envelope.schema.json](notification-envelope.schema.json)。需要 `events.live.v1` 和
`events.read`，初期仅允许现有已认证 local-owner；事件不授予任何后续资源查询权限。

订阅独占一条已认证/协商的连接：普通带 id 的 events.subscribe 请求、带相同 id 的就绪响应，
随后是无 id 的 events.batch/events.closed Notification Envelope。普通请求/响应 id 合同不变，
不以 id:null 代替通知。订阅期间不接受其他请求，关闭连接取消订阅，不影响命令连接。

就绪结果含 subscriptionId、instanceId、baselineCursor 与 resyncRequired。首次订阅取得基线 B，
接收 B 之后的事件，再读取所需快照。有效同实例游标从已接纳位置补发；实例变化或合法游标
不可恢复时明确 resyncRequired=true 并给新基线，同时继续传递其后的事件。

events.batch 包含订阅/实例关联、递增 batchId、sequence 升序事件和 nextCursor。owner 过滤
导致的 sequence 间隔合法。默认每批100、最多1000条，完整编码帧不得超过现有4MiB；达到
字节界限时分为较小批次，游标只推进到已包含记录，不丢弃剩余事件。单条超限明确失败。

游标另含必填 `resourcesRevision`，批次必含 `invalidatedScopes`。本轮唯一失效类别为
`extensions`：M6 已提交的 desired/grant 修订变化没有对应持久 Event，故由实际写入边界
推进此独立内存水位，不伪造 Event、不改变数据库聚合约束、不新增活动记录。无变化、回滚
和幂等重放不推进。资源水位非递减，推进时必须带相应失效类别；类别数组最多一项。

仅资源失效时 events 可以为空，但 invalidatedScopes 必须非空、resourcesRevision 必须
严格前进，而持久 sequence 保持上次位置。有真实事件时 sequence 仍严格上升且 nextCursor
指向最后返回事件。没有事件也没有新失效的普通空批次仍禁止。未知失效类别为协议错误。
同实例旧资源水位不保留内存历史；不能连续恢复时返回 resyncRequired=true 与新基线，
由当前权威查询恢复状态，不能假装仅补发持久事件便已同步。

服务端最多持有一份待发送批次，完整内容来自持久 Events；Store 在实际提交边界更新保存
最新水位的信号。先登记监听再检查/等待，避免提交恰好落在检查与等待之间。写线程不等待
传输、前端 ACK 或页面查询；回滚/无变化/重放不产生新事件。

订阅就绪等待上限5秒，发送和前端 ACK 等待各10秒；健康空闲没有周期性读超时、空批次或
业务心跳。服务端发送位置、原生接收位置和前端接纳位置分别维护。断线可重复补发，依据
已安全接纳游标恢复，不承诺传输恰好一次。前端状态丢失必须重新取得快照基线。

events.closed 携带明确错误和 retryable。非法帧、未知通知、协议/权限错误永久停止自动重连；
暂时传输错误使用250ms起、最高30秒的故障退避。原生最多一个待确认批次，ACK绑定订阅、
连接代际及批次；仅表示共享入口已登记失效信息，不等待页面读取/渲染。旧 ACK/取消不影响
新订阅，消费者停滞或窗口重载可取消并重新同步。客户端不得静默退回业务轮询。

项目最近活动时间推进使用 `project.activity-recorded`，仓库内容未变而公开检查时间推进使用
`repository.observed`；沿用当前资源 revision，不改变项目内容或 Plan 的 revision 语义。
`project.activity-recorded` 同时投影为“请求在Unity中打开”的活动，不代表编辑器已成功打开。
`repository.observed` 与 `operation.progress` 不投影为独立活动条目；时间未变、
回滚和幂等重放不产生新事件。公开任务恢复状态的变化仍须递增 revision，并同事务发布
对应状态事件；已经处于相同恢复状态的重复恢复不产生变化。

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
| `repositories.inspect` | `repositories.read.v1` | `repositories.read` | 读取 local/remote source，不修改 registry；远程预览可供同一连接确认登记时复用 |
| `repositories.list` | `repositories.read.v1` | `repositories.read` | 官方、精选内置仓库在前，其余按登记顺序分页 |
| `repositories.get` | `repositories.read.v1` | `repositories.read` | 读取一个 last-known-good snapshot |
| `repositories.packages` | `repositories.read.v1` | `repositories.read` | raw package/version identity 分页 |
| `repositories.register` | `repositories.registry.v1` | `repositories.manage` | 完整读取成功后写 registry/cache；同一连接可复用 60 秒内同一规范化远程来源的预览 |
| `repositories.refresh` | `repositories.registry.v1` | `repositories.manage` | 条件刷新并保留 last-known-good |
| `repositories.refreshAll` | `repositories.registry.v1` | `repositories.read` + `repositories.manage` | Core 滚动刷新调用者已登记的仓库，复用共享下载额度 |
| `repositories.setCredentials` | `repositories.registry.v1` | 本机所有者及 `repositories.manage` | 用仓库 revision 和幂等键替换认证引用，显式 `credentials: null` 清除 |
| `repositories.setDisplayName` | `repositories.registry.v1` | `repositories.manage` | 设置已登记仓库的本地显示名，显式 `displayName: null` 清除 |
| `repositories.unregister` | `repositories.registry.v1` | `repositories.manage` | 只删 registry/cache，保留 local source；Core 的内置仓库返回 `permission_denied` |

仓库列表继续使用返回的 `nextCursor`。若游标对应的仓库在分页期间被移除，
Core 返回 `revision_conflict`，客户端可从首页重读。

显示名适用于内置与用户仓库。输入最多 128 个 Unicode 标量值，拒绝控制字符。
Core 去掉首尾空白，空值恢复默认名称。读取及写入结果中的 `displayName` 仅在设置覆盖值时出现。
沿用仓库修订及幂等键，只有实际变化才写入 `repository.display-name-changed` 事件。

本机所有者可在 `repositories.inspect` 和 `repositories.register` 传入写入专用 `credentials`。
`headers` 最多 16 项，完整凭据 JSON 上限 2560 字节。旧 `allowedOrigins` 字段被拒绝，旧凭据需重新输入。
索引和 ZIP 首跳直接使用仓库头，ZIP 同名包头优先。重定向更换协议、主机或端口时移除
`Authorization`、`Cookie`、`Cookie2`、`Proxy-Authorization` 和 `WWW-Authenticate`，其余已验证的头继续发送。
首次下载缺少发布方摘要的 ZIP 可使用 HTTP(S)，验证后固定摘要。列表导入支持 `vcc://vpm/addRepo` 的 `headers[]`，
内层仓库允许 HTTP(S)，忽略未知查询参数。
读取只显示 `credentialsConfigured`，显式导出认证仓库时，文件包含该仓库的 VCC 链接及明文请求头。包计划固定仓库修订和内部引用，轮换使旧计划失效。

`projects.inspect/register` 的 RPC path 必须绝对；CLI 可先解析相对路径。local repository path 同样
必须绝对，remote URL 只允许无 userinfo 的 HTTP(S)。DTO、枚举、长度、cursor 与可选字段由
`m3-project-repository.schema.json` 冻结。列表默认 limit 100、最大 1000；registry page 按
`(registeredAtMs DESC, id DESC)`，package page 按 `(packageId ASC, version ASC)`，cursor exclusive。
`repositories.refreshAll` 接收单个最多 64 字节的幂等键。Core 对本 principal 最多 4096 个已登记的源
滚动发起刷新，修订冲突时读取当前修订并重试一次。结果按登记快照顺序逐仓库返回 `refreshed` 或
`failed`，失败附稳定错误码。单仓失败时保留其 last-known-good 状态。
注册项目的 `ProjectSnapshot` 以兼容可选字段返回 `registeredAtMs`（Unix epoch
milliseconds）；`projects.inspect` 产生的未注册 snapshot 不返回该字段。客户端必须继续接受字段缺失，
不得使用检查时间推断或伪造注册时间。

`ProjectSnapshot.lastModifiedAtMs` 为兼容增加的可选 Unix epoch milliseconds 字段，由 Core
维护对应 v3 用户语义的项目最近活动／修改时间。已注册项目只有
存在权威记录时才返回；未注册 inspect 和旧记录没有可靠依据时省略，客户端显示未知，不得以
检查时间、`registeredAtMs` 或数据库 `updatedAtMs` 替代。该字段不新增 method、capability、
permission 或请求参数，不改变 registry 分页顺序／cursor、Plan/Apply 或项目内容 revision 语义。

项目 `observedAtMs` 已按所有者批准从 `ProjectSnapshot`／内部项目观察模型移除，属于发布前
RPC v1 的破坏性字段删除；不提供别名、fallback、双版本模型或新 method/capability/permission。
State v15→v16 物理删除项目观察列并清理项目快照与永久幂等响应中的旧字段；旧未 Apply 的
Unity migration Plan 失效，须重新生成 Plan，不重签旧 authority hash。已执行的 Plan／journal
保留原 fingerprint 和迁移专有重检证据。Unity 安装扫描时间与 copy/delete writer evidence
的同名字段属于不同合同，继续保留。普通项目重新检查只产生项目内容，no-op refresh 不改变
revision、最后修改时间或候选权威指纹；内容变化仍按现有 revision／Plan 验证处理。

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
migration。完整 GUI/CLI/MCP、credential、完整 legacy VPM 和 v3 parity 仍未实现，不得因这些
method 已接通而描述为完整 package manager。

所有 Plan 方法是 bounded 同步 RPC，需要 `projects.read + repositories.read + packages.read` 及目标
resource scope。它们保存 immutable durable Plan，使用已登记的 resolver-ready snapshot，不写 Unity
项目或触发 repository refresh。为验证旧资产声明可读取对应清单；选中 HTTP(S) 包缺少发布方摘要时，
仅下载尚未锁定的归档以验证并固定实际 SHA-256。

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

仓库声明 `zipSHA256` 时接受 64-lower-hex SHA-256 并严格比对，畸形值返回 `package_hash_required`。
字段缺省时首次下载后本地固定摘要。Apply 使用计划中的固定摘要核对实际归档。
`author.email` 可省略。Yanked 版本不参与新选择。

普通包安装、更新和解析计划处理 VPM 的 `legacyFiles`、`legacyFolders` 和
`legacyPackages`。Plan 使用固定来源和归档清单，依据项目相对路径和 GUID 匹配旧资产，
随资产纳入存在的 `.meta`，并为仍登记的旧包生成 remove mutation。若所选依赖仍需要
待移除的旧包，返回 `package_dependency_conflict`。

采用该迁移依据的 ChangeSet 使用 `formatVersion: 4`。非空 `legacyRemovals` 为最多
4096 项的有序数组，每项包含项目相对 `path`、`kind`（`file` 或 `directory`）与
声明来源 `packageId`。清单及内部身份、内容证据由 ChangeSet 摘要绑定。
Apply 复验并在现有事务中隔离旧项。目标清单变化时返回 `plan_stale`，
无法安全确认的路径返回 `package_legacy_cleanup_required`。

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
| `unity.read.v1` | `unity.installations.list/get`、`unity.projectLaunchConfig.get`、`unity.writerState` | `unity.read` |
| `unity.manage.v1` | `unity.installations.register/remove/refresh`、`unity.projectLaunchConfig.set/clear` | `unity.manage` |
| `unity.launch.v1` | `unity.launchOptions`、`unity.launch` | `unity.launch` |

manual register 与 discovery candidate 必须经同一 executable validator。`unity.installations.register`
只接受绝对 executable path 与 idempotencyKey；path、Hub/CLI 声明或显示版本本身不能证明 identity。
`refresh` 是 bounded registry synchronization，可返回显式 partial diagnostics，但 malformed/不存在的
candidate 不得注册为有效 installation。Hub CLI 不成为权威依赖。

Project launch config 只保存 bounded argv array，不保存 InstallationId 或选择状态。缺失 config 规范化为
`arguments=[]`、`revision=0`、`updatedAtMs=0`；`expectedRevision=0` 要求 config 尚不存在，正整数要求精确
stored revision。same-value set 与 clear-missing 是 semantic no-op。参数不得包含 `-projectPath` 或等价重复
project selector；daemon 固定把已验证 absolute project root 作为独立 `-projectPath` argv 传给已验证 Editor，
不得经 shell。

`unity.writerState` 返回 `running_confirmed`、`running_suspected`、`not_observed` 或 `unknown` 及有界、
脱敏 evidence。process/path/argv 检查失败必须为 unknown；not_observed 不表示 definitely not running。
`running_suspected` 需要正向项目关联证据：进程的项目目录身份匹配目标，但 executable 身份尚未核实。
已明确属于其他项目的进程不影响目标项目；无法关联的同名进程或进程观察失败保留为 `unknown`，不谎称未运行。
package/backup mutation 对 confirmed 返回 `unity_project_running`，suspected/unknown 仅 advisory
并继续 live fingerprint gate。Unity launch 仅对目标项目 confirmed 拒绝第二实例，对目标项目 suspected 返回
`unity_launch_state_uncertain`；not_observed 与无法关联的 unknown 允许 spawn，不因其他或身份不明进程形成全局启动禁令。
项目删除和 Unity 版本迁移同样只对目标项目 confirmed/suspected 拦截，无法归属的 unknown 不作全局阻断。
保留目标目录身份、内容、权限及文件系统边界复验；删除在隔离目录前再次观察目标占用。
迁移已观察到自有 Unity 子进程退出时，unknown 不妨碍目标目录/版本复验；中断后没有退出证据时仍要求恢复核实，禁止仅凭 unknown 自动报成功或重启 Unity。

`unity.projectLaunchConfig.get` 返回必需 `config`、`defaultArguments`、`effectiveArguments`；
`config.arguments` 保留项目覆盖语义。优先使用已保存项目覆盖（含空数组），其次全局覆盖，最后内置空数组；
整组替换，不拼接。项目 revision 0 表示继承，clear 恢复继承。get 沿用 `projects.read + unity.read`，
set/clear 的结果形状及权限保持。已接受 launch 固定有效参数，后续设置变更不改已接受启动；相同幂等键的有效参数变化返回冲突。
flag 和 value 各占一项，通过无 shell 的 argv 数组传递；带空格路径保持为一个字符串，不要求额外 shell 引号。

`unity.launchOptions` 由 Core 按 canonical exact Unity version 返回确定排序的匹配 installation。`unity.launch`
要求调用方提供 one-shot InstallationId；该选择不持久化，并在启动前复验 project/installation identity、
project revision、canonical exact version、permission 与永久 idempotency。成功
只返回 opaque launch record 且 `state=opening`/`spawnAccepted=true`，不宣称 Unity 已完全打开。
启动请求接受与失败继续写入正常启动幂等记录；客户端断开不终止 Unity。不提供按启动记录手动复查进程状态的 RPC、SDK 或 CLI 接口，
也不建立替代轮询或通用 supervisor。foreground/activation 不在本合同中。

新增稳定错误为 `confirmation_required`、`unity_installation_not_found`、
`unity_installation_invalid`、`unity_installation_in_use`、`unity_version_unverified`、
`unity_version_mismatch`、`unity_architecture_unsupported`、`unity_project_running`、
`unity_launch_state_uncertain`、`unity_project_selector_forbidden`、`unity_launch_failed` 与
`unity_launch_not_found`。普通 error 不包含完整进程命令行、私密路径、PID 列表或 OS debug。

完整 DTO、enum 与上限由 `m5-unity.schema.json` 冻结。State v14 删除旧 editor preference 并保留独立 launch config。daemon
在 store 成功初始化后通过 hello 广告实际 `dataSchema` 和客户端实际协商的已实现 capability。此兼容增加不
改变 M1-M4 方法语义，也不表示 Backup 或完整 CLI 已实现。

## 20. M7 新建项目与模板声明

Core 通过 `projects.create.v1` 发布 `projects.create`。已认证的本机所有者持有 `projects.create` 权限时可提交
`{name,targetParent,unityVersion,idempotencyKey}`，并获得持久任务和预分配项目
`{operationId,projectId,replayed}`。完整 Unity Editor 版本不要求已安装。同一 owner、方法和幂等键下，
相同请求返回原任务，不同请求返回 `idempotency_conflict`。成功的任务结果包含 `projectId`。
Core 校验目标父目录的身份，只向不存在的目标发布，并在同一事务登记项目、完成任务和写入事件。

模板目录通过 `projectTemplates.v2` 发布 `projectTemplates.list`、`projectTemplates.save`、
`projectTemplates.export` 和 `projectTemplates.remove`，仅限已认证的本机所有者。
列表按 ID 分页，记录包含 ID、显示名、可选 Unity 版本、修订和锁定标记。变更提交
`project-template.changed` 事件，内部 UUID 只用于关联事件。同 ID 同内容复用记录，
同 ID 异内容需提供现有修订。锁定记录拒绝替换或删除。

保存支持已选 `.alcomdtemplate` 文件、已登记项目及手动填写三种来源。导入归档须在
`ProjectSettings/ProjectVersion.txt` 声明一个完整 Unity 版本。
项目来源要求所选项目的 `projects.read` 和当前修订，两个可选开关默认关闭。
保存与导出使用持久幂等任务，接受后返回 `{operationId,replayed}`。
保存成功结果为 `{template}`，导出成功结果为 `{id,revision}`。
导出目标通过独占发布产生。模板归档内部为标准 ZIP 及 VCC 目录结构。

`projects.create` 以 `template: {id, revision}` 选择当前归档，在接受事务中冻结模板。
过期引用返回 `revision_conflict`。使用模板依赖仍检查 `packages.read`、`repositories.read`
和 `packages.manage`。请求的 Unity 版本必须与模板声明精确匹配。以前登记的无版本模板须
补全后替换才能用于创建项目。安装锁定或求解后的包后才发布。
公开任务失败可通过结果的 `packageId` 标识受影响的软件包。
State28 替换 State23 的模板声明和能力。精确 DTO 见 `m7-project-create.schema.json`。

## 21. M5 兼容增加：Backup Create

Backup Create 在 RPC major 1 上冻结并发布两项 capability：`backups.read.v1` 与
`backups.create.v1`。完整 DTO 与 bounds 由 `m5-backup-create.schema.json` 冻结：`backups.list/get`
要求 `backups.read`；`backups.create` 要求 `backups.manage` 与目标 Project read scope，并直接返回预分配
且可幂等重放的 `operationId`/`backupId`。它没有 Plan/Apply，不接收任意 output path。

请求必须提供 `projectId`、`expectedRevision`、`idempotencyKey`。`compressionMode`、`excludeVpmPackages`、`includeLibrary`
可分别省略，由 application 从同一权威设置快照解析默认值和目的地；显式值优先，null/非法值拒绝。`compressionMode`合法值为store/fast/level3/level5/level7/maximum，分别对应0/1/3/5/7/9级，旧值含义保持。
接受后持久固定有效选项，省略参数的同幂等键重放沿用原选项、不重读变动默认；显式改变已接受选项返回幂等冲突。
旧已接受请求省略 Library 时保持 false；false 选择保留既有指纹，true 加入内容选择指纹。
Library=true 只启用标准根 Library 缓存，false 保留原排除和 LastSceneManagerSetup 规则；其他路径/链接/配额门禁保持。
Archive v1 manifest、BackupRecord形状保持；State18仅扩展备份压缩等级约束，不增加平行归档格式。
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
detail。Restore dispatcher/worker/client/CLI 已实现并按协商广告，仍只支持注册且重新验证的原生归档恢复到不存在的新项目目标，
不支持 overwrite/merge 或任意ZIP。当前State18保留原恢复证据，Library缓存按归档内容恢复，不替代M11真实迁移或v3对齐验收。

## 22. M6 兼容增加：Extension Runtime

M6 在 RPC major 1 上实现生命周期与授权变更；M7 H4 兼容增加只读授权 capability：

| capability | method | permission |
|---|---|---|
| `extensions.lifecycle.v1` | `extensions.list/get/planInstall/applyInstall/enable/disable/planUninstall/applyUninstall` | read 使用 `extensions.read`；mutation 使用 `extensions.manage` |
| `extensions.permissions.v1` | `extensions.setGrant/revokeGrant` | `extensions.permissions.manage` |
| `extensions.permissions.read.v1` | `extensions.permissions.list` | `extensions.read`，且当前 Principal 拥有目标 ExtensionId |

完整 DTO 与 bounds 由 `m6-extension-runtime.schema.json` 冻结。install/uninstall 先创建永久 immutable Plan；Apply
复验 package/source/signature/publisher/trust/revision/digest 并返回 OperationId，不重新 Plan。uninstall 默认
`retain_data`；`delete_data` 必须在 immutable Plan 中显式选择。
Plan 的 `requiredPermissions` / `optionalPermissions` 直接投影冻结 evidence，每组最多 64 项、每项最多 128 字符；
不从当前源文件、GUI 输入或已授权 scope 推断声明，不自动授权。

`extensions.permissions.list({extensionId,cursor?,limit?})` 返回
`{extensionId,grantRevision,requiredPermissions,optionalPermissions,grants,nextCursor?}`，其中 active `grants` 只含
`{permission,resourceKind,resourceId}`。默认 limit 100、允许 1–1000；三字段按二进制顺序 keyset 分页。
声明来自与当前安装身份匹配且已有 `state_committed` 证据的已应用 install Plan；一个只读事务同时取得当前记录、
冻结声明及有效授权，不读取撤销历史、凭据、Host 身份或扩展数据，也不创建持久查询会话。
`nextCursor` 是最多 4096 字符的版本化 opaque 字符串，绑定 ExtensionId、grantRevision、record revision、
package/manifest/component 摘要、安装时间与对应 install Plan。授权或包/记录快照改变时后续页返回
`revision_conflict`，要求重读；无效或跨 ExtensionId 游标返回 `invalid_request`。调用方不得拼接不同快照的页。

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
对应 capability 全部生产可用后返回 `{major:1,world:"alcomd:extension/extension-v1@1.0.0"}`。
State v8 是该功能的引入版本；当前 hello 报告 ready Store 的实际 State v18。旧 client 必须忽略未知 optional field。

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
action、replay 与 renderer state 都不持久化。State v9 是该功能的引入版本；当前 hello 报告 ready Store 的实际
State v18，并只在 client 请求时回显已实现的 `extensions.ui.portable.v1` capability。

## 24. M7：Official GUI settings

M7 保留设置 base methods 与独立 `unity.hub.open.v1`。所有者于2026-11-05批准撤销日志、活动记录及提示系统，这是未发布RPC major 1的破坏性删除：`activity.list/get`、`diagnostics.list`、`diagnostics.log.read/history/health`、`diagnostics.log.v1`及其专属权限/DTO不再提供，无兼容别名；旧方法按未知方法拒绝。业务Operation、Event与`events.live.v1`不受影响。

| method | params | result | permission |
|---|---|---|---|
| `settings.get` | `{}` | `configSchema`, `revision`, full normalized `settings`, derived `unityHub` | `settings.read` |
| `settings.update` | `expectedRevision`, closed partial `update` | new `revision`, full normalized `settings`, derived `unityHub` | `settings.manage` |
| `unity.hub.open` | `expectedRevision`, required nullable `expectedPath` | `outcome: spawned \| download_required` | `settings.read` + `unity.hub.open`, local owner only |

Config12 的 `settings.network.proxy` 是完整的代理模式对象。省略 `update.network` 保留当前偏好。
显式更新复用 settings revision CAS，保存成功后新请求采用该偏好。模式、地址边界和迁移见
[Config12](../config/settings-v12.md)。

已批准的 Unity Hub 位置合同增加 `settings.unity.hubPath: string | null`；它只保存 Hub 可执行文件
偏好，不修改该可执行文件或项目。省略保持原值，null 清除显式路径，unity 容器 null／空对象及
未知字段拒绝；无效显式路径与同行更新整次拒绝。Core 校验绝对路径、实际文件及可执行性，
不执行所选程序或验证厂商身份。公开 `unityHub` 证据包含 status、source、path、
configuredPathStatus；读取使用该响应的准确 ConfigSnapshot，派生状态不持久化、不推进 revision。
已成功提交后的证据检测失败返回 unavailable 证据，不将提交改报失败。具体枚举、4096 UTF-8
字节路径界限、三平台发现顺序、16 KiB 配置上限及尚未覆盖的 Hub 行为见
`../config/unity-hub-location.proposal.contract.json` 和 `../config/settings-v8.md`。

settings.read/manage 明确授权的设置响应可返回所请求的 Hub 绝对路径，这是通用 redaction
denylist 的窄例外；Event和普通错误不得因此泄露路径。位置偏好本身不新增
method、capability、permission、Config version 或 State migration；严格旧客户端须更新新增的发布前响应字段。

已另行批准的 `unity.hub.open` 使用独立 capability `unity.hub.open.v1` 和权限
`unity.hub.open`；它不扩大 Project／Editor 的 `unity.launch` 或 registry 的 `unity.manage`。
仅认证的 `builtin:local-owner` 可调用，且同时要求 `settings.read`，在配置／文件／进程访问前
完成 Principal 与权限校验。closed params 的 `expectedRevision` 至少为 1；`expectedPath` 必须
提供，可为 null，否则为无控制字符的非空字符串，最多 4096 UTF-8 字节。它是当前 Core
effective path 的相等性防护，不是 executable／argv／URL 指令。

Core 在现有 settings 协调下读取当前配置、验证 revision 并重新检测 Hub，目标与 guard 不同
返回 `hub_path_changed`，不能自动打开另一个目标。可用的 configured 或 discovered 目标均可
打开，包括无效手工偏好但默认位置可用的既有 fallback。仅配置路径为 null 且当前确实未发现
Hub、expectedPath 也为 null 时返回 `download_required`，不启动进程；坏的显式路径无 fallback
返回 `hub_path_invalid`，I/O 无法验证返回 `hub_unavailable`。GUI 只在 download_required 后
通过封闭系统适配器打开固定官网 `https://unity.com/download`，Core 不联网或安装程序。

`spawned` 只证明 OS 接受创建进程，不证明 Hub 窗口就绪。实际程序仅使用 Core 解析且派发前
再次校验的路径、零参数、null 标准流，不经 shell、不接受环境覆盖。只证明当时文件可执行，
不证明 Unity 厂商身份、签名或抵御二进制替换；这是打开所有者选择的本地程序的权限。
独立 launcher port 预留最多 4 个 reaper，先成功建立 worker 再 spawn；worker 即使响应接收者
消失也负责 wait 自己的 Child，不等待无关进程，不因 GUI 断开主动 kill Hub，也不阻塞 daemon
退出。容量不足返回 `hub_open_busy`，OS 启动失败返回 `hub_open_failed`，意外内部故障使用现有
`internal_error` 和 diagnosticId。所有错误文案固定且不含路径、argv、原始 I/O 或进程详情。

此方法不写配置／项目／State，不创建 Plan、Apply、Operation 或 Event。没有持久幂等承诺；
传输结果不明不能自动重试，GUI 保留明确错误并要求确认 Hub 是否已经打开，再由所有者再次
点击形成新意图。完整请求、结果及新增错误枚举由当前 RPC Schema 冻结。

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
`project_copy_plans`、`project_copy_filesystem_journal` 与 `operations.kind='projects.copy'`；State v10 是该功能的
引入版本，当前 hello 报告 ready Store 的实际 State v18。新增 method/capability/字段是 RPC v1 兼容增加，既有 Project/Package 方法语义不变。

## 26. M7 兼容增加：Project Favorite 与 Unity product-model replacement

Project Favorite 继续使用既有 compatible addition：

| method | params | result | capability / permission |
|---|---|---|---|
| `projects.setFavorite` | ProjectId、boolean Favorite、positive exact Project revision、idempotency key | 既有 `ProjectWriteResult` | `projects.registry.v1` / `projects.manage` |

注册 Project 的 `ProjectSnapshot` 兼容增加 `favorite: boolean`；anonymous `projects.inspect` snapshot 省略该字段。
Favorite 是 registry metadata，不进入 filesystem observation 或 `snapshot_json`。Core list ordering/cursor 不变；真实 mutation
增加 Project revision并写 `project.favorite_changed`，精确 same-value request 是保存幂等结果的 semantic no-op。

早期 P5-B 的 Automatic/Explicit Editor preference 是历史实现证据，不再是当前产品合同。公开发布前的 State v14
直接删除 `ProjectEditorPreference`、`ProjectEditorSelection*` 与 `unity.projectEditor.*`，把非空 argv 迁移到
`project_unity_launch_config`；Project Unity Version 成为唯一用户级版本真相。当前 active DTO、CAS、one-shot launch 与
exact candidate resolution 由 `m5-unity.schema.json` 和 `m7-unity-project-version.proposal.schema.json` 冻结。

## 27. M7 兼容增加：Project Directory Delete

M7 在 RPC major 1 上实现并广告 `projects.delete.v1`：

| method | params | result | permission |
|---|---|---|---|
| `projects.planDeleteDirectory` | ProjectId、expected Project revision、idempotency key | immutable bounded Delete Plan、`replayed` | `projects.read + projects.delete` |
| `projects.applyDeleteDirectory` | PlanId、expected Project revision、idempotency key | durable OperationId、ProjectId、`replayed` | `projects.delete` |

调用只接受 `ProjectId`，不接受路径。外部 filesystem writer 只允许 `builtin:local-owner`；Permission
`projects.delete` 不授予 extension。Plan 有效期固定为 900000ms，只冻结 canonical root/parent、filesystem identity、
leaf、ProjectVersion marker、writer evidence、profile 与 fingerprint，不保存 recursive inventory。Apply 必须 fresh
revalidate，并返回 `projects.delete-directory` Operation。

Operation phase 固定为 `accepted`、`preflight_complete`、`quarantine_intent`、`root_quarantined`、
`registry_commit_intent`、`state_committed`、`deleting`、`cleanup_complete` 与 `recovery_required`。
`quarantine_intent` 是最后取消边界；进入后只允许 forward recovery。root 只可 rename 到同一 parent 下
`.alcomd-delete-<operation-id>.quarantine/payload/`，成功只写一个 `project.directory_deleted` Event，不同时写
`project.unregistered`。quarantine 后重建的 original path 永远不得被 recovery 触碰。

新增的 delete 专用稳定错误只有 `project_delete_plan_not_found`、`project_delete_plan_stale`、
`project_delete_source_missing`、`project_delete_source_unsafe`、`project_delete_source_changed` 与
`project_delete_recovery_required`；既有通用 `project_not_registered`、`revision_conflict`、
`unity_project_running`、`idempotency_conflict`、`permission_denied` 与 `internal_error` 继续复用。
完整 DTO、phases、path/link/mount 与 recovery vectors 由 `m7-project-delete.proposal.schema.json`、
`../security/m7-project-delete-path-vectors.json` 与 `../storage/state-v13.md` 冻结。删除事务始于 State v13；
当前 Store 初始化至 State v18 后，hello 广告 `dataSchema: 18`。新增 method/capability/字段是 RPC v1
兼容增加，不改变 unregister 的 registry-only 语义。

## 28. M7 兼容增加：Project Unity Version migration

M7 在 RPC major 1 上实现并广告 `projects.unity-migration.v1`：

| method | params | result | permission |
|---|---|---|---|
| `projects.planUnityMigration` | ProjectId、target InstallationId、expected Project revision、idempotency key | no-change 或 immutable bounded Plan | `projects.read + unity.read + projects.unity-migrate` |
| `projects.applyUnityMigration` | PlanId、idempotency key | durable OperationId、`replayed` | `projects.unity-migrate` |

Plan 只接受 verified InstallationId，target canonical version 由 Core 推导；TTL 固定 900000ms。Apply 不重新
Plan，不创建 public package child Operation，并且只允许 `builtin:local-owner` 执行外部 filesystem mutation。
版本降级默认 `supportedForApply=false`；writer state 只有 `not_observed` 可进入 Plan/Apply。

`projects.unity-migration` Operation 复用既有 Project Resource Lock，并使用 State v14 的 immutable Plan 与 append-only
journal。`preparation_intent` 或 `launch_intent` 后禁止普通取消；daemon restart 不自动 respawn、不得 kill Unity。最终成功必须
重新观察同一 Project root identity 且 canonical version exact target，随后只提交一次 Project revision `+1` 和一个
`project.unity_version_migrated` Event；Unity exit status 本身不是成功 authority。

公开 Plan/Event/error 不包含 project/executable 完整路径、argv、PID、stdout/stderr 或 private recovery locator。受控本地技术
日志可以保留诊断所需的准确绝对路径，但不得改变 RPC、Event或普通错误 的脱敏合同。完整 DTO、phase、
permission 与稳定错误由 `m7-unity-project-version.proposal.schema.json`、`state-v14-unity-project-version.proposal.contract.json`
和 M7 Unity ExecPlan 冻结。当前 Store 初始化至 State v18 后，hello 广告 `dataSchema: 18`。


## 29. M7 H2-A 兼容增加：Project package candidate evidence

项目所有者批准单个只读 `packages.queryProjectCandidates`，协商 capability
`packages.candidates.v1`；RPC major 仍为 1，不新增权限或持久 Schema。
所有调用必须同时满足 `projects.read + repositories.read + packages.read + settings.read`
及既有 Principal / Project / source scope。此查询与后续 Package Plan 使用同一应用层。

请求有 summary / versions 两种视图；closed DTO、排序、选择集合和额度见
`m7-package-candidates.proposal.schema.json` 与已批准的
`../../docs/exec-plans/M7-package-candidate-evidence-stop-a.md`。历史 proposal 文件名保留。
Latest 在确认展示来源集合中按 Core precedence 选择，Project Latest 额外检查直接 Unity 条件；
只有明确较新的项目候选产生 update.target。依赖图及实际 Apply 可行性仍由原 Plan/Apply 验证。
版本和来源身份不可分离，同 source 同 precedence build 变体在分页前判歧义。

使用已登记 Project/source/Config 快照；不得网络访问、刷新可变源、下载、写业务状态或创建
Plan/Operation/Event。无持久 snapshot/session/catalog revision。指纹和 cursor 绑定快照及查询，
每页重新鉴权；快照变化返回 `package_candidate_evidence_stale`，额度失败返回
`package_candidate_limit_exceeded`。其余权限、参数、项目/revision 错误复用已有 code。
请求最多 64 KiB、响应最多 1 MiB（实际编码且符合 envelope 后 RPC frame 边界），每页默认64/最大128，
显式 IDs 最多256，最多4096来源/100000相关版本；完整小页和 cursor 可替代超大页，禁止静默截断。

这是兼容增加：既有 repository lexical cursor、source selector、版本范围、来源优先级、resolver、
Plan/Apply DTO/结果/错误及 permissions 全部保留。候选 capability 缺失时 GUI 不自行推导 Latest。

摘要的 `providers` 只增加必要的完整来源归属证据（现有 source selector + sourceRevision，
最多4096、遵守已有响应字节上限），避免用获胜来源冒充完整来源集合或为来源筛选拉取全版本。
它包含确认可见的非获胜/未知来源，不受单行显式来源选择缩窄；不改变 resolver 来源策略。

候选摘要按 Core 的包 ID 顺序分页，并从同一快照返回可选的 `displayName` 和 `links`。
Core 优先选取可见来源中与已锁定版本完全一致的记录，否则按项目候选、一般候选选择名称。
已锁定包的链接仅取自精确版本，同优先级有多个来源时省略链接。未安装包使用所选候选的链接。
Core 校验 HTTP(S) 地址，去掉含凭据或无效的链接。

摘要请求可带 `view.filter`。Core 对 `search`（最多 256 个 Unicode 标量值）按包 ID、包显示名和可见来源的登记名称做不区分大小写的匹配，也可按 `status` 和 `source` 筛选。各条件同时生效。来源匹配覆盖完整的可见提供者，包括未被选中的提供者。`missing` 要求包未安装、没有可见提供者且 Core 明确返回无候选。Core 在分页前筛选，保留摘要原顺序。游标绑定筛选条件，快照标识仍与完整清单一致。

Package mutation 与候选查询的 source DTO 分别遵守各自冻结 Schema。P6
`m7-p6-package-closure.proposal.schema.json` 的 source selector 使用
`repositoryId`/`userPackageId`，reinstall selection 使用 `packageIds`，bulk intent 使用
`packageId`/`versionRange`/`includePrerelease`；kind 值不变。候选查询使用独立
`CandidateSource`，保持 `repository_id`/`user_package_id`，请求和响应均如此。
2026-10-31 项目所有者批准将 P6 Rust 序列化修正为其冻结 camelCase 合同；这是对当前实际
P6 snake_case wire 的直接替换，不接受旧字段或混合格式，不增加别名、capability 或 RPC major。

## 30. M7 H2-B：仓库列表文件交换

项目所有者批准在 RPC major 1 增加 `repositories.list-files.v1` 和 `repositories.list-import.v1`，
使用 `m7-repository-list.schema.json` 的独立公开 DTO。列表导入复用仓库登记事务和事件。

| Method | Params / Result | Access |
| --- | --- | --- |
| repositories.inspectListFile | inspectListFileParams / inspectListFileResult | 已认证 local-owner + repositories.read |
| repositories.importListFile | importListFileParams / importListFileResult | 已认证 local-owner + repositories.read + repositories.manage |
| repositories.cancelListRegistration | cancelListRegistrationParams / cancelListRegistrationResult | 已认证 local-owner + repositories.manage |
| repositories.previewListExport | previewListExportParams / previewListExportResult | repositories.read |
| repositories.exportListFile | exportListFileParams / exportListFileResult | 已认证 local-owner + repositories.read |

输入为最多 1 MiB、4096 个非空条目的 UTF-8 文本，接受可选 BOM、LF/CRLF、trim 与 `#` 行内注释。
普通 HTTP(S) URL 与 `vcc://vpm/addRepo?url=...` 可以成为 candidate。添加链接可通过 `headers[]`
为内层 HTTP(S) 仓库携带请求头，预览只显示是否配置认证。重复来源标为 duplicate，无效或不支持条目
保留行号和稳定原因。含 userinfo 或无法完整表达的认证项不降级为匿名 URL。不返回或记录原始无效行和认证秘密。
`inspectListFile` 只读解析并返回文件精确
字节的 SHA-256 fingerprint，不登记、不下载。
GUI 确认一次后，将选定路径、预览 fingerprint 与本次幂等键交给 `importListFile`。
Core 复用相同读取和解析边界，文件内容变化时返回 `repository_list_changed`，下载和登记均未开始。
Core 跳过已登记项，在共享的 64 路下载额度内滚动下载和解析其余候选，按原列表顺序登记。每个不同的有效来源返回一项，
包括已登记项。结果含原文件行号与 added、existing、failed、unknown 或 unprocessed 状态，
失败或未知项带有界错误码。结果最多 4096 项，继续受 RPC 帧限制。
`cancelListRegistration` 按本次幂等键请求停止接纳新条目。`stopped: true` 表示 Core 已记录停止，
包括停止请求先于导入请求到达的情况。已开始项按原流程完成，原导入请求返回逐项结果。

导出预览基于 Core 一致的用户仓库快照，范围独立于 GUI 当前页，排除 VRChat 官方及精选内置仓库。
fingerprint 绑定输出字节数、范围、修订、内部凭据引用及排除项。执行时变化在创建文件前返回
repository_list_changed，要求重新预览。
无凭据的仓库直接写入完整 URL，保留查询参数及顺序。认证仓库由 Core 从凭据存储读取请求头，
写入含明文凭据的 VCC 链接。GUI 提示并取得显式确认，RPC 响应和日志不返回明文，不自动上传。
本地或不支持的来源逐项列出排除原因。

仅允许新建输出文件：已存在目标返回 repository_list_target_exists 并保持原文件；Core 数据和
已登记项目内目标拒绝。写入失败只清理本次创建且身份匹配的半成品，不删除其他文件；保存选择
取消不调用写 RPC。稳定错误另包括 repository_list_invalid、repository_list_target_invalid、
repository_list_io_error；普通错误与日志均不包含原始秘密。

## M7 实现依据整改：创建版本与兼容语义（2026-11-05已批准）


包/锁依赖的普通范围与项目直接依赖范围分开；普通完整裸版本为exact，项目直接依赖单版本沿有据的minimum语义。unityRelease补充unity最低版本判断，Core候选与Plan一致；无unity时不施加此门槛。权限、来源、强摘要、Plan/Apply和启动exact匹配保持。

本轮PackageChangeSet formatVersion=3固定新的范围上下文及保留锁元数据写出依据（Schema同时描述历史1/2）。1/2未接受Plan返回既有plan_stale；已接受任务保持原冻结摘要恢复，禁止自动重签。此依据标记不改变SQL结构、权限或请求幂等语义。

## M7 单页业务日志（2026-11-06，所有者已批准实施）

范围和存储以`docs/exec-plans/M7-official-gui-extension-ui.md`的“单页业务操作日志”为准。这是向RPC v1新增只读方法和经能力协商的失效范围，不恢复diagnostics/activity接口。

- 新增`logs.read`，能力`logs.read.v1`，仅authenticated local-owner且有既有`operations.read`权限，按owner隔离，无公开写入方法。
- params：可选`beforeSequence`、`afterSequence`互斥，limit默认100、最大200；无位置读最近记录，before向旧、after向新。所有位置为非负JSON安全整数。
- result：`entries`始终sequence升序；`highWatermark`为同一数据库读取快照中该owner的最新日志sequence；`hasMore`及有剩余时的`nextSequence`支持内部续读。before的next为本批最小sequence，after的next为最大sequence；无位置按before处理。单条512KiB、帧4MiB，按完整条目编码大小控制批次，不截断或跳过剩余条目。
- 每行保存稳定`action`/`step`、实际发生时的`outcome`、目标快照、真实必要详情，GUI负责标签本地化。sequence和entryId跨重启保持；不从RPC响应、事件投影或GUI提示生成日志。
- `actor`是发起入口（GUI、CLI、MCP、Local API、Core或Local IPC），沿用`system.hello`的客户端名称元数据绑定到连接，不代表权限或自然人身份。任务执行及恢复沿用原发起入口，取消请求记录实际提交入口。
- 既有`events.batch.invalidatedScopes`拟增加`operation-log`；由Store提交后的日志水位变化唤醒，共用订阅/ACK/取消和重连，不新增计时轮询或第二条页面通知连接。失效不包含日志正文、不授予读取权限。
- 页面无用户分页，自动按滚动范围续读；历史仍在Core，展示缓存有界。读取失败按现有错误响应处理，不能返回空成功；Core离线保留已读内容并明确过期/重试。

以下仅为预期数据形状，非实际运行记录：

```json
{
  "sequence": 4,
  "entryId": "11111111-1111-4111-8111-111111111111",
  "executionId": "22222222-2222-4222-8222-222222222222",
  "actor": "GUI",
  "occurredAtMs": 1793937603000,
  "action": "projects.register",
  "step": "read_manifest",
  "outcome": "failed",
  "target": {
    "kind": "project",
    "name": "项目A",
    "path": "C:\\ALCOMD-Test\\项目A\\Packages\\vpm-manifest.json"
  },
  "details": "Failed to read project manifest: C:\\ALCOMD-Test\\项目A\\Packages\\vpm-manifest.json\ncaused by: access denied (os error 5)"
}
```

owner仅为内部归属，不在条目中暴露；operationId仅关联已接受任务时存在。数据库不可用无法保证写历史，不能由前端补造。普通业务RPC错误不因此增加任意技术正文。
