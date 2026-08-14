# ADR: v3 事务迁移与零残留

- 状态：Accepted
- 日期：2026-08-14

## 背景

参见 `docs/architecture/ALCOMD-V4.md`。


## 决策

迁移先在 staging 中构建并验证 v4，再进入 Commit 和旧资源清理。

## 结果

失败时恢复 v3；成功时迁移后的系统必须等价于全新 v4 加相同用户数据。
