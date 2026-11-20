# MCP client identity v1

`mcp.clients.v1` provides `mcp.clients.list`, `create`, `issueCredential` and
`revoke`. A successfully authenticated client can use all currently published
MCP tools. The same rule applies to STDIO, local HTTP Bearer and HTTPS OAuth.
Each connection uses the registered `mcp:<ClientId>` Principal. Tool providers
still need their own extension permissions. Business validation and operation,
plan and idempotency ownership stay in Core.

Client administration checks `mcp.clients.read` or `mcp.clients.manage` for
the authenticated principal. Credential issuance returns the plaintext credential once. The store
persists its digest and a nonsecret issuance receipt. Rotation and revocation
take effect on subsequent requests even on an open connection.

The [RPC Schema](m8-mcp-clients-v1.schema.json) defines the client and credential records.
