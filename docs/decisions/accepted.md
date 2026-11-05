# 已接受决策

| ID | 决策 |
|---|---|
| A-001 | 当前用户品牌为 ALCOMD3，产品家族为 ALCOMD，技术名称为 `alcomd`，发布者为 `VRCLearn`，系统标识为 `com.vrclearn.alcomd`，Windows AUMID 为 `vrclearn.ALCOMD`；组织身份修订见 A-040 |
| A-002 | 技术名称根固定为 `alcomd` |
| A-003 | Bundle / Tauri identifier 固定为 `com.vrclearn.alcomd` |
| A-004 | `alcomd` 是每用户唯一核心与唯一写入者 |
| A-005 | GUI、CLI、MCP、API 和扩展共用同一应用用例层 |
| A-006 | MCP 协议适配器 `alcomd-mcp` 独立于 GUI |
| A-007 | MCP 管理 GUI 作为第一方扩展 |
| A-008 | Discord Rich Presence 全部作为第一方扩展 |
| A-009 | 第一方扩展只能使用公开 Extension API |
| A-010 | v3、VCC 与旧 ALCOM 格式仅用于迁移或隔离导入 |
| A-011 | v3 成功迁移后执行可验证的零残留清理 |
| A-012 | 新仓库使用无关 Git 历史，最终以破坏性方式替换远端 main |
| A-013 | 已于 2026-08-15 发布的 ALCOMD3 3.4.0 是 v3 迁移入口版本（v3 migration entry release），也是进入 v4 的唯一直接迁移来源；更早的 v3.x 必须先升级到 3.4.0。3.4.0 已将更新 JSON 源切换到 `https://alcomd.cqmhv.com/api/v1/updates/stable.json` 与 `https://alcomd.cqmhv.com/api/v1/updates/beta.json` |
| A-014 | ALCOMD v4 自有代码、SDK、规范、文档、脚本及第一方扩展统一采用 `AGPL-3.0-only`，不自动授权后续版本，也不双重许可 |
| A-015 | ALCOMD v4 是独立新项目；v3 仅作迁移与行为参考，vrc-get 不是上游，禁止复制、移植、Fork、包装或改写二者源码；VPM 独立实现 |
| A-016 | 冻结版本 vrc-get 是 M-1 和 v4 功能对齐所必需的只读功能、安全行为、CLI 与错误处理基线；该基线不产生代码继承、依赖或源码复用关系 |
| A-017 | Windows x86_64、macOS Apple Silicon 与 Linux x86_64/amd64 三个平台，以及 Windows Inno Setup EXE、macOS DMG、Linux AppImage、Linux DEB 四种主要用户发行格式，全部是 4.0.0 Release Blocker；Windows 当前用户/所有用户安装是同一个 EXE 的两种模式，不是两个发行资产；最低系统版本、运行库与平台测试实现另行冻结 |
| A-018 | 4.0.0 优先交付 native RPC 与 TypeScript/Rust SDK；Loopback API、Python SDK 和 .NET SDK 后置，但初始公共合同不得阻碍以后兼容扩展 |
| A-019 | MCP 管理第一方扩展默认安装并启用；Discord 第一方扩展默认安装但新用户默认禁用；v3 升级用户迁移原 Discord 启用状态 |
| A-020 | GUI 必须保持用户入口、用例、数据结果、错误、进度与可访问性等价，但不要求像素级复刻；导航或视觉重构必须有冻结流程对照与人工截图签收 |
| A-021 | 4.0.0 不采用或广告 MCP Tasks；长任务返回 ALCOMD `OperationId`，并提供显式 query、input、approve、reject、resume 与 cancel 工具；未来只以兼容增加方式评估 Tasks |
| A-022 | Extension ABI v1 使用 WASI 0.2 Component Model，并通过版本化 WIT 定义兼容合同；运行时采用届时合适的 Wasmtime LTS，固定主版本线，同时允许且要求升级兼容的安全与关键正确性补丁；WASI 0.3 不阻塞 4.0.0，后续通过兼容层或 Extension ABI v2 评估，不直接破坏 ABI v1 |
| A-023 | MCP 管理权限使用 `mcp.requests.read`、`mcp.connections.read` 与 `mcp.subscription-streams.read`，不使用 `mcp.sessions.read`；HTTP Principal 按每请求 Bearer 身份隔离后端 RPC，STDIO 使用启动 Principal，自报 clientInfo 不参与安全决定 |
| A-024 | ALCOMD 是基于 Rust 的多组件本地应用平台，只有 `alcomd-gui` 是 Tauri 应用；正式发行由独立的全产品流水线收集、验证和打包全部 release-blocker 组件，并支持按当前发布策略执行平台签名。Windows 主安装器为单文件 Inno Setup EXE，默认当前用户安装并仅在选择所有用户安装时提权；macOS 主格式为 DMG；Linux 主格式为 AppImage 与 DEB。本决策不禁止 Tauri Bundler：Tauri/Tauri Bundler 可以构建 `alcomd-gui`，也可以参与适合的平台打包步骤，但只能作为受 `cargo xtask dist` 控制的底层工具，不定义产品组成、安装、更新或迁移生命周期 |
| A-025 | 4.0.0 平台技术基线：Windows x86_64 的正式测试范围为 Windows 10 22H2 与 Windows 11，不增加主动拒绝更旧 Windows 的应用级版本门槛；WebView2 使用 Evergreen Runtime，安装/更新先检测，缺失时支持 Microsoft 在线 Bootstrapper，并为离线安装提供 Standalone Installer 路径。Linux x86_64 以 Ubuntu 22.04 构建，发行二进制不得要求高于 `GLIBC_2.35` 的符号版本。macOS arm64 设置 `MACOSX_DEPLOYMENT_TARGET=11.0`，不增加额外应用级系统版本门槛。当前 Windows Authenticode 与 Apple Developer ID/notarization 均不是 4.0.0 blocker，允许未做平台代码签名、未公证的发行；但 v3 bridge、自动更新资产、Manifest 与第一方扩展的应用层签名/摘要验证不因此取消 |
| A-026 | MCP v4工具命名草案保留正式Schema基线；v3的33个用例按有效产品范围审计，高影响操作优先Plan/Apply，长任务使用OperationId。公开错误保留稳定code，internal_error仍使用有界diagnostic_id。2026-11-05所有者后续明确撤销日志/活动/提示系统，原diagnostics.read、activity.read及其日志重建授权已撤销，无兼容入口；任务、Event及其权限不因删除日志改变。 |
| A-027 | M3 将外部 Unity 项目与本地/匿名 HTTP(S) VPM Repository 作为只读源；低影响 `register`/显式 `refresh`/`unregister` 只原子修改 `state.db` registry 与 last-known-good normalized cache，不创建 Operation。M3 冻结绝对路径与 opaque 文件身份、bounded parser、Schema v2、同步命令永久幂等、四项兼容 RPC capability、`projects.read/manage` 与 `repositories.read/manage`；远程 refresh 为 M3 blocker，但 credential、proxy、package payload、SemVer、Plan/Apply 和项目写入继续排除 |
| A-028 | M4 采用 durable immutable Plan 与显式 Apply；Apply 固定 source/revision/fingerprint/URL/SHA-256 且不得重新求解。Schema v3 增加 repository priority、resolver-ready snapshot、package plan 与 filesystem journal；首个文件事务只改 `Packages/<id>` 与 `vpm-manifest.json`，通过同卷 staging、write-ahead evidence、固定 commit/rollback phase 和 `PackageCache`/Project 锁证明可恢复。M4 remote archive 必须有 SHA-256，此限制不代表完整 VPM hashless/legacy/credential 兼容已完成；生产依赖仍需独立人工批准 |
| A-029 | M4 版本实现采用 `semver 1.0.28` 作为唯一 Version/precedence/build metadata model，不使用其 Cargo `VersionReq`/`Comparator`；`alcomd-vpm` 私有最小 range AST/parser 只实现冻结的 VPM vectors。批准 `zip 9.1.3`（defaults off，仅 `deflate-flate2-zlib-rs`）、`sha2 0.11.0`（defaults off）和 `unicode-normalization 0.1.25`（defaults off，仅 `std`）用于 bounded archive、SHA-256 和 path collision；不引入第二套版本模型、完整 VPM framework 或通用 parser/workflow abstraction |
| A-030 | M5 CLI contract v1 使用 human 默认与互斥的 JSON/NDJSON machine mode，冻结 stdout/stderr、`0/1/2/3/130` 退出码、`--quiet/--yes/--dry-run/--no-wait`、非 TTY/EOF、Ctrl-C detach 与静态 completion；CLI 永远只经 `alcomd-client`/RPC，命令只有在 backend capability 真实实现时才发布 |
| A-031 | M5 采用顺序化 CLI、Unity、Template、Backup create、Backup restore slices。State Schema v4 只增加 Unity installation/project preference、template registry 与 backup metadata；Unity writer state 固定为 confirmed/suspected/not-observed/unknown，mutation 仅 confirmed hard reject，launch 原M5对 suspected/unknown fail closed；2026-11-05所有者批准的M7整改将启动、删除及Unity版本迁移限制为目标项目confirmed/suspected拦截，unknown不作全局禁令，已启动迁移的未知结果仍需恢复核实。新增窄权限 `unity.manage` 与 `projects.create`；in-place/overwrite/merge backup restore 不属于 M5 |
| A-032 | M5 Backup Restore 只把已注册并重新验证的 ALCOMD Backup Archive v1 恢复为不存在的全新 Unity Project。它使用 durable immutable Plan/Apply、预分配 ProjectId、`ProjectCreate(parent_identity, leaf)` 锁、同卷 sibling staging、atomic publish 与 publish 后 forward recovery；Schema v7 只增加 `backups.restore`、窄 `backup_restore_plans` 和 Restore 专用 append-only filesystem journal，不支持 overwrite/merge/legacy/arbitrary ZIP，也不隐式联网或 resolve package |
| A-033 | ALCOMD3 v4 official GUI 默认继承 ALCOMD3 v3 的用户模型与可识别结构：sidebar/main shell、导航深度和用户语义分组、信息密度、主要 workflow 空间关系，以及 Projects、packages/resources、Extensions、Settings、Log 等既有用户概念。v4 的默认目标是该用户结构加 Material Design 3 现代化和更可靠、安全、可观察的实现，而不是为 daemon、RPC、Operation、Extension Host、Portable UI、permission/scope、State Schema 等内部边界建立新产品领域。只有 v3 无对应用户用例、用户确需独立工作区、无法自然容纳于既有概念且独立入口明确提升可用性时，才考虑新增 user-level surface。Extensions 是 v3 已有用户概念；v4 runtime/permission/quarantine/UI 增强默认进入其既有用户归属。Material Design 3 现代化组件、交互和视觉语言，但不替代产品 IA；本决策进一步约束 A-020，不使 A-020 失效 |
| A-034 | 原批准范围（本次 State17 窄增量见 A-035）：保留数据库 Schema 版本机制，当时以 State v16 为开发基线；`system.hello.dataSchema` 必须与成功初始化的实际存储版本一致，不与应用版本或 RPC major 混用。未发布开发库不承诺长期兼容，仅对明确需要保留的数据维护迁移；可重建测试数据通过重建 fixture 适配，不能因此自动删除已有数据库、Plan、Operation 或恢复证据。首次正式发布前单独审计并收敛当时有效的初始 Schema，移除无保留需要的开发迁移和旧格式支持；正式发布后严格维护用户数据升级与恢复。当前不改回 v1、不新增版本、不删除现有迁移；清理为独立任务，公共合同变更仍需审批。 |
| A-035 | 2026-11-03 所有者批准 M7 全局默认 Unity 启动参数及实际约束所需的最小迁移。该次批准的权威版本为 State17/Config6；State17 在一个事务中只重建 `project_unity_launch_config`，将数组长度下限1改为0，保留全部旧行、列、外键、STRICT、其他约束及所有无关状态；不补造项目覆盖。Config5→6 保留 revision 和现有设置，增加 nullable `unity.defaultLaunchArguments`。这是 A-034 当时“不新增版本”的明确窄例外，不授权清理旧迁移、删除数据或扩大后续里程碑。合同见 `specs/storage/state-v17-empty-unity-override.md` 与 `specs/config/settings-v6.md`。 |
| A-036 | 2026-11-03 所有者批准窗口状态记忆最小设置合同。该次权威版本为 State17/Config7；Config7 仅增加默认 false 的 `window.rememberState`，沿用 settings.read/manage 与串行 CAS。Config1–6 有效迁移保留 revision/其他设置并补 false；窗口逻辑宽高与最大化仅为可丢弃 GUI 视图缓存，不进入 Core/State，窗口位置、最小化启动和全屏偏好不在范围内。不新增 RPC method、权限名或依赖。 |
| A-037 | 2026-11-03 所有者明确批准“项目 / 备份”最小合同：Config8 增加默认 true 的 projects.autoDiscover、fast/true/false 的 backups.compressionMode/includeVpmPackages/includeLibrary；旧有效配置保留 revision/全部原值并补默认。自动发现由 Core/application 启动/设置变更及30秒周期调度，只查默认项目根的直属安全子目录，每轮最多4096项且单在途，关闭/切换后停止旧轮登记；字段及触发扫描的路径修改附加现有 projects.manage。backups.create 支持省略选项使用权威默认并增加 includeLibrary；接受后固定全部选项，旧 Library=false 任务与指纹保留。State17、Archive v1 manifest/BackupRecord 形状、权限名及依赖保持，内容选择 profile 按合同修订。默认根内仅从列表移除的项目可再次发现，不增加排除清单。 |
| A-038 | 2026-11-03 所有者在已说明最小公开合同与数据库迁移成本后，以NanaZip截图明确选择压缩六档0/1/3/5/7/9：仅存储、极速、快速、标准、最大、极限。默认保持1；旧store/fast/maximum含义0/1/9保持，新增level3/level5/level7。Config9保留有效旧配置全部值和revision；State18仅扩展backups等级约束，保留原行、索引、约束及恢复计划外键；Archive v1同步扩展枚举，旧归档和已接受任务/指纹保留。设置及单次备份一致，不新增依赖、权限或公开方法，未执行真实工程备份、不清理已有数据、不提交或推送。 |
| A-039 | 2026-11-03 所有者批准跨启动保留全屏模式最小合同：Config10新增window.fullscreen布尔值，默认false，沿用settings.read/manage和串行CAS；有效Config1–9保留revision及全部已有设置，仅补false。窗口PATCH可独立更新rememberState/fullscreen，显式null、空对象、未知字段及非法类型整次拒绝；GUI主窗口薄适配器应用已接受偏好，窗口卡片恢复默认将两项置false并退出全屏。不新增RPC方法、State18迁移、权限或依赖，不修改已接受任务。 |
| A-040 | 2026-11-05 所有者明确批准首次公开发布前将个人身份统一为组织：发布者/自有作者及版权声明为VRCLearn，Bundle/Tauri/Linux desktop ID为com.vrclearn.alcomd，声明Windows AUMID为vrclearn.ALCOMD，当前网站/服务基址为https://alcomd.vrclearn.com/。同步第一方派生扩展ID、Windows IPC命名空间及v4 Schema引用；原A-001/A-003及ADR0001原位修订，不增加旧v4别名。v3冻结请求源/迁移输入、历史证据及真实机器身份保留；更新时必须成套停旧换新，禁止新旧Core对同一数据目录并行。Inno AppId及原生AUMID显式接入不由声明变更冒充完成，不新增RPC形状、权限、Schema版本或依赖。 |
