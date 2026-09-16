# M7 H2-A Design QA

## Comparison target

- Source visual truth: the current runtime capture from the locally installed ALCOMD3 3.4.0 Project Manage page. The promotional composite is explicitly not accepted as visual truth.
- Structural source evidence: `../ALCOMD3-v3-readonly/vrc-get-gui/app/_main/projects/manage/index.tsx`, `../ALCOMD3-v3-readonly/vrc-get-gui/app/_main/projects/manage/-package-list-card.tsx`, `../ALCOMD3-v3-readonly/vrc-get-gui/components/layout.tsx`, `../ALCOMD3-v3-readonly/vrc-get-gui/components/ScrollableCardTable.tsx`, `../ALCOMD3-v3-readonly/vrc-get-gui/components/SearchBox.tsx`, and `docs/gui/alcomd3-v3-layout-baseline.md`.
- Rendered implementation:
  - `%TEMP%/ALCOMD-M7-H2-A-Visual-Gate-2/03-mixed-installed-available.png`
  - `%TEMP%/ALCOMD-M7-H2-A-Visual-Gate-2/04-project-overflow-menu.png`
  - `%TEMP%/ALCOMD-M7-H2-A-Visual-Gate-2/05-package-plan-review.png`
  - `%TEMP%/ALCOMD-M7-H2-A-Visual-Gate-2/06-operation-progress.png`
  - `%TEMP%/ALCOMD-M7-H2-A-Visual-Gate-2/07-source-ambiguity.png`
  - `%TEMP%/ALCOMD-M7-H2-A-Visual-Gate-2/08-real-tauri-projects.png`
  - `%TEMP%/ALCOMD-M7-H2-A-Visual-Gate-2/09-real-tauri-workspace.png`
  - `%TEMP%/ALCOMD-M7-H2-A-Visual-Gate-2/10-real-tauri-project-menu.png`
  - `%TEMP%/ALCOMD-M7-H2-A-Visual-Gate-2/11-v3-realigned-workspace.png`
  - `%TEMP%/ALCOMD-M7-H2-A-Visual-Gate-2/12-package-filter-menu.png`
  - `%TEMP%/ALCOMD-M7-H2-A-Visual-Gate-2/16-v4-table-final.png`
- Direct v3 runtime reference:
  - `%TEMP%/codex-clipboard-d0e9a6c5-9311-41fd-9785-98c462f472fe.png`
- State: wide desktop Project workspace with installed and available packages; focused states cover project overflow, compact package filters, Plan Review, Operation progress, and source ambiguity. Real Tauri evidence uses the existing local `m7-visual-project` and its actual installed package.

## Viewport and normalization

- Source runtime capture dimensions: 1652 × 670. It shows the populated Project Manage content canvas in the installed ALCOMD3 3.4.0 application. Direct inspection of `app/_main/route.tsx` confirms that v3 still renders `SideBar`; the supplied source image is therefore treated as a content-canvas crop, not evidence that Project Manage hides primary navigation.
- Deterministic browser evidence: 1440 × 900 CSS pixels, device scale factor 1, 1440 × 900 output pixels.
- Real Tauri WebView evidence: 1180 × 760 CSS/output pixels at device scale factor 1. No CDP viewport emulation was used because viewport emulation invalidates the real Tauri bridge; the final screenshots are from the native window dimensions.
- Crops were not normalized into a false pixel comparison. Full shell relationships and focused interaction regions were compared separately.

## Full-view comparison evidence

- The current implementation screenshot was inspected after direct v3 source review and an actual populated v3 Project Manage runtime capture. `16-v4-table-final.png` uses a 1920 × 670 full viewport, producing a 1648-pixel v4 content canvas that is directly comparable to the 1652-pixel v3 content crop without deleting or collapsing either product's sidebar.
- The implementation intentionally modernizes controls with the existing MD3 tokens and Material Web components, while retaining the recognizable v3 package-management hierarchy. It does not introduce dashboard cards, metadata heroes, or an Operation-first page.
- At the native 1180 × 760 viewport, project identity and actions remain a single compact context row. The package toolbar also stays on one row: package heading, icon-only refresh, search, overflow, and one Package filters menu. Resolve remains available through overflow instead of being presented as a permanent toolbar button.

## Focused region comparison evidence

- Project overflow: Backups, directory/copy and Unity preference remain project utilities; `Remove from list` and `Delete Project Directory…` are distinct and visually grouped as destructive actions.
- Package filters: the two persistent large selects were replaced by one Material menu. Status and source remain independently selected and visibly marked without occupying a second toolbar row.
- Conditional toolbar actions: direct v3 source inspection confirms that `Upgrade all` and `Upgrade all stable` are state-dependent, while the resolve suggestion is rendered only when `should_resolve` is true. Their absence in one runtime screenshot must not be interpreted as removal from the product contract.
- Package rows: one state-relevant primary action is visible; reinstall/version/link/remove alternatives use a Material menu. The action footprint is stable.
- Table geometry: the header keeps the v3 blank selection heading, rows use the v3 72-pixel rhythm, Installed/Latest/Source retain bounded widths, and row actions use standard Material icon buttons so the package identity column receives the remaining width.
- Plan Review: the dialog shows a concise change summary and does not expose Plan IDs, revisions, fingerprints, or JSON.
- Operation: progress remains contextual in the lower-right of the package workspace and does not navigate to an internal Operation detail route.
- Source ambiguity: the source chooser stays attached to the package row and names repository/User Package choices without showing a full filesystem path.

## Required fidelity surfaces

- Fonts and typography: the current system/MD3 typography remains compact, with truncated project/package identity where necessary. Heading and secondary-path hierarchy match the intended desktop density; no actionable wrapping or baseline defect remains in the inspected states.
- Spacing and layout rhythm: header, toolbar, table, and contextual overlays form one package-centric workspace. The native Tauri width keeps both context and package controls on one row without hiding actions.
- Colors and visual tokens: all new surfaces and states use existing MD3 semantic tokens. No new hard-coded product palette was introduced.
- Image quality and asset fidelity: this workflow has no product imagery. All visible icons use the approved self-hosted Material Symbols assets through `@alcomd/ui`; no emoji, CSS drawing, inline SVG, or second icon language was added.
- Copy and content: project identity is primary, the canonical path is secondary, destructive labels are explicit, and package/user-facing copy remains contextual rather than exposing architecture terminology.

## Findings and comparison history

1. Earlier P1: the workspace overflow combined removal and deletion behind one generic item, which did not match the approved hierarchy. Fixed by presenting distinct `Remove from list` and `Delete Project Directory…` entries while preserving the existing review/typed-confirmation/Operation paths. Post-fix evidence: `04-project-overflow-menu.png` and `10-real-tauri-project-menu.png`.
2. Earlier P2: the first capture occurred during Material menu/dialog animation and visually overlapped states. Fixed in the evidence harness with deterministic settle waits; no production animation was bypassed. Post-fix evidence: `04-project-overflow-menu.png` and `05-package-plan-review.png`.
3. Earlier P2: the native 1180-pixel Tauri viewport clipped Source/Resolve/count/overflow on a single toolbar row. The interim bounded two-row layout remained visibly unlike v3. Fixed by consolidating status/source presentation into one Material `Package filters` menu and restoring the toolbar to one row. Post-fix evidence: `11-v3-realigned-workspace.png` and `12-package-filter-menu.png`.
4. Earlier P2: checkbox label content occupied a visual table cell in the first prototype. Fixed by retaining the accessible label while hiding only its duplicate visible span. Post-fix evidence: `03-mixed-installed-available.png`.
5. Earlier P1: the project header treated Back and Backups as persistent text actions, exposed the Windows extended path prefix, and did not reproduce v3's project-context hierarchy. Fixed with an icon-only Back action, user-readable location text, explicit Unity-version context, and Backups inside project overflow. Post-fix evidence: `11-v3-realigned-workspace.png`.
6. Earlier P2: the package table used a visible select-all control absent from v3, narrow fixed metadata columns, wide text action buttons, and a 64-pixel row rhythm. Fixed by restoring the blank selection heading, measured metadata widths, 72-pixel rows, and accessible Material icon actions. Post-fix evidence: `16-v4-table-final.png`.

The source audit supports the current hierarchy changes, but a runtime v3 capture is still required before claiming that no actionable P0/P1/P2 visual mismatch remains. Complete responsive/200%-zoom acceptance remains the already-deferred H6 scope and is not represented as closed by this report.

## Interactions and runtime evidence

- Primary package action, row overflow, project overflow, source chooser, bulk selection, Plan Review, Apply/Operation follow, delete review reachability, and keyboard/focus regressions are covered by the 37-test official GUI Playwright suite.
- The real release Tauri WebView was launched from the local candidate, connected to the matching current-HEAD daemon, navigated from Projects to the actual `m7-visual-project`, and opened the real project overflow menu.
- Browser console inspection was not used as a substitute for the repository check/build/test gates; no runtime error appeared in the successfully captured real Tauri flow.

## Open questions

- Project-owner Visual Gate 2 approval is still required. This internal QA result does not mark H2-A or M7 complete.
- The populated v3 Project Manage runtime state is now captured. Final visual closure still requires the target and source to be captured at the same viewport and comparable UI state.
- Deterministic mixed rows, Plan/Operation, and source ambiguity use the official browser harness; the real Tauri evidence covers the actual Projects shell, package workspace, and project overflow without manufacturing package mutations in the user's real project.

## Final result

final result: blocked
