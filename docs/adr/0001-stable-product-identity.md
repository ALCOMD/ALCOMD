# ADR: 稳定产品身份

- 状态：Accepted
- 日期：2026-08-14

## 背景

参见 `docs/architecture/ALCOMD-V4.md`。


## 决策

用户品牌、产品家族和技术身份分离：

- 用户品牌：`ALCOMD3`
- 产品家族：`ALCOMD`
- 技术名称：`alcomd`
- 系统标识 / Bundle ID：`com.cqmhv.alcomd`
- Windows AUMID：`CQMHV.ALCOMD`
- 数据目录：`ALCOMD`

用户品牌可以随产品决策变化，但不得修改产品家族或技术身份。

## 结果

需要集中式 `alcomd.product.toml`、生成/校验脚本和旧标识 CI 扫描。
