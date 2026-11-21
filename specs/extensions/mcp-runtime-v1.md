# MCP Core observation Host interface v1

Owner-approved on2026-11-17. The additive interface is
`alcomd:mcp-runtime/observations@1.0.0`. The `runtime-v1` world includes the
management-v1 and extension-v1 interfaces.
The single `snapshot` reads the same bounded projection as mcp.runtime.snapshot;
its fields and65536-byte budget are frozen in the adjacent Schema.

Render and action may read; background, close and tool-provider invocations cannot.
Every invocation rechecks interactive Client and Extension permissions, declared
capabilities, current Core-issued lease, grant/lifecycle revision, UI scope and
invocation cancellation/deadline. No reading creates authority for future calls.
`mcp.connections.read` and `mcp.requests.read` independently use `Global/*` and
are optional, off by default. No external MCP administration grant or token is added.

Rows are actual authenticated Core IPC and in-flight RPC metadata, not online
clients, idle HTTP sockets, all MCP requests, sessions or subscriptions. No secret,
parameter, response body, caller-selected request ID, path or URL is projected.
A query failure is unavailable, not an empty success. The database stores the granted permissions; observations remain transient and bounded. No termination,
request cancellation or Operation cancellation management function is introduced.
