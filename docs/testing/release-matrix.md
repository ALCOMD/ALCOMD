# 4.0.0 发布测试矩阵

状态：产品、打包与平台技术基线已由 A-024/A-025 批准；实现与真实机器证据待完成

2026-11-25 按所有者决定延期 macOS 原生验收。Windows 和 Linux 可先走隔离验收，公开 4.0.0 沿用 A-017。实施顺序、签名和生产放行见 [M12 实施计划](../exec-plans/M12-full-product-distribution.md)。

已用合成签名完成三目标离线验签和 Windows 实际运行文件的暂存核对。四个完整包及对应客户端验收继续按下表追踪。[阶段 1 运行载荷证据](../exec-plans/M12-full-product-distribution.md#阶段-1-运行载荷)

## 产品发行单元

ALCOMD 是一个基于 Rust 的本地应用平台，其官方 GUI 使用 Tauri 构建。

正式发行物必须包含目标平台所需的 `alcomd`、`alcomd-gui`、`alcomd-cli`、
`alcomd-mcp`、`alcomd-extension-host`、`alcomd-bootstrap`、`alcomd-updater` 和第一方扩展。
`tauri build` 生成的单一 GUI bundle 不是完整产品发行物。

## 三个平台、四种主要发行格式

| 平台 | 主要用户格式 | 建议资产名称 | 4.0.0 blocker |
|---|---|---|---:|
| Windows x86_64 | Inno Setup EXE | `ALCOMD_4.0.0_windows_x86_64_setup.exe` | 是 |
| macOS arm64 | DMG | `ALCOMD_4.0.0_macos_aarch64.dmg` | 是 |
| Linux x86_64 | AppImage | `ALCOMD_4.0.0_linux_x86_64.AppImage` | 是 |
| Linux amd64 | DEB | `ALCOMD_4.0.0_linux_amd64.deb` | 是 |

Windows 当前用户与所有用户安装是同一 Inno Setup EXE 的两种模式，不是两个发行资产。
签名文件、更新压缩包和更新 Manifest 是辅助发行产物，不增加平台或主要格式类别。

## Windows 安装范围矩阵

| 场景 | 预期行为 |
|---|---|
| 当前用户默认安装 | 不提升；安装至 `%LOCALAPPDATA%\Programs\ALCOMD\`；只写用户 PATH |
| 所有用户显式安装 | 仅在用户选择后请求 UAC；安装至 `%ProgramFiles%\ALCOMD\`；只写系统 PATH |
| 当前用户转所有用户 | 检测旧范围，受控迁移后移除旧用户 PATH 与安装登记，不允许双实例 |
| 所有用户转当前用户 | 检测旧范围，受控迁移后移除旧系统 PATH 与安装登记，不允许双实例 |
| 多用户 | 用户数据与全局程序文件隔离，非安装用户不得继承错误的用户级状态 |
| 自定义路径 | 所有写入、升级、卸载和恢复均使用解析后的真实安装根，不能退回默认路径 |

安装布局必须只公开 `bin/alcomd-cli.exe`；`runtime/` 中的 daemon、GUI、MCP、扩展宿主、
bootstrap 和 updater 不进入 PATH。更新不得重复添加 PATH，卸载只移除安装器创建的精确条目。

Inno Setup 只负责部署、卸载登记、快捷方式、协议/文件关联、CLI 入口和调用 bootstrap；不得
解析 v3 数据库、修改 Unity 项目或实现业务迁移。Windows 完整产品更新由 `alcomd-updater`、
`alcomd-bootstrap` 与经过 ALCOMD 应用层签名/摘要验证的完整 Inno Setup 包共同完成；当前
不要求 Authenticode。

Windows 正式测试 Windows 10 22H2 与 Windows 11，不增加主动拒绝更旧版本的应用级检查。
WebView2 Evergreen Runtime 必须覆盖已安装、缺失后在线 Bootstrapper、离线 Standalone
Installer、安装取消与失败恢复。

Windows 11 Sandbox 已通过当前用户与全用户夹具的登记和卸载验证，并打开了使用已安装 WebView2 的原生 GUI。正式 Windows 10 22H2 与 Windows 11 矩阵仍需使用完整产品验证安装、启动、WebView2 部署、托盘、注册表、用户数据路径、升级、回滚和卸载。[Windows 登记及夹具验收](../exec-plans/M12-full-product-distribution.md#2026-11-25-windows-core-安装登记)

Windows 11 的完整 Inno 实包 `72a85893` 已通过双范围的 12 项卸载与启动互斥检查。全用户验收使用两个普通账号和实际 SDK 持续连接。[Windows 跨账号卸载](../exec-plans/M12-full-product-distribution.md#2026-11-25-windows-跨账号卸载)

## macOS 安装与 CLI

- DMG 中的 `ALCOMD.app` 必须包含目标平台所需的完整产品组件，所有 arm64 产物使用
  `MACOSX_DEPLOYMENT_TARGET=11.0`；不增加额外应用级系统版本门槛。
- 当前允许未做 Developer ID 签名与 notarization，必须在真实 macOS 记录挂载、复制、首次
  启动、Gatekeeper 警告/用户操作和升级；未来启用签名时再增加嵌套签名、公证和 stapling。
- 拖入 Applications 不得隐式修改 shell 配置。
- GUI 提供用户主动触发的“安装命令行集成”，创建稳定的 `alcomd-cli` 入口；重复执行、升级
  与移除必须幂等且可审计。
- 4.0.0 不以 PKG 作为普通用户主发行格式。

## Linux 安装与 CLI

- AppImage 与 DEB 在 Ubuntu 22.04 x86_64 基线构建；逐个扫描 ELF，最高所需 glibc 符号版本
  不得超过 `GLIBC_2.35`。
- DEB 将公开命令安装为 `/usr/bin/alcomd-cli`，内部组件位于软件包管理器控制的应用目录；
  应用不得自行覆盖 `/usr/bin` 或其他由软件包管理器拥有的文件。
- AppImage 是便携格式；GUI 可在用户明确请求后创建 `~/.local/bin/alcomd-cli` 命令入口，
  入口不得假定 AppImage 永久位于某个固定路径。
- RPM、Flatpak 与 Snap 不属于 4.0.0 blocker。

## 统一发行流水线

全产品发行通过 `cargo xtask dist --target <target>` 分阶段完成。

1. 构建目标平台 Rust 组件与 GUI 前端。
2. 使用 Tauri 编译 `alcomd-gui`，但不把其 bundle 当作最终产品。
3. 构建第一方扩展并执行 Extension 合同要求的应用层签名。
4. 收集所有 release-blocker 组件到统一 staging 目录。
5. 验证组件清单、版本一致性、权限、许可证与禁止旧身份。
6. 调用 Inno Setup、DMG、AppImage 或 DEB 打包工具链。
7. 生成应用层签名/摘要与更新 Manifest；仅在当前策略启用时执行 Authenticode 或 Apple
   Developer ID/notarization。
8. 执行安装、升级、卸载、范围转换、更新恢复和残留测试。

`npm run gui:build` 仍可用于 GUI 构建验证，但不是完整产品发行命令。

Windows Inno、Linux AppImage 和 Linux DEB 的原生输入由 `--package-layout` 导出。使用 `--package` 可以在对应系统上直接组装完整资产。该步骤读取经签名的运行目录，核对当前干净检出的 SHA 和 Git 签名，并在新目录中原子发布资产及工程回执 `asset.json`。

```text
cargo xtask dist --target x86_64-unknown-linux-gnu --package --format deb \
  --source-sha <signed-commit> --staging <signed-runtime> --output <new-directory> \
  --packager /usr/bin/dpkg-deb --packager-sha256 <tool-sha256>
```

Inno 使用 `--format inno`，提供 ISCC 和既有的 `--webview2-bootstrapper`、`--webview2-bootstrapper-sha256`。AppImage 使用 `--format appimage`，提供 appimagetool 及 `--appimage-runtime`、`--appimage-runtime-sha256`。输出目录的父目录须已存在，源码树内的暂存应位于 Git 忽略目录。工程回执记录格式、目标、源码与版本、实际资产字节和摘要、签名运行清单摘要及打包工具摘要。正式发布清单的签署和公开继续按审批矩阵执行。macOS DMG 仍按原计划延期。

## 迁移入口与更新源

- ALCOMD3 3.4.0 是最后一个受支持的 v3 迁移入口版本，不执行完整 v4 替换迁移。
- 更早公开 v3.x 必须先通过旧更新链升级到 3.4.0。
- Windows 3.4.0 从冻结更新 API 发现、验签并启动 v4 bridge installer。Windows 旧版在启动被接受后退出。
- AppImage 和 macOS 使用用户主动启动的独立 bridge。公共旧源须验证相应客户端不会执行原位替换式更新。DEB 由包管理器交接程序文件，数据迁移按账号执行。
- v4 bridge installer 安装完整 v4 产品，并包含或调用 `alcomd-bootstrap` 完成迁移协调、健康
  检查、清理与回滚。
- 不支持的旧 v3.x 直接启动 bridge 时必须安全拒绝并提示先升级到 3.4.0。

必须覆盖：旧版到 3.4.0、3.4.0 到签名 bridge、bridge 到完整 v4、任一故障点保持可恢复，
以及 DEB 由软件包管理器接管程序文件升级的独立路径。

## 用户目录与功能对照

- 默认 Documents、OneDrive 重定向、非 ASCII 用户名与路径、自定义项目/备份目录。
- 目标目录已存在、文件被占用、权限不足与磁盘空间不足。
- v3 与 v4 对相同项目副本的最终结果。
- GUI、CLI、MCP、RPC 和扩展入口调用同一用例；JSON 输出与错误码保持合同。
- GUI 功能、状态、错误、进度和可访问性等价，不要求像素复刻。

## 并发、扩展与故障注入

同时验证一个 GUI、两个 CLI、多个 MCP 客户端与多个扩展；同项目写入串行、不同项目并行，
取消和断线可恢复。通过设置页「客户端」查看多个连接实例并断开指定实例。Discord 新用户默认禁用，升级用户迁移原状态；
禁用或卸载 Discord 扩展立即清除 Presence。

每个平台和格式都要覆盖安装快照、迁移、升级、卸载与 residue audit，并在迁移、数据库提交、
网络、包校验、签名、扩展宿主、daemon、bootstrap 和 updater 的关键阶段注入故障。
