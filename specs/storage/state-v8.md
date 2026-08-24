# ALCOMD State Schema v8：Extension Runtime contract draft

状态：M6 Stop A review closure；`state.db` production migration 尚未实现，daemon 仍广告 v7。

权威 contract snapshot 是 `state-v8-migration.contract.json`。生产获批后才允许创建
`crates/alcomd-store/migrations/0008_extension_runtime.sql`、接入自动 migration 并广告 `dataSchema: 8`。

## Operation kinds

v8 只增加：`extensions.install`、`extensions.uninstall`。install/uninstall 使用 immutable Plan/Apply 和各自 recovery
dispatcher；enable/disable 是 revisioned、永久幂等的窄 lifecycle command，不建立 durable Operation kind 或通用
workflow engine。既有与新增 dispatcher 永久只接管自己的 exact kind；unknown future kind fail closed。

## 表

### `extensions`

每 ExtensionId 一行：version/API major、package/Manifest/component SHA-256、publisher fingerprint、
`official|user_approved_for_extension` trust decision、extension PrincipalId、`installed_disabled|enabled|uninstalling`
desired state、`clear|quarantined` quarantine state、grant revision、lifecycle generation、revision、timestamps。没有
runtime `running` boolean；quarantine 不覆盖用户 desired intent。

### `extension_grants`

主键 `(extension_id, permission_name, resource_kind, resource_id)`。M6 business scope 只有
`Project + lowercase UUID`；`background.run` 使用 `Extension + ExtensionId` self scope。state 是 `granted|revoked`，
row 记录产生它的 current grant revision。对一个 extension 的 grant mutation 与 `extensions.grant_revision + 1`
在同一 transaction 提交，该 commit 是 revoke linearization point。

### `extension_instances`

每 ExtensionId 最多一行 current observation：InstanceId、PrincipalId、bound grant revision、lifecycle generation、
daemon epoch、`stopped|starting|running|stopping|crashed`、lease expiry/cancel、started/updated time。只有 current daemon
epoch + live in-memory child handle 才能证明 running；restart 先把旧 epoch active state 收敛为 crashed。

### `extension_crashes`

保存 bounded crash window 所需 timestamp 与 safe reason code；按 ExtensionId + occurred time 查询。每 ExtensionId
最多保留最近 16 条、并只使用最近 300,000 ms 的计数 evidence，不保存 trap/backtrace/path/argv，也不建立后台 GC
service 或第二份 Event/telemetry history。

### `extension_plans`

永久 immutable install/uninstall Plan authority：PlanId、owner Principal、action、state、ExtensionId、expected revision/
absence、`local_owner_selected|first_party_packaged` source kind、source filesystem identity、canonical SemVer `version`
（1-128 UTF-8 bytes）、固定 `api_major=1`、固定 `profile_version=1`、所有 digest、publisher/trust decision、requested
permission/interface snapshot、uninstall data disposition、plan fingerprint、唯一 Apply OperationId 与 timestamps。
上述字段均为独立 immutable authority，不得藏入 permissions/grant/scope JSON；唯一 update 是 `unapplied + NULL ->
applied + matching OperationId`；禁止 delete/rewrite/TTL。

Apply 与 recovery 必须同时复核 ExtensionId、version、API/profile version、package/Manifest/component digest、publisher、
permission/interface snapshot 与 source evidence；typed snapshot JSON 不得覆盖或绕过这些独立列。

### `extension_filesystem_journal`

append-only `(operation_id, step)` evidence，绑定 PlanId/ExtensionId/action、exact phase、intent/completed state、package/
staging/backup identity、bounded safe evidence 与 time。phase 必须来自 lifecycle v1；禁止 update/delete。

### `extension_data_namespaces` / `extension_data_items`

namespace authority 至少由 `(ExtensionId, publisher fingerprint)` 绑定，并保存 revision、key count、total value bytes；
items 保存 key、opaque BLOB、key revision。namespace 不以会被 uninstall 删除的 `extensions` registry row 为
`ON DELETE CASCADE` parent。same ExtensionId + different publisher 不能 attach/read 旧 namespace，返回
`extension_data_owner_mismatch`；M6 不实现 publisher transfer/key rotation。trigger/transaction 强制 1,024 keys、
4 MiB total、128-byte key、64 KiB value。无 list index、cross-extension foreign key、blob object、TTL 或 shared namespace。

任何 uninstall 都在移除 package authority 前 durable revoke 全部 grants/lease/session/handle；grant row 不作为未来
reinstall 的 active authority，reinstall 始终从 deny-by-default 开始。只有 immutable uninstall Plan 明确
`delete_data` 才允许删除 namespace/data。

## Migration/rollback

- 0008 production migration 必须 `BEGIN IMMEDIATE` 单事务，从 v7 重建 operations strict kind constraint、创建上述表，
  最后设置 `user_version=8`；失败完整回滚到 v7。
- v1-v7 既有 tables、rows、FK、revision、Event sequence、journal 与 idempotency byte-equivalent 保留。
- `PRAGMA foreign_key_check` 必须为空，future schema fail closed。
- Schema 存在不等于 dispatcher/capability 已实现；production wiring 前 system.hello 仍返回 dataSchema 7 且不广告
  M6 capability/extensionApi。
