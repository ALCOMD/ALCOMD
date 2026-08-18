# ADR: Operation 与资源锁

- 状态：Accepted
- 日期：2026-08-14

## 背景

参见 `docs/architecture/ALCOMD-V4.md`。


## 决策

耗时任务使用可查询、可取消、可恢复的 Operation；写操作通过分层资源锁协调。

M2 唯一真实 Operation kind 是只读 `state.check`。状态集合固定为 queued、planning、
waiting_for_input、running、cancelling、succeeded、failed、cancelled、interrupted、recovering；
终态 succeeded/failed/cancelled 永不可变。M2 不实现 planning/waiting_for_input 的审批输入引擎。

重启恢复规则：queued 保持 queued 并重新调度；running、cancelling、recovering 依次进入
interrupted 与 recovering。每次公开变化增加 revision 并与 Event 在同一 SQLite transaction
提交。取消为 cooperative，只在进入检查前、两个 SQLite 检查阶段之间及检查后观察；不尝试
中断正在执行的单条 pragma。

M2 Resource Key 仅有 `StateStore` 与 `Operation(operation_id)`。锁是 daemon 进程内 RAII async
exclusive lock；多 key 去重后按 canonical byte order 获取。等待锁时不得持有 SQLite
transaction，guard 不跨用户输入或外部长期等待。crash 后不恢复 stale owner，恢复器依据持久
Operation/journal 重新获取锁。

Operation revision 从 1 开始，既有 Operation 写命令必须携带 expectedRevision；幂等重放和
no-op 不增加 revision。Event aggregateRevision 等于提交后的 revision。公开 u64 在 store 层
限制为 SQLite signed i64 正整数范围。

## 结果

客户端退出不默认取消任务，同项目写入串行，不同项目可并行。
