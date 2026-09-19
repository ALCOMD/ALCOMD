# Logs: frozen v3 comparison

Date: 2026-09-19. Frontend source audit; Visual Gate remains PENDING.

Read in full: frozen `vrc-get-gui/app/_main/log/index.tsx`, `-activity-list-card.tsx`, and `-logs-list-card.tsx`.
The reference contains an Activity / Technical segmented heading, search, activity source/status/kind filters,
secondary/detail toggles, dense tables, technical severity filtering and automatic scroll.

The independent v4 implementation retains the real Activity / Diagnostics routes and typed reads. Activity now
has a search field, Status and Record type filters, Show details, a timestamp-first dense table, resource identity,
and a read-only operation dialog. Diagnostics has search, real severity and subsystem filters, diagnostic IDs,
and the same operation dialog. Opening that dialog calls existing `operationGet`; it does not reuse an activity
summary as an authoritative operation result. No Apply/cancel/retry mutation is performed by that dialog.

`ActivityItem` has no GUI/MCP/DeepLink provenance, elapsed duration, secondary classification or human-readable
target name. `type` means operation/event; it is explicitly called Record type and is not presented as v3 Source
or Write/Read activity kind. Missing state is Unknown. `resourceKind`/`resourceId` are displayed only if returned.

`DiagnosticItem` only supplies warning/error, subsystem, stable code, bounded summary and optional identities.
Diagnostics therefore remains named Diagnostics rather than pretending to be full technical logging; no invented
message body, Debug/Info severity, automatic stream, file opening or persisted logging settings are added.

Filtering only examines loaded pages and this boundary is visible next to the filters. Existing explicit Load more
and retry remain. Each SDK list request is already bounded to 100 entries. Search never causes an unbounded
fetch or silently claims to search all historical records. Initial empty pages retain the toolbar. Refresh replaces
the page session; no new cursor or persistence system is introduced.

The existing explicit Run state check action is retained. A synchronous request lock supplements disabled state
to prevent rapid repeated submissions. It remains an explicit operation, not a side effect of opening Logs.

New browser tests: `m7-logs-workspace.spec.ts` covers search/no-match recovery, filtered severity, detail identity,
dialog close/Escape and empty-list controls. These tests were authored but not run by the delegated source task;
the integration owner runs them with the already-running browser harness. No desktop or screen-reader pass is claimed.

## Repeated filter selection correction

The first browser run showed warning filtering worked, but immediately opening Severity again left no visible
error option. Waiting for `aria-expanded=false` did not fix the repeated click: that state precedes animation completion.
Installed Material `select/internal/select.js:558` resets `open=false` unconditionally in the previous menu's
`handleClosed`, so an open request during closing can be lost. The existing public `quick` property is documented
in `select/internal/select.d.ts:41` as synchronous opening without animation and is forwarded by Material to its
menu; menu closing also skips animation when quick is true. The v4 Select wrapper now exposes optional `quick`,
defaulting to false. Only Logs filter controls opt in. No value, keyboard selection, data filtering, dependency or
other page's selection semantics change. The regression retains consecutive mouse selections without inserted delays.
