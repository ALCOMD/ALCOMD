# ALCOMD State Schema v2

状态：Frozen for M3

权威 migration 是 `crates/alcomd-store/migrations/0002_projects_repositories.sql`。v2 必须从已经完整
提交的 v1 开始，在单个 `BEGIN IMMEDIATE` transaction 中升级；不得修改 `0001_state.sql`。

## 新增表

v2 只增加：

- `projects`：已注册项目、opaque 文件身份、bounded normalized snapshot、revision 与时间戳。
- `repositories`：local/remote source identity、bounded normalized metadata、HTTP validator、issue、
  revision 与时间戳。
- `repository_package_versions`：按 repository/package/version 保存 M3 raw identity/display model。

不得增加 dependency、package payload、ZIP cache、credential、Plan、Unity 写入或迁移表。路径 identity
与显示 path 分开：identity 使用 BLOB，显示 path 使用 Unicode-lossless TEXT。

## 兼容重建

`events` 的 column 顺序和既有约束保持不变，只将 `aggregate_kind` 集合从 `operation` 扩为
`operation/project/repository`。migration 必须保留全部 row、sequence、两个 index 和迁移前
`sqlite_sequence` 精确值。任何一步失败都回滚到完整 v1。

`idempotency_records.operation_id` 改为 nullable。约束固定为：

- pending 必须有 OperationId 且无 response；
- completed 必须有 response，可以有或没有 OperationId；
- M2 Operation command 既有 row 与 foreign key 保持有效。

## revision 与 transaction

Project/Repository 从 revision 1 开始。normalized semantic state 变化时，在同一 transaction 更新
aggregate、写 Event 和 completed idempotency response。HTTP validator-only 变化可以更新 row，但
不增加 revision、不发 Event。失败、304 与 no-op 不改变 aggregate revision。

外部 I/O 与 parse 不得在 SQLite transaction 内。同步 command 的最终 transaction 顺序是：
idempotency check/reservation、expected revision、aggregate mutation、Event、durable response、commit。
提交前外部失败不创建 idempotency row。

Unregister 删除 aggregate row和 repository package rows，但保留 Event/idempotency history；其 Event
使用删除前 revision + 1。重新注册同一 source 创建新 UUID。
