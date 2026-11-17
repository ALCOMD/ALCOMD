# State43: subscription-stream observation read permission

Owner approved A/B on 2026-11-17. Config13 is unchanged.

Migration 0043 only rebuilds extension_grants in a single transaction to permit
`mcp.subscription-streams.read` with exactly Global/*. All old columns, keys,
foreign keys, records, states, revisions and applicable checks are preserved.
No permission is automatically granted or widened. Both required and optional
extension declarations may request it under existing consent rules.

No persistent observation/history tables, credentials, identity model or old
migrations are changed. Failure rolls back rows, schema and user_version.
Reopen does not regrant or repeat the migration. Old Core rejects newer State43;
original running/review State42 data is not upgraded for this verification.
