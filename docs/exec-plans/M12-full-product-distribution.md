# M12：完整产品发行、安装与更新

状态：后续计划；M-1 人工批准及前置里程碑完成前不得执行

## 目标

实现由 ALCOMD 自己控制的跨平台完整产品发行流水线，以：

```text
cargo xtask dist --target <target>
```

构建、收集、验证和打包目标平台所需的全部 release-blocker 组件，生成强制的 ALCOMD
应用层签名/摘要，并产出 Windows
Inno Setup EXE、macOS DMG、Linux AppImage 与 DEB。安装、更新、迁移、CLI 集成与卸载必须
遵守 A-024 和 ADR 0015。

ALCOMD 是一个基于 Rust 的本地应用平台，其官方 GUI 使用 Tauri 构建。Tauri 构建
`alcomd-gui`，并可在适合的平台参与打包；这不是全面禁止 Tauri Bundler。它必须作为
`cargo xtask dist` 控制的底层工具，不得定义完整产品组成、安装生命周期、更新或迁移。

## 前置条件

- M-1 已由项目所有者人工批准，且 `m1_complete` 的变更另行审查。
- M0 至 M11 的适用实现和合同测试已完成。
- A-025 已关闭 O-003，目标平台最低系统、运行库与构建基础镜像已冻结。
- 永久 Windows AppId、应用层更新签名/轮换和真实发布端点已人工批准。Windows Authenticode
  与 Apple Developer ID/notarization 当前不是前置条件；以后启用时必须另行批准身份和流程。
- v3.4.0 到 v4 bridge、bootstrap journal、Health Check、Commit marker 与恢复合同已冻结。

## 非目标

- 不把 Tauri GUI bundle 改名后当作完整产品。
- 不在安装脚本中实现数据库、Unity 项目或业务迁移。
- 不让 Tauri Updater成为完整产品的权威更新器。
- 不把 PKG、RPM、Flatpak 或 Snap 作为 4.0.0 blocker。
- 不在开发或 PR 验证中使用真实生产签名密钥、notarization 凭据或更新发布权限。
- 不让 Python/.NET SDK 或可选 Loopback API 扩大 4.0.0 发行范围；初始布局和合同只需保留
  以后兼容增加的空间。

## 影响范围

本计划获批后可修改：

```text
xtask/ 中的 dist 模块
distribution/ 中的平台打包定义与受控脚本
安装/更新/CLI 集成所需的 application 用例与适配器
.github/workflows/ 中的发行、应用层签名与平台安装测试
更新 Manifest Schema、契约快照与 Fixture
docs/testing/*
docs/status.md
docs/exec-plans/M12-full-product-distribution.md
```

生产代码、公共 RPC、权限、数据库 Schema 或更新合同如需变化，必须在对应里程碑取得人工
审批，不能以“打包需要”为由绕过架构边界。

## 全产品 staging 合同

每个 target 的 staging Manifest 至少固定：

- 产品版本、target triple、构建来源与可复现输入。
- daemon、GUI、CLI、MCP、扩展宿主、bootstrap、updater 和第一方扩展的必需清单。
- 公开 `bin` 与内部 `runtime` 边界、文件模式、运行库、许可证和资产来源。
- 每个组件的版本、摘要、应用层签名状态、可选平台签名状态与允许的入口。
- 禁止旧内部身份、缺失 release-blocker、意外调试文件和秘密材料。

`xtask dist` 必须先完成 staging 验证，再调用平台打包工具；任一组件缺失或版本不一致时不
得生成看似成功的最终资产。

## 实施阶段

### 1. dist 骨架与确定性 staging

1. 新增版本化 dist Manifest 与 target 配置。
2. 构建 Rust Workspace 组件、GUI 前端和第一方扩展。
3. 使用 Tauri 编译 `alcomd-gui`，仅提取受清单允许的 GUI 产物。
4. 收集完整组件并校验版本、摘要、权限、许可证和身份。
5. 提供不使用真实应用层私钥的 `--check`/fixture 路径供 CI 验证，不伪造正式签名成功；
   Authenticode/Developer ID 未启用时必须明确标记为 `not-required`，不能伪装成已签名。

### 2. Windows Inno Setup

1. 由统一 staging 生成单个 `ALCOMD_<version>_windows_x86_64_setup.exe`。
2. 当前用户为默认无提升模式，安装至 `%LOCALAPPDATA%\Programs\ALCOMD\`；所有用户模式只
   在明确选择后请求 UAC，安装至 `%ProgramFiles%\ALCOMD\`。
3. 两范围互斥，支持双向受控转换和失败回滚。
4. `bin/alcomd-cli.exe` 是唯一 PATH 入口；`runtime/` 组件不公开。
5. Inno 负责部署、卸载、快捷方式、协议/文件关联和启动 bootstrap；所有业务迁移由
   bootstrap/迁移用例负责。
6. Windows 正式测试 Windows 10 22H2 与 Windows 11，不增加主动拒绝更旧 Windows 的
   应用级门槛；WebView2 Evergreen 覆盖在线 Bootstrapper 与离线 Standalone Installer。
7. 当前不要求 Authenticode；测试必须覆盖未签名安装器的下载、SmartScreen 提示、启动和
   应用层更新验签。
8. M0 的 Windows Server hosted 与开发机 compile-only 结果不属于客户端运行证据；在 Win10
   22H2/Win11 实际安装并启动完整产品，验证 WebView2 渲染、托盘、注册表、用户数据路径、
   更新、升级和卸载后，才可关闭 `platform.windows-client-runtime` deferred 项。

### 3. macOS DMG

1. 以 `MACOSX_DEPLOYMENT_TARGET=11.0` 生成包含完整 arm64 组件的 `ALCOMD.app` 与
   `ALCOMD_<version>_macos_aarch64.dmg`，不增加额外应用级 OS 版本门槛。
2. 当前允许未做 Developer ID 签名/notarization；真实机器验证 Gatekeeper 警告、用户启动
   路径和升级。未来启用平台签名时再执行嵌套签名、notarization 与 stapling。
3. 拖入 Applications 不修改 shell；GUI 中的显式操作负责创建或移除稳定 CLI 入口。
4. 首次启动由 bootstrap 协调 v3 迁移和健康检查。

### 4. Linux AppImage 与 DEB

1. AppImage 包含便携运行所需完整组件；用户可主动创建 `~/.local/bin/alcomd-cli`，移动
   AppImage 后入口必须可修复且不得静默指向旧路径。
2. DEB 将 `alcomd-cli` 安装到 `/usr/bin`，内部程序置于受管理目录，程序文件升级与卸载
   完全服从包管理器所有权。
3. AppImage self-update 与 DEB package-manager update 使用同一产品版本/签名合同，但不得
   互相覆盖对方拥有的文件。
4. 两种格式都在 Ubuntu 22.04 x86_64 基线构建；逐个扫描 ELF，最高所需 glibc 符号不得
   超过 `GLIBC_2.35`。

### 5. 完整产品更新与 v4 bridge

1. `alcomd-updater` 验证更新 Manifest、目标、版本、ALCOMD 应用层完整包签名/摘要与组件清单；
   不得把 Authenticode、Developer ID 或 notarization 当作唯一真实性来源。
2. `alcomd-bootstrap` 协调停止、journal、替换、迁移、健康检查、Commit、恢复和清理。
3. Windows 使用应用层验签的完整 Inno 包；macOS 使用应用层验签的完整产品替换流程；
   AppImage 使用便携更新；DEB 只通过包管理器更新程序文件。
4. v3.4.0 只发现、验签并启动 v4 bridge installer；bridge 安装完整 v4 并包含或调用
   bootstrap，不得把完整迁移责任回推给 v3.4.0。

### 6. 发行与故障测试

执行 `docs/testing/test-plan.toml` 中全部 `distribution.*`、`installer.*`、
`updater.full-product-atomic`、迁移与 residue 测试。真实机器覆盖安装、升级、范围转换、CLI、
协议/文件关联、多用户、进程占用、磁盘满、提升取消、重启、签名失败和每个恢复阶段。

## 验收标准

- 四种主要格式均从同一版本化 staging 合同产生并包含完整必需组件。
- Windows 只有一个主 EXE 资产，两个安装范围互斥且可恢复转换。
- 三个平台只公开 `alcomd-cli`，入口安装、升级、重定位和卸载无越权或残留。
- Tauri GUI bundle 未被任何文档、Manifest 或更新器当作完整产品。
- updater/bootstrap/平台包对完整产品版本和组件集合达成一致，故障注入可恢复。
- 应用层签名/摘要、安装、升级、卸载与 residue 测试在真实目标系统通过；当前未启用的
  Authenticode/Developer ID/notarization 明确标为非 blocker，并验证对应系统警告路径。
- 所有公开 Schema、快照、ExecPlan 进度和 `docs/status.md` 已更新，并再次停在人工发布审批点。

## 人工审批点

- 开始实现前批准永久 Windows AppId、四种格式的最终身份和目录布局。
- 接入应用层签名前批准密钥保管、权限、轮换、离线/托管签名和应急撤销流程；未来启用
  Authenticode 或 Developer ID/notarization 时再单独批准平台身份与凭据流程。
- 首次 bridge rollout 前批准 v3.4.0 更新响应、分批策略、恢复源和停止条件。
- 任何真实发布、tag、上传、notarization 或第三方系统写入都需要单独明确授权。

## 进度日志

- 2026-08-16：M-1 只建立本后续 ExecPlan；未实现安装器、dist、签名或发行资产，未修改机器环境。
