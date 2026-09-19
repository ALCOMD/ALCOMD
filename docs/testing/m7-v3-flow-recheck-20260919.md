# M7 v3 flow recheck — rejected utility-page baseline

Date: 2026-09-19. This is a source-based audit of the uncommitted GUI above
`a41c915106ed647b14d7eef0b89b24c065c3e17d`, before the current corrective rewrite.
It is not a visual acceptance result. The owner rejected the preceding utility-page
rework: modal workflows and Settings were not reproduced, and checking only those
two examples would not satisfy the request. Visual Gate 2 remains PENDING.

Only this audit document was written by the audit subtask. No v3 implementation was
copied or modified. Source locations below refer to the frozen sibling checkout,
under `../ALCOMD3-v3-readonly/vrc-get-gui/`. Current implementation locations refer
to `apps/alcomd-gui/src/`. A generic dense table is not evidence of matching v3:
v3 uses different structures for repositories, templates, extensions and settings.

## Audit matrix

`Frontend` means the existing client can support the correction without a new
public contract. `Gap` means the cited GUI client/DTO does not supply the complete
v3 behavior; this audit does not approve inventing an RPC, changing a source rule,
or adding a platform picker. Settings has a separate detailed recheck and is not
claimed covered by this matrix.

| Flow | Frozen v3 source / observed design | Rejected v4 implementation | Correction and existing support / gap |
| --- | --- | --- | --- |
| Repository toolbar | `app/_main/packages/repositories/index.tsx:505`: heading selector, sorting mode, Add with import/export overflow | `CorePages.RepositoriesPage`: Add plus generic Refresh; revision column; no visibility or contextual remove | Frontend: keep repository actions with their rows and Add in heading; `settingsGet/settingsUpdate` owns visibility; it does not expose repository priority editing. Do not silently redefine priority via display sorting. |
| Add repository | `.../repositories/-use-add-repository.tsx:47,75,161,496`: one modal proceeds input → loading → repository confirmation; Cancel exits before saving; errors/duplicates have their own state | `CoreActions.RegisterRepositoryPanel:93`, `UtilityWorkspace:105,127`: expands inline input then opens a generic second confirmation | Frontend: modal entry and review using `repositoriesInspect`, then explicit `repositoryRegister`; inspect evidence can show name/source/issues. Preserve failed state for correction and cancellation. Do not claim an inspection is a durable Plan. |
| Add headers | `.../-use-add-repository.tsx:251`: optional header rows with validation | `RepositorySource` contains only local path or remote URL (`core-models.ts:204`) | Gap: headers are not represented by current client DTO. Do not add fake editable headers or send ignored values. |
| Repository package list | `.../repositories/index.tsx:1166`, `-repository-package-list.tsx:6`: repository-named modal with scrollable package names and Close | `RepositoryDetailPage`: navigates to a generic detail route showing revisions and a version table | Frontend: open list in a modal anchored to selected repository, keep parent list context; `repositoryGet/repositoryPackages` supports bounded loading. A retained deep link may remain secondary, but is not the primary v3 interaction. |
| Repository removal | `.../repositories/index.tsx:1200`: named confirmation modal | `RepositoryActions` has confirmation but is buried under detail-page disclosure | Frontend: contextual row action with named confirmation, `repositoryUnregister(id, revision)`; Cancel performs no write. Keep installed-project effects text accurate. |
| Repository display name | `.../repositories/index.tsx:1052`: edit-name modal, Cancel/Save | No current equivalent | Gap: snapshot has name but current client has no display-name mutation. Do not mutate catalog identity or store an authoritative alias in localStorage. |
| Repository import/export | `.../repositories/index.tsx:539`, `-use-import-repositories.tsx:67`: split menu; file picker, selected-list review, loading/progress/cancel | No equivalent | Gap: current GUI lacks repository-list import/export RPC/picker flow. Do not wrap repeated registration as an atomic bulk import. |
| User Package addition | `app/_main/packages/user-packages/index.tsx:57,88`: heading Add invokes native directory picker; cancellation is a no-op | `UserPackageManager` correctly uses `selectDirectory/userPackageEnroll`, but action is within body, below heading | Frontend: preserve working picker semantics and move entry to heading action region; no invented source-path form necessary. |
| User Package removal | `.../user-packages/index.tsx:206`: row remove opens named modal, Cancel/Remove | `UserPackageManager.remove`: immediate `userPackageRemove` from Remove enrollment button | Frontend, high priority: restore confirmation with package/source identity; do not remove before confirmation. Preserve error and disable duplicate invocation. |
| User Package table | `.../user-packages/index.tsx:143,180`: name/version/source-oriented rows and contextual remove | Current source cell shows generic Local/User Package plus revision | Frontend can remove low-value revision emphasis. Gap: do not invent a source folder if returned `UserPackageRecord` has no path. |
| Template heading | `app/_main/packages/templates/index.tsx:85,524`: Create plus import overflow; creation opens editor | `TemplatesPage`: only Import; `TemplateDetailPage` exposes Derive from project | Partial: existing `templatePlanDerive` can be a clearly named top-level “Create from project” modal, using actual registered project selection and revision. It is not v3 arbitrary template editing. |
| Template table and favorite | `.../templates/index.tsx:130,317,434`: name, ID, modified, category; star and row overflow | Current table version/source/View template; favorite buried in detail | Frontend: restore available ID/updatedAt/source grouping and inline favorite using `templateSetFavorite`; avoid inventing v3 category mappings unsupported by `sourceKind`. |
| Template edit/duplicate | `.../templates/index.tsx:141,156,563,856`: modal general info, base template, Unity requirement and package rows; Cancel/Save | No template editor; detail derive/create/export forms are not equivalent | Gap: `templateGet` returns record metadata, not editable manifest; current client exposes derive/import/export/remove, not arbitrary edit/duplicate. Do not relabel derive as Edit. |
| Template import | `lib/import-templates.tsx`, `.../templates/index.tsx:85`: modal import workflow from heading menu | `TemplateImportPanel`: inline bundle path, expected revision number and override; generic plan fingerprint review | Frontend: modal with actual `templateInspectBundle` evidence, then existing Plan/Apply; manual expected revision is not acceptable ordinary UI. Gap: list result has no registry revision; inspect/get rules must be checked before removing that technical input. Do not substitute guessed max(record.revision). |
| Template export | `.../templates/index.tsx:434`: row overflow, not a persistent form in every detail | `TemplateActions`: always-mounted export path form | Frontend: contextual export dialog, `templateExport` with exact chosen template revision. Existing client has directory picker only; do not claim a native file Save picker already exists. |
| Template remove | `.../templates/index.tsx:265,396`: confirmation modal naming target; builtin/category restrictions | `TemplateActions.simple('remove')`: immediate mutation | Frontend, high priority: named Cancel/Remove confirmation and existing source restrictions, use `templateRemove`; retain failed state. |
| Create project from template | v3 primary project creation flow; template-specific actions should retain selected-template context | `TemplateActions`: inline parent/name under arbitrary template detail | Frontend: modal context with selected template plus parent chooser using existing `selectDirectory`, then actual `templatePlanCreateProject` review. Do not change accepted Projects layout. |
| Extensions page structure | `app/_main/extensions/index.tsx:440,528,568,600`: management sections, named cards, enable switch, Open action | `ExtensionsPage`: generic ID/version/state table then separate detail; common lifecycle actions hidden | Frontend: extension cards with real runtime/trust status, inline enable/disable and Portable UI Open only where supported. `extensionEnable/Disable` and `extension.ui` already support these. Do not copy v3 built-in sidebar definitions into v4 host. |
| Extension groups/catalog | `.../extensions/index.tsx:520,552`: Installed and Not installed sections | `extensionsList` exposes registry records, not a discoverable full catalog | Gap: no invented uninstalled catalog or hardcoded first-party identities. Group only what authoritative records establish. |
| Extension sidebar ordering | `.../extensions/index.tsx:285`: sorting modal with reset/cancel/save | No equivalent | Gap: no current extension-order mutation in `GuiRpcClient`; do not store authoritative order as disposable localStorage. |
| Extension install | v4-specific secure lifecycle; v3 page is not an arbitrary package installer | `ExtensionInstallPanel`: inline package path, manual registry revision, trust checkbox | Frontend: dedicated modal retaining explicit publisher approval and real install Plan review. Registry revision discovery is a contract/read-model question, not a CSS fix; no arbitrary revision default masquerading as evidence. |
| Extension permissions/uninstall | v4-specific public extension security model | `ExtensionActions`: inline technical permission/resource-ID form; delete-data checkbox always on detail | Frontend: separate named permissions and uninstall dialogs, keep actual scope and data disposition explicit; `extensionSetGrant/RevokeGrant`, `extensionPlanUninstall/ApplyUninstall` stay authoritative. Typed scope selection can use real projects/extension records, never invented grants. |
| Logs heading | `app/_main/log/index.tsx:289`: Logs heading, Activity/Technical choice, search; technical controls in same toolbar | `ActivityPage/DiagnosticsPage`: alternate links and table, no search | Frontend: shared heading/search can filter explicitly loaded evidence. Gap: `diagnosticsList` is not a raw technical-log stream, so do not rename it Technical logs and claim parity. |
| Activity filters/details | `.../log/index.tsx:345`: source/status/kind plus show secondary/details | `ActivityPage`: summary/time/status/View operation only | Frontend: filters only for returned fields and explicitly bounded loaded pages. Gap: missing source/kind fields must not be reconstructed from codes; current client only accepts cursor. Need honest loaded-scope label if frontend filtering is used. |
| Technical log actions | `.../log/index.tsx:441`, `-logs-list-card.tsx:46`: level filters, auto-scroll, time/level/message, folder access | `DiagnosticsPage` adds Run state check | Gap: state check is a separate operation, not technical-log viewing. No directory opener/raw stream in current GUI client; keep state-check identity distinct. |
| Backup preparation | `components/BackupProjectDialog.tsx:125,225,297`; project manage menu `:703`: context action opens options/name dialog and then progress | `ProjectBackupsPage` body Create disclosure; `BackupCreatePanel` inline options then generic confirm | Frontend: modal preparation with project identity and existing compression/exclude settings, then actual `backupCreate`/Operation feedback. Gap: existing create signature has no custom archive name; do not display an ignored name field. |
| Backup restore | `components/RestoreProjectFromBackupDialog.tsx:110,203,276`: selection then name modal and progress/cancel | `BackupDetailPage` contains permanent parent/leaf restore form | Frontend: Restore opens modal anchored to actual backup, parent chooser plus leaf, real `backupPlanRestore` Review, Cancel before Apply. Preserve current registered-backup model; do not imply arbitrary archive picker support. |
| Backup progress/cancel | `components/BackupProjectDialog.tsx:115,164`; restore component `:88,276`: clear running/cancelling states | `MutationFeedback/OperationFollow` embeds operation output | Existing `operationGet/operationCancel` supports truthful progress and cooperative cancel. Closing a preparation/review dialog is not cancellation of an accepted Operation. Preserve that distinction; no fake completed/cancelled state. |
| Project launch arguments | `app/_main/projects/manage/index.tsx:608,627,640`: launch-options modal with scroll area, Cancel/Save | `ProjectUnityPage/ProjectUnityActions`: separate route, permanent textarea/save/clear | Frontend: launch-arguments dialog using current launch-config read/set/clear, draft reset on Cancel; do not restore obsolete Automatic/Explicit editor model or change accepted package toolbar. |
| Project Unity migration | `app/_main/projects/manage/-unity-migration.tsx`; approved v4 Unity contract supersedes v3 execution internals | Current workspace already has version chooser and actual migration Plan modal, secondary page exposes extra permanent controls | Preserve authoritative v4 exact-version and Plan/Apply flow. Audit secondary route composition without changing resolver, launch model or accepted Projects controls merely to mimic obsolete v3 behavior. |

## Cross-cutting rejection criteria

- `UtilityWorkspace.actionOpen` / `utility-workspace-panel` and `ActionDisclosure`
  currently implement in-page expansion. Changing their padding cannot restore a
  modal workflow. A modal needs retained parent context, a meaningful title, draft
  lifetime, focus ownership/restoration, Escape/Cancel, and the relevant submit or
  review state. Do not place a second overlapping modal over the first form.
- A generic `ConfirmDialog` closes in `finally`. This should be reviewed for the
  changed flows: a failed mutation must not appear successful or make its context
  and error inaccessible. Repeated activation must not create duplicate Plans or
  writes. Busy close and close-after-read behavior need explicit tests.
- `TemplatePlanDialog` currently shows action/fingerprint only. A technical hash
  is not a useful user review; show actual known source/target/template identity,
  with backend Plan as the execution authority. Do not invent missing Plan fields.
- Visible revisions, IDs and raw permission strings are not substitutes for user
  selection. Read existing snapshots when they supply identity/revision. Where a
  registry revision is absent, record the exact gap instead of guessing it.
- Preserve known working Projects, source identities, candidate evidence,
  Plan/Apply and extension permission boundaries. Browser fixtures can prove UI
  behavior and RPC intent; they do not prove real Tauri desktop or screen-reader
  acceptance. Historical test counts remain historical, not design compliance.

## Follow-up validation required

For each corrected entry: open from its actual parent row/toolbar, inspect empty,
loaded and error states, verify Cancel/Escape makes no mutation, verify focus return,
verify repeated submit is single-action, verify the review displays the real Plan
where that use case has a Plan, and verify failed operations retain an explicit
error. Capture full-window browser screenshots with each representative modal open
and with the parent page visible behind it. Separately retain the missing desktop
and screen-reader evidence. This document does not assert those checks have run.
