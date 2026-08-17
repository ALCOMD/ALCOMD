# ALCOMD3 v3 功能基线

状态：审计源已冻结，静态功能审计已完成，安装快照与黑盒 Fixture 未完成

## 冻结信息

从 `docs/baselines/source-lock.toml` 读取。`alcomd3_v3_audit_source` 锁定审计源、冻结时分支
上下文与 commit API 证据，
`alcomd3_v3_migration_entry_release` 单独锁定唯一迁移入口版本，
`alcomd3_v3_migration_assets` 锁定实际安装与迁移测试资产。锁定输入不代表功能审计完成。

## v4 迁移入口

- 已于 2026-08-15 发布的 ALCOMD3 3.4.0 是 v3 迁移入口版本（v3 migration entry release），也是进入 v4 的唯一直接迁移来源。
- 更早的公开 v3.x 版本必须先通过原有更新链升级到 3.4.0，不由 v4 迁移器直接解析。
- 3.4.0 已将更新 JSON 源切换到 `https://alcomd.cqmhv.com/api/v1/updates/stable.json` 与 `https://alcomd.cqmhv.com/api/v1/updates/beta.json`，并从该 API 获取、验签和启动 v4 迁移桥接安装器（v4 bridge installer）；3.4.0 本身不执行完整 v4 替换迁移。
- v4 bridge installer 是完整 v4 产品安装包，包含或调用 `alcomd-bootstrap` 完成迁移协调、健康检查、回滚与清理。
- 上述已上线路径和频道映射作为 M-1 基线；v4 迁移桥接安装器的 JSON Schema、版本推进、签名与失败回退行为必须根据 3.4.0 实现冻结，不得由 v4 猜测。

## v3.4.0 可执行资产快照

GitHub Release 当前没有启用 immutable releases，因此仅锁 tag 和 commit 不足以唯一标识
用户实际安装的二进制。`source-lock.toml` 额外锁定 Release ID，以及从冻结提交中的
`alcomd3.config.json` 派生出的所有三平台 updater、签名和安装资产的 asset ID、名称、
大小与 SHA-256；同时锁定 updater 公钥 blob、规范化指纹和 minisign key ID。

该快照的状态为 `frozen`，含义是远端变化可被 `freeze-baselines.ps1 -Check` 检出，
并不表示 GitHub 端资产本身不可编辑。迁移测试必须按锁文件摘要获取资产，不得只按文件名下载。

## 审计方法

对每个用户功能记录：

| 字段 | 内容 |
|---|---|
| Feature ID | 稳定机器可读 ID |
| 用户入口 | 页面、菜单、URI、CLI 或 MCP |
| 行为 | 正常路径与边界条件 |
| 持久状态 | 文件、数据库、注册表、凭据 |
| 隐性行为 | 锁、进度、取消、错误恢复、日志脱敏 |
| 证据 | 文件路径、符号、截图、测试或安装快照 |
| v4 目标 | 核心用例与入口覆盖 |
| 状态 | unknown / verified |

## 必须审计领域

- 项目注册、扫描、创建、复制、迁移、排序和打开。
- 仓库、用户包、包搜索、安装、移除、升级、降级和冲突。
- Unity Hub / Editor 检测、兼容性、启动和窗口激活。
- 模板、UnityPackage、备份与恢复。
- GUI 设置、主题、多语言、操作进度和错误对话框。
- 活动记录、技术日志和脱敏。
- MCP 的全部工具、任务、权限与配置。
- Discord Rich Presence 的全部设置和生命周期。
- 安装器、更新器、稳定/beta 频道、URI、文件关联和卸载。
- 更早 v3.x 到 3.4.0、3.4.0 更新源切换及 3.4.0 到 v4 迁移桥接安装器的完整更新链。
- Windows 用户安装、全局安装与多用户行为。

## 产物

审计结果必须回填 `feature-parity.toml`，不能只保留在本文。

静态用户入口、持久状态、隐性行为和源码证据见 `alcomd3-v3-audit.md`。其中未经过
真实安装、视觉截图、恶意输入或跨平台实机验证的结论保持待验证，不得据静态结构推断为
已完成的 v4 功能。
