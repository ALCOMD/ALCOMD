# State1 客户端管理申请

本项细化已核定的 C3/C4。下表保存申请人看到的不可变目标和所有者的决定。凭据和 OAuth 机密只通过首次批准响应交付。创建和决定分别复用现有调用者、方法及幂等键回执。现行 State1 重开逻辑按完整 DDL 核对本表和索引。

```sql
CREATE TABLE mcp_client_requests (
    request_id TEXT PRIMARY KEY NOT NULL CHECK(length(request_id)=36),
    requester TEXT NOT NULL CHECK(length(requester) BETWEEN 1 AND 128),
    action_json TEXT NOT NULL CHECK(json_valid(action_json) AND length(action_json)<=65536),
    fingerprint TEXT NOT NULL CHECK(length(fingerprint)=64),
    created_at_ms INTEGER NOT NULL CHECK(created_at_ms>=0),
    expires_at_ms INTEGER NOT NULL CHECK(expires_at_ms>created_at_ms),
    status TEXT NOT NULL CHECK(status IN ('pending','approved','denied')),
    decided_by TEXT,
    decided_at_ms INTEGER,
    CHECK((status='pending' AND decided_by IS NULL AND decided_at_ms IS NULL)
        OR (status<>'pending' AND decided_by IS NOT NULL AND decided_at_ms IS NOT NULL
        AND decided_at_ms>=created_at_ms AND decided_at_ms<expires_at_ms))
) STRICT;
CREATE INDEX mcp_client_requests_expiry ON mcp_client_requests(status,expires_at_ms);
CREATE INDEX mcp_client_requests_actor ON mcp_client_requests(requester,created_at_ms,request_id);
```

过期是按 Core 时钟计算的状态。创建时间、申请人、完整动作（含目标修订）、到期时间和 ID 共同确定审批指纹。公开查询根据已认证调用者筛选记录，所有者决定在同一 SQLite 事务中写入动作结果与决定回执。更改不符合当前定义的开发数据库仍按普通结构校验失败。
