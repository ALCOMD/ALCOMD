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

每条连接第一条有效请求必须为 `system.hello`。M1 只支持 major 1，客户端 metadata 不参与
授权，未知 capability 可安全忽略。M1 只实现 `system.hello` 与只读 `system.status`；hello 不
暴露尚未实现的 data/config/extension Schema 版本。

响应允许兼容增加未知可选字段和 capability，客户端必须忽略；删除字段、改变既有字段
类型/语义或改变既有方法语义才要求提升 RPC major。完整线合同与错误表由
`specs/rpc/alcomd-rpc-v1.md` 和对应 JSON Schema 定义。

M2 以兼容方式增加 `state.check`、`operations.get`、`operations.list`、`operations.cancel` 与
`events.list`。对应 capability 固定为 `state.check.v1`、`operations.v1` 与
`events.replay.v1`；方法只有在本连接 hello 协商到所需 capability 后才可调用，否则返回
`capability_required`。

store 成功初始化后，hello result 可选增加 `dataSchema: 1`。这是诊断/兼容信息，不替代 RPC
major 或 capability 协商；不得同时虚构 `configSchema`/`extensionApi`。旧 M1 客户端必须继续
能够忽略该字段并调用 `system.status`。

M2 仍不增加 notification、batch、server-initiated request 或新 transport。Operation/Event 的
分页、revision、幂等、Principal 与稳定错误由 RPC v1 规范和对应 JSON Schema 冻结。

## 结果

RPC v1 独立于应用版本。新增 method、capability 或可选响应字段不提升 major；破坏性变化提升
RPC 大版本。M1 基础合同继续有效；M2 只增加已批准的可选字段、capability、method 与 DTO。
