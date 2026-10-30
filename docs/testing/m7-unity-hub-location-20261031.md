# Unity Hub location — approved implementation and local verification

Status: **owner-approved contract implemented; desktop acceptance pending**.
Current scope remains M7 H2-A; Visual Gate 2 **PENDING**.

## Current header and approved Hub open action

The owner requested Auto detect at the card header's right side and a neighboring Open
Unity Hub button, switching to Download Unity Hub if no Hub is specified. Auto detect has
been moved into the existing SettingsSection actions slot; Select remains beside the editable
field. The focus boundary now uses the existing whole section, so header actions, the draft
and Retry retain one group. No CSS or fake Open/Download placeholder was added. Frontend
check/build, all **33/33** existing Hub/autosave cases (including header geometry and revised
Shift-Tab order), and the focused HEX/HCT **1/1** compatibility case passed. Evidence is
`gui-header-{build,browser,theme-field}.log/.exit`; full Chinese/Japanese fixture images
`gui-header-settings-zh-CN.png` / `gui-header-settings-ja-JP.png` were inspected/saved.

The owner explicitly approved the exact `openActionProposal` on 2026-10-31: **“批准，按合同实现”**.
The method, capability, permission, application command, platform launcher and GUI adapters
are now wired under that approval. The existing location/selection contract remains active.

### Active minimum increment

- Method `unity.hub.open`; capability `unity.hub.open.v1`.
- Require `settings.read` and new narrow `unity.hub.open`; initially authenticated
  `builtin:local-owner` only, before any IO. Do not broaden existing `unity.launch`, which
  explicitly covers Project/exact-Editor authority, or settings/manage rights into launch.
- Params `expectedRevision` and nullable `expectedPath` from accepted Core evidence only.
  Core rereads current settings, inspects the effective location, verifies revision/path,
  and launches only that Core-resolved executable with no arguments or shell. Request path
  is a comparison guard, never a command. The existing validator proves usable filesystem
  executable, not vendor/signature or immutable binary identity; expectedPath is not an
  identity guarantee. The action is a local-owner launch of the file chosen/found as Hub.
- Result is `{"outcome":"spawned"}` (OS acceptance, not UI readiness) or
  `{"outcome":"download_required"}`. The latter occurs only with no configured override,
  not_found evidence, and expectedPath=null. Available/discovered is valid even when a
  broken manual preference had a usable default fallback; use exactly the displayed effective
  target without rewriting preferences. Broken manual preference without fallback or I/O
  uncertainty is an error, not a silent download fallback.
- Reuse the readonly Hub inspection port, plus a separate `UnityHubLauncher` command port.
  Platform uses existing std process/thread facilities. Establish a bounded reaper worker
  before spawning; at most four live owned child/reaper pairs; always wait the owned Child
  even if the reply is lost. No cancellation-driven kill of user Hub, no daemon shutdown
  blocked by a Hub lifetime. No automatic retry after unknown outcomes.
- GUI opens only the fixed `https://unity.com/download` through a closed, no-URL-argument
  adapter using its existing `open` dependency after download_required. It opens a page;
  it does not download/install an executable or automate account login. The canonical URL
  is linked by [Unity's official install instructions](https://docs.unity.com/en-us/hub/install-hub-win-mac).
  Tool-following that link reached an inaccessible Unity account redirect; no website-open
  acceptance or binary download is claimed.
- Header labels depend on accepted authority, not unsaved draft. Dirty draft, busy work,
  unavailable evidence or absent capability disables the action with a meaningful reason.
  There is no implicit save-and-launch of a draft, no fleeting success label, no auto-retry.
- No Config/State migration, durable Plan/Operation/Event/table, new dependency, unsafe,
  native API, resolver/source policy or Editor launch changes. No third-party permission
  grants. The method/capability/permission are additive RPC v1 increments; persisted formats
  and existing authority meanings remain unchanged.

```json
{
    "id": "open-unity-hub",
    "method": "unity.hub.open",
    "params": {
        "expectedRevision": 153,
        "expectedPath": "C:\\Program Files\\Unity Hub\\Unity Hub.exe"
    }
}
```

The approved contract includes twelve executable shape vectors and implementation vectors for permission,
revision/path drift, detection/fallback, process ownership/reaping, download URL closure,
dirty-draft focus, duplicate clicks and unknown replies. Shape validation is contract proof
only; implementation tests and actual desktop acceptance remain separate.

Stable errors: existing invalid_request / permission_denied / revision_conflict;
hub_path_changed, hub_path_invalid, hub_unavailable, hub_open_busy and hub_open_failed;
unexpected internal errors retain diagnosticId. Responses/events/ordinary errors do not
gain paths, argv, process metadata or arbitrary URLs. Active schemas, DTOs, Rust/TypeScript
clients, capability negotiation and the permission catalog match this approved increment.
No commit/push; Visual Gate 2 stays PENDING.

### Implementation verification

The separate application command rejects non-owner principals and missing permissions before
IO, validates required revision/path guards, and retains settings coordination through the
launcher acknowledgement. Broken explicit paths, changed targets and unavailable evidence
do not silently open a download page. Launcher/reply failures never trigger an automatic retry.
The platform worker validates again before spawn, starts without arguments/shell/inherited
stdio and retains bounded Child ownership even after response cancellation. Its seven focused
tests use a harmless system utility or private fixtures, never the installed Hub.

Frontend check/build passed with 844 locale messages. The first browser run was **43 passed /
2 failed** out of 45: two tooltip checks queried Material's inner native button while title
and aria-describedby belong to its standard host. Correcting those test selectors preserved
disabled/keyboard/no-RPC assertions; the two cases plus Download screenshot case passed **3/3**
in the targeted rerun. The other 43 were not repeated. Logs are `gui-open-build`,
`gui-open-browser` and `gui-open-recheck`; original failed output is retained. Full-window
Chinese/Japanese and missing-Hub fixture screenshots are `gui-open-settings-zh-CN.png`,
`gui-open-settings-ja-JP.png` and `gui-open-download.png`. These are browser fixtures, not
native Hub or system-browser launch evidence.

Complete `cargo test --workspace --no-fail-fast -- --test-threads=1` passed: **474 passed,
0 failed, 8 existing subprocess-helper ignored**, exit 0. All 76 result summaries agree;
63 binary runners and thirteen doctest runners are included. The original 457-test checkpoint
below remains historical. The new run includes the 31/31 protocol contract tests and the
seven platform launch tests. Workspace Clippy, final fmt, metadata, diff check and the
approved Open vectors against active Schema **12/12** passed. Evidence is `workspace-full-open`,
`hub-open-clippy-final`, `hub-open-fmt-final` and `open-active-schema-final` logs/exit files.

The existing native IPC test exercises missing capability and stale revision/path guards;
its download-result assertion is conditional on the host not having a Hub. This host has a
Hub, so automatic absence and successful launch outcomes are proved by application fakes
and browser fixtures, not by claiming a native missing-Hub or installed-Hub launch. The
platform tests create only harmless owned fixture/system-utility processes.

Real Hub window activation, the system download-page open, native picker interaction and
screen-reader disabled-state announcements remain unverified. No desktop pass is inferred
from DOM properties, fixture screenshots or a successfully restored Tauri window.

### Open-action native build and current source checkpoint

After the successful full regression, the owned daemon was rebuilt and the original
`npm.cmd run tauri dev --workspace @alcomd/gui` was restored with its existing isolated data
directory `target/m7-icon-acceptance-20260906/data`. Both are **dev/debug, unoptimized +
debuginfo**. Native GUI compilation finished successfully; PID 52016 has a nonzero main
window handle, title ALCOMD3 and Responding=true. This is runtime availability evidence,
not a native visual or Hub-open acceptance result. Final `cargo xtask check` also passed.

| Component | Current artifact / SHA-256 |
| --- | --- |
| GUI | `target/debug/alcomd-gui.exe`, PID 52016 |
| GUI SHA-256 | `F98E4F0D89949CB064B98DC47E27A9D626A51724E2AFA3CB36899776B6955B31` |
| Daemon | `target/debug/alcomd.exe`, PID 58024, original absolute data-dir arguments |
| Daemon SHA-256 | `09B3CB43DC3DC128DBDA4B7063D47E427F1FCA9B6203C523D1DDB9967D709F4B` |

Read-only CLI with `--no-start-daemon` confirmed Core ready, `unity.hub.open.v1` advertised,
and the same two isolated sample projects. All nine captured production source hashes
remained unchanged through build/restoration. The settings file SHA-256 remains
`9C1ECEBD8402E19654EDB5D58579F396CE62799093EE9404F0D4FBF96FF768A8` before and after.
Evidence: `hub-open-source-inputs.json`, `runtime-restoration-open.json`,
`restored-runtime-proof-open.json`, `restored-open-dev.stderr.log`, `daemon-build-open.log`
and `hub-open-core-status.json`. No real Hub/system browser was launched by this verification;
no user Unity/v3 process or user Project was altered. At that build checkpoint HEAD was
`7d938cf2845cd982c25b12f106a8dac33981c8e7`, main, index empty, inherited WIP retained,
not committed or pushed. The same production source is now archived by grouped Core/UI/GUI commits, ending at
`7357d8054e425400ad2befe5dd93132d6c073689`; committing did not change build inputs. Visual Gate 2 remains PENDING.

## Current follow-up: direct path editing

The owner explicitly requested typing directly into the Hub path field. It now keeps a
local draft and submits through the same serial settings writer and Core path validation
on Enter or when focus leaves the entire setting's action group. Escape restores the latest
accepted path. Typing, IME-composition Enter, unchanged values and Tab between the local
action buttons do not write. A missing/unavailable status is placeholder text, not a path;
if no effective path exists, a saved broken path remains editable for correction. An explicitly
emptied dirty draft shows a local required-style error without RPC; Auto detect remains the
explicit way to clear the preference. A field with no configured/detected path is not globally
marked required or invalid merely for being empty.

Core-normalized success replaces the draft; failure retains both the draft and error.
Unchanged failed text is not silently retried on another blur; Enter/Retry is explicit.
Returning to the accepted baseline clears only the edit-origin error without writing.
Fresh authority is reconciled before committing typed edits, so another client's unseen
manual setting cannot be overwritten. Edits can queue behind unrelated ordinary saves,
and incoming accepted settings cannot replace an active draft. Existing Auto/Select/Retry
actions suppress premature draft submission; picker cancellation preserves the draft and
restores focus for continued editing.

The shared TextField gained standard typed focus/keyboard callbacks; its existing no-argument
blur callback is unchanged. The shared Button forwards Material's existing `softDisabled`:
Hub actions waiting for a reply keep focus while library activation and the synchronous
submission guard both prevent duplicate work. This corrects the actual focus-loss race where
hard-disabling the focused Select button was mistaken for leaving the group. No custom
delay, per-page CSS or permanent helper/status text was introduced. Existing settings content
spacing is reused for the focus group.

Verification: final frontend check/build passed (830 locale messages), **33/33** browser cases
(27 Hub + six existing autosave), plus **1/1** existing HEX/HCT field compatibility case.
The first 32-case run was 30 passed / two failed: the focused-button loss above and an invalid
non-composed checkbox fixture event. The fixture now uses its native click event without
moving focus; all original assertions remain, and the final run additionally covers blur
while another ordinary save is held. Intermediate failure logs remain separate; no tests or
acceptance rules were removed. Current logs are `gui-edit-check.log`, `gui-edit-build-final.log`,
`gui-edit-browser-final.log`, `gui-edit-theme-field.log`; all final exits are zero. Complete
Chinese/Japanese fixture images `gui-edit-settings-zh-CN.png` / `gui-edit-settings-ja-JP.png`
were saved and the Chinese image inspected. They remain browser evidence, not desktop signoff.

This correction changes only frontend code/tests and documentation: no backend, public DTO,
active Schema, dependency, CSS or Tauri Rust change. Backend checkpoints below are retained;
no Rust rebuild/full Workspace rerun is required for this frontend-only follow-up. The original
dev command is reopened for owner testing: PID 57388 is responding with a visible window,
and Vite serves the editable-field handlers and Material softDisabled call sites. The existing
daemon PID 45024 remains on the original isolated data directory and the unchanged debug
executable hash `92431AA06C80630EB363C6B188E6E60B8B7F62120B203DA82E5EF5A1DC15366D`.
This frontend-only runtime proof is `gui-edit-runtime-proof.json`; it does not replace the
backend/native artifact checkpoint below. System picker, real desktop typing and screen-reader
acceptance remain pending. No commit/push; Visual Gate 2 remains PENDING.

## Previous correction checkpoint: readable Windows path and Auto detect

The owner clarified that the unwanted path detail was the Windows internal `\\?\` prefix,
and explicitly requested a clickable “自动查找” action. The platform now returns ordinary
DOS/UNC notation only after checking it resolves back to the same canonical file; unknown
device/volume paths or paths needing verbatim semantics remain intact. GUI does not trim
prefixes or interpret filesystem identities. A read-only diagnostic using the actual platform
implementation found the installed Unity Hub 3.22.5 at
`C:\Program Files\Unity Hub\Unity Hub.exe`, source Discovered, with no stored override.

The standard tonal Auto detect／自动查找／自動検出 button remains beside Select. It uses the
existing nullable preference and serial queue: an existing manual override is cleared by a
normal revision-checked `hubPath:null`; an already automatic preference performs a fresh
`settings.get`, updating even same-revision evidence without writes or Editor refresh.
Retries preserve null intent and reconcile unknown outcomes; an unseen concurrent manual
setting is surfaced instead of silently overwritten. This adds no RPC, field, permission,
dependency, native API, unsafe or migration. Registry/Hub-loader boundaries below remain.

Current correction checks: Windows platform **7/7**, actual native Hub RPC **1/1** including
readable-path and same-canonical-file assertions, Workspace Clippy and fmt passed. Frontend
check/build passed (827 messages), autosave **15/15**, functional browser **22/22**, then the
exact Chinese “自动查找” label adjustment passed its separate **2/2** localization cases and
check/build. These are distinct runs, not a fabricated combined 24/24. New Chinese/Japanese
full-window fixture screenshots are `gui-auto-settings-zh-CN.png` / `gui-auto-settings-ja-JP.png`;
Chinese layout and the exact button wording were inspected. The original complete Workspace
457/0/8 result below predates this small correction; its checkpoint is retained, not relabeled
as a complete rerun of this follow-up. Current evidence filenames include
`readable-path-platform.log`, `local-autodetection.log`, `hub-rpc-autodetect.log`,
`clippy-autodetect.log`, `gui-auto-*.log` and `gui-auto-label-*.log` under the same ignored
evidence directory. Matching debug daemon/dev are rebuilt/restored after the RPC run;
`runtime-restoration-autodetect.json` records the unchanged isolated settings hash.

The owner requested the v3 Unity Hub location selection behavior. Work is on `main` in
the sole standard workspace, above `7d938cf2845cd982c25b12f106a8dac33981c8e7`.
All inherited GUI/Core WIP is retained; the index remains empty. This contract does not
authorize commit/push, another worktree, or later milestones.

## Actual reference and capability boundary

The frozen v3 source is read only, not copied or ported:

- `vrc-get-gui/app/_main/settings/index.tsx:154–208`: Hub Path before Editor installations;
  readonly path, Select, no Open location; selection saves immediately; settings and Editor
  paths are reloaded after the picker settles, including cancellation.
- `vrc-get-gui/components/common-setting-parts.tsx:177`: shared path row and Select control.
- `vrc-get-gui/src/commands/environment/settings.rs:160–232`: native picker seeded with the
  current path, Windows exe filter/macOS app filter/Linux unrestricted files; macOS bundle
  maps to its Hub executable; cancel does not save. Windows/Linux do not validate the
  selected executable in this handler.
- `vrc-get-vpm/src/environment/unity_management.rs:174`: configured existing file first,
  then platform defaults; Windows also probes a registry install location. Fallback detection
  changes the v3 preference. This is observed behavior, not implementation source for v4.
- `vrc-get-gui/src/commands/environment/unity_hub.rs:126`: Editor discovery separately uses
  Hub files or the legacy Hub CLI. Hub path selection is not the same feature as that loader.

Before this increment, v4 `ConfigSettings` had no Hub location. `settings.get/update` owns authoritative
preferences and CAS; `ConfigStore` owns recoverable configuration persistence. Existing
`unity.installations.*` manages Editors, and `discover_unity_executables` scans conventional
Editor roots, not Hub's own executable or its configured Editor roots.

## Implemented exact increment

Owner-approved machine-readable contract and schema fragments:
[`unity-hub-location.proposal.contract.json`](../../specs/config/unity-hub-location.proposal.contract.json).

Extend **existing** `settings.get` and `settings.update`. Keep `settings.read` /
`settings.manage`, no new method, capability, permission, State migration, or Config version.
Add `settings.unity.hubPath: string | null` and a top-level **derived** `unityHub` location
result to both responses. A separate `unity.hub.get/set` pair would duplicate settings
authorization, revision and write coordination; putting Hub data in Editor list rows would
mix application location with installation registry pagination. Neither is needed.

Illustrative request (existing envelope and method):

```json
{
    "id": "hub-location-save",
    "method": "settings.update",
    "params": {
        "expectedRevision": 8,
        "update": {"unity": {"hubPath": "C:\\Tools\\Unity Hub\\Unity Hub.exe"}}
    }
}
```

Core result fields, alongside existing appearance/packages/locale:

```json
{
    "configSchema": 3,
    "revision": 9,
    "settings": {
        "unity": {"hubPath": "C:\\Tools\\Unity Hub\\Unity Hub.exe"}
    },
    "unityHub": {
        "status": "available",
        "source": "configured",
        "path": "C:\\Tools\\Unity Hub\\Unity Hub.exe",
        "configuredPathStatus": "available"
    }
}
```

The response example omits unchanged settings fields for readability; the actual response
remains complete. `hubPath` omission in a partial update means no change; `hubPath:null`
clears the explicit preference and enables default discovery. `unity:null`, empty container,
unknown keys, empty/relative/control-containing/non-UTF-8/oversized paths reject the whole
update. Only explicit path updates require executable validation; unrelated changes still
save if a previously selected Hub was removed.

`unityHub.status` is `available | not_found | unavailable`; `source` is
`configured | discovered | none`; `path` is the usable normalized executable or null.
`configuredPathStatus` is `unset | available | invalid | unavailable`, preserving the
distinction between a missing manual choice and a broken saved path even when a fallback
is found. Available means a usable filesystem executable at inspection time, not a vendor,
signature, future execution or Editor discovery guarantee.

Use the current unreleased Config3 optional `[unity] hub_path` string. Existing files with
no section mean null; do not rewrite on detection, do not create Config4 or a second model.
Preserve revision and all other fields. Extend the store's parser/serializer to round-trip
escaped backslashes, quotes and Unicode rather than interpolating a raw Windows path into
TOML. Keep the 16 KiB file limit; path limit is 4096 UTF-8 bytes. This is a pre-release public
response extension that strict clients must update for, not a claim of strict old-client
compatibility.

## User behavior and deliberate boundaries

Settings initializes the editable draft from the accepted effective path (or a saved broken
path when no effective path exists), before Unity installations, using the existing
section/Material primitives and frozen v3 English/Chinese/Japanese section labels.
The current Tauri dialog plugin only returns the selection. A narrow application inspection
port, implemented in platform and injected by daemon, owns normalization and bounded
filesystem inspection. Application keeps permissions, checked CAS and cancellation/cache
discipline; ConfigStore remains the only configuration publisher. Derived evidence is not
persisted in ConfigSnapshot and is not added to package evidence hashing.

After selection, use the existing serial autosave queue and display accepted evidence only;
cancel does nothing. No transient success text, global save bar, localStorage authority or
new private business command. Real errors remain visible and retry reconciles current
authority. A save cannot be reported as failed just because a subsequent evidence read or
Editor refresh failed. One picker/save at a time; old connection results are discarded.

After a confirmed save, refresh Editors once through existing `unity.installations.refresh`
only when `unity.manage` is granted, then reread its existing list. `settings.manage` is
sufficient to save the location; it does not silently grant Unity management permission.
The accepted authority and picker seed remain unchanged until save is confirmed; the input
may show an explicitly unconfirmed local draft, never a fabricated Core result.

Differences that require explicit review rather than a false “full v3 parity” claim:

- v4 validates an actual usable file before saving, but never launches an arbitrary selected
  executable to identify it. A filename alone cannot prove Unity Hub identity.
- Default detection is bounded and read only; it never silently saves a replacement. The
  proposal lists at most eight candidates per platform, using daemon environment roots.
- v3's Windows registry probe is outside this narrow increment; nonstandard locations are
  manually selectable. No dependency, unsafe, native API or shell registry query is added.
- Cancel does not trigger Editor discovery. A confirmed save can refresh only v4's existing
  conventional Editor scan. Hub config/CLI loading, legacy-loader options, custom Editor
  roots and Hub install links remain separate parity gaps, not implemented by this selector.
- Authorized settings results intentionally contain their requested paths. Document that
  narrow exception to the generic response-redaction list; ordinary errors/Activity/Event
  do not gain raw paths. Candidate cursors continue to include config revision exactly as
  before, so saving a Hub path can invalidate old cursors; discovery alone cannot.

## Verification and acceptance

Local evidence is in ignored `target/m7-unity-hub-location-20261031/`:

| Check | Result and limit |
| --- | --- |
| Application settings tests | 10/10; permission/CAS before path inspection, mixed invalid update atomicity, normalize/clear, no unrelated revalidation, exact snapshot evidence, post-commit inspection panic cannot fail the save |
| Windows platform Hub tests | 5/5; file usability, configured/default priority, bounded search, invalid path, exe extension. Unix permissions/symlink and macOS bundle cases are compiled conditionally and not executed on this Windows host |
| Protocol contract tests | 30/30; null/empty/unknown unity input, clear, required result fields, existing contract snapshots |
| Active Schema shape vectors | 15/15 against active definitions; shape proof only, not execution proof |
| Frontend check/build | Passed, 824 locale messages; existing Vite large-chunk warning remains |
| Autosave coordination | 10 scenarios passed, including serial Hub/ordinary/theme writes, unknown commit reconciliation and same-revision evidence |
| Browser regression | 17/17: eleven Hub cases plus six existing autosave cases. Real-user browser run after sandbox browser setup failed; no product assertions weakened |
| Workspace Clippy | Passed `--workspace --all-targets -- -D warnings`; first run found a new test-only clone lint, corrected without suppression; final log separate. Existing compiler incremental-cache hardlink-copy warnings remain |
| Workspace library tests | `cargo test --workspace --lib -- --test-threads=1`: 230 passed, zero failed/ignored across thirteen runners, including sixteen config-storage cases; this excludes integration-test binaries |
| Full Workspace / real RPC, initial implementation checkpoint | Final `cargo test --workspace --no-fail-fast -- --test-threads=1`: **457 passed, 0 failed, 8 ignored**, exit 0; 76 runners and 76 result summaries agree. The eight existing ignored entries are subprocess helpers invoked by kill/restart tests, not newly skipped coverage. Follow-up correction checks are listed above |
| fmt / xtask / metadata / diff check | Passed |

The storage adapter includes durable path/clear/reopen/recovery, escaped Unicode/quotes/
backslashes, old Config3 defaults, frozen old-schema rejection and a serialized 16 KiB
bound that cannot replace the accepted file. The contract-stage baseline freeze check passed in the real-user
environment; frozen v3 remains `4aa98ae4f18d42c10137278997180dbede991e88`, tag `v3.4.0`.

The existing `m7_official_rpc` integration test now includes six additional malformed mixed
updates (sixteen total), persisted selection, read-only observation after fixture removal,
unrelated-setting update with missing Hub, and clear. It passed in the final full Workspace
run over actual native IPC. The owner explicitly authorized temporarily stopping and
restoring the identified Workspace dev tree and isolated daemon. Process identity and
descendant ownership were checked before interruption; the installed v3 window was untouched.

The first full run had 456 passed / 1 failed / 8 ignored: the new Hub steps advanced the
fixture to revision 14 and Japanese, while its final exact file assertion still expected
the pre-Hub revision 11 and Chinese. Corrected that exact oracle and added sequential
revision assertions for the ordinary update and Hub clear; no production code or assertion
strength was reduced. A separate complete rerun produced the final result above; original
`workspace-full.log/.exit` and final `workspace-full-final.log/.exit` remain distinct. The
changed RPC test also passed `cargo clippy -p alcomd-testing --test m7_official_rpc -- -D warnings`
and final fmt. No new UI build or browser repeat was needed for that test-only correction.

## Native runtime at the automatic-lookup checkpoint

The original dev command `npm.cmd run tauri dev --workspace @alcomd/gui` and daemon arguments
were restored after testing. Both executables are **dev/debug, unoptimized + debuginfo**, not
release. Build logs, unchanged production source input hashes and live process paths are
recorded for the current correction in `autodetect-source-inputs.json`,
`runtime-restoration-autodetect.json` and `restored-runtime-proof-autodetect.json`.
The dev instance from the earlier checkpoint had already exited before the latest test;
the identity guard refused that obsolete process tree without stopping anything. After
confirming no other Workspace GUI/dev existed, only the known isolated daemon was paused,
and the original dev command was started again after the successful targeted RPC check.

| Component | Restored process / artifact |
| --- | --- |
| GUI | PID 46880, `target/debug/alcomd-gui.exe`; visible window handle, title ALCOMD3, responding |
| GUI SHA-256 | `E600B2414DAEB6435F1694863BF0D054FDDD2890B7C8286A1E1AF94B4EE48FE5` |
| Daemon | PID 45024, `target/debug/alcomd.exe --data-dir target/m7-icon-acceptance-20260906/data` (actual launch uses absolute paths) |
| Daemon SHA-256 | `92431AA06C80630EB363C6B188E6E60B8B7F62120B203DA82E5EF5A1DC15366D` |

Read-only CLI with `--no-start-daemon` confirmed Core state `ready` and the same two isolated
sample projects. The settings file SHA-256 before interruption and after restoration remains
`9C1ECEBD8402E19654EDB5D58579F396CE62799093EE9404F0D4FBF96FF768A8`;
automatic Hub discovery did not save a path or modify preferences. No user project package
Apply, migration, copy or delete was performed. HEAD remains
`7d938cf2845cd982c25b12f106a8dac33981c8e7`, branch main, index empty, inherited WIP retained.

Complete-window Chinese/Japanese fixture screenshots `gui-settings-zh-CN.png` and
`gui-settings-ja-JP.png` show sidebar, page title, Hub path row and Editor table. The Chinese
image was inspected; long paths remain within the readonly field. They are browser evidence,
not native Tauri screenshots. New native picker and required response fields require matching
rebuilt Tauri/daemon processes; Vite hot reload alone is insufficient.

Real Tauri file-picker cancellation, selection, saved reopening and native restart persistence
remain untested in this run. Browser fixtures and the restored window cannot stand in for those desktop
checks or screen-reader evidence. Visual Gate 2 remains **PENDING**.

Root `AGENTS.md` requires manual approval of public contracts. The owner explicitly approved
this exact reviewed increment with “是” on 2026-10-31. The implementation follows its stated
registry/Hub loader boundaries; it does not expand into those omitted capabilities. No new
method/capability/permission/dependency/Config version/State migration, commit or push.
