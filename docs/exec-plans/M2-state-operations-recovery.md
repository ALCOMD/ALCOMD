# M2：SQLite 权威状态、Operation、Event、Revision、幂等、资源锁与恢复

状态：草案；等待人工审批，未开始 M2 生产实现

## 目标

在已经完成的 M1 单实例 daemon、本地 RPC、握手与类型化 client 基础上，建立第一个持久状态与
可恢复 Operation 垂直切片：

```text
alcomd-client: state.check
        │
        ▼
ALCOMD RPC v1 compatible additions
        │
        ▼
alcomd-application command/query
        │
        ├─ Principal / permission check
        ├─ idempotency reservation
        ├─ Resource Lock
        └─ Operation worker
                │
                ▼
alcomd-store -> data/state.db
        ├─ Operation + journal
        ├─ Event sequence
        └─ revision + durable result
```

`state.check` 是针对 ALCOMD 自身 SQLite 状态的真实、无外部副作用完整性检查，不是测试专用
假命令。它返回 `OperationId`，允许 M2 用最小业务面验证持久化、查询、取消、幂等重放、事件
断点续读、daemon 崩溃与重启恢复。M2 不借此建立通用工作流引擎，也不引入项目、VPM 或文件
事务。

## 交付物

1. `alcomd-store` 中单一 `state.db` 连接所有者、Schema v1、迁移与短事务 API。
2. transport-neutral 的 Operation、Event、Revision、幂等和 Resource Key 领域合同。
3. `state.check`、Operation 查询/取消和 Event 断点读取 application 用例。
4. RPC v1 的兼容新增 method、capability、DTO、稳定错误、JSON Schema 与合同快照。
5. daemon 启动时的迁移、未完成 Operation 扫描和确定性恢复。
6. Windows、Ubuntu 与 macOS 上真实 SQLite/IPC/崩溃恢复集成测试。
7. 更新后的测试元数据、feature parity 实现状态、ExecPlan 进度和 `docs/status.md`。

## 完成定义

- daemon 启动时安全创建或打开每用户 `data/state.db`，只接受受支持的 Schema，并在迁移失败时
  不留下部分迁移。
- SQLite 只有一个进程内连接所有者；所有写入仍只能从 `alcomd-application` 用例进入。
- `state.check` 首次请求创建持久 Operation；相同 Principal/method/key 的相同请求重放返回同一
  Operation/结果，不重复执行。
- Operation 状态、revision、journal、Event 与幂等记录遵守已冻结的事务边界；不存在“状态已
  提交但事件缺失”或“副作用开始但 journal 未准备”的路径。
- 客户端断开不取消 Operation；重新连接可用 `OperationId` 查询，并可从已确认 sequence 后继续
  读取 Event。
- daemon 被强制终止后，重启能识别所有非终态 Operation；M2 的 `state.check` 可安全恢复，未知
  或不可恢复 kind 不会被猜测为成功。
- revision 冲突、幂等冲突、权限不足、Schema 不支持、Operation 不存在/不可取消等均返回稳定
  机器可读错误，不泄露数据库路径、SQL、堆栈或凭据。
- 同一资源写操作串行；不同 Resource Key 可并行；锁不会跨 await 持有 SQLite transaction。
- 全部本地门禁和三个 hosted 平台 CI 通过，三份锁文件无意外变化，工作树干净。
- M2 状态更新为“等待人工验收”，且没有开始 M3。

## 前置条件

- M1 最终提交 `7ed70626a0176f855a8e9efdc6d35d317f51ca78` 与 GitHub Actions run
  `32126344788` 已由项目所有者人工验收。
- M1 后续最小文档一致性提交已经将架构 `system.hello` 示例与冻结 RPC Schema 对齐。
- A-004 唯一写入者、A-005 application 边界、A-026 稳定结构化错误保持有效。
- M1 的 framing、envelope、握手、端点和按需启动合同不得被 M2 破坏。

## 最小可运行垂直切片

M2 只增加以下真实能力：

- `state.check`：创建可恢复 Operation，对 `state.db` 执行有界完整性检查，结果只返回安全的
  `ok`/失败分类，不返回原始 SQL 或本地路径。
- `operations.get`：按 ID 查询当前状态、revision、时间、结果或稳定错误。
- `operations.list`：按稳定游标分页查询当前 Principal 可见的 Operation。
- `operations.cancel`：携带 `expectedRevision` 与幂等键提交合作式取消请求。
- `events.list`：从 `afterSequence` 之后按升序读取 Event，供断线恢复。

M2 不增加任意脚本、任意 SQL、通用 job definition、用户自定义 workflow 或通用键值存储。
`state.check` 的生产实现可以很快完成；故障注入测试使用测试专用 gate 拉长阶段，不在公开 RPC
加入 sleep/failpoint 参数。

## SQLite 与 state.db

### 路径与权限

- Windows：`%LOCALAPPDATA%\ALCOMD\data\state.db`。
- Linux：`$XDG_DATA_HOME/alcomd/state.db`，缺失时使用经所有权检查的标准 per-user data fallback。
- macOS：`~/Library/Application Support/ALCOMD/state.db`。
- 测试只能使用显式隔离 data root，不得触及真实用户数据。
- Unix 自建父目录必须属于有效 UID 且为 `0700`；数据库、WAL 与 SHM 不允许放入共享可写或
  symlink 可替换路径。Windows 路径与 ACL 由 `alcomd-platform` 提供安全适配。

生产数据路径实现可能需要扩展现有 `windows-sys` feature 或引入单一平台目录 crate，必须在
实施前按“生产依赖审批点”单独批准，不能解析显示名或调用外部命令猜测路径。

### Schema v1

以下是 M2 的最小逻辑 Schema。实施前应转换为受快照测试约束的迁移 SQL；字段类型、约束、
索引与公开 DTO 必须在合同阶段由项目所有者批准。

```sql
-- migration transaction 最后才执行 PRAGMA user_version = 1

CREATE TABLE operations (
    operation_id       TEXT PRIMARY KEY,
    kind               TEXT NOT NULL,
    state              TEXT NOT NULL,
    revision           INTEGER NOT NULL CHECK (revision >= 1),
    owner_principal_id TEXT NOT NULL,
    request_json       TEXT NOT NULL,
    result_json        TEXT,
    error_code         TEXT,
    diagnostic_id      TEXT,
    cancel_requested   INTEGER NOT NULL CHECK (cancel_requested IN (0, 1)),
    created_at_ms      INTEGER NOT NULL,
    updated_at_ms      INTEGER NOT NULL,
    started_at_ms      INTEGER,
    completed_at_ms    INTEGER
);

CREATE TABLE operation_journal (
    operation_id   TEXT NOT NULL REFERENCES operations(operation_id),
    step           INTEGER NOT NULL,
    kind           TEXT NOT NULL,
    state          TEXT NOT NULL,
    payload_json   TEXT NOT NULL,
    updated_at_ms  INTEGER NOT NULL,
    PRIMARY KEY (operation_id, step)
);

CREATE TABLE events (
    sequence           INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id           TEXT NOT NULL UNIQUE,
    kind               TEXT NOT NULL,
    aggregate_kind     TEXT NOT NULL,
    aggregate_id       TEXT NOT NULL,
    aggregate_revision INTEGER NOT NULL,
    principal_id       TEXT NOT NULL,
    occurred_at_ms     INTEGER NOT NULL,
    payload_json       TEXT NOT NULL
);

CREATE TABLE idempotency_records (
    principal_id        TEXT NOT NULL,
    method              TEXT NOT NULL,
    idempotency_key     TEXT NOT NULL,
    request_fingerprint TEXT NOT NULL,
    state               TEXT NOT NULL,
    operation_id        TEXT REFERENCES operations(operation_id),
    response_json       TEXT,
    created_at_ms       INTEGER NOT NULL,
    expires_at_ms       INTEGER NOT NULL,
    PRIMARY KEY (principal_id, method, idempotency_key)
);
```

必需索引仅覆盖 Operation owner/state/created time 与 Event aggregate/sequence 查询。M2 不预建
项目、包、仓库、设置、扩展、审计或全文搜索表。

JSON 字段是版本化的内部持久 DTO，不是第三方 API。每个字段有严格大小上限并用类型化结构
读写；禁止把任意 SQL、堆栈、token、完整私密路径或未经筛选的内部错误写入 Event/结果。

### SQLite 运行策略

- 推荐一个专用 store worker 拥有一个 `rusqlite` connection，通过有界 channel 接受短请求；
  不引入连接池、ORM、async SQL 宏或通用 repository framework。
- 启用 `foreign_keys=ON`、WAL、`synchronous=FULL` 和有界 `busy_timeout`；实际 pragma 组合必须
  用崩溃/重启测试证明，不能为性能静默降低持久性。
- 不在 SQLite transaction 内等待 Resource Lock、IPC、用户输入或长任务。
- Operation worker 在 transaction 外运行；每次状态转换使用一个短 `BEGIN IMMEDIATE`
  transaction。

## Schema 版本与 migration

- `PRAGMA user_version` 是 M2 数据 Schema 当前版本的唯一数值来源；M2 从 `0 -> 1`。
- migration 以按版本排序的内嵌 SQL 随二进制发布，禁止从 data directory 动态加载 SQL。
- 每次 migration 在一个排他 transaction 中执行，最后才更新 `user_version`；失败完整回滚并
  阻止 daemon 对外报告 ready。
- 数据库版本高于二进制支持值时返回 `data_schema_unsupported` 并拒绝写入，不自动降级。
- M2 不实现 down migration。未来兼容程序升级只能追加严格递增 migration；破坏性或不可逆
  数据变更必须另行审批并先定义备份/恢复策略。
- Schema v1 真实存在后，`system.hello` 可兼容增加可选 `dataSchema: 1`，但不得让旧客户端必须
  理解该字段，也不得加入仍未实现的 `configSchema`/`extensionApi`。

## Operation 生命周期

冻结状态集合继续使用：

```text
queued -> planning -> waiting_for_input -> running -> cancelling
                                            │             │
                                            ├─────────────┤
                                            ▼             ▼
                                      succeeded | failed | cancelled

non-terminal crash -> interrupted -> recovering -> queued/running/failed/cancelled
```

- `succeeded`、`failed`、`cancelled` 是不可变终态。
- `interrupted` 是必须持久化并发出 Event 的恢复观察点，不代表成功；恢复器随后进入
  `recovering` 并按 operation kind 策略处理。
- `cancelling` 表示取消意图已接受，不保证最终一定为 `cancelled`；完成竞态可合法到
  `succeeded`/`failed`。
- M2 的 `state.check` 动态路径只需要 queued/running/cancelling/终态/interrupted/recovering。
  全部状态转换规则在领域单元测试冻结；`waiting_for_input` 的公开 input/approve/reject/resume
  方法等第一个真实 Plan/Apply 用例出现时再冻结，不在 M2 预建通用审批引擎。
- 客户端断开不改变 Operation；取消只能通过显式 `operations.cancel`。
- 每次公开可观察变化增加 Operation revision，并在同一 SQL transaction 写对应 Event。

Operation 结果与稳定 domain error 持久化；未知内部失败只存 `internal_error` 与随机
`diagnostic_id`。技术诊断进入受控日志，不进入普通 Operation DTO。

## Event sequence 与断线恢复

- Event 使用单个数据库范围的严格递增 `u64 sequence`，已提交顺序是唯一排序事实；时间戳只
  用于展示，系统时钟回拨不影响顺序。
- SQLite 只保存正的 signed 64-bit INTEGER；公开 DTO 可使用 `u64`，但 store 必须在
  `i64::MAX` 前拒绝继续分配 sequence，不能溢出、绕回或复用旧值。
- sequence 不要求连续，客户端必须按“大于最后确认值”读取，不能用数量推算下一值。
- 状态变更与 Event 在同一 transaction 提交。
- `events.list(afterSequence, limit)` 使用 exclusive cursor、sequence 升序，M2 `limit` 建议默认
  100、最大 1000；返回 `nextSequence`。
- 客户端断线后使用最后已处理 sequence 重试；重复 Event 必须可安全去重。
- M2 不实现 server-initiated notification 或无限流，避免破坏 M1 传输假设。后续可兼容增加
  live subscription，但必须仍以持久 sequence 为恢复事实。
- M2 暂不删除 Event。Schema/RPC 预留未来 `event_cursor_expired` 错误语义；任何实际 retention/
  compaction 策略必须在产生高频业务 Event 前另行批准。

## Revision 与 expectedRevision

- 每个可变聚合拥有从 `1` 开始的 `u64 revision`；M2 首个聚合是 Operation。
- 对既有聚合的写命令必须携带 `expectedRevision`。不匹配返回 `revision_conflict`，响应可在授权
  后包含当前 revision，但不自动重试写入。
- 成功的可观察状态变化 revision 加一；纯查询、同一幂等请求重放和无变化的取消请求不增加。
- Event 的 `aggregateRevision` 必须等于产生该 Event 后的聚合 revision。
- SQLite INTEGER 是有符号 64 位；实现必须拒绝超出安全正整数范围，不能静默绕回。

M3 及后续项目/仓库资源必须复用该语义，但各自何时增加 revision 由对应里程碑冻结，M2 不替
未来资源做决定。

## command 幂等

- 写命令要求非空 ASCII `idempotencyKey`，建议最大 128 bytes。
- 唯一 scope 是 `(PrincipalId, method, idempotencyKey)`；不同 Principal 不能共享结果。
- 首次请求在开始工作前以 transaction 原子保留 key。相同 key 和相同稳定 fingerprint 返回同一
  OperationId/已保存响应；相同 key 不同 fingerprint 返回 `idempotency_conflict`。
- fingerprint 来自已验证的类型化 params 的稳定、非敏感序列化。M2 不使用进程随机 hash，也不
  为此保存 token、完整路径或原始任意 JSON。
- 建议完成结果至少保存 24 小时；pending/non-terminal 记录不得在 Operation 完成前过期。精确
  TTL、清理时机与“过期后 key 可重用”语义是实施前人工审批点。
- idempotency 只防重复提交，不替代 `expectedRevision`、授权或 Resource Lock。

## Resource Lock 模型

M2 只实现两个 canonical key：

```text
StateStore
Operation(operation_id)
```

- 锁由 `alcomd-application` 的小型 coordinator 管理，daemon/CLI/RPC adapter 不自行定义锁。
- 单 daemon 下使用进程内、生命周期绑定的异步独占锁；不把锁文件存在或 SQLite 行当作持锁
  进程存活证据。
- 多资源请求先去重再按 canonical byte order 一次性获取，避免死锁。
- 同 key 串行，不同 key 可并行；取消等待锁时不能留下 owner 或 journal。
- 锁 guard 覆盖业务临界区，但不能跨用户输入等待，也不能在持有 SQLite transaction 时等待锁。
- daemon 崩溃后进程内锁自然消失；恢复器依据 durable Operation/journal 重新获取，而不是恢复
  stale owner。

未来 `Project(id)`、`PackageCache` 等 key 必须在真实用例里程碑兼容增加；M2 不实现分布式锁、
租约续期、优先级调度或通用锁 DSL。

## crash/restart recovery journal

- 每个可恢复 Operation 在执行可观察阶段前，先在 `operation_journal` 写 `prepared` step；数据库
  状态变更与 journal step 尽可能同 transaction 提交。
- 完成阶段后将 step 置为 `applied`；需要补偿的外部文件步骤不是 M2 范围，M4 前必须另行冻结
  文件 journal、fsync、rename 与 rollback 合同。
- daemon 启动并完成 migration 后、对外 ready 前扫描所有非终态 Operation：
  1. 原子标记为 `interrupted` 并写 Event；
  2. 依据显式 operation-kind recovery policy 决定是否进入 `recovering`；
  3. `state.check` 可安全重跑；已请求取消则收敛到 `cancelled`；
  4. 未知 kind 或 journal 不一致进入 `failed`/`internal_error + diagnostic_id`，不得猜测成功。
- 恢复使用同一个 idempotency reservation 和 Resource Lock；不会创建第二个 Operation。
- M2 不实现跨版本任意 workflow 恢复。升级前存在的 operation kind 若新版本不认识，daemon 必须
  保留证据并安全失败。

## Principal 与权限的 M2 边界

M2 第一次引入写 RPC，不能继续把 `client.name/version/instanceId`、SID、pipe 名或 OperationId
当作授权事实。

建议最小合同：

- 引入 transport-neutral `PrincipalId`，Operation、Event visibility 和 idempotency scope 均绑定
  Principal。
- M2 只提供一个明确命名的内建 first-party local-owner Principal，用于当前同用户官方客户端；
  它不是“任意同用户进程已被安全识别”的声明。
- M2 提议冻结最小权限：`state.check`、`operations.read`、`operations.cancel`、`events.read`。
- 每个 application 用例再次校验 Principal/permission；RPC adapter 的预检查不构成授权。
- 测试可使用隔离的 synthetic Principal 验证 owner、permission、撤销和幂等隔离，但不把 synthetic
  credential 当成发行机制。

外部应用配对、token 签发、OS credential store、MCP Bearer/STDIO Principal、扩展身份与完整
授权管理 UI 不属于 M2。它们分别在后续里程碑实现；在这些合同完成前，M2 不向任意外部应用
宣传写接口安全。上述 Principal 形态和四个权限名称属于公共合同，实施前必须人工批准。

## RPC v1 兼容新增

M2 只以 M1 已允许的方式增加 method、capability、DTO 和可选字段，不提升 RPC major：

- capabilities 建议：`state.schema.v1`、`operations.v1`、`events.replay.v1`。
- `system.hello.result` 兼容增加可选 `dataSchema: 1`。
- methods：`state.check`、`operations.get`、`operations.list`、`operations.cancel`、`events.list`。
- DTO：`OperationSummary`/`OperationDetail`、`EventRecord`、分页 cursor、`Revision`、
  `IdempotencyKey` 与命令接受结果。
- errors：`capability_required`、`permission_denied`、`revision_conflict`、
  `idempotency_conflict`、`operation_not_found`、`operation_not_cancellable`、
  `event_cursor_expired`、`data_schema_unsupported`、`store_unavailable`。

每项都必须有 JSON Schema、成功/错误 golden snapshot、未知可选字段兼容测试和输入大小上限。
旧 M1 client 在不请求新 capability 时仍可完成 hello/status；M2 server 不得让 `dataSchema` 成为
旧客户端必读字段。M2 不增加 notification、batch、server-initiated request 或新传输。

## transaction boundary

### 创建 Operation

```text
validate/authenticate/authorize
    -> acquire Resource Lock
    -> BEGIN IMMEDIATE
    -> reserve/check idempotency
    -> insert Operation(revision=1, queued)
    -> insert journal prepared step
    -> insert Event(sequence, aggregateRevision=1)
    -> store accepted response
    -> COMMIT
    -> schedule worker
```

### 状态转换/取消

```text
authorize owner/scope
    -> acquire Operation(id) lock
    -> BEGIN IMMEDIATE
    -> compare expectedRevision / validate transition
    -> update Operation + journal
    -> insert Event with new revision
    -> save idempotent result
    -> COMMIT
```

SQLite commit 前不得向客户端报告成功。commit 后 IPC 响应丢失由幂等重试恢复同一结果。长工作、
IPC 写入和锁等待不在 SQL transaction 内执行。

## 允许修改范围

M2 实施获批后可最小修改：

```text
apps/alcomd/                         # store lifecycle、M2 dispatcher、worker/recovery
crates/alcomd-domain/                # Operation/Event/Revision/Principal/ResourceKey 纯类型
crates/alcomd-application/           # M2 用例、ports、权限与锁协调
crates/alcomd-store/                 # SQLite、migration、journal、短事务
crates/alcomd-platform/              # 仅安全 data directory/权限适配
crates/alcomd-protocol/              # 仅批准的 RPC v1 兼容新增 DTO
crates/alcomd-client/                # 仅 M2 类型化调用
crates/alcomd-testing/               # 隔离 store、fault gate、跨进程测试
specs/rpc/                            # M2 method/error Schema 与快照
docs/adr/0004-alcomd-rpc.md
docs/adr/0005-operation-and-lock-model.md
docs/adr/0006-client-permissions.md
docs/adr/0008-native-data-format.md
docs/exec-plans/M2-state-operations-recovery.md
docs/testing/test-plan.toml
docs/status.md
feature-parity.toml
Cargo.toml
Cargo.lock
xtask/src/main.rs                    # 仅依赖/Schema/边界门禁
scripts/validate-metadata.py         # 仅 M2 元数据门禁
scripts/                             # 仅 M2 验收入口
.github/workflows/ci.yml             # 仅接入 M2 测试，不改变平台基线
```

若决定增加最小 CLI 观察入口，必须先把 `apps/alcomd-cli/` 明确加入批准范围；当前草案不依赖 CLI
扩面完成 M2。

依赖方向：

```text
alcomd -> alcomd-application -> alcomd-domain
alcomd -> alcomd-store
alcomd-store -> alcomd-application
alcomd-store -> SQLite dependency
alcomd-protocol / alcomd-client -> public DTO only
```

`alcomd-store` 实现 application 定义的窄 persistence ports；domain 不依赖 SQLite/Tokio/OS，
protocol 不依赖 store/domain，client 不直接打开数据库。

## 明确排除范围

- 项目、仓库、VPM、Unity、模板、备份、包缓存和任何项目文件写入（M3-M5）。
- Plan/Apply 业务 ChangeSet、文件事务、ZIP、下载和双 manifest 提交（M4）。
- 完整 CLI 命令面、NDJSON、progress、completion 与自动化合同（M5）。
- Extension API/WASI、GUI、MCP、Discord、Loopback API 与公共 SDK 硬化（M6-M10/Post-v4）。
- v3 migration、bootstrap/updater、安装器、签名、dist 和零残留（M11-M12）。
- 通用工作流引擎、任意 operation kind 注册、分布式锁、消息总线、ORM、连接池或 event sourcing
  框架。
- 外部客户端配对/credential 发行、扩展 Principal 和 MCP token 实现。
- `settings.toml`、object store 和 OS credential store；M2 只实现 `state.db`。
- Windows 10/11 真实客户端安装运行验证；继续 deferred 到 M12。

## 生产依赖审批点

M2 规划阶段不新增任何依赖。实施前至少需要项目所有者审批：

1. **SQLite crate**：建议优先评估固定版本 `rusqlite` + `libsqlite3-sys` bundled SQLite。需报告
   精确版本、feature、SQLite 版本、许可证、MSRV、维护状态、C toolchain/二进制体积、三平台
   构建、系统 SQLite 替代方案和 Cargo.lock diff。SQLx/ORM/连接池不是默认方案。
2. **数据目录/Windows API**：优先复用已批准 `windows-sys`；若需新增 feature 或单一平台目录
   crate，需报告精确用途和替代方案。不得通过环境显示名或外部命令猜测路径。
3. **Tokio feature**：store worker/channel 与 lock coordinator 若需要现有 Tokio `sync` feature，
   可在审批时作为最小 feature 变化列出；不得启用 `full`。
4. **测试故障注入**：优先使用 `cfg(test)` hooks 和子进程终止，不引入生产 failpoint framework。

任何额外 production crate、unsafe 扩大、C FFI 直调、RPC major 变化或公共权限名称变化都必须
停止审批。

## 测试与验收

### 单元/合同测试

- migration `0 -> 1`、重复启动、较新 Schema 拒绝、migration 中途失败完整回滚。
- 每张表约束、foreign key、大小上限、非法状态/revision/时间拒绝。
- Operation 全状态转换表、终态不可变、取消/完成竞态、revision 单调性。
- idempotency scope、相同/不同 fingerprint、pending/complete/expiry 边界。
- Resource Key canonical ordering、同 key 串行、不同 key 并行、等待取消。
- RPC DTO/Schema、capability 协商、M1 client 忽略 `dataSchema`、全部稳定错误快照。
- Event sequence、分页、重复读取和游标边界。

### 集成测试

- 隔离 data root 启动 daemon，hello -> `state.check` -> operation get -> events list。
- 两客户端同 idempotency key 的请求只创建一个 Operation；不同 Principal 不共享结果。
- cancel 的 stale/current revision、重复 cancel、cancel/complete race。
- 同资源 100 个并发 command 串行，不同 Operation key 并行且无死锁。
- 响应发送前断开后，以同 key 重试取得原 Operation。
- daemon 重启后查询 Operation/Event/result，不依赖原连接或 client instanceId。
- 正常路径只创建批准的 data directory/SQLite 文件，不写项目、VCC、v3 或真实用户目录。

### 故障注入

逐个边界强制失败或终止子进程：

1. migration 建表前/中/设置 `user_version` 前；
2. idempotency reserve、Operation insert、journal prepare、Event insert、commit 前后；
3. worker 开始、状态变 running、检查中、结果/event commit、响应发送前后；
4. cancel intent 与 worker 完成竞态；
5. WAL/数据库只读、磁盘满模拟、损坏/截断 fixture、权限错误；
6. daemon kill/restart 多次，验证无重复 Operation、无虚假 succeeded、sequence/revision 单调。

测试应比较数据库逻辑快照和公开 RPC 结果，不能仅断言“进程未崩溃”。不对真实用户数据库做
破坏性测试。

### 三平台验收

- Windows Server 2025 hosted：真实 data directory override、SQLite/WAL、IPC、并发、强制结束
  daemon/recovery。它仍不代表 Win10/Win11 客户端发行验收。
- Ubuntu 22.04：真实权限、WAL、锁、kill/restart；继续要求最高 `GLIBC <= 2.35`。
- macOS 15 arm64：Application Support 规则的隔离等价路径、权限、WAL、kill/restart；继续验证
  arm64/minos 11.0。
- 三个平台继续执行 M0/M1 setup/check/test、release binaries、Tauri no-bundle、三锁文件与
  final diff 门禁。

## 机器可读测试计划

M2 至少关联：

- `state.sqlite-migrations`
- `state.transaction-boundaries`
- `operations.lifecycle`
- `events.replay-sequence`
- `state.revision-idempotency`
- `state.resource-locking`
- `state.crash-recovery`
- `access.principal-revocation`

`access.principal-revocation` 在 M2 只验证核心 Principal/permission/owner/revocation 语义；外部
应用、扩展与 MCP 的真实 credential/enrollment 集成仍由对应后续里程碑补齐。

## Release blocker 与风险

- **SQLite 供应链/ABI**：bundled 与 system SQLite 影响安全更新、体积和平台一致性，必须先审批。
- **虚假异步**：同步 SQLite 不能阻塞 Tokio executor；使用单 worker，不用随意 `spawn_blocking`
  扩散连接所有权。
- **事务过长**：不得在 transaction 内运行完整性检查、等锁或等客户端。
- **幂等泄漏**：scope 必须包含 Principal；fingerprint/result 不保存敏感原始输入。
- **事件无限增长**：M2 低频且不清理；在 M3 产生高频事件前必须冻结 retention/compaction。
- **恢复误判成功**：未知 kind/journal 不一致一律安全失败并保留 diagnostic ID。
- **同用户威胁边界**：内建 local-owner 不是外部客户端身份隔离；在外部写入口前必须完成配对与
  credential 合同。
- **过度抽象**：只实现 `state.check`、两个 Resource Key 和显式状态机；不预建未来业务引擎。
- **Windows 证据误用**：hosted Server 只证明构建/运行测试，真实 Win10/11 发行验证仍在 M12。

## 人工审批点

实施前必须一次性审批：

1. Schema v1 的最终 SQL、SQLite pragma、migration/较新版本拒绝策略。
2. `state.check` 作为 M2 最小真实 Operation kind 及其安全结果。
3. Operation 状态转换、`interrupted/recovering`、取消竞态与结果持久化边界。
4. Event sequence、分页、M2 不清理及未来 `event_cursor_expired` 合同。
5. revision/expectedRevision 和 idempotency key/fingerprint/建议 24 小时 TTL。
6. `StateStore`/`Operation(id)` Resource Lock 与进程内 RAII 模型。
7. built-in local-owner Principal、四个最小权限名及 M2 同用户威胁限制。
8. RPC v1 method/capability/DTO/error 名称和可选 `dataSchema`。
9. 精确 SQLite dependency/feature/lockfile diff，以及任何平台路径/Tokio feature 变化。

批准这些合同后仍须 contract-first：先更新 ADR/RPC Schema/migration snapshot 和合同测试，再写
生产实现。偏离决定、增加生产依赖、扩大 unsafe 或进入 M3 都必须重新停止审批。

## 与 M1、M3、M12 的关系

- M2 复用 M1 的 endpoint、framing、hello 与按需启动；只做 RPC v1 兼容增加。
- M2 使 `alcomd` 第一次成为持久状态唯一写入者，但不写 Unity 项目。
- M3 项目/仓库只读切片复用 state store、revision 和 Event；M3 不应再另建数据库或事件总线。
- M4 包事务复用 Operation/Lock/journal 基础，但文件 fsync/rename/rollback 必须另行冻结，不能把
  M2 的 DB-only recovery 直接宣称足够。
- M12 仍负责安装布局、data directory 升级/卸载、真实 Win10/11、备份恢复与完整产品发行。

## M2 完成后的停止条件

1. 全部合同、migration snapshot、本地测试、故障注入和三个 hosted CI 通过。
2. 更新 ExecPlan、状态、feature parity 与测试元数据，只标记实际完成的 M2 slice。
3. 提交并推送最终候选，确认 HEAD、`origin/main` 与 CI head SHA 一致且工作树干净。
4. 状态设为“M2 等待人工验收”；不得创建或执行 M3 生产实现。
5. 项目所有者人工审查 Schema、事务、恢复、Principal、依赖与跨平台证据后，明确批准才能进入
   M3 规划或实现。

## 进度日志

- 2026-08-18：M1 已通过人工验收。创建本 M2 ExecPlan 草案；未修改 RPC Schema、migration、
  Cargo 依赖或生产代码，M2 生产实现尚未获批。
