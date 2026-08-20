# Extension Permissions v1

状态：Draft

## 通用数据与业务权限

```text
projects.read
projects.manage
projects.create
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
settings.read
settings.manage
access.read
access.manage
```

## 扩展专用权限

```text
ui.contribute
background.run
network.request
notifications.send
clipboard.read
clipboard.write
external-config.read
external-config.manage
integrations.discord.presence
mcp.sessions.read
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

- `unity.read`：查询 installation registry、project Editor preference、writer state 与 launch status；
  不允许修改 registry、启动 Editor 或读取任意进程技术信息。
- `unity.manage`：manual installation add/remove、受限 discovery refresh 与 project Editor preference
  修改；不包含 `unity.launch`、package mutation 或任意 settings 写入。
- `unity.launch`：只允许通过 application 启动/观察已验证 Editor 与显式 ProjectId；不允许 registry
  修改、shell command、任意 executable 或绕过 writer gate。
- `projects.create`：只允许在显式、不存在的 destination 创建一个全新 Project；不允许删除、覆盖、
  merge 或修改既有 Project，也不扩大 `projects.manage` 的 M3 registry 语义。
- template create-project 需要 `templates.read + projects.create`；backup restore 需要
  `backups.read + backups.manage + projects.create`；backup create 需要 `backups.manage` 与目标 project
  read scope。
- `builtin:local-owner` 可获得 M5 已批准权限。真实外部 Principal credential enrollment/revocation
  尚未实现，高影响 write path 不得描述为已向任意第三方开放。
- capability、InstallationId、LaunchId、TemplateId、BackupId、路径、PID 与 client metadata 都不是
  Principal 或授权凭据。
