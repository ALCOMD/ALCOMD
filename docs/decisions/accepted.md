# 已接受决策

| ID | 决策 |
|---|---|
| A-001 | 当前用户品牌为 ALCOMD3，产品家族为 ALCOMD，技术名称为 `alcomd`，系统标识为 `com.cqmhv.alcomd`，Windows AUMID 为 `CQMHV.ALCOMD` |
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
| A-013 | 已于 2026-08-15 发布的 ALCOMD3 3.4.0 是 v3 迁移入口版本（v3 migration entry release），也是进入 v4 的唯一直接迁移来源；更早的 v3.x 必须先升级到 3.4.0。3.4.0 已将更新 JSON 源切换到 `https://alcomd.cqmhv.com/api/v1/updates/stable.json` 与 `https://alcomd.cqmhv.com/api/v1/updates/beta.json` |
| A-014 | ALCOMD v4 自有代码、SDK、规范、文档、脚本及第一方扩展统一采用 `AGPL-3.0-only`，不自动授权后续版本，也不双重许可 |
| A-015 | ALCOMD v4 是独立新项目；v3 仅作迁移与行为参考，vrc-get 不是上游，禁止复制、移植、Fork、包装或改写二者源码；VPM 独立实现 |
| A-016 | 冻结版本 vrc-get 是 M-1 和 v4 功能对齐所必需的只读功能、安全行为、CLI 与错误处理基线；该基线不产生代码继承、依赖或源码复用关系 |
