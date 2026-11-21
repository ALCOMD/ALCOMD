# MCP runtime Core observations v1

Owner-approved on 2026-11-17. Exact fields and limits are in the adjacent Schema;
implementation and evidence belong to the M8 ExecPlan. Capability
`mcp.runtime.read.v1` is advertised only after real wiring has been verified.

`mcp.runtime.snapshot({clientId?,limit?})` reads only verified Core IPC connections
and in-flight Core RPCs for MCP Principals. It does not observe idle HTTP sockets,
SDK queues, protocol sessions or subscription streams. A connection proves that a
previously authenticated IPC still exists, not that current business authorization
is valid. Core reauthenticates the caller and checks its permissions on each request.

Read groups require `mcp.connections.read` and `mcp.requests.read` independently.
An absent permission omits its group and returns its canRead flag false; no permitted
group returns the existing permission_denied. Filters cannot supply an identity.
Authenticated GUI, CLI and MCP clients use the same Core observation permissions.
When an extension reads these groups, the Host checks both principals, the
extension declaration, its lease and lifecycle and the current UI authority.

Default limit25, range1–100 per group; a response is at most65536 serialized UTF-8
bytes. totalTracked refers to matching tracked rows; truncated reports row/byte
limits. Core tracks at most4096 connections and one call per tracked connection;
coverageIncomplete reports untracked live connections without rejecting business.
Rows sort by start time then generated ID. Every refresh is a new current snapshot;
there are no persistent query sessions, cursors or completed-request history.

No parameters, response bodies, external request IDs, credentials/hashes, headers,
paths or URLs enter the projection. Method names come from the static Core method
allowlist. Errors mean unavailable, never zero activity. Core instanceId changes on
restart and old rows must be discarded. Closing observations or a connection does
not cancel accepted Operations. Adapter-reported subscription streams have a separate
[transport observation contract](m8-mcp-transport-v1.md).
