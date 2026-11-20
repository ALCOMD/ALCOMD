# M9 ALCOMD Discord Presence

状态为已完成。扩展沿用 v3 的 Discord 应用身份。当前阶段见[项目状态](../status.md)。

## 交付范围

| 项目 | 已确认值 |
| --- | --- |
| 显示名 | **ALCOMD Discord Presence** |
| 计划仓库 | 公开的 `ALCOMD/alcomd-discord-presence` |
| 扩展包 ID | `com.vrclearn.alcomd-discord-presence` |
| Discord Application ID | `1536569303653154996`，沿用冻结 `v3.4.0` 配置中的 `discordApplicationId` |

扩展在独立仓库使用第三方开发者可用的公开版本化合同构建，提供构建、测试、打包和安装调试步骤，继续使用 WASM/WASI 后台和 Portable UI。M9 首版从 Unity 会话接入。Blender 与 Photocraft 属于后续可接入的状态来源。

扩展处理 Discord IPC、Rich Presence 生成、连接和重连、更新和清除、隐私设置及预览。主仓提供应用无关的项目与 Unity 用例、权限、后台租约和组件运行时。发行默认策略继续按 A-019，发行装配按 M12。[ADR 0013](../adr/0013-discord-first-party-extension.md) · [已接受决策](../decisions/accepted.md)

## 现有公开接口核对

| 需求 | 已有入口和结论 |
| --- | --- |
| 设置、隐私选项和预览 | `guest-ui` 和 Portable UI 支持呈现与操作，`host-data` 支持扩展命名空间内设置的读取、写入和删除。直接复用。 |
| 已知项目的安全摘要 | `host-projects.get-summary` 可按已知 `ProjectId` 读取显示名、项目种类和可选 Unity 版本。直接复用。 |
| 后台启停 | `guest-lifecycle.activate/deactivate` 与 `background.run` 支持生命周期和后台租约。Core 的运行时维护会续租和回收，但没有定期调用 guest 的入口。 |
| 发现、跟踪 Unity 项目会话 | 已公开的扩展激活上下文只有扩展及实例信息。当前 guest WIT 缺少读取当前 Unity 会话或接收其变化的入口。Core 已有按项目检查 Unity 写入状态的用例，可作为进一步核对的上游证据。 |
| 与本机应用通信 | 当前 Host 链接的公开 WIT 没有本地 IPC 通道。扩展的 Discord 协议实现缺少可用传输。 |

接口依据为 [当前 Extension WIT](../../specs/extensions/wit/extension-v1/world.wit)、[激活上下文](../../specs/extensions/wit/extension-v1/types.wit)、[公开 Host 合同](../../specs/extensions/host-protocol-v1.md)及 [Unity 状态用例](../../apps/alcomd/src/m5_rpc.rs)。

## 第一批公共合同

以现有 `extension-v1` world 为基础，增加独立版本化接口和 `background-integration-v1` world。现有 world、调用签名和旧组件的 linker 保持兼容。只有在 Core 授权、宿主连接和兼容性验证齐备后才发布新 world 的运行时资格。[WIT](../../specs/extensions/wit/extension-v1/background-integration.wit)

### Unity 会话与后台回调

- 建议 `host-unity-sessions.current` 受 `unity.sessions.read` 的 `Global/*` grant 保护。成功结果包含可选的当前会话和已确认编辑器数。当前会话仅含 `ProjectId` 和 Unix 毫秒启动时间。空会话表示观测成功但没有匹配项。读取失败、证据失效或未初始化返回稳定的不可用错误，guest 清除当前展示。项目名和 Unity 版本继续通过 `host-projects.get-summary` 读取。
- Core 在有调用者时每 5 秒共享一次采样。复用现有进程观测、已知编辑器可执行文件身份及登记项目路径身份。对 PID 与进程启动时间的组合去重，排除可识别的辅助和批处理进程。能验证前台编辑器时选前台，否则选最近启动者。Windows 使用前台窗口 PID，macOS 使用前台应用 PID。Ubuntu 首版在无可信前台证据时使用最近启动规则。前台候选须出现在同一批已确认进程中。原 Unity 写入状态分类不变。
- 建议新增 `guest-background.tick`。仅声明该接口且持有有效 `background.run` 租约的活动实例每 2 秒获得一次调度。调用不重入，忙时最多保留一个待处理 tick，租约失效时丢弃。采用现有单实例、2 秒普通调用和调用速率预算。UI 调用继续使用原接口。后台调用由有界任务驱动，Core 的 250 毫秒维护循环只排程和回收。

macOS 前台采样使用已锁定的 `objc2-app-kit = 0.3.2` 与 `objc2 = 0.6.4`。仅在 macOS 平台层开启 `NSWorkspace` 和 `NSRunningApplication` 所需的特性，在采样线程为每次调用建立 autorelease pool。许可证为 Zlib、Apache-2.0 或 MIT。直接调用 Objective-C runtime 会扩大项目自有 FFI，使用辅助进程则增加发行和后台生命周期成本。选择上游 AppKit 绑定，维护风险是 macOS 系统框架和绑定升级需定向复验，特别是无桌面会话时的无前台结果。[绑定上游](https://github.com/madsmtm/objc2) · [Apple 前台应用接口](https://developer.apple.com/documentation/appkit/nsworkspace/frontmostapplication)

### 本机 IPC 目标与句柄

- 签名 Manifest 声明有限的命名本机应用目标，每个目标包含稳定标识、各平台对端身份约束及有界候选规则。Discord 扩展声明 Windows 本机命名管道和 Unix 套接字候选。Core 在受信运行时目录和本地管道命名空间内解析，guest 不传任意路径。应用身份必须在连接后、交付句柄和首次写入前验证。Windows 校验管道服务端 PID、用户、会话和可执行文件发布者。Unix 校验本地套接字、peer credentials 及可验证的应用身份。身份不明时拒绝，端点和进程证据不暴露给 guest。跨平台采用各自可验证的证据，身份验证的具体策略须随威胁模型报审。[Discord IPC](https://docs.discord.com/developers/topics/rpc#rpc-over-ipc)
- 建议 `host-local-ipc` 提供 `connect(target-id)`、限长 `read`、`write` 和 `close`。`connect` 返回不透明句柄与持有该连接的本地进程 PID，用于应用协议关联，不返回对端 PID。连接绑定扩展、实例、租约、授权修订及签名目标描述摘要。每个实例同时最多一条连接，单次读写最多 64 KiB，操作受现有调用预算限制。明确返回未就绪、超时、拒绝、对端关闭和句柄失效。部分写入后无法继续时关闭流，扩展重新握手。平台归属、路径及传输细节保留在 Core/Host，Discord 协议及消息格式留在扩展。[SET_ACTIVITY 参数](https://docs.discord.com/developers/topics/rpc#set_activity)
- 建议权限 `local-ipc.connect` 使用 `LocalIpcTarget` 范围，资源键绑定签名目标描述的规范化摘要。会话投影、IPC、调度均在每次调用或排程时复核当前租约和修订。远程管道、任意地址、产品私有 IPC 及身份不匹配的候选必须拒绝。授权失效或对端退出时关闭句柄，重连不得沿用过期句柄。

### 用户启用与存储兼容

- 标准可信界面显示扩展实际请求的 `background.run`、`unity.sessions.read`、`projects.read` 及目标 IPC 范围。「启用展示」一次确认。扩展 UI 负责隐私设置与预览，启用操作由可信 GUI 发起。启用 RPC 只接受与当前扩展、已验证包摘要、目标摘要及预期修订一致的同意。Core 同事务提交实际 grants 与启用状态，并复用幂等控制。旧启用请求、guest、安装、重启和重连不得隐式授予权限。显式撤权停止该次展示，并阻止旧请求重新授予。
- State45 从 State44 增量迁移，只扩大 `extension_grants` 的权限名和范围 `CHECK`，保持旧行和旧分支等价。项目摘要沿用 `Global/*`，后台权限沿用当前 Extension 范围。无需新增会话表、连接表或 Discord 设置列。扩展设置继续走现有命名空间。
- 正常停用在有效授权和租约范围内给 guest 一次有界清除机会，并禁止此时打开新通道。显式撤权的持久化仍是授权失效点，生效后阻断旧授权下所有 I/O 并立即断开。卸载、租约到期及异常退出统一回收资源。真实客户端验证断连后的清除表现，不能以宿主回收代替 Discord 端证据。

## 实施顺序与停止点

1. 核对私有运行时入口和 State44 约束。用合成对端验证 Windows 管道能识别实际服务端进程。用已确认的 v3 Discord Application ID 和获准的本机客户端验证握手、展示、清除及对端应用身份。真实证据未具备前，继续实现独立的 WIT 和 Unity 观测，IPC 对端身份策略与运行时启用停在预验点。
2. 公共合同获批后先接通 Core 授权、运行时调度、Unity 投影和三平台 IPC，再用普通第三方样例覆盖合同。真实用户环境写入与对外展示需在获准的测试环境执行。
3. 按独立授权创建公开仓库。锁定已批准的公开合同版本和摘要，交付锁定依赖的构建、测试、`.alcomdextension` 打包、测试签名、安装和调试步骤。独立检出验证通过后，移除主仓占位 backend 的构建、锁文件和脚本引用。新增生产依赖先记录用途、替代方案和维护风险。
4. 扩展首版启用后立即尝试连接。Discord 未启动时退避重试，启动、重启或换端点后自动恢复并重新读取当前会话。可用会话发布 Presence，空会话或观测失败清除。相同内容去重。Portable UI 提供连接状态、预览，以及项目名、Unity 版本、编辑器数量和自定义文本设置。默认只展示通用 Unity 编辑状态；详细字段由用户开启，自定义文本上限 128 个 Unicode 字符。预览和发送共用隐私过滤。
5. 在主仓和独立仓库候选固定后验证并记录 SHA、合同摘要及测试包摘要。通过适用的 Windows、Ubuntu、macOS Hosted CI 和 CodeQL。扩展合同测试覆盖旧组件、授权迁移、并发启停、失效句柄、目标拒绝、I/O 限额、回收及普通第三方包。扩展测试覆盖协议失败和重连、多开选择、隐私设置、预览及清除。Windows 的真实 Unity 与 Discord 闭环是 M9 收尾门槛，须覆盖 GUI 关闭、Discord 晚启动及换端点、项目切换和结束、停用、撤权、卸载和异常退出。Ubuntu 与 macOS 的真实桌面闭环可延期，按平台记录。
6. M9 收尾更新本 ExecPlan 和 `docs/status.md`。新用户默认安装但禁用沿用 A-019，v3 启用状态迁移归 M11，正式签名、随包装配与发行归 M12。

## 准备阶段记录

- 2026-11-20 第一批合同已获实施批准。已核对只读 v3.4.0 提交 `4aa98ae4f18d42c10137278997180dbede991e88` 及 `alcomd3.config.json` blob `b6780e8155f73db6808483e5ddd23cc5c0bc88ab` 与冻结记录一致，沿用其中公开的 Application ID。[冻结配置](https://github.com/ALCOMD/ALCOMD3/blob/4aa98ae4f18d42c10137278997180dbede991e88/alcomd3.config.json)
- Windows 合成管道使用独立子进程和一次性名称。`GetNamedPipeServerProcessId` 返回实际服务端 PID，`ProcessIdToSessionId` 确认同会话，错误的预期 PID 校验失败。已经通过开始菜单查到可执行的 Discord 安装，客户端未运行。应用发布者身份核验和 Discord 协议仍待在获准使用的账号上真实预验。独立仓库创建与推送仍按各自授权执行。
- 本地候选新增 `background-integration-v1` WIT，以及 Core 的 Unity 会话采样、缓存和安全投影。Windows 和 macOS 提供前台采样，Ubuntu 使用最近启动规则。`m5::tests` 10 项及 Wasmtime WIT 绑定 1 项通过，M6–M8 的三组旧扩展合同共 16 项通过。受影响包严格 Clippy、fmt、元数据校验、macOS 和 Linux 平台层交叉检查通过。`cargo xtask check` 首轮发现 Discord 包 ID 校验仍用旧后缀，与产品身份派生规则对齐后通过。交叉检查未启动真实桌面。新 WIT 尚未在 Extension Host 注册，数据库仍为 State44。
- Windows 只读预验确认已安装 Discord `1.0.9269` 的 Authenticode 签名有效，签名组织是 `Discord Inc.`。已准备忽略目录内的 `target/m9-discord-preflight.ps1`，脚本缺省只做签名检查。真实模式需要显式开关，核对管道服务端、Windows 用户、会话和签名后才握手，发布合成内容并请求清除。真实模式待账号授权。

## Windows 真实预验与 IPC 接线

- 所有者明确批准使用当前 Windows Discord 账号。第一轮完成管道对端核验，探针读取异步握手时失败，未发送 `SET_ACTIVITY`。修正 `ValueTask` 等待后，真实 Discord `1.0.9269` 确认 `READY`、合成 `ALCOMD M9 connectivity test` 的 `SET_ACTIVITY`，以及紧随其后的 `activity: null` 清除。展示不含项目名或路径。取证范围为 Discord IPC 回执。
- Windows 原生连接器随后以无消息模式打开并关闭同一签名客户端的管道。正确发布者通过，错误发布者被拒绝。`windows-sys` 保留锁定版本，仅扩展对管道服务端 PID、进程会话、WinTrust 和证书属性所需的 Windows feature。
- 新 world 由签名 Manifest 三项必需接口明确选择，扩展 Host 接通后台回调、Unity 投影及有界 IPC。Windows 按用户、会话、可执行文件名和 Authenticode 组织验签，macOS 按签名标识和 Team ID，Linux 按同 UID 和已声明的系统安装路径验证实际进程。Core 在每个操作期间持有经验证的授权，数据库将撤权提交排在在途操作之后。State45 保留旧授权，没有自动授权。
- 本地 M9 Host 合同 3 项通过，包含真实 Wasm fixture 经 Host 私有管道执行后台 tick、Unity 读取和 IPC 打开/写入/读取/关闭。fixture SHA-256 为 `1c2d589077ab5915c65c1dfd7ca231cf841692a0fe508aec208b8d298edc88d5`。旧 Host 6 项、Store 17 项、迁移 4 项、M6–M8 合同 27 项分别通过。Windows 受影响包严格 Clippy、元数据和仓库检查通过。
- macOS 和 Linux 的平台适配器通过目标 Clippy。完整 Core 交叉检查在 `aws-lc-sys` 构建阶段受本机外平台 C 工具链限制，本轮没有对应的完整 Core 或真实桌面结果。Linux 目前支持签名目标所声明的系统路径，安装在用户目录或独立沙箱容器的客户端未纳入此合同。
- 这一阶段通过已有的显式权限入口验证精确目标。后续候选新增标准 GUI 的一次启用和常规停用的有效租约清除流程。

## 独立扩展与产品闭环

- Guest 使用已锁定且在主仓现有组件中使用的 `wit-bindgen = 0.57.1`、`serde = 1.0.229` 与 `serde_json = 1.0.151`，用于生成公开 WIT 绑定、精确保存隐私设置及编码 Discord JSON。替代方案是手写 Component ABI 和 JSON 解析，成本集中在兼容性、消息边界及 Unicode 校验。依赖限于不继承 WASI 环境的 Guest，升级绑定生成器或 JSON 解析器时需复验，同步锁文件。wit-bindgen 为 Apache-2.0 WITH LLVM-exception，Serde 为 MIT 或 Apache-2.0。独立仓库交付前在主仓现有扩展目录开发，从只含公开合同快照的独立检出验证。
- 已按授权创建公开的 [ALCOMD/alcomd-discord-presence](https://github.com/ALCOMD/alcomd-discord-presence)，默认分支为 `main`。独立检出提供锁定的 Rust 工具链、三平台 CI、构建说明和开发签名工具。组件编码器使用主仓同版本的 `wit-component = 0.254.2`。它只用于开发打包，随扩展分发的 WASM 不链接编码器。签名工具使用 OpenSSL 3 的 Ed25519 和 Python 标准库，生成的开发密钥和包在忽略目录中。独立仓库本地候选 `4ee687ecbfd811aa3f67670382c1a2fd0aed3a26` 已由真实 Windows 用户签名并验签。远端仍为空，默认 CodeQL 已配置，等待首笔提交推送后运行。
- Windows 独立检出通过 Guest 和编码器严格 Clippy、Guest 3 个单元测试与 1 个文档测试、WASM release 和组件编码。开发包内容摘要为 `0184f648bdc34612648365541492a3cb590426ebbc09c0efd522595301eb13ff`，归档 SHA-256 为 `60bac5acc95d327516312b2f7808598ca6ffb70d6014a0f1aa1d34ec0ae68520`。归档存在于本地开发输出，正式签名归 M12。独立检出通过后，主仓移除了重复的后端、锁文件和构建引用，保留产品 Manifest 和公共合同。WIT `background-integration.wit` SHA-256 为 `04d5ffb117c33cf3ca9a71764f4f76127b503b66e6ddae7c5c9980de882c78c0`，7 份 WIT 文件逐字节匹配主仓。
- 可信界面显示统一授权和签名目标的本平台身份。Core 将同意绑定到候选包摘要与注册修订，在一个事务内写入 grants 和启用，权限页支持撤销精确目标。常规停用先给实例一次有界清除机会，再提交停用并收回句柄。显式撤权先提交，再停止实例。该 RPC 变化是可选字段和独立启用请求类型，旧组件和无本机目标的启用入口仍按已有合同运行。
- Core 的空闲退出检查包含已经授权的定向后台集成。当前 Core 进程中的有效租约和符合条件的崩溃重启等待期会保留 Core。停用、撤权、隔离或所有有效工作结束后恢复原空闲退出。
- 独立签名包通过 Windows 原生隔离 Core 流程。安装后无 grants，缺失、过期或不匹配的确认被拒绝，单次有效确认授予四个权限并启动后台。Portable UI 三项信息开关默认关闭，修改开关与合成自定义文本后可读取已保存设置。撤销精确目标会停用，重放旧启用请求无法恢复授权，新的显式同意可以重新启用再正常停用。此用例无需 Unity 展示。
- Chromium 定向通过 17 项，覆盖扩展工作区和 Portable UI。标准启用确认显示签名目标，取消时不调用启用，确认时只发一次带摘要的启用请求。权限页按精确目标范围发送撤权。
- Windows 整仓的早期轮次发现两处旧权限断言需要同步。一处补齐目录中的两个新权限，一处保留普通权限的必需和可选断言，并验证 `local-ipc.connect` 必须是可选权限且带签名目标和完整的启用权限。
- Windows 整仓收集轮为 1103 通过、4 失败、21 忽略。4 项失败来自三处旧断言。修正后的协议 Schema 53 项、State44 迁移保真复验 1 项和真实 MCP 传输订阅复验 3 项通过，传输复验另保留 2 项既有忽略。整仓严格 Clippy、fmt、`cargo xtask check`、原生 GUI 编译检查、前端检查和构建及元数据校验通过。
- 主仓候选 `6e95e205` 的 Windows 单元与集成测试随后达到 1107 通过、0 失败、21 忽略。运行期间发现空闲退出缺口并开始修复，旧轮次的 rustdoc 在源码更新后遇到旧 trait 产物冲突。后续按更新后的源码复跑整仓 doctest、严格 Clippy、fmt、仓库与元数据检查均通过。独立签名包的 Windows 原生复验确认断开最后一个客户端后后台继续运行，停用并断开后 Core 自动退出。签名包复验首轮在并行编译期间触及 Host 既有 5 秒就绪上限，编译结束后按相同上限复验通过。修复后的原生扩展生命周期回归为 12 通过、0 失败、3 忽略，开发签名包用例另行通过。
- Windows 真实产品验收用例已编译，仍待同一 Discord 账号的实际展示和重连授权。用例 `discord_publishes_and_clears_generic_activity_for_real_unity` 使用临时 Core 数据、独立开发包、真实 Unity Editor 和两个合成工程。真正的 Windows 展示、清除与异常生命周期，以及两仓的 Hosted CI 和 CodeQL 仍是 M9 收尾条件。

## 2026-11-20 实机验证与远端复验

- 所有者批准两仓快进和当前 Windows Discord 账号实机验证，包括为测试重启 Discord。首次快进了主仓 `d3c07abc` 和独立仓库 `4ee687e`。主仓原有四笔提交随普通快进一并推送，所有新增提交逐笔验签。
- 独立仓库经 `0ff04bcb094d2929618bec9b62d7e3b74c1477a8` 补齐 Rust 和 Python 源码 CodeQL。Windows、Ubuntu、macOS 的开发包构建全部通过，默认 Actions CodeQL 通过，Rust 和 Python 源码分析均通过且各报告 0 项发现。这个提交只添加扫描工作流，实机包内容、摘要和 WIT 保持与 `4ee687e` 一致。[三平台 CI](https://github.com/ALCOMD/alcomd-discord-presence/actions/runs/44638151791) · [Actions CodeQL](https://github.com/ALCOMD/alcomd-discord-presence/actions/runs/44638151294) · [源码 CodeQL](https://github.com/ALCOMD/alcomd-discord-presence/actions/runs/44638151598)
- Windows 实机使用隔离的 Core、两个合成工程、Unity 2022.3.22f1 和已签名的 Discord 1.0.9269。仅发布通用文案 `Editing in Unity`。Discord 对展示与工程结束后的清除均返回 IPC 回执。断开最后一个客户端后重新连接 Core，当前展示仍有效。晚启动、切换工程、显示期间停用、撤权、卸载均通过，Discord 客户端日志分别记录了停用、撤权和卸载的连接关闭。测试结束后本人资料的「动态」页未见测试状态。补验时资料面板未显示已获 IPC 回执的活动，因此未取得断开自动清除的视觉对照。补验结束时的显式清除也获 Discord 回执。
- 第一次换端点失败定位到已占用的旧命名管道。Windows 暂态 `ERROR_PIPE_BUSY` 现在会跳过该候选，并对下一个候选继续执行签名对端核验。定向 Windows 测试和实机复验通过。签名 Discord 从 `discord-ipc-0` 换到 `discord-ipc-1` 后重新确认展示，旧端点测试对端收到零协议字节。终止本轮隔离 Host 后，新 Host 重新发布，Discord 返回回执，测试完成后签名 Discord 客户端已恢复运行。[Windows 管道错误合同](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-client)
- 主仓 `d3c07abc` 的四语言 CodeQL 通过。其 macOS Hosted CI 检查因先前深链接提交缺少 `tauri::Manager` 导入而编译失败，已在 `3155d104` 修复。Ubuntu 的六项 MCP 综合用例因复用默认 Unix IPC 端点而报 `AlreadyRunning`。`b22ca2cc` 同时包含 macOS 编译修复和 Windows 换端点修复，Ubuntu 再现了这组夹具失败。[首次 Hosted CI](https://github.com/ALCOMD/ALCOMD/actions/runs/44636975978) · [首次 CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/44636974959) · [换端点修复候选 CI](https://github.com/ALCOMD/ALCOMD/actions/runs/44639111803)
- `7812dd31` 为 MCP 真实测试统一隔离 Unix 端点。Ubuntu 和 macOS 的 MCP 综合测试各 20 通过、1 失败、3 项原有忽略。六项 `AlreadyRunning` 已消失，剩余 Provider OAuth 夹具误连默认端点，授权入口返回 HTTP 503。Windows 本机同组 21 通过、0 失败、4 项原有忽略。[夹具首轮 Hosted CI](https://github.com/ALCOMD/ALCOMD/actions/runs/44640177927)
- 主仓签名候选 `4043d7a817378b84e9796c3b8ad1ba417ed785ce` 让 HTTPS OAuth 夹具也必须传入相同的隔离 Core 配置。Windows 本机 Provider 三传输定向测试 1 通过、0 失败，严格 Clippy、格式和元数据检查通过。同 SHA 的四语言 CodeQL 和三平台整仓 `Check` 通过。Windows、macOS 完整 Hosted CI 通过。Ubuntu CLI、构建和平台门禁通过，完整 Chromium 回归因 60 分钟作业总时限被终止。[候选 Hosted CI](https://github.com/ALCOMD/ALCOMD/actions/runs/44642231588) · [候选 CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/44642232180) · [Ubuntu 作业](https://github.com/ALCOMD/ALCOMD/actions/runs/44642231588/job/135975916436)
- 主仓签名候选 `d25eefe2cee53e5c3e6e74b971d3e67e61808deb` 将 Ubuntu Hosted CI 的作业总时限设为 90 分钟，同 SHA 四语言 CodeQL 通过。Ubuntu、macOS 完整 Hosted CI 通过，Chromium 各 738 项通过。Windows 通过 Chromium 前的全部步骤，浏览器回归记录了 706 项通过后因 90 分钟作业总时限被终止。[Hosted CI](https://github.com/ALCOMD/ALCOMD/actions/runs/44650589035) · [CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/44650589310) · [Windows 作业](https://github.com/ALCOMD/ALCOMD/actions/runs/44650589035/job/136004180764)
- 主仓签名候选 `feca8df3bee266c515a4a8806fe85d955ad0d85c` 将三平台的作业总时限统一为 120 分钟，为冷构建和完整浏览器回归预留时间。沿用原测试和单项限额，仓库检查通过，候选已快进至 `main`。[Hosted CI](https://github.com/ALCOMD/ALCOMD/actions/runs/44661714414) · [CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/44661714489)

## 2026-11-20 M9 收尾

- 主仓 `feca8df3bee266c515a4a8806fe85d955ad0d85c` 的 Windows Server 2025、Ubuntu 22.04 和 macOS 15 arm64 Hosted CI 均通过。三平台均完成整仓检查、CLI 自动启动、二进制和 Tauri 构建、适用平台边界检查，以及各 738 项 Chromium 回归。真实 MCP 综合测试各 21 通过、0 失败，Windows 有 4 项原有忽略，Unix 各 3 项。Rust、Python、JavaScript/TypeScript 和 Actions CodeQL 四项均通过。[Hosted CI](https://github.com/ALCOMD/ALCOMD/actions/runs/44661714414) · [CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/44661714489)
- 独立扩展 `0ff04bcb094d2929618bec9b62d7e3b74c1477a8` 的 Windows、Ubuntu 和 macOS 构建及 Actions、Rust 和 Python CodeQL 均通过。主仓与独立扩展的签名候选均已验签并与各自远端 `main` 一致。两仓所用的七份 WIT 文件逐字节一致，`background-integration.wit` SHA-256 为 `04d5ffb117c33cf3ca9a71764f4f76127b503b66e6ddae7c5c9980de882c78c0`。实机开发包内容摘要为 `0184f648bdc34612648365541492a3cb590426ebbc09c0efd522595301eb13ff`，归档 SHA-256 为 `60bac5acc95d327516312b2f7808598ca6ffb70d6014a0f1aa1d34ec0ae68520`。[扩展仓库](https://github.com/ALCOMD/alcomd-discord-presence) · [三平台 CI](https://github.com/ALCOMD/alcomd-discord-presence/actions/runs/44638151791) · [Actions CodeQL](https://github.com/ALCOMD/alcomd-discord-presence/actions/runs/44638151294) · [源码 CodeQL](https://github.com/ALCOMD/alcomd-discord-presence/actions/runs/44638151598)
- Windows 实机的首次完整通过覆盖 GUI 关闭、Discord 晚启动和换端点、Unity 工程切换和结束、停用、撤权、卸载与 Host 异常退出。Discord IPC 回执确认通用展示和清除，本机客户端日志确认停用、撤权和卸载时对应连接关闭。本人资料仅用于观察测试后的状态，未取得断连自动清除的视觉对照。实机后的主仓候选更改限于 MCP 测试夹具与 CI 作业时限。Ubuntu 和 macOS 的真实 Unity 与 Discord 桌面闭环按计划延期。
- 新用户默认安装但禁用沿用 A-019。v3 启用状态迁移归 M11，正式签名、随包装配和发行归 M12。
