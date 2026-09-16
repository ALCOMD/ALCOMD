# Standard workspace consolidation — 2026-09-17

This is a recovery/audit record, not a second current-stage authority. Read `docs/status.md`
for the current stage. The owner requested one standard directory and delegated conflict
classification to the existing approval history rather than choosing Git branches.

## Result and provenance

The only active worktree is `C:\Users\M2922\MyProjects\Rust\ALCOMD`, branch `main`.
No new GUI feature or business contract was implemented during consolidation.

| Previous location | Observed state | Disposition |
| --- | --- | --- |
| Standard `ALCOMD` | `24b0391` plus nine modified tracked files, no non-ignored untracked files | All nine files committed unchanged as recovery snapshot `b4547d56e752a2b7ea9949c3ec9482fdf2703916`. Not a validated candidate. |
| `ALCOMD-m7-unity-stop-a` | Clean at `e9349fd0f4cc9c18d9174dfc5826d48654aeb6f5` | Already an ancestor of the later GUI baseline. Extra worktree removed normally. |
| `ALCOMD-m7-h2-a-vg2` | Clean at `9307572feccb802c6ee74cb5e95be822c6e41ca5` | Selected current baseline, incorporating the owner-approved Unity closure and later GUI work. Extra worktree removed normally. |

Explicit supersession merge `9d0ddbb3f4cc4baa5808d4c8043b10797bfd8e43` has parents
`b4547d5` and `9307572`. Its complete Git tree equals `9307572`; this was checked with
`git diff --exit-code 9307572 HEAD -- .` immediately after integration. The standard branch
advanced by fast-forward to this merge. Both historical lineages remain reachable; no
reset, rebase, amend, forced worktree removal, or remote write was used.

## Why the older edits were preserved but not applied over the newer baseline

| Older edit group | Classification against subsequent owner direction |
| --- | --- |
| CorePages project header, Backups placement, and static Unity-version label | Pre-Unity visual experiment, not a replacement for the subsequently approved exact-version selector and launch model. It still exposes Automatic Editor. Keep only as historical evidence. |
| Resolve overflow, compact search, status/source menu | The intended maintenance/filter access was subsequently implemented in `d821267`, including repository and prerelease persistence. Do not reinstate the narrower old filter implementation. |
| Material MenuItem `selected` property | Used by the old filter menu; no need to reintroduce an unused API when retaining the newer filter form. |
| Package table widths, 72px rows, icon-only actions, removal of select-all, and per-page CSS | Historical design experiment, not proven final owner acceptance. Preserve for the existing row-density/action-placement backlog; do not silently mix into the later accepted shared-component foundation. |
| Accessibility, package, and create/restore test changes | Assert the old presentation and explicitly expect Automatic Editor. Preserve, but retain the newer exact-Unity and shared-control regression suites as active. |
| design-qa.md and old ExecPlan/status changes | Describe the older 38-test/pre-Unity candidate. Preserve in the snapshot, not as current acceptance; later 48-test evidence and the still-open Visual Gate remain current. |

This does not declare every newer GUI detail visually accepted. Unfinished version selection,
update eligibility, row presentation, and batch actions remain H2-A work. No technical or
visual gate was relaxed to perform the consolidation.

## Recovery and local files

All ignored directories returned by Git for both extra worktrees were moved, not deleted,
under the standard root:

`target/worktree-recovery/20260917/<old-worktree-name>/`

Preserved directories are `node_modules/`, `apps/alcomd-gui/node_modules/`,
`apps/alcomd-gui/dist/`, `apps/alcomd-gui/src-tauri/gen/`,
`apps/alcomd-gui/test-results/`, and the GUI worktree's `target/`.
The Unity worktree had no separate ignored `target/` at inspection time. Existing standard
directory caches and `target/m7-icon-acceptance-20260906` fixtures were not moved or removed.
Archive dependencies/build outputs may retain historical paths and are not active tooling.
Do not run binaries from the recovery archive or delete it as part of ordinary target cleanup.

Both exact source roots were verified as non-reparse directories. Every moved path was
checked to remain inside its source worktree and recovery destination. After preservation,
both worktrees were clean including ignored/untracked files, and their HEADs were verified
as ancestors of standard main. `git worktree remove` used no force flag; both original sibling
directories are gone. The v3 read-only repository and other sibling projects were untouched.

Historical branches were renamed, not deleted:

- `codex/archive/m7-h2-a-vg2-20260917`
- `codex/archive/m7-unity-model-stop-a-20260917`
- `codex/archive/h2-a-vg2-paused-unity-model`
- `codex/archive/h2-a-vg2-pre-unity-model`

Inspect old edits without restoring them using `git show b4547d5` or
`git show b4547d5:design-qa.md`. The snapshot is an ancestor of main, independent of archive
branch retention. Do not switch the live workspace back to it to continue development.

## Verification in the standard directory

- Production source, contracts, manifests, and locks remain identical to the selected
  `9307572` baseline; subsequent consolidation changes are instructions/documentation only.
- `npm run check`: PASS.
- `npm run build`: PASS; existing Vite chunk-size warning retained.
- `cargo xtask check`: PASS.
- First browser suite with the default five workers: 47 PASS / 1 FAIL. The navigation test
  observed zero `md-list` elements within its timeout. The precise cause was not established;
  this is not silently classified as a product regression or an environment-only failure.
- Full single-worker rerun (`npm run test:browser --workspace @alcomd/gui -- --workers=1`):
  48/48 PASS in 1.9 minutes, including the earlier navigation failure. This demonstrates
  that the failure did not reproduce in that run, not a proven root cause or a blanket
  concurrency guarantee. No assertions, production code, or test configuration were changed.
- `scripts/validate-metadata.py` and `git diff --check`: PASS.
- Real GUI interaction, full Rust workspace tests, and Hosted CI were not rerun as part of
  this directory consolidation. Prior evidence remains bound to its original SHA/environment.
