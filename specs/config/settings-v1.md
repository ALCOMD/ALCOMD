# settings.toml Schema 1

状态：Draft

`settings.toml` 保存用户可理解、适合备份和人工检查的公开设置。项目、仓库、扩展、授权、操作与活动记录不应塞入此文件。

示例：

```toml
schema = 1

[appearance]
language = "zh-CN"
theme = "system"

[updates]
channel = "stable"

[paths]
projects = ""
backups = ""
```

规则：

- 文件使用 UTF-8。
- 写入采用临时文件 + 原子替换。
- 未知字段默认保留还是拒绝必须在 Schema 冻结前决定。
- token、私钥、Authorization 和凭据禁止写入。
- 平台默认路径由 `alcomd-platform` 解析，不硬编码英文 Documents。
