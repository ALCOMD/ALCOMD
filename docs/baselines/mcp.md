# MCP 协议基线

冻结规范：[`2026-07-28`](https://modelcontextprotocol.io/specification/2026-07-28)

状态：M-1 核心规范、conformance 输入与工具合同基线已冻结；正式 Schema 与实现留在后续里程碑

最后核验：2026-08-16

`alcomd-mcp` 是无业务状态的协议适配器，核心长任务的权威状态始终是 ALCOMD
Operation。当前应用仍是 scaffold，本页描述验收基线，不代表协议已经实现。

## 官方核心 MUST/SHOULD

- 无协议级 session，不使用旧 `initialize` / `notifications/initialized`、`Mcp-Session-Id`、
  HTTP GET/DELETE session 端点或 `Last-Event-ID` 恢复。
- 每个 Request 的 `params._meta` 独立携带协议版本和客户端能力；`clientInfo` 为 SHOULD，
  但 client/server info 均为自报信息，禁止用于身份或授权判断。能力不得跨请求继承。
- 每个成功 Result 含 `resultType`；普通完成结果为 `complete`。服务端 MUST 实现
  `server/discover`，并在响应 metadata 中提供 server info。
- Streamable HTTP 每个 JSON-RPC request/notification 使用独立 POST。`Accept` 同时声明
  `application/json` 与 `text/event-stream`；响应可为单一 JSON 或仅覆盖该请求的 SSE。
- HTTP 每个 POST 携带 `MCP-Protocol-Version`，每个 Request 携带 `Mcp-Method`；只有
  `tools/call`、`resources/read`、`prompts/get` 等规范指定方法要求 `Mcp-Name`。header 与
  body 不一致返回 HTTP 400 / `-32020`，非 ASCII 名称必须使用规范 Base64 形式。
- 若工具 Schema 采用 `x-mcp-header`，HTTP 客户端必须镜像 `Mcp-Param-*`，服务端必须与
  body 逐值核对；这些 header 不得单独成为授权输入。
- HTTP 服务 MUST 校验 Origin；本地服务还必须按威胁模型校验 Host、仅绑定 loopback 并对
  每个请求认证，防止 DNS rebinding 和跨站请求。
- STDIO 使用 UTF-8、单行 newline-delimited JSON-RPC；stdout 只能输出 MCP，日志进入
  stderr。EOF 为优雅停机，服务端不得主动发 JSON-RPC Request。
- 订阅仅使用 `subscriptions/listen`。首个流消息先确认实际接受的 filter，通知带相同
  subscription ID；请求级 progress/message 不得混入订阅流。
- 普通请求取消只取消该请求。HTTP 关闭对应 SSE；STDIO 使用
  `notifications/cancelled(requestId)`。未知、已完成或畸形取消应忽略。
- Multi Round-Trip Requests 只适用于 `prompts/get`、`resources/read`、`tools/call`。
  `requestState` 视为不可信输入；涉及授权或业务状态时必须防篡改，并绑定 Principal、方法、
  参数摘要和短 TTL。中间态和 retry 不得缓存。
- `complete` 的 discover、tools/list、prompts/list、resources/list、
  resources/templates/list、resources/read 结果必须带非负 `ttlMs` 与
  `public|private cacheScope`。通知使对应缓存失效；按权限过滤的结果默认 private。
- 核心错误至少冻结 `-32020 HeaderMismatch`、`-32021 MissingRequiredClientCapability`、
  `-32022 UnsupportedProtocolVersion` 与标准 JSON-RPC 错误，并映射正确 HTTP status。
- 新实现不广告已弃用的 Roots、Sampling、Logging、旧 HTTP+SSE、旧资源订阅方法或服务器
  主动 Request。MRTR 内嵌的相关结构只在当前请求明确声明能力时使用。

## ALCOMD 更严格的产品合同

- 列表在同一数据 revision 下使用稳定排序。MCP 只要求缓存提示，且官方明确分页不保证跨页
  快照一致；稳定顺序是 ALCOMD 自己的确定性要求，不冒充协议 MUST。
- 自定义 metadata 使用 `com.cqmhv.alcomd/...` 命名空间；W3C trace 字段校验后才转发，
  baggage 不进入日志。
- 每个 MCP 请求只映射公开 ALCOMD RPC Query/Command，再进入 application use case；适配器
  不直连 SQLite、项目、VPM、GUI 私有命令，也不成为业务状态持有者。
- HTTP Principal 来自每请求 Bearer 授权上下文，STDIO Principal 来自受控启动配置；禁止用
  `clientInfo`、工具名、OperationId 或 taskId 证明身份。
- 当前 RPC 合同按连接绑定 Principal。无 session HTTP 端点可能混入多个 token，因此不同
  Principal 不得复用同一个后端 RPC 连接，除非未来经审批引入可验证的逐请求委托合同。
- 写工具保留 Plan/Apply、审批、幂等键、expected revision 与资源锁。响应丢失后以幂等键
  找回同一 Operation，不得重复写入。
- GUI 未运行，或 MCP 管理扩展被禁用/卸载时，STDIO 与 HTTP 协议服务仍必须可用。

## 请求取消与 Operation 取消

关闭 HTTP 流或发送 STDIO request cancellation 只终止适配器对当前请求的处理。它不得默认
取消已经创建或已经返回 `OperationId` 的核心 Operation。只有显式
`alcomd_cancel_operation` 才请求核心执行合作式业务取消；未来若兼容增加 Tasks，
`tasks/cancel` 也必须遵守相同 Operation 合同。

当前候选工具只有 get/cancel，无法为已经返回的 `waiting_for_input` Operation 提交审批、输入
或 resume。MRTR 不能充当持久 Operation 输入通道。A-021 已决定 4.0.0 补显式
input/approve/reject/resume 工具，不采用 `tasks/update`。

## Tasks 扩展阻塞

Tasks SEP 已是 Final，官方发布把它列为正式扩展；但所审扩展实现仓库/页面仍带 Draft 或
experimental 标记，并使用 `-32003`，与最终核心 Schema 的 `-32021` 冲突。未来若兼容增加
Tasks，必须另行冻结扩展仓库 commit、Schema/文档摘要、兼容 core 版本和 conformance suite；
不能只凭 SEP 状态或核心日期，也不能在冲突关闭前把当前 artifact 当作稳定 ABI。

A-021 已决定 4.0.0 不采用 Tasks：discover 不广告该扩展，`tools/call` 不返回
`resultType=task`。未来兼容增加前仍必须冻结 Operation→Task 状态映射、retention/TTL、
错误结果语义、Principal owner 校验、输入处理和取消语义。

## 已确认的本地冲突

- `specs/mcp/toolset-v1.md` 末句把 transport request cancellation 与 Operation cancellation
  混在一起；该公开工具合同需人工审批后修订。
- A-023 已批准以 `mcp.requests.read`、`mcp.connections.read` 和
  `mcp.subscription-streams.read` 替代 `mcp.sessions.read`；`specs/` 的实际合同修改必须在允许
  修改该范围的里程碑执行并更新 Schema/快照。
- `specs/rpc/alcomd-rpc-v1.md` 的连接 Principal 模型尚未定义 MCP HTTP 多 Principal 隔离。
- 旧 `specs/mcp/toolset-v1.md` 的 8 个候选工具尚未更新为 A-026 合同；M-1 已在
  `docs/baselines/mcp-tool-contract.md` 冻结 33 个用例映射、权限、错误、Plan/Apply 与
  waiting-for-input 方向，正式 input/output Schema、快照和实现仍由后续里程碑完成。

## 可复现输入

`docs/baselines/source-lock.toml` 已冻结：

- 规范仓库 commit `4df2d6b6e3588efb46e7542d98498e5c630a0a86`。
- `schema/2026-07-28/schema.ts` 的 Git blob 与 SHA-256。
- `@modelcontextprotocol/conformance@0.2.0-alpha.9` 的 npm tarball URL、SHA-1 与 SRI。

conformance 仍是 alpha 测试输入，不代表 Tasks Draft 已稳定，也不自动替代 ALCOMD 自己的安全
与 Operation 差异测试。官方材料若有示例与最终 Schema/传输正文冲突，以冻结的最终 Schema
与传输正文为准，并记录差异测试。下列 `main` 链接仅供人阅读，机器基线使用锁文件中的 commit。

官方入口：

- `https://github.com/modelcontextprotocol/modelcontextprotocol/blob/main/blog/content/posts/2026-07-28-spec-ga/index.md`
- `https://github.com/modelcontextprotocol/modelcontextprotocol/tree/main/docs/specification/2026-07-28`
- `https://github.com/modelcontextprotocol/modelcontextprotocol/blob/main/schema/2026-07-28/schema.ts`

## M-1 工具合同基线

`docs/baselines/mcp-tool-contract.md` 已将冻结 v3 的 33 个用户工具逐项映射到 v4 工具、公开
RPC 用例、最小权限、资源 scope、Plan/Apply、Operation 和错误族。A-026 已批准公开工具名
作为正式 Schema 命名基线、`diagnostics.read` 与结构化错误方向；M-1 仍不修改 `specs/` 或实现。

## 后续里程碑工作

- MCP 工具合同的后续正式 Schema/快照与兼容别名策略实现。
- MCP 客户端 OAuth scope 到 A-023 权限/Principal 的映射细节。
- `waiting_for_input` 的 input/approve/reject/resume 工具 Schema。
- Rust SDK 候选版本、MSRV、规范覆盖和 conformance 版本；M-1 不新增生产 SDK 依赖。
