# ADR: 多组件产品与跨平台全产品打包

- 状态：Accepted
- 日期：2026-08-16

## 背景

ALCOMD 由核心、GUI、CLI、MCP、扩展宿主、bootstrap、updater 和第一方扩展组成。只有
`alcomd-gui` 使用 Tauri。把 Tauri GUI bundle 当作完整产品发行物，会遗漏非 GUI 组件，并让
Tauri Bundler 错误地主导产品组成、安装、更新和迁移生命周期。

统一产品描述：

> ALCOMD is a Rust-based local application platform with a Tauri-powered official GUI.

> ALCOMD 是一个基于 Rust 的本地应用平台，其官方 GUI 使用 Tauri 构建。

## 决策

1. ALCOMD 是 Rust Workspace 和多组件本地应用平台；`alcomd-gui` 是唯一使用 Tauri 构建的
   官方 GUI 子应用。
2. 安装器和发行包安装完整 ALCOMD 产品，不是单独安装 Tauri GUI。
3. 正式发行由未来的 `cargo xtask dist --target <target>` 统一构建、收集、验证、签名和打包
   release-blocker 组件。这里不是全面禁止 Tauri Bundler：`tauri build`/Tauri Bundler 可以
   构建 GUI，也可以参与适合的平台打包步骤，但必须是由 ALCOMD 流水线控制的子步骤。
4. Windows x86_64 主格式是单文件 Inno Setup EXE。一个安装器支持当前用户与所有用户两种
   模式；当前用户为默认且不提权，只有明确选择所有用户时才请求管理员权限。
5. macOS Apple Silicon 主格式是包含完整 `ALCOMD.app` 产品组件的 DMG；4.0.0 普通用户
   主格式不采用 PKG。当前 Apple Developer ID 签名与 notarization 不是发布 blocker，允许
   未签名、未公证发行，但必须验证并记录真实用户启动路径和系统警告。
6. Linux x86_64/amd64 同时发布 AppImage 与 DEB，二者都包含各自发行范围要求的完整组件；
   RPM、Flatpak、Snap 不阻塞 4.0.0。
7. 更新由 `alcomd-updater`、`alcomd-bootstrap` 与经过 ALCOMD 应用层签名/摘要验证的完整
   产品包共同管理，不采用 Tauri Updater 作为完整产品的权威更新系统。应用层更新真实性
   不依赖 Windows Authenticode 或 Apple Developer ID。
8. ALCOMD3 v3.4.0 只是最后受支持的 v3 迁移入口。ALCOMD v4 bridge installer 安装完整 v4
   产品并包含或调用 `alcomd-bootstrap`；数据迁移、健康检查、回滚和 v3 清理由 bootstrap
   协调，安装器脚本不得解析 v3 数据库、修改 Unity 项目或实现业务迁移。

## 平台 CLI 集成

- Windows：产品布局分为公开 `bin/alcomd-cli.exe` 与内部 `runtime/`。仅把 `ALCOMD/bin`
  加入用户或系统 PATH；内部程序不作为普通终端命令公开。
- macOS：DMG 中的 `ALCOMD.app` 包含 CLI。拖入 Applications 不修改 shell 配置；用户通过
  GUI 的“安装命令行集成”操作主动创建稳定入口。
- Linux DEB：公开 CLI 安装到 `/usr/bin/alcomd-cli`，内部程序进入受包管理器管理的应用目录；
  应用不得覆盖包管理器拥有的文件。
- Linux AppImage：GUI 提供用户主动启用的集成，可在 `~/.local/bin/alcomd-cli` 创建稳定入口；
  不假设 AppImage 本体位于固定路径。

## Windows 安装与 bootstrap 边界

Inno Setup 负责部署文件、卸载登记、快捷方式、`alcomd://`、文件关联、CLI PATH 入口和调用
`alcomd-bootstrap`。它不得持有业务迁移逻辑。

`alcomd-bootstrap` 负责组件停止/恢复、v3→v4 数据迁移、健康检查、回滚、v3 清理和更新期
协调。当前用户与所有用户安装不得同时存在；安装范围转换必须被显式检测、计划并验证。

## 结果

- Tauri Bundler 仍是允许评估和使用的构建/打包工具；被否定的是它作为整个 ALCOMD 产品
  安装框架或生命周期权威的地位，而不是工具本身。
- 当前允许 Windows 安装器没有 Authenticode、macOS DMG/app 没有 Developer ID 签名与
  notarization；这只放宽操作系统平台签名门槛，不放宽 bridge、updater、Manifest、扩展和
  包完整性合同。
- `npm run gui:build` 与 `tauri build` 只证明 GUI 可构建，不是产品发行命令。
- Windows 当前用户/所有用户是一个 EXE 的两种安装模式；签名、更新包和 manifest 是辅助
  发行产物，不增加平台类别。
- 正式发行必须验证 staging 中所有组件版本、权限、许可证、签名和原子更新兼容性。
- Inno Setup、DMG、AppImage、DEB 和 `xtask dist` 的实现进入独立后续 ExecPlan；本 ADR 不
  授权在 M-1 编写安装器、安装工具、生成发行包或修改当前计算机。

## 被替代方案

以下方案均被全产品打包模型替代（Superseded by the full-product packaging model）：

- 单个 Tauri NSIS `installMode = both`。
- 两个 Tauri NSIS 安装器。
- 当前用户 NSIS + 全局 WiX/MSI。
- Tauri Updater 作为整个 ALCOMD 产品的权威安装/更新系统。
