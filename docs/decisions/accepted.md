# 已接受决策

| ID | 决策 |
|---|---|
| A-001 | 当前用户品牌为 ALCOMD3，产品家族为 ALCOMD，技术名称为 `alcomd`，发布者为 `VRCLearn`，系统标识为 `com.vrclearn.alcomd`，Windows AUMID 为 `vrclearn.ALCOMD`；组织身份修订见 A-040 |
| A-002 | 技术名称根固定为 `alcomd` |
| A-003 | Bundle / Tauri identifier 固定为 `com.vrclearn.alcomd` |
| A-004 | `alcomd` 是每用户唯一核心与唯一写入者 |
| A-005 | GUI、CLI、MCP、API 和扩展共用同一应用用例层 |
| A-006 | MCP 协议适配器 `alcomd-mcp` 独立于 GUI |
| A-007 | GUI 在原生设置页「客户端」管理连接，CLI 与 MCP 调用相同的 Core 用例 |
| A-008 | Discord Rich Presence、协议和连接生命周期由独立仓库中的第一方扩展实现。扩展作为第三方示例，使用与第三方相同的应用无关的公开接口。见 [ADR 0013](../adr/0013-discord-first-party-extension.md) |
| A-009 | 第一方扩展只能使用公开 Extension API |
| A-010 | 旧配置仅用于迁移或隔离导入。仓库清单支持显式选择 VCC TXT 交换格式 |
| A-011 | v3 成功迁移后执行可验证的零残留清理 |
| A-012 | 新仓库使用无关 Git 历史，最终以破坏性方式替换远端 main |
| A-013 | 已于 2026-08-15 发布的 ALCOMD3 3.4.0 是 v3 迁移入口版本（v3 migration entry release），也是进入 v4 的唯一直接迁移来源；更早的 v3.x 必须先升级到 3.4.0。3.4.0 已将更新 JSON 源切换到 `https://alcomd.cqmhv.com/api/v1/updates/stable.json` 与 `https://alcomd.cqmhv.com/api/v1/updates/beta.json` |
| A-014 | ALCOMD v4 自有代码、SDK、规范、文档、脚本及第一方扩展统一采用 `AGPL-3.0-only`，不自动授权后续版本，也不双重许可 |
| A-015 | ALCOMD v4 是独立新项目；v3 仅作迁移与行为参考，vrc-get 不是上游，禁止复制、移植、Fork、包装或改写二者源码；VPM 独立实现 |
| A-016 | 冻结版本 vrc-get 是 M-1 和 v4 功能对齐所必需的只读功能、安全行为、CLI 与错误处理基线；该基线不产生代码继承、依赖或源码复用关系 |
| A-017 | Windows x86_64、macOS Apple Silicon 与 Linux x86_64/amd64 三个平台，以及 Windows Inno Setup EXE、macOS DMG、Linux AppImage、Linux DEB 四种主要用户发行格式，全部是 4.0.0 Release Blocker；Windows 当前用户/所有用户安装是同一个 EXE 的两种模式，不是两个发行资产；最低系统版本、运行库与平台测试实现另行冻结 |
| A-018 | 4.0.0 优先交付 native RPC 与 TypeScript/Rust SDK；Loopback API、Python SDK 和 .NET SDK 后置，但初始公共合同不得阻碍以后兼容扩展 |
| A-019 | 客户端管理由 Core 提供，GUI 入口为设置页「客户端」，三端共用用例。Discord 第一方扩展默认安装但新用户默认禁用；v3 升级用户迁移原 Discord 启用状态 |
| A-020 | GUI 必须保持用户入口、用例、数据结果、错误、进度与可访问性等价，但不要求像素级复刻；导航或视觉重构必须有冻结流程对照与人工截图签收 |
| A-021 | 4.0.0 不采用或广告 MCP Tasks；长任务返回 ALCOMD `OperationId`，并提供显式 query、input、approve、reject、resume 与 cancel 工具；未来只以兼容增加方式评估 Tasks |
| A-022 | Extension ABI v1 使用 WASI 0.2 Component Model，并通过版本化 WIT 定义兼容合同；运行时采用届时合适的 Wasmtime LTS，固定主版本线，同时允许且要求升级兼容的安全与关键正确性补丁；WASI 0.3 不阻塞 4.0.0。首次发布前所有获准的 WIT 改动原位纳入 ABI 1，同步 host、guest 和测试 |
| A-023 | MCP 管理权限使用 `mcp.requests.read`、`mcp.connections.read` 与 `mcp.subscription-streams.read`；HTTP Principal 按每请求 Bearer 身份隔离后端 RPC，STDIO 使用启动 Principal，自报 clientInfo 不参与安全决定 |
| A-024 | ALCOMD 是基于 Rust 的多组件本地应用平台，只有 `alcomd-gui` 是 Tauri 应用；正式发行由独立的全产品流水线收集、验证和打包全部 release-blocker 组件，并支持按当前发布策略执行平台签名。Windows 主安装器为单文件 Inno Setup EXE，默认当前用户安装并仅在选择所有用户安装时提权；macOS 主格式为 DMG；Linux 主格式为 AppImage 与 DEB。本决策不禁止 Tauri Bundler：Tauri/Tauri Bundler 可以构建 `alcomd-gui`，也可以参与适合的平台打包步骤，但只能作为受 `cargo xtask dist` 控制的底层工具，不定义产品组成、安装、更新或迁移生命周期 |
| A-025 | 4.0.0 平台技术基线：Windows x86_64 的正式测试范围为 Windows 10 22H2 与 Windows 11，不增加主动拒绝更旧 Windows 的应用级版本门槛；WebView2 使用 Evergreen Runtime，安装/更新先检测，缺失时支持 Microsoft 在线 Bootstrapper，并为离线安装提供 Standalone Installer 路径。Linux x86_64 以 Ubuntu 22.04 构建，发行二进制不得要求高于 `GLIBC_2.35` 的符号版本。macOS arm64 设置 `MACOSX_DEPLOYMENT_TARGET=11.0`，不增加额外应用级系统版本门槛。当前 Windows Authenticode 与 Apple Developer ID/notarization 均不是 4.0.0 blocker，允许未做平台代码签名、未公证的发行；但 v3 bridge、自动更新资产、Manifest 与第一方扩展的应用层签名/摘要验证不因此取消 |
| A-026 | MCP v4工具命名草案保留正式Schema基线；v3的33个用例按有效产品范围审计，高影响操作优先Plan/Apply，长任务使用OperationId。公开错误保留稳定code，internal_error仍使用有界diagnostic_id。2026-11-05所有者后续明确撤销日志/活动/提示系统，原diagnostics.read、activity.read及其日志重建授权已撤销，无兼容入口；任务、Event及其权限不因删除日志改变。 |
| A-027 | M3 将外部 Unity 项目与本地/匿名 HTTP(S) VPM Repository 作为只读源；低影响 `register`/显式 `refresh`/`unregister` 只原子修改 `state.db` registry 与 last-known-good normalized cache，不创建 Operation。M3 冻结绝对路径与 opaque 文件身份、bounded parser、同步命令永久幂等、四项兼容 RPC capability、`projects.read/manage` 与 `repositories.read/manage`；远程 refresh 为 M3 blocker，但 credential、proxy、package payload、SemVer、Plan/Apply 和项目写入继续排除 |
| A-028 | M4 采用 durable immutable Plan 与显式 Apply；Apply 固定 source/revision/fingerprint/URL/SHA-256 且不得重新求解。State1 保存 repository priority、resolver-ready snapshot、package plan 与 filesystem journal；文件事务通过同卷 staging、write-ahead evidence、固定 commit/rollback phase 和 `PackageCache`/Project 锁保持可恢复。M7 修订允许 HTTP(S) 包缺省或提供格式无效的发布方摘要，Core 验证并固定首次取得的 SHA-256，发布方提供的有效摘要继续严格比对。Core 保存绑定，作者邮箱可缺省；生产依赖仍需独立人工批准 |
| A-029 | M4 版本实现采用 `semver 1.0.28` 作为唯一 Version/precedence/build metadata model，不使用其 Cargo `VersionReq`/`Comparator`；`alcomd-vpm` 私有最小 range AST/parser 只实现冻结的 VPM vectors。批准 `zip 9.1.3`（defaults off，仅 `deflate-flate2-zlib-rs`）、`sha2 0.11.0`（defaults off）和 `unicode-normalization 0.1.25`（defaults off，仅 `std`）用于 bounded archive、SHA-256 和 path collision；不引入第二套版本模型、完整 VPM framework 或通用 parser/workflow abstraction |
| A-030 | M5 CLI contract v1 使用 human 默认与互斥的 JSON/NDJSON machine mode，冻结 stdout/stderr、`0/1/2/3/130` 退出码、`--quiet/--yes/--dry-run/--no-wait`、非 TTY/EOF、Ctrl-C detach 与静态 completion；CLI 永远只经 `alcomd-client`/RPC，命令只有在 backend capability 真实实现时才发布 |
| A-031 | M5 采用顺序化 CLI、Unity、Template、Backup create、Backup restore slices。Unity writer state 固定为 confirmed/suspected/not-observed/unknown，mutation 仅 confirmed hard reject，launch 原M5对 suspected/unknown fail closed；2026-11-05所有者批准的M7整改将启动、删除及Unity版本迁移限制为目标项目confirmed/suspected拦截，unknown不作全局禁令，已启动迁移的未知结果仍需恢复核实。新增窄权限 `unity.manage` 与 `projects.create`；in-place/overwrite/merge backup restore 不属于 M5 |
| A-032 | 普通 ZIP 恢复从 Core 主机上的归档读取，目标目录须满足当前恢复合同。Core 校验归档与目标、安全发布、恢复与任务归属。[项目 ZIP](../../specs/backups/project-zip.md) |
| A-033 | ALCOMD3 v4 official GUI 默认继承 ALCOMD3 v3 的用户模型与可识别结构：sidebar/main shell、导航深度和用户语义分组、信息密度、主要 workflow 空间关系，以及 Projects、packages/resources、Extensions、Settings、Log 等既有用户概念。v4 的默认目标是该用户结构加 Material Design 3 现代化和更可靠、安全、可观察的实现，而不是为 daemon、RPC、Operation、Extension Host、Portable UI、permission/scope、State Schema 等内部边界建立新产品领域。只有 v3 无对应用户用例、用户确需独立工作区、无法自然容纳于既有概念且独立入口明确提升可用性时，才考虑新增 user-level surface。Extensions 是 v3 已有用户概念；v4 runtime/permission/quarantine/UI 增强默认进入其既有用户归属。Material Design 3 现代化组件、交互和视觉语言，但不替代产品 IA；本决策进一步约束 A-020，不使 A-020 失效 |
| A-034 | 首发数据库由完整 DDL 在空目录直接创建。已存在数据库先校验文件、版本、全部表、索引与约束，配置按 Config1 完整结构校验。失败时保全源数据。system.hello 报告实际初始化的 State1 和 Config1。 |
| A-035 | Unity 全局默认启动参数为可空参数数组。项目启动参数可独立覆盖，空数组表示明确覆盖为空。设置通过 revision CAS 保存。[Config1](../../specs/config/settings-v1.md) |
| A-036 | 窗口设置 rememberState 默认关闭。窗口宽高和最大化属于可丢弃的 GUI 本地视图状态。Core 通过设置 CAS 保存用户偏好。 |
| A-037 | 自动发现默认开启。Core 在启动、设置变更和周期扫描时检查默认项目根的直属安全子目录，每轮最多 4096 项且只运行一轮。关闭或切换后停止原轮登记。自动发现和项目目录变更由 Core 验证相应权限。备份选项以当前配置为默认，在任务接受时固定。 |
| A-038 | 普通 ZIP 备份使用 0、1、3、5、7、9 六档压缩，首次使用默认等级 5。单次显式选项优先于保存偏好。 |
| A-039 | window.fullscreen 默认关闭。设置更新允许独立设置 rememberState 和 fullscreen，Core 校验字段类型及 revision。GUI 应用保存成功的偏好，恢复默认将两项置为 false 并退出全屏。 |
| A-040 | 2026-11-05 所有者明确批准首次公开发布前将个人身份统一为组织：发布者/自有作者及版权声明为VRCLearn，Bundle/Tauri/Linux desktop ID为com.vrclearn.alcomd，声明Windows AUMID为vrclearn.ALCOMD，当前网站/服务基址为https://alcomd.vrclearn.com/。同步第一方派生扩展ID、Windows IPC命名空间及v4 Schema引用；原A-001/A-003及ADR0001原位修订，不增加旧v4别名。v3冻结请求源/迁移输入、历史证据及真实机器身份保留；更新时必须成套停旧换新，禁止新旧Core对同一数据目录并行。Inno AppId及原生AUMID显式接入不由声明变更冒充完成，不新增RPC形状、权限、Schema版本或依赖。 |
| A-041 | GUI、CLI 和认证后的 MCP 客户端通过相同 Core 用例执行同一业务操作。Core 记录实际调用者并校验认证、撤销、资源和任务归属。扩展按公开权限及沙箱执行，扩展 projects.read 限于获准的安全摘要。 |
| A-042 | M10 原生 RPC 以系统账号为信任边界。官方及外部原生 GUI 和 CLI 使用同一按用户隔离的端点与本机所有者权限，无配对、应用凭据或逐应用撤销。Core 对两者执行相同的业务验证。MCP 使用连接凭据和真实 Principal 取得相同业务权限，扩展使用各自的授权。 |
