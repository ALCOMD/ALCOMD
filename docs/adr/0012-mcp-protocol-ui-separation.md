# ADR: MCP 协议与 GUI 分离

- 状态：Accepted
- 日期：2026-08-14

## 背景

参见 `docs/architecture/ALCOMD-V4.md`。


## 决策

`alcomd-mcp` 独立实现 MCP 协议；MCP 可视化管理作为第一方扩展。

## 结果

关闭 GUI 或禁用管理扩展不影响 MCP 协议服务。
