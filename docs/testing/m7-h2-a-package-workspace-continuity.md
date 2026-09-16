# M7 H2-A package workspace: v3 continuity follow-up

Date: 2026-09-06. Status: partial local engineering evidence; Visual Gate 2 remains open.

Implementation and regression checkpoint: `d8212676babe73523236c9b065f03164471a75e2`
on `codex/m7-h2-a-vg2`. This is a local commit, not a pushed or visually accepted candidate.

## Reference and scope

The owner requested continued GUI work against v3's real user experience. The reference
is the read-only source `ALCOMD3-v3-readonly/vrc-get-gui/app/_main/projects/manage/-package-list-card.tsx`,
not promotional screenshots. Its toolbar (approximately lines 515–659), selected actions
(678–862), rows (1030–1189), and version selector (1196 onward) were inspected for behavior.
No v3 implementation was copied, ported, or modified.

This increment changes only the official GUI/shared UI and tests/documentation. It does
not change RPC, permissions, State, resolver semantics, dependencies, lockfiles, unsafe,
or platform APIs. The accepted single leading play icon and ordinary Material button's
18px icon size remain unchanged. No narrow-window adaptation is introduced.

## Implemented increment

| Previously visible | Current local implementation |
| --- | --- |
| Separate outlined search preset in package management | Projects and package management share the same filled Material search preset. |
| Text Refresh, two permanent filter fields, and Resolve crowded the toolbar | Icon Refresh, growing search, overflow actions, and a Filter packages entry. |
| Resolve permanently exposed beside ordinary browsing | Resolve Dependencies is in the existing maintenance overflow, still using the existing review flow. |
| Repository visibility/prerelease controls absent from this context | Anchored filter form includes repository/local-package visibility, prerelease confirmation, status, and source-kind selection. |

Repository/local-package visibility and prerelease choices use existing revision-checked
`settingsUpdate`, followed by a catalog reload. Controls retain the last confirmed state
on permission/revision failures; they do not pretend a failed save succeeded. Status and
source-kind choices remain local presentation filters. None of these choices creates a
package Plan or Apply. Enabling prereleases requires confirmation; cancellation leaves
the setting unchanged.

The shared container uses the browser's non-modal top-layer popover and contains real
Material Web Checkbox, Select, and Button controls. Material Web 2.5.0 Menu supports menu
items rather than a form of independently focusable filters; it is not repurposed into a
custom checkbox-menu widget. Escape/light-dismiss and focus return are handled at this
shared boundary, not with package-page z-index exceptions. The shared controlled Checkbox
reconciles its property even when the owner rejects a proposed change. Material dialogs
have an explicit accessible name matching their headline.

## Evidence

- `npm run check`: PASS, including Material controls and all 30 exact icon assets.
- Full GUI browser suite: 48/48 PASS. Added coverage includes the toolbar, repository
  visibility persistence/catalog reload, permission and revision failures, top-layer/Escape
  focus behavior, and prerelease confirmation/cancellation.
- After naming the two fault-fixture modes in the typed harness union, frontend check and
  all 14 package-workspace browser tests passed again.
- `npm run build`: PASS. The existing Vite >500 kB chunk warning remains; it is not suppressed.
- `npm run gui:build -- --debug --no-bundle`: PASS after closing the old executable that
  prevented replacement on Windows. The rebuilt embedded-frontend GUI was restarted.
- `cargo xtask check`, metadata validation, and `git diff --check`: PASS.
- Desktop verification of the new filter interactions is pending: computer-use repeatedly
  reported user input while attempting to navigate from the restarted Projects page.
  Input was paused rather than competing with the owner. Browser-test coverage is not
  reported as real Tauri interaction evidence.

No Hosted CI or owner visual acceptance is claimed for this increment.

## Remaining H2-A gaps (not completed by this increment)

1. Installed-version selection belongs in its column; the existing Versions/free-text
   Choose Version flow is not the v3 interaction. Preserve explicit source identity and
   daemon validation when replacing it. Do not infer upgrade/downgrade from lexicographic
   ordering or create a second frontend resolver.
2. Latest/update eligibility and conditional update-all/stable actions need alignment with
   the frozen package contract and real catalog evidence. A different version string alone
   is not evidence that an upgrade exists.
3. Selected package actions currently expose reinstall, not the complete contextual
   install/upgrade/remove combinations shown by v3. Audit eligibility and reuse one existing
   package Plan/Apply, never an array of hidden child operations.
4. Row density, installed-version/source presentation, and conditional primary/overflow
   actions still need their own implementation and regression evidence.

Continue only within H2-A. Do not promote aggregate feature statuses or start H2-B,
H3–H7, M8, M9, or M11 on the basis of these partial results.
