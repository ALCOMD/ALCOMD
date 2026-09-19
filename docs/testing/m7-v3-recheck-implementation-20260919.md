# M7 utility pages: owner-rejected design recheck

## Status and scope

The owner rejected the previous utility-page design. Its historical 78/79 initial
browser run, corrected 26/26 subset and debug build remain historical results, not
visual acceptance. This follow-up uses frozen v3 solely as behavioral/design evidence;
no v3 source is copied, translated or included in the product.

Only `C:\Users\M2922\MyProjects\Rust\ALCOMD`, branch `main`, base
`a41c915106ed647b14d7eef0b89b24c065c3e17d`. Existing WIP was preserved throughout implementation. No push, worktree,
backend/contract change, dependency or migration is introduced. Local commits were
subsequently requested by the owner, as recorded below.
Visual Gate 2 remains **PENDING**, not a result of browser tests.

## Re-audit and corrections

- [Flow-by-flow v3 audit](m7-v3-flow-recheck-20260919.md) identifies source locations,
  modal versus page behavior, supported calls, and contract gaps.
- [Settings audit](m7-v3-settings-audit-20260919.md): Unity installation table →
  Packages → Appearance (language, animation, compact checkbox rows) → existing v4
  Theme → system links. The rejected Preferences/Unity tab composition is removed.
  Dirty settings block refresh; existing revisioned Save/Discard and navigation guard
  remain. Raw Config revision text is removed from the ordinary page.
- Repositories: Add input and real `repositoriesInspect` preview occupy one modal;
  row package browsing opens a repository-named paged modal; row removal has named
  confirmation and retains an explicit failure. No inspection is called a durable Plan.
- User Packages: native directory selection is in the toolbar; removal requires
  package-name/version confirmation, retains errors and never deletes source folders.
- Templates: inline favorite and contextual menu; create-from-project, create-project,
  export and remove dialogs. Derive selects registered projects and reads the actual
  revision; import inspects the bundle and reads a matching template revision. Review
  uses real existing Plan results. Builtin removal remains disabled.
- Extensions: installed cards, enable switches and declared Portable UI Open actions.
  Install input/Plan Review share one modal; permissions and uninstall have named
  dialogs. Permission/Plan/Operation contracts and explicit publisher approval remain.
- Backups: preparation and restore Review become modal flows; restore uses the actual
  backend target/requirements. Closing an accepted operation's dialog does not claim
  operation cancellation. Project Unity launch arguments have Cancel/Save modal drafts;
  the accepted workspace's exact launch/migration model is unchanged.
- [Logs audit](m7-v3-logs-audit-20260919.md): loaded-snapshot search, returned-field
  filters, details and real `operationGet` modal. Diagnostics is not mislabeled as raw
  Technical logs. Explicit pagination is retained; no hidden full-catalog traversal.

Shared utility dialogs wait for native close before unmounting so focus can return
to the invoking button. Busy requests prevent Escape/Cancel. Preparation and Review
actions share a single footer. The existing Material Select `quick` option is enabled
only for Logs filters to prevent a closing animation from consuming a rapid reopen.
Projects icons/layout and package actions are not redesigned.

## CSS and compatibility boundaries

The existing WIP already changed `styles.css`; the earlier “CSS unchanged” claim is
not true for this rework. Utility-scoped toolbar/table/card styles are retained and
corrected, Settings gains vertical group/row rules, utility dialog contents lose the
old nested card styling, and repository actions have a compact row layout. Separate
`ExtensionWorkspace.css` and `LogsWorkspace.css` own card and log-column geometry.
No new narrow-window adaptation is added. Shared Dialog gains optional width and
dismissibility; Select gains optional `quick`, with existing defaults preserved.

Task Center/About and secondary deep links remain v4 surfaces. They are not claimed
to have an identical v3 counterpart. The existing page-loading/paging fixes remain.

## Explicit gaps (not implemented or accepted)

The existing contracts do not support the complete v3 Settings (Hub path, global
launch arguments/default paths/backup preferences/update/migration settings), arbitrary
template manifest editing/duplicate, repository headers/alias/import-export, or full
technical-log streaming/folder access. They are not replaced with fake controls.
Extension install still requires a verified registry revision because the current
read interface does not expose it; it is an explicitly labeled transitional input,
not a guessed zero or a completed ordinary-user installation flow. Current template
and extension client list wrappers also do not expose every available paging input.
These are retained review gaps, not authorization to expand RPC in this round.

## Validation and evidence

Initial new 10-test run: 3 passed, 7 failed. Most failures were native Shadow DOM
dialog-descendant test scopes; browser inspection confirmed named dialogs existed
while their slotted controls were outside that native node's DOM subtree. Tests keep
the accessible-name assertion and scope controls through the Material host instead.
The next 24-test run: 19 passed, 5 failed. It exposed the real utility Cancel focus
loss and Logs rapid-reopen issue; both received the scoped production fixes above.
Other failures were obsolete Close/Cancel labels, menu host selectors and a Unity
clear test acting during authoritative refresh. Those expectations were corrected
without changing Plan/Apply or weakening disabled-action checks.

Final applicable evidence:

- Complete browser suite first run: **85/91**. Six failures comprised outdated
  expected UI text, ambiguous/scoped locators, one template invocation detached
  during a concurrent Vite hot reload, and the obsolete draft-after-Cancel
  expectation. Do not report this run as 91/91.
- Source-frozen affected five-spec run: **43/44**; the remaining assertion expected
  the old “Repository registered” text while the UI reported “Repository added.”
  Corrected multi-workflow test: **1/1**, including actual fixture registration,
  template/backup/extension Plan or explicit mutation and Operation feedback.
- Full-window inspection then found the repository package modal's outer Material
  max-width still clipped its wider contents. Optional wide Dialog now enlarges
  both boundaries; added first-column/Close geometry assertions and repeated
  utility page group: **6/6**. The final screenshot shows all three columns.
- Final frontend build/check **PASS** (226 modules; JS index-0EfduljS.js,
  CSS index-BFsJ2fzA.css). Existing Vite large-chunk warning remains.
- cargo xtask check and git diff --check **PASS**. Staging area empty; HEAD
  unchanged. Tracking comparison is 0 behind / 10 ahead, without fetching or push.
- Exact normalized-text comparison against HEAD: CorePages from PageProps through
  the Projects/package section, PackageActions, and ProjectUnityWorkspaceActions
  are unchanged. Shared optional Material additions retain existing defaults.
- Frozen v3 HEAD independently confirmed as
  4aa98ae4f18d42c10137278997180dbede991e88 using a command-local ownership exception;
  no global Git setting or reference-repository file was changed.

Screenshots under `target/m7-v3-recheck-20260919/browser/` are full-window browser
fixtures, not real Tauri or v3 screenshots. The old rejected screenshots remain in
their original directory. Native Tauri was not rebuilt/launched for this frontend
recheck; the previous debug executable therefore does not contain this new WIP.

Real desktop interaction and screen-reader disabled announcement remain unverified.
The 39 candidate vectors retain 38 implementation mappings and one screen-reader
evidence limitation. Full Rust workspace retains its prior local-environment blocker;
it was not rerun for this frontend-only change. Hosted candidate CI was not run (no
push). Neither M7 nor H2-A is closed.


## Owner-requested local commit checkpoint

On 2026-09-19 the owner explicitly requested splitting and committing the accumulated
work. The implementation tree was retained without further product changes:

- `c67dca1`: paged utility workspaces, shared modal primitives and standalone tests.
- `ce443ec`: utility page reconstruction, modal workflows, styles and page regression tests.
- This documentation commit: v3 audits, historical verification and current pending status.

Earlier references to an unchanged HEAD, empty index and uncommitted WIP describe the
pre-commit validation checkpoint. This local commit request does not approve the
visual design, close H2-A, authorize a push or make the earlier debug Tauri binary
match these frontend changes. No tests were rerun merely to split the existing tree;
staged diff checks were performed for each commit.
