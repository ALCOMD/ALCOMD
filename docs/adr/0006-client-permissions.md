# ADR: 客户端身份与权限

- 状态：Accepted
- 日期：2026-08-14

## 背景

参见 `docs/architecture/ALCOMD-V4.md`。


## 决策

每个外部应用、MCP 客户端和扩展拥有独立 Principal 与最小权限。

M2 引入内建 `builtin:local-owner` Principal，仅代表当前同用户官方客户端的 bootstrap
Principal，不代表任意同用户进程已经完成可信身份认证。M2 权限名固定为：

- `state.check`
- `operations.read`
- `operations.cancel`
- `events.read`

每个 application 用例必须重新执行 permission 与 owner/visibility 校验。Operation、Event 可见
范围和 idempotency scope 均绑定 Principal；client metadata、SID、pipe 名和 OperationId 都不是
授权凭据。

M2 synthetic Principal 只用于隔离测试。真实 credential/pairing/revocation 仍属于后续里程碑，
`access.principal-revocation` 不得因 M2 基础隔离测试而整体标为 implemented。第一个产生项目、
包或外部文件副作用的写 RPC 出现前，必须先冻结真实 Principal credential 合同；后续业务权限
不会自动授予 `builtin:local-owner`。

## 结果

授权可审计、可撤销，不提供全局万能 token。
