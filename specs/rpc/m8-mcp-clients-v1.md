# MCP 客户端身份 RPC v1

状态：2026-11-13 所有者批准。结构见同目录 `m8-mcp-clients-v1.schema.json`，实现与验证记录见 M8 ExecPlan。Core 身份接线验证后才协商 `mcp.clients.v1`，管理方法同时要求该 capability 与原有权限。

`system.hello.authentication` 的可选身份为 `{kind:"mcp-client",credential:string}`；Core 验证后绑定自身签发的 Principal。MCP 业务连接不得省略凭据或在认证失败时切换至 local-owner。现有本机 GUI/CLI 的 OS IPC 模式保持。认证字段须与 Core 的绑定和逐调用复查同时接入，禁止先接受字段、后补校验。

`mcp.clients.list/create/setGrants/issueCredential/revoke` 沿用 RPC envelope、原有幂等键与稳定错误。list 使用 opaque cursor/limit，cursor 绑定当前集合、调用者可见范围及授权快照；集合或权限变化拒绝混页。create 默认空 grants；setGrants 完整替换且要求 expectedRevision；revoke 保留记录和已有任务。权限/资源组合必须由 application 验证，DTO 反序列化不能代替授权。

grant 名称、资源种类及 opaque 资源 ID 为有界 ASCII，不能以路径或通配 owner 代替资源授权。分页同时受 RPC 帧字节上限约束，不能因客户端数量最多64就把全部 grant 一次塞入响应；到达字节预算时返回下一游标。Unsupported 的权限/范围组合明确拒绝。

本批支持 `projects.read/project/<ProjectId>`；`state.check`、`operations.read`、`operations.cancel`、`events.read`、`repositories.read`、`packages.read` 的范围为 `global/*`。项目必须已登记，重复或其他组合拒绝；这不代表 MCP 业务工具已经开放。项目读取连接当前授权快照，不改变资源 owner；Operation、Plan、事件可见性和幂等键按实际调用 Principal 隔离。客户端不能调用未接通范围检查的入口，项目集合的专用视图筛选暂不开放。

2026-11-13获批首条包业务切片增加支持`packages.manage/project/<ProjectId>`组合，不新增权限名称或身份接口。Apply必须同时具有同项目read、既有仓库/包read及operations.read；当前元组在接受事务内复查，不接受global包写授权。仅原remove Plan可由MCP委托执行，项目owner不变，已接受任务的执行/恢复绑定其原Operation/Plan/journal，之后的撤销不得被幂等重放绕过，也不自动取消既有任务。

写入使用既有 `idempotencyKey`，更换授权和撤销要求 `expectedRevision`。普通成功响应按原接受时的非秘密元数据及不可变授权快照精确重放；历史响应不是当前授权证明，当前状态须重新 list。State34 的当前快照指针是唯一有效授权来源，重放旧结果不会恢复权限。

`issueCredential` 仅 local-owner，返回一次明文和非秘密标识/revision。凭据不进入通用命令响应持久化、日志、事件或普通页面；重试不能因无法重放明文而再生成凭据。响应未知时保持“已签发、明文不可恢复”的事实，要求用户显式重新签发并撤销旧凭据，不自动重试写入。请求的 clientInfo、actor、资源 ID 均不能证明身份。

已接受签发的同键重试返回既有 `revision_conflict` 并说明一次性响应不可重放；既不返回明文，也不再次轮换。凭据为32字节OS随机数的64位十六进制表示，Core只持久化SHA-256摘要。受控 STDIO 从 `ALCOMD_MCP_CLIENT_CREDENTIAL` 读取启动凭据并交给 typed client；没有凭据时只提供无Core连接的协议发现，认证失败不降级。自动启动 daemon 时不继承该环境变量；MCP请求取消及连接关闭不取消已经接受的Core业务任务。

本批是现有 RPC 的兼容增加（旧本机模式、Config13 保持）；保留已推送State33原迁移，另经所有者批准增加[State34授权快照](../storage/state-v34.md)，旧Core不能打开新版数据。本批不升级原开发环境；不是 MCP wire 协议变更。HTTP OAuth、Host/WIT 桥和工具 ABI 不在这次批准中，尚未实现的接口不得广告为可用。
