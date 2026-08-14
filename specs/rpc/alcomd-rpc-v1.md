# ALCOMD RPC v1

状态：Draft

## 目标

在当前用户范围内，为官方客户端与授权第三方客户端提供稳定、可版本化、可审计的核心访问协议。

## 传输

```text
Windows Named Pipe
Unix Domain Socket
```

官方客户端不通过固定 TCP 端口连接。

## 帧

初始方案：`u32` 小端长度前缀 + UTF-8 JSON 消息。最终帧格式在 M1 ADR 中冻结。

## 语义

采用 JSON-RPC 2.0 风格：

- request ID。
- method。
- params。
- result / error。
- server-to-client notification。

协议可以借鉴 JSON-RPC，但必须明确记录任何偏差。

## 握手

第一条请求必须为 `system.hello`。

请求 Schema：`system-hello.request.schema.json`
响应 Schema：`system-hello.response.schema.json`

## 错误码

最低集合：

```text
rpc_version_unsupported
component_upgrade_required
authorization_required
permission_denied
invalid_request
resource_conflict
revision_conflict
operation_not_found
operation_requires_input
operation_cancelled
data_migration_required
internal_error
```

## 幂等

可能重试的写命令必须接受幂等键。核心在有限时间内缓存执行结果，避免断线重试造成重复安装或重复删除。

## Revision

可变资源返回 revision。修改请求可携带 `expectedRevision`，冲突时返回 `revision_conflict`。

## 事件

客户端显式订阅事件，并能从 sequence 断点继续。事件必须区分：

- 资源状态变化。
- Operation 进度。
- 审批请求。
- 组件状态。
- 扩展生命周期。

## 安全

- IPC 端点只允许当前用户访问。
- 不信任客户端传入路径。
- 每个连接绑定 Principal。
- 所有写操作重新检查权限。
- 敏感字段在日志中脱敏。
