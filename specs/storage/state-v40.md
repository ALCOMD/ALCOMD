# State40: extension Core-observation read grants

Approved by the owner on 2026-11-17. Config13 is unchanged.

Migration 0040 runs in one transaction and only rebuilds `extension_grants` to add
`mcp.connections.read` and `mcp.requests.read`, each with exactly `Global/*`.
All previous columns, rows, keys, foreign keys, revisions and applicable constraints
are preserved. No grant is created or widened. No observation table or history is
persisted, and no old migration is edited. Original running data is not upgraded.

Core observations live only in Core memory and disappear on restart. State40 does
not add administrative permissions to external MCP business grants or OAuth scopes.
Failure before commit rolls back the table, data and schema version; reopening does
not repeat the migration. Older Core rejects State40 data.
