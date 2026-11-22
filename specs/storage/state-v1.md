# State1 首次发布基线

桌面动作使用现有任务及幂等回执表。任务类型为 `desktop.focusProjectUnity`、`desktop.openProjectDirectory` 和 `desktop.openUnityHubDownload`。Core 在同一事务中复核调用者、登记运行中的任务及 pending 回执，然后请求本机桌面。成功及失败均持久化。Core 重启时把仍在等待桌面结果的任务终结为 `desktop_result_unknown`，相同幂等键只读取结果。系统动作受理后无法取消。

Core 在新建环境中以单个 SQLite 事务执行 [初始 DDL](../../crates/alcomd-store/src/schema.sql) 并写入 `user_version=1`。结构包含当前项目、Unity 安装、仓库、包、模板、扩展与 MCP 状态，以及任务、幂等、事件和文件系统事务证据。普通 ZIP 任务使用任务与恢复日志，归档文件由 Core 按请求位置读写。

Unity 迁移计划在 `project_unity_migration_plans.target_sdk_version` 可空 TEXT 列保存已选定的 SDK；不需要更换 SDK 的计划存 NULL。Core 重启后，读取计划与 Apply 复验都使用该列保存的版本。

重开时先检查主文件标识、版本和结构。Core 对照初始 DDL 核实表、索引和触发器，比较时忽略 SQL 空白、注释和未加引号的名称及关键字大小写，保留约束、运算符和字符串值。SQLite 在上述检查通过后进入正常读写和 WAL 恢复。存在未知数据库、无主文件的伴随文件或不受信任目录时拒绝。当前数据库与 Config1 同时预检。空环境初始化时只接受已知且经过验证的安装登记或当前设置。

当前合同验证结构、初始化事务、重开以及不符合现行结构的输入在写入前被拒绝。[结构合同测试](../../crates/alcomd-store/tests/schema_contract.rs) · [M11 实施计划](../../docs/exec-plans/M11-v3-migration-and-zip.md)
