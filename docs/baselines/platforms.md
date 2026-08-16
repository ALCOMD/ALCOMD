# 跨平台构建与测试基线

状态：A-017/A-024/A-025 已冻结发布范围、全产品打包模型与平台技术基线

最后核验：2026-08-17

## 定位

本文冻结仓库在 Windows、Linux 与 macOS 上的构建前提和最低验证面，不替代
`docs/testing/release-matrix.md`。ALCOMD 是一个基于 Rust 的本地应用平台，其官方 GUI 使用
Tauri 构建。A-017/A-024 已决定三个发布平台和 Windows Inno Setup EXE、macOS DMG、Linux
AppImage/DEB 四种主要用户格式都是 4.0.0 Release Blocker；A-025 已关闭 O-003。

## A-025 平台技术基线

| 平台 | 正式构建/测试范围 | 兼容边界 | 当前平台签名策略 |
|---|---|---|---|
| Windows x86_64 | Windows 10 22H2、Windows 11 | 不增加主动拒绝更旧 Windows 的应用级门槛；只有真实依赖/OS API 失败才报告不兼容 | Authenticode 不是 4.0.0 blocker |
| Linux x86_64 | Ubuntu 22.04 构建 | 所有发行二进制最高所需 glibc 符号版本 `<= GLIBC_2.35` | 使用应用层摘要/签名，不要求平台代码签名 |
| macOS arm64 | `MACOSX_DEPLOYMENT_TARGET=11.0` | 不增加超出平台 loader/依赖结果的应用级版本门槛 | Developer ID/notarization 不是 4.0.0 blocker |

Linux 的条件描述的是“二进制不能引用比 `GLIBC_2.35` 更新的符号”，不是要求运行环境版本
小于等于 2.35。Ubuntu 22.04 的官方 `libc6` 基线是 2.35；构建后必须用 ELF version-info
扫描每个发行二进制和本地动态库，不能只相信构建容器标签。

Windows 使用 WebView2 Evergreen Runtime。安装/更新先检测 Runtime；缺失时支持 Microsoft
在线 Evergreen Bootstrapper，同时为离线场景提供 Standalone Installer 路径。不得假定
Windows 10/11 一定预装 Runtime，也不得把 Edge 浏览器本体当生产 WebView2 Runtime。

当前允许未做 Windows Authenticode、未做 Apple Developer ID 签名/公证的发行。必须明确
记录相应 SmartScreen/Gatekeeper 用户路径和警告；这不取消 v3 bridge、自动更新资产、更新
Manifest、第一方扩展与包下载的 ALCOMD 应用层签名/摘要校验。

## 官方构建前提

以下 Tauri 前提只适用于 `alcomd-gui`，不定义完整产品的安装或发行工具链。

| 平台 | `alcomd-gui` 的 Tauri 2 构建前提 | 仓库最低验证 |
|---|---|---|
| Windows x64 | MSVC C++ Build Tools、WebView2 | Rust Workspace、前端、Tauri Rust 壳、完整 GUI build |
| Linux x64 | WebKitGTK 4.1、编译工具链、OpenSSL、ayatana appindicator、librsvg 等发行版依赖 | Rust Workspace、前端、Tauri Rust 壳、无签名 GUI build |
| macOS Apple Silicon | Xcode Command Line Tools | Rust Workspace、前端、Tauri Rust 壳、`MACOSX_DEPLOYMENT_TARGET=11.0` GUI build |

依据：

- Tauri 2 prerequisites：`https://v2.tauri.app/start/prerequisites/`
- Tauri 2 distribution：`https://v2.tauri.app/distribute/`
- Microsoft WebView2 Runtime distribution：
  `https://learn.microsoft.com/microsoft-edge/webview2/concepts/distribution`
- Ubuntu 22.04 `libc6` 2.35：`https://packages.ubuntu.com/jammy/libc6`
- Apple deployment target build setting：
  `https://developer.apple.com/documentation/xcode/adding-a-build-configuration-file-to-your-project`
- GitHub-hosted runner labels：
  `https://docs.github.com/en/actions/how-tos/write-workflows/choose-where-workflows-run/choose-the-runner-for-a-job`

## 当前 CI 事实

`.github/workflows/ci.yml` 当前覆盖：

- Ubuntu 24.04：元数据、非 GUI Rust、独立 Discord 后台、TypeScript 与 Vite。
- Windows 2025：`cargo check -p alcomd-gui`。
- macOS：无任务。
- Linux GUI：未安装 Tauri 系统依赖，未检查 GUI Rust 壳或 bundle。
- Windows 完整 GUI：未执行 `npm run gui:build`。
- 前端安装仍使用 `npm install`，尚未满足 M0 的锁文件复现要求。
- Cargo 命令未统一使用 `--locked`，也没有构建后 lockfile diff gate。
- 根 Workspace 的 Clippy/test 仍排除 GUI test target；独立 Discord backend 没有 test gate。
- setup/check/test 的 PowerShell 与 Bash 检查面不完全对称，且 setup 未诊断 Python、PowerShell
  7 和 Tauri 平台原生依赖。

因此当前 CI 只能证明初始化骨架的部分可构建性，不能证明三平台发行就绪。

## M0 建议验证矩阵

M0 只验证骨架，不生成或发布正式签名资产：

1. 所有平台运行元数据、格式、全 Workspace Clippy/test、独立 Discord backend test 与前端
   check/build；Cargo 命令使用 `--locked`，最终证明两份 lockfile 未改变。
2. Windows x64 运行 `alcomd-gui` 的完整 Tauri `build --no-bundle`，不能只做 `cargo check`。
3. Ubuntu x64 安装 GUI 所需 Tauri 系统依赖后运行 `alcomd-gui` 的 `build --no-bundle`。
4. macOS arm64 使用显式原生 runner/target 和 deployment target 11.0 运行 `alcomd-gui` 的
   `build --no-bundle`；若配额不允许，必须记录替代的真实机器验证证据，不能静默跳过。
5. 所有 npm 任务使用 `npm ci`，不得在 CI 中重新求解依赖。
6. setup/check/test 的 PowerShell/Bash 脚本检查面对称；setup 只检测并报告原生依赖，不隐式
   安装或改变系统。

这些命令只证明 GUI 子应用可编译；不得把 `tauri build` 的单一 GUI bundle 当作完整产品。

## 全产品发行阶段附加门槛

- 未来由 `cargo xtask dist --target <target>` 构建、收集、验证、签名并打包完整产品组件。
- Windows 单一 Inno Setup EXE 的当前用户默认模式、所有用户显式提升模式、双向范围转换、
  多用户、自定义路径、WebView2 在线/离线部署、未签名 SmartScreen 路径与应用层更新验签。
- macOS 完整 `ALCOMD.app` 的 DMG、deployment target、未签名/未公证 Gatekeeper 用户路径、
  升级和用户主动 CLI 集成；未来启用签名时再增加嵌套签名/notarization 门槛。
- Linux 完整产品 AppImage/DEB、桌面集成、权限、依赖、升级、包管理器所有权和 CLI 集成。
- 各平台执行安装快照、迁移、卸载和零残留比较。

这些门槛属于后续里程碑；M0 不得预先实现安装器、签名或发布工作流。

Linux 发行资产不能从 Ubuntu 24.04 PR CI 直接生成；正式 AppImage/DEB 必须在 Ubuntu 22.04
基线构建并通过 `GLIBC_2.35` 符号上限检查。macOS 完整 GUI 黑盒测试不能由 Windows/Linux
WebDriver 结果代替，需要真实 Apple Silicon 自动化或人工证据。
