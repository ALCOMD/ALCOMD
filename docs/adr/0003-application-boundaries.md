# ADR: 应用层边界

- 状态：Accepted
- 日期：2026-08-14

## 背景

参见 `docs/architecture/ALCOMD-V4.md`。


## 决策

所有协议适配器调用 `alcomd-application`。领域层不依赖具体传输、存储或操作系统。

## 结果

通过 Cargo 依赖图和 CI 阻止业务逻辑回流到 GUI、CLI 或 MCP。
