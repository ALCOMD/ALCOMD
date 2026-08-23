# ALCOMD State Schema v7：Backup Restore immutable authority

状态：M5 Backup Restore contract-first 已冻结；Restore production dispatcher/worker 尚未实现。

权威 migration 是 `crates/alcomd-store/migrations/0007_backup_restore.sql`。0001-0006 保持 byte-for-byte
不变。daemon 完整迁移并初始化 store 后通过 `system.hello` 广告 `dataSchema: 7`；这不等同于广告
`backups.restore.v1`。

## 唯一 Schema 变化

v7 只增加 Restore 实际需要的三项状态：

1. `operations.kind` 严格增加 `backups.restore`；
2. `backup_restore_plans` 保存永久、immutable 的 Restore Plan authority；
3. `backup_restore_filesystem_journal` 保存 Restore 专用 append-only publish/recovery evidence。

不修改或泛化 `package_plans`、`template_plans`、`package_filesystem_journal`；不建立 generic plans、workflow
DSL、history、TTL/cleanup worker、restore counter 或 Backup metadata 历史字段。

## backup_restore_plans

每行保存：

- PlanId、owner Principal、`unapplied | applied` state；
- BackupId 与预分配且唯一的 ProjectId；
- archive SHA-256、文件身份、byte size、`formatVersion=1` 与 `backup.json` fingerprint；
- `excludeVpmPackages` 与 bounded `excluded_packages_json`；
- absolute target parent path、parent filesystem identity、normalized target leaf 与
  `target_must_be_absent=1`；
- bounded expected Unity Project summary、32-byte Plan fingerprint 与严格 version 1 `plan_json`；
- 唯一可空 Apply OperationId 与创建时间。

Plan 不 TTL、不自动删除。唯一允许的 update 是：

```text
unapplied + NULL apply_operation_id
    -> applied + backups.restore OperationId
```

trigger 验证所有其他字段 byte-equivalent、Operation kind 精确匹配；重复 Apply、回退、改字段和删除都失败。

## Restore filesystem journal

Template create-project 的 persisted checkpoint 明确使用 `operation_journal.kind='templates.create-project'`，
因此 v7 不改变其语义。Restore 使用窄表，记录 OperationId/step/PlanId/预分配 ProjectId、固定 phase、
intent/completed state、parent/target identity、project fingerprint、有界 evidence 与时间。插入必须同时匹配
已绑定 Plan、Operation kind 和预分配 ProjectId；row append-only 且不可删除。

固定 phase：

```text
accepted
archive_verified
extracting
staging_complete
publish_intent
target_published
project_registry_commit_intent
state_committed
```

publish 前 staging 可由 recovery 丢弃并重新 extract；`target_published` 后只能在 target identity、Project
fingerprint、Plan evidence 与 ProjectId 一致时 forward-finalize。外部修改进入
`backup_restore_recovery_required`，不能删除/覆盖 target 或虚假 succeeded。

## Migration 不变量

- migration 是 `BEGIN IMMEDIATE` 单事务，最后设置 `user_version=7`；失败完整回滚到 v6。
- v1 按 0001→...→0007 升级；v6 可直接执行 0007。
- 重建 operations 时完整保留 operation journal、idempotency、package Plan/journal、template Plan、
  Backup Create Operations、backups rows、Project/Repository/Unity state、revision 与 Event sequence。
- `PRAGMA foreign_key_check` 必须为空；future schema 继续 fail closed。
- recovery dispatcher 必须按 exact Operation kind 隔离；v7 Schema 不实现或发布 Restore handler。
