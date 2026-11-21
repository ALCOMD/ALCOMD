# ADR: ALCOMD 原生数据规范

- 状态：Accepted
- 日期：2026-08-14

## 背景

参见 `docs/architecture/ALCOMD-V4.md`。


## 决策

v4 使用 `settings.toml`、SQLite、对象库与 OS 凭据库。VCC 与 v3 格式不作为运行时状态。

首发 State1 的表、索引和约束由[完整 DDL](../../crates/alcomd-store/src/schema.sql) 定义，项目、仓库、模板、扩展、任务及事件一并初始化。

SQLite 依赖固定为关闭默认 feature、只启用 bundled 的 `rusqlite 0.40.1`，预期使用
`libsqlite3-sys 0.38.1` / SQLite 3.53.2。一个专用 store worker 独占 connection，不使用 ORM、
连接池、SQLx 或 cache feature。

daemon 取得每用户 OS 单实例锁，先只读预检配置和数据库。数据文件须有 SQLite 标识、当前 user_version 和完整 DDL；空环境的初始化事务同时建立完整结构和 user_version。普通运行启用 WAL、外键、synchronous=FULL 与忙等待。只有恢复完成才发布 IPC 端点，预检失败保全输入文件。

幂等 scope 是 `(PrincipalId, method, idempotencyKey)`；同 key/同 method-specific canonical
fingerprint 返回原结果，不同 fingerprint 返回 idempotency_conflict。M2 不删除幂等记录、不按
时间过期，也不允许 key 因时间经过重新使用。

Event sequence 是数据库全局 AUTOINCREMENT 正整数。events.list 以 afterSequence 为 exclusive
cursor，默认 100、最大 1000、严格 ASC；空页 nextSequence 等于输入，否则等于本页最后 sequence。
M2 不删除 Event，不实现 retention。

## 结果

数据、配置、RPC、扩展和导出格式各自只保留首发定义，首次正式发布前编号均为 1。
