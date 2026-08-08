# 参与 ALCOMD3 贡献

语言：[English](../CONTRIBUTING.md) | [日本語](CONTRIBUTING.ja.md) | 简体中文

感谢你帮助改进 ALCOMD3。我们欢迎问题报告、功能建议、文档改进、翻译、测试和代码修改。

## 开始修改之前

- 先搜索已有的 [Issue](https://github.com/ALCOMD3/ALCOMD3/issues) 和
  [Discussion](https://github.com/ALCOMD3/ALCOMD3/discussions)。
- 使用 [Issue 表单](https://github.com/ALCOMD3/ALCOMD3/issues/new/choose)报告问题或建议功能，
  使用 Discussions 提问。
- 小型修复可以直接提交。大型功能、兼容性变化或架构调整应先讨论，再开始实现。
- 讨论时请保持尊重，并以解决问题为目标。
- 不要公开报告安全漏洞，请发送邮件至
  [github@cqmhv.com](mailto:github@cqmhv.com)。

## 开发环境

你需要 `alcomd3.config.json` 中指定的 Rust 工具链、Node.js 24，以及
[Tauri v2 要求的目标平台依赖](https://v2.tauri.app/start/prerequisites/)。

克隆自己的 Fork 后，安装 GUI 依赖并启动应用：

```bash
cd vrc-get-gui
npm ci
npm run tauri dev
```

## 修改要求

- 保持修改范围集中，并遵循现有代码风格。
- 行为变化时新增或更新测试。
- 用户可见文本必须通过本地化系统添加，具体说明见
  [GUI 贡献指南](../vrc-get-gui/CONTRIBUTING/CONTRIBUTING.zh-CN.md)。
- 行为或公开配置变化时更新相关文档。
- 重要的用户可见变化或影响发布的变化，应写入 `CHANGELOG.md` 的相应 `Unreleased` 分类。
  内部重构、测试、格式调整或仅涉及 CI 的修改不需要记录。
- 部分 `vrc-get` 名称因兼容性仍需保留，不要将其作为普通清理内容重命名。具体边界见
  [MAINTENANCE.md](../docs/MAINTENANCE.md)。

## 检查

请运行与修改范围相关的检查，完整说明见 [TESTING.md](../docs/TESTING.md)。

Rust 修改：

```bash
cargo fmt --all --check
cargo clippy --workspace --exclude windows-installer-wrapper --all-targets --locked -- -D clippy::correctness
cargo check --workspace --exclude windows-installer-wrapper --locked
cargo test --workspace --exclude windows-installer-wrapper --locked
```

GUI 修改请在 `vrc-get-gui/` 中运行：

```bash
npm run check
npm run lint
npm test
npm run build
```

如果无法运行相关检查，请在 Pull Request 中说明。

## Pull Request 与 CLA

Pull Request 应说明问题和解决方案、关联相关 Issue、列出已运行的检查，并为界面可见变化提供
截图。不要在同一个 Pull Request 中混入无关修改。

个人贡献者的 Pull Request 必须在合并前签署 [Contributor License Agreement](../CLA.md)。
CLA 工作流会提供签署方法。如果雇主可能拥有你的贡献，或你代表组织贡献，请在签署前发送邮件至
[github@cqmhv.com](mailto:github@cqmhv.com) 联系维护者。
