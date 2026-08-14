# 开放决策

以下事项必须由项目所有者确认，Codex 不得自行拍板。

## O-001 最终许可证

候选：继续 `AGPL-3.0-only`。
必须确认核心、GUI、SDK 与第一方扩展是否统一许可证，以及旧代码复用的署名方式。

## O-002 v3 迁移入口范围

选择之一：

1. 所有公开 v3.x 版本可直接迁移。
2. 用户必须先升级到最后一个 v3 Bridge 版本，再迁移 v4。

第二种能显著缩小解析与测试矩阵。

## O-003 4.0.0 发布平台

是否将以下全部设为 Release Blocker：

- Windows x64 用户安装。
- Windows x64 全局安装。
- macOS Apple Silicon。
- Linux x64 AppImage。
- Linux x64 DEB。

## O-004 Local API 与多语言 SDK

`alcomd-api`、Python SDK、.NET SDK 是否属于 4.0.0 发布门槛。建议 native RPC 与 TypeScript/Rust SDK 为 v4 必需，其余可在兼容 API 上后续交付。

## O-005 第一方扩展默认状态

候选规则：

- MCP 管理扩展：默认安装并启用。
- Discord 扩展：默认安装但新用户默认禁用。
- v3 升级用户：按 v3 Discord 启用状态迁移。

## O-006 GUI 允许变化范围

需要冻结 v3 页面截图、交互流程、错误状态与进度状态，并明确哪些视觉/流程变化属于必要变化。

## O-007 MCP 长任务映射

MCP 候选基线 `2026-07-28` 将 Tasks 置于扩展轨道。需要决定：

- 只返回 ALCOMD `OperationId` 并提供查询工具。
- 同时实现官方 MCP Tasks 扩展。
- 分阶段实现。

## O-008 Windows 安装器技术

选择 NSIS/Inno Setup、WiX/MSI 或自定义 Bootstrap 组合，并冻结永久 AppId / UpgradeCode。

## O-009 扩展 WASM Component ABI

选择 WASI Preview / Component Model 版本、运行时、资源上限、能力注入方式与兼容策略。

## O-010 上游 VPM 复用策略

选择：

- 直接依赖并包裹 `vrc-get-vpm`。
- Fork 后内部维护。
- 以行为对齐为目标重新实现。

无论选择哪项，应用层只能依赖 `alcomd-vpm` 门面。
