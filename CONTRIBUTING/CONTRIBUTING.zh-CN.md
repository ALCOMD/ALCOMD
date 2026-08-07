# Contributing

语言: [English](../CONTRIBUTING.md) | [日本語](CONTRIBUTING.ja.md) | 简体中文

### 项目标准

ALCOMD3 作为独立项目维护。当前仓库、文档、发布流程和用户可见行为是判断标准。

外部修复可能有价值，但必须按 ALCOMD3 当前架构审查和适配，尤其是 GUI/MCP 共享操作模型。

### 开发范围

- GUI 代码位于 `vrc-get-gui/`。
- VPM 包和项目管理代码位于 `vrc-get-vpm/`。
- CLI 兼容代码位于 `vrc-get/`。
- MCP bridge 代码位于 `alcomd3-mcp/`。
- MCP IPC 协议类型位于 `alcomd3-mcp-protocol/`。
- 发布和打包辅助任务位于 `xtask/`。

部分目录和包名仍因兼容性与历史原因使用 `vrc-get`。除非明确作为兼容性迁移，否则不要重命名。

### 环境

推荐工具：

- Rust stable toolchain。
- Node.js 和 npm。
- 构建 Windows MSVC target 时需要 Windows build tools。
- 本地处理 Windows installer 时可使用 Inno Setup；任务支持时也可让 `xtask` 下载 / 缓存。

直接 clone 本仓库：

```bash
git clone https://github.com/ALCOMD3/ALCOMD3.git
```

### 本地开发

构建并测试 Rust workspace 成员：

```bash
cargo check
cargo test
```

以开发模式运行桌面 GUI：

```bash
cd vrc-get-gui
npm install
npm run tauri dev
```

### 发布策略

ALCOMD3 拥有自己的发布流程。使用 ALCOMD3 的发布自动化、签名 secret、updater metadata 和发布命名。

稳定版本使用 SemVer，例如 `2.0.0`、`2.0.1`、`2.1.0`。预发布构建可使用 `2.1.0-beta.1` 这类后缀。

Windows release build 使用脚本化 release flow。本地 artifact 验证使用：

```powershell
cargo xtask release-build --version 2.0.1 --channel stable
```

完整发布流程记录在 `docs/RELEASE/RELEASE.zh-CN.md`。Updater key 和签名细节记录在
`docs/ALCOMD3_UPDATER/ALCOMD3_UPDATER.zh-CN.md`。
Agent 发布流程记录在 `docs/skills/alcomd3-release/SKILL.md`。
查找维护、发布、MCP、格式和历史文档时，以 `docs/README/README.zh-CN.md` 作为文档索引。

### 贡献者许可协议

个人贡献者的 pull request 必须在合并前签署 [ALCOMD3 个人贡献者许可协议](../CLA.md)。

CLA 不转让版权；你仍拥有自己贡献的版权。纳入公开项目的贡献按照 ALCOMD3 当前主许可证
`AGPL-3.0-only` 分发，同时 CLA 授予维护者使用、修改、分发和重新许可这些贡献的广泛权利，
包括用于可能的闭源分发。

创建 pull request 后，CLA workflow 会评论签署说明。请回复 workflow 显示的准确文本进行
签署；每个 CLA 版本只需签署一次。

如果你的雇主可能拥有相关权利，或者你代表公司或组织贡献，请先联系维护者，不要直接签署
个人 CLA。

### 外部变更引入

将其他仓库的变更视为选择性引入，而不是 merge 标准：

1. 识别来源 commit、pull request、release note 或 issue。
2. 判断是否影响安全、数据安全、VRChat/VPM 兼容性或用户可见 bug。
3. 按 ALCOMD3 架构适配变更，不要盲目 merge。
4. 验证受影响的 Rust、GUI 和 MCP 代码路径。
5. 在同一 PR 中将每项重要的用户可见变化记录到 `CHANGELOG.md` 的 `Unreleased` 对应分类。

涉及包操作、项目修改、仓库管理、操作取消、资源锁或 MCP 可见性的变更需要额外谨慎。
GUI 和 MCP bridge 应继续共享同一套后端业务逻辑和安全检查。

### 兼容性规则

- 不要随意修改 Tauri identifier、已安装可执行文件名、协议名、用户数据路径或 `vrc-get` 兼容路径。
- URL、版本、颜色或公开路径已有项目配置管理时，不要硬编码。
- ALCOMD3 updater metadata 应指向 ALCOMD3 自有 endpoint。
- 除非经过单独设计审查明确改变边界，否则 MCP 默认关闭且仅限本地。
- 保留既有用户数据和迁移路径。

### Pull request 要求

- PR 保持聚焦。
- 说明用户可见行为变化。
- 包含验证结果；未验证时清楚说明原因。
- 用户可见行为、兼容性、弃用/移除、安全、已知问题、打包或公开配置变化时，在同一 PR 中
  更新文档和 `CHANGELOG.md` 的 `Unreleased` 对应分类。
- Changelog 不是 commit/PR 清单。纯 CI/workflow、测试、内部重构或仅供维护者使用的实现细节，
  除非产生用户可见或发布影响，否则不应写入。
- 避免无关格式化 churn。
