# MCP HTTPS OAuth v1

Owner approved 2026-11-15; exact limits and Core contracts are in the M8 ExecPlan and
`../rpc/m8-mcp-oauth-v1.{md,schema.json}`. Frozen MCP revision is 2026-07-28.

`alcomd-mcp serve-oauth --port <nonzero-port> --tls-cert <certificate.der> --tls-key <key.pkcs8>`
binds only 127.0.0.1. Issuer is `https://localhost:<port>`, resource is exactly its `/mcp`.
The certificate must name localhost and match the PKCS8 private key. TLS 1.3/HTTP1.1 is used.
Trust and certificate validity are checked by each client; server startup cannot infer all
client trust stores. No CA creation, OS trust change, verification bypass or HTTP fallback.
Tests trust only a public disposable end-entity fixture in their own client.

Reuse the existing controlled alcomd-client daemon-start policy, with authentication already
bound before connecting; never retry as native owner. `--no-start-daemon` disables automatic
startup. Isolated/manual validation **must** use this flag and an already identified Core with
an explicit disposable data directory. It must not auto-start a default-data Core if that
isolated instance exits. Existing `serve` and STDIO profiles are unchanged.

| Route | Contract |
| --- | --- |
| GET `/.well-known/oauth-protected-resource/mcp` | Exact resource, one AS, header Bearer, `mcp` connection scope |
| GET `/.well-known/oauth-authorization-server` | Exact issuer, code/refresh, S256, none/client_secret_basic, RFC9207 iss; no OIDC/CIMD/DCR claims |
| GET `/authorize` | Pre-registered client, exact callback/resource, S256, scope/state; create bounded Core pending request |
| GET `/authorize/result` | Original Secure/HttpOnly host cookie proves browser flow; pending stays visible, owner-approved response redirects once with code/state/iss |
| POST `/token` | Form-encoded code or rotating refresh exchange; Basic for confidential clients; no client_secret_post, implicit/password/client_credentials |
| POST `/revoke` | RFC7009 client-authenticated family revocation; unknown tokens do not disclose existence |
| POST `/mcp` | All published SDK tools, independent OAuth-authenticated Core connection for every request, no protocol session |

Owner pre-registration and consent use formal CLI plus existing Core permissions and local-owner
identity. The owner approves or declines the whole connection. The `mcp` scope provides all
currently published tools. Core checks client status, credential generation, expiry and
revocation on each request. Project continuation cursors carry Core-issued authorization_binding,
tied to the token, resource, current client revision and position. Missing, altered and
cross-token bindings return revision_conflict. An independent private Core process key
authenticates them. Core restart requires a new first page.

Registered callback queries cannot contain OAuth response keys code/state/iss/error/
error_description/error_uri, preventing duplicate response parameters. Registered redirects
are at most16, 2048 bytes each, with a32768-byte serialized collection budget. List pages retain
revoked history, fingerprint the complete metadata snapshot, and obey both count and64KiB
response budgets. No secret/hash is included in metadata. Reject mixed-snapshot cursors.

Access tokens last at most10minutes, refresh inactivity7days/absolute family30days, code60seconds,
pending10minutes with32 global/4 per-registration active flows. Completed flows release admission
capacity. Public authorization traffic is bounded at60/minute overall and10/minute per bucket;
the result-continuation bucket conservatively shares10/minute across registrations. TCP32,
active requests16; TLS/header10s, body15s, Core60s, output10s. Headers16KiB/32fields; AS query/form
16KiB, metadata64KiB, MCP request/response budgets unchanged.

Host is the configured localhost authority, with ordinary default-port443 HTTP normalization;
issuer/resource remain exact configured identifiers. Reject forwarded authorities, aliases,
invalid/ambiguous Host, duplicate fields and controls. AS permits absent Origin or exact same
origin; MCP resource rejects every Origin. No CORS/browser-JS profile, remote listen or proxy.

The 401 challenge advertises PRM, the `mcp` scope and invalid_token where applicable.
Core business denials remain tool errors. OAuth errors are fixed bounded tags. All responses
are no-store/no-referrer. Waiting HTML is semantic, script-free and has no third-party resources.

HTTP/TLS cancellation, timeout and connection loss cancel only the protocol exchange. Accepted
Core Operations keep their caller, execute independently and remain recoverable. Lost business
write responses retain core_write_outcome_unknown and explicit original-key recovery. Code/
refresh/client-secret responses cannot be replayed or automatically issued again after loss.
An indeterminate token response returns503 temporarily_unavailable with a fixed instruction
to start authorization again; it does not claim that the exchange rolled back. A consumed
refresh still triggers family revocation when replayed after its own idle deadline, until
the family's absolute expiry.

CIMD/DCR automatic registration, external AS integration, remote deployment and automatic
certificate provisioning are not implemented or advertised. This profile is usable by clients
that support the frozen MCP protocol and pre-registration, not every arbitrary MCP client.
