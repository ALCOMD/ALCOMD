# ADR: ALCOMD RPC

- 状态：Accepted
- 日期：2026-08-14

## 背景

参见 `docs/architecture/ALCOMD-V4.md`。


## 决策

官方组件使用 Named Pipe / Unix Domain Socket 上的版本化本地 RPC。公共 DTO 与领域对象分离。

## 结果

RPC v1 独立于应用版本。破坏性变化提升 RPC 大版本。
