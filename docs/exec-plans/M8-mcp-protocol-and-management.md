# M8：MCP 协议与管理扩展

状态：IN_PROGRESS，2026-11-15。所有者已批准按既有计划进入 M8；不自动推送，不启动 M9–M12。

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

当前受控STDIO已接通获批身份与六个包卸载工具；无凭据入口仍仅协议发现，不连接Core或提供业务。只广告已接通的tools，不广告resources/subscriptions，不接受旧initialize/ping或Tasks。HTTP回环预配Bearer已接通，取得本地定向/真实隔离及9fea7b6同SHA三平台完整CI与四项CodeQL技术通过（含5行测试同步）；历史失败/取消及后续只读审计见末节，不宣称管理扩展、完整OAuth或整个M8完成。

## 公共合同与接线状态

客户端身份最小合同和State34快照增量均已批准。application、Store、daemon dispatcher、typed client及受控STDIO已实际接通，真实Core闭环及最终capability协商候选复验通过，`mcp.clients.v1`已按真实实现协商。当前接线与后续边界（各候选证据分别见下文）：

- 获批回环预配Bearer及预登记客户端HTTPS/OAuth均已实现，含发现/resource绑定、到期、PKCE、审批、刷新轮换、撤销及逐请求Core授权；不取自clientInfo、不回退owner。OAuth普通浏览器/目标客户端人工接入待验证，CIMD/DCR/外部授权服务器尚不支持；本候选远端门禁见末节，不能据实现存在宣布整个M8完成。
- MCP管理扩展的客户端及授权管理、独立Host/WIT、真实Core/Host/Component闭环已实现，隔离人工查看入口已准备、人工查看未执行。实时请求、连接、订阅读取及正式默认分发仍未完成；沿用既定权限分类及扩展所有权/范围规则，不将未撤销记录当作在线。
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
