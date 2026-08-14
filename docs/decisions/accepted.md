# 已接受决策

| ID | 决策 |
|---|---|
| A-001 | 当前用户品牌为 ALCOMD3，稳定产品家族为 ALCOMD |
| A-002 | 技术名称根固定为 `alcomd` |
| A-003 | Bundle / Tauri identifier 固定为 `com.cqmhv.alcomd` |
| A-004 | `alcomd` 是每用户唯一核心与唯一写入者 |
| A-005 | GUI、CLI、MCP、API 和扩展共用同一应用用例层 |
| A-006 | MCP 协议适配器 `alcomd-mcp` 独立于 GUI |
| A-007 | MCP 管理 GUI 作为第一方扩展 |
| A-008 | Discord Rich Presence 全部作为第一方扩展 |
| A-009 | 第一方扩展只能使用公开 Extension API |
| A-010 | v3、VCC 与旧 ALCOM 格式仅用于迁移或隔离导入 |
| A-011 | v3 成功迁移后执行可验证的零残留清理 |
| A-012 | 新仓库使用无关 Git 历史，最终以破坏性方式替换远端 main |
