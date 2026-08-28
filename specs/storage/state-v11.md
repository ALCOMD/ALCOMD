# ALCOMD State Schema v11 Project Preferences Proposal

状态：P5-B contract-first 已获人工批准，等待 production wiring；尚无 production migration，daemon 仍广告
`dataSchema: 10`。

v11 候选只承载 Project Favorite 与可清除的 Unity editor selection，不包含 Package、Remove Directory、GUI view state、
Config visibility 或任何 generic preference framework。

## `projects.favorite`

给既有 `projects` table 增加：

```sql
favorite INTEGER NOT NULL DEFAULT 0 CHECK (favorite IN (0, 1))
```

它是注册 Project 的用户 metadata，不进入 `snapshot_json`，也不由 refresh 覆盖。所有 v10 row迁移为 `0`；新注册、Create、
Copy 与 Restore 项目默认 `0`。unregister 删除 row 后不保留 Favorite tombstone。

## `project_editor_preferences`

以单次 migration 重建既有 table，保留 table 名与 arguments/revision authority，增加：

- `selection_mode TEXT NOT NULL CHECK (selection_mode IN ('automatic','explicit'))`；
- `installation_id` 改为 nullable FK；
- table CHECK 只允许 `automatic + NULL` 或 `explicit + non-NULL`。

v10 existing row 全部迁移为 `explicit`，原 installation、arguments、revision 与 updated timestamp 原样保留。缺失 row 不在 migration
中物化；其 canonical public 语义是 automatic、empty arguments、preference revision 0、updatedAtMs 0。清除 explicit override 时
保留 row 和 arguments，把 mode 改为 automatic并清空 installation ID。没有第二张 preference/arguments table。

migration 使用单一 `BEGIN IMMEDIATE` transaction，先增加 Favorite column，再重建 preference table；`foreign_key_check` 必须为空，
`user_version = 11` 最后设置。任何失败完整回滚到 v10 authority。

## Revision、Event 与 idempotency

- Favorite 在精确校验 expected Project revision 后判断状态；变化增加 Project aggregate revision并写
  `project.favorite_changed`。相同值 no-op 不增加 revision/Event/updated_at，但保存 completed idempotency response。
- Editor clear 只增加 project-editor-preference aggregate revision，写 `unity.project-editor.selection_cleared`；不增加 Project
  revision。已经 automatic 的 clear 是 no-op。
- 两者均使用既有永久 `idempotency_records`；same key/same fingerprint replay，same key/different fingerprint conflict。
- migration 保留所有 v10 rows、Event sequence、idempotency authority 与 FK；失败完整回滚，future schema 继续 fail closed。

`ProjectRecord.favorite` 使用 serde default，保证 v10 durable JSON 可读取/replay。旧 `ProjectEditorPreference` 与 get/set response
保持 explicit-only；新 selection state 单独表达 automatic/explicit，并且仅其 public revision 接受 sentinel 0。

精确机器合同位于 `state-v11-migration.proposal.contract.json`。在 production migration、store、application、RPC、typed client 与 GUI
全部接线前不得广告 dataSchema 11。
