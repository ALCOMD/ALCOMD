# 更新日志

语言: [English](../CHANGELOG.md) | [日本語](./CHANGELOG.ja.md) | 简体中文 | [繁體中文](./CHANGELOG.zh-TW.md)

此文件是英文主版 [`../CHANGELOG.md`](../CHANGELOG.md) 的简体中文版本。版本和变更事实仍以
主 `CHANGELOG.md` 为权威来源；本文件的目标版本条目用于 GitHub Release 的中文部分。

该文件遵循 [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) 约定，项目版本行为符合 [语义化版本控制](https://semver.org/spec/v2.0.0.html)。
GitHub Release 正文按 English、日本語、中文顺序，由结构一致的三个目标版本条目组成。稳定版
条目记录距离上一稳定版的最终净变化；预发布版记录自上一个已发布版本（稳定或预发布）以来
的变化。

开发期间的用户可见修改，按同一 PR/变更写入主文档 `../CHANGELOG.md` 的 `Unreleased`。准备
发布时翻译目标版本条目，并与英文主版保持相同日期、分类顺序和各分类项目数。

## [Unreleased]

### Added

- 新增持久化的用户自定义存储库显示名称。
- 新增持久化的 MCP 工具名称显示开关，可在调用名称与本地化名称之间切换，并在悬停时显示另一名称。

### Changed

- MCP 软件包工具统一使用存储库 ID 选择来源；存储库 URL 仅用于添加和删除存储库。
- Windows 全新安装现在默认勾选创建桌面快捷方式，升级时仍保留此前的选择。
- Windows 卸载时现在可以选择删除配置、缓存和其他本地应用数据，同时保留“文档”文件夹中的项目与备份。
- MCP 工具列表和项目卡片列表现在会按容器宽度响应式显示为最多三列，并在切换至三列前保留更充足的单项宽度。

### Fixed

- 修复切换是否显示辅助记录时活动记录表格列宽异常和布局抖动的问题。

## [3.2.0] - 2026-08-09

### Added

- 新增通过共享资源管理后端进行项目模板的 MCP 发现与管理能力。
- 新增标准化的变更日志（Changelog）作为发布间的重要变更权威记录。

### Changed

- 简化了 MCP 仓库与模板的数据结构（payload），并使用仓库 URL 作为管理身份。
- 简化了 Unity 启动状态标签文案。
- 将 Unity 编辑器聚焦能力扩展至 macOS，并改进了进程匹配和 Windows 编辑器就绪缓存。
- 将 GitHub Release 描述合并到该变更日志中，并将本地化 updater 摘要移入结构化发布元数据。

### Fixed

- 停止在 Issue 模板和应用内问题反馈链接中请求已废弃的 `vrc-get-gui` 标签。
- 在 Windows 上保存环境设置时保留用户自定义包路径。
- 检测到任意匹配的 Unity 进程在运行时会聚焦到对应编辑器窗口。
- 本地化了包操作报错文本，并避免仓库长字段溢出界面。

### Removed

- 删除了独立的按版本发布说明目录及其重复内容。

## [3.1.0] - 2026-08-01

### Added

- 新增支持 `Bearer` Token 保护、仅回环地址的 MCP Streamable HTTP 端点，以及可选客户端接入设置。
- 新增 Unity 项目打开与就绪状态跟踪、重复启动防护与编辑器聚焦能力。
- 新增 MCP 对 VPM 依赖和 UnityPackage 关联引用的模板编辑支持。

### Changed

- 当侧边栏条目被隐藏时保留扩展页可见；并将「项目 / 资源库 / 设置」设为侧边栏永久可访问项，修复历史配置导致的不可见问题。
- 允许内置 MCP 扩展被禁用：撤销访问、停止端点、移除侧边栏入口，并取消应用内 MCP 持有的项目任务。
- 在 MCP 后台扩展与端点启动期间，仍可显示主窗口与 MCP 工具界面。
- 在刷新期间保持项目列表可见，并更及时地更新项目类型与已保存仓库名。
- 在可选 MCP 配置时，保留与旧 stdio 传输无关的客户端设置；并要求相关客户端切换到受保护端点和 Token。

### Security

- MCP 默认保持关闭，并在本地回环端点上校验 Bearer 认证、主机与来源信息。

## [3.1.0-beta.3] - 2026-08-01

### Added

- 新增 Unity 项目打开与就绪状态跟踪、重复启动防护与编辑器聚焦。

### Changed

- 刷新期间保持项目列表可见；刷新已保存仓库名；更清晰地展示内置扩展行为。

## [3.1.0-beta.2] - 2026-07-28

### Added

- 允许禁用内置 MCP 扩展：撤销访问、停止端点、移除侧边栏入口，并取消应用内 MCP 的项目任务。

### Changed

- 在端点启动继续进行的后台期间，仍可显示主窗口与 MCP 工具。
- 创建项目或变更 VRChat SDK 包后，立即刷新项目类型。

## [3.1.0-beta.1] - 2026-07-28

### Added

- 新增支持 Bearer Token 的 MCP Streamable HTTP 端点，并为 Codex、Claude Code、Cursor 提供可选客户端配置；同时保留与旧设置无关的客户端项。

### Changed

- 在 MCP 配置对话框中归并端点信息与客户端设置。
- 创建项目或 VRChat SDK 包变更后，立即刷新项目类型。
- 要求配置为旧 stdio 传输的 MCP 客户端切换到受保护端点并使用 Token。

### Security

- MCP 默认关闭，并在仅回环端点上验证 Bearer 认证、主机与来源信息。

## [3.0.1-beta.1] - 2026-07-27

### Added

- 为可见的已安装扩展新增打开按钮。

### Changed

- 保持「项目 / 资源 / 设置」在侧边栏中的可见性并修正永久可见项，恢复已有配置下的正确展示。

## [3.0.0] - 2026-07-26

### Added

- 首个公开版 ALCOMD3 发布，覆盖 Windows x64、macOS Apple Silicon 与 Linux x86_64。

### Security

- 使用 ALCOMD3 自有更新端点、内嵌公钥和签名更新负载。

[Unreleased]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.2.0...HEAD
[3.2.0]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.1.0...v3.2.0
[3.1.0]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.0.0...v3.1.0
[3.1.0-beta.3]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.1.0-beta.2...v3.1.0-beta.3
[3.1.0-beta.2]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.1.0-beta.1...v3.1.0-beta.2
[3.1.0-beta.1]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.0.1-beta.1...v3.1.0-beta.1
[3.0.1-beta.1]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.0.0...v3.0.1-beta.1
[3.0.0]: https://github.com/ALCOMD3/ALCOMD3/releases/tag/v3.0.0
