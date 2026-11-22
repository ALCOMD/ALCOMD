# PR1 原生客户端、连接和所有者审批合同核定稿

**状态：所有者已核定 A1–A4、B1–B3、C1–C6、State1 和 D1–D3。** 核对基线为 `94b3413eb949`。下列公开方法、安装格式、权限边界和验收入口在 RPC v1、State1 和 SDK 现行定义上实施。包元数据链接的桌面处理列为独立差异。

## 已核实的实现

| 入口 | 当前行为 | PR1 所需差异 |
|---|---|---|
| 原生 IPC | Windows Named Pipe 限本机用户，Unix socket 校验目录及同用户对端。握手必填 `client.name/version/instanceId`，协商 capability，并另行识别 MCP 认证和传输观察员。 | 保留同用户原生直连。自报三个字段用于诊断。安装发现、活动统计均独立于认证。 |
| Core 连接 | listener 用 `JoinSet` 驱动连接，`serve_connection` 每条连接生成 `connection_id`，退出时清理观察、仓库预览和扩展 UI 会话。 | 建立通用的活动连接索引，支持空闲与订阅中的连接按目标关闭。 |
| 安装发现 | 产品清单含 GUI、CLI、MCP 及内部组件的不同二进制名称。现有 `core-installation-v1` 仅提供经过校验的 Core 运行目录，供 SDK 寻找可启动的 Core。 | 新增独立的客户端安装记录。daemon、SDK bridge、updater、bootstrap、扩展宿主不因此成为用户设置里的独立客户端。 |
| 第三方 SDK | Rust 客户端通过系统端点连接；Node SDK 通过随 SDK 分发的 `alcomd-sdk-bridge` 子进程接入。 | bridge 是 IPC 对端，可能服务于不同 GUI。区分实际 GUI、bridge 进程和 IPC 连接。 |
| MCP 身份与观察 | 已有客户端登记与发证、OAuth 注册与申请、Core 调用观察及 MCP 传输观察。第一方扩展 `com.vrclearn.alcomd.extension.mcp` 用三个公开 WIT 接口实现管理面。 | 原生页面取代随附管理扩展，独立 `alcomd-mcp` 继续提供传输与工具。AI 客户端登记独立于程序安装记录。 |
| 桌面 | GUI 的 `gui_focus_project_unity` 校验项目修订及 Unity 写入状态，仅 Windows 有聚焦后端；`gui_open_project_directory` 检查 Core 返回的登记根目录；`gui_open_unity_hub_download` 使用固定下载地址。 | GUI、CLI、MCP 请求 Core 所在机器执行相同动作。操作系统实际结果由 Core 返回。 |

核对入口：[产品清单](../../alcomd.product.toml)、[安装位置](../../crates/alcomd-platform/src/core_installation.rs)、[Unix IPC](../../crates/alcomd-platform/src/unix.rs)、[Windows IPC](../../crates/alcomd-platform/src/windows.rs)、[握手请求](../../specs/rpc/system-hello.request.schema.json)、[Core 连接](../../apps/alcomd/src/lib.rs)、[SDK stdio](../../crates/alcomd-client/src/sdk_stdio.rs)、[Node SDK](../../packages/alcomd-sdk/src/node-runtime.mjs)、[GUI 桌面命令](../../apps/alcomd-gui/src-tauri/src/lib.rs)、[Unity 窗口](../../apps/alcomd-gui/src-tauri/src/unity_windows.rs)。

## A．安装发现格式和连接归属

1. **A1 产品名与文件格式。** 提议 `client-installation-v1`，每个程序一份严格 JSON：`{schema:1, programId, kind, displayName, productVersion, installRoot, executable}`。三个官方 `programId` 由产品清单提供，建议分别为 `com.vrclearn.alcomd.gui`、`.cli`、`.mcp`（后两项使用相同前缀）。第三方使用自有反向域名及 `kind:"gui"`。官方另有 `kind:"cli"|"mcp"`。程序标识限小写 ASCII 域名段并设 128 字节上限；展示文字有界且不含控制字符。安装根为绝对目录，程序文件为其下相对路径。文件名为 `<programId>.json`，大小上限 16 KiB。产品发行版本独立于格式号。
2. **A2 写入、读取与退出。** 安装器在安装成功时原子发布记录，更新时替换，卸载时只移除自己安装的记录。Core 首次查询即读取，未运行的客户端也出现在列表。用户级目录为既有每用户数据目录下 `clients.d`；机器级目录为 Windows 的 ProgramData、macOS 的 `/Library/Application Support`、Linux 安装时的数据前缀（包括标准 `/usr/local/share` 与 `/usr/share`）下的产品子目录加 `clients.d`，目录名由平台产品布局派生。随用户复制的便携文件只有完成安装登记后才计为已安装。Core 只读清单，验证文件名、所有权/写权限、根路径、条目类型、边界及可执行文件存在性，单份损坏不阻断其他记录。目录读取设界限并显式报告覆盖不全。重复安装以 programId 为一行，可在详情列出不同安装范围与版本。当前已卸载但仍在运行的进程显示为活动，退出后自然消失。
3. **A3 身份与显示。** 清单给出安装声明，文件可读、名称相同或自带摘要均不能证明官方签发。官方标识保留给能由官方受保护安装布局/产品发布记录验证的资产；冲突记录不得遮蔽可信记录。第三方和开发客户端都通过现有同用户端点免审批。清单不授予权限，未知程序可正常工作。记录不执行内含程序，也不用其路径启动 Core。PR1 对第三方安装登记交付 GUI，无需扫描 AI 宿主或其他产品。
4. **A4 实例归并。** Core 根据 OS 可验证的进程身份（端点 PID 加创建标识与可执行文件身份，须防 PID 重用）产生程序归属、随机 `instanceId` 与每次握手成功的 `connectionId`。GUI 的读、写、订阅连接属于同一可验证实例。Node bridge 仅在能验证实际启动它的 GUI 进程及其登记可执行文件时归并到 GUI 实例；普通脚本、短命父进程、受限系统查询等无法证明归属时按实时未识别对端列出。按已验证的同一映像可合并活动行；仅能验证连接时一条连接对应一个临时实例，不猜测它是哪个程序。展示的自报名称带来源，不能伪造已安装程序计数或审批者。实时 ID 均在当前 Core 实例内有效，退出和重启不写入永久客户端名单。

M11 的隔离测试用相同格式完成安装、更新、卸载和从未连接的发现。生产安装器按各平台实际范围写入并由 M12 对真实安装产物验收。[平台分发布局](../adr/0015-platform-product-packaging.md) · [M12 完整分发](M12-full-product-distribution.md)

## B．程序列表与断开

提议一个能力 `clients.registry.v1`，三端及原生 Rust/TypeScript SDK 使用如下 RPC。原生所有者和已认证、仍有效的 AI 客户端按同一业务规则访问；观察员只可使用既有专用协议，扩展如需这些用例须另有获批的公开宿主接口。无需新增扩展 permission。

| 审批项与方法 | 参数 → 返回 | Core 行为 |
|---|---|---|
| B1 `clients.list` | `{cursor?,limit?} → {coreInstanceId,items,nextCursor?,coverageIncomplete}` | 每程序一行。行含 `programKey, programId?, displayName, kind, installations[], activeInstanceCount, activeConnectionCount`。安装项公开范围与版本，未知活动行用临时 key。 |
| B2 `clients.instances.list` | `{programKey,cursor?,limit?} → {coreInstanceId,items,nextCursor?}` | 每行 `{instanceId, startedAtMs?, connections[]}`，连接含 `connectionId, connectedAtMs` 及可选有界自报诊断。每个结果受 Core 同时接纳连接数与协议帧上限限制。无需公开原始 PID、路径、凭据或请求内容。 |
| B3 `clients.disconnect` | `{expectedCoreInstanceId,target:{kind:"connection"\|"instance",id},idempotencyKey} → {disconnectedConnectionIds[]}` | 定位指定活动连接，或在锁内截取该实例当时已有的连接集合；后续连接使用新 ID。已消失目标返回普通未找到，跨 Core 返回失效，同一逻辑请求重放已知回执。若包含自身，应先交付回执后关闭自身。 |

列表稳定分页，游标绑定查询、调用者和视图修订。安装记录或相关实时集合变化使游标过期时可从首页重取。断开触发正常连接结束清理，并唤醒空闲读、事件订阅等长调用；不会取消已由 Core 接收的计划执行或任务。断开不撤销凭据、不移除安装，允许立即重连。发布与清理均按 Core 自身连接 ID，覆盖握手失败、EOF、显式断开及 daemon 退出。界面为设置 → 客户端 → 程序 → 实例/连接；CLI 和 MCP 可按同一 ID 分页查看并断开。

## C．AI 客户端、授权与原生 MCP 管理

| 审批项 | 现有接口及本次变更 |
|---|---|
| C1 客户端身份 | `mcp.clients.list/create/issueCredential/revoke` 继续使用 `mcp.clients.v1`。保留现有 `clientId,principalId,expectedRevision,idempotencyKey` 和一次性发证结果。GUI 和 CLI 可在已建立的独立所有者渠道完成管理，以 AI 身份发起的管理变更需所有者确认。AI 客户端可查自身记录或其获准访问的请求，Core 在执行和重放前复查真实身份及撤销状态。 |
| C2 OAuth | `mcp.oauth.registrations.list/set/revoke`、`mcp.oauth.requests.get/decide` 以及 `mcp.oauth.begin/finish/token/revoke` 沿用 `mcp.oauth.v1`。前者是管理与批准，后者各自校验协议材料。现有 `requestId/expectedFingerprint`、时效、目标资源、redirect URI、PKCE、一次性代码、轮换与撤销继续由 Core 校验。请求持有者能读取自己的安全摘要；`browserSecret`、PKCE verifier、握手自报、请求 ID 和 `approve:true` 均不能替代所有者决定。 |
| C3 管理申请 | 提议在 `mcp.clients.v1` 增加 `mcp.clients.requests.create/get/decide`，用于 C1/C2 中需要变更其他身份或颁发新授权的管理操作。OAuth 已存在的登录申请继续用自己的 `requests.get/decide`。新增申请 `{action, idempotencyKey}`，action 为 `createClient/issueCredential/revokeClient/setOAuthRegistration/revokeOAuthRegistration` 的严格变体，以对应现行方法的业务字段绑定目标及修订。Core 返回 `{requestId, action, status, expiresAtMs, fingerprint}` 的脱敏投影。get 接受 `{requestId}`；decide 接受 `{requestId,approve,expectedFingerprint,idempotencyKey}`。独立所有者核对不可变目标、连带变化、申请者及修订后执行一次，过期、源修订已变或申请者已撤销时拒绝。没有注册的 OAuth 客户端经所有者登记后才能按现有流程 begin。 |
| C4 一次性秘密与审计 | 现有直接发证/机密 OAuth 注册仅向独立所有者渠道一次性交付秘密；批准 C3 后如生成一次性秘密，在批准响应的该动作专属字段向所有者交付，由所有者按已有凭据交付流程配置目标客户端。申请者的创建、get、重放、审计、日志、观察流均只含脱敏状态。真实 OAuth 客户端凭自身 PKCE/授权码完成 token 交换。不可从批准 API 或列表恢复丢失的一次性秘密，沿用现有显式轮换规则。 |
| C5 观察 | `mcp.runtime.snapshot`、`mcp.transport.snapshot` 各自保留现有 `mcp.runtime.read.v1`、`mcp.transport.read.v1` 及各字段可见性。原生管理页直接使用；CLI、MCP 也能查询有权观察的范围。`mcp.transport.report/bindRequest` 继续仅允许实际传输观察员。 |
| C6 扩展和分发 | 移除随附管理扩展 `extensions/first-party/alcomd-extension-mcp/` 的产物和默认装配，原生设置页使用相同 Core 管理用例。将其描述为第一方扩展的规则、脚手架安装项、生成器、快照、夹具和分布测试须在删除前逐文件清点。MCP 工具提供者扩展、Discord、沙箱及权限系统继续工作。保留第三方可用的 `mcp-management/clients` WIT 和 `mcp-runtime`、`mcp-transport` 两个 world 及其管理依赖。第三方管理与观察按现有各项扩展授权执行，管理 WIT 不具备签发秘密或批准 AI 授权的接口。装配与八项文件删除按[扩展合同 E12–E13](M11-extension-entry-contract-review.md)核定。 |

**独立所有者渠道。** 现有 `AccessContext::is_interactive_client` 同时对本地所有者与已认证 MCP 客户端返回真。目前 OAuth 的管理校验、`issueCredential` 的该判断无法独立证明用户批准。建议在涉及授权的用例、实际写入事务和回放路径区分 Core 已认证的原生同用户所有者、已委托 AI 身份、观察员和扩展。要让 AI 发起的请求得到批准，用户在自行打开的本机原生管理页面或 CLI 查看并决定同一个请求；MCP 工具返回请求 ID 与状态。申请者的会话和工具不能切换成所有者或通过直接调用原有 set/issue/decide 绕过审批。拥有本机用户权限的任意原生程序属于同一 OS 信任边界；“未经审批可直连”与“已委托 AI 不能自批”通过实际连接身份同时成立。所有者对自己的原生管理操作由同一用例直接受理。

**存储差异。** A/B 安装记录归安装器，实时连接仅存于内存。C3 提议在 State1 原位新增管理申请表，记录申请 ID、发起 principal、动作及其规范化业务参数、指纹、目标修订、时效、状态、决定者和脱敏回执，不存明文秘密；索引覆盖请求 ID 和失效回收。以既有 caller + key 的幂等合同绑定指纹，批准/拒绝、原业务变更、审计和回执在同事务内完成。C2 登录继续使用当前 OAuth 应用层的有界内存 pending 队列，注册记录和 token 保存在现有 SQLite 表中。正式 DDL、索引、受影响的现行 State1 验证须随实施差异单核定。

依据：[应用层身份](../../crates/alcomd-application/src/lib.rs)、[AI 客户端](../../crates/alcomd-application/src/m8_mcp.rs)、[OAuth](../../crates/alcomd-application/src/m8_oauth.rs)、[扩展入口及随附文件清理清单](M11-extension-entry-contract-review.md)、[WIT](../../specs/extensions/wit/mcp-management-v1/management.wit)。

## D．Core 本机桌面动作

提议一个 `desktop.actions.v1`，以下每项返回 `{outcome:"requested"}`；聚焦成功返回 `{outcome:"focused"}`。没有平台实现、无可用桌面、窗口不存在、系统拒绝聚焦、打开失败分别返回稳定的分类错误。GUI 可在调用前显示可用性，CLI、MCP 和 SDK 获得相同实际结果。系统文件选择器留在交互端，三端向 Core 提交同一业务方法所需的路径或资源 ID。

| 审批项与方法 | 必填参数 | 目标与校验 |
|---|---|---|
| D1 `desktop.focusProjectUnity` | `projectId,expectedRevision,idempotencyKey` | 按真实 caller 读取项目，检查修订、登记目录及对应运行中的 Unity，再对该目录匹配的窗口请求聚焦。保留 Windows 已有的找不到窗口和系统拒绝结果。 |
| D2 `desktop.openProjectDirectory` | `projectId,idempotencyKey` | Core 根据真实 caller 可读的已登记项目重新获得并校验根目录，再调用 Core 本机的文件管理器。 |
| D3 `desktop.openUnityHubDownload` | `idempotencyKey` | 只接受产品固定的官方 Unity 下载网页，使用 Core 本机默认浏览器。已有 `unity.hub.open` 的修订、目标 token、启动或需要下载结果继续独立使用。 |

每项只接受表中业务参数，按 caller、方法、参数和 key 绑定回执；接受时再次核对实际 caller 授权。OS 启动副作用与持久化不可能原子提交，先记录接受，同键重复不再次启动；崩溃窗口内以结果未知报告，用户明确重做才使用新键。现行 State1 的 `idempotency_records.pending` 必须关联 operation；D1–D3 可采用现有 operation 与回执合同，先以同事务受理，再由平台驱动执行，重开后不重派处于系统副作用不确定窗口的动作。不新增仅为跳过该约束的特殊记录。D1/D2 复用项目读取边界，D3 允许同一原生/AI 客户端业务角色。观察员和受限扩展不通过这些 RPC 获取任意 shell、URL 或路径执行能力。

另有 `gui_open_package_link(url)` 由项目包页和仓库包浏览器使用。若纳入同一 PR，应单列目标为已选包记录、来源、版本与链接种类的 RPC，由 Core 从现行包元数据解析并校验链接，覆盖隐藏候选与仓库目录各自的可见性。此入口未包含在 D1–D3 的核定范围内。

## 三端调用与独立验收

| 范围 | GUI | CLI | MCP |
|---|---|---|---|
| 程序、实例和断开 | 设置 → 客户端，按程序展开 | 列程序、列指定程序实例、按目标断开 | 同一分页/目标三项工具，使用 AI 身份 |
| AI 客户端管理 | 设置内独立 AI 客户端与请求视图 | 现有 client/OAuth 命令接同一 Core 管理；可按请求 ID 审批 | 申请、查看授权范围内状态、查已提供的运行信息；需独立用户批准时返回请求 ID |
| 桌面动作 | 使用 Core 返回结果提示用户 | 发送 D1–D3 参数 | 同一请求作用于 Core 所在机器 |

1. **安装器与独立 SDK。** 在三个系统的隔离安装/用户目录先放置仍未启动的官方三端及第三方 GUI 登记，启动全新 Core 就能查到四个程序。验证多版本/重复目录、损坏项、冲突官方 ID、移动、升级、卸载、重开和真实 M12 安装链。Rust 客户端与 Node 客户端从独立项目经正式 SDK 接入，覆盖真正的 GUI launcher、bridge、普通脚本和未安装开发 GUI；自报同名没有身份效果。
2. **真实连接生命周期。** 在 Named Pipe 与 Unix socket 上启动十次 CLI、一个多连接 GUI 和一个 MCP 适配器。CLI 单行，十个并发进程在详情分别可见；结束后计数归零。验证半握手、订阅、对自身断开、跨实例 ID、已结束的目标、重连、并发查询和 Core 重启。关闭 GUI 一条连接不影响相邻连接、已受理写入、审计归属或用户进程；断开未安装 GUI 全部连接后临时行消失。核对客户端能原样续传游标。
3. **真实 AI 认证。** 两个独立 AI 客户端分别以真实凭据或 OAuth 连接，同一适配器程序与两个持久 AI 身份各自可见。申请人从三个前端分别发起等价测试，证明在独立所有者批准之前授权不变；另一个 AI、原有管理方法、伪造 hello、token 离线重试、扩展、传输观察员均不能伪造所有者批准。在隔离 Core 写入层验证拒绝、过期、撤销、同键重放、源修订变化与实际批准只落一次；验证列表、日志、MCP 工具和重放均不泄露一次性秘密。
4. **桌面。** 三个入口对照实际进入 Core 的方法、目标、修订、key、actor 和结果。CI 使用受控平台驱动记录实际调用次数，覆盖无桌面、非 Windows 聚焦、过期项目、相同/不同 key 和撤销等待。真实桌面上分别验证文件管理器定位、浏览器打开固定地址以及 Unity 窗口被系统接受或拒绝的结果。记录 OS 和桌面环境，后台无图形环境仅计错误路径覆盖。
5. **原生替换和分发。** 拿掉随附管理扩展再启动官方 GUI，直接管理测试 AI 客户端、审批、撤销并观察 stdio/HTTP；分别用 CLI、MCP 和独立 SDK 复验相同用例。单独装第三方 MCP 工具扩展验证 provider ABI 和授权；核对 SDK 原生目录、Schema、生成绑定、工具清单、命令目录、产品装配及门禁与核定项一一对应。原生直连及独立 AI 审批的理由同步到当前信任模型。

审批边界为 A1–A4、B1–B3、C1–C6、State1 和 D1–D3。扩展整目录与依赖清理须附生成物、测试和引用的逐文件删除清单；具体 SQL、安装平台路径、返回错误码和 SDK 映射须与最终批准方案同步。现行 SDK 目录及操作矩阵仅记录已交付入口。[现行方法目录](../../specs/rpc/native-sdk-v1.methods.json) · [首发操作矩阵](M11-client-operation-matrix.md)
