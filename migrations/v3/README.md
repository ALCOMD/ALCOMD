# v3 Migration Area

此目录与正常运行时代码隔离。

规则：

- `alcomd`、GUI、CLI、MCP、API 与扩展不得依赖这里的 crate。
- 旧解析器只由临时 `alcomd-migrate-v3` 使用。
- Fixture 必须脱敏。
- 删除对象必须在 `artifacts.toml` 中记录所有权证据。
- 迁移产物不进入普通 v4 安装。
