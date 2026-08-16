# MCP 协议基线

冻结规范：[`2026-07-28`](https://modelcontextprotocol.io/specification/2026-07-28)

以下行为以该官方规范为准：

- 无协议级 session。
- 不使用旧 `initialize` / `notifications/initialized` 生命周期。
- 每个请求在 `_meta` 中携带协议版本和客户端能力。
- 支持 `server/discover`。
- Streamable HTTP 不使用 `Mcp-Session-Id`。
- 订阅使用规范定义的长连接请求流。
- 长任务是否采用官方 Tasks 扩展必须另行决策。

`alcomd-mcp` 是协议适配器，不持有业务状态。核心长任务的权威状态始终是 ALCOMD Operation。
