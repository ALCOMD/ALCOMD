# 项目状态

最后更新：2026-08-16

## 当前阶段

`M-1：只读审计、基线冻结与开放决策（进行中，未完成）`

## 已完成

- 稳定命名与产品身份配置。
- Rust Workspace 骨架。
- Tauri 2 + React + Vite GUI 壳。
- CLI、MCP、API、Extension Host、Bootstrap 与迁移程序占位。
- MCP 管理与 Discord 第一方扩展清单。
- 架构、ADR、契约和 Codex 工作规则骨架。
- CI、Dependabot、初始化和检查脚本骨架。
- 旧内部标识扫描 `cargo xtask check`。
- 已接受 ALCOMD3 3.4.0 作为唯一直接迁移入口；更早 v3.x 必须先升级到 3.4.0。
- `source-lock.toml` 已锁定 v3 审计源、v3.4.0 迁移入口版本、GitHub Release 安装与 updater/签名资产、updater 公钥指纹、vrc-get 功能行为基线和 MCP `2026-07-28` 规范；`freeze-baselines.ps1` 可通过远端引用和 Release API 确定性生成并校验该文件。
- v3.4.0 Release 当前 `immutable = false`；锁文件以 release/asset ID、大小和 SHA-256 建立可检测篡改的快照，任何远端资产变化都会使 `freeze-baselines.ps1 -Check` 失败。
- `docs/baselines/vrc-get.md` 已恢复为独立的功能、安全与行为基线；明确 vrc-get 是必须覆盖的验收来源，但不是代码上游。
- ALCOMD3 v4 已明确为 ALCOMD 产品家族中的独立新项目：继承 v3 的用户品牌与功能定位，但不复制、移植或改写 v3、vrc-get 或 vrc-get-vpm 源码，VPM 独立实现。
- v4 自有代码、SDK、规范、文档、脚本与第一方扩展统一采用 `AGPL-3.0-only`；完整许可证与第三方边界已经落盘。

## 尚未完成

- v3 与冻结版本 vrc-get 的细粒度功能审计；`feature-parity.toml` 当前仍是领域级种子清单，尚无完整用户入口与证据覆盖。
- IPC 与 daemon。
- SQLite、Operations、Events、Locks。
- VPM、项目、模板与备份。
- Extension Host 和 WASM。
- MCP 实现。
- Discord IPC。
- v3 迁移与 Bootstrap。
- 安装器、签名与发布。

## 当前阻塞与缺口

- v3 真实安装快照和迁移 Fixture 尚未建立。
- `feature-parity.toml` 尚未拆分到页面、菜单、vrc-get CLI 与错误行为、MCP、URI、更新器和安装器等用户入口，不能满足 M-1 验收标准。
- v4 迁移桥接安装器（v4 bridge installer）尚未发布；需在 M-1 中基于已上线的新标准更新 API 和 ALCOMD3 3.4.0 实现，冻结其 JSON Schema、版本推进、签名验证与错误契约。
- 发布平台矩阵尚未确认。
- MCP 长任务映射尚未决定。

## 下一停止点

完成细粒度功能审计、证据回填、迁移 Fixture 和 `docs/exec-plans/M-1-audit.md` 的其余产物，经人工审核后停止。当前结果只是 M-1 中途草案，不得进入 M0。
