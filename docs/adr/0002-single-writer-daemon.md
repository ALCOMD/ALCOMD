# ADR: 每用户唯一核心与唯一写入者

- 状态：Accepted
- 日期：2026-08-14

## 背景

参见 `docs/architecture/ALCOMD-V4.md`。


## 决策

`alcomd` 是每用户唯一核心进程，也是数据库和项目的唯一写入者。

## 结果

GUI、CLI、MCP、API 和扩展只能通过 RPC 提交请求。必须实现单实例、资源锁、事务与断线恢复。
