# State39 · Remove publisher confirmation

Owner-approved breaking change on 2026-11-16. No additional publisher approval is required for a user-selected signed package. `publisherApproval`, `trustDecision` and the publisher-confirmation error are removed from current RPC/SDK contracts; removed input fields are rejected, not ignored or automatically supplied.

One IMMEDIATE transaction drops `extensions.trust_decision` and `extension_plans.trust_decision`. The `extension_plans_immutable` trigger is recreated with exactly its former conditions except the removed column equality. All other columns/cells, keys, foreign keys, indexes, triggers, permissions, namespaces, accepted MCP effects, operations and event sequences remain intact. No grant is created and no existing permission or lease is expanded. Old migrations remain unchanged; no new table, Config, permission name, identity model or dependency is introduced.

Existing installation still uses signed-package validation, immutable Plan/Apply, source/digest revalidation and Operation recovery. Ed25519/fingerprint checks remain mandatory, as does retained-data publisher identity isolation. Signature validity describes integrity and identity, not publisher trust or an implicit capability grant. External callers cannot claim the separately controlled bundled source.

Failure after either column drop rolls back the two columns, trigger, rows and version atomically. Reopening State39 does not repeat the migration. Older Core rejects version39. This batch does not upgrade original running data. Historical migrations/test fixtures and opaque audit records may contain the old values, but no current runtime field or decision consumes them.
