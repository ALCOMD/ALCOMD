# State37 · Capability permission switches

Approved by the owner on 2026-11-15. Extension `projects.read` changes from exact
Project grants to one `Global/*` grant covering current and future registered
projects' existing safe summaries. No filesystem or write authority is added.

One IMMEDIATE transaction invalidates leases of extensions with old project-read
grants, increments their grant/record revision, and rebuilds only extension_grants
with the new scope CHECK. It drops all old project-read rows, including revoked
ones, without creating a global grant. Other grant rows, extension metadata,
project and credential records, namespaces, events and sequences remain.
An explicit new authorization is necessary; old partial approval never widens.

Old migrations are unchanged. Failure rolls back the whole transaction, including
revision and lease changes. A newer version remains rejected by old Core. Use
disposable data only; do not migrate the owner's original running data.

The owner also approved the MCP/OAuth addition: current clients with old project-scoped
read/package grants receive a new immutable snapshot without those grants, preserving
other grants. Both client revision and snapshot revision advance to old client revision+1.
Old snapshots and historical idempotency evidence remain immutable and cannot confer
current authority. Credential hashes and registrations remain unchanged. Token families
with an old project ceiling, or a registration for an affected client, are revoked;
users must explicitly authorize capabilities again. Unaffected clients/families remain.
The temporary migration ID set is dropped before commit; no persistent session/table is added.
