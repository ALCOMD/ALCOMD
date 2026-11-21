# MCP management Host interface v1

`alcomd:mcp-management/clients@1.0.0` provides list, create and revoke.
The [WIT](wit/mcp-management-v1/management.wit) and
[Host Schema](mcp-management-v1.schema.json) define exact records and errors.
Each request or response is limited to 65,536 serialized UTF-8 bytes.

Listing requires `mcp.clients.read` on both the interactive client and
extension. Create and revoke require `mcp.clients.manage` on both. Render
only reads. Actions are rechecked against the active UI session, extension
lease and current permissions in the acceptance transaction. The read-only
`can-manage` field controls presentation. Writes use the extension Principal
for idempotency. The CLI provides the credential issuance entry point.

The client view shows connection and revocation status. An authenticated MCP
client can use all published tools.
