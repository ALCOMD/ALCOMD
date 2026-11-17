# MCP tools-list invalidation

Owner-approved on 2026-11-17; frozen MCP revision 2026-07-28.

`mcp.tools.catalog` now returns `catalogFingerprint`, including on named lookup
and every page. Core signs the complete authorized catalogue, current caller
authorization binding, grant revision and provider lifecycle/package contents.
It is an opaque comparison value, not an authorization or a persistent session.
The new required result field breaks older strict DTO readers: upgrade matching
Core/client/MCP binaries together; no legacy-result fallback is introduced.
`mcp.tools.notifications.v1` advertises this support only after Core negotiation.

STDIO and stateless HTTP accept `subscriptions/listen` only for
`notifications.toolsListChanged=true`. Unsupported filters are excluded; an empty
accepted filter is InvalidParams. The SDK sends the acknowledged filter and
subscription ID before any invalidation. Each stream takes a pre-ack baseline,
queries Core immediately after ack and at most once per second thereafter, and
sends only changes. Failed authentication or a failed query terminates the stream;
it never represents an empty catalogue. There is no notification history/replay.

Each adapter/authentication context allows four active streams. Establishment is
bounded to ten seconds, lifetime to ten minutes, and pending invalidation output
to one awaited send with ten-second backpressure. HTTP retains 32 TCP connections
and 16 active requests/streams, with 15-second SSE comment heartbeat. A stream's
idle wait does not use ordinary response's ten-second total output deadline.
Ordinary requests keep their existing limits. Per-socket stalled writes still
expire after ten seconds. HTTP has no protocol session, GET/DELETE or resumption.

Cancellation, EOF, credential rotation, revocation or expiry ends only the
protocol request/stream. Already accepted Core Operations remain owned by their
actual Principal and continue under the existing recovery contract. No business
write is automatically retried or cancelled. No new dependencies, permissions,
State or Config are introduced by this subscription contract.
