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
