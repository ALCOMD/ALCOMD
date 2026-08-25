# M7 official GUI manual visual and flow checklist

Status: prepared, not executed. This checklist is the owner-visible evidence gate for M7 and does not replace the
Playwright Chromium suite or M12 platform-client accessibility testing.

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

1. Launch the production GUI and confirm the shell reaches Home through `alcomd-client`/RPC. Capture the wide shell
   in light appearance and record the daemon connection state.
2. Navigate using only the keyboard through Projects, Repositories, Templates, Unity, Operations, Extensions,
   Activity, Diagnostics, Settings, and About. Confirm each route moves focus to its page heading and keeps a visible
   focus indicator.
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
Blocking issue:
Owner acceptance: pending
```

M7 remains in progress until this record is completed, the candidate's three hosted CI jobs pass, and the project
owner explicitly accepts the milestone.
