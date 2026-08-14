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
