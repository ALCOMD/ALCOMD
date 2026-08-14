# ADR: Operation 与资源锁

- 状态：Accepted
- 日期：2026-08-14

## 背景

参见 `docs/architecture/ALCOMD-V4.md`。


## 决策

耗时任务使用可查询、可取消、可恢复的 Operation；写操作通过分层资源锁协调。

## 结果

客户端退出不默认取消任务，同项目写入串行，不同项目可并行。
