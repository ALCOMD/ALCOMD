# Transport observation Host interface

`alcomd:mcp-transport/observations@1.0.0` is an independent read-only WIT snapshot
interface. It exposes the bounded projection in `specs/rpc/m8-mcp-transport-v1.md`
and the adjacent closed JSON Schema; previous WIT interfaces are unchanged.

Core validates the current interactive Client + Extension authority, extension
ownership, active UI invocation, deadline, lease, lifecycle and grant revision.
The three read groups require their corresponding permissions independently;
missing groups are omitted, not exposed as empty readable groups. Background,
UI-close and MCP tool invocations cannot use this management read bridge.
The host cannot submit reports, bind tickets or claim a Principal through WIT.

Threat boundary: a protected local reporter can lie about transport activity, so
all rows retain adapterReported source and explicit freshness/coverage. Neither
these reports nor a successful read authorizes business. Core alone authenticates
bound identities. No ticket, credential, request content or tool result crosses
the extension bridge. Partial, stale and unavailable states remain distinguishable.
The existing Core IPC observations retain their separate authoritative meaning.
Extension manual review remains deferred by the owner; no claim of that acceptance.
