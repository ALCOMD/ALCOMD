# ADR: ALCOMD 原生数据规范

- 状态：Accepted
- 日期：2026-08-14

## 背景

参见 `docs/architecture/ALCOMD-V4.md`。


## 决策

v4 使用 `settings.toml`、SQLite、对象库与 OS 凭据库。VCC 与 v3 格式不作为运行时状态。

## 结果

数据、配置、RPC、扩展和导出格式分别版本化。
