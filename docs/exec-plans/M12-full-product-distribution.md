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

接入更新服务后的 GUI 同时导入 `vcruntime140.dll` 和 `vcruntime140_1.dll`。Windows `--build` 从 Microsoft x64 再发行目录接收 `--windows-crt <dll> --windows-crt-sha256 <sha256>` 和 `--windows-crt-companion <dll> --windows-crt-companion-sha256 <sha256>`。`--prepare` 用两个独立摘要核对已暂存文件。两只 DLL 进入同一签名运行清单，缺失、摘要不符和架构不符在生成发行产物前拒绝。稳定 CLI 只导入 `vcruntime140.dll`，仍沿用现有入口布局。沿用发布合同已允许的伴随 DLL 可复用现有签名和安装校验，静态链接或另装全局再发行包会改变部署方式。[Microsoft 再发行规则](https://learn.microsoft.com/cpp/windows/redistributing-visual-cpp-files)

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

### 2026-11-27 同源原生资产组装

`cargo xtask dist --package` 在原生平台从签名源码提交和已验签运行载荷组装 Inno、AppImage 和 DEB。命令校验打包工具摘要，和包一同写出记录源码、载荷及成品摘要的 `asset.json`。用 `a70c181e` 在含空格和 Unicode 的目录完成三种工程包组装。AppImage 的外部打包工具从受核验的暂存目录使用相对参数。错误工具摘要、已有输出、工具失败及工具没有生成成品均被拒绝，已有输出摘要保持一致。Linux 两种成品解包后再次通过载荷验签。

| 格式 | 字节 | 成品 SHA-256 | 组装回执 SHA-256 |
| --- | ---: | --- | --- |
| Inno EXE | 33,815,326 | `f614dfe92ac037e1cf3c6849695ceb46933e20ac1faf523420de9568d5720d43` | `f7bafa853e3f55c554ae6e527128d9b76daa9cf4c433817307956714bb5cbb95` |
| AppImage | 46,586,360 | `21a5581d5bbd5fcc3d9a91c37dac4956441b9cf7984e75c442986674625dd888` | `10cadafb2de297625d92ffe4a0a2911c4f457dece85dae27746987c3e9dcb203` |
| DEB | 38,674,398 | `61472c09b72749e65d317647ae0eb8f9423a9ded3499cae9ec37c0e9f7159ee4` | `30d31e4dc82a93ddeeb71a8fc3bc6e0e4d94d9a40bd52d3bd11a9745107431e5` |

隔离 Windows 11 24H2 通过当前用户 Unicode 路径安装、已安装 WebView2 渲染、稳定 CLI 冷启动和卸载保全。冷启动耗时 2711 毫秒，回执 SHA-256 为 `eabd25f2d747a9f263595f994d547064578f60f8c16e7b3404501dce8cc6f125`。Ubuntu 22.04 在隔离 WSL2 环境验证 AppImage 原生 FUSE 和实际 DEB 安装。两个包在 1400 × 800 和 2800 × 1600 下均验证项目搜索、列表视图和创建项目的键盘焦点。AppImage、DEB 回执分别为 `cc607e34db86453a273b72a51b42caf359f134a2ad80ac145402c6e66dbf7179` 和 `35c24e8d3e9cc88e097cc9245064525141e0464b4f27ef75a0083155d2882784`。

用同一个 Inno 实包补验 Windows 11 全用户安装。已有来宾会话将产品安装到含 Unicode 的 Program Files 子目录，机器部署结束时仍无账号状态库。稳定 CLI 首次连接 Core 耗时 1981 毫秒。真实 WebView2 渲染通过，卸载后状态库摘要一致。回执 SHA-256 为 `27b526a5c97b07b62d6734d93c802876fa194a0f6f71bac1ede77ebcca5543b7`。

同一 AppImage 补验了 Orca 42 的实际合成朗读。首轮读到项目搜索和列表视图，未取得创建项目的朗读回执，键盘阶段日志 SHA-256 为 `8860483526495264f025832be3110dc946027ed8ba61703c2599fae2886e892b`。用相同条件与同摘要资产复跑，三个控件均有朗读输出，隔离音频装置录得 179,744 个合成样本。通过日志 SHA-256 为 `06a15e30ac5d379b8667d0e62cb88f0c16d1df6e76c9c699135b44ce73b6e094`。

本轮运行载荷使用隔离测试签名。相同打包代码与第一轮验收记录一同纳入主仓候选 `696953b1`。该候选通过本地整仓门禁，Rust 1,108 项、Chromium 765 项和 Node SDK 7 项。十笔新增提交逐笔验签后普通快进。Windows Server 2025、Ubuntu 22.04 和 macOS 15 arm64 Hosted 均在该 SHA 上通过整仓、外部 SDK、发行组件构建、Tauri 构建和 Chromium。四项 CodeQL 通过。Hosted CI 与 CodeQL 终态回执的 SHA-256 分别为 `d891514a6acba932b8b85438ac0c8b957dc65cace4c579e81471f0ca33af6675` 和 `46e3ab50181ecae4482975519eb0e44df2d38a2b6d536c461be4290e13cd902d`。Windows 10 和 macOS 原生验收延期，公开发行继续按 A-017 放行。[Hosted CI](https://github.com/ALCOMD/ALCOMD/actions/runs/45935368535) · [CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/45935368135)

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

修订工程候选 `25afa078` 的三平台 Hosted CI 和 CodeQL 均通过。Windows Chromium 首轮失败后，在同一 SHA 的重跑通过整项门禁。下载器候选 `bd13cbed` 在干净检出通过本机整仓、762 项 Chromium 和外部 SDK 合同，快进前逐笔验签四笔提交。同一 SHA 的四项 CodeQL 通过，三平台 Hosted CI 继续跟踪。[25afa078 Hosted CI](https://github.com/ALCOMD/ALCOMD/actions/runs/45507904871) · [25afa078 CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/45507905452) · [bd13cbed Hosted CI](https://github.com/ALCOMD/ALCOMD/actions/runs/45537509247) · [bd13cbed CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/45537509198)

中间候选 `590f7c84` 的三平台 Hosted CI 与四项 CodeQL 均通过。后续下载器候选 `bd13cbed` 的 Ubuntu Hosted 通过整仓、原生 SDK 和发行构建，Chromium 同轮 761 项通过。版本菜单重复选择用例在指定来源后打开第二次确认弹窗时失败。[590f7c84 Hosted CI](https://github.com/ALCOMD/ALCOMD/actions/runs/45531161768) · [590f7c84 CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/45531161429) · [Ubuntu Chromium 首轮](https://github.com/ALCOMD/ALCOMD/actions/runs/45537509247/job/138947625177)

该用例两次选择版本都改为等待实际菜单打开、选项就绪和菜单关闭，再检查 Core 的计划参数。原用例在 Windows 重跑 20 次、Ubuntu 22.04 重跑 30 次均通过。修订后两平台各通过 30 次定向重复和完整候选用例组的 45 项。

同一 `bd13cbed` 的 macOS Hosted 在模板目录失效场景首轮出现提交按钮仍禁用，其余 Chromium 761 项通过。模板用例增加对实际菜单打开、选项就绪、菜单关闭、模板值和 Unity 版本的核对，保留提交和失效断言。Windows 本机对原失败用例重跑 20 次通过，修订后的两个模板场景共复跑 40 次通过，macOS Hosted 另行复验。[macOS Chromium 首轮](https://github.com/ALCOMD/ALCOMD/actions/runs/45537509247/job/138947624770)

同一候选的 Windows Hosted 有 761 项通过。仓库详情用例在选中版本后只记录到更新日志调用。用例现等待版本菜单关闭，分别核对文档和更新日志的路径及调用顺序。原用例在 Windows 本机重跑 20 次通过，修订用例定向重跑 30 次和仓库用例组全部 37 项通过。[Windows Chromium 首轮](https://github.com/ALCOMD/ALCOMD/actions/runs/45537509247/job/138947625006)

### 2026-11-25 Windows 全用户安装首次启动

`25afa078` 接通官方 Windows 入口。没有登记且 IPC 尚未就绪时，受保护的稳定 CLI 和版本化 GUI 可调用同一运行目录中的 Bootstrap。Bootstrap 核验已安装载荷，Core 在当前账号下初始化并发布登记后服务 IPC。独立 SDK 继续按登记定位 Core，沿用五秒就绪预算。

Windows 11 Sandbox 使用 Inno 6.5.4、合成签名和 Release Bootstrap，安装包 SHA-256 为 `2d75f526a149b8b213c35a0eeee4067c53f558b5fa1a1fed2bbff21a200f0d90`。全用户部署未写入安装账号的资料。签名篡改后，普通账号的资料目录保持空白。恢复有效签名后，普通账号首次运行 `bin/alcomd-cli.exe` 收到 `ready` 并发布本账号的登记。另一普通账号的四个并发 CLI 均在约 1.4 秒内收到 `ready`，按 SID 观察到一个 Core。Windows 客户端与平台库的 91 项测试通过，3 项沿用既有跳过。

Debug GUI 试验未启动 WebView2。改用干净 `25afa078` 的 `npm run gui:build -- --no-bundle` 构建 Release GUI 后，替换合成签名夹具中的 GUI 并重签，安装包 SHA-256 为 `dd7c1f097235817f2ff89ef7b986647887a90fdbabf18f3b1bcd46652c937e05`。隔离图形会话显示 `ALCOMD3` 窗口和 5 个对应 WebView2 进程，约 2.1 秒发布 Core 登记。禁止自动启动的独立 CLI 返回 `ready`。同一轮通过 7 项检查，涵盖全用户部署、签名拒绝、普通用户单次及并发首启、GUI 首启、外部程序读取登记、卸载后状态保留及残留登记失效。

从干净签名 `25afa078` 构建全部七个 Windows Release 程序并生成正式布局输入。合成签名的运行清单摘要为 `6b00bb300e44b5e49c18ec2a6123cc5b0346c17548bee8ae2ecfcf4b349af172`。同一个 Inno 夹具 EXE `f67183320fd119190b083b33ae036174c1e8dae01eb10ac514e6c61443f6db4e` 在两个独立 Windows 11 Sandbox 中复验。当前用户范围用普通账号通过安装、CLI 首次启动、Bootstrap 注销登记和保留状态的卸载。全用户范围重复通过上述 7 项检查，四个 CLI 约 1.1 秒全部就绪，只观察到同账号一个 Core。Release GUI 约 2.0 秒发布登记。正式 Inno 事务和完整客户端矩阵按阶段 2 接线。

### 2026-11-25 Ubuntu 22.04 同候选原生夹具

从独立 Ubuntu 22.04 x86_64 的干净签名检出构建 `25afa078` 的七个 Release 程序。最高 glibc 符号版本为 `GLIBC_2.34`。在限制性 `umask` 下构建 AppImage 与 DEB 的合成签名夹具，二者解出相同清单 `08bd169468b2bdf92b145444433675ad95577afc62237b57225384025f9c20f4` 并通过验签。AppImage 用原生 FUSE 挂载，DEB 用真实 `dpkg` 安装、重装、冷启动并移除，保留状态库。Xvfb 中由同一组二进制验证了 Tauri 窗口、WebKit 渲染流程和实际 Core IPC。

| 格式 | SHA-256 |
| --- | --- |
| AppImage | `9571a9b6a4b157a29d6bfc4147610423359defe16bd75c574eb93161fe641a1f` |
| DEB | `cca36ec9ee5706cf385f868bfa4068ee9a87970c13d29b6c9728b4501b49bdc9` |

### 2026-11-25 Ubuntu 产品 DEB 首装

签名源码 `0c99475e1ce78d63f406722ba8280609c47de7cc` 在独立 Ubuntu 22.04 x86_64 构建全部七个 Release 程序。合成密钥签名后的运行清单摘要为 `54ab44fdae998003c8bfc615c6c5ba89ce0561694433893fecd29ed9da0869f8`。产品 DEB 的 SHA-256 为 `61fba1219aedb8de2a9e67288beadce6432fd843c50c76b2217537b2e93573f6`。实际包名、版本和架构为 `alcomd 4.0.0~alpha.0-1 amd64`。包安装稳定 CLI、版本化运行目录和桌面入口。

用真实 `dpkg --unpack` 保留未配置状态，再用 `dpkg --configure -a` 恢复，随后经 APT 重装同一包。安装账号没有生成 Core 登记。篡改签名后首次 CLI 调用失败，目标账号资料目录保持空白。恢复签名后，标准用户分别从 CLI 和实际 Tauri 窗口发布私有登记。独立 Rust SDK 在登记前拒绝启动，并从已登记路径冷启动。执行 purge 后 CLI、桌面入口和运行组件移除，两个状态库的摘要保持一致，残留登记无法启动被移除的 Core。跨版本更新、回滚和 v3 DEB 的包管理交接继续按阶段 3 与 4 验收。

DEB 跨版本登记使用工作区已锁定的 `semver 1.0.28` 校验旧目录名，沿用发行工具对规范 SemVer 和构建元数据的限制。手写版本解析会重复维护合法预发布版本的规则。同一受保护系统安装根内的旧目录须已被完整移除。官方 CLI 或 GUI 经 Bootstrap 核验当前载荷，Core 在实例锁下成功打开现行资料后再原子替换登记。SDK 遇到失效登记时继续返回可处理的错误。未知目录、链接、仍存在的旧运行目录和 AppImage 归属均不接管。

#### Ubuntu 22.04 DEB 跨版本登记与回退

在隔离 Ubuntu 22.04 x86_64 从签名基线 `748933d8` 和只变更版本号的签名检出 `bd30aba3` 分别生成七个 Release 组件和合成签名 DEB。两者最高要求 `GLIBC_2.34`。首先用已验收的 `0c99475e` 版 `alpha.0` 初始化两个账号，再经实际 `dpkg` 升到 `alpha.1`，最后用 APT 回退到相同基线的 `alpha.0`。

| 测试版本 | 包 SHA-256 | 载荷清单 SHA-256 |
| --- | --- | --- |
| `4.0.0~alpha.0-1` | `aa97ee9139577b21e04559cdd4824db546a3764b4c1fc50d76a9763b1fb89c9e` | `0a2e93aa1c7326c1b29c9d22fa89fdd5832190475c8086792d31958fc689254d` |
| `4.0.0~alpha.1-1` | `731b6ceeb70fc5c3d88881442d326e1520db6e586712a4ca1e264e4304154286` | `38cbb6d769dbb931edc81ff411c881262decf4488a9517825802dd50373aa5d1` |

九组原生验收通过。解包结束时旧目录整体移除，两个账号的旧登记仍在，`dpkg --configure -a` 恢复配置。外部 Rust SDK 拒绝失效登记。先用官方 CLI 切换第一账号，再从真实 Tauri 窗口切换第二账号，两人的语言设置保持独立，SDK 可以从新登记冷启动。签名损坏、其他安装、悬空链接、仍有文件的旧目录及未知数据库均按原样保留。使用 APT 回退后，两个账号都冷启动并重新登记。最终 purge 移除程序，两个数据库摘要不变。Jammy 定向 Clippy、两项平台回归和 17 项客户端测试也通过。更广的包管理器故障注入、跨数据版本降级及 v3 实包交接继续按阶段 3 与 4 验收。

### v4 更新声明离线核验

Updater 复用工作区已锁定的 `base64 0.22.1`、`chrono 0.4.45` 和 `url 2.5.8`，分别解析无填充 Base64url、UTC 时间和 HTTPS 地址。手写解析需要独立维护尾位、日历和 URL 规范化，现有依赖沿用工作区的锁文件与依赖审查。本切片复用现有发布信任锚和文件摘要校验，网络下载及平台替换继续按阶段 3 实施。

`alcomd-updater verify-product-update` 使用构建时的发布公钥核验声明原始字节，并据此核对本地资产尺寸和 SHA-256。Windows 定向 Clippy 与 6 项 Updater 测试通过。另使用合成密钥和资产实测 7 个命令场景，覆盖正常验签、同序号同摘要重读、过时序号、同序号不同摘要、错误签名域、等长篡改资产和未批准的资产主机。HTTP 同频道交接、安装提交及回滚尚待接入。

离线核验可在受信任的现有目录启用持久防重放。每个频道、目标和包格式使用单独的锁和回执，签名与资产通过后原子更新。Windows 11 Sandbox 的普通用户通过 8 项复验，覆盖进程重开、同序号冲突、旧序号、并发占锁、新回执替换，以及错误签名和篡改文件保留原状态。

下载接线复用工作区的 `reqwest 0.13.4` 和 `tokio`，沿用现有 TLS 实现与版本锁。手写 HTTP/TLS 需要重复维护证书检查、分块读取、超时和重定向。下载器仍自行核对产品限定的 HTTPS 主机、响应和已签名资产。

`alcomd-updater download-product-update` 根据产品清单、当前频道和包格式定位 HTTPS 更新源。下载器限制响应大小与时间，逐跳验证目标主机，先核验声明签名和防重放序号，再读取资产。暂存需使用受保护的现有目录。文件完成校验后独占发布，同一更新流在持有回执锁时再次核验序号和文件。重复请求可复用已核验文件。

Windows 定向 Clippy 和 10 项 Updater 测试通过。独立 Windows 11 Sandbox 用仅在测试机内受信的证书和合成发布密钥，通过本机 HTTPS 完成 9 项实际调用，覆盖 204、签名下载、同摘要复用、过时序号、等长篡改、跨主机拒绝、截断下载、无更新时原回执保全和四进程并发。中断与拒绝场景未留下暂存文件。测试没有访问公开更新端点，平台安装提交和回滚继续按阶段 3 实施。

下载器同时独占发布原始签名响应，按完整响应的 SHA-256 命名，并在响应与资产均可重新校验后更新防重放回执。两个文件可交给现有 `verify-product-update` 离线复验。定向 Clippy 和 11 项 Updater 测试通过。Windows 11 Sandbox 的六项 HTTPS 与离线检查验证了原始字节、重复下载、仅剩完整资产时恢复声明、关闭测试服务器后复验，以及等长资产和声明被篡改时拒绝并保留原回执。安装替换事务继续按阶段 3 接线。

Windows 同范围更新先接当前用户。Inno 首装入口拒绝已有安装根，独立暂存入口只解出已核验的候选载荷。Bootstrap 复验签名声明、外层 Inno 摘要及解包后的运行清单摘要，再对用户数据执行只读健康核对。持久化提交意图后，按安装根的活动版本发布稳定 CLI、GUI 入口和卸载归属。旧版留到恢复条件明确。全用户流程由已安装的受保护 Bootstrap 请求提升。提升后的进程通过仅限本机的一次性管道接收有界的签名声明和安装包字节，并在 Program Files 下独立重验。程序切换由机器范围负责，每个账号在首次接入时核验自己的资料并切换登记。

### 2026-11-25 Windows 当前用户更新预检

`stage-installed-update` 在持有安装共享租约与当前用户 Core 实例锁时暂存并预检。已签名 Inno 把完整候选载荷发布到安装根的同卷兄弟目录。Bootstrap 核对版本、签名与载荷摘要，再用候选 Core 只读校验现行状态库与配置。重试重新验证原始声明、安装包和暂存内容。无效暂存保留供诊断。

断网 Windows 11 Sandbox 从 `4.0.0-alpha.0` 预检 `4.0.0-alpha.1`。源版本 Bootstrap 来自 `ca6811d8`，测试候选的 Core 与 Bootstrap 从 `11087fc3` 的隔离版本变体构建，其余组件复用 `72a85893` 的完整载荷。两个实包均用合成密钥验签。源版本 Inno SHA-256 为 `c289688cd6adc02228bedb111042f9e30f9d53c72ef09c75b1fe49617b4e2549`，候选为 `63a76821d0f89c99356332dff7f6cb2d8ce16659c31bba9ffa4c01d455c84596`。

八项检查覆盖安装维护锁、活跃 Core、错误文件名、内外摘要不匹配、同摘要暂存复用、损坏配置的只读拒绝、暂存篡改和原版本卸载。已核对原运行清单、数据库和登记摘要。后续验收继续覆盖正式提交、切换入口、故障恢复与全用户提升。

完整签名源码 `f75808a91486a1352f211d118168f8ddecdaa6d9` 的七个 Windows Release 程序打包为 Inno EXE `9f4094fdd6747a2e24dabd476e8cba46c9abf1e6c632741e258266f93a262be1`，运行清单为 `b44f71ca3a5079788228de19c145679e731b0fb541d8174d7afe7d3e3592f466`。用同一完整源版本包和上述 `alpha.1` 隔离候选重复预检。九项检查通过，新增在启动已验签的 Inno 后、发布暂存目录前终止 Bootstrap 及 Inno 进程树，核对旧运行清单、数据库和登记摘要后重试。源版本包还通过全用户范围的六项核心启动与卸载互斥复验。

用合成 Inno 夹具另行核对入口交接。沿用 AppId、范围和安装根升级时，Inno 6.5.4 追加到原卸载日志，PATH 中仍只有一条对应目录。原 CLI PATH 与开始菜单归属回执丢失，旧快捷方式仍指向旧版本。正式切换需要从事务快照恢复有效归属并发布候选入口，再核对卸载可精确清理新旧组件。夹具验证范围为 Inno 的登记与系统入口行为。

模拟入口事务在校验原快捷方式后替换目标，随后在写入新归属回执之前终止进程。重启从快照重新核对摘要并补齐 PATH 与快捷方式回执，PATH 保持一个对应目录。再次恢复时已被替换的同名快捷方式原样保留。三个检查在独立 Windows 11 Sandbox 通过，覆盖入口发布和归属回执之间的中断。

M12 并行的 `81e7a498` macOS Hosted 在 Chromium 重开 AI 客户端撤销确认时超时。关闭流程在收到 Material 的 `closed` 事件后卸载内容。现有用例在关闭凭据与撤销确认后等待对应内容卸载，再继续操作。原用例在 Windows 定向重复共 60 项通过，修订后共 80 项通过。等待点修正已签名提交为 `72e48e9f`，独立检出的前端检查、构建及 762 项 Chromium 均通过。[macOS 首轮](https://github.com/ALCOMD/ALCOMD/actions/runs/45601387153/job/139160906637)

### 2026-11-25 Windows 当前用户版本切换与恢复

从现行基线安装时，Inno 写入受保护的 `active-runtime-v1`。已安装的 Bootstrap 用 `apply-installed-update` 接收下载器保存的原始声明、已签名的安装包和防重放目录。Bootstrap 取得原运行时独占租约与当前用户的 Core 实例锁，核验旧版、稳定入口、目标路径、候选载荷和现有数据，再保留可独立复验的声明与安装包，并持久发布提交意图。Inno 只在相同 AppId、安装根和当前用户范围内提交，原子发布候选程序并恢复准确的 PATH 和开始菜单归属，完成后写入回执。

提交意图存在时，已安装 Core 拒绝启动。用已安装 Bootstrap 执行 `resume-installed-update` 可重新核验并继续同一事务。Bootstrap 复验 Inno 回执、安装载荷及稳定 CLI，按归属切换现有 Core 登记，最后发布活动版本并解除启动阻断。提交前的拒绝保留原活动版本，提交后的失败保留签名恢复输入。卸载前复验稳定入口和保留的版本载荷。

断网 Windows 11 Sandbox 使用 Inno 6.5.4 从 `4.0.0-alpha.0` 更新到 `4.0.0-alpha.1`。当前用户正常升级、无事务的安装和未知目标路径拒绝、原工程读取及升级后卸载共 6 项通过。另在候选运行目录已发布而 Inno 尚未完成时终止 Bootstrap 及安装进程树。恢复场景共 9 项通过，涵盖新旧 Core 阻断、篡改的保留安装包拒绝、删除原下载后重启恢复、占用登记时保留原选择器、新版 Core 重开原工程，以及卸载保留数据库、配置和测试工程。

两版夹具的 Core、Bootstrap、CLI 和安装器生成工具从 `f5932d1c6f65432de1eae9ca2714ec8783ff3b71` 及其版本派生构建，其余四个程序复用隔离的签名试验载荷。源版本 Inno 摘要为 `5270ecc807628395ebcf69cdd9e78f1ef6bc78022da7bdc0fcd62538a3ad3dbd`，候选为 `b47b939fbd80587a12eb3dee6439aedb0788a38e913d0b98653192e7434845a8`，候选运行清单为 `67171851a87e0861f34a3e2b1cc7b8c809ed07520ffefc0a36e6e2310ee62db1`。

从 `3a02f8692b19a180017a70f5e5e6ca272e0f52b6` 按同样方法构建两版并复验，10 项通过。新增在旧运行目录中改动已登记文件，验证卸载拒绝并保留文件和当前 Core 登记。恢复文件原字节后正常卸载。源版本 Inno 摘要为 `f1c8cce50ea8d8f56fdb644f5e63bc7180d16e014477a6c36d18848dc1bf8b7b`，候选为 `d76a92f2096dce26ee54018f27d70c7b26d489c71be6785970b2768a94202839`，候选运行清单为 `002c632a054348ecc9354bef642e18cbec332a5d0a4836d5218eaaa5d7349f8d`。

该源版本包还在独立 Sandbox 通过全用户安装六项复验，覆盖两个普通账号分别连接 Core、显式维护锁阻止直接和目录联接冷启动、任一账号连接时拒绝提升卸载，以及卸载后保留两份账号状态和登记。

从干净签名源码 `b95e132fefdc113e48f835f5388dd01610861949` 构建七个 Release 组件并打包完整 Inno 源版本。其安装包摘要为 `d7c428c70d53b53bc13831477ea874a1113d5c72a058a0ad65325f14418de113`，运行清单摘要为 `973ed482dc4d97658d37591aadc40fc803647a60ec5afe050eefa04f855170d0`。用该包作为源版本、此前 `3a02f869` 派生的 `alpha.1` 夹具作为更新目标，再次通过 10 项实包恢复检查，包括提交意图后的实际进程树终止。该完整源版本也通过六项全用户安装及卸载互斥回归。

从该源提交派生只改七个版本元数据文件的签名检出 `6a79c90efc95738ca9ab4851033a7068d23554c8`，重新构建七个 `alpha.1` Release 组件。升级安装包摘要为 `a4922ecd6d48805475357cd474ab051f3e726443570a733ce59744279f6a8bf1`，运行清单摘要为 `545c375c31f7f2d0853797610032430ffde993cf67548f505d23f3aae132674e`。两套完整 Release 实包通过 8 项正常升级和 11 项中断恢复检查。两次验收都由更新后的开始菜单快捷方式启动真实 WebView2 窗口并连接新版 Core。

通过应用界面发起更新和其余故障场景继续按阶段 3 验收。

`b95e132f` 的干净检出通过本机整仓与 762 项 Chromium。逐笔验签后普通快进主仓。同一 SHA 的 Windows Server 2025、Ubuntu 22.04 和 macOS 15 arm64 Hosted 作业均通过整仓、外部 SDK、Release、Tauri 和 Chromium 门禁。Actions、JavaScript／TypeScript、Python 和 Rust 的 CodeQL 均通过。[Hosted CI](https://github.com/ALCOMD/ALCOMD/actions/runs/45641567398) · [CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/45641566264)

### 2026-11-26 Windows 全用户版本切换与中断恢复

全用户更新由已安装的 Bootstrap 启动受保护的提升进程。普通请求进程创建一次性本机上传管道，提升进程以仅识别级别打开读取。双方核对对端进程 ID，只传输签名声明以及声明长度的安装包。提升进程在 Program Files 下验签和暂存。提交期间以旧版清单租约和机器提交意图阻止各账号启动 Core。新版活动选择器生效后，已初始化的账号可从原生入口或外部 SDK 接入，在本账号成功打开资料后原子更新登记。

断网 Windows 11 Sandbox 对签名源码 d1555aa3 的合成签名 Inno 夹具完成六项正常升级和八项中断恢复检查。两版重新构建了 Core、Bootstrap 和 CLI，其余四个运行组件复用已有夹具。源安装包 SHA-256 为 3201d8b61a32a9940d16ce9c059f11187442b548e02d3a298430208e88583441，更新安装包 SHA-256 为 d438f4886e5ad05e0ac15af357bd925bd58dafe35cb4bd40cb5632d63a8392f0。

一个账号的外部 SDK 保持 Core 在线时，升级在持久提交前拒绝。释放占用后，升级切换共享版本、HKLM 卸载登记、机器 PATH 和开始菜单，保留两份原账号登记和资料。中断场景在新版文件已落盘、完成回执仍缺失时终止提升 Bootstrap 与 Inno 进程树。两个账号均拒绝启动两版 Core，篡改保留安装包时拒绝恢复。还原保留包并删除原始下载后，新版 Bootstrap 可恢复同一事务。两组验收均由稳定 CLI 和仓库外 Rust SDK 分别接回两个普通账号，读取原 Unity 工程，并核对升级后卸载保全两份账号资料。

断网 Sandbox 复验了普通账号创建上传管道，由不同 SID 的已提升进程接收。修复源码 `20a9d8f7` 的 Bootstrap 与优化运行组件配合，通过活跃 Core 互斥、跨账号上传、共享激活、逐账号接入和卸载保全六项检查。同一源包另通过八项真实进程终止和中断恢复检查。两轮升级后的 CLI 与独立 SDK 均在一次连接内就绪。源安装包 SHA-256 为 `fb459a7c924742cf8390e741c6c5ae0c168c130c5f7ce146f63f3eadb1c531b2`。

混合夹具的部分未初始化账号首次冷启动超过五秒，重试可以接通。标准账号交互输入另一管理员凭据、从干净 Release 复验未初始化账号的五秒冷启动及同一候选的 Hosted 门禁分别记录。

### 2026-11-26 应用内产品更新

Windows Inno 和 Linux AppImage 的「关于」页按当前构建所属频道检查获批准的 HTTPS feed。检查只取并验证签名声明。用户选择下载后，Updater 读取同一声明指定的资产，提供取消并再次核对摘要。用于安装和恢复的文件路径保留在原生进程。选定安装后，GUI 退出，Bootstrap 等待本账号 Core 空闲，再执行平台事务并重开桌面。仍在使用的其他 Core 继续按安装范围检查。DEB 由包管理器更新，macOS 原生替换延期。

GUI 复用 AGPL-3.0-only 的工作区 Updater。调用独立程序解析文本输出会重复维护进程和结果合同。现有 Updater 提供签名、下载、时限和防重放，桌面适配器只管理用户确认和进程生命周期。前端类型、Material 控件、三语言文案、Windows 定向 Clippy 和三项 Chromium 用例已通过。

用 `f9bf2431` 构建 GUI 与 Bootstrap，并与已有测试组件组成签名源包。源安装包和清单 SHA-256 分别为 `5d6228683679d052afa6d64811fb8978dcbd9eeee815cde697fce5f4f1bc7325` 和 `b3a4c0053cb5da5673842901a88551fe102d0f50757c4e1e1d4e61fe54542fa2`。目标使用已签名的 `6a79c90e` Release 夹具。断网 Windows Sandbox 通过本机 HTTPS 提供合成签名声明和安装包，并用 Windows UI Automation 操作真实 WebView2。八项检查覆盖安装、仅检查签名声明、下载期间保留原版、取消确认、自动安装并重启，以及原工程和 Core 登记保全。实际 GUI 导入的 `vcruntime140_1.dll` 已纳入生产打包固定摘要。Updater 11 项与 xtask 5 项定向验证通过。

### 2026-11-26 Ubuntu 产品 AppImage 整包更新

Bootstrap 在持有本账号 Core 启动锁时，重新核对原 AppImage 文件、已登记运行时、签名声明、完整候选和其中的运行清单。候选经只读数据预检后，在原 AppImage 同卷完成暂存并持久化提交意图。原文件名由原子交换交给完整新版，旧文件保留。同一安装的 Core 登记随后切换到经签名校验的运行时。断电或进程终止后的恢复按原文件身份和签名摘要判断是否已经交换，在原位置重新打开 AppImage 可恢复保留的事务。DEB 登记和未知目标继续受原归属检查保护。

Ubuntu 22.04 隔离样本用合成密钥，复用已验证的 Linux 运行组件并重新构建两版 Bootstrap。外层 AppImage SHA-256 分别为 `4c87dccf70217615ce3b9281e16f954fae3c6f86157bc3da585973fb64be5dbe` 和 `7cee280b480a9c22b16d6a74e5819fe1d86455dc2d61c78c9012bce1387debe9`。对应的运行清单为 `3d6a204edc4282bffa12d3c81080aea3ada78076ef41b1a41acee1905202ea68` 和 `05f66007008fc613280f0d3bcfdcc828c31918a778d130cc279086b064092d84`。

十项原生检查覆盖占用目录保全、正常替换、文件名与执行模式保全、保留并核验原 AppImage、交换后登记前的实际 SIGKILL、篡改拒绝、原下载删除后的恢复、状态库和项目摘要对账、真实 FUSE 与 WebKit 和新版 CLI 读回原工程。Jammy 定向 Clippy 与 68 项 Rust 测试通过。部分混合夹具的首次连接超出五秒，单次冷启动继续由干净 Release 候选核验。

### 2026-11-26 AppImage 可选 CLI 集成

「关于」页提供显式启用、修复和移除。用户入口指向受保护的私有启动器，由该启动器从本账号 Core 登记解析并验签当前 AppImage 运行时。应用移动后从新位置重开可重新绑定，原命令也能跟随整包更新。发布入口以前检查私有目录和现有路径归属，保留第三方同名文件与 DEB 链接。

Ubuntu 22.04 隔离环境的签名 AppImage 夹具通过八项原生 FUSE 和 IPC 检查，覆盖首启、显式启用及幂等、缺失后修复、移动拒绝与重新绑定、升级、移除后资料保全和 DEB 路径拒绝。两版源包 SHA-256 分别为 `1f06e1335df73bb74b2dc427118214951ea92f1772772dc0d9b1118eb0a8c370` 与 `ff159d782d542d5feb21c34d5405f388f5ad71599d8167aaad6ae0cd6be57372`。定向 61 项 Rust 与 Clippy、前端检查通过。夹具重建 Bootstrap 并复用现有 GUI，新增「关于」页操作随完整候选另验。

### 2026-11-26 Ubuntu 完整 AppImage 与 DEB 原生验收

隔离 Ubuntu 22.04 x86_64 从干净、已验签的 `e0fc95dcc2a8140953b7eeee78842bc5262bbb3c` 和只改变版本元数据的子提交 `1708884872e80428caedacda5438cdf593b3ee5c` 构建全部七个 Release 程序。两版最高均为 `GLIBC_2.34`。用隔离验收密钥签名运行载荷，外部 Rust SDK 从签名源码构建。四个实包由同一流程验证并对账。

| 版本 | AppImage SHA-256 | DEB SHA-256 | 运行清单 SHA-256 |
| --- | --- | --- | --- |
| alpha.0 | `05258ad360dc9928ca6cf5f7ee866d4f0f93ab2328acf7dbae7e7332c1b09b9f` | `e28fff87aaf88610b00a00ab3aa6439f29aaf35ac2cf9535bbbd14aae870eb77` | `2ea90dd15f97ae8c6c06ca1742c1083f7362c5c7dabc2092a829b3c97e4d1057` |
| alpha.1 | `3170ead71c757d31f0ff71f5c27f971a0b708b515fb4993a0eb88de15d12447a` | `5a5ead15ea5b1399246619ebe67fd8783053e3f7b3773d033cd3719f6804d218` | `86f2a1b3d4ea6fb2df3d3a588428670025066d8f2d2a9ed6ab7f0adb45d4ce61` |

AppImage 用原生 FUSE 与实际 WebKit 窗口操作「关于」。外部 Rust SDK 从已登记安装仅调用一次，817 毫秒冷启动。检查、下载和取消对应回环 HTTPS 的独立声明与资产请求。确认后，实际 AppImage 替换为目标摘要并重启，CLI 入口身份保持，Unicode 路径的 Unity 工程和内容可读。用界面完成启用、移位重新绑定、修复和移除。首次驱动在确认对话框的可访问性节点定位失效，修订定位后整套确认通过。原生确认结果摘要为 `ceaa94ba146a625771dafba2964bdbb7e5c16b802093887c159842eb5f45a3dc`。进程终止恢复沿用上文独立签名夹具证据。

DEB 在隔离网络内用回环 APT 仓库验证真实首装、`dpkg --unpack` 中断后配置恢复、坏签名与失效登记拒绝，以及两个普通账号分别用 CLI 和 WebKit 注册新版。外部 Rust SDK 的首次调用分别为 870 和 1076 毫秒。重装、同数据版本 APT 回退、remove 和 purge 后，两个账号的登记、数据库、设置及工程文件摘要一致。结果摘要为 `c42f8a06f03e8c40601acd9655308d0c75eeb729e73da04a75d6191c10365a57`。保留实包、摘要和验收日志，清理约 5.9 GiB 独立编译暂存。冻结 v3.4.0 到完整包的交接仍属阶段 4。

### 2026-11-26 Windows 桥接程序双范围暂存

从干净、已验签的源码 `120578976e5a667d47fb1b9e29e1b9a78aff8088` 构建完整 Windows Release。Inno 6.5.4 生成在线前置组件版，EXE 为 33,775,004 字节，SHA-256 为 `a1aa35f92e1fb028b31302dbbf98e35065e26fac4c596ede2350b0bf59b363f7`。产品清单 SHA-256 为 `bd60400b5b14d02a7d1e4cf8aaa37998a8c0b307d80942f05f16df044c846812`。同一签名源码独立构建 v3 迁移器，桥接清单逐文件核对普通产品签名载荷，并只增加临时迁移器。实际 Updater 使用隔离合成公钥通过 beta 桥接验签，桥接清单 SHA-256 为 `0230b4f1cbda6de41edc4a4810a86242591f0611f3825a5b54922cd916a4f0eb`。

断网 Win11 Sandbox 使用完整 Inno EXE，当前用户和全用户分别通过四项检查。错摘要和混用私有模式被拒绝，未知安装目录逐字节保留，重复暂存保持已验签文件不变，缺少原账号已提交的迁移日志无法启用产品。逐文件核对同卷暂存与签名产品清单，确认没有正常安装根、卸载登记或活动选择器。结果 SHA-256 为 `59e3963327493f13ed9c7db0f8d206902695d043a5c099e146c0b833f94d0c70`。

`39bd104f` 随后让 Bootstrap 验证两份签名载荷的完整配对，并拒绝额外文件、缺少读取器和摘要差异。Updater 12 项测试、Updater 与 Bootstrap 定向 Clippy 及格式检查通过。上述原生暂存结果对应 `12057897`。冻结更新器启动完整外层桥接、提交后原生激活和双账号交互 UAC 继续验收。

两种冻结版交接驱动的隔离输入已通过签名 Git 提交、冻结安装器与产品配对检查。首次进入当前用户预验时，Windows Sandbox 的默认登录账号处于高完整性，且 `EnableLUA=0`。驱动按约定在写入前停止。原账号未提升运行和不同管理员账号的 UAC 尚未取证。

#### 安装程序中断恢复

`9570fdaa` 在复制前持久化绑定产品清单和原迁移日志的启用意图。签名运行目录同卷原子发布，在 Inno 的卸载登记同步并写完回执后才开放 CLI、开始菜单和活动版本。已有回执可恢复 Shell 入口发布，并保留既有原生卸载器与日志。普通安装目录中已有的无关文件保留。卸载脚本收录桥接预先部署的准确运行文件和 CLI 文件。[Inno 6.5.4 安装与卸载日志实现](https://github.com/jrsoftware/issrc/blob/is-6_5_4/Projects/Src/Setup.Install.pas)

断网 Win11 Sandbox 用合成 Bootstrap 证明标记和真实 Inno EXE 运行故障注入。两种范围分别实际终止 Inno 进程树两次，覆盖程序及稳定 CLI 已部署、卸载登记已保存的两个持久点。原日志重试通过，换另一份日志被拒。恢复前没有活动版本，恢复后两个原生卸载文件摘要保持不变，CLI PATH 和开始菜单各发布一次。正常卸载移除预先部署的程序并保留无关文件。全用户首次复验被目录枚举的父目录伪项拦下，跳过目录枚举中的点项并保留原路径检查后通过。共 8 项，夹具 SHA-256 为 `69e735f130758968902d9929fee789ae6fe9c31b522c090c7fe9f5d2dc73d9b6`，结果 SHA-256 为 `6559c7e742bc998a743830871200656194047174c595b823887af4034594565b`。分发定向测试 6 项及 Clippy 通过。完整签名候选和冻结更新器的原生交接仍待复验。

从干净签名源码 `9570fdaaa3ec8b67e67a49b56f0b506a85c4d1ad` 构建并核验了完整 Inno 和同源迁移载荷，采用隔离合成签名。Inno 为 33,775,645 字节，SHA-256 为 `eeb675a97594b31126672223d0ee816d62a058dbc08cff75a0ba82161664d70c`。产品清单为 `ae5cb6b2b5b853eec8b1ef2c5deb04949b71019b092bad273c8d92746c580c17`，beta 迁移清单为 `b56bf501e55fb02ca06b94444008ea8269b87c4f74ce16cdc3d49840795662ff`。该实包在第二套断网 Win11 Sandbox 重跑双范围暂存，8 项均通过，结果为 `3ca0c6652c816b86d00afe0b1f791564ace2643e0faf04953b410d53f03b1234`。两个持久点的终止与恢复证据来自上文注入夹具，原账号实际状态提交和启用待冻结版交接。

#### 冻结 Windows 更新器入口与逐账号首启

`769b1585` 接入冻结 v3.4.0 更新器启动的独立 EXE。它仅接受冻结安装参数和明确安装范围，核验旧安装的登记、实际程序摘要与配对卸载入口，并由原账号确认资料范围。Inno 部署程序，Bootstrap 处理原账号资料。旧程序、登记和资料继续保留，已验收的精确清理须另在交接完成后执行。外层从同一源码签名候选的 Inno、迁移载荷和两种范围的签名健康证明组装，出包时复验配对、完整摘要及冻结更新器的 256 MiB 上限。任一范围缺健康输入都拒绝出包。原生 PE 链接确认 `asInvoker` 且 `uiAccess=false`。出包 7 项、外层 2 项单测和定向 Clippy 通过。

`d202e7db` 在机器桥接激活前持久化受保护的账号门禁。普通 Core 持有单实例锁后先核对账号状态，迁移已提交的同账号资料可进入正常检查。无现行状态库时，必须经该账号明确审批并持有与机器记录身份配对的就绪回执。该回执不能用于另一个安装实例。Win11 真实 Inno 注入夹具复验两种范围及机器门禁的发布时序和卸载，共 9 项通过，结果 SHA-256 为 `d4eec8f8261b7383147516343769bb0e860db2bc410d240338e21cfd1741103a`。平台定向测试拒绝未批准空资料和其他安装实例的回执。

`b605e19b` 接通首位全用户账号无旧资料的情况。原账号明确确认后持久保存绑定程序清单与账号 SID 的证明，惰性签名 Bootstrap 核验该证明及资料目录缺失。全用户程序启用后只初始化已确认的账号。其他账号的首次启动仍受门禁保护。定向测试覆盖有效证明、不同安装范围、不同候选、已有资料和冻结 CLI 参数。Win10、Win11 的原账号交互 UAC 及冻结版从旧频道下载实际外层 EXE 的完整交接还需取证。

同一 `b605e19b` 干净签名源码编出完整 Windows Release Inno 和配套临时迁移载荷，使用合成签名信任。断网 Win11 Build 26100 使用实际标准账号安装并启动冻结 v3.4.0。该账号调用原生迁移阶段，验证 v4 程序仍处于惰性暂存，并在字节相同的迁移副本上完成项目登记、读取、修改及取消登记。以同一迁移计划回滚后，v3 源资料摘要一致，v4 资料与活动程序均未发布，冻结桌面再次打开。观测到重新打开后，只停止该隔离进程。本轮 Inno SHA-256 为 `70f7ec6be0b2dce514228324a7bfca9c933a7b00d55b4410e3a7cf08c6350e6c`，迁移清单为 `6f51551f44e1f00ef85f92205f87f3faccbd70fe262b2faa87a4fd840181f1ce`，原生回执为 `f7f1ff533d12deb319724984c590dbf895d87df83ac7689016681d32d399cdeb`。

同包的全用户正常安装保持部署管理员的资料未初始化。由另一标准账号实测受保护的机器桥接标记，Core 以待迁移状态拒绝登记，未创建资料目录和 Core 登记。撤下测试标记后，使用同一账号继续运行健康检查。测试脚本修正了 .NET Framework 标准输入写入的 UTF-8 前导标记。复验后两个安装范围均通过真实 WebView2 窗口、Core RPC、稳定 CLI、一次性凭据 MCP 项目读取和撤销、以及签名第一方扩展发现。当前用户和全用户原生回执 SHA-256 分别为 `084513ce429e38be7cbb5c6ada653b928ef0aeec1c0f5947ddba310771064021` 与 `044c072f51528050f86cde0ee040ab8b35a6401daa5cd0c45ff37eabf3758f89`。这两份回执支持隔离合成健康签名。冻结更新器下载及旧钥验签、原账号交互凭据提升另验。

使用同一干净签名检出和双范围原生回执，生成合成健康签名并通过 `cargo xtask dist --package-windows-bridge` 组装 beta 外层。独立检查冻结更新器资产名 `ALCOMD3_4.0.0-alpha.0_windows_x86_64_setup.exe`、PE 与大小门禁。资产为 182057984 字节，小于被冻结的 268435456 字节上限，SHA-256 为 `772a6baf178f76f39cb469277ee0f5551bd8d5adfd9dffe2272b84173a794b63`。

在独立 Win11 标准账号中以冻结的 `/SP- /SILENT /NOICONS /CURRENTUSER` 参数直接运行该外层。首次确认时选择取消，程序与原生数据均未进入暂存，v3 登记和资料原样保留，旧桌面可重开。另一已授权合成账号在核对暂存计划与签名程序后确认提交，拒绝启用 Discord。核对活动选择器、原生提交日志、12 个已激活载荷文件及稳定 CLI 的实际 Core 项目读取，并验证 v3 程序、登记和资料一致且可重开。取消和提交回执 SHA-256 分别为 `54d1d90399055c661f3fda5b2ea0cd3fe0a084f89583db64fd6b33493d97bd5f` 和 `ea0cd6d18612f26663f92a4c6fae20dbe91158b8df1fc5d36dec8551b02afc9b`。

按批准范围通过原 v3 钥为已预验、摘要固定的外层生成离线分离签名。以冻结 Git 对象内的公钥独立通过 Minisign 验证，签名绑定原资产名及 `purpose:release`，签前签后外层摘要一致。分离签名 SHA-256 为 `437ef5f664f93ec8de3671d2eca36a6fca83777f1fcec09572bf5bf9a0775a4a`。

同一外层在两套 Win11 Build 26100 Sandbox 通过冻结 v3.4.0 桌面更新器的当前用户验收。只在测试客户机内使用回环 HTTPS 模拟旧入口与新服务。实际 stable 请求经同频道 307 得到 204。beta 经同频道 307 取得声明并下载完整的 182057984 字节。正向验证下载和被启动的外层 SHA-256 均为 `772a6baf178f76f39cb469277ee0f5551bd8d5adfd9dffe2272b84173a794b63`。更新器保留四个冻结参数并消费待安装回执。测试在外层自己的原生确认窗选择取消，外层以 0 退出，v3 源资料、程序与登记保全，旧桌面可重开。回执 SHA-256 为 `349aa688d714fea0236333bc7a622f9f09adc46fed2ef4306a57ce37cfefd0f0`。

负向用原钥曾签发的同名历史资产签名，先独立验证签名与本轮字节不匹配，再由冻结更新器下载本轮完整字节。更新器记录失败，无待安装包与外层启动，旧版安装保持有效且桌面可重开。回执 SHA-256 为 `787b211c8e5f2e24c83b8260f2d66d15f4515727c48f1b332de740f0fb17ab76`。当前用户原生提交由前述独立外层测试覆盖。

同一签名外层也通过全用户版本的冻结更新器验收。Win11 隔离管理员安装冻结 v3.4.0，另一个中完整性的标准账号从机器登记打开 v3。真实更新器取得相同包摘要，自然传递 `/SP- /SILENT /NOICONS /ALLUSERS`，外层留在发起标准账号的 SID 下。取消发生在机器安装和原生资料暂存之前，旧机器登记、程序及发起账号资料保持一致，v3 桌面可重开。回执 SHA-256 为 `c1fda1dba4443f5c78a3435f8a398442d340fed552fe9e6b2e4cfc3044c0f3d1`。该 Sandbox 观测到 `EnableLUA=0`，Win10、Win11 的交互凭据提升及全用户提交仍待验收。

### 2026-11-26 Linux 冻结版桥接入口

`cd37b2ce` 接通普通账号的 AppImage 与 DEB 桥接。AppImage 使用独立路径运行已验签的新镜像，旧镜像保留。DEB 继续由 APT 或 dpkg 部署，并核对已发布旧包、包管理器登记及恢复资产。两种方式复用持久迁移日志，用户确认期间普通启动拒绝初始化空 v4 数据。Bootstrap 12 项、分发 7 项和定向 Clippy 通过。Bootstrap 有 1 项既有忽略用例。定向日志 SHA-256 为 `239b2de2ecec4e32616c619f0e1fcab5c131f6fcb32a11a3132fb01fb1088510`。

隔离 Ubuntu 22.04 用合成信任和混合夹具验证实际 FUSE。外部 Updater 先验新 AppImage 完整文件，再运行原生桥接。待确认期间没有安装运行目录、启动桌面或创建正式状态。普通启动和坏健康签名均拒绝。提交后打开真实 WebKit 桌面并通过 CLI 接入 Core。冻结版公开 AppImage、v4 测试镜像与复制的旧设置及 LiteDB 摘要不变。原生日志 SHA-256 为 `9d7099c27d5416f27f70f719302f06f57bc5026d9c6c83c2ba264cba49b4cca6`。冻结 DEB 已在独立 dpkg 根核对 `alcomd3`、实际入口和 `3.4.0-1`，结果 SHA-256 为 `db0f50a7370c6c7881aee4e8a80420be6beb3f1ca7b82d48138976e5d5eb75c1`。完整 Ubuntu 24.04 旧桌面及 DEB 交接继续验收。

`0ec3ed1c` 接通独立 AppImage 桥接归档。断网 Ubuntu 22.04 用合成信任独立验证归档分离签名后解包。直接确认、篡改产品签名及篡改整包镜像均在执行前拒绝。移除本地更新声明后，仍可离线暂存并显式确认。只有确认后才发布到选择的位置并进入提交阶段。实际内核 FUSE 挂载、WebKit 窗口和 CLI 接入同时通过。另一隔离资料目录在提交前撤回暂存，未发布新的镜像和 v4 资料。冻结镜像以及三份旧资料文件通过原摘要核对。

定向测试 33 项通过，1 项沿用既有忽略。Clippy 和组装命令参数检查通过。定向日志 SHA-256 为 `4f9e1e9918cc5a20adbbd69789f7bb8a612ae89be2ff627294a499a6ba3abde6`，否定用例与完整原生提交及回滚日志分别为 `92033d62f4c9ffe6bc2ab20817ed98e08681832b0c2814df5de19cb3d7cc1e5b` 和 `f4b189c17f81a0ff147f28bcea30cbc4f5de12e3e094149b69bdbcda47905bfa`。原生夹具包含本轮构建的独立启动器和 Bootstrap，并复用此前构建的其余运行组件。`cargo xtask dist --package-appimage-bridge` 从干净签名候选组装的完整交付资产继续复验。

专用 Ubuntu 24.04 x86_64 测试来宾通过 APT 安装真实 `alcomd3 3.4.0-1`，核验 `/usr/bin/alcomd3` 和桌面条目的 dpkg 归属及冻结二进制摘要。在最小系统中补齐系统 CA 后，断网普通账号两次打开真实 v3 WebKit 窗口。旧资料由冻结版实际生成，LiteDB 在重开前后摘要一致。日志 SHA-256 为 `ac89855bf5028d85405bee83a2e33b537df9820f5d03ce4831c55df6932b21b7`。

从干净签名源码 `310065bfe1b37037f30ad5d7c5d28d4526cb385c` 在 Ubuntu 22.04 构建七个 Release 组件、AppImage、DEB 和配套迁移载荷。最高符号要求为 `GLIBC_2.34`。AppImage 与 DEB 的 SHA-256 分别为 `9eed4fdbc6d9cd6e3493e7c85a9c3fb26667cca27fc6ff4f4cdbb215daa0fba5` 和 `f95f81736024130c783272f2194e9c1eb44dea973b470c333f0064c8762dcc15`。两种格式在隔离标准账号通过真实 Core IPC、CLI 工程读写、WebKit、MCP 认证与撤销、以及 Core 验签的第一方扩展发现。AppImage 实际经 FUSE 挂载。相应回执 SHA-256 为 `e89a7b006cd636b9b6c9768823b4e8044766669af982a0613998f64cbb04dd58` 和 `9d48fd85bfc3b81ddf13f57cb12a4bd4c7ed70a0559a6998d7def13bd85746ba`。按两份回执生成并独立核验隔离健康证明，总回执 SHA-256 为 `109b6076a1358dcb23046b3665811b9715ac6d185562e01e06a9c3d6b0951e7b`。

在隔离 Ubuntu 24.04 用 APT 安装同一 v4 DEB。两个标准账号分别启动冻结版生成真实 v3 资料，再通过原生 Bootstrap 停在待确认点。普通首启被阻断。第一个账号回滚并重开 v3 桌面，第二个账号先验证坏健康签名拒绝，再提交并用 GUI、Core 读回迁入的合成设置。两份旧资料、旧程序和安装归档通过摘要核对。清理只移除测试账号、资料及 v4 测试包，保留已配置的冻结 v3 包。交接回执 SHA-256 为 `61b2c1682f61c2031002ada5325c430a07cc33eff95c8e674c65292bf76fc12a`。

同一 `310065bf` 的独立 AppImage 桥接归档经正式组装入口生成，并用合成密钥签名。归档及分离签名的 SHA-256 分别为 `c33e349366875c818b096fe47c94edf1c743fdf90c2a154717750baeb6fcd33f` 和 `ad8a6944db9542e9501dc74dcf74bb5cfb9c792142c44b3d01d293a26cb7ce38`。在 Ubuntu 24.04 断网标准账号下，先用固定公钥验证归档再解包。两个独立资料目录分别由实际冻结 v3.4.0 的 FUSE/WebKit 桌面生成。回滚用例重开冻结桌面，提交用例显式确认后启动同源 v4 FUSE/WebKit 并通过实际 IPC 核对 Core 版本。两种操作均复核冻结镜像和原资料摘要。复验在最小系统补齐 `/etc/mtab` 后验证自有测试挂载与资料均已移除，回执 SHA-256 为 `c69cac1cd07feb97edf15a035410fa9fd7648383d2aa81c22796c9f130cc5f88`。

### 2026-11-27 Linux 发行 ELF 扫描

在隔离 Ubuntu 22.04 核对 `310065bf` 的 AppImage 与 DEB 摘要后解包。检查 AppImage 外层加载器及两个包内所有普通文件的 ELF 标识和 x86_64 机器类型。AppImage 为加载器和七个运行组件，共 8 个。DEB 为七个运行组件和 `/usr/bin/alcomd-cli`，也为 8 个。两包最高所需 glibc 符号均为 `GLIBC_2.34`，各自对应上一节已运行 Ubuntu 24.04 的完整资产。以 Ubuntu 24.04 的 Bash ELF 做负样本，`scripts/check-glibc.sh` 对 `GLIBC_2.38` 返回失败。逐文件扫描日志 SHA-256 为 `f27651c4ea68961e1f6bbf50f7f1ee906b83375df54e9ba4bf8b2bd175e223ed`，负样本日志为 `7ce45d9704888fbcc060df8311c40c6cd3b89afb9771f97c3f39da6a0d4d1772`。

### 2026-11-27 Linux Orca 读屏冒烟

隔离 Ubuntu 22.04 中的 Orca 42 和 Speech Dispatcher 运行在独立 X11 与 D-Bus 会话。启动 SHA-256 为 `9eed4fdbc6d9cd6e3493e7c85a9c3fb26667cca27fc6ff4f4cdbb215daa0fba5` 的签名 AppImage，打开真实 WebKit 窗口。原生 Tab 键使项目搜索、列表视图和创建项目依次获得焦点。Orca 实际朗读了三个控件名称，并在隔离虚拟声卡捕获了 179,432 个单声道合成音频样本。总日志和键盘阶段的 Orca 日志摘要分别为 `60d0faf20aaa3c5506ef47827eff8b5347331c419c7e3a9fba8400a8527ecfba`、`a3d9644e1887c6688970f72ac79c5d7f435a3b866b4f36fec7c5f8ea984e80f1`。Windows Narrator、macOS VoiceOver 以及最终发行摘要对应的复验仍按发行矩阵执行。[Orca 使用说明](https://help.gnome.org/orca/introduction.html)

### 2026-11-27 Linux 同源工程包复验

复用隔离 Ubuntu 22.04 已有构建缓存，从干净签名源码 `2a1f93b978a2180f87d69a4bc54f1f9e52317491` 生成 AppImage、DEB 及配套迁移载荷。AppImage SHA-256 为 `03d74d4f974d82fdc35818c17edb5f477fe3867690d15543b4d4d569ff5f0686`，DEB 为 `131012075fc3f1a2b9f31f4a1036fdeb363b492d96f8e41d3924582ed0d7dc38`。两包逐一检查 8 个 ELF，最高要求均为 `GLIBC_2.34`，绑定两个包摘要的扫描回执为 `6b755d9bc204282ab9ae11a6c8659c267b5d527d3fe32683b6e83f70ba627ede`。

Ubuntu 22.04 回环隔离环境的两个标准账号通过 Core、CLI、WebKit、MCP 认证与撤销和签名第一方扩展，AppImage 使用真实 FUSE。两个作用域的健康证明用合成密钥固定，复核汇总回执为 `7126b33a30bd8bd89a56008d2367478c52d45f8d2b69df05ad6ffd0b859ab33a`。同摘要 AppImage 的 Orca 复验读取三个真实控件并捕获 179,417 个合成音频样本，回执为 `c94dbc2e9ae8c84ae92951f95e1b1e2cbb9933ed34ce3cc3ad25f5da71d93b53`。

Ubuntu 24.04 回环隔离环境用真实冻结 v3.4.0 分别复验两种完整工程包。DEB 经离线 APT 交接，验证待确认锁、回滚重开和签名提交。AppImage 经分离验签的独立归档交接，归档摘要为 `1cfa57bab8a6e82873b176de009bf92aed1972afe437463573a158d3e00f9c6d`，验证实际 FUSE 上的旧版重开与新版启动。两种方式均核对旧资料、原程序和临时资源清理。DEB 与 AppImage 的回执分别为 `848f68b355fc03a6a35f547507a2cd2d8f99354a2915a742163e532e9d773329`、`f21c27621d732b04d09b8fb1e0418edf412759bbd1f756c335ff8c481edea49e`。

### 2026-11-27 Ubuntu 原生 GTK 缩放复验

在隔离 Ubuntu 22.04 的 X11 和 D-Bus 会话中，使用同源签名候选 `2a1f93b9` 的 AppImage 和 DEB，按 GTK 程序窗口缩放分别启动 100% 和 200% 两次。AppImage 实际通过 FUSE 挂载，DEB 实际经 `dpkg` 安装并登记 Core。两包分别核对文件及运行清单摘要。两种格式的真实 WebKit 窗口从 `1400 × 800` 到 `2800 × 1600` 像素，AT-SPI 观察到原生 Tab 依次聚焦项目搜索、列表视图和新建项目。200% 截图可见三个控件及焦点环。夹具移除本轮 AppImage 挂载并卸载测试 DEB。AppImage 与 DEB 原始回执 SHA-256 依次为 `077e12a0d450dbf8688dd76c313c802ca1bc2da37e99401eaf5674fcf9fea813`、`11cef2bfcff777e136b217a4fe0fcf44367ff7155024f1b94da0baae7a7a8689`。此轮覆盖启动时的 GTK 整数缩放，跨显示器切换和最终发行包留在发行矩阵中。[GTK 3 X11 缩放](https://docs.gtk.org/gtk3/x11.html)

### 2026-11-27 Windows 高对比度与讲述人复验

断网 Windows 11 24H2（`26100.9974`）来宾安装签名工程候选 `2a1f93b9`。Inno 安装器 SHA-256 为 `e8dbbe0e44d8c10ebed68acee7f677700b825bf11d5ccd66d6e96e6e668ee425`，已装运行时清单为 `2c65dae115f1667122700ec2d4e944336f6976a11e844b907c103398f66dc90b`。系统高对比设置切换后，同一个运行中的 WebView2 窗口底色从 `#FFFFFF` 切换到 `#202020`，关闭高对比后恢复原配色。项目搜索、列表视图和新建项目在切换后依次可用 Tab 聚焦。通过回执和高对比截图的 SHA-256 分别为 `7e5ca3a554efb4d9d776b10af79995673af0f902cac3bab816d24b21138a54ea`、`534d6c3b29bc9e6ea4c11b6149abf94c35f3484a0b778d91acd1b7804723df25`。[Windows 高对比度接口](https://learn.microsoft.com/en-us/windows/win32/winauto/high-contrast-parameter)

原生讲述人 `10.0.26100.7015` 在该来宾启动，三个控件可通过 Tab 聚焦，讲述人焦点框可见。设置显示当前语音为 Microsoft Huihui Desktop。项目控件和标准 Windows Forms 对照控件均未在语音回顾中产生可核对文字，复制最新短语也未取得文本。回执 SHA-256 为 `1f4d78d820421a56ca5201ddd4f64cb4977e008f2ab7fd96e04ff2b85420fff4`。Windows 原生朗读仍待通过，最终发行包须按 A-017 复验。macOS VoiceOver 继续延期。[讲述人语音回顾说明](https://support.microsoft.com/en-us/accessibility/windows/narrator/complete-guide-to-narrator)

同一工程安装包的 WebView2 在来宾初始为 96 DPI。直接打开显示设置和从系统设置导航两次尝试均未取得可用的缩放控件，125% 原生缩放未执行。第二次回执 SHA-256 为 `d07409b0b9c05940430aa6719612d906fa3f18d65a8c268888aa34f8d80b840c`。

### 2026-11-27 Windows 原生明暗主题复验

在新的断网 Windows 11 24H2 来宾安装同一 `2a1f93b9` Inno 工程包并核对运行时清单。保持同一个 WebView2 窗口运行，在关闭高对比度时切换来宾账号的 Windows 应用主题并通知桌面。产品项目页在浅色、深色、恢复浅色时的底色分别为 `#FFFFFF`、`#080F16`、`#FFFFFF`。深色下项目搜索、列表视图和新建项目仍存在于原生可访问性树并可聚焦。来宾主题恢复原值。回执 SHA-256 为 `4fb04fa34c856ca3e1a0815e6d59da104ec5d2bfc3c30ca9eae01dae7e590400`，深色截图 SHA-256 为 `46336e5b6414073a3c3767d4b0b90e257d5b3297071db435670822c8d340f2cf`。Windows 10 和最终发行包的系统主题仍按发行矩阵复验。[Windows 应用主题设置](https://learn.microsoft.com/en-us/windows/apps/develop/settings/settings-common#personalization---colors)

### 2026-11-26 隔离发行停推与恢复演练

仓库的隔离演练只在回环 HTTPS 提供合成签名 feed，生产 Updater 验证下载和持久防重放。断网 Win11 Sandbox 使用 `6a79c90efc95738ca9ab4851033a7068d23554c8` 的既有完整 Inno 资产，文件 SHA-256 为 `a4922ecd6d48805475357cd474ab051f3e726443570a733ce59744279f6a8bf1`，运行清单 SHA-256 为 `545c375c31f7f2d0853797610032430ffde993cf67548f505d23f3aae132674e`。九项检查覆盖发布前 204、同频道签名下载、跨频道隔离、老客户端与新客户端的停推响应、过时序号、同摘要恢复、等长篡改和恢复后的新客户端下载。该脚本用于按最终候选与平台格式重复演练。

隔离 Ubuntu 22.04 以当前工程候选 `2a1f93b9` 的完整 AppImage 复验全部九项。客户端先拒绝了由不同账号拥有的临时目录，复验在归属一致的临时目录执行，沿用生产路径安全校验。签名 AppImage SHA-256 为 `03d74d4f974d82fdc35818c17edb5f477fe3867690d15543b4d4d569ff5f0686`，演练回执为 `ab5a9f90210cf65239e710dc40819a9c04bb2a859f57e50c64310057fc9c901e`。测试证书、监听器、签名 feed 和缓存都随回环演练结束清理。

### 2026-11-27 主仓候选 bffef439 的隔离发行演练

从干净签名源码 `bffef43911fe7a0cc990036df8ff453ab9898ed5` 分别构建 Windows 和 Linux Updater 演练探针。Windows 11 24H2 Sandbox 与隔离 Ubuntu 22.04 都使用仅回环的 HTTPS 和合成签名。完整 Inno 与 AppImage 的 SHA-256 分别为 `3f39873e3dd05a197853d4426f5f1ccd59e171f24a1adbaeed46af474466950a` 和 `4fe1a8c82e2d24b0ada4fe3db015aa90c6d09277744ef73a5d1d8725b75b2b75`，与同源原生安装的文件摘要一致。

两种格式各通过九项，从未发布时的 204、发布后验签下载、频道隔离，到停推保全既有下载、让新客户端看不到更新、拒绝旧序号、同摘要恢复、拒绝等长篡改和恢复后的新客户端下载。Windows 首次临时证书装载超时，另一次独立探针缺少其 PE 导入的 Microsoft CRT。复验在来宾内安装并清理专用测试证书，在探针目录放入候选携带的已核验 CRT。原生产 Updater 下载校验保持有效。Windows 与 Linux 回执 SHA-256 分别为 `44e893d0be7f4f11613588df186c6e98662f54ba5d33dc9a37d94d653b05acc8` 和 `e70b55d132c85f7e49528fc44dfd1efc1f8dd8ee0dc608070b32ef03a0f6f8b9`。来宾、回环服务、临时信任和下载暂存随演练清理。正式资产及公开频道继续按 A-017 验收与审批。

### 2026-11-27 DEB 独立下载发行演练

Ubuntu 22.04 的隔离回环 HTTPS 向两个独立下载目录提供 `49b25485` 的完整 DEB。用临时 Ed25519 密钥对既有 `asset.json` 的原始字节生成独立签名，客户端用本次演练的公钥验签后，核对候选、格式、目标、载荷清单和包大小与摘要。工程包及 `asset.json` 的 SHA-256 分别为 `232bc08f634724a188a8dbf33f8a040a64460fbab1ade114a1b510758b3d0bbf` 和 `3079144a9f80621d1922e19bc09b181a931e8319ae0aa34ae1e718256b6eb671`。

九项检查通过。发布前和撤回后，清单、签名和包均返回 404。修改清单会在下载包前触发签名拒绝，修改等长 DEB 会触发摘要拒绝。撤回期间，先前验签的包通过断网 APT 安装，已安装载荷清单摘要一致，普通账号冷启动 Core 并重开合成工程。恢复后新客户端下载的清单、签名和包逐字节一致，purge 保全用户资料。回环服务、临时密钥、证书和暂存均已清理。演练回执与 HTTPS 日志的 SHA-256 分别为 `9f048d2fcf0d3b746790a0bfc8aac670bde1585357ee33a64fd905002fe86757` 和 `ea131b6775042f8ac25ec6c1a7df164621c3cbfdc50320733b1aba886ab414e6`。正式独立发行清单仍按冻结候选和签名审批复验。

### 2026-11-26 桌面更新候选门禁

工程候选 `fbcfa1d0` 的 Ubuntu Hosted 测试在部分写入后重新打开迁移日志时遭遇锁争用。隔离 Jammy 通过保留锁文件描述符的副本确定性复现。`73b1a108` 在迁移日志关闭时主动解锁，修复后同一回归通过。Jammy 的 Bootstrap 库通过 8 项测试、定向 Clippy 和 20 轮并行复验，现有的持久阶段进程终止测试继续通过。[首轮 Ubuntu 作业](https://github.com/ALCOMD/ALCOMD/actions/runs/45688341296/job/139466801364)

最终源码 `73b1a108373c1b7a060b3db037017def84f336a8` 通过干净检出的本机整仓和 765 项 Chromium。两次普通快进逐笔验签。最终 SHA 的 Windows Server 2025、Ubuntu 22.04 和 macOS 15 arm64 Hosted 作业均通过整仓、外部 SDK、Release 和 Tauri 无打包构建，各自通过 765 项 Chromium。Actions、JavaScript／TypeScript、Python 和 Rust 的 CodeQL 均通过。[Hosted CI](https://github.com/ALCOMD/ALCOMD/actions/runs/45696134018) · [CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/45696132490)

### 2026-11-27 同源工程候选门禁

签名源码 `2a1f93b978a2180f87d69a4bc54f1f9e52317491` 在干净隔离检出通过格式、整仓 Clippy、`cargo xtask check`、前端检查和生产构建。整仓 Rust 109 组共 1,106 项通过，23 项沿用现有忽略条件。首轮全量 Chromium 为 764 通过、1 项滚轮超时。原用例连续 8 次通过，随后独立全量 765 项通过。保留首轮追踪及原断言。整仓测试和最终全量 Chromium 日志 SHA-256 分别为 `87b502fb5452e9642119998fbe0419ba6eead273f01b2f9792dad36f55811ead`、`2fde025258a0e09d2a3cd0f0889c49bf7de66af106f9e2e9c175fc8a484f8879`。

从 `73b1a108` 到 `2a1f93b9` 共 50 笔提交逐笔验签后普通快进。Windows Server 2025、Ubuntu 22.04 和 macOS 15 arm64 Hosted 都通过外部 SDK、整仓、Release、Tauri 无打包构建及各 765 项 Chromium。同 SHA 的 Actions、JavaScript／TypeScript、Python 和 Rust CodeQL 均通过。macOS 原生客户端与 A-017 公开发行条件保持待验。[Hosted CI](https://github.com/ALCOMD/ALCOMD/actions/runs/45872055470) · [CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/45872054421)

### 2026-11-26 Windows 同范围修复与转换维护锁

同版本 Inno 修复检查本范围的卸载登记、活动版本及包内固定清单摘要。取得 Core 程序租约后，同包逐项核对现有运行文件，仅安装缺失组件，并重新核对完整运行目录和稳定 CLI。篡改、其他范围、活跃 Core 及未完成的维护事务会保留现场。Win11 24H2 Sandbox 上的合成 Inno 夹具通过七项检查，包含 GUI、扩展和稳定 CLI 的字节恢复、无关文件保全和拒绝后的重试。测试 EXE 的 SHA-256 为 `263c594088e15e89a69d786434bca865c9449b4b900da6fb0ac1c4da3efc0ec6`。正式双范围实包和 Shell 入口修复继续按验收矩阵执行。

从签名源码 `f11372bd0d877d7eeb9e56f2bcfce64f2d63cc9a` 构建全部七个 Windows Release 程序。Inno 工程包 SHA-256 为 `7a73e048e1a394c290c70ddc61a1fe798774814f337a321d800dbf383bb4e892`，运行清单为 `def1e255857a78cab54436e9f16f971578592df1b14923eacea181a8136a48e0`。Win11 双范围修复暴露出 Inno 重建卸载登记会丢失快捷方式归属。全用户复验也确认 PATH 归属丢失，两项入口在随后卸载时残留。

`5933f904` 改用同卷原子发布逐项恢复已核验的缺失文件，并保留原 Inno 卸载器、日志与登记。沿用同一完整签名载荷，更新打包脚本并嵌入固定的 Microsoft 在线 Bootstrapper，生成隔离工程包 `b4fb975c9a2ebadf57b49b05d6183d9d982f86fdb7f35709a82dac43e0acab3a`。两个干净的 Win11 24H2 Sandbox 各通过 15 项。当前用户和全用户的单次 CLI 首启分别为 1520 毫秒和 1570 毫秒，均验证实际 WebView2 渲染。修复后卸载精确移除自有 PATH 与快捷方式，保留数据库和无关文件。修复拒绝篡改、持有租约和待完成维护，解除阻塞后重试通过。两份结果摘要依次为 `7eecb06bb14aa1b492925282f19c001d47f9fcbb3e085a43f82c7679b90808b6` 和 `fe744478f8275a35e5428bc961ecf1d281d22de2dee9c6f3f53999a90db952a5`。整包同 SHA 和 Win10 复验仍在发行矩阵中。[Inno 修复包卸载器行为](https://jrsoftware.org/inno-setup/documentation/topic_scriptuninstall.htm)

Windows 平台定向回归验证了转换意图持久化后的启动阻断。后台维护进程退出后，源 Core、GUI、稳定 CLI 和同范围更新仍拒绝启动或写入。匹配的事务可重新取得租约和撤销准备，不同事务无法替换或删除已有意图。Inno 更新和普通卸载也会拒绝待转换的安装。`baa2b3ba` 接通源范围停用、目标范围激活、发起账号登记切换与清理。Clippy、Inno 6.5.4 样包编译、目标端两项回归与 Bootstrap 九项测试通过，完整候选的双向转换和中断场景待 Win11、Win10 原生验收。

从干净签名源码 `310065bfe1b37037f30ad5d7c5d28d4526cb385c` 生成的同一个完整 Inno 实包，在独立 Win11 Sandbox 的当前用户和全用户范围各通过 8 项。安装器拒绝重解析祖先与同范围的第二个安装根。实际标准账号在包含空格和中文的路径冷启动 Core 并登记合成 Unity 工程。删除卸载登记和一个已签运行文件后，同一安装器按受保护的定位记录恢复原目录、原文件和卸载登记，保留 PATH、快捷方式和数据摘要。卸载仅清除已拥有的程序、定位记录和 Shell 入口，保留旧资料、工程和无关文件、PATH 项及快捷方式。

首轮脚本从管理账号解析用户快捷方式，并在 Inno 卸载子进程仍运行时判断结果。复验在原标准账号核对快捷方式目标，并等待原生卸载日志确认结束。实包 SHA-256 为 `b659d8b118ead96f02bc71bc2bbd79d21262c22fba3ed9e16d382a4e89c01dfc`，当前用户和全用户回执分别为 `d074646fad7b405eedc4461d1363f51a741a1a216890f85bb0820b9b1ddd73cb` 和 `287722ab02190c323901daf191f244c755934bce0c28c1a16431a4287e802fba`。Win10、双向范围转换和自定义目录整包更新另外取证。

Windows 协议登记使用受保护定位记录里的安装根拼写，避免自定义目录的等价大小写写法改变归属。登记时在同一注册表事务中创建根键和写入归属标记，移除时在事务中复核只含原归属标记的叶键并删除。类型和值按原始 UTF-16 字节核验。Win11 断网夹具在两个提交点前分别强制终止实际安装器，复验未提交认领消失、删除失败时归属保留、重试、准确命令及外部登记保全。夹具 SHA-256 为 `254aacf8dc127aea30471919f67d1c22bd59df812e298da61f8cbc339e44bec3`，回执为 `e86922c23afa62037e79dadb63656ae34d859393073a40a435b8c455d8116c40`。完整发行包的 Shell 调用、并发改写和作用域转换另行取证。

### 2026-11-27 Windows 当前用户自定义目录整包升级

断网 Windows 11 24H2（`26100.9974`）将 `2a1f93b9` 的完整 Inno 工程包安装到父目录和安装目录名都含空格及 Unicode 的路径。以只改七处版本元数据的签名提交 `16e5184a` 构建完整 `alpha.1` 载荷。修复前旧版能在自定义根启动，签名升级返回系统错误 `2`。Inno 在自定义根同级以固定产品名生成暂存目录，Bootstrap 以实际目录名查找。Inno 暂存和提交改用同一个按实际安装目录名推导的路径。

用修正后的 Inno 模板重打包同一已签运行时。目标安装器 SHA-256 为 `e5c4e9462b5bc838985cf4c07bf42e5d439904d597b6da34d4f9ca459117a1d0`，目标运行清单 SHA-256 为 `b2c4843dc182d1b8967cfc2580e35b88f088d4beb56c737a7be20e5b875a004d`。正常升级通过七项。中断恢复通过十项，实际终止更新进程树，验证待完成事务阻止两版启动、篡改保留安装包被拒，以及删除原下载后仍能恢复。两次均核对原自定义根、默认根始终缺失、安装与 Core 登记、PATH、快捷方式及新版 WebView2。原工程可重开，稳定 CLI 冷启动分别为 1371 和 654 毫秒。卸载仅移除自有程序和入口，保留原资料及目录内的无关文件。正常和中断回执分别为 `60d0f52bb38029188180da9db3753eb60f10e93daaac53fcdeac0b638990ad43`、`d2c493b228c77d0327c7edae7987602930ddef3526e611afda05354245029272`。全用户、Win10 和正式发行候选另验。

另从干净签名源码 `bffef43911fe7a0cc990036df8ff453ab9898ed5` 构建七个 Release 组件并生成完整 Inno 工程包。安装器 SHA-256 为 `3f39873e3dd05a197853d4426f5f1ccd59e171f24a1adbaeed46af474466950a`，运行清单为 `d424453871bb3aa10ba4f716052bd65c2fd60aa7db5a62ccf253527672acbb0f`。断网 Win11 当前用户复验真实 Unicode 自定义目录、开始菜单打开真实 WebView2、CLI 连接桌面已启动的 Core，以及卸载清理自有入口并保留数据库和无关文件。回执 SHA-256 为 `bd64243480775300c01569fac1c2ba271469121018a5cf5e07d9ff6917ce8615`。

该 SHA 在干净检出通过本机整仓聚合检查，Rust 测试共 1107 项，Chromium 765 项。从 `2a1f93b9` 起逐笔验签 12 笔提交后普通快进到主仓 `main`。该 SHA 上的 Windows Server 2025、Ubuntu 22.04 和 macOS 15 arm64 Hosted 均通过整仓、外部 SDK、Release 和 Tauri 构建，每个平台的 Chromium 均通过 765 项。四项 CodeQL 全部通过。Hosted 终态回执 SHA-256 为 `7465ae2125a1dd8d8ef15ee3cf9279ec50ba749b47e109ced4d32f7f83f2f94f`。macOS 原生客户端仍延期，正式发行继续按 A-017 验收。[Hosted CI](https://github.com/ALCOMD/ALCOMD/actions/runs/45898023638) · [CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/45898023276)

### 2026-11-27 主仓候选 bffef439 的 Linux 原生复验

在隔离 Ubuntu 22.04 复用既有编译缓存，从干净的签名提交 `bffef43911fe7a0cc990036df8ff453ab9898ed5` 落包。AppImage 与 DEB 的 SHA-256 分别为 `4fe1a8c82e2d24b0ada4fe3db015aa90c6d09277744ef73a5d1d8725b75b2b75` 和 `f67ae2c7ae742454c879497ab6595f8d70ebb4acd498897c1b0f00eda5a6ab44`，共同运行清单为 `8ac42afd3034dd053066acd085c5bd75a68d6dff2047800e9c8b617617879705`。`dist --check`、配对桥接验签和两包各 8 个 ELF 的符号扫描通过。最高要求均为 `GLIBC_2.34`。

断网标准账号分别通过原生 Core、稳定 CLI 工程读写、WebKit、MCP 认证与撤销、第一方扩展验签。AppImage 实际经 FUSE 运行，DEB 实际由 APT 安装和清理。DEB、AppImage 原生回执分别为 `d03f465024d2c2b492e398caa5c7ff8b0eac1a9689bec1055490aa0dc54e1c4a` 和 `eee3202f6fe786d2c7d2c4991092be16009204c0a7570dd2f23ec9a9d978653b`。独立核验组件、清单和两份回执后生成隔离健康证明。

用以上相同摘要的 v4 AppImage 和 DEB，在隔离 Ubuntu 24.04 从实际冻结 v3.4.0 界面复验交接。两个格式均通过回滚后重开 v3，以及确认提交后启动 v4 GUI 并连接同版本 Core。合成签名的独立 AppImage 桥接归档为 `74755ac1e689b3524cdc0e71528403e06ce3b95df61653445b765d2600f86db3`。DEB、AppImage 迁移回执分别为 `8fbd47b6ea0498deeccd57ac0a94501f057ce6a2ceb245ecffb93468b5e05327` 和 `c120f76f6dd6b7890f104f8c32a863f469032a7015a1dc6fee9bb794093cd0db`。原冻结程序保留，两套测试资料和临时挂载按夹具归属清理。正式资产仍须按发行矩阵复验。

同摘要两包在隔离 Ubuntu 22.04 的 X11 WebKit 中通过 100% 与 200% 缩放。窗口由 `1400 × 800` 变为 `2800 × 1600`，项目搜索、列表视图和新建项目的 Tab 顺序一致。首轮 AppImage 的 FUSE 清理早于渲染进程释放，等待释放后复验正常卸载。AppImage 与 DEB 缩放回执分别为 `7cfc52756737258d6f6e471293f9cefe48cb28c1f7fee05ec472439210694151` 和 `0f5882a88bbbce067189887dbf1a105f96d1a3ee4fbd0beb5edcf8003e521cde`。Orca 复验实际朗读同三个控件，捕获 179,461 个合成音频样本，日志摘要为 `f2b027f5df1345185eaa0ce1f8952009f7ac7c9b66cfd9db089920d0871994a7`。

同摘要 DEB 在断网 Ubuntu 22.04 通过 GIO 启动 dpkg 所有的桌面入口。在隔离 XDG 用户配置中选定该处理器，再分别用仓库和扩展链接冷启动、向已有窗口转发。四次均核对真实 WebKit 确认表单的预填地址，取消后 Core 的仓库、扩展列表与各自基线一致。卸载移除 DEB 自有桌面入口、CLI 和运行目录。回执 SHA-256 为 `8bbb74a579250d434b58b82fb1001765f7c8451bc4552ef3b37a496172d9f309`。

### 2026-11-27 主仓候选 bffef439 的 Linux 整包升级

复用已验签 `bffef439` 的 alpha.0 实包，目标 `41b7a82be5dd57bcf26e5c58b97ef8169a1ae97c` 是只调整七处版本元数据的签名子提交。隔离 Ubuntu 22.04 从干净检出构建完整 alpha.1 AppImage、DEB 和仓库外 Rust SDK 消费者。目标 AppImage 与 DEB SHA-256 依次为 `bd13ddf69293b74dd4578e3b58470d24b19e3d4e8e3e926e9f463e1a0bc9294c` 和 `e099209820212f115a141530cc5f0a4afa3f14e118adadf7c605d0445cc57e63`。已验签目标运行清单为 `95e99cea2702e5233a89c23349d98f795626c151d2482b9b1e0f1fdc1584fdd6`，最高 ELF 符号要求为 `GLIBC_2.34`。

AppImage 原生 FUSE 和 WebKit 的「关于」页通过回环 HTTPS 分步检查、下载、取消、确认替换和重启。原 Unicode 路径工程可读且内容摘要一致。CLI 集成的启用、移位重绑、修复和移除通过，外部 SDK 从已登记的原安装首次冷启动为 974 毫秒。另一用例在实际交换镜像后、切换登记前终止更新进程，确认篡改的保留镜像被拒，原下载删除后仍能从原地恢复。数据库、设置、工程和保留的原安装包均按摘要核对。界面与恢复回执依次为 `ce54341e539a6f6a091575f91691e5149614d63ddcd6b17a61c37d75bcaf5161` 和 `2ace8bba5442fa65e4086ffcff558eb14c6fc84cb0bb9cddb92dfda0518c671f`。

DEB 通过离线 APT 首装、停在 `dpkg --unpack` 后的配置恢复、双账号独立启用、同数据版本回退、重装及 remove/purge。坏签名和失效安装登记被拒，两个账号的数据库、设置和工程摘要均通过对账。外部 SDK 单次冷启动为原版 971 毫秒、新版第二账号 973 毫秒。DEB 回执为 `bcf23168a9751e15c54db637c2dc2eb4c4082d390fbd788e1523740619f89c2b`。本轮测试包、临时账号和暂存已清理。工程包继续使用隔离签名，正式资产仍按 A-017 复验。

### 2026-11-27 DEB 配置触发器中断复验

在断网 Ubuntu 22.04 x86_64 复用 `748933d8` alpha.0 和 `bd30aba3` alpha.1 的签名工程包。新版解包后，`dpkg --configure --pending` 实际进入系统 `hicolor-icon-theme` 的图标缓存触发器。私有挂载命名空间在该触发器调用缓存辅助程序的入口暂停本轮进程组，再以 `SIGTERM` 终止。退出值为 143。此时 ALCOMD alpha.1 已标记为已安装，图标包处于 `half-configured`。缓存修改时间、旧账号登记、数据库、设置和工程文件均与中断前一致。

解除暂停用的临时挂载并核对原辅助程序摘要后，以 `dpkg --configure --pending` 恢复。系统图标包回到已安装，原生缓存重新生成，`dpkg --audit` 为空。原标准账号冷启动 alpha.1、更新该账号的登记并读回同一工程，卸载后资料摘要一致。临时测试数据已清理。回执和恢复日志的 SHA-256 分别为 `683919e51738e8bf5a13cbc010fd392ef4c9d9b1596735e1f1610583163f46a1` 和 `5d91d93dce8d8d40ff2fc7afdefc6fd2940e9111e07b78122782dfb241c6f00f`。

### 2026-11-27 DEB 解包存储故障复验

在 Ubuntu 22.04 x86_64 的独立网络和挂载命名空间复用 `748933d8` alpha.0 和 `bd30aba3` alpha.1 的隔离签名工程包。两个包的 SHA-256 分别为 `aa97ee9139577b21e04559cdd4824db546a3764b4c1fc50d76a9763b1fb89c9e` 和 `731b6ceeb70fc5c3d88881442d326e1520db6e586712a4ca1e264e4304154286`。用 384 MiB tmpfs 承载版本化程序目录，合成工程和账号资料使用独立的可写目录。

安装卷仅余 8 MiB 时，`dpkg` 写入新版 Core 实际收到 `ENOSPC`。另一轮将该卷设为只读，实际收到 `EROFS`。两次失败后包管理器均保留 alpha.0 的已安装状态，旧运行时、稳定 CLI、Core 登记、数据库、设置和项目文件摘要一致。普通账号通过 IPC 重开原工程和日语设置。分别释放空间或恢复可写后，断网 APT 用同一 alpha.1 包升级成功，新 Core 读取同一工程和设置。两次 purge 均保全资料，卸载测试挂载后确认程序入口、暂存和测试 Core 均已清理。磁盘满和只读回执的 SHA-256 分别为 `e67bfbe56fb5c03be1f372c79518085fc568d3fef977ecdb0c0cdc51700801ef` 和 `16b37bfa6967f529087d8015a1b8a30556d5e9aa496308c1a7f9b30de1ca6b02`。

### 2026-11-27 DEB 下载中断复验

在 Ubuntu 22.04 复用上节两版隔离签名工程包。独立网络命名空间只开放回环，将临时 Ed25519 公钥仅用于本次 APT 测试源。APT 验证签名索引，从该索引选定 SHA-256 为 `731b6ceeb70fc5c3d88881442d326e1520db6e586712a4ca1e264e4304154286` 的 alpha.1 包。测试源声明 36,796,100 字节，发送 1,048,576 字节后断开。APT 报下载失败，`dpkg` 仍记录 alpha.0 已安装。旧运行时、Core 登记、数据库、设置和合成工程的摘要一致，普通账号经旧 Core 重开原工程并核对日语设置。

恢复完整响应后，APT 沿用同一份签名索引和同一摘要的软件包完成升级。新 Core 打开原工程并读回设置，purge 保全资料。核对隔离来宾的包状态无异常，测试源、密钥、暂存和测试进程均已清理。回执与 HTTP 请求记录的 SHA-256 分别为 `4babfd940bea4a93795b304ba6324bf63d759c9f2a842b5949d3af0607013319` 和 `e6040fcdbaf44e56a703f5e10d6d8eafd7d36412aacbeab929eba0b137ce8de5`。

### 2026-11-27 DEB 缺失依赖复验

在断网 Ubuntu 22.04 x86_64 的一次性覆盖根，使用 `49b25485` 签名工程 DEB，包 SHA-256 为 `232bc08f634724a188a8dbf33f8a040a64460fbab1ade114a1b510758b3d0bbf`。临时系统通过 APT 移除直接依赖 `libwebkit2gtk-4.1-0` 和依赖它的开发包，核对运行库文件确实缺失。使用空 APT 软件源和原 DEB 首装时，APT 明确报告该依赖不可安装。失败前后的 `dpkg` 状态摘要一致，无产品程序、CLI 或桌面入口。

恢复时向断网 APT 同时提供原 DEB 和 Ubuntu 官方 amd64 依赖包 `2.50.4-0ubuntu0.22.04.1`。依赖包 SHA-256 为 `f1836c87c10ae3d0d352d996c63e0bb40a80537533075f688ff4eeb9d50f3952`。安装成功后核对实际 GUI 可执行文件的 WebKitGTK 动态库解析，普通账号冷启动 Core 并重新读取合成工程。Purge 保全工程和数据库。外层 Jammy 的包状态及依赖文件摘要一致，临时覆盖根和下载的依赖副本均已清理。回执与拒绝日志的 SHA-256 分别为 `687e2e8522b00192ce13966aa3b08bd476900c6664e5e5afb68c65e12fab49bb` 和 `d103960462160e8265d340986a475ae8ab22f095d386d6b4829170dfca6c434d`。[Ubuntu 官方包及摘要](https://packages.ubuntu.com/jammy/amd64/libwebkit2gtk-4.1-0/download)

### 2026-11-27 AppImage 整包更新磁盘满复验

在 Ubuntu 22.04 的断网隔离实例用普通测试账号复用 `bffef439` 的 alpha.0 和 `41b7a82b` 的 alpha.1 签名实包。旧 AppImage SHA-256 为 `4fe1a8c82e2d24b0ada4fe3db015aa90c6d09277744ef73a5d1d8725b75b2b75`，目标沿用上节摘要。用独立 tmpfs 限制空间，合成更新声明和下载包均预先校验。

- 替换前，安装文件所在挂载剩余 4 MiB，生产 Bootstrap 在暂存 46,578,168 字节的目标包时返回 `ENOSPC`。旧镜像、登记、数据库、设置和工程摘要一致，暂存和更新意图未留下。通过 FUSE 重开旧版桌面和 Core 并读回工程。释放空间后用原声明和原下载包升级成功，保留旧镜像并从新版读回工程。日志 SHA-256 为 `e2da0c359819520c41edc8a345126096f2ec1b80fcd644f3a8f48cb87b95f6a8`。
- 替换后、登记前，实际终止更新进程并删除原下载包，再将独立资料卷可用空间压到 0。跟踪确认 AppImage 提取器的写入返回 `ENOSPC`，本轮的 Bootstrap 报告签名解码错误。恢复失败期间，同一意图、原登记、新旧镜像和资料摘要均保持。一次复验在 512 MiB 挂载内释放占位后仍缺少重解包空间。最终复验释放至 550,252,544 字节后用现场保留资产完成同一事务，清除意图并从真实新版 FUSE/WebKit 与 Core 读回原工程。结果和系统调用证据 SHA-256 分别为 `c08aaf2bcbcc4a5d239148d82b536f9f53d4c9c6e7520affb689d557fb8713bc`、`d2a139df4b52ae79ef55a2f0644201969f157286ff774885ded90cb081fa2631`。

固定的 Type 2 提取器在 4 MiB 的私有文件系统上返回成功且不输出诊断，尽管提取文件不完整。Bootstrap 将提取和签名核验作为相邻步骤处理，失败时检查暂存文件系统可供普通账号使用的块和文件项。空间耗尽时返回 `StorageFull`，其他失败仍保留原有原因。正常流程仍核对完整签名、清单摘要及每个组件。

Ubuntu 22.04 定向 Release Clippy 和 63 项 Rust 库测试通过。合成签名夹具在 alpha.0 实包中只替换此次构建的 Bootstrap，alpha.1 资产保持既有摘要。夹具 AppImage 的 SHA-256 为 `de3b9c003ad976ea6b678f745703bce4d9373538792737e29f4641f7725e3cc0`，Bootstrap 为 `3ee4cee3a6669059b837937d518afc68c4a3a1da87f6f0c2ca3eb5c2ab011366`。替换后空间耗尽时，恢复报告 `no storage space`，并保全原意图、镜像和资料。释放空间后用保留资产完成原事务，经 FUSE、WebKit 和 Core IPC 证实 alpha.1 可重开原工程。日志 SHA-256 为 `47a0eba16c0736d0c8c5a14628f48ea2e4042b5ee7839a57fddeeb624f574b65`。本次结果属于定向修复验收，正式整包候选另走发布门禁。

### 2026-11-27 49b25485 同源实包与门禁

在两个平台的干净检出从签名源码 `49b25485a315502ea96767eb12c2afa1401724a3` 构建和组装三种工程实包。运行清单使用隔离签名，Windows 摘要为 `36d67906b4f3686722b8bd92b16b972552f7ac810b68cdf79e5ebf70df330e5f`，Linux 两种格式共用 `65eb2298d05dc905e101a9646a145b077190ffc65f744c190e04f7a449778a2b`。Linux 两包解包后再次验签，三个实包的 SHA-256 如下。

| 格式 | SHA-256 |
| --- | --- |
| Inno EXE | `fb9890353f26a8d6c2507e8688ee5a013811aac53edb4a6343ca19e8612f9aef` |
| AppImage | `f00536323b403e4b7643ff8bcc1d10389c8f0b3a1ab144296f902e08588d5327` |
| DEB | `232bc08f634724a188a8dbf33f8a040a64460fbab1ade114a1b510758b3d0bbf` |

Ubuntu 22.04 用该 AppImage 和既有 `41b7a82b` alpha.1 资产复验原生磁盘满。替换后再次提取时系统写入返回 `ENOSPC`，签名候选报告空间不足。在原下载不存在的情况下，释放空间后从保留镜像完成原事务，FUSE、WebKit 和 Core IPC 均通过。日志 SHA-256 为 `4a0eccb09dbaff4fdc1b919d1bb0723f316e09a0c5e5238cca634d2dc5af3aa2`。DEB 通过实际安装和 purge，GTK 100% 与 200% 下三个项目控件均能依次 Tab 聚焦。回执为 `e667f9e5601e8d3c9c135ae2d80e140c5a0e2111ab2294569681dadf2fa649a1`。

Win11 24H2 两种安装范围均通过 Unicode 安装目录、WebView2、稳定 CLI 和卸载保全。当前用户冷启动为 2779 毫秒，回执为 `35b94e6d892d05a922f89b993f739bdbe4fb1fb3752f033a94e74b0a9d0ecf5f`。全用户首轮达到 5 秒且未就绪，回执为 `825c71d561b1496180fe55841b26bb7eeb57ea94852d4e7cbf61fc528c303534`。在新来宾按原条件复跑为 2050 毫秒，回执为 `df0045aae2cc323aeefb8fa6a7fbbce34662f18fa84d3b64f9218a182616bf40`。

同一 Win11 Inno 与 `16e5184a` 的 alpha.1 目标包在专用 768 MiB NTFS 虚拟盘补验当前用户整包更新。盘上保留 8 MiB，独立写入确认 Win32 `ERROR_DISK_FULL`（112）。更新在持久提交前失败，原安装、登记、资料与下载包摘要一致，旧 Core 可打开原工程。释放空间后用同一声明和目标包重试成功，新 Core 可读回原工程。当时的 Bootstrap 返回找不到文件（Win32 2）。早期试验的默认虚拟盘 ACL 和非标准下载目录被校验拒绝。通过回执 SHA-256 为 `6278e6bd50b3fb6f0cf03ba6bc9af45e6c27cb462ecdecc13f00aaf18c0221e5`。

签名提交 `aed59608` 修正 Windows 暂存诊断。Inno 返回约定的退出码后，若暂存目录未发布且安装卷可用空间小于已验签安装包的大小，Bootstrap 报告空间不足并显示两个测得值。定向 Clippy 和两包 90 项 Rust 测试通过。隔离签名夹具仅重建 Bootstrap，其余运行组件沿用 `49b25485`。Bootstrap 和 Inno 摘要分别为 `537e1ff0a75fd1049b258bc9843219ed45a1b1d877ee2bd44f3454d665b91633`、`06e96306df0922fd22695a1a6cd85d4bea0eefea2f63de55ea0f7432d019cdd4`。

同一 Win11 NTFS 场景复验通过。Bootstrap 报告安装卷可用 8,388,608 字节，签名目标包为 33,826,183 字节。提交前保全原安装、登记和资料，释放空间后用同一签名包和声明重试，新 Core 经 IPC 打开原工程。回执 SHA-256 为 `d692c95ac2a6782d29f9e56bdcf4837cf260e3e11820f7e843520fd651edbf09`。本轮覆盖 Win11 当前用户的磁盘满诊断与复原。

整仓规则检查要求将 Win32 调用置于现有 Windows 原生模块。签名候选 `9eba5b98` 完成调整，用同样的隔离签名组装方法重新编译 Bootstrap 并复验。暂存失败再次报告 8,388,608 字节可用，旧安装及资料保全，同包重试和新 Core 重开原工程均通过。Bootstrap、Inno 与复验回执的 SHA-256 依次为 `3830d4ec6d721330cad8a9bdd9aa7708154c57696ae7817cea73255472a38bfd`、`cc91e1934f61cadf77264af60fe37422e300881884ddc6d40049c7a2feaf9ec6`、`781e50d882ca606b52e6377de97f0fc72c6045abeb45f300a9f4c6de4a0b88e1`。

同一候选的本机整仓门禁和前端构建通过，含 1,108 项 Rust、7 项 Node SDK 和 765 项 Chromium 测试。四笔提交逐笔验签后普通快进至主仓 `main`。整仓标准输出和 Chromium 回执的 SHA-256 分别为 `bd614ce95b1603724c1a763160af9c1ba7dea95344d754b7d5c7e4a752f15154`、`dc1444448e0c607ca8a46bc53f4e3254dcf322844d50eddafd571a85941677d0`。

Windows Server 2025、Ubuntu 22.04 和 macOS 15 arm64 Hosted 均在 `9eba5b98` 完成整仓检查、外部 SDK 消费者验收、Release 与 Tauri 构建和 Chromium 回归。四项 CodeQL 作业也在同一 SHA 通过。Hosted CI 和 CodeQL 终态回执的 SHA-256 分别为 `7445937bf041b3f1f0de1919f8f316226b5679579d265eed1ee484d197839365`、`5444ed791ebce091ca6e2442817541aec0998dc9224fe8fd8a9eaaaac2f78824`。[Hosted CI](https://github.com/ALCOMD/ALCOMD/actions/runs/45974897471) · [CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/45974897091)

候选逐笔验签后普通快进至主仓 `main`。本地整仓通过 1,108 项 Rust、765 项 Chromium 和 7 项 Node SDK 测试。Windows Server 2025、Ubuntu 22.04、macOS 15 arm64 Hosted 在同一 SHA 通过整仓、外部 SDK、Release、Tauri 与 Chromium，四项 CodeQL 通过。终态回执 SHA-256 依次为 `24c36509717941687641c722d41b9dc90f29cf6cf7575e61feedda64bf6a5a13` 和 `4a49888e01edfc5558847db5a063108f68ab74a234b137fad017f79f7b3782b2`。Windows 10 和 macOS 原生客户端验收继续延期，公开发行仍按 A-017 放行。[Hosted CI](https://github.com/ALCOMD/ALCOMD/actions/runs/45952341316) · [CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/45952341173)

### 2026-11-27 9eba5b98 Linux 同源实包复验

隔离 Ubuntu 22.04 在干净签名检出 `9eba5b982c6a6b7622fad40b001d541f3ce3e333` 构建完整 Release 和 Tauri 组件。用固定摘要的工具组装 AppImage 和 DEB，两个包解出相同的签名载荷清单 `36a3467bd72b6dbcb643ee8da5269ff87ad9373fe2d630afc5ab4295425ecb1f` 并重新验签。最高 ELF 符号要求为 `GLIBC_2.34`。AppImage 和 DEB 的 SHA-256 分别为 `de7134d4db21e41765142a5369df41e384bf63492c1397ae625048f7602c259a` 和 `3b04f0ce711f92be33820f007214a7eb0c0c22ec622c24db60d3f850be742b70`，组装回执同时通过资产摘要、错误工具摘要、拒绝覆盖和打包器失败时的原子发布检查。

断网普通账号用这两份实包复验真实 WebKit。AppImage 在两个缩放档均经原生 FUSE 挂载，DEB 经实际 `dpkg` 安装，并登记 Core 到各自的隔离配置。100% 和 200% 下窗口分别为 `1400 × 800` 和 `2800 × 1600`，项目搜索、列表视图与新建项目的焦点顺序一致。随后清理了临时运行时和 FUSE 挂载，DEB 的 purge 移除程序和系统入口，包数据库无待处理项。AppImage 和 DEB 原生回执的 SHA-256 分别为 `a5d3f0b531f317d1937f05dc98ad9e0102497ea5330e6e2cb6309f00b9784f9f` 和 `23c510787a847ab5d032b99160f23d52c7b67a15680825eb809cfa71e04f2838`。

### 2026-11-27 9eba5b98 Windows 同源实包复验

从与上述两份 Linux 工程包相同的签名源码 `9eba5b982c6a6b7622fad40b001d541f3ce3e333` 重建 Windows 全部 Release 组件与 Tauri GUI。用已固定摘要的 Inno 6.5.4 和 Microsoft WebView2 Bootstrapper 组装完整 Inno。已验签载荷清单 SHA-256 为 `245d34c92003b964efc708adbf694d3f556decd134499abf01c38e8e5f0d4a07`，成品为 `f975c2c47bcd9dedc0bdd8ccc3c6c40d6a9d5d487c43558332baf209d5d20e52`，大小为 33,829,599 字节。该 SHA 对应上文已经通过的本机整仓、Chromium、三平台 Hosted CI 和四项 CodeQL。

两个断网 Win11 24H2 Sandbox 用同一个 EXE 验证当前用户和全用户安装。两种范围均通过 Unicode 目录安装、稳定 CLI 与 Core 冷启动、真实 WebView2 窗口和卸载资料保全。全用户安装阶段保留账号的未初始化状态。两次冷启动分别为 1,539 和 1,566 毫秒，回执分别为 `660170a100a8a2bdeeabe4173dcf33b83925d391fadd9701e1e3814096251c24` 和 `84c8f9ef2d88aacebe0b569aee7c49f8f6338497e0ded1f586d10545a909543c`。

另在当前用户范围用专用 NTFS 卷复验整包更新，从同一 Inno 升级至此前验签的 `16e5184a` alpha.1 包。安装卷仅余 8 MiB 时，Bootstrap 报告空间不足，旧程序、登记和资料保全，旧 Core 仍能打开原工程。释放空间后用同一声明和签名包重试，新 Core 经 IPC 打开原工程。回执 SHA-256 为 `9fd92991151318b940bcfa737e9b36c8705461e33d4bae3a55a183e1e9851aae`。本轮回收 5.32 GiB Windows 编译缓存并清理三份重复测试输入。正式资产及延期的 Win10、macOS 原生验收继续按 A-017 记录。

### 2026-11-27 Windows 全用户整包更新磁盘满复验

在断网 Win11 24H2 Sandbox 的既有管理员会话，用 `9eba5b98` 的完整 Inno 将 alpha.0 全用户安装到独立 NTFS 虚拟卷上的 Unicode 目录。目标用已固定摘要的 `16e5184a` alpha.1 工程包，签名安装器为 33,826,183 字节。用真实 NTFS 分配将可用空间降至 8,388,608 字节。独立写入探针返回 Win32 `112`，机器更新在保留包阶段失败，未留下已发布的事务或残留包。

第二次留出 43,266,048 字节。保留的签名安装器、更新声明和验收凭据已落盘，解包暂存失败后卷剩余 9,433,088 字节。两次均未发布新版、暂存目录或提交意图。更新操作前后的稳定 CLI、旧版 Core、版本选择器、机器与账号登记、数据库、设置和合成 Unity 工程一致。每次都通过 CLI 冷启动旧版 Core 重开工程。

释放空间后用原声明和同摘要安装器重试成功。机器选择器切换到新版，账号登记在该账号首次启动 Core 时更新，原工程可再次读取。入口在两处分别报告向机器维护进程传输失败和全用户维护未完成。三项通过的回执 SHA-256 为 `87583c215e7d64ad614364bf75b482f63858034368e310cb2e8f36d8c6baf38d`。标准账号交互 UAC、Win10 和最终发行包继续按发行矩阵验收。

签名修复 `49a3d526` 让全用户更新调用方从系统维护进程的退出状态识别存储空间耗尽。发送期间接收端已断开时，调用方在限定时间内核对同一进程的结果。仅报告固定错误分类。源文件和暂存路径留在维护进程本机诊断中。正常失败、已有目标和无法确认结果仍按原合同处理。[Microsoft 进程退出状态文档](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getexitcodeprocess)

Win11 复验使用 `49a3d526` 构建的 Release Bootstrap，其他组件沿用已验签 `9eba5b98` 载荷。Bootstrap SHA-256 为 `190d6461e54d625684ea508506522d5c2fccf7340618e254ff989b94e7a37f6a`，隔离测试签名的 Inno 为 `ac98a4a2a25b2e8f9c4b221fd8764517f3e3c80c0440389021c6187fec5de9eb`。对同一 `16e5184a` 目标实测两处空间不足，调用方均报告 Win32 `112`，旧版与资料保全。已有目标目录时仍返回原来的传输失败，目录内容保全。释放空间后同包重试完成，账号重开后登记新版 Core 并读回工程。四项复验回执为 `a02eb6acdd844568a49be2c2792c8bdf6af0c289aef66e7aba8d27141179ea5f`。

另在全新的断网 Win11 中，复用同一对已验证实包，经 Bootstrap 的桌面重启入口执行磁盘满和重试。专用安装卷剩余 8 MiB 时报告 Win32 `112`，写入本账号的桌面失败标记，重开旧版真实 WebView2 窗口并经旧版 Core 读回原工程。释放空间后用原签名包重试，标记清除，实际新版窗口重开，新版 Core 读回工程，设置与工程文件摘要一致。两项回执 SHA-256 为 `e13559774da9245c5764becdc0f1fc042809b764d4874eae33f9db2e15492ead`。

签名候选 `49a3d52626679c50bde8ce055f0c0c1be5cd0873` 在独立干净检出通过整仓聚合门禁，Rust 1,108 项、Chromium 765 项和外部 Node／Rust SDK 均通过。逐笔核对 11 笔提交的签名后从 `9eba5b98` 普通快进，远端读取确认候选一致。聚合门禁和远端核验回执的 SHA-256 分别为 `d6a53f5c6ba919fab39ba97c9e83b6087a75305684c85eb5323f9a6f7e8f977a` 和 `76dcea8dd63b3f89d66054f513ad43384baec9203f5e0353c521dc26043850b5`。同 SHA 的四项 CodeQL 已通过，终态回执 SHA-256 为 `d94840db395d4d128b3f0d5f2cc3ed05edc40e6609579a32c4afd78743e93a69`。三平台 Hosted CI 等待终态。[Hosted CI](https://github.com/ALCOMD/ALCOMD/actions/runs/46016326613) · [CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/46016326331)

### 2026-11-27 Windows 全用户自定义目录整包升级

在断网 Windows 11 24H2（`26100.9974`）来宾的既有管理员会话中，用上述 `2a1f93b9` 安装包和修正 Inno 后的 `16e5184a` 目标载荷执行全用户整包升级。安装根的父目录与末级目录均含空格和 Unicode。正常升级通过九项。机器部署不抢先初始化账号资料，机器升级保留旧的逐账号登记，原账号重开后才登记新版 Core。原安装根、机器 PATH 与开始菜单仍指向该实例，真实 WebView2 和稳定 CLI 可读回原工程。CLI 冷启动耗时 1552 毫秒，卸载后数据库、逐账号登记和目录内的无关文件保全。正常回执 SHA-256 为 `96e3ffc15dd320a44b37b85dfb325eacbdda88b6b0ef167b3b714cdf7d595fe0`。

文件占用与进程终止复验各通过十二项。文件占用用例在新版本已落盘时因稳定 CLI 文件仍被占用而停在持久提交阶段。进程终止用例在同一持久点实际终止了该请求的维护进程树。两次中断均保留原版本选择器和资料，稳定 CLI 和两版 Core 均被门禁阻止启动。解除占用并删除原下载后，Bootstrap 用事务保留的签名包恢复，CLI 冷启动分别耗时 1112 和 857 毫秒。文件占用和进程终止的回执 SHA-256 分别为 `dd682f499804afb061d5065f912fae117854bfe36afea73356f20163e903b79b`、`3e7a85a3a6fcd022941a86ee2da851bd5a212ad8e7fe2942fe7c4d108257d8f1`。真实标准账号的交互 UAC、Win10 与最终发行候选另验。

### 2026-11-27 Windows 安装范围转换拒绝复验

用 `bffef439` 的完整 Inno 工程包，在两个断网 Windows 11 24H2（`26100.9974`）来宾分别测试当前用户转全用户、全用户转当前用户。两个方向各通过七项拒绝和保全检查。缺少业务确认时安装器保留源范围。提供匹配的产品清单确认值后，原生 Bootstrap 拒绝既有提升会话直接作为转换发起人。两个方向的 Inno 安装均在激活前退出，保留源范围的注册表、PATH、开始菜单、`alcomd` 协议、账号登记、数据库、设置和测试工程。两次都从源范围的稳定 CLI 在 496 毫秒内冷启动并读回工程。

已确认但发起身份不符的尝试均留下未激活的目标预留目录。目标范围的活动选择器、稳定 CLI、卸载登记与系统入口均未发布。当前用户转全用户时 Inno 提示从拥有安装的账号发起，全用户转当前用户时 Inno 返回通用载荷准备错误，直接核对已安装 Bootstrap 可取得原非提升账号错误。两方向回执 SHA-256 依次为 `50b9712b950273c65b5eb8223f58628b37c506cb16fa1b01b49b299c7e90c9de`、`429ade2b45b594de5f553fa76c5327a22bf72f1bd2be1053d684a52059ead910`。双向正向切换、交互 UAC 以及最终发行包继续按原矩阵验收。

### 2026-11-27 Windows 协议事务并发复验

Win11 断网夹具先复现了已完成的第三方写入被稍后提交的安装器覆盖。安装器改为在注册表事务中暂存变更后重新核对公开键树的归属、值与形状。若第三方在最终核对后成功改写已参与事务的键，Windows 终止事务。范围转换在最后发布和退役清理时仍须确认归属，遇到第三方登记返回可重试结果。[Windows 注册表事务](https://learn.microsoft.com/en-us/windows/win32/api/winreg/nf-winreg-regopenkeytransactedw)

同一 Win11 合成夹具通过四项基线和七项并发检查。覆盖认领和移除的提交前进程终止、受保护的根路径拼写、外部登记保全、暂存前命令和子键改写、最终核对后写入，以及范围转换的可重试结果。Inno 6.5.4 也用合成载荷编译了完整安装脚本。原生夹具 SHA-256 为 `5767f8caddbe62ef785a14933d711b2c658246e37b21cb35802b2b6a390beca5`，回执为 `be377a828ca8e9d08499b4ba3b1bc5d184dddc9711b627943b7e091a03093c3`。

从干净签名源码 `2a1f93b978a2180f87d69a4bc54f1f9e52317491` 构建七个 Release 组件，核对合成签名后用固定 Microsoft WebView2 Bootstrapper 生成完整 Inno 工程包。包 SHA-256 为 `e8dbbe0e44d8c10ebed68acee7f677700b825bf11d5ccd66d6e96e6e668ee425`，运行清单为 `2c65dae115f1667122700ec2d4e944336f6976a11e844b907c103398f66dc90b`。两个 Win11 Sandbox 分别以已登录账号按当前用户、全用户范围安装到 Unicode 路径。两种范围均通过准确键和作用域核对，实际 Shell 冷启动进入仓库确认窗口，热转发扩展链接至同一 GUI 并显示准确的预填来源。取消后正常退出，实际卸载移除准确的自有协议和程序。当前用户和全用户回执 SHA-256 依次为 `a8b8acc9045edaf53653aa214d1ff8488965638ee91b51772ce4c4a4b398d5b6`、`7a45ef57e152588632473fb78ce3003fe91bb554040df4d2726bb870d70784da`。标准账号和双向范围转换另验。

### 2026-11-26 Windows WebView2 前置部署

Windows Inno 发行打包接收固定摘要的 Microsoft Evergreen Bootstrapper，按需接收固定摘要的 x64 Standalone。打包校验 Microsoft Authenticode 发行者与文件摘要，安装时重新核对提取文件。安装器核对范围、归属和维护状态后，先确认 WebView2 已就绪再写产品文件。有 Standalone 时使用离线路径。当前用户优先使用已有机器运行时，也接受本账号运行时，并保护非提升安装范围。沿用 Windows 客户端基线已有的 Evergreen 依赖。[Microsoft WebView2 分发](https://learn.microsoft.com/microsoft-edge/webview2/concepts/distribution)

Win11 Sandbox 夹具通过已装运行时跳过、缺失时拒绝提升环境下的当前用户部署、坏摘要拒绝和清除失败后重试。用既有签名运行载荷和实际 Microsoft 输入编译的工程 Inno EXE，在线版 `30,860,133` 字节，离线版 `325,059,060` 字节。在线版在冻结 v3 更新器 256 MiB 约束内，离线版超出该上限。完整候选的 v3 自动桥接使用在线包尺寸约束，独立离线交付另列资产。Win10 缺失运行时的在线、离线和取消恢复仍需实际客户端取证。

隔离 Windows 10 22H2（构建 19045）的标准账号已验证系统 UAC 取消。探针经 Windows Shell 请求提升，安全桌面取消后返回 Win32 错误 `1223`，回执 SHA-256 为 `df242b888f6aa187897a946eb2110db7af3f8d9787e3936432f045bc3fddfc4b`。该回执覆盖系统提示和取消行为。完整安装器的取消恢复、正向交接与 WebView2 安装仍需客户端复验。

### 2026-11-27 Windows 10 22H2 隔离环境

用固定摘要的 Windows 10 Enterprise Evaluation 22H2 x64 镜像准备断网来宾。第一轮在初始化完成前关停，重启时 Windows Setup 报错。第二轮写入系统镜像后从同一虚拟盘继续以 TCG 启动，观察到首次设置的区域选择页面。页面截图 SHA-256 为 `79ac82a7e5018295369d0bb61cd922608337d72fef305badb7d4dd2ab010f62f`。关停后 QCOW2 结构检查无错误。按所有者许可暂缓人工账号设置及此来宾的完整实包验收，A-017 放行条件继续有效。

离线测试包由 `bffef439` 与同 SHA 的已验签运行清单重打包，使用固定摘要的 Microsoft WebView2 Standalone x64。离线包 SHA-256 为 `4cf26113aac667103d3dbd1652f0a515563d8387f02e77b7dca845c17d80bd17`，大小为 328,020,630 字节，用于手动离线交付验收。同源在线包 SHA-256 为 `3f39873e3dd05a197853d4426f5f1ccd59e171f24a1adbaeed46af474466950a`。两个版本的包和回执盘已就绪。Win10 上的断网拒绝、离线运行时安装及真实 WebView2 渲染均待运行。

### 2026-11-25 Ubuntu 产品 AppImage 首启

签名源码 `b324627eba5305457294bc4d921674535ead6dd8` 在隔离 Ubuntu 22.04 x86_64 构建七个 Release 程序，最高 glibc 符号版本为 `GLIBC_2.34`。合成密钥签名后的运行清单摘要为 `b82591144c327f87058905947ec9e12a3144aecb922422a708763fc948f99673`。用已固定摘要的 appimagetool 和 Type 2 runtime 生成 AppImage，文件 SHA-256 为 `e3c5e4dc60d39ca1966cd34180e05873a99b0f672b33357c2e6672b9fe381bdb`。

从含空格及 Unicode 的路径原生挂载 AppImage，实际 Tauri 窗口和 WebKit 渲染进程从持久目录运行并提供 Core IPC。登记和便携归属记录均为 `0600`，归属记录绑定实际外层文件摘要。独立 Rust SDK 在首次登记前拒绝启动，完成登记后的一次冷启动约为 0.98 秒。移动本体使外部启动失败，用户从新位置启动后用原有签名载荷重新绑定。再次用链接替换本体时拒绝冷启动。应用遇到含未知文件的初始资料目录时保留文件摘要，未创建运行目录或数据库。桌面运行使用隔离环境中现有的 WebKitGTK 系统依赖。可选 CLI 集成、干净桌面依赖范围、正式更新和卸载继续验收。

新建 `ALCOMD-M12-Jammy` 独立 WSL 2 实例。Ubuntu 22.04.5 x86_64 镜像 SHA-256 为 `4499c4fe257f2fc83145b429ce211a0a43fd590e70d6261ede616210947d9f8f`。用普通测试账号和上文同一 AppImage 实测 FUSE 挂载、Xvfb 下的实际窗口、WebKit 渲染进程及 Core IPC。

在同一实例安装此前从签名源码 `748933d8` 构建的 DEB。AppImage 拒绝使用 DEB 登记的资料目录，DEB Bootstrap 也拒绝接管 AppImage 的资料目录。两份登记和数据库摘要在拒绝场景和 `dpkg purge` 后保持一致。验证使用隔离的两套 XDG 资料目录。应用包移除后清理试验样本，保留已配置的隔离实例供后续 AppImage 更新验收。

### 2026-11-25 Windows 产品 Inno 双范围

Inno 只向对应范围的 PATH 添加稳定 `bin` 目录。它保留原值类型和用户已有条目，在卸载登记中记录本次添加的确切值。卸载时仅清理可唯一识别的自有条目，并保留安装后其他 PATH 改动。已有的等价条目不被接管。安装器变更结束时通知 Windows 刷新环境。

Windows 11 Sandbox 用 `e79db74f` 的原签名运行载荷和 Inno 6.5.4 验证此脚本。四个原生检查涵盖当前用户范围与原值类型、第三方改动保留、已有条目去重、初始无 PATH 值的恢复，以及全用户安装和卸载仅修改机器 PATH。测试 EXE 摘要为 `922e5073fa8ed640d58137eb41336f3072a70ca2f82d180695ed8d2863dd243f`。同范围更新和双向范围转换的 PATH 交接继续按阶段 2 与 3 验收。

开始菜单快捷方式按安装范围放置，AUMID 从产品清单派生，目标为同一安装的版本化 GUI。Inno 将快捷方式暂存在已拥有的程序目录，再以不覆盖已有文件的方式发布。卸载按登记的路径和摘要清理，保留原有和后来被替换的同名项。沿用显式 `/NOICONS` 选项。Windows 11 Sandbox 用同一签名载荷通过五项快捷方式检查与四项 PATH 复验，测试 EXE 摘要为 `e77057e86907980a996715767f6e7316669b302520880ae2fdf1d995b4cbfde9`。

从已经快进主仓的干净签名 SHA `62b6f562` 构建七个 Windows Release 程序并使用隔离密钥验签。Inno 6.5.4 生成的完整产品 EXE 为 `be81c9cb01e84afc50acbf7f2b2de5e20d7f649a7be26bd330ec8d6ccd15c810`，运行清单为 `fcefdefe51b5d761c2ae8ecb028556179677590351322ccbce3f00e918022ddc`。同一 EXE 在 Windows 11 Sandbox 通过前述九项系统入口检查，并复验稳定 CLI 冷启动、Core 登记路径和已安装 Updater 的下载命令，共 11 项。本机整仓门禁和 762 项 Chromium 与此 SHA 一致，四项 CodeQL 已通过。[Hosted CI](https://github.com/ALCOMD/ALCOMD/actions/runs/45540736904) · [CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/45540736352)

源码 `e79db74f6d989a8a2e7c4c4daafa3c110cedffc9` 的七个 Release 程序通过合成密钥验签。运行清单摘要为 `4d956522ef525d346c49851c00d37fd25edcc0c25d9e48f97d5aa72f46c069b1`，Inno 6.5.4 生成的 EXE 摘要为 `40b3d0cf35966f47db8fdea9711be2358906ccb18a809e16e9bf9f04f58a1b5a`。同一个 EXE 在不同 Windows 11 Sandbox 验证当前用户和全用户安装。

当前用户的普通账号通过 6 项检查，覆盖永久 AppId、重复安装拒绝、CLI 首启、独立 Rust SDK 保持 Core 在线时拒绝卸载、清单篡改时保留程序和登记。正常卸载注销本账号的登记，保留状态库摘要和安装根下新增的无关文件。早期测试驱动的单次 CLI 和事件订阅未为卸载探测持续持有连接，最终使用仓库外 Rust SDK。全用户范围通过 8 项，覆盖永久 AppId、阻止另一安装范围、避免初始化安装账号、篡改签名拒绝、普通账号首启、四个 CLI 约 1.3 秒连接同一个 Core、实际 WebView2 和独立程序使用登记。卸载保留各账号资料，使残留登记失效。

本轮使用已带 WebView2 的 Windows 11 默认安装目录。Windows 10、WebView2 在线和离线前置条件、两个范围之间的转换、自定义目录、桌面关联以及完整更新继续按发行矩阵验收。

### 2026-11-25 Windows 跨账号卸载

从干净签名源码 `72a85893bcd1f5cd0a9afb17421881095866a297` 构建七个 Windows Release 组件。合成密钥签名的运行清单 SHA-256 为 `d18b45bfa21acf8a1621cf2796c1a8256af702f691ef0faa5b9db8aebeef5848`。Inno 6.5.4 实包 SHA-256 为 `e4a2ab09360ec98cba8332f7339fe8c5ae9219c550cc894ab0e1414006f35228`。同一包用于两种范围的断网 Windows 11 Sandbox 验收。

安装的 Core 在读写状态前取得共享租约。Inno 在确认卸载后取得同一租约的独占访问，持有至卸载结束。全用户范围通过 6 项检查，涉及直接冷启动及目录联接启动的拒绝、两个普通账号各自持有独立 Rust SDK 连接、任一活跃账号阻止管理员卸载，以及正常卸载后的两份数据库和登记摘要对账。两份残留登记均无法启动已移除的 Core。

当前用户范围通过 6 项检查。标准账号通过首装、重复安装拒绝和 CLI 冷启动。独立 SDK 持有连接时拒绝卸载，运行清单被篡改时保留安装与登记。正常卸载注销本账号，数据库摘要与无关文件保持一致。Windows 10 和整产品更新另按发行矩阵复验。
