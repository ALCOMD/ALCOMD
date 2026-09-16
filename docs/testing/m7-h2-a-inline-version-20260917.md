# H2-A inline versions and selected installed packages — 2026-09-17

Status: partial local engineering work above `f86937bbf6ac2716c12dd1968a1fbd908ce4872a`.
Not a Visual Gate 2 candidate acceptance or Hosted CI result. Only the standard `ALCOMD`
directory on `main` was used; the starting tree was clean. No commit or push was performed.

## Implemented within the existing contract

- The Installed column contains a Material version menu. Each selectable repository entry
  identifies its exact version and source. Selection submits one existing install Plan with
  that source and the daemon's prerelease flag; it never infers upgrade/downgrade direction.
  The daemon ChangeSet is reviewed before explicit Apply. Cancelling keeps installed state.
- Installed exact versions cannot be selected again. Yanked entries, unknown classification
  and prereleases hidden by the confirmed setting are excluded from new inline choices.
  Menu order is catalog display order, not a claim of SemVer precedence or Unity compatibility.
- Public UserPackageRecord has no prerelease classification. A narrow manual downgrade
  fallback remains when classification is unavailable or user-package sources are present.
  It preserves the selected source when submitting the existing downgrade Plan. Treating
  absent classification as stable, or deleting this existing user use case, is not acceptable.
- Selected installed packages can be removed with one existing Bulk Plan. Reinstall and
  removal use the complete selection even when search/status/source filters hide a selected
  row. Missing, no-longer-installed, empty or over-256 selections cannot silently become a
  partial or empty batch. Explicit Apply remains separate.
- Shared MenuItem guards disabled activation and exposes host `aria-disabled`. Material Web
  2.5.0 does not forward that attribute to its internal menuitem; tests use its public host
  `disabled` property. This is not screen-reader certification.

No production Rust, RPC, State, permission, dependency, icon asset, or layout CSS changes.
The original single leading play icon, 18px button icons and existing table/window geometry
remain. No narrow-window adaptation was added. No v3 implementation was copied or modified.

## Reference and remaining contract gap

The v3 read-only package-list card and collect-package-row-info were inspected. They use
backend-provided compatible/incompatible candidates and latest/stable upgrade evidence.
v4's raw repository DTO and lexical pagination cannot supply that evidence; user-package
metadata is even narrower. Internal Core uses SemVer precedence and resolver-ready data.

**Existing Latest/Update presentation remains uncorrected and must not be accepted as
authoritative:** the old lexical ordering/different-string test remains outside this bounded
increment. Conditional update-all, stable update, mixed install/upgrade selection, and full
user-package inline selection are unfinished. No Plan is created merely to probe eligibility.
The documentation-only proposal `docs/exec-plans/M7-package-candidate-evidence-stop-a.md`
requires owner approval; no part of it is implemented or advertised by this increment.

## Verification

- Frontend check/build passed; the existing Vite chunk-size warning remains.
- First targeted run could not start Vite because sandbox directory access was denied.
  The approved real-user rerun found two new-test failures: textbox vs searchbox targeting,
  and expecting the internal Material menuitem to expose native disabled semantics.
  The search locator was corrected; the public host disabled property and no-request-on-cancel
  behavior are now asserted. A second run was 16/17 before the disabled assertion correction.
- The full 51-test browser run passed. A subsequent review found the user-package fallback
  regression, which was fixed and given an additional request/source regression.
- The subsequent full browser suite passed 52/52, including the user-package regression.
  Final Tauri debug no-bundle and current daemon/CLI builds passed. The fallback retains
  the original form classes and stylesheet; no layout CSS diff remains.
- `cargo xtask check`, metadata validation and `git diff --check` passed. Full Rust workspace
  tests were not rerun for these GUI-only changes. No Hosted CI or owner visual signoff.

## Desktop fixture and observations

The current daemon uses the preserved isolated data at
`target/m7-icon-acceptance-20260906/data`; the two existing synthetic projects remain registered.
A review-only local repository was registered via the official CLI under
`target/m7-h2-a-inline-20260917/repository.json` with ID
`25cf574b-e3c0-491e-adbc-ecdcddf084fc`. Its remote archive URLs use `.invalid` and synthetic
hashes: it is only suitable for browsing and Plan review, never Apply/download acceptance.
Source project hashes were captured before the session. No user project is part of this test.

Computer Use returned `Computer Use app approval timed out` when reading the running v3
window. No v3 runtime observation or interaction is claimed for that attempt.
Launching the newly built v4 GUI through Computer Use returned the same app-approval timeout.
Consequently the new filter, inline menu and batch-removal desktop interactions remain
unverified; the 52 browser tests must not be represented as real Tauri interaction evidence.
The real isolated daemon/CLI was healthy and returned both preserved synthetic projects and
the new repository registration. No package Apply was attempted. All five source fixture
file hashes remained unchanged. The daemon started by this session (PID 28116) was stopped
after verifying its exact executable path; fixtures and logs are retained.
