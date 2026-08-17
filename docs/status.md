# 项目状态

最后更新：2026-08-18

## 当前阶段

`M0：仓库骨架、身份与 CI（完成；停止在 M1 之前等待人工验收）`

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
- 项目所有者已确认冻结 vrc-get 只作为实现与风险参考，不继续对其执行攻击性黑盒或网络
  安全验证；路径、ZIP、事务、来源确定性和凭据边界进入 ALCOMD 自身实现的验收测试。
- 项目所有者已批准 M-1 完成；`feature-parity.toml` 的 `verified` 只表示对应基线、范围和验收
  合同已冻结，所有 `implementation_status` 仍独立反映真实实现状态。
- M0 ExecPlan 已细化为固定工具链、`npm ci`、Cargo lock、三平台 `--no-bundle` build 和独立
  扩展 test gate。
- M0 已补齐纯占位 `alcomd-updater` app 边界；它只报告 scaffold 状态，不包含下载、签名、
  替换或回滚实现。
- `alcomd.product.toml` 现在覆盖 updater 身份；xtask 与元数据验证器校验 Cargo Workspace、
  Tauri、npm、第一方扩展和派生身份，并固定本地 crate 依赖方向。
- PowerShell/Bash setup/check/test 已对齐：Node 安装使用 `npm ci`，Cargo 构建/Clippy/test 使用
  `--locked`，完整 Workspace 不再排除 GUI，独立 Discord backend 增加 test gate，命令执行
  前后校验根 `Cargo.lock`、根 `package-lock.json` 与 Discord backend `Cargo.lock` 摘要。
- 跨平台 CI 已按批准方案配置并实际通过：`windows-2025`、`ubuntu-22.04`、Apple Silicon
  `macos-15` hosted job；所有 job 固定 Rust 1.97.1、Node.js 24、Python 3.11 与 action commit。
- Linux job 安装含 `libgtk-3-dev` 的 Tauri 前提并执行 GLIBC 符号上限检查；macOS job 检查
  arm64 与实际最低部署版本。GitHub Actions run `32056593208` 在提交
  `8a6f2968bdf4212a7a98f0ea55d93cc291883e87` 上通过：Linux 实测最高 `GLIBC_2.34`，九个
  macOS Mach-O 均为 arm64 / minos 11.0，三平台三锁文件 gate 均通过。
- Windows 本机已通过 setup、完整 check/test、Git Bash 语法检查和 Tauri
  `build --no-bundle`；该结果只证明 GUI 子应用可构建，不是完整产品发行验证。

## 后续里程碑尚未完成

- v3.4.0 完整安装后快照、脱敏迁移 Fixture 和 GUI 冻结截图/流程已由项目所有者明确后移到
  M11；M-1 仅保留 VM 操作报告，不把它升级为实例证据或删除授权。
- MCP 33 个 v3 用例的 M-1 工具合同基线已形成并获 A-026 批准；正式 Schema、快照、兼容
  别名策略和协议实现留在对应后续里程碑。
- IPC 与 daemon。
- SQLite、Operations、Events、Locks。
- VPM、项目、模板与备份。
- Extension Host 和 WASM。
- MCP 实现。
- Discord IPC。
- v3 迁移与 Bootstrap。
- 安装器、签名与发布。

## 当前阻塞与缺口

- Windows 10 22H2 与 Windows 11 仍是正式目标支持平台，但真实客户端运行验证尚未完成，
  不得记为通过。项目所有者已将仅重复编译的 M0 self-hosted job 取消，并把验证 deferred 到
  M12：届时必须安装并启动完整产品，覆盖 WebView2 渲染、托盘、注册表、用户数据路径、
  更新器、安装器、升级与卸载。
- GitHub 已宣布 `ubuntu-22.04` hosted runner 从 2026-09-17 开始弃用并于 2027-04-17 退役；
  当前 M0 仍使用该构建基线，未来替代不能直接用 Ubuntu 24.04 冒充 Ubuntu 22.04 /
  `GLIBC_2.35` 等价验证。

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
- 现有 `alcomd-mcp`、daemon、GUI 等仍是 scaffold；M-1 的 `verified` 只表示基线证据和验收
  合同完成，不表示生产功能已经实现、Fixture 已建立或动态验证已经通过。

## 下一停止点

M0 已完成并停止。下一步只能在项目所有者人工验收并明确批准后开始 M1；当前不得实施 M1。
Windows 客户端运行验证留在 M12，迁移删除、永久 Windows AppId/GUID、安装器与
`cargo xtask dist` 仍不在当前授权范围内。
