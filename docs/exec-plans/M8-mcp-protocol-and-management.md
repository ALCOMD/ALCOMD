# M8：MCP 协议与管理扩展

状态：IN_PROGRESS，2026-11-18。所有者已批准按既有计划进入 M8；不自动推送，不启动 M9–M12。

## 既有范围与复用核对

本记录落实架构第 16 节、ADR-0012、A-021/A-023/A-026 及冻结的 MCP 基线，不重新规划项目。

| 能力 | 进入 M8 时的已有实现 | 本阶段处理 |
| --- | --- | --- |
| Core 用例、Plan/Apply、Operation、事件、权限 | M1–M7 已实现 | 通过已有 RPC 复用，不让适配器写数据库或项目 |
| 扩展签名、发布者信任、WASM Host、作用范围、Portable UI | M6/M7 已实现 | 复用真实安装和同窗口界面，不新建运行时 |
| `alcomd-mcp` STDIO / HTTP | 仅命令行占位，未处理协议 | 先实现独立协议基础，再接入获批的身份和工具合同 |
| MCP 管理扩展 | manifest / README 占位，无 Component | 只补本阶段管理功能，不实现 Discord 或其他扩展业务 |
| MCP 工具合同 | `specs/mcp/toolset-v1.md` 为旧 Draft，M-1 是方向基线 | 按当前 RPC/产品范围提出具体差异，审批后实施；不复活已撤销 Activity/Diagnostics 权限或旧模板模型 |
| 外部客户端身份 | 当前普通本机 IPC 连接固定为 local-owner | 必须补 Core 权威身份绑定；不可把 HTTP Bearer 映射成 local-owner |

M7 于本地文档提交 `b6de51de` 正式收束。原生程序/数据保持；真实IPC验证按后来获批的短暂停用、备份及恢复流程执行，不操作真实项目，不恢复已停止的桌面控制。

## 协议与依赖

身份接线新增直接依赖：daemon 使用已在 lockfile 的 `getrandom = =0.4.3`（MIT/Apache-2.0）取得32字节OS随机数，application复用已冻结sha2做凭据摘要。替代UUID拼接无法直接保证256位均匀随机；自行调用各OS随机API会重复维护平台实现。固定版本、熵源失败拒绝签发；不引入密码学框架或可恢复秘密存储。MCP实际连接复用alcomd-client，协议类型/帧预算直接复用alcomd-protocol，xtask精确依赖规则随实际引用同步。

遵循 `docs/baselines/mcp.md` 和 `docs/baselines/source-lock.toml` 中锁定的 `2026-07-28` Schema：逐请求版本/能力元数据、`server/discover`、无 initialize/session、Core Operation 与请求取消分离、不支持 MCP Tasks。精确上游 Schema 的 SHA-256 已核对为 `742750af0bb8c716e7030c4977c992b55d1adc4407e9e66997db5846baedc2cd`。

首批新增官方 Rust SDK `rmcp = =3.10.0`（Apache-2.0，MSRV 1.88，低于项目 1.97.1），关闭默认特性，生产仅开启`server`、`transport-io`。用途是复用JSON-RPC类型/响应编解码、请求调度及取消；生产不启用宏、客户端、Tasks或HTTP服务特性，后续真实SDK测试的dev特性见末节。已有serde/serde_json、Tokio和Core协议依赖复用。首批lockfile仅新增5个包，无既有包升级；新增包为rmcp、pastey、const-oid、schemars_derive 1.2.2、serde_derive_internals 0.30.0。

替代方案：自行编写 JSON-RPC/HTTP/SSE 栈会重复协议及安全边界；旧版 SDK 不满足冻结协议。维护风险：SDK 同时支持旧生命周期，必须用无握手服务入口、显式逐请求校验及定向测试阻止兼容行为；精确锁版本、审查 lockfile 增量，更新时重验冻结 Schema。HTTP 后续复用 SDK 的无状态传输，不自行实现 HTTP 解析。

首轮验证还确认 SDK 的通用 NDJSON reader 会静默丢弃损坏和超长输入。因此只补严格的输入分帧：上限复用 RPC 的 4 MiB，跨取消保留分片/丢弃状态，损坏 JSON 返回 ParseError，超限/不合法结构返回 InvalidRequest；响应仍由 SDK 写出。错误写入 future 跨 receive 取消保留，避免截断或重复输出；不记录原始输入。该差异不能以放宽测试掩盖。

上游核对：[冻结 Schema](https://github.com/modelcontextprotocol/modelcontextprotocol/blob/4df2d6b6e3588efb46e7542d98498e5c630a0a86/schema/2026-07-28/schema.ts)、[官方 Rust SDK](https://github.com/modelcontextprotocol/rust-sdk)、[当前传输规范](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports)。

当前受控STDIO已接通获批身份与六个包卸载工具；无凭据入口仍仅协议发现，不连接Core或提供业务。已接通tools及获批的toolsListChanged订阅；不广告resources，不接受旧initialize/ping或Tasks。HTTP回环预配Bearer已接通，取得本地定向/真实隔离及9fea7b6同SHA三平台完整CI与四项CodeQL技术通过（含5行测试同步）；历史失败/取消及后续只读审计见末节，不宣称管理扩展、完整OAuth或整个M8完成。

## 公共合同与接线状态

客户端身份最小合同和State34快照增量均已批准。application、Store、daemon dispatcher、typed client及受控STDIO已实际接通，真实Core闭环及最终capability协商候选复验通过，`mcp.clients.v1`已按真实实现协商。当前接线与后续边界（各候选证据分别见下文）：

- 获批回环预配Bearer及预登记客户端HTTPS/OAuth均已实现，含发现/resource绑定、到期、PKCE、审批、刷新轮换、撤销及逐请求Core授权；不取自clientInfo、不回退owner。OAuth普通浏览器/目标客户端人工接入待验证，CIMD/DCR/外部授权服务器尚不支持；本候选远端门禁见末节，不能据实现存在宣布整个M8完成。
- MCP管理扩展的客户端及授权管理、独立Host/WIT、真实Core/Host/Component闭环已实现，已有真实窗口查看与多轮反馈整改，尚无M8最终人工签收。已批准的Core IPC连接/在途Core RPC只读观测首片也已实现，不能等同于全部MCP请求、HTTP空闲连接或订阅。A/B本地候选已补传输观测及toolsListChanged订阅，当前新增代码远端待验证、正式默认分发仍未完成；沿用当前按能力授权与扩展所有权规则，不将未撤销记录当作在线。
- 本次六工具之外的业务映射及来源委托；正式Schema已覆盖获批卸载切片，旧Draft未提供已部署ABI。现有Operation仅get/list/cancel，不能伪造尚无公开合同的input/approve/reject/resume。

本批身份合同不再等待审批。其余真正新增的公共合同仍先提交具体差异，不暂停独立实现。

### 最小身份合同（2026-11-13 已批准）

批准前 `apps/alcomd/src/lib.rs::dispatch_payload` 把所有本机连接固定为 local-owner；`HelloParams` 没有认证字段。仅凭 adapter 内部记录 clientInfo/Bearer 不能改变 Core 权威身份。所有者已批准以下一批；不得在适配器中代持所有者权限：

1. `system.hello` 增加可选 `authentication: { kind: "mcp-client", credential: string }`，capability `mcp.clients.v1`。Core 验证凭据并绑定其签发的 `mcp:<ClientId>` Principal；无效凭据使用已有 authorization_required/permission_denied 并关闭连接，不回退为 owner。现有 GUI/CLI 的受保护 OS IPC 本机模式保持。MCP adapter 业务连接必须带凭据，clientInfo 仍只作展示。
2. 新增 `mcp.clients.list/create/setGrants/issueCredential/revoke`：create 输入名称（1–128字符）和显式 grants，默认零授权；setGrants 用 expectedRevision 完整替换本客户端 grants；revoke 使客户端及其凭据失效，保留审计和既有 Operation。list 用已有 cursor/limit 模式、绑定集合 revision，不返回凭据或散列。写入沿用幂等键，最多64个客户端、每客户端64种权限/4096条作用范围。
3. grant 明确为 `permission/resourceKind/resourceId`，复用已实现权限及适用范围，不创造通配的所有者身份；未知权限和未支持的组合拒绝。对所有者已登记资源的授权是委托访问，不改资源 owner；Operation/Plan/幂等记录归实际调用 Principal，跨客户端不可读或复用。Core 每次 RPC 重读客户端状态和当前 grants，撤销及范围变更对已连接调用同样生效；不得只在 hello 缓存授权。
4. 管理权限沿用已声明的 `mcp.clients.read/manage`；实际调用须 local-owner，或经正常扩展授权的管理调用。`issueCredential` 额外仅 local-owner：Core 生成独立256位随机凭据，只在显式签发响应中返回一次；轮换立即使旧凭据失效。普通列表、Portable UI、事件及日志不返回明文/散列。STDIO 从受控启动凭据读取，禁止命令行参数承载秘密或把启动秘密继承给自动启动的 daemon。
5. State32→33 仅新增 `mcp_clients`、`mcp_client_grants`、`mcp_client_credentials`（凭据只存不可逆摘要/签发和撤销信息），保留原表、数据与任务，不改 Config13。增加对应 RPC/Schema/快照和真实 Core/Store 隔离、撤销、幂等及迁移保全测试。

后续补充已批准的[State34](../../specs/storage/state-v34.md)：0033保持原样，新增0034单事务保全旧行并给授权增加不可变快照及当前指针，以在64KiB通用幂等记录内精确重放最大授权结果。鉴权只连接当前快照，旧快照不产生授权；凭据表及Config13不变，原运行数据仍State32。

本批不增加 HTTP OAuth 端点、扩展 Host/WIT 接口或业务工具 ABI。HTTP OAuth 与管理扩展所需的最小 Host 桥仍按冻结规范和实际接线另列具体差异；待批准前不开放无认证 HTTP，也不向管理扩展提供原生或数据库旁路。

## 验证与交付边界

日常仅检查受影响包及关联合同，使用独立的`target/m8-check`，避免覆盖原环境程序。协议、身份和工具证据分别关联输入；STDIO/身份/六工具的3ccc3f4远端基线与后续HTTP各轮实际结果见后文；历史f452ac5的CodeQL成功、CI有已确认Windows夹具失败，新增夹具修复12b4767已随7eac047获准推送，同SHA三平台完整CI和四项CodeQL均成功，dbe82de同步修正已随9fea7b6获准推送并取得同SHA完整CI/CodeQL成功，不复用旧M7结果替代。

稳定业务候选形成后再集中执行适用门禁，真实包/权限和管理界面另据实际实现验收。未实现、未执行与失败保持分列。

### 2026-11-17 现有候选的推送准备

先冻结现有代码/证据候选`edd2927213c7153dd7cbed4de45c81ff10131b4c`，本节仅补本地文档；下一批未批准合同另外提交，避免混入现有实现的推送范围。只读ls-remote确认实际origin/main仍为`717d3e085b6ce29649633e24c08e732c2e172855`，main至edd29272的19笔完整线性链和124文件范围已核对，包括原合同文档、各次实现和记录，没有遗漏文档提交或无关修改。Cargo.lock仅增加已批准真实HTTPS测试的既有alcomd-vpm开发依赖边，无版本升级、workflow改动、强推/发布/标签或远端写入。

`target/m8-prepush-contract-audit-20261117/prepush-inputs.json`保存完整SHA/标题链、逐文件SHA256及证据输入差异。当前828份已跟踪且排除docs/和Markdown的审计集合摘要为`78865250244ccfe44b3ecd715bcb79848368266222514749520e05f8ff486a1f`；此集合不同于以前937/938份测试输入集合，不直接比较聚合摘要、也不宣称828份全部测试已运行。逐文件核对后：

| 当前适用证据 | 精确适用范围与缺口 |
| --- | --- |
| 3d6c2a1b的真实HTTPS/Core、本地安装生命周期、State40–42及相关Store/协议/签名回归、受影响严格Clippy及匹配debug构建 | 与当前Core、MCP、application/client/protocol/store运行代码无差异；按当时单轮结果复用，不把原失败或ignored合并为通过 |
| fb72f0aa的GUI来源单轮8/0、前端check/build和GUI debug构建；953ce174的四模块36/0及另1/0 | 当前938份输入集合相比仅m7-extension-workspace测试及扩展README变化，GUI运行代码未变；更早30/1仍保留 |
| 7aed2b36的两个浏览器模块独立12/0及前端check | 当前该测试输入未变，原模块3/1保留；不等同完整Chromium通过 |
| 实际Codex HTTP Bearer的13项检查、独立OAuth的9项检查 | 与当前MCP/Core相同运行输入；STDIO不兼容、准备失败、清理补救、未调用取消/未验证刷新等仍按对应节保留 |
| Host旧管理Component具名1/0及历史诊断组合4/0 | 不证明历史2/2 resource_limit已修复；原因仍未知，后续若复现按有界诊断定位，不盲目重跑 |
| 本轮廉价门禁 | cargo fmt --all -- --check、cargo xtask check、locked/no-deps metadata、git diff --check通过。没有重复Rust全量、完整Chromium或release |
| 远端 | 717d3e0的CI44019052791仍为失败、CodeQL44019052564为成功；新增本地代码无远端结果，不能借旧结果替代 |

这是可提交远端技术验证的材料，不是宣称完整本地门禁、M8人工或整体已通过。仍有已披露未定位Host失败风险；扩展相关人工测试按所有者延期，不写豁免/通过。原环境、运行程序、Unity、系统信任和桌面控制均未操作。按当前明确授权仅推进本地准备；普通快进推送须另获所有者对具体候选的确认，获准后只收集自动push的CI/CodeQL，不dispatch/rerun。下一批草案只修改现有计划，公共接口/Schema/权限尚不改。

### 下一批最小合同：目录变化订阅与传输观测（2026-11-17，已批准实施中）

现有实现的拟推送前缀冻结为`e6d0b44b953bbd0d3e404555d3e6875e94e2e9a3`，相对origin/main为20笔提交；本节在bba8b905提交时是独立草案，不在此前缀内。其后所有者已分别明确批准推送这20笔、以及“批准A和B，按草案实施”；A/B不再等待审批。不重审已批准身份、六工具、Bearer/OAuth、扩展提供工具及State40–42。

**审计依据**：再次取得冻结commit `4df2d6b6e3588efb46e7542d98498e5c630a0a86`的[2026-07-28最终Schema](https://github.com/modelcontextprotocol/modelcontextprotocol/blob/4df2d6b6e3588efb46e7542d98498e5c630a0a86/schema/2026-07-28/schema.ts)，SHA256严格匹配`742750af0bb8c716e7030c4977c992b55d1adc4407e9e66997db5846baedc2cd`，只读副本在target/m8-prepush-contract-audit-20261117/frozen-mcp-schema.ts。已锁rmcp3.10.0有SubscriptionContext、过滤/ack/取消及无session HTTP路径，不需升级SDK。其SubscriptionContext::establish是SDK内部方法：STDIO原Service外壳须保留严格分帧/元数据/旧握手拒绝，只将已验证listen委托给SDK ServerHandler默认分发来建立上下文，不复制SDK实现或转回握手入口。当前lib.rs拒绝listen，http.rs只容许三个方法且GuardedBody有10秒总输出限制；Core mcp.tools.catalog已计算完整、带调用者授权和包证据的HMAC指纹但未返回字段。Core观察注册表只含真实IPC/RPC；适配器才知道TCP/TLS、协议请求和订阅，不能换名冒充同一数据。

#### A. 有真实通知源的最小订阅

| 差异 | 推荐合同 |
| --- | --- |
| Core目录结果 | 在既有mcp.tools.catalog结果增加catalogFingerprint：64位小写hex，取自同一次全量授权目录计算，覆盖当前身份/授权、OAuth上限、provider包/生命周期/授权及完整工具描述。所有页同一证据对应同一指纹，Core重启或证据变化可改变；它不是凭据、授权或可恢复session。协商capability mcp.tools.notifications.v1，仅在接线/真实测试后开放 |
| MCP方法及能力 | STDIO和HTTP增加冻结subscriptions/listen，仅接受notifications.toolsListChanged=true这一有效类型，其他类型不接受/不广告；接受集合必须是请求与已实现能力的交集。交集为空用既有-32602拒绝，ack前不发通知；先发notifications/subscriptions/acknowledged，再仅发notifications/tools/list_changed。每条均用原listen请求ID作为subscriptionId，优雅结束按冻结SubscriptionsListenResult，不携带请求进度/日志或业务结果 |
| 真实变化来源 | 每个listen使用独立的已认证Core连接，至多每秒查询既有目录、比较Core指纹，变化才通知；首次ack前取得基线，ack后立即再查以覆盖建立竞态。每次查询继续真实认证、OAuth到期/当前上限及授权复查，失败/撤销即结束流，不能将不可用当成空目录。指纹未变不发消息，不根据adapter自报变化或旧本地目录判定。首次实现复用原目录计算；若成本不合适，可内部用有界变更唤醒优化，不改变此合同或增加持久订阅表 |
| HTTP生命周期增量 | 仍是独立POST/逐请求Bearer、精确Host/Origin、无session/GET/DELETE/Last-Event-ID。只给listen增加长流分支：建立/首ack至多10秒，总寿命至多10分钟，15秒SSE注释心跳；单帧输出或通知队列阻塞最多10秒即关闭，正常等待通知不套旧10秒总输出或业务60秒限额。原32 TCP/16活跃请求或流保持；每适配器/认证上下文至多4个listen（不是跨全部进程的全局客户端配额），待发通知队列至多1项且只合并同类目录失效，慢客户端不积累历史 |
| 取消与身份 | HTTP断流、STDIO notifications/cancelled、EOF、过期/轮换/撤销和停机只结束该订阅/协议请求。先有的Core Operation仍归原Principal、照常执行和可恢复；无自动业务取消或写重试。客户端换token须新请求，旧流不切换身份。没有MCP Tasks、订阅持久恢复或服务端主动JSON-RPC Request |

本块只新增目录结果字段、协商capability及真实MCP订阅行为，不增加业务工具、State/Config、权限名称或依赖。HTTP原有普通请求的大小/认证/60秒预算及写响应未知合同不变；不能全局放宽超时来实现长流。Core接口/Schema及STDIO、HTTP、OAuth协议合同在获批后同步。

#### B. 传输观测的权威边界与最小接线

推荐与既有Core观测分开保留来源，扩展仍只走公开Core读取，不直连各adapter，不查询私有数据库。保留mcp.runtime.snapshot和旧WIT的IPC/RPC语义，新增以下窄接口；没有新监听、身份凭据表或通用遥测框架。

| 新增公共差异 | 具体边界 |
| --- | --- |
| 受限hello分支 | authentication={kind:"mcp-transport-observer",credential:""}，resource省略，仅经过原受保护本机IPC；Core绑定固定transport:mcp-transport-observer，绝不构造local-owner或MCP客户身份。只准mcp.transport.report，无业务、目录读取、状态查询、客户端/授权/凭据管理或任意RPC。显式kind非法则失败关闭，不回退owner。复用现有OAuth endpoint受限连接的实现模式；上报不是业务认证，不能由clientInfo或可执行文件名获得此身份 |
| mcp.transport.report | 该受限连接每秒最多一次完整快照：{sequence,transport,connections,requests,subscriptionStreams,coverageIncomplete}。sequence正安全整数且严格递增，旧/乱序用revision_conflict；整批验证失败不部分应用。transport固定stdio/http/https-oauth；输入只含本adapter内部UUID localRef及关联、固定方法枚举、阶段和已接受toolsListChanged过滤器，不含clientId、Principal、Core连接ID、外部JSON-RPC ID、URL/端口、头、参数、正文、工具结果或secret。对外行ID和接收/首次看到时间由Core生成；去掉的行及旧关联票据在同一应用中失效，不能用新快照更新旧请求的身份 |
| 观测关联票据与mcp.transport.bindRequest | report在第一次看到请求时可经该连接一次返回短时随机256位票据（10秒、仅内存、每行一次，不进入日志/事件/通用幂等/投影）。真实业务Core连接经既有MCP凭据/OAuth认证后，调用bindRequest({ticket})，Core用本次权威AccessContext关联实际MCP Principal；不得提交或信任clientId。仅MCP认证连接可绑定，票据须属于仍在上报的未绑定请求、未过期且未消费。绑定仅是观测关联，不能授予能力或证明Operation归属，不替代后续调用检查；单个票据不能换身份/请求复用。丢失report响应时不重新签票据/重试业务，允许该行身份未知；观察通道失败不使已接受业务Operation取消 |
| 来源可信度 | report只证明受保护本机上报通道及其所有权，不能证明上报内容来自获信任程序，不能据此认证网络客户端。所有传输行固定source=adapterReported；只有成功bind的请求才带Core确认的clientId，其余为未知。一个HTTP TCP连接可承载多个Principal，不能把第一次身份变成socket owner。clientId筛选只匹配当前已绑定请求/流及其关联连接，未归属的空闲socket不冒充该客户端活动 |
| mcp.transport.snapshot | 参数{clientId?,limit?}，capability mcp.transport.read.v1。返回Core instanceId/observedAt、每个observer的lastReportAt和available/stale/unavailable、coverageIncomplete，以及独立可读的connections/requests/subscriptionStreams分组。每组含items/totalTracked/truncated；limit默认25、1–100，响应总量64KiB。请求/连接各沿用mcp.requests.read、mcp.connections.read；订阅使用下述独立权限。未授权组省略，全部无权为permission_denied；外部MCP八项业务授权不能读管理投影。无reporter或其断线/超时应显示不可用/覆盖不足，不能宣称MCP零活动 |
| 只读Host/WIT | 新独立alcomd:mcp-transport/observations@1.0.0的snapshot与上项同字段/预算，复用原交互Client＋Extension双重权限、所有权、lease、生命周期和UI调用检查；旧接口不改名/换语义。扩展相关人工测试继续延期，本批不重做界面或新增导航；先完成Core/MCP及只读桥的定向证据 |

上报限额：每IPC observer至多128连接、128请求、32订阅行，输入64KiB；Core最多64个observer、全局三类行合计4096，容量不足明确coverageIncomplete。采样状态不是完整历史，已完成请求不留档；每个快照注明接收时间。报告延迟超过2秒即stale、不算新鲜活动；5秒无报告或通道断开移除其活动行，覆盖不足/不可用信息保留在当前响应，不保留持久历史。Core重启清空全部、变更instanceId、废弃关联票据；adapter可重新上报，但不得自动重放业务。报告通道使用without_daemon_start，随adapter停止关闭；不为观测启动/切换Core。它会在adapter运行时持有本机连接，按既有Core连接生命周期正常释放。

本块新增mcp.transport.report.v1 / mcp.transport.read.v1两项协商capability；report仅受限分支，bindRequest仅当前MCP身份且仍逐次重验；均不增加MCP业务授权。新增mcp.subscription-streams.read，仅扩展Global/*可选能力（所有扩展也可声明为必需，仍须原安装明确同意），不自动授权。拟新增State43：只扩展extension_grants现有CHECK允许此组合，保全旧列/记录/主外键/索引、事件高水位和其他表，不改旧迁移、不改Config、不加观测持久表。扩展可授权目录12→13，同步Manifest/RPC/WIT/Schema及威胁模型；不升级原运行数据。标准库/Tokio、已锁rmcp、getrandom和sha2足够，没有新增包或feature/版本计划。

#### 验证与后续业务范围

获批后先做定向验证：实际冻结SDK的STDIO/HTTP ack顺序和过滤、两token并发订阅隔离、实际授权/撤销/provider变化通知、无变化不通知、建立时变更不漏、慢读/队列/断流/时限、OAuth到期/轮换后结束旧流、关闭订阅后已接受Operation仍终态及写未知恢复保持；能用本机Codex现代HTTP订阅则补真实客户端证据，不用其不兼容STDIO冒充。观测覆盖零授权/未知身份、乱序/超限/过期/重复票据、伪造clientId拒绝、HTTP同socket多身份、旧role越权、断线/陈旧/Core重启、双重Host权限及大集合；State43做旧数据保全/失败回滚/重开。保护字段不得进入投影、日志、普通错误或幂等。形成稳定候选后集中门禁，不每次小改重跑全套或恢复桌面控制。

其余业务审计已确认：当前外部MCP Core allowlist只有原六用例及相关status/state/events/catalog；M-1的项目创建/登记/复制/备份恢复、模板、仓库/包搜索与安装重装、日志诊断及Operation输入/审批/恢复均不等于已开放。Core已有这些GUI用例也不能由adapter代持owner调用；activity.read/diagnostics.read等旧草案词不能直接恢复。下一批业务合同需对照当前RPC/能力授权另作紧凑工具映射，优先包搜索→安装/重装所需来源委托及原Plan/Apply，不为凑33用例新增无效工具；不夹在本节A/B审批中，不进入M9。

本节A/B已获集中批准，直接同步合同、实现与定向测试；不再追加纯文档审批。尚未完成的接线和真实验证不计为功能通过。扩展默认分发、正式签名材料和普通浏览器人工TLS不在本批，不创建第二套身份/任务系统。新增公共合同仍遵守AGENTS.md人工审批边界，现有推送前缀的验证及授权独立处理。

### 2026-11-13 协议基础实际结果

- 独立 STDIO 子进程：最终单轮 **12 通过 / 0 失败**，无重试。覆盖发现结果及不广告未实现能力、逐请求元数据、版本拒绝、旧生命周期/Tasks/未知方法拒绝、损坏 JSON/UTF-8、4 MiB 正好边界及超限恢复、分片/并发成功错误写入、EOF、未实现 HTTP 明确失败退出。无 Core 连接、数据库或真实项目操作。
- 首轮 **5 通过 / 3 失败** 保留：1 项新测试把 UnsupportedProtocolVersion 错写为 MissingRequiredClientCapability 的 `-32021`，按冻结 Schema 改为 `-32022`；2 项暴露 SDK 静默丢弃损坏/超长输入。增加严格分帧后8/0，再补边界/并发用例得到12/0；超长输入按 InvalidRequest `-32600` 验证，不假定其已经过 JSON 解析。后续12/0的一轮有新测试 unused_mut 警告，已移除并最终重新执行12/0、无警告；未拼接不同轮次结果。
- `cargo clippy -p alcomd-mcp --all-targets --locked -j 1 -- -D warnings`、包级 fmt、metadata 与 diff 检查通过。Clippy 首轮两个依赖 rustc 以 Windows 状态 `0xc0000043` 退出、无 Rust 源码诊断；核对无其他编译进程及工具链可启动后，单任务编译通过，未放宽门禁。
- 输入：父提交 `b6de51de` 加本轮 MCP 增量；MCP、协议依赖、Cargo manifests/lock、工具链与本地构建配置共20份文件，聚合 SHA-256 `19b5810c127f6052ea9cff506bd370eb9b66c349ca83fd3dc4d4b59cc93d03b9`。逐文件清单与原始分轮输出位于 `target/m8-entry-audit-20261113/`，最终测试为 `mcp-tests-verified.log`，严格 Clippy 为 `mcp-clippy-serial.log`。该结果只代表此协议基础，不是完整 M8 或同 SHA 远端门禁。
- 只读确认原 Core PID137312 / GUI PID142392，程序 SHA-256 分别仍为 `D06CC2959B181F22F364854188B86B15A15BCC902EA1316DEBBDC5B1353E9069`、`8B8BDBE1F53D8E5DCAD3978E74E038E70B67A3556897D6EF5CDC20672BCBE448`；没有替换程序、暂停原环境或恢复桌面控制。
- 本批仅本地提交，不推送。HTTP OAuth、正式工具 Schema、业务工具、管理扩展/Host 接线与真实界面验收均未执行，继续留在 M8。

### 已批准身份合同：Schema / 迁移首批

2026-11-13 所有者明确回复“批准这批最小合同”，无需再次审批本批。已新增 `specs/rpc/m8-mcp-clients-v1.{md,schema.json}`、管理DTO、State33合同及三表迁移，接入原初始化链。现有本机模式兼容增加，Config13不变；不会恢复旧MCP协议或共享万能身份。

实际结果：新增迁移5/0（包括全旧表行/结构保全、失败回滚、限额/外键、摘要长度与轮换事务、真实初始化后重开），DTO2/0（默认零授权、身份注入拒绝、Debug凭据脱敏），既有Store Schema42/0、StoreHandle12/0。初始迁移四例4/0保留，补初始化链后最终单轮5/0，不拼接。受影响protocol/store两个包的all-targets严格Clippy、包级fmt、metadata及diff通过；格式检查初次指出新DTO排版，修正后通过。没有完整Workspace、浏览器、release或远端工作流。

证据父提交`ec5df3ae`，9份本批代码/Schema/合同输入聚合SHA-256 `5990ed5f3ab17a0161bef7da19e9b6836ae1b3028df305d652d49cf574e51d57`；逐文件摘要`target/m8-entry-audit-20261113/state33-inputs.json`。原始结果为`state33-tests.log`（初始四例）、`state33-tests-final.log`、`mcp-identity-dto-tests.log`、`state33-compatibility.log`、`state33-clippy.log`。

这是当时的实现检查点，**当时身份合同尚未全部实现**：尚未接入hello认证或发布capability，当时HelloParams拒绝额外认证字段，避免忽略凭据后以owner执行。application/Store客户端命令、随机凭据签发/摘要认证、管理RPC、逐请求撤销与范围复查、客户端隔离和秘密不落日志的真实Core用例当时仍待实施；不以DTO/SQLite约束测试冒充这些验证。该首批没有启动新Core、升级原环境或恢复Computer Use；新增代码当时没有对应远端验证。当前闭环结果见后文，历史证据保留。

### 当前远端失败与身份接线历史

`ca196c47` 的 CI 43328525847 三平台均因 xtask 中 MCP 的旧精确本地依赖名单失败，CodeQL 43328524955 success。没有重跑或另触发 workflow。现 MCP 的受控身份连接实际使用 alcomd-client，协议 DTO/帧常量实际使用 alcomd-protocol；同步精确规则后 cargo xtask check 通过，不删除依赖检查或添加空引用。

当时继续编写 application/Store/daemon/typed client/STDIO 接线，真实端点闭环尚未执行，capability 未开放。原开发 Core/GUI 不替换；所有者先要求保持运行，随后明确“批准占用”，因此仅在测试准备就绪、原环境0活动且五组状态保存后，按既有流程短暂停用并恢复，不触碰 Unity 或恢复桌面控制。

当时发现的大授权集永久幂等缺口：通用幂等响应上限64KiB，4096条grant原响应可超过；需要非秘密元数据＋授权快照引用。自动审批拒绝原地修改已推送State33，该编辑未执行。随后单独提交最小State34审批，不将临时的revision冲突回放行为计为合同完成。

State34 增量迁移现已获所有者明确批准（“批准 State34 增量迁移”）。按新增0034迁移实施；0033原文件保持未变，不再等待此项审批。

### 身份接线实现范围

- hello严格解析凭据，显式null/错误/无效凭据关闭连接，不回退owner；每请求从Core当前记录重查客户端、凭据与授权，clientInfo只作展示。无凭据STDIO仅做协议发现、不连接Core。
- 五个管理RPC复用既有权限、帧预算、分页及幂等模式；默认零授权，签发额外仅local-owner，使用256位OS熵，明文只返回一次，重复签发请求返回明确冲突而不再次轮换。普通列表、事件、日志及幂等响应不含秘密。
- 当前支持的权限/范围组合明确列于[RPC合同](../../specs/rpc/m8-mcp-clients-v1.md)，未接通的组合及入口拒绝。资源读取显式连接当前授权，资源owner不变；任务、事件、Plan及幂等记录继续按实际Principal隔离。不因此宣称正式MCP工具已实现。
- MCP生产依赖实际复用alcomd-client和alcomd-protocol；真实Core/SQLite仅存在于集成测试dev依赖。受控启动凭据不进入CLI参数，也不继承给自动启动的daemon。
- 自动审批曾拒绝潜在owner替换方案及过早广告capability；对应命令未执行。改用显式授权SQL，先通过真实后端闭环再添加协商门禁，没有换入口绕过审批。新增getrandom只增加已冻结版本的依赖边，未增加或升级lockfile中的包版本。

### 2026-11-13 身份闭环实际验证

本批完成点为身份闭环，不是M8整体完成。本地代码候选`80dec2a5dd9f0507c05d2bd1dec575ee97cb2ca2`（父提交`ca196c4727cdab642b703a37a919a1adbb2b60d5`）；最终Windows工作区的462份Rust/manifest/lock/构建配置/合同输入聚合SHA-256为`b4706d1ea0046b8e4816de68a49157d69caf4e90e7d849320c729c0b463e19a4`，逐文件记录为`target/m8-entry-audit-20261113/identity-final-inputs.json`。提交后重新逐文件核对输入未变；随后只有状态文档整理，不重跑完整验证。下列是独立执行记录，不拼成Workspace全套通过：

| 验证 | 实际结果 | 本轮原始记录（均在上述target目录） |
| --- | --- | --- |
| application默认拒绝/精确范围/不支持授权/伪造owner | 4通过，模块后续未变 | `identity-application-tests.log` |
| protocol单元/Schema合同 | 分别20通过、49通过，后续DTO/Schema未变 | `identity-protocol-contract.log` |
| Store身份与State33/34迁移 | 最终同轮10通过，含>64KiB授权结果精确重放、旧快照无效、保全/回滚/限额/轮换 | `identity-final-store-tests.log` |
| Core认证拒绝/管理门禁 | 2通过，拒绝后确认无登记副作用 | `identity-final-daemon-tests.log` |
| hello当前Schema/capability协商 | 1通过 | `identity-final-hello-test.log` |
| typed client秘密Debug/daemon环境边界 | 1通过；后续仅增加capability请求项，受测边界未变 | `identity-client-secret-test.log` |
| 冻结STDIO协议真实子进程 | 最终同轮12通过，零重试 | `identity-final-stdio-tests.log` |
| Windows真实受保护IPC/Core/双身份/STDIO子进程 | 最终同轮1通过，8.35秒 | `native-preservation/final-20261113T0205490808071Z/identity-test.log` |
| 7个受影响生产包all-targets严格Clippy；MCP及受影响testing包all-targets严格Clippy | 分别通过；后者覆盖新增重启断言及既有HelloParams构造的编译 | `identity-final-clippy-3.log`、`identity-final-integration-clippy.log` |
| xtask精确依赖/metadata/fmt/diff | 通过 | `xtask-identity-candidate.log`、`identity-final-metadata.json`及执行记录 |

首轮后端闭环在未广告capability时1通过、6.77秒，输入摘要`6b9a63d411e41cd3a98b68c94feedb261807d845223cf8f16d66cd29a54f6078`；最终增加协商门禁及撤销后再次重启断言后，单独执行上述最终轮，不将两轮拼接。先前8项迁移、后来9项迁移以及独立最大授权1项的输出全部保留，最终Store单轮是10项。新增协商检查曾因遗漏常量导入、测试泛型推断失败，修正后严格Clippy通过；对应`identity-final-clippy.log`、`identity-final-clippy-2.log`保留。早期编译、格式与旧夹具接线失败同样保留，不改断言、不删用例、不放宽权限。

真实用例通过既有Core入口和真实Store/命名管道创建A/B两个身份：默认无权；显式项目授权后只读各自资源；同键任务分别持有、互不可读/取消，事件只见本身份；Plan保持实际Principal所有权且项目owner仍为local-owner。已接受任务在连接关闭后完成；连接存续期间项目范围和操作权限的修改立即生效；轮换使旧连接、旧凭据和实际STDIO子进程后续请求拒绝；重启保存新凭据/范围/幂等结果；撤销后再次重启仍拒绝该身份，B继续可用。Core数据、日志和通用幂等响应扫描无凭据明文。没有通过MCP业务工具写真实项目。

原环境已实际恢复：最终Core PID150048、GUI PID145568，GUI响应并已连接；State32/Config13、0活动任务、完整设置revision8和五组公开内容与暂停前即时快照一致（2项目/30仓库/5模板/1Unity/0扩展）。最初只读快照曾有4项目，第一次暂停前已为2；本次两轮各自前后均为2，不将历史4条回写或冒充当前计数。每轮正常关闭GUI、等待Core自行退出，离线备份并核对数据/profile；最终36份data、345份profile和两程序离线摘要未变，原12个Unity/Hub进程身份保持，没有强杀、替换原程序、迁移原数据或恢复桌面控制。最终`candidate.json`、`source-inputs.json`、`before.json`、`after.json`、`backup-manifest.json`、`reopened/open-result.json`及`result.json`在上述final证据目录。测试二进制SHA-256 `938E4920DB2BE403DB0555279968DB0CB899ED75F9208EEFD2B5C6EA927C7037`，STDIO程序`822B528631409EC00BFF02DB1E75823F7C2F54397A0096EBC88D41693896287F`。

本地收尾当时远端仍为：`ca196c47`的CI `43328525847` completed/failure（三平台精确依赖名单失败），CodeQL `43328524955` completed/success（四语言）；保存为`remote-ci-43328525847.json`和`remote-codeql-43328524955.json`。当时新增代码只取得本地证据、未推送，无新Hosted CI/CodeQL，未运行完整Workspace、浏览器全套或release构建，macOS/Linux本批真实身份运行也尚未验证。后续获准推送的实际结果见下一节。业务工具/正式Schema、HTTP授权及管理扩展Host/WIT桥继续留在M8后续具体合同，不计为完成，不启动M9。

### 阶段候选推送与远端结果（2026-11-13）

按所有者要求，本批保持“**本地验证通过、远端待验证**”；已启动轮已实际失败，修复候选未取得远端结果，不能把该标签理解为CI仍运行或已通过。已核对真实Windows用户、main干净工作树、最新origin/main及完整提交链，仅将`80dec2a`和`db37593`从`ca196c47`普通快进推送到`origin/main`。0033未改，无强推、标签、发布或手动workflow。

| `db37593d9ba0a41d9e95cb70a684dc840b93a6c5`检查 | 实际终态 |
| --- | --- |
| [CI 43359695787](https://github.com/ALCOMD/ALCOMD/actions/runs/43359695787) Windows / macOS / Ubuntu | 三平台全部failure；各自Store模块105通过/1失败，均为`state19_removal_preserves_all_business_rows_and_never_reuses_event_sequence` |
| [CodeQL 43359695853](https://github.com/ALCOMD/ALCOMD/actions/runs/43359695853) | success，actions/Rust/JS-TS/Python四语言全部完成 |

三平台日志均已通过精确依赖检查，严格Clippy及本批真实IPC双身份闭环用例也已通过；这只是对应步骤证据，不是整个Workspace通过。CI在上述测试处停止，后续repository-contract阶段、release/Tauri构建及Chromium均未执行。未手动重跑，macOS/Linux真实身份用例的新增证据绑定该SHA及各自Hosted环境，不冒充本机运行。完整JSON及三平台job日志保存于`target/m8-stage-push-20261113/`。

根因是旧State19保全用例仍期待旧表集合，而获批State33/34必然新增三张空MCP表，不是旧业务数据改变。本机先单例复现0通过/1失败（`state19-reproduction.log`），随后只新增`mcp_clients/mcp_client_grants/mcp_client_credentials`必须各为空的明确断言；原`after == before`、历史删除边界和Event高水位断言逐字保留。没有过滤忽略新增表或改生产迁移/Schema/运行时代码。修复提交`49d0687f8af00aee0672d43c6457fcdca22fe561`，修改仅测试9行；同模块4/0、Store all-targets严格Clippy、fmt/xtask/diff通过，原失败记录保留，不拼成105/1失败轮通过。该文件SHA-256 `8ef2b82748aae742b7b39c15767a06c8ca72fb24238d7a28acb39aa01b524a09`，结果`state19-targeted-fixed.log`和`state19-clippy.log`。所有者其后单独批准收齐结果后推送该修复、并批准以下六工具及Core窄差异；实际交付和验证见末节，不再标记为待审批。

### 首个真实业务工具合同：项目包卸载闭环（2026-11-13已批准，实施中）

所有者已明确批准本节六工具及所列最小Core差异，直接按先读后写实施，不重复审批身份合同。先完成“读取获准项目及已装包→生成卸载Plan→明确确认后Apply原Plan→查询/取消任务”。这是完整包管理写流程，不以发现或模拟响应代替；安装/搜索/重装的来源委托另列后续缺口，不把全部工具塞入首切片。正式Schema见`specs/mcp/package-removal-v1.schema.json`，实现验证前不宣称工具可用。

依赖复用：适配器直接使用已冻结workspace `serde`声明有界输入DTO，不新增包版本（MIT/Apache-2.0，与现有协议/client依赖相同）；替代方案手写任意JSON校验容易漏掉未知字段。测试仅给现有`rmcp=3.10.0`启用client/transport-child-process，不增加生产传输或HTTP依赖；特性增量按lock检查及真实Discover客户端测试验证。

依据为冻结[2026-07-28 Schema](https://raw.githubusercontent.com/modelcontextprotocol/modelcontextprotocol/4df2d6b6e3588efb46e7542d98498e5c630a0a86/schema/2026-07-28/schema.ts)、`mcp-tool-contract.md`中仍有效的命名方向，以及`80dec2a`实际RPC/application/Store。旧草案的Tasks、activity/diagnostics及Operation输入接口不继承。Core卸载分支明确使用空且完整的catalog，不扫描或下载仓库，适合作为首个有真实写结果的最小流程。

**一次审批的工具映射**（工具字段snake_case，通过typed client显式转换现有RPC的camelCase；无任意method/params透传）：

| 工具 | 输入 | `structuredContent.data`成功结果 | 对应RPC / 最小权限和范围 |
| --- | --- | --- | --- |
| `alcomd_list_projects` | `cursor? {registered_at_ms,project_id}`, `limit?` | `projects[{project_id,display_name?,revision,project_type,unity_version,directory_state}]`, `next_cursor?` | `projects.list` / `projects.read/project/<id>`；仅当前获准项目 |
| `alcomd_get_project` | `project_id` | 上述项目字段＋`vpm_manifest,upm_manifest,direct_dependencies,locked_dependencies,issues`；依赖项为`{package_id,value}`，问题仅code/component等有界字段 | `projects.get` / 目标项目的`projects.read`；这里是登记的包快照，不宣称文件系统实时扫描；Plan会重新读取当前项目并复核，不冒充全局包搜索 |
| `alcomd_plan_package_uninstall` | `project_id,expected_revision,package_id` | `plan_id,action,state,project_id,project_revision,change_set_fingerprint,change_set`，change_set保留当前RPC的全部变更、依赖、removal_conflicts、legacy_removals等字段和值；文本摘要明确版本移除及受影响依赖 | `packages.planRemove` / `projects.read/project/<id>`＋既有`repositories.read/global/*`、`packages.read/global/*`；不写项目，但创建真实持久Plan |
| `alcomd_apply_package_changes` | `plan_id,expected_revision,idempotency_key,confirm:true` | `operation_id,replayed`，仅表示接受，终态通过get_operation读取 | `packages.applyPlan` / 上述权限＋**新增支持的组合**`packages.manage/project/<id>`＋`operations.read/global/*`；只接受本Principal的remove Plan且项目一致 |
| `alcomd_get_operation` | `operation_id` | `operation_id,kind,state,revision,created_at_ms,updated_at_ms,started_at_ms?,completed_at_ms?,progress?,result?,error_code?,diagnostic_id?`；包操作result仅含`plan_id,change_set_fingerprint` | `operations.get` / `operations.read/global/*`＋实际Operation owner；首切片仅支持packages.apply，其他kind明确报invalid_request、不透出未知result |
| `alcomd_cancel_operation` | `operation_id,expected_revision,idempotency_key` | `operation`（同上）, `replayed`；cancelling不宣称cancelled | 先`operations.get`核对packages.apply，再`operations.cancel` / `operations.read/global/*`＋`operations.cancel/global/*`及实际owner；沿用Core合作式取消及不可取消错误 |

工具输入根对象均`additionalProperties:false`；UUID为规范小写36字符，revision为1..2^53−1安全整数；limit默认100、1..1000，cursor时间为非负安全整数；package_id沿用Core的1..128字符校验，幂等键沿用既有1..128字节有界合同。列表沿用Core的登记时间/ID稳定排序，**不新增跨页快照保证**；每页重新检查授权。Core若返回不可安全表达的整数、未知结果类型或超过预算的完整Plan，明确报错，不截断成可确认成功。响应沿用4MiB帧预算，工具data限定3MiB、文本摘要至多4KiB；Plan超过工具预算返回既有`plan_too_large`，项目文件未写入。

**发现、结果及确认**：仅带受控凭据并成功绑定Core的STDIO开放标准`tools`能力和这六个静态定义；零授权可见定义不代表有权调用，Core仍逐请求拒绝。无凭据仅协议发现、不连接Core；撤销后tools/list也不得返回缓存成功。tools/list为`resultType:complete,ttlMs:0,cacheScope:private`，不广告listChanged/订阅或Tasks；逐请求版本/能力保持冻结规范。每个工具同时提供完整inputSchema/outputSchema和短文本：成功`{data:...}`，业务失败`{error:{code,message,retryable,details?,operation_id?,diagnostic_id?,outcome?}}`并`isError:true`；结果都是complete，不伪装MRTR。读工具readOnlyHint=true；Plan会持久化所以readOnlyHint=false、destructiveHint=false；Apply/Cancel为写操作，Apply destructiveHint=true。Plan不承诺幂等；Apply/Cancel同Principal同参数同幂等键幂等，工具hint不代替校验。

Apply调用方必须先展示完整Plan及风险并取得操作者确认，再提交`confirm:true`；它只表达调用方明确确认意图，不是假造的人工签名或权限凭据。false/缺失在适配器校验阶段拒绝。Core只接收原`plan_id/expected_revision/idempotency_key`，不可变Plan ID已经绑定原计划；不新增MCP预览缓存、确认token或Core RPC字段，不接受工具端重写change_set、allow_conflicts或隐式重新计划。依赖移除风险沿用Core现有formatVersion 1..4及确认后的行为，不能照抄旧草案伪造尚不存在的waiting/input/approve流程。来源、revision、文件身份及运行中项目等检查继续由Core承担；失败需要用户重新计划时明确返回原错误，不替用户换Plan。

**错误与取消**：输入/工具Schema错误`-32602`，未知工具或未开放tools能力`-32601`；已进入业务用例的`permission_denied/plan_not_found/plan_stale/revision_conflict/idempotency_conflict/package_not_found/package_not_installed/operation_not_found/operation_not_cancellable/store_unavailable`等保留当前Core稳定code、脱敏message和允许的details，internal_error必须有diagnostic_id。RPC没有retryable字段，适配器显式映射：读取的store_unavailable及下述core_unavailable可以显式重试，其余默认false，不解析message猜原因。新增仅MCP层的`core_unavailable`（未提交或读连接不可用）、`core_write_outcome_unknown`（写入发出后响应丢失，outcome=unknown）；不把后者报成失败已回滚或自动换键重试，提示以原键/原参数显式查回同一结果。读取failed任务时工具读取成功，但data.state/error_code必须保留失败事实；Apply接受也不是卸载成功。`notifications/cancelled`/EOF只结束协议请求，不能默认取消已接受Operation；中断在途RPC的连接按typed client既有usable规则丢弃，不复用残留响应或回退owner。业务取消只能显式调用cancel_operation，可能因阶段不能取消或最终成功，均如实返回。

**需要一并批准的Core差异，限这条流程**：

1. 不新增权限名、身份RPC、凭据模型或State/Config版本；只给既有grant元组开放`packages.manage/project/<id>`组合，且必须同时具有同项目read和上述既有读权限。现有global读授权不被扩大为项目写入。
2. `m8_mcp_rpc`只新增`packages.applyPlan`入口。`M4Application::apply_plan`当前硬拒绝所有非local-owner；仅对已认证MCP、本Principal remove Plan、当前精确项目写范围开放，并在接受事务内再次查当前授权/客户端状态。不允许安装/升级/reinstall/bulk Plan借此入口执行；原生owner流程保持。
3. 当前accept/run/complete/recovery按“项目owner=Plan调用者”读取或更新，会让委托写失败。改为Core内部根据已接受Operation→同owner的不可变Plan→project_id/revision/路径身份解析目标，保留所有路径、锁、journal、回滚和资格检查。项目owner保持原值；Plan、Operation、幂等记录仍归调用MCP Principal，不拼造owner AccessContext。项目变化通知按真实资源可见性失效，操作结果事件按调用者隔离，原GUI也应刷新。
4. 撤销/范围减少必须拒绝后续Apply（包括旧Plan的重放入口，不能借幂等绕过当前授权）；已经接受的任务继续由Core按冻结目标执行和恢复，不因STDIO断开或后来撤销自动取消。内部执行权来自已接受任务绑定，不来自重新冒充owner或任意传入ProjectId。显式业务取消仍走原接口。无须增加持久授权任务表或运行时框架。

**后续缺口不在本次切片偷偷开放**：`packages.queryProjectCandidates`当前还需要settings.read且按caller owner读取项目/来源；安装Plan的resolver_catalog、归档pin及来源读取也按caller owner隔离。安装/搜索/重装须另行明确来源委托及安全输出，不能把空catalog当可用、改资源owner或代持local-owner。HTTP授权、管理扩展Host/WIT及M9–M12仍不实施。

批准后按先读后写实施。同一组用冻结`rmcp=3.10.0`真实client的`ClientLifecycleMode::Discover`驱动STDIO子进程（禁用Initialize/Auto回退；仅测试启用client/child-process特性，不新加生产SDK），经过受控启动凭据和实际Core完成隔离项目卸载。只补工具引入的关联验证：零授权/错误范围/撤销、风险字段和失败准确性、显式确认前零项目写入、原Plan无重规划、旧revision拒绝、同键/跨client隔离、任务终态与合作取消、请求取消不取消已接受任务、owner/项目文件实际结果及重启恢复。复用已通过身份测试，不为这条切片先建管理GUI或重跑完整全套；原运行Core/GUI及真实项目保持。

### 获批修复推送、六工具实现及真实闭环（2026-11-13）

所有者集中批准后，核对origin/main、main提交链和工作树，只将`49d0687f8af00aee0672d43c6457fcdca22fe561`普通快进推送至origin/main；当时本地`4fb1d94c`待审工具记录未夹带推送。无强推、标签、发布或手动workflow。该轮已全部终结：

| 49d0687检查 | 实际终态 |
| --- | --- |
| [CI 43365902105](https://github.com/ALCOMD/ALCOMD/actions/runs/43365902105) | 三平台failure；旧State19保全用例已通过，另一旧`empty_unity_override`模块各2通过/1失败，失败为`state16_upgrade_preserves_every_existing_row_and_empty_override_survives_reopen` |
| [CodeQL 43365902090](https://github.com/ALCOMD/ALCOMD/actions/runs/43365902090) | success，四语言完成 |

新失败仍是旧保全夹具未显式列入三张获批新增的空MCP表。先本机复现0/1，再只补三表为空的断言，旧完整相等、Event序列、外键及reopen检查保留；修复后同模块3/0，本地独立提交`d98038f`。没有改生产迁移、0033或放宽检查。远端停在cargo test，后续repository-contract/release/Tauri/Chromium未执行，不拼成完整通过。完整run JSON和三平台日志在`target/m8-business-20261113/ci49-final.json`、`codeql49-final.json`、`ci49-job-*.log`；旧db37593、ca196c47失败和各自CodeQL结果仍保留。

**实际完成**：本地代码候选`88cb0adaa8ce9a8492815f948f343fad5ab83c1e`接通六工具，复用typed alcomd-client和现有RPC；不在适配器访问数据库/项目。Core仅开放已认证MCP的原remove Plan和精确项目写元组，接受事务重新校验当前授权，即使幂等重放也不能绕过撤销；内部执行/完成依据已接受Operation→原Plan→不可变journal及项目路径身份。项目owner保持，Plan/任务/幂等属于实际调用者，项目变更事件送资源owner。安装/重装等非remove Plan明确拒绝。正式Schema是获批工具增量；旧Draft未形成已部署ABI，无新增Core RPC形状、权限名称、State/Config或持久查询机制。

工具返回真实Plan全部变更/风险和独立Operation，接受不报成卸载成功；未知写结果不报回滚，也不自动重试。收尾特别修正Plan创建本身会持久化：其响应丢失同样返回`core_write_outcome_unknown`；Plan无幂等键，须重新查看项目后显式创建另一Plan，Apply/Cancel才以原参数/原键查回。已断开的typed client不能复用，不回退owner。协议取消与业务取消仍分开。

**实际验证及输入**（各轮独立，不能相加成完整Workspace）：

| 验证 | 实际结果 / 证据 |
| --- | --- |
| 真实冻结SDK→STDIO子进程→隔离Core→实际项目包事务 | 最终单例1/0，4.07秒；`rmcp 3.10.0` Discover，明确只接受2026-07-28，不走Initialize/Auto；实际包目录、VPM direct/locked项移除，UPM字节保持 |
| 六工具正式JSON Schema | JSON Schema2020-12校验通过；最终实际39次请求/结果全部匹配、含成功和业务拒绝；tools/list六项、complete/ttl0/private，无结果截断或未知RPC透传 |
| Store组件集中回归 | 单轮210/0（含105项库测试及105项集成测试），`store-component-final.log`；包含两项旧迁移夹具和新委托写回归，不改写旧失败轮 |
| 应用授权映射 | m8模块5/0，`scoped-write-unit-tests.log`，含读不隐含写、精确项目写、全局包写拒绝 |
| MCP工具与协议 | 最终工具库2/0，`mcp-unit-final.log`；未变STDIO回归单轮12/0，`mcp-protocol-final.log`，其中先前库2/0不与最终轮拼成4项 |
| 严格检查 | application/Store/Core/MCP all-targets Clippy通过（`business-clippy-3.log`）；末次仅MCP错误映射变化后MCP all-targets Clippy通过（`mcp-clippy-final.log`）；fmt/xtask/metadata/diff通过 |

最终输入：候选`88cb0ad`，465份文件，聚合SHA-256 `44a6925883047993d63a78b1ca3a880b959b78659d7bf9cd17a8b52adae93c7b`；逐文件在`target/m8-business-20261113/source-inputs-final.json`。真实测试程序SHA-256 `503467E35319222450D1250899567403BD0D25A8342D7EF834756BDC77DECCB1`，STDIO程序`13B15A5E4238895DDF712BF7B4F41982E91B82C9CEA119205FF83309B921E102`。早轮真实1/0、37次Schema校验和聚合`f919021912a71d88ebcbc34e5656321bfd97ea896926c6687a81190213b89a85`保留；末次仅`tools.rs`的未知Plan响应分类/文案及关联断言变化，Store/application/Core输入未变，复用其精确模块证据。没有重跑完整Rust/浏览器/release/三平台CI。

最终业务证据目录`target/m8-business-tests/d3d7307b-b908-417c-a8bf-4f73c0fc23a8/`保存`tools-list.json`、`tool-trace.json`和`coverage.json`；`actual-contract-validation-final.log`记录六Schema及39次实测校验。两个客户端均由owner明确创建/签发，默认零授权被拒；授权后只读获准项目，范围外拒绝，读权限不能Apply；缺确认在协议输入层拒绝、项目不写。真实不存在包失败准确返回，依赖风险完整展示，外部改动后原Plan过期拒绝、不重规划；另一客户端不能使用Plan或读取任务。同键重放同Operation，撤掉写权限后同键也拒绝；实际取消终态为cancelled；SDK/STDIO关闭不自动取消已接受的另一任务，重连查询成功。Core重启后凭据、作用范围和原任务/幂等结果持久；撤销后已连接请求及tools/list拒绝，另一客户端仍可用。原资源owner不变。Store真实事务测试还证明撤销前已接受任务可依原绑定完成、不可借撤销重放，journal不可修改且变更项目身份会拒绝内部目标解析。

实现期间的SDK类型/调用签名编译诊断及Clippy分轮输出保留在本轮target证据目录。新增Store用例首次0/1是测试试图修改append-only journal，Core拒绝符合原保护；改为明确断言此修改失败，再以项目身份变化验证目标解析拒绝，定向1/0（`delegated-store-test-2.log`），最终另有上述组件单轮210/0。未删除journal保护、放宽断言或把失败轮次拼成通过。

**原环境已恢复并实际核对**：两轮均先确认0活动任务、原程序路径/数据与五组公开快照，正常关闭GUI并等待Core自行结束，离线备份36份data、345份profile和原程序。最终恢复Core PID163652 / GUI PID155308，GUI响应且连接，State32/Config13、0活动、完整设置revision8；前后均2项目/30仓库/5模板/1Unity/0扩展，内容一致。两程序、离线data/profile摘要未变，本轮暂停前存在的9个Unity/Hub进程PID/启动时间保持，未强杀、替换程序、升级原数据、操作真实项目或恢复桌面控制。历史身份轮12个进程不是本轮计数，未强行恢复已由用户结束的进程。备份、源输入、before/after、重开及`result.json`在`target/m8-business-20261113/native-preservation/final-20261113T0522383781023Z/`。

测试依赖增量仅现有rmcp客户端/子进程特性引入`process-wrap 10.0.1`（MIT OR Apache-2.0）及`tokio-stream 0.1.19`（MIT），未升级其他包，不增加生产HTTP或运行时框架。Schema独立校验器`jsonschema 4.25.1`及其依赖仅安装于本轮target目录，不写全局Python或生产依赖；无新通用测试框架。

**当时未覆盖/后续归属**：上述STDIO关闭证据不能代替在途`notifications/cancelled`/真实IPC半写故障；这两项已按所有者后续要求在下一节补齐，不宣称覆盖所有竞态。Core重启验证的是已完成任务及授权持久，不宣称做了所有文件系统阶段崩溃恢复；既有恢复测试复用，新增委托绑定有Store撤销回归。尚无本批macOS/Linux真实SDK工具闭环、完整Workspace、release或新Hosted CI/CodeQL。HTTP认证、正式管理扩展/Host桥以及安装/搜索/重装的来源委托均未实施，继续属于M8具体后续合同，不能默认获批；不扩大到M9–M12。

本批按“本地业务闭环验证通过、远端待验证”交付。`d98038f`、`88cb0ad`及收尾文档仅本地提交；后续推送尚无授权，未触发新workflow，M8继续进行中。

### 推送前两项传输风险与完整提交链核对（2026-11-13）

按所有者要求只补两个明确缺口，不重审六工具或身份合同，不进入HTTP/管理扩展。代码提交`99d07c4fdec100218cf1ad7f12eac5b04f21a804`，新增一个真实SDK定向用例`frozen_sdk_inflight_cancel_and_lost_write_response`同时覆盖两种故障；原卸载闭环未重跑。两个占用同一真实端点的业务用例在测试二进制内串行，避免CI并发撞管道。Core故障门控、计数和控制方法全部受既有`test-kill-gates`编译条件约束；无生产参数、环境开关、公共RPC、认证或数据库旁路。普通Core构建检查通过。

| 风险 | 实际故障与结果 |
| --- | --- |
| 在途MCP请求取消 | Core已通过真实权限/Plan校验并提交接受事务，测试暂停真实IPC响应；冻结SDK发出该Apply请求的`notifications/cancelled`，SDK响应为Cancelled。取消之后、Core响应仍未释放前，后续工具可及时返回`core_unavailable`，证明handler已释放且中断IPC不可复用。显式重连、原Plan/revision/key恢复相同Operation、replayed=true；最终succeeded，Core权威`cancel_requested=0`，不是业务取消 |
| 真实写响应丢失 | 另一真实Apply提交接受事务后，发送实际长度前缀及一半实际响应，再关闭IPC；不伪造工具或Core结果。MCP返回`core_write_outcome_unknown`、outcome=unknown、retryable=false；旧连接后续明确不可用。Core入站Apply计数证明无自动重试；显式用原身份/原Plan/revision/key恢复同一个Operation，replayed=true，最终succeeded |

两次初始Apply＋两次显式恢复共**4次**Core请求、**2个**Operation；每个项目revision仅增加1，包目录确实移除，每个Operation的accepted/completed journal恰为1，最终均succeeded且无业务取消标志。12份真实工具结果全部通过既有正式Schema校验（`risk-schema-validation.log`），未把结果未知报为业务回滚或成功。

有效修复前轮**0/1**（7.12秒）暴露SDK只发出取消token、适配器未监听，handler继续持有连接锁；日志在`target/m8-risk-20261113/native-preservation/before-fix-20261113T0603550380067Z/risk-test.log`。仅让工具handler监听既有context取消信号，中止协议future及等待连接锁；typed client既有规则保留不可复用标记，不调用operations.cancel、不自动重连或重试。修复后相同用例**1/0**（4.38秒，原卸载用例1项过滤），两个风险分支均执行。没有放宽超时或断言。最初误用尚未编译完的旧测试程序得到**0项执行**，明确不作为通过证据；该轮原环境也已恢复，记录保留于`reproduction-20261113T0602101376607Z`。启动脚本已增加指定用例存在、执行前再次核对二进制摘要和恰好执行1项的检查。

最终风险证据：`target/m8-business-tests/8f7c81b4-f2ee-48b8-97c0-1f1f2a923014/risk-tool-trace.json`及`risk-coverage.json`；原环境保全及最终日志在`target/m8-risk-20261113/native-preservation/final-20261113T0607306176597Z/`。465份实际输入聚合SHA-256 `a7d196d77c9ae345ea71ec807f723706a9931bacdb1515c900fd77130eb6492a`（`source-inputs-final.json`），提交前逐文件再次核对，源码与`99d07c4`一致。测试二进制SHA-256 `577D8F085A403F73F79E8629A0CEEA4AD4F6EE55BD264DDF2B86AE1917BA32AD`，实际STDIO程序`97EA77CC413195A4455A2C0E5500829470FDDF867EBA3AFC7D47DB9F4B8A7EAC`。新增测试夹具的首次编译类型错误日志亦保留，不冒充执行结果。

相关检查：Core/MCP all-targets严格Clippy通过；本轮MCP库2/0、STDIO12/0；普通Core无test feature的`cargo check -p alcomd --locked -j1`通过；fmt、xtask、metadata、diff通过。先前Store210/0等证据仍对应未变输入，不重跑完整Rust、Chromium、release构建或CI。

`d98038f`精确核对：只新增10行，三张表必须存在且各为空，缺失或非空都失败；移除这10行后，**整个文件**与其父提交逐字一致（只统一Git输出换行用于比较），证明原State16完整行快照相等、Event sqlite_sequence、外键、空override重开、失败迁移保留State16/原DDL/无临时表等断言均未削弱。`state16-diff-audit.txt`保留核对结果。生产迁移和Config相对远端49d0687均无变更，0033原迁移不改。

原环境每轮均按已批准流程正常关闭/自行退出、离线备份/核对并恢复。最终Core PID151604 / GUI PID164732，GUI响应且连接；State32/Config13、0活动、完整设置revision8、五组内容一致（2项目/30仓库/5模板/1Unity/0扩展）。原程序、离线data/profile摘要不变；本轮暂停前7个Unity/Hub进程身份保持，未触碰Unity、真实项目或桌面控制。

推送基线已通过`git ls-remote origin refs/heads/main`核实仍为`49d0687f8af00aee0672d43c6457fcdca22fe561`，该提交是本地HEAD祖先，可普通快进。此前CI43365902105三平台失败和CodeQL43365902090四语言成功继续保留，均不代表本地新业务候选通过远端。提交链按祖先到后继完整包括：

1. `4fb1d94c820c3d09ce0b6e2ea881f154a7164faa`：身份候选远端结果及当时待审六工具合同文档；保留这笔提交，后续记录明确已获批。
2. `d98038fdccc1fd7aab6f1c71659466c5fb39aeea`：旧State16夹具三张空表断言。
3. `88cb0adaa8ce9a8492815f948f343fad5ab83c1e`：获批六工具、Core精确委托写、正式Schema及真实闭环测试。
4. `a8390d6f01d1454e36a93ecf481e0098f816262b`：实际远端失败、本地闭环与原环境恢复记录。
5. `99d07c4fdec100218cf1ad7f12eac5b04f21a804`：本节取消窄修、测试专用响应门控及两项真实故障回归。
6. 推送前风险验证及提交链记录提交：仅本节及status文档，最终提交SHA见交付清单。

整条候选链仅上述M8业务、已批准合同、旧夹具修复和对应记录；无GUI/真实程序更新、HTTP/管理扩展实现、额外依赖升级或无关迁移。以真实Windows用户在main提交，不改写历史、不创建标签/发布。收尾当时不推送，交付完整候选SHA、清单及干净工作树后等待确认；随后所有者批准的实际推送见下一节。

### fa1e7fc获准推送及本轮远端结果（2026-11-13）

所有者确认后，再次核对真实Windows用户`cqmhv-dt\m2922`、标准目录/main、干净工作树、origin地址、完整六笔提交及远端49d0687。已普通快进推送至`fa1e7fc3cd9ad01a58a36d9d195bee3ba50a0918`，包含此前待审合同记录`4fb1d94c`，远端SHA实读一致。无强推、标签、发布或手动workflow；同SHA自动运行结果为：

| 检查 | 实际状态 |
| --- | --- |
| [CI 43377645951](https://github.com/ALCOMD/ALCOMD/actions/runs/43377645951) | completed/failure，Windows、macOS、Ubuntu均在`m7_project_actions_contract`模块6通过/3失败 |
| [CodeQL 43377644992](https://github.com/ALCOMD/ALCOMD/actions/runs/43377644992) | completed/success，actions/Rust/JS-TS/Python四语言完成 |

三平台失败均为三项旧静态合同仍写`assert_eq!(CURRENT_DATA_SCHEMA,32)`，实际已是获批State34：项目复制、目录删除和历史编辑器偏好合同分别在`m7_project_actions_contract.rs:102/465/626`失败。并非本轮业务用例失败，亦不重开M7。三平台日志均明确通过原State19、State16保全用例，真实身份闭环及两个SDK业务/传输风险用例也均通过；这些只计为该SHA各自步骤证据，不拼成完整Workspace通过。后续repository-contract、release/Tauri、Chromium尚未执行。此前49d0687/db37593/ca196c47失败原样保留。

本机先原样复现同模块**6/3**（`current-schema-reproduction.log`），然后仅将三处严格相等预期从32同步到34，其余文件内容逐字不变，未改成范围断言或删除校验。修复后同模块单轮**9/0**（`current-schema-fixed.log`），定向严格Clippy、fmt/xtask/diff通过；独立本地提交`5bfbc90c425e250882cb6492614ec414b765518c`仅3行替换，文件SHA-256 `DE3C750D877291DC50BEE524DD3BC5A2A84D63332AD461AC7C5593F457847EAB`。同类当前Schema常量审计未见其他旧32相等断言；历史迁移版本保留。生产代码/Schema/权限/0033与0034迁移、Config和已有业务证据输入未变，不重复完整Rust/浏览器/release验证，也不盲目重跑workflow。

运行JSON、job日志、本机复现与修复、严格检查及逐字差异审计位于`target/m8-hosted-fa1e7fc-20261113/`。本次只执行推送和本地静态合同修复，未暂停、替换原Core/GUI、迁移原数据或操作真实项目。收尾当时额外修复及本节记录仅本地提交，没有修复后远端结果，不将fa1e7fc的CodeQL通过冒充修复后通过；其后所有者再次确认的实际交付见下一节。M8继续进行中。

### 3ccc3f4获准推送与同SHA门禁（2026-11-13）

所有者再次确认后，核对main干净工作树、远端fa1e7fc及完整两笔提交，以真实Windows用户普通快进推送`5bfbc90c425e250882cb6492614ec414b765518c`（三处严格当前State34预期）和`3ccc3f428ddadd6b113777b77cce7fe6fa78506d`（结果记录）。远端main实读为3ccc3f4，无夹带、强推、标签或发布，只收集push自动启动的同SHA运行，不手动dispatch/rerun。

| 同SHA检查 | 实际结果 |
| --- | --- |
| [CI 43383251231](https://github.com/ALCOMD/ALCOMD/actions/runs/43383251231)，Ubuntu 22.04 | success；Chromium单轮703/0，17.1分钟 |
| 同一CI，macOS 15 arm64 | success；Chromium单轮703/0，26.0分钟 |
| 同一CI，Windows Server 2025 | success；Chromium单轮703/0，18.6分钟 |
| [CodeQL 43383250817](https://github.com/ALCOMD/ALCOMD/actions/runs/43383250817) | completed/success，actions/Rust/JS-TS/Python四语言 |

整轮CI及三平台job均completed/success，CodeQL四语言completed/success，全部绑定同一SHA `3ccc3f428ddadd6b113777b77cce7fe6fa78506d`。三平台日志逐项确认：严格workspace Clippy、完整Rust测试、原State16/19保全及三处当前State34合同通过；真实Core独立身份、实际SDK卸载以及取消/截断写响应风险用例均通过。两个脚本各自执行的Rust轮次分别保留，不累加成双倍测试数。生产release bins、Tauri no-bundle、平台约束、锁文件/diff及完整Chromium门禁据各平台真实步骤记录；浏览器沿用`retries: 0`，各平台分别单轮703/0，不借用旧M7的700项或拼接历史失败轮。

最终run JSON（`ci-final.json`、`codeql-final.json`）及三平台完整job日志保存于`target/m8-hosted-3ccc3f4-20261113/`。此前fa1e7fc、49d0687、db37593、ca196c47失败及各自CodeQL结果保持历史，不改写为通过。GitHub查询期间一次HTTP504仅重查原运行，没有rerun；日志按平台保存而非拼接轮次。本机不为这次推送重复Rust/浏览器全套、release或真实端点验证；未暂停或替换原Core/GUI、迁移原数据、操作真实项目或恢复桌面控制。状态收尾只执行适用的metadata/xtask/diff检查并本地提交，不再次推送纯记录触发一轮CI。

本轮仍只覆盖冻结STDIO协议、客户端身份和六工具卸载切片。安装/搜索/重装的来源委托、HTTP授权、正式管理扩展/Host桥未实施，未获本批实现授权，不因门禁进展自动开放；全部文件系统崩溃阶段和取消/丢响应竞态也未宣称穷尽。M8继续IN_PROGRESS，不重开M7或宣布整个M8完成。

### HTTP现状审计与最小合同提案（2026-11-13，已批准，实施中）

所有者已明确批准推荐的回环预配Bearer合同及必要依赖，直接实施本节范围，不重复审批身份或六工具。正式承载见`specs/mcp/http-loopback-v1.md`；本节保留批准前审计事实，实际实现/定向证据另记。无新增Core RPC、State、Config或权限。

实际锁解析补充：此前静态审计漏掉SDK `server-side-http`同时启用`base64`且要求`^0.23`，现有0.21.7/0.22.1不能满足。需要额外的SDK传递依赖`base64 0.23.1`（MIT OR Apache-2.0，官方未yank，2026-08-04发布，checksum `ac07cdecf99051d9a5238b80f35af32cdeba5b336e55d957b318b50137e18da5`），用于冻结HTTP头的Base64编码/核对，不是新认证机制；保留原两个版本及其消费者，不改SDK源码或以不兼容旧版本强凑。闭包首次解析选中sse-stream0.2.6，现已恢复获批精确0.2.4。草稿锁文件只新增base64/httpdate/sse-stream三包，无既有包升级；base64增量已按所有者本轮明确授权批准，恢复HTTP构建，未把原两包审批扩成任意新包。首次cargo check中断记录在`target/m8-http-audit-20261113/implementation-check-1.log`，不能计为通过；源码及测试准备继续，现有生产程序/数据不变。

下列审计和提案保留批准前事实。当时只审计HTTP，正式HTTP合同、Cargo和锁文件尚未修改，远端基线及本地记录f5042f3保持；批准后的接线与验证状态见本节首尾，不把旧STDIO/身份远端结果计为HTTP通过。

**冻结依据**：继续使用规范commit `4df2d6b6e3588efb46e7542d98498e5c630a0a86`，通过官方GitHub blob API读取并核对Git blob及SHA-256，非浮动main：

- [Streamable HTTP](https://github.com/modelcontextprotocol/modelcontextprotocol/blob/4df2d6b6e3588efb46e7542d98498e5c630a0a86/docs/specification/2026-07-28/basic/transports/streamable-http.mdx)：POST、请求头镜像、Origin及请求级SSE；本版无HTTP取消通知、GET流、协议session或Last-Event-ID恢复。
- [Authorization](https://github.com/modelcontextprotocol/modelcontextprotocol/blob/4df2d6b6e3588efb46e7542d98498e5c630a0a86/docs/specification/2026-07-28/basic/authorization/index.mdx)：HTTP认证SHOULD遵循OAuth profile；进入该profile后，PRM、授权服务器发现、token audience等MUST不能省略或伪造。
- 本次共核对transport overview/streamable-http及authorization overview/discovery/security五份正文，摘要清单在`target/m8-http-audit-20261113/frozen-doc-inputs.json`。M-1基线中的scaffold、旧工具草案及历史冲突不作为当前已实现能力清单。

| 现有入口/权威 | 审计结论及真实缺口 |
| --- | --- |
| `apps/alcomd-mcp/src/main.rs` | STDIO可读启动凭据；`serve --port`默认0但立即报未实现，没有listener、HTTP认证或安全中间层。不能把CLI参数存在计为HTTP完成 |
| `src/lib.rs`、`src/tools.rs` | 冻结逐请求metadata、三个已实现方法（discover/list/call）、六工具、Core错误及取消处理可复用；现为直接`Service<RoleServer>`，SDK HTTP要求`ServerHandler`，须薄适配并抽取共用校验，不能简单把原实例跨HTTP客户共享 |
| typed client / Core / Store | `with_mcp_credential`经hello绑定Core签发Principal；`dispatch_payload`每次从当前凭据/授权快照重新authenticate，Apply接受事务再查精确授权。连接与权限逻辑已有，不新增认证数据库、代理owner或RPC透传 |
| `rmcp=3.10.0` | 本地源码已有stateless、请求Parts扩展、Host/Origin、流式body上限及断线取消；HTTP feature尚未启用。默认legacy session=true、metadata强制=false、空Origin列表不校验，均须明确覆盖；SDK默认未知通知202也不能当本片通知支持 |
| HTTP宿主与错误边界 | 尚无TCP监听、资源上限、Bearer→请求私有Core连接接线；SDK部分500 helper直接格式化Display错误且tracing可打印原消息，须在HTTP出口收敛静态有界错误并移除秘密上下文。不增加管理数据库、通用脱敏系统或观测RPC来完成本片 |

**推荐首片：严格回环、预配Bearer profile。** `Authorization: Bearer`承载现有Core签发MCP凭据，只支持已配置Bearer的原生客户端；不把其冒称OAuth access token，不声称支持OAuth自动登录。它是同一MCP客户端的预配凭据，可用于本机STDIO/该HTTP入口，只沿用既有轮换/撤销，没有新增过期时间或HTTP resource/audience声明；不接受第三方token。这是对HTTP认证SHOULD采用OAuth的明确、有边界的偏离，所有者已批准此首片范围；不能把OAuth profile内的MUST降为可选。本片不提供假PRM/authorize/token/refresh/动态注册端点或scope字符串映射。完整OAuth仍是M8待处理项，不因此宣称M8认证全部完成。若本片必须覆盖OAuth-only客户端，应改选完整OAuth并先冻结真实授权服务器、resource/audience及既有Principal绑定差异，不能在本片暗建第二套身份。

| 合同面 | 已批准的具体边界 |
| --- | --- |
| 监听/CLI | 复用`serve --port <0..65535>`，默认`127.0.0.1:0`，成功后stderr报告无凭据实际`http://127.0.0.1:<port>/mcp`，stdout空。无持久配置、新host参数、IPv6补绑定、自动公网暴露或代理模式；非回环及TLS部署另审，不信任Forwarded/X-Forwarded-* |
| HTTP路由 | 仅`POST /mcp`，不接受query/fragment式凭据或路径别名、不重定向；其他路径404，/mcp的GET/DELETE/HEAD/OPTIONS为405且Allow: POST。仅HTTP/1.1首片，HTTP/2/TLS及浏览器客户端不在此片 |
| Bearer/Principal | 每个POST单独携带唯一Authorization；scheme按HTTP规则解析，凭据沿既有64个ASCII小写十六进制字符格式（256位随机数）；缺失401，重复/畸形400，Core确认无效/已轮换/已撤销401。所有失败无owner回退。HTTP不得借用STDIO环境凭据，也不从cookie、URL、clientInfo、参数或连接历史取得身份 |
| Core绑定与生命周期 | 每个请求新建独立、带该Bearer的typed Core连接，直到请求/响应流结束释放；同一HTTP keep-alive连接可先后使用不同Bearer，但后端连接不共享、不池化。idle TCP不保存Principal；仅活跃请求/流持有Core连接，沿现有Core空闲退出/安全自动启动逻辑处理后续请求。Core每次调用及接受事务继续查当前授权；不缓存鉴权成功或授权快照 |
| 撤销竞态/结果 | hello认证失败在HTTP层401；hello后授权减少/撤销仍由Core拒绝实际RPC，不被预检或幂等重放绕过。业务权限/范围拒绝保留现有tool `isError/permission_denied`，可为HTTP200；不得把它伪称OAuth insufficient_scope或自动申请权限。已经接受任务保持原Principal及不可变Plan/journal绑定 |
| Host与DNS rebinding | 实际绑定后只允许精确`127.0.0.1:<实际端口>` authority，检查Host和absolute-form URI一致、拒绝缺失/重复/冲突值、userinfo及非本端点authority；不按DNS解析结果放行，不允许localhost别名、后缀/通配host或任意port。SDK校验作为基础，重复/歧义输入须窄补并测试 |
| Origin/CORS | 本片无浏览器跨域使用：SDK显式`enforce_origin_validation()`并使用空allowlist，拒绝所有存在的Origin（包括null）；无Origin的原生客户端仍必须Bearer。无CORS放行头、cookie认证或成功preflight；不以缺Origin等同有权限 |
| 线协议 | 单个UTF-8 JSON-RPC request；Content-Type application/json（可UTF-8 charset），Accept须同时允许application/json和text/event-stream。强制2026-07-28 header及每请求body metadata；Mcp-Method、tools/call的Mcp-Name与body核对，复用SDK Base64 sentinel规则；六工具未声明x-mcp-header，不新增参数头授权。批量/客户端response/不支持通知400，HTTP notifications/cancelled不受理，不广告通知扩展 |
| SDK模式 | `legacy_session_mode=false`、`stateless_protocol_metadata_required=true`、`NeverSessionManager`、无session/event store；不发/不接受session ID、Last-Event-ID恢复或旧initialize/ping。保留仅六工具能力和静态定义；实现ServerHandler薄适配并明确拒绝未实现方法，不能启用默认空资源列表等占位成功 |
| JSON/SSE | 复用SDK请求级SSE，`json_response=false`，也接受SDK按状态返回单JSON；keep-alive仅15秒SSE注释，禁用SSE retry/replay。终态后关闭流，不新建订阅或第二套进度系统，继续用Operation get/cancel。Cache-Control: no-store，tools/list沿用ttl0/private |
| 大小/并发 | HTTP头16KiB/32字段，JSON body最多4MiB（流式累计，chunked不能绕过），拒绝非identity Content-Encoding；结果沿现有3MiB工具data/4KiB文本，完整HTTP MCP响应预算4MiB，不截断成成功。最多32 TCP连接、16个活跃请求/流，无无界等待队列；头/空闲10秒、body15秒、Core连接/处理60秒及输出10秒有界，均为产品限制而非MCP MUST |
| 取消/超时/停机 | 客户端在首条响应前断开、关闭SSE、处理截止或进程正常停止，只结束该HTTP请求与其Core IPC exchange，不发operations.cancel、不取消已接受Operation。截止时已知未发写可报core_unavailable；写请求可能已发则使用既定core_write_outcome_unknown/unknown/retryable=false，不能盲目换键或重试。连接已丢时不能保证客户端收到错误，仍须以原Plan/revision/key显式恢复。Ctrl-C/SIGTERM停止接收、5秒内清理请求连接，Core业务由Core继续执行/恢复 |
| 秘密与日志 | 在进入SDK前移除Authorization，仅在请求私有扩展传入已认证Core连接；不把原header/凭据写入SDK tracing、access log、事件、普通错误或URL输出。禁止raw request/body/header/debug dump，日志仅固定阶段/状态和opaque diagnostic_id；不启用SDK会打印消息的debug/trace subscriber，不转发baggage。普通错误不回显凭据、URL query或内部认证原因 |

错误层次固定：格式合法但不允许的Host/Origin 403，HTTP语法/重复Host/冲突authority 400，头超限431；缺/失效凭据401（有界WWW-Authenticate: Bearer challenge，不伪造resource_metadata），畸形认证400；HTTP method 405、媒体类型415、Accept 406、body超限413、读头/body超时408。TCP配额满直接拒绝连接，活跃HTTP请求配额满429（Retry-After: 1），Core在接受业务前不可用503，均不能报业务已回滚。SDK按冻结规则处理HeaderMismatch -32020、缺必需能力 -32021、版本不支持 -32022及缺metadata为400，方法不支持404/-32601；普通工具参数-32602和业务失败沿SDK JSON-RPC/HTTP200语义，不擅自改工具合同。合法请求的在途Core写超时以HTTP200完整tool error交付unknown；已开SSE后只能发准确终态并结束，不能假称已改HTTP状态。所有限制在Core业务调用前尽量拒绝，SDK原始Display/解析错误不直接外透，保留稳定code及有界静态文案，意外内部错误给opaque diagnostic_id而非反射请求。

**必要内部与依赖差异（原两包及直接边已批准，base64增量已批准）**：不新增Core RPC、State、Config、权限名或身份存储；只新增HTTP公开入口/安全与生命周期合同。复用SDK HTTP解析/协商/头核对/SSE，不用Tokio手写HTTP或另造协议引擎。提升现有锁版本`hyper 1.11.0`（server/http1）、`hyper-util 0.1.20`（tokio）、`http-body-util 0.1.5`为最少直接边，基础HTTP类型复用Hyper导出；不加axum/tower-http/oauth2/JWT/TLS框架。启用现有rmcp的`transport-streamable-http-server`会新增`sse-stream 0.2.4`，Hyper server会新增`httpdate 1.0.3`；两者均MIT OR Apache-2.0、官方未yank，前者当前0.3.0不满足SDK的0.2系列边界，不为用新版本升级已冻结SDK。替代方案手写SSE/HTTP会增加协议风险；其余依赖已锁，批准后才核对实际lock闭包并同步xtask边界，遇额外必要包先说明，不夹带升级。测试可启用同SDK的HTTP client/reqwest特性，复用既有锁定reqwest，不建立通用测试框架。

**本片获批验证要求**（以下是要求，执行结果见末节）：

1. 冻结SDK Discover经真实loopback HTTP和隔离Core，双Bearer零授权/显式授权/范围外/同TCP交替身份/并发同请求ID隔离；同一六工具实际卸载及Plan/Operation/幂等归属保持。
2. 连接存续期间授权减少、轮换、撤销；hello到Apply接受间变更仍拒绝；无凭据、错误Bearer、STDIO环境凭据均不能降级owner。复用既有身份/Store证据，只补HTTP引入边界。
3. Host/absolute URI冲突、重复头、攻击域名解析到loopback、null/任意Origin、CORS/preflight、URL/cookie/重复Bearer及原始请求泄密标记；确认失败不触发业务写入。
4. 版本/能力header-body不一致、方法/名称（含Base64）、Accept/Content-Type、非法JSON/batch/response、旧session/initialize/ping/GET/DELETE/HTTP取消通知，错误状态与body均精确。
5. Content-Length与chunked边界、4MiB超限、压缩拒绝、慢头/慢body、32/16配额及释放、超时/停机、无GUI/管理扩展时独立工作；SSE断线不泄漏任务、连接或permit。
6. 复用本轮Core响应门控，真实Apply接受后关闭HTTP/SSE及截断写响应，证明无自动POST重试、无业务cancel、原键恢复同Operation且项目只提交一次；不能只用mock错误或借STDIO通过替代HTTP证据。

批准前本轮只完成源码/冻结来源/依赖静态审计和提案，未开启HTTP监听、占用Core端点、修改运行程序/数据或执行新的完整测试。官方资料、SDK/锁文件摘要及必要包元数据在`target/m8-http-audit-20261113/`。预配Bearer profile及原两包和直接依赖边现已批准，额外base64依赖现已批准。管理扩展Host/WIT与界面合同随后单独整理，不与本片同时实施，不通过私有数据库或宿主特权访问客户端数据。

#### 审批等待期间的实施准备（历史状态）

- 已准备回环`serve`入口、严格Host/Origin/媒体准入、逐请求Bearer→独立typed Core连接、SDK无session适配、请求/输出限额和正常停止。共享六工具及Core身份/权威未重做；STDIO原调用入口保持，连接失败提示改为适用于两种传输的文字。
- 静态核对修正SSE处理/输出截止的区别；输出超限/截止必须产生传输错误，不能把截断当作正常EOF。此处仍为源码，不能代替实际测试结果。
- 测试源码复用既有真实Core卸载及故障注入流程，增加HTTP SDK入口、同TCP切换Bearer/复用请求ID、metadata/header差异、旧协议拒绝、16请求饱和/断开释放与chunked body限额，以及无Core的Host/Origin/凭据环境变量隔离检查。旧STDIO测试仍保留其独立断言。上述HTTP用例及受影响STDIO回归均**未执行**。
- `cargo xtask check`、锁定离线metadata、最终fmt及diff检查已通过；metadata确认MCP正常本地依赖仍精确为client/protocol。首次HTTP `cargo check`在新增依赖闭包暴露后主动中断，**未完成**。没有执行完整Workspace、Chromium、release或新的CI/CodeQL；HTTP代码仍未提交/推送，尚无稳定候选SHA或Hosted证据。原Core/GUI及数据未暂停、替换或迁移，未恢复桌面控制。

base64增量获批后继续编译、严格MCP Clippy、定向测试及获批的真实隔离验证；到时核对实际结果、恢复原环境并记录输入，再形成本地候选，不以本段的源码准备宣布HTTP或M8完成。

#### base64 0.23.1 最终依赖核对

所有者明确批准本轮SDK传递依赖。首先执行要求的`cargo tree --locked -e features -i base64@0.23.1`，并另以`-p alcomd-mcp -e normal,features`排除dev边核实生产路径：`alcomd-mcp → rmcp/transport-streamable-http-server → server-side-http → base64/default → std/alloc + simd-unsafe`。dev HTTP SDK client也复用同一版本；ALCOMD任何manifest不新增直接base64依赖，也不直接调用其API。

精确锁版本0.23.1，MIT OR Apache-2.0，MSRV 1.71.0，checksum沿上文。用途是冻结2026-07-28 HTTP头Base64 sentinel编码/解码核对。默认`simd-unsafe`确实开启，包含第三方`src/engine/simd.rs`中的AVX2/NEON intrinsic、指针读写及unsafe块，第三方提供按平台/运行时指令集检测的SIMD引擎，不能描述为没有unsafe影响或整个第三方依赖无unsafe。维护边界：不修改SDK或第三方源码、不假称关闭该feature、不以新增直接依赖强行覆盖feature；沿用审批的版本/头及body限额，补实际长Base64头正反例覆盖。上游安全保证仍依赖第三方实现，本轮不宣称完成对其SIMD unsafe的形式证明或跨平台专项验证。

完整/仅生产feature tree分别保存于`target/m8-http-audit-20261113/base64-feature-tree.txt`、`base64-production-features.txt`。其中SDK的server-session支持代码是该既定HTTP feature的传递边，运行时仍显式legacy=false/NeverSessionManager，不启用协议session。首次offline tree因未缓存精确sse-stream0.2.4失败，随后仅下载锁定包重查成功；无包升级，不改HTTP/身份合同，最终lock/feature清单随候选复核。
#### HTTP本地候选集中验证（历史候选准备）

已完成获批回环HTTP接线，不增加Core RPC/State/Config/权限或身份框架。每请求独立认证typed连接、严格Host/Origin、无session/owner回退；SDK负责冻结协议和SSE，GUI/原Core不参与HTTP宿主。`serve --port 0`报告无凭据URL，stdout空。ALCOMD生产源码没有直接base64 API或新增unsafe。

最终审查补齐SDK只读取首值的重复MCP版本头拒绝；其Method/Name重复核对继续由SDK处理。准入/单元/STDIO集中命令最新一轮分别5/0、2/0、12/0，合计19/0，日志`no-core-tests-candidate.log`，不拼接旧失败轮次。实际HTTP SDK两项流程先前完整2/0（`native-preservation/final-20261113T1534098413501Z/risk-test.log`），覆盖37条业务工具记录和31条风险工具记录；5类故障产生5个独立Operation，10次Apply=5次原请求+5次原键显式恢复，零自动重试、零业务cancel、均成功且各提交一次。新增版本头准入后将再对准确源码候选集中复核这两项，不提前将前一轮标为最终候选通过。

历史真实结果保留：首轮HTTP check因依赖审批中断；随后SDK optional DTO两次编译错误，修正后all-targets check通过。资源测试首轮HTTP 1/1揭示Hyper读头超时直接EOF而非约定408；窄补固定408后关联轮次1/1揭示失去原写半关闭语义的Windows reset，恢复正常shutdown后2/0，最后19项同轮通过。各轮原始日志保留，没有改超时或降低断言。严格MCP Clippy和xtask已通过，最终候选还将核对fmt/metadata/diff/lock与feature清单；不执行完整Workspace、Chromium、release或三平台CI。

真实环境事实与旧流程前提不同：原Core/GUI进程和旧`target/debug`两份程序在本轮开始验证时已经不存在；首次快照连接超时、旧程序hash前置缺失，均未开始测试或关闭任何程序。随后按真实关闭/缺失状态保全，保持关闭，不为测试重启或重建原程序。最终已执行轮次原36份data、346份profile逐文件SHA-256不变，7个Unity/Hub进程PID/路径/启动时间保持，测试后无Core/GUI/MCP残留；原数据未迁移。没有声称对关闭环境进行实时五组RPC核对或“恢复运行成功”。相应backup-manifest/result及468份输入在`target/m8-http-audit-20261113/`。

base64 feature tree/锁审计确认只新增0.23.1、httpdate1.0.3、sse-stream0.2.4，无既有包升级/删除，22份工作区manifest均无直接base64。SDK当前头实现调用BASE64_STANDARD/GeneralPurpose的scalar路径；默认SIMD unsafe模块仍编译进入依赖，不能据此说整个依赖没有unsafe，更没有宣称验证第三方SIMD跨平台正确性。
#### HTTP本地候选与验证结果（2026-11-13）

**本地技术验证通过，远端待验证；不是HTTP人工签收或M8完成。** 源码候选`8338d3e5480a90ba3ac818f31684312861228eb1`，真实Windows用户`cqmhv-dt\m2922`在标准main本地提交。后续仅整理状态文档，不推送。

| 本轮候选验证 | 实际结果与范围 |
| --- | --- |
| 受影响MCP构建 | `cargo test --locked --offline -p alcomd-mcp --lib --test stdio --test http_transport --test business_tools --no-run`通过，日志`candidate-build.log`；没有重建release/GUI |
| 同候选无Core定向测试 | 已构建测试二进制同轮单元5/0、HTTP准入/资源2/0、STDIO12/0，`candidate-no-core.log`；不是完整Workspace测试 |
| 真实HTTP SDK→隔离Core | 单轮2/0，80.15秒，`native-preservation/candidate-8338d3e-20261113T1544504023202Z/risk-test.log`；默认拒绝/范围/同TCP交替及并发同ID隔离，授权减少、hello后撤权、轮换/撤销、重启持久及六工具真实卸载均通过 |
| 五类在途故障 | 请求取消、Core响应截断、真实HTTP响应前缀截断、生产60秒处理截止、HTTP服务正常停止均通过；5个Operation全部succeeded，各提交一次，cancel_requested=0；10次Apply仅含5次原请求和5次原Plan/revision/key显式恢复，自动重试0次。Core截断/处理截止2个完整unknown结果均retryable=false；HTTP连接已丢的情况不伪称收到tool结果 |
| 冻结工具合同 | `schema-candidate.log`：现有六工具Schema与37条业务、31条风险调用记录的适用输入/结果校验通过；认证401记录单独辨认，不伪造成业务返回或不合法参数 |
| 一致性 | 同候选严格`cargo clippy --locked --offline -p alcomd-mcp --all-targets -- -D warnings`、fmt、xtask、锁定metadata、diff通过；最终完整/仅生产feature tree再次核对，SDK HTTP feature/default/simd-unsafe路径符合批准。只新增base640.23.1/httpdate1.0.3/sse-stream0.2.4，既有包无升级或删除，无ALCOMD直接base64依赖 |

468份输入摘要`e0204d39fa67110295dbfd4bf47fc073d8ed650136d84d45895dbff468025491`，实际MCP可执行文件`target/m8-check/debug/alcomd-mcp.exe` SHA-256 `f1e22dcbf283034476ab8032312030acf9112c897b6c5ccd82bc11b03aa9b961`。输入、测试二进制摘要及结果索引在`target/m8-http-audit-20261113/candidate-summary.json`、`source-inputs-candidate.json`；本次仅文档收尾后再核对输入未变，不重新制造一轮全量测试。旧失败及前一轮2/0证据仍保留各自输入和原始日志，没有拼为最终套件通过。

原环境维持验证前真实关闭状态；旧target/debug程序缺失状态保持。原36份data/346份profile逐文件摘要、7个Unity/Hub进程身份与启动时间相同，最终无Core/GUI/MCP残留。保全记录为上述candidate目录的`backup-manifest.json`/`result.json`；没有以“守候”代替核对，也没有为了快照重启原环境或迁移其数据。

**未执行/后续边界**：HTTP新代码无Hosted CI/CodeQL，尚未推送；三平台旧3ccc3f4通过只属于旧STDIO/身份/六工具切片。此轮真实运行仅Windows；Unix Ctrl-C/SIGTERM及macOS/Linux HTTP尚未实测，第三方SIMD unsafe未做专项跨平台证明。真实SDK为冻结rmcp3.10.0，未逐个验证其他MCP客户端。已有测试覆盖10秒输出边界的错误/permit释放，但未声称穷尽所有网络背压、操作系统崩溃和并发竞态。完整OAuth profile、非回环/TLS/HTTP2/浏览器跨域、管理扩展/Host/WIT及安装/搜索/重装来源委托均不在本片实现，不开放无认证入口、不代持owner。M8保持IN_PROGRESS，后续公共差异仍按既有计划单独审阅。
### aab87a8获准推送、首轮远端失败与本地窄修（2026-11-14）

所有者明确确认后，以真实Windows用户在标准main核对干净工作树、origin地址及完整四笔`f5042f3a → df7d2700 → 8338d3e5 → aab87a8d`，普通快进从3ccc3f4推送至`aab87a8db279ceaa8c2b8a54c93f91798f2eedd5`；远端实读相同，468份已验输入未变、8338源码后的差异仅两份状态文档。没有强推、标签、发布、手动dispatch或rerun。该次push的CI `43534404444`已全部收齐，总结论failure：Ubuntu完整成功，macOS/Windows分别失败。CodeQL `43534404321`四项成功；仅收集这一轮自动运行，没有dispatch/rerun。

| aab87a8本轮远端 | 实际结果 |
| --- | --- |
| macOS 15 arm64 | 失败。`Check`中MCP单元5/0、真实业务工具4/0（含两项HTTP和两项STDIO）、身份1/0通过；HTTP准入模块1/1，`raw()`读响应遇`ConnectionReset`，后续合同/release/Tauri/浏览器均未执行 |
| Windows Server 2025 | 失败。`Check`中MCP单元4/1；到期输出用例期待TimedOut，却得到`Collected([late])`。该点之后的业务/HTTP模块、合同、release/Tauri/浏览器均未执行 |
| Ubuntu 22.04 | 全流程成功。Check（含严格Clippy、Workspace Rust及前端检查/构建）、合同、release、Tauri、GLIBC/锁文件/diff和Chromium703/0均完成；挂载夹具步骤虽成功但实跑0项，见末节覆盖更正；浏览器日志703 passed（14.6m），不与其他平台或轮次拼接 |
| CodeQL | Actions、JavaScript/TypeScript、Python、Rust四项成功，绑定aab87a8，不能当作本地新增窄修的结果 |

**关闭边界修复ddd557f**：旧macOS日志没有请求子项标签，只能精确定位到准入helper的TCP读。检查拒绝大输入后的关闭路径，发现发送FIN后立即释放仍有未读输入的接收端；这会让错误响应读到reset。新真实socket回归在本机原实现0/1，证明接收端在对方发送完成前被释放；没有声称在Windows复现macOS错误码54。窄修为写半关闭后，在既有10秒及4MiB+16KiB预算内丢弃待读字节直至peer FIN，再释放；不重新解析、鉴权或处理请求，不扩大接受上限，不记录输入、不延长既有5秒停机宽限。参照[Microsoft shutdown关闭序列](https://learn.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-shutdown)。原431/408/准入及完整读断言保持，诊断仅添加请求/响应字节数，不打印headers/body/凭据。修复后macOS远端仍未验证。

**绝对输出截止修复33811d4**：Windows原断言没有错误；仅检查Sleep readiness会在timer driver处理tick前放过已经到绝对截止的ready数据。新“截止等于now、无tick”的用例本机原实现0/1，返回late，确认根因。GuardedBody现先比较单调Instant与绝对截止，保留Sleep唤醒及原TimedOut/permit释放路径；不延长10秒输出预算、不改旧过去1ms断言或将错误改为成功。

修复源码候选`33811d415a73fbcbceb88b1a9c1dcf7a155179d1`仅本地，含前一笔`ddd557fd11159a57378365ed3f410f2e7142cef1`。同输入集中验证：单元7/0、HTTP2/0、STDIO12/0（`deadline-regression-fixed.log`）；真实HTTP/Core单轮2/0、81.15秒，五类故障/5个Operation/10次Apply/5次原键恢复、自动重试0、业务cancel0均保持，37条业务+30条风险记录按既有六工具Schema校验。严格MCP Clippy、fmt/xtask/diff通过，无新RPC/State/Config/权限/依赖变化，不重跑全Workspace、浏览器或release。

468份输入摘要`3596d671fae78cfc4b5bd1750f54f5e7940e3ad325d1204af660285533e0d291`，MCP可执行SHA-256 `d760cf0f6db06b4cbd7a05bc652549f0a9f74a0c3962854cf4f04c68bdac2e60`。远端JSON/逐job日志、本机两个0/1复现、修复后记录及`fixed-summary.json`保存在`target/m8-hosted-aab87a8-20261114/`；只读状态查询的两次短暂失败均只重查同一运行，没有重跑workflow。CI/CodeQL最终JSON及Ubuntu/macOS/Windows逐job原始日志均已归档，最终结果不能用于33811d4两笔新增修复。

本轮两次隔离均保持原Core/GUI关闭、旧程序缺失状态；原36份data/346份profile逐文件摘要不变，保全当时10个Unity/Hub进程。最终修复保全见`target/m8-http-audit-20261113/native-preservation/hosted-two-fixes-20261114-20261114T0059073579071Z/result.json`，未操作真实项目或恢复桌面控制。macOS/Windows历史失败保留，修复后的Hosted CI/CodeQL仍未取得；原四笔推送授权不覆盖新增提交，现有结果已收齐，新增修复及本节记录等待单独确认额外推送。M8保持IN_PROGRESS，不以本地修复宣布远端通过或M8完成。
本次远端失败没有导致修改工作流、删测试、放宽状态/完整读/截止断言或延长超时。本地新增修复只涉及HTTP模块和准入测试的有界诊断，公共合同/依赖/权限保持。`fixed-summary.json`核对67条适用输入/结果记录与5类故障，不能把旧轮68条记录写成本轮数量；Operation轮询次数可随真实完成时间变化。源码候选33811d4的21项无Core定向测试与2项真实HTTP/Core同输入结果、严格MCP Clippy、fmt/xtask/metadata/diff通过；后续仅本地文档整理，未再次运行全量验证、构建或触发CI。原四笔推送已完成，新增三笔（两项窄修及本次状态记录）尚未推送，需按项目推送边界明确授权后方可用新候选取得远端结果。
### e3cd4ac获准推送与修复后同SHA门禁（2026-11-14）

所有者再次明确批准三笔提交后，核对真实Windows用户/main、干净工作树、远端aab87a8及完整链`ddd557f → 33811d4 → e3cd4ac`，普通快进推送至`e3cd4acb7db91e6a464272a947f0d4c7768055c1`，远端实读一致。源码仍为33811d4，之后仅两份状态文档，468份输入摘要3596d671未变；不因推送重复本地Rust/HTTP/浏览器全量或release构建，没有替换原Core/GUI或操作原数据。

该push自动启动CI `43545258103`和CodeQL `43545257774`，均绑定e3cd4ac；现已全部收齐：CodeQL四项成功，CI总结论cancelled（macOS/Windows完整成功，Ubuntu因60分钟总预算在浏览器阶段取消），不能标为三平台完整通过，不复用aab87a8的成功/失败充当修复后结果。只读取这两轮既有run，无手动dispatch/rerun。当前JSON、最终日志与推送/输入审计保存于`target/m8-hosted-e3cd4ac-20261114/`。本次授权仅覆盖上述三笔；结果收尾只本地整理，不额外推送纯状态提交触发验证。M8保持IN_PROGRESS。
#### e3cd4ac的CI总预算阻塞与重复执行收束（本地候选）

Ubuntu check annotation明确为`The job has exceeded the maximum execution time of 1h0m0s`，不是测试断言失败或人工取消。该job在02:15:26Z开始、03:15:46Z结束；Check成功19.82分钟，后续Test repository contracts又用9.03分钟，release成功10.60分钟、Tauri成功3.35分钟，浏览器启动703项/2 workers、运行15.83分钟后被取消，没有完整终态，不能写703/0。macOS同SHA整个job成功，Chromium703/0（22.4m）；Windows同SHA整个job也已成功，Chromium703/0（18.6m）；本轮全部结果已收齐。

原因审计确认`.github/workflows/ci.yml`连续调用check和test聚合脚本：Check已包含完整`cargo test --locked --workspace`、Discord测试及`npm run check`；test脚本重复这三项，只额外增加Core/CLI构建与CLI自动启动集成。修复`2a319d7a`仅更改三平台工作流编排：完整Check仍为默认必经前置步骤，之后直接执行原`cargo build --locked --package alcomd --package alcomd-cli`及原test-m1-auto-start脚本。没有删除测试文件、测试用例或断言，没有skip/continue-on-error，没有减少Rust/浏览器覆盖；Workspace/Discord/前端检查仍各完整执行一次。所有其他步骤、触发器、权限、runner及90/60/90分钟作业上限保持原样，测试和产品内超时也没有改变；本地check/test聚合脚本保持可独立使用的原行为。

固定基线e3cd4ac与新YAML的逐job结构/命令集合审计通过，证据`ci-coverage-audit.json`验证每个原测试门禁都保留在未改动的Check、CLI附加用例仍保留且所有其他步骤相等。PyYAML6.0.3仅置于忽略的target审计目录，未改生产或CI依赖/manifest/lock。PowerShell AST、两个Bash -n及metadata/xtask/diff检查通过；Bash首次因沙箱不能创建MSYS命名对象而未执行，改在真实Windows用户环境完成语法检查，没有实际运行CLI/Core或重跑本地全量。未声称新编排已在远端通过；该新增工作流提交仍仅本地，本轮既有结果已收齐，待单独申请新增提交推送，不手动rerun已取消job。
#### e3cd4ac最终结果与待推送编排候选

| 本轮实际结果（均为e3cd4ac） | 结论 |
| --- | --- |
| CI 43545258103 | 总结论cancelled，非三平台完整通过 |
| Windows Server 2025 | 整个job成功，严格检查/完整Rust/CLI集成/release/Tauri/锁/diff/Chromium703/0，零重试；浏览器18.6分钟 |
| macOS 15 arm64 | 整个job成功，包含完整Rust/CLI集成/release/Tauri/部署目标/锁/diff/Chromium703/0；挂载夹具步骤虽成功但实跑0项，见末节覆盖更正，零重试；浏览器22.4分钟 |
| Ubuntu 22.04 | 严格检查/完整Rust/CLI集成/release/Tauri/GLIBC/锁/diff均成功；挂载夹具步骤虽成功但实跑0项，见末节覆盖更正；浏览器703项执行中触及job60分钟预算而取消，没有最终计数，不记录为PASS |
| CodeQL 43545257774 | Actions、JavaScript/TypeScript、Python、Rust四项全部成功 |

两个HTTP关闭/截止缺陷的新增回归及真实HTTP/Core闭环已在三平台Rust测试通过；当时定位的未收束门禁是Ubuntu的完整浏览器终态及总作业预算；3196e1b收尾又发现继承的挂载夹具空跑覆盖缺口，见末节更正。Check与Test在同job重复执行完整Workspace/Discord/前端检查的证据保留原日志，不能用Mac/Win或aab87a8的浏览器结果拼接成Ubuntu当前通过。此轮只读查询偶发失败均仅重查原run，无手动dispatch/rerun。

待推送工作流候选`2a319d7a7d0f89bf72dcf728d8c778178eb3f95c`仅修改ci.yml三处串行编排，没有改生产代码、测试、聚合脚本、依赖、断言、超时或仓库设置。完整Check仍为每job默认必经前置，后续保留原Core/CLI构建及自动启动脚本；所有其他步骤及锁文件最终核对逐项相等。固定e3cd4ac基线的YAML结构/门禁集合审计、PowerShell AST、Bash -n、metadata/xtask/diff通过。468份运行时/测试输入3596d671不变，保留33811d4与e3cd4ac的有效功能证据，不为仅编排和文档变更重跑本机全量。

ci-final/codeql-final JSON、三平台逐job日志、Ubuntu超时annotations及ci-coverage-audit.json均保存在`target/m8-hosted-e3cd4ac-20261114/`。新编排未在远端执行，不能提前记为通过；原三笔推送授权已用完，工作流候选及本节记录仅本地提交，等待新的明确推送授权，下一轮只由新push自动验证，不人工重跑旧job。M8保持IN_PROGRESS，完整OAuth/管理扩展Host桥等范围仍未实施。

### 3196e1b获准推送与去重编排远端验证（2026-11-14）

所有者明确批准后，核对真实Windows用户/main、干净工作树、远端e3cd4ac及完整两笔`2a319d7a → 3196e1b2`，普通快进推送至`3196e1b2c099d990cd561304f2ac7a52df33831d`，远端实读一致。468份运行时/测试输入摘要3596d671保持；本批只改CI编排及两份状态文档，不因推送重复本机全量、release或HTTP闭环，不操作原Core/GUI及数据。

本次push自动启动[CI 43556014614](https://github.com/ALCOMD/ALCOMD/actions/runs/43556014614)及[CodeQL 43556014397](https://github.com/ALCOMD/ALCOMD/actions/runs/43556014397)，均绑定3196e1b；现已收齐全部结果，CI总结论failure，CodeQL四项成功。仅查询这两项既有运行，不手动dispatch/rerun、不把e3cd4ac或更早结果写为本候选通过。推送/输入审计、最终JSON及三平台逐job完整日志归档于`target/m8-hosted-3196e1b-20261114/`。M8保持IN_PROGRESS。

| 3196e1b本轮实际结果 | 结论 |
| --- | --- |
| Windows Server 2025 | 整个job成功，完整Check/CLI/release/Tauri/锁/diff及Chromium703/0，零重试；浏览器13.1分钟 |
| Ubuntu 22.04 | 整个job成功，完整Check/CLI/release/Tauri/GLIBC/锁/diff及Chromium703/0，零重试；浏览器14.4分钟、job约43.2分钟，未触及60分钟预算。旧挂载步骤虽成功但实跑0项，不记有效挂载覆盖 |
| macOS 15 arm64 | 完整Check及CLI成功，HTTP相关回归和真实Core闭环随Check通过；随后挂载夹具Cargo空跑0项、卸载退出16。release/Tauri/部署目标/锁/diff/Chromium均未执行，不拼接e3cd4ac结果为本轮通过 |
| CodeQL | Actions、Rust、Python、JavaScript/TypeScript四项成功，均为3196e1b；不覆盖本地新增夹具修复 |

三平台新CLI集成均明确报告权威daemon数量1，去重编排没有跳过该附加验证。当前远端真实阻塞为macOS挂载夹具清理失败，并须补回Linux/macOS挂载测试实际执行证明；已认可HTTP身份/六工具及33811d4功能证据继续有效。收尾只做适用metadata/xtask/diff，新增夹具提交及本节状态记录仅本地保留；旧推送授权已用完，不自动推送或用旧运行冒充新修复通过。

#### 挂载夹具空跑与清理窄修（本地6d370e9）

3196e1b的macOS原始日志显示：完整Check、CLI自动启动通过后，挂载夹具的Cargo命令返回`0 passed; 0 failed; 40 filtered out`，随后EXIT清理中的`hdiutil detach -quiet`退出16。脚本路径确认退出来自卸载，不是CLI或HTTP断言；静默日志没有给出具体占用者，不声称已识别哪个进程。16对应Darwin的EBUSY（[Apple errno定义](https://raw.githubusercontent.com/apple-oss-distributions/xnu/main/bsd/sys/errno.h)），本次移除卸载的quiet以保留后续真实原因，不强制卸载或终止进程。

检查源码确认真实忽略测试全名为`project_delete::tests::real_nested_mount_is_rejected_and_its_sentinel_survives`；旧脚本缺少`project_delete::`，搭配`--exact`造成零命中，不是缺少Cargo feature（该包没有feature门控）。同样核查已归档aab87a8的Ubuntu、e3cd4ac的Ubuntu/macOS及当前3196e1b的Ubuntu，均是0项；此前“挂载边界通过”的覆盖推断予以纠正，原job成功状态与其他有效结果保留，不能再据此宣称真实挂载断言已执行。证据`target/m8-hosted-3196e1b-20261114/mount-zero-test-audit.json`；不重开M7或撤回已有人工签收。

本地提交`6d370e9378b86bca9e3d0e751248855183bdf1ce`只修原挂载脚本：完整模块名与`--lib`；挂载前精确列出并要求命中1项；执行后必须同时出现具名ok与1通过/0失败/0忽略，零执行不再成功。macOS只对卸载16作最多5次正常尝试、间隔1秒；其他错误立即失败，重试耗尽或Linux卸载失败均保留尚挂载的夹具，禁止继续递归清理；保留原测试失败退出码，单独清理失败也不能报成功。无生产代码、Rust测试断言、依赖、公共合同或工作流改动。

真实Windows用户完成最终文本Bash语法检查及15项受控命令分支检查：Linux/macOS正常、预检空/重复/失败、运行空/错名/忽略/失败、繁忙恢复/耗尽、非繁忙失败、测试与卸载同时失败、Linux卸载失败及不支持平台。假命令仅验证脚本流程，不执行真实mount/hdiutil/Cargo/Core，也不替代Unix挂载证明；结果在`fixture-checks/results.json`及逐用例trace/output。metadata、xtask、diff通过；468份Rust/HTTP等既有输入逐文件摘要3596d671保持（该清单不包含本次修改的Unix夹具脚本），没有重跑本机完整Rust/Chromium/release。新增夹具的真实Linux/macOS执行仍未验证，需后续获准推送的自动CI；本轮已结束CI是3196e1b，不能覆盖本地6d370e9。该新增提交未获推送授权，当前全部远端结果已收齐，待以准确完整候选另获推送确认。

### f452ac5获准推送与真实挂载验证（2026-11-14）

所有者明确批准后，以真实Windows用户在标准main核对干净工作树、origin/main基线3196e1b及完整两笔`6d370e9378b86bca9e3d0e751248855183bdf1ce → f452ac59788fb49653e8db693e0e02cacd468bf0`，普通快进推送至f452ac5，远端实读一致。首次只读ls-remote遇TLS EOF，保持TLS校验原样重查成功后才推送；没有重试不明写操作、强推、标签、发布或手动workflow。468份既有Rust/HTTP输入3596d671不变，Unix夹具脚本摘要单独记录，不因推送重跑本地全量、构建或受控夹具检查。

本次自动启动[CI 43561391720](https://github.com/ALCOMD/ALCOMD/actions/runs/43561391720)及[CodeQL 43561391559](https://github.com/ALCOMD/ALCOMD/actions/runs/43561391559)，均绑定f452ac5；CodeQL四项成功、最终JSON及完整日志已归档；CI现已收齐、总结论failure：Windows在项目发现夹具创建Backups时AlreadyExists失败，Ubuntu整个job成功/Chromium703/0（14.7m），macOS在真实挂载/release/Tauri等通过后Chromium702/1（25.2m）。两Unix挂载均具名执行1项、正常清理，不能再描述为尚未覆盖。仅查询上述既有run，重点核对Linux/macOS挂载前列出1项、实际具名1项通过及正常清理，不能将旧0项日志或本机假命令检查当作真实挂载证据。推送审计、当前/最终JSON及完整日志保存于`target/m8-hosted-f452ac5-20261114/`。M8保持IN_PROGRESS，原环境未替换或迁移，结果收尾只本地整理，授权不覆盖额外提交推送。

#### Windows项目发现夹具同刻度碰撞（本地12b4767）

f452ac5的Windows在完整Workspace开始执行首个alcomd库测试时失败：54通过/1失败/1原有忽略；`startup_recovery_reservation_defers_discovery_before_any_prepare_io`在Fixture::new创建Backups时收到Windows183/AlreadyExists，后续alcomd-mcp包测试、CLI、release/Tauri及浏览器未执行。原日志`windows-failed.log`保留，不能用3196e1b的Windows703/0替代。

根因是该测试模块的临时根目录名仅含PID与SystemTime时间戳；同一时钟刻度的夹具会被create_dir_all(Projects)合并，然后在严格创建Backups时冲突，并存在相互清理的风险。没有证据指向真实Core业务或HTTP失败。新增固定同一时刻的回归在原命名行为下0/1，复现同样183；修复`12b4767c31f72f762f8792160bcf17002ac8bbf1`只在cfg(test)夹具中加入进程内AtomicU64序号，并独占创建根目录，再严格创建Projects/Backups。回归同时核对根目录隔离、独立子目录、不会看到另一夹具sentinel，以及删除第二夹具不影响第一夹具文件。

修复后项目发现模块单轮7/0（8个测试线程，包含原失败用例），受影响包严格Clippy、workspace fmt-check及diff通过；没有扩大为完整Rust/浏览器/release验证，没有占用Core端点或操作原数据/Unity。生产部分及从FixturePlatform起的全部原适配器/业务测试逐字节不变；既有468份输入中仅此文件的测试区域改变，不继续声称整份3596d671摘要仍相同。MCP/HTTP生产实现和其余467份原输入保持，原有效证据按各自输入保留。定向前后日志、Clippy及源码边界审计见本轮`discovery-collision-before.log`、`discovery-collision-fixed.log`、`discovery-fixed-clippy.log`、`discovery-fix-source-audit.json`。

所有者随后明确“可以推送”，授权本笔12b4767及本节必要状态记录普通快进推送。提交记录前再次核对定向结果与生产边界，不重复通过的测试；推送前执行便宜的metadata/xtask/diff与完整提交链/工作树核对。f452ac5的Unix其余作业仍须收齐，不取消或手动重跑旧运行；新候选仅由获准push自动验证，不能用旧结果替代。当前授权只覆盖该窄修与记录，不包含其他新增代码或额外推送。

### 7eac047获准推送与夹具隔离远端验证（2026-11-14）

所有者在f452ac5的Unix作业尚未结束时明确“可以推送”；完成metadata/xtask/diff、真实Windows用户/main/干净工作树、完整两笔及origin/main基线核对后，普通快进推送`12b4767c31f72f762f8792160bcf17002ac8bbf1 → 7eac0479bdb40282f1668cb1466185bd218279de`，远端实读7eac047。仅含cfg(test)夹具窄修与两份状态文档，无生产或旧业务断言变更；没有重跑通过的模块、完整Rust/浏览器或release，也没有取消尚未结束的旧作业。

push自动启动[CI 43564128872](https://github.com/ALCOMD/ALCOMD/actions/runs/43564128872)和[CodeQL 43564128896](https://github.com/ALCOMD/ALCOMD/actions/runs/43564128896)，均绑定7eac047；CodeQL四项成功，已逐job归档完整日志，CI三平台整个job均成功，Chromium各703/0、零重试，Windows/Ubuntu/macOS分别18.1/15.9/25.1分钟；两Unix具名真实挂载各1/0并正常清理。f452ac5原CI已独立收齐、结果见下节；新旧轮次的结论、挂载具名1项和浏览器计数分别核对，不拼接。新候选推送审计及当前/最终JSON、日志保存于`target/m8-hosted-7eac047-20261114/`，包含原468份输入的唯一变化及其完整文件hash、生产/旧测试不变证据引用；不把原3596d671摘要直接写成新测试输入。结果收尾仅本地提交，不再推送纯状态文档或手动触发检查；M8保持IN_PROGRESS，未实施完整OAuth或管理扩展Host/WIT。

#### f452ac5最终结果与包来源用例同步修正（本地dbe82de）

f452ac5两轮检查结果现已全部收齐，不与7eac047混读：CI `43561391720` failure，CodeQL `43561391559`四项success。Ubuntu整个job成功，Chromium703/0、14.7分钟；Windows54/1/1原有忽略的夹具碰撞保持失败。macOS的Check、CLI、具名真实挂载1项、release/Tauri/架构目标/锁/diff均成功，浏览器为702/1、25.2分钟，零重试；失败是`m7-package-candidates.spec.ts`首条“Duplicate repository versions...”在取消审阅后选择来源、等待opened超时。Linux同文件系统bind mount及macOS APFS的具名通过/1 passed和清理均已逐日志核对，证据`ubuntu-mount-proof.json`、`macos-mount-proof.json`，旧0项记录继续保留。

源码表明Dialog在真实closed事件后才清理审阅状态，并等待原目标updateComplete再异步归还焦点；旧测试仅点击Cancel就转向来源菜单，未等待这两项合同完成。新临时故障注入只延迟原目标就绪200ms，原测试在相同来源opened等待点0/1；补closed事件及原版本控件焦点等待后，同注入1/0。证明了异步回焦可打断菜单打开的竞态；没有取得Mac失败trace，不声称本机自然复现其实际调度。首次npm命令受Windows转义影响No tests found，记录为未执行；随后仓库已安装Playwright直接入口确实执行单条，日志分开保留。

本地`dbe82deb9829aa269710996de1c7cbb09c8515b9`仅新增5行测试同步/断言，临时注入全部移除，原来源/Plan业务断言、菜单opened/closed等待、703项总数、30秒超时及零重试保持；生产Material/Dialog/Core未改，不重开M7。最终未注入版本相关4/0（原失败用例及3项焦点归还合同），GUI tsc/Material/locale及diff通过。前后注入、最终回归、源码SHA和未执行首命令记录见`focus-before.log`、`focus-before-direct.log`、`focus-injected-fixed.log`、`focus-final.log`、`focus-gui-check.log`、`focus-fix-summary.json`；未重复本机完整浏览器、Rust或release。

7eac047不含这5行，不能把它的远端结果写成dbe82de验证通过。该新增本地修正未获额外推送授权；7eac047完整结果已全部收齐，新增修正与必要记录按准确候选另行确认；不手动rerun任何旧workflow，不借本缺口扩展页面功能或将M7签收推翻。

#### 7eac047最终技术结论与本地收尾

[CI 43564128872](https://github.com/ALCOMD/ALCOMD/actions/runs/43564128872)与[CodeQL 43564128896](https://github.com/ALCOMD/ALCOMD/actions/runs/43564128896)均completed/success，准确候选为`7eac0479bdb40282f1668cb1466185bd218279de`。本轮没有手动dispatch/rerun、取消旧任务、扩大测试超时或删除断言。原f452ac5全部结果独立收齐，其Mac702/1不会被本轮703/0改写。

| 7eac047本轮同SHA结果 | 实际证据 |
| --- | --- |
| Windows Server 2025 | 整个job成功，完整Check（含严格Clippy、Workspace/MCP及前端）、CLI权威daemon数1、release/Tauri/锁/diff、Chromium703/0（18.1m）。新增同刻度夹具隔离及原恢复预留用例均通过 |
| Ubuntu 22.04 | 整个job成功，完整Check/CLI/release/Tauri/GLIBC/锁/diff、Chromium703/0（15.9m）。真实同文件系统bind mount具名1通过/0失败/0忽略，清理成功；新旧项目发现回归通过 |
| macOS 15 arm64 | 整个job成功，完整Check/CLI/release/Tauri/架构与部署目标/锁/diff、Chromium703/0（25.1m）。真实APFS挂载具名1通过/0失败/0忽略，正常卸载清理；新旧项目发现回归通过 |
| CodeQL | Actions、Python、JavaScript/TypeScript、Rust四项全部成功；逐job完整日志、最终JSON与准确SHA一致 |

逐平台完整日志、两Unix挂载原始片段、`ci-final.json`、`codeql-final.json`及验证这些计数的`hosted-summary.json`位于`target/m8-hosted-7eac047-20261114/`。部分GitHub只读请求EOF仅重查相同run；限额核对非耗尽，后续以官方run/jobs API及逐job日志收齐。进程级HTTP/1.1仅用于gh读取，保留TLS/认证，未改生产HTTP合同或全局网络设置。

本轮已推送代码的远端门禁无剩余失败阻塞，获批回环预配Bearer/既有身份及六工具切片保持；不将其当作完整OAuth、管理扩展Host/WIT或M8完成。额外本地dbe82de的5行测试同步不含在7eac047，只有注入前后及最终4项定向证据和GUI check，不冒称完整703项或远端通过。该修正及本节收尾记录仅本地提交，工作树/提交链与metadata/xtask/diff按常规核对，不重复本地Rust/浏览器全套、release或制造纯文档CI；如需推送新增两笔仍须新的明确授权。

### 9fea7b6获准推送与浏览器同步远端验证（2026-11-14）

所有者明确批准后，以真实Windows用户在标准main核对干净工作树、origin/main基线7eac047及完整两笔`dbe82deb9829aa269710996de1c7cbb09c8515b9 → 9fea7b6e5afca24674230408041ae5e0789baa9f`，普通快进推送至9fea7b6，远端实读一致。差异仅一份浏览器测试新增5行及两份状态文档，生产源文件、Rust测试、manifest/锁文件及构建配置未变；不声称不同CI环境/产物字节相同。既有原行为/注入修复/移除注入后的4项定向及GUI check、metadata/xtask/diff证据按相同输入复用，没有重复本地全量、构建或已通过模块。

本次push自动启动[CI 43571868211](https://github.com/ALCOMD/ALCOMD/actions/runs/43571868211)及[CodeQL 43571868047](https://github.com/ALCOMD/ALCOMD/actions/runs/43571868047)，均绑定9fea7b6；现已全部收齐、均completed/success，CI三平台整个job成功，Chromium各703/0、零重试，Windows/Ubuntu/macOS分别15.3/16.9/22.9分钟，两Unix真实挂载各1/0、正常清理；CodeQL四项成功。仅收齐这两项既有运行，不手动dispatch/rerun或取消作业，不把7eac047全通过拼接成当前通过。推送审计、准确测试SHA与原运行时不变证据、当前/最终JSON及完整日志保存于`target/m8-hosted-9fea7b6-20261114/`。本轮不操作原Core/GUI、数据、真实项目或Unity，M8仍IN_PROGRESS，不新增完整OAuth或管理扩展/Host合同。结果收尾仅本地提交，不推送纯状态记录触发新一轮完整门禁。

#### 管理扩展后续只读复用审计（未实施、未批准新合同）

等待上述既有CI期间，按已批准M8顺序只读核对现有实现，不与HTTP同时新增管理功能。`extensions/first-party/alcomd-extension-mcp`仍只有README/manifest scaffold，无Component或签名；M7的MCP管理Portable UI快照仅为渲染夹具，不代表真实管理扩展。

| 已有能力 | 确认的最小缺口与后续审阅方向 |
| --- | --- |
| Core/application/Store/typed client已接通list/create/setGrants/issueCredential/revoke | Host WIT、私有消息白名单和runtime路由只有project-summary/self-data，没有MCP管理桥。后续仅补独立、显式协商的Host interface，按既有`mcp.clients.read/manage`映射；不得给扩展万能RPC或local-owner身份 |
| M7已有current lease、InvocationContextId、Client/Extension双Principal、render只读/action写/close无权限 | 新能力逐调用复查两侧权限、lease/grant/lifecycle/UI状态，实际Extension Principal保留为写入/幂等actor。现有MCP客户端是全局管理对象，不存在客户端owner列，不能凭空宣称只管理扩展自己创建的客户端 |
| Core列表最多3 MiB，单客户端最多4096条grant；WIT值256 KiB、Host帧512 KiB | 直接透传整份RPC DTO不成立。需审阅有界客户端摘要、revision绑定的授权分页及有界授权修改语义；大集合不能静默截断或借拆包伪称原子事务。推荐Core内单次revision/幂等接受的差量修改，是否增加公共方法须在最小合同中明确，当前不实施 |
| issueCredential已在dispatcher及application双重限定local-owner，凭据一次返回 | 当前CLI/GUI没有普通用户签发入口，仅typed client/测试可调用。后续需单独定义owner显式签发/轮换入口；不向Host/Portable UI暴露签发或秘密，不代持owner、不自动签发或重试未知结果 |
| Portable UI已有list/form/select/status/progress及M7签名安装/权限审阅 | 足以复用现有组件承载客户端及授权管理；不新增私有页面/HTML/特权渲染。当前client记录是登记/撤销/授权状态，没有实时连接/活跃请求查询，不把已签发或未撤销冒充在线；观测缺口另列最小合同 |

后续审批必须遵守`extensions/AGENTS.md`的新Host能力要求（threat model、public permission、Schema及人工审批）和冻结ABI old-guest/new-host回归；现有WIT/type/world/权限Schema、State/Config、运行时与GUI均未改。此处仅记录真实复用点与缺口，不是整份M8重新规划，也不作为新Host、凭据入口或观测合同已获批的依据。

#### 9fea7b6最终技术结果与后续边界

准确远端候选`9fea7b6e5afca24674230408041ae5e0789baa9f`的[CI 43571868211](https://github.com/ALCOMD/ALCOMD/actions/runs/43571868211)与[CodeQL 43571868047](https://github.com/ALCOMD/ALCOMD/actions/runs/43571868047)均completed/success，全部平台/语言结论已独立收齐。此候选确实包含dbe82de的5行关闭/焦点同步，不再沿用“仅本地、未获远端验证”的当前状态；旧轮702/1、夹具失败、0项空跑及预算取消仍保留为历史事实，不拼成通过。

| 9fea7b6同SHA结果 | 实际覆盖 |
| --- | --- |
| Windows Server 2025 | 整个job成功；完整Check（严格Clippy、Workspace/MCP、前端）、CLI权威daemon数量1、release/Tauri/锁/diff、Chromium703/0（15.3m），零重试 |
| Ubuntu 22.04 | 整个job成功；完整Check/CLI/release/Tauri/GLIBC/锁/diff、Chromium703/0（16.9m）；同文件系统bind mount具名1通过/0失败/0忽略、正常清理 |
| macOS 15 arm64 | 整个job成功；完整Check/CLI/release/Tauri/架构与部署目标/锁/diff、Chromium703/0（22.9m）；APFS挂载具名1通过/0失败/0忽略、正常卸载清理 |
| CodeQL | Actions、Python、JavaScript/TypeScript、Rust四项成功，均绑定9fea7b6；逐job完整日志已归档 |

本轮完整JSON、三个作业日志、两Unix挂载原始片段及`hosted-summary.json`保存在`target/m8-hosted-9fea7b6-20261114/`，并核对已提交浏览器测试文件的完整SHA-256与push-audit一致。生产/Rust源文件、manifest/锁和构建配置相对7eac047未变，不把“源码相同”扩大为不同CI环境或产物字节相同；本机200ms注入前0/1、修后1/0、移除注入后的4/0及GUI check证据仍按其准确测试输入保留，没有为推送重复本地全量。GitHub只读EOF仅重查固定run；未触发重复workflow、取消作业、放宽断言、延长单用例超时或操作真实Core/GUI/数据/项目/Unity。

本轮HTTP回环预配Bearer候选技术门禁已通过，无当前远端失败阻塞；不是完整OAuth、管理扩展或M8整体签收。管理扩展只完成上述只读复用审计，新的Host/WIT/Schema/凭据入口及观测合同仍须具体差异审阅，不因本轮全通过而自动获批。状态收尾只本地提交、运行适用metadata/xtask/diff，工作树与提交链核对后不再推送纯状态文档。

### 管理扩展最小合同批次（2026-11-14，已批准）

所有者于2026-11-14明确批准本节管理扩展合同及所列构建期依赖，授权直接实施、定向验证与稳定本地候选，不自动推送。复用现有Config13、`mcp.clients.*`、身份与六工具，不新增Core RPC或权限名称。随后另行批准下节State35和can-manage增量，修正最初“不改State”的假设。首片完成客户端登记、授权与撤销；完整OAuth、实时连接/请求/订阅观测、其他业务工具及M9以后仍不在本片。以下“审批后”要求从本次授权起执行，实际实现/验证结果另记，合同获批不等于功能验收通过。

#### 独立Host接口与有界数据

新增可协商接口`alcomd:mcp-management/clients@1.0.0`，新world `alcomd:mcp-management/management-v1@1.0.0`包含旧world并导入此接口。旧`alcomd:extension/extension-v1@1.0.0`、所有既有type及Portable UI协议保持原样；新包required声明该接口，旧Host明确拒绝，旧guest在新Host继续可用。新接口不是任意RPC透传，仅提供下表六个函数。

公共投影：`client-summary={client-id,display-name,revision,created-at-ms,revoked-at-ms?,grant-count}`；grant保持现有`permission/resource-kind/resource-id`三元组；项目摘要只返回`project-id/name`；页统一为`{items,next-cursor?}`，授权页另带revision。UUID、revision、时间、grant字段沿用现有强类型及界限，游标最多512字节ASCII，幂等键沿用1–128字节ASCII。不返回Principal、凭据或凭据摘要、数据库路径或完整项目路径。所有输入/输出在既有私有消息编码下各限64 KiB，保留全局WIT值256 KiB/Host帧512 KiB限额；页按数量及字节双重限制并返回游标，不能静默丢项。

| 新WIT函数（同步result） | 输入、输出及既有权限 |
| --- | --- |
| `list` | `{cursor?,limit?}`→客户端摘要页；默认32、最多64，稳定按client-id排序；`mcp.clients.read` |
| `grants` | `{client-id,expected-revision,cursor?,limit?}`→`{revision,items,next-cursor?}`；默认64、最多128，按三元组排序；`mcp.clients.read` |
| `projects` | `{cursor?,limit?}`→项目摘要页，默认32、最多64；只返回当前Client与Extension两侧均获`projects.read/project/<id>`授权的交集，不扩大资源owner或暴露其他项目 |
| `create` | `{display-name,idempotency-key}`→摘要；名称沿用1–128字符规则，必定零授权；`mcp.clients.manage` |
| `change-grants` | `{client-id,expected-revision,add,remove,idempotency-key}`→摘要；add/remove各最多128条，禁止空差量、重复或相互重叠；`mcp.clients.manage` |
| `revoke` | `{client-id,expected-revision,idempotency-key}`→摘要；沿用永久撤销，不删除记录、回滚任务或伪造可恢复开关；`mcp.clients.manage` |

游标是不透明的有界值，绑定查询种类、客户端（如适用）、当前集合/客户端及grant快照版本、调用双方与lease/包/授权版本；对应证据变化返回`revision-conflict`并要求重读，不混页，不建立持久查询会话。游标本身不是授权。项目读取同时绑定可见交集及当前项目摘要；读取与摘要投影来自Core权威状态，不缓存另一份管理数据库。Core内部新增投影用例，避免为客户端摘要读取整份4096条grants，更不能把最多3 MiB的现有RPC响应直接装进WIT。

`change-grants`由Core在一个接受事务内核对expected-revision、合并差量、验证最终4096条/64种权限上限并写入单份State34授权快照、增加一次revision。新增元组沿用现有组合/项目资格规则；新增项目范围还须处于上述双方可读交集。保留元组不扩大权能，删除已失去登记或可见性的项目元组仍允许，不能因项目已删除而无法清除旧授权。Core新增内部命令，不新增公共RPC；配套唯一State增量是下节追加批准的State35。幂等指纹绑定原始规范化差量及调用actor，使用固定独立方法命名空间，不能改为对重算的完整列表取指纹。同键同输入重放原非秘密摘要，同键不同输入拒绝，不把历史结果当作当前权限。

新接口使用自己的错误type：`invalid-argument/permission-denied/scope-denied/lease-expired/lease-revoked/revision-conflict/idempotency-conflict/resource-limit/cancelled/unavailable/write-outcome-unknown/internal-error`；只在internal-error附不透明diagnostic-id，不改旧WIT错误枚举。客户端不存在/不可用走invalid-argument；撤销后写入或版本变化走revision-conflict，均不返回秘密或原始Store错误。私有消息层明确区分业务失败与传输成功；接受后的响应丢失或无法证明未接受时是write-outcome-unknown，不报告取消成功、回滚或自动重试。

#### 权威、安全与普通界面

沿用Core签发的InvocationContext、lease与UI会话，不信任guest传入的身份。render只读、action才写、close无权限；本片不开放后台MCP管理调用。每调用同时检查交互Client及实际Extension Principal的对应权限、当前lease/包/grant/会话；写接受与撤销竞争的检查必须在同一Core/Store权威事务内线性化，不能只凭入口缓存。写入与幂等actor保留实际Extension Principal；不代持local-owner。MCP客户端是全局管理对象，`mcp.clients.manage`明确代表可修改全部客户端的高影响管理权，不虚构每扩展客户端owner或自动授予此权限。

审批后同步extension threat model、ABI协商规范、Manifest/权限映射、Host消息Schema/白名单及兼容快照，再实现。威胁重点为过大数据、伪造/过期上下文、授权撤销竞态、项目范围提升、重复写、秘密泄漏与第一方特权旁路。声明只表达申请；MCP扩展required `mcp.clients.read`，optional `mcp.clients.manage`、`projects.read`，无管理权时只读，无项目范围时不提供项目新增选择。既有沙箱不增加WASI、网络、进程、文件、环境、终端或凭据权限。

客户端列表→详情/授权编辑→明确保存，撤销时确认名称及影响。复用Portable UI的list/form/select/status与现有Material/错误出口，项目用名称+ID区分，权限显示申请内容与实际范围。大集合提供真正分页/增删，不用一次加载或全量覆写冒充完成。刷新/授权变化后重新读权威状态，旧结果不得覆盖新状态；UI断开/取消不撤销已接受写入，未知结果提示重新核对，不自动重试。普通页面只显示登记、授权、撤销，不能把未撤销或签发过凭据显示为“在线”。本片不新建状态面板、第二套GUI组件或任务系统。

#### local-owner正式凭据入口

新增CLI `alcomd-cli mcp client list [--cursor … --limit …]`与`alcomd-cli mcp client issue-credential <client-id> --expected-revision <n> --idempotency-key <key> [--secret-stdout]`；复用现有OS IPC、typed client与原RPC，无扩展签发函数。签发/轮换前展示目标及“旧凭据立即失效”，沿用交互确认/全局`--yes`。stdout非终端或使用JSON/NDJSON时，须显式`--secret-stdout`才发送RPC；不支持的dry-run/no-wait组合预先拒绝，避免在意料之外签发。终端只一次展示秘密，list/普通错误/日志/事件/UI均无秘密。输出失败或连接结果未知不得再次自动签发；先读取非秘密revision，明确说明可能已生效且明文不能恢复，只有用户显式新确认才用新revision/key轮换。不开文件、剪贴板、外部客户端配置或秘密持久存储旁路。

#### Component构建与依赖差异

本批审批时尚无真实MCP Component；现已采用独立guest crate和已锁定的`wit-bindgen = "=0.57.1"`，关闭default，仅`macros/realloc/std`，编译无WASI的guest，再复用现有`wit-component 0.254.2`作Component编码。官方生成器避免手写Canonical ABI和手工WAT业务程序；不安装功能更宽的wit-bindgen CLI，不重写运行时。生成绑定及realloc含Canonical ABI所需unsafe，只限生成层并说明来源；普通管理逻辑继续禁止手写unsafe，不能宣称没有unsafe影响。

临时隔离Cargo解析已核对：运行时wit-bindgen 0.57.1原已锁定；新增构建期包为wit-bindgen-core/rust/rust-macro各0.57.1、prettyplease 0.2.37、unicode-xid 0.2.6，以及wasm-encoder/wasm-metadata/wasmparser/wit-component/wit-parser各0.247.0。后五者是宏生成器要求的并行构建依赖，**不降级**现有Host的0.254.2。三项wit-bindgen及WASM/WIT包许可证`Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT`；prettyplease/unicode-xid为`MIT OR Apache-2.0`。已声明MSRV分别1.85、1.62、1.17；rust-macro未独立声明MSRV，不能冒称已验证，批准后以项目1.97.1编译确认。Bytecode Alliance上游生成器与Wasmtime版本各自冻结，维护成本为两代构建工具并存及生成ABI审计。

审批前解析的wit-bindgen feature集合确为macros/realloc/std，未启用其async/bitflags/macro-string或WASI；生成器传递wasmparser启用的simd是Wasm解析feature，不授予guest OS权能。当时的准确metadata、临时清单/锁位于`target/m8-management-contract-audit/`，当时生产manifest/锁未改；初次metadata仅因Cargo缓存写权限失败，获工具审批后读取成功，不是编译或功能验证。批准后的最终锁与feature tree现已再次对照，结果见下节；不得随意升级现有包。

#### 本片验收与明确未覆盖项

审批后定向覆盖：old-guest/new-host与不支持接口拒绝；无管理权/仅一侧权限/伪造过期lease；4096条授权完整分页、游标变化拒绝、项目交集与孤立授权删除；原子差量/revision竞争/撤销/幂等重放；接受后响应丢失和会话关闭；CLI非终端预拒绝、一次返回、输出失败不重签；真实Component安装信任/启停/管理UI及两客户端当前授权实际生效。复用已通过的身份/业务/HTTP证据，不重复开发或把旧结果当作新Host已通过。稳定候选再集中适用验证，不逐修改跑全量。

测试包须遵守现有签名、发布者确认和范围授权，只在获准隔离环境用开发测试密钥；不读取或更改真实签名材料，不直接往原环境安装。架构A019的默认安装/启用仍需后续实际分发证据，不能由占位manifest或一次开发包安装冒充完成。实时连接/请求/订阅观测仍缺公共查询，完整OAuth及其他M8剩余项分别后续收束；本片不是M8完成或推送授权。

本轮准备检查：metadata、`cargo xtask check`及diff检查通过，仅修改本计划与status；没有执行完整测试/发布构建、修改正式Schema/WIT/生产依赖或操作原运行环境。沿用9fea7b6的准确远端证据，不将其当作本节未实施接口已经通过；本节文档只本地提交。

#### 实施中的单项信息缺口（已追加批准）

真实guest无法从目前的客户端摘要获知可选`mcp.clients.manage`是否已授予。为实现“无管理权只读”，
已单独请求在新接口`client-page`增加`can-manage: bool`：由Core根据当前Client/Extension双方权限、lease和UI状态
计算，只控制控件展示，不是后续写入保证；写入仍在接受事务逐次复查。旧WIT/公共RPC/State/Config/权限名称不变。
所有者随后明确批准此只读字段；新WIT/Schema、Core投影和guest控件展示已同步，真实流程证据见下节本地候选记录。
该信息缺口不再等待审批，不重审已经批准的主体；旧编译或只读流程不能算作完整管理验收。

#### 真实Core发现的扩展授权表约束缺口（State35已追加批准）

首次真实隔离Core验证未通过：签名/发布者确认、安装、缺必需授权拒绝启用均已执行，随后
`extensions.setGrant(mcp.clients.read, Global, *)`返回store_unavailable，不能记为完整流程通过。
定位`0008_extension_runtime.sql`的`extension_grants`：permission_name CHECK仅允许background.run/projects.read，
resource_kind CHECK仅允许Extension/Project，resource_id最短3字节，组合CHECK也仅允许旧两种。
此前“不需要State变更”的复用审计漏查了这些数据库约束。不能通过复用background授权、self-data旁表、
跳过CHECK或改写已推送迁移来假装接通；当时此部分暂停，独立实现继续，现已按下述追加批准修复。

所有者随后明确批准一次State34→35增量，现已编写迁移及初始化接线：只在同一IMMEDIATE事务内重建extension_grants的CHECK，
增加已存在的mcp.clients.read/manage名称、Global种类及仅这两项允许的`Global/*`组合。resource_id下限改1，
仍由完整组合CHECK保证旧background仅Extension/自身ID、projects.read仅Project/36字节ID。保留全部旧列、
主键、外键、state/grant_revision/updated_at_ms和每条现有记录；无新表类别或新字段，无权限名称/Config变化。
步骤为create extension_grants_v35→逐列复制旧记录→drop旧表→rename新表→user_version35→commit；任一步失败全回滚。
0008及已推送0033/0034保持字节不变；原运行数据仍不迁移，State35仅用于获准的隔离候选。公共授权Schema/DTO
只同步这两个既有权限的Global/*映射，不开放其他全局扩展权限。本提案修正主体合同中的“不改State”一条，
并非重审身份、六工具、HTTP或管理六函数。

批准后已补真实旧授权/实例/数据/事件/序列保全、外键与CHECK拒绝、失败回滚及State35重启验证，并重做实际失败的
管理流程；准确结果见下节。前期Store原子差量/分页单元通过与CLI编译不覆盖此表的可用性；0/1真实失败日志为
`target/m8-mcp-management/host-flow-readonly.log`，历史结果保留。can-manage只读字段亦已获批；不再追加纯规划审批，实施与验证继续，原运行数据不升级。

### 管理扩展本地候选与实际验证（2026-11-14）

推送前代码候选`30e7f7d1c3448da8b07c77787bac58b40be4b33e`，在标准目录/main、真实Windows用户`cqmhv-dt\m2922`本地提交；本节记录当时尚未推送的本地证据，后续推送与远端结果见下一节。前序合同提交15e9b2b和远端记录039ea3c继续保留。9fea7b6的CI/CodeQL成功只证明旧HTTP候选，不覆盖此批新增代码。M8仍进行中，本片记为本地定向验证通过、远端及人工查看待验证。

已接通实际Core→application/Store接受事务→独立Host/WIT→普通沙箱Component→原Portable UI renderer；管理界面可以登记零授权客户端、分页查看、审阅添加/删除授权、处理revision冲突及永久撤销。可选管理权限由Core的can-manage提示控制显示，写入始终重新校验；项目选择只来自双方可读交集，孤立授权仍可删除。接受后的写结果丢失不回滚、不自动重试。CLI复用现有typed client和一次性签发RPC，扩展看不到凭据。共享Material权限弹窗只增加既有MCP全局权限映射，没有第二套GUI或身份系统。

实际Rust Component暴露了两个内部问题并已窄修：生成器需要guest表与Canonical ABI跳板表，Host现允许两表共享原10000元素总预算，64MiB/实例/燃料/时间限额不变；冷启动编译准备消耗了旧render时间戳，现于准备完成后签发原2秒render预算，仍受原会话idle/absolute期限限制。另修guest的同父节点连续order及select option合法ID，保留原公共验证器，未放宽协议。State35只重建扩展授权CHECK；0008/0033/0034字节未改，不升级原数据。

本地集中定向结果（每行独立执行，不代表完整Workspace）：

| 范围 | 实际结果与日志（均在`target/m8-mcp-management/`） |
| --- | --- |
| Store MCP写入/管理差量/4096完整分页 | 5/0，`final-store-m8.log`；含旧原生MCP写回归、原输入幂等重放、孤立授权、容量与游标变化 |
| State33–35关联迁移 | 12/0，`final-store-migrations.log`；其中State35三项保全全表旧行/事件序列/凭据/实例/数据、精确CHECK/FK/PK、持久回滚/重启 |
| 新合同及现有版本关联合同 | 2/0＋9/0，`final-contracts-2.log`；旧WIT六文件逐字节相同，新Schema界限及实际Component输入/二进制摘要 |
| Core私有消息身份/能力矩阵 | 4/0，`final-core-authority.log`；伪造/过期上下文拒绝，render只读、action六函数、background/close拒绝 |
| UI会话/范围/期限关联回归 | 13/0，`final-ui-authority.log`；含新冷启动预算与原绝对期限上限 |
| Host旧/新ABI及资源配额 | 4/0＋2/0，`final-host-abi.log`、`final-host-limits.log`；旧guest仍运行，新接口错误可读，两表共享配额及失败增长返还 |
| 扩展包/私有协议/接口协商 | 15/0，`final-extension-contracts.log`；含新版本/未知能力拒绝、65536字节精确界限，不开放凭据函数 |
| 实际guest | 单元3/0＋生成绑定doc-test1/0，`final-guest-tests.log`；只读、未知写结果保留并禁写、撤销态；guest严格Clippy通过 |
| 真实Core/Host/Component | 具名1/0，`final-real-management.log`；正常签名与发布者确认、缺必需授权拒绝启用、只读/启停/重开/卸载、实际创建/项目授撤权/并发冲突/永久撤销、已连接MCP身份立即受影响、范围外拒绝、接受后丢UI回复/关连接不重复写；任务均终态，项目只在可丢弃目录内 |
| CLI真实签发与输出故障 | 具名1/0＋预检查1/0，`final-real-cli.log`、`final-cli-preflight.log`；无opt-in不发写请求，真实凭据可认证，过期revision不轮换，只读stdout故障后仅一次轮换、原哨兵文件保持，旧凭据失效 |
| 关联Chromium | 首轮15通过/1失败，`final-browser.log`；失败是browserContext.newPage的Target crashed，尚未页面操作。原trace保留为`browser-page-crash-trace.zip`；该项单独复验1/0，`browser-crash-directed.log`，零自动重试。不记成单轮16/0；新管理授权用例首轮已通过 |
| 静态/构建 | 受影响7个Rust包all-targets严格Clippy、新合同两test目标严格Clippy、guest Clippy、root/guest fmt、metadata、xtask、前端check/build、PowerShell脚本语法及diff通过；未运行完整Rust、703项浏览器、Tauri release或三平台CI |

真实流程已证明扩展侧授权/项目范围与已连接MCP身份权限实时生效。交互Client双侧门禁复用Core-issued AccessContext、现有UI范围与调用身份回归；本片没有为获得受限交互Client而增加测试专用认证入口。UI未知结果的保留/禁写有guest单元证据；实际故障注入位于UI RPC成功回复边界，不冒称已注入SQLite底层commit未知故障。原生Tauri人工点击、跨平台执行、全部4096条授权在真实窗口逐页查看未执行；Store分页证据不能代替人工体验签收。

历史失败不覆盖：初次State34真实流程0/1；旧单表配额Host ABI3/1、修表后新guest节点order验证0/1；冷启动预算0/1、自动脚本连续动作触及原限速0/1；修正实际原因后最终真实流程才取得上述1/0。脚本按原1次/秒节奏等待，不延长或取消产品限速。CLI首次闭管道用例0/1当时被归因为Windows注入不稳定，这一判断不准确：后续远端诊断已确认普通BrokenPipe成功退出策略会掩盖凭据输出错误，见下节。改用只读句柄后Windows用例1/0仅证明该路径，不能代表断管正确；浏览器新用例初次定位器歧义0/1、修正定位与生命周期等待后1/0。初次Store命令误编译无关集成目标遇到页面文件1455而未完成，随后只跑`--lib`定向；新摘要测试曾因sha2摘要类型不能直接LowerHex编译失败，改为逐字节编码后两合同模块通过。Clippy首轮分支/借用及新增测试模块位置失败均保留，修正后各适用严格检查通过。原始日志均未删除，未删断言、降权限或拼接失败轮次。

依赖最终核对：根Cargo.lock包名/版本集合完全保持，仅xtask增加已锁定wit-component直接边；独立guest恰新增批准的10项构建包，MSRV/许可证/生成unsafe边界沿用上节披露。`final-dependency-audit.json`、`final-guest-feature-tree.txt`、`final-guest-metadata.json`与`final-guest-feature-audit.json`证实wit-bindgen仅macros/realloc/std，无WASI/async/额外feature。生成器与Host两代构建工具并存，不宣称生成绑定没有unsafe。

输入与产物：`candidate-inputs.json`记录1040份本机文件摘要及Git blob，排除的仅为本计划和status两份持续更新记录，清单SHA256 `a65ab5886c636ce409f1777d219579490cf8d21611006d2c37371da034821da1`。各日志保留自己的执行范围；真实流程之后仅补/移动cfg(test)边界测试、格式化新合同测试及文档，运行时代码/实际Component不再变化，不由清单宣称所有命令读取过全部文件。已提交Component的SHA256 `ce56c941109c0a034ff0191baedd03c32b814aa1e40915222c6da00004350b7e`，旁置provenance绑定guest清单/锁、源码、新旧WIT及工具链，合同测试核对。浏览器截图`permissions-global-scope.png`仅为harness权限弹窗截图，不是真实原生桌面。

可安装开发包真实存在：`C:\Users\M2922\MyProjects\Rust\ALCOMD\target\m8-mcp-management\management-development-state35.alcomdextension`（125919字节），由通过的实际流程按新建文件规则导出。它是AGPL-3.0-only测试身份`dev.alcomd.fixture`，使用既有开发测试密钥并经过正常验签/明确发布者批准，不是正式发布签名或默认分发包。只能在匹配本候选的隔离Core/Host/GUI使用：Extensions→安装扩展→选择此文件→明确批准测试发布者→创建/审阅/应用安装Plan→权限中授予必需MCP读取后启用；写入另需显式管理权限，项目选择另授指定隔离项目读取。勿装入原运行数据。本轮未打开或替换原生验收窗口，也未恢复桌面控制。

运行前后只读检查均未发现原Core/GUI在运行；本轮未暂停、替换、升级或启动原程序，没有“守候恢复”进程。本轮自建隔离Core/Host已正常结束并清理，不停止未知进程或Unity，不读写真实项目。后续仍欠完整OAuth、实时连接/请求/订阅查询合同、默认安装/启用的正式分发证据；业务工具安装/搜索/重装来源委托未实施，本片不进入M9或宣布M8完成。普通文案/布局简化继续作为非阻塞体验事项；guest首片使用简体中文，其他语言未验证。

### 管理候选推送与Hosted验证（2026-11-14）

所有者明确批准当前候选推送及收齐自动运行。核对真实Windows用户、唯一标准目录/main、干净工作树、origin=`https://github.com/ALCOMD/ALCOMD.git`，fetch确认远端仍为9fea7b6、是HEAD祖先。完整链为039ea3cf（旧远端记录/复用审计）、15e9b2b1（获批合同）、30e7f7d1（实现）、dcc76ffe（本地证据/剩余项），无遗漏或无关修改；普通`git push origin main:main`成功，远端核对为`dcc76ffe22005a17ad787c76fe8412d903e20ea0`。未强推、打标签、发布或修改设置/Secrets/真实签名材料。

只跟踪本次自动生成的[CI 43601727656](https://github.com/ALCOMD/ALCOMD/actions/runs/43601727656)与[CodeQL 43601727390](https://github.com/ALCOMD/ALCOMD/actions/runs/43601727390)，均核对headSha=dcc76ffe。没有dispatch/rerun或以旧结果代替。本地既有验证输入未变，不为这次推送重复全量本地验证。

| 同SHA远端范围 | 实际结果 |
| --- | --- |
| Ubuntu 22.04 | Check失败；CLI集成模块1通过/1失败，在mcp_cli_issues_once_and_broken_stdout_never_reissues的`!failed.status.success()`断言；后续release/Tauri/浏览器未执行 |
| macOS 15 arm64 | Check失败；同一CLI模块1通过/1失败、同一断言；后续release/Tauri/浏览器未执行 |
| Windows Server 2025 | 整个job成功：完整Check（含Workspace、独立guest与前端门禁）/CLI自启动/release/Tauri no-bundle/锁/diff，Chromium单轮704/0（17.7m）、零重试 |
| CodeQL | 四语言全部success，完整JSON及四job完整日志已归档 |

最终CI结论为failure，CodeQL结论为success，所有作业均已终态；两Unix的Workspace在CLI处中止，后续Rust包及release/Tauri/浏览器均不能记为已执行。准确证据存放`target/m8-hosted-dcc76ff-20261114/`：完整推送链/范围、ci-final.json、codeql-final.json、三个平台完整job日志及codeql-full.log，hosted-summary.json保留各step/计数与证据SHA256。`gh run view --job --log`因整轮未结束拒绝提供日志后，只改用GitHub官方只读已完成job日志接口，没有重跑或取消任何作业。监视中一次GitHub API EOF仅重查同一个固定run；最终API另核对两个run_attempt均为1。此轮CI失败保留，CodeQL成功不能替代功能门禁。

#### CLI输出失败的实际原因与本地窄修

原只读句柄、哨兵文件保持、准确一次轮换、旧凭据失效、无secret错误输出等全部断言保留，没有删除失败测试或放宽成功条件。冻结Rust1.97.1的[官方stdio源码](https://github.com/rust-lang/rust/blob/1.97.1/library/std/src/io/stdio.rs)已通过只读API核对并归档：StdoutRaw把EBADF转成成功，Unix只读stdout正触发此行为；此前只在Windows验证通过，不能由此推断Unix故障交付已正确。另有生产错误：凭据复用普通CLI的BrokenPipe→成功退出策略；并且输出未显式flush，可能漏报延迟错误。

本地修复`db5b60a7cf6af323718e118dd4d92de5e2b07c63`只涉及CLI源码及该真实集成测试。非控制台凭据输出安全复制已有stdout的OS句柄/描述符，以标准File写入并明确flush，避免StdoutRaw吞错；没有新建目标文件、存储秘密、手写unsafe或新增依赖。Windows真实控制台保留标准库Unicode输出。共用原格式化逻辑，Human/JSON/NDJSON输出形状不变；普通CLI保持原BrokenPipe约定，只有一次性凭据在输出错误时明确失败并提示“已签发、明文不能恢复、没有自动重签”。Core调用/幂等/身份/公共合同未变。

先在本机复现两个新回归：`cli-output-before.log`为1通过/2失败，证实凭据断管退出码及延迟flush错误；修复后`cli-unit-after.log`单轮8/0。原真实CLI模块扩展一个闭管道阶段，保留原全部断言，`cli-real-after.log`单轮2/0：正常签发后revision2，只读输出故障仅一次轮换到3，闭管道故障仅一次轮换到4，均报告输出失败且不重签，零授权保持。只使用原端点空闲时的可丢弃Core，不暂停原环境或操作真实项目；本轮实例正常结束。

CLI all-targets严格Clippy、fmt、metadata、xtask及diff通过；`cli-fix-inputs.json`记录两个准确文件摘要及修复SHA。`cli-fix-reuse-audit.json`核对旧1040份输入中1038份未变，仅CLI源码及其集成测试变化；只在原日志实际范围内复用证据，不扩大为新候选完整通过。没有重复完整Rust、Chromium、Tauri release或CI。本地修复的Unix条件分支尚无执行证据；dcc76ffe的远端失败/CodeQL成功都不覆盖db5b60a。本次推送授权只覆盖原候选；本轮远端结果现已收齐，修复及收尾记录的额外推送仍须单独确认，不自动推送。M8仍进行中，不推进新合同或功能范围。

### CLI修复重推与管理候选验证（2026-11-14）

以上为补充推送获批前的历史记录。所有者随后明确批准db5b60a与记录5207287两笔；真实Windows用户/main核对工作树干净、fetch后origin/main仍dcc76ffe，严格核对完整待推送链恰为这两笔及4个文件，无无关修改，普通快进推送成功，远端为`5207287572d0cfc812974e7f75e7386d5d70e353`。没有重写历史、打标签、发布或手动触发workflow，也没有为推送重复已有本地全量。

只跟踪本次自动[CI 43609783925](https://github.com/ALCOMD/ALCOMD/actions/runs/43609783925)及[CodeQL 43609783793](https://github.com/ALCOMD/ALCOMD/actions/runs/43609783793)，两者headSha均为5207287、run_attempt均为1；本轮于2026-11-14推送、2026-11-15收齐全部作业，CI整体failure、CodeQL success。新证据目录`target/m8-hosted-5207287-20261114/`与dcc76ffe历史目录分开。

| 同SHA远端范围 | 实际结果 |
| --- | --- |
| Ubuntu 22.04 | Workspace全部成功，原CLI真实模块2/0、新管理Component真实流程1/0；随后独立guest cargo test的原生cdylib链接失败，ELF导出脚本不能接受`alcomd:extension/...`中的冒号；后续release/Tauri/浏览器未执行 |
| macOS 15 arm64 | Workspace全部成功，原CLI真实模块2/0、新管理Component真实流程1/0；随后独立guest原生dylib导出列表将Wasm符号解释为未定义的Mach-O符号，链接失败；后续release/Tauri/浏览器未执行 |
| Windows Server 2025 | 整个job成功：完整Check/CLI自启动/release/Tauri no-bundle/锁/diff，Chromium单轮704/0（12.3m）、零重试 |
| CodeQL | 四语言全部success，codeql-final.json及完整四job日志已归档 |

#### guest原生导出边界的窄修

两个Unix的实际失败点均已逐job日志核对，分别是ELF版本脚本语法错误和Mach-O未定义符号，没有把后者直接套成前者的同一错误消息。原CLI缺陷现已取得这两Unix真实通过证据，Workspace内的Core/Store/Host管理闭环也已执行通过；当前阻塞是独立guest测试构建误导出了Wasm Canonical ABI。Windows允许这些符号不等于原生共享库是支持的扩展运行形式；扩展仍只运行Wasm，不发布DLL/so/dylib。全部门禁保持，不删除或跳过原3项单元与1项绑定文档测试，不改cdylib/rlib构建合同、依赖、旧WIT、RPC、Schema或权限。

本地窄修`4eb41313469ad826ca86730938a90324846e6c8b`：在guest入口只为wasm32调用export宏与导入Management实现，业务模块在wasm32或cfg(test)编译，生成绑定继续可用于原生单元及doc-test；同步provenance的入口源码摘要。只有这两个文件、5行目标条件/说明及1个摘要变化。

实际定向结果：guest原生默认`cargo test --locked`单元3/0＋doc-test1/0，严格all-targets Clippy/fmt通过；Wasm release重建及既有encoder验证通过，新Component与原已提交产物逐字节相同，SHA256仍为`ce56c941109c0a034ff0191baedd03c32b814aa1e40915222c6da00004350b7e`。因此复用这个未变运行时产物的真实Core/Host/桌面相关证据，不再次运行这些流程。新provenance/旧WIT/Schema合同2/0，metadata/xtask/diff通过，锁文件与包版本未改。guest-native-after.log、guest-clippy-after.log、guest-wasm-after.log、component-after.log、guest-contract-after.log、target-gating-proof.json及guest-fix-inputs.json记录准确输入和范围。

ELF/Mach-O修复尚无实际执行通过证据，不能把Windows原生编译或Wasm字节相同写成Unix链接已通过，也不能把5207287的CodeQL成功当作4eb4131的远端验证。修复只本地提交，额外推送仍须明确授权。本轮作业现已全部收齐；ci-final.json/codeql-final.json、三平台完整job日志、四语言完整日志、hosted-summary.json及摘要已归档，少量GitHub EOF只重查固定run，没有重跑或取消作业。原运行Core/GUI/数据、真实项目和Unity未操作，桌面控制未恢复，M8仍进行中。

### 管理扩展候选同SHA远端技术收束（2026-11-15）

上节为再次推送获批前的历史证据。所有者随后明确批准4eb4131及记录78920f3：真实Windows用户/标准目录/main核对干净工作树、fetch后origin/main仍5207287、完整待推送链恰为这两笔和4个文件；普通快进成功，Git及官方只读ref均核对远端为`78920f382d1a0c58dece4a51fb2f54d84f48fcf2`。没有夹带其他提交、重写历史、tag/release或改动GitHub设置/Secrets/真实签名材料。

本次自动[CI 43616497449](https://github.com/ALCOMD/ALCOMD/actions/runs/43616497449)与[CodeQL 43616496767](https://github.com/ALCOMD/ALCOMD/actions/runs/43616496767)均headSha=78920f3、run_attempt=1，全部作业已成功终态；只有push自动运行，没有dispatch/rerun。

| 同SHA范围 | 实际最终结果 |
| --- | --- |
| Windows Server 2025 | 整个job success；完整Check、CLI自启动、release bins、Tauri no-bundle、锁/diff；Chromium单轮704/0（18.4m），零重试 |
| Ubuntu 22.04 | 整个job success；完整Check、CLI自启动、真实bind mount具名1/0/0忽略、release/Tauri、GLIBC/锁/diff；Chromium单轮704/0（15.0m），零重试 |
| macOS 15 arm64 | 整个job success；完整Check、CLI自启动、真实APFS挂载具名1/0/0忽略并正常弹出、release/Tauri、架构/部署目标/锁/diff；Chromium单轮704/0（22.1m），零重试 |
| CodeQL | Python、Actions、Rust、JavaScript/TypeScript四job全部success |

三个完整Check均包含Workspace及独立guest。逐平台日志另核对真实CLI输出故障用例、实际Core/Host/Component管理闭环成功；guest原生3项业务单元与1项生成绑定doc-test全部保留并通过。原CLI输出错误及ELF/Mach-O原生链接两处阻塞至此均获同SHA实际修复证据，当前切片没有未解决的远端技术阻塞。此前dcc76ffe的CLI失败、5207287的guest链接失败、本地15/1与单项1/0等原始结果全部保留；本轮各平台704/0来自各自一次完整运行，不拼接局部通过。

证据独立归档`target/m8-hosted-78920f3-20261115/`：完整两笔推送链/范围、首次运行证据、ci-final.json/codeql-final.json、三个job完整日志及四语言完整日志、两个Unix真实挂载原始片段、hosted-summary.json及逐文件SHA256。Wasm Component仍为逐字节相同的`ce56c941109c0a034ff0191baedd03c32b814aa1e40915222c6da00004350b7e`，CLI与guest窄修本机证据仍各按准确提交/输入保存。本次同SHA完整结果来自Hosted，本机状态收尾仅metadata/diff检查，没有为纯文档重跑Rust/浏览器全套、release/Tauri或真实桌面流程；最终记录仅本地提交，不追加推送制造重复验证。

本片技术门禁通过，M8仍IN_PROGRESS。完整OAuth、实时连接/请求/订阅观测、正式默认安装/启用分发、安装/搜索/重装来源委托及原生人工查看仍按既有计划保留，不能由此宣布M8整体完成。原运行程序/数据、实际项目及Unity保持，桌面控制未恢复；普通文案/布局/语言简化等非阻塞体验待办继续保留。

### 完整OAuth接入审计与集中待审合同（2026-11-15）

**状态：2026-11-15所有者已集中批准推荐方案，进入实施。** 批准涵盖本节HTTPS/预登记OAuth、9个RPC、两个hello分支、State36两表和两条锁定TLS依赖边，不再重复审批同一范围。基线为远端78920f382d1a0c58dece4a51fb2f54d84f48fcf2及本地审计478af1b；既有同SHA CI/CodeQL通过仍有效，但不覆盖OAuth新增代码。以下审计保留当时事实，实际实现与验证另记；不重做身份、六工具、STDIO、HTTP或管理扩展，不推进M9，不推送。

#### 实际复用与必要缺口

| 位置 | 已有能力 | 真实缺口 |
| --- | --- | --- |
| `apps/alcomd-mcp/src/main.rs` / `http.rs` | STDIO受控启动凭据；serve回环HTTP；每请求独立typed Core连接、严格Host/Origin、限额、无session | 只有`/mcp`，没有PRM/AS发现、HTTPS、授权码或token端点；401仅Bearer realm |
| `rmcp = 3.10.0`的本地源码/feature | 已冻结SDK承担HTTP/JSON-RPC/响应和取消；auth模块提供OAuth**客户端**流程 | 开启auth不会得到授权服务器；本方案不启用其客户端auth feature或另造MCP栈 |
| `alcomd-client`、daemon hello/dispatcher、application、Store | `mcp-client`凭据绑定真实`mcp:<ClientId>`；每RPC及写接受复查当前授权；Core生成秘密并仅存摘要 | 现有64字符凭据没有issuer/resource/expiry/授权上限，不能直接改名为OAuth access token；当前hello不支持受限OAuth端点角色；Store委托项目查询只join当前客户端grants，尚无token子集限制 |
| 管理扩展/CLI | 创建零授权客户端、明确修改现有权限及项目范围、永久撤销；owner CLI一次签发旧凭据 | 没有OAuth客户端登记、授权请求审阅或token生命周期；扩展不得读取秘密，现有WIT不扩展本批OAuth流程 |

冻结依据是上游提交`4df2d6b6e3588efb46e7542d98498e5c630a0a86`的2026-07-28文档；authorization、discovery、security、transport正文及摘要复用`target/m8-http-audit-20261113/frozen-doc-inputs.json`，本轮另通过GitHub contents API核对同提交client-registration正文。不能套用旧initialize/session或把DCR写成必需。关键规范：

- [Authorization](https://github.com/modelcontextprotocol/modelcontextprotocol/blob/4df2d6b6e3588efb46e7542d98498e5c630a0a86/docs/specification/2026-07-28/basic/authorization/index.mdx)：PRM、AS发现、resource、token audience、错误与scope challenge；AS可以与资源服务同机。
- [Security](https://github.com/modelcontextprotocol/modelcontextprotocol/blob/4df2d6b6e3588efb46e7542d98498e5c630a0a86/docs/specification/2026-07-28/basic/authorization/security-considerations.mdx)：所有AS端点必须HTTPS，PKCE、精确redirect、public client刷新轮换；不能将callback的localhost例外用于AS自身。
- [Client registration](https://github.com/modelcontextprotocol/modelcontextprotocol/blob/4df2d6b6e3588efb46e7542d98498e5c630a0a86/docs/specification/2026-07-28/basic/authorization/client-registration.mdx)：预登记是允许的路径；CIMD为SHOULD，DCR为MAY且已弃用。

#### 推荐部署与协议边界

推荐**本机HTTPS、预登记客户端、Core权威OAuth状态**。不强迫用户部署外部身份平台，也不把本机浏览器请求当owner。HTTP适配器只承载标准端点，复用现有typed client把固定OAuth用例交给Core。替代方案是另行接入外部AS，须增加issuer信任、外部subject绑定和运维前提；当前无现成AS，因此不建议将外部服务设为产品必需。直接给旧Bearer补metadata不满足audience/expiry与授权流程，不采纳。

新增显式`alcomd-mcp serve-oauth --port <1..65535> --tls-cert <DER文件> --tls-key <PKCS8-DER文件>`；仅绑定127.0.0.1，issuer为`https://localhost:<port>`、resource为其`/mcp`。端口必须非零且稳定，issuer/resource精确绑定；更换端口需重新登记/授权，旧token不能自动迁移audience。证书必须适配localhost，并由用户预先配置客户端/浏览器信任；**不创建CA、不修改OS信任库、不关闭证书验证、不读取正式签名材料**。本批可以用独立测试证书及只对该隔离客户端设置的信任完成自动验证，不能据此声称普通浏览器已无提示接入。没有可用受信任证书时，OAuth启动应明确失败或保持未配置，不降级HTTP。TLS私钥只在端点进程内读入，不进入Core数据库、GUI或日志。

既有`serve`回环预配Bearer和STDIO合同保持；OAuth端点只接受OAuth access token，旧Core凭据和refresh token不能在`/mcp`替代access token。认证失败不试另一模式或owner。保留配置/运行期区分是既有已批准入口的明确保留，不引入旧协议兼容层。Config13、六工具、Portable UI及现有权限名称不变。

| HTTPS端点 | 最小行为 |
| --- | --- |
| GET `/.well-known/oauth-protected-resource/mcp` | resource、唯一authorization_servers、Bearer header方式；401 challenge指向此地址 |
| GET `/.well-known/oauth-authorization-server` | 精确issuer、authorization/token/revocation端点，code、authorization_code/refresh_token、S256、none/client_secret_basic、iss支持；不广告OIDC、CIMD或DCR |
| GET `/authorize` | 仅预登记client_id、精确redirect_uri、response_type=code、S256 challenge、resource及有界scope/state；Core产生待审请求；浏览器只显示等待及请求标识，不授予权限 |
| GET `/authorize/result` | 仅在浏览器持有该流程的随机秘密证明且owner已批准后，返回一次性code、原state及iss到已验证callback；未批准等待，拒绝/到期明确结束 |
| POST `/token` | form编码的authorization_code或refresh_token；验证client、PKCE/secret、精确redirect/resource，消费旧凭据与保存新摘要同事务；Bearer access token、expires_in、scope，按登记选择签发refresh token |
| POST `/revoke` | 沿RFC7009验证对应client并撤销本token family；未知token也返回无泄漏的成功，不能借token线索撤销其他client |
| POST `/mcp` | 复用现有SDK/六工具、预算和取消边界；每请求绑定OAuth token对应的既有Principal；401 invalid_token、403 insufficient_scope附PRM，普通Core业务错误仍沿原工具合同 |

AS元数据`scopes_supported`列现有可委托权限；PRM只给最小`projects.read`，`offline_access`不作为资源要求。scope使用已有`projects.read/packages.read/repositories.read/packages.manage/operations.read/operations.cancel/state.check/events.read`，不新增Core权限；项目ID通过本机授权审阅选择，不塞进scope字符串。没有Core层次关系的scope互不暗含，列出的权限不会自动互相授予。scope不足在接受业务前给本次操作完整的最小scope集合；token scope足够而具体项目不在当前范围内仍为原业务permission_denied，不能靠重复OAuth获得越权。

必要内部修正包括将Core已验证的token授权上限传入现有委托读取/写接受：`crates/alcomd-store/src/m8_mcp.rs::projects/project`目前只有当前grant join；OAuth项目子集必须在查询分页前与之求交，单项目及Plan/Apply接受同样核对，不能仅在GUI/适配器过滤一页。游标绑定token上限与当前授权快照，防止混页。普通旧凭据继续以当前Core完整授权为上限，不改变资源owner、任务/计划/幂等Principal或原有安全断言。

预登记同时支持public `none`和confidential `client_secret_basic`，都强制S256；不支持implicit、password、client_credentials、私有JWT、token exchange。每个OAuth登记精确映射一个现有Core客户端，一个Core客户端至多一个有效登记；不同客户端不合并Principal。最多16个精确callback（HTTPS，或localhost/127.0.0.1/[::1]回环HTTP；拒绝userinfo/fragment、通配和任意端口匹配）。预登记不自动创建Core授权，默认仍零授权。CIMD暂不实现的SHOULD偏离理由：首批仅预登记、不新增远程metadata抓取及SSRF面；DCR不实施。能配置预登记client_id的客户端可接入；只支持自动注册的客户端不能宣称兼容，通用客户端自动接入仍列后续。

#### 已集中批准的Core差异

新增capability **`mcp.oauth.v1`**，仅在真实接线和隔离测试完成后发布；不是现有mcp.clients.v1的重新审批。下表为固定typed用例，不提供通用RPC代理。

| RPC / hello | 输入与输出要点 | 调用边界 |
| --- | --- | --- |
| `system.hello`新增`{kind:"mcp-oauth-endpoint"}` | 不携带业务凭据；绑定固定、零业务权限的OAuth端点角色 | 只准下述begin/finish/token/revoke；不能调用普通业务、clients管理或授权决定；不推断OS owner，不复用无认证hello |
| `mcp.oauth.begin` | client_id、issuer/resource、精确callback、S256、scope、state；返回requestId及一次性的浏览器流程秘密 | 端点角色；只创建有界待审请求，不改Core grants；不能自行批准 |
| `mcp.oauth.finish` | requestId、浏览器秘密；返回pending/denied/expired或一次性code及已冻结callback/state/iss | 端点角色；只有持有证明的原流程能领取code；owner get/decide不返回code或浏览器秘密 |
| `mcp.oauth.token` | tagged authorization_code/refresh_token的标准字段、client认证；一次返回access/refresh与TTL/scope | 端点角色；Core生成/验证/轮换，适配器不签发或持久化秘密；结果丢失不重放秘密、不自动重签 |
| `mcp.oauth.revoke` | token、client认证、可选token_type_hint | 端点角色；幂等无秘密响应，撤销family不影响已接受Operation |
| `system.hello`新增`{kind:"mcp-oauth-access",credential,resource}` | 验证Core发行的access token、精确audience、到期、登记/family及旧凭据代际；绑定原`mcp:<ClientId>` | 每RPC及写接受继续重验；refresh/code/旧Core credential不能用此kind；错误关闭连接，不回退 |
| `mcp.oauth.registrations.list/set/revoke` | list分页；set绑定现有Core client、issuer/resource、callback、public/confidential及是否refresh，带expectedRevision/key；revoke保留审计、失效相关token | 既有mcp.clients.read/manage且额外local-owner；不向扩展或外部MCP开放；confidential secret仅set明确新建/轮换结果一次返回，未知结果不自动再签 |
| `mcp.oauth.requests.get/decide` | get按requestId分页审阅，含身份、callback、scope/实际项目、当前revision及approvalFingerprint；decide显式批准/拒绝、选择授权上限、expectedFingerprint/key | 既有mcp.clients.read/manage且额外local-owner；批准前重新核对当前Core grants，变化时revision_conflict并重审；不能扩大既有Core授权 |

owner交互先复用正式CLI：`mcp oauth registration list/set/revoke`、`mcp oauth request show/approve/deny`。浏览器展示请求ID；用户在本机CLI查看完整身份、callback、权限/项目后确认，再回到原浏览器继续。无需GUI/管理扩展运行，不增加MCP协议session、设备码授权或自动开桌面。对普通界面不堆JSON/revision；CLI技术标志仍显式。owner控制与HTTP请求分离：本批不为浏览器设置owner登录cookie，不新增宿主特权或WIT旁路。

必要新增公开错误：RPC `mcp_oauth_invalid_grant`、`mcp_oauth_expired`、`mcp_oauth_response_unavailable`，其他复用invalid_request/permission_denied/revision_conflict/idempotency_conflict等稳定错误。HTTP按OAuth标准invalid_request/invalid_client/invalid_grant/unsupported_grant_type/invalid_scope/invalid_target/access_denied/temporarily_unavailable，未确认client/callback前绝不redirect；出错响应不反射原请求或秘密。原core_write_outcome_unknown仍只处理业务写响应未知。

#### State36、生命周期和依赖（2026-11-15已批准）

仅新增0036增量、单事务迁移，不改0033–0035及其他历史迁移，不升级原运行数据。新增两张表：

- `mcp_oauth_registrations`：oauth client_id、现有mcp_client FK、精确issuer/resource、callback集合、client类型、可选client secret摘要、refresh选项、revision、创建/撤销时间；单客户端有效登记唯一，列表不返回secret或摘要。
- `mcp_oauth_tokens`：256位随机token的SHA256摘要/种类、登记FK、family/parent、签发时旧Core凭据代际标记、冻结的scope及grant上限、issuer/resource、签发/到期/消费/撤销时间。复用已有不可变Core grant快照并保存明确选择的有界子集；鉴权取**token上限∩当前Core授权**，scope/项目后续新增不自动扩大旧token。不是第二套资源owner或授权权威。

待审请求和code仅由Core持有有界易失记录，重启作废；授权码只存摘要且一次兑换，原浏览器领取后秘密不再重放。access token 10分钟；选用refresh时7天闲置、30天绝对到期，每次消费必须轮换，旧refresh重用撤销整个family；业务Operation不随失效取消。原mcp.clients.revoke失效该客户端全部OAuth能力；原issueCredential轮换以现有credential id代际变化使旧OAuth token失效，不改变STDIO轮换规则；OAuth family刷新不轮换无关STDIO凭据。token/refresh交换跨响应丢失只能重新授权，不能重新执行已接受业务写入。State重开仍保留撤销/消费证据。

全局32待审/每登记4，待审10分钟、code60秒；现有64客户端/64权限/4096范围上限保持。每客户端至多16有效family、64有效access token；拒绝超额，不用无界缓存。OAuth表只清理确定过期且超出重放检测窗口的本批记录；不能删旧业务/幂等记录，refresh消费证据至少保留到family绝对期限。临时请求、code、cookie、client secret、token都不进日志、普通列表/事件/Portable UI、通用持久幂等响应。秘密响应/失败均no-store、no-referrer；授权页面无第三方资源，CSP/form-action/frame-ancestors收紧，cookie Secure/HttpOnly/SameSite，其他网页不能靠Origin获取授权。

Host精确`localhost:<port>`且仅127.0.0.1绑定，拒绝转发头/代理authority及DNS别名；不监听非回环。OAuth浏览器导航允许无Origin，存在Origin时仅允许精确同源，`/mcp`继续拒绝Origin；不开放跨域浏览器JS客户端/CORS。authorize/callback参数有重复值、控制字符、歧义或超限即拒绝，不作字符串拼接redirect。请求头16KiB/32项，AS query/form各16KiB，metadata响应64KiB，最多32TCP/16并发；TLS握手10秒；其他头/body/Core/输出deadline复用旧HTTP，公共授权入口另每登记每分钟10次、全局每分钟60次，超限429，无无界队列。

必要生产依赖仅将锁定的 **rustls =0.23.46**、**tokio-rustls =0.26.4**加入alcomd-mcp直接依赖，用于真正TLS服务端；MSRV均1.71，许可证分别Apache-2.0 OR ISC OR MIT、MIT OR Apache-2.0。关闭默认features，显式复用现有aws_lc_rs/std；不新增ring、PQ偏好、TLS1.2、fips、压缩或日志feature。全工作区已有feature合并不能描述为因此消失。复用已有getrandom/sha2/url/serde及SDK协议层，禁止直接依赖base64只为凑实现。替代方案外部TLS代理增加部署/可信转发边界，手写TLS不可接受；本方案无crate版本升级或新的锁包，实施前后仍须按精确feature tree/lock验证。aws-lc是既有第三方C/汇编/unsafe密码学实现，不得称本方案无unsafe影响。新增生产依赖边及xtask精确分层规则也在本批审批范围。

#### 定向验收与当前独立准备

审批后同步RPC/Schema、State36规范、CLI/help及测试，再开始上述新合同实现，不先开放capability。只增关联验证：

1. 标准HTTPS发现→预登记→S256授权码→owner明确审阅→access token→真实六工具；public/confidential、零授权、项目上限、403 step-up、错误回调/issuer/audience/PKCE/code重放。
2. 两个真实客户端不串Principal/Plan/Operation/幂等；授权减少/撤销/凭据轮换/expiry对已连接的下一请求和Apply接受生效；不重做原身份测试矩阵。
3. 原子refresh轮换/重用/并发/响应丢失/重启；没有秘密重放、重新签发或重复业务Apply。保留旧取消与写响应丢失证据，仅补OAuth入口关联。
4. TLS验证失败、Host/Origin/DNS rebinding、畸形/超额/秘密日志检测；State35→36全旧行/序列/外键保全、失败回滚、重开。测试证书只在隔离客户端信任，不改真实系统信任。
5. 原STDIO和预配Bearer的受影响定向回归、typed客户端/Schema、fmt/Clippy/xtask/metadata；稳定候选再集中适用门禁，本次审计不跑全量。

本轮管理扩展人工入口准备见[短清单](../testing/m8-management-manual-20261115.md)：真实开发包存在、来源和Component输入需核对后复用；完整原生程序组缺失，尚未启动窗口，不恢复桌面控制。只读Windows进程核对Core/GUI/Host/MCP均0；没有需要本轮恢复的已暂停原进程，也没有修改原环境/数据。完整OAuth审批与原生匹配构建补齐分别处理；未实现的实时观测、正式分发及来源委托不夹入本批。

本轮实际检查：开发包ZIP/manifest、Component摘要与12份provenance输入全部匹配；5份既有冻结正文摘要及同提交client-registration共6份核对通过。`scripts/validate-metadata.py`、`cargo run --locked -j 1 --target-dir target/m8-repository-check -p xtask -- check`、diff检查通过；`cargo tree --locked -p alcomd-mcp --edges normal,build,features`确认当前生产未开启rmcp auth或oauth2，本轮未改变manifest/lock/feature。审计JSON、冻结新增正文和feature tree位于`target/m8-oauth-audit-20261115/`。未运行Rust/Chromium全量、release、真实Core新用例或桌面安装，没有OAuth实现/新远端结果。只提交三份审计/状态/查看记录，不推送。

### 已批准OAuth本地候选与定向结果（2026-11-15）

所有者集中批准后，代码提交 `6bf3493cc69d28e7364f27192635e1f3f7aecade` 已在标准目录/main由真实Windows用户 `cqmhv-dt\m2922` 本地提交。上节审计的“没有实现”等描述仅指478af1b时点；本节为当前实现结果。没有推送或新增远端运行，78920f3的CI/CodeQL不覆盖此OAuth增量，M8保持IN_PROGRESS。

已接通：Core application/SQLite State36/dispatcher/typed client、九个固定RPC、受限端点和access身份、正式CLI预登记及指纹审批、HTTPS发现/授权码/刷新/撤销、原SDK与六工具。签发和授权均由Core执行，访问取token上限与当前grants交集，写接受在原事务内重验；原资源owner、实际调用者Plan/Operation/幂等归属不变。端点角色不能借clientInfo或无凭据回退owner。普通STDIO、预配Bearer、六工具Schema、Config13、WIT及旧迁移0033–0035保持。

补齐了实际边界：零授权不能批准/签发空scope；callback拒绝预含code/state/iss/error等响应参数；登记历史不因64个Core客户端上限丢失，元数据分页绑定完整快照并限制64KiB。已消费refresh在自身7天闲置到期后、family30天绝对期前重放，仍撤销存续后代。一次性token回复丢失返回503 temporarily_unavailable及固定“结果未知、重新授权”说明，不声称回滚、不自动重签；confidential CLI断管是失败退出，先检查登记revision再显式轮换。

项目查询保留六工具既有tuple游标：Core在SQL分页前求当前token范围交集，续页核对签发grantRevision及游标项目是否仍在上限。**它不是携带token指纹的不透明游标，未提供跨token的游标来源证明；切换token须从第一页开始，不能宣称继续同一快照。** 如需严格拒绝所有跨token混用，既有游标公开形状需另作最小合同变更；本批没有暗改六工具。OAuth登记/审批自身的不透明游标已绑定对应完整快照，变化明确拒绝混读。

444份最终输入见 `target/m8-oauth-audit-20261115/inputs-committed.json`，聚合SHA256 `74929637c95eb3e745cceeeff4dd312bc42d91ceda94f0d4030ef2745b28acdd`，提交前后相同。这是输入清单，不代表清单内所有模块都被执行。真实HTTPS轮次之后只变更两份静态合同测试和两份RPC Schema，随后相关模块单独通过；运行时代码/构建输入及真实用例代码未变，未为纯文档再跑业务闭环。原生/测试二进制摘要、依赖差异和完整逐模块日志位于同目录 `stable-summary.json` 及具名日志。

| 实际验证范围 | 本轮最终结果 | 日志/说明 |
| --- | --- | --- |
| OAuth Store/use case + State36保全/回滚 | 11/0，零忽略 | `store-oauth-stable.log`；含并发refresh、重启、expiry、权限收窄、65条历史及字节分页 |
| Protocol OAuth / 旧MCP DTO | 各2/0 | `protocol-oauth.log` / `protocol-mcp.log` |
| RPC Schema整个模块 | 单轮50/0 | `protocol-schema-stable.log`，没有放宽旧断言 |
| 旧MCP委托写 / State33 / State34 / State35 | 分别2/0、5/0、4/0、3/0 | 对应`store-mcp`/`migration-*`日志；原保全/序列/FK/回滚断言保持 |
| 当前State静态合同 / client连接生命周期 | 9/0、3/0 | 仅将三处当前版本严格预期35改36；不重开M7 |
| CLI单元 / MCP单元 / HTTP接入 / 冻结STDIO | 10/0、10/0、2/0、12/0 | `cli-unit`/`mcp-unit`/`http-admission`/`stdio-frozen`日志 |
| 真实HTTPS/Core/冻结SDK业务闭环 | 1/0 | `native-stable-20261114T2211196189269Z/`，实际Core接受后截断业务写及token写回复 |
| 真实OAuth CLI / 既有MCP CLI秘密输出 | 各1/0 | `native-cli-stable-20261114T2214432599634Z/`、`native-cli-existing-20261114T2216160704861Z/` |
| 严格Clippy / fmt / metadata / xtask / diff | 通过 | Clippy限定受影响七包all-targets；不冒称workspace全量 |
| Core、MCP、CLI本地debug构建 | 通过 | `build-native-stable.log`；没有release/Tauri/浏览器或三平台重跑 |

保留诊断历史：最初OAuth Store6项1/5，原因是通用幂等fingerprint必须为合法JSON，原始hash未编码；修正后当时OAuth/迁移8/0，随后增加边界后10/0。新增迟到refresh单例原样0/1，修正实际撤销判断后最终整组11/0。Schema清单最初遗漏数组长度而编译失败，修正后单轮49/1发现新Schema `$id` 不在既定rpc/v1命名空间；只修文档ID后最终模块50/0。没有把局部通过拼成全套通过；首次Clippy的嵌套if告警及修正也保留。

真实HTTPS用例使用三个独立客户端和可丢弃项目：未授权拒绝、owner选择项目子集、范围外拒绝、confidential Basic、token scope403、完整多页读取、原Plan Apply、跨客户端Plan/Operation拒绝、在途请求取消不取消已接受Operation、实际写响应丢失后显式原key恢复。Apply请求总数为5（含拒绝及显式恢复），恢复key只有一条幂等记录。token实际提交后丢回复，返回未知结果；原refresh重放不再签发并撤销family，登记下总共6条token行（原对+两轮各一对）。存续连接在授权变更/撤销后被拒绝。公开tool trace不含token/refresh/client secret；项目owner仍local-owner、Operation仍实际MCP Principal。

依赖核对：Cargo.lock只增加alcomd-mcp→rustls/tokio-rustls两条直接边，没有新增锁包或升级。MCP生产normal/build图新增16个**工作区已锁定**的TLS及构建闭包包，包含aws-lc-rs/sys、webpki、zeroize及C工具链；不能写成“运行时图完全没变”。features仅是该闭包的std/aws_lc_rs及其依赖features，未开启rmcp auth/oauth2、ring、TLS1.2、fips或PQ。rustls0.23.46、tokio-rustls0.26.4的许可/MSRV沿上节批准记录。既有base640.23.1 simd-unsafe仍存在，aws-lc第三方C/汇编/unsafe影响明确保留；没有新增ALCOMD直接base64依赖。精确前后tree及inverse记录均归档。

三次最终真实用例均先核对Core/GUI/Host/MCP进程为0；原环境本来已关闭，结束后仍关闭，没有“恢复了一个不存在的暂停进程”。原已识别data/profile逐文件摘要和已有Unity进程身份前后保持，本批runtime剩余0；未替换原程序、未迁移原运行数据、未访问真实工程。桌面控制没有恢复；测试证书为公开可丢弃end-entity/key，只在测试客户端信任，未写系统信任库或正式签名材料。

实际候选入口（已构建并核对help，**未启动持久人工环境**）：

```text
C:\Users\M2922\MyProjects\Rust\ALCOMD\target\m8-check\debug\alcomd.exe
C:\Users\M2922\MyProjects\Rust\ALCOMD\target\m8-check\debug\alcomd-mcp.exe
C:\Users\M2922\MyProjects\Rust\ALCOMD\target\m8-check\debug\alcomd-cli.exe
```

后续人工接入顺序：先确认用户唯一受保护端点空闲；Core必须显式`--data-dir <新建可丢弃目录>`。测试身份及最小项目范围需先通过既有typed `mcp.clients.create/setGrants`或管理扩展建立；当前正式CLI的`mcp client`只有list/issue-credential，**没有create/grants子命令**，不能提供不存在的命令。原生管理入口仍未准备好，不将此顺序称为现成一键环境。随后`mcp oauth registration set <ClientId> --port <固定非零端口> --redirect-uri <客户端实际回调> --expected-revision 0 --idempotency-key <新key>`预登记。所有隔离CLI加`--no-start-daemon`，不省略显式确认。

端点命令为`alcomd-mcp.exe serve-oauth --port <同一端口> --tls-cert <受信任localhost证书.der> --tls-key <匹配密钥.pkcs8> --no-start-daemon`。普通浏览器/客户端须自身信任该证书，不推荐把已知测试私钥用作日常服务。冻结协议客户端完成发现、S256与resource请求后，浏览器只显示待审ID；本机`mcp oauth request show <RequestId>`审阅，`request approve <RequestId> --expected-fingerprint <所见指纹> --grant <permission/kind/id> --idempotency-key <新key>`显式确认，再回原页面继续，获得access后使用原六工具。不要把过时fingerprint或未知秘密写结果直接重试。

未覆盖/后续：普通浏览器人工TLS接入、外部第三方客户端自动登录、CIMD/DCR和外部AS/自动证书均未验证或实施，不承诺任意MCP客户端兼容。管理扩展的匹配Host/GUI原生查看、实时观测、正式默认分发及包安装来源委托仍为M8后续；本批不进入这些实现或M9。现有tuple游标的严格跨token来源绑定限制如上保留。当前只有本地技术结果，等待新增代码远端验证；不自动推送。

#### 已批准合同补充：OAuth项目续页来源绑定

c791a94时记录的差异：已批准要求“游标绑定token上限与当前授权快照”，同时保留原六工具合同。实际旧cursor只有timestamp/ProjectId，无法证明来自哪枚token；当时查询仍权威限制每页范围和revision，不存在借游标读取范围外项目，但不能把这一点冒充来源绑定完成。该历史缺口现已按所有者补充批准实施，实际结果如下；不改写旧候选的边界。

所有者已明确批准此最小补充，直接实施，不再重复审批。仅给`projects.list`的cursor及六工具`alcomd_list_projects`对应cursor增加可选`authorizationBinding`（工具侧snake_case），非OAuth继续省略；OAuth返回Core生成的有界不可伪造绑定，续页必须带回。绑定覆盖token、当前grantRevision、token上限、timestamp/ProjectId；错误/缺失/不同token或快照用既有revision_conflict，要求重新第一页。不增加State、Config、权限、session、缓存或万能RPC。新字段同步DTO/Schema/typed client/工具透传及定向篡改/跨token/撤销回归；原仓库游标公开形状不变。实现采用现有sha2的标准HMAC-SHA256、Core启动时由OS随机生成的256位独立私钥及有域隔离的规范序列化；token摘要作为绑定消息，不能当作客户端可推导的MAC key。私钥不写数据库、响应或Debug，Core重启后旧游标需重读第一页，无新依赖或unsafe。此处更新的是实施授权，不提前记录测试通过；不重审OAuth身份、九RPC或依赖，也不推送。

### OAuth游标补充的本地收束（2026-11-15）

代码提交`0cc9b28fef07661d3e4d99f47a1d11cdcea865f3`在标准目录/main本地完成，工作者为真实Windows用户。前批6bf3493/c791a94中的tuple来源绑定缺口现已收束；没有借此重审OAuth九RPC、身份或依赖，没有推送或启动M9。原远端78920f3的CI/CodeQL仍不覆盖这些新增代码。

`projects.list`使用单独的ProjectRegistryCursor，仓库仍使用原RegistryCursor；仅项目请求/结果增加可选authorizationBinding。非OAuth省略字段，原JSON保持。typed client及MCP对应cursor的authorization_binding完整透传，适配器不签发、验证或扩大权威。Core在同一SQLite读取事务中校验token/当前grants、HMAC和分页内容；绑定覆盖token摘要、resource、当前grantRevision、冻结token上限及timestamp/ProjectId。缺失、篡改、位置变化、同一Principal换token、跨客户端或授权修订变化均返回既有revision_conflict。

MAC采用现有sha2的标准HMAC-SHA256，向量核对[RFC4231](https://www.rfc-editor.org/rfc/rfc4231.html)；向量通过不替代密钥来源检查。Core启动时用既有getrandom生成独立256位私钥，不以客户端可推导的token/摘要作密钥。私钥只附在Core认证产生的内部上下文，不写State、响应、事件或Debug。没有新依赖、State/Config、权限、session、持久查询或缓存。Core重启后旧MAC失效，原本仍有效的token可重新读取第一页；这是无持久游标状态的明确边界，不是假定token被撤销。

本轮最终输入444份，`target/m8-oauth-audit-20261115/inputs-cursor-committed.json`聚合SHA256 `6778aab1f45ec206d190bf4b4528f974ca43cd7464da140415bb1277660e8930`，代码提交前后未变。`cursor-receipt.json`绑定准确代码SHA、原生三程序字节/摘要、两个真实用例的二进制摘要及环境保全结果；输入清单不代表每个所列模块都已执行。

| 本轮实际范围 | 结果 | 对应记录 |
| --- | --- | --- |
| HMAC向量及独立Core密钥边界 | 2/0 | cursor-mac-stable.log |
| RPC Schema整个模块，含可选/非null字段及仓库不变 | 单轮51/0 | cursor-schema.log；原50/0仍属前批 |
| MCP单元，含字段解析/公开Schema | 11/0 | cursor-mcp-unit.log |
| 相关OAuth/迁移用例 | 11/0 | cursor-oauth-associated.log，原断言保持 |
| 既有仓库分页 / 项目视图分页 | 各1/0 | cursor-repository-page.log / cursor-project-view.log |
| 新增真实HTTPS/SDK/Core游标用例 | 1/0，零忽略 | native-cursor-20261115T0736167205046Z/ |
| 既有真实HTTPS卸载闭环关联复验 | 1/0，零忽略 | native-cursor-associated-20261115T0738466128829Z/ |
| 受影响六包all-targets严格Clippy、fmt/metadata/xtask/diff | 通过 | clippy-cursor-stable.log及本轮检查记录 |
| Core/MCP/CLI debug候选构建 | 通过，2m35s | cursor-receipt.json；没有GUI/release/全量重跑 |

真实游标用例读取完整两页并直接调用typed Core证明不是GUI/适配器检查；篡改MAC、时间、ProjectId、缺失绑定、同Principal另一token（较大上限）及另一个客户端均被拒绝。授权变化后旧游标失效，同一token重新第一页/续页成功；撤销作用于新HTTP请求及存续Core连接。关闭并重开同一隔离Core后，另一个未撤销token仍可认证，旧MAC拒绝，重新第一页/续页正常。非OAuth native第一页和续页没有新增输出字段。公开trace位于`target/m8-oauth-cursor-real/eb0e3d5a-d7e4-4dc7-980a-bb9e9572636f/cursor-trace.json`，不含access秘密。

原真实OAuth卸载用例完整保留此前Plan/Operation/权限、取消及实际响应丢失断言，关联复验1/0；不是整个business_tools六用例全部通过，也没有拼接为Workspace或全量浏览器通过。首次Clippy因新增测试手动去前缀告警失败，改用strip_prefix后最终严格通过，原日志clippy-cursor.log保留。原生构建首次调用在工具自动审批阶段超时，未启动；工具允许的一次重试已实际成功，不将超时写成构建成功。

两个真实用例的入口都先确认原Core/GUI/Host/MCP为0，结束后无本批runtime残留；原已识别data/profile文件摘要及Unity进程前后相同。原环境本来关闭，继续保持关闭，没有替换原程序/迁移原数据，也没有自动恢复桌面控制。现有`target/m8-check/debug/alcomd.exe`、`alcomd-mcp.exe`和`alcomd-cli.exe`已匹配本轮源码重建；只提供本地候选，不称持久人工窗口已打开。管理扩展仍缺匹配Host/嵌入前端GUI，普通浏览器TLS/外部客户端人工接入尚未验收，其他M8后续沿既有记录。本批结果为本地技术通过、远端待验证，M8保持IN_PROGRESS。

### OAuth候选33056db9远端收齐与本地窄修（2026-11-15）

所有者明确批准当前OAuth候选推送后，实际Windows用户cqmhv-dt\m2922先fetch核对origin/main=78920f382d1a0c58dece4a51fb2f54d84f48fcf2、工作树/暂存区干净、无分叉，再普通快进推送至`33056db9017036a1c8a5b0e6816698ac37e9ceb7`。完整六笔依次为93967f57（前批管理远端记录）、478af1b5（OAuth审计/查看准备）、6bf3493c（OAuth实现）、c791a94f（结果/缺口记录）、0cc9b28f（获批游标补充）、33056db9（收尾记录），无无关修改。推送前metadata/xtask/diff通过，复用已有准确本地输入，未重复完整测试。推送未强制、未发布或打tag。

这次自动运行现已全部结束，均绑定上述同SHA、run_attempt=1：

| 运行 / job | 实际结果 | 具体范围 |
| --- | --- | --- |
| [CI43699307220](https://github.com/ALCOMD/ALCOMD/actions/runs/43699307220) / Windows Server2025 | 失败 | Check中的Store lib123/1；旧State19保全测试失败。此前MCP business_tools单轮6/0，含两项OAuth真实用例 |
| 同CI / Ubuntu22.04 | 失败 | 同一旧State19保全测试，Store lib123/1；此前MCP business_tools单轮6/0 |
| 同CI / macOS15 arm64 | 失败 | MCP business_tools单轮4/2；两个OAuth用例分别在Core启动AlreadyRunning、原生TLS校验InvalidCertificate处失败；未执行后续Store测试 |
| [CodeQL43699306717](https://github.com/ALCOMD/ALCOMD/actions/runs/43699306717) | 四项全部通过 | actions、python、javascript-typescript、rust；该自动分析的event为dynamic，并非手动dispatch |

三个CI job均停止在Check；独立CLI自启动、两个Unix挂载用例、release、Tauri、平台/锁/diff及Chromium步骤均skipped，不写成通过。真实OAuth CLI具名用例此前在三个job中均通过，但这不等于整个CI成功；不将Windows/Ubuntu的MCP6/0与macOS局部结果拼成三平台完整通过。78920f3及其他旧候选的成功仍只属于旧证据。

完整最终JSON为`target/m8-oauth-hosted-20261115/{ci,codeql}-final.json`；Windows/Ubuntu/macOS逐job日志及CodeQL完整日志均保存在同目录，摘要/大小/准确失败用例见`evidence-summary.json`。持续读取时曾因沙箱无法使用keyring而触发匿名API限流，停止该轮询后改用真实Windows用户已有有效gh认证读取；没有重新登录、更改认证配置、操作Secrets或启动重复workflow。

诊断及本地修复分开提交：

- `983a67bf673f076d16d115b6df4b2f364b0869ee`仅修`remove_activity_tests`：迁移后两张State36 OAuth表均为空，但旧快照比较未列入它们。真实Windows原用例原样0/1（state19-before.log），补充mcp_oauth_registrations/mcp_oauth_tokens必须存在且为空，再保留完整after==before、事件水位+1、触发器移除及独立失败回滚断言。相关整个模块单轮4/0（state19-after-module.log），Store all-targets严格Clippy/fmt/xtask/diff通过；没有改生产代码、历史迁移或删除测试。
- `f334910a505d5bc5b25fade6a8bed2a7cd85837b`仅修OAuth测试设施：旧自签leaf有效期2020–2045，超过[Apple对2019-07后TLS证书的825天要求](https://support.apple.com/en-ph/103769)，macOS原生验证明确拒绝。使用原公开可丢弃测试私钥重签365天leaf，2026-11-01至2027-11-01，SHA256 c25ff3e9ac18e3fc0042e90bbb84744d6d606217e5dd6495a73d5fdb95918988；RSA2048/SHA256、SAN localhost、serverAuth及CA:false保持。没有关闭验证、切换弱验证器、改生产证书或系统信任库；fixture必须在2027-11-01前按README轮换。另沿用既有CLI测试的Unix私有短runtime目录方式，将新OAuth测试的Core、owner、HTTP后端和存续客户端统一绑定该实例，重开也用同一路径；只清理已核对的自有目录，Windows仍保留原受保护唯一端点。它不是生产发现/认证或IPC新入口。

本机仅重编译受影响business_tools二进制，具名游标和原HTTPS业务用例分别1/0（2.26s、2.04s），保留全部权限、取消及实际响应丢失断言；具名记录为`target/m8-oauth-audit-20261115/native-hosted-fixture-cursor-20261115T0958105053347Z/`及`native-hosted-fixture-flow-20261115T0959493714988Z/`。两轮原data/profile摘要及Unity身份前后保持，runtime残留0，原环境本来关闭并保持关闭，无桌面控制。MCP all-targets严格Clippy、fmt、metadata、xtask、diff通过。**当前Windows没有本机执行macOS原生证书策略或Unix分支；这些修正仍需下一候选的真实远端确认，不声称macOS已通过。**

444份新输入摘要为`312a22297d5499a2a0368cf14fd5908197b1e14cabf5f2f8ba6ebdba2556bc6e`，提交前后相同；相对已推送33056db9的前批输入只改变两份Rust测试及一个测试DER文件（README另为文字记录），生产代码/构建manifest/Cargo.lock/State36原迁移均未变，不为测试修正重做Rust全量、292/704项浏览器、release/Tauri或三平台本地验证。此处4/0和两个1/0均为本机定向轮次，不替代33056db9的真实远端失败。

CI阻塞仍存在于远端候选33056db9；上述本地修复及本节记录超出这次固定候选推送范围，未自动推送，须由所有者另行明确授权普通快进到origin/main。后续若获批只由push触发现有CI/CodeQL，不dispatch或rerun旧运行。M8保持IN_PROGRESS，管理扩展原生窗口与普通浏览器人工TLS接入等未覆盖项沿既有归属保留，不开始M9。

### OAuth候选4005583远端收齐与State16补充（2026-11-15）

所有者明确批准983a67b、f334910、4005583三笔后，已在真实Windows用户环境核对固定SHA、三笔完整链、干净工作树、远端祖先关系、metadata/xtask/diff，再普通快进推送至`4005583087921f40d9fdbd869588edcc7ae07939`。没有额外提交、强推、tag、release、workflow_dispatch或rerun；沿用原本地有效输入，不重复全量验证。

本轮同SHA自动运行现已全部结束，均run_attempt=1：

- [CI43703345842](https://github.com/ALCOMD/ALCOMD/actions/runs/43703345842)：**三平台失败**。Windows/Ubuntu/macOS均在`empty_unity_override`的`state16_upgrade_preserves_every_existing_row_and_empty_override_survives_reopen`失败，三个模块各为单轮2/1，失败差异都是新增的两张空OAuth表。
- 三平台此前的MCP business_tools各单轮6/0（含两个OAuth真实用例）、Store lib各单轮124/0。State19保全用例三平台均通过；macOS的原生TLS校验、私有IPC、真实身份/业务/游标闭环已实际通过，f334910不再记为仅Windows本地证据。这是本轮各模块的实际结果，不与旧33056db9或本地轮次拼接。
- [CodeQL43703345588](https://github.com/ALCOMD/ALCOMD/actions/runs/43703345588)：rust、javascript-typescript、actions、python四项全部成功（该自动分析event为dynamic）。此结果属于4005583，不冒称后续本地修复已取得CodeQL结果。
- 三个CI job均停止在Check，独立CLI自启动、两个Unix挂载用例、release、Tauri、平台/锁/diff和Chromium步骤均skipped。完整workspace、独立guest/前端检查及后续步骤不能据前缀模块通过而记为整体通过。M8仍IN_PROGRESS，M7签收不重开，未进入M9。

完整最终JSON、三job原始日志、CodeQL完整日志及每份SHA256/大小位于`target/m8-oauth-hosted-4005583-20261115/`，总表为`evidence-summary.json`。与33056db9的目录分开保存，两个失败轮次和各自CodeQL成功分别保留。只持续只读跟踪这两个自动run，没有重启、取消或重复workflow。

关联核对时发现上一轮只补了State19，未同步State16的另一处相同保全夹具。本机在未改动时运行该三用例模块原样2/1（state16-before.log），不是等远端失败后盲重跑。`174aa35ed2857cf46af21903d722fcf982aeed17`只给`crates/alcomd-store/tests/empty_unity_override.rs`的已批准新增权威表清单补mcp_oauth_registrations/mcp_oauth_tokens，仍要求两表存在且为空，再继续完整retained==before、事件序列、外键、空override重开保留及失败回滚等原断言；没有过滤未知表或删除/放宽测试。

修复后同模块单轮3/0、零忽略（state16-after.log），该integration target严格Clippy（state16-clippy.log）、fmt、xtask和diff通过。没有重跑Store全套、完整Rust/浏览器、release/Tauri或三平台本地测试。新输入444份，摘要`1b2a4e32d6545093cb1233c0b4547164f19fb641b5f75ca6b927b3799d635abf`；相对已推送4005583的有效输入仅一份Rust测试变化，生产代码/manifest/Cargo.lock/State36迁移均未变。原环境/原数据/Unity未操作，不恢复桌面控制。本机3/0不替代远端2/1，也不是整个剩余workspace通过。

当前远端阻塞为4005583中的State16旧保全夹具。174aa35及本节记录为本次推送之后的新增本地提交，尚未推送；待所有者明确授权普通快进后，仅由push触发下一候选CI/CodeQL，不手动重跑旧运行。原生管理扩展窗口、普通浏览器人工TLS接入、CIMD/DCR等既有未覆盖项继续保留，不因技术前缀验证通过而宣布M8整体完成。

### OAuth候选36a782c与隔离查看准备（2026-11-15）

所有者在明确的两笔推送及后续顺序后确认继续，已由真实Windows用户核对干净main、174aa35→36a782c完整链、origin/main祖先关系及提交文字，普通快进至`36a782c7b78584a46e242b91270fb464bfd9fa29`。未夹带新合同或其他修改，无强推/tag/release/dispatch/rerun。上一节的“未推送、待授权”是当时历史状态，本候选已包含State16修复。

同SHA自动运行现已完整收齐，均attempt1，没有dispatch/rerun：

- [CI43708317935](https://github.com/ALCOMD/ALCOMD/actions/runs/43708317935)三平台整个job全部成功：完整Check（Workspace/独立guest/前端与metadata）、CLI自启动、release/Tauri、适用平台/锁/diff门禁均完成。Chromium各单轮704/0、零重试，Windows17.8m、Ubuntu14.9m、macOS24.0m；两个Unix真实挂载具名各1/0、零忽略，保留常规Workspace内原有忽略，不冒称所有条件用例均执行。
- 三平台MCP business_tools各单轮6/0、Store lib各124/0、State16保全integration模块各3/0，以上模块均零忽略。两个真实OAuth（HTTPS/权限/业务/取消/实际响应丢失与游标来源绑定）用例及State19/State16具名保全用例在三平台全部通过，已批准修复现有同SHA证据；不是把旧33056db9、4005583或本地局部结果拼成通过。
- [CodeQL43708317682](https://github.com/ALCOMD/ALCOMD/actions/runs/43708317682)的rust、javascript-typescript、actions、python四项全部成功。当前切片没有遗留远端技术门禁阻塞；M8后续及人工查看未据此完成。

最终JSON、三平台逐job完整原始日志、CodeQL完整日志及每份大小/SHA256位于`target/m8-oauth-hosted-36a782c-20261115/`，总表`evidence-summary.json`。摘要提取对Windows颜色码及macOS stdout/stderr交错仅规范解析视图，原始日志保留；没有重跑测试。期间一次GitHub只读轮询因unexpected EOF结束，随后以同一认证读取原运行并继续收齐，未重启workflow。旧33056db9、4005583的CI失败及各自CodeQL成功继续保留；本段结果只属于36a782c。

并行补齐[管理扩展隔离查看入口](../testing/m8-management-manual-20261115.md)，不恢复桌面控制：

- 只在独立`target/m8-check`单任务构建Host及Tauri嵌入前端debug GUI，分别成功；Tauri的beforeBuild前端check/build通过。没有执行release、完整Workspace或浏览器全套。本轮构建日志为`target/m8-oauth-audit-20261115/manual-{host,gui}-build.log`。
- 从0cc9b28到本候选只变测试/测试证书/文档，生产源码与依赖未变，复用现有Core/CLI/MCP；444份输入摘要仍为`1b2a4e32d6545093cb1233c0b4547164f19fb641b5f75ca6b927b3799d635abf`。五个实际程序的路径/摘要存于`target/m8-review-36a782c/candidate.json`，不承诺不同编译的二进制字节相同。GUI单独放置且无同目录Core，避免失联后默认自启动访问原数据。
- 当前样例包125919字节、SHA256 `3f41189c80200eec2c0c8ef79676f51896b0518721180f10f55c04587f752ca7`保持；未改包、重做正式扩展或跳过验签/发布者/权限检查，安装留给正式流程。
- `Open-Review.ps1 -CoreOnly`真实成功启动明确`--data-dir`的隔离Core，以正式CLI登记两个可丢弃项目；system/projects/operations/clients/oauth-registrations五项公开读取成功，2项目、0活动任务、0客户端、0OAuth登记。它们是隔离准备读取，不冒称原环境五组公开状态复核。核对自有PID/路径/命令行及0任务后关闭Core；随后重开、只读取得同样五项结果，两个ProjectId/revision保持，再核对0任务并停止自有Core，runtime残留0。原环境本来关闭并保持关闭，原36份data/346份profile摘要保持，未操作Unity（准备前后均0进程）。证据`target/m8-review-36a782c/records/{preparation-result,reopen-result,reopen-preservation}.json`；辅助保全摘要首轮因空Unity数组未输出文件而未完成，按初始脚本实际零条语义核对后通过，未伪造历史快照、未重跑业务测试。
- 准备阶段GUI启动分支只做语法/源码核对，未执行、没有真实窗口截图或人工通过结论；随后按所有者明确启动授权实际打开，结果见下段。入口发现已有Core/GUI/Host/MCP即拒绝，不自动暂停、终止、替换或迁移原环境。普通浏览器/目标MCP客户端OAuth接入仍需用户受信任localhost证书，未安装系统信任、未启动新监听或用公开测试私钥作为日常服务。

M8继续IN_PROGRESS。管理界面人工查看、普通浏览器TLS接入、实时连接/请求/订阅观测、正式默认分发及其余既有后续仍分别保留；不开始M9。本次状态与准备记录仅本地提交，不追加推送或因此制造全量验证。纯文档适用metadata、xtask及diff检查通过，444份输入未变；计划入口及扩展README不再将已实现预登记OAuth、客户端与授权管理误列为未实施，CIMD/DCR等未支持能力仍明确保留。

所有者随后明确要求打开隔离验收环境并保留窗口。在确认没有原Core/GUI占端点、五程序摘要一致后，已实际执行Open-Review.ps1的GUI分支：隔离Core PID45200、GUI PID42264，窗口ALCOMD3可见且响应。只读查询GUI自身句柄的管道服务端PID，5条连接均指向45200；同时核对Core可执行文件/明确data参数、WebView2独立profile及预期2样例/启动时0活动任务，确认GUI确实连接本轮Core。没有恢复Computer Use、发送界面输入、拍摄截图、暂停或替换原环境。原36份data/346份profile摘要与准备基线相同，原环境继续关闭；本轮Core/GUI保持运行供用户检查。证据`target/m8-review-36a782c/records/{gui-connection,gui-start-preservation}.json`。这是窗口和连接核对，不是人工体验签收或M8完成；不推送，不重跑完整验证。

### 管理扩展零授权启动与权限弹窗整改（2026-11-15）

所有者实际遇到无法启用，并明确要求无需预授权即可启动；随后确认看不懂的是“管理扩展 → 管理权限”弹窗。只整改这两个入口，不重开M7或扩建身份系统。本地代码候选为`5884d5e1dc5152995bec20206f1a642d21d7ae9f`，未推送；36a782c的Hosted通过仅属于此前基线，不能替代本次新代码远端验证。

**原因与改动**：原开发包把`mcp.clients.read`列为必需，而实际grants为空，Core依既有合同正确拒绝启用。现将此管理扩展的read/manage/projects.read三项均声明为可选，零授权可启用、打开受限页面；未获读取权限时显示中性未授权状态，不泄漏客户端数据或提供写控件。明确授权后才读取，撤销使旧会话失效，重新打开仍可进入受限页面。没有自动授权、owner旁路、Core校验删除或公共合同变化；其他扩展的必需权限未获授权仍拒绝启用。

权限弹窗原先分离展示声明、授权列表和通用下拉表单，难以看出“这一项允许了什么”。现每项直接展示中文名称、用途、实际状态及允许/撤销按钮；项目权限列出具体项目并单独确认所选项目，选择本身不授权。展示前读齐同revision所有分页，读取失败不伪装为零授权，迟到旧读取不能覆盖新状态；结果未知时先刷新权威状态，不能自动重写。继续复用Material、共享反馈与displayPath，仅改变显示文本，原ProjectId及RPC参数保持。不建立新通知/授权系统；无RPC、State/Config、WIT、权限名称或依赖增量。

本地定向证据全部位于`target/m8-zero-grant-20261115/`：

| 范围 | 真实结果与输入 |
| --- | --- |
| 独立guest | 单元3/0、doc-test1/0、严格Clippy通过；仅重建受影响Wasm release，Component SHA256 `1ec0fb1ac969e6711db44103d95cba225b8c9b251c1c1a325be4af75c7156ba8`，12项provenance输入已更新，依赖/WIT/工具链未变 |
| Host及合同 | 实际Component链接/普通Portable UI校验1/0；`m8_management_contract`2/0；Host tests严格Clippy通过 |
| 真实Core新增 | `optional_management_permissions_allow_zero_grant_start_and_restrict_data`1/0：正常验签与明确测试发布者批准、零授权启用/开页、无数据/写控件、伪造创建拒绝且权威客户端数未变、显式读取授权/撤销、旧会话拒绝及新受限页、终态清理 |
| 真实Core关联 | 原`management_component_uses_real_core_grants_and_loses_access_after_revocation`1/0：原必需权限启用拒绝、真实管理操作、项目范围、实际授权及撤销、结果未知保护全部保留；不是GUI假客户端证明安全 |
| GUI | check与嵌入前端debug no-bundle构建通过；权限/事件/反馈关联子集最终单轮14/0、零重试；随后只将两项错误夹具改用真实公开`extension_scope_denied`并断言可读信息，同文件三项3/0。后三项覆盖前述子集，不相加或称完整704项通过 |
| 格式与仓库 | root/guest fmt、locked metadata、xtask、diff检查通过；无Workspace全量、完整Chromium、Tauri release或新三平台运行 |

失败轮次保留：浏览器首轮6/8为新选择器误计Material内部li/span，收窄到实际授权行后第二轮14/0、最终界面轮14/0，原精确数量/身份/授权/迟到断言保留；首轮新增真实Core0/1，在Plan阶段被`extension_manifest_invalid`拒绝，原因是新可选权限未按现行合同字典序排列，修正清单后同例1/0，不放宽验证。日志分别为`browser-{first,second,final}.log`、`browser-error-contract.log`及`native-optional-first-failed.log`、`native-{optional,existing}.log`。临时部署助手首次因选中不同构建的serde rlib而编译失败，匹配client实际依赖指纹后编译成功，未新增依赖。

**真实隔离更新**：仅核对0活动任务后暂停本轮自有Core/GUI，为上述两例让出唯一管道，测试使用全新可丢弃数据；finally恢复同一review数据/profile。首次恢复窗口的CLI短连接断开后触发Core既有1秒空闲退出，已确认无Core而非强杀未知进程；后续用有界60秒只读typed连接覆盖WebView启动间隙，实际GUI连上后不再依赖该连接，不修改Core空闲合同。首次复制GUI时进程退出后的文件锁尚未释放，核对旧摘要未变且GUI已退出后才完成复制，未删除或改用原环境程序。原36份data/346份profile及Unity保持，证据`native-preservation.json`与最终保全记录。

测试成功后，review中的旧开发包经正常Core卸载Plan/Apply（retain_data）与新安装Plan/Apply更新；两个Operation均succeeded，0活动任务。新包为`target/m8-zero-grant-20261115/management-zero-grant.alcomdextension`，126423字节，SHA256 `223b35f44ee3b35437da209b4b5c30968305897d487ffa3f4f35c06cfb6c47a6`，同一已批准测试发布者，包内摘要`8625ebed1877900d8b68f5f25dda1dceb861a187de56088adbd0875ebee20a01`。实际启用成功、required=[]、grants=[]，通过Core打开的真实Portable UI为未授权中性状态，无创建控件；原两个项目及客户端记录保持。记录`live-package-update.log`及`live-after.log`，没有GUI/脚本直接读写数据库。

当前GUI PID39500、Core PID47292，窗口可见且响应，5条真实管道均连到指定review Core，WebView使用原review profile。新GUI SHA256 `9a2381b194948d6cd76067a22822169754378645f91956f7a7c70b4223efaa8a`；Core/CLI/MCP/Host字节复用36a782c输入，五程序/新包摘要及本地候选写入review的candidate.json。连接证据`gui-connection-zero-grant.json`；旧45200/42264已关闭，`records/gui-connection.json`现为最新状态。更新后的Open-Review入口仅做语法检查；相同程序/参数的实际启动与连接已分别验证，不冒称修改后的整个入口又执行过一遍。没有恢复桌面控制或伪造原生截图。浏览器前后截图分别在`target/m8-mcp-management/permissions-global-scope.png`和`apps/alcomd-gui/target/m8-zero-grant/`，明确属于浏览器夹具。

窗口保留供所有者查看简化后的权限弹窗与零授权启动，清单见[当前人工入口](../testing/m8-management-manual-20261115.md)。原环境本来关闭并保持关闭；M8未人工签收、仍IN_PROGRESS，普通浏览器/目标客户端OAuth接入等后续保持，不进入M9，不自动推送。

### 权限单开关与项目读取范围差异（已批准，2026-11-15）

所有者要求每项权限仅用一个开关，不再逐项目选择。`background.run`和两项MCP权限本来就是单一scope，可直接复用现有Material Switch与set/revokeGrant，展示只反映Core已确认结果，不自动授权、合并权限或重试未知写入。

`projects.read`现行公开合同及State36继承的CHECK仅允许`Project/<ProjectId>`，不能把GUI遍历当前项目逐条授权假装为一个全局权限。建议批准以下最小破坏性差异后再实施受影响部分：

- 扩展的`projects.read`改为`Global/*`单开关：开启允许读取当前及未来登记项目的既有安全基本摘要，不开放原始文件/路径或写入；交互Client仍须有自己的项目读取权限，实际访问取双方交集。MCP客户端自身的项目范围、OAuth上限及六工具范围完全不变。
- 复用原setGrant/revokeGrant/list与现有权限名称，不新增RPC、Config、WIT或自动授权。更新Core所有扩展项目读取、必需权限与管理Host项目列表的权威检查及对应Schema/规范。
- State37单事务仅调整extension_grants组合；删除旧的扩展逐项目`projects.read`授权（含revoked记录），受影响扩展grant_revision/revision递增并使原lease/session失效，其他授权/客户端/项目/凭据与业务数据保留。旧有限授权不自动扩大为全局，迁移后需用户明确打开。历史迁移不改、原运行数据不升级。
- GUI四项统一开关；读取未知时不显示假“关闭”，写入中锁定，失败还原权威状态，结果未知先刷新，撤销立即生效。定向验证保全/回滚、当前及后续项目、双重范围、关闭/撤销、旧会话与分页失效；无需重做身份/六工具或运行完整门禁。

所有者已批准上述扩展Global/*与State37范围，直接实施，不再重审。其他三项Material开关已通过前端check及相关浏览器单轮15/0（含新增键盘/在途/失败还原用例），这尚不是四项完整整改通过。

随后所有者明确“任何针对项目的授权”均不应反复按新项目授予。审计确认另有MCP客户端的`projects.read`与`packages.manage`仍绑定ProjectId，OAuth token上限继承该授权。所有者随后已批准以下统一能力公共语义及State37补充，直接实施，不再重复审批：

- MCP客户端这两项改为与其余六项一样的`global/*`能力授权；覆盖当前和未来登记项目，其中`packages.manage`可修改任一项目的包，但仍须既有Plan、显式确认、原Plan Apply及实时权限检查。工具的project_id只是操作目标，保留真实Principal、owner/任务/计划/幂等隔离；不改成owner代理，不放宽路径、项目身份与事务保护。
- 复用现有mcp.clients.*、CLI、Host/WIT、OAuth scope与六工具；输入输出形状/名称不变，仅收紧为全局组合并同步Schema。管理扩展不再让用户选择项目授权，按能力展示；OAuth仍绑定token/授权快照与既定scope上限，保留游标来源绑定、轮换和撤销，不重建身份。
- 在同一未实施State37中追加：对当前含旧逐项目授权的MCP客户端创建新不可变快照，只移除这两种旧逐项目授权、保留其余权限，client revision/grant_revision递增；旧快照及历史幂等证据不改写，不作当前授权。使包含旧逐项目上限或对应受影响客户端的OAuth存续access/refresh family失效，需重新明确授权。普通客户端凭据/登记及业务数据不删除、不扩大权限；无受影响授权的客户端不变。
- 这是对先前“其他MCP客户端范围不变”的已批准修订，与扩展State37一并实施。原运行数据不升级，不推送，不进入M9。定向覆盖未来项目、关闭/撤销、跨客户端Plan/Operation隔离、OAuth上限与旧token失效、快照/迁移保全及取消/写结果未知。

### 统一能力开关与State37本地收束（2026-11-16）

所有者已集中批准上节两批差异。本地代码`485eddb2548f122037585667b19c383b729d48e6`，在标准目录/main由真实Windows用户`cqmhv-dt\m2922`提交，未推送。本节取代5884d5e的逐项目按钮作为当前状态，保留其历史结果；M8继续IN_PROGRESS，人工查看未签收，不进入M9。

**实现与破坏性边界**：GUI四种扩展权限复用已有Material Switch；读取失败不伪装全关，分页读齐、写入中锁定、失败/未知刷新和旧响应防护保留。扩展projects.read只授予当前/未来项目的安全摘要，交互Client仍须有相同读取权。MCP八项授权只接受global/*；目标ProjectId仍绑定原业务、Plan/Operation/幂等与实际Principal，资源owner不改。OAuth当前授权/能力上限与token来源游标绑定继续复用。管理guest将八项能力编辑为开关草稿，沿用原审阅确认和change-grants，不再列项目或增加新接口；自身无项目读取调用，清单仅剩read/manage两项可选权限，零授权仍可启用。四种可授予扩展能力为background.run、projects.read、mcp.clients.read/manage；并非整个系统权限目录均能授予扩展。扩展在签名清单中声明required/optional只是申请，实际授予仍由用户和Core决定。

State37仅新增0037，旧0033–0036原样保留。一个事务撤销旧扩展project-read及lease、递增revision；MCP当前含旧project授权的客户端转到去掉旧授权的新快照，其他授权保留，旧快照/历史幂等不改；相关OAuth family失效，无受影响记录不变。没有全局授权补造、原数据升级、新依赖、Config、权限名称、RPC/WIT形状或身份框架。规格见state-v37及当前RPC/扩展Schema；资源范围枚举和组合同步收紧，不兼容旧逐项目授权请求。历史大快照仍可精确重放，但不能成为当前授权。

证据位于`target/m8-zero-grant-20261115/`，均为此代码输入；以下为各自轮次，不能相加为完整套件：

| 范围 | 实际结果 |
| --- | --- |
| GUI定向 | 第一轮12/2，窄窗两个主题的Material焦点环超出4px；只为开关行补容纳空间，保留<=1px断言。最终单轮15/0、零重试，含键盘/在途锁定/失败还原/完整授权读取/迟到请求/两主题窄窗；`switch-global-browser-{first,second}.log`。此前三开关15/0仅为中间输入 |
| Store扩展 | 首轮12/1，旧假设把不存在的项目当未授权；正确区分ProjectNotFound后同模块13/0，后补旧Project格式拒绝与无副作用断言，具名最终1/0；`store-extension-capabilities{,-second}.log`、`store-required-global-final.log` |
| Store MCP/OAuth | 首轮13/1为4096历史容量用例仍尝试登记旧范围，按批准迁移及新请求拒绝改为精确历史/当前快照断言后同模块14/0；`store-capabilities-{first,second}.log`，没有删除容量、分页或幂等断言 |
| State37及既有迁移 | 新迁移模块4/0，精确旧行保全、无自动扩大、lease/revision、旧token family失效、安全family仍认证、回滚/重开；旧State35关联3/0。初期缺少PrincipalId导入编译失败保留，非运行通过；`state37-final.log`、`state35-reopen-state37.log` |
| Application/CLI/合同 | 身份5/0、OAuth CLI单元2/0，m6合同5/0、M7现行版本断言9/0、管理合同/真实产物来源2/0；`application-capabilities.log`、`cli-oauth-capabilities.log`、`capability-contracts-final.log`。M7仅同步获批版本断言，不重开验收 |
| Guest/Host | guest5/0、doc-test1/0、严格Clippy及单独Wasm release成功；Component `d43bd812027db11da1670755874c3a87c523dfd41acea759ff88cf44ff41a496`，12项provenance核对通过。实际Host ABI1/0、正常签名/零授权Core及原完整管理Core用例各1/0；`guest-switches-final.log`、`capability-host-abi.log`、`capability-native-{optional,management}.log` |
| 双客户端真实身份 | 首轮0/1，重启后旧断言仍期望另一项目拒绝；改为精确读取包含授权后新建项目的全部三项后1/0。默认拒绝、实时授权/撤销、轮换/重启、Plan/任务/幂等隔离保留；`capability-native-identity-first-failed.log`与`capability-native-identity.log` |
| 真实STDIO/HTTP/OAuth六工具 | 首轮4/2，STDIO卸载及两类在途取消/丢响应、OAuth游标来源绑定通过；HTTP仍假设列表第一条必为旧单项目，OAuth零授权用例仍发送旧project格式。分别修成精确四项目集合与合法global请求仍拒绝越过零上限后，两例各1/0；`capability-native-business-first-failed.log`、`capability-native-{http,oauth}-recheck.log`。没有重跑整模块，不记6/0 |
| 构建/严格检查 | 前端check/build、嵌入前端GUI debug与匹配Core/CLI/MCP/Host debug构建通过；受影响包及guest严格Clippy、最终测试Clippy、root/guest fmt、locked metadata、xtask/diff通过。两次错误target的GUI编译已仅停止自有编译树，保留interrupted日志；最终用绝对target构建成功。无全量Workspace/Chromium/release/新三平台 |

806份源码/测试/Schema/清单/锁输入的`capability-inputs.json`摘要为`52633e9de3deb8946bb63099ad6622d6d6a2299ef08345a8faf8271985f0721f`；五个实际程序/包摘要在review/candidate.json，不宣称不同构建字节相同。现有远端36a782c、CI43708317935、CodeQL43708317682通过仅属于原候选，本轮新增代码尚无远端验证。

**真实隔离更新**：先核对自有review PID/路径/data及0任务，保存system/projects/operations/clients/oauth-registrations五组公开结果；仅暂停本轮Core/GUI，备份review data后升级36→37（只读核对实际版本）。新程序组放在review/capabilities-37，GUI目录无同名Core，旧程序/备份保留，不创建worktree或并行运行两套Core。旧测试包正常retain_data卸载Operation `316775e7-452d-48d4-a5a8-ff4ffc56b6f6`成功；初次安装助手误把目录拼成不存在的m8-capabilities，Core正确拒绝extension_package_invalid。核对卸载终态和空扩展列表后，仅继续安装真实包，未重复卸载/删除备份/跳过校验。新安装Operation `cb56acab-d537-4c9b-b8c1-c8dc0fb4ba25`成功，实际packageDigest `a9100306ceb8ade3450fe9b9b359d1d25a81f5bc50a2016dfa0ec4ee8949bac7`。正常测试发布者批准、真实签名校验、启用和Core开页均成功，required=[]、optional=[manage,read]、grants=[]，未自动授权。

新包`target/m8-zero-grant-20261115/management-capabilities.alcomdextension`真实存在，119860字节，SHA256 `8980614f33694a4fadd11faf05d2a106d848b48ee8eed8cc651988aaaa10b9da`。更新前后两个项目、客户端和OAuth登记逐项一致、0活动任务。原环境本来关闭并保持关闭，原36份data/346份profile摘要保持，Unity未干扰。首轮部署只读断言误用了system.status没有的dataSchema字段、恢复脚本漏带快照函数，均在写入前停止并修正；日志保留，不计产品通过。部署助手编译初期serde_json指纹目录匹配失败亦保留。

前次分段部署的60秒只读启动连接在助手排障间隙过期，GUI虽可见但Core空闲退出，未把它记为连接成功。完成安装后，已实际执行更新后的Open-Review完整入口同时打开Core/GUI，复用已有有界保活覆盖WebView启动，无新启动机制或空闲合同修改。当前GUI48912可见且响应，5条GUI自身管道均连Core49940，WebView使用review/profile；后续最终核对确认保活已退出而GUI连接仍在。记录`capability-final-start.log`、`capability-gui-connection.log`、`capability-live-package-{update-path-failed,install}.log`、`capability-final-preservation.json`及review/records。未恢复桌面控制或拍摄原生截图。浏览器截图仅为夹具，前后分别`permissions-narrow-light.png`与`permission-switches-light.png`，不冒称真实桌面。

人工入口见现有manual：只看扩展两项开关、零授权启动与客户端八项能力编辑，不重新验收M7或身份安全系统。普通浏览器受信任TLS/目标客户端人工接入、会话超时/外部断线、实时观测与正式默认分发等既有后续仍未覆盖，不宣布M8完成，不自动推送。

最终残留核对发现协议序列化样例仍写旧Project，另以纯测试提交`768022aa361779e2710e4fd2972f7df08b73a209`同步为Global/*并断言Schema范围组合；仅该具名用例1/0（其余50项过滤），证据`permission-schema-final.log`与`permission-schema-final-input.json`。未改运行源码/依赖/构建输入，继续复用485eddb实际程序，不再构建或重跑业务回归。最终被动核对时启动助手已自然退出，GUI48912仍有5条连接Core49940；Unity状态与原快照一致。

### 未发布授权格式严格读取（2026-11-16）

所有者针对“旧逐项目授权撤销”明确修订：未发布版本的运行逻辑只认识现行能力授权，逐项目数据不是可兼容解释的配置。该要求覆盖输入、列表、认证、授权快照与幂等重放；原章节关于旧格式快照仍精确返回的实现/结果仅为前候选历史，不是现行行为。

只修实际残留：Store grant_snapshot复用validate_mcp_grants，遇到不支持的存储组合整体返回既有Unavailable，列表/认证/Host分页/幂等读取共同生效；create/setGrants在查幂等记录前也校验输入，不借旧记录接受无效请求。没有逐项目特例、兼容解析/转换、静默删项、空授权回退或新错误码。扩展现有valid_scope及OAuth token校验原本已拒绝，无需重复开发。State37仍是此前批准的开发数据清理，SQL与旧迁移未改；保留原始历史行不等于运行时支持其授权语义。

新增unsupported_stored_grants_are_rejected_by_reads_and_idempotent_replays修复前真实0/1：client读取返回了project格式。修复后受影响M8模块单轮15/0；随后补正常八项能力逐页完整读取并收紧迁移证据过滤到精确客户端/新revision，最终单轮16/0。旧1024/4096样例现在分别证明不支持的快照/写入/重放拒绝、原始行和compact幂等记录保留；原分页数量、字节上限与游标快照/身份绑定继续保留，用原始证据ID测试通用page边界，不能据此宣称旧权限能分页读取。现行八项能力另有真实Store快照分页用例，不以旧格式冒充现行配置。State37保全/无扩大/回滚/重开模块4/0；Store lib/tests严格Clippy通过。日志unsupported-grants-{before,module,module-final,state37,clippy}.log位于target/m8-zero-grant-20261115，失败不覆盖、不拼接。

同步当前RPC规范、State37解释与A-041，不新增State/Config、依赖、权限或接口。最终fmt/metadata/xtask/diff通过，仅本地提交、不推送。本次未重建部署程序或占用端点，不触碰正供用户查看的485eddb隔离Core/GUI，也不操作原数据；当前窗口、之前真实Core/浏览器证据只绑定原候选。本次后端严格读取只有上述本地定向证据，尚无新增原生/远端验证。M8仍IN_PROGRESS，不进入M9。

### 内部逐项目授权模型清理（2026-11-16）

所有者要求彻底清理内部残留并同步测试/文档。实际审计发现AccessContext仍能保存项目读取/包写入范围，构造器允许Exact项目集合；旧调用虽没有当前公开登记入口，模型仍可表达逐项目授权。代码`465f82386ba530b35cf883c7fe1370cea19cb6c6`现删除这两个字段、with_project_read_scopes及两类project-scope检查器，无旧别名/桥接/保留给未来适配器的旁路。ExtensionId访问仍需要独立隔离，原通用AccessScopes收紧命名为ExtensionUiScopes，仅用于extensions.ui.use。

项目读取、候选查询、模板来源和Host交互改为检查ProjectsRead能力；包Apply保留PackagesManage及原read能力，OAuth接受事务仍实时读取token/当前授权并逐项检查。已有重复能力检查只删除冗余部分；实际项目/仓库可见性与owner、MCP真实Principal、原Plan/目标身份/revision/pinned source、Operation与幂等隔离不变。管理Host项目列表不再按内部项目集合过滤，仍在同一事务先检查Client与Extension双方能力/lease，再由既有safe summary取得目标。OAuth能力scope与UI的ExtensionId隔离不是逐项目授权，继续保留。

管理客户端授权指纹删除项目范围项，继续编码排序后的当前权限、ExtensionId范围、Principal、连接/实例与会话；能力撤销或上述身份变化仍使游标绑定不同。旧内存会话/游标不建立兼容层。Store项目列表助手改名authorized_projects，明确其检查的是当前能力和OAuth上限，不是项目ACL。

测试按现行合同更新：旧“对项目A允许、B拒绝”断言改为项目读取不隐含包修改/扩展页面权限；候选查询仍逐项拒绝缺少四种必要能力，并在读源前拒绝非法目标ProjectId。新增管理指纹测试验证权限顺序稳定，能力撤销、ExtensionId及Principal/连接/实例/会话变化均改变绑定。权限规范、主RPC、Portable UI、威胁模型及备份合同清除旧project read/write scope表述，架构和A-041明确内部模型也仅按能力授权。内部Rust API为有意删除，不新增公共RPC/WIT形状、权限、依赖、State/Config或数据库迁移。

本轮证据目录`target/m8-project-model-cleanup-20261116/`，绑定上述代码（candidate-evidence.json），均为定向模块：

| 证据 | 结果 |
| --- | --- |
| application-mcp.log | 5/0，零授权、身份/伪owner、未知格式、读写能力分离 |
| application-portable.log | 14/0，含新增授权指纹、ExtensionId隔离、会话关闭与敏感回放清理 |
| application-candidates.log | 5/0，必要能力、目标身份、查询/来源/游标变化 |
| store-mcp-oauth.log | 16/0，真实Store包接受事务、撤销/重放/owner保留、OAuth轮换/上限/幂等及不支持格式拒绝 |
| security-backup-contracts.log | 安全3/0、备份4/0；保留Host调用双方权限/lease绑定和备份权限边界 |
| project-package-reads.log | 1/0，实际隔离文件读取、内存分页、读后数据库不变；不是原生IPC占端点测试 |
| extension-project-capabilities.log | 2/0，必需全局读取、后续项目及lease逐次撤销/namespace校验 |
| affected-clippy.log | application/store/Core所有targets严格Clippy通过 |
| fmt.log、metadata.json、xtask.log | 格式、locked metadata、仓库一致性与diff通过 |
| runtime-model-audit.json | 扫描436份相关Rust/TS源码，旧项目范围字段/构造/检查器与旧通用范围名零引用 |

未重复完整Workspace、Chromium、Tauri/release或三平台，也未重新占用Windows唯一管道。旧SQL迁移、原始历史行、旧格式拒绝/迁移测试仍作为证据保留，不是可用的逐项目授权模型。之前各候选结果及失败记录不改写，不拼接为新候选完整通过。

本轮未重建部署Core/GUI，不停止正供用户查看的review程序；review/candidate.json仍为485eddb，不能将源码清理描述成该窗口已升级或新原生闭环已通过。原数据未访问/升级，没有恢复桌面控制。未推送、没有本轮Hosted/CodeQL结果；M8保持IN_PROGRESS，人工查看仍待确认，不进入M9。

### 扩展MCP工具Provider v1（2026-11-16，所有者已批准实施）

所有者要求实现已集中讨论的设计。决策：扩展工具与内置工具统一目录/调用；MCP客户端仍仅按现有业务能力授权，不增加扩展或工具白名单。新增可选扩展权限mcp.tools.provide，默认关闭。首版包含真实Core写入，以既有包卸载Plan/Apply/Operation闭环验证，工具种类不限制为六种，不扩展Unity/备份/外部网络业务。

批准公共增量：Manifest可选mcp.tools固定指向component/mcp-tools.json（签名包内容，最多32项/64KiB）；独立alcomd:mcp-tools/provider@1.0.0导出和core@1.0.0受限业务导入；mcp.tools.catalog与extensions.mcpTools.call，capability mcp.extension-tools.v1；扩展可授予现有repositories.read/packages.read/packages.manage/operations.read/operations.cancel，并使用Global/*，MCP客户端仍是原八项能力。State38只增窄extension_mcp_calls记录表和extension_grants权限CHECK；保全原行、旧迁移不改，Config13不变，不升级原运行数据。调用记录只保存摘要/冻结来源/权威Core结果，不保存参数、凭据或扩展正文。

执行边界：真实MCP Principal+扩展当前权限/lease取交集，OAuth当前上限/撤销复查，Core业务接受事务原子记录结果引用；Plan/Operation/幂等保持实际MCP调用者。每次最多一个持久Core动作，不可一工具内静默Plan再Apply；请求取消不取消已接受Operation，同键恢复不得重跑guest。UI只加现有权限开关/声明预览，复用Material和共享错误。Provider按需运行，不伪装UI、不隐含background.run；旧WIT world/枚举不改。原验收窗口保持，原数据/Unity/桌面控制不动；不推送、不进入M9。

生产依赖审批记录：Host新增jsonschema = =0.61.2，default-features=false，用于2020-12输入/输出和定义校验，不自研第二套Schema解释器。已通过官方rust-v0.61.2 tag核对MIT、MSRV1.85（低于1.97.1），发布2026-11-12。禁用resolve-http/file/async及TLS/idna；同时注入拒绝外部引用的retriever。替代仅手写类型不能支持扩展自定义Schema。新增传递闭包、serde_json float_roundtrip及bytecount/uuid-simd第三方SIMD/unsafe影响须记录最终tree/lock，不称零unsafe，不额外升级无关依赖。Schema校验只在有界Host子进程内，Core不被不可信Schema同步阻塞。锁定依赖闭包和许可检查属于本批实施与验证，不授权其他依赖或推送。

实现和验证按接口依赖顺序进行，未完成接线不发布capability，不将Schema/表存在算功能通过。工作目录main；日常定向检查，稳定候选集中验证，不每次重跑完整Workspace/浏览器/release/三平台。实际结果另记。

### 扩展工具本地接线与稳定候选（2026-11-16）

代码 `54ba42fd47b7228d1a7be08dff9d468a6d31821e` 已由真实Windows用户在main提交，未推送。[实际结果与未覆盖项](../testing/m8-extension-tools-20261116.md)取代上节“待接线”作为当前状态。生产 capability 仅在真实接线验证后开放，hello具名1/0；最终同一代码的签名Provider→冻结SDK→真实Core/Host复验1/0，整轮MCP关联模块7/0另保留输入。真实权限、OAuth上限/撤销、原Plan/实际Principal、原子结果、取消和写响应丢失的恢复已接通；不把guest输出当Core接受证据。

State38仅0038新增，旧迁移未动。Store136/0后仅新增持久重启用例1/0，不写成整轮137项通过；旧迁移测试只补新表为空和现行版本，不削弱原行保全/序列/回滚。Host Schema3/0及真实组件5/0、前端check/build和相关浏览器单轮21/0、受影响严格Clippy/门禁与三程序debug构建通过；无完整Workspace/Chromium/release/三平台重复执行。失败轮与修正依据见测试记录，不拼接为全量通过。

最终依赖审计：精确新增28个锁定包、无旧包移除/升级，完整名称/版本/许可/MSRV及feature树见测试记录和其dependency-audit.json/host-feature-tree.txt。jsonschema feature集为空，HTTP/file/async/TLS/idna resolver关闭；serde_json新增float_roundtrip feature统一。bytecount runtime-dispatch-simd、uuid-simd/vsimd detect以及zerocopy simd包含第三方SIMD/unsafe，不作零unsafe声明；五个传递包未声明MSRV，当前冻结工具链Windows编译已验证。无需其他生产依赖；MCP仅增加三个既有包的测试边。参考组件输入按仓库LF规范化后重建，字节摘要保持d9ac5e89…e8721，provenance/旧WIT精确合同2/0。

831份输入摘要afc5f954…a4f55d1；原data36份、review data10份/profile303份逐文件集合和SHA256保持。测试前原Core/GUI已经关闭，测试后仍关闭，无待恢复进程或本轮Core/Host遗留，Unity40632未动；不替换原程序、不升级原库、不恢复桌面控制。新GUI未部署或人工验收，截图仅浏览器夹具；Provider对运行中任务成功取消的独立原生场景、跨平台/Hosted/CodeQL仍未验证。OAuth仍复用非空consent，零授权纯计算不因此新增OAuth模型。M8保持IN_PROGRESS，不进入M9，不自动推送。

### 移除发布者确认（2026-11-16，State39与公共删除清单已批准）

所有者明确要求：用户选择安装扩展即可安装，不再提供或要求额外的发布者信任确认。移除GUI复选框、信任状态展示、安装输入publisherApproval及PublisherConfirmationRequired错误，不通过自动勾选或偷偷补approve_for_extension模拟删除。安装仍走原Plan/Apply/Operation；申请能力审阅与实际授权不自动扩大，扩展权限、隔离与恢复不变。包仍须原严格Ed25519签名、内容摘要、发布者指纹一致性和路径/ABI/Schema校验；签名用于完整性与来源身份，不将“签名有效”表述为发布者可信。外部调用者不能伪造first_party_packaged来源，保留既有来源限制和留存扩展数据的发布者身份隔离。

所有者已明确批准“彻底删除”，直接实施以下破坏性差异：从安装RPC/DTO删除publisherApproval枚举/字段，从ExtensionRecord/ExtensionPlan及SDK/Portable UI的只读扩展摘要删除trustDecision，清除共享错误合同中的extension_publisher_confirmation_required。不保留旧参数兼容、自动批准字段或第二套安装机制；未知旧参数按既有严格解析拒绝。

已批准的State39最小删除清单：仅删除extensions.trust_decision和extension_plans.trust_decision两列；单事务临时移除extension_plans_immutable触发器，通过SQLite DROP COLUMN去除两列，随后重建触发器，仅去除对应比较。原计划的其余不可变条件、主键/外键/索引、签名/来源摘要、权限/数据、MCP调用接受记录、业务结果及事件序列逐项保留。旧0008/0009/0038等已提交迁移不改，不新增持久表、Config、权限、依赖或身份合同，不升级原运行数据。补State38→39精确删列/全旧值保全/回滚/重开，以及无确认正常安装、坏签名拒绝、必需权限与实际授权仍独立的定向回归。此项不再等待审批，实际结果见后续收束记录。

批准前的独立GUI检查点：移除确认checkbox及三处信任状态展示，frontend-removal-check.log通过；当时未提交/部署、Core确认门禁尚未移除，不作为完整功能证据。后续完整删除和验证在同一target/m8-publisher-confirmation-20261116记录，不自动填approve_for_extension，也不删签名、权限或留存数据的发布者身份保护。

### 2a9d123远端结果及声明预览修复（2026-11-16）

所有者授权后核对干净main、origin与完整12笔链，普通快进36a782c→2a9d12331d01dfde1ca82958e71bb3fc9f7edb10；推送前cargo xtask check/diff通过，不因推送重跑完整测试。自动CI43803776166三平台均failure，CodeQL43803775667四语言全部success，均attempt1，没有dispatch/rerun。失败不是新代码已通过远端：三平台均在m6_lifecycle中repeated_real_host_crashes_quarantine_only_that_enabled_extension的extensions.get处返回revision_conflict，模块各6/1及1原有ignored；之后步骤不能据前缀检查通过记整体通过。原始失败日志ci-43803776166-failed.log保留。

根因是54ba42f增加签名声明预览后对两次完整ExtensionRecord做相等比较，后台崩溃/重启在读期间合法变更revision/runtime。窄修b9f19d5只用包摘要和发布者指纹判断声明来源是否变化，并返回第二次已授权查询的最新状态；不缓存状态、不重试、不放宽包/权限校验，不改原失败用例或公开合同。原具名真实Core/Host回归1/0（7.76秒）、Core所有targets严格Clippy及fmt/diff通过；日志quarantine-read-regression.log/quarantine-clippy.log。未重跑整个模块/Workspace/浏览器/三平台，该修复尚未推送、无对应远端结果。原Core/GUI已关闭，测试只用本轮可丢弃目录，Unity40632未动，未恢复桌面控制。M8保持IN_PROGRESS，发布者确认整改仍按上节独立等待必要删除清单确认。

### 发布者确认删除本地收束（2026-11-16）

所有者已明确批准彻底删除，前节“待确认”为当时历史。代码de8ead33f789c1264df574c69ecae0fb5bf155bd已本地提交，[完整记录](../testing/m8-publisher-confirmation-20261116.md)。当前安装只接受所选有效包，无发布者确认字段、信任状态或自动批准；原Plan/Apply、严格签名/摘要、来源及发布者数据身份、零授权与必需权限门禁保持。State39原子删除两列，触发器仅删除对应比较；旧0001–0038和所有无关数据不改。当前模型无残留TrustDecision类型或批准输入，历史SQL/审计/负向拒绝夹具不作为兼容功能。

新迁移4/0后Store全库单轮141/0，扩展Store13/0、协议24/0、相关合同各自通过；真实Core/Host模块单轮7/0及1原有ignored、Provider具名1/0（70.69秒）；GUI相关单轮26/0、零重试，后仅删除无当前调用者的旧映射/译文，再补check/build通过。受影响严格Clippy/fmt/metadata/xtask/diff及Core/MCP/Host debug构建通过。原CI失败用例包含在本轮Host模块中，不改断言；初次新负向测试的ZIP API编译失败保留，修正后再完整执行。不重复全量Workspace/Chromium/Tauri/release/三平台，不拼成其他模块完整通过。

834份输入汇总789f1440…e20ec1d5；原data36份、review data10份/profile303份逐文件集合和SHA256保持，原环境/review原本关闭并保持关闭，无本轮Core/GUI/Host遗留、未升级原库或部署新GUI。最终原Unity40632已不再观测到、原因未定；本轮没有Unity启停指令，也不自动恢复。数据保全通过与进程集合变化分别记录，不写成原进程保持。b9f19d5与本次代码仍未推送、无对应远端结果；2a9d123的CI失败/CodeQL成功仍只对应原候选。M8保持IN_PROGRESS，不进入M9，不恢复桌面控制。

### e99cd2c四笔推送与远端收集（2026-11-16）

所有者明确批准四笔：b9f19d57291c69db6308a06f11985c435b33f7c1、9c6f3291c98cbb9d1126e804e09be24945ff6983、de8ead33f789c1264df574c69ecae0fb5bf155bd、e99cd2c9d9254025eb8926c23ea2976f599e99fd。已逐项核对精确提交链、干净main、最新远端2a9d123及祖先关系，执行普通快进推送并通过ls-remote确认目标SHA。推送前cargo xtask check/diff通过，不为推送重复既有完整本地验证。无无关提交、强推、标签或发布。

自动CI43816630421与CodeQL43816630482均head=e99cd2c、attempt1，运行路径分别ci.yml及dynamic/github-code-scanning/codeql；终态已收齐：CI failure，Ubuntu Chromium709/1、Windows710/0（18.3分钟）、macOS710/0（23.2分钟）；Windows/macOS整个job成功，Ubuntu仅浏览器步骤失败，三平台Check/release/Tauri及锁文件/diff均成功。CodeQL四语言全部success。只读监视保存完整状态/step JSON于target/m8-push-e99cd2c-20261116，不dispatch或rerun旧运行。前节“未推送”为历史，当前结果不由2a9d123替代；M8保持IN_PROGRESS，不操作原环境或开始下一切片。


Ubuntu完整Chromium709/1的唯一失败为备份压缩下拉框英文用例，选择level7后值仍fast。测试未同步Material select展开/关闭生命周期；窄修`882eb3d99474158a7440dc8050ff3a5c8f64e276`复用已有opened/closed同步，原交互/标签/值/RPC/精确写入断言、超时和生产代码保持。原例本机6倍CPU四次4/0，未复现远端失败，不声称已捕获远端确切事件顺序；修复后相关两模块单轮19/0、英文同条件4/0、前端check通过。新提交仅本地、尚无对应远端结果，需另获推送授权；macOS当前完整710/0不能替代Ubuntu失败。详细运行与日志见本轮发布者确认验收记录。


最终收集使用同SHA、attempt1的实际日志与metadata，未重跑workflow；ci-final-metadata.json、codeql-final-metadata.json及三个平台job.log保存于本轮target证据目录。锁定metadata、cargo xtask check及diff检查通过，未因状态记录重跑完整测试或构建。仅上述四笔已推送，新修复882eb3d及本轮结果记录均留在本地，待所有者另行批准普通快进；M8仍IN_PROGRESS。


### 3752c08两笔推送与后续定向收尾（2026-11-16）

所有者另行批准882eb3d99474158a7440dc8050ff3a5c8f64e276、3752c0870574536524d966c975e5c831a656f9ed。真实Windows用户在标准目录/main核对干净工作树、最新origin/main=e99cd2c、精确两笔链及祖先关系后，普通快进至3752c08并以ls-remote确认；未夹带、强推或手动dispatch/rerun。复用既有有效本地证据，不为推送重复完整测试。

自动CI43837500362及CodeQL43837499791均绑定3752c08/attempt1。终态：Ubuntu success/Chromium710/0（12.3分钟），Windows success/710/0（18.4分钟），macOS failure/709/1（26.1分钟）；三平台Check、CLI自动启动、release/Tauri及锁文件/diff通过，CodeQL四语言success。前轮备份压缩选择已在本轮三平台通过；本轮唯一失败为m7-package-workspace.spec.ts:301用户包来源选择后仍显示Multiple sources，不替换或拼接历史失败结果。

核对现有调用和Material生命周期后，测试窄修56eed3fd9d4cdbd0077d86b3c8bde6cc170277b4等待opened/closed，原点击/显示/Plan审阅断言及超时不变，生产代码未改。原例CPU6倍四次4/0未复现远端失败，明确不声称已捕获远端时序；修复后整个相关模块单轮29/0、单worker CPU6倍四次4/0、前端check及locked metadata/xtask/diff通过。初次诊断grep经npm.cmd转义后未发现测试，保留该未执行记录，修正调用后才有4/0；诊断钩子已清理。新修复及结果记录仅本地、需新推送授权，当前远端仍为CI失败，不宣布M8完成。完整日志和终态JSON保留于target/m8-push-3752c08-20261116，原环境、数据和桌面控制未操作。


### e371d27两笔推送与事件证据初始化（2026-11-16）

所有者批准56eed3fd9d4cdbd0077d86b3c8bde6cc170277b4、e371d27f961404a6606700ed87e4c01a8cfb6c6c后，已核对真实Windows用户、干净main、最新origin/main=3752c08、精确两笔链/祖先关系，普通快进至e371d27并以ls-remote确认。无夹带、强推、标签/发布或手动dispatch/rerun，不为推送重复完整本地验证。

同SHA/attempt1的CI43851129715终态failure：Ubuntu完整Chromium710/0（17.5分钟）、macOS710/0（26.5分钟），两job成功；Windows709/1（19.0分钟）。三平台Check、CLI自动启动、release/Tauri及锁文件/diff均success；CodeQL43851129372四语言success。前两轮两个选择用例在本轮三平台均通过，但旧失败仍保留；当前唯一失败为Windows m7-event-limits.spec.ts:63的5000行缓存测试，其初始化助手在eventLimits未定义时解引用coordinator，尚未进入缓存断言。

已沿harness确认标题先渲染、再void import证据模块，旧expect.poll回调异常不会等待模块。测试窄修6ffde60599087e895cdaec86f5c2bfac9957e416使缺失的全局返回未就绪，仍精确要求ready后暂停时钟，不吞业务异常或放宽超时。新增受控模块延迟回归，在首次读取发生后才释放模块；旧逻辑下单轮1失败、相同TypeError，修复后该模块单轮10/0（9原例+1新例）、前端check及locked metadata/xtask/diff通过。所有缓存/限流/任务等原断言保持，生产代码和harness未改；不重复完整Workspace/Chromium/release。

原始各平台日志、终态metadata与确定性前后回归见target/m8-push-e371d27-20261116；新修复/结果记录仅本地、需新的明确推送授权，不把10/0写成远端通过。原Core/GUI、原数据、Unity及桌面控制均未操作，M8仍IN_PROGRESS，不进入M9。


### dd5aa9b两笔推送与仓库刷新焦点（2026-11-16）

所有者批准6ffde60599087e895cdaec86f5c2bfac9957e416、dd5aa9bae0a4b04ff4f3dd51b1433a240f0be7cf，核对真实Windows用户/干净main/最新origin及精确两笔链和祖先关系后，普通快进e371d27→dd5aa9b并ls-remote确认。未夹带、强推、标签/发布或手动dispatch/rerun，不重复既有完整本地验证。

同SHA/attempt1的CI43865931992终态failure：Ubuntu完整Chromium711/0（14.8分钟）、Windows711/0（13.9分钟），两job成功；macOS710/1（20.4分钟）。三平台Check、CLI自动启动、release/Tauri及锁文件/diff均success；CodeQL43865930193四语言success。此前事件初始化/压缩/来源选择用例均在本轮三平台通过，旧失败保留，不拼接成整体通过。

macOS唯一失败位于仓库详情返回包列表后的原toBeFocused断言。受控挂起既有刷新RPC，先返回并恢复焦点，再释放刷新，旧分页DOM被替换后新按钮失焦：1失败/1通过，确认是真实产品竞态，非仅断言过早。生产窄修d667a012e601b729d220b4527cdc6948f7a0e1a0把局部焦点意图放到Catalog的分页会话外，移除旧列表前只捕获实际focused包名，待新Material控件就绪恢复；用户已转向其他控件则不抢焦点。缺失目标清理意图，不改通用PagedItems、Material、布局或键盘合同，不依赖DOM移除时blur事件差异（[MDN](https://developer.mozilla.org/en-US/docs/Web/API/Element/blur_event)）。无RPC/Schema/权限/依赖变化。

仓库模块单轮35/0、2.8分钟、1 worker，含两新增受控回归和原失败例；前端check/build通过，JS产物90adae98…7c75443、CSS480fdffa…9d4725b，完整输入/产物SHA256见target/m8-push-dd5aa9b-20261116/repository-focus-inputs-and-build.json。locked metadata/xtask/diff通过，不重复全量Rust/Chromium/Tauri release。实际失败日志、受控前后结果与本地headless截图/trace分别保存，不冒充原生桌面证据。新修复和结果记录只本地，远端/原生复验未执行，仍需新的明确推送授权；原环境及桌面控制未操作，M8仍IN_PROGRESS。


### e10bce7远端结果与权限读回、动画诊断（2026-11-17）

所有者批准d667a012e601b729d220b4527cdc6948f7a0e1a0、e10bce7b67e77ef76e2f7db9a7dce2b41f22c4fe两笔后，已核对真实Windows用户、干净main、最新origin及精确链/祖先关系，普通快进dd5aa9b→e10bce7并以ls-remote确认，无夹带、强推、标签/发布或手动dispatch/rerun，复用既有本地有效验证。

同SHA/attempt1：CI43885504578终态failure，Ubuntu完整Chromium713/0（17.3分钟）、macOS712/1（26.5分钟）、Windows710/3（15.3分钟）；三平台Check、CLI自动启动、release/Tauri及锁文件/diff均success。CodeQL43885504395四语言success。仓库焦点原例和两新增回归三平台通过，但不拼成整套通过；Mac失败为权限开关错误提示未出现，Windows同例、项目读取开关连续操作及动画系统偏好传播失败，原日志分别保留。gh run view端点EOF后直接官方API核验同SHA/attempt并继续读取，未重跑workflow。

权限本地修复f1f1027819b78558d9ddc8e82863a1ccf99a8473：ExtensionPermissions把快照绑定到当前owner/generation，新的归属读取完成前不展示旧授权为可编辑；保留当前Core与readVersion检查、未知写结果冻结、所有分页/revision/权限与声明规则。受控旧行为暴露可编辑窗口，单例1失败；新回归在最终四模块中通过。原测试把Material原生乐观checked当成Core提交完成，请求诊断失败时仅记录两次setGrant、没有revoke；成功操作现在控制现有模拟写入和读回边界，仍由真实键盘/点击驱动，不删原状态/错误/精确RPC/revision断言，不添加生产旁路或通用框架。测试辅助的初版挂起时机与一处RPC参数错误均留存为实施失败，不能算Core失败或通过。

动画088710612a7bb37fdaac7a759c3b626a87bdc586只给现有live-OS测试增加原生matchMedia状态和change事件输入见证及7字段有界失败记录，原quick/页面状态/写入断言、超时与零重试不变；生产motion.ts及依赖未改。原Windows失败本机CPU6倍四次4/0未复现，**真实原因仍未知，不标记修复或豁免**。新本地输入四相关模块单轮37/0（1.1分钟）、前端check/build、metadata/xtask/diff通过，不重复完整Rust/Chromium/Tauri release。JS829aca43…414b25df、CSS480fdffa…9d4725b与源码摘要绑定于target/m8-push-e10bce7-20261116/permissions-motion-inputs-and-build.json。新候选需远端核对权限修复并取得原生输入证据；未新增RPC/Schema/权限/依赖或修改workflow，未自动推送，原环境与桌面控制未操作，M8仍IN_PROGRESS。


### 6bbf876推送、终态与读回布局窄修（2026-11-17）

所有者明确批准f1f1027819b78558d9ddc8e82863a1ccf99a8473、088710612a7bb37fdaac7a759c3b626a87bdc586、6bbf876fb5b2c413eaa50fba0f7fde1dcc3258e2后，真实Windows用户核对干净main、精确完整三笔链、最新origin与祖先关系，普通快进e10bce7→6bbf876并ls-remote确认。CI43915767271同SHA/attempt1终态failure：Windows完整Chromium714/0（18.0分钟）、job成功；Ubuntu713/1（15.6分钟），项目读取开关第二次点击没有撤销RPC；两平台此前Check、CLI自动启动、release/Tauri及锁文件/diff均success。macOS未取得托管arm64 runner，官方容量/未获取注解、空runner与零步骤证据确认其取消且未执行。CodeQL43915766843四语言success，未dispatch/rerun，不以旧结果代替未覆盖平台。Windows动画用例本轮通过，不据此宣称上轮未知根因已修复。

沿唯一代码失败定向定位：CPU6倍四次4/0未复现；原速带有界输入诊断6通过/2失败，第二次只有pointerdown、没有pointerup/change或撤销RPC。读回时移除授权行使弹窗收缩/重排，trace中点击目标纵坐标由359.5变为458；受控读回布局回归在旧代码1失败。225726bc4ebb13156688d6b58588f19cadc29d1a在同Core/同扩展重读时保留禁用行及位置，仍以当前归属读回为写前提，不将上一代数据当成可写权威；不同身份不可见旧快照，读取能力变更也使旧归属失效。未知写结果、错误、分页/revision与精确RPC断言保持；测试辅助由行移除断言改为相同行数全部禁用，并新增位置稳定/一次撤销断言，不屏蔽后台事件、不重复点击、不加重试或延长超时。未修改共享Material、Core/RPC/Schema/权限/依赖/workflow。

相关三模块单轮29/0、45.5秒，原失败例原速8/0、21.8秒；前端check/build（372模块、1286条三语言）、xtask/diff通过，locked metadata复用同依赖输入；不重跑完整Rust/Chromium/Tauri release。最终仅澄清一行注释，无执行逻辑差异。JS8e8752d3…6f7c19f1、CSS480fdffa…9d4725b，完整输入/构建摘要见target/m8-push-6bbf876-20261117/permission-layout-inputs-and-build.json，原失败日志/trace及各轮结果分别保留。新代码和记录仅本地，需新的明确推送授权，无对应远端/原生桌面证据；原环境与桌面控制未操作，M8仍IN_PROGRESS。


### 下一批：实时 Core 观测最小合同（2026-11-17，已批准、实施中）

本轮最初让717d3e0的自动CI44019052791/CodeQL44019052564继续运行，现已收齐：CI三平台各714/1（同一新增布局测试基准），CodeQL四语言通过；本地cd9d009修复测试基准，尚无远端验证。独立审计M8剩余能力，不重开身份/OAuth/工具或M7，不进入M9。实施前审计：`apps/alcomd/src/lib.rs::ConnectionState/serve_connection`已有权威connectionId、认证后MCP Principal及逐次dispatch；`apps/alcomd-mcp/src/lib.rs`的STDIO保持Core连接，`http.rs`每个认证请求独立连接Core。没有实时注册表/读取RPC，客户端list只是持久登记；Host仅有clients/grants等管理投影。架构16节列出的观测权限目前也未进入可授权枚举/State39约束。不得把未撤销客户端称为在线，也不能从这条Core链推断HTTP空闲TCP、SDK等待队列或协议订阅。

推荐先完成真实、只读的**Core接入/调用观测**，明确与完整MCP传输遥测区分；首片不新增适配器自报、内部凭据或第二套通信，也不增加可取消/关闭连接的管理写接口。

| 公共差异 | 准确范围 |
| --- | --- |
| 新RPC `mcp.runtime.snapshot({clientId?,limit?})` / capability `mcp.runtime.read.v1` | 返回Core instanceId、observedAtMs、canReadConnections/canReadRequests及分别获准的连接/在途Core调用投影。无任一读取权限则既有permission_denied。limit默认25、1–100，分别限制两组；只读当前快照，不新增持久查询会话/历史。连接行含connectionId、已认证clientId、connectedAtMs、activeCallCount；调用行含Core生成callId、connectionId、clientId、固定RPC method、startedAtMs。无请求参数、响应、原始JSON-RPC id、token/hash、headers、路径或外部URL。Core方法名来自真实dispatcher的静态方法集合，非客户端文本。 |
| 两项权限 `mcp.connections.read` / `mcp.requests.read` | 从架构既有分类落实到当前公开枚举及Global/*授权；默认不授予，不隐含mcp.clients.read/manage。只供本机管理Client/扩展路径使用，本轮不加入外部MCP八项业务grant或OAuth scope，不扩大身份或业务访问。读行中的clientId不返回名称；名称只有另经已有clients.read读取后在视图中匹配。 |
| 独立Host接口 `alcomd:mcp-runtime/observations@1.0.0` | 仅snapshot只读投影，字段/64KiB预算与Core一致；现有clients接口和extension-v1 ABI不改，新增world组合现有接口。复用交互Client＋Extension双权限、当前lease/UI/声明和撤销校验，返回canRead标记只控制展示，查询仍逐次权威检查。 |
| State39→40最小增量 | 仅单事务重建extension_grants的CHECK，增加上述两项Global/*组合；保留所有旧列、主键/外键、记录、revision及触发器语义。旧迁移不改，其他表/Config13不变，不建观测数据库表，不删除/扩大旧授权，不升级原运行数据。 |

**真实语义与边界**：Core在认证完成后登记，在dispatch进入/退出和连接销毁时维护RAII在内存中的投影。认证失败或OAuth端点专用身份不计入MCP业务连接，clientInfo不能提供clientId。连接行只表示该已认证Core IPC仍存在，不证明凭据/授权现在仍有效；当前业务鉴权照常逐请求复查，撤销不被观测替代。HTTP以当前请求的Core桥计数，不声称协议session或空闲HTTP连接在线。已接受Operation继续按原规则运行；Core调用结束、断线或观测关闭不取消它。暂不记录完成历史，不把持续Operation显示为持续RPC。

实时表最多跟踪4096条连接及其在途调用，超过跟踪预算只标记coverageIncomplete并给出明确的不完整状态，不拒绝既有业务、不静默假装全量；每组返回totalTracked与truncated，按连接/调用开始时间及Core生成ID稳定排序。每次查询产生独立当前快照；不把跨刷新结果拼成同一快照或制造revision游标。Core重启清空并更换instanceId，GUI据此丢弃旧行；观测错误显示不可用，不能显示为0活动。未实现的MCP subscription-streams继续不广告，页面明确不支持，不返回伪造空订阅表或新增session模型。

**界面**：现有管理扩展页内增加简洁“活动”分段，使用现有Portable UI list/status和刷新动作，无一级导航/常驻技术卡。权限各一个可选能力开关，管理功能仍零授权可打开；不开启观测读取不展示活动。请求表标注“Core调用”，不冒称全部MCP请求；无权限的一组不显示。加载/失败/重试沿用共享反馈和当前会话失效规则。正式MCP请求/HTTP socket/SDK队列的完整传输层遥测仍是未覆盖项，不以本片宣布M8完成。

**定向验证**：两个真实MCP Principal的Core连接/调用正确归属、在途/完成/异常断开/重启清理；认证失败及伪clientInfo不产生他人行；无授权/单项授权/双方任一撤销/lease失效拒绝；RPC参数/秘密不进入投影、日志或Portable UI；超限完整性标记；Operation接受后断线仍继续；旧guest/new Host兼容；State40保全/外键/回滚；扩展页加载失败/刷新/迟到响应、两项权限开关。复用既有身份/OAuth/Operation证据，不重跑完整套件。

所有者于2026-11-17明确批准“Core只读观测首片”，直接实施上述RPC/权限/Host/State40及定向验证，不再次审批同一范围。批准前仅完成审计和人工入口准备；不重复审批既有身份、HTTP/OAuth或扩展工具，不授权推送。


#### 本地实现与定向证据（2026-11-17）

Core内存注册表已接真实认证完成、dispatcher及RAII清理；公开RPC、typed client、SDK、双权限Store检查、独立Host/WIT、可选权限开关和现有扩展“活动”分段接通。真实Core/签名组件通过后才开放capability。State40只放宽获批两项Global/*的CHECK；Config13、依赖锁、旧迁移、MCP八项业务grant及OAuth scope不变。

两客户端实际IPC/静态Core调用归属、伪clientInfo、错误凭据、零业务授权拒绝、Extension/Client分别缺权限、独立读组、授权撤销/旧UI失效、查询过滤/截断、断线清理与重启持久身份/空观测已验证。新2项与旧guest兼容2项单轮4/0，日志runtime-real-core-final-outputs-separated.log。之前两轮单项0/1分别为测试清单权限未排序、受限测试调用者漏配既有extensions.read，修正夹具后单项1/0；合并首轮3/1的唯一失败为旧样例共用create-new导出路径，正确拒绝已有文件，改独立输出后4/0。原记录不改写。

模型4/0、DTO1/0、State40保全/外键/回滚/重开3/0、Store管理关联4/0、Host实际投影与两份Schema1/0、调用种类矩阵1/0、冻结ABI/双组件来源合同3/0、当前Schema关联9/0分别通过。guest7/0及doc-test1/0、严格Clippy通过；GUI前端检查/构建通过（1290条三语言）。权限浏览器模块单轮6/1：1项在browserContext.newPage空白页创建时Target crashed，尚未进入业务用例，根因未定，原trace保留；精确行号单项复验1/0，不拼成模块通过。一次带首锚正则未匹配完整Playwright标题，记未执行，未修改用例或断言。

其后仅做trait impl移至测试模块前、测试expect_err上下文及仓库LF规范化；旧组件字节不改，新WIT规范化后真实重建的组件也逐字节不变（e6d80999cb6da6984953317d292c29b1c115e46127a4e1a9ba7b56acc1dad46c）。旧来源两个文件以原摘要冻结在测试夹具，原来源断言未删除；新组件另记当前来源。严格检查曾发现嵌套条件/测试模块排布/unwrap_err，按原门禁修正，不增加allow。匹配程序debug构建和受影响严格Clippy已通过；最终LF迁移/来源合同各3/0。代码96c5322与诊断2c4f2a1均仅本地，不以此声明远端或人工验收通过。

证据目录target/m8-push-717d3e0-20261117，真实129500字节新样例位于target/m8-runtime-observations-20261117/management-runtime-verified.alcomdextension，已核对组件、排序声明、签名及Core安装/启用/读取。复制旧review数据库的717入口保留Core-only历史证据并禁止GUI；新入口使用全新数据经正式Core登记/安装，不直接改数据库路径。新窗口现已实际打开并保留，GUI88248的5条真实管道指向隔离Core83476；State40、全新数据内实际包位置及零授权已只读核对，原数据36份、profile346份及旧review10/303份最终集合/摘要全部保持。

本片覆盖Core IPC/在途Core RPC，不覆盖全部MCP协议请求、HTTP空闲连接、SDK队列或订阅。已接受Operation的调度/取消代码未改变，沿用既有真实业务闭环证据；本片未新增任务取消或关闭连接能力。OAuth实际客户端人工接入、正式默认分发等既有未完成项继续保留，M8仍进行中、不进入M9，不自动推送。


96c5322最终同提交四项复验为2通过/2失败：两个旧guest在extension_ui_open返回extension_resource_limit，两项新观测用例通过。新增固定字段分类诊断后，旧单项1/0（UI打开1–2ms、约2s余量、fuel充足），原组合单轮4/0（63.76秒）；**原失败未复现，具体超限阶段/根因未知，不记修复、不标豁免**。2c4f2a1只保留失败时静态导出名/耗时/fuel和固定trap分类，测试特性转发Host stderr；生产Core仍关闭该私有stderr，不记录输入、响应或秘密。全部RuntimeLimits、原断言和超时不变，两个受影响包严格Clippy通过。日志runtime-96c5322-real-core.log、runtime-old-component-limit-diagnostic.log、runtime-combined-limit-diagnostic.log和runtime-failure-diagnostics-clippy.log独立保留。该问题是未解决的验证风险，不能由后一次通过抹去。

匹配四程序正常debug构建、诊断增量构建及GUI debug构建分别保留日志，GUI/CLI/MCP未变输入与已有结果复用；最终非docs候选922份输入与程序摘要记录于runtime-final-build-inputs.json、review/candidate.json。新包SHA256 b921e7847ae4adbe5df6de855e714e68588f53ba332ac91a83a2bb132a154419，组件仍e6d80999…1dad46c。当前准备初次停在脚本错误的Operations游标判断：唯一install已succeeded、activeCount0，nextCursor是续读位置而非hasMore或活动任务；修正脚本后仅只读续接、未重复登记/安装。准备Core正常空闲退出后，以原已批准流程重开同一隔离数据，随后GUI成功连接，已实际验证分段启动；新Open-Review封装的GUI分支仍仅语法检查，不冒称完整入口已执行。当前窗口供人工查看，不重新验收M7，不自行签收M8。

最终保全记录review/final-preservation-and-window.json确认922份非docs输入未变、四组原数据/旧review摘要保持；两个启动保活助手已自然退出，真实GUI仍可见/响应并保留5条Core管道。未使用桌面控制，未推送，本片不宣告M8完成。

## 扩展权限目录与安装同意（2026-11-17，已批准实施中）

所有者已批准本轮完整方案：全部12项扩展可授权权限均可声明为必需或可选；管理页按已声明/未声明两组显示并允许授权。移除mcp.tools.provide仅可选特例，不新增权限，不恢复逐项目授权。包可附带签名的纯文本installation_terms（title最多128字符，text最多16KiB）；同一冻结Plan先确认条款，再集中确认必需权限，空步骤跳过，拒绝不提交安装。Apply安装使用独立参数和绑定planFingerprint的consent（installationTermsAccepted/requiredPermissionsAccepted），必需权限额外要求extensions.permissions.manage，在安装状态提交事务中登记；可选/未声明默认关闭，恢复重放不重新授予撤销权限。extensions.permissions.list增加availablePermissions；State41仅扩展冻结Plan条款证据与不可变触发器，同意复用原Operation请求和幂等证据，不删除历史/补旧授权。旧未提交计划不能沿用旧Apply参数，须按新合同审阅确认。同步Manifest/RPC/SDK/威胁模型与定向回归；不升级原运行数据，不自动推送，不恢复桌面控制。此项改变原安装及声明约束，是发布前公共合同破坏性调整；未保留旧安装参数旁路。

本轮接线与定向验证已完成：真实签名包→Core→Host同意闭环1/0、原生命周期1/0；Core调用者授权/读回1/0；Store扩展模块最终16/0（首轮6/7为旧前置与新授权语义冲突，保留），State41三项3/0、受影响State39四项4/0与State40三项3/0、扩展事件四项4/0；扩展包/签名模块18/0，RPC对应两项各1/0。浏览器受影响三模块首轮26/4（四项旧开关数断言），数量按完整12项目录修正后的权限模块7/0；最后稳定输入三模块独立单轮30/0、零重试，不拼接轮次。前端check/build、受影响全部targets严格Clippy及新增Store测试Clippy、fmt/metadata/xtask/diff通过；未重跑完整Workspace、Chromium、release或远端。新增代码未推送、无对应Hosted/CodeQL结果，M8人工待验。匹配程序与新隔离入口正在准备，不将旧窗口当作本轮GUI已部署。

候选`1c47a1793e92394bcc35fcfb01754fe6167447d6`已由真实Windows用户本地提交，工作树代码稳定；匹配Core/CLI/MCP/Host及嵌入前端GUI的debug构建完成。新State41隔离窗口GUI90176实际连接Core87756（5条管道），保活退出后仍可见/响应，当前Core对1.0.1签名样例条款和必需权限预览成功，尚未Apply，留给所有者操作。全新环境0安装/0授权/0任务，原四组目录摘要保持；原GUI88248/Core83476仅在核对0任务后关闭。实际已执行新完整启动入口，不宣称原生流程已人工签收。样例路径、ZIP/content-tree/组件摘要、845份输入及各轮失败和通过见现有人工记录。未推送，无对应Hosted/CodeQL；M8继续进行中。

2026-11-17 所有者纠正权限管理分组：用“必需权限”“可选权限”“其他权限”三类替代先前已声明/未声明两组，其他权限指当前可授权目录中未声明的项。仅GUI呈现调整，去除逐行重复类型标记，空组显示“无”；安装同意、授权、RPC/State41/WIT均不变。相关浏览器单轮19/0、前端check通过，原Core验证继续按1c47a17及原输入复用，GUI候选更新中，不以旧二分类窗口冒充新版。

三分类GUI已以e58c0d58bdc259a70482c7f8bc22fc1a0c7b5a38本地提交并完成前端check/build及仅GUI的嵌入资源debug构建。新窗口GUI84624实际连接隔离Core83680（仍用1c47a17程序/同一data/profile），5条管道、可见/响应；前后安装和授权完整行摘要相同。旧GUI覆盖文件锁失败、保活结束导致原隔离Core空闲退出的过程保留，现已按原程序/数据恢复，不改写为未重启。无新远端或人工签收，不重跑Core/迁移或全量门禁。

## 单扩展源在线安装（2026-11-17，完整方案已批准实施）

一个匿名HTTPS地址返回alcomd-extension-source-v1 JSON：packages恰好一个扩展ID，其下最多256版本（索引实际读取最多1MiB），每版本声明apiMajor/url/zipSHA256。Core下载现有签名.alcomdextension文件（64MiB），复验文件摘要、包签名、ID/版本/发布者及原权限接口；不把清单声明当授权或条款。新增extensions.inspectSource({url})与extensions.planInstallFromSource({url,version,expectedSourceFingerprint})、extensions.remote-install.v1、remote_extension_source来源及State42冻结来源证据；只允许认证local-owner＋extensions.manage，不新增权限或WIT。元数据变化revision_conflict，不静默换版本/重规划。安装同意、Apply、Operation/幂等/恢复及三类权限沿用，只有最后确认后安装；取消准备忽略迟到结果，不自动Apply或声称回滚。

复用既有VPM适配代码中的HTTP、代理、限额和原子缓存，通过内部接口注入扩展引擎；扩展请求使用独立匿名HTTPS重定向策略，禁降级/userinfo/Referer/认证headers，查询参数保留且不记录敏感URL，不增加生产依赖。GUI同一安装入口本地/在线分段，不加仓库管理、未安装条目、更新或云服务。State42重建Plan来源CHECK并保全全部列/记录/外键/不可变条件，旧迁移和原运行数据不变。范围内本地提交获准，不推送、不进入M9、不恢复桌面控制。定向验证、HTTPS真实Core闭环及当前文档同步完成后交付。


### 在线来源本地接线与证据（2026-11-17）

已实现匿名HTTPS适配器→扩展引擎→application→Store/daemon→typed client/SDK→同一GUI安装弹窗。只支持单扩展多版本的新安装，来源变化先拒绝，再由用户重读；签名条款、必需权限、Plan/Apply及Operation沿用，不自动升级/启用或恢复发布者信任。State42仅重建计划来源CHECK并保存不可变来源，签名及权限声明仍取自包。

没有新增生产依赖或包版本。为复用已实现的HTTPS客户端与缓存，在MCP既有真实Core测试模块新增一条alcomd-vpm测试依赖边；Cargo.lock只多这一条边。TLS夹具复用原365天公开测试证书，显式CA只在test-kill-gates测试构造器，不写系统信任，不放宽TLS，不新增生产参数或环境变量。新SDK对外类型与Schema同步，普通错误不显示原始响应或敏感URL。

来源解析最终5/0，含严格重复键/未知字段/单ID/数量/字节/哈希/SemVer/空可选描述及完整query指纹；Schema/DTO1/0，既有M6合同5/0；State42首轮1/2定位SQLite重命名时外部日志触发器短暂缺表，修正连接局部legacy rename后3/0，最终新增重命名模式恢复断言的3/0；六项真实initializer升级/重开6/0。所有历史迁移保留。包/签名共用模块本轮9/0；缓存模块10/0、3项既有外部代理/认证夹具ignored，不能记全部覆盖。独立HTTPS URL规则1/0。

新来源浏览器模块首轮6/0；稳定前端两相关模块单轮26/0、零重试，覆盖原本地路径、条款/必需权限、拒绝/空步骤、未知Apply不重试、权限变更，以及在线版本、清单变化、下载失败重试、长文本窄窗、取消/关闭动画/Esc迟到Plan不Apply。前端check/build和native桥检查通过；没有完整Chromium或release构建。

真实HTTPS→typed RPC→Core→签名包→安装→幂等重放/重启来源的初轮1/0；精确代码候选3d6c2a1b的最终补充用例独立1/0，新增十次重定向可用/第十一次拒绝、真实外部Principal在联网前拒绝及无Authorization/Referer最终读回。原本地生命周期同候选另1/0，不拼接为完整Core套件通过。测试在全新State42数据中执行。原State41自有窗口先确认0活动任务及精确归属才暂停；首轮结束以原程序/数据/profile恢复，五组公开状态完全一致，真实5条管道已核对。原四组数据/profile（36/346/10/303份）摘要保持，Unity及原开发程序未操作，不恢复桌面输入。

前端最初翻译表逗号、确定性假客户端未同步的类型错误和新增Schema用例误放无校验依赖的包均保留为开发检查失败，修正后通过；Schema验证移至已有jsonschema依赖的Host测试入口，没有为测试增加新包。严格Clippy最初指出测试宏连续if及辅助函数置于测试模块后，按原门禁调整，无allow/断言/超时放宽。最终候选SHA、匹配程序摘要、最终真实用例和人工入口同步在现有状态/人工记录；未推送，没有本批Hosted CI/CodeQL，不以717d3e0的失败CI及成功CodeQL替代，M8仍进行中。


最终本地候选为3d6c2a1b80a124e1c16794b4a303ac0d534791c7，937份非docs输入及五程序摘要已绑定；最后全部受影响targets严格Clippy、fmt/metadata/xtask/diff通过。正常debug Core/CLI/MCP/Host及最终dist嵌入GUI已构建，未运行release。新独立查看窗口GUI95592/Core47824、5条实际管道已核对，State42、0活动任务，自动发现关闭且仅隔离存储路径。旧State41数据和程序保留，原环境/Unity和四组原摘要保持；新GUI入口已实际运行，不只是语法检查。原生成功在线安装尚待普通受信任HTTPS源人工查看，测试HTTPS端点已关闭，没有把测试CA装入系统。样例本地包真实可用，完整路径/短清单及各失败轮次留在原人工记录。仅本地提交，无本批远端验证，不宣布M8完成。

启动助手退出后再次被动核对GUI95592自身仍有5条连接指向Core47824，可见/响应；原四组36/346/10/303份最终摘要再次完全匹配。最终只整理三份文档，以locked metadata与diff检查收尾，不为状态更新再跑测试/构建。


### 所有者反馈：在线优先的安装入口（2026-11-17）

按最新指示，工具栏复用SplitButton：主按钮“安装扩展”直接打开在线网址，右箭头菜单才有本地文件。删除弹窗内部来源分段。原整行“读取清单”来自纵向表单的stretch和重复操作组合；读取移动到底部唯一主操作，读取后原位变为安装，清单变化后回到读取。字段Enter显式触发当前步骤并保留IME组合输入；不改变Core/RPC/State42、签名、条款、必需权限、取消及未知Apply边界。

修复前新回归0/1，保存完整浏览器窗口；按新入口修正首轮相关模块30/1，唯一失败暴露Material自定义网址框的Enter没有原生表单隐式提交，添加局部键盘接线后四相关模块单轮36/0；原断言/超时未放宽，另弹窗取消/焦点用例1/0，分别保留、不拼接旧轮。首次utility行号315未匹配，后以实际316单项1/0；没有将未命中记通过。三语言/Material/TypeScript及前端build、仅GUI的嵌入资源debug构建、locked metadata/diff通过。Core等不变输入复用3d6c2a1b证据，不重跑Rust、完整Chromium、release或CI。截图为浏览器夹具，非原生截图。仅本地提交、不推送；更新真实隔离窗口后同步现有人工记录，不恢复桌面控制。

GUI窄修已以953ce174614b382762d0c0c0551d911efbcdb5fe本地提交，当前GUI91416通过原data/profile连接原Core47824（Core输入仍3d6c2a1b），5条管道且可见/响应。独立GUI目录避免覆盖运行文件；五组公开状态与安装/授权完整行摘要保持。938份非docs输入及浏览器对比图已绑定；原生入口交给所有者查看，不冒称已在桌面点击或在线安装通过。未推送，M8保持进行中。


2026-11-17 后续用户文案纠正：输入网址后的动作从“读取清单”改为“继续”，获取扩展信息保留为内部实现；后续选择版本与安装确认不变，没有新增步骤或公共合同。移除旧三语言用户文案，现有来源模块单轮8/0及前端check/build、仅GUI debug构建通过。实际GUI85304自身5条管道连接原Core47824，保活已关闭，原data/profile、五组公开状态及安装/授权摘要保持；不恢复桌面控制，不推送，不把旧通过当作这次远端验证。

### 2026-11-17 候选续查与剩余接线边界

所有者批准按既定顺序继续。只读ls-remote确认origin/main仍为`717d3e085b6ce29649633e24c08e732c2e172855`；至代码/测试候选`7aed2b368ad8f5e470afe46dfbb03b50020452d8`共17笔线性本地提交，涵盖先前已批准的观测、安装同意、在线源、GUI整改和相关测试/文档。完整SHA/标题链、程序与测试输入摘要在`target/m8-candidate-audit-20261117/candidate-audit.json`。Cargo.lock相对远端只增加真实HTTPS测试使用的既有alcomd-vpm包依赖边，没有包版本升级。未推送或手动触发workflow；旧CI44019052791失败及CodeQL44019052564成功均只属于717d3e0。

本轮发现一项过时回归：`m7-extension-workspace.spec.ts`仍点击主按钮进入本地安装，并期待已撤销的Create install plan/Publisher/Discard plan审阅。原模块实跑3通过/1失败。7aed2b36只更新该用例，沿用既有右箭头本地入口，核对冻结扩展ID/版本、请求权限、原始路径、拒绝/取消及全程零Apply，不恢复旧信任步骤、不改生产代码或放宽断言。修复后该模块与在线源模块独立单轮12/0；前端check通过。原3/1、历史30/1和其他轮次分别保留，不拼成完整浏览器通过。fmt、xtask、diff通过；本轮没有完整Rust/Chromium、release或三平台重跑。原始输出`target/m8-candidate-browser-{before,final}.log`及`target/m8-candidate-frontend-check.log`。

**resource_limit诊断**：沿真实Core→Host管道→Wasmtime链核对，Host先Ready再编译Component，Core分别约束启动/activate及UI读取；Guest另有fuel/epoch/内存/栈限额。当前application已经在Host准备后签发新的2秒render authority，并继续受原会话上限约束，不能重复实现这个修复。仅凭旧公开错误无法确定失败属于哪一层；冷编译/调度等待只能列为待验证假设。本轮不占Core端点的真实Host旧管理Component具名用例1/0（2.54秒，日志`target/m8-candidate-host-baseline.log`），不是完整Core复验。原96c5322的2/2、后续诊断单项1/0及组合4/0继续分别保留；本轮未复现原失败，不记已修复或豁免，不调整任何限额。既有2c4f2a1a有界失败阶段/预算/trap/fuel诊断保留，后续出现失败时先按这些证据区分来源，不盲目重跑。

| 剩余项 | 已有能力及真实缺口 | 下一步边界 |
| --- | --- | --- |
| 普通可信HTTPS原生在线安装 | 真实TLS/Core自动用例、GUI来源流程和签名包均已通过；没有公开托管且普通候选信任的测试源 | 等待实际源网址；不发布外部资产、不安装测试CA，不用本地安装冒充在线安装 |
| 真实OAuth客户端接入 | 预登记、S256、授权码、刷新/撤销及真实Core/SDK已实现；尚无目标客户端及受信任localhost证书/PKCS8密钥输入 | 使用原serve-oauth和registration/request CLI，明确无daemon自动启动；不猜测任意旧协议客户端兼容、不重做认证 |
| 完整传输观测 | http.rs知道真实TCP/TLS和协议请求；Core snapshot只知道认证IPC/在途RPC，两个集合不能互相替代 | 需另审适配器到Core的有界观测通道及身份/可信度边界，再扩展现有只读投影和Host；不能把clientInfo或自报统计作为认证事实 |
| 订阅流 | lib.rs未实现或广告subscriptions/listen；HTTP当前只接受discover/tools/list/tools/call | 需先确定实际订阅过滤器/通知来源、取消及撤销语义，再审批协议/Host/观测差异；不新增session或假空订阅表 |
| 正式默认分发 | 已有普通签名包、安装Plan/Apply及启停流程；manifest指纹占位，没有受控默认安装接线。外部planInstall还会拒绝冒充first_party_packaged | 复用原安装权威；正式签名身份、包交付和默认安装后的用户停用/卸载保留策略须明确，不能使用公开测试密钥或每次启动重装 |
| 其余业务映射 | 六工具及扩展提供工具已经接通；M-1其他用例/输入审批恢复等不是已部署合同，Core也无通用input/approve/reject/resume RPC | 按实际可用用例另列最小合同；不把旧Draft当授权，不夹入这轮安装/接入验证 |

本轮修正管理扩展README的过时“尚无实时连接/请求”和“两项可选权限”描述，使其符合已实现的Core观测及四项可选声明；不改接口或manifest。上述新增合同仍需具体审批，未直接实施或广告。真实接入缺少外部输入已单独询问，不因此停止独立审计。

只读再次核对当前GUI85304可见/响应，以自身5条管道连Core47824，review/data和profile不变，五程序摘要均匹配原candidate.json。原data/profile及旧review的36/346/10/303份文件摘要与既有快照完全一致。本轮没有暂停或替换Core/GUI、写真实项目、修改系统信任或恢复桌面输入。程序仍为Core3d6c2a1b/GUI fb72f0aa，7aed2b36仅测试变化，不为此重建程序。M8继续IN_PROGRESS，未宣称人工/远端验收通过，不进入M9。

### 2026-11-17 实际Codex客户端接入与扩展测试顺序调整

所有者明确先测MCP，扩展相关人工测试留到后续实际开发扩展时；保留未覆盖项，不记通过/豁免，不将“后续”自行归入M9或M12。扩展源网址不再作为本轮MCP验证的前置要求。原用户localhost开发证书存在但Windows仍报UntrustedRoot，未导出其私钥、安装系统信任或修改日常配置。

复用本机Codex CLI0.153.4（exe SHA256 `444a3f0008050605cae73cd9b7a2dcac61294062dfaab56dd20430fd6498518b`），在target下新建一次性CODEX_HOME，analytics关闭、模型端点仅指未监听的回环地址且未发出推理请求。通过其实际app-server MCP工具入口调用，不是假SDK或模拟结果；ALCOMD身份/审批/登记仍走原alcomd-client typed方法，私有驱动不做任意RPC透传。凭据只经匿名管道/内存传递；OAuth明确使用一次性home内的file store，未写系统keyring。没有新增生产依赖/合同/权限或Core修复。Core/MCP程序来源3d6c2a1b，至本轮父提交611436c4的相关Core/client/protocol/store/MCP运行代码完全无差异，沿用现有程序及输入证据。

| 独立实际场景 | 真实结果与边界 | 原始记录（target/m8-codex-client-20261117下） |
| --- | --- | --- |
| Codex现代回环HTTP Bearer | 最终场景13项检查通过：0授权连接/空目录；明确授权后新线程发现六工具，项目和实际包快照读取、缺包真实失败、原卸载Plan无文件修改、显式Apply、同Operation成功/实际包移除、同键幂等重放、撤权后原线程旧工具调用permission_denied。按客户端缓存行为新开线程刷新授权后的目录，未宣称自动推送目录更新；cancel_operation本轮未调用 | run-2fb96d14bc0e4ee2b68edae4a3617ff3/result.json |
| Codex预登记HTTPS/OAuth | 独立场景9项检查通过：真实TLS校验、精确预登记ID/回调/resource、Codex S256、本机Core审阅/显式批准、实际Codex授权码交换、OAuth状态及仅两项读工具、真实Core读取。复用已冻结公开测试证书，只给本轮子进程设CODEX_CA_CERTIFICATE；保留证书/主机校验，没有修改系统信任、普通HTTP回退或owner回退 | run-065067eea8ef43b7b972ec22266e6062/result.json |

真实接入还揭示客户端边界：静态exe含现代协议并不表示默认启用。Codex features list显示mcp_2026_07_28默认false/under development；仅在本轮隔离配置开启后现代HTTP可用。其STDIO即使启用仍发送initialize，透明原字节诊断记录唯一方法initialize及-32601；当前不能作为这个版本的可用STDIO客户端，不改ALCOMD冻结协议迎合它。未带flag的STDIO、带flag的STDIO及具名trace失败分别保留于run-d3f1ded18a7544fcb53d5ca4fae28931、run-bac504bbf7744304a2e70151bf7502fd、run-db6279168c4e48c580b7795acfc99074。不是Core STDIO功能已失效，既有真实冻结SDK STDIO结果仍按原输入保留。

准备失败同样保留：run-2493410710574b13934a6f4be7232a5e的私有rustc驱动误混不同feature的Tokio产物导致无reactor，尚未创建客户端；按Cargo指纹精确匹配现有依赖后才继续，不归因Core。首个HTTP run-05981706a73d4eb9a437ef3e198dbcd7已连接但因新驱动错误期待零授权返回六工具而失败；真实合同要求授权过滤，修正本轮新测试为零授权空目录、明确授权后六工具，没有放宽任何原仓库断言或权限。HTTP业务成功后的清理曾误取project_get外层revision；独立cleanup-follow-up.json记录按精确项目ID、文件目录身份及0活动任务补取消登记成功，原错误不覆盖、不重跑业务。私有HTTP runner也只复用生产库并明确without_daemon_start，未新增持久验证框架或公共工具。

收尾确认：所有本轮临时MCP身份、OAuth注册已撤销；OAuth自有.credentials.json已删除，测试监听/私有驱动退出，样例仅取消登记、文件保留，最终0项目/0活动Operation。原Core47824和GUI85304保持5条自身管道，五程序摘要及原四组36/346/10/303份文件摘要保持；没有替换原程序、升级原数据、操作Unity或恢复桌面控制。普通浏览器人工TLS/授权体验、实际Codex刷新/到期及取消场景尚未本轮覆盖，复用既有SDK证据而不改记为Codex覆盖；完整传输/订阅观测、默认分发及其余既有缺口仍保留。仅更新文档和适用metadata/diff，不重复Workspace/Chromium/release/三平台，无新增远端结果或推送，M8仍IN_PROGRESS。

### 2026-11-18 已授权推送的真实远端结果

- 仅将此前核对的20笔717d3e0→e6d0b44b953bbd0d3e404555d3e6875e94e2e9a3普通快进推送并核对远端。bba8b905及A/B后续代码不在授权推送范围；没有dispatch、rerun、强推、tag或release。
- [CI44117137295](https://github.com/ALCOMD/ALCOMD/actions/runs/44117137295)：同SHA、attempt1，Windows/macOS/Ubuntu均failure，各m6_lifecycle为9通过/1失败/1原有ignored；后续阶段未执行，不能记完整本地或远端门禁通过。
- [CodeQL44117136645](https://github.com/ALCOMD/ALCOMD/actions/runs/44117136645)：同SHA、attempt1，Rust/JavaScript-TypeScript/Actions/Python四语言success。
- 三平台共同失败在management_component_uses_real_core_grants_and_loses_access_after_revocation：State41已按安装必需权限同意授予read，旧用例却直接断言无read启用应拒绝。窄修保留拒绝及权威状态/版本断言，先核对真实安装授权、显式撤销，再测试拒绝；不放宽权限/断言、不重试工作流。修复的实际定向结果另记，尚无修复后的远端结果。
- gh日志端点返回403（凭据非admin），改用现有GitHub连接器只读取得三个完整job日志；未变更凭据或设置。终态JSON和原始连接器日志存于target/m8-push-e6d0b44b-20261117/。原gh失败输出保留，不当作完整日志。
- 本轮未报出历史Host resource_limit不等于根因已修复；仍保留该风险及扩展人工延期、正式分发、普通TLS人工接入等既有未覆盖项。M8继续进行中，不进入M9。

### 2026-11-18 A/B 本地候选与定向结果

本轮代码候选为 `71a1ffe9a77d4fd109fe844c41b55461bcbd1ba6`，另有CI前置窄修 `f8ad8f60d0e93ab78ec86b24d6092fa3044f1d46`。均只本地提交，未推送。远端仍是e6d0b44b，不能用其CodeQL成功替代本轮代码验证，也不能把其CI失败改记通过。M8仍IN_PROGRESS，不进入M9。

实际接线：

- A已把Core既有全目录HMAC返回为catalogFingerprint，STDIO/HTTP协商支持后才广告toolsListChanged；仅接受该过滤器，SDK先ack，再独立认证连接按秒比较。失败/撤权结束流，不把不可用当空目录。建立10秒、总寿命600秒，读取/发送也受绝对期限约束；HTTP心跳15秒，保留原普通请求和32 TCP/16请求或流限额、每认证上下文4流、一项待发失效及10秒写阻塞预算。没有session或业务自动取消/重试。
- B已接通受限本机observer hello、report/bindRequest/snapshot、typed Rust client、Core内存注册表与真实STDIO/TCP/TLS/请求/订阅守卫。所有行均adapterReported，只有Core当前认证的MCP连接能消费一次性票据关联clientId；socket无client owner，不含URL/外部请求ID/参数/结果/秘密。快照分别按连接、请求和订阅权限省略组，并保留来源、不可用/陈旧、覆盖不足、计数及截断。
- 已建立上报通道失去响应后停止观测，直到adapter重启；不通过重连为同一请求重新取票据。Core忘记该通道后显示不可用，业务及既有订阅独立继续。复用原非生产test-kill-gates一次性回包故障门；仅共享原门类型并增加固定ticket-bearing report注入点，不新增生产入口/依赖、公共RPC或认证旁路，不保留票据正文。
- State43仅扩extension_grants的CHECK，新增mcp.subscription-streams.read的精确Global/*，保全旧数据且不自动授权；目录12→13。独立只读WIT/Host沿用Client+Extension/所有权/lease/生命周期/UI校验；旧WIT、Portable UI协议、Config13、八项MCP业务授权和身份系统不变。只给原权限开关补名称/描述，不重做扩展页。
- 目录结果新字段对旧严格DTO读者是破坏性变化，应匹配升级Core/client/MCP，不添加旧结果兼容回退。State43不升级原环境或当前旧review数据；旧Core继续拒绝更高Schema。无新生产依赖、版本/feature升级或根Cargo.lock变更；62,233字节测试专用Component复用已冻结wit-bindgen0.57.1/现有组件编码器，附完整源码/WIT/锁文件/产物摘要，不是正式管理扩展的新业务版本。

各轮结果独立保留，不相加为全套通过：

| 检查 | 实际结果与边界 |
| --- | --- |
| CI共同阻塞的真实Core管理用例 | 修正安装授权前置后单项1/0；原拒绝/状态/revision断言保留，未重跑整个生命周期模块 |
| 新Core观测注册表 | 4/0，含不同身份/同socket、票据单次/过期、乱序/无副作用、陈旧/断线/重启及组省略 |
| 公共DTO / Host投影Schema | 分别1/0、1/0，拒绝身份/秘密额外字段、禁止暴露未授权组 |
| State43迁移 | 最终单轮3/0，全旧记录、精确作用域、回滚及初始化重开；Clippy曾指出新测试单元素循环，按诊断简化，未放宽断言 |
| M8合同及测试Component来源 | 首轮3/1（精确权限目录仍为12项）；同步批准的第13项后独立单轮4/0。相关现行State43/M7静态合同另为9/0，不是H2–H7重新人工验收 |
| 新真实Core/Wasm/Host只读链 | 首轮0/1为夹具假定WIT枚举Debug格式，改为显式PermissionDenied匹配后单项1/0；验证零授权、只授订阅组、授权变化和撤销后旧UI租约失效 |
| 冻结SDK真实STDIO/HTTP与上报 | 首轮0/2为夹具含../非规范路径，Core正确拒绝；第二轮0/2为误用多权限helper而得到5工具，改成仅projects.read，保留2工具精确断言。之后独立2/0，绝对期限收紧后另一次2/0；最终加入真实票据回包丢失后**单轮3/0**，不拼成7/0 |
| SDK最终3/0具体覆盖 | ack首帧/接受过滤器/通知subscriptionId、空目录→明确授权→通知→2工具、B不被A变化误通知、HTTP空闲跨旧10秒界限、取消后重建、撤销终止、实际上报及两客户端Core关联。故障注入在Core已接受带票据的上报后丢回包，确认超过两个报告周期仍不重连重签，投影不可用，原订阅及实际授权读取继续 |
| 原请求取消/Apply写响应丢失 | 关联独立2/0；已接受Operation继续、Apply不重复及原未知结果/幂等恢复语义有效。日志与当时二进制摘要独立保留，不称所有风险或完整套件均已覆盖 |
| MCP受影响单元 / 既有STDIO协议 | 分别单轮15/0、12/0；保留冻结版本、元数据、严格分帧、旧握手/Tasks拒绝及限额，不以新增支持放松旧断言 |
| 前端 | check通过；两个权限相关Chromium模块同轮10/0。另一个原三分类用例首轮0/1（其他组仍按10项），按1必需+1可选+11其他同步并新增可读标签断言，独立1/0；不拼成浏览器全套通过。稳定候选前端build通过，保留既有chunk大小警告 |
| 严格/一致性 | 受影响九包all-targets严格Clippy通过；末轮回包故障门/生产Core无test-kill-gates检查和最终窄Clippy均通过、分别记录。fmt、xtask、locked metadata、diff适用检查通过；没有Workspace全量、完整浏览器或Tauri release重复构建 |

证据统一在target/m8-prepush-contract-audit-20261117/：最终SDK为sdk-subscriptions-and-observer-loss-final.log、directed-5-results.json（实际二进制摘要）；原生Host为new-transport-host-fixed.log；CI前置为ci-required-revocation.log；正常与失败的分轮日志、scoped-results.json及后续定向纠正日志均保留。969份非docs/非归档输入聚合SHA-256为90dea8208cb16ec3240655f7865e8c616e3a58eaa9617ca31bcd039f0430a8f6，逐文件/分轮日志摘要在final-candidate-evidence.json；它绑定本轮实际代码，不把旧远端或不同轮局部结果拼接成完整通过。

当前人工窗口仍是此前已验证的GUI fb72f0aa/Core3d6c2a1b/State42，不是本轮A/B的新桌面构建。本轮仅按既有授权短暂关闭自己的隔离窗口，测试用独立State43数据，结束后恢复原程序/数据/profile并实际核对五组公开状态。最后GUI97464/Core97292，可见/响应、5条GUI自身管道；原data36/profile346、上一review10/303份集合和摘要保持，五份旧程序摘要保持。未恢复桌面输入、未启动或干扰Unity、未升级原数据。保全与恢复证据在final-environment-preservation.json及directed-5-*-before/after.json、directed-5-restored-window.json。

保留未覆盖项：新增代码尚无远端验证；没有10分钟自然终止实测、长时/最大规模压测或实际网络慢读压测；OAuth订阅的到期/轮换/刷新及实际Codex通知消费未单独实测，既有OAuth/Core逐请求鉴权证据仅按原输入保留。扩展人工/普通可信HTTPS安装按所有者要求留待后续实际扩展开发，不能记通过或豁免；正式默认分发仍未完成。历史Host resource_limit根因未定位，保留风险；M7既有签收和非阻塞体验待办不重开，不擅自给未明确归属事项指定M12。

### 2026-11-18 OAuth订阅续验（仅测试与记录）

测试提交为`04a7fbec8bb6eff92217e53f4f25e00ff1b6adca`，生产代码仍对应71a1ffe9，唯一源码差异是现有oauth_flow.rs新增一个具名用例，未修改认证实现、公共合同、依赖、State43、Config13或权限。以下为上节未覆盖项的增量，历史检查点与失败保持，不将它与之前SDK3/0合并为完整套件。

`oauth_flow::oauth_live_subscriptions_follow_grants_refresh_and_revocation`真实HTTPS/冻结SDK/Core首轮**1通过/0失败**（7.11秒），只使用已有公开localhost测试证书及测试进程的显式信任、不修改系统信任，不访问用户工程。复用既有S256、预登记、Core明确审批及Token端点，没有测试专用生产旁路。

- 已建立订阅能收到Core授权删除/恢复的目录通知；删除后目录为空，恢复并额外给Core客户端operations.read后，原token仍严格只有两个项目读取工具，不能超出原审批上限。
- 实际TLS适配器上报为https-oauth，订阅clientId经Core认证关联；公共投影未包含本次access/refresh秘密。
- 正常refresh返回新access与refresh，继承原能力上限；旧未到期access的既有订阅继续有效，不能错误地将正常refresh视为旧access撤销。重放已消费refresh后，同一family的新旧两个订阅都终止，旧access的真实HTTP调用401。
- Core凭据轮换、OAuth注册revision轮换、注册显式撤销分别使已建立的旧订阅终止，不能用虚假的空目录通知代替拒绝；新generation/新注册revision经重新明确审批可建立新流。

定向严格Clippy、最终编译、fmt、xtask、diff通过。没有重跑Rust/浏览器全套或release。生产输入未变，所以之前A/B的SDK3/0、Host1/0、迁移3/0及其他有效证据按原输入继续适用，本轮不冒称重新执行。自然10分钟到期仍未实测，原Store中到期边界的受控时间用例仅保留为组件证据；没有修改系统时钟、伪造到期结果或增加新时钟机制。长时/最大规模压力、实际Codex通知消费、延期的扩展人工/普通可信HTTPS安装、正式默认分发及历史resource_limit仍未收束。

五组公开状态在测试前后实际一致；恢复旧GUI105140/Core105036，仍为fb72f0aa/3d6c2a1b、State42，非新桌面构建。原data36/profile346、上一review10/303份集合及摘要保持，五份旧程序摘要保持；原开发环境仍按原先关闭状态保留。未恢复桌面输入、未操作Unity、未迁移原数据。

原始用例日志、二进制SHA256和五组前后状态在target/m8-oauth-subscriptions-20261118/，输入及保全摘要为evidence.json；编译/Clippy日志位于原target/m8-prepush-contract-audit-20261117/的oauth-subscription-*-20261118.log。新增提交未推送，远端仍e6d0b44b（CI failure / CodeQL success），不作为新代码通过；本次“继续”没有扩大推送授权。M8仍IN_PROGRESS，不进入M9。

### 2026-11-18 六笔推送与权限目录断言窄修

所有者明确批准准确六笔后，真实Windows用户在main核对远端e6d0b44b、干净工作树、完整线性链和969份验证输入，普通快进推至`a94428b3bfd86e7c8911b15c48168234ac69a6a6`并ls-remote确认。顺序为bba8b905、f8ad8f60、71a1ffe9、7ebda512、04a7fbec、a94428b3；不含额外提交，没有强推、发布、标签或手动工作流。

- 同SHA、attempt1的[CodeQL44146171781](https://github.com/ALCOMD/ALCOMD/actions/runs/44146171781)已success，Rust、JavaScript/TypeScript、Python、Actions四语言全部通过。
- 同SHA、attempt1的[CI44146171369](https://github.com/ALCOMD/ALCOMD/actions/runs/44146171369)已终态failure：Windows、macOS、Ubuntu均为同一旧权限数量断言失败，m6_lifecycle各自单轮10通过/1失败/1原有ignored；后续测试/构建/浏览器阶段未执行，不拼接成本轮完整通过。此前必需授权前置修复用例及新增只读Host用例在三平台该轮均通过，不代表整个套件通过。
- 具体失败是install_consent_and_undeclared_permissions_use_real_core_and_signed_package的旧数量断言：available_permissions实际13，仍期待12。已批准的新mcp.subscription-streams.read使目录增加，Core返回符合现行合同；这是遗漏的测试同步，不是CI平台故障。
- 本地修复`b9948171f236c15b465c2e71329a2c7d54e583fe`仅把旧数量改为独立写明的完整13项精确目录，不能遗漏、重复或夹带未知权限。保留原安装条款/必需权限拒绝、成功只授必需项、撤销阻止启用/旧UI失效、明确授予未声明权限及幂等重放断言；生产代码、合同、权限、迁移、依赖、工作流不变。
- 真实Windows/Core/签名包单项1/0（2.79秒），受影响集成测试严格Clippy、fmt、xtask、diff通过，不重复Workspace、浏览器、release。日志、二进制及源码摘要在target/m8-push-a94428b3-20261118/consent-fix-evidence.json；自动运行JSON和完整失败job日志同目录保存，不拼接原失败和本地单项成完整通过。
- 仅暂停归属已核实且0活动任务的本轮自有review，测试后恢复原GUI84872/Core98828，可见/响应及5条自身IPC管道。五组公开状态完全一致，原36/346/10/303份文件集合及摘要、五程序摘要保持；依然是fb72f0aa/3d6c2a1b、State42的旧验收窗口。原开发环境保持此前关闭状态，无桌面控制、Unity操作或原数据升级。

三个完整job日志及ci-final.json、codeql-final.json均保留在target/m8-push-a94428b3-20261118/；与旧e6d0b44b的运行分开。未使用工作流重跑、未修改CI环境或放宽检查。本次六笔推送授权已执行完；新增本地窄修尚未推送、没有对应远端结果。仍保留自然到期/长时压力、实际Codex订阅通知消费、延期的扩展人工/普通可信HTTPS安装、正式默认分发、其余业务映射及历史Host resource_limit未定位等缺口。M8仍IN_PROGRESS，不进入M9。

### 2026-11-18 本轮持续CI收束授权与OAuth合同测试同步

所有者明确批准b9948171、337f4972后，已核对真实Windows用户、main、干净工作树、准确两笔及测试输入、远端a94428b3，普通快进至`337f49724a61c0c5af22a9eac294543bfc808318`并ls-remote确认。随后所有者明确“本轮无需再反复请求审批。直接推进到ci没有问题”：本轮范围内必要CI窄修、定向验证、提交及普通快进推送沿用此授权，不再逐笔确认；不授权强推、发布、标签、修改GitHub设置或无关功能，不以手动dispatch/rerun代替修复。

同SHA自动[CodeQL44151487149](https://github.com/ALCOMD/ALCOMD/actions/runs/44151487149)已终态success，四语言全部通过；[CI44151487184](https://github.com/ALCOMD/ALCOMD/actions/runs/44151487184)已终态failure，三平台同一失败。新失败是oauth_schema_exposes_only_approved_roles_methods_and_bounded_review：旧测试读取hello.authentication根上的$ref，已批准的受限观测身份使这里实际为oneOf组合；不是认证实现或CI平台故障。三个平台的Schema模块各51通过/1失败，后续阶段未执行。三个平台分别有生命周期11/0/1原有ignored、实际MCP业务12/0、STDIO12/0，分别按该模块原始结果保留，不拼作完整套件通过。

本地`7b1a4f0efaf8ee0d8db0a376656f57af45e36432`只把旧直接引用断言同步为完整oneOf精确形状：原OAuth引用必须保留，新增受限观测身份必须要求kind及空credential、禁止额外字段；没有删除OAuth角色/方法/页大小/秘密排除断言。生产代码、Schema、权限、迁移、依赖、工作流不变。本机先复现原单项0/1，再独立执行整个受影响Schema合同模块52/0；窄严格Clippy、fmt、xtask、diff通过，不重复全量。日志、完整job日志及终态JSON保留在target/m8-push-337f4972-20261118/，与前轮分开。

这项验证不占Core端点，未暂停/替换验收窗口或原程序、修改原数据/系统信任、操作Unity或恢复桌面控制。新修复的远端结果尚未取得；M8仍IN_PROGRESS，未覆盖项和延期的扩展人工保持，不进入M9。

### 2026-11-18 Store安装同意数量断言续修

已按本轮持续收束授权普通快进推送7b1a4f0e和记录223e8ce4，准确候选`223e8ce48cd7ebda747b3d01121166a07465c739`。同SHA、attempt1的[CodeQL44156608097](https://github.com/ALCOMD/ALCOMD/actions/runs/44156608097)四语言success；[CI44156608608](https://github.com/ALCOMD/ALCOMD/actions/runs/44156608608)三平台均failure：此前OAuth Schema模块各52/0，随后Store m6_runtime各15/1。失败为install_consent_rejects_missing_or_changed_evidence_before_acceptance_and_grants_all_required仍期待12项，实际已批准13项；后续阶段未执行，不拼接作完整通过。

本地`80ebb2643543b4d801330d37e670727e536c025d`仅将这一行预期同步为13；原逐项确认全部必需权限、缺少同意/指纹不符时权威状态和Operation不变、安装后原Plan幂等重放断言保持。集中核对Core、Store、Host、合同及GUI相关的权限目录/数量引用，未发现其他旧12项断言；128项边界、SQLite列序号/时刻及历史记录不误改。生产代码、Schema、迁移、权限、依赖和工作流不变。

本机先复现原单项0/1，再独立执行受影响Store模块16/0（8.95秒）；窄严格Clippy、fmt、xtask、diff通过，没有重复Workspace/浏览器/release。三个完整job日志、ci-final.json、codeql-final.json、经stdout/stderr交错核对的module-summaries.json和源码/实际测试二进制摘要store-consent-fix-evidence.json在target/m8-push-223e8ce4-20261118/。新修复尚待远端验证，不占原Core端点或操作窗口/原数据/Unity；扩展人工延期和原未覆盖项继续保留，M8仍进行中、不进入M9。

### 2026-11-18 测试诊断与生产禁日志边界续修

已普通快进推送80ebb264和记录3fa1b3b1，候选`3fa1b3b15dfe57244b89d2c150d719a95f2c7807`。同SHA、attempt1的[CodeQL44162172266](https://github.com/ALCOMD/ALCOMD/actions/runs/44162172266)四语言success；[CI44162172455](https://github.com/ALCOMD/ALCOMD/actions/runs/44162172455)三平台均failure，Store安装同意模块各16/0，随后m7_security_conformance各2/1；后续阶段未执行，不记全套通过。

具体为sensitive_ui_payload_has_no_production_logging_sink整文件扫描m6_runtime.rs时命中三处test-kill-gates条件下的eprintln诊断。核对内容只有派发到期、Host export名称/时间预算、接收超时枚举；不含UI内容、RPC参数或秘密，且该feature不是生产默认入口。没有发现这三处诊断在关闭测试feature时产生生产输出，不将静态命中误报成已证实秘密泄漏。

本地`0c73bf4b4aed9bb8b5e88efa36b81952e2738769`将诊断收回既有test_support模块，三个调用仍逐个受原test-kill-gates限制，模块自身仍由相同feature限制；函数为pub(crate)，不新增公共RPC/Extension API、日志政策或生产日志入口。原禁日志安全测试逐字节保持，没有白名单、扫描排除或删断言。既有诊断文本和Host超时/终止/错误逻辑保持，历史resource_limit根因仍未定位，不能宣称修复该历史问题。

本机原单项0/1，修复后原安全合同模块独立3/0；生产Core的--no-default-features检查、启用test-kill-gates的Core库严格Clippy、fmt、xtask、diff通过。不重复Workspace、浏览器或release，不占Core端点或操作原窗口/数据/Unity。三个完整job日志、终态JSON、逐平台模块摘要及源码/原安全检查不变证据ui-log-fix-evidence.json位于target/m8-push-3fa1b3b1-20261118/。新候选仍待远端结果；按所有者本轮持续授权直接收束，M8仍进行中，不进入M9。

### 2026-11-18 扩展工具旧目录快照续修

按本轮授权已推送0c73bf4b和记录9461c76f，候选`9461c76f199b1261835ad51dae51d02e059614d2`。同SHA、attempt1的[CodeQL44167329975](https://github.com/ALCOMD/ALCOMD/actions/runs/44167329975)四语言success；[CI44167330779](https://github.com/ALCOMD/ALCOMD/actions/runs/44167330779)三平台均failure：原m7_security_conformance各3/0，后续provider_abi_is_additive_and_core_controls_write_targets仍期待10项权限、实际13项，m8_extension_tools_contract各1/1；后续阶段未执行，不能记完整通过。

前次同类检索仅覆盖旧12项，遗漏更早10项快照；本轮明确扩大到同一扩展/权限合同的静态检查，不误改无关数量/SQL列/历史记录。新本地`2818f97845f70eb4a4fc654cbb3b03fccb9606d2`仅将旧数量断言替换为完整13项精确枚举，保留原mcp.tools.provide包含断言、固定7个ABI函数、无WASI/凭据/万能RPC、旧WIT字节和真实Component来源摘要断言，不修改Schema或生产行为。

新增候选的首次组合提交/推送命令被自动审批在执行前拒绝，理由为其认为本轮授权未明确覆盖这两笔新增提交；该命令未执行提交或推送。已独立核对只有上述合同测试与三份真实记录，并引用所有者本轮“无需再反复请求审批。直接推进到ci没有问题”的授权申请复核；不更换入口，未以工具拒绝代替CI结论。本地文档提交属于此前已有授权，独立完成。

本机先复现原单项0/1；修复后同一次命令的m6_contract、m7_contract、m8_extension_tools_contract、m8_management_contract分别5/0、7/0、2/0、4/0，属于本轮关联静态合同验证，不是重开M7人工验收。窄严格Clippy、fmt、xtask、diff通过，不重复完整Rust、浏览器或release；不占Core端点或操作原环境。终态JSON、三个完整job日志、逐平台模块摘要和四模块实际二进制/输入摘要provider-fix-evidence.json位于target/m8-push-9461c76f-20261118/。Windows未完成时取日志曾返回BlobNotFound，未将该404当产品失败或完成日志，最终已取得归档日志。新修复尚待远端验证；M8仍进行中，延期人工及原缺口保留，不进M9。

### 2026-11-18 本地扩展安装入口的浏览器回归续修

所有者明确批准将 2818f978 和 91065849 普通快进推送至 origin/main，并授权本轮同一批 CI 阻塞所需的后续普通快进。推送前核对标准目录、真实 Windows 用户、提交链及干净工作树。远端确认候选为 `91065849a71e5b6d2c280dd809a2f250c97db03e`。

同 SHA 的 [CI 44245716844](https://github.com/ALCOMD/ALCOMD/actions/runs/44245716844) 首次运行在三平台均通过仓库 Check、CLI 自动启动集成、发布版及 Tauri 构建。Chromium 各 729 通过、2 失败。[CodeQL 44245716183](https://github.com/ALCOMD/ALCOMD/actions/runs/44245716183) 四语言全部通过。此前的扩展目录断言已在三平台通过。

两条旧浏览器用例仍以主安装按钮打开本地包流程。现行主入口打开在线来源，本地文件从下拉菜单进入。本机两项均复现，失败快照显示在线地址字段。修复提交 `48c6e5341bfde68e1a5d2e60f731f298244327b1` 复用现有本地入口测试助手。文件选择仍验证内联控件与回填路径，安装用例验证原包路径进入 Plan、同意前无 Apply、同意后恰好一次 Apply 及任务反馈。仅测试文件变更，保留原超时及检查要求。

本机修复后同轮相关四个 Chromium 模块 38/0，GUI 类型、Material 和本地化检查、xtask 与 diff 均通过。原失败、定向日志、输入摘要和自动运行终态分别保存在 target/m8-push-91065849-20261118/。新候选的自动结果仍待取得，扩展人工继续延期，M8 保持进行中。

### 2026-11-18 本批自动检查终态

按本轮授权，从 91065849 普通快进推送 48c6e534 和记录 2d6e4bb2。远端 `origin/main` 确认指向 `2d6e4bb235bc40e85b93b4159e7a6df729d8602f`。

[CI 44254367013](https://github.com/ALCOMD/ALCOMD/actions/runs/44254367013) 同 SHA 的首次自动运行在 Windows Server 2025、Ubuntu 22.04 和 macOS 15 arm64 全部通过。三平台均通过仓库检查、CLI 自动启动集成、发布版构建、Tauri GUI 构建和完整 Chromium 回归。Chromium 每平台 731 通过、0 失败，Windows 18.7 分钟、Ubuntu 14.1 分钟、macOS 25.3 分钟。[CodeQL 44254366154](https://github.com/ALCOMD/ALCOMD/actions/runs/44254366154) 同 SHA 首次自动运行的 Rust、JavaScript/TypeScript、Python 和 Actions 全部通过。

三平台原始任务日志、运行终态与 SHA-256 摘要保存在 target/m8-push-2d6e4bb2-20261118/，汇总为 final-evidence.json。此前失败保留各自候选及运行编号。扩展人工检查仍延期，历史 Host resource_limit 根因仍待定位。M8 保持进行中。

### 2026-11-18 OAuth 订阅自然到期

本地测试提交 `af4ff5f6be3cfac036382eaf46c48c1cbd93f4e4` 新增一个按需运行的十分钟真实用例。Windows 隔离 Core、真实 HTTPS、冻结 SDK 和 S256 授权码交换使用实际时钟。签发后等 25 秒再订阅，流在开始换取 token 后约 600.067 秒关闭，此时订阅仅存续 575.041 秒。到期没有发出虚假的目录变化通知，旧 token 调用返回 HTTP 401，重新授权取得的 token 可读取两个获准工具。具名用例 1/0，耗时 601.65 秒，测试二进制 SHA-256 为 `a3e2c6261b2e179018c55319b1bc111c143ccccbb2814b656012461321bf53a9`。编译、该测试目标的严格 Clippy、fmt、xtask、metadata 和 diff 检查通过。

证据在 target/m8-subscription-expiry-20261118/。本用例覆盖访问 token 到期，订阅流自身十分钟上限、长时并发与慢读仍保留。生产代码相对上面的远端候选未变，新增测试提交没有对应远端运行。

### 2026-11-18 本机 Codex 工具目录续验

用 Windows 上实际安装的 Codex CLI 0.153.4 `app-server` 连接同一候选的隔离 Core 与回环 Bearer MCP，使用独立 Codex 配置和临时线程，无模型调用。只记录协议方法和 HTTP 状态。初始发现两个项目读取工具。Core 撤销一次性样例客户端的读取权限后，7 秒观察内没有自动刷新请求，前后 Core 均为零个绑定订阅流，线上未见 `subscriptions/listen`。主动查询同一线程状态时，Codex 重新发出 `server/discover` 和 `tools/list` 并返回空目录，新线程也返回空目录。本结果覆盖该版本和该配置的回环 Bearer 接入，真实 Codex 的推送消费及其他传输上的行为仍未证实。冻结 SDK 的订阅证据见前述用例。

首次启动在隔离 Core 正常退出等待期内提前停止，探针未运行。核对隔离进程已退出、端点空闲及原有零活动基线后续跑，探针退出 0，临时凭据已撤销。两项验证后均恢复同一隔离 GUI/Core、数据目录和 WebView 配置，系统、项目、Operation、MCP 客户端、OAuth 注册五组公开状态一致。最终 GUI 102556 可见且响应，其五条管道均连接隔离 Core 118452。未恢复桌面控制。前后快照、工具链输入摘要和探针输出保存在同一证据目录。

### 2026-11-18 第一方管理扩展默认分发审计

[A-019](../decisions/accepted.md) 已确定正式产品默认安装并启用 MCP 管理扩展 `com.vrclearn.alcomd.extension.mcp`。扩展当前可以通过普通安装流程运行，清单中只有可选权限。默认启用应沿用零授权启动，外部 MCP 服务仍由用户独立启动。

- 正式清单当前使用占位发布者指纹。现有构建说明产出 Wasm Component，正式扩展包、应用层签名输入和随产品装配入口仍缺失。当前 Tauri 配置关闭 bundle，也未声明随包资源。正式包、安装器装配和发行验证归 [M12](M12-full-product-distribution.md)，此处只核对接口和 Core 生命周期。
- Core 目前通过正常 Plan/Apply、任务和恢复流程安装，完成落库后为停用状态。卸载删除扩展记录，仅根据记录是否存在补装会在用户卸载后再次安装。公共安装 RPC 也明确拒绝调用者提供 `first_party_packaged` 来源，该检查应保持。
- 建议由 Core 在首次符合条件的新配置中处理一次默认安装，复用现有包签名、摘要、Plan/Apply、任务恢复及启用规则。包来自正式产品装配清单，Core 核对固定扩展 ID、发布者公钥和包摘要。安装或启用中断时按原任务恢复，后续启动尊重用户停用和卸载。实施前先确定可长期保留的一次性记录及失败恢复方式，若确需新增持久状态或公共合同则单列审批。隔离验证可使用测试签名和可丢弃配置，正式身份和发行验收仍由 M12 完成。
- 现有扩展 Plan 会保留来源、扩展标识和关联 Operation，且卸载后仍保留。应先验证能否用受控 `first_party_packaged` Plan 和原任务结果识别初始化尝试及安装完成。还须分别验证安装后首次启用中断、用户后来主动停用或卸载、包校验失败和重复启动，避免将这些状态误判为需要重新安装。

管理扩展的活动页当前读取 Core IPC 连接和在途 RPC。A/B 已接通订阅流投影和只读 Host 接口，第一方管理扩展尚未声明或调用这项接口。订阅流在管理扩展内的实际展示和延期的原生人工检查仍待完成。

### 2026-11-18 默认安装的持久证据核对

在 State43 隔离 SQLite 中，以 `first_party_packaged` 来源创建 Plan 并通过现有 Store 接受安装，验证重启恢复入口可找到同一 Operation。安装提交并以“卸载并删除数据”完成卸载后，扩展记录已移除，原 Plan 仍能关联成功 Operation 和 `state_committed` 日志。数据库外键阻止删除该已应用 Plan。State42 重建 `extension_plans` 时未重建原防删除触发器，因此只使用带有任务日志的已应用 Plan，未应用 Plan 不作为完成证据。该测试覆盖 Store 行为，正式包验证及启动接线仍待实施。

- 产品启动阶段只对符合条件的新配置请求安装固定扩展。Core 使用受产品控制的包及预期签名身份、摘要进行验证，公共 RPC 仍拒绝调用者提供随包来源。若包需要用户同意条款或授予必需权限，不走默认安装。已有同名安装及此前用户卸载的记录不触发补装。
- 已接受的安装交给现有 Operation 恢复。推荐在同一安装落库事务首次写入启用状态和完成证据，避免安装与启用之间中断后误把用户停用当成待启用。零默认授权，无后台运行权限。成功卸载后以旧安装日志识别已经做过的初始化。包校验失败和已受理任务失败保留真实错误，允许用户正常安装，不在每次启动重复创建失败任务。
- 已接受与已提交的恢复和一次性判断可复用现有记录。还需与正式分发对接新配置的初始化触发和受验证的包来源，明确 Core 接受任务前发生中断的恢复入口。此处仅核对设计和 Store 证据，未新增公共合同、State 迁移、正式签名资源或启动行为。

Store 定向模块单轮 4/0，含上述安装证据用例和三项 State42 迁移保全、回滚及重开检查。测试二进制 SHA-256 `2d744fe525ca259c37b532cb7de05c2707161739fe06d41a5790c10475b3c8fd`。

### 2026-11-18 管理扩展订阅流接线

本地代码 `dccf953ffd9e9eea16d5c2109c2101ba72b67e70` 在原活动页接通已批准的 `alcomd:mcp-transport/observations@1.0.0`，清单声明 `mcp.subscription-streams.read` 为可选权限。Core 连接与在途调用保留原接口，订阅流标明来自适配器上报。报告不可用、过期或覆盖不全另有状态，缺少授权隐藏该组，零授权仍可启动。没有变更公共 WIT、权限目录、RPC、数据库或依赖。

Guest 单元 8/0、文档测试 1/0、合同 4/0，原生和 Wasm 定向严格 Clippy、Wasm 发布构建及 Component 编码通过。组件 SHA-256 `0a80303d846e0e24d0161054f3d18477a17aca7bf0313bb316603577cd868c5f`。实际 Windows Host 加载该组件并完成三个 Host 接口的调用，使用模拟 Core 结果验证仅订阅流授权、刷新后隐藏与普通 Portable UI 输出。该探针初次以错误文件名启动，被现有路径校验拒绝，使用规定的 `extension.wasm` 后通过。

真实 Core/Host 回归已编译。首次准备时，Windows 隔离窗口正常退出两次超时，端点未释放，用例当时未启动。

所有者随后关闭隔离窗口，确认原隔离进程均退出后补跑 `runtime_observations_use_real_authenticated_connections_and_revocable_host_grants`，单次 1/0、22.10 秒，使用 `dccf953f` 的组件和定向二进制 SHA-256 `468428fc5074a2fc129c9eceb73bc238fb29b771da3bdbbfc7add070f3d00e13`。真实签名测试包经 Core 安装和启用，独立授予订阅流读取后，活动页通过 Host 读取真实 Core 接受的受限观测夹具报告。报告缺失时不显示零条，报告到达后显示一条订阅流，仍隐藏未授权的 Core 连接与调用。撤销权限后旧 UI 会话不能刷新。该用例验证管理扩展接线，报告由测试夹具提交，实际 MCP adapter 的观察链见前述独立证据。

原隔离窗口使用同一程序、数据目录和 WebView 配置恢复，GUI 117348 可见且响应，五条 GUI 管道均连接隔离 Core 116016。系统、项目、Operation、MCP 客户端和 OAuth 注册五组公开状态均与使用前相同。未恢复桌面控制。证据位于 target/m8-management-streams-20261118/。扩展人工继续延期，新增代码尚无远端结果。

### 2026-11-18 Core 随包初始化接线

Core 启动入口可以接收一份已确认新配置资格的发行输入，包含本地包路径、固定发布者公钥指纹和签名内容摘要。Core 重新验证签名及三项身份约束，包括固定扩展 ID，并要求 Portable UI、无安装条款且无必需权限。校验失败时 Core 继续启动并报告具体错误，用户可按普通规则安装。公共安装 RPC 仍拒绝声明随包来源。

现有 SQLite 事务一次写入 Plan、Operation、安装日志和幂等记录。安装登记事务把随包扩展设为启用，授权仍为空。启动恢复接手已接受的任务，先前接受的随包任务、已有安装、正在处理的同 ID 安装及已提交的卸载都会阻止重复初始化。用户停用后保持停用，卸载并删除数据后保持移除。普通安装继续落库为停用。

当前开发二进制未装配发行输入。产品新配置资格、Core 接受任务前的启动交接记录、正式签名包及安装器装配仍归 M12。本批未新增 RPC、Schema、权限、生产依赖、运行时开关或签名材料。

本地代码候选 `6f9da743fa6937b4e355147c8b0ef282c910bddb`。真实 Windows Core、Host 与签名测试包的启动用例单次 1/0、6.35 秒，覆盖摘要不符且无安装任务、RPC 冒充随包来源被拒绝、零授权启用与页面打开、停用后启动、卸载并删除数据后再次启动。测试二进制 SHA-256 `c010465fc7652e4cf79e6d4d8345a4b69cbc2760f8baba576bad0ed848ccb4d5`。该二进制使用严格 Clippy 提示修正前的输入，随后只删除幂等键构造处多余的借用，生成的键文本和校验一致。最终候选的 Store 定向 4/0，二进制 SHA-256 `817eb77f8e6698f67909d110caa771b9522eb55afc7783d8a6da4ac1c90508b4`，受影响四包严格 Clippy、fmt、metadata、xtask 和 diff 通过。

原隔离程序、数据及 WebView 配置已恢复，GUI 98796 可见且响应，五条管道均连接隔离 Core 86168。系统、项目、Operation、MCP 客户端和 OAuth 注册五组公开状态相同。证据保存在 target/m8-packaged-mcp-20261118/。扩展人工仍延期，新增代码没有对应远端验证。

### 2026-11-18 订阅容量与独立寿命续验

使用隔离 Windows Core、生产回环 HTTP 适配器、已授权 Bearer 和按需用例。容量用例 1/0，23.62 秒。同凭据四条流建立后第五条返回 HTTP 429，四条真实网络响应暂停读取正文 16 秒，另一身份仍成功调用 `alcomd_list_projects`，恢复读取后收到工具目录变化通知。全实例 16 个请求槽占满后普通调用返回 429，释放一条流后普通调用成功，订阅槽也可重新分配。32 条实际 TCP 连接占满后新增连接无法取得响应，释放一条后普通工具调用恢复。

慢读用例暂停应用层读取，未实测耗尽操作系统发送缓冲。现有 `WriteBound` 可控阻塞与空闲单元分别通过，单轮 2/0。容量和独立寿命保留为按需用例，避免每轮 CI 占用十分钟和额外端点。HTTP 文档同步此前批准的订阅语义。

首次独立寿命用例 0/1，601.42 秒。到期前同一 Bearer 仍可调用工具。SDK 在十分钟边界收到 `Abrupt`，未取得协议终态，后置调用当轮未执行。原订阅循环及 HTTP 响应体各自设置十分钟计时，硬截止可先于 SDK 终态写出。修正让两层共享 HTTP 绝对截止点，最后一秒供 SDK 发送终态，HTTP 仍保留硬上限，普通请求预算保持。订阅循环优先检查期限，正常到期时正在等待的目录读取或通知发送不会变成错误结束。首轮二进制 SHA-256 `278bc575ac38523b1b253d98524803614e264fd02d5db403adf4ccda3b6e9892`。

修复代码 `1883dac8cf62ded4d1ea808d1278ed0cb2109f8d`。同一二进制的五项独立复验均通过。SDK 在 599.043 秒取得正常终态，同一 Bearer 随后成功调用工具和重新订阅。容量 1/0（23.57 秒），冻结 SDK 的 HTTP、STDIO 和 OAuth 授权变更与撤销回归各 1/0。五项共用的二进制 SHA-256 为 `14ee5ddd99d856a5797a5d52c0f8a0167d97ca99cb1bfc01a1e0df860e27cecf`。最终 HTTP 单元 10/0，MCP 严格 Clippy、fmt、metadata、xtask 和 diff 通过。未改变公共 RPC、Schema 或依赖。

首轮及复验分别存于 target/m8-subscription-limits-20261118/ 和 target/m8-subscription-limits-recheck-20261118/。第二轮准备时旧隔离窗口正常退出超时，所有者手动关闭后完成复验。原程序、数据和 WebView 配置已恢复，GUI 120268 可见且响应，五条管道均连接原隔离 Core 117724，五组公开状态一致。普通 Codex 的推送消费、持续满载、TCP 发送缓冲耗尽以及延期的扩展人工验收仍未覆盖。没有推送或重新执行全量。

### 2026-11-18 订阅与管理扩展候选远端结果

所有者批准后，将已核对的 10 笔提交从 `2d6e4bb235bc40e85b93b4159e7a6df729d8602f` 普通快进推送至 `origin/main`。远端确认候选为 `2735119f6245d7c97552b2c19c5f467c5c0ad4ae`。

- [CI 44300586408](https://github.com/ALCOMD/ALCOMD/actions/runs/44300586408) 首次自动运行整体结论为 failure。Windows Server 2025 和 Ubuntu 22.04 作业全部通过，均包含仓库 Check、新增签名包启动回归、CLI 自动启动集成、发布构建、Tauri 构建及 Chromium。Windows Chromium 731/0（18.5 分钟），Ubuntu Chromium 731/0（15.7 分钟）。长时订阅与容量按需用例沿用同一生产代码的本地证据。
- [macOS 作业 134788964082](https://github.com/ALCOMD/ALCOMD/actions/runs/44300586408/job/134788964082) 在排队约 15 分钟后取消，未分配运行器、未执行任何步骤。GitHub 注释明确记录 hosted 运行器未接手及 macOS arm64 容量紧张。该平台尚无同 SHA 构建或测试结果。
- [CodeQL 44300585894](https://github.com/ALCOMD/ALCOMD/actions/runs/44300585894) 同 SHA 的首次自动运行完成，Rust、JavaScript/TypeScript、Python 和 Actions 均通过。

首次运行终态、已执行作业的原始日志和摘要、macOS 取消注释归档于 target/m8-push-2735119f-20261118/。只收集推送自动触发的运行。M8 保持进行中，旧三平台通过记录继续对应其原 SHA，扩展人工继续延期，正式签名包与发行装配仍归 M12。

范围核对确认，现有外部 MCP 允许列表只开放包移除 Plan 和既有 Apply，Core 接受事务也要求 MCP 写入是无来源集合的移除计划。项目包候选查询、安装和重装需另行明确允许委托的包来源和可见字段，再审阅工具与 Core 授权的最小差异。现有身份和运行时不需要重做。

### v3 MCP 能力对照与首批包目录合同（首批已批准）

按所有者确认，M8 以冻结 v3.4.0 的实际用户能力为下限，按用户能否在现行授权下完成同等流程并取得所需结果验收。允许一个 V4 工具覆盖多个旧用例，也允许一个流程复用多个现有入口。工具数量、名称、字段组织和交互顺序由 V4 的实际需要决定。

对照 `4aa98ae4f18d42c10137278997180dbede991e88` 的工具参考、`vrc-get-gui/src/mcp/tools.rs` 注册目录和 `vrc-get-gui/src/backend/mcp_capabilities.rs`，确认实际注册 33 个工具。既有 [M-1 逐项映射](../baselines/mcp-tool-contract.md#33-个冻结用例的-v4-映射) 用作查漏索引。下表保留首批实施前的审计。当前结果见[能力与证据清单](#2026-11-19-当前能力与证据清单)。

| v3 用例 | 当前 V4 MCP 状态及具体缺口 |
| --- | --- |
| 项目列表、详情（2） | 已有登记项目、Unity 版本、清单及锁定依赖读取。v3 详情还报告是否需要解析及已安装包的详细元数据，现有投影未完整覆盖，列入包详情和项目证据补充。身份使用 `ProjectId`，不需要传绝对路径。 |
| 全局列包、按仓库列包、包详情、安装、卸载、重装（6） | 单包卸载已有真实闭环。其余五项未对外开放。全局目录与仓库目录需要脱离项目读取。安装要支持最新可用兼容版、精确版本和指定来源，重装须保持已安装版本。v3 本地用户包也在聚合目录中，本轮方案考虑已登记本地包。 |
| 列出、添加和移除仓库（3） | Core 有 `repositories.list/inspect/register/unregister`，外部 MCP 未开放。首批目录提供查包所需的仓库索引与可用性，完整来源显示策略、登记和移除另批。仓库管理须对 URL、凭据和来源变更定义独立边界。 |
| 创建、登记、复制项目、创建备份、从备份恢复（5） | Core 有 `projects.create/register/planCopy/applyCopy` 和 `backups.create/planRestore/applyRestore`。MCP 调用身份、导入及目标目录边界、备份授权和对应 Operation 投影待审。 |
| 模板列表、详情、创建、编辑、设置和移除包、设置和移除 UnityPackage 附件、移除模板（9） | Core 使用 v4 模板模型及 `projectTemplates.list/save/remove`，外部 MCP 未开放。优先复用同一模板读取和编辑流程覆盖包及附件的增删，验证只改变用户指定的内容，保留内置模板保护；现模型无法完成的操作单列。 |
| 环境设置（1） | Unity 安装、有效启动参数和默认位置通过同一 Core 读入口提供。三端使用相同权限和原值，见下方环境读取对齐记录。 |
| 活动搜索、详情、统计、上下文及技术日志搜索、详情、统计（7） | A-026 后续决定明确撤销原活动/技术日志产品入口及相关权限。列为已撤销能力，当前 M8 的 Operation、事件和传输观测各按自身合同记验收。若要恢复这七项，需先明确修订 A-026。 |

工具外仍有两项对照。v3 七个长任务可轮询、取消，个别流程可等待输入和审批。V4 使用 A-021 的 `OperationId`；目前工具只覆盖包任务查询及取消，需随各流程核对真实确认、继续和结果能力，Core 尚无通用 input/approve/reject/resume RPC。v3 Windows 上还有 Codex、Claude Code、Cursor 快速配置与覆盖确认。V4 当前有连接配置说明、受控凭据及真实 Codex HTTP/OAuth 证据，便捷配置仍有缺口，需按各客户端的冻结协议支持和独立凭据边界处理。管理扩展人工验证仍按所有者要求延期。

#### 首批已批准的最小外部合同

建议完整交付一个目录到写入的流程。全局查包和按仓库查包由同一查询工具覆盖，安装、重装和已实现的卸载共用现有 Apply 和任务工具。采用两个固定 Core RPC `mcp.packages.query`、`mcp.packages.plan` 和协商 capability `mcp.packages.catalog-plan.v1`。适配器使用 typed client，Core 调用已有目录、项目读取和 M4 Resolver/Plan/Apply。现有 GUI/CLI RPC 形状不动。新投影有必要，现有 `packages.queryProjectCandidates` 强制要求项目和 `settings.read`，`repositories.packages` 是原始单仓库报文，`packages.planInstall/planReinstall` 的 Plan RPC 输出包含内部来源标识和下载地址。

| MCP 工具（名称均加 `alcomd_`） | 输入与返回 | Core 与能力 |
| --- | --- | --- |
| `list_package_sources` | `cursor?`, `limit?` → 来源 ID、名称、类型、可用性和受限原因、下一页 | `mcp.packages.query` 的 `sources`，`repositories.read` + `packages.read` |
| `search_packages` | 空筛选可列全量。`query?`, `repository_id?`, `project_id?`, `expected_revision?`, `cursor?`, `limit?` → 包 ID、名称、代表版本及各来源、快照、下一页；项目视图另含当前锁定版本、兼容状态和候选原因 | `query` 的 `summary`；同上，项目视图再需 `projects.read`，`project_id` 与 `expected_revision` 同时提供 |
| `get_package` | `package_id`, `version?`, `source?`, `project_id?`, `expected_revision?`, `cursor?`, `limit?` → 匹配版本的名称、描述、关键词、别名、依赖及范围、被替代包、Unity 要求、撤回状态、来源和可公开文档链接。项目视图另含兼容性 | `query` 的 `versions`；权限及项目参数同上 |
| `plan_package_install` | `project_id`, `expected_revision`, `package_id`, `version_selector: {kind: latest_compatible} 或 {kind: exact, version}`, `source?` → 原 Plan ID、revision、变更摘要和风险 | `mcp.packages.plan` 的 `install`；`projects.read` + `repositories.read` + `packages.read`。使用 Core 当前可见、兼容的候选与原 resolver；不会绕过 yanked、Unity 或预发布规则 |
| `plan_package_reinstall` | `project_id`, `expected_revision`, `package_id`, `source?` → 原 Plan ID、revision、变更摘要和风险 | `plan` 的 `reinstall`；同上。以项目实际锁定版本生成单包重装计划，版本不能漂移，无法取得原版本时明确返回错误 |
| 现有 `apply_package_changes`、`get_operation`、`cancel_operation` | 原参数与任务结果。Apply 只接收 `plan_id`, `expected_revision`, `confirm: true`, `idempotency_key`。独立查询和显式取消 | 既有 `packages.applyPlan` 与 Operation RPC。Apply 在包计划读取能力之外要求 `packages.manage`。查询用 `operations.read`，取消用 `operations.cancel`，见下方一致性整改 |

- `source` 为已登记的 `{kind: repository, repository_id}` 或 `{kind: user_package, user_package_id}`。按下方来源一致性决策，Core 对三端共用来源可见性、优先级、解析、来源身份、下载与归档校验。查询使用既有索引，缺少的包详情字段由 Core 从现有缓存 manifest 投影。用户包按冻结归档与摘要执行，适配器不读取目录。隐藏来源同时从目录版本、直接解析和传递依赖中排除。认证、参数化仓库及本地来源遵守同一规则。
- Core 始终在真实客户端权限及 OAuth token 上限的交集内读受委托目录和项目，仅在确定资源身份时取其登记 owner 作为数据选择条件。Plan、Operation 和幂等记录仍归真实 Principal。计划冻结本次解析的完整来源集合、各自 revision/摘要及项目证据。接受事务重查当前授权、目标身份/版本、每个 pin 的来源及快照，来源可见性设置也在接受前按当前 revision 复核。旧 Plan 失效时提示重查或重新确认，不切换来源或静默重规划。后续任务只执行已接受 Plan 和既有恢复规则；请求取消、连接断开或撤销不能隐式取消已接受任务。扩展 provider 原 Host 工具集合继续使用其原卸载操作。
- 读取默认 100、每页最多 200，包 ID、UUID、revision 和幂等键沿用既有限制。索引分页排序固定，Core 不透明游标绑定当前 Principal、认证上限/授权修订、查询及目录快照，项目视图再绑定项目证据。失效或篡改使用既有 `revision_conflict`，从第一页重新读。目录缓存缺失、歧义、未授权来源和真实无匹配可区分。沿用 Core 有界读取及 MCP 3 MiB 数据预算，目录按条数和字节分页，完整 Plan 超限明确 `plan_too_large`。
- Plan 摘要由 Core 生成，至少列版本变更、触发依赖、来源 ID/名称、冲突原因和计划中清理的相对路径。新查询、Plan、Operation、RPC 错误和事件均不带下载直链、认证头、凭据引用、内部来源 locator 或完整本机路径。文档/变更日志链接只采用现有安全链接投影中可公开的地址。保留已有业务错误与取消语义；真实写响应丢失沿用 `core_write_outcome_unknown`，以同身份、原 Plan 和同键恢复，不自动再次写入。
- 无新增 State、Config、生产依赖或权限名称。原八项能力继续仅为 `Global/*`。同步 Core RPC、工具 Schema、typed client、目录通知与能力筛选。仓库增删、模板、项目/备份、环境与快捷配置继续按对照表逐批补齐。

定向验收使用冻结协议客户端和隔离 Core。验证跨项目的全局目录、单仓库、包详情、同版本多来源、分页快照、最新兼容与精确版、项目兼容候选、已登记认证来源及本地包和依赖、重装保持原版本、风险展示、来源变动后拒绝原 Plan、零授权/逐次撤销/OAuth 上限、跨客户端隔离、同键恢复与在途取消。保留六工具现有卸载回归。

#### 后续批次需集中审批的合同边界

所有者批准继续实施首批。以下按当前 Core 公共接口核对，首批授权未包含这些差异。优先归并公共入口，每种能力只有通过真实用户流程后才计入覆盖。

| 后续用户流程 | 最小差异与推荐边界 |
| --- | --- |
| 仓库查看、添加、移除 | 使用已登记来源按同一 Core 规则。登记和移除仍需准备现有 `repositories.inspect/register/unregister` 的调用者、资源 owner、确认和凭据管理差异。来源可用权限与仓库管理权限各按既有业务范围校验。 |
| 项目登记、创建、复制、备份与恢复 | 核对 `projects.register/create/planCopy/applyCopy`、`backups.create/planRestore/applyRestore` 的本机或 owner 限制。开放相关现有能力时，将授权的默认项目、备份根与资源身份作为 Core 执行条件。根外的外部输入须以本机审批绑定精确目标和请求调用者，不能借用 owner。注册备份和新项目发布沿用现有持久任务及原 Plan。旧 v3 zip 仍归 M11。路径审批只服务实际需要根外输入的流程，接口与持久化差异须另经集中审批。 |
| 模板与环境 | 模板列表提供摘要，`save` 接受完整模板来源。建议使用详情与带预期修订的合并编辑复用保存，写入仍保护内置记录。首次实现只涵盖现行原生模板格式，UnityPackage 与旧模板归 M11。环境复用现有设置及 Unity 读入口。 |
| 长任务和接入 | `operations.get/cancel` 适合继续共用，当前适配器把结果限制为 `packages.apply`。各业务增加明确结果投影；只有真实需要等待的工作才增加绑定任务/修订的 typed input、approve、reject、resume。客户端配置优先交付显式本机引导，隔离校验后仅更新用户确认的目标配置项。需要的 Host/WIT 接口集中评估，扩展仍不持有明文秘密或任意配置写权限。 |

批准前不扩展这些接口、权限或数据库状态。扩展人工延期、普通浏览器受信任 TLS、实际 Codex 订阅消费、持续满载与历史 Host `resource_limit` 按现有责任单独记结果。

**来源一致性决策（2026-11-19）**。所有者要求 MCP 与 GUI、CLI 使用同一业务规则，撤回独立认证来源开关的草案。查包、Plan、Apply 和传递依赖均使用 Core 同一份已登记来源、可见性及解析证据，涵盖认证仓库、带查询参数的来源、本地仓库及用户包。继续逐次检查调用者业务能力和 OAuth 上限，保持 Plan/Operation/幂等归属。完整来源地址、认证材料和内部凭据引用继续留在 Core，对外只投影工具所需的安全字段。

### 2026-11-19 MCP 与 GUI、CLI 一致性审计

同一业务在获得对应授权后应复用 Core 的资格、输入校验、确认、任务状态和结果。对照 Core 分派与用例、MCP 已发布工具以及 GUI/CLI 接线，额外发现以下差异。

| 位置 | 实际差异与后续收束 |
| --- | --- |
| 包计划与结果 | `mcp.packages.plan` 仅接入安装、单包重装，另有单包卸载。GUI 或 CLI 已可升级、降级、重新解析，GUI 还有批量操作。普通计划支持版本范围、预发布选项及显式接受 Unity 版本不兼容的受控例外，MCP 只接最新兼容版或精确版及默认策略。后续共用已有 Core 行为及确认约束。v3 的项目包状态仍需覆盖。进一步核对确认，当前 V4 的 `packagesRequireResolve` 属于备份与恢复，`projects.get` 没有这个字段。 |
| Apply 与任务取消 | 审计时 `packages.applyPlan` 对 MCP 额外要求 `operations.read`，取消任务也先读取详情。任务结果只投影 `packages.apply`。所有者要求整改后，Apply 按共享包写入权限，取消直接调用 Core，读取与取消复用各自能力。任务读取和取消不再按 MCP 工具种类过滤，逐业务保留安全结果投影。 |
| 仓库和其他现有流程 | 外部 MCP RPC 允许列表仍缺仓库检查、登记、刷新及移除，以及项目登记、创建和复制、备份与恢复、用户包和模板管理、Unity 与环境读取等接线。仓库检查预览目前绑连接，HTTP 每次请求独立连接，且现有登记/移除用同一调用者作资源 owner 与幂等归属。需先绑定真实调用者、实际来源和显式确认，再复用同一业务用例。路径输入和本机操作继续由 Core 校验。 |
| 扩展工具 Host | 扩展注册的 MCP 工具只可调用当前窄 Host 用例。内置工具新增的目录或安装能力不会自动授予扩展。需要这些能力时，沿用扩展和调用者双方授权，复用同一 Core 用例并审查必要的 Host 合同。 |

身份独立、OAuth 上限、实时撤销、原 Plan/Operation 归属、破坏性确认以及凭据和本机敏感字段的安全投影仍适用于各自调用者。仓库来源差异已修正。其余公共合同差异合并到本计划的后续批次审阅。

#### 包计划、任务列表及仓库接线（本轮实施范围）

所有者在收到本批明确差异后要求继续。实施沿用以下合同。项目登记、创建、复制、备份、模板、环境读取和客户端引导按 M8 计划另批推进。

- **包计划。** 扩展现有 `mcp.packages.plan` 的 `intent`，新增 `upgrade`（包 ID、可选版本范围、来源、预发布策略）、`downgrade`（包 ID、精确版本、来源）、`resolve`（可选预发布策略）和 `bulk`（复用 `PackageBulkIntent` 的 install/upgrade/remove/reinstall，最多 256 项）。以现有 `project_id` 和 `expected_revision` 锚定一个项目。`install` 增加 `range` selector 及可选 `include_prerelease`，`reinstall` 接受现有的 all/指定包集合与包来源。安装、升级、降级只有在精确版本且显式指定时允许复用现有 Unity 兼容性例外，风险放入原 Plan。用一个 `alcomd_plan_package_changes` 暴露完整的 typed intent，现有三个专用途径继续接受原参数并委托同一计划入口。复用 `mcp.packages.catalog-plan.v1`、现有安全 Plan 投影、读取能力、`packages.manage` 与 Apply，必要时在接收事务显式区分既有扩展 Host 的卸载许可和内置工具的已授权 Plan。无新权限、State、Config 或依赖。验证与本机原 Plan 的变更和风险一致、批量上限、旧参数、OAuth 上限、过期和按同键恢复。
- **任务列表。** 增加 `alcomd_list_operations(cursor?, limit?, active_only?)`，复用既有 `operations.list`、游标和 `operations.read`。返回同一安全任务视图的分页、下一游标及活动数。现有任务查询/取消工具继续使用各自权限，无新 Core RPC。
- **仓库。** 建议使用 `mcp.repositories.list(cursor?, limit?)`、`inspect(url)`、`register(previewId, confirm, idempotencyKey)`、`refresh(repositoryId, expectedRevision, idempotencyKey)`、`remove(repositoryId, expectedRevision, confirm, idempotencyKey)` 及对应固定工具，协商 `mcp.repositories.v1`。只读使用 `repositories.read`，登记、刷新及移除使用既有 `repositories.manage` 的 `global/*` 明确授权。列表和写回提供仓库 ID、名称、修订、来源种类、内置状态和可用状态。远程检查遵循 Core 默认 URL 规则并返回安全摘要及限时不透明句柄，绑定实际认证、精确规范化来源和检查快照，供另一请求确认登记。未命中的句柄、过期或证据变化须重新检查。移除保留内置保护。认证和本地路径继续从原本机入口配置，MCP 仍使用这些已登记来源。资源 owner 与幂等调用者分开，现有 SQLite 通用 grant 结构足够。

### 2026-11-19 首批包目录本地候选

代码 `495cd4dd82aefe0e25afae83c5b6af153cc11d8d` 已接通两个专用 RPC、五个新增工具、协商与目录筛选。新 Plan 的只读投影去掉内部来源 pin，继续在 Core 冻结来源、校验真实认证、原 Plan 和当前可见性，复用既有 Apply、Operation 与恢复。无 State、Config、权限名称、生产依赖或 WIT 变更。兼容增加 RPC/tool 能力，现有六工具输入和输出形状保持。

| 验证 | 实际结果 |
| --- | --- |
| Windows 冻结 SDK、STDIO、真实隔离 Core | 最终包用例 1/0，4.39 秒。验证零授权、全局分页、两身份游标/Plan 隔离、版本详情及项目兼容性、准确安装本地包与依赖、同键回读、重装保持锁定版本和在途连接撤权。最终测试二进制 SHA-256 `11b3fe4eb8eaa7d669e1693c598f7ff31315dbfbb1e396e1e7033eb02ccde2b0`。 |
| SQLite 来源 | 相关单项 1/0。真实索引只允许匿名 HTTPS 两个版本，排除受保护仓库、带下载认证头与带查询参数的直链，排除的未索引仓库不阻塞匿名来源。没有执行实际 HTTPS 仓库下载。首轮 SDK 夹具重复登记同一旧本地包 ID，Store 夹具先缺来源身份再使用非法凭据引用，均按现有持久约束修正夹具，原失败不计通过。 |
| 已有业务路径 | 前一编译候选同轮分别通过旧卸载 1/0、HTTP 在途请求取消和真实写响应丢失 1/0，对应测试二进制 `2dcf3e03fd1d0469b8ad3dfe94f201ed7f9b58f8d77426520f4dbd604cc4593c`。最终窄改仅简化同一来源匹配条件并增加指定受限来源的明确拒绝。 |
| 签名 Provider 相关回归 | 两轮均 0/1，失败在签名扩展安装 Plan 的 `extension_package_invalid`，未到合并目录分页断言。同签名包与早先成功样例的 SHA-256 一致，显式 Host 独立预检在空子进程环境通过，耗时 4004 ms。真实 Core 预检失败的具体原因仍未定位，未调整限制或记为通过。 |
| 静态检查 | 七个受影响 Rust 包的 `--all-targets` 严格 Clippy、fmt、metadata、xtask、两份 Schema 的 JSON 与内部引用检查、diff 均通过。 |

证据位于 `target/m8-package-catalog-20261119/`、`target/m8-package-catalog-regressions-20261119/` 和 `target/m8-package-catalog-final-20261119/`。每次完成测试后均恢复原隔离 Core/GUI、原数据及 WebView 配置，实际比较五组公开状态一致，最终 GUI 129108 可见且响应并连接 Core 124728。没有操作原开发数据、真实工程或 Unity，没有恢复桌面控制。

该候选尚缺普通 HTTPS 仓库实际下载、OAuth 上限专项及签名 Provider 合并目录复验，结果见下方后续记录。二进制本地候选无对应 Hosted CI/CodeQL，旧结果保持原 SHA。

### 2026-11-19 包目录 OAuth 与 HTTPS 补验

`33aca62a5a1efc82702bc4ef6f73fd8435e53435` 修复了 HTTP 准入的本地工具定义只覆盖旧六项的问题。首次 OAuth 用例中，Core 目录按 consent 隐藏新安装工具，HTTP 备用定义找不到该名称，提前返回 404。修复使准入同时从两份已批准 Schema 识别内置工具，目录仍由 Core 权威筛选，实际拒绝沿用 OAuth 的 403 `insufficient_scope`。`d2cf20c249b9001ebc866899344cb7398839324b` 只给测试门控构建增加 Host 预检阶段、时间和退出状态证据。无公共合同、State、Config、权限或依赖变更。

- 最终两个独立 Windows 隔离测试共用 SHA-256 `b82bcac7599261d46146b449c236550257efd6f2c547f4a887f195fb56c3b8a1`。冻结 SDK→回环 HTTPS/OAuth→真实 Core→匿名 HTTPS VPM 源 **1/0，4.25 秒**。全局仓库读取与项目读取按各自授权，读 token 无法生成 Plan，计划 token 无法 Apply，完整 token 仅执行原 Plan。真实 TLS 下载一次，检查归档摘要与安装内容，源端未收到 OAuth 凭据或 referer，同键回读同一 Operation。测试证书仅由门控构建的隔离客户端显式信任，系统信任保持。
- 最终签名 Provider **1/0，95.44 秒**，执行了真实安装、合并目录分页、权限隔离及副作用用例。两个 Host 预检退出均为 0，分别 4031 ms 和 3250 ms。上一个带诊断候选的 Provider 也独立通过 1/0，104.52 秒，其同轮 OAuth 用例保留 404 失败。更早两轮 `extension_package_invalid` 未复现，历史原因仍未确认，未改变原预检限时或签名验证。
- MCP 单元 15/0，四个受影响 Rust 包严格 Clippy、fmt、metadata、xtask、diff 通过。原隔离 GUI 134448 恢复可见且响应，实际连接原隔离数据上的 Core 125516，五组公开状态一致。证据在 `target/m8-package-source-oauth-20261119/` 和 `target/m8-package-source-oauth-final-20261119/`。该候选没有对应 Hosted CI/CodeQL。当时的认证来源单独授权草案已由上方来源一致性决策撤回。

### 2026-11-19 共享包来源验证

代码 `7e3efd244926a3c80c490e5bcbc5a2c4dad74216` 删除 MCP 专用来源筛选。目录和解析、来源可见性、传递依赖及接受前的来源证据核对沿用 Core。内部仍以本机所有者定位已登记来源，以真实调用者检查授权并归属 Plan、Operation 和幂等记录。未变更 RPC/工具结构、State、Config、生产依赖或 WIT。

- Windows 冻结 SDK 与真实隔离 Core 的三项独立回归各 1/0。公开 HTTPS/OAuth 3.23 秒，带参数 URL 与包声明下载认证头的 HTTPS/OAuth 3.28 秒，本地用户包的 STDIO 安装及重装 3.79 秒。两项 HTTPS 用例均比对本机 owner 入口和 MCP 的同一变更摘要，验证真实归档下载一次、同键恢复、OAuth 上限，以及凭据和 referer 未转发到仓库。测试二进制 SHA-256 `ffc2fed2aa222b8ba7154db95808939e220e1446de93317afa2886d487913512`。
- SQLite 单项 1/0，覆盖匿名、凭据引用、参数化及本地仓库的索引与可见性，并验证接受事务拒绝过期凭据引用。实际 OS 凭据存储到 HTTPS 仓库的端到端读取未执行。五个受影响 Rust 包 `--all-targets` 严格 Clippy、fmt、锁定 metadata、xtask 和 diff 通过。
- 原隔离 GUI 123212 可见且响应，经五条管道连接原隔离数据上的 Core 136052。原程序、数据和配置已恢复，五组公开状态一致。证据在 `target/m8-shared-sources-20261119/`。未推送，未执行完整本地套件或获取新远端结果。

### 2026-11-19 包 Apply 与任务权限整改

代码 `dbda0679c3bd3830f1fa29ade3f0dc258116d0e6`。Apply 继续要求同样的包计划读取权限与 `packages.manage`，保留原 Plan、重验和幂等。移除 MCP 额外的 `operations.read` 要求。取消直接走 Core 原用例，需要 `operations.cancel`、实际任务归属和预期 revision。任务查询仍单独要求 `operations.read`。Tool 声明、Core 和 OAuth 准入已同步。内置任务工具沿用相同状态和错误，覆盖包变更和状态检查的安全结果，其余类别返回安全状态摘要。没有新增公开 Core RPC、权限名称、State、Config、依赖或 WIT。

- 首轮 Windows 冻结 SDK 加真实隔离 Core，STDIO 卸载/取消 1/0（7.03 秒）、HTTP Bearer 卸载/取消 1/0（26.61 秒）、OAuth 参数化来源与权限 1/0（3.52 秒）、签名 Provider 持久副作用 1/0（117.32 秒）。实际验证写权限可应用原 Plan，同时无法读取任务；仅有取消授权可以到达 Core，归属与终态校验仍生效；同一身份经 Core 创建的 `state.check` 可读取完整的安全分类。该轮二进制 SHA-256 `c431f7285844e3aceacab93c58462080129a1961d2d0fd1e31ec452e32c05928`。
- 同轮 OAuth 综合用例 0/1，旧断言仍期待 6 个工具，实际为现行 11 个工具。本轮改为核对完整名称并定向复验 1/0（9.27 秒），复验覆盖 OAuth 身份、撤权、权限挑战、请求取消与写响应丢失的同键恢复。期间仅修正测试目录断言，原安全和业务断言保留。复验二进制 SHA-256 `648ea62def0e60c49458831bf0e306e06e30ee5630af5dfc28f6d3241c655ee3`，两轮各自保留结果。
- MCP 单元 15/0，受影响四包严格 Clippy、fmt、锁定 metadata、xtask、MCP 工具 Schema 引用和 diff 通过。原隔离 GUI 115264 可见且响应，五条管道连接原 Core 128300，恢复前后五组公开状态相同。证据在 `target/m8-operation-parity-20261119/` 和 `target/m8-operation-parity-recheck-20261119/`。无完整本地套件或新远端验证，未推送。

### 2026-11-19 包计划、任务分页与仓库管理本地候选

代码 `c3e7912a096eb64f379a78a987cf75e6a136dc9d`。通用包计划接通升降级、重新解析、选择或全部重装以及批量操作，沿用原 Core 校验、来源、风险和显式 Apply。任务列表复用 `operations.list` 的归属、修订、游标和授权。五个仓库工具使用同一登记表和 Core 事务，确认句柄绑定调用者、认证和检查证据，管理授权默认关闭。原登记资源归属与实际调用者的幂等归属分开，保留内置仓库和凭据保护。管理扩展增加已有 `repositories.manage` 的全局开关。无 State、Config、依赖或 WIT 变更。

- 包计划与任务分页真机定向 1/0（8.00 秒），二进制 SHA-256 `2dd490b2fc4ee5ec52331d69ad85b5c1a3aa04c35df4e65d0265cd2ef5434fe6`。真实执行本地来源安装、锁定重装、范围升级、精确降级和混合批量变更，比较原生 Core 升降级及解析 Plan 摘要，验证任务游标和身份隔离。最初升级夹具尝试重复登记同包 ID，Core 按原规则拒绝。后续从本地仓库提供另一版本，保留校验。最后一次变动仅涉及仓库撤权用例断言。
- 仓库 HTTPS/OAuth 最终定向 1/0（2.55 秒），二进制 SHA-256 `d0b9c0d6667749d63f8d03d448425eeb69f1fcb41946bf3926ee7f2f5b5ac667`。验证原生登记可见、实际分页、无确认、他人预览和来源变化时拒绝写入，覆盖刷新、移除、内置 ID 保护、逐请求撤权和原键恢复。只读检查隔离库确认三条仓库写入回执归实际调用者，输入摘要和响应未留存地址或认证材料。保留此前两项夹具失败。其一默认隔离 Core 未预置内置行，后续直接测试受保护 ID。其二将授权撤回后的结构化 `permission_denied` 当作 HTTP 403，复验检查实际 MCP 错误和持久副作用。
- 关联 OAuth 综合与签名 Provider 分别 1/0（8.44 秒和 110.02 秒），对应先前集成二进制 `e3dd20bbf1a840cce00269b5756db049c4396b8ab23bab02abb1b290bf38e5ff`。原生仓库 Store 7/0，九项授权分页 1/0，MCP 单元 15/0，管理扩展单元 8/0及 Wasm Component 构建通过。受影响七包及最终 MCP 适配器严格 Clippy、fmt、metadata、xtask、Schema 引用和 diff 通过。
- 证据在 `target/m8-plan-parity-20261119/`、`target/m8-registry-parity-20261119/`、`target/m8-registry-parity-final-20261119/` 和 `target/m8-registry-revocation-20261119/`。Windows 链接器锁住旧输出文件时改用单独的测试产物名。原隔离 GUI 129908 可见且响应，五条管道连接 Core 86540，恢复前后五组公开状态一致。未推送，暂无同 SHA 远端验证。下一批按既有顺序核对项目、备份、模板和环境能力。

### 2026-11-19 项目与备份集中合同差异（已批准）

Core 已有登记、模板创建、复制、创建备份和恢复。当前登记将登记资源与幂等记录都归到调用者。创建、复制与恢复显式要求本机所有者，Worker 的源资源查询及发布登记也依赖这个身份。备份读取和创建按调用者自己的项目及备份记录查找。外部 MCP 允许列表尚未开放这些入口。原生 RPC 的项目快照、复制及恢复 Plan 带实际本机路径，MCP 沿用安全投影。已核对 `M3Application::{register_project,get_project}`、`ProjectCreateApplication::create`、`M7CopyApplication::{plan_copy,apply_copy}`、`M5BackupApplication::{list,get,create,plan_restore,apply_restore}` 与对应 Store 接受及完成事务。

新增协商 capability `mcp.project-workflows.v1`。现有 SDK 方法、原生请求及结果保留。下表列出 MCP 工具名（均加 `alcomd_`），参数名以工具使用的 snake_case 表达。

| 工具及输入 | Core 方法与结果 | 能力 |
| --- | --- | --- |
| `register_project(source, idempotency_key)` | `mcp.projects.register`，复用既有精确项目检查与登记，返回安全项目快照和 `replayed`。`source` 为 `{kind:default,relative_path}` 或 `{kind:approved,approval_id}`。 | `projects.manage` |
| `list_project_templates(cursor?, limit?)` | 复用 `projectTemplates.list`，返回可选模板 ID、名称、Unity 版本、修订和既有只读标记。 | `projects.create` |
| `create_project(target, unity_version, template?, idempotency_key)` | `mcp.projects.create`，模板用 `{id,revision}`，返回 `operation_id`、`project_id`、`replayed`。继续冻结模板和原 Unity 版本。 | `projects.create`。模板含包变更时继续需要 `packages.read`、`repositories.read`、`packages.manage`。 |
| `plan_project_copy(source_project_id, expected_revision, target, idempotency_key)` 和 `apply_project_copy(plan_id, expected_revision, confirm, idempotency_key, path_approval_id?)` | `mcp.projects.planCopy/applyCopy`。计划返回原 `plan_id`、源项目和修订、目标名称、预分配项目 ID、写入者状态、复制范围、到期时间及指纹。Apply 返回原 Operation 与目标项目 ID。 | 两步均为 `projects.read`、`projects.create` |
| `list_backups(project_id?, cursor?, limit?)`、`get_backup(backup_id)`、`create_backup(project_id, expected_revision, compression_mode?, exclude_vpm_packages?, include_library?, idempotency_key)` | 复用 `backups.list/get/create` 和原有 DTO、默认设置及归档证据。列表及详情返回原安全 `BackupRecord`。创建返回原 `operation_id`、`backup_id`、`replayed`。 | 读取 `backups.read`，创建 `backups.manage` 和 `projects.read` |
| `plan_backup_restore(backup_id, target)` 和 `apply_backup_restore(plan_id, confirm, idempotency_key, path_approval_id?)` | `mcp.backups.planRestore/applyRestore`。使用已登记 `BackupId` 和原归档验证。计划返回 Plan、备份及预分配项目 ID、目标名称、归档摘要、需要重新解析及排除包证据。Apply 返回原 Operation 与项目 ID。 | 计划为 `backups.read`、`projects.create`，确认恢复另需 `backups.manage` |

只开放已有 `projects.manage`、`projects.create`、`backups.read`、`backups.manage` 的 `global/*` 组合，默认关闭。OAuth、管理扩展和 grant Schema 同步。登记、模板和备份查询先检查真实身份。共享登记项目、模板和备份以 Core 的资源所有者定位。新登记内容和登记变更事件也以该资源所有者写入，同一个 ID 可由 GUI、CLI 和获准 MCP 读取。Plan、Operation、任务事件和幂等仍按实际调用者归属。接受和同键重放时在 Core 权威边界重查当前凭据、业务能力、OAuth 上限和原证据。已接受 Worker 按既有持久任务和资源证据执行与恢复。

新目标 `target` 为 `{kind:default,name}` 或 `{kind:approved,approval_id}`。默认目的地使用 Core 已生效的项目根和单一目标名称，登记使用根下显式相对目录。Core 只在已有默认根和已审批的外部路径上校验规范化目录与文件身份，拒绝绝对输入、越界、链接改指向和既有发布目标。备份创建使用已生效的备份目录，恢复使用 Core 已登记并校验身份与摘要的归档。默认根在未接受的 Apply 时重新核对。目标身份或位置授权失效使用既有计划失效或 `permission_denied`，要求重新检查。已经接受的写入继续使用原冻结证据。

新增一个仅本机所有者可调用的 `mcp.projectPaths.approve(clientId, purpose, path, confirm)`。CLI `alcomd-cli mcp approve-path --client <id> --purpose <register|create|copy|restore> --path <absolute-path>` 展示精确路径、用途和客户身份并显式确认后发送 `confirm:true`。对于发布，审批的是最终绝对路径，Core 绑定其已存在父目录身份及单一目标名。登记绑定现有项目目录身份。返回不透明 `approvalId` 与绝对到期时间，10 分钟且最多 64 条待用审批，Core 重启使未接受的审批失效。句柄仅供所指定的真实 Principal 和用途使用，MCP 不能自签。首次写入将句柄绑定本次参数与幂等键，同键可恢复原结果，换键、换 Plan 或跨 Principal 拒绝。复制和恢复的 Apply 仍要求显式 `confirm:true` 并使用原 Plan。句柄只从本机审批回执取得并通过获准请求提交，原生路径、句柄和凭据均不出现在 MCP 目录、业务响应、公开事件或返回的幂等回执。

现有任务页同时投影 `projects.create/copy` 的新项目 ID、`backups.create` 的安全 `BackupRecord`，以及 `backups.restore` 的备份/项目 ID、目标名、重新解析与排除包结果。进度只给现有阶段。当前失败、取消、未知写结果和同键恢复语义保持。无需新 State、Config、生产依赖或 WIT。旧备份和 v3 模板格式仍在 M11。本轮模板接口只提供创建选材。

定向验收覆盖冻结协议或 typed client 到真实隔离 Core。验证五条实际业务、原生端可读的登记结果、来源和目标变化、根外和假句柄、两个客户端隔离、授予/撤销/OAuth 上限、实际任务归属、同键重放、取消保留、恢复安全及既有原生回归。不安装新依赖、不触发远端工作流。

审批前在 `c3e7912a` 取得独立原生验证。`alcomd-store --test backup_directories` 2/0，验证默认目的地变化后原接受归档仍按已冻结证据恢复，以及旧请求可读取。`alcomd-testing --test m7_project_copy_rpc project_copy_plan_expiry_and_all_writer_states_are_fail_closed` 1/0（0.64 秒），验证复制 Plan 过期和 Unity 写入者条件。两组都在可丢弃数据上执行，用于确认可复用的原生行为。

#### 项目与备份本地实现和验证

所有者明确批准了本节合同。代码 `aa3a8c5f7ea86a8be56bbeef7ba27d33b7388387` 接通十个 MCP 工具、typed client、Core 授权与共享登记、路径审批 CLI、已有任务结果及管理扩展开关。公开方法是兼容增加，原生 RPC 的请求、输出和既有任务保留。继续使用现有 State、Config、生产依赖和 WIT。

- 真实 Windows typed RPC 用例 1/0。两名客户端验证默认根登记、外部批准后从内置模板创建、确认原复制 Plan、备份、恢复到新目录、共享资源查读、实际任务归属、跨客户端拒绝、同键恢复和已连接客户端撤权。恢复结果未返回目录路径。
- 冻结 SDK 经实际 HTTP Bearer 与隔离 Core 的定向用例 1/0。覆盖零授权拒绝、登记工具、备份创建、任务结果和备份详情，以及原连接撤权拒绝。两个已执行的二进制及 SHA-256、终态和运行日志见 `target/m8-project-workflows-20261119/project-workflows-results.json`。新工具在 OAuth 下的完整端到端、作业取消及真实丢包注入未在此轮再跑。已有旧输入证据各自保留。
- Store 备份目录 3/0、Plan 路径证据单项 1/0，Core 路径审批定向 1/0，MCP 单元 16/0，管理扩展单元 9/0 和 Wasm release 编译通过。受影响严格 Clippy、fmt、锁定 metadata、xtask、Schema 内部引用及 diff 通过。
- 首次沙箱 RPC 测试 0/1，停在连接受保护管道前置。首次直接复验检查发现唯一真实 Windows 端点正在使用，在启动前停止。之后核验精确进程归属和 0 活动任务，仅暂停上轮隔离验收窗口，执行以上两项后恢复同一程序、数据和 WebView 配置。GUI 138568 可见、响应且经五条管道连接 Core 135320，五组公开状态逐项相同。未操作原开发数据、真实项目或 Unity。

工作树使用本地 `main`。新候选暂无对应 Hosted CI 与 CodeQL。M8 继续进行。

### 2026-11-19 项目详情、模板和环境接口审计

| 用户流程 | 可复用的 Core 行为 | 后续公共合同须明确的最小差异 |
| --- | --- | --- |
| 读取实际安装的包及判断是否需要解析 | 候选页已经用项目内 `Packages/<id>/package.json` 与锁定 ID、版本核对展示信息。包计划检查器可识别锁定包的实际缺失，目录查询可以返回同版本的可用来源。 | 建议三端共用项目包读取能力，沿用 `projects.read` 和 `packages.read`。返回锁定版本、实际观察状态及已核实的包信息，具备来源证据时才提供来源。解析状态基于已观察的目录、声明和锁记录。不完整或变化的观察标为 `unknown`。 |
| 查看和修改项目模板 | `projectTemplates.list` 只给摘要。`save` 可从文件、项目或手动定义保存。用原模板作基础时保留资产树、UPM 清单和 VPM 锁，直接依赖只追加或覆盖，无法精确移除。归档已校验并按摘要冻结。 | 建议新增共享 `projectTemplates.get(id)` 与 `projectTemplates.edit(id, expectedRevision, changes, idempotencyKey)`。详情返回标识、修订、可编辑性、名称、Unity 版本、直接依赖和原生附件目录。编辑以明确增删改修改现有归档，复用原保存任务、归档校验、发布及登记事务。锁定模板拒绝修改、移除。现有原生附件是归档里的内容，没有独立附件登记；编辑只限模板归档 `Assets/` 中显式选择的原生资产及其关联 `.meta`，新增输入使用本机所有者批准的文件，不接受任意路径。UnityPackage 导入行为及旧模板格式仍属 M11。模板浏览与编辑需要各自的默认关闭能力，创建项目所需的模板选择权保持独立。 |
| 读取有效环境 | Unity 安装列表已有真实登记及基于 Core 的检查。有效启动参数由 `unity.projectLaunchConfig.get` 对应的 application 用例按项目设置或默认值取用。项目和备份目录共用已解析的设置与平台默认值。 | 所有者要求按 Core 权限对齐三个入口。MCP 直接调用已有 `settings.get`、`unity.installations.list`、`unity.projectLaunchConfig.get`，获得对应的原有结果，包括 Unity 安装位置和完整有效启动参数。 |

下节记录项目包详情和模板编辑的共享合同。项目与备份的路径预览句柄覆盖其各自列明的用途。模板编辑使用现有文件校验与保存任务。

#### 下一组合并公共合同草案

所有者回复继续实施以下已列明的差异。GUI、CLI 和 MCP 使用同一 Core 用例、输入和结果。MCP 连接成功后可使用已发布的工具。

| 能力 | 建议公开差异与验收证据 |
| --- | --- |
| 项目实际安装包 | 新增共享只读 `packages.projectInventory({projectId,expectedRevision,cursor?,limit?})` 与同结果 MCP 工具 `alcomd_list_installed_packages`。复用候选页已有的项目身份和本地观察，区分声明、锁定、观察版本。缺少文件、版本不符和无法读取给出各自结果。解析状态使用可核实的本地证据。分页按包 ID，默认 50、最多 200 项，游标绑定调用者、项目修订、实际观察指纹。元数据和来源只返回本地可证实的部分。沿用 `projects.read + packages.read`。 |
| 模板详情与编辑 | 新增共享 `projectTemplates.get({id,cursor?,limit?})`、`projectTemplates.edit({id,expectedRevision,changes,confirm,idempotencyKey})`，对应 MCP 查看和编辑工具。详情返回模板名称、修订、Unity、直接依赖和按路径分页的附件目录。编辑只改明确指定的名称、依赖与附件，沿用模板保存 Operation。查阅复用模板列表的 `projects.create`，编辑复用 `projects.manage`，保留锁定模板保护、原模板登记归属和修订检查。 |
| 模板附件文件 | 编辑输入指明源单文件和归档 `Assets/` 下的目标相对名。三端共用 Core 文件校验与确认。拒绝链接、路径逃逸及 Core 私有数据目录，保留未修改内容及既有资产配对规则。Core 在接受任务前冻结实际材料，只在持久任务中保存可恢复的摘要和目标绑定。接受后从冻结材料恢复，源文件变化或重启无需重新读原路径。旧模板与 UnityPackage 格式保留到 M11。 |

以 `packages.project-inventory.v1` 和 `project-templates.edit.v1` 协商。模板能力名使用现有握手允许的小写形式。沿用现有权限名称、配置、数据库 Schema 和 WIT。模板编辑保留实际调用者的 Operation 和幂等归属，原任务 journal 引用已冻结的归档。[共享 RPC Schema](../../specs/rpc/m8-project-assets-v1.schema.json)

#### 模板创建、导出与移除的共享合同

冻结 v3 的 MCP 能创建及移除用户模板。现行 MCP 已能列出模板并从模板创建项目，模板读取与编辑采用上节共享合同。`projectTemplates.save` 与 `projectTemplates.remove` 仍要求本机所有者，GUI 有相应入口，CLI 尚无对应命令。编辑已存在的模板无法替代创建或移除。

所有者批准三端共用，并采用同键恢复移除结果。复用现有 RPC 与 `projects.manage`，通过 `alcomd_create_project_template`、`alcomd_export_project_template` 和 `alcomd_remove_project_template` 接通 MCP，CLI 使用同一用例。创建沿用手工定义、现有项目及模板文件作为来源，由 Core 执行既有来源和归档校验，返回原 `projectTemplates.save` Operation。替换要求原记录的修订，路径和资源规则与 GUI 保持一致。移除仍按模板 ID 与修订检查，并保持内置保护。为 GUI、CLI 和 MCP 的响应丢失恢复，在共享 `projectTemplates.remove` 请求中增加必需的 `idempotencyKey`，在现有幂等表和同一个移除事务内保存原回执。重复原请求返回原 ID 与修订，换请求使用同键拒绝。实际调用者持有回执和任务，模板登记仍属于现有资源所有者。MCP 在现有业务入口提示确认后调用。

导出复用带幂等键的 `projectTemplates.export`、现有任务、输出格式、目标校验及禁止覆盖规则。删除请求新增必需字段，是当前未发布候选的破坏性变化。无 State、Config、WIT 或生产依赖增量。定向验证覆盖共享结果、内置保护、跨身份任务隔离、同键恢复以及移除后同名重建。

#### 环境读取按三端权限对齐

所有者明确要求 MCP、GUI、CLI 对齐能力范围。Core 依据身份及授予的能力决定结果。复用原三个 RPC、原请求和结果类型。把现有 `settings.read`、`unity.read` 接入 MCP 显式授权与 OAuth 同意。项目启动参数继续用原 `projects.read + unity.read` 拿到完整默认、覆盖和有效值。已登记安装查同一资源，调用身份保持原 Principal。GUI、CLI 原有设置或 Unity 写入走既有授权。 [工具映射](../../specs/mcp/toolset-v1.md#读取设置和-unity-环境)

`3ec540fe` 的 Windows 真实隔离 Core 与冻结协议 SDK 定向用例 1/0。原生所有者、受授权的本机 typed client 和 HTTPS/OAuth MCP 读取同一设置、目录与完整启动参数。用两条隔离的预校验安装记录核对路径、按页读取及版本选项。零授权被拒绝，OAuth 同意缺少 Unity 读取能力时返回 403。撤回 Unity 授权后，存续客户端读取启动配置被拒绝，设置读取保持有效。测试二进制 SHA-256 `b18282f1fbbc7f6e81738e90168bf7606eeb8d48ef062aeea6e891bb3e24900d`。MCP 单元 16/0，管理扩展单元 9/0、doc-test 1/0，Core、CLI、MCP 和扩展严格 Clippy、扩展 Wasm、fmt、锁定 metadata、关联 JSON 引用及授权枚举、diff 检查通过。原隔离窗口恢复，五组公开状态一致。证据位于 `target/m8-environment-parity-20261119/`。本轮覆盖读取合同，实际 Unity 自动发现和桌面交互沿用先前独立验收。

`24c4f36d` 对齐 `projects.read` 返回值。项目列表及详情带同一 Core 登记的 `root_path`，保持已有权限、资源所有权、Plan 和 Operation 归属。严格解析旧输出的客户端需使用新 Schema。真实 Windows HTTP 业务用例首轮 0/1，聚合断言仍禁止在任何响应中出现路径字段。更新为只在授权成功的项目读取数据中允许路径，同时保留所有凭据与其他工具的路径断言，复验 1/0。分别将列表和详情路径与本机原生登记比对，并保留零授权、撤权和任务归属检查。最终测试二进制 SHA-256 `e25e11bec793d8b1891e1fbc93ee8d37b8240c83fa023eb2714ca273db0d6fcb`。两轮证据在 `target/m8-project-path-parity-20261119/`，原隔离窗口恢复，五组公开状态一致。

协议 Schema 套件 52/0。管理扩展合同首轮 3/1，旧测试组件的源码摘要与当前授权页面源码不符。`9204a352` 用现有构建与编码入口重建测试组件，核对其余冻结输入，更新当前组件及两个摘要，合同复验 4/0。新组件 SHA-256 `bbc87a71fc804e42ef74416ab305637ba0b933a2d1eb7b79afcc1a3fec4f5dfe`，保留其余旧组件及来源。真实 Windows Core/Host 加载重建的签名测试组件并验证观测授权，具名用例 1/0，二进制 SHA-256 `12aaf7e4a7d0956fe6611f6574f64ba4ade1b5ca99d4f3bbc9d27f22b8bc2f68`。原隔离窗口与五组公开状态已恢复。以上候选尚无对应远端结果。

### 2026-11-19 Codex 快速接入核对

管理扩展的已登记客户端详情加入只读接入提示。继续使用原 CLI 签发一次性凭据。扩展既有权限、Host/WIT、配置和写入入口保持原合同。[实际操作](../../specs/mcp/toolset-v1.md#codex-cli-01534-的回环-http-接入)给出 Codex 回环 Bearer 的配置核对、注册、凭据传入和清理步骤。

本机 Codex CLI 0.153.4 在独立 `target/m8-codex-config-d23e782e80a047fd9e3c96ae40e1e4f4` 配置中实测添加、读取、同名替换、移除。其他配置和条目在该夹具中保持原字节，移除恢复整个原文件，同名替换没有内置确认。写入内容只引用凭据的环境变量名。测试使用无监听的回环示例地址和无凭据配置，未重新验证端到端调用；真实发现、调用及撤销仍引用 2026-11-17 的独立记录。Claude Code 和 Cursor 在当前执行环境的命令路径和检查的常见位置未定位到可执行入口，兼容性仍未知。自动配置需要覆盖预览、显式确认、并发变化保护和保持其他配置的独立验证，当前交付手动配置。未连接或替换原 Core、GUI 和配置。

接入提示与说明的候选为 `84a2e53f`。管理扩展单元 9/0，严格 Clippy、Wasm release 构建、fmt 和 `cargo xtask check` 通过。此候选尚无真实桌面人工验收或远端结果。

### 2026-11-19 项目与备份风险复核

- `ec99aa05` 同步项目复制的已有 Core 错误及工具错误 Schema。真实冻结 SDK 调用在目标目录已存在时返回 `project_copy_target_exists`。在 Core 已接受备份后，测试代理截断真实 HTTP 回包。SDK 报传输失败，原生备份列表先观察到已完成归档，再由同一客户端显式使用原键找回同一备份及任务。无重复归档。真实 Windows HTTP 用例 1/0，测试二进制 SHA-256 `946516c7ecccf0586ed507786802ba463a44abbbd6dccc7b85cd46325f9b7a4e`。
- 同轮 OAuth 首次 0/1。前半段验证只有读权限的 token 直接写入返回 403 且无归档。失败在夹具误将有效 token 撤销当前 Core 写权限后的业务拒绝预期为 HTTP 403。修正该项后单项 1/0，验证具备写 scope 时可接受备份，显式同键返回原回执，随后撤回当前 Core 写权限，同 token 得到结构化 `permission_denied`，归档仍只有一份。最终测试二进制 SHA-256 `e5b271fa0e5cb2cbc7dbb48efd413f194f61c88b38c658b87d4c178812061598`。
- `78ef488` 复用仅在测试构建启用的 IPC 回包门闩。备份被 Core 接受后暂缓真实回包，由冻结 SDK 取消仍在途的 MCP 请求。原 SDK 在门闩释放前仍可读取项目。释放后以原键找回相同 Operation，任务成功且未重复写归档。同轮复验原 STDIO 包 Apply 取消和截断 IPC 回包，两个用例各 1/0，二进制 SHA-256 `b7a160fd174a040f1b98094df6c01fd16b87b7bd1df07c7989d0019d98e8bd1d`，记录位于 `target/m8-project-backup-cancel-20261119/`。
- MCP 单元 16/0，受影响严格 Clippy、fmt、锁定 metadata 和 diff 检查通过。日志及各轮二进制摘要分别位于 `target/m8-project-backup-risks-20261119/` 与 `target/m8-project-backup-oauth-recheck-20261119/`。每轮恢复同一隔离 Core、GUI 和 WebView 配置，五组公开状态逐项一致。没有操作原开发数据。

### 2026-11-19 Claude Code 回环接入

`17dc4a65` 加入明确选择才运行的真实 Claude Code 连接用例和手动接入步骤。Windows x64 2.1.327 使用 Anthropic 官方 npm 分发包，包完整性与发布元数据一致。可执行文件 SHA-256 `d6c8075bfb3b87f1c1254e64b6c35dadc628d21e75930ae51e47047b3f05a4a2`。首次用例 0/1，隔离数据路径未经规范化，Core 在启动前拒绝。修正夹具后单项 1/0，测试二进制 SHA-256 `59066769877dc551c42c5c12759e57ed876aa647c56dccbe022e3e050e131295`。

测试在独立的 Claude 配置中登记回环 HTTP，使用仅注入子进程的独立 Core 凭据，验证 `mcp get` 报告 Connected。同名登记被拒绝，无关配置保留，移除后目标条目消失。配置只含环境变量引用。没有登录 Claude 账户、修改系统信任或改动用户配置。原隔离窗口恢复，五组公开状态一致。具名结果位于 `target/m8-claude-code-compat-20261119/`。STDIO、OAuth、模型工具调用、自动配置及 Cursor 的实际接入仍未验证。 [实测接入步骤](../../specs/mcp/toolset-v1.md#claude-code-21327-的回环-http-接入)

本机整仓 `cargo xtask check` 失败，18 条路径均属于已存在的嵌套 Git worktree `.local/worktrees/alcomd-deep-links`。检查器扫描该独立工作树后用主仓路径比较 unsafe 白名单。保持该工作树、扫描规则和原断言。本批无新增远端结果。

### 2026-11-19 Cursor 具名版本核对

本轮从 Cursor 官方 Windows 安装端点取得其指定的 x64 CLI 版本 `2026.11.17-f532c45`。包 SHA-256 为 `74c2bc22d0f1b8e424fa8bc5cf8ffc756ebc0e89706be5cdbfd56c57e4b9eaf8`。仅解包到 `target/m8-cursor-compat-20261119/`，调用包内 Node 和 CLI 入口，配置、项目和缓存均在一次性目录。没有运行安装脚本、登录或改写用户配置。 [官方安装入口](https://cursor.com/docs/cli/installation)

首次 `mcp list-tools` 未找到夹具配置，没有发请求。配置放入隔离用户目录的 `.cursor/mcp.json` 后，实际 CLI 找到条目。HTTP 的 `mcp list-tools` 和 `mcp list` 共观察到三次没有协议头的 `initialize`，请求版本都是 `2025-11-25`，随后退到 GET。独立 STDIO 夹具又观察到同版本的两次 `initialize`。具名版本的这两种传输均无法按 ALCOMD 冻结的 `2026-07-28` 协议完成握手。探针只记录协商，未连接真实 Core；OAuth 与其他 Cursor 版本未验证。结果和运行脚本保存在该目录。HTTP 结果与厂商此前说明的版本行为一致。 [Cursor 协议说明](https://forum.cursor.com/t/mcp-client-cannot-connect-to-modern-only-2026-07-28-streamable-http-servers-legacy-initialize-rejected/172536/5)

### 2026-11-19 当前能力与证据清单

| 用户能力 | 现行入口与证据 | 仍需处理 |
| --- | --- | --- |
| 查看项目、查包、选来源、安装和包变更 | `alcomd_list_projects` 与 `alcomd_get_project` 返回 Core 登记与路径。查包、来源、安装、升级、降级、重装、卸载、解析与合并 Plan 由同一 Core 负责。真实包与仓库定向候选 `c3e7912a`，项目路径和原卸载回归 `24c4f36d`。`e7c2c526` 增加本机实际安装观察和解析状态，见下节。 | 已安装观察面向登记项目中声明或锁定的 VPM 包。可从包目录取得的元数据见既有查包流程。 |
| 查看、登记、刷新和移除仓库 | `mcp.repositories.v1` 与关联工具，真实 HTTPS/OAuth 仓库用例 1/0。Core 按登记来源使用原凭据规则，内置仓库继续受保护。候选 `c3e7912a`。 | 既有登记能力可以使用，来源地址和凭据管理仍遵循各自已接受的合同。 |
| 登记、创建和复制项目、备份与恢复 | `mcp.project-workflows.v1` 共用 Core 业务和 Operation。`aa3a8c5f` 的原生与冻结 SDK 定向各 1/0，OAuth 撤权、同键恢复与在途取消分别见 `ec99aa05`、`78ef488`。 | 最终需按同一候选汇总适用门禁。 |
| 模板定义、依赖及附件 | 三端使用共享 Core 目录、详情、编辑、创建、导出和移除。创建项目可使用同一模板记录。当前格式的附件、资源归属、任务和移除重放已通过真实隔离验证。 | 旧模板、UnityPackage 和旧备份格式按既定 M11 边界。 |
| 设置、已登记 Unity 和有效启动参数 | `3ec540fe` 复用既有设置、安装和项目启动参数读接口。真实隔离 Core 验证权限和结果一致、OAuth 上限与撤权。 | 已有桌面验收继续有效。 |
| 任务状态、取消、恢复及观测 | `alcomd_list_operations`、`alcomd_get_operation`、`alcomd_cancel_operation` 已接通。项目与备份已有真实身份归属、回包丢失及在途取消证据。工具目录订阅、只读连接和请求观测有既有冻结 SDK 与 Host 证据。 | 当前已接通流程的确认在接受任务前完成。若后续真实业务需要任务在途输入，再为该业务核对继续入口。真实 Codex 推送消费、持续满载和 TCP 发送缓冲耗尽未验证。 |
| 客户端快速接入 | Codex 0.153.4 具名回环 HTTP Bearer 和预登记 OAuth，Claude Code 2.1.327 具名回环 HTTP，均有独立配置证据。 | Cursor 本轮具名构建的 HTTP 和 STDIO 握手不兼容，自动配置、普通浏览器受信任 TLS 仍未验证。 |
| 管理扩展、分发及撤销能力 | 管理、权限开关、活动查看及签名测试组件具备定向证据。A-026 的旧日志入口处于已撤销状态。 | 扩展人工体验和普通在线来源按原安排延期，正式签名和装配归 M12。历史 Host `resource_limit` 仍未定位。 |

`06dd8b03` 修正仓库检查沿 `.local/worktrees` 进入独立 Git 工作树的误报。检查器只排除 Git 实际登记且仍存在的该位置子工作树，主仓路径和安全断言保持。实际主仓 `cargo xtask check` 通过，在同一父目录放置普通未登记的测试 Rust 文件时能识别违规，撤回该临时文件后同一检查再次通过。xtask 严格 Clippy、fmt 和 diff 通过。原 18 项失败属于改动前，历史记录保留。

本地稳定代码和各轮门禁见下文。现有远端 `2735119f` 的 macOS 作业未执行，原 CodeQL 与 Windows、Ubuntu 结果仅对应该 SHA。新候选仍需同 SHA 三平台 CI 与 CodeQL，M8 保持进行中。

### 2026-11-19 MCP 启用并连接后使用工具

所有者明确要求 MCP 启用并成功连接后，客户端可以使用当前发布的全部 MCP 工具。撤回为包详情、模板等功能追加逐项 MCP 权限开关的提案。按既定三端共用规则，Core 负责项目、模板、附件和任务的业务校验、操作确认及归属。扩展自行申请和使用的 Host 权限仍由现有扩展规则决定，工具的发布、启停和可用状态由实际提供方决定。MCP 连接使用既有认证身份及撤销能力，所有权和幂等归实际调用者。

旧实现曾在 Core、Store、工具目录、HTTP、OAuth 同意和管理扩展分别筛选 MCP 客户端权限。连接级实现移除这套筛选和开关。OAuth 同意展示可使用全部已发布工具。

所有者确认不存在需要过渡的旧限权令牌。沿用现有流程创建身份、签发连接凭据、认证、轮换和撤销。上方旧候选的测试记录对应原逐项授权模型。

#### 连接级工具访问的公共合同

- 客户端 RPC 保留 `list/create/issueCredential/revoke`，移除 `mcp.clients.setGrants` 及创建请求、客户端记录中的 `grants`。以 `mcp.clients.v2` 协商这一破坏性变更。管理扩展切到 `alcomd:mcp-management@2.0.0` 的同类列表、创建和撤销接口，移除逐项授权列表、增删、授权计数及该页面的旧项目选择接口。扩展访问客户端管理继续使用既有 `mcp.clients.read/manage`，保留租约检查。
- OAuth 将业务权限选择合并为单一连接 scope `mcp`。令牌响应与元数据使用该 scope，审阅以整个 MCP 访问为单位保留允许或拒绝。审阅和决定 DTO 移除逐项 grants 与授权项分页，以 `mcp.oauth.v2` 协商破坏性变更。保留预登记、PKCE、资源与回调绑定、到期、轮换和撤销。Bearer、STDIO 与 OAuth 成功认证后可发现并调用全部当前发布的工具。Core 继续控制工具入口、任务归属、凭据生命周期、扩展自身权限和具体操作流程。
- State44 增量移除 `mcp_client_grants`、`mcp_clients.grant_revision`、`mcp_oauth_tokens.grant_revision/grants_json`，删除附属的快照触发器。保留身份、凭据摘要、OAuth 注册和令牌生命周期字段、Plan、Operation 与历史迁移。隔离库用例比对其余数据和事件序列，覆盖迁移中途回滚及重开。
- 验证各传输认证后拥有相同工具目录，刷新与撤销行为、跨客户端任务隔离、显式执行与同键恢复继续生效。模板查看与编辑按共享 Core 工具处理，取消为 MCP 新设 `templates.read/manage` 开关的提案。

客户端 RPC、OAuth 同意和管理 Host v2 是破坏性变更。旧 MCP 业务授权字段和方法不再识别。现行管理 Guest、运行态观测 Guest 及隔离传输 Guest 使用新的管理 WIT 类型，原扩展基础 world 和观测数据形状保持既有合同。项目包详情和模板编辑仍按三端共用的下一组差异推进。

代码 `9235349e72d8d5d3ee5a879e0d1c8b3f86a7e797`。本地定向验证已通过 Store 连接级与 OAuth 等 13 项、SQLite 迁移与权限回归 57 项、应用层 M8 16 项、MCP 适配器 16 项、OAuth Schema 1 项、管理合同与 Wasm 摘要 4 项、真实独立 Host 1 项、Guest 单测 7 项与 doctest 1 项。Core 读入现行工具合同逐项核对所需能力，全部已发布内置工具均通过。受影响包严格 Clippy、格式和仓库一致性检查通过。早期 Store 12/1 和迁移 49/8 为独立失败轮次，修正了旧授权断言及只适用于当前初始化的 State44 预期后复验。

此前旧隔离窗口占用 Windows 当前用户唯一 Core 端点。窗口释放后，使用真实 Windows 用户和可丢弃隔离数据验证，原环境未升级。

- `9235349e` 生产代码的首轮真实身份用例 0/1，业务套件 10/9、3 项原有忽略、自然到期用例未执行。跨客户端 Plan 应按所有者查不到，分页断言应读取完整目录，当前版本应为 State44。改正后身份单项 1/0，9 项业务定向复验 7/2。仍失败的两项分别是 HTTP 写前撤销返回含糊的目录错误，以及仓库用例继续预期某枚已认证令牌无权取得登记预览。
- `ce1e93dea64d674c34192689b5c302537d422ec5` 将同一 HTTP 工具元数据读取放到 SDK 发送 SSE 响应头之前。真实回环在 Core hello 成功、请求正文仍未发完时撤销凭据，返回 401，且 Core 的 Apply 调用计数保持不变。最终四项定向复验 4/0，覆盖 HTTP 卸载、在途取消和实际写回包丢失、真实 HTTPS OAuth 业务及三传输完整目录一致，以及仓库同身份不同令牌预览隔离与同键恢复。适配器单测 16/0，严格 Clippy、格式及仓库一致性检查通过。首轮和复验分别保存在 `target/m8-connection-identity.log`、`target/m8-connection-business.log`、`target/m8-connection-business-recheck.log` 与 `target/m8-connection-final-risks.log`。
- 真实 Core/Host 定向单轮 6/0，覆盖客户端登记与撤销、扩展零授权启用和数据隔离、连接与请求观测、租约失效及打包扩展安装一次和保留用户卸载。记录在 `target/m8-connection-management-real.log`。

自然到期长时用例及原有忽略项仍待对应验证。新增代码尚无远端结果。

### 2026-11-19 项目包观察和模板生命周期

签名代码 `e7c2c526e5ae8a31d0752a9a4117c77312e69c8d` 接通三端项目包观察、模板详情和编辑。项目详情的包操作菜单提供本机状态，明确区分缺失、版本不符和未知。资源页的模板操作提供详情与编辑，写入沿用现有任务。附件限现行归档的 `Assets/`，Core 在接受前冻结材料。CLI 入口为 `package inventory`、`template get` 和 `template edit`。

签名代码 `539d81857aa90215569501c0a2861e6e8a0552da` 落实获批的创建、导出和移除。GUI 沿用资源页，CLI 加入 `template list/save/export/remove`，MCP 使用[工具清单](../../specs/mcp/toolset-v1.md#项目已安装的包和模板编辑)中的公开入口。模板移除使用原预期修订和原幂等键找回同一回执。创建、导出使用既有任务，实际调用者保持回执和任务归属。

- 真实 Windows 隔离 Core 和冻结 SDK 最终定向 2/0，测试二进制 SHA-256 `dd5e003f00c5717225ebedaddf5b41d9385c5de49114fd9474a705d86f56c8be`。覆盖包文件缺失、版本不符和观察变化使续页失效。确认模板内置保护、修订失效、私有附件拒绝、真实导出字节、项目来源、导入、重建和跨身份归属。截断提交后的真实 IPC 回包，SDK 得到 `core_write_outcome_unknown`，同键找回确切回执。同 ID 重建后再次重放保留新模板。首轮握手拒绝、SQLite 指纹 JSON 检查拒绝以及用于定位的失败轮次保留。
- 相关 Chromium 模块首轮 46/1，入口移入原包操作菜单后原工具栏高度断言通过。次轮 47/2，失败来自测试用原生对话框节点访问 Material 插槽内的控件。按现有宿主选择方式修正后，三条定向 3/0，三个相关模块同轮 49/0。模板生命周期新增同键重试后模块单独 16/0。Schema 首轮 52/1 为新 Schema ID 路径缺少 RPC 版本，按既有要求修正后 53/0。
- 具名结果保存在 `target/m8-shared-project-assets-20261119/`。本轮端点空闲，真实用例使用独立 Core 和数据并正常退出。原 Core、GUI、数据和 Unity 未启停或迁移。

完整 Chromium 使用 `539d8185` 的前端输入，734 项同轮结果为 730/4。四项均触发原有 30 秒全用例时限，分别是 GUI 路由遍历、高影响操作遍历、800px 布局和项目卡片布局。保留完整日志、失败截图与 trace。Rust 构建结束后仅复验原四项，使用一名浏览器工作线程，原断言与时限均保持，结果 4/0。完整首轮保持 730/4。

Windows 完整仓库门禁首轮在严格 Clippy 中止。新增 Schema 用例的两处 `unwrap` 不符合仓库 lint 规则。签名提交 `218d14b6b8effcbc910a0f386e0cbe0608bf9a08` 仅增加失败诊断，两处仍执行原断言，业务代码与前端输入一致。

恢复后的完整仓库门禁通过严格 Clippy，原生模板 IPC 用例通过。工作区测试在真实扩展 Host 生命周期模块中止，模块结果 8/4、1 项忽略。四次失败均由 Host 等待回包的约 4,999 ms 激活预算触发，原错误为 `extension_resource_limit`。Host 原先先发送 Ready，再编译和实例化组件，导致冷启动工作占用激活窗口。修正将 Ready 移到组件准备完成之后，沿用分别对启动和激活生效的 5,000 ms 上限以及原 WIT 与运行态限制。现有测试诊断增加阶段，便于区分 Ready 与激活超限。

`7548e77cc583ea5dbce3df87cc26603d9b061dc4` 的真实生命周期模块定向 12/0、1 项原有忽略，独立 Host Component 模块 6/0。新增用例确认无效组件退出且未发 Ready。更早的 `resource_limit` 轮次保留原各自记录。

`f49f7fbdf1842117b28cf562a710a68327c70983` 补上模板编辑对附件和依赖可能有移除效果的工具标记。Schema 结构比较确认只改变该工具的提示属性。MCP 模块 17/0，受影响严格 Clippy 通过。

`7548e77c` 的下一轮整仓运行中，真实 Host 生命周期 12/0、1 项原有忽略，MCP 业务集成 21/0、4 项原有忽略。运行在项目操作静态合同模块中止，模块为 6/3，三项均把现行 StateSchema 断言成 43。全仓核对现行断言后，`522dabc58ec270a375041fa44645be5b3a1a9a21` 只同步这三处为 State44，定向复验 9/0。历史迁移、RPC、权限、数据保全和安全断言继续逐项检查。

### 2026-11-19 M8 本地候选验收

- Windows 完整 Rust 工作区在代码 `522dabc58ec270a375041fa44645be5b3a1a9a21` 单轮执行 `cargo test --locked --workspace -j 3 --no-fail-fast`，`RUST_TEST_THREADS=2`。日志记录 99 组测试与 doctest 结果，合计 1096 通过、0 失败、18 忽略。原始日志为 `target/m8-shared-project-assets-20261119/workspace-final-complete.log`。
- 同一代码的 `xtask check`、fmt、整仓严格 Clippy、Windows 原生 GUI `cargo check`、两扩展后端 fmt 与严格 Clippy、前端检查和构建、metadata、锁文件及 diff 检查通过。MCP 管理扩展后端 7 项单元、1 项 doctest 通过。Discord 后端命令正常结束，共发现 0 项测试，本阶段没有 Discord 业务验证。完整输出为 `target/m8-shared-project-assets-20261119/workspace-final-static-and-frontend.log`。本轮未构建本机 Tauri Release。
- Chromium 使用 `539d8185` 至候选未变化的前端工作区、根 package manifest 和 lockfile。完整单轮 730 通过、4 超时，独立定向复验 4 通过。新的同 SHA 完整远端浏览器结果待验证。
- 管理扩展人工体验、普通在线扩展源继续按所有者决定延期。真实 Codex 工具目录推送消费、普通浏览器受信任 TLS、持续满载及 TCP 发送缓冲耗尽未执行。此前复现的 Host 激活预算问题已修正并验证，早期其他 `resource_limit` 记录仍保留。当前 Cursor 具名版本协议不兼容。旧模板、UnityPackage 和旧备份格式按 M11，正式签名和发行装配按 M12。A-026 的日志能力保持撤销。
- 最终远端验证要求同一候选 SHA 的三平台 CI 和 CodeQL。此前 `2735119f6245d7c97552b2c19c5f467c5c0ad4ae` 的 Windows、Ubuntu 及 CodeQL 结果继续保留，macOS 未执行；该批结果仅适用于原候选。M8 收束等待新候选远端证据和所有者确认剩余事项。

### 2026-11-19 M8 候选远端验收

所有者批准完整候选 `83c023566698b2aa013e9f22bb1b3c925d862cf0`。在真实 Windows 用户环境下核对 `main` 工作树、远端 `2735119f6245d7c97552b2c19c5f467c5c0ad4ae`、快进关系及 42 笔签名后，普通快进推送。`ls-remote` 确认远端指向候选。收集的运行均由该次推送自动触发。

- [CI 44531024198](https://github.com/ALCOMD/ALCOMD/actions/runs/44531024198) 的 macOS 和 Ubuntu 均在 `Check` 的全目标严格 Clippy 中止。`business_tools` 测试在 Unix 不能移出 `CoreConfiguration` 的 `client` 字段，编译器返回 `E0509`。该配置只在 Unix 实现 `Drop` 以清理自有临时端点，Windows 不会触发这一编译错误。两个作业的后续测试、构建和浏览器门禁均未执行。
- 同一 CI 首轮总体失败，Windows 作业完整通过。包含 `Check`、CLI 自启动集成、发布可执行文件、Tauri 无安装包构建、锁文件与差异检查，以及完整 Chromium 单轮 734 通过、0 失败。此结果对应远端 `83c02356`，本机 Chromium 的首轮超时与定向复验继续分别保留。
- 同 SHA 的 [CodeQL 44531022627](https://github.com/ALCOMD/ALCOMD/actions/runs/44531022627) 首轮通过。Actions、JavaScript/TypeScript、Python、Rust 四个分析作业均通过。
- 签名修复 `36137b6c707d9c4c3ae8f453adc577e61003c905` 仅为 HTTPS 夹具复制连接配置，保留配置清理、测试断言和全部生产输入。Linux 与 macOS 目标上的最小所有权探针均复现原 `E0509`，修复行能编译。Windows 真实隔离 Core 的同一 OAuth 仓库流程定向 1/0，严格 Clippy、fmt、xtask check、锁定 metadata 和 diff 检查通过。本机完整 Linux 交叉编译在依赖构建前置因缺少 `x86_64-linux-gnu-gcc` 中止，真实 Unix 定向仍待新远端验证。
- 工作流快照、三平台作业原始日志、交叉探针及 Windows 定向日志保存在 `target/m8-shared-project-assets-20261119/`。本轮未操作原 Core、GUI、真实项目或 Unity。记录时远端保持 `83c02356`，修复候选尚未推送。M8 继续等待同 SHA 三平台门禁。

### 2026-11-19 Unix OAuth 夹具远端复验

所有者确认继续后，只将签名夹具修复 `36137b6c707d9c4c3ae8f453adc577e61003c905` 普通快进至 `origin/main`。工作树及签名通过检查，远端确认指向该提交。上一轮的验收文档提交只保留在本地。

- 同 SHA 的 [CodeQL 44551755201](https://github.com/ALCOMD/ALCOMD/actions/runs/44551755201) 首轮四项分析通过。
- [CI 44551756595](https://github.com/ALCOMD/ALCOMD/actions/runs/44551756595) 中，macOS 和 Ubuntu 均通过此前阻塞的编译，原 OAuth 仓库确认与重放用例实际通过。macOS 的 `business_tools` 首轮 20 通过、1 失败、3 忽略。Ubuntu 为 13 通过、8 失败、3 忽略。两平台均在综合 OAuth 用例的 STDIO 探测失败。Ubuntu 稍后的六项返回 `AlreadyRunning`，另外一项扩展 Provider 计划断言在这之前就失败。
- 本轮 CI 首轮总体失败。北京时间 11 月 20 日，Windows 原作业结束并完整通过，包括仓库检查、CLI 集成、发布可执行文件、Tauri 构建和 Chromium 单轮 734 通过、0 失败。终态和三平台完整作业日志保存在 `target/m8-shared-project-assets-20261119/`。
- 综合测试原先通过私有端点启动 Core 和 OAuth，真实 STDIO 子进程以及 Bearer 监听器仍使用默认端点。`78b90927c9c41794b3b9b1538c60112a0d0d48ce` 使用同一个私有 Core 配置启动 Bearer，并使用操作系统现有的运行目录变量只配置 STDIO 子进程。两条目录一致性断言及同一凭据保持。Unix 子进程不继承用于定位真实用户数据目录的 `HOME`。Windows 真实隔离 SDK 的 OAuth 综合、STDIO 卸载、HTTP Bearer 卸载各定向 1/0，严格 Clippy、fmt、xtask、锁定 metadata 和 diff 通过。Unix 运行待同 SHA 复验。
- Ubuntu 独立的 Provider 失败发生在综合 OAuth 用例之前。原断言只输出 `is_error`，结果不足以判定失败阶段。`622cb62ad507c700de42cd2db9ddd13a7835a754` 保留原断言，只记录失败的夹具序号和返回的稳定错误码。Windows 真实签名 Provider 用例定向 1/0，严格 Clippy 和 fmt 等廉价门禁通过。未复现 Ubuntu 失败，未对生产代码或预算进行调整。两项本地提交均通过签名验证。
- 远端仍为 `36137b6c`。下一次快进的完整本地链是前一轮验收记录 `628597ce`、端点修复 `78b90927` 和 Provider 诊断 `622cb62a`，等待对这一范围的推送授权。M8 维持进行中。
