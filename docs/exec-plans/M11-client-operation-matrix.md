# 首发业务入口对照

本表按 Core 的生产方法逐行核对。入口名称指用户能在 GUI 发起的操作、可执行的 CLI 子命令或 MCP 公布的工具。GUI 的自动读取和协议握手标明了触发条件。使用 MCP 凭据直接调用原生 SDK 能验证 Core 的业务权限，不能据此认定 MCP 用户已有对应工具。[首发清单](../../specs/first-release-contracts.json) · [原生方法目录](../../specs/rpc/native-sdk-v1.methods.json) · [分域验收计划](M11-client-parity.md)

“已提供”表示已从用户入口追到实际调用。“部分可达”标出原生方法与入口在目标、参数、结果或独立调用方式上的具体差异。“已验证”只用于已运行且能对应该入口的用例，证据在单元格内。“缺少入口”是后续交付项。“仅本机交互”和“协议专用”分别记录具体的桌面交互和传输责任。同一方法通过专用适配器调用时保留其名字，以便核对实际请求。

| Core 业务操作 | GUI | CLI | MCP |
|---|---|---|---|
| `clients.disconnect` | 已提供 · 确认当前 Core 实例与选定连接后断开 | 已提供 · client disconnect | 已提供 · alcomd_disconnect_client |
| `clients.instances.list` | 已提供 · 查看所选程序的实例、进程和连接 | 已提供 · client instances | 已提供 · alcomd_list_client_instances |
| `clients.list` | 已提供 · 设置 → 客户端显示已安装或已连接的程序 | 已提供 · client list | 已提供 · alcomd_list_clients |
| `desktop.focusProjectUnity` | 已提供 · 项目操作聚焦 Core 所在机器上的 Unity 窗口 | 已提供 · desktop focus-project-unity | 已提供 · alcomd_focus_project_unity |
| `desktop.openProjectDirectory` | 已提供 · 项目操作在 Core 的文件管理器中打开目录 | 已提供 · desktop open-project-directory | 已提供 · alcomd_open_project_directory |
| `desktop.openUnityHubDownload` | 已提供 · Unity 设置打开固定的 Hub 下载页 | 已提供 · desktop open-unity-hub-download | 已提供 · alcomd_open_unity_hub_download |
| `mcp.clients.create` | 已提供 · 登记 AI 客户端 | 已提供 · mcp client create | 仅本机交互 · AI 通过 alcomd_request_client_change 提交同一动作，由独立所有者批准 |
| `mcp.clients.issueCredential` | 已提供 · 确认替换凭据，签发后展示一次 | 已提供 · mcp client issue-credential | 仅本机交互 · AI 通过 alcomd_request_client_change 提交同一动作，由独立所有者批准 |
| `mcp.clients.list` | 已提供 · AI 客户端分页清单 | 已提供 · mcp client list | 已提供 · alcomd_list_ai_clients |
| `mcp.clients.requests.create` | 已提供 · 提交已审阅的管理动作 | 已提供 · mcp client request create | 已提供 · alcomd_request_client_change |
| `mcp.clients.requests.decide` | 已提供 · 所有者批准或拒绝原申请 | 已提供 · mcp client request approve 或 mcp client request deny | 仅本机交互 · 独立所有者决定，AI 调用方可查询原申请 |
| `mcp.clients.requests.get` | 已提供 · 查看申请人、原操作与期限 | 已提供 · mcp client request get | 已提供 · alcomd_get_client_change_request |
| `mcp.clients.requests.list` | 已提供 · 管理申请分页列表 | 已提供 · mcp client request list | 已提供 · alcomd_list_client_change_requests |
| `mcp.clients.revoke` | 已提供 · 选中并确认撤销客户端 | 已提供 · mcp client revoke | 仅本机交互 · AI 通过 alcomd_request_client_change 提交同一动作，由独立所有者批准 |
| `mcp.oauth.registrations.list` | 已提供 · OAuth 登记分页清单 | 已提供 · mcp oauth registration list | 已提供 · alcomd_list_oauth_registrations |
| `mcp.oauth.registrations.revoke` | 已提供 · 确认撤销选中的 OAuth 登记 | 已提供 · mcp oauth registration revoke | 仅本机交互 · AI 通过 alcomd_request_client_change 提交同一动作，由独立所有者批准 |
| `mcp.oauth.registrations.set` | 已提供 · 确认 OAuth 登记配置 | 已提供 · mcp oauth registration set | 仅本机交互 · AI 通过 alcomd_request_client_change 提交同一动作，由独立所有者批准 |
| `mcp.oauth.requests.decide` | 已提供 · 所有者审阅跳转目标后决定 | 已提供 · mcp oauth request approve 或 mcp oauth request deny | 仅本机交互 · 独立所有者决定，AI 调用方可查询原申请 |
| `mcp.oauth.requests.get` | 已提供 · 按 ID 读取 OAuth 授权申请 | 已提供 · mcp oauth request show | 已提供 · alcomd_get_oauth_request |
| `mcp.runtime.snapshot` | 已提供 · 客户端活动中的 Core 连接与调用 | 已提供 · mcp runtime | 已提供 · alcomd_get_mcp_runtime_snapshot |
| `mcp.transport.snapshot` | 已提供 · 客户端活动中的传输、观察者与订阅 | 已提供 · mcp transport | 已提供 · alcomd_get_mcp_transport_snapshot |
| `backups.create` | 已提供 · 项目备份对话框选择目的地及覆盖项，确认创建 | 已提供 · backup create | 已提供 · alcomd_create_backup；项目修订、可选目的地和备份选项 |
| `backups.restore` | 已提供 · 项目列表「恢复备份」选择 ZIP、名称或目的地 | 已提供 · backup restore | 已提供 · alcomd_restore_backup；确认源 ZIP 及恢复目标 |
| `events.list` | 缺少入口 · 没有历史事件页，任务页查询 operations | 缺少入口 · CLI 未提供事件读取或订阅；operation follow 轮询单项任务 | 缺少入口 · MCP 的工具目录变更订阅不能读取或续接 Core 业务事件 |
| `events.subscribe` | 已提供 · 应用启动后订阅；事件进入通知、任务和资源列表刷新 | 缺少入口 · CLI 未提供事件读取或订阅；operation follow 轮询单项任务 | 缺少入口 · MCP 的工具目录变更订阅不能读取或续接 Core 业务事件 |
| `extensions.cliCommands.invoke` | 协议专用 · GUI 按已签名的可移植界面声明展示并分派控件动作 | 已提供 · `extension run` | 协议专用 · MCP 使用单独签名的工具声明和导出 |
| `extensions.cliCommands.list` | 协议专用 · GUI 按已签名的可移植界面声明展示并分派控件动作 | 已提供 · `extension commands` | 协议专用 · MCP 使用单独签名的工具声明和导出 |
| `extensions.applyInstall` | 已提供 · 扩展安装审阅后同意权限并提交原计划 | 已提供 · `extension apply-install` | 已提供 · `alcomd_apply_extension_install` |
| `extensions.applyUninstall` | 已提供 · 扩展卸载对话框确认已显示计划 | 已提供 · `extension apply-uninstall` | 已提供 · `alcomd_apply_extension_uninstall` |
| `extensions.disable` | 已提供 · 扩展列表或详情的停用按钮 | 已提供 · `extension disable` | 已提供 · `alcomd_disable_extension` |
| `extensions.enable` | 已提供 · 扩展列表或详情的启用按钮 | 已提供 · `extension enable` | 已提供 · `alcomd_enable_extension` |
| `extensions.get` | 已提供 · 扩展详情或打开扩展视图时读取记录 | 已提供 · `extension get` | 已提供 · `alcomd_get_extension` |
| `extensions.inspectSource` | 已提供 · 扩展安装对话框检查远程来源 | 已提供 · `extension inspect-source` | 已提供 · `alcomd_inspect_extension_source` |
| `extensions.list` | 已提供 · 扩展管理页读取当前扩展 | 已提供 · `extension list` | 已提供 · `alcomd_list_extensions` |
| `extensions.permissions.list` | 已提供 · 扩展详情的权限面板读取授权 | 已提供 · `extension permissions` | 已提供 · `alcomd_list_extension_permissions` |
| `extensions.planInstall` | 已提供 · 扩展安装对话框选择本地包并预演 | 已提供 · `extension plan-install` | 已提供 · `alcomd_plan_extension_install` |
| `extensions.planInstallFromSource` | 已提供 · 扩展安装对话框选定已检查的源及版本再预演 | 已提供 · `extension plan-install-from-source` | 已提供 · `alcomd_plan_extension_install_from_source` |
| `extensions.planUninstall` | 已提供 · 扩展详情预演卸载 | 已提供 · `extension plan-uninstall` | 已提供 · `alcomd_plan_extension_uninstall` |
| `extensions.revokeGrant` | 已提供 · 扩展权限面板撤销选定授权 | 已提供 · `extension revoke` | 已提供 · `alcomd_revoke_extension_permission` |
| `extensions.setGrant` | 已提供 · 扩展权限面板授予选定权限 | 已提供 · `extension grant` | 已提供 · `alcomd_grant_extension_permission` |
| `extensions.ui.close` | 已提供 · 离开扩展页面或关闭视图后释放原会话 | 协议专用 · 可移植视图绑定原 Core IPC 连接，独立 CLI 命令结束时连接关闭 | 协议专用 · MCP 工具没有该可移植视图的连接及控件状态，MCP 可调用独立签名工具 |
| `extensions.ui.dispatch` | 已提供 · 在扩展视图内操作控件，发送会话、快照修订和事件序号 | 协议专用 · 可移植视图绑定原 Core IPC 连接，独立 CLI 命令结束时连接关闭 | 协议专用 · MCP 工具没有该可移植视图的连接及控件状态，MCP 可调用独立签名工具 |
| `extensions.ui.open` | 已提供 · 从导航或扩展页打开界面并取得会话 | 协议专用 · 可移植视图绑定原 Core IPC 连接，独立 CLI 命令结束时连接关闭 | 协议专用 · MCP 工具没有该可移植视图的连接及控件状态，MCP 可调用独立签名工具 |
| `extensions.ui.refresh` | 缺少入口 · 桥接层声明了刷新，实际视图只打开、分派和关闭；没有刷新调用点 | 协议专用 · 可移植视图绑定原 Core IPC 连接，独立 CLI 命令结束时连接关闭 | 协议专用 · MCP 工具没有该可移植视图的连接及控件状态，MCP 可调用独立签名工具 |
| `logs.read` | 已提供 · 日志路由按条件查询并续页 | 缺少入口 · CLI 没有读取 Core 操作日志的子命令 | 缺少入口 · 工具目录没有读取 Core 操作日志的工具 |
| `operations.cancel` | 已提供 · 任务面板及创建项目进度窗口取消当前任务 | 已提供 · operation cancel | 已提供 · alcomd_cancel_operation；自己的任务、预期修订和幂等键 |
| `operations.get` | 已提供 · 任务条目或等待操作完成时按 ID 查询 | 已提供 · operation get；follow 轮询同一任务 | 部分可达 · alcomd_get_operation；返回任务状态和修订，部分任务不返回类型专属结果 |
| `operations.list` | 已提供 · 任务页与活动任务指示器读取分页列表 | 已提供 · operation list；支持活动筛选和完整游标 | 部分可达 · alcomd_list_operations；游标、活动筛选和调用者范围，条目沿用受限结果投影 |
| `packages.applyPlan` | 已提供 · 软件包审阅窗口提交当前预演所得计划 | 已提供 · package apply-plan；或各预演命令确认后提交 | 已提供 · alcomd_apply_package_changes；确认原计划、修订和幂等键 |
| `packages.planBulk` | 部分可达 · 软件包表的批量操作可组合所选行；界面不接受任意意图数组 | 缺少入口 · package 命令没有该计划或候选查询动作 | 已提供 · alcomd_plan_package_changes；使用 bulk 意图，经 mcp.packages.plan |
| `packages.planDowngrade` | 已提供 · 软件包版本菜单选择较低版本，可确认精确 Unity 例外 | 已提供 · package downgrade | 已提供 · alcomd_plan_package_changes；降级意图，经 mcp.packages.plan |
| `packages.planInstall` | 部分可达 · 软件包表直接安装 Core 推荐或选定精确版本、来源；没有自由输入 SemVer 范围的控件 | 已提供 · package install；省略版本走 Core，可显式传范围、来源或精确例外 | 已提供 · alcomd_plan_package_install 或 alcomd_plan_package_changes；缺省版本由 Core 选择 |
| `packages.planReinstall` | 已提供 · 软件包行菜单重装选中包；工作区菜单可全部重装 | 缺少入口 · package 命令没有该计划或候选查询动作 | 已提供 · alcomd_plan_package_reinstall 或 alcomd_plan_package_changes；按锁定包重装 |
| `packages.planRemove` | 已提供 · 软件包行的移除动作预演连带变更 | 已提供 · package remove | 已提供 · alcomd_plan_package_uninstall；通用 changes 工具也接受移除意图 |
| `packages.planResolve` | 部分可达 · 软件包菜单重新解析依赖，按 Core 设置选择预发布策略；没有针对本次解析单独覆盖的控件 | 已提供 · package resolve | 已提供 · alcomd_plan_package_changes；解析意图，经 mcp.packages.plan |
| `packages.planUpgrade` | 部分可达 · 软件包表使用推荐或选定版本、来源；没有自由输入 SemVer 范围的控件 | 已提供 · package upgrade；省略版本走 Core，可显式传范围、来源或精确例外 | 已提供 · alcomd_plan_package_changes；升级意图可省略版本 |
| `packages.projectInventory` | 已提供 · 项目详情打开已安装软件包清单 | 已提供 · package inventory；支持项目修订及分页 | 已提供 · alcomd_list_installed_packages；直接读取项目清单 |
| `packages.queryProjectCandidates` | 已提供 · 项目包表、版本和来源选择器按当前项目快照查询 | 缺少入口 · package 命令没有该计划或候选查询动作 | 部分可达 · alcomd_search_packages、alcomd_get_package、alcomd_list_package_sources 经 mcp.packages.query 查询；不提供原生候选批量动作视图 |
| `packages.userPackages.enroll` | 已提供 · 资源页的用户包列表选择本地目录登记 | 缺少入口 · CLI 没有管理本地用户包来源的命令 | 缺少入口 · 工具可使用已登记来源，缺少管理本地用户包的工具 |
| `packages.userPackages.get` | 已提供 · 用户包详情查询选定来源 | 缺少入口 · CLI 没有管理本地用户包来源的命令 | 缺少入口 · 工具可使用已登记来源，缺少管理本地用户包的工具 |
| `packages.userPackages.list` | 已提供 · 资源页读取本地用户包清单 | 缺少入口 · CLI 没有管理本地用户包来源的命令 | 缺少入口 · 工具可使用已登记来源，缺少管理本地用户包的工具 |
| `packages.userPackages.refresh` | 已提供 · 用户包详情重新读取目录 | 缺少入口 · CLI 没有管理本地用户包来源的命令 | 缺少入口 · 工具可使用已登记来源，缺少管理本地用户包的工具 |
| `packages.userPackages.remove` | 已提供 · 用户包详情移除登记 | 缺少入口 · CLI 没有管理本地用户包来源的命令 | 缺少入口 · 工具可使用已登记来源，缺少管理本地用户包的工具 |
| `plans.get` | 缺少入口 · 界面审阅内存中刚生成的计划，没有按既有 ID 读取计划的入口 | 部分可达 · package apply-plan、project apply-* 的交互确认可内部读取原计划；没有独立查询命令 | 缺少入口 · 工具可以提交自己的既有计划，缺少按 ID 查询计划详情的工具 |
| `projectTemplates.edit` | 已提供 · 模板详情编辑并确认保存 | 已提供 · template edit | 已提供 · alcomd_edit_project_template |
| `projectTemplates.export` | 已提供 · 模板详情选择输出位置后导出 | 已提供 · template export | 已提供 · alcomd_export_project_template |
| `projectTemplates.get` | 已提供 · 模板详情读取定义与附件 | 已提供 · template get | 已提供 · alcomd_get_project_template |
| `projectTemplates.list` | 已提供 · 模板工作区和创建项目对话框列出可用模板 | 已提供 · template list | 已提供 · alcomd_list_project_templates |
| `projectTemplates.remove` | 已提供 · 模板工作区确认移除所选模板 | 已提供 · template remove | 已提供 · alcomd_remove_project_template |
| `projectTemplates.save` | 已提供 · 模板工作区创建并保存用户模板 | 已提供 · template save | 已提供 · alcomd_create_project_template |
| `projects.applyCopy` | 已提供 · 复制对话框确认原计划 | 已提供 · project apply-copy；或 project copy 确认后提交 | 已提供 · alcomd_apply_project_copy；确认原计划 |
| `projects.applyDeleteDirectory` | 已提供 · 项目目录删除对话框确认原计划 | 已提供 · project apply-delete-directory；或 project delete-directory 确认后提交 | 缺少入口 · 工具目录没有这项项目操作 |
| `projects.applyUnityMigration` | 已提供 · 确认迁移对话框中已审阅的计划 | 已提供 · project apply-unity-migration；或 project migrate-unity 确认后提交 | 缺少入口 · 工具目录没有这项项目操作 |
| `projects.create` | 已提供 · 项目列表打开创建对话框，选择模板、Unity 版本和目录 | 已提供 · project create | 已提供 · alcomd_create_project；经 mcp.projects.create 解析目标并调用创建用例 |
| `projects.get` | 已提供 · 从项目列表进入项目详情 | 已提供 · project get | 部分可达 · alcomd_get_project；按 ID 查询，返回项目快照选取的字段 |
| `projects.inspect` | 缺少入口 · 有 projectsInspect 桥接，项目添加对话框直接登记，没有独立检查操作 | 已提供 · project inspect | 缺少入口 · 工具目录没有这项项目操作 |
| `projects.list` | 已提供 · 项目列表或网格，支持筛选、排序和加载余页 | 部分可达 · project list 可分页；未提供原生列表的搜索和排序视图 | 部分可达 · alcomd_list_projects；支持分页与 OAuth 绑定，缺少搜索/排序视图，列表只返回摘要 |
| `projects.planCopy` | 已提供 · 项目操作菜单输入副本目录并预演 | 已提供 · project copy --dry-run | 已提供 · alcomd_plan_project_copy；经 mcp.projects.planCopy 解析复制目标 |
| `projects.planDeleteDirectory` | 已提供 · 项目操作菜单选择删除目录并预演 | 已提供 · project delete-directory --dry-run | 缺少入口 · 工具目录没有这项项目操作 |
| `projects.planUnityMigration` | 已提供 · 项目选择另一 Unity 安装，预演迁移 | 已提供 · project migrate-unity --dry-run | 缺少入口 · 工具目录没有这项项目操作 |
| `projects.refresh` | 缺少入口 · 重新加载列表走 projects.list，项目详情走 projects.get；未发现 projectRefresh 的界面调用 | 已提供 · project refresh | 缺少入口 · 工具目录没有这项项目操作 |
| `projects.register` | 已提供 · 项目列表选择现有目录登记 | 已提供 · project register | 部分可达 · alcomd_register_project；经 mcp.projects.register 登记目录，只返回已登记项目摘要 |
| `projects.setFavorite` | 已提供 · 项目行或卡片的收藏按钮 | 缺少入口 · project 命令未提供收藏设置 | 缺少入口 · 工具目录没有这项项目操作 |
| `projects.unregister` | 已提供 · 项目操作菜单从列表移除登记 | 已提供 · project unregister | 缺少入口 · 工具目录没有这项项目操作 |
| `repositories.cancelListRegistration` | 已提供 · 导入对话框用当前导入键停止后续登记 | 缺少入口 · repository 命令没有这项管理动作 | 缺少入口 · 工具目录没有这项仓库管理操作 |
| `repositories.exportListFile` | 已提供 · 资源页导出仓库清单并选择文件及格式 | 缺少入口 · repository 命令没有这项管理动作 | 缺少入口 · 工具目录没有这项仓库管理操作 |
| `repositories.get` | 已提供 · 仓库详情按 ID 读取元数据 | 已提供 · repository get | 缺少入口 · 工具目录没有这项仓库管理操作 |
| `repositories.importListFile` | 已提供 · 导入对话框确认检查过的文件，可选择覆盖已有配置 | 缺少入口 · repository 命令没有这项管理动作 | 缺少入口 · 工具目录没有这项仓库管理操作 |
| `repositories.inspect` | 已提供 · 添加仓库对话框检查来源，可输入认证头 | 已提供 · repository inspect | 部分可达 · alcomd_inspect_repository；通过 mcp.repositories.inspect 检查远程来源，不接收本地路径或自定义认证头 |
| `repositories.inspectListFile` | 已提供 · 导入仓库文件后显示逐行检查结果 | 缺少入口 · repository 命令没有这项管理动作 | 缺少入口 · 工具目录没有这项仓库管理操作 |
| `repositories.list` | 已提供 · 资源页的仓库列表及其顺序 | 已提供 · repository list；可原样续传游标 | 部分可达 · alcomd_list_repositories；有序分页摘要，省略来源地址和凭据 |
| `repositories.move` | 已提供 · 仓库列表的排序控件移动目标 | 缺少入口 · repository 命令没有这项管理动作 | 缺少入口 · 工具目录没有这项仓库管理操作 |
| `repositories.packages` | 已提供 · 仓库详情按页查看完整包版本清单 | 已提供 · repository packages；可原样续传游标 | 部分可达 · alcomd_search_packages 可按仓库筛选候选；工具面缺少含隐藏预发布版的原生仓库管理分页清单 |
| `repositories.previewListExport` | 已提供 · 导出对话框先显示当前格式的预览 | 缺少入口 · repository 命令没有这项管理动作 | 缺少入口 · 工具目录没有这项仓库管理操作 |
| `repositories.refresh` | 已提供 · 仓库详情的刷新按钮更新该来源 | 已提供 · repository refresh | 部分可达 · alcomd_refresh_repository；按 ID、修订刷新，返回仓库摘要 |
| `repositories.refreshAll` | 已提供 · 资源页刷新所有仓库 | 缺少入口 · repository 命令没有这项管理动作 | 缺少入口 · 工具目录没有这项仓库管理操作 |
| `repositories.register` | 已提供 · 添加仓库对话框确认检查过的来源 | 已提供 · repository register；本地路径或远程 URL，远程可带凭据 | 部分可达 · alcomd_register_repository；确认原远程预览，缺少本地来源及带自定义认证头的登记入口 |
| `repositories.setCredentials` | 已提供 · 仓库认证对话框替换或清除凭据 | 已提供 · repository set-credentials | 缺少入口 · 工具目录没有这项仓库管理操作 |
| `repositories.setDisplayName` | 已提供 · 仓库详情修改显示名称 | 缺少入口 · repository 命令没有这项管理动作 | 缺少入口 · 工具目录没有这项仓库管理操作 |
| `repositories.unregister` | 已提供 · 仓库详情确认移除该来源 | 已提供 · repository unregister | 已提供 · alcomd_remove_repository；经 mcp.repositories.remove 确认选定 ID、修订 |
| `settings.get` | 已提供 · 应用启动及设置页读取设置 | 缺少入口 · 本基线没有 settings 命令组 | 已提供 · alcomd_get_settings；直接读取 Core 配置 |
| `settings.update` | 已提供 · 设置页自动保存修改项，发送预期修订 | 缺少入口 · 本基线没有部分设置更新命令 | 缺少入口 · 目前只有 settings 读取工具 |
| `state.check` | 缺少入口 · 能力表和通知层识别请求名，界面没有提交检查的操作 | 缺少入口 · CLI 没有 state check 命令 | 缺少入口 · 工具目录没有发起 Core 状态检查的工具 |
| `system.status` | 已提供 · 首页、应用连接状态读取 Core 状态 | 已提供 · system status | 缺少入口 · 工具目录没有 Core 状态工具；server/discover 返回 MCP 能力 |
| `unity.hub.open` | 已提供 · 设置页打开已配置的本机 Unity Hub | 缺少入口 · unity 命令未提供启动 Hub 的动作 | 缺少入口 · 工具只提供安装列表或项目启动参数读取，缺少这项操作 |
| `unity.installations.get` | 缺少入口 · 桥接和事件缓存识别单条读取，安装管理页实际使用 installations.list | 已提供 · unity get | 缺少入口 · 工具只提供安装列表或项目启动参数读取，缺少这项操作 |
| `unity.installations.list` | 已提供 · Unity 设置和新建项目版本选择器读取安装列表 | 部分可达 · unity list 可分页；未提供 includeVersionOptions | 已提供 · alcomd_list_unity_installations；读取登记和版本选项 |
| `unity.installations.refresh` | 已提供 · Unity 设置或工具页发现安装 | 已提供 · unity refresh | 缺少入口 · 工具只提供安装列表或项目启动参数读取，缺少这项操作 |
| `unity.installations.register` | 已提供 · Unity 设置选择编辑器可执行文件登记 | 已提供 · unity register | 缺少入口 · 工具只提供安装列表或项目启动参数读取，缺少这项操作 |
| `unity.installations.remove` | 已提供 · Unity 设置移除选定安装 | 已提供 · unity remove | 缺少入口 · 工具只提供安装列表或项目启动参数读取，缺少这项操作 |
| `unity.launch` | 已提供 · 项目行或详情选择匹配的编辑器后启动 | 已提供 · unity launch | 缺少入口 · 工具只提供安装列表或项目启动参数读取，缺少这项操作 |
| `unity.launchOptions` | 已提供 · 项目启动或版本迁移前查询匹配安装 | 部分可达 · unity launch-options 读取精确匹配；未提供 includeMigrationVersionOptions | 缺少入口 · 工具只提供安装列表或项目启动参数读取，缺少这项操作 |
| `unity.projectLaunchConfig.clear` | 已提供 · 项目启动参数对话框清除覆盖并恢复继承 | 已提供 · unity project-launch-config-clear | 缺少入口 · 工具只提供安装列表或项目启动参数读取，缺少这项操作 |
| `unity.projectLaunchConfig.get` | 已提供 · 项目详情与参数对话框读取覆盖值及有效参数 | 已提供 · unity project-launch-config-get | 已提供 · alcomd_get_project_launch_config；读取覆盖、默认值与生效参数 |
| `unity.projectLaunchConfig.set` | 已提供 · 项目启动参数对话框保存覆盖 | 已提供 · unity project-launch-config-set | 缺少入口 · 工具只提供安装列表或项目启动参数读取，缺少这项操作 |
| `unity.writerState` | 已提供 · 项目详情读取编辑器运行状态 | 已提供 · unity writer-state | 缺少入口 · 工具只提供安装列表或项目启动参数读取，缺少这项操作 |

## 本机交互

| 桌面操作 | GUI | CLI | MCP |
|---|---|---|---|
| 原生文件和文件夹选择器 | 仅本机交互 · 系统对话框接收当前桌面用户选取的路径 | 协议专用 · 在有相应业务命令时用路径参数指定目标 | 协议专用 · 在有相应工具时用路径或目录目标字段指定 |
| 聚焦本机 Unity 窗口 | 已提供 · 从项目操作定位本机已观察到的 Editor 窗口 | 已提供 · desktop focus-project-unity | 已提供 · alcomd_focus_project_unity |

有可用桌面时，启动 Unity Hub、打开目录或聚焦窗口可以由客户端向桌面宿主请求。文件选择器本身依赖当前桌面会话。业务操作接收目标后仍由 Core 校验。扩展界面的打开、事件分派和刷新依赖会话快照，扩展的命令或工具另按扩展声明和授权交付。

## 连接和 MCP 适配方法

| 服务方法 | GUI | CLI | MCP |
|---|---|---|---|
| `extensions.mcpTools.call` | 协议专用 · GUI 的可移植扩展界面走 UI 分派，MCP 工具调用属于工具传输 | 协议专用 · CLI 没有 MCP 工具传输会话 | 协议专用 · tools/call 分派已公布的扩展工具，宿主复核扩展权限和沙箱 |
| `mcp.oauth.begin` | 协议专用 · 发起授权属于 OAuth 客户端与 HTTP 授权端交互 | 协议专用 · CLI 管理授权请求，不发起这个协议交换 | 协议专用 · HTTP 授权端接收 OAuth 请求，调用 oauth_begin |
| `mcp.oauth.finish` | 协议专用 · 系统浏览器参与跳转，GUI 不作为授权协议端点 | 协议专用 · CLI 决定请求，授权端负责完成重定向 | 协议专用 · HTTP 授权端调用 oauth_finish，完成浏览器授权流程 |
| `mcp.oauth.revoke` | 协议专用 · OAuth 撤销端点由外部客户端访问 | 协议专用 · CLI 的登记撤销不是此令牌撤销端点 | 协议专用 · HTTP 令牌撤销请求调用 oauth_revoke |
| `mcp.oauth.token` | 协议专用 · GUI 不承担 OAuth 客户端的令牌交换 | 协议专用 · CLI 授权管理不交换外部客户端令牌 | 协议专用 · HTTP token 端点调用 oauth_token，按协议交付令牌 |
| `mcp.packages.plan` | 协议专用 · 项目包表走原生 packages.plan*，无需 MCP 工具投影 | 协议专用 · package 命令走原生计划方法 | 协议专用 · 安装、重装及通用计划工具将工具意图转成 mcp.packages.plan |
| `mcp.packages.query` | 协议专用 · 项目包表走 queryProjectCandidates，不使用工具分页投影 | 协议专用 · 此适配器服务 MCP 工具；CLI 的项目候选入口见主表 | 协议专用 · 搜索包、读取包或列来源工具调用统一的 MCP 目录查询 |
| `mcp.projects.create` | 协议专用 · 创建对话框走原生 projects.create | 协议专用 · project create 走原生方法 | 协议专用 · alcomd_create_project 将工具目标映射到 Core |
| `mcp.projects.planCopy` | 协议专用 · 项目复制对话框走原生 projects.planCopy | 协议专用 · project copy 走原生方法 | 协议专用 · alcomd_plan_project_copy 将复制目标映射到 Core |
| `mcp.projects.register` | 协议专用 · 项目选择器走原生 projects.register | 协议专用 · project register 走原生方法 | 协议专用 · alcomd_register_project 解析工具中的目录目标 |
| `mcp.repositories.inspect` | 协议专用 · 添加仓库走原生 repositories.inspect | 协议专用 · repository inspect 走原生方法 | 协议专用 · alcomd_inspect_repository 用远程来源取得认证绑定的预览 |
| `mcp.repositories.list` | 协议专用 · 资源页走原生 repositories.list | 协议专用 · repository list 走原生方法 | 协议专用 · alcomd_list_repositories 返回工具侧仓库摘要和续页游标 |
| `mcp.repositories.register` | 协议专用 · 确认登记走原生 repositories.register | 协议专用 · repository register 走原生方法 | 协议专用 · alcomd_register_repository 确认已绑定的远程预览 |
| `mcp.repositories.remove` | 协议专用 · 仓库详情走原生 repositories.unregister | 协议专用 · repository unregister 走原生方法 | 协议专用 · alcomd_remove_repository 明确确认要移除的仓库 |
| `mcp.tools.catalog` | 协议专用 · GUI 查看管理运行快照，工具发现属于 MCP 传输 | 协议专用 · CLI 没有工具目录协商流程 | 协议专用 · MCP tools/list、扩展工具发现和目录变更订阅读取 Core 目录 |
| `mcp.transport.bindRequest` | 协议专用 · 只有收到该连接请求的传输能提供绑定证据 | 协议专用 · CLI 无传输连接证据 | 协议专用 · MCP 传输绑定实际接收的请求与身份 |
| `mcp.transport.report` | 协议专用 · GUI 观察快照，本身不持有 MCP 传输观测证据 | 协议专用 · CLI 不产生该传输观测 | 协议专用 · MCP 传输报告实际连接和请求状态 |
| `system.hello` | 协议专用 · Tauri 客户端连接 Core 时自动协商 | 协议专用 · 命令建立 Core 连接时自动协商 | 协议专用 · MCP 服务建立 Core 连接时自动协商；与 MCP 客户端的 initialize 不同 |

## 对照请求与复验入口

下列测试文件是复验入口。单测或浏览器夹具能观察字段是否省略、按钮能否发出请求，以及界面如何处理重试。核实持久化写入次数、任务终态和撤销还需运行真实 Core 场景。历史运行记录只覆盖记录中列出的候选和用例。

| 方法范围 | 调用约束与可复核证据 |
|---|---|
| 原生方法、CLI 与 MCP 目录 | 对照目标、请求和响应类型，核实命令的实际分派及工具的实际处理器。CLI 的 completion 只生成 shell 补全，MCP 没有任意原生方法透传。[原生合同](../../specs/rpc/native-sdk-v1.methods.json) · [CLI 命令目录](../../specs/cli/command-catalog-v1.json) · [CLI 分派](../../apps/alcomd-cli/src/main.rs) · [MCP 工具目录与处理](../../apps/alcomd-mcp/src/tools.rs) |
| GUI 页面和桌面适配 | 从路由、控件或自动订阅定位业务请求，并沿 TypeScript 客户端追到 Tauri。仅有客户端方法定义、能力标识、通知名称或测试桩不能证明用户可发起请求。[主页面](../../apps/alcomd-gui/src/CorePages.tsx) · [操作对话框](../../apps/alcomd-gui/src/CoreActions.tsx) · [客户端](../../apps/alcomd-gui/src/rpc.ts) · [Tauri 分派](../../apps/alcomd-gui/src-tauri/src/lib.rs) |
| 项目、计划与任务 | 核对项目修订、路径、planId、确认、幂等键、任务归属和终态。CLI 交互读取既有计划再提交，指定 --yes 时按参数情况跳过展示。GUI 与 MCP 的 Apply 都需回到已审阅计划。任务与普通项目清单的游标不可混用。[CLI 分派与确认](../../apps/alcomd-cli/src/main.rs) · [GUI 项目流程](../../apps/alcomd-gui/src/CorePages.tsx) · [MCP 项目工作流](../../apps/alcomd-mcp/src/tools/workflows.rs) · [MCP 项目工具测试](../../apps/alcomd-mcp/tests/support/project_workflows_flow.rs) · [GUI 任务回归](../../apps/alcomd-gui/tests/browser/m7-operation-follow.spec.ts) |
| 软件包 | 单包及批量默认安装、升级时省略版本；显式范围、来源、精确版本及 Unity 例外逐项对照。Apply 复核计划原项目、修订、可见候选及调用者。[GUI 请求构造](../../apps/alcomd-gui/src/CoreActions.tsx) · [GUI 版本与来源选择](../../apps/alcomd-gui/tests/browser/m7-package-candidates.spec.ts) · [MCP 包投影](../../apps/alcomd-mcp/src/tools/packages.rs) · [MCP 包测试](../../apps/alcomd-mcp/tests/support/package_catalog_flow.rs) · [真实包 RPC](../../crates/alcomd-testing/tests/m4_package_rpc.rs) |
| 仓库、设置与同步 | 仓库清单保持自身分页、排序与完整收录范围。更新设置时对照部分字段、显式 false、预期修订和关闭预发布可见性后的偏好复位。仓库登记重试绑定调用者、原来源和原键。[GUI 仓库文件导入导出](../../apps/alcomd-gui/src/RepositoryListDialog.tsx) · [MCP 仓库适配](../../apps/alcomd-mcp/src/tools/repositories.rs) · [跨客户端仓库 RPC](../../crates/alcomd-testing/tests/m7_repository_list_rpc.rs) · [GUI 设置回归](../../apps/alcomd-gui/tests/browser/m7-settings-autosave.spec.ts) · [CLI 真实 IPC](../../apps/alcomd-cli/tests/m1_cli.rs) |
| 模板、备份与 Unity | 对照模板 ID、ZIP 目标、明确覆盖的选项、启动参数及原修订。GUI 文件选择和 CLI/MCP 路径输入最终指向同一个 Core 目标。[GUI 创建与恢复](../../apps/alcomd-gui/src/ProjectCreationDialogs.tsx) · [GUI 模板工作区](../../apps/alcomd-gui/src/ProjectTemplatesWorkspace.tsx) · [备份用例](../../apps/alcomd-gui/tests/browser/m7-project-backup-defaults.spec.ts) · [MCP 模板测试](../../apps/alcomd-mcp/tests/support/project_assets_flow.rs) · [真实 Unity RPC](../../crates/alcomd-testing/tests/m5_unity_rpc.rs) |
| 日志、事件与 GUI 扩展 | 日志、资源变化和调用者任务分别观察。检查订阅续接、会话 ID、快照修订、事件序号和显式授权。适配器未接入的按钮不能通过调用测试桩代替。[GUI 事件消费](../../apps/alcomd-gui/src/core-events.ts) · [日志页面](../../apps/alcomd-gui/src/OperationLogsRoute.tsx) · [GUI 便携界面分派](../../apps/alcomd-gui/src/App.tsx) · [会话交互测试](../../apps/alcomd-gui/tests/browser/m7-h4-portable.spec.ts) |
| 连接与管理服务 | 第三方扩展的 WIT 调用和原生 RPC 可以到达同一管理用例。检查真实客户端、凭据代次、授权请求归属、一次性展示、撤销和扩展沙箱。OAuth 授权端的输入和状态与原生管理请求分开验证。[第三方管理接口测试](../../crates/alcomd-testing/fixtures/m8/management-guest-source/src/lib.rs) · [CLI 授权管理](../../apps/alcomd-cli/src/m8_oauth.rs) · [HTTP 端点](../../apps/alcomd-mcp/src/http/oauth.rs) · [传输观测](../../apps/alcomd-mcp/src/observations.rs) · [OAuth 真实工具测试](../../apps/alcomd-mcp/tests/support/oauth_flow.rs) · [扩展工具集成测试](../../apps/alcomd-mcp/tests/support/extension_tools_flow.rs) |
