# State Schema v13

状态：M7 P7 Project Directory Delete 已实现并接入 production。

权威 migration 是 `crates/alcomd-store/migrations/0013_project_directory_delete.sql`。State v13 从 v12
以单一 `BEGIN IMMEDIATE` transaction 迁移，并且只完成以下持久状态变化：

- `operations.kind` 兼容增加 `projects.delete-directory`；
- 新增 immutable `project_delete_plans`；
- 新增 append-only `project_delete_filesystem_journal`；
- 将 `package_plans.project_id`、`package_filesystem_journal.project_id`、
  `project_copy_plans.source_project_id` 与 `project_copy_filesystem_journal.source_project_id` 从当前
  `projects` registry 外键改为 checked lowercase UUID scalar，使 durable Plan/Journal 在 registry row 删除后继续存在；
- `project_editor_preferences.project_id` 继续是对当前 `projects` row 的 `ON DELETE CASCADE` 外键。

State v13 不增加 generic deletion、workflow、tombstone、recursive inventory 或 Trash registry。Delete Plan
只允许从 `unapplied` 绑定到精确 `projects.delete-directory` Operation；Plan 和 journal 禁止删除，journal
禁止更新。journal phase 固定为：

```text
accepted
preflight_complete
quarantine_intent
root_quarantined
registry_commit_intent
state_committed
deleting
cleanup_complete
recovery_required
```

`quarantine_intent` 保持最后取消边界及已移动目录的 forward-only 恢复。2026-11-10 所有者明确要求占用失败直接结束：rename 明确失败且可信 adapter 证明原目录/父目录 identity 保持、payload 不存在且无此前 quarantine identity 时，Store 可在该 intent 或原 recovery_required 阶段写 failed/completedAt；空准备目录清理失败仅记录技术诊断。项目登记/内容和 append-only journal 保持，不写伪造 cleanup_complete，不在解锁/重启后重试失败任务。实际已移动或状态不明继续恢复；Store 拒绝将这一未移动证明用于后续已有隔离身份的阶段。这是原公开终态的语义修正，不新增 Schema、journal phase 或 RPC 字段。

物理 project row 只在 quarantine identity 复验后删除；同一
transaction 只写一个 `project.directory_deleted` Event，并保留 Plan、Operation、journal 与 apply idempotency
authority。Operation 只有在 `cleanup_complete` durable evidence 已提交后才可进入 `succeeded`。

migration 必须保留全部 v12 row、Operation/Plan/Journal/幂等 authority、revision 与 Event sequence；
`pragma_foreign_key_check` 必须为空，失败完整回滚到 v12，future schema 继续 fail closed。精确机器合同位于
`state-v13-project-delete.proposal.contract.json`。
