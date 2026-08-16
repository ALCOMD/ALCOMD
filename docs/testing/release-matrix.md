# 4.0.0 发布测试矩阵

状态：产品与打包模型已由 A-024 批准；最低系统与运行库基线仍待 O-003 确认

## 产品发行单元

ALCOMD 是一个基于 Rust 的本地应用平台，其官方 GUI 使用 Tauri 构建。

正式发行物必须包含目标平台所需的 `alcomd`、`alcomd-gui`、`alcomd-cli`、
`alcomd-mcp`、`alcomd-extension-host`、`alcomd-bootstrap`、`alcomd-updater` 和第一方扩展。
`tauri build` 生成的单一 GUI bundle 不是完整产品发行物。

## 三个平台、四种主要发行格式

| 平台 | 主要用户格式 | 建议资产名称 | 4.0.0 blocker |
|---|---|---|---:|
| Windows x86_64 | Inno Setup EXE | `ALCOMD_4.0.0_windows_x86_64_setup.exe` | 是 |
| macOS arm64 | 签名并公证的 DMG | `ALCOMD_4.0.0_macos_aarch64.dmg` | 是 |
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
`alcomd-bootstrap` 与签名的完整 Inno Setup 包共同完成。

## macOS 安装与 CLI

- DMG 中的 `ALCOMD.app` 必须包含目标平台所需的完整产品组件并通过嵌套签名、notarization
  与 Gatekeeper 验证。
- 拖入 Applications 不得隐式修改 shell 配置。
- GUI 提供用户主动触发的“安装命令行集成”，创建稳定的 `alcomd-cli` 入口；重复执行、升级
  与移除必须幂等且可审计。
- 4.0.0 不以 PKG 作为普通用户主发行格式。

## Linux 安装与 CLI

- DEB 将公开命令安装为 `/usr/bin/alcomd-cli`，内部组件位于软件包管理器控制的应用目录；
  应用不得自行覆盖 `/usr/bin` 或其他由软件包管理器拥有的文件。
- AppImage 是便携格式；GUI 可在用户明确请求后创建 `~/.local/bin/alcomd-cli` 命令入口，
  入口不得假定 AppImage 永久位于某个固定路径。
- RPM、Flatpak 与 Snap 不属于 4.0.0 blocker。

## 统一发行流水线

未来以 `cargo xtask dist --target <target>` 为唯一全产品发行入口，依次完成：

1. 构建目标平台 Rust 组件与 GUI 前端。
2. 使用 Tauri 编译 `alcomd-gui`，但不把其 bundle 当作最终产品。
3. 构建并签名第一方扩展。
4. 收集所有 release-blocker 组件到统一 staging 目录。
5. 验证组件清单、版本一致性、权限、许可证与禁止旧身份。
6. 调用 Inno Setup、DMG、AppImage 或 DEB 打包工具链。
7. 签名最终资产并生成更新 Manifest。
8. 执行安装、升级、卸载、范围转换、更新恢复和残留测试。

`npm run gui:build` 仍可用于 GUI 构建验证，但不是完整产品发行命令。

## 迁移入口与更新源

- ALCOMD3 3.4.0 是最后一个受支持的 v3 迁移入口版本，不执行完整 v4 替换迁移。
- 更早公开 v3.x 必须先通过旧更新链升级到 3.4.0。
- 3.4.0 从标准更新 API 发现、验签并启动 v4 bridge installer；它只在启动被接受后退出。
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
取消和断线可恢复。MCP 管理扩展默认启用；Discord 新用户默认禁用，升级用户迁移原状态；
禁用或卸载 Discord 扩展立即清除 Presence。

每个平台和格式都要覆盖安装快照、迁移、升级、卸载与 residue audit，并在迁移、数据库提交、
网络、包校验、签名、扩展宿主、daemon、bootstrap 和 updater 的关键阶段注入故障。
