# ADR: ALCOMD 原生数据规范

- 状态：Accepted
- 日期：2026-08-14

## 背景

参见 `docs/architecture/ALCOMD-V4.md`。


## 决策

v4 使用 `settings.toml`、SQLite、对象库与 OS 凭据库。VCC 与 v3 格式不作为运行时状态。

M2 首次实现 `data/state.db`，数据 Schema v1 只包含 operations、operation_journal、events 与
idempotency_records 四张 STRICT table。最终 SQL 由
`crates/alcomd-store/migrations/0001_state.sql` 冻结；不预建项目、包、仓库、设置或扩展表。

SQLite 依赖固定为关闭默认 feature、只启用 bundled 的 `rusqlite 0.40.1`，预期使用
`libsqlite3-sys 0.38.1` / SQLite 3.53.2。一个专用 store worker 独占 connection，不使用 ORM、
连接池、SQLx 或 cache feature。

daemon 先取得生命周期绑定的每用户 OS 单实例锁，但尚不发布 IPC endpoint；随后打开 store 并
先读取 user_version。若高于支持版本立即 fail closed 且不写入；随后设置并验证 foreign_keys=ON
与 journal_mode=WAL，设置 synchronous=FULL 与 busy_timeout=5000ms。
`0 -> 1` migration 在一个短 transaction 内执行，user_version=1 是最后的 Schema 版本写入；
失败完整回滚。恢复完成后才 bind endpoint；store 初始化失败时 daemon 在 ready 前 fail closed，
不提供 degraded 模式。

幂等 scope 是 `(PrincipalId, method, idempotencyKey)`；同 key/同 method-specific canonical
fingerprint 返回原结果，不同 fingerprint 返回 idempotency_conflict。M2 不删除幂等记录、不按
时间过期，也不允许 key 因时间经过重新使用。

Event sequence 是数据库全局 AUTOINCREMENT 正整数。events.list 以 afterSequence 为 exclusive
cursor，默认 100、最大 1000、严格 ASC；空页 nextSequence 等于输入，否则等于本页最后 sequence。
M2 不删除 Event，不实现 retention。

## 结果

数据、配置、RPC、扩展和导出格式分别版本化。
