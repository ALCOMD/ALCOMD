# ALCOMD v4 许可证与代码来源决策

状态：**Accepted**

## 许可证

ALCOMD v4 全面采用 GNU Affero General Public License v3.0 only，SPDX 标识为
`AGPL-3.0-only`。仓库根目录的 `LICENSE` 是完整许可证正文。

ALCOMD v4 自有内容的版权声明为：`Copyright (C) 2026 CQMHV`。

该许可统一适用于 ALCOMD v4 自有的：

- 核心、GUI、CLI、MCP、Local API、Bootstrap 与迁移工具；
- Rust/TypeScript 组件、SDK、Schema、规范、文档与脚本；
- 第一方扩展和仓库内示例。

`only` 表示不自动授权使用 AGPL 的后续版本。本项目不对上述自有内容提供双重许可。

## 项目来源

ALCOMD v4 是具有独立 Git 历史与独立代码库的新项目。它在品牌和功能定位上继承
ALCOMD3 v3，但不是 v3 的修改版、派生代码库或增量补丁。

实现必须遵守洁净边界：

- 不复制、移植或改写 ALCOMD3 v3 源码；v3 仅用于功能审计、迁移格式验证和兼容性测试。
- 不复制、移植、Fork 或包装 vrc-get / vrc-get-vpm 源码；vrc-get 不是 ALCOMD v4 的上游。
- VPM 能力依据公开格式、生态兼容需求和本项目规范独立实现。

因此，ALCOMD v4 自有代码不继承 ALCOMD3 v3 或 vrc-get 的源码许可证、版权声明或
署名义务。如果将来确需引入第三方代码，必须单独审核、记录来源并遵守对应许可证，
且不得默认为本决策所授权。

## 第三方材料

第三方依赖、工具和资产不因包含在本仓库或发行包中而被重新许可。它们继续受各自
许可证约束，详见 `THIRD_PARTY_NOTICES.md`；GUI 图标的 CC BY 4.0 许可见
`apps/alcomd-gui/icon-LICENSE`。
