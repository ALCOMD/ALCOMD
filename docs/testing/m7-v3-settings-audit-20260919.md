# Settings: frozen v3 comparison and implementation boundary

Date: 2026-09-19. This is a source audit and frontend rework record, not a Visual Gate pass.
The owner rejected the previous utility-page redesign and requested another complete comparison against v3.

## Reference and method

The frozen reference is `../ALCOMD3-v3-readonly/vrc-get-gui/`.
`app/_main/settings/index.tsx` was read in full, including all cards and the launch-argument dialog.
`components/common-setting-parts.tsx` was also read in full. These files remain read-only.
The implementation is independently written React using existing v4 Material components and typed RPC.
No v3 implementation, parser, dependency, command binding, or persistence format was copied.

## Group-by-group comparison

The order below is the actual render order in `app/_main/settings/index.tsx:98`, not an inferred preference layout.

| v3 group and source | Actual v3 presentation | Existing v4 authority | Current treatment / remaining gap |
| --- | --- | --- | --- |
| Page and card, `index.tsx:78`, `:142` | Single Settings heading, scrollable vertical cards with compact padding | Presentation only | A single Settings workspace; remove Preferences / Unity top-level segmentation; scoped stacked section styles |
| Unity Hub path, `index.tsx:154` | Heading, readonly path, Select button | No Hub path field or setter in `OfficialSettings` | Not fabricated; group remains unavailable until an approved authority exists |
| Unity installations, `index.tsx:210` | Version / Path / Source table with reload and Add Unity at top; legacy Hub-loading checkbox below | `unityInstallationsList`, `unityInstallationRegister`, `unityInstallationRemove`, `unityInstallationsRefresh` | Real table inside Settings; explicit pagination; existing management actions open in a dialog. No legacy Hub-loading checkbox. Reload discovery remains explicit in management; toolbar Refresh reads the snapshot only |
| Default Unity arguments, `index.tsx:375` | Current argument list with Edit opening a large dialog | Per-project launch configuration exists; global default settings field does not | Global group not synthesized from a random project's configuration |
| Default project path, `index.tsx:496` | Explanation, readonly path, Select / Open location | No default-path settings field or picker setter | Not rendered as an editable fake preference |
| Backup, `index.tsx:549` | Path row, format selector, descriptions | Contextual Backup Plan/Apply exists; global backup path/format settings do not | No global backup preference created; contextual backup workflow remains elsewhere |
| Packages, `index.tsx:615` | Clear cache button, prerelease checkbox and explanation | `settings.packages.showPrerelease`; hidden Repository IDs and User Package visibility are v4 additions | Packages precedes Appearance; preserve all existing visibility values. Cache clearing is not invented. Repository option read failures are explicit; pagination never clears unseen hidden IDs |
| Appearance, `index.tsx:698` | Language row, GUI animation checkbox, compact GUI checkbox | `locale`, `appearance.motion`, `appearance.density` | Same control sequence. Animation checked maps to `system`, unchecked to `reduced`; system reduced-motion remains authoritative rather than forcing animation |
| Files and folders, `index.tsx:709` | Buttons opening known v3 files/directories | GUI may not directly read/write daemon files; no equivalent approved settings-file action | Do not reuse v3 file paths or add filesystem access |
| Legacy import, `index.tsx:770` | Existing legacy import panel | Not part of current normal settings RPC; migration remains separately scoped | No placeholder import action |
| Application, `index.tsx:782` | Update / issue actions, auto-update, beta channel, deep-link registration, license link | No updater/auto-update/deep-link settings contract in current `OfficialSettings` | No fake controls or new remote update flow |
| Contributors, `index.tsx:998` | Conditional contributor avatars from reference-specific source | No corresponding contributor data in settings snapshot | Do not copy v3 identities or invent contributor data |
| System information, `index.tsx:1043` | OS, architecture, WebView, version, commit | Existing About and Diagnostics pages | Reuse explicit About / Diagnostics entries; not an assertion that the v3 detail rows are all available |

The v3 language selector is at `components/common-setting-parts.tsx:35`, animation at `:85`, compact mode at `:133`.
v4 theme mode and source color are retained in a separate Theme section after Appearance. Existing custom canonical source colors stay selectable rather than being replaced by one of the preset colors.

## Persistence and interaction differences retained deliberately

v3 writes individual settings immediately. This rework keeps the already-authorized v4 revision-checked
Save settings / Discard changes flow and the existing route-dirty guard. It does not silently switch to optimistic
autosave or claim exact interaction parity. Controls are disabled during saving; a synchronous submission guard
prevents rapid repeat submissions. Errors retain the draft, including revision conflict. No authority is moved to localStorage.

Unity management uses the existing actions and capability gates. No installation discovery runs merely because
Settings opened. The normal page read and explicit read refresh use `unityInstallationsList`; discovery is a separate
existing action. No implicit project mutation, Plan, or Apply is added.

The lack of global Hub/default-path/backup/updater authority is a **real remaining parity gap**, not a completed
setting hidden for appearance. These omissions must be included in owner review and cannot be reported as a full v3 Settings implementation.

## Validation ownership

`SettingsWorkspace.tsx` is delivered as an isolated component for main-thread integration and scoped styling.
Frontend type/build checks and browser screenshot/interaction review are run by the integration owner after that
integration. This source audit alone proves neither visual fidelity nor accessibility. Real desktop and screen-reader
evidence remain open; Visual Gate 2 remains PENDING.
