# State35 · Extension MCP management grants

Approved by the owner on 2026-11-14 after the real State34 Core rejected the new extension grant.
This explicitly supersedes the management slice's earlier assumption that no State change was needed.

One IMMEDIATE transaction rebuilds only `extension_grants` to extend its CHECK constraints with the
existing `mcp.clients.read/manage` permissions and their exact `Global/*` combination. All old columns,
the primary key, foreign key/CASCADE, records, states, revisions and timestamps are retained. The old
background and project combinations retain their previous restrictions. No new permission name or
Config change; no client, credential, instance, data, Operation, event or sequence changes.

Create the replacement table, copy explicit columns, drop/rename, set user_version35 and commit.
Any failure rolls the whole transaction back. Historical migrations, including 0008/0033/0034, remain
unchanged. A newer data version is rejected by old Core. This slice uses disposable data only and
does not upgrade or replace the owner's original running data/programs.
