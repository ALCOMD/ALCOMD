# Extension Permissions v1

状态：M7 active permission contract；M6 runtime 权限与 M7 Portable UI client permission 已接入 production。

## 通用数据与业务权限

```text
projects.read
projects.manage
projects.create
projects.delete
projects.unity-migrate
packages.read
packages.manage
repositories.read
repositories.manage
templates.read
templates.manage
unity.read
unity.manage
unity.launch
backups.read
backups.manage
operations.read
operations.cancel
events.subscribe
activity.read
diagnostics.read
settings.read
settings.manage
access.read
access.manage
extensions.read
extensions.manage
extensions.permissions.manage
extensions.ui.use
```

## 扩展专用权限

```text
background.run
network.request
notifications.send
clipboard.read
clipboard.write
external-config.read
external-config.manage
integrations.discord.presence
mcp.clients.read
mcp.clients.manage
mcp.configuration.read
mcp.configuration.manage
```

## 规则

- 第一方和第三方扩展使用同一权限名称。
- 权限按 Principal 授予并可随时撤销。
- Host Capability 每次调用仍需检查权限。
- 权限不能隐式扩大到子权限。
- 高风险权限必须显示具体资源范围。
- `integrations.discord.presence` 只允许 Presence 窄能力，不代表任意 Discord IPC。

## M3 项目与 Repository 权限

- `projects.read`：允许 `projects.inspect/list/get` 以及读取被授权项目的 normalized snapshot；不允许
  修改项目文件。
- `projects.manage`：允许在 ALCOMD registry 中 register/refresh/unregister 项目；它不隐含
  `packages.manage`，也不允许创建、删除或修改项目目录。
- `repositories.read`：允许 `repositories.inspect/list/get/packages` 的 normalized metadata 查询。
- `repositories.manage`：允许 register/refresh/unregister local 或匿名 HTTP(S) source 及
  last-known-good cache；不允许 credential、自定义 header、package payload 下载或项目写入。
- M3 `builtin:local-owner` 获得上述四项权限。capability、client metadata、路径、URL 与资源 ID
  均不是身份或授权凭据。
- 外部 Principal 的逐项目/逐 repository resource scope 和 credential enrollment/revocation 尚未
  实现；`access.principal-revocation` 继续保持 planned。

## M7 Project Directory Delete 权限

- `projects.delete` 只授权 `projects.planDeleteDirectory` / `projects.applyDeleteDirectory` 对一个已注册
  `ProjectId` 执行已冻结的 sibling-quarantine permanent delete；它不接受 caller path，也不隐含
  `projects.manage`、`projects.create`、package mutation 或任意目录删除能力。
- Plan 同时要求 `projects.read`；Apply 再次要求 `projects.delete`，并复验 Plan owner、Project revision、
  root/parent filesystem identity、ProjectVersion marker、writer evidence 与 protected-root policy。
- 外部 filesystem writer 只允许 `builtin:local-owner`。`projects.delete` 在 4.0.0 不授予 extension；
  first-party 身份、ExtensionId、ProjectId、PlanId、OperationId 与 client metadata 都不能替代授权。
- `quarantine_intent` 之后只能 forward recovery。权限不允许跳过 sibling quarantine、跨 mount、跟随 link、
  触碰 quarantine 后重建的原路径，或把 `not_observed` 表述为 Unity 确定未运行。

## M7 Project Unity Migration 权限

- `projects.unity-migrate` 只授权 `projects.planUnityMigration` / `projects.applyUnityMigration` 的 sealed
  Project Unity Version workflow；不接受 caller path、target version 双 authority、任意 package ChangeSet 或 public child Operation。
- Plan 同时要求 `projects.read + unity.read`；Apply 再次要求 `projects.unity-migrate`，并 fresh revalidate immutable
  Plan、Project revision/root/marker、target installation identity/version 与 writer state。
- 外部 filesystem mutation 只允许 `builtin:local-owner`。`projects.unity-migrate` 在 4.0.0 不授予 Extension，第一方
  Extension 也没有隐藏 privileged API。
- `preparation_intent` / `launch_intent` 之后只能按同一 Operation forward recovery，不允许普通 cancel、自动 respawn、
  kill Unity、跳过 exact Project reobservation 或产生中间 Project/package public revision/Event。

## M4 Package Plan/Apply 权限

- `packages.planInstall/planRemove/planUpgrade/planDowngrade/planResolve` 必须同时具备
  `projects.read`、`repositories.read` 与 `packages.read`，并受目标 project 与所引用 repository
  scope 限制。Plan 的内部 durable persistence 不要求 `packages.manage`，也不授权项目写入。
- `packages.applyPlan` 必须再次具备上述 read 权限，并额外具备 `packages.manage` 与目标 project
  write scope；每次 Apply 都复验 Plan owner、scope、revision 与 pinned source。
- `packages.manage` 不隐含 repository credential、legacy cleanup、任意项目路径写入或 Unity 启动。
  它只授权执行已经批准且绑定目标 ProjectId 的 package ChangeSet。
- capability、PlanId、OperationId、package/repository ID、SID 和 client metadata 都不是身份凭据。
- M4 仅允许 `builtin:local-owner` 使用 Apply 写路径。真实外部 credential enrollment/revocation 与
  第三方 mutation 尚未实现，不得因 Schema 已冻结而描述为开放。

## M5 CLI、Unity、Template 与 Backup 权限

- `unity.read`：查询 installation registry、project launch config、exact launch options、writer state 与 launch status；
  不允许修改 registry、启动 Editor 或读取任意进程技术信息。
- `unity.manage`：manual installation add/remove、受限 discovery refresh 与 project launch arguments
  修改；不包含 `unity.launch`、Installation preference、package mutation 或任意 settings 写入。
- `unity.launch`：只允许通过 application 启动/观察与 Project canonical exact version 匹配的已验证 Editor，
  且 InstallationId 只作为 one-shot request authority；不允许 registry
  修改、shell command、任意 executable 或绕过 writer gate。
- `projects.create`：只允许在显式、不存在的 destination 创建一个全新 Project；不允许删除、覆盖、
  merge 或修改既有 Project，也不扩大 `projects.manage` 的 M3 registry 语义。
- `templates.read`：允许 list/get/inspect/export 已授权 template；不允许注册、favorite、remove、derive、
  object publish 或创建项目。inspect/export 仍由 application/RPC 执行，不能据此直接读取 object store。
- `templates.manage`：允许 user template import/explicit override、derive、favorite 与 remove；不允许修改或
  删除 builtin，不隐含 `projects.create`、`projects.read` 或任意 package mutation authority。
- derive from Project 还要求目标 Project 的 `projects.read` scope。template create-project Plan 需要
  `templates.read + projects.create`；有 dependency 时还需要 `packages.read + repositories.read`，Apply
  materialization 另需既有 `packages.manage`。`templates.manage` 不能隐式提供这些 package 权限。
- `backups.list` 与 `backups.get` 只要求 `backups.read`。Backup Create contract-first 的
  `backups.create` 要求 `backups.manage` 与目标 Project 的 `projects.read` scope；它不要求
  `projects.manage`、`projects.create` 或 `packages.manage`。`excludeVpmPackages` 只读取 normalized locked
  set，不授予 package mutation authority。filesystem write 仍只对 `builtin:local-owner` 开放。
- `backups.planRestore` 需要 `backups.read + projects.create`；它只创建永久 immutable Plan，不创建
  staging、target 或 Operation。`backups.applyRestore` 必须重新验证相同权限，并额外需要
  `backups.manage`。两者都只允许恢复到明确不存在的全新 Project target，不隐含 package/repository/
  Unity 权限，也不允许 overwrite、merge、delete-then-restore 或 arbitrary ZIP。
- Template/Backup 所有外部 filesystem write 当前仍只允许 `builtin:local-owner`；Schema/PlanId、
  TemplateId、bundle digest、target path 与 object locator 都不是授权凭据。
- `builtin:local-owner` 可获得 M5 已批准权限。真实外部 Principal credential enrollment/revocation
  尚未实现，高影响 write path 不得描述为已向任意第三方开放。
- capability、InstallationId、LaunchId、TemplateId、BackupId、路径、PID 与 client metadata 都不是
  Principal 或授权凭据。

## M6 Extension Runtime 权限与 scope

M6 第一条生产 slice 冻结并仅使用以下权限：

| permission | caller | exact authority |
|---|---|---|
| `extensions.read` | local management Principal | list/get installed extension、desired/runtime safe summary；不返回 Host PID、路径、lease secret 或 data value |
| `extensions.manage` | local management Principal | plan/apply install、enable、disable、plan/apply uninstall；不隐含 permission grant |
| `extensions.permissions.manage` | local management Principal | set/revoke extension grant 与 specific resource scope；grant revision durable update 是 revoke linearization point |
| `background.run` | extension Principal | 允许已验证 background Component 被 Host 启动；不隐含任何业务 capability |
| `projects.read` | extension Principal | 仅通过 `host-projects.get-summary` 读取 grant 中 specific ProjectId 的安全摘要 |

规则：

- extension Principal 默认零权限。Manifest request、ExtensionId、publisher、first-party status、package digest、
  WIT capability 或 `ExtensionInstanceLease` ID 都不是 grant。
- M6 scope 的唯一 business resource kind 是 `Project`，resource ID 是 lowercase UUID。`projects.read` 对 extension
  必须有至少一个 specific ProjectId；不允许 wildcard、path、URL 或自报 selector。
- `ExtensionInstanceLease` 绑定 current grant revision；每次 Host/data call 重新解析真实 Principal/grant/scope。
- revoke transaction 提交后，尚未被 application 接受的 call、Host queue、session/handle 均失败或取消。
- uninstall 无论 `retain_data|delete_data` 都在 package authority removal 前 revoke 全部 grant/lease/session/handle；
  retained namespace 不保留 active grant，未来 reinstall 重新从 deny-by-default 开始。
- 第一方只可获得同名 permission 与同种 scope；official trust 不绕过 grant/revoke/quota。
- `network.request`、notifications、clipboard、external-config、Discord 与 M7 UI placement 仍 planned，M6 第一条
  slice 不链接或广告它们。
- `mcp.sessions.read` 已由 A-023 拒绝；未来 MCP request/connection/subscription scope 属于 M8，不在 M6 重建。

## M7 Portable UI 权限与双重授权

- `extensions.ui.use` 是 client permission，resource kind 固定为 `Extension`，resource ID 必须是 exact ExtensionId。
  它不允许 list/install/enable/grant、读取 extension-owned data 或调用其他 extension scope。
- `extensions.ui.open/refresh/dispatch` 每次都要求 client 同时拥有 `extensions.read` 与 scoped
  `extensions.ui.use`；close 只允许当前 connection/session owner best-effort 执行。
- interactive UI 中的 `host-projects.get-summary(ProjectId)` 要求 Client Principal 与 Extension Principal 对同一
  ProjectId 均有 `projects.read`。任一侧不能扩大另一侧 authority。
- interactive UI 中的 host-data 要求 client scoped `extensions.ui.use`，同时 Extension Principal 只能访问自身
  ExtensionId namespace；client 不因 UI session 获得 raw extension data authority。
- 不存在 `ui.contribute`。Manifest `[ui]` 只是 package declaration，不是 permission 或 OS/Core capability。
- `background.run` 只允许没有 active client UI session 时持有 background lifecycle lease，不由 `[ui]` 隐式加入。
  UI-only extension 可不请求它；Portable UI session 也不产生隐式 background lease。
- client capability/metadata、GUI identity、session ID、guest-session-id 与 InvocationContextId 都不是授权凭据。

## M7 Official GUI settings/activity/diagnostics

- `settings.read` 只允许读取 Config Schema 1 的规范化公开设置；不允许直接读取文件、extension-owned
  data 或未来 updater/MCP/Discord/migration setting。
- `settings.manage` 只允许以 `expectedRevision` 更新同一 closed partial settings DTO；它不授权通用
  key/value config、路径写入或 State DB 修改。
- `activity.read` 只允许读取现有 durable Event/Operation 的 closed user-facing projection；不返回 payload、
  request/result JSON、技术日志、路径、credential 或 extension value。
- `diagnostics.read` 只允许读取 Operation failure 与 safe Event evidence 的 bounded redacted projection；不
  授权 raw log export、argv/env、SQL、Debug/backtrace、路径或 Portable UI payload，且默认不授予外部
  Principal。
- `settings.get`、`settings.update`、`activity.list`、`diagnostics.list` 是 base RPC v1 compatible additions；
  不新建 capability，也不能把 client metadata 当作上述 permission。
