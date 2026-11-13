# MCP loopback HTTP v1

2026-11-13 所有者批准。完整边界及必要依赖见 [M8 已批准合同](../../docs/exec-plans/M8-mcp-protocol-and-management.md#http现状审计与最小合同提案2026-11-13已批准实施中)。这是冻结2026-07-28 Streamable HTTP的本地预配Bearer profile，非完整OAuth；不广告PRM、授权服务器发现、token audience/expiry或浏览器登录。

- `alcomd-mcp serve --port <u16>`，默认0，只绑定`127.0.0.1`；stderr给实际`/mcp` URL，stdout空。仅HTTP/1.1 POST，无重定向、代理、TLS、IPv6或持久网络配置。
- 每请求唯一Bearer为既有64字符MCP凭据，必须新建独立typed Core连接并经hello绑定。Core仍逐RPC及Apply接受事务重新鉴权。无owner回退、跨请求连接池、cookie/URL/STDIO环境凭据继承。
- Host精确匹配实际IP:port，歧义/冲突400，不允许的authority403。拒绝所有存在的Origin，无CORS；query、旧session/replay头400。其他路径404，非POST405/Allow: POST。
- SDK强制无session及逐请求metadata；请求体是单个JSON-RPC request，方法、版本及名称header/body核对由SDK实现。HTTP不接受notifications/cancelled、batch或客户端response。仅现有discover/list/six tools，不广告Tasks、MRTR或订阅。
- 请求头16KiB/32字段，body4MiB流式上限，非identity压缩415；32 TCP/16活跃请求或流，无无界等待队列。头/空闲10秒，body15秒，Core连接/处理60秒，输出10秒；响应预算4MiB，保留3MiB工具数据预算。
- 缺/失效Bearer401，畸形认证400；Accept406、媒体415、body413、头431、读超时408、请求饱和429、业务前Core不可用503。协议错误沿冻结SDK映射；合法工具业务错误仍可HTTP200/isError。SDK原始动态错误不得反射到普通响应。
- 连接关闭、SSE关闭、截止和停机只取消协议exchange，不取消已接受Operation。写响应未知沿`core_write_outcome_unknown`，禁止自动重试；显式以原Plan/revision/key恢复同一结果。Core负责继续执行和恢复。
- 仅请求级SSE及终态JSON，无SSE retry/replay。Cache-Control: no-store；现有tools/list保持ttl0/private。Authorization在进入SDK前移除，无原始header/body/access debug日志，错误有界，内部异常使用opaque diagnostic_id。

本合同没有新增Core RPC、State、Config、权限或工具形状；六工具继续使用`package-removal-v1.schema.json`。HTTP验证必须有实际SDK、独立Bearer与隔离Core证据，不能以STDIO或mock替代。
