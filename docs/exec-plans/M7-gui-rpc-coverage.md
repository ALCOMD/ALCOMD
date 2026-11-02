# M7 Official GUI RPC coverage matrix

状态：2026-11-02 当前已批准 RPC 覆盖矩阵，包含仓库列表文件交换、H3 与 H4 权限只读接线。来源是 `crates/alcomd-protocol` 的 public method
constants 与 `crates/alcomd-client` 的 typed methods；实现过程中必须保持两者与本矩阵可机械核对。
本矩阵记录方法归属，不表示 M7 或聚合功能已验收通过；方法清单一致性不代表所有方法已有独立 GUI 动作。当前公开常量与矩阵 93/93 对齐；此前 92/92 绑定新增权限读取前的清单。当前阶段见 `../status.md`；H3 定向证据见 `../testing/m7-h3-contextual-20261101.md`，H4 见 `../testing/m7-h4-extensions-20261102.md`。

统一状态 `data` 表示 initial-loading、refreshing/last-known-good、empty、error、disconnected；`mutation` 另含
confirmation-required、operation-running、success、failed、cancelled。`Plan review` 必须展示 daemon 返回的 typed
ChangeSet/risk summary，Apply 不重新规划。`N/A` 表示 transport/internal source，不直接形成 GUI action。

| RPC method | capability | page / route | user action | R/W | confirmation | Operation | state handling | test owner |
|---|---|---|---|---|---|---|---|---|
| `system.hello` | RPC v1 handshake | app shell | connect/reconnect; ready Store dataSchema, Config3 | R | no | no | disconnected/reconnecting/capability refresh | `gui.m7-core-surfaces` |
| `system.status` | base | Home, About, shell | read Core version/status; local GUI build version remains independent | R | no | no | data/error/reconnect | `gui.m7-core-surfaces` |
| `state.check` | `state.check.v1` + `operations.v1` for GUI progress | Diagnostics | start integrity check and follow the accepted ID | W | explicit action | yes | mutation/error/terminal refresh | `gui.m7-operation-flows` |
| `operations.get` | `operations.v1` | Project backup progress / Restore / Operation detail / Log detail | follow the same accepted ID and open Core results | R | no | existing Operation | polling/read retry/terminal/client cleanup | `gui.m7-operation-flows` |
| `operations.list` | `operations.v1` | Operations | page Operations | R | no | no | data/keyset | `gui.m7-core-surfaces` |
| `operations.cancel` | `operations.v1` | Operation detail | cooperative cancel | W | yes | existing Operation | mutation/stale | `gui.m7-operation-flows` |
| `events.list` | `events.replay.v1` | N/A direct | daemon-side Activity source/cursor recovery only | R | no | no | N/A | `activity.log-redaction` |
| `projects.inspect` | `projects.read.v1` | Projects add dialog | validate candidate path | R | no | no | dialog data | `gui.m7-core-surfaces` |
| `projects.list` | `projects.read.v1` | Projects | page projects | R | no | no | data/keyset | `gui.m7-core-surfaces` |
| `projects.get` | `projects.read.v1` | Project detail | load project | R | no | no | data | `gui.m7-core-surfaces` |
| `projects.register` | `projects.registry.v1` | Projects add dialog | register inspected project | W | yes | no | mutation/idempotency | `gui.m7-operation-flows` |
| `projects.refresh` | `projects.registry.v1` | Project detail | refresh registered model | W | no | no | refreshing/stale | `gui.m7-operation-flows` |
| `projects.setFavorite` | `projects.registry.v1` | Projects | toggle favorite | W | no | no | mutation/revision/idempotency | `gui.m7-project-action-contract` |
| `projects.unregister` | `projects.registry.v1` | Project detail | remove registry entry | W | yes | no | mutation/stale | `gui.m7-operation-flows` |
| `projects.planCopy` | `projects.copy.v1` | Project Copy dialog | plan copy to new directory | R/Plan | review follows | no | data/writer/stale | `gui.m7-project-action-contract` |
| `projects.applyCopy` | `projects.copy.v1` | Project Copy dialog | apply reviewed Copy Plan | W | required | yes | mutation/recovery | `gui.m7-project-action-contract` |
| `projects.planDeleteDirectory` | `projects.delete.v1` | Project remove dialog | plan directory deletion | R/Plan | review follows | no | data/writer/stale | `gui.m7-project-action-contract` |
| `projects.applyDeleteDirectory` | `projects.delete.v1` | Project remove dialog | apply reviewed Delete Plan | W | required | yes | mutation/recovery | `gui.m7-project-action-contract` |
| `projects.planUnityMigration` | `projects.unity-migration.v1` | Project Unity migration | plan supported Unity migration | R/Plan | review follows | no | data/writer/stale | `gui.m7-project-action-contract` |
| `projects.applyUnityMigration` | `projects.unity-migration.v1` | Project Unity migration | apply reviewed migration Plan | W | required | yes | mutation/recovery | `gui.m7-project-action-contract` |
| `repositories.inspect` | `repositories.read.v1` | Repositories add dialog | validate source | R | no | no | dialog data | `gui.m7-core-surfaces` |
| `repositories.inspectListFile` | `repositories.list-files.v1` | Resources / Import list | select and parse a bounded list file; local-owner only | R/file | explicit native file selection | no | loading/classification/duplicate/invalid/unsupported/read error | `gui.m7-core-surfaces` |
| `repositories.previewListExport` | `repositories.list-files.v1` | Resources / Export list | preview full Core snapshot, exclusions and fingerprint | R | no | no | loading/scope/exclusions/credential-sharing warning | `gui.m7-core-surfaces` |
| `repositories.exportListFile` | `repositories.list-files.v1` | Resources / Export list | create a new list file from confirmed preview; local-owner only | W/file | reviewed scope/address risk and explicit new name/folder | no | changed snapshot/existing target/write failure/saved result | `gui.m7-operation-flows` |
| `repositories.list` | `repositories.read.v1` | Repositories | page repositories | R | no | no | data/keyset | `gui.m7-core-surfaces` |
| `repositories.get` | `repositories.read.v1` | Repository detail | load repository | R | no | no | data | `gui.m7-core-surfaces` |
| `repositories.packages` | `repositories.read.v1` | Repository detail, Project Packages | browse catalog | R | no | no | data/keyset | `gui.m7-core-surfaces` |
| `repositories.register` | `repositories.registry.v1` | Repositories add dialog | register inspected source | W | yes | no | mutation/idempotency | `gui.m7-operation-flows` |
| `repositories.refresh` | `repositories.registry.v1` | Repository detail | refresh repository | W | no | no | refreshing/stale | `gui.m7-operation-flows` |
| `repositories.unregister` | `repositories.registry.v1` | Repository detail | remove repository | W | yes | no | mutation/stale | `gui.m7-operation-flows` |
| `packages.planInstall` | `packages.plan.v1` | Project Packages | plan install | R/Plan | review follows | no | data/stale | `gui.m7-operation-flows` |
| `packages.planRemove` | `packages.plan.v1` | Project Packages | plan removal | R/Plan | review follows | no | data/stale | `gui.m7-operation-flows` |
| `packages.planUpgrade` | `packages.plan.v1` | Project Packages | plan upgrade | R/Plan | review follows | no | data/stale | `gui.m7-operation-flows` |
| `packages.planDowngrade` | `packages.plan.v1` | Project Packages | plan downgrade | R/Plan | review follows | no | data/stale | `gui.m7-operation-flows` |
| `packages.planResolve` | `packages.plan.v1` | Project Packages | plan deterministic resolve | R/Plan | review follows | no | data/stale | `gui.m7-operation-flows` |
| `packages.planReinstall` | `packages.plan.v2` | Project Packages | plan reinstall locked packages | R/Plan | review follows | no | data/stale/source ambiguity | `gui.m7-package-workspace` |
| `packages.planBulk` | `packages.plan.v2` | Project Packages | one Plan for full selected set or Update All | R/Plan | review follows | no | data/complete evidence/256 limit/stale | `gui.m7-package-workspace` |
| `packages.queryProjectCandidates` | `packages.candidates.v1` | Project Packages | read Core summaries and lazy version pages | R | no | no | data/snapshot/paging/quota/stale | `gui.m7-package-workspace` |
| `packages.applyPlan` | `packages.apply.v1` | Project Packages | apply reviewed Plan | W | required | yes | mutation/stale/recovery | `gui.m7-operation-flows` |
| `packages.userPackages.list` | `packages.user-packages.v1` | Resources / User Packages | page enrolled packages | R | no | no | data/keyset | `gui.m7-package-workspace` |
| `packages.userPackages.get` | `packages.user-packages.v1` | User Package detail | load enrolled package | R | no | no | data | `gui.m7-package-workspace` |
| `packages.userPackages.enroll` | `packages.user-packages.v1` | Resources / User Packages | enroll explicit folder | W | explicit action | no | mutation/idempotency | `gui.m7-package-workspace` |
| `packages.userPackages.refresh` | `packages.user-packages.v1` | User Package detail | refresh enrolled source | W | no | no | refreshing/revision/idempotency | `gui.m7-package-workspace` |
| `packages.userPackages.remove` | `packages.user-packages.v1` | User Package detail | remove enrollment | W | yes | no | mutation/revision/idempotency | `gui.m7-package-workspace` |
| `unity.installations.list` | `unity.read.v1` | Unity | page installations | R | no | no | data/keyset | `gui.m7-core-surfaces` |
| `unity.installations.get` | `unity.read.v1` | typed adapter only; Settings currently uses list snapshots | no separate GUI detail action | R | no | no | adapter available, standalone UI not implemented | `gui.m7-core-surfaces` |
| `unity.installations.register` | `unity.manage.v1` | Unity add dialog | register validated editor | W | yes | no | mutation/idempotency | `gui.m7-operation-flows` |
| `unity.installations.remove` | `unity.manage.v1` | Unity installation detail | remove registry entry | W | yes | no | mutation/stale | `gui.m7-operation-flows` |
| `unity.installations.refresh` | `unity.manage.v1` | Unity | refresh discoveries | W | no | no | refreshing/idempotency | `gui.m7-operation-flows` |
| `unity.projectLaunchConfig.get` | `unity.read.v1` | Project Unity | load launch arguments | R | no | no | data/revision | `gui.m7-core-surfaces` |
| `unity.projectLaunchConfig.set` | `unity.manage.v1` | Project Unity | save launch arguments | W | explicit save | no | mutation/revision/idempotency | `gui.m7-project-action-contract` |
| `unity.projectLaunchConfig.clear` | `unity.manage.v1` | Project Unity | restore default launch arguments | W | yes | no | mutation/revision/idempotency | `gui.m7-project-action-contract` |
| `unity.writerState` | `unity.read.v1` | Project Unity | observe writer evidence | R | no | no | refreshing/unknown | `gui.m7-core-surfaces` |
| `unity.launchOptions` | `unity.launch.v1` | Projects / Open Unity | resolve exact-version 0/1/many installations | R | no | no | data/unknown | `gui.m7-project-action-contract` |
| `unity.launch` | `unity.launch.v1` | Projects / Open Unity | launch explicit one-shot installation | W | explicit action/selection | launch record | mutation/writer gate | `gui.m7-operation-flows` |
| `unity.launchStatus` | `unity.launch.v1` | Projects / Project workspace more actions after a launch accepted on this page | inspect/refresh the same launch record, without another launch | R | explicit inspect/refresh | launch record | known-ID retry/identity validation/late response cleanup/focus return; no automatic polling or cross-page launch history | `gui.m7-operation-flows` |
| `unity.hub.open` | `unity.hub.open.v1` | Settings / Unity | open revalidated Hub or return download_required | W | explicit action | no | revision/path/local-owner/unknown result | `gui.m7-core-surfaces` |
| `templates.list` | `templates.read.v1` | Templates | page templates | R | no | no | data/keyset | `gui.m7-core-surfaces` |
| `templates.get` | `templates.read.v1` | Template detail | load template | R | no | no | data | `gui.m7-core-surfaces` |
| `templates.inspectBundle` | `templates.read.v1` | Template import dialog | validate bundle | R | no | no | dialog data | `gui.m7-core-surfaces` |
| `templates.planImport` | `templates.manage.v1` | Template import dialog | plan import | R/Plan | review follows | no | data/stale | `gui.m7-operation-flows` |
| `templates.applyImport` | `templates.manage.v1` | Template import dialog | apply reviewed import | W | required | yes | mutation/recovery | `gui.m7-operation-flows` |
| `templates.planDerive` | `templates.manage.v1` | Template derive dialog | plan derive | R/Plan | review follows | no | data/stale | `gui.m7-operation-flows` |
| `templates.applyDerive` | `templates.manage.v1` | Template derive dialog | apply reviewed derive | W | required | yes | mutation/recovery | `gui.m7-operation-flows` |
| `templates.export` | `templates.manage.v1` | Template detail | export native bundle | W | yes | no | mutation | `gui.m7-operation-flows` |
| `templates.setFavorite` | `templates.manage.v1` | Templates, Template detail | set favorite | W | no | no | optimistic-disabled/stale | `gui.m7-operation-flows` |
| `templates.remove` | `templates.manage.v1` | Template detail | remove template | W | yes | no | mutation/stale | `gui.m7-operation-flows` |
| `templates.planCreateProject` | `templates.create-project.v1` | Template detail | plan project creation | R/Plan | review follows | no | data/stale | `gui.m7-operation-flows` |
| `templates.applyCreateProject` | `templates.create-project.v1` | Template detail | apply reviewed creation | W | required | yes | mutation/recovery | `gui.m7-operation-flows` |
| `backups.list` | `backups.read.v1` | Project Backups | page backups | R | no | no | data/keyset | `gui.m7-core-surfaces` |
| `backups.get` | `backups.read.v1` | Backup detail | load backup | R | no | no | data | `gui.m7-core-surfaces` |
| `backups.create` | `backups.create.v1` | Project Backups | create backup | W | yes | yes | mutation/recovery | `gui.m7-operation-flows` |
| `backups.planRestore` | `backups.restore.v1` | Projects / Backup detail shared restore dialog | select managed backup or use current backup, review target | R/Plan | review follows | no | loading/list retry/keyset/invalid target/stale | `gui.m7-operation-flows` |
| `backups.applyRestore` | `backups.restore.v1` | Projects / Backup detail shared restore dialog | apply once and open Core-registered result | W | required | yes | same-ID follow/unknown result/no automatic retry/failed/cancelled | `gui.m7-operation-flows` |
| `extensions.list` | `extensions.lifecycle.v1` | Extensions | page extensions | R | no | no | data/keyset | `gui.m7-core-surfaces` |
| `extensions.get` | `extensions.lifecycle.v1` | Extension detail | load trust/runtime/quarantine/UI | R | no | no | data | `gui.m7-core-surfaces` |
| `extensions.permissions.list` | `extensions.permissions.read.v1` | Extension detail / permissions dialog | read declarations and page actual granted scopes | R | no | no | loading/empty/read retry/revision-bound keyset/package or grant change/stale read cleanup | `gui.m7-h4-management` |
| `extensions.planInstall` | `extensions.lifecycle.v1` | Extensions install dialog | plan new install with expected absence; review frozen requested permissions | R/Plan | review follows | no | data/stale/explicit publisher approval | `gui.m7-operation-flows` |
| `extensions.applyInstall` | `extensions.lifecycle.v1` | Extensions install dialog | apply reviewed install | W | required | yes | mutation/recovery | `gui.m7-operation-flows` |
| `extensions.enable` | `extensions.lifecycle.v1` | Extension detail | enable | W | yes | no | mutation/stale | `gui.m7-operation-flows` |
| `extensions.disable` | `extensions.lifecycle.v1` | Extension detail | disable | W | yes | no | mutation/stale/session close | `gui.m7-operation-flows` |
| `extensions.planUninstall` | `extensions.lifecycle.v1` | Extension detail | plan uninstall | R/Plan | review follows | no | data/stale | `gui.m7-operation-flows` |
| `extensions.applyUninstall` | `extensions.lifecycle.v1` | Extension detail | apply reviewed uninstall | W | required | yes | mutation/recovery | `gui.m7-operation-flows` |
| `extensions.setGrant` | `extensions.permissions.v1` | Extension Permissions | grant exact scope | W | yes | no | mutation/revision | `gui.m7-operation-flows` |
| `extensions.revokeGrant` | `extensions.permissions.v1` | Extension Permissions | revoke exact scope | W | yes | no | mutation/revision/session stale | `gui.m7-operation-flows` |
| `extensions.ui.open` | `extensions.ui.portable.v1` | Extension Portable UI | open session | W/session | no | no | loading/error/disconnected | `gui.m7-portable-ui-production` |
| `extensions.ui.refresh` | `extensions.ui.portable.v1` | Extension Portable UI | explicit refresh | W/session | discard if dirty | no | refreshing/stale | `gui.m7-portable-ui-renderer` |
| `extensions.ui.dispatch` | `extensions.ui.portable.v1` | Extension Portable UI | activate/submit action | W/session | host-owned where required | no | progress/error/replay | `gui.m7-portable-ui-security` |
| `extensions.ui.close` | `extensions.ui.portable.v1` | Extension Portable UI | close route/session | W/session | discard if dirty | no | best-effort | `gui.m7-portable-ui-production` |
| `settings.get` | base RPC v1; no new capability approved | Settings, shell | load Config3 and Hub location evidence | R | no | no | data/revision | `settings.authoritative-storage` |
| `settings.update` | base RPC v1; no new capability approved | Settings | serial CAS narrow PATCH; theme Save/Cancel | W | immediate ordinary changes; explicit theme save | no | mutation/stale/intent retained/unknown commit | `settings.authoritative-storage` |
| `activity.list` | base RPC v1; no new capability approved | Activity | page safe Event/Operation projection | R | no | no | data/keyset | `activity.log-redaction` |
| `diagnostics.list` | base RPC v1; no new capability approved | Diagnostics | page redacted diagnostics | R | no | no | data/keyset | `diagnostics.redacted-list` |

## Coverage rules

- New methods, public capabilities and breaking DTO changes require owner approval. The methods above include the later approved Project/Package, exact Unity, candidate evidence and Hub contracts; the original four M7 utility additions are not the full current set.
- `packages.plan.v2` also supports the original package Plan methods; `planReinstall` and `planBulk` require v2. GUI candidate/source filters use complete Core evidence; optional display metadata does not determine business state.
- A method is complete only when typed Rust and TypeScript adapters, real page/action behavior, common states and its test owner all exist.
- `events.list` remains a daemon projection source; Activity UI must call `activity.list`, not join raw Events and Operations in JavaScript.
- Direct registry/lifecycle writes that do not have a Core Plan method receive explicit confirmation but must not invent a GUI Plan.
- Any Apply returning an Operation follows the same Operation detail model and survives route/window close and reconnect.
- List import reuses `repositories.inspect/register` sequentially after selection and confirmation; it appends without clearing existing sources, has no atomic-transaction claim, and keeps cancellation, partial and unknown results distinct. Export preserves validated query text and requires a new file. Tests: `m7-repository-list.spec.ts`, repository-list Core/RPC cases.

2026-11-01 完成度复核：枚举 protocol 各源码模块的公开 method 常量（排除 error_code 下的
`METHOD_NOT_FOUND` 错误码）共 92 个，与矩阵逐项匹配；Rust typed client 和 Tauri/GUI 列表接线
保持已验证输入。这里只补方法归属记录，不把矩阵完整性提升为 M7 或新代码远端验收通过。

H4 经批准增加上述权限只读方法，声明与有效授权分开显示；Grant/Revoke 继续走原写接口，以当前 grantRevision 提交。
扩展列表现在接通后续 cursor；启停/恢复复用原生命周期，安装/卸载只跟踪已接受的同一 Operation。
Portable UI 继续使用原四个 session RPC，不因 GUI 生命周期修正新增协议或方法。H2-B/H3/H4 新代码的远端验证分别保持未取得状态。
