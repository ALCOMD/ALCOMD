# State18：备份压缩等级

状态：项目所有者于 2026-11-03 根据 NanaZip 截图确定六档压缩，并批准对应 Config9 / State18 最小合同。

`backups.compression_mode` 原 CHECK 仅接受 `store`、`fast`、`maximum`，分别保留为 ZIP Stored / Deflate 1 / Deflate 9。
本次只扩展该 CHECK，增加 `level3`、`level5`、`level7`；不增加别名、列、公开方法或权限。
全部旧备份记录、归档定位、ID、接受任务和恢复计划保持原含义。其他表不重建、不改写。

## 事务与外键

SQLite 不能在活动事务中切换 `foreign_keys`。`sqlite.rs` 的专用迁移入口先验证连接处于 autocommit 且外键已开启，
在事务外关闭并核实外键，然后开启 `IMMEDIATE` 事务。它在同一事务中读取 `backups` 原有显式索引和触发器定义，
执行 [0018 迁移主体](../../crates/alcomd-store/migrations/0018_backup_compression_levels.sql)，随后原样重建这些索引/触发器。

新表保留原 STRICT、主键、唯一性、所有列及除压缩枚举外的 CHECK；通过显式列清单同时复制 rowid 和全部原值，
不补造或转换已有数据。旧表移除后，新表改回原名 `backups`，不先重命名旧表，避免改写引用方。
`backup_restore_plans` 及其 `REFERENCES backups(backup_id)`、不可变触发器、恢复 journal 均保持原定义和原行。

`PRAGMA user_version=18` 与表替换属于同一事务。提交前实际读取完整 `foreign_key_check`，任何外键问题都拒绝提交。
SQL、对象恢复、外键检查或提交失败时，由事务对象回滚；所有退出路径都在事务结束后恢复并核实 `foreign_keys=ON`。
不把 `execute_batch` 忽略 PRAGMA 结果当成验证，不留下关闭外键的可用连接。

迁移 SQL 有意不自行 BEGIN/COMMIT 或切换外键，必须通过该入口执行。正常连接继续使用 WAL、FULL 和现有 busy timeout。
后续打开已为 State18 的数据库不重复迁移；更高版本仍在写入前拒绝。

## 验证范围

合成 fixture 包含三档旧备份、真实关联恢复计划、Operation、恢复 journal、Event、幂等记录及有间隔 rowid。
比较迁移前后全部表行、sqlite_sequence、表定义、索引和触发器，仅允许压缩 CHECK 与 user_version 的明确变化。
保留并核验恢复外键及 STRICT，新增六档经公开 Store 读取/重开验证；非法别名和非六档值继续拒绝。

故障注入在换表和版本赋值后失败，要求原 Schema、原数据、原版本一起回滚且外键重新开启。
另以预先损坏的恢复外键证明 `foreign_key_check` 会阻止迁移，并保留原损坏证据供诊断，不删除或修补数据。
只操作可丢弃隔离数据，不备份、恢复或清理用户真实工程。
