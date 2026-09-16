# M7 package candidate evidence — Stop A proposal

Status: **DRAFT / NOT APPROVED — contract review only**, 2026-09-17.
The owner authorized completion of this proposal, proposed Schema/vectors and current
completeness records. This is not authorization for production wiring. `docs/status.md`
remains the sole current-stage entrypoint; this supplements the existing H2-A contract,
not a new M7 plan. Visual Gate 2 remains **PENDING**.

## 1. Decision and existing evidence

Recommend one additive read-only method `packages.queryProjectCandidates` with `summary`
and `versions` views. Keep existing DTOs and methods unchanged.

| Alternative | Cost / decision |
| --- | --- |
| Add classification/order fields to `repositories.packages` | Useful raw metadata, but does not answer Project Unity, installed relation, user packages or cross-source ambiguity. Its existing lexical `(package_id, version_text)` cursor must not become a SemVer cursor. Adding Project parameters would change its responsibility and permissions. Rejected as insufficient. |
| Extend `projects.get` | Every project snapshot caller would inherit catalog work, additional read permissions and new size/paging concerns. Rejected as broader. |
| One Project candidate query | One bounded read combines existing normalized evidence and exposes lazy versions plus summary. Recommended; no catalog persistence or new resolver. |

Capability comparison: `repositories.read.v1` and `packages.user-packages.v1` each cover
only one source family; `packages.plan.v2` means mutation planning, not this query.
After that comparison, propose **`packages.candidates.v1`** for feature detection, with no
RPC envelope/protocol version bump. Require **all four existing permissions**:
`projects.read`, `repositories.read`, `packages.read`, `settings.read`, on the current
principal. No new permission, `packages.manage`, owner-only bypass or GUI-only route.
An old daemon lacking the capability yields unavailable evidence; clients must not invent
Latest or fall back to string comparison. Active capability registries are not changed here.

Audited existing code: protocol `RepositoryPackageVersion`, `UserPackageRecord`,
`ProjectSnapshot`, `PackageSourceSelector`; store `m3.rs::list_repository_packages` and
`m4.rs::resolver_catalog`; VPM `resolver.rs::candidate_satisfies/candidate_order`,
`range.rs::Predicate::Exact`, `engine.rs::validate_action_direction` and bulk validation;
application `m4.rs` Plan catalog-completeness check and `m7_official.rs` ConfigSnapshot.
Repository versions currently use lexical pagination; UserPackageRecord lacks prerelease
and Unity evidence. Neither is an authority for GUI Latest/update comparison.

Frozen v3 audit source: `4aa98ae4f18d42c10137278997180dbede991e88`, read-only
`../ALCOMD3-v3-readonly/vrc-get-gui/app/_main/projects/manage/`:
`-package-list-card.tsx`, `-collect-package-row-info.ts`, and
`vrc-get-gui/src/commands/project.rs`. v3 has backend candidate ordering, compatible Latest,
full-list Update All and full-selection bulk checks. It deduplicates bare versions: that
implementation must NOT be copied because it loses source identity. v3-specific Unity/SDK
exceptions are not adopted. No v3 or vrc-get source is copied, rewritten or ported.

## 2. Closed DTO proposal

Normative machine shape: `specs/rpc/m7-package-candidates.proposal.schema.json`.
Objects reject unknown fields. Core validation of existing project/package/source IDs,
uniqueness by packageId and the cross-field invariants below remains mandatory in addition
to JSON Schema. Examples/vectors are in the neighboring `.contract-vectors.json` file.

`ProjectCandidateRequest`:
- `projectId`, `expectedRevision`: existing Project identity and revision; required.
- `view`: `CandidateSummaryView` or `CandidateVersionsView`.
- `limit`: optional integer 1..128, default 64.
- `expectedSnapshot`: optional 64-character lowercase SHA-256 equality fingerprint.
- `cursor`: optional `CandidateCursor` (snapshotFingerprint, queryFingerprint, offset).

`CandidateSummaryView`: `{kind:"summary", packageIds?, sources?}`.
Explicit packageIds: 1..256 distinct valid IDs; every ID returns a row, including absent
packages. Omission enumerates the union of locked IDs, direct IDs, and IDs in the visible
registered source snapshots. `sources` is at most 256 unique `{packageId,source}` overrides;
with explicit IDs it must be a subset. A stale/missing/hidden override does not silently
fall back to another source. Summary pages sort packageId by UTF-8 bytes ascending.

`CandidateVersionsView`: `{kind:"versions",packageId,source?}`. Omit source to show each
visible provider; specify an existing selector to inspect that provider. Each version/source
record stays separate. Selectors preserve existing wire spelling:
`{kind:"repository",repository_id:"..."}` or
`{kind:"user_package",user_package_id:"..."}`. No path, URL or arbitrary range input.

`ProjectCandidatePage`: projectId, `snapshot` (fingerprint, projectRevision, configRevision),
`showPrerelease`, `catalogComplete`, `view`, `items`, `nextCursor` (null means complete).
Versions view also echoes packageId. All rows in all accumulated pages must share snapshot.
`catalogComplete` is the existing principal-wide resolver-ready check, including hidden and
unrelated repositories. False is a known Plan blocker, NOT proof a given package has no
candidate; do not call Plan simply to rediscover this blocker.

`PackageCandidateSummary`: packageId, `direct` boolean, `installed`,
`latest`, `latestStable`, `projectLatest`, `projectLatestStable`, `update`, `stableUpdate`.
Installed is `{kind:"absent"}`, `{kind:"locked",version}`, or
`{kind:"unknown",reason:"installed_evidence_unknown"}`. Never infer a locked version from
a direct range. An absent lock is not automatically an absent direct requirement (`direct`
remains separate). An invalid/unreadable normalized project observation fails the query
using existing Project read errors rather than pretending every package is uninstalled.

`PackageCandidateEvidence`: version, source selector, sourceRevision,
`classification` = stable | prerelease | unknown;
`relation` = not_installed | newer | same_precedence | older | unknown;
`unity` = not_required | compatible | incompatible | unknown;
`eligibility` = eligible | blocked | unknown; `reasons` = distinct reason codes.
This is direct eligibility only: known valid version/classification, visible selected source,
current prerelease policy, not yanked, supported normalized metadata, and direct Unity check.
The versions view retains yanked, policy-excluded prerelease and unknown entries for explanation
but disables them. Hidden sources are omitted, not just disabled. Each source's valid
versions are in Core SemVer precedence descending; combined pages use that precedence,
then existing numeric priority, then source kind/ID and full version UTF-8 order solely for
deterministic display. Invalid versions sort last, source kind/ID then raw version bytes.
Display ties never resolve source/build ambiguity. Core computes every classification,
relation and comparison; GUI never parses SemVer/Unity.

`CandidateChoice`:
- `{kind:"candidate",candidate:PackageCandidateEvidence}`: unique known choice.
- `{kind:"none",reasons:[...]}`: affirmative known-empty eligible set.
- `{kind:"unknown",reasons:[...]}`: missing evidence could change the answer.
- `{kind:"ambiguous",reason:"source_ambiguous"}`: existing Core cannot pick a unique
  candidate at the preferred highest precedence; versions view explains the alternatives.

`PackageUpdateEvidence`:
- `{kind:"target",candidate:...}`: project choice eligible AND relation newer.
- `{kind:"no_update",reason:"same_precedence"|"installed_newer"|"not_installed"}`.
- `{kind:"unavailable",reasons:[...]}`: no target, explicit affirmative or unknown reasons.
Target must exactly equal the corresponding project choice; it is not an Apply permit.
Known same/older outcomes retain the candidate in projectLatest for display.

Closed reason vocabulary: `no_visible_candidate`, `source_unavailable`, `all_yanked`,
`prerelease_excluded`, `stable_unavailable`, `unity_incompatible`, `project_unity_unknown`,
`catalog_incomplete`, `metadata_invalid`, `classification_unknown`,
`installed_evidence_unknown`, `legacy_metadata`, `source_ambiguous`.
Reasons report all applicable independent blockers in this vocabulary order; nonempty for
blocked/unknown/none/unavailable, empty for eligible. Unknown takes precedence over known
empty when missing relevant evidence could change the result. No missing prerelease field
is interpreted as stable. `legacy_metadata` blocks this UI's candidate target without
changing the existing Plan error. Incomplete relevant repository normalization yields
unknown rather than a lexical fallback. An unrelated incomplete repository sets only
catalogComplete=false; it does not erase otherwise known local candidate evidence.

### Wire payload examples

Method `packages.queryProjectCandidates`, params (existing RPC envelope omitted):

```json
{
  "projectId": "11111111-1111-4111-8111-111111111111",
  "expectedRevision": 7,
  "view": {
    "kind": "summary",
    "packageIds": [
      "com.example.package"
    ]
  },
  "limit": 64
}
```

Result excerpt (the vector file contains all four choices, stableUpdate and paging fields):

```json
{
  "projectId": "11111111-1111-4111-8111-111111111111",
  "snapshot": {
    "fingerprint": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "projectRevision": 7,
    "configRevision": 3
  },
  "view": "summary",
  "showPrerelease": false,
  "catalogComplete": true,
  "items": [
    {
      "packageId": "com.example.package",
      "installed": {
        "kind": "locked",
        "version": "1.9.0"
      },
      "update": {
        "kind": "target",
        "candidate": {
          "version": "1.10.0",
          "source": {
            "kind": "repository",
            "repository_id": "22222222-2222-4222-8222-222222222222"
          },
          "sourceRevision": 12,
          "classification": "stable",
          "relation": "newer",
          "unity": "compatible",
          "eligibility": "eligible",
          "reasons": []
        }
      }
    }
  ],
  "nextCursor": null
}
```

This labeled excerpt is not a complete Result fixture; the complete shape is validated from vectors.

## 3. Exact sets and selection rules

Let V be all recorded entries for this package from sources visible under **confirmed**
hidden-repository/hide-local-package settings and any explicit source selector. Local
search, status and source-kind filters do not change V. Use enrolled normalized user
snapshots, not the mutable source folder. User candidates reuse existing resolver
normalization (`yanked=false`, `legacy_metadata_present=false`, Unity minimum from enrolled
manifest); do not invent new user-package manifest policy.

Latest = highest usable Core precedence in V, excluding yanked, unsupported metadata and
unknown classification, honoring confirmed showPrerelease, **before Project Unity filtering**.
Latest Stable = same, stable only regardless of showPrerelease. Unknown relevant evidence
prevents claiming these maxima. Both apply existing highest-precedence THEN source-priority/
ambiguity rules. These display fields intentionally differ from v3's compatible Latest name.

Project Latest / Project Latest Stable = corresponding sets with direct Unity eligibility
applied before choosing maximum. Thus a newer incompatible release remains visible as Latest,
while a lower compatible release can be the update target. When Unity is unknown and a
potential winning candidate has a minimum, project choice is unknown; do not select a lower
known candidate while a higher unknown might win. No minimum means `not_required` and passes
even with unknown Project Unity. Existing Core compares only Unity major/minor minimum;
no new unityRelease interpretation or v3 SDK exceptions.

Within the maximum precedence group, reuse Core minimum numeric repository priority.
Without explicit source, a repository/user mixture is ambiguous even if repository priority
would otherwise favor one. More than one preferred record is ambiguous, including same-source
build variants. Explicit source can resolve cross-source ambiguity, not build ambiguity.
SemVer build metadata does not affect relation. Existing `=version` matches precedence,
NOT exact build identity. Keep each version/source option, but block an option's submission
when same-source equal-precedence alternatives remain. Core determines this over the complete
same-source matching set before pagination and returns eligibility=blocked with
source_ambiguous on every affected versions entry; GUI does not compare pages or parse versions.
Summary maximum selection still reports ambiguous, not an empty set obtained by dropping
these blocked entries. Show the Plan limitation instead of
promising an exact artifact pin. No resolver or range semantics change is proposed.

Installed unknown allows display maxima but produces unknown relation and unavailable update.
Yanked/Unity-incompatible/unknown entries never become automatic targets. `none` is not used
for missing evidence. An ambiguous or unknown project choice yields unavailable update with
its reason; a known-empty choice carries its reasons; absent/same/older yields no_update.
A target is only direct candidate eligibility. Dependencies, ranges elsewhere in the graph,
source changes and project changes are still decided by existing Package Plan and Apply.

## 4. User actions (normative)

All enabled mutation actions require current project revision/snapshot, required existing
mutation capability/permissions, no conflicting active project mutation and explicit Plan
review. Loading/stale/unknown evidence disables candidate-dependent actions with a reason.
No query creates a durable Plan, Operation or Event. Plan is created only after user intent.
All Apply flows keep existing confirmation, expectedRevision and idempotency behavior.

| Action / visibility | Enable and target/source | Empty / partial handling | Existing Plan |
| --- | --- | --- | --- |
| Inline versions, including User Package; in the existing Installed entry for a row with recorded choices | Core ordered pages, each source/version separate. Eligible unambiguous exact option; same full installed version is no-op. Different build with same precedence is not Upgrade/Downgrade and remains non-actionable. not_installed/newer/older use returned relation; explicit source; Install/Upgrade use versionRange=`=version`, Downgrade uses its existing bare `version` field; prerelease intent from Core classification. Incompatible/yanked/unknown shown disabled with reason. | Empty shows no selectable versions; no Plan. Pending pages cannot be labeled complete. Unsupported exact-build pin disabled. | `packages.planInstall` / `packages.planUpgrade` / `packages.planDowngrade`, by relation |
| Single Update; visible only with update.target | Exactly returned project-compatible newer target and source. | No target: contextual reason instead of active Update; no probe Plan. | `packages.planUpgrade` |
| Single stable selection | v3 has no separate single-row Stable button; reuse existing inline stable option. If a separate stable action is later approved, only stableUpdate.target different from ordinary target qualifies. This contract does not redesign/add its placement. | Same/older/absent/unknown no stable upgrade; explicit older inline choice remains Downgrade. | `packages.planUpgrade` for newer stable; inline rules otherwise |
| Update All / stable variant; toolbar target count, stable variant only if it changes at least one target | Complete snapshot over **all locked package IDs**, direct and transitive, independent of selection/search/status/source-kind visibility. Ordinary/stable targets from corresponding evidence; persistent hidden/prerelease settings and explicit row source choices apply. | Known no_update/known none excluded with counts and reasons in review. Any unknown/ambiguous row blocks action until resolved. Zero targets disabled. More than 256 targets disabled, never split; owner may explicitly choose a smaller selected batch. | ONE `packages.planBulk`, upgrade intents |
| Selected install/upgrade; selection toolbar | **All selected IDs**, including search-hidden rows, 1..256. Unlocked rows need eligible project choice; locked rows need update.target. Each exact source retained. Separate install/upgrade filters qualify only if every selected row matches; combined action may mix both intents. | AND across entire selection. Any unknown/ambiguous/same/older/missing/unavailable item disables entire action with reasons. No silently filtered subset. | ONE `packages.planBulk`, install/upgrade intents |
| Selected stable install/upgrade; only where at least one target differs | Same full selection, using projectLatestStable/stableUpdate; otherwise same rules. | All must qualify; zero/no difference no duplicate action. | ONE `packages.planBulk` |
| Selected remove; selection toolbar | All selected IDs must be direct requirements; remove deletes from desired direct set, not arbitrary locked transitive nodes. Does not require candidate evidence, but existing planBulk requires catalogComplete even for remove-only intents. | Empty, missing/non-direct item or >256 disables entire action. No subset. | ONE `packages.planBulk`, remove intents |
| Selected reinstall; selection toolbar | All selected IDs must have known locks; full selection, preserve explicit source if provided, do not infer installed provenance. Catalog complete required; resolver still verifies version/source/content. | Empty/missing/unlocked/>256 disables entire action. Source ambiguity surfaces through existing Plan; no hidden loop. | ONE `packages.planBulk`, reinstall intents |

All table actions (including remove-only Bulk) additionally gate on catalogComplete. Only the
existing single `packages.planRemove`, outside this bulk row, bypasses that check. Candidates can still be displayed
when the global catalog is incomplete. Existing refresh is explicit. Unknown actual archive
availability is a Plan concern, not something this read-only query downloads to prove.
Selection remains keyed by package ID across visual filters. A selected ID disappearing from
authoritative data invalidates the action; only explicit deselection changes the selected set.
Changing a source setting invalidates evidence, not selection. Each bulk is exactly one Bulk
Plan → one explicit Apply → one Operation; not a claim of stronger transactional guarantees.

## 5. Snapshot, quotas and errors

Fingerprint binds current principal, normalized Project observation/revision/fingerprints,
registered repository and user-source inventory, their existing revisions/fingerprints,
repository priorities/readiness, and existing ConfigSnapshot revision plus relevant settings.
Use length-delimited canonical sorted records and SHA-256; revision/fingerprint materials
already exist. No persistent global catalog revision, snapshot table or token cache. Fingerprint
is an equality token, not authorization, and cannot be passed to Apply. It covers all sources
including ones not returned so adding/removing a provider invalidates pages.
Read one coherent existing database snapshot and immutable Config snapshot; recheck Config
revision before returning and fail stale if changed. Never combine old Project/new source
rows. No project/source filesystem refresh or mutable user directory reads. Unrefreshed
external files remain outside observed evidence; later Plan/Apply validation is unchanged.

Cursor `{snapshotFingerprint,queryFingerprint,offset}`: queryFingerprint binds projectId,
view, sorted packageIds and source overrides; excludes limit. Cursor is a bounded pagination
position, not authority. Reauthorize and recompute both fingerprints every page; reject changed
query/invalid offset with existing invalid-params behavior, changed snapshot with
`package_candidate_evidence_stale`. If expectedSnapshot and cursor disagree, invalid params.
First page may omit expectedSnapshot; every later page or related request uses the first token.
Events, reconnect and stale errors discard the whole accumulated set. No durable cursor state.

Frozen proposed limits: request JSON <=64 KiB UTF-8; response result <=1 MiB UTF-8 (below
existing 4 MiB frame); page default64/max128; explicit summary IDs/source overrides max256;
source descriptors read max4096; relevant normalized version records inspected max100000;
page offset <=100000. Exceeding any work/size quota returns
`package_candidate_limit_exceeded`, no successful truncation. Traversal totals at most
100000 items/1563 default-size pages; client hard maximum 1563 pages per traversal, so a
smaller chosen page size may require restarting at default64. No unbounded catalog drain:
summary pages omit version arrays; version pages load only for an opened row. At most two
candidate requests in flight per GUI, at most one in flight per traversal, no polling/retry
loop; one stale response invalidates and waits for explicit reload or one debounced event reload.
Update All requests explicit locked IDs in <=256-ID batches, token-bound, and finishes all
pages/batches before enabling; >100000 total summary IDs aborts with visible limit. Existing
project snapshot limits still apply. Explicit versions view is lazy; no per-row Plan probing.

Existing authentication/permission/project-not-found/revision-conflict/invalid-params errors
are reused, without introducing aliases. The only proposed new stable errors are
`package_candidate_evidence_stale` and `package_candidate_limit_exceeded`; error details expose
no paths or metadata. Size failure is not `none`; missing metadata is not an empty response.

## 6. Required future production work and proof boundary

Only after this precise contract is approved: add protocol DTO/method/capability/error entries,
application read use case, read-only store projection, reuse/refactor existing pure VPM
ordering/classification helpers without changing their behavior, daemon dispatcher/permission
mapping, public SDK types/method and thin GUI query adapter/candidate actions. No new
production dependency, unsafe, platform API, State/Config migration, persistent table or
capability/permission grant shortcut. CLI/MCP/Local API, where exposed, use this same use case.

Future tests in the contract-vector file are **planned expectations, not runtime PASS**.
Prove zero writes using DB/files/cache fingerprints and Plan/Operation/Event counts before/
after query, denial, paging and failure. Assert zero network/downloader/refresh calls. Repeat
unchanged resolver/Plan/Apply regression vectors, including source priority and exact-build
ambiguity, dependency conflict despite eligible candidate, global incomplete catalog, stale
project/source at Plan/Apply. Query tests must not weaken those errors. Exercise permission
subsets, principal isolation, negative DTO shapes, real page sizes, work caps and snapshot races.
GUI fixtures must consume Core responses, not manufacture a second comparator. Confirm full
selection vs search-visible subset, direct-only removal and one Bulk request/Apply/Operation.

## 7. Current WIP, completion and acceptance

Preserve current inline repository/source entry, complete selection across search, single
Bulk remove, user-source downgrade fix and 52/52 browser evidence. Manual version entry is a
**transitional fallback**, not final inline closure. Conditional Update/Stable/Update All,
full selected install/upgrade and User Package inline remain unfinished; existing lexical
Latest/different-version logic is not acceptable authoritative evidence. Production proposal
wiring is absent. Historical P6/P8 PASS remains historical, not current H2-A completion.

Actual CSS audit: current `styles.css` has zero diff against f86937b; both Git blob hashes
are `1d9298767cb9074cae033893fdb7730a98f37d19`. Earlier transient editing removed the
`.package-action-kind, .package-action-id, .package-action-version { width:100%; }` rule
and `.package-action-version` from the media selector while dropping the manual dialog;
retaining that fallback restored these lines. No such CSS change remains to submit. Do not
confuse older committed toolbar/search CSS changes with this working-tree diff.

Menu host disabled/aria-disabled and click guard/browser tests do not prove the internal
Material menuitem is announced disabled by a screen reader. Keep that evidence open.
Computer-use authorization timeouts mean desktop evidence was not obtained, neither functional
failure nor acceptance. Do not retry authorization for contract review. The owner may manually
launch a specified accepted build, run the checklist and provide screenshots; computer-use
is not mandatory and its authorization must not be bypassed. Visual Gate 2 remains PENDING.

Contract validation only: proposal shape/positive and negative vectors, metadata, xtask,
baseline freeze and diff checks. No production/GUI or desktop execution claim is derived
from these checks. Results are recorded in `docs/testing/m7-package-candidates-stop-a-review.md`.
Stop after these materials for human approval; no production implementation or push.
