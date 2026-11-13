# State33 · MCP 客户端身份

这是已推送的历史迁移合同，0033文件保持原样。当前增量使用[State34授权快照](state-v34.md)承接，初始化须执行到当前版本后才报告ready。

状态：所有者于2026-11-13批准；实施范围见 M8 ExecPlan 的最小身份合同。

State32→33 只增加 `mcp_clients`、`mcp_client_grants`、`mcp_client_credentials`，保留全部旧表、行、索引、任务、计划、幂等记录及 Config13。不新增持久会话或查询缓存表。

- 客户端 UUID 对应 Core 生成的 `mcp:<ClientId>` Principal，名称1–128字符、正 revision，最多64个客户端（包括已撤销记录）。默认无 grant。
- grant 为 permission/resourceKind/resourceId，主键去重，最多64种权限/4096条作用范围。合法权限及适用范围由 application 复验，SQLite 不自行授予权限。
- 凭据只保存32字节摘要、随机 credentialId、签发/撤销时间。每客户端至多一个有效凭据，轮换在单事务中撤销旧摘要、登记新摘要并提升 revision。无明文、加密明文或可恢复 token。
- 写命令复用现有 principal/method/idempotency key 记录。issueCredential 的幂等记录仅保存非秘密签发结果；重复请求不能重放明文或重复轮换，返回已有稳定冲突要求显式重新签发，不把响应未知误报成未签发。
- 客户端撤销不删除 grant、凭据历史或 Operation；认证/每次调用均检查撤销和当前授权。外键禁止孤立 grant/credential；事务失败不得部分轮换或部分替换授权。

迁移使用既有初始化链、单事务执行，只有 ready 后才报告 State33。旧 Core 会拒绝较新数据；运行环境升级前仍按既有离线备份/恢复流程执行。新增 Schema 不代表认证、RPC 或管理扩展已经可用，capability 仅在实际接线完成后发布。本轮不迁移原开发环境。
