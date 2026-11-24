# M12 完整产品安装、更新、卸载与发行

2026-11-25 所有者批准阶段 0 的永久身份和发行合同，进入阶段 1。正式签名、远端写入和发行各有独立审批点。[阶段 0 冻结合同](M12-stage-0-release-contract.md)

## 1. 目标与交接

从干净的签名源码候选执行 `cargo xtask dist --target <target>`，构建、验证并打包可独立运行的完整产品。用户可首次安装、从已发布的 v3.4.0 迁移、更新整套 v4、恢复中断的操作，以及按范围卸载。验收证据追溯到同一源码 SHA 和各格式的实际文件 SHA-256。

- M-1 审计已完成。M11 的 Windows/Linux 原生迁移与候选 `fb6b0455` 的 Windows/Ubuntu Hosted 门禁及 CodeQL 已通过。M12 复用迁移账本、核对逻辑和冻结样本，验收实际安装包与包管理器的交接。[M11 实施记录](M11-v3-migration-and-zip.md)
- 按所有者决定延期 macOS 原生验收。Windows 与 Linux 的合同、实现和隔离验收可以先推进。保留 macOS 构建和包静态检查设计，等运行器与 Apple Silicon 客户端可用再分别取证。A-017 仍要求三平台四格式才能公开发 4.0.0。若拟先公开 Windows/Linux，需单独批准发行范围变更、平台声明和频道策略。
- 阶段 1 复用 Updater 的限额、无跟随文件校验、Ed25519 和 SHA-256，以及既有扩展包验签。分发工具复用工作区依赖和锁文件，使用已锁定的 `tempfile` 创建同卷暂存与合成夹具，避免自行维护临时路径和创建失败后的清理。共享文件校验可减少离线检查和后续安装校验之间的差异，桥接与普通发行仍各自校验完整载荷合同。Bootstrap 已能暂存、核对和提交 v3 资料，Core 安装登记已有安全读取器。M12 补齐构建打包、登记写入、正常 v4 下载与替换、安装范围转换、卸载和发行流程。
- 沿用产品清单的技术身份、首发 v1 合同和 Core 唯一状态写入者。Tauri 负责 GUI 构建，并可作为受控打包子工具。旧解析器仅进入独立临时迁移产物。Python/.NET、Loopback API、PKG、RPM、Flatpak、Snap 按现行路线留在 4.0.0 发行范围之外。[架构与发行基线](../architecture/ALCOMD-V4.md#22-更新与发行) · [ADR 0015](../adr/0015-platform-product-packaging.md)

## 2. 实施顺序与完成条件

| 阶段 | 实施内容 | 进入下一步的条件 |
| --- | --- | --- |
| 0 合同冻结 | 提交 AppId、目录及入口图、登记事务、更新 Schema、信任和频道规则、卸载资产归属、发版清单与风险 | 人工批准本计划和待冻结合同。Windows/Linux 隔离环境、平台样本和构建工具来源已登记 |
| 1 完整构建 | 实现统一 staging、版本推导、组件核验、平台适配和合成签名。生成离线候选 | 干净 checkout 可构建，坏载荷被拒，产物清单可复验 |
| 2 安装与卸载 | 接入 Windows 双范围、AppImage、DEB，维护登记、CLI 和系统入口，并实现安装修复与精确清理 | 真机或独立完整客户端虚拟机通过初装、启动、修复、范围转换和卸载矩阵 |
| 3 整产品更新 | 接通 updater、Bootstrap 与平台适配，完成 v4→v4 同频道、故障恢复与包管理器边界 | 同一候选各适用平台通过真实更新、失败恢复、断电与资料保全 |
| 4 发布版交接 | 从冻结 3.4.0 启动完整 bridge 或进入受控包管理器交接，按账号迁移与核对，并补齐 macOS 所需证据 | 真实平台、实际产物和真实冻结版通过，审批清单中的清理收据齐全 |
| 5 发版演练与生效 | 候选冻结、签名、隔离发布演练、复检公开通道和停止机制 | 同 SHA 门禁与包摘要一致，独立人工批准公开资产和频道生效 |

每阶段在 ExecPlan 记录候选、差异、失败、回退与复验。新增生产依赖先记录用途、替代方案、维护风险和许可证。涉及公共 RPC、数据库 Schema、权限名、Extension API 或迁移删除清单时先提交差异与证据，获批后实施。

## 3. 构建、产物与安装登记

- 构建目标为 `x86_64-pc-windows-msvc`、`x86_64-unknown-linux-gnu` 和 `aarch64-apple-darwin`。Linux 目标产生 AppImage 和 DEB。阶段 0 的产出包括目标和组件清单、允许的辅助资产、签名输入与输出、源提交和制品摘要的对照。用坏版本、缺组件、意外文件、错误权限、路径逃逸、重复条目及错误目标的负样本执行拒绝验证。
- `dist` 根据源提交、`alcomd.product.toml`、锁文件及固定工具链构建 daemon、GUI、CLI、MCP、Extension Host、Bootstrap、Updater 和获批的第一方扩展。单独校验第三方运行库、权限、许可证、源码获取方式及扩展 API 范围。Discord 沿用第一方签名/加载合同，新装默认禁用，旧状态按 M11 迁移与授权。
- 分别生成构建清单、待签字节、签名材料附属件以及最终资产清单。记录源码 SHA、版本、目标、工具及依赖、各资产尺寸与 SHA-256、应用层验签结果、平台签名状态和授权的文件角色。实现 `dist --check` 或等价离线 fixture 路径，使用隔离测试密钥。正式配置缺少可信公钥、必需签名、组件或许可证时构建门禁失败。当前 Authenticode、Developer ID、公证标记为按现行策略未要求。实际系统警告仍纳入验收。
- 复用 `core-installation-v1`。每用户登记只写规范、受控、可验证的版本化运行目录，按产品身份派生 Core 名称。Bootstrap/平台适配器在产品初始化和健康检查完成后原子切换登记与公开入口，失败恢复上一有效登记，卸载按确切所有权失效。复用同用户 IPC、链接与权限检查、单实例和 5 秒就绪预算。升级期间外部 Node/Rust SDK 尝试启动时应连接已就绪的实例，或收到可处理的未就绪错误。测试缺失登记、失效登记、并发首次连接和空闲后重启。
- 机器级安装由特权适配器维护程序与机器入口。各账号在自己的身份下初始化和切换登记、资料与凭据。未登录的其他账号保持原状，下一次启动时依据获批的机器安装记录重新校验并初始化。卸载后待处理账号的旧登记应无法启动已移除的 Core。以真实标准用户和与其不同的 UAC 管理员账号验收。
- 发布版本与渠道是阶段 0 提案的参数。使用候选源码标识与明确批准的首发版本，构建程序和包管理器元数据校验版本一致。开发仓库当前版本与旧测试资产只用于验证。v3 bridge 需要的临时迁移器和冻结版命名签名工件只随专用交接产物交付。

## 4. 平台安装与卸载矩阵

| 平台/格式 | 实现边界 | 放行所需的客户端证据 |
| --- | --- | --- |
| Windows x86_64 · 单个 Inno Setup EXE | 默认不提权，安装到 `%LOCALAPPDATA%\Programs\ALCOMD\`。显式选择所有用户才请求提升，安装到 `%ProgramFiles%\ALCOMD\`。永久 AppId 单独冻结。只把对应作用域的 `bin\alcomd-cli.exe` 所在目录加入 PATH，Inno 管理卸载登记、快捷方式、AUMID、协议和获批文件关联。 | Windows 10 22H2 和 Windows 11 各验证真实下载、SmartScreen、WebView2 渲染、托盘、默认及非 ASCII 自定义路径、两种安装范围、双向转换和拒绝/取消 UAC。测试已装 Evergreen、缺失时在线 Bootstrapper、离线 Standalone 和失败恢复。记录依赖来源、签名与摘要。验证两个账号资料隔离、无双活实例、重复安装/修复、PATH 与系统项的拥有权、升级和卸载。 |
| Ubuntu 22.04 x86_64 · AppImage | 便携文件移动可重新定位。提案使用按用户、版本管理且可核验的持久运行目录供 Core 冷启动。FUSE 临时挂载仅服务便携应用运行。GUI 显式操作可建立 `~/.local/bin/alcomd-cli`，稳定入口绑定当前已核验的应用实例。 | 真实 AppImage 挂载运行、首次初始化、跨目录移动、CLI 修复/移除、外部 SDK 冷启动、便携更新及中断恢复。验证旧 AppImage 可启动和未知同名文件保全。覆盖 Ubuntu 22.04 和较新 Ubuntu 运行冒烟。 |
| Ubuntu 22.04 amd64 · DEB | `/usr/bin/alcomd-cli` 和内部版本目录由包管理器拥有。应用提示或交给包管理器完成程序更新，用户态 Bootstrap 处理各账号的数据。维护脚本只协调包和系统入口，具备幂等性。 | 以真实 `apt/dpkg` 验证安装、依赖、重装、升级、移除与 purge。注入解包/配置中断，验证包管理器恢复、资料保全及重新登记。若正式提供 APT 仓库，另验证归档认证、频道和撤回。独立下载 DEB 也核对发布清单与摘要。 |
| macOS arm64 · DMG | `ALCOMD.app` 包含完整载荷，`MACOSX_DEPLOYMENT_TARGET=11.0`。拖入 Applications 后逐账号初始化，CLI 仅从 GUI 显式启用。延期保留验收项。 | 在 Apple Silicon 设备测试未公证的真实下载和 Gatekeeper 用户路径、挂载/拖入、首次启动、Keychain、CLI 重定位、整套更新、卸载及回滚。Hosted 构建与 Mach-O 静态证据另记录。 |

Windows 双范围互斥和自定义根按现行基线实施。转换前检查机器与发起账号的安装实例，计划明确旧作用域及新作用域。其他账号的资料、登记和入口需要具备确切归属与批准才能操作。冲突实例归属、当前程序占用或作用域无法确认时，安全拒绝或等待可重试状态。在唯一活动版本和健康状态确定后再清理旧入口。[Windows 安装器基线](../baselines/windows-installer-options.md) · [Inno 安装模式](https://jrsoftware.org/inno-setup/documentation/topic_admininstallmode.htm) · [Microsoft WebView2 分发](https://learn.microsoft.com/microsoft-edge/webview2/concepts/distribution)

Linux 在 Ubuntu 22.04 构建，逐个核对发行 ELF 和本地动态库，最高要求 `GLIBC_2.35`。冻结 v3.4.0 Linux 样本的 `GLIBC_2.39` 交接使用 Ubuntu 24.04 合适环境另验。DEB 由包管理器接管程序文件，保留已核验的旧包与可执行恢复流程。在下载/依赖/解包/配置失败或中断时验收实际 `dpkg` 状态，恢复后用完整版本重新核对。AppImage 与 DEB 相遇时验证安装归属及入口冲突，避免对另一格式拥有的文件或数据进行替换。[Debian 维护脚本政策](https://www.debian.org/doc/debian-policy/ch-maintainerscripts.html)

### 卸载、所有权与验收快照

- 默认卸载停止当前实例的相关进程，清理明确拥有的程序、安装登记、快捷方式、CLI 入口、PATH 项、协议/关联及临时更新文件。保留其他安装、共享运行库、第三方关联及用户自定义内容。若用户或第三方已接管同名入口，按现场归属保留。AppImage/DMG 覆盖显式移除集成及系统直接移除本体后的失效处理。DEB `remove`/`purge` 管理软件包自己的系统资源。
- 用户数据默认保留。清理个人设置、托管缓存、运行日志、凭据及外部 MCP 客户端条目时，提案列出具体账号、实例、精确对象、数据影响和恢复源，并单独确认。保留 Unity 工程、备份、用户模板、外部目录、第三方资源和未证实归属的文件。路径换向、链接/重解析点、归属或摘要变化时拒绝清理。
- 安装前、提交后与卸载后对照进程、文件、路径权限、注册表/桌面项、凭据引用、CLI、MCP 与各账号的数据快照。对比全新 v4 加等价数据的结果。零残留判据限定为已验明并获准清理的产品或旧版实例。用户数据和第三方的预期保留项按内容摘要与拥有者核对。[已批准实例边界](../../migrations/v3/artifacts.toml)

## 5. 正常更新与 v3 桥接

### v4 → v4

1. Updater 读取经批准的 stable/beta 端点，用有界输入验证产品、目标、版本、格式和当前渠道。提案为应用层签名覆盖元数据、资产尺寸和摘要，冻结签名域、信任锚标识、HTTPS 地址与重定向、文件规模、有效期及过期响应/重放规则。拒绝跨平台、错误范围、篡改、版本回退和格式不符。stable 不接收预发布，beta 切回 stable 需要用户显式选择并遵守版本推进规则。覆盖无更新、非 2xx、错误响应类型、响应丢失、过期元数据和不完整下载，核对已有完整安装可继续使用。初次安装的人工校验方式与自动更新共用正式签名清单。
2. 下载到隔离临时位置并核对签名和摘要，发布前再次核验同一文件身份。Bootstrap 取得更新锁、协调 GUI/Core/CLI/MCP/Host 的活跃进程、保护已接受任务并记账。存好完整旧程序、旧登记与允许恢复的本机状态，准备新版本并做健康检查，然后持久化提交意图、切换入口和重开验证。
3. 提交前自动保全或恢复前一完整版本。提交意图之后依据同一事务恢复到经验证的完整版本。失败时使用经确认的恢复包与配套状态。遇到无法由旧版本安全读取的数据，保留数据和恢复线索，明确阻止盲目降级。拒绝同时运行不兼容的两套 Core。清理旧程序按专用所有权和保留窗口执行。Windows 更新走完整 Inno 包，AppImage 使用同卷便携替换，DMG 的整产品替换延期实测，DEB 留给包管理器。
4. 用同一候选衍生两个测试版本，实测检查、下载、取消、部分写入、重复请求、响应丢失、原子提交、重启恢复及旧版本重开。各持久阶段覆盖断电或进程终止、占用、权限、磁盘满与签名错误，另测同时启动 CLI、多个 MCP 和扩展时的状态治理。不因没有历史公开 v4 而加入开发格式兼容逻辑。

### v3.4.0 → v4

- Windows 从冻结更新器验证完整 bridge 的实际下载、旧钥验签以及原安装范围传递，保留白名单参数并验证未知参数。验证 v3.3.0 先经原渠道到 3.4.0。分别核对当前用户、全用户、多个登录账号、UAC 取消和旧版恢复。若使用冻结 updater 的别名文件名，按冻结规则签名并纳入资产清单。核对 Windows 256 MiB 下载上限与 WebView2 离线方案，超出时提交方案差异审批。
- AppImage 和延期的 macOS 使用用户主动启动的独立 bridge，先保全旧版可执行源，再独立暂存 v4。冻结的 macOS/AppImage 自动更新会原地替换旧版，故公共旧 feed 的响应设计必须显式验证这些客户端不会误进入覆盖式桥接。DEB 以包管理器交接程序，按账号运行数据迁移。无适用资产时验证实际客户端提示，并核对 stable/beta 共用服务与按平台响应的缓存隔离。[冻结更新 API](../baselines/update-api.md)
- 复用 M11 签名的发布候选健康报告和逐账号本机核对。签名报告绑定候选载荷摘要与平台、安装范围、已执行的 Core RPC/GUI/CLI/MCP/第一方扩展证据。逐账号核对导出摘要、数据内容、记录数与重开结果，资料保留在本机。用户批准迁移范围后进入 Bootstrap 提交。清理针对另行确认的旧实例。真实业务数据、路径、凭据或旧 token 不进入发行报告。
- 默认沿用现有 v3 应用层签名钥处理冻结桥接。v4 首发沿用既定密钥选择，阶段 0 核对冻结指纹、v4 载荷与健康证明验证公钥，以及两种签名格式的实际互操作。需要变化时以明确证据进入单独换钥审批。开发与 Hosted 用合成密钥。正式签名只在获批准的隔离流程签发已核对的准确字节。公开退出策略允许暂停推送或恢复上一兼容频道指针，已安装用户的程序/资料恢复走正式恢复流程。

## 6. 放行证据与公开发行顺序

1. 按 [发行测试矩阵](../testing/release-matrix.md) 与 [结构化测试计划](../testing/test-plan.toml) 逐项执行。保留整仓 Rust/前端/Chromium、外部 Node/Rust 消费者、三平台 Hosted CI 和 CodeQL 同一 SHA 门禁。真机/完整客户端虚拟机以实际发行包补齐快捷方式、协议/关联、真实 WebView、键盘焦点、主题、高对比和缩放。覆盖 Windows Narrator、Linux 可用读屏器及延期的 macOS VoiceOver。记录实际测试系统、安装范围、版本、包摘要、阶段结果和可追溯的最小证据。
2. 冻结候选和发行清单。为四种实际安装包、更新 Manifest、桥接资产与第一方扩展生成对应签名/摘要和许可证资料。从干净机器重新下载、验签、安装并核对。用于实测的资产与提交批准的发布资产应具有同一摘要。重打包或重签后重新执行受影响验证。
3. 先在隔离发行位置验证上传、下载、恢复和停止机制。提交负责人、资产列表、公开地址、版本/tag、各平台状态、发布说明、健康报告及恢复演练证据。获批后创建正式 tag 和发布记录，上传已冻结的资产，核对下载字节与签名。候选 tag/发布方式按批准策略执行。
4. 获得单独放行后切换更新频道和旧入口。先以已批准的 beta 或限定人群部署，观察约定窗口，复核可用率、签名拒绝、迁移/提交失败、旧版可重开和用户数据保全。稳定频道需要单独放行。监控信号、观察期、负责人、阈值和停止动作在发版前冻结。出现错误频道/版本、摘要不符、状态损坏或无法恢复的迁移时立即停止该频道推进，保存证据，核对恢复指针并复验冻结客户端的实际响应。
5. 公开前验证隐私保护。保留脱敏流水线、摘要、失败分类与恢复证据，使用隔离数据验证普通错误、更新界面、支持日志和迁移报告。遥测服务、真实更新端点及第三方服务写入各按审批矩阵执行。

## 7. 审批矩阵

| 审批点 | 需要所有者确认 | 获批可执行范围 |
| --- | --- | --- |
| 实施与合同冻结 | 本计划、永久 Windows AppId、最终身份与布局、AppImage 持久运行目录、登记所有权和事务、更新/频道/恢复合同、卸载清单的政策 | 在现有主工作树实现和本地检查，使用合成密钥和合成资料。公共合同、权限或生产依赖的变化另行审查 |
| 真实客户端隔离验收 | 可用 Windows 10/11、Ubuntu 与日后的 Mac 测试入口，隔离账号、样本、允许的故障注入、项目/备份只读副本、实际客户端的管理员操作 | 在所列隔离环境安装、升级、卸载、回滚与清理测试。用户日常系统与旧资料写入单独批准 |
| 正式签名及密钥策略 | 旧钥指纹对应、正式工具与签名身份、权限、旋转/吊销与应急流程、候选源码 SHA、未签输入与预期发布的准确摘要 | 仅签批准的候选资产、更新元数据与平台健康报告。Authenticode、Developer ID/notarization 若启用另批 |
| 清理真实旧实例 | 确切机器与账号、实例归属、对象列表、备份/恢复源、凭据与外部配置影响、需要移动的用户目录 | 按已批准清单执行，归属或内容漂移时停止对应清理 |
| Git 和外部环境操作 | 签名提交与远端快进的范围，CI 所需远端操作、隔离托管位置与账号、上传范围 | 按获批引用、目标和动作执行。GitHub 设置、Secrets、真实更新端点与签名材料沿用仓库既有禁止边界 |
| 公开标签和发行资产 | 版本/tag、GitHub Release、资产、签名、发布说明、可公开平台、M12 真机与 CI 证据。明确授权创建 tag、上传和公开各项操作 | 创建获批的 tag、上传准确资产并公开所批准的发行记录。macOS 未验收时拟分平台提前发版需先修订 A-017 |
| 生产分批上线和应急恢复 | 具体新旧域名、stable/beta 分流、阶段、人群、恢复源、阈值、观察期、操作人及停推授权 | 仅执行已批准的频道变更。生产操作由所有者或其明确授权的发布者执行。恢复、撤回或变更超过预批准范围时另批 |

**M12 工程验收完成条件**为四种完整产品产物、对应平台的真实客户端生命周期验收、同 SHA 门禁、签名与可验证恢复证据均齐全，并已同步状态、合同快照与结构化测试记录。**公开发行完成条件**为对应审批已落实，公开下载的制品已验签并与清单一致，已获准生效的频道已验证响应并完成观察。

## 进度记录

- 2026-08-16 建立后续 ExecPlan。
- 2026-11-25 整理完整实施顺序、平台验收和审批矩阵。按所有者决定延期 macOS 原生验收，继续按 A-017 记录正式发版条件。
- 2026-11-25 所有者批准阶段 0 的永久 Windows AppId、安装与登记布局、v4 更新 Schema 和卸载归属规则。[阶段 0 冻结合同](M12-stage-0-release-contract.md)

## 阶段 1 运行载荷

`cargo xtask dist --target <target> --build --staging <new-directory> --source-sha <full-commit> --discord-package <archive> --discord-sha256 <sha256>` 在匹配目标的原生系统上构建七个程序。它用已配置的 Git 信任验证源码签名，核对干净工作树、锁文件和已固定摘要的第一方 Discord 包，再发布同卷暂存的运行目录。Linux、macOS 原生构建还执行现有的系统版本门禁。工作区内的暂存路径须放在忽略的目录。每次构建使用产品清单中的版本。

`--prepare --staging <directory> --source-sha <full-commit> --discord-sha256 <sha256>` 可为单独构建的运行目录生成待签清单。此入口核对目录内容、目标与第一方扩展，构建来源由构建记录证明。清单绑定版本、目标、源提交、完整文件集合、大小和 SHA-256。签名输入由 `ALCOMD product payload v1`、一个 NUL 字节以及 `product-payload.json` 的准确字节组成。`product-payload.sig` 是 128 个小写十六进制字符。正式 `--check` 以构建时注入的发布公钥验证准确字节，再核对每个文件。

`cargo xtask dist --target x86_64-pc-windows-msvc --package-layout --format inno --source-sha <full-commit> --staging <signed-runtime> --output <new-directory>` 导出 Windows 产品打包输入。它复验同一签名运行载荷后生成 `runtime/<version>`，并按签名清单的文件摘要校验 `bin/alcomd-cli.exe` 与 `bin/vcruntime140.dll`。输出在同卷暂存后发布，已有输出或不匹配的候选会被拒绝。产品安装脚本及安装事务接线归阶段 2。

用合成签名载荷验证了导出、重验、拒绝错源、拒绝用途不匹配和拒绝覆盖。Inno 6.5.4 直接消费导出的两个目录生成隔离夹具。Windows 11 Sandbox 实测安装、稳定 CLI 冷启动、卸载程序与登记并保留状态库。Ubuntu 22.04 的共享打包回归通过。

对 Windows Release 程序的 PE 导入检查确认六个组件依赖 `vcruntime140.dll`。Windows `--build` 接收来自 Visual Studio x64 再发行目录的 `--windows-crt <dll> --windows-crt-sha256 <sha256>`，复制到七个程序所在目录。`--prepare` 接收相同的 `--windows-crt-sha256`，并核对已暂存文件。Windows 发行验证要求同目录存在签名清单覆盖的 x64 DLL。隔离候选选用 Microsoft 签名的 14.44.35211.0，SHA-256 为 `d5e4d9a3e835fa679450145d6a7d94e36573a509317111904d9b3712c30d9066`。[Microsoft 再发行规则](https://learn.microsoft.com/cpp/windows/redistributing-visual-cpp-files)

Windows 发行构建为 Cargo 和 Tauri 提供已核验为同一对象的普通路径。Tauri 的权限文件枚举使用 glob，Windows 扩展路径中的问号会使该轮枚举遗漏 Core 权限。构建继续校验现有的 GUI 权限合同。[Tauri 权限路径问题](https://github.com/tauri-apps/tauri/issues/11950)

`--check --fixture` 创建并校验一次性夹具。`--prepare --fixture --staging <directory>` 可用公开合成密钥暂存真实的开发二进制，随后用 `--check --fixture --staging <directory>` 复验。合成载荷有独立用途标记和全零源提交，正式验签路径拒绝。普通运行载荷要求产品许可证和第三方声明，排除 v3 迁移器与 Loopback API。Windows 专用辅助库仅接受清单允许的文件名。

平台封装、外部发行资产清单与签名、干净签名候选验收仍按阶段 1 继续。安装登记、卸载与 v4 下载更新分别按后续任务接入。

2026-11-25 Windows 上完成离线验证。三个目标的合成载荷通过。用 M11 已编译的七个 Windows 程序、两个 VC 运行库与 Discord 包组成合成签名载荷，复验清单摘要 `e1089401c7801b5e00722311bb0b35e399383353d65e1942cc9df44827624ab6`。重复准备保持原清单，修改文件后拒绝，恢复原字节后再次通过。发行构建拒绝当前未提交工作树，在拒绝时未创建目标目录。Bootstrap、Platform、Updater 和 xtask 共 85 项通过，4 项沿用既有跳过。Ubuntu 24.04 WSL 下 glibc 门禁接受要求 2.34 的 ELF，并拒绝要求 2.38 的 ELF。

M9 留存的第一方 Discord 测试包通过扩展验签、固定摘要与产品清单字段核对，错误摘要被拒绝。本机 Git 使用 M11 留存的公开签署人清单对当前 HEAD 完成一次性验签。

上一轮 Windows 开发工作树的七个 Release 程序均已编译，Tauri GUI 采用 no-bundle 构建。将新编程序、已验签的 Discord 测试包和许可证文件放入含中文及空格的目录后，合成载荷准备与再次验签均通过，清单 SHA-256 为 `015f1bb87d6215bf8a6852d7c9c2de216d416c117f8f2b3b735e60a0667ec5f5`。正式验证入口在构建未配置发布公钥时拒绝。同轮 Linux x86_64、macOS arm64 的 Updater 与 xtask 交叉静态检查通过。

### 原生格式的测试包

`cargo xtask dist --target <target> --package-layout --fixture --staging <signed-fixture> --format <inno|appimage|deb> --output <new-directory>` 仅导出测试身份的打包输入。工具按签名清单复制，复核副本签名和摘要，再同卷发布完整布局，目标已存在则拒绝。

- Windows 对输出目录内的 `fixture.iss` 执行 `ISCC.exe`。测试包使用独立 AppId、当前用户权限和空白目标目录。安装程序只写测试载荷，无快捷方式、注册表、卸载登记及自动启动。
- Linux AppImage 对 `AppDir` 使用 `ARCH=x86_64 appimagetool --no-appstream --runtime-file <verified-runtime> AppDir <fixture.AppImage>`。测试入口打印载荷清单 SHA-256。Linux 原生导出保留运行文件权限，跨文件系统准备时在隔离的 Linux 暂存中设置正确模式后打包。
- Linux DEB 对 `deb-root` 执行 `dpkg-deb --root-owner-group --build deb-root <fixture.deb>`。测试包使用 `alcomd-dist-fixture` 包名及其专用内部目录。按 Debian 版本格式生成预发布版本，验证时用 `dpkg-deb --extract`。

工具来源和核验记录如下。它们用于开发验收，生产运行时无需携带这些工具。

| 工具 | 核对的 SHA-256 |
| --- | --- |
| [Inno Setup 6.5.4](https://github.com/jrsoftware/issrc/releases/tag/is-6_5_4) | `fa73bf47a4da250d185d07561c2bfda387e5e20db77e4570004cf6a133cc10b1`，Authenticode 签署者为 Pyrsys B.V. |
| [appimagetool x86_64，11d54c9](https://github.com/AppImage/appimagetool/releases/tag/continuous) | `e8c1169c4af66c3184d7ee110713d4be04befe671fa358e2c1591ee840553b03` |
| [AppImage Type 2 x86_64 runtime](https://github.com/AppImage/type2-runtime/releases/tag/continuous) | `617aa77314d57c8b8e4fae9c3f0b35af7b30d5529cb69e98f64f92510ed98a46` |

Windows 用实际编译的七个运行程序生成 Inno 测试 EXE。仅向隔离工作目录展开，第二次展开到原目录被拒绝，原载荷再次验签通过。Ubuntu 24.04 WSL 中，AppImage 与 DEB 使用由 `/usr/bin/true` 生成的七个 ELF 测试文件验证容器流程。AppImage 启动测试入口并打印摘要，DEB 元数据为 `alcomd-dist-fixture 4.0.0~alpha.0-1 amd64`，两种包解出相同签名清单 `cc35d4888e7e80d4ea0d002cb81b11615247f04131bf1c383f8c7f3328b4a27f`。外层测试包摘要如下。

| 格式 | SHA-256 |
| --- | --- |
| Inno EXE | `a81861d460d76326e93b684b000f3673fa39286614b51624f98e2c19d0e2dd4c` |
| AppImage | `7d3f2f8018cc7aa9884046998d58268c2cf0846c1eac15880870ee9fe9ad0f49` |
| DEB | `f7157fec77ae5fb5bc1b5dbf148d6e668a55a0e958c6e864b49f4268b19b794f` |

产品安装包的身份、启动登记、集成入口和生命周期按阶段 2 实施。三平台四格式的公开放行按 A-017。

### 2026-11-25 Windows Core 安装登记

`4a80ac26` 实现 Windows 登记与精确注销。Bootstrap 核验安装载荷后，由当前账号下的 Core 获取单实例锁、完成初始化并发布登记。注销获取同一锁，使用已核验的文件句柄仅移除归属相符的登记。`98784db0` 补齐真正卷根的 ACL 校验。它通过打开的句柄识别卷根，仍检查 `FILE_DELETE_CHILD` 及其他适用的危险授权。

Windows 11 Enterprise 24H2 Sandbox 中，合成签名载荷以 `98784db0` 编译 Core、CLI 和 Bootstrap。9 项验收覆盖签名候选不匹配、未知状态拒绝、首次及重复登记、外部 CLI 冷启动、Core 仍在运行时拒绝注销、停止后注销、重复注销及注销后拒绝冷启动。Inno 6.5.4 的当前用户夹具另外通过安装并登记、卸载程序并注销、保留状态库。夹具使用独立 AppId。正式安装器的范围转换、入口、WebView2 与更新流程仍需接线和验收。

全用户夹具用 `/ALLUSERS` 部署到 Program Files，安装阶段没有在执行安装的账号下初始化 Core。两个普通账号分别登记、创建状态库并从安装目录外冷启动同一套程序。注销只影响发起账号。卸载程序后两份状态库均可读取，另一个账号的遗留登记无法启动已卸载的 Core。这轮覆盖 Win11 的夹具流程。Win10、UAC 取消以及最终安装器仍待验收。

Windows 11 Sandbox 另用无本地管理员权限的账号通过当前用户夹具的安装、登记和卸载。卸载保留状态库。

Windows 11 Sandbox 在签名夹具中打开实际 Tauri 窗口，检测到 WebView2 `157.0.4341.60` 的渲染进程，CLI 通过真实 Core IPC 返回 `ready`。`3eaf0935` 将客户端目录的稳定 CLI 校验对齐到同一安装根的 `runtime/<version>` 和 `bin/alcomd-cli.exe`，并通过 Windows 和 Ubuntu 22.04 的定向回归。

### 2026-11-25 Ubuntu 22.04 隔离构建

独立 Ubuntu 22.04.5 x86_64 WSL 使用经过摘要校验的 Canonical Jammy 镜像，实际 glibc 为 2.35。验签后的 `4a80ac26` 干净检出编译全部七个发行程序，glibc 门禁测得最高要求 `GLIBC_2.34`。Tauri GUI 采用 no-bundle 构建。正式产品的 AppImage 持久登记、DEB 系统生命周期和客户端验收继续按阶段 2 执行。[Canonical Jammy 镜像摘要](https://cloud-images.ubuntu.com/wsl/jammy/current/SHA256SUMS)

原生 `appimagetool` 拒绝了仅构建账号可读的桌面文件。`c6c9ede0` 在 AppImage 和 DEB 导出时明确写入目录、程序和普通文件模式，保留签名内容。Ubuntu 22.04 的限制性 `umask` 下 5 项 xtask 测试通过。复验用七个实际编译的组件，验证 AppImage 原生挂载和清单签名。DEB 经 `dpkg` 安装及同版本重装，普通账号从包安装目录冷启动 Core，卸载移除程序且保留状态库。正式产品的系统入口、升级和恢复沿用发行矩阵。

同一套 Ubuntu 产物通过 Xvfb 启动了原生 Tauri 窗口。核对 WebKit 渲染进程和真实 Core IPC 返回的 `ready`。

### 2026-11-25 签名格式预验

从冻结 Git 对象复核了 v3 公钥的 Minisign key ID `DDEF8C83EF8404B0`。提取的 32 字节 Ed25519 公钥 SHA-256 为 `85928b8e67983ce4e9289635ebc284a16e9ee91e609cd24e0a5433719c41c094`。

用官方已验签的 Minisign 0.12 和全新合成密钥生成两种签名。Minisign 验过预哈希封装。v4 运行载荷验签器接受同一密钥签出的原始 Ed25519 签名，拒绝 Minisign 封装，并在恢复原始签名后再次通过。合成试验覆盖发布签名域的准确字节，准备好复用同一 Ed25519 密钥的格式验证。正式签名仍按阶段 0 的授权边界执行。[Minisign 格式](https://jedisct1.github.io/minisign/#signature-format)

另用 Tauri 签名工具生成有密码的合成密钥。Tauri 签发的预哈希封装通过 Minisign 验证。同一加密密钥经解开 Tauri 外层编码后由 Minisign 签出原始 Ed25519 签名，两者用同一公钥通过验证。试验结束移除了合成私钥文件。

### 2026-11-25 首批工程候选门禁

签名候选 `5d58b43b` 的干净检出通过本地整仓、前端、SDK 和 762 项 Chromium。推送前逐笔验签 8 笔提交，远端 `main` 快进到该候选。四项 CodeQL 已通过。同一候选的 macOS 15 arm64 Hosted 首次尝试因运行器容量取消，启动前无测试步骤运行。[CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/45479473999) · [Hosted CI](https://github.com/ALCOMD/ALCOMD/actions/runs/45479474031) · [macOS 首次尝试](https://github.com/ALCOMD/ALCOMD/actions/runs/45479474031/job/138748427086)

同一 SHA 的 Windows Server 2025 和 Ubuntu 22.04 Hosted 均通过。两端完成整仓检查、外部原生 SDK 消费者、CLI 冷启动、Release 与 Tauri 构建和 762 项 Chromium。Ubuntu 编译产物最高要求 `GLIBC_2.34`。[Windows Hosted](https://github.com/ALCOMD/ALCOMD/actions/runs/45479474031/job/138748426712) · [Ubuntu Hosted](https://github.com/ALCOMD/ALCOMD/actions/runs/45479474031/job/138748427300)

同一 SHA 仅重跑 macOS 15 arm64。该轮取得运行器，`Check` 执行到 `xtask` 时 5 个分发夹具报路径并非普通目录。测试将带链接祖先的系统临时路径传给生产目录校验。macOS 的 Chromium、仓库外 SDK 验收和后续构建未执行。[macOS Hosted 第二次尝试](https://github.com/ALCOMD/ALCOMD/actions/runs/45479474031/job/138798375951)

### 2026-11-25 Windows 全用户安装首次启动

`25afa078` 接通官方 Windows 入口。没有登记且 IPC 尚未就绪时，受保护的稳定 CLI 和版本化 GUI 可调用同一运行目录中的 Bootstrap。Bootstrap 核验已安装载荷，Core 在当前账号下初始化并发布登记后服务 IPC。独立 SDK 继续按登记定位 Core，沿用五秒就绪预算。

Windows 11 Sandbox 使用 Inno 6.5.4、合成签名和 Release Bootstrap，安装包 SHA-256 为 `2d75f526a149b8b213c35a0eeee4067c53f558b5fa1a1fed2bbff21a200f0d90`。全用户部署未写入安装账号的资料。签名篡改后，普通账号的资料目录保持空白。恢复有效签名后，普通账号首次运行 `bin/alcomd-cli.exe` 收到 `ready` 并发布本账号的登记。另一普通账号的四个并发 CLI 均在约 1.4 秒内收到 `ready`，按 SID 观察到一个 Core。Windows 客户端与平台库的 91 项测试通过，3 项沿用既有跳过。

Debug GUI 试验未启动 WebView2。改用干净 `25afa078` 的 `npm run gui:build -- --no-bundle` 构建 Release GUI 后，替换合成签名夹具中的 GUI 并重签，安装包 SHA-256 为 `dd7c1f097235817f2ff89ef7b986647887a90fdbabf18f3b1bcd46652c937e05`。隔离图形会话显示 `ALCOMD3` 窗口和 5 个对应 WebView2 进程，约 2.1 秒发布 Core 登记。禁止自动启动的独立 CLI 返回 `ready`。同一轮通过 7 项检查，涵盖全用户部署、签名拒绝、普通用户单次及并发首启、GUI 首启、外部程序读取登记、卸载后状态保留及残留登记失效。

### 2026-11-25 Ubuntu 22.04 同候选原生夹具

从独立 Ubuntu 22.04 x86_64 的干净签名检出构建 `25afa078` 的七个 Release 程序。最高 glibc 符号版本为 `GLIBC_2.34`。在限制性 `umask` 下构建 AppImage 与 DEB 的合成签名夹具，二者解出相同清单 `08bd169468b2bdf92b145444433675ad95577afc62237b57225384025f9c20f4` 并通过验签。AppImage 用原生 FUSE 挂载，DEB 用真实 `dpkg` 安装、重装、冷启动并移除，保留状态库。Xvfb 中由同一组二进制验证了 Tauri 窗口、WebKit 渲染流程和实际 Core IPC。

| 格式 | SHA-256 |
| --- | --- |
| AppImage | `9571a9b6a4b157a29d6bfc4147610423359defe16bd75c574eb93161fe641a1f` |
| DEB | `cca36ec9ee5706cf385f868bfa4068ee9a87970c13d29b6c9728b4501b49bdc9` |
