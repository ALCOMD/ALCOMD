# M7 Official GUI RPC coverage matrix

状态：2026-08-26 planning baseline。来源是 `crates/alcomd-protocol` 的 public method constants 与
`crates/alcomd-client` 的 typed methods；实现过程中必须保持两者与本矩阵可机械核对。

统一状态 `data` 表示 initial-loading、refreshing/last-known-good、empty、error、disconnected；`mutation` 另含
confirmation-required、operation-running、success、failed、cancelled。`Plan review` 必须展示 daemon 返回的 typed
ChangeSet/risk summary，Apply 不重新规划。`N/A` 表示 transport/internal source，不直接形成 GUI action。

| RPC method | capability | page / route | user action | R/W | confirmation | Operation | state handling | test owner |
|---|---|---|---|---|---|---|---|---|
| `system.hello` | RPC v1 handshake | app shell | connect/reconnect（client-owned） | R | no | no | disconnected/reconnecting | `gui.m7-core-surfaces` |
| `system.status` | base | Home, About | load daemon/product status | R | no | no | data | `gui.m7-core-surfaces` |
| `state.check` | `state.check.v1` | Diagnostics | start integrity check | W | explicit action | yes | mutation | `gui.m7-operation-flows` |
| `operations.get` | `operations.v1` | Operation detail | load/follow one Operation | R | no | no | data/reconnect | `gui.m7-operation-flows` |
| `operations.list` | `operations.v1` | Operations | page Operations | R | no | no | data/keyset | `gui.m7-core-surfaces` |
| `operations.cancel` | `operations.v1` | Operation detail | cooperative cancel | W | yes | existing Operation | mutation/stale | `gui.m7-operation-flows` |
| `events.list` | `events.replay.v1` | N/A direct | daemon-side Activity source/cursor recovery only | R | no | no | N/A | `activity.log-redaction` |
| `projects.inspect` | `projects.read.v1` | Projects add dialog | validate candidate path | R | no | no | dialog data | `gui.m7-core-surfaces` |
| `projects.list` | `projects.read.v1` | Projects | page projects | R | no | no | data/keyset | `gui.m7-core-surfaces` |
| `projects.get` | `projects.read.v1` | Project detail | load project | R | no | no | data | `gui.m7-core-surfaces` |
| `projects.register` | `projects.registry.v1` | Projects add dialog | register inspected project | W | yes | no | mutation/idempotency | `gui.m7-operation-flows` |
| `projects.refresh` | `projects.registry.v1` | Project detail | refresh registered model | W | no | no | refreshing/stale | `gui.m7-operation-flows` |
| `projects.unregister` | `projects.registry.v1` | Project detail | remove registry entry | W | yes | no | mutation/stale | `gui.m7-operation-flows` |
| `repositories.inspect` | `repositories.read.v1` | Repositories add dialog | validate source | R | no | no | dialog data | `gui.m7-core-surfaces` |
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
| `packages.applyPlan` | `packages.apply.v1` | Project Packages | apply reviewed Plan | W | required | yes | mutation/stale/recovery | `gui.m7-operation-flows` |
| `unity.installations.list` | `unity.read.v1` | Unity | page installations | R | no | no | data/keyset | `gui.m7-core-surfaces` |
| `unity.installations.get` | `unity.read.v1` | Unity installation detail | load installation | R | no | no | data | `gui.m7-core-surfaces` |
| `unity.installations.register` | `unity.manage.v1` | Unity add dialog | register validated editor | W | yes | no | mutation/idempotency | `gui.m7-operation-flows` |
| `unity.installations.remove` | `unity.manage.v1` | Unity installation detail | remove registry entry | W | yes | no | mutation/stale | `gui.m7-operation-flows` |
| `unity.installations.refresh` | `unity.manage.v1` | Unity | refresh discoveries | W | no | no | refreshing/idempotency | `gui.m7-operation-flows` |
| `unity.projectEditor.get` | `unity.read.v1` | Project Unity | load editor preference | R | no | no | data | `gui.m7-core-surfaces` |
| `unity.projectEditor.set` | `unity.manage.v1` | Project Unity | select editor | W | yes | no | mutation/stale | `gui.m7-operation-flows` |
| `unity.writerState` | `unity.read.v1` | Project Unity | observe writer evidence | R | no | no | refreshing/unknown | `gui.m7-core-surfaces` |
| `unity.launch` | `unity.launch.v1` | Project Unity | launch selected editor | W | yes | launch record | mutation/writer gate | `gui.m7-operation-flows` |
| `unity.launchStatus` | `unity.launch.v1` | Project Unity | follow launch | R | no | launch record | refreshing/terminal | `gui.m7-operation-flows` |
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
| `backups.planRestore` | `backups.restore.v1` | Backup detail | plan restore | R/Plan | review follows | no | data/stale | `gui.m7-operation-flows` |
| `backups.applyRestore` | `backups.restore.v1` | Backup detail | apply reviewed restore | W | required | yes | mutation/recovery | `gui.m7-operation-flows` |
| `extensions.list` | `extensions.lifecycle.v1` | Extensions | page extensions | R | no | no | data/keyset | `gui.m7-core-surfaces` |
| `extensions.get` | `extensions.lifecycle.v1` | Extension detail | load trust/runtime/permissions/UI | R | no | no | data | `gui.m7-core-surfaces` |
| `extensions.planInstall` | `extensions.lifecycle.v1` | Extensions install dialog | plan install | R/Plan | review follows | no | data/stale | `gui.m7-operation-flows` |
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
| `settings.get` | base RPC v1; no new capability approved | Settings, shell | load Config v1 | R | no | no | data/revision | `settings.authoritative-storage` |
| `settings.update` | base RPC v1; no new capability approved | Settings | update closed partial settings | W | explicit save | no | mutation/stale | `settings.authoritative-storage` |
| `activity.list` | base RPC v1; no new capability approved | Activity | page safe Event/Operation projection | R | no | no | data/keyset | `activity.log-redaction` |
| `diagnostics.list` | base RPC v1; no new capability approved | Diagnostics | page redacted diagnostics | R | no | no | data/keyset | `diagnostics.redacted-list` |

## Coverage rules

- New method beyond the four approved M7 additions or any new public capability requires owner approval.
- A method is complete only when typed Rust and TypeScript adapters, real page/action behavior, common states and its test owner all exist.
- `events.list` remains a daemon projection source; Activity UI must call `activity.list`, not join raw Events and Operations in JavaScript.
- Direct registry/lifecycle writes that do not have a Core Plan method receive explicit confirmation but must not invent a GUI Plan.
- Any Apply returning an Operation follows the same Operation detail model and survives route/window close and reconnect.
