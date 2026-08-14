# vrc-get 功能与安全基线

状态：未冻结

## 冻结要求

在 `docs/baselines/source-lock.toml` 中填写固定 commit、tag 和版本。冻结后，v4.0.0 的验收终点不再随上游移动。

## 审计领域

- CLI 命令和退出码。
- VPM repository 解析、优先级与缓存。
- 包搜索、依赖解析、安装、移除、升级和 outdated。
- Unity 版本兼容性。
- 包哈希与下载校验。
- 项目文件事务与错误恢复。
- 路径安全。
- 跨平台差异。
- 上游 crate 的许可证、公开 API 与维护风险。

## 复用决策

必须比较：

1. 直接依赖并通过 `alcomd-vpm` 包裹。
2. Fork 后内部维护。
3. 行为对齐的重新实现。

最终选择必须写入 ADR。
