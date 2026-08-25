# M7 official GUI manual visual and flow checklist

Status: reset after pre-checklist visual rejection. Candidate `19267230507071dc61ba306b98c8cfdd113e9ea2` was
technically valid but failed layout/component-system inspection before this checklist began; it is not an accepted visual
baseline and must not be pushed. This checklist applies only after H0-H6 and the four visual gates are approved and passed.

This checklist is the owner-visible evidence gate for M7 and does not replace the Playwright Chromium suite or M12
platform-client accessibility testing.

## Prerequisite visual gates

Each gate runs the real production Tauri GUI, stores a bounded screenshot outside the repository, records the exact SHA and
receives project-owner approval before the next large slice starts.

1. Visual Gate 1: v3-recognizable wide main shell/navigation and narrow adaptive drawer.
2. Visual Gate 2: Projects, Project package workspace, Packages & Templates grouping, Repositories.
3. Visual Gate 3: Templates, Unity, Backups, Operations and remaining core pages.
4. Visual Gate 4: Extensions, Portable UI, Settings, Activity, Diagnostics and About.

The reviewer checks structure and action placement, not pixel identity. Every intentional deviation from
`docs/gui/m7-layout-mapping.md` must already be approved. A gate failure stops the next production slice.

## Evidence boundary

- Run one real interactive ALCOMD desktop session using the production Tauri GUI and daemon/client path.
- Store screenshots outside the repository. Do not capture private project paths, command lines, credentials,
  extension form values, tokens, or raw diagnostics.
- Record the exact candidate SHA, operating system, display scale, effective viewport/window size, appearance mode,
  and whether keyboard-only navigation was used.
- A screenshot is visual evidence only. The reviewer must also exercise the listed focus, confirmation, reconnect,
  progress, and terminal-state transitions.
- This gate does not certify Narrator, VoiceOver, Linux screen readers, WebView2/WebKitGTK/WKWebView compatibility,
  installers, updates, or uninstall. Those remain M12 responsibilities.

## Required flow

1. Launch the production GUI and confirm the shell reaches the approved default work entry (normally Projects) through
   `alcomd-client`/RPC. Capture the wide shell in light appearance and record the daemon connection state. If Home/status is
   retained as an approved deviation, visit it separately. Confirm the shell remains recognizable from the v3-to-v4 mapping.
2. Navigate using only the keyboard through the approved groups to Projects, Repositories, Templates, Unity, Operations,
   Extensions, Activity, Diagnostics, Settings and About. Confirm each published route moves focus to its page heading and
   keeps a visible focus indicator; do not flatten grouped destinations merely to satisfy this checklist.
3. On a safe owner-selected fixture, open Project detail, Packages, Project Unity, and Backups. Confirm loading,
   refreshing/last-known-good, empty, error, and disconnected presentations are distinguishable and do not expose a
   private path.
4. Review one high-impact frozen Plan. Confirm the dialog shows the daemon-provided change/risk summary, focus remains
   trapped, Escape closes it, and focus returns to the invoking control. Apply only if the selected fixture is safe;
   then follow the same OperationId through progress to a terminal state. A stale Plan must fail explicitly and must
   not be silently replaced.
5. Open one first-party Portable UI surface. Confirm host-owned extension identity/chrome remains outside the
   extension document, a dirty form requires discard confirmation, and no generic Tauri/RPC surface is exposed.
6. Open Settings, change appearance and language, leave without saving once to verify discard confirmation, then save
   and relaunch to confirm daemon-owned Config Schema 1 persistence. Confirm no `localStorage` authority is involved.
7. Inspect Activity and Diagnostics. Confirm only bounded redacted summaries and opaque identifiers are visible; no
   token, Authorization header, raw argument, environment, SQL, stack trace, extension value, Portable UI payload, or
   complete private path may appear. Start a state integrity check and follow its Operation.
8. Repeat the representative shell/detail/dialog/Portable UI views in dark appearance. Capture a wide dark screenshot.
9. Resize to a 320 CSS px-equivalent narrow window. Confirm the modal drawer opens with focus inside it, Escape closes
   it and restores the menu toggle, all primary actions remain reachable, dialogs remain completable, and no critical
   horizontal clipping occurs. Capture the narrow view.
10. Enable the product reduced-motion setting and confirm state remains understandable without animation. At 200%
    effective text/display scale, confirm content and confirmation controls remain reachable without loss.
11. Exercise representative filled/tonal/icon buttons, text field, select, switch/checkbox, tabs, dialog and progress in both
    Core and Portable UI. Confirm observable Material hover/pressed/focus/ripple/disabled behavior where Material Web provides
    it. Confirm both render through the shared `@alcomd/ui` foundation; do not inspect private shadow DOM internals.

## Required record

Record these values before approval:

```text
Candidate SHA:
Operating system:
Display scale:
Wide window size:
Narrow window size:
Light screenshot location (outside repository):
Dark screenshot location (outside repository):
Narrow screenshot location (outside repository):
Keyboard-only route/focus flow: pass/fail
Dialog focus/Escape/restore: pass/fail
Plan/Apply/Operation flow: pass/fail/not safely exercised
Portable UI host chrome/dirty form: pass/fail
Settings persistence/discard: pass/fail
Activity/Diagnostics redaction review: pass/fail
Reduced motion/200% layout: pass/fail
V3 macro layout and approved deviations: pass/fail
Material component presence and interaction: pass/fail
Core/Portable shared design system: pass/fail
Blocking issue:
Owner acceptance: pending
```

M7 remains in progress until this record is completed, the candidate's three hosted CI jobs pass, and the project
owner explicitly accepts the milestone.
