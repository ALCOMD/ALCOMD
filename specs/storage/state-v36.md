# State36: Core-authoritative OAuth

Owner approved 2026-11-15. Incremental migration `0036_mcp_oauth.sql` preserves migrations
0033–0035 and every existing table/row/sequence. Run in a single IMMEDIATE transaction. Newer
schemas are rejected before migration by older binaries. Original running data is not upgraded.

Only two tables are added:

- `mcp_oauth_registrations`: OAuth client identity, FK to existing MCP client, exact issuer/resource,
  registered redirects, public/confidential client type and optional secret hash, refresh policy,
  revision and timestamps. Active Core-client binding is unique. No authority is granted by registration.
- `mcp_oauth_tokens`: SHA-256 hash and access/refresh kind, registration revision, family/parent,
  one-use code-source hash, original Core credential generation, grant snapshot revision and
  explicitly selected grant ceiling, audience/issuer and lifetime/consumption/revocation timestamps.
  Code-source uniqueness prevents repeated exchange. Family revocation and refresh consumption
  with child-token insertion are atomic. No plaintext token, code, client secret or TLS key persists.

Current Core grants remain the authority. Token grants only narrow them. Neither historical grants
nor a refreshed token can add authority. Existing credential rotation invalidates the earlier OAuth
generation; Core/client/registration/family revocation and expiration apply to future requests and
write acceptance, not retroactive cancellation of accepted Operations.

Pending browser authorization and codes are bounded volatile Core records, not persistent tasks or
protocol sessions. Restart invalidates them; persistent access/refresh state retains expiry,
consumption and revocation. Consumed refresh evidence remains through the family's absolute expiry.
Only OAuth records whose entire family has expired may be pruned. Existing business/idempotency
records are not pruned. Generic idempotency stores only non-secret registration results.

Config13 and existing permissions/WIT are unchanged. This is additive storage with no row removal;
rolling an upgraded database back to State35 is unsupported. Migration tests must preserve all old
rows, sqlite_sequence and foreign keys, verify strict constraints, rollback and reopen.
