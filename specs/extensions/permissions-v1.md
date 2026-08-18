# Extension Permissions v1

状态：Draft

## 通用数据与业务权限

```text
projects.read
projects.manage
packages.read
packages.manage
repositories.read
repositories.manage
templates.read
templates.manage
unity.read
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
