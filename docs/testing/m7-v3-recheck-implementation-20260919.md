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

## 2026-10-30 composite-button audit

The frozen sibling `ALCOMD3-v3-readonly/vrc-get-gui` was searched for split-button
containers and every application `DropdownMenuTrigger`. Four split groups were found;
the other triggers belong to ordinary row, filter or version menus. This is a behavior
audit, with no source copied or ported.

| Surface / v3 source | Before | After |
| --- | --- | --- |
| Projects, `app/_main/projects/index.tsx:345` | Separate Create/Register/Restore toolbar buttons | Shared Create project split button; Add existing project and Restore from backup in its menu |
| Templates, `app/_main/packages/templates/index.tsx:90` | Import in the toolbar; deriving a template only reachable through an existing template | Create template primary opens the existing derive workflow, including an empty catalog; Import template is in its menu |
| Project management, `app/_main/projects/manage/index.tsx:771` | Open Unity, Backups and more actions separate | Open Unity split button; launch arguments, directory, copy, backups and removal in its menu |
| Repositories, `app/_main/packages/repositories/index.tsx:504` | Add repository only | Existing Add repository remains functional; v3 import/export cannot be grouped until the missing public contract is approved and implemented |

All implemented groups use the standalone GUI `SplitButton.tsx`, composed from existing
Material Button/Menu primitives, and shared theme rules. Installed Material Web 2.5.0
does not expose a split-button component. The two halves reuse the list's 8px inner
corner and 2px gap tokens, with capsule outer corners and no separator border. Outer
radii use half the actual button height (currently 20px), so CSS overlapping-radius
normalization cannot scale the 8px inner corners down to almost zero. Template
derivation and launch-argument dialogs were extracted from the current v4 implementation
for reuse; their typed client methods, authoritative revisions, explicit errors and
Plan/Apply boundaries remain. Project workspace launch still owns the exact 0/1/many
installation flow. Projects table/grid actions are retained. v3's conditional cached
Unity-path reset has no equivalent in the accepted v4 launch model and is not restored.
Repository import/export is the pre-existing contract gap below, not a disabled mock
menu or an implicit authorization to add public RPC. No dependency, Rust, schema,
permission, persistence, resolver or Plan/Apply changes are included.

The first scoped browser run was 13/17. One real regression, return focus after template
import, was fixed. Two geometry checks measured Material's internal button instead of
the joined component hosts, and one old menu selector became ambiguous after adding a
second menu. Corrected host/scoped locators retain the original requirements. Recheck
was 6/6, including all 12 shared list routes at 1280/1440px; stable-menu screenshot and
workflow recheck was 2/2. Passing runs exited 0, initial run exited 1. Each completed
run required cleanup of its verified workspace-owned port-4173 Vite server; the existing
port-1420 dev and GUI were retained. This is not a full-suite result. Final frontend
check/build and diff check pass; the existing large-chunk build warning remains.
Evidence is under `target/m7-composite-buttons-20261030/`, with settled full-window menu
screenshots in `stable/`. Browser fixtures do not establish native desktop or screen-reader
acceptance. No real project was launched, copied, deleted or otherwise mutated.
Visual Gate 2 remains **PENDING**. The owner-requested local commits are `aa88d01` (shared rounded SplitButton) and `acd1061` (composite workflows and regression tests). Related MD3 spacing is `b3542d0`; this audit/status record is committed separately. No push or history rewrite. The source/test tree was preserved byte-for-byte during splitting, with staged diff checks instead of repeating the already completed tests.

Standalone-component/shape follow-up: the initial 3/3 at
`target/m7-split-button-shape-20261030/` checked computed radii, not their effective
painted size, and did not catch the owner-reported straight inner corners. Both host
and background computed styles still reported 8px while the inherited 9999px outer
radii forced all radii to shrink together. A screenshot pixel regression reproduced
the failure before the fix (exit 1): an inner cutout pixel was purple instead of the
gap's white background. With height-relative outer radii, both inner corner pixels
match the gap while the button body remains filled. Final check/build and diff check
pass, and all three existing split workflows pass 3/3 (exit 0), retaining menu actions
and dialog focus. Before/after crops and full-window screenshots are in
`target/m7-split-painted-corners-20261030/`; the painted crop was inspected. Existing
port-1420 dev received HMR; only owned port-4173 browser test servers were stopped after
completion. This adds browser evidence only, without a native rebuild or Visual Gate
acceptance.

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
