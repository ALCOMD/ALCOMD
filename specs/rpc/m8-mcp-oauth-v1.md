# MCP OAuth v1

`mcp.oauth.v1` uses the existing nine RPCs and the two OAuth hello roles.
The single scope `mcp` authorizes connection to the MCP server. The owner
reviews the client, issuer, resource and callback, then approves or declines
the whole request. The review fingerprint binds current client and
registration metadata and credential generation. Review and decision have no
grant list. Registration lists retain snapshot and byte-bounded pagination.

The ingress hello can call begin, finish, token and revoke. After exact token
and resource verification, the access hello binds the registered MCP Principal.
Every request rechecks expiry, revocation and credential generation. Approval,
registration, PKCE S256, callback binding, one-time code consumption, refresh
rotation, replay-family revocation and secret handling retain their existing
checks. The adapter uses independent Core connections for requests.

Project continuation cursors carry a Core-issued MAC binding token, resource,
client revision and page position. Missing, forged, cross-token or stale
bindings fail with `revision_conflict`. After Core restarts, start again from
the first page. The key remains private to the Core process.

See the [RPC Schema](m8-mcp-oauth-v1.schema.json) and
[HTTPS profile](../mcp/http-oauth-v1.md).
