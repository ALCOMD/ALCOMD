# State Schema v6：Backup Create Operation kind

状态：M5 Backup Create contract-first 已冻结；Backup Create worker 尚未实现。

权威 migration 是 `crates/alcomd-store/migrations/0006_backup_create.sql`。daemon 从 v5 原子迁移到 v6，
完成 store 初始化后才通过 `system.hello` 广告 `dataSchema: 6`。

## 唯一 Schema 变化

v6 只向 `operations.kind` 的严格枚举增加 `backups.create`。SQLite CHECK constraint 要求原子重建
`operations`，migration 同时重建并完整复制所有直接依赖：`operation_journal`、
`idempotency_records`、`package_plans`、`package_filesystem_journal`、`template_plans`。其他表、row、
revision、Event sequence 与外键不变。

Schema v4 已有 `backups` 表继续是成功归档 metadata authority；v6 不增加 backup plan、restore、通用
workflow、settings 或 filesystem journal 表。Operation kind 允许持久化不等于 capability、dispatcher、
worker 或 CLI 已发布。

## 迁移与恢复不变量

- migration 从 `BEGIN IMMEDIATE` 到最后写 `PRAGMA user_version = 6` 的 `COMMIT` 是单一事务；失败回滚
  后保持完整 v5。
- v1 必须按 0001→0002→0003→0004→0005→0006 升级；v5 可直接执行 0006。
- 高于 v6 的 future schema 继续 fail closed。
- generic state-check recovery 只处理 `state.check`；package、template 与 backup handler 各自只认其
  Operation kind，unknown future kind 不能被错误接管。
- `operations.kind` 继续是严格枚举，不为未来 kind 放宽为任意 string。

## M7 默认备份目录与保留数据

Config5 默认保存目录增量不要求 State 迁移；当前数据库为 State17（独立的 Unity 显式空参数覆盖迁移）。
本节只补充已存在 JSON/locator 字段的内部语义，
不修改 0006、表、列或 public `backups.create` 参数。新接受的备份请求在 `request_json` 和 accepted checkpoint 中
固定可选 `destination = { directory, identity }`：有界绝对 canonical Unicode 目录及 opaque filesystem identity。
新任务读取同一 application 设置权威一次；执行、取消清理和重启恢复只使用其冻结目录并重新验证 identity。
目录必须位于源项目之外。缺少 `destination` 仅表示保留的旧任务，继续使用原 `data_root/backups`。

新归档直接保存为冻结目录中的 `<BackupId>.zip`，同目录 `.alcomd-<OperationId>.zip.part` 暂存，
使用平台无覆盖发布并保留原哈希、取消及恢复门禁。成功记录的内部 locator 为
`backup-file-v1:<canonical absolute archive path>`，文件名必须匹配 BackupId；既有
`backup-v1:<BackupId>` 继续定位原 `data_root/backups/objects/<BackupId>.zip`，不搬迁旧文件。
locator 不进入普通 RPC/Activity 返回，也不替代记录中的文件 identity、SHA-256、大小或格式校验。

`backups.create` 幂等 fingerprint 仍只包含原公开意图；读取旧幂等结果先于查询当前目录设置，
再次提交或设置变更不能改写原任务目的地。排队取消没有创建归档时直接终态；已有 partial 的取消只清理原冻结位置。
进入 publish intent 后恢复仍完成原事务，不因后续目录设置或迟到取消而重定向或丢弃已发布归档。
