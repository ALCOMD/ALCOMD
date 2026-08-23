# 项目状态

最后更新：2026-08-24

## 当前阶段

`M5 Backup Create contract-first 已冻结并完成本地合同验收；生产实现尚未批准`

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
- M0 最终提交 `8112415f1dae0dc6f521d5cc3a2c980baac3b408` 已通过 GitHub Actions run
  `32066209115`：Windows Server 2025、Ubuntu 22.04 与 macOS 15 arm64 三个 hosted job
  全部成功；项目所有者已确认 M0 最终验收通过。
- M1 的 RPC v1 帧、envelope、稳定错误、`system.hello`/`system.status` Schema 与兼容规则已经
  contract-first 冻结并通过 Rust/Schema 合同测试；响应不虚构后续 data/config/extension 版本。
- 已实现真实的最小只读垂直切片：`alcomd-platform` 提供每用户安全端点与生命周期实例锁，
  daemon 只经 `alcomd-application` 返回 ready 状态，`alcomd-client` 完成握手和类型化查询，
  `alcomd-cli system status` 支持 human/JSON 与 `--no-start-daemon`。
- Windows 本机已动态通过当前用户 SID Named Pipe/DACL、单实例、并发 client、帧错误、CLI
  human/JSON 与两个 CLI 并发按需启动测试；最终只产生一个权威 daemon，测试进程已清理。
- Unix 实现使用获批的 `rustix 1.1.4` safe API 完成有效 UID、逐组件 no-follow、0700/0600、
  fd-based 类型/所有权校验、非阻塞独占 flock 与 stale socket 恢复；Linux 与 macOS target
  编译检查及各自 hosted 环境中的真实运行测试均已通过。
- Windows FFI 仅存在于私有 `windows_security.rs`，所有 unsafe block/impl 有 SAFETY 说明；
  xtask 禁止其他文件使用 unsafe 或新增 allowance。Cargo.lock 只新增获批的 `rustix 1.1.4` 和
  `linux-raw-sys 0.12.1`，复用现有 bitflags/errno/libc/windows-sys。
- M1 实现提交 `e509554af6cb1029f4a023e26013b495c0a56ffe` 已通过 GitHub Actions run
  `32124358425`：Windows Server 2025、Ubuntu 22.04 与 macOS 15 arm64 三个 hosted job 全部
  成功；Linux 实测最高 `GLIBC_2.34`，九个 macOS Mach-O 均为 arm64 / minos 11.0，三平台
  三锁文件与最终 diff 门禁均通过。Windows hosted 结果仍不代表 Win10/Win11 客户端发行验收。
- 项目所有者已确认 M1 最终提交 `7ed70626a0176f855a8e9efdc6d35d317f51ca78` 与 GitHub Actions
  run `32126344788` 人工验收通过；HEAD、`origin/main` 与 CI head SHA 一致，M1 正式完成。
- M2 已按冻结合同实现 `state.db` Schema v1、单连接 SQLite worker、`state.check`、Operation/
  Event/Revision/永久幂等、两个 Resource Key、恢复 journal、五个兼容 RPC 方法与三项 capability；
  daemon 在 store 初始化和恢复完成后才 bind，启动失败保持 fail closed。
- Windows 正式数据目录通过获批的私有 `windows_known_folder.rs` 使用
  `SHGetKnownFolderPath(FOLDERID_LocalAppData)`；COM 初始化和返回内存由局部 RAII 平衡，xtask
  将 unsafe 硬限制在两个已批准 Windows 文件。没有新增 crate 或额外 windows-sys 版本。
- M2 聚焦验证已覆盖 Schema/migration 回滚、幂等与 Revision 冲突、Principal owner 隔离、
  Event/Operation 分页、取消竞态、100 并发命令、journal 不一致安全失败、真实 IPC 垂直切片及
  子进程强制终止/重启恢复。完整本地 `check.ps1`、`test.ps1`、冻结基线、跨目标 platform
  compile 与差异门禁均已通过。
- M2 最终提交 `9076574ef0f4d3de8690865dfb18aa5856d7ad64` 对应 GitHub Actions run
  `32144082427`：Windows Server 2025、Ubuntu 22.04 与 macOS 15 arm64 三个 hosted job
  全部成功；Ubuntu 实测最高 `GLIBC_2.34`，macOS 预期产物均为 arm64 / minos 11.0。
- 首轮 Windows hosted 测试发现 SQLite worker shutdown 与测试目录清理之间的生命周期竞态；
  最终提交已改为在最后一个 state store handle 释放时确定性关闭并回收 worker，随后重新通过
  Windows 完整测试。项目所有者已确认 M2 人工验收通过，M2 正式完成。
- M3 已冻结 ADR 0016、RPC/storage Schema v2、项目/仓库权限与稳定错误；实现 exact/parent 项目
  发现、VPM/UPM/ProjectVersion bounded 读取、local/anonymous HTTP(S) repository 读取、平台对象
  identity、Schema v2 registry、revision/Event/永久幂等、304/no-op 与 last-known-good 语义。
- M3 已接通 daemon/application/store/vpm/protocol/client/CLI 的真实只读垂直切片；synthetic IPC
  测试确认读取前后项目与 repository 源文件字节不变。完整 v3 parity、repository import/deep-link、
  credential、SemVer/resolver、package 下载/安装与项目写入仍未实现。
- M3 直接 HTTP 依赖保持精确 `reqwest 0.13.4`、`default-features = false`、仅 `rustls`；feature
  graph 证明确认 quinn/HTTP3、system-proxy、cookie 和压缩未激活。Cargo.lock 中可存在获批的
  optional/target-only 锁定项，但不将它们描述为 ALCOMD 当前运行时组件。
- M3 Windows 本机 `check.ps1`、`test.ps1`、Tauri no-bundle、冻结基线与锁文件门禁已经通过；
  最终提交 `2082b5596d246975ca7a48dab20826899103e03d` 对应 GitHub Actions run
  `32174028968` 的 Windows Server 2025、Ubuntu 22.04 与 macOS 15 arm64 job 全部成功，且
  CI head SHA、`HEAD` 与 `origin/main` 一致。项目所有者已完成人工验收，M3 正式完成。
- M4 已按 contract-first 合同实现 State Schema v3、确定性 VPM range/resolver、不可变 Plan/
  ChangeSet、stale/source revalidation、SHA-256 content cache、bounded hostile ZIP extraction、项目级
  Resource Lock、install/remove 文件事务、持久 filesystem journal、阶段 progress 与重启恢复。
- `semver 1.0.28` 是唯一 Version model；私有最小 range AST 只覆盖冻结 vectors，并使用
  `Version::cmp_precedence`，不使用 Cargo `VersionReq`/`Comparator`。`zip 8.6.0`、`sha2 0.11.0` 与
  `unicode-normalization 0.1.25` 保持获批的精确最小 features。
- 本机离线测试已证明真实 RPC `Plan -> Apply -> Operation` 安装/卸载、强制终止子进程后的同一
  Operation 重启恢复、append-only journal 合法重复阶段、脱敏持久 progress，以及同项目串行/
  不同项目并行的锁语义。恢复测试发现并移除了错误的 phase/state 唯一约束；journal 仍以 step 为
  主键且禁止 update/delete。
- 补充真实进程测试保留 `archive_ready`，并覆盖 `prepared`、旧包已 rename 到 backup、新包已
  publish、VPM manifest 已 atomic replace、`filesystem_committed` 后五个关键 checkpoint。每例均
  在 durable test evidence 后强制终止 daemon，重启后复用原 OperationId/Plan/幂等键且不重新 Plan；
  最终 package tree 与 manifest 收敛完整新状态，原 Apply 重放返回同一 OperationId。test gate 不
  增加公开 RPC/Schema 或生产 failpoint framework。
- `feature-parity.toml` 引用的四个 M4 test ID 均已存在并绑定真实 evidence；metadata gate 永久拒绝
  空、重复或不存在的 feature test reference，并要求 `implemented` M4 test 的 evidence path 存在。
  integrity/cache 与 concurrency 描述已收敛到实际离线/本地测试证据，不把已排除的攻击性网络、
  凭据传播或公网故障测试描述为完成。
- M4 保持 SHA-256-required remote archive 安全子集和 `Packages/manifest.json` byte-for-byte 不变；
  hashless VPM、legacy cleanup、credential、local user package、完整 GUI/MCP 与 M5 CLI 体验仍未完成，
  因此完整 `packages.vpm`、`packages.transaction-safety` 与 `packages.security` 只标记 `in_progress`。
- M4 最终代码候选 `cd125da1ff4609dd34bf893ad193e7034fd91674` 对应 GitHub Actions run
  `32274892596` 的 Windows Server 2025、Ubuntu 22.04 与 macOS 15 arm64 job 全部成功；Ubuntu
  实测最高 `GLIBC_2.34`，九个 macOS 预期产物均为 arm64 / minos 11.0，三平台锁文件与最终 diff
  门禁均通过。首轮 CI 暴露的 macOS directory fsync `EINVAL` 平台差异和 Windows 测试临时路径
  时间戳碰撞已在最终代码候选中修复；Ubuntu 前两次重跑停滞于系统软件源，第三次未复现且完整通过，
  未修改 baseline、CI 或安全门禁以规避外部故障。
- M4 最终补充验收提交 `20a86d674b480981d269088cf0615ffdcd9b8e70` 对应 GitHub Actions run
  `32289522274`：Windows Server 2025、Ubuntu 22.04 与 macOS 15 arm64 全部成功；Ubuntu 实测最高
  `GLIBC_2.34`，macOS 所有预期产物均为 arm64 / minos 11.0。真实 filesystem kill/restart matrix
  覆盖 `archive_ready`、`prepared`、旧包 backup rename、新包 publish、VPM manifest atomic replace
  与 `filesystem_committed`，均复用原 OperationId、Plan 和幂等键恢复且不重新 Plan。metadata
  test-reference gate 已永久加入。项目所有者已确认最终人工验收通过，M4 正式完成。
- M5 CLI contract v1 已冻结 human/JSON/NDJSON、stdout/stderr、`0/1/2/3/130` 退出码、alias、
  `--yes`/`--dry-run`/`--no-wait`、TTY/EOF、Ctrl-C detach、Operation 分离与静态 completion；命令
  catalog 只有后端 capability 真实实现后才允许发布，CLI 继续只能经 `alcomd-client`/RPC。
- M5 Unity contract 已冻结 installation registry、project editor preference、bounded argv、launch/
  observation、四态 writer evidence、hard/advisory gate、RPC capability/error 与最小权限。State Schema
  v4 已接入 v3→v4 自动 migration，daemon/client 已广告并实现十个 Unity method 与三项 capability。
- 项目所有者已批准 `sysinfo 0.39.6`（defaults off，仅 `system`，官方 MSRV Rust 1.95）。依赖只进入
  `alcomd-platform`，最小 refresh 显式 `without_tasks()` 且只请求 exe/cmd；没有启用 rayon、增加
  ALCOMD unsafe 文件或平台 API。fake provider、真实短生命周期子进程和并行观察测试已接入。
- Unity 最小生产切片已实现 synthetic Editor 校验、手工/known-root registry、project Editor preference、
  PID+start-time writer evidence、无 shell 独立 argv 启动、永久幂等 launch/status 与真实 daemon RPC
  往返。完整 Hub 格式、真实 Unity/Hub parity、迁移/foreground 和完整 CLI 表面仍未完成。
- Unity production slice 已作为独立提交 `8b63c6923b178a6ebb12bd5964412b2db7268e04` 保存。M5
  Template contract-first 已冻结原生 `.alcomdtemplate` v1、独立 archive quota、三个 native builtin
  inventory/AGPL provenance、Schema v4 registry compatibility、RPC/permission/error、已发布 CLI、synthetic
  Fixture 和 contract/security/migration snapshot。项目所有者已批准 Schema v5 的窄 immutable
  `template_plans` authority 与三个精确 Template Operation kind，并批准闭环通过后直接实施 production。
  Template capability、registry/import/export/derive/create-project、M4 frozen staging package adapter 与
  RPC/CLI 已真实接入。create-project 的六点真实 daemon kill/restart matrix 已复用原
  OperationId/Plan/idempotency 收敛成功；v3 `.alcomtemplate` parity 继续 blocked 到 M11。
- M5 Backup Create contract-first 已冻结 `ALCOMD Backup Archive v1`、严格 `backup.json`、ZIP64
  Stored/Deflate、64 GiB/500,000 entries/32 GiB single file/128 GiB uncompressed/depth 128/path 1,024
  bytes/ratio 10,000:1 配额，以及 Logs/Obj/Temp、任意 `.git`、根 `Library*` 与唯一直接文件例外的精确
  排除表。locked VPM 排除不猜测、不联网，manifest 与 unknown/unlocked/embedded child 保留。
- State Schema v6 只增加严格 Operation kind `backups.create`，原子重建并保留 operation journal、
  idempotency、package plan/filesystem journal 与 template plan 外键依赖；现有 `backups` 表保持不变。
  Backup RPC/permission/error/recovery phase 与 planned CLI catalog 已冻结，但 dispatcher/client/CLI help/
  worker 均未发布，`backups.m5-create` 继续 planned，完整 backups feature 保持未实现。
- CLI 高影响确认现在由窄本地 `CliError::ConfirmationRequired` 分类；非 TTY/EOF/拒绝以退出码 1 和
  `confirmation_required` 返回，不再被错误映射为 `daemon_unavailable`，且没有改动 RPC/daemon。

## 后续里程碑尚未完成

- v3.4.0 完整安装后快照、脱敏迁移 Fixture 和 GUI 冻结截图/流程已由项目所有者明确后移到
  M11；M-1 仅保留 VM 操作报告，不把它升级为实例证据或删除授权。
- MCP 33 个 v3 用例的 M-1 工具合同基线已形成并获 A-026 批准；正式 Schema、快照、兼容
  别名策略和协议实现留在对应后续里程碑。
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
- `alcomd-mcp`、GUI 与其他后续入口仍是 scaffold；M1 只实现 daemon/client/CLI 的最小 status
  切片，不得将其描述为完整 RPC、CLI、Operation 或业务功能。
- `projects.v3-parity` 继续保持 `blocked`，直到 M11 建立脱敏真实 v3 Fixture；synthetic/public
  Fixture 只能证明独立实现的格式与工程行为，不能冒充真实 v3 differential evidence。
- 真实 credential enrollment/revocation 仍未完成；完整 `projects.management`、
  `repositories.management`、`daemon.single-writer`、`rpc.local` 与 `cli.complete` 仍只实现了各自
  的阶段性切片，必须保持真实的 `in_progress`/`planned` 状态。
- `templates.v3-parity`、`backups.v3-parity` 与 `unity.v3-vrc-parity` 继续 blocked；M5 Unity engineering
  测试只证明 synthetic Editor、进程观察和 RPC/migration 行为，不证明真实 Unity/Hub、模板、备份
  或 v3 differential parity。
- M4 已批准并精确锁定 `semver 1.0.28`、`zip 8.6.0`、`sha2 0.11.0` 与
  `unicode-normalization 0.1.25`；normalization 许可证记录保持
  `(MIT OR Apache-2.0) AND Unicode-3.0`。`semver` 只提供 Version/precedence，冻结 VPM range 语义由
  `alcomd-vpm` 私有最小 parser/matcher 实现；`nodejs-semver` 与 `node-semver` 未进入生产依赖。

## 下一停止点

M0、M1、M2、M3 与 M4 均已完成并通过最终人工验收。M5 Slice 0/1 CLI/Unity contract-first 工件已
冻结，`sysinfo` 依赖与 Unity 最小生产垂直切片已接入并通过 Windows 本地完整验收；Template
contract-first、Schema v5 closure 与 production/RPC/CLI slice 已完成本地验收。Backup Create
contract-first、Schema v6/RPC/permission/error/archive profile 已冻结并完成合同测试；当前等待 production
实现审批，尚未开始 Backup worker。M4 完整
VPM 产品功能以外的未完成范围继续按 feature/test 元数据推进，不因里程碑验收而虚构为 implemented。
`projects.v3-parity` 与真实 credential
revocation 仍未完成；Windows 10/11 完整客户端
安装、启动、WebView2、更新与卸载验证继续 deferred 到 M12，不得把 Windows Server hosted 结果
描述为客户端发行证据。
