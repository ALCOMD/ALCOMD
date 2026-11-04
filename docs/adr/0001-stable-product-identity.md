# ADR: 稳定产品身份

- 状态：Accepted
- 日期：2026-08-14
- 组织身份修订：2026-11-05，所有者明确批准在首次公开发布前统一到 VRCLearn。

## 背景

参见 `docs/architecture/ALCOMD-V4.md`。


## 决策

用户品牌、产品家族和技术身份分离：

- 用户品牌：`ALCOMD3`
- 产品家族：`ALCOMD`
- 技术名称：`alcomd`
- 发布者：`VRCLearn`
- 系统标识 / Bundle ID：`com.vrclearn.alcomd`
- Windows AUMID：`vrclearn.ALCOMD`
- 数据目录：`ALCOMD`
- 当前站点：`https://alcomd.vrclearn.com/`

用户品牌可以随产品决策变化，但不得修改产品家族或技术身份。

此次组织身份替换同步派生的第一方扩展 ID 和 Windows IPC 命名空间，不保留旧 v4 身份别名。
Inno Setup AppId 仍待 M12 单独确定；Windows AUMID 当前仅为声明，GUI 显式应用仍待接入。

## 结果

需要集中式 `alcomd.product.toml`、生成/校验脚本和旧标识 CI 扫描。
