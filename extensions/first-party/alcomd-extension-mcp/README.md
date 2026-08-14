# MCP 管理第一方扩展

扩展 ID：`com.cqmhv.alcomd.extension.mcp`

职责：

- MCP 客户端配置向导。
- 已连接客户端与活跃请求可视化。
- 权限、审批、操作进度与诊断。
- `alcomd-mcp` 组件状态展示。

本扩展不实现 MCP 协议，不直接读 token，不直接修改外部配置，也不管理 stdio 子进程。
