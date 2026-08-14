# ADR: 更新与签名过渡

- 状态：Accepted
- 日期：2026-08-14

## 背景

参见 `docs/architecture/ALCOMD-V4.md`。


## 决策

4.0.0 使用旧 v3 更新信任链分发 Bridge；Bridge 安装新身份和新更新信任链。

## 结果

正常 v4 不保留旧更新兼容代码，正式密钥不进入开发仓库或普通 Agent 环境。
