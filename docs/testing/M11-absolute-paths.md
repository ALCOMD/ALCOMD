# M11 · 绝对路径入口对照

Core 以运行所在机器的文件系统解释绝对路径。MCP 使用原有身份、业务权限和撤销机制。复制的 Apply 使用 Plan 冻结的父目录身份和目标名，并保留人工确认。进程重启后可继续使用未过期的 Plan。备份目录和项目目录用作默认位置，调用者可明确指定其他位置。

GUI 和 CLI 使用原生 RPC，Rust 与 Node SDK 对应同一套公开方法。下表中「无此命令」和「无此工具」表示该端没有这项现行业务入口。

| 路径输入 | GUI → Core | CLI → Core | MCP → Core | 自动化入口 |
|---|---|---|---|---|
| Unity 工程目录 | 检查、登记 → `projects.inspect`、`projects.register` | `project inspect <path>`、`project register <path>` → 同左 | `alcomd_register_project.source.path` → `projects.register` | [MCP 项目路径 IPC](../../crates/alcomd-testing/tests/m11_project_paths_rpc.rs) |
| 新建项目目录 | 选择父目录并填写项目名 → `projects.create` | `project create [target-parent] <name>` → 同左 | `alcomd_create_project.target.parent_path` → `projects.create` | [创建及模板 IPC](../../apps/alcomd/src/project_create_ipc_tests.rs)，[MCP 项目路径 IPC](../../crates/alcomd-testing/tests/m11_project_paths_rpc.rs) |
| 复制项目目录 | 选择父目录并填写项目名 → `projects.planCopy`、`projects.applyCopy` | `project copy <project-id> [target-parent] <target-leaf>` → 同左 | `alcomd_plan_project_copy.target.parent_path` → `projects.planCopy`，确认 → `projects.applyCopy` | [原生复制 IPC](../../crates/alcomd-testing/tests/m7_project_copy_rpc.rs)，[MCP 项目路径 IPC](../../crates/alcomd-testing/tests/m11_project_paths_rpc.rs) |
| 备份 ZIP 的保存位置 | 文件选择 → `backups.create.destinationPath` | `backup create --destination` → 同左 | `alcomd_create_backup.destination_path` → `backups.create` | [备份 IPC](../../crates/alcomd-testing/tests/m5_backup_rpc.rs) |
| 恢复 ZIP 与新项目目录 | 文件选择及可选完整目标目录 → `backups.restore.archivePath`、`destinationPath` | `backup restore <archive> --destination` → 同左 | `alcomd_restore_backup.archive_path`、`destination_path` → `backups.restore` | [备份 IPC](../../crates/alcomd-testing/tests/m5_backup_rpc.rs) |
| 仓库清单文件 | 导入、导出选择器 → `repositories.inspectListFile`、`repositories.importListFile`、`repositories.exportListFile` | 无此命令 | 无此工具。仓库登记接受仓库 URL | [仓库清单 IPC](../../crates/alcomd-testing/tests/m7_repository_list_rpc.rs)，[仓库清单浏览器回归](../../apps/alcomd-gui/tests/browser/m7-repository-list.spec.ts) |
| 模板导入、附件源与导出文件 | 文件选择 → `projectTemplates.save`、`projectTemplates.edit`、`projectTemplates.export` | `template save --source`、`template edit --changes` 从本机 JSON 读取文件路径，`template export <id> <destination>` 传递完整目标 | `alcomd_create_project_template.source.source_file`、`alcomd_edit_project_template` 的附件 `source_file`、`alcomd_export_project_template.destination_file` → 同左 | [模板 IPC](../../apps/alcomd/src/project_create_ipc_tests.rs)，[模板浏览器回归](../../apps/alcomd-gui/tests/browser/m7-template-lifecycle.spec.ts) |
| 本地包目录 | 登记本地包 → `packages.userPackages.enroll.sourcePath` | 无此命令 | 安装已登记包时使用资源 ID | [本地包校验](../../crates/alcomd-vpm/src/user_package.rs)，[资源浏览器回归](../../apps/alcomd-gui/tests/browser/m7-resources-lifecycle.spec.ts) |
| Unity 编辑器可执行文件 | 手动登记 → `unity.installations.register.executablePath` | `unity register <executable-path>` → 同左 | 现有工具读取 Unity 登记 | [Unity IPC](../../crates/alcomd-testing/tests/m5_unity_rpc.rs) |
| Unity Hub 路径、项目与备份默认目录 | 设置 → `settings.update` 的 `unity.hubPath`、`paths.projectDirectory`、`paths.backupDirectory` | 无此命令 | 现有工具读取允许公开的设置 | [设置 IPC](../../crates/alcomd-testing/tests/m7_official_rpc.rs)，[目录浏览器回归](../../apps/alcomd-gui/tests/browser/m7-storage-directories.spec.ts) |

MCP 项目路径 IPC 用例包含配置目录外的空格和 Unicode 路径、相对路径与父目录遍历拒绝、已有目标拒绝、目标目录换向与身份变化、普通结果脱敏、跨进程 Apply、同键回放和撤销复验。

[MCP SDK HTTP 回归](../../apps/alcomd-mcp/tests/support/project_workflows_flow.rs)使用配置目录外的路径登记项目、写入 ZIP 和恢复项目，并验证回包丢失后的同键回执。[MCP OAuth 回归](../../apps/alcomd-mcp/tests/support/oauth_flow.rs)覆盖同意后的备份、同键重试及撤销。两项均使用隔离 Core。
