# 首发业务入口对照

三端执行同一业务用例。表中“缺少入口”是后续分域 PR 的交付项。MCP 的关联工具可提供目标选择与结果摘要；身份、归属、撤销、修订和计划时效均由 Core 校验。

| Core 业务操作 | GUI | CLI | MCP |
|---|---|---|---|
| `backups.create` | 已提供 | 已提供 | 已提供 |
| `backups.restore` | 已提供 | 已提供 | 已提供 |
| `events.list` | 缺少入口 | 缺少入口 | 缺少入口 |
| `events.subscribe` | 已提供 | 缺少入口 | 缺少入口 |
| `extensions.applyInstall` | 已提供 | 缺少入口 | 缺少入口 |
| `extensions.applyUninstall` | 已提供 | 缺少入口 | 缺少入口 |
| `extensions.disable` | 已提供 | 缺少入口 | 缺少入口 |
| `extensions.enable` | 已提供 | 缺少入口 | 缺少入口 |
| `extensions.get` | 已提供 | 缺少入口 | 缺少入口 |
| `extensions.inspectSource` | 已提供 | 缺少入口 | 缺少入口 |
| `extensions.list` | 已提供 | 缺少入口 | 缺少入口 |
| `extensions.permissions.list` | 已提供 | 缺少入口 | 缺少入口 |
| `extensions.planInstall` | 已提供 | 缺少入口 | 缺少入口 |
| `extensions.planInstallFromSource` | 已提供 | 缺少入口 | 缺少入口 |
| `extensions.planUninstall` | 已提供 | 缺少入口 | 缺少入口 |
| `extensions.revokeGrant` | 已提供 | 缺少入口 | 缺少入口 |
| `extensions.setGrant` | 已提供 | 缺少入口 | 缺少入口 |
| `extensions.ui.close` | 已提供 | 缺少入口 | 缺少入口 |
| `extensions.ui.dispatch` | 已提供 | 缺少入口 | 缺少入口 |
| `extensions.ui.open` | 已提供 | 缺少入口 | 缺少入口 |
| `extensions.ui.refresh` | 已提供 | 缺少入口 | 缺少入口 |
| `logs.read` | 已提供 | 缺少入口 | 缺少入口 |
| `operations.cancel` | 已提供 | 已提供 | 已提供 |
| `operations.get` | 已提供 | 已提供 | 已提供 |
| `operations.list` | 已提供 | 已提供 | 已提供 |
| `packages.applyPlan` | 已提供 | 已提供 | 已提供 |
| `packages.planBulk` | 已提供 | 缺少入口 | 已提供 |
| `packages.planDowngrade` | 已提供 | 已提供 | 已提供 |
| `packages.planInstall` | 已提供 | 已提供 | 已提供 |
| `packages.planReinstall` | 已提供 | 缺少入口 | 已提供 |
| `packages.planRemove` | 已提供 | 已提供 | 已提供 |
| `packages.planResolve` | 已提供 | 已提供 | 已提供 |
| `packages.planUpgrade` | 已提供 | 已提供 | 已提供 |
| `packages.projectInventory` | 已提供 | 已提供 | 已提供 |
| `packages.queryProjectCandidates` | 已提供 | 缺少入口 | 已提供 |
| `packages.userPackages.enroll` | 已提供 | 缺少入口 | 缺少入口 |
| `packages.userPackages.get` | 已提供 | 缺少入口 | 缺少入口 |
| `packages.userPackages.list` | 已提供 | 缺少入口 | 缺少入口 |
| `packages.userPackages.refresh` | 已提供 | 缺少入口 | 缺少入口 |
| `packages.userPackages.remove` | 已提供 | 缺少入口 | 缺少入口 |
| `plans.get` | 缺少入口 | 已提供 | 缺少入口 |
| `projectTemplates.edit` | 已提供 | 已提供 | 已提供 |
| `projectTemplates.export` | 已提供 | 已提供 | 已提供 |
| `projectTemplates.get` | 已提供 | 已提供 | 已提供 |
| `projectTemplates.list` | 已提供 | 已提供 | 已提供 |
| `projectTemplates.remove` | 已提供 | 已提供 | 已提供 |
| `projectTemplates.save` | 已提供 | 已提供 | 已提供 |
| `projects.applyCopy` | 已提供 | 已提供 | 已提供 |
| `projects.applyDeleteDirectory` | 已提供 | 已提供 | 缺少入口 |
| `projects.applyUnityMigration` | 已提供 | 已提供 | 缺少入口 |
| `projects.create` | 已提供 | 已提供 | 已提供 |
| `projects.get` | 已提供 | 已提供 | 已提供 |
| `projects.inspect` | 已提供 | 已提供 | 缺少入口 |
| `projects.list` | 已提供 | 已提供 | 已提供 |
| `projects.planCopy` | 已提供 | 已提供 | 已提供 |
| `projects.planDeleteDirectory` | 已提供 | 已提供 | 缺少入口 |
| `projects.planUnityMigration` | 已提供 | 已提供 | 缺少入口 |
| `projects.refresh` | 已提供 | 已提供 | 缺少入口 |
| `projects.register` | 已提供 | 已提供 | 已提供 |
| `projects.setFavorite` | 已提供 | 缺少入口 | 缺少入口 |
| `projects.unregister` | 已提供 | 已提供 | 缺少入口 |
| `repositories.cancelListRegistration` | 已提供 | 缺少入口 | 缺少入口 |
| `repositories.exportListFile` | 已提供 | 缺少入口 | 缺少入口 |
| `repositories.get` | 已提供 | 已提供 | 缺少入口 |
| `repositories.importListFile` | 已提供 | 缺少入口 | 缺少入口 |
| `repositories.inspect` | 已提供 | 已提供 | 已提供 |
| `repositories.inspectListFile` | 已提供 | 缺少入口 | 缺少入口 |
| `repositories.list` | 已提供 | 已提供 | 已提供 |
| `repositories.packages` | 已提供 | 已提供 | 缺少入口 |
| `repositories.previewListExport` | 已提供 | 缺少入口 | 缺少入口 |
| `repositories.refresh` | 已提供 | 已提供 | 已提供 |
| `repositories.refreshAll` | 已提供 | 缺少入口 | 缺少入口 |
| `repositories.register` | 已提供 | 已提供 | 已提供 |
| `repositories.setCredentials` | 已提供 | 已提供 | 缺少入口 |
| `repositories.move` | 已提供 | 缺少入口 | 缺少入口 |
| `repositories.setDisplayName` | 已提供 | 缺少入口 | 缺少入口 |
| `repositories.unregister` | 已提供 | 已提供 | 已提供 |
| `settings.get` | 已提供 | 缺少入口 | 已提供 |
| `settings.update` | 已提供 | 缺少入口 | 缺少入口 |
| `state.check` | 已提供 | 缺少入口 | 缺少入口 |
| `system.status` | 已提供 | 已提供 | 已提供 |
| `unity.hub.open` | 已提供 | 缺少入口 | 缺少入口 |
| `unity.installations.get` | 已提供 | 已提供 | 缺少入口 |
| `unity.installations.list` | 已提供 | 已提供 | 已提供 |
| `unity.installations.refresh` | 已提供 | 已提供 | 缺少入口 |
| `unity.installations.register` | 已提供 | 已提供 | 缺少入口 |
| `unity.installations.remove` | 已提供 | 已提供 | 缺少入口 |
| `unity.launch` | 已提供 | 已提供 | 缺少入口 |
| `unity.launchOptions` | 已提供 | 已提供 | 缺少入口 |
| `unity.projectLaunchConfig.clear` | 已提供 | 已提供 | 缺少入口 |
| `unity.projectLaunchConfig.get` | 已提供 | 已提供 | 已提供 |
| `unity.projectLaunchConfig.set` | 已提供 | 已提供 | 缺少入口 |
| `unity.writerState` | 已提供 | 已提供 | 缺少入口 |

## 本机交互

| 系统适配操作 | GUI | CLI | MCP |
|---|---|---|---|
| 原生文件和文件夹选择器 | 仅本机交互 | 通过参数提供路径 | 通过工具参数提供路径 |
| 聚焦本机 Unity 窗口 | 仅本机交互 | 缺少入口 | 缺少入口 |

打开系统文件选择器、聚焦原生 Unity 窗口和启动系统浏览器需要当前桌面会话。接受路径的业务操作在 Core 校验目标。扩展 UI 的打开、事件分派和刷新用于渲染交互式视图，业务授权受扩展权限和宿主沙箱控制。

连接握手、OAuth 重定向和令牌交换属于会话协议。CLI 提供本机凭据与授权请求命令，GUI 通过管理扩展展示连接和撤销操作。认证资料的签发、保管及一次性展示使用相应传输规则。

## 连接和 MCP 适配方法

| 方法 | 实际调用方与用途 |
|---|---|
| `system.hello` | 三端及 SDK 在每次新连接上协商。 |
| `mcp.clients.list/create/issueCredential/revoke` | 管理扩展经宿主操作客户端；CLI 提供列表与凭据签发。MCP 工具面缺少管理入口。 |
| `mcp.oauth.registrations.list/set/revoke`、`mcp.oauth.requests.get/decide` | CLI 提供登记、撤销及知情授权，GUI 和 MCP 工具面缺少相应入口。 |
| `mcp.oauth.begin/finish/token/revoke` | MCP HTTP 按 OAuth 交互处理浏览器跳转、令牌交换和撤销。 |
| `mcp.packages.query/plan` | MCP 工具投影有界候选与计划。Core 包目录和求解规则与 GUI、CLI 共用。 |
| `mcp.repositories.list/inspect/register/remove` | MCP 工具投影来源摘要并绑定预览；GUI 和 CLI 直接调用仓库用例。 |
| `mcp.projects.create/register/planCopy` | MCP 工具接受默认目录或绝对目标；创建和复制由 Core 绑定目录身份并复验。 |
| `mcp.runtime.snapshot`、`mcp.transport.snapshot` | 管理界面通过扩展宿主读取连接/传输快照。 |
| `mcp.transport.report/bindRequest` | MCP 传输进程报告和绑定它实际接收的连接/请求。 |
| `mcp.tools.catalog`、`extensions.mcpTools.call` | MCP 传输发现并分派扩展工具，宿主校验扩展授权及沙箱。 |

## 同一操作的调用与重试

| 操作 | 三端送入 Core 的关键信息 | 重试或续传 |
|---|---|---|
| 默认软件包安装、升级 | 项目、修订、包 ID 和动作。显式选择才携版本策略或来源。批量操作对每个包沿用相同规则。 | Apply 重用已审阅的 planId、原项目修订、幂等键。 |
| 项目复制、删除与 Unity 迁移 | 明确项目或安装目标，Plan 冻结来源、目标和业务证据。命令行在确认前可按计划 ID 读取原计划。 | 同一次计划和执行分别保留各自的键。Apply 复查归属、时效和目标。 |
| ZIP 备份 | 项目、修订、选定目的地，以及用户明确覆盖的选项。省略选项由 Core 读取。 | 回执未知时重发相同参数和键。 |
| 项目、仓库与包清单分页 | 原样续传前页返回的游标。任务列表使用活动筛选和完整的时间、任务 ID 游标。 | 延续同一次查询的筛选与顺序。 |
| 设置更新 | 只发送变更字段及预期修订。关闭预发布可见性时 Core 在同一次写入将偏好复位。 | 回执未知后回读设置和修订核实结果。 |
| 扩展可移植界面 | 打开、刷新、关闭受具体会话寿命约束。交互事件发送请求 ID、序号和快照修订。 | 活跃会话重放原事件时保留三项证据。 |
