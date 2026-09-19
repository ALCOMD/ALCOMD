# M7 existing utility pages — owner-requested rework

Date: 2026-09-19. Base: `a41c915106ed647b14d7eef0b89b24c065c3e17d`, `main`.
Status: local frontend work; no Visual Gate signoff or remote operation.

## Authority and reference

Before H2-A desktop acceptance the owner requested basic correction of all other existing
pages, then clarified that the visual direction was fundamentally wrong and could require
rebuilding the pages. The requested reference level is the same as Projects: reproduce v3's
design, using the v4 stack rather than copying implementation. This supplements the current
ExecPlan; it does not approve new contracts or the entire remaining milestone roadmap.

Read-only reference: `ALCOMD3-v3-readonly/vrc-get-gui/app/_main/`, including
`packages/-tab-selector.tsx`, repositories, user-packages and templates routes, Settings,
Log, and Extensions. Reference facts: one page toolbar, resource segmentation, dense
tables/lists, grouped Settings, Unity managed under Settings, Activity/technical logs in
one area, contextual secondary operations. No v3 source was copied or modified.

## Reworked presentation

- A single utility workspace toolbar holds the page title or resource/log segments on the
  left and contextual actions/refresh on the right. Its body scrolls independently.
- Repositories, Templates, User Packages, Unity installations, Backups, Task Center,
  Extensions, Activity and Diagnostics use tables instead of large generic cards.
- Add/import/install forms open on demand and preserve their input when collapsed.
- Settings has separate, labelled appearance/language/package groups. Unity has a normal
  entry from Settings; Diagnostics has a normal entry from Logs. Details have parent links.
- Diagnostics prioritizes the diagnostic list; its state-check action is in the toolbar.
  Stable error codes and diagnostic IDs remain available. Times explicitly state UTC.
- Extension permission editing is a secondary disclosure; lifecycle actions remain visible.
- Repository package and Activity/Diagnostics pages explicitly load subsequent pages through
  existing cursors. Errors retain loaded content and expose retry. Refresh replaces the old
  pagination session, invalidating older requests.

The first styling pass still retained too much of the prototype's structure. It was replaced
by the utility workspace composition after the owner's clarification. It is not the final
visual direction merely because a browser test passes.

## Boundaries and evidence

Production changes are GUI presentation/read coordination only. Existing typed client,
Core, resolver, Plan/Apply, Operation, permissions, state/config formats, dependencies,
lockfiles and platform APIs remain unchanged. The confirmed Projects/package workspace
layout and icon assets are preserved. New CSS is scoped to utility pages; no new narrow
layout rules are added.

The initial real Tauri window was inspected with the existing isolated sample. Computer Use
was then stopped by physical Escape; that is an interruption, not a functional failure.
The owner explicitly requested browser-first continuation. Browser harness screenshots use
synthetic data and do not establish real Tauri/daemon end-to-end or screen-reader acceptance.

An initial targeted browser run passed 13/20. Four failures exposed an unsupported 20px
`arrow_back` asset in the new return button; the implementation now uses its existing 24px
asset. Two failures exposed the distinction between a Material host's `aria-current` and
its internal button; tests must not claim the latter or platform announcements. One test
matched hidden supporting text instead of the visible settings label; its locator was
corrected without removing geometry assertions. Initial failure traces are retained under
`target/m7-other-pages-audit-20260919/browser-first-run-failures`.

The existing full Rust workspace environment limitation, unrun Hosted candidate CI, and
39 candidate vectors (38 implementation-linked, one screen-reader evidence limitation)
are unchanged. Historical P6/P8 results are not rewritten.

## Explicitly deferred

- Template/extension import still requires the existing registry revision input because
  current list DTOs do not expose authoritative registry revision. Do not guess or add RPC.
- Template derivation and extension permission scope still contain technical input fields;
  this rework does not redesign their public workflows or permission semantics.
- Complete localization, unrepresented real source/extension scenarios, real screen-reader
  announcements and owner visual approval are not claimed.
- The latest desktop binary and browser result must be distinguished from the original
  clean `a41c915` candidate; working-tree changes are not included in that commit SHA.

Visual Gate 2 remains PENDING. No H2-A closure, push, or later milestone completion.

## Final local verification

- `npm run check` and production frontend build: PASS. Vite retains its existing >500 kB
  chunk warning; no dependency or chunk-threshold change was made.
- Full Chromium suite: **78/79 PASS**. The only failure was the old User Package test's
  heading/article locator after the card-to-table change. The test now uses the package row;
  the revision-2 and enrollment-removal assertions remain. Revision is a secondary source
  detail in the table, rather than being removed from the UI.
- Final affected suites (`m7-package-workspace`, `m7-utility-pages`,
  `m7-project-create-restore`): **26/26 PASS**. This is not a second full 79/79 run.
- New paging tests (3) and workspace concurrency tests (2) passed. A mutation completing
  during an in-flight refresh queues one follow-up read; route changes discard old results.
- The template fixture incorrectly used `built-in` for `sourceKind`; the protocol's snake-case
  enum is `builtin`. Corrected the fixture, retained free-form provenance, and verified the
  builtin template has no Remove action. No production policy was weakened.
- `cargo xtask check` and `git diff --check`: PASS. No production Rust changed; the expensive
  workspace suite was not repeated. Its previously recorded environment blocker remains.
- Final Tauri `--debug --no-bundle`: PASS (`dev`, unoptimized + debuginfo). GUI executable:
  `target/debug/alcomd-gui.exe`, SHA-256
  `0E0DB602F77E6E75A6F89A6036C191FEB43B77981BD8D5CCFECCD630861DB9E2`.
  This build includes the uncommitted GUI changes, not just the base commit.
- Daemon/CLI hashes remain respectively
  `2624B2108C56A7E4848C07CA8E0A79D53EB453018611B06A6CE2B07EA14306F2` and
  `152EDA7C17840CAB467FBE19C96DA0FE2A529718080C26DE7E189E3CD454DCAB`.
  Source/build correspondence is recorded in
  `target/m7-other-pages-audit-20260919/build-manifest.json`.
- 17 full 1440×900 browser screenshots are in
  `target/m7-other-pages-audit-20260919/browser/`, covering 16 routes and the populated User
  Package state. These are synthetic frontend fixtures. Updated Tauri interaction was not
  performed. The initial owned test GUI (PID 52052, window 8524752) was identified and stopped
  for rebuilding; no Unity/v3/unidentified process was stopped.
- Material's current-navigation state is checked on its host property. This does not prove
  that the internal button or a platform screen reader announces it. The menu-disabled
  announcement limitation also remains open.
- HEAD remains `a41c915106ed647b14d7eef0b89b24c065c3e17d`; work stays unstaged, no commit or
  push. Tracking-ref comparison is 0 behind / 10 ahead; no remote fetch was performed.
