# ADR: 客户端身份与权限

- 状态：Accepted
- 日期：2026-08-14

## 背景

参见 `docs/architecture/ALCOMD-V4.md`。


## 决策

每个外部应用、MCP 客户端和扩展拥有独立 Principal 与最小权限。

## 结果

授权可审计、可撤销，不提供全局万能 token。
