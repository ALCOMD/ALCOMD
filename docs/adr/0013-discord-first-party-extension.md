# ADR: Discord 第一方扩展

- 状态：Accepted
- 日期：2026-08-14

## 背景

参见 `docs/architecture/ALCOMD-V4.md`。


## 决策

Discord GUI、Rich Presence 生成与后台生命周期全部由第一方扩展实现。

## 结果

核心仅提供通用 Unity/项目事件和窄 Discord Presence Host Capability。
