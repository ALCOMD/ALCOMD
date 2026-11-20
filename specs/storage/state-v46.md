# State46 首次发布基线

Core 在新建环境中以单个 SQLite 事务执行 [初始 DDL](../../crates/alcomd-store/src/schema.sql) 并写入 `user_version=46`。结构包含当前项目、Unity 安装、仓库、包、模板、扩展与 MCP 状态，以及任务、幂等、事件和文件系统事务证据。普通 ZIP 任务使用任务与恢复日志，归档文件由 Core 按请求位置读写。

重开时先检查主文件标识、版本和结构。SQLite 在上述检查通过后进入正常读写和 WAL 恢复。存在未知数据库、无主文件的伴随文件或不受信任目录时拒绝。当前数据库与 Config13 同时预检。空环境初始化时只接受已知且经过验证的安装登记或当前设置。

当前合同验证结构、初始化事务、重开以及旧或未知输入的无写入拒绝。此前版本的存储规范为开发记录。[结构合同测试](../../crates/alcomd-store/tests/schema_contract.rs) · [M11 实施计划](../../docs/exec-plans/M11-v3-migration-and-zip.md)
