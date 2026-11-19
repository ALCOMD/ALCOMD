# MCP client identity v2

`mcp.clients.v2` provides `mcp.clients.list`, `create`, `issueCredential` and
`revoke`. A successfully authenticated client can use all currently published
MCP tools. The same rule applies to STDIO, local HTTP Bearer and HTTPS OAuth.
Each connection uses the registered `mcp:<ClientId>` Principal. Tool providers
still need their own extension permissions. Business validation and operation,
plan and idempotency ownership stay in Core.

Client administration uses the existing permissions. Credential issuance
requires the local owner and returns the plaintext credential once. The store
persists its digest and a nonsecret issuance receipt. Rotation and revocation
take effect on subsequent requests even on an open connection.

Client records and creation parameters omit the former grant fields.
`mcp.clients.setGrants` was removed. See the [RPC Schema](m8-mcp-clients-v2.schema.json)
and the [State44 migration](../storage/state-v44.md).
