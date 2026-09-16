# M7 H2-A candidate evidence — Stop A review record

Date: 2026-09-17. Status: **CONTRACT REVIEW READY / NOT APPROVED**.
Current stage is exclusively `docs/status.md`; Visual Gate 2 remains PENDING.

## Scope and baseline

Started on main at `f86937bbf6ac2716c12dd1968a1fbd908ce4872a`, ahead of the local
origin/main tracking ref by 8 commits. The working tree already contained GUI, browser-test,
M7 ExecPlan/status and inline-version report WIP. It was not clean and was not reset.
Only the existing standard workspace was used; no worktree was created or restored.

The owner authorized proposal completion, not production implementation. The existing Stop A
file now freezes the single read method, permission/capability comparison, closed DTOs,
ordering/selection sets, full action matrix, snapshot/paging/quotas, and future proof cases.
No active RPC registry, SDK, daemon, resolver, Plan/Apply, database, dependency, platform API,
icon or layout implementation was changed by this contract pass.

## Materials and review corrections

- `docs/exec-plans/M7-package-candidate-evidence-stop-a.md`: existing proposal completed.
- `specs/rpc/m7-package-candidates.proposal.schema.json`: inactive closed proposed DTO schema.
- `specs/rpc/m7-package-candidates.contract-vectors.json`: concrete positive/negative shapes
  and separately labeled future semantic expectations.
- `scripts/validate-m7-package-candidates.ps1`: PowerShell built-in Test-Json validation;
  no added dependency and no runtime RPC calls.
- Existing completeness fixture gains `currentH2AAssessment`; historical P6/P8 verdicts remain
  byte-for-byte in meaning, and the test-plan description explicitly limits their scope.
- `docs/status.md` records the current gaps and contract-only approval boundary; its previous
  GUI WIP remains unstaged rather than being bundled into this independent contract commit.

Independent read-only Core review corrected three details before submission: remove-only Bulk
still requires a complete catalog; Downgrade uses bare `version` rather than `versionRange`;
and same-source build ambiguity is computed before pagination and explicitly blocks affected
version entries. Source selector wire keys retain the existing snake_case spelling.

## Validation

- Proposal shape validation: PASS, 21 positive/negative cases. Test-Json compiles the proposed
  schema and validates each case against its selected Request/Result definition.
- Semantic vector inventory: 39 complete named expectations; **not executed production tests**.
- `cargo xtask check`: PASS.
- `scripts/validate-metadata.py` using the existing bundled Python: PASS.
- `scripts/freeze-baselines.ps1 -Check`: initial sandbox run failed Git dubious ownership for
  the read-only v3 checkout; real-Windows-user rerun PASS. No safe.directory mutation, baseline
  update or reference-repository write was needed.
- `git diff --check`: PASS; Git line-ending normalization notices are not whitespace failures.
- Recorded SHA-256 comparison of the five pre-existing GUI/test WIP files plus styles.css:
  unchanged throughout this contract pass. Styles Git blob and HEAD blob both
  `1d9298767cb9074cae033893fdb7730a98f37d19`; current CSS diff is empty.

No Rust workspace/GUI/browser/desktop test was rerun in this contract-only pass. Prior 52/52
browser evidence remains a historical local checkpoint. No candidate query is wired or
advertised, and shape validation cannot establish candidate algorithm correctness.

## WIP and evidence still open

Preserved: inline repository version/source entry, full selection across search, single Bulk
remove, user-source downgrade fallback fix and associated browser regressions. Manual version
entry is transitional. Candidate reads, conditional Update/Stable/Update All, full selected
install/upgrade and User Package inline selection are still not closed.

Earlier transient CSS edits removed the action-field width rule and action-version media
selector while removing the manual dialog; restoring the fallback restored those lines.
They are absent from the actual diff. Older committed toolbar CSS is not this WIP.
Material host disabled/click guard evidence does not prove internal menuitem disabled
announcement by a screen reader. Computer-use app authorization timeouts mean desktop
evidence was not obtained, not functional failure or acceptance. No repeat authorization
attempt was made. The owner may manually launch a specified build, run the checks and submit
screenshots later. Visual Gate 2 remains PENDING.

This record is prepared for an independent local contract-only commit with explicit file
staging. Existing GUI WIP and its earlier report/ExecPlan/status changes are excluded from
that commit. No push or remote write. Stop here for approval of the precise proposal.
