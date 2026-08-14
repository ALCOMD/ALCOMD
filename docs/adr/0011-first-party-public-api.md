# ADR: 第一方扩展只用公开 API

- 状态：Accepted
- 日期：2026-08-14

## 背景

参见 `docs/architecture/ALCOMD-V4.md`。


## 决策

第一方扩展与第三方扩展使用相同 Manifest、权限、UI Bridge、Extension API、宿主和数据隔离。

## 结果

禁止第一方私有 Tauri command、隐藏 IPC 和绕过授权的快捷路径。
