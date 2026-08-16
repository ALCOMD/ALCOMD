# ADR: 更新与签名过渡

- 状态：Accepted
- 日期：2026-08-14

## 背景

参见 `docs/architecture/ALCOMD-V4.md`。


## 决策

已于 2026-08-15 发布的 ALCOMD3 3.4.0 是 v3 迁移入口版本（v3 migration entry release），也是进入 v4 的唯一直接迁移来源。更早的公开 v3.x 必须先通过原有更新链升级到 3.4.0。

3.4.0 已将更新 JSON 源切换到 `https://alcomd.cqmhv.com/api/v1/updates/stable.json` 与 `https://alcomd.cqmhv.com/api/v1/updates/beta.json`，并通过该 API 获取与验证 v4 迁移桥接安装器（v4 bridge installer）元数据。v4 迁移桥接安装器安装新身份和新更新信任链，并启动迁移程序。

## 结果

正常 v4 不保留早于 3.4.0 的解析器、旧更新源兼容代码或旧更新信任链。正式密钥不进入开发仓库或普通 Agent 环境。

上述已上线路径和频道映射作为 M-1 基线；v4 迁移桥接安装器的 JSON Schema、版本推进、签名验证和错误行为必须在 M-1 中冻结。

ALCOMD3 3.4.0 Release 当前可变，因此迁移测试不得只信任 tag 或资产名。M-1 锁文件必须记录 Release ID、全部配置声明资产及签名资产的 ID、大小和 SHA-256，以及 updater 公钥指纹；远端快照变化必须使基线校验失败。
