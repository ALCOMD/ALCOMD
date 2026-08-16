# vrc-get 功能、安全与行为基线

状态：审计源已冻结，静态行为审计已完成，黑盒与恶意 Fixture 验证未完成

## 定位

vrc-get 是 ALCOMD v4 必须覆盖的功能与行为对照基线之一，但不是代码上游。
`source-lock.toml` 中固定的 commit、版本和 describe 值用于确保功能审计、黑盒行为观察、
安全测试与差异测试可复现。ALCOMD 不直接依赖、复制、Fork、包装、移植或改写
vrc-get / vrc-get-vpm 源码，所有生产实现必须独立完成。

vrc-get 基线与 `vpm-ecosystem.md` 分工如下：

- 本文冻结特定 vrc-get 版本实际提供的功能、CLI、安全行为和错误行为。
- `vpm-ecosystem.md` 冻结公开格式、外部兼容要求和跨实现 Fixture。
- 两者共同构成 VPM 与项目管理能力的验收输入，互不替代。

## 锁定信息

权威值由 `scripts/freeze-baselines.ps1` 确定性写入
`docs/baselines/source-lock.toml` 的 `[vrc_get_function_behavior]`，包括：

- 仓库和完整 commit。
- CLI 与 vrc-get-vpm 的 describe 值。
- CLI、GUI 和 vrc-get-vpm 版本。
- 审计范围与禁止源码复用标志。

`scripts/freeze-baselines.ps1 -Check` 必须能够证明本地只读仓库与锁文件完全一致。

## 必须审计的领域

- CLI 命令、参数、标准输出、标准错误、交互方式和退出码。
- 项目发现、创建、迁移、Unity 版本检测、启动和状态判断。
- VPM repository 解析、优先级、缓存、刷新和失效行为。
- 包搜索、依赖解析、版本选择、安装、移除、升级和 outdated。
- 包下载、哈希校验、归档解压、路径规范化和不可信输入处理。
- 项目文件事务、并发冲突、中断恢复、失败回滚和部分成功处理。
- 无效参数、网络失败、解析失败、依赖冲突和文件系统失败的错误分类与可观察行为。
- Windows、Linux、macOS 的路径、权限、进程与平台差异。

## 审计产物

每项能力必须进入细粒度 `feature-parity.toml`，并记录：

- `vrc-get-frozen` 来源标识。
- 对应命令、输入、输出、退出码或状态变化。
- 安全边界和错误行为。
- 可复现证据与差异测试计划。
- ALCOMD 独立实现的 GUI、CLI、RPC、MCP、API 或 Extension 覆盖要求。

仅锁定 commit 不代表审计完成。上述用户入口和行为未逐项建档、取证并纳入验收测试前，
M-1 必须保持 `in_progress`。

静态审计结果、源码证据、安全差异与黑盒测试要求见 `vrc-get-audit.md`。该文档记录的是
可观察行为和独立实现验收输入，不授权复用实现结构。
