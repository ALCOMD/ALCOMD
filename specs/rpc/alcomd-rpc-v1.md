# ALCOMD RPC v1

状态：M1 基础合同已批准；M2+ 章节仍为未来设计

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

Operation、Event、Revision、幂等写操作、Principal、权限、订阅和恢复语义属于 M2 或后续
里程碑。本文件末尾只保留其未来设计方向，不构成 M1 已冻结或已实现合同。

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

## 9. M2+ 未来设计（未由 M1 冻结或实现）

下列方向仍有效，但必须在对应里程碑另行冻结 Schema、错误和安全合同：

- 写命令幂等键与有限期结果缓存。
- 可变资源 revision 与 `expectedRevision`。
- Event 订阅、sequence 和断点恢复。
- Operation、审批、输入、取消和恢复。
- 每连接 Principal、配对、权限、撤销与审计。

M1 不得广告或返回这些能力，也不得因本节存在就把它们记为已实现。
