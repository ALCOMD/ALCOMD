# ALCOMD state.db Schema v1

状态：M2 Schema v1 已实现；等待三平台最终验证

## 文件与所有权

`state.db` 是 `alcomd` 的内部权威状态，不是第三方 API。只有每用户唯一 daemon 的 store worker
打开 SQLite connection；GUI、CLI、MCP、扩展和其他进程不得直接访问。

正式路径：

```text
Windows: %LOCALAPPDATA%\ALCOMD\data\state.db（FOLDERID_LocalAppData）
Linux:  $XDG_DATA_HOME/alcomd/state.db 或经所有权检查的标准 per-user fallback
macOS:  ~/Library/Application Support/ALCOMD/state.db
```

测试必须使用隔离 data root。Unix 自建目录要求有效 UID 所有、0700、不跟随 symlink；SQLite
文件及 WAL/SHM 不得位于共享可写目录。

## 初始化顺序

1. daemon 先取得生命周期绑定的每用户 OS 单实例锁，但尚不发布 IPC endpoint。
2. 打开 connection，首先读取 `PRAGMA user_version`。
3. 若版本高于 `1`，立即 fail closed；不执行 pragma 变更、migration 或任何写入。
4. 对支持版本执行 `PRAGMA foreign_keys=ON` 并读回验证为 `1`。
5. 执行 `PRAGMA journal_mode=WAL` 并验证返回 `wal`。
6. 执行 `PRAGMA synchronous=FULL`。
7. 设置 `busy_timeout=5000 ms`。
8. 若版本为 `0`，执行 `0001_state.sql` 的单个短 transaction。
9. migration 内最后设置 `user_version=1`；任何错误完整回滚。
10. 重新读取并验证 user_version、foreign_keys 与 journal_mode。
11. 扫描并恢复 Operation；完成后 daemon 才可 bind/报告 ready。

启动期任何失败都使 daemon fail closed。M2 不提供 half-ready/degraded 模式。daemon ready 后的
store I/O 失败映射为安全的 `store_unavailable`；不得返回数据库路径、SQL 或完整 SQLite 文本。

## Schema

权威 migration：`crates/alcomd-store/migrations/0001_state.sql`。

只包含四张 STRICT table：

- `operations`
- `operation_journal`
- `events`
- `idempotency_records`

有限状态字段有 SQL CHECK；正整数同时由 SQL 与 Rust 验证且不超过 `i64::MAX`。JSON 字段有
`json_valid` 与 byte size 上限。Schema 不包含未来项目、包、仓库、设置、扩展或 credential 表。

## transaction boundary

Operation 创建 transaction 原子写入 idempotency reservation、queued Operation、prepared journal、
首个 Event 与 accepted response。状态变化 transaction 原子校验 expectedRevision、更新 Operation/
journal、插入 Event 并保存幂等响应。SQLite commit 前不得报告成功；commit 后响应丢失由同一 key
重放原结果。

Resource Lock 必须在开启 SQL transaction 前取得。transaction 内不得等待 IPC、锁、用户输入、
SQLite integrity check 或其他长任务。

## idempotency

唯一 scope 是 `(PrincipalId, method, idempotencyKey)`。M2 使用 method-specific、版本化、类型化、
非敏感 canonical JSON fingerprint；不使用 Rust DefaultHasher 或随机 hash。记录永久保留，不含
expiry，key 不因时间经过重新可用。

## Event 与 Operation pagination

Event sequence 是数据库全局 AUTOINCREMENT，允许空洞。`events.list` 按 Principal + sequence
升序读取 exclusive afterSequence；默认 100、最大 1000。空页 nextSequence 等于输入，非空页
等于本页最后 sequence。

Operation list 使用索引 `(owner_principal_id, created_at_ms DESC, operation_id DESC)`；cursor 携带
上一页最后项的 createdAtMs/operationId，下一页使用严格 tuple comparison。
