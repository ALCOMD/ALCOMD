# ADR: 稳定产品身份

- 状态：Accepted
- 日期：2026-08-14

## 背景

参见 `docs/architecture/ALCOMD-V4.md`。


## 决策

用户品牌、产品家族和技术身份分离：

- 用户品牌：`ALCOMD3`
- 产品家族：`ALCOMD`
- 技术根：`alcomd`
- Bundle ID：`com.cqmhv.alcomd`
- 数据目录：`ALCOMD`

未来品牌变化不得修改技术身份。

## 结果

需要集中式 `alcomd.product.toml`、生成/校验脚本和旧标识 CI 扫描。
