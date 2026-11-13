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

新增官方 Rust SDK `rmcp = =3.10.0`（Apache-2.0，MSRV 1.88，低于项目 1.97.1），关闭默认特性，只开启 `server`、`transport-io`。用途是复用 JSON-RPC 类型/响应编解码、请求调度及取消；现阶段不启用宏、客户端、Tasks 或 HTTP 服务特性。已有 serde/serde_json、Tokio 和 Core 协议依赖复用。lockfile 仅新增 5 个包，无既有包升级；新增包为 rmcp、pastey、const-oid、schemars_derive 1.2.2、serde_derive_internals 0.30.0。

替代方案：自行编写 JSON-RPC/HTTP/SSE 栈会重复协议及安全边界；旧版 SDK 不满足冻结协议。维护风险：SDK 同时支持旧生命周期，必须用无握手服务入口、显式逐请求校验及定向测试阻止兼容行为；精确锁版本、审查 lockfile 增量，更新时重验冻结 Schema。HTTP 后续复用 SDK 的无状态传输，不自行实现 HTTP 解析。

首轮验证还确认 SDK 的通用 NDJSON reader 会静默丢弃损坏和超长输入。因此只补严格的输入分帧：上限复用 RPC 的 4 MiB，跨取消保留分片/丢弃状态，损坏 JSON 返回 ParseError，超限/不合法结构返回 InvalidRequest；响应仍由 SDK 写出。错误写入 future 跨 receive 取消保留，避免截断或重复输出；不记录原始输入。该差异不能以放宽测试掩盖。

上游核对：[冻结 Schema](https://github.com/modelcontextprotocol/modelcontextprotocol/blob/4df2d6b6e3588efb46e7542d98498e5c630a0a86/schema/2026-07-28/schema.ts)、[官方 Rust SDK](https://github.com/modelcontextprotocol/rust-sdk)、[当前传输规范](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports)。

当前 MCP 只提供真实协议发现，受控STDIO可使用获批身份连接Core，不宣称 MCP 业务或管理扩展完成。不广告未实现的 tools/resources/subscriptions，不接受旧 initialize/ping 或 Tasks。HTTP尚未实现，明确失败退出，不监听无认证端口。

## 公共合同与接线状态

客户端身份最小合同和State34快照增量均已批准。application、Store、daemon dispatcher、typed client及受控STDIO已实际接通，真实Core闭环及最终capability协商候选复验通过，`mcp.clients.v1`已按真实实现协商。下列后续部分尚未完成：

- HTTP授权与每请求Bearer绑定；不能取自clientInfo，也不能回退owner。
- MCP 管理扩展的客户端、请求、连接、订阅读取和授权管理；沿用既定权限分类及扩展所有权/范围规则。
- 按现行 Core RPC 冻结工具输入/输出 Schema；旧 Draft 未提供正式 ABI。现有 Operation 仅 get/list/cancel，不能伪造尚无公开合同的 input/approve/reject/resume。

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

日常仅检查 MCP 包及关联合同；使用独立的 `target/m8-check`，避免覆盖当前原环境程序。覆盖逐请求元数据、协议版本、发现结果、旧生命周期拒绝、错误帧恢复、有界输入、STDIO EOF 和 stdout 纯协议。新增代码尚无 Hosted CI/CodeQL；不复用旧 M7 结果替代。

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

根因是旧State19保全用例仍期待旧表集合，而获批State33/34必然新增三张空MCP表，不是旧业务数据改变。本机先单例复现0通过/1失败（`state19-reproduction.log`），随后只新增`mcp_clients/mcp_client_grants/mcp_client_credentials`必须各为空的明确断言；原`after == before`、历史删除边界和Event高水位断言逐字保留。没有过滤忽略新增表或改生产迁移/Schema/运行时代码。修复提交`49d0687f8af00aee0672d43c6457fcdca22fe561`，修改仅测试9行；同模块4/0、Store all-targets严格Clippy、fmt/xtask/diff通过，原失败记录保留，不拼成105/1失败轮通过。该文件SHA-256 `8ef2b82748aae742b7b39c15767a06c8ca72fb24238d7a28acb39aa01b524a09`，结果`state19-targeted-fixed.log`和`state19-clippy.log`。修复额外推送授权已单独申请，未自动推送；以下业务合同也尚待集中审批。原运行Core/GUI、原数据和Unity未改，未恢复桌面控制。

### 首个真实业务工具合同：项目包卸载闭环（待审批）

所有者要求集中审阅首个完整业务流程，本节是具体差异提案，**尚未批准或实施**，不改变已批准身份合同。推荐先完成“读取获准项目及已装包→生成卸载Plan→明确确认后Apply原Plan→查询/取消任务”。这是完整包管理写流程，不以发现或模拟响应代替；安装/搜索/重装的来源委托另列后续缺口，不把全部工具塞入首切片。

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
