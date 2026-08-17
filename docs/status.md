# 项目状态

最后更新：2026-08-17

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
- `source-lock.toml` 已锁定 v3 审计源、v3.4.0 迁移入口版本、GitHub Release 安装与
  updater/签名资产、updater 公钥指纹、vrc-get 功能行为基线和 MCP `2026-07-28` 规范；
  `freeze-baselines.ps1` 可通过冻结时分支上下文、不可变 commit API 证据和 Release API
  确定性生成并校验该文件，且不会因上游分支或标签后来前进而使固定 commit 失效。
- v3.4.0 Release 当前 `immutable = false`；锁文件以 release/asset ID、大小和 SHA-256 建立可检测篡改的快照，任何远端资产变化都会使 `freeze-baselines.ps1 -Check` 失败。
- `docs/baselines/vrc-get.md` 已恢复为独立的功能、安全与行为基线；明确 vrc-get 是必须覆盖的验收来源，但不是代码上游。
- ALCOMD3 v4 已明确为 ALCOMD 产品家族中的独立新项目：继承 v3 的用户品牌与功能定位，但不复制、移植或改写 v3、vrc-get 或 vrc-get-vpm 源码，VPM 独立实现。
- v4 自有代码、SDK、规范、文档、脚本与第一方扩展统一采用 `AGPL-3.0-only`；完整许可证与第三方边界已经落盘。
- 已完成冻结 v3.4.0 与 vrc-get commit 的静态细粒度审计，覆盖项目、包/仓库、Unity、模板、
  备份、GUI、CLI、MCP、Discord、更新器、安装器、错误和高风险行为；结论分别记录在
  `docs/baselines/alcomd3-v3-audit.md` 与 `docs/baselines/vrc-get-audit.md`。
- `feature-parity.toml` 已提升到 user-entry 清单，区分“基线审计状态”和“v4 实现状态”；所有
  release blocker 已关联机器可读测试计划，但需要真实 Fixture 的计划仍为 blocked。
- `migrations/v3/artifacts.toml` 已按 R0/R1/R2/C1/N 阶段记录源码确认的路径/身份模板、所有权、
  操作与 residue test；没有真实安装快照的可删除实例全部保持 `confirmed = false`。
- MCP 核心规范已锁到官方 commit、最终 Schema blob/SHA-256 和固定 conformance npm tarball；
  A-021 已决定 4.0.0 使用 OperationId 与显式输入/审批工具，不采用 Tasks；A-023 已冻结无
  session 权限名与 HTTP/STDIO Principal 隔离方向。
- 更新 API 与 v3.4.0 实际接受的 bridge 输入、Minisign 身份、文件名、下载上限和失败语义已
  冻结；M-1 不实现或发布 bridge。
- A-024 与 ADR 0015 已关闭 O-008：ALCOMD 是多组件 Rust 本地应用平台，只有
  `alcomd-gui` 是 Tauri 子应用；Windows 使用单一 Inno Setup EXE 的两种互斥安装模式，
  macOS 使用 DMG，Linux 使用 AppImage 与 DEB。三个平台、四种主要格式均为
  4.0.0 blocker。
- 完整产品发行、三平台 CLI 集成、updater/bootstrap 边界与未来 `cargo xtask dist` 已进入
  `docs/exec-plans/M12-full-product-distribution.md`；本阶段未实现安装器、签名或发行资产。
- A-025 已关闭 O-003：Windows 正式测试 Win10 22H2/Win11 并采用 WebView2 Evergreen 在线/
  离线部署；Linux 在 Ubuntu 22.04 构建且发行 ELF 最高所需符号不超过 `GLIBC_2.35`；macOS
  arm64 deployment target 为 11.0。当前 Authenticode 与 Apple Developer ID/notarization
  不是 blocker，但应用层更新/bridge/扩展签名和摘要验证仍为强制要求。
- A-026 已批准 MCP v4 工具命名基线、33 项功能覆盖而非一对一工具、Plan/Apply、
  `OperationId`、`diagnostics.read` 与稳定结构化错误方向；未知内部错误固定使用
  `internal_error + diagnostic_id`。
- M0 ExecPlan 已细化为固定工具链、`npm ci`、Cargo lock、三平台 `--no-bundle` build 和独立
  扩展 test gate；尚未执行。

## 尚未完成

- v3.4.0 完整安装后快照、脱敏迁移 Fixture 和 GUI 冻结截图/流程已由项目所有者明确后移到
  M11；M-1 仅保留 VM 操作报告，不把它升级为实例证据或删除授权。
- MCP 33 个 v3 用例已形成并获 A-026 批准逐项工具合同方向；正式 Schema、快照与兼容别名
  策略留在对应协议实现里程碑。
- vrc-get 高风险静态推断的独立黑盒确认，包括包 ID 路径穿越、同版本来源 tie、ZIP 链接、
  Windows atomic replace 与跨 origin Authorization redirect。
- IPC 与 daemon。
- SQLite、Operations、Events、Locks。
- VPM、项目、模板与备份。
- Extension Host 和 WASM。
- MCP 实现。
- Discord IPC。
- v3 迁移与 Bootstrap。
- 安装器、签名与发布。

## 当前阻塞与缺口

- 真实安装快照和迁移 Fixture 尚未建立；因此 artifact 模板继续保持 `confirmed = false`，
  迁移删除、GUI/模板/备份/Unity 差异测试仍 blocked。项目所有者已决定不在 M-1 继续投入
  合成状态与安装后采集，完整证据后移到 M11；这不再阻塞 M-1 的其余静态审计收尾。
- v4 bridge 尚无版本/tag/资产、handoff、journal、恢复源、Health Check、Commit marker、DEB
  交接和 rollout 设计。M-1 已冻结输入与失败契约；实现/发布属于 M11，不能在本阶段伪造。
- A-017 至 A-026 已批准发布平台范围、Local API/SDK 后置、第一方扩展默认状态、GUI 等价
  边界、MCP Operation 映射、Extension ABI v1、MCP 管理权限、全产品打包模型与平台技术
  基线和 MCP 工具/诊断/错误方向。O-008 已被 A-024/ADR 0015 替代，O-003 已由 A-025 关闭。
- Tasks SEP 已 Final，但扩展 artifact 仍带 Draft/experimental 标记，且所审错误码与最终 core
  Schema 冲突；在固定兼容版本前保持阻塞。
- `specs/extensions/permissions-v1.md` 与 `specs/mcp/toolset-v1.md` 尚未应用 A-021/A-023；M-1
  允许范围不包含 `specs/`，必须在对应协议里程碑经 Schema/快照更新落地，生产实现不得继续
  使用 `mcp.sessions.read`。
- 现有 `alcomd-mcp`、daemon、GUI 等仍是 scaffold；本阶段的 `verified` 只表示基线证据完成，
  不表示生产功能已经实现。

## 下一停止点

VM 采集已按项目所有者决定停止，且未授予任何迁移删除权限。MCP 逐工具合同方向已由 A-026
批准，M-1 下一项是 vrc-get 高风险静态推断的黑盒边界；迁移实例证据留在 M11。
`feature-parity.toml` 保持
`m1_complete = false`，不得进入 M0；安装器与 `xtask dist` 只能在后续独立 M12 ExecPlan
获批后实现。
