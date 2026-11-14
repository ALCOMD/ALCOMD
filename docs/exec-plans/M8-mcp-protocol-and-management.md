# M8：MCP 协议与管理扩展

状态：IN_PROGRESS，2026-11-13。所有者已批准按既有计划进入 M8；不自动推送，不启动 M9–M12。

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

当前受控STDIO已接通获批身份与六个包卸载工具；无凭据入口仍仅协议发现，不连接Core或提供业务。只广告已接通的tools，不广告resources/subscriptions，不接受旧initialize/ping或Tasks。HTTP回环预配Bearer已完成本地候选，定向与真实隔离验证通过，远端未验证，见末节；不宣称管理扩展或整个M8完成。

## 公共合同与接线状态

客户端身份最小合同和State34快照增量均已批准。application、Store、daemon dispatcher、typed client及受控STDIO已实际接通，真实Core闭环及最终capability协商候选复验通过，`mcp.clients.v1`已按真实实现协商。下列后续部分尚未完成：

- HTTP授权与每请求Bearer绑定；不能取自clientInfo，也不能回退owner。
- MCP 管理扩展的客户端、请求、连接、订阅读取和授权管理；沿用既定权限分类及扩展所有权/范围规则。
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

日常仅检查受影响包及关联合同，使用独立的`target/m8-check`，避免覆盖原环境程序。协议、身份和工具证据分别关联输入；已推送身份候选的实际远端失败/CodeQL通过见后文，本次工具及追加夹具修复尚无对应Hosted CI/CodeQL，不复用旧M7结果替代。

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
| Ubuntu 22.04 | 全流程成功。Check（含严格Clippy、Workspace Rust及前端检查/构建）、合同、挂载边界、release、Tauri、GLIBC/锁文件/diff和Chromium703/0均完成；浏览器日志703 passed（14.6m），不与其他平台或轮次拼接 |
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
| macOS 15 arm64 | 整个job成功，包含完整Rust/CLI集成/挂载边界/release/Tauri/部署目标/锁/diff/Chromium703/0，零重试；浏览器22.4分钟 |
| Ubuntu 22.04 | 严格检查/完整Rust/CLI集成/挂载边界/release/Tauri/GLIBC/锁/diff均成功；浏览器703项执行中触及job60分钟预算而取消，没有最终计数，不记录为PASS |
| CodeQL 43545257774 | Actions、JavaScript/TypeScript、Python、Rust四项全部成功 |

两个HTTP关闭/截止缺陷的新增回归及真实HTTP/Core闭环已在三平台Rust测试通过；e3cd4ac当前唯一未收束的门禁是Ubuntu的完整浏览器终态及总作业预算。Check与Test在同job重复执行完整Workspace/Discord/前端检查的证据保留原日志，不能用Mac/Win或aab87a8的浏览器结果拼接成Ubuntu当前通过。此轮只读查询偶发失败均仅重查原run，无手动dispatch/rerun。

待推送工作流候选`2a319d7a7d0f89bf72dcf728d8c778178eb3f95c`仅修改ci.yml三处串行编排，没有改生产代码、测试、聚合脚本、依赖、断言、超时或仓库设置。完整Check仍为每job默认必经前置，后续保留原Core/CLI构建及自动启动脚本；所有其他步骤及锁文件最终核对逐项相等。固定e3cd4ac基线的YAML结构/门禁集合审计、PowerShell AST、Bash -n、metadata/xtask/diff通过。468份运行时/测试输入3596d671不变，保留33811d4与e3cd4ac的有效功能证据，不为仅编排和文档变更重跑本机全量。

ci-final/codeql-final JSON、三平台逐job日志、Ubuntu超时annotations及ci-coverage-audit.json均保存在`target/m8-hosted-e3cd4ac-20261114/`。新编排未在远端执行，不能提前记为通过；原三笔推送授权已用完，工作流候选及本节记录仅本地提交，等待新的明确推送授权，下一轮只由新push自动验证，不人工重跑旧job。M8保持IN_PROGRESS，完整OAuth/管理扩展Host桥等范围仍未实施。