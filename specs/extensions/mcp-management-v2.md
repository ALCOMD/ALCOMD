# MCP management Host interface v2

`alcomd:mcp-management/clients@2.0.0` provides list, create and revoke.
The [WIT](wit/mcp-management-v2/management.wit) and
[Host Schema](mcp-management-v2.schema.json) define exact records and errors.
Each request or response is limited to 65,536 serialized UTF-8 bytes.

Listing requires `mcp.clients.read` on both the interactive client and
extension. Create and revoke require `mcp.clients.manage` on both. Render
only reads. Actions are rechecked against the active UI session, extension
lease and current permissions in the acceptance transaction. The read-only
`can-manage` field controls presentation. Writes use the extension Principal
for idempotency. Credential issuance remains in the local-owner CLI.

The client view shows connection and revocation state. An authenticated MCP
client can use all published tools. The previous grant, project picker and
grant modification operations have been removed from this interface.
