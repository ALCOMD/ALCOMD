# ALCOMD3 v3.4.0 细粒度功能审计

状态：冻结源码静态审计完成；真实安装快照、视觉流程、恶意输入和跨平台实机验证待执行

审计提交：`4aa98ae4f18d42c10137278997180dbede991e88`

最后更新：2026-08-16

## 审计边界

本文件记录 v3.4.0 用户入口、持久状态、隐性行为和验收证据。v3 只用于功能审计、迁移格式
验证和兼容性测试；本文不授权复制、移植或改写其源码。所有生产目标均由 v4 独立实现。

## 项目

| Feature ID | 用户行为与边界 | 主要证据 |
|---|---|---|
| `projects.list` | `/projects` 列表/网格、名称搜索、多字段正反排序、收藏置顶、后台元数据刷新；视图写 GUI config，项目与收藏写数据库 | `vrc-get-gui/app/_main/projects/-projects-list-card.tsx`；`src/commands/environment/projects.rs:environment_projects`；`src/config.rs:GuiConfig` |
| `projects.register` | GUI 多目录选择和 MCP `add_existing_project`；逐个识别 Unity 项目，拒绝无效/重复，MCP 要求绝对路径 | `src/commands/environment/projects.rs:environment_add_existing_projects`；`src/mcp/mod.rs` |
| `projects.create` | 选择模板、Unity、名称、父目录；复核模板快照，完成模板/依赖后注册；持久化上次模板与最多 8 个最近位置 | `app/_main/projects/-create-project.tsx`；`src/commands/environment/projects.rs:environment_create_project` |
| `projects.copy` | 同级或指定位置复制，目标不能在源内且必须不存在；跳过 symlink、`.git`、根 Logs/Obj/Temp；失败/取消清理目标 | `src/commands/environment/projects.rs:environment_copy_project`、`environment_copy_project_for_migration`；`src/utils.rs` |
| `projects.remove` | “取消注册”与“目录移到系统回收站”两个行为；回收站成功后才删除数据库记录 | `src/commands/environment/projects.rs:environment_remove_project_by_path` |
| `projects.favorite` | 切换收藏并影响所有排序的置顶 | `app/_main/projects/-project-row.tsx`；`src/commands/environment/projects.rs` |
| `projects.migrate-vpm` | 旧 SDK 到 VPM，支持先备份、复制或直接迁移；Unity 正在运行时拒绝 | `app/_main/projects/-project-row.tsx`；`src/commands/project.rs:project_migrate_project_to_vpm` |
| `projects.migrate-unity2022` | 2019→2022，推荐 Hub 版本，支持备份/复制，包迁移后批处理调用 Unity 并流式检查输出/退出码 | `app/_main/projects/manage/-unity-migration.tsx`；`src/commands/project.rs` |

列表查询在 v3 中会迁移/清理项目并同步数据库/settings；v4 Query 不得隐式写项目或权威状态，
修复和迁移必须形成显式 Plan/Apply。

## 包、仓库和本地资源

| Feature ID | 用户行为与边界 | 主要证据 |
|---|---|---|
| `packages.catalog` | 合并官方、精选、用户仓库与本地包；考虑来源、隐藏、prerelease、yanked、Unity compatibility；已安装不可见版本仍展示 | `app/_main/projects/manage/-package-list-card.tsx`；`vrc-get-vpm/src/environment/settings.rs`；`src/mcp/mod.rs` |
| `packages.plan-apply` | 安装、卸载、重装、resolve、升级和降级先生成 Pending Changes，展示依赖原因、冲突和 legacy 删除，再确认写入 | `app/_main/projects/manage/-use-package-change.tsx`；`src/commands/project.rs`；`src/state/project_apply.rs` |
| `packages.bulk` | 多选、范围选择、升级稳定版/全部、重装全部，合并为统一计划 | `app/_main/projects/manage/-package-list-card.tsx` |
| `packages.progress` | 等待、下载、解压、安装、移除、失败，可最小化、取消和重试 | `app/_main/projects/manage/-use-package-change.tsx`；`src/commands/project.rs` |
| `repositories.manage` | 官方/精选/用户仓库列表，刷新、显隐、自定义名、用户仓库排序和删除；同步 ID、清重复 | `src/commands/environment/packages.rs`；`vrc-get-vpm/src/environment/settings.rs:remove_id_duplication` |
| `repositories.add` | URL+headers 下载预览后确认；拒绝无效、网络失败、重复 URL/ID 和默认仓库冲突 | `src/commands/environment/packages.rs`；`vrc-get-vpm/src/environment/settings.rs:can_add_remote_repo` |
| `repositories.import-export` | UTF-8 每行 URL、`#` 注释、http(s) 与 `vcc://vpm/addRepo`、headers；导入预览/部分成功，导出用户仓库 | `docs/repository-list-file-format/`；`src/commands/environment/packages.rs`；`vrc-get-vpm/src/environment/settings.rs:export_repositories` |
| `repositories.deeplink` | 用户选择注册 `vcc://vpm/addRepo` handler；深链仍需确认 | `src/deep_link_support.rs`；`src/commands/environment/settings.rs:environment_set_use_alcom_for_vcc_protocol` |
| `packages.local` | 多目录添加/取消注册本地 loose package，可整体隐藏；绝对路径、有效 manifest、不重复，移除不删源目录 | `src/commands/environment/packages.rs`；`vrc-get-vpm/src/environment/settings.rs:add_user_package` |
| `packages.cache` | 仓库/包 cache、ZIP SHA-256 sidecar、读取校验、旧 cache 位置兼容和清除 | `vrc-get-vpm/src/environment/package_installer.rs` |

v4 的 GUI、CLI、MCP、API 必须共享 catalog 与 Plan/Apply 策略。仓库 header 属于凭据，不能按
v3 行为明文进入普通设置、日志或导出。

## Unity

| Feature ID | 用户行为与边界 | 主要证据 |
|---|---|---|
| `unity.discovery` | 自动/手工发现 Hub 与 Editor，从 Hub 配置或调用 Hub，同步版本、路径、架构和推荐版本；并发刷新共享一次任务 | `src/commands/environment/unity_hub.rs`；`src/commands/environment/settings.rs:environment_unity_versions`、`environment_pick_unity` |
| `unity.launch` | Closed/Opening/Open，结合 UnityLockFile 与进程识别，防重复启动并更新项目时间；Unix 检查 noexec | `src/commands/project.rs:project_open_unity`；`src/unity_process.rs` |
| `unity.arguments` | 项目级参数覆盖全局默认参数 | `src/commands.rs:DEFAULT_UNITY_ARGUMENTS`；settings/project commands |
| `unity.foreground` | 支持的平台定位主 Unity 窗口并置前，不支持平台明确失败 | `src/commands/project.rs` |

## 模板

| Feature ID | 用户行为与边界 | 主要证据 |
|---|---|---|
| `templates.builtin` | Avatars/Worlds/Blank，覆盖三种 Unity 版本，处理包、换行、iOS、productName 与随机 productGuid；不可编辑删除 | `src/templates.rs`；`project-templates/list.txt`、`README.md` |
| `templates.derived` | 展示名、基础模板、Unity 范围、VPM 依赖、UnityPackage；校验包/版本/绝对普通文件并防依赖环 | `src/backend/templates.rs` |
| `templates.archive` | 从项目导出 tar.gz payload，创建时安全展开、导入 UnityPackage、替换名称/GUID并解析 VPM 依赖 | `src/backend/templates.rs`；`src/templates/alcom_template.rs` |
| `templates.import-export` | `.alcomtemplate` v1/v2、多文件、缺 ID 分配、重复覆盖确认、不能覆盖内置，逐文件部分成功 | `src/backend/templates.rs`；`src/templates/alcom_template.rs` |
| `templates.favorite` | 收藏和上次使用模板写 GUI config | `src/config.rs`；创建项目/模板页面 |

## 备份与恢复

| Feature ID | 用户行为与边界 | 主要证据 |
|---|---|---|
| `backups.create` | ZIP、默认时间名、store/fast/maximum、可排除 locked VPM 包；跳过 symlink/.git/Logs/Obj/Temp/Library（保留特定文件）；create_new，不覆盖，失败删半成品；全局单任务 | `src/backend/project_archive.rs`；`src/utils.rs`；`src/commands/project.rs`；`src/state/project_backup.rs` |
| `backups.restore` | 只能恢复到默认项目根下的新子目录；安全解压、验证 Unity 项目后注册；失败/取消清目标；单 restore task | `src/commands/environment/projects.rs:environment_restore_project_from_backup` |

真实恶意 ZIP 必须验证 symlink metadata、混合分隔符、重复 entry、膨胀率和资源配额；静态审计
不能据底层库行为确认这些边界。

## GUI、设置与旧数据导入

| Feature ID | 用户行为与边界 | 主要证据 |
|---|---|---|
| `gui.setup` | 外观→旧数据→Hub→项目路径→备份→条件系统设置→完成；每页进度持久化 | `app/_setup/setup/*` |
| `gui.settings` | Hub/Editor、项目/备份路径、备份格式、prerelease、Unity args、动画/紧凑、频道/自动更新、VCC handler、系统信息 | `app/_main/settings/index.tsx`；`components/common-setting-parts.tsx`；`src/commands/environment/settings.rs` |
| `gui.i18n` | de/en/fr/ja/ko/zh-Hans/zh-Hant，即时切换、系统 locale 默认和持久化 | `lib/i18n.ts`；`locales/*.json5`；`src/config.rs` |
| `gui.theme` | Material Theme source/HCT、auto/light/dark 与 9 种 scheme，默认 `#6cb6ff`/auto/vibrant | `components/MaterialThemePanel.tsx`；`lib/material-theme.ts`；`src/config.rs:ThemeConfig` |
| `gui.config-recovery` | GUI/theme/repository/template/log 状态；坏 GUI config 重命名 `.bak.N` 后默认；pretty JSON 原子写，窗口状态持久化 | `src/storage.rs`；`src/state/config.rs`；`src/config.rs` |
| `gui.extensions-v3` | MCP/Theme/Logs/Discord 四个内置扩展可启停、不可安装卸载；管理侧边栏；不同扩展具有生命周期副作用 | `src/extensions.rs`；`src/main.rs`；`lib/sidebar-extensions.ts` |
| `gui.update` | stable/beta、自动/手工检查、下载暂存安装和稍后提醒；频道切换清理不同频道 staging | `src/commands/environment/settings.rs`；`src/updater.rs`；`src/storage.rs` |
| `migration.legacy-import` | VCC/ALCOM/ALCOMD3 Beta 的 Projects/Resources/Theme/Settings 分类导入、路径限定、cache 失效与选择性键迁移 | `src/commands/environment/legacy_import.rs` |

v3 实际没有可安装的第三方扩展运行时；registry/测试结构不能被记作已交付功能。v4 Theme、MCP、
Logs、Discord 第一方扩展必须使用与第三方相同的公开 API、权限、宿主和沙箱。

## 活动与技术日志

| Feature ID | 用户行为与边界 | 主要证据 |
|---|---|---|
| `activity.structured` | Source/Kind/Status/Importance，开始/结束合并，每日 JSONL、30 文件、内存 1000、尾部读取、坏行跳过，URL/token 脱敏 | `src/activity_log.rs` |
| `logs.technical` | 实时/文件日志、级别筛选、摘要/详情、最多 30 文件、尾部读取、token/secret/Auth/API key/URL 脱敏 | `src/logging.rs`；`app/_main/log/index.tsx`；`src/config.rs` |

v3 为诊断保留完整路径；v4 日志规则禁止完整私密路径，需使用摘要或受控展示，不能机械对齐。

## MCP

| Feature ID | 用户行为与边界 | 主要证据 |
|---|---|---|
| `mcp.lifecycle-v3` | 嵌入 GUI、扩展默认开而 access 默认关；关闭扩展停止 endpoint/任务；管理页展示 endpoint、客户端、工具活动 | `src/mcp/lifecycle.rs`；`src/mcp/mod.rs`；`app/_main/mcp/index.tsx` |
| `mcp.auth-v3` | 固定 loopback 端口、Bearer 常量时间比较、Host/Origin、并发/速率限制；token 明文 GUI config/环境变量 | `src/mcp/types.rs`；`src/mcp/http.rs`；`src/mcp/tools.rs`；`src/config.rs` |
| `mcp.tools-v3` | 33 个项目、模板、备份、仓库、包、设置、活动和技术日志工具 | `docs/mcp/tools.zh-CN.md`；`src/mcp/tools.rs`；`src/mcp/mod.rs`；`src/backend/mcp_capabilities.rs` |
| `mcp.tasks-v3` | 7 个长任务工具，working/input_required/completed/failed/cancelled，get/update/cancel，TTL/轮询；实验映射 | `src/mcp/tools.rs`；`src/mcp/mod.rs`；MCP 文档 |
| `mcp.client-config-v3` | Windows Codex、Claude Code、Cursor 快速配置，合并精确条目并提示覆盖，写用户环境变量 token | MCP client configuration 模块；`docs/mcp/mcp.zh-CN.md` |
| `mcp.observability-v3` | 最近客户端最多 20/TTL 10 分钟、事件节流、工具/client/request 活动与脱敏 | `src/mcp/mod.rs` |

冻结的 33 个工具：

```text
list_projects
list_templates
get_template
create_template
edit_template
set_template_package
remove_template_package
set_template_unitypackage
remove_template_unitypackage
remove_template
get_project_details
create_project
add_existing_project
backup_project
copy_project
restore_project_from_backup
list_repositories
add_repository
remove_repository
list_packages
list_repository_packages
get_package_details
get_environment_settings
search_activity_logs
get_activity_log_entry
summarize_activity_logs
get_activity_log_context
search_technical_logs
get_technical_log_entry
summarize_technical_logs
install_project_package
uninstall_project_package
reinstall_project_package
```

v4 保留能力而改变架构：独立 `alcomd-mcp`、无 GUI 依赖、每客户端 Principal、新协议版本、
OS credential、公开 Operation。v3 Tasks 不能绕过 A-021 直接进入 4.0.0 公共协议。

## Discord

| Feature ID | 用户行为与边界 | 主要证据 |
|---|---|---|
| `discord.presence-v3` | 总开关、项目名/Unity/编辑器数量、自定义文本；每 5 秒选择前台或最近 Unity；Discord 后出现自动重连；禁用/关闭分享清除；Unicode 128 字符 | `src/discord_presence.rs`；`src/commands/discord_presence.rs`；`app/_main/discord/index.tsx` |

v4 完整能力属于第一方扩展；不保存 Discord token/用户数据库，禁用、卸载或撤销时立即清除。

## 持久状态证据

- `config/gui-config.json`：语言、备份、排序、频道、更新、VCC handler、Unity 参数、日志、MCP、
  Discord、项目视图、模板、扩展、侧边栏和窗口。
- `config/theme-config.json`：Material Theme。
- `config/repositories.json`：已下载仓库状态。
- `settings.json` 与备份：项目/备份根、Hub、本地包、用户仓库和 prerelease。
- `vcc.liteDb`：项目、Unity 安装、收藏和项目参数。
- `templates/`、`activity-logs/`、`technical-logs/`、包/仓库 cache、updater staging。

主要证据：`vrc-get-gui/src/storage.rs`、`src/config.rs`、
`vrc-get-vpm/src/environment/settings.rs`。

## 必须保持待验证

- 完整 GUI 截图、键盘/可访问性、错误与进度视觉流程。
- 恶意 ZIP symlink、压缩炸弹和跨平台路径差异。
- Unity 置前、Hub 调用、协议注册和 updater 的 Windows/macOS/Linux 实机行为。
- Discord 客户端真实兼容性、非 UTF-8 路径全流程和真实安装/卸载残留。
- v3 MCP Tasks 是旧实验映射，不是 2026-07-28 Tasks 扩展的证据。
