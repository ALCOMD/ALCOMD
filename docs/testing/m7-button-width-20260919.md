# M7 shared button spacing and stable state widths

Owner requests: keep changing button labels from shifting neighboring controls, and
use MD3 spacing consistently within each button type without page-level overrides.

Original width baseline: `a531e565a216e30079de2c29fcc1a8c5ea2e8253`. Current spacing
work is on `main` at `b41afe2`, preserving existing WIP. Frontend-only; no RPC,
dependency, resolver, Plan/Apply, icon asset or persistence changes.

## Implementation

- The shared `@alcomd/ui` theme owns Material Web 2.5.0 / MD3 spacing by type. Filled,
  tonal and outlined buttons use 24px each side without an icon, or 16px on the icon
  side and 24px opposite. Text buttons use 12px each side without an icon, or 12px
  on the icon side and 16px opposite. Leading/trailing icon rules are both defined;
  the existing library owns its 8px icon-label gap. Text buttons are intentionally
  narrower than the other variants. See the official
  [filled](https://github.com/material-components/material-web/blob/v2.5.0/tokens/_md-comp-filled-button.scss)
  and [text](https://github.com/material-components/material-web/blob/v2.5.0/tokens/_md-comp-text-button.scss)
  tokens.
- Remove the table-sort and project-card spacing overrides. GUI CSS/TSX may not
  redefine button spacing tokens; the existing Material boundary check enforces this
  ownership. SplitButton's fixed-size arrow area remains a shared component rule,
  with its label-bearing half using ordinary filled-button spacing.
- Project cards need at least 376px instead of 320px to contain the existing three
  complete actions with standard padding. Their order, one-line action layout and
  40px button height remain. At the checked 1180/1280/1440px desktop widths the grid
  uses two columns; at 1600px it uses three. This replaces the former card-specific
  padding reduction, without removing actions, clipping labels or adding a narrow
  window layout.
- Shared React `Button.widthLabels` reserves the intrinsic width of all known state labels using overlapping grid cells. Measurement labels are hidden and `aria-hidden`; the visible label remains the accessible name. Slotted icons remain direct children of the Material host.
- Existing refresh, save, confirmation, planning/apply, favorite and pagination state labels supply their alternatives. Pagination includes loading, retry and normal labels. This is intrinsic text sizing, not a hardcoded pixel width; unknown/unbounded future labels require the caller to supply suitable alternatives or a separate bounded-width design.
- Button action handlers, disabled conditions and backend calls are unchanged.

## Evidence

### Current MD3 spacing — 2026-10-30

- Final `npm run check`, `npm run build` and diff check: PASS. Existing large-chunk
  build warning remains.
- First scoped browser run: 7/8, exit 1. The grid at 1440px exposed buttons exceeding
  the card's inner boundary after removing its spacing override. Minimum card width
  was corrected; geometry recheck is 1/1, exit 0, at 1180/1280/1440/1600px with light
  and dark themes, long content, complete buttons and scrolling. Assertions retain
  all three actions, inner boundaries and one-row layout; column-count expectations
  follow the required standard button dimensions.
- Shared launch/directory/copy Review/cancel/removal and capability/missing-editor
  grid regressions: 2/2, exit 0. The initial passing seven include real computed
  button spacing by type and leading-icon presence, stable changing labels, all
  three split-button workflows (including painted corners), all 12 list routes at
  1280/1440px, and utility dialog draft/focus checks. This is not a full-suite result.
- Evidence: `target/m7-md3-button-padding-20261030/`; final grid screenshots in
  `grid-final/` were visually checked. Existing port-1420 dev received HMR; each
  completed test run needed cleanup of its verified workspace-owned port-4173 Vite
  process. No native rebuild, real project mutation or screen-reader acceptance.

### Historical width correction — 2026-09-19

The following results describe the former equal-spacing candidate, not the current
per-type MD3 geometry:

- `npm run check`: PASS.
- Initial sandbox `npm run build`: blocked by esbuild parent-directory access denial. Real Windows user environment build: PASS, with the existing large-chunk warning.
- Material foundation + utility pages: **17/17 PASS**, including two new regressions checking all four variants with/without icons, changing-label accessible names and neighbor positions, and Resources navigation before/after selection.
- Pagination: **3/3 PASS** after explicitly listing all three paging labels.
- Utility browser screenshots refreshed under `target/m7-v3-recheck-20260919/browser/`; full-window Repositories screenshot inspected. These use browser fixtures, not a real Tauri/daemon acceptance session.
- No screen reader run. Accessible-name assertions do not establish actual screen reader behavior.

The current correction is organized into owner-requested local commits. No push.
Visual Gate 2 remains **PENDING**.
