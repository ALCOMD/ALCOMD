# M7 H2-A icon integration and reconnect local acceptance

Date: 2026-09-06. Status: partial local acceptance; Visual Gate 2 remains open.

Evidence was collected from local changes above `9a1ddae0824cf05806331a9b861df8cf8390599d`
in `codex/m7-h2-a-vg2`. The completed implementation and regressions were subsequently
committed as `d8212676babe73523236c9b065f03164471a75e2`, together with the package-toolbar
increment documented separately. This record is not a Hosted CI result or owner visual signoff.

## Shared icon boundary

- The source remains Google's `google/material-design-icons`, pinned at
  `e083cc60a0828fdd3b404cea0cb8a5b900e9c23e`. The local asset manifest records
  each upstream path and SHA-256; style is Rounded, weight 400, grade 0, fill 0.
- Named static `?raw` imports stay in `@alcomd/ui`; business pages consume named
  ALCOMD exports. There is no Google Fonts runtime request, dynamic name registry,
  new dependency, SVG transformer, or third-party npm icon package.
- The shared renderer now uses Material Web `md-icon` with an explicit SVG/path,
  matching the supported [SVG integration](https://material-web.dev/components/icon/).
  Only the pinned single-path asset shape is accepted; arbitrary SVG markup is not injected.
- CSS masks and the wrapper's hard-coded `slot="icon"` size override are removed.
  The Material button owns its icon's rendered dimensions through its official token.
  Tests verify the default 18px size and a 22px token override, while standalone
  navigation icons remain 24px and sort glyphs retain their 20px optical asset.
- Owner correction: the unapproved replacement of Open Unity's leading `play_arrow`
  with trailing `open_in_new` is reverted. Open Unity retains its original leading
  play icon and pinned 24px optical asset, rendered through `md-icon`. Official
  rendering guidance does not authorize changing a button's icon or placement.

## Reconnect defect discovered during desktop acceptance

With the initial daemon connection unavailable, retry could load project data while
the shell retained a failed capability snapshot. Actions stayed disabled until navigation
or a window reload. The shared reconnect signal now refreshes shell settings and
capability discovery along with the page retry. Missing/unavailable capabilities remain
fail-closed; no capability name, RPC, permission, or backend behavior changes.

A browser regression covers retry while still disconnected and recovery on the same route
after the test transport returns. The failure was observed in the real desktop application;
the exact disconnect/reconnect sequence after the fix was verified in the browser harness,
not re-created against the real daemon. The rebuilt desktop application was checked with
the isolated daemon online.

## Executed local checks

- `npm run check`: PASS for all applicable workspaces.
- `npm run build`: PASS after the icon correction; the final GUI check/build also
  passed through the Tauri build after the reconnect fix.
- Full GUI browser suite: 44/44 PASS on the final frontend candidate.
  An earlier attempt failed during Vite dependency loading with `504 Outdated Optimize Dep`;
  the full rerun passed without weakening assertions.
- `npm run gui:build -- --no-bundle`: PASS on the final frontend candidate.
- `cargo build --locked --release -p alcomd -p alcomd-cli`: PASS, using unchanged Rust
  sources to supply the isolated real-daemon acceptance environment.
- `git diff --check`: PASS.
- `cargo xtask check`: PASS after the evidence/status update.
- `scripts/validate-metadata.py`: PASS using the bundled Python executable;
  the unqualified `python` command was unavailable on this shell's PATH.
- No dependency manifest or lockfile change. No production Rust, RPC, State, permission,
  unsafe, or platform API change in this correction.

The existing bundle-size warning is not a failure. No Hosted CI or three-platform GUI
claim is made. Full Rust workspace tests were not rerun for this frontend-only correction.

## Real Windows GUI observations

Computer-use operated the rebuilt Tauri executable, not the browser test page. A dedicated
daemon data directory and a synthetic project were used under
`ALCOMD/target/m7-icon-acceptance-20260906`. No existing user project was changed.

- Dark-theme navigation and Open Unity icons rendered through the shared integration.
- At the maximized 1920px window, the project list and package workspace were usable.
- The zero-exact-Unity-installation flow showed the matching-installation empty state;
  no Unity Editor was installed or launched for this test.
- Package overflow actions were visible. Remove opened the real daemon-backed change
  review, with readable version/removal text and contained buttons. Escape cancelled it;
  no package Apply or removal was executed. The table header remained below the scrim.
- Project overflow opened Copy Project. The explanatory text wrapped; the location input
  stayed within the form, and both actions were contained. Review showed the sibling
  `Project Copy` target with the full destination wrapping inside the dialog.
  Escape cancelled the review and returned focus to the project overflow trigger.
  No copy was started.

These observations are partial engineering evidence, not full visual approval, true v3
differential parity, or proof of every dialog/viewport/platform combination.

## Owner correction verification

After restoring the original leading `play_arrow`, `npm run check`, `npm run build`,
and all 9 tests in `m7-material-foundation.spec.ts` passed. The test now explicitly
protects the original icon identity, non-trailing placement, and official leading-icon
button padding. The earlier 44-test run above predates this narrow restoration.
The restored asset and its manifest entry match HEAD; `open_in_new` and the unused
wrapper trailing-icon option are removed. No responsive layout changes were made.
The corrected Tauri no-bundle build also passed; the restarted real Windows GUI
shows the restored leading play glyph. Metadata validation and `git diff --check` passed.

## Observed layout behavior (no responsive adaptation)

Subsequent owner request: temporarily show both `play_arrow` and `open_in_new`
side by side for direct comparison, using the same Material button slot dimensions.
This deliberately reintroduces the pinned `open_in_new` asset as a comparison only,
not a production icon selection. A local `tauri build --debug --no-bundle` provides
WebView DevTools without changing Cargo features or Tauri configuration. Frontend
check/build passed. The original nine-test Material suite returned 8 PASS / 1 FAIL:
the extra temporary icon makes the project column remain at its minimum at 1440px,
invalidating the existing stretch assertion. That assertion and layout were not relaxed;
this temporary comparison must not be described as a release-ready candidate.
The independent same-size comparison test passed (both glyph boxes and SVGs 18px,
side by side on the same baseline). The debug no-bundle build passed and was launched
in the real Windows GUI; the original release GUI was closed. No dependency feature,
permission, or Tauri configuration change was needed.

At narrower desktop widths, the project table's existing sticky action column covers
date/observation content and a horizontal scrollbar is present. The current automated
test protects the sticky action position; it does not prove that every data column remains
unobscured. Passing that test is not visual acceptance.

The owner explicitly does not require narrow-window adaptation. Do not introduce responsive
action regrouping, hide buttons/columns, or change the minimum window width to address this
observation. The report does not establish a new product-direction blocker or approve the
observed overlap. No layout change or test relaxation is introduced in the icon restoration.
H2-A remains open; H2-B/H3-H7/M8/M9/M11 are not started.

## Accepted 18px default and continued desktop acceptance

The owner accepted the ordinary Material Web button's 18px icon size. The temporary
second glyph, its asset/manifest entry, and comparison-only test are removed. Open Unity
again has exactly one leading `play_arrow`; the regression asserts this count as well as
the official dimensions. The asset manifest matches HEAD. No glyph cropping/scaling,
responsive adaptation, icon substitution, or dependency/configuration change is added.

After removing the comparison, `npm run check`, the full 44/44 GUI browser suite,
and `npm run gui:build -- --debug --no-bundle` passed. This supersedes the temporary
comparison's failing stretch test; the assertion was not weakened. The latest desktop
observations below use the rebuilt debug executable (embedded built frontend, real daemon),
not a release/Hosted CI acceptance run. DevTools remain available in this debug build.

Using computer-use on the isolated synthetic fixture, Copy Project was exercised beyond
review: default sibling target, review, Start copy, terminal `Copy succeeded: cleanup
complete`, automatic list refresh, Close, and Manage on the new project. The destination
did not exist before execution. The source retained revision 1, and all five source file
hashes were unchanged from the pre-copy snapshot. The copy has exactly the same five files,
with matching SHA-256 values. Official CLI/RPC project listing confirmed both registrations,
valid UPM/VPM manifests, the same Unity version and package dependency, and no issues.

- Source ProjectId: `99e725d8-b414-4680-be9f-3cc0b648879f`.
- Copy ProjectId: `16119a2a-7457-479f-8137-5871c5098dfd`.
- Target: `ALCOMD/target/m7-icon-acceptance-20260906/Project Copy`.
- Verified files: `Assets/fixture.txt`, `Packages/manifest.json`,
  `Packages/vpm-manifest.json`, `Packages/com.example.visual/package.json`,
  and `ProjectSettings/ProjectVersion.txt`.

The copy's package workspace loaded its installed package. Open Unity displayed the exact
2022.3.22f1 installation-required state; the dialog text and actions stayed contained,
and Escape dismissed it. No real Editor was installed/launched, migration was not started,
and no user project was changed. The synthetic copy is retained for further testing.
These checks remain partial H2-A engineering evidence, not owner Visual Gate 2 approval.
