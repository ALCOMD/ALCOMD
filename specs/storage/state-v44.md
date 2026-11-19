# State44 MCP connection access

`0044_mcp_connection_access.sql` removes the MCP client grant snapshot table
and its attached triggers, the MCP client's grant revision and the OAuth
token's grant revision and grant JSON. The migration runs in one transaction.
It preserves client identity, credential digests, OAuth registration, token
lifecycle fields, Core business resources and event sequences. Extension
permission records retain their authority and schema.

The former MCP grant data has no runtime interpretation. Previously applied
migrations remain historical. State44 introduces no credential conversion.
