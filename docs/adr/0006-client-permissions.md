# ADR: 客户端身份与权限

- 状态：Accepted
- 日期：2026-08-14

## 背景

参见 `docs/architecture/ALCOMD-V4.md`。


## 决策

独立 MCP 客户端和扩展拥有各自的 Principal 与最小权限。M10 选择同系统用户的
原生直连模型。能访问该账号原生端点的 GUI 和 CLI 同样以 `builtin:local-owner`
进入 Core，无应用配对、应用级信任区分或逐应用撤销。系统账号是这一入口的信任边界。

M2 初次引入 `builtin:local-owner` 时将其用于官方客户端的 bootstrap，权限名固定为：

- `state.check`
- `operations.read`
- `operations.cancel`
- `events.read`

每个 application 用例必须重新执行 permission 与 owner/visibility 校验。Operation、Event 可见
范围和 idempotency scope 均绑定 Principal。系统依据 OS 端点访问边界建立本机所有者上下文。
自报的 client metadata、SID 字符串、pipe 名和 OperationId 均不能授予权限。

M2 synthetic Principal 只用于隔离测试。MCP 和扩展的授权与撤销由其已批准合同验收。

## 结果

原生业务仍由 Core 验证权限、revision、目标及显式确认。MCP 和扩展授权可审计与撤销。
