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

M7 于本地文档提交 `b6de51de` 正式收束。原生 Core/GUI 保持运行，不操作真实项目，不恢复已停止的桌面控制。

## 协议与依赖

遵循 `docs/baselines/mcp.md` 和 `docs/baselines/source-lock.toml` 中锁定的 `2026-07-28` Schema：逐请求版本/能力元数据、`server/discover`、无 initialize/session、Core Operation 与请求取消分离、不支持 MCP Tasks。精确上游 Schema 的 SHA-256 已核对为 `742750af0bb8c716e7030c4977c992b55d1adc4407e9e66997db5846baedc2cd`。

新增官方 Rust SDK `rmcp = =3.10.0`（Apache-2.0，MSRV 1.88，低于项目 1.97.1），关闭默认特性，只开启 `server`、`transport-io`。用途是复用 JSON-RPC 类型/响应编解码、请求调度及取消；现阶段不启用宏、客户端、Tasks 或 HTTP 服务特性。已有 serde/serde_json、Tokio 和 Core 协议依赖复用。lockfile 仅新增 5 个包，无既有包升级；新增包为 rmcp、pastey、const-oid、schemars_derive 1.2.2、serde_derive_internals 0.30.0。

替代方案：自行编写 JSON-RPC/HTTP/SSE 栈会重复协议及安全边界；旧版 SDK 不满足冻结协议。维护风险：SDK 同时支持旧生命周期，必须用无握手服务入口、显式逐请求校验及定向测试阻止兼容行为；精确锁版本、审查 lockfile 增量，更新时重验冻结 Schema。HTTP 后续复用 SDK 的无状态传输，不自行实现 HTTP 解析。

首轮验证还确认 SDK 的通用 NDJSON reader 会静默丢弃损坏和超长输入。因此只补严格的输入分帧：上限复用 RPC 的 4 MiB，跨取消保留分片/丢弃状态，损坏 JSON 返回 ParseError，超限/不合法结构返回 InvalidRequest；响应仍由 SDK 写出。错误写入 future 跨 receive 取消保留，避免截断或重复输出；不记录原始输入。该差异不能以放宽测试掩盖。

上游核对：[冻结 Schema](https://github.com/modelcontextprotocol/modelcontextprotocol/blob/4df2d6b6e3588efb46e7542d98498e5c630a0a86/schema/2026-07-28/schema.ts)、[官方 Rust SDK](https://github.com/modelcontextprotocol/rust-sdk)、[当前传输规范](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports)。

当前独立实现只提供真实协议发现，不宣称 MCP 业务或管理扩展完成。不广告未实现的 tools/resources/subscriptions，不接受旧 initialize/ping 或 Tasks。HTTP 在身份合同批准实现前明确失败退出，不监听无认证端口。

## 公共合同与接线状态

客户端身份与 State33 最小合同已批准，正在同步 Schema、DTO 和隔离迁移；下列其余部分仍按实际接线推进：

- 外部客户端身份与 Core 权威 Principal/权限/作用范围绑定；HTTP 每请求 Bearer 与受控 STDIO 启动身份不能取自 clientInfo。
- MCP 管理扩展的客户端、请求、连接、订阅读取和授权管理；沿用既定权限分类及扩展所有权/范围规则。
- 按现行 Core RPC 冻结工具输入/输出 Schema；旧 Draft 未提供正式 ABI。现有 Operation 仅 get/list/cancel，不能伪造尚无公开合同的 input/approve/reject/resume。

本批身份合同不再等待审批。其余真正新增的公共合同仍先提交具体差异，不暂停独立实现。

### 下一批最小身份合同（2026-11-13 已批准，实施中）

当前 `apps/alcomd/src/lib.rs::dispatch_payload` 把所有本机连接固定为 local-owner；`HelloParams` 没有认证字段。仅凭 adapter 内部记录 clientInfo/Bearer 不能改变 Core 权威身份。所有者已批准以下一批，直接实施；不得在适配器中代持所有者权限：

1. `system.hello` 增加可选 `authentication: { kind: "mcp-client", credential: string }`，capability `mcp.clients.v1`。Core 验证凭据并绑定其签发的 `mcp:<ClientId>` Principal；无效凭据使用已有 authorization_required/permission_denied 并关闭连接，不回退为 owner。现有 GUI/CLI 的受保护 OS IPC 本机模式保持。MCP adapter 业务连接必须带凭据，clientInfo 仍只作展示。
2. 新增 `mcp.clients.list/create/setGrants/issueCredential/revoke`：create 输入名称（1–128字符）和显式 grants，默认零授权；setGrants 用 expectedRevision 完整替换本客户端 grants；revoke 使客户端及其凭据失效，保留审计和既有 Operation。list 用已有 cursor/limit 模式、绑定集合 revision，不返回凭据或散列。写入沿用幂等键，最多64个客户端、每客户端64种权限/4096条作用范围。
3. grant 明确为 `permission/resourceKind/resourceId`，复用已实现权限及适用范围，不创造通配的所有者身份；未知权限和未支持的组合拒绝。对所有者已登记资源的授权是委托访问，不改资源 owner；Operation/Plan/幂等记录归实际调用 Principal，跨客户端不可读或复用。Core 每次 RPC 重读客户端状态和当前 grants，撤销及范围变更对已连接调用同样生效；不得只在 hello 缓存授权。
4. 管理权限沿用已声明的 `mcp.clients.read/manage`；实际调用须 local-owner，或经正常扩展授权的管理调用。`issueCredential` 额外仅 local-owner：Core 生成独立256位随机凭据，只在显式签发响应中返回一次；轮换立即使旧凭据失效。普通列表、Portable UI、事件及日志不返回明文/散列。STDIO 从受控启动凭据读取，禁止命令行参数承载秘密或把启动秘密继承给自动启动的 daemon。
5. State32→33 仅新增 `mcp_clients`、`mcp_client_grants`、`mcp_client_credentials`（凭据只存不可逆摘要/签发和撤销信息），保留原表、数据与任务，不改 Config13。增加对应 RPC/Schema/快照和真实 Core/Store 隔离、撤销、幂等及迁移保全测试。

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

这是一批实现检查点，**身份合同尚未全部实现**：没有接入hello认证或发布capability，原HelloParams仍拒绝额外认证字段，避免忽略凭据后以owner执行。application/Store客户端命令、随机凭据签发/摘要认证、管理RPC、逐请求撤销与范围复查、客户端隔离和秘密不落日志的真实Core用例继续按已批准合同实施；不得用DTO/SQLite约束测试冒充这些验证。没有启动新Core、升级原环境或恢复Computer Use；新增代码没有对应远端验证，不自动推送。
