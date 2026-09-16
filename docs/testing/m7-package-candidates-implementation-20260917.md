# M7 H2-A package candidate implementation — local evidence

Date: 2026-09-17. Local functional candidate verified with the existing Unity environment
limitation below; Visual Gate 2 **PENDING**.
`docs/status.md` remains the only current stage entrypoint.

## Authority and baseline

Owner approved implementation after Stop A review commit
`c9b1d162e01d1fc6ec911dbfaa2f535618cade54`. The approval adds no second proposal gate.
Work stays in the standard ALCOMD directory on main; previous GUI/test/status WIP is
integrated and preserved, no reset/worktree/push. The complete approval constraints were
synchronized into the existing Stop A contract before implementation.

The method/capability are `packages.queryProjectCandidates` / `packages.candidates.v1`.
Existing four read permissions and Principal/Project/source scope are checked. Source
selectors retain existing snake_case public fields. Tauri and SDK are typed transport
adapters; candidate decisions remain Core-side over registered snapshots.

## Engineering findings and corrected failures

- The first real RPC summary returned internal_error. A production DTO roundtrip test
  isolated serde flatten + deny_unknown_fields rejecting the legitimate items field.
  The public response decoder was replaced with an explicit closed tagged representation;
  the schema and unknown-field requirements were not relaxed.
- The first new VPM dependency-conflict test saw the existing PackageNotFound ordering path.
  Its fixture now orders the shared dependency after both root constraints so the intended
  dependency conflict is exercised; resolver behavior was not changed to satisfy the test.
- Store test fixtures initially used repository priority 0, violating the existing >=1
  constraint. Fixtures were corrected to 1 without changing production priority semantics.
- Final audit found older Project snapshots can contain wider IDs/versions than the new
  read DTO. Unrepresentable summaries now fail the candidate quota instead of emitting an
  invalid response; older Project and Plan data rules are unchanged.
- Failed Config reads/publication now invalidate the in-memory candidate cache. The owned
  settings lock remains inside the disk task, so cancelling a caller cannot detach a later
  disk publication from cache invalidation. Explicit successful settings reads restore it.
- Changing an explicit GUI source immediately invalidates the prior summary, even before
  the asynchronous reload effect runs. A delayed-query browser test protects that interval.
- The first full Rust run reached an obsolete Copy Project source-text assertion. It now
  checks the current menu-to-CopyProjectDialog binding; the nine action contract tests pass.
- The first complete browser run was 66/67: Unity's one-shot chooser remained disabled in
  one run. Unchanged single-case and five-repeat reruns passed; the cause is unconfirmed.
  A later newly added Update All exclusion assertion targeted Material's internal dialog
  rather than its slotted host text. The locator was corrected, retaining exact reason text.
- A subsequent 67/68 browser run exposed an old User Package test opening versions before
  Material's closed-event source action completed. The new source invalidation correctly
  closed that stale menu. The test now waits for the selected source label and enabled
  version entry before opening it; the exact downgrade and source assertions remain.
  Repetition also showed a real early-click interval where waiting for `closed` alone did
  not initiate closure. MenuItem now explicitly calls the component's existing `close()`
  after registering its callback and guards duplicate activation; it still executes after
  closure. Tests do not bypass opening animations to hide this problem.
- One final Rust rebuild exhausted memory and disk; LLVM reported OOM and no space, leaving
  incomplete artifacts. Only the resolved workspace `target/debug/incremental` cache
  (26,837,034,291 bytes) was removed. Tests are rerun with two build jobs and incremental
  caching disabled; neither assertions nor writer gates were changed.

## Acceptance boundaries

Known UI source/settings/Operation callbacks and reconnect invalidate candidate evidence.
There is no pre-existing Tauri daemon-event subscription bridge; none is invented here.
An explicit user Plan intent rechecks the candidate snapshot using the approved read query;
external changes then fail stale instead of silently replacing the intended target. Existing
Plan/Apply still revalidate and review actual graph changes. This introduces no query polling,
per-row probe Plan, additional public method or generic event/scheduling framework.

Historical desktop computer-use authorization timeouts remain evidence not obtained.
No attempt bypasses/repeats that authorization. Owner manual launch and screenshot acceptance
using isolated fixtures is supported. No real user project package changes or real Unity
process termination are authorized. Screen-reader disabled announcement remains separate
from host disabled, mouse/keyboard click guards and focus restoration.

## Implemented behavior

Repository and enrolled User Package records use the same source-preserving evidence model.
`latest` respects the visible source set and prerelease settings before Unity filtering;
`latestStable` uses its stable subset. `projectLatest` and `projectLatestStable` additionally
check direct Unity compatibility. `update`/`stableUpdate` require a unique newer candidate:
1.10.0 beats 1.9.0; build metadata alone is same; a stable version older than an installed
prerelease is not a stable update. Unknown classification is never inferred stable.
Yanked/incompatible/unselectable versions remain explanatory disabled entries. Dependency
graph eligibility is deliberately not promised: real Plan retains dependency failures.

Summary `providers` retains nonwinning and ambiguous visible source identities/revisions,
before per-package source override. This small same-method field addition is needed to keep
existing local/remote/user filters correct without downloading every versions page. It does
not alter source priority or introduce another selector. Public selectors remain
`{kind:"repository",repository_id:...}` / `{kind:"user_package",user_package_id:...}`.

The inline menu is lazy and submits the chosen source with Core-directed Install/Upgrade/
Downgrade. Same/unknown/yanked/incompatible entries cannot submit. The prior manual user
version fallback is superseded. Update All collects all locked packages regardless of search
or selection; known exclusions are shown, uncertain/stale/incomplete evidence blocks it.
Selected install/upgrade/remove/reinstall uses every selected ID, including hidden rows,
without silently dropping unsuitable members. Remove requires direct dependencies and
the existing complete catalog; reinstall requires locked versions and retained selectors.
Every bulk remains 1–256 intents, one existing Plan, explicit Apply and one Operation.

The query uses a SQLite read snapshot plus cached Config, checks Config again before return,
and binds Project/source/config evidence in an ephemeral fingerprint. Pagination binds view,
IDs, source overrides and order; every request repeats authorization. `catalogComplete` is
independent of `nextCursor`. Request/response limits include actual wire encoding/envelope;
64 KiB/1 MiB, default64/max128, IDs256, sources4096, related records100000. GUI shares a
two-request limit and rejects mismatched/stale pages instead of merging them.

## Boundary audit

The only resolver edit extracts its unchanged boolean source-ambiguity condition; Unity
parsers gain crate visibility for direct reuse. No resolver search, Plan, Apply, migration,
database schema, permission or dependency change is introduced. Settings caching is an
in-memory read projection of existing settings actions, not new persistent state.

All 57 captured dependency/lock/icon/style file hashes remain unchanged. `styles.css` has
zero diff; the earlier temporary removal/restoration of action-field width/media selectors
was already absent at the approval baseline. Existing GUI/test WIP is retained and integrated.
Material adds the host aria-disabled marker, disabled/duplicate activation guards and an
explicit close request. The native closed-event sequencing predates this implementation
and remains intact. These checks do not prove screen-reader announcement correctness.

## Owner desktop acceptance

Use the resulting `target/debug/alcomd-gui.exe` and matching `target/debug/alcomd.exe`.
The preserved isolated data directory is `target/m7-icon-acceptance-20260906/data`; its
synthetic projects and `target/m7-h2-a-inline-20260917/repository.json` can support visual
browsing/Plan review. That repository deliberately uses invalid archive URLs and must not
be used as successful Apply evidence. Never connect these checks to actual user projects.
The owner can start the matching daemon with `--data-dir` pointing at that isolated data,
then launch the GUI; an existing per-user daemon must be accounted for before launching,
not killed automatically. Capture the actual window, inline source/version menu, conditional
Update, search-hidden selection and bulk review; separately verify disabled announcement
with a screen reader. Automated browser Apply uses controlled fixtures only.

No desktop launch or computer-use authorization retry occurred in this implementation turn.
Visual Gate 2 remains **PENDING**, regardless of automated results below.

## Verification record

- Frontend check: PASS; final complete Chromium suite **69/69 PASS** (2.7m), including
  the exact-source User Package downgrade, source-switch interval and duplicate activation.
  Log: `target/candidate-implementation/gui-browser-final.log`.
- Rust format check and full workspace Clippy with `-D warnings`: PASS. The final Clippy
  uses `CARGO_INCREMENTAL=0`, two build jobs; no lint or validation is disabled.
- Schema: 21 positive/negative shapes PASS. All 39 original semantic vectors retain their
  given/expected text and map to 76 real test declarations: 38 implemented-test-reference,
  one screen-reader evidence-limit, zero partial-reference entries. The validator checks
  declarations only; Rust/browser execution is separate. See the machine-readable
  `specs/rpc/m7-package-candidates.contract-vectors.json` for every ID/file/test symbol.
- Full Rust execution reaches the new production RPC, Config race/cancellation/oversized
  Project evidence, DTO, cursor, and original package Plan/Apply tests successfully.
  The existing `m7_project_copy_rpc::project_copy_plan_apply_registers_an_exact_independent_copy`
  fails its `not_observed` expectation with actual `running_suspected`. Three real Unity
  Editor processes were observed; none was stopped. The existing writer gate is preserved.
  A complete `--no-fail-fast` run records all remaining targets instead of hiding this
  environment limitation or presenting a failed workspace run as PASS.
  Final `cargo test --workspace -j 2 --no-fail-fast` completed every target: exactly one
  failed test/target, the above existing Copy test; all other targets passed. Log:
  `target/candidate-implementation/workspace-tests-complete.log`. Existing ignored subprocess
  entrypoints are invoked by their parent fault tests; no new ignored test was introduced.
- `cargo xtask check`, metadata validation, baseline freeze check, diff whitespace and
  all 57 dependency/lock/icon/style boundary hashes: PASS. Baseline check used the real
  Windows user because sandbox ownership cannot validate the read-only reference checkout.
- No Hosted CI, real desktop interaction or screen-reader announcement PASS is claimed.
- Final Tauri debug no-bundle, matching daemon and CLI builds: PASS. Vite retains its
  informational >500 kB chunk warning; no code splitting or warning threshold change was
  introduced. Final frontend asset is `index-CAUudNCx.js`, CSS `index-BUlN4dA2.css`.

Final local build SHA-256 (these binaries are untracked build outputs):

| Binary under `target/debug` | SHA-256 |
| --- | --- |
| `alcomd-gui.exe` | `BEEFBC75DA04260318805A66CBF393ABC2F4DB467823E7EF7C888CBDCEC110F6` |
| `alcomd.exe` | `2624B2108C56A7E4848C07CA8E0A79D53EB453018611B06A6CE2B07EA14306F2` |
| `alcomd-cli.exe` | `152EDA7C17840CAB467FBE19C96DA0FE2A529718080C26DE7E189E3CD454DCAB` |

All implementation/test/documentation changes, including the preserved prior GUI WIP, are
prepared as one local implementation commit on main after `c9b1d162`. No push or remote
operation. Current H2-A remains IN_PROGRESS solely as an unaccepted local candidate, with
desktop/announcement evidence open and the Copy environment limitation explicitly recorded.
