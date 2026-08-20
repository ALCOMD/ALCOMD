# ALCOMD State Schema v5

状态：Implemented（M5 Template durable Plan contract closure）

权威 migration 是 `crates/alcomd-store/migrations/0005_template_plans.sql`。daemon 从 v4 原子迁移到
v5，完整迁移成功并完成 store 初始化后才通过 `system.hello` 广告 `dataSchema: 5`。0001-0004
保持不变。

## 范围

v5 只增加当前 Template slice 已出现的两个持久合同：

- `template_plans`：`import | derive | create-project` 三种 immutable Plan；
- `operations.kind`：精确增加 `templates.import`、`templates.derive`、
  `templates.create-project`。

不增加 export Operation、Backup Plan、TTL、expiry、cleanup worker、Plan history、generic workflow、
generic Plan registry、GUI/MCP/Extension 字段或独立文件 Plan store。`package_plans` 保持 M4 专用表，
不迁移、不泛化。

## template_plans

`template_plans` 保存 36 字符 PlanId、owner Principal、kind、`unapplied | applied` state、32-byte
fingerprint、bounded `plan_json`、唯一可空 OperationId 与创建时间。`plan_json` 必须是 JSON object，
UTF-8 bytes 不超过 4 MiB，并携带整数 `version: 1` 和与 row 相同的 kind。application 层为三个 kind 使用不同
的严格 DTO；SQLite 的 version/kind check 是持久层 fail-closed 边界，不替代 DTO validation。

Plan 永久不可删除。唯一允许的 update 是：

```text
unapplied + NULL apply_operation_id
    -> applied + matching OperationId
```

trigger 同时验证 Operation kind 与 Plan kind 精确匹配。其他字段、重复 Apply、回退到 unapplied、
删除或错误 Operation kind 全部失败。

## Migration 不变量

- migration 在 `BEGIN IMMEDIATE` 中执行，最后设置 `user_version=5`；失败完整回滚到 v4。
- operations 重建时同步重建其直接引用表，逐行保留 OperationId、kind/state/revision、
  `operation_journal`、`idempotency_records`、`package_plans` 和
  `package_filesystem_journal`。
- M4 package Plan 的 triggers、foreign keys、journal append-only 规则和 Operation kind 保持不变。
- `events` 不重建，sequence/AUTOINCREMENT 状态与既有 Event rows 不变化。
- Schema v4 的 Unity installation、project preference、Template registry 与 Backup metadata 不变化。
- migration 后 `PRAGMA foreign_key_check` 必须为空；future schema 继续 fail closed。

## RPC compatibility

RPC v1 只兼容增加新的 Operation kind 和 `dataSchema: 5`。Template capabilities 与 methods 仍只有
在 application/backend 真实接入后才协商和发布；Schema v5 本身不证明 Template 生产功能已经实现。
