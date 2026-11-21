# ADR: ALCOMD RPC

- 状态：Accepted
- 日期：2026-08-14

## 背景

参见 `docs/architecture/ALCOMD-V4.md`。


## 决策

官方组件使用 Named Pipe / Unix Domain Socket 上的版本化本地 RPC。公共 DTO 与领域对象分离。

M1 冻结的 RPC v1 基础合同为：`u32` little-endian 长度前缀、最大 4 MiB UTF-8 JSON payload，
以及 JSON-RPC-inspired、但不宣称兼容 JSON-RPC 2.0 的 request/result/error envelope。request ID
是最多 64 bytes 的非空字符串，公开错误以稳定字符串 `error.code` 为机器可读事实来源。

framing error（零长度、超限、截断或无法形成完整 payload）直接关闭连接且不生成 RPC 响应；
完整 payload 的 JSON、envelope 或 params 错误返回结构化 `invalid_request`。

每条连接以 `system.hello` 开始，客户端 metadata 仅用于诊断。Core 协商 RPC major 1，并按当前握手 Schema 返回 `dataSchema`、`configSchema`、扩展 ABI 主版本与基础 WIT world（`extensionApi`）以及实际可用能力。未协商的能力不能用于调用。

首发前所有 RPC 变更直接修订版本 1。字段的必填性、扩展性和校验错误按 `specs/rpc/alcomd-rpc-v1.md` 和对应 JSON Schema 执行。

状态管理提供 `state.check`、`operations.get`、`operations.list`、`operations.cancel` 与
`events.list`。对应 capability 固定为 `state.check.v1`、`operations.v1` 与
`events.replay.v1`；方法只有在本连接 hello 协商到所需 capability 后才可调用，否则返回
`capability_required`。

Core 在存储和配置就绪后返回握手结果。Operation 与 Event 的分页、修订、幂等和稳定错误依据当前 RPC 规范及相应 JSON Schema。

## 结果

RPC v1 独立于产品版本。首次正式发布前，所有变更统一写入 RPC 1。
