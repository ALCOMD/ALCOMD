# M7 H2-A Design QA

## Comparison target

- Source visual truth: `../ALCOMD3-v3-readonly/docs/release-assets/ALCOMD3-BOOTH-2.png` plus `docs/gui/alcomd3-v3-layout-baseline.md` and the frozen v3 Project Manage source named by that baseline.
- Rendered implementation:
  - `%TEMP%/ALCOMD-M7-H2-A-Visual-Gate-2/03-mixed-installed-available.png`
  - `%TEMP%/ALCOMD-M7-H2-A-Visual-Gate-2/04-project-overflow-menu.png`
  - `%TEMP%/ALCOMD-M7-H2-A-Visual-Gate-2/05-package-plan-review.png`
  - `%TEMP%/ALCOMD-M7-H2-A-Visual-Gate-2/06-operation-progress.png`
  - `%TEMP%/ALCOMD-M7-H2-A-Visual-Gate-2/07-source-ambiguity.png`
  - `%TEMP%/ALCOMD-M7-H2-A-Visual-Gate-2/08-real-tauri-projects.png`
  - `%TEMP%/ALCOMD-M7-H2-A-Visual-Gate-2/09-real-tauri-workspace.png`
  - `%TEMP%/ALCOMD-M7-H2-A-Visual-Gate-2/10-real-tauri-project-menu.png`
- State: wide desktop Project workspace with installed and available packages; focused states cover project overflow, Plan Review, Operation progress, and source ambiguity. Real Tauri evidence uses the existing local `m7-visual-project` and its actual installed package.

## Viewport and normalization

- Source composite: 2048 × 2048 pixels at 72 DPI. It is a release composite rather than a one-to-one workspace capture, so it is used for structural hierarchy and density rather than pixel measurements.
- Deterministic browser evidence: 1440 × 900 CSS pixels, device scale factor 1, 1440 × 900 output pixels.
- Real Tauri WebView evidence: 1180 × 760 CSS/output pixels at device scale factor 1. No CDP viewport emulation was used because viewport emulation invalidates the real Tauri bridge; the final screenshots are from the native window dimensions.
- Crops were not normalized into a false pixel comparison. Full shell relationships and focused interaction regions were compared separately.

## Full-view comparison evidence

- The v3 reference and current implementation were opened in the same comparison pass. Both keep a persistent left navigation rail, a right rounded work canvas, compact project context at the top, and a package list occupying the primary content area.
- The implementation intentionally modernizes controls with the existing MD3 tokens and Material Web components, while retaining the recognizable v3 package-management hierarchy. It does not introduce dashboard cards, metadata heroes, or an Operation-first page.
- At 1440 × 900, the package toolbar remains one compact row. At the native 1180 × 760 Tauri viewport it wraps to two bounded rows; Source remains with the filters and Resolve/count/overflow remain visible on the second row.

## Focused region comparison evidence

- Project overflow: utility actions remain first; `Remove from list` and `Delete Project Directory…` are distinct and visually grouped as destructive actions.
- Package rows: one state-relevant primary action is visible; reinstall/version/link/remove alternatives use a Material menu. The action footprint is stable.
- Plan Review: the dialog shows a concise change summary and does not expose Plan IDs, revisions, fingerprints, or JSON.
- Operation: progress remains contextual in the lower-right of the package workspace and does not navigate to an internal Operation detail route.
- Source ambiguity: the source chooser stays attached to the package row and names repository/User Package choices without showing a full filesystem path.

## Required fidelity surfaces

- Fonts and typography: the current system/MD3 typography remains compact, with truncated project/package identity where necessary. Heading and secondary-path hierarchy match the intended desktop density; no actionable wrapping or baseline defect remains in the inspected states.
- Spacing and layout rhythm: header, toolbar, table, and contextual overlays form one package-centric workspace. Wide mode is dense; the native Tauri width uses at most two toolbar rows without hiding actions.
- Colors and visual tokens: all new surfaces and states use existing MD3 semantic tokens. No new hard-coded product palette was introduced.
- Image quality and asset fidelity: this workflow has no product imagery. All visible icons use the approved self-hosted Material Symbols assets through `@alcomd/ui`; no emoji, CSS drawing, inline SVG, or second icon language was added.
- Copy and content: project identity is primary, the canonical path is secondary, destructive labels are explicit, and package/user-facing copy remains contextual rather than exposing architecture terminology.

## Findings and comparison history

1. Earlier P1: the workspace overflow combined removal and deletion behind one generic item, which did not match the approved hierarchy. Fixed by presenting distinct `Remove from list` and `Delete Project Directory…` entries while preserving the existing review/typed-confirmation/Operation paths. Post-fix evidence: `04-project-overflow-menu.png` and `10-real-tauri-project-menu.png`.
2. Earlier P2: the first capture occurred during Material menu/dialog animation and visually overlapped states. Fixed in the evidence harness with deterministic settle waits; no production animation was bypassed. Post-fix evidence: `04-project-overflow-menu.png` and `05-package-plan-review.png`.
3. Earlier P2: the native 1180-pixel Tauri viewport clipped Source/Resolve/count/overflow on a single toolbar row. Fixed with a bounded two-row flex layout, a smaller search flex basis, and right-aligned secondary actions. Post-fix evidence: `09-real-tauri-workspace.png`.
4. Earlier P2: checkbox label content occupied a visual table cell in the first prototype. Fixed by retaining the accessible label while hiding only its duplicate visible span. Post-fix evidence: `03-mixed-installed-available.png`.

No actionable P0, P1, or P2 issue remains in the H2-A Project workspace states inspected here. Complete responsive/200%-zoom acceptance remains the already-deferred H6 scope and is not represented as closed by this report.

## Interactions and runtime evidence

- Primary package action, row overflow, project overflow, source chooser, bulk selection, Plan Review, Apply/Operation follow, delete review reachability, and keyboard/focus regressions are covered by the 37-test official GUI Playwright suite.
- The real release Tauri WebView was launched from the local candidate, connected to the matching current-HEAD daemon, navigated from Projects to the actual `m7-visual-project`, and opened the real project overflow menu.
- Browser console inspection was not used as a substitute for the repository check/build/test gates; no runtime error appeared in the successfully captured real Tauri flow.

## Open questions

- Project-owner Visual Gate 2 approval is still required. This internal QA result does not mark H2-A or M7 complete.
- Deterministic mixed rows, Plan/Operation, and source ambiguity use the official browser harness; the real Tauri evidence covers the actual Projects shell, package workspace, and project overflow without manufacturing package mutations in the user's real project.

## Final result

final result: passed
