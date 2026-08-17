# v3.4.0 迁移 Fixture 采集规范

状态：后移到 M11；M-1 未接受任何可授权删除的真实安装 Fixture

Windows 首个 Fixture 的分阶段操作见 `windows-collection.md`；清单格式从
`manifest.example.toml` 复制。原始采集物必须留在仓库外，完成人工脱敏复核后才能把最小
Fixture 复制进本目录。

2026-08-17 的隔离 Windows 11 VM 操作报告确认了冻结安装器哈希、干净安装前 19 项目标均
不存在、当前用户默认安装、v3.4.0 卸载登记/可执行文件/GUI 版本与显式应用内退出生命周期。
安装后采集器在写出结果前失败，未产生可复核的脱敏 Fixture。项目所有者决定停止 M-1 的
合成状态和安装后采集，把完整证据后移到 M11。该操作报告不能把任何 artifact 的
`confirmed` 改为 `true`，也不能授权迁移清理。

本目录只接收从冻结版本 ALCOMD3 `v3.4.0` 采集并脱敏的迁移输入。源码审计只能确认
artifact 模板，不能替代 Windows 注册表、安装目录、用户数据和真实项目的实例证据。

## 必需 Fixture

| ID | 平台/场景 | 必需内容 |
| --- | --- | --- |
| `win-per-user-default` | Windows x64 当前用户默认安装 | 三个卸载注册表视图、安装目录清单、快捷方式、协议与文件关联、v3 数据根、默认项目/备份目录 |
| `win-per-machine-custom` | Windows x64 全局自定义路径 | HKLM32/HKLM64、安装 scope、Unicode/空格路径、多用户数据隔离 |
| `macos-arm64-app` | macOS Apple Silicon | app bundle 位置、Info.plist 身份、LaunchServices 可观察解析、数据根与 updater staging |
| `linux-x64-appimage` | Linux AppImage | 实际 executable、XDG 数据与 desktop handler、更新前后路径 |
| `linux-x64-deb` | Linux DEB | dpkg 状态、包文件清单、XDG/desktop/icon 状态；不得用 AppImage 更新路径代替 |
| `corrupt-and-conflict` | 全平台可移植数据 | 损坏 JSON/LiteDB、未知文件、符号链接、重名模板、失配 MCP token、第三方接管的 `vcc` handler |

## 采集规则

1. 在隔离 VM 或专用测试账号中安装冻结的 v3.4.0 资产，并先核对
   `docs/baselines/source-lock.toml` 中的 SHA-256。
2. 只采集结构、文件类型、权限、大小、哈希和经过脱敏的内容；不得提交真实 token、Authorization
   header、用户名、主目录、私有项目名或第三方服务器地址。
3. 绝对路径使用占位符：`<USER_HOME>`、`<LOCAL_APP_DATA>`、`<DOCUMENTS>`、`<V3_INSTALL>`。
4. 每个 Fixture 必须包含 `manifest.toml`，记录平台、v3 资产摘要、采集工具版本、脱敏规则、文件
   清单摘要以及预期 owner/action；敏感值使用固定测试值重新生成，不能只做部分遮盖。
5. 注册表只导出精确键和值；禁止提交整棵用户注册表。外部 MCP 配置应构造含无关条目与注释的
   合成副本，用来证明迁移只 patch 精确 entry。
6. 项目、模板和备份必须是可公开的最小合成内容；不得提交用户原始项目。

## 验收不变量

- Commit marker 之前任一故障都保持 v3 数据哈希不变且 v3.4.0 可重新启动。
- 成功迁移后，记录数、关键字段、相对路径和用户内容哈希与迁移前对账一致。
- 未识别文件、第三方 handler、失配环境变量和外部配置中的无关内容全部保留。
- 迁移日志、v4 配置、备份和错误中不出现旧 token 或仓库认证 header。
- `migrations/v3/artifacts.toml` 只有取得相应实例 Fixture 后才能把具体 artifact 的
  `confirmed` 改为 `true`；模板证据本身不授权删除。
