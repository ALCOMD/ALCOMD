# M2：SQLite 权威状态、Operation、Event、Revision、幂等、资源锁与恢复

状态：M2 实现、本地验收、三平台 hosted CI 与人工验收全部完成；尚未进入 M3

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

- `state.check`：创建可恢复 Operation，只读执行有严格输出数量/大小上限的
  `PRAGMA integrity_check` 与 `PRAGMA foreign_key_check`。它不执行 repair、`VACUUM`、`REINDEX`
  或隐式修改，公开结果只返回安全分类，不返回原始 SQL、路径或完整 SQLite 错误文本。
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
    state              TEXT NOT NULL CHECK (state IN (
        'queued', 'planning', 'waiting_for_input', 'running', 'cancelling',
        'succeeded', 'failed', 'cancelled', 'interrupted', 'recovering'
    )),
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
) STRICT;

CREATE TABLE operation_journal (
    operation_id   TEXT NOT NULL REFERENCES operations(operation_id),
    step           INTEGER NOT NULL,
    kind           TEXT NOT NULL,
    state          TEXT NOT NULL CHECK (state IN ('prepared', 'applied')),
    payload_json   TEXT NOT NULL,
    updated_at_ms  INTEGER NOT NULL,
    PRIMARY KEY (operation_id, step)
) STRICT;

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
) STRICT;

CREATE TABLE idempotency_records (
    principal_id        TEXT NOT NULL,
    method              TEXT NOT NULL,
    idempotency_key     TEXT NOT NULL,
    request_fingerprint TEXT NOT NULL,
    state               TEXT NOT NULL CHECK (state IN ('pending', 'completed')),
    operation_id        TEXT REFERENCES operations(operation_id),
    response_json       TEXT,
    created_at_ms       INTEGER NOT NULL,
    PRIMARY KEY (principal_id, method, idempotency_key)
) STRICT;

CREATE INDEX operations_owner_page
    ON operations(owner_principal_id, created_at_ms DESC, operation_id DESC);
CREATE INDEX events_principal_sequence
    ON events(principal_id, sequence);
```

最终 migration 还必须对 revision、sequence、step 等正整数执行 SQL CHECK，并在 Rust 入口做
相同 signed-`i64` 范围校验。必需索引只覆盖 Operation owner 的稳定分页和 Principal Event
sequence 查询。M2 不预建
项目、包、仓库、设置、扩展、审计或全文搜索表。

JSON 字段是版本化的内部持久 DTO，不是第三方 API。每个字段有严格大小上限并用类型化结构
读写；禁止把任意 SQL、堆栈、token、完整私密路径或未经筛选的内部错误写入 Event/结果。

### SQLite 运行策略

- daemon 先取得生命周期绑定的每用户 OS 单实例锁，再打开 store；初始化与恢复完成后才 bind
  IPC endpoint，避免并发按需启动短暂产生两个 SQLite owner，也不在 ready 前接受请求。
- 一个专用 store worker 拥有一个 `rusqlite` connection，通过有界 channel 接受短请求；
  不引入连接池、ORM、async SQL 宏或通用 repository framework。
- 打开后先读取 `user_version`；若高于当前支持版本立即 fail closed，不执行 migration 或写入。
- 对受支持数据库设置并验证 `foreign_keys=ON` 与 `journal_mode=WAL`，设置
  `synchronous=FULL`，`busy_timeout=5000 ms`。
- `0 -> 1` 使用单个短 transaction，`user_version=1` 是 transaction 中最后的 Schema 版本
  写入；失败完整回滚。
- store 初始化或 migration 在 ready 前失败时 daemon fail closed，不建立 half-ready/degraded
  模式，也不改变 M1 `system.status state="ready"` 的成功语义。
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
queued -> running -> cancelling
           │            │
           ├────────────┤
           ▼            ▼
     succeeded | failed | cancelled

planning / waiting_for_input：仅保留名称，不是 M2 state.check 动态路径

queued crash -> queued -> rescheduled
running/cancelling/recovering crash -> interrupted -> recovering
```

- `succeeded`、`failed`、`cancelled` 是不可变终态。
- queued 重启后保持 queued 并直接重新调度，不产生虚假的 interrupted。
- running、cancelling 或 recovering 重启后才进入 `interrupted`，随后进入 `recovering`；两次
  公开变化分别增加 revision，并与 Event 在同一 transaction 提交。
- `cancelling` 表示取消意图已接受，不保证最终一定为 `cancelled`；完成竞态可合法到
  `succeeded`/`failed`。
- M2 的 `state.check` 动态路径只需要 queued/running/cancelling/终态/interrupted/recovering。
  全部状态转换规则在领域单元测试冻结；`waiting_for_input` 的公开 input/approve/reject/resume
  方法等第一个真实 Plan/Apply 用例出现时再冻结，不在 M2 预建通用审批引擎。
- 取消是 cooperative：进入检查前、`integrity_check` 与 `foreign_key_check` 阶段之间及检查后
  检查 `cancel_requested`。M2 不承诺中断正在执行的单条 SQLite pragma，也不引入 interrupt
  framework；取消/完成竞态可合法收敛到 succeeded、failed 或 cancelled。
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
  100、最大 1000。`nextSequence` 为本页最后一个 Event 的 sequence；空页时等于输入
  `afterSequence`。下一页直接回传它，绝不能使用 `lastSequence + 1`，因为 sequence 允许空洞。
- 客户端断线后使用最后已处理 sequence 重试；重复 Event 必须可安全去重。
- M2 不实现 server-initiated notification 或无限流，避免破坏 M1 传输假设。后续可兼容增加
  live subscription，但必须仍以持久 sequence 为恢复事实。
- M2 不删除 Event、不实现 retention。RPC 仅保留未来 `event_cursor_expired` 错误名称，M2 不得
  声称或测试当前实现能够产生该错误。

`operations.list` 固定按 `(createdAtMs DESC, operationId DESC)` 排序。opaque cursor 至少携带
最后一项的 `createdAtMs + operationId`，客户端只应原样回传；下一页使用严格 tuple comparison，
避免相同时间戳重复/遗漏。新 Operation 插入不改变已经继续向后的分页边界。M2 不建立通用
pagination framework。

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
- M2 不自动删除 idempotency record，key 不因时间经过而重新可用；Schema v1 不包含
  `expires_at_ms`。retention、expiry 与 key reuse 等第一个高频写用例出现时重新审批。
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
  1. queued 保持 queued 并重新调度；
  2. running/cancelling/recovering 原子进入 `interrupted` 并写 Event，再进入 `recovering` 并写
     第二个 Event；
  3. `state.check` 可安全重跑；已持久 cancel_requested 的 Operation 进入恢复后收敛；
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
- 内建 Principal 固定为 `builtin:local-owner`；最小权限固定为 `state.check`、
  `operations.read`、`operations.cancel`、`events.read`。
- 每个 application 用例再次校验 Principal/permission；RPC adapter 的预检查不构成授权。
- 测试可使用隔离的 synthetic Principal 验证 owner、permission、撤销和幂等隔离，但不把 synthetic
  credential 当成发行机制。

外部应用配对、token 签发、OS credential store、MCP Bearer/STDIO Principal、扩展身份与完整
授权管理 UI 不属于 M2。它们分别在后续里程碑实现；在这些合同完成前，M2 不向任意外部应用
宣传写接口安全。上述 Principal 形态和四个权限名称属于公共合同，实施前必须人工批准。

## RPC v1 兼容新增

M2 只以 M1 已允许的方式增加 method、capability、DTO 和可选字段，不提升 RPC major：

- capabilities：`state.check.v1`、`operations.v1`、`events.replay.v1`。方法只有在本连接 hello
  协商到所需 capability 后才可调用，否则返回 `capability_required`。
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

项目所有者已批准 M2 使用：

1. **SQLite**：`rusqlite = 0.40.1`，关闭默认 feature，只启用 `bundled`；预期
   `libsqlite3-sys 0.38.1` / SQLite 3.53.2。禁止 cache、ORM、连接池和 SQLx。
2. **Tokio**：现有依赖最小增加 `sync`，不得启用 `full`。
3. **数据目录/Windows API**：已批准现有 `windows-sys 0.61.2` 增加
   `Win32_UI_Shell`/`Win32_System_Com`，且只在私有 `windows_known_folder.rs` 中调用
   `SHGetKnownFolderPath(FOLDERID_LocalAppData)` 与所需 COM/内存释放 API。不得通过环境变量、
   注册表、显示用户名或外部命令猜测正式路径，也不得把该边界扩展为通用 COM/Known Folder
   abstraction。
4. **测试故障注入**：使用 `cfg(test)` hooks 和子进程终止，不引入生产 failpoint framework。

任何额外 production crate、unsafe 扩大、C FFI 直调、RPC major 变化或公共权限名称变化都必须
停止审批。

## 测试与验收

### 单元/合同测试

- migration `0 -> 1`、重复启动、较新 Schema 拒绝、migration 中途失败完整回滚。
- 每张表约束、foreign key、大小上限、非法状态/revision/时间拒绝。
- Operation 全状态转换表、终态不可变、取消/完成竞态、revision 单调性。
- idempotency scope、相同/不同 fingerprint、pending/complete 与永久 key 重放边界。
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

`access.principal-revocation` 在 M2 只实现并验证核心 Principal/permission/owner 隔离。真实
credential enrollment 与 revocation 尚未实现，因此该测试整体保持 `planned`，由第一个外部
写入口所在里程碑继续完成。

## Release blocker 与风险

- **SQLite 供应链/ABI**：已固定 bundled SQLite 3.53.2；安全补丁升级需显式更新精确依赖、锁文件
  与三平台证据，不得无意切换为 system SQLite。
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

上述 M2 合同与 SQLite/Tokio 依赖已于 2026-08-18 获批；Windows Known Folder feature 与第二个
私有 unsafe 边界随后获得专项批准。ADR、RPC Schema、最终 migration SQL、状态/恢复/分页/
幂等/权限/事务合同、生产垂直切片、本地测试和三个 hosted 平台的最终候选验证均已通过。
项目所有者已完成 M2 人工验收；尚未进入 M3。

仅在新增 production crate、进一步扩大 Windows feature/unsafe、需要偏离已冻结
Schema/RPC/permission，或进入 M3 时重新停止审批。

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
- 2026-08-18：项目所有者批准按修正后的合同进入 contract-first，并批准固定
  `rusqlite 0.40.1` bundled 与 Tokio `sync`；合同与合同测试通过前不得开始生产实现。
- 2026-08-18：contract-first 门禁通过：migration/schema 5 项、事务/幂等/分页 5 项、RPC
  Schema/golden 8 项、领域状态/恢复/Revision/权限 6 项和 fingerprint 2 项。开始最小
  application persistence port 与 RAII lock coordinator；未实现 store/daemon RPC 垂直切片。
- 2026-08-18：Windows `FOLDERID_LocalAppData` 只读实现研究确认需要新增
  `windows-sys` 的 `Win32_UI_Shell` 与 `Win32_System_Com` feature，并需要新的私有安全 FFI
  边界；按批准条件在修改 manifest/unsafe 前暂停等待人工审批。
- 2026-08-18：项目所有者批准上述两个 Windows-only feature 与第二个私有 unsafe 边界。
  已实现 Local AppData Known Folder/COM RAII、data path、单连接 SQLite worker、五个 M2 RPC、
  capability/permission/owner 校验、幂等/Revision/Event/锁与恢复；Windows 聚焦测试、真实
  IPC state.check、子进程强制终止/恢复、Clippy、xtask 和元数据门禁均通过。最终完整门禁和
  Windows/Ubuntu/macOS hosted 结果尚待执行，未进入 M3。
- 2026-08-18：本地完整 `check.ps1`/`test.ps1`、冻结基线、metadata、diff、Linux/macOS
  `alcomd-platform` cross-target compile 均通过；并发自动启动改为先取得 OS 单实例锁、再打开/
  恢复 SQLite、最后 bind endpoint，测试数据通过隐藏参数落在隔离目录。当前仅待最终差异审查、
  提交推送与三个 hosted job。
- 2026-08-18：首轮 Windows Server 2025 hosted 测试发现最后一个 `StateStoreHandle` 释放后，
  SQLite worker 异步退出与测试目录清理存在生命周期竞态；最终修复使最后一个 handle 确定性
  关闭命令通道并回收 worker，随后重新通过 Windows 完整测试。
- 2026-08-18：最终提交 `9076574ef0f4d3de8690865dfb18aa5856d7ad64` 对应 GitHub Actions
  run `32144082427`。Windows Server 2025、Ubuntu 22.04 与 macOS 15 arm64 三个 hosted job
  全部成功；Ubuntu 实测最高 `GLIBC_2.34`，macOS 预期产物均为 arm64 / minos 11.0。
  项目所有者确认人工验收通过，M2 正式完成；尚未进入 M3。
