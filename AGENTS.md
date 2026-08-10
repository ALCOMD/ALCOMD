# AGENTS.md

## 基本要求

- 回答、审查结论和任务说明使用简体中文。
- 默认缩进为 4 个半角空格；遵循已有文件格式化规则。
- 修改前先阅读相关代码、配置和文档；更深层级的 `AGENTS.md` 优先。
- 避免无关重构、格式化或元数据变更。
- 不删除或覆盖用户已有改动；不确定时先确认或绕开。
- 避免在项目中硬编码。URL、版本、主题色、公开路径和产品名优先使用已有配置或集中常量。

## 项目定位

- ALCOMD3 已作为独立项目维护，当前代码、文档、发布流程和用户体验以 ALCOMD3 自身为准。
- 外部改动只做选择性引入。安全修复、数据损坏修复、VRChat/VPM 兼容性修复和重要崩溃修复优先。
- 不要机械套用其他项目的结构或流程。涉及 GUI、MCP、操作状态、资源锁、取消机制或仓库管理的改动，应按 ALCOMD3 当前架构重新评估和适配。

## 兼容性边界

- 不要随意修改 Tauri identifier、安装目录、主程序文件名、协议名、用户数据路径或 `vrc-get` 兼容路径。
- `vrc-get-gui/`、`vrc-get-vpm/`、`vrc-get/` 等命名目前具有历史和兼容成本，不作为普通清理项重命名。
- ALCOMD3 updater 使用 ALCOMD3 自有 endpoint 和签名材料，不复用非 ALCOMD3 的发布配置、secret 或 updater metadata。
- MCP 默认关闭并内置于 GUI 进程；对外仅使用带 bearer token 的本机回环 Streamable
  HTTP，不得恢复独立 helper、私有 IPC，也不得监听局域网或公网地址。
- 面向用户时对所有已发布平台使用同一文案原则：不因某个平台的打包、签名、安装或更新
  机制增加专属披露、警告或帮助内容；只有本版本确有该平台的用户可见变化时才点名说明。

## 开发与验证

- Rust 改动按影响范围运行 `cargo check`、`cargo test` 或更聚焦的包级命令。
- GUI 改动按影响范围运行 `npm.cmd run check`、`npm.cmd run lint`、`npm.cmd run build` 或更聚焦验证。
- 无法验证时说明原因、风险和未覆盖范围。

## 文档与发布

- `docs/README.md` 是统一文档入口；需要定位维护、发布、MCP、格式说明或历史记录时优先从这里进入。
- 处理发布、发布审计、版本准备、GitHub Release、updater metadata、stable/beta channel 或 changelog 任务时，先读取 `docs/skills/alcomd3-release/SKILL.md`，再按 `docs/RELEASE.md` 执行或审计。
- 普通开发中，每项重要的用户可见行为、兼容性、弃用/移除、安全、已知问题、打包或公开配置变化，都应在同一变更或 PR 中写入 `CHANGELOG.md` 的 `Unreleased` 对应分类，并按需同步文档。不要把 commit/PR 清单、纯 CI/workflow、测试、内部重构或仅供维护者使用的实现记录写入 changelog，除非它们产生用户可见或发布影响。
- 发布 beta 时，将适用的 `Unreleased` 增量移动到对应带日期版本条目；发布 stable 时，从期间 beta 条目和 `Unreleased` 整理出相对上一个 stable 的最终净变化，允许与 beta 条目有意重叠，但不包含后来撤销的中间状态。两种发布都必须在顶部保留新的 `Unreleased`，同步日语和简体中文目标版本条目，并补齐 `release-metadata/updater-notes/` 中对应版本的七语 updater 摘要。
- 根 `CHANGELOG.md` 是版本和变更事实的唯一权威来源。GitHub Release 正文固定按 English、日本語、中文顺序组合根文件、`CHANGELOG/CHANGELOG.ja.md` 和 `CHANGELOG/CHANGELOG.zh-CN.md` 的目标版本条目；三个条目的日期、分类顺序和各分类项目数必须一致并通过 `release-validate`。繁体中文 changelog 仅供阅读，不作为 Release 正文输入。
- Changelog 的 stable 版本链接对比上一个 stable，beta 版本链接对比紧邻的上一个已发布版本（stable 或 beta）；首个公开版本没有前序基线，链接到自身 GitHub Release。
- `CHANGELOG.md` 使用 Keep a Changelog 分类并按时间倒序维护。已公开的 Git tag、GitHub Release、changelog 已发布版本条目和 updater metadata 均视为历史记录，不得为后续功能或 PR 改写；只有在明确执行发布审计/修复，且目标是恢复为实际已发布内容时，才可修改。当前分支不保留独立的按版本发布说明文件。
- 发布流程以 ALCOMD3 自己的 `xtask`、文档和 updater 签名流程为准。
- GitHub Actions 是计划内的默认发布编排入口，发布规则仍集中在 `xtask`。本地默认只生成可安装、可签名、可验证的临时测试产物；确需手动发布时，必须显式生成 release artifacts，并从 clean、已同步的 `main` 创建或更新 GitHub Release。Updater metadata 始终由 Release 发布事件触发的 Actions 流程处理。
- 修改 GitHub release workflow 或发布阶段语义时，应先单独设计、验证，并同步发布手册与 release skill。
