# 项目状态

最后更新：2026-08-14

## 当前阶段

`M-1：只读审计、基线冻结与开放决策`

## 已完成

- 稳定命名与产品身份配置。
- Rust Workspace 骨架。
- Tauri 2 + React + Vite GUI 壳。
- CLI、MCP、API、Extension Host、Bootstrap 与迁移程序占位。
- MCP 管理与 Discord 第一方扩展清单。
- 架构、ADR、契约和 Codex 工作规则骨架。
- CI、Dependabot、初始化和检查脚本骨架。
- 旧内部标识扫描 `cargo xtask check`。

## 尚未开始

- v3 功能审计。
- vrc-get 基线冻结。
- IPC 与 daemon。
- SQLite、Operations、Events、Locks。
- VPM、项目、模板与备份。
- Extension Host 和 WASM。
- MCP 实现。
- Discord IPC。
- v3 迁移与 Bootstrap。
- 安装器、签名与发布。

## 当前阻塞

- 最终许可证未决定。
- v3 与 vrc-get commit 尚未冻结。
- v3 真实安装快照和迁移 Fixture 尚未建立。
- 发布平台矩阵尚未确认。
- MCP 长任务映射尚未决定。

## 下一停止点

完成 `docs/exec-plans/M-1-audit.md` 的全部产物，经人工审核后停止。不得直接进入 M0。
