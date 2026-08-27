# ALCOMD State Schema v10 Project Copy proposal

状态：M7 proposal-only；production migration 未创建，daemon 继续广告 `dataSchema: 9`。

v10 是 Project Copy 的最小 durable authority，不包含 Favorite、Package Reinstall/Bulk、Config visibility、Remove Directory
或任何 generic workflow/table。它在单个 `BEGIN IMMEDIATE` migration 中从 v9 增加：

- `operations.kind` 允许 `projects.copy`；
- `project_copy_plans`；
- `project_copy_filesystem_journal`。

## `project_copy_plans`

每行由 UUID `plan_id` 主键，绑定 owner Principal、source Project FK、source revision/canonical root/opaque filesystem identity、
project kind、Unity version/revision、writer evidence、target parent canonical path/opaque identity、normalized target leaf、
`target_must_not_exist=1`、预分配 target ProjectId、profile ID/version、profile/quotas/exclusion JSON、Plan fingerprint、plan-method
idempotency key、`created_at_ms`、`expires_at_ms=created_at_ms+900000`、state 与可选唯一 apply OperationId。

state 只允许 `unapplied`/`applied`。唯一 UPDATE 是 `unapplied + NULL operation` 到 `applied + exact projects.copy
operation`，其余字段逐列 `IS`/`=` 不变；其他 UPDATE trigger fail，DELETE trigger fail。owner/method/key/fingerprint 由既有永久
idempotency表绑定，不能通过修改 Plan row 换 owner/target/profile。source FK 只保证注册 identity；Apply 仍重新验证 revision 与
filesystem identity。

Plan JSON 与 evidence 每项有 4 MiB 上限，且必须是 canonical valid JSON。完整 inventory **不**保存在 Plan row。

## `project_copy_filesystem_journal`

append-only 主键 `(operation_id, step)`，每行绑定 OperationId、PlanId、source/target ProjectId、phase、intent/completed state、
source/target parent/target identity evidence、inventory fingerprint、exact bounded inventory evidence或其 daemon-owned locator、
target owner/recovery marker、updated timestamp。insert trigger要求 Operation kind=`projects.copy` 且 Plan 的 apply Operation完全相同。

phase 只允许：`accepted`、`inventory_ready`、`staging`、`staging_complete`、`publish_intent`、
`target_published`、`project_registry_commit_intent`、`state_committed`、`cleanup_complete`、`recovery_required`。
UPDATE/DELETE 全部拒绝；recovery index 是 `(operation_id, step DESC)`，另有未终结 Operation 恢复索引。journal-owned staging/backup
locator不得进入 public RPC/Event/activity/log。

## 事务与恢复不变量

- Plan 接受与 idempotency response 在一个短 transaction 中提交；不执行 filesystem traversal。
- Apply 接受把 immutable Plan 绑定到预分配 OperationId，提交 accepted journal evidence 后才返回。
- inventory/staging/publish 使用 append-only evidence；SQLite transaction 不跨文件 I/O。
- publish 前不得写 Project registry；publish复验后在一个短 transaction提交 Project row、revision、Event、idempotency Apply
  result、Operation state与state-committed journal evidence。
- `publish_intent` 前取消可以删除 journal-owned staging；写入 intent后只 forward recover。
- target被外部修改时保留可见 target并进入 recovery_required，不回滚覆盖。
- migration保留全部 v9 row/FK/revision/Event sequence；`foreign_key_check`必须为空，失败完整回滚为v9。
- schema version大于10继续fail closed。

精确机器可读提案位于 `state-v10-migration.proposal.contract.json`。production activation前必须人工批准真实 0010 SQL、trigger
逐列不变量、migration tests 和 capability advertisement；本文件本身不授权实现。
