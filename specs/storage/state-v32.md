# State32 · 可选软件包展示信息

所有者于2026-11-10明确批准本次 State32 修正方案，对应 [M7 仓库清单可见性修复](../../docs/exec-plans/M7-repository-manifest-visibility.md)。迁移文件为 `0032_optional_package_presentation.sql`。本轮在隔离数据库验证，真实运行实例尚未升级。

移除 resolver-ready INSERT/UPDATE 触发器对 `author_name`、`display_name` 的非空要求，保留列上的现有长度限制，以及 SemVer/归档来源/清单指纹/摘要等要求。缺少作者真实保存 NULL；显示名缺省沿用使用包 ID 的展示，不伪造元数据。没有增删表列、包版本或业务记录。

仅对 `issues_json` 含 `resolver_metadata_invalid` 的仓库标记 `resolver_indexed=0`，清除 etag/last_modified，由现有后台维护完整重新读取。其余来源继续复用健康索引与 HTTP 条件请求。已有包行、归档固定摘要、登记/修订/优先级/自定义名/认证、设置及所有项目/计划/操作保持，不修改冻结 JSON。重新读取依旧走现有事务与版本变化合同。

Core 初始化链在 ready 前执行迁移；成功后 hello/dataSchema 为32，Config13 保持。旧 Core 拒绝高于其支持版本的数据，若回退运行程序，须恢复升级前的离线数据备份。本次批准数据库方案，尚未执行真实程序更新或数据库迁移。
