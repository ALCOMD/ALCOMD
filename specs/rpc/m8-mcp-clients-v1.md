# MCP 客户端身份 RPC v1

历史合同。现行接口见 [MCP 客户端身份 v2](m8-mcp-clients-v2.md)。

`system.hello.authentication` 的可选身份为 `{kind:"mcp-client",credential:string}`；Core 验证后绑定自身签发的 Principal。MCP 业务连接不得省略凭据或在认证失败时切换至 local-owner。现有本机 GUI/CLI 的 OS IPC 模式保持。认证字段须与 Core 的绑定和逐调用复查同时接入，禁止先接受字段、后补校验。

`mcp.clients.list/create/setGrants/issueCredential/revoke` 沿用 RPC envelope、原有幂等键与稳定错误。list 使用 opaque cursor/limit，cursor 绑定当前集合、调用者可见范围及授权快照；集合或权限变化拒绝混页。create 默认空 grants；setGrants 完整替换且要求 expectedRevision；revoke 保留记录和已有任务。权限/资源组合必须由 application 验证，DTO 反序列化不能代替授权。

grant 名称、资源种类及 opaque 资源 ID 为有界 ASCII，不能以路径或通配 owner 代替资源授权。分页同时受 RPC 帧字节上限约束，不能因客户端数量最多64就把全部 grant 一次塞入响应；到达字节预算时返回下一游标。Unsupported 的权限/范围组合明确拒绝。

2026-11-15 所有者批准统一能力授权。`projects.read`、`packages.manage`、`state.check`、`operations.read`、`operations.cancel`、`events.read`、`repositories.read`、`packages.read` 均只接受 `global/*`。覆盖当前及未来登记项目，Operation、Plan 和幂等记录按实际 Principal 隔离，项目 ID 只作操作目标。仓库、项目与备份流程允许显式授予既有的 `repositories.manage`、`projects.manage`、`projects.create`、`backups.read`、`backups.manage`。环境读取沿用 `settings.read` 与 `unity.read`，完整项目启动配置同时要求 `projects.read`。均使用 `global/*`。未知权限、重复及其他组合拒绝。

包修改要求显式`packages.manage/global/*`，不能由read推导；接受事务仍复查项目read、仓库/包read及operations.read。只Apply原remove Plan、保持明确确认、原Principal及资源owner，既有接受任务/journal与撤销边界不变。State37为旧逐项目授权建立去除该授权的新当前快照、递增revision；旧不可变快照仅保留历史，不自动扩大为global，须重新明确授予。

写入使用既有 `idempotencyKey`，更换授权和撤销要求 `expectedRevision`。普通成功响应仅在其授权快照符合现行能力格式时，按原接受时的非秘密元数据及不可变授权快照精确重放；历史响应不是当前授权证明，当前状态须重新 list。State34 的当前快照指针是唯一有效授权来源，重放旧结果不会恢复权限。

`issueCredential` 仅 local-owner，返回一次明文和非秘密标识/revision。凭据不进入通用命令响应持久化、日志、事件或普通页面；重试不能因无法重放明文而再生成凭据。响应未知时保持“已签发、明文不可恢复”的事实，要求用户显式重新签发并撤销旧凭据，不自动重试写入。请求的 clientInfo、actor、资源 ID 均不能证明身份。

已接受签发的同键重试返回既有 `revision_conflict` 并说明一次性响应不可重放；既不返回明文，也不再次轮换。凭据为32字节OS随机数的64位十六进制表示，Core只持久化SHA-256摘要。受控 STDIO 从 `ALCOMD_MCP_CLIENT_CREDENTIAL` 读取启动凭据并交给 typed client；没有凭据时只提供无Core连接的协议发现，认证失败不降级。自动启动 daemon 时不继承该环境变量；MCP请求取消及连接关闭不取消已经接受的Core业务任务。

本批是现有 RPC 的兼容增加（旧本机模式、Config13 保持）；保留已推送State33原迁移，另经所有者批准增加[State34授权快照](../storage/state-v34.md)，旧Core不能打开新版数据。本批不升级原开发环境；不是 MCP wire 协议变更。HTTP OAuth、Host/WIT 桥和工具 ABI 不在这次批准中，尚未实现的接口不得广告为可用。

2026-11-16所有者明确未发布版本不提供逐项目授权兼容：新请求里的不支持组合返回既有invalid_request；列表、认证、授权快照读取和幂等重放若遇到该存储格式，按既有读取不可用错误拒绝整次结果，不过滤成空授权、不转换成global，也不返回旧格式响应。OAuth token既有格式校验同样拒绝。迁移保留的原始历史行仅是不可变记录，不是可解释的授权格式；State37的开发数据清理不构成运行时兼容层。
