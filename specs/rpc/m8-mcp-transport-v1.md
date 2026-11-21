# Transient adapter-reported MCP observations

Approved A/B contract, 2026-11-17. DTO/limits are in the adjacent JSON Schema.
`mcp.runtime.snapshot` remains a separate authoritative Core IPC projection.

Protected native IPC hello with explicit `mcp-transport-observer`, empty credential
and no resource binds only `transport:mcp-transport-observer`. It negotiates only
`mcp.transport.report.v1` and can only submit `mcp.transport.report`. It has no
owner fallback, business, read or administrative authority. Native owner cannot
use this restricted role to bind a client. External MCP authenticates normally.

A report replaces the reporter's full memory snapshot atomically, no more than
once per second: positive safe-integer sequence, fixed transport kind, local UUID
references, phases, four fixed MCP methods and accepted toolsListChanged streams.
Reject replay/out-of-order, malformed relationships, phase regression and any
extra identity/body/secret field. No URL, port, JSON-RPC ID, arguments, results or
headers are collected. Core assigns public IDs/times. Local references are not
authentication. Each channel is limited to 128 connections, 128 requests and 32
streams, 64 KiB; Core allows 64 reporters and 4096 rows with explicit incomplete
coverage rather than silently presenting a complete empty snapshot.

New requests receive one random 256-bit, ten-second binding ticket once. Only
`mcp.transport.bindRequest({ticket})` on a separately, currently authenticated MCP
Core connection can attach that actual Principal; no caller-supplied client ID.
The ticket is memory-only, one-use, removed with the request, never logged, sent
to an extension, persisted in idempotency responses, or returned by snapshot.
After an established report channel fails or its response is lost, the observer
stops until adapter restart; it does not reconnect/re-report active requests to
obtain new tickets. Core forgets that channel, so coverage is explicitly unavailable.
Initial connection attempts may wait for Core without starting it. Reporting/binding failure never
retries business or cancels accepted Operations. Adapter binding work is bounded
to four in-flight best-effort tasks; unbound sampled rows remain explicitly unknown.

`mcp.transport.snapshot({clientId?,limit?})`, capability `mcp.transport.read.v1`,
returns independent permission-filtered groups and Core instance/time. Existing
connections/requests read permissions plus `mcp.subscription-streams.read`, all
Global/*, govern each group. No available group returns permission_denied. The same
permissions apply to authenticated GUI, CLI and MCP clients. An extension must
also have the corresponding grants. All rows are `source=adapterReported`.
A clientId is Core-authenticated only after binding. An HTTP socket has no client
owner: filters match bound current requests/streams and their related socket.

Per-reporter age over two seconds is stale, five seconds expires rows/tickets,
and disconnect forgets the channel. Restart clears everything. Missing/stale
coverage is not "zero activity". Queries default to 25 rows/group (1–100), with
counts/truncation and a total 64 KiB response bound. This is sampled current state,
not a complete history of fast MCP requests or a persistent subscription system.
