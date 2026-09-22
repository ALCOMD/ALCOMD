# M7 shared button width correction — 2026-09-19

Owner request: fix width changes between selected/unselected buttons at the shared component level, and keep changing button labels from shifting neighboring controls.

Base: `a531e565a216e30079de2c29fcc1a8c5ea2e8253`, `main`. Frontend-only working-tree change; no RPC, dependency, resolver, Plan/Apply, icon asset or persistence changes.

## Implementation

- Material Web text and tonal buttons have different default horizontal padding. The shared `@alcomd/ui` preset now defines the same horizontal spacing for filled, tonal, outlined and text variants: 24px without an icon; 16px on the icon side and 24px opposite. Existing table-sort-specific spacing overrides remain intact.
- Shared React `Button.widthLabels` reserves the intrinsic width of all known state labels using overlapping grid cells. Measurement labels are hidden and `aria-hidden`; the visible label remains the accessible name. Slotted icons remain direct children of the Material host.
- Existing refresh, save, confirmation, planning/apply, favorite and pagination state labels supply their alternatives. Pagination includes loading, retry and normal labels. This is intrinsic text sizing, not a hardcoded pixel width; unknown/unbounded future labels require the caller to supply suitable alternatives or a separate bounded-width design.
- Button action handlers, disabled conditions and backend calls are unchanged. No general page/layout redesign accompanies this fix.

## Evidence

- `npm run check`: PASS.
- Initial sandbox `npm run build`: blocked by esbuild parent-directory access denial. Real Windows user environment build: PASS, with the existing large-chunk warning.
- Material foundation + utility pages: **17/17 PASS**, including two new regressions checking all four variants with/without icons, changing-label accessible names and neighbor positions, and Resources navigation before/after selection.
- Pagination: **3/3 PASS** after explicitly listing all three paging labels.
- Utility browser screenshots refreshed under `target/m7-v3-recheck-20260919/browser/`; full-window Repositories screenshot inspected. These use browser fixtures, not a real Tauri/daemon acceptance session.
- No screen reader run. Accessible-name assertions do not establish actual screen reader behavior.

Visual Gate 2 remains **PENDING**. No push or new local commit in this correction.
