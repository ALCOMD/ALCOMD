# M7 official GUI manual visual and flow checklist

Status: **M7 IN_PROGRESS / H2-A ACCEPTED / Visual Gate 2 PASS**. The current scope and approved contracts are defined by
`docs/status.md` and `docs/exec-plans/M7-official-gui-extension-ui.md`. This checklist records the accepted H2-A desktop candidate;
it does not authorize H2-B, H3-H7 or a later milestone.

This checklist is the owner-visible evidence gate for M7 and does not replace the Playwright Chromium suite or M12
platform-client accessibility testing.

## Visual gates and current scope

Each gate uses the real Tauri GUI and daemon/client path, stores bounded screenshots outside the repository, records the
exact SHA and any uncommitted input hashes, and receives project-owner approval before the next large slice starts.
Record whether the GUI uses a development server or an embedded frontend and whether its native build is debug or release.

1. Visual Gate 1: the approved v3-recognizable wide main shell/navigation; its historical owner approval remains valid.
2. Visual Gate 2: current H2-A Projects/package work and the other existing-page usability fixes explicitly approved in
   the current ExecPlan, including shared controls, offline recovery, Config3 and Unity Hub location actions.
3. Visual Gates 3/4 and later slices retain their separate approval boundaries. Existing routes may be regression-checked;
   visiting them does not start or accept those slices.

The current desktop baseline is 1400×800. Narrow-window adaptation, a collapsed navigation drawer and 320 CSS px desktop
acceptance are outside this H2-A scope; existing browser accessibility checks remain intact.

The reviewer checks structure and action placement, not pixel identity. Every intentional deviation from
`docs/gui/m7-layout-mapping.md` must already be approved. A gate failure stops the next production slice.

## Evidence boundary

- Run one real interactive ALCOMD desktop session using the current Tauri GUI and daemon/client path.
- Store screenshots outside the repository. Do not capture private project paths, command lines, credentials,
  extension form values, tokens, or raw diagnostics.
- Record the exact candidate SHA and uncommitted input hashes, operating system, display scale, effective
  viewport/window size, appearance mode and whether keyboard-only navigation was used.
- A screenshot is visual evidence only. The reviewer must also exercise the listed focus, confirmation, reconnect,
  progress, and terminal-state transitions.
- This gate does not certify Narrator, VoiceOver, Linux screen readers, WebView2/WebKitGTK/WKWebView compatibility,
  installers, updates, or uninstall. Those remain M12 responsibilities.

## Required flow

1. Launch the production GUI and confirm the shell reaches the approved default work entry (normally Projects) through
   `alcomd-client`/RPC. Capture the wide shell in light appearance and record the daemon connection state. If Home/status is
   retained as an approved deviation, visit it separately. Confirm the shell remains recognizable from the v3-to-v4 mapping.
2. Navigate the current H2-A and approved utility routes using only the keyboard. Confirm destination headings receive
   focus and focus indicators remain visible; do not flatten grouped destinations merely to satisfy this checklist.
3. On a safe owner-selected fixture, open Projects in list/grid modes, Project detail and Packages. Confirm loading,
   refreshing/last-known-good, empty, error, and disconnected presentations are distinguishable and do not expose a
   private path.
4. Review one high-impact frozen Plan. Confirm the dialog shows the daemon-provided change/risk summary, focus remains
   trapped, Escape closes it, and focus returns to the invoking control. Apply only if the selected fixture is safe;
   then follow the same OperationId through progress to a terminal state. A stale Plan must fail explicitly and must
   not be silently replaced.
5. Exercise package search, source/prerelease filters, version selection and bulk/update controls using complete Core
   candidate evidence. Review the resulting Plan and retain the single explicit Apply/Operation contract. Record any
   action that cannot be safely exercised with the selected fixture.
6. Open Settings and change an ordinary preference such as language. Confirm it saves immediately through Config3 and
   persists after relaunch. Open the separate theme editor, test preview/Cancel and explicit Save, and verify persistence.
   Do not expect a global Save/Discard bar. Restore the original preferences after the check.
7. Use the Unity Hub path chooser and confirm cancellation/selection and same-group focus behavior. Exercise revalidated
   Open with the already installed Hub. The download action is reachable only when Core returns `download_required`;
   record it as untested if the real environment cannot produce that result without changing the installation.
8. Disconnect and reconnect the test daemon while an approved utility page is open. Retry must restore both page data
   and current connection capabilities; navigate back to Projects and verify its actions recover.
9. Inspect Activity and Diagnostics. Confirm only bounded redacted summaries and opaque identifiers are visible; no
   token, Authorization header, raw argument, environment, SQL, stack trace, extension value, Portable UI payload, or
   complete private path may appear. Start a state integrity check and follow its Operation.
10. Repeat representative shell/detail/dialog views in dark appearance and capture a wide dark screenshot. Check the
    approved reduced-motion preference without introducing a new narrow-window adaptation requirement.
11. Exercise representative filled/tonal/icon buttons, text field, select, switch/checkbox, tabs, dialog and progress in both
    the current Core pages and any already available Portable UI regression fixture. Confirm observable Material
    hover/pressed/focus/ripple/disabled behavior where Material Web provides it and the shared `@alcomd/ui` foundation;
    do not install an extension merely to expand this H2-A desktop check or inspect private shadow DOM internals.

## Required record

Current record (2026-10-31). Detailed evidence and build boundaries are in
`docs/testing/m7-visual-gate-2-20261031.md`; the project owner accepted this H2-A Visual Gate 2 record on 2026-10-31:

```text
Candidate SHA: 4eae1d4619b4534cf6bd4916e9c282b87be0eec4 + uncommitted main changes
Final GUI SHA-256: 0AA8ADA4BABA8192D6953FBEA6E93CE106942B4030B02F132E9776EBF435BCD2
Uncommitted inputs: target/m7-visual-gate-2-20261031/final-candidate-inputs-before.json
Evidence directory: target/m7-visual-gate-2-20261031/
Native build: release; embedded production frontend; no development server
Operating system: Windows 11 Pro 10.0.26300 x64
Display scale: device reports 96 DPI; no cross-platform/client certification
Wide window: 1400x800 content baseline; main captures 1402x832 including title bar
Outside-repository screenshot root:
  C:/Users/M2922/.codex/visualizations/2026/08/17/01a00f12-6751-7d50-9346-7b58e94f2eea/m7-visual-gate-2-20261031/
Light screenshot: 68-final-release-projects-light.jpg
Dark screenshots: 59-final-release-projects-dark.jpg; 62-final-package-workspace-dark.jpg
Dark dialog: 65-final-dark-bulk-review.jpg
Keyboard-only route/focus flow: PASS for listed grouped navigation and heading paths
Dialog focus/Escape/restore: PASS; primary/version origins and final bulk toolbar return
Plan/Apply/Operation: PASS; same GUI install Operation through Running/Succeeded and Activity
Stale Plan: PASS; explicit rejection, no added Operation or changed project/files
Package controls / candidate evidence: PASS for recorded fixture; final source filtering works
Ordinary Config3 persistence: PASS; locale and reduced motion survive Core/GUI restart
Theme preview/Cancel/Save persistence: PASS; preview/Cancel do not write; Save persists
Unity Hub chooser: PASS for selection, prior Escape cancellation and same-group draft handling
Unity Hub Open: actual final GUI click observed; existing Hub window returned, readiness unproven
Unity Hub download: NOT EXERCISED; already installed, installation was not altered
Offline Retry/page data/capability recovery: PASS; initial P6 release build, separately tagged
Activity redaction: PASS for recorded summaries and exact opaque Operation detail
Diagnostics: empty native projection; state.check Running/Succeeded and Core integrity ok
Reduced motion: PASS preference/action observation; native frame timings not measured
V3 macro layout and approved deviations: ACCEPTED by project owner, 2026-10-31
Material presence/interaction: PASS for available representative Core controls
Portable UI native regression: NOT EXERCISED; no installed fixture; independent tests retained
Runtime restoration: original Projects/Editors/Operations/settings + revision 155 unchanged
Complete Chromium: final independent 247/0 (9.4 minutes); one worker; zero retries
Prior complete run: 246/1 network bootstrap failure, independently archived
Blocking issue: no observed functional blocker in recorded scope; listed unexercised items remain
Owner acceptance: ACCEPTED for H2-A / Visual Gate 2, 2026-10-31
```

M7 remains in progress until the remaining approved slices and gates are complete, the candidate's three hosted CI
jobs pass, and the project owner explicitly accepts the milestone. This H2-A acceptance does not approve the next slice.
