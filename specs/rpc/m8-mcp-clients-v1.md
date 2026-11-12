# MCP 客户端身份 RPC v1

状态：2026-11-13 所有者批准，实施中。结构见同目录 `m8-mcp-clients-v1.schema.json`，授权与安全边界见 M8 ExecPlan。DTO/Schema 和迁移落地不等于 capability 已开放。

`system.hello.authentication` 的可选身份为 `{kind:"mcp-client",credential:string}`；Core 验证后绑定自身签发的 Principal。MCP 业务连接不得省略凭据或在认证失败时切换至 local-owner。现有本机 GUI/CLI 的 OS IPC 模式保持。认证字段须与 Core 的绑定和逐调用复查同时接入，禁止先接受字段、后补校验。

`mcp.clients.list/create/setGrants/issueCredential/revoke` 沿用 RPC envelope、原有幂等键与稳定错误。list 使用 opaque cursor/limit，cursor 绑定当前集合、调用者可见范围及授权快照；集合或权限变化拒绝混页。create 默认空 grants；setGrants 完整替换且要求 expectedRevision；revoke 保留记录和已有任务。权限/资源组合必须由 application 验证，DTO 反序列化不能代替授权。

grant 名称、资源种类及 opaque 资源 ID 为有界 ASCII，不能以路径或通配 owner 代替资源授权。分页同时受 RPC 帧字节上限约束，不能因客户端数量最多64就把全部 grant 一次塞入响应；到达字节预算时返回下一游标。Unsupported 的权限/范围组合明确拒绝。

`issueCredential` 仅 local-owner，返回一次明文和非秘密标识/revision。凭据不进入通用命令响应持久化、日志、事件或普通页面；重试不能因无法重放明文而再生成凭据。响应未知时保持“已签发、明文不可恢复”的事实，要求用户显式重新签发并撤销旧凭据，不自动重试写入。请求的 clientInfo、actor、资源 ID 均不能证明身份。

本批是现有 RPC 的兼容增加（旧本机模式、Config13 保持），State33 为新增表迁移；不是 MCP wire 协议变更。HTTP OAuth、Host/WIT 桥和工具 ABI 不在这次批准中，尚未实现的接口不得广告为可用。
