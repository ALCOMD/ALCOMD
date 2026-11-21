# M11 扩展入口与客户端管理合同审阅

基线为 `main` 的 `94b3413e`。所有者已核定 E1–E14 及下列八个删除路径，涵盖 GUI/CLI/MCP 入口、公开方法、权限、WIT 和 State1。原生管理替代通过验收后删除第一方管理扩展。所有自有合同原位保持版本 1。

## 现有调用与拟改权限

| 审批项 | 实际生产者和消费者 | 待实施行为 |
| --- | --- | --- |
| E1 · 删除权限 `mcp.tools.provide` | Domain 的 `Permission::McpToolsProvide`、Application 的两个扩展权限目录及常量、Manifest、扩展授权 RPC Schema、TS SDK 联合类型、GUI 权限页和 State1 的 `extension_grants` 共用此名称。 | 从所有枚举与两条 DDL CHECK（名称、Global 范围）中移除。GUI 删除授权控件和翻译。首发按完整现行定义校验。没有发布授权、替代授权、自动授权或存量授权转换。其他声明的 required/optional 业务权限、同意及撤销语义照常。 |
| E2 · 解析、预演与安装 | `crates/alcomd-extensions/src/package.rs` 对压缩包、已安装目录都检查“无 MCP 声明却有发布权限”。`tools()` 反向要求有发布权限，并校验每个 `uses` 所需业务权限出现在 Manifest 中。`crates/alcomd-store/src/m6.rs::reject_tool_namespace_collision` 当前靠权限字串判断是否检查命名空间。 | 两条读取路径均按签名声明处理。保留包签名、内容限制、导出类型预检、业务权限需在 Manifest 声明等检查。由经验证的 MCP 声明驱动现有公开名冲突检查。安装 Plan 继续披露工具及其业务权限；同意只覆盖真实的 required 权限。 |
| E3 · 工具发现 | `mcp.tools.catalog` → `apps/alcomd/src/m8_tools_rpc.rs::entries` → `crates/alcomd-store/src/m8_tools.rs::snapshot`。SQL JOIN 要求发布 grant；daemon 又排除缺少任一业务 grant 的声明，目录指纹含 grantRevision/grants。 | 验证真实调用者后，列出已启用且未隔离的扩展包内核验过的签名声明。无业务授权也可发现其名称、Schema、来源、效果及所需权限。指纹只绑定影响当前可见目录和分页的数据。业务授权变化本身不撤下声明；禁用、隔离、卸载、签名或活动包变动立即反映。保留 MCP 分页及目录变化通知。 |
| E4 · 调用与重放 | `extensions.mcpTools.call` 在目录解析之前可以先按键读写回执。`store::m8_tools::check` 在实际 Host 访问时检查 lease、主体、包身份、声明、发布 grant 和业务 grants；`lookup` 单独检查调用者的业务权限。 | 进入调用和取回已提交结果均验证实际客户端、当前扩展可用性与业务权限，声明中列出的 Host 操作继续受限。提交产生副作用的事务再次验证权限、lease、原目标及修订。关闭权限后条目仍可见，受限调用得到普通权限错误；关闭扩展或撤销客户端后不能用旧键取回受限结果。成功重放一次写入的原始 Core 效果。纯计算允许 `uses: []`。 |

现状代码可从[包验证](../../crates/alcomd-extensions/src/package.rs)、[权限目录](../../crates/alcomd-application/src/m6.rs)、[业务操作与效果映射](../../crates/alcomd-application/src/m8_tools.rs)、[工具目录和调用](../../apps/alcomd/src/m8_tools_rpc.rs)、[Store 授权与回执](../../crates/alcomd-store/src/m8_tools.rs)、[State1 DDL](../../crates/alcomd-store/src/schema.sql)及[宿主类型预检](../../apps/alcomd-extension-host/src/mcp_tools.rs)复核。

## 声明、命令与公共 wire 拟案

| 审批项 | 首发合同 |
| --- | --- |
| E5 · 包结构 | 必须有签名的 `[entrypoints].component = "component/extension.wasm"`。现有可选 `[ui].protocol = "portable-v1"` 和 `[mcp].tools = "component/mcp-tools.json"` 保持独立。新增可选 `[cli].commands = "component/cli-commands.json"`，文件纳入同一包签名和本地/远程预演。缺省即未声明该类入口。允许三项都缺省或任意组合，包括仅后端、仅 CLI、GUI+MCP。未声明入口不能从包文件名或组件导出反向推断。 |
| E6 · 签名 CLI 声明 | 新文件为 UTF-8 JSON `{ "version": 1, "commands": [...] }`。每项必填 `name,title,description,inputSchema,outputSchema,uses`。复用 MCP 已验证的声明形状、64 KiB、1–32 项、名字正则 `^[a-z][a-z0-9_]{0,31}$`、JSON Schema 边界、业务操作和禁止多类写效果混装的检查。局部名字在同一扩展的同一入口类型内唯一，CLI 按完整扩展 ID 与局部名寻址，MCP 按现有公开名寻址。运行业务权限只由 Core 从 `uses` 派生。拟在既有安装计划和扩展详情中以姊妹字段 `cliCommands` 披露签名预览，与对应 DTO 的 `mcpTools` 取相同的缺省和必填规则。 |
| E7 · CLI 和 SDK | 新增 `alcomd-cli extension commands [--extension-id ID] [--cursor CURSOR] [--limit LIMIT]` 与 `alcomd-cli extension run EXTENSION_ID COMMAND [--arguments JSON]`。纯/读命令缺省参数取 `{}`，Core 继续检查实际输入 Schema。原生能力拟为 `extensions.cli-commands.v1`；仅注册 `extensions.cliCommands.list` 和 `extensions.cliCommands.invoke`，Rust/TS SDK 各提供对应的两个类型化方法。CLI 直接经 SDK/Core 运行声明的命令，内置 CLI 子命令的解析与命名空间独立。 |
| E8 · 原生 DTO | list 入参 `{extensionId?,cursor?,limit?}`，结果 `{commands,nextCursor,catalogFingerprint}`。每项包含 `name,title,description,inputSchema,outputSchema,effect,requiredPermissions,origin`；`origin` 为现有扩展 ID、发布者指纹、包摘要和版本。分页采用 Core 已签名的游标，绑定过滤条件与目录。invoke 入参 `{extensionId,name,arguments}`，结果 `{data,coreEffect,replayed,error}` 复用现有结果语义。编码拒绝未知字段，主体、授权、安装包路径和内部 lease 均由 Core 取得。CLI 输出沿用既有 JSON/文本模式。 |
| E9 · CLI 写控制 | 对 plan、apply、cancel 使用既有受 Core 控制的 `idempotency_key`；apply 还需原 `plan_id,expected_revision,confirm:true`，cancel 需 `operation_id,expected_revision`。CLI 按现有原生选项约定，用 `--idempotency-key`、`--plan-id` / `--operation-id`、`--expected-revision` 与交互确认或 `--yes` 填入这些保留字段。调用者自定义参数与保留字段冲突报用法错误。一次逻辑操作复用原键和参数。`--dry-run` 遵守全局门禁，只有可返回真实计划的命令接受，不能把执行任意 guest 写命令当作预演。 |
| E10 · guest ABI | 当前 `alcomd:extension/extension-v1@1.0.0` 同时强制导出 lifecycle 和 GUI，宿主 `ExtensionV1::new` 无条件绑定两者。拟将基础 world 改为只要求生命周期、保留既有 host imports；另提供 `alcomd:extension/portable-ui-v1@1.0.0` world，包含基础并导出现有 `guest-ui`。后台、CLI、MCP 组合按实际声明预检、独立绑定导出。拟新增 `alcomd:cli-commands/provider@1.0.0` 的 `invoke(name:string, arguments:list<u8>) -> result<list<u8>,string>` 及包含基础 world 的 `provider-v1`。CLI 专用 guest 导出独立分派；同名 CLI 命令与 MCP 工具可返回不同结果。无 GUI 的组件无需空 GUI 导出。便携 UI 类型与各个现有管理、观察接口继续可用于第三方组件。 |
| E11 · 业务 Host API | 将当前 `alcomd:mcp-tools/core@1.0.0` 的六个类型化 Host 函数及错误类型原位归一为跨入口的 `alcomd:extension/host-operations@1.0.0`。CLI 和 MCP 的 provider world 在需要这些业务调用时使用同一 host 接口，独立的 provider 导出和清单文件保留。同步签名示例、bindings、预检、受影响的 Schema 与 provenance，首发只提供归一后的业务进口。保留 Wasmtime 限制、Core 子进程 IPC、lease、当前扩展实例/包/发布者、声明的 uses 和调用者认证。新增业务权限为 **0**，移除一个 `mcp.tools.provide`。 |
| E12 · State1 事务回执 | 当前 `extension_mcp_calls` 限制 principal 为 `mcp:%`，唯一键为 `(principal_id,tool_name,idempotency_key)`。改为单一 `extension_entry_calls`，在现有字段基础上引入 `entry_kind IN ('cli','mcp')`、用 `entry_name` 存签名局部名，记录完整扩展 ID，唯一键 `(principal_id,extension_id,entry_kind,entry_name,idempotency_key)`。允许真实本机主体和已认证 MCP 主体。扩展身份及租约由 Core 单独绑定。保留原请求指纹、效果种类、调用次数、时间及有界原回执的约束；请求指纹覆盖作用域、实际入参和受控字段。去掉数据库与代码中的仅 MCP 调用者前提。State1 不新增第二套回执或旧数据转换。 |

E11 的六项业务操作及其现有授权如下。签名声明列明允许调用的操作，Core 在调用时检查扩展已获权、真实调用者可执行并可访问目标。写入绑定原计划/任务，guest 不能自选用户已确认的执行目标。

| `uses` | 现有业务授权 |
| --- | --- |
| `projects.list`、`projects.get` | `projects.read` |
| `packages.planUninstall` | `projects.read`、`repositories.read`、`packages.read` |
| `packages.applyPlan` | 以上三项及 `packages.manage` |
| `operations.get` | `operations.read` |
| `operations.cancel` | `operations.cancel` |

同步范围包括[基础 world](../../specs/extensions/wit/extension-v1/world.wit)、[MCP provider world](../../specs/extensions/wit/mcp-tools-v1/world.wit)、[宿主入口](../../apps/alcomd-extension-host/src/main.rs)、[私有宿主 IPC](../../crates/alcomd-extensions/src/host_protocol.rs)、[原生协议 DTO](../../crates/alcomd-protocol/src/m8_tools.rs)、[CLI 分派](../../apps/alcomd-cli/src/main.rs)、[Manifest Schema](../../specs/extensions/manifest-v1.schema.json)、[权限规范](../../specs/extensions/permissions-v1.md)、[扩展威胁模型](../../specs/security/extension-threat-model.md)与[首发注册清单](../../specs/first-release-contracts.json)。构建门禁应以消费真实绑定的签名组件验证任意端组合。

## 第一方管理功能的替换与保留

原生 GUI 的「设置 → 客户端」、CLI 与 MCP 的业务入口复用现有 Core 方法 `mcp.clients.list/create/issueCredential/revoke`、`mcp.runtime.snapshot`、`mcp.transport.snapshot`，以及 OAuth 登记 list/set/revoke 和授权请求 get/decide。GUI 登记、签发、吊销及活动查看不再经过 Portable UI guest。CLI 当前的 `mcp client` 仅有 list/issue-credential，`mcp oauth` 已有登记和授权请求管理。MCP 客户端管理、审核和活动工具与具体命令名由原生三端矩阵核定，遵守同一套一次性凭据签发与显示规则。令牌交换、传输报告、请求绑定是协议和传输内部职责，不能作为活动管理工具向普通用户转发。签发返回的一次性凭据只交给明确执行该操作的原生调用者，不写日志、通用历史或扩展观察投影。

第三方继续可用 `alcomd:mcp-management/clients@1.0.0` 的 list/create/revoke、`alcomd:mcp-runtime/observations@1.0.0` 与 `alcomd:mcp-transport/observations@1.0.0` 的 snapshot。这些公开接口原本不签发明文凭据。保留下列五个扩展业务权限及各自投影：`mcp.clients.read`、`mcp.clients.manage`、`mcp.connections.read`、`mcp.requests.read`、`mcp.subscription-streams.read`。保留这三套 WIT 及其 deps、宿主 linker、应用/Store 投影、RPC 和真实隔离访问测试。独立的 MCP provider、示例及工具 WIT 继续维护，只按 E1–E11 修订。第一方包的清单路径列在 E13，公开接口参见[客户端 RPC](../../specs/rpc/m8-mcp-clients-v1.md)、[观察接口](../../specs/extensions/wit/mcp-runtime-v1/runtime.wit)、[传输观察接口](../../specs/extensions/wit/mcp-transport-v1/transport.wit)及[公开管理接口](../../specs/extensions/wit/mcp-management-v1/management.wit)。

### E13 · 逐项整文件删除清单

以下路径均为仓库根目录相对路径。前六项构成整个已跟踪第一方 MCP 扩展。末两项是直接从该后端构建的测试产物。删产物前须让公开 runtime/management WIT 在独立、可重建的测试 guest 上通过。

| 路径 | 用途 |
| --- | --- |
| `extensions/first-party/alcomd-extension-mcp/README.md` | 第一方管理包说明 |
| `extensions/first-party/alcomd-extension-mcp/alcomd-extension.toml` | 第一方包声明 |
| `extensions/first-party/alcomd-extension-mcp/backend/Cargo.toml` | guest crate |
| `extensions/first-party/alcomd-extension-mcp/backend/Cargo.lock` | guest 专属依赖锁 |
| `extensions/first-party/alcomd-extension-mcp/backend/src/lib.rs` | guest 生命周期、界面与观察 |
| `extensions/first-party/alcomd-extension-mcp/backend/src/management.rs` | guest 管理页面与交互 |
| `crates/alcomd-testing/fixtures/m8/mcp-runtime-v1.wasm` | 第一方 guest 编译产物 |
| `crates/alcomd-testing/fixtures/m8/mcp-runtime-v1.provenance.json` | 上一项的来源及输入哈希 |

### E14 · 混合文件原位修订

- 在 Core/Application/Store 及其测试中移除专为此包设置的 `PACKAGED_MCP_EXTENSION_ID`、`PackagedMcpExtension`、构造器参数、首次启动自动登记/启用及重开检查。保留通用 `first_party_packaged` 安装源，Discord 导入仍使用它。修订交叉引用、`feature-parity.toml`、`xtask/src/main.rs`、`scripts/check.ps1`、`scripts/check.sh`、`scripts/common.ps1`、`scripts/common.sh`、`scripts/validate-metadata.py`、`.github/workflows/ci.yml` 的包验证、依赖锁与缓存。更新项目规则、架构、ADR 0012、当前决策及 M6/M7/M8 合同。
- 真实 guest 回归见 `apps/alcomd-extension-host/tests/support/m8_management_flow.rs`、`m6_lifecycle.rs`、`minimal_component.rs` 和 `crates/alcomd-testing/tests/m8_management_contract.rs`。独立 `crates/alcomd-testing/fixtures/m8/transport-guest-source` 和其 `mcp-transport-v1.wasm`/provenance 仍在验证第三方接口。为 runtime/management 补齐独立来源并执行真实调用。保留本机无扩展时的三端管理回归、第三方获得/失去管理或观察授权后的正反例。
- `crates/alcomd-testing/fixtures/m7/mcp-management-snapshot.json` 是多端共用的 Portable UI 一致性向量；GUI renderer、浏览器 harness、Rust `m7_contract`、`m7_headless_conformance` 同时消费。将内容和引用原位调整为普通第三方管理 UI 示例，保留表单、无障碍及危险操作确认覆盖。混合 H4/M8 浏览器套件及人工记录中涉及原生管理的用例移到原生客户端设置，涉及通用扩展授予/撤销的用例仍在扩展管理测试执行。

## 实施后的验收证据

1. 对真实签名组件跑后端加三种可选声明的全部八个组合，覆盖仅后端和仅 CLI 都无需 GUI export、声明存在而对应 ABI 不符在安装前失败、远程与本地包结果相同。安装预演、SDK 描述、三端发现、宿主调用核对同一个摘要与扩展 ID。
2. 未授予可选业务权限时，CLI/MCP 仍能列出声明，受限调用没有业务效果。授予后可调用，撤销后立即重新拒绝。覆盖纯计算、grant 变更时续页、禁用/隔离/卸载后去除目录项、同名不同扩展、同一扩展 CLI/MCP 同名但实现不同和未声明业务调用。
3. 在真实 Core 启动 CLI 与 OAuth MCP。冻结原计划、修订和键，覆盖取消确认、确认后一次提交、系统断开、相同请求重试、不同参数、越主体、撤销认证、权限撤销及相同本地名跨入口。逐项断言业务写次数、Core 效果与重放结果。静态合同检查覆盖 State1 单一表、1 个新能力与 2 个原生方法、权限目录和 Schema/SDK/WIT 一致。
4. 三个原生入口在无管理扩展时完成客户端登记、签发、活动查看和撤销；验证一次性秘密不出现在列表、观察投影和日志。普通第三方 guest 独立验证已公开的五项授权、管理和两类观察 WIT。分别保留 WASM 签名、清单边界和资源限制回归。

现有定向入口包括 [CLI 真实调用](../../apps/alcomd-cli/tests/m1_cli.rs)、[MCP 扩展工具流程](../../apps/alcomd-mcp/tests/support/extension_tools_flow.rs)、[Host 管理接口](../../apps/alcomd-extension-host/tests/support/m8_management_flow.rs)、[Store 权限和 DDL 测试](../../crates/alcomd-store/src/sqlite_authority_tests.rs)、[MCP 工具静态合同](../../crates/alcomd-testing/tests/m8_extension_tools_contract.rs)和[GUI 权限浏览器回归](../../apps/alcomd-gui/tests/browser/m8-management-permissions.spec.ts)。上述回归是后续实现的验收条件，本次 PR1 审阅只核对仓库现状。
