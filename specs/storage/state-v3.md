# ALCOMD State Schema v3

状态：Frozen for M4 contract-first；生产 store 尚未启用

权威 migration 是 `crates/alcomd-store/migrations/0003_package_transactions.sql`。v3 从完整 v2 在单个
`BEGIN IMMEDIATE` transaction 升级；不得修改 `0001_state.sql` 或 `0002_projects_repositories.sql`。
M4 生产依赖与实现未获批前，daemon 的 current schema 仍为 v2，不得自动执行本 migration。

## 兼容变化

- `operations.kind` 兼容增加 `packages.apply`；既有 Operation、journal、idempotency row、索引和
  foreign key 必须原样保留。
- `repositories.priority` 为正整数，数值越小越优先。v2 row 按同一 owner 内
  `(registered_at_ms ASC, repository_id ASC)` 依次赋值；UUID 只用于迁移排序稳定性，不成为同业务
  priority 的 source tiebreak。
- `repository_package_versions` 增加 resolver metadata。所有 v2 row 的 `resolver_ready` 固定为 0；
  只有显式 M4 `repositories.refresh` 完整成功后才可写入 semantic version、author、artifact URL、
  SHA-256、Unity/release、dependencies、manifest fingerprint、legacy presence 并置为 1。

## 新增表

`package_plans` 保存 immutable durable Plan。状态只有 `unapplied`/`applied`，无 expiry/TTL/cleanup。
唯一允许的 UPDATE 是一次性绑定 `apply_operation_id` 并把状态从 unapplied 改为 applied；trigger 禁止
修改其余输入、fingerprint、ChangeSet、source set 或时间。`change_set_json` 受 4 MiB、1,024 mutations
与 4,096 dependency edges 上限约束。

`package_filesystem_journal` 是 append-only write-ahead evidence。每个 destructive phase 先写
`intent`，filesystem mutation 与 fsync/evidence 完成后写 `completed`。phase 只允许：

```text
accepted
archive_ready
extracted
prepared
packages_replaced
vpm_manifest_committed
filesystem_committed
state_committed
rolling_back
rolled_back
recovery_required
```

journal 保存 opaque project identity、ChangeSet fingerprint 与 bounded evidence JSON，不保存完整
私密路径、raw manifest、archive、credential 或 token。恢复必须结合 project-local marker 与 old/new
digest；证据不一致时进入 `recovery_required`，不得猜测或删除证据。

## transaction boundary

外部网络、hash、ZIP preflight/extraction、fsync 与 Resource Lock 等待都不得出现在 SQLite
transaction 内。每个 filesystem phase 使用短 intent/completed transaction。只有 durable
`filesystem_committed` 后，才允许最终短 transaction 原子提交 Project revision、Event、Operation、
idempotency response 与 `state_committed`；commit 前不得报告 succeeded。

Plan 写入是 ALCOMD 内部持久化，不修改项目。Apply 永久幂等 scope 仍为
`(PrincipalId, packages.applyPlan, idempotencyKey)`，fingerprint 固定 planId 与 expectedRevision。
同一 Plan 最多绑定一个 Apply Operation。
