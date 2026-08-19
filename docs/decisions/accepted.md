# 已接受决策

| ID | 决策 |
|---|---|
| A-001 | 当前用户品牌为 ALCOMD3，产品家族为 ALCOMD，技术名称为 `alcomd`，系统标识为 `com.cqmhv.alcomd`，Windows AUMID 为 `CQMHV.ALCOMD` |
| A-002 | 技术名称根固定为 `alcomd` |
| A-003 | Bundle / Tauri identifier 固定为 `com.cqmhv.alcomd` |
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
| A-026 | MCP v4 工具名称草案作为正式 Schema 的命名基线；v3 的 33 个用例必须保持功能覆盖，但不要求与 v4 工具一对一对应，高影响操作优先拆分为 Plan/Apply，长任务使用 `OperationId`，正式 Schema 冻结后的重命名必须版本化或提供兼容别名。新增 `diagnostics.read`，与 `activity.read` 分离；它默认只返回脱敏诊断且默认不授予外部客户端，未来原始诊断导出使用独立权限。所有公开错误必须有稳定机器可读 code；未知内部错误返回 `internal_error` 与 `diagnostic_id`，敏感技术详情受 `diagnostics.read` 控制 |
| A-027 | M3 将外部 Unity 项目与本地/匿名 HTTP(S) VPM Repository 作为只读源；低影响 `register`/显式 `refresh`/`unregister` 只原子修改 `state.db` registry 与 last-known-good normalized cache，不创建 Operation。M3 冻结绝对路径与 opaque 文件身份、bounded parser、Schema v2、同步命令永久幂等、四项兼容 RPC capability、`projects.read/manage` 与 `repositories.read/manage`；远程 refresh 为 M3 blocker，但 credential、proxy、package payload、SemVer、Plan/Apply 和项目写入继续排除 |
| A-028 | M4 采用 durable immutable Plan 与显式 Apply；Apply 固定 source/revision/fingerprint/URL/SHA-256 且不得重新求解。Schema v3 增加 repository priority、resolver-ready snapshot、package plan 与 filesystem journal；首个文件事务只改 `Packages/<id>` 与 `vpm-manifest.json`，通过同卷 staging、write-ahead evidence、固定 commit/rollback phase 和 `PackageCache`/Project 锁证明可恢复。M4 remote archive 必须有 SHA-256，此限制不代表完整 VPM hashless/legacy/credential 兼容已完成；生产依赖仍需独立人工批准 |
| A-029 | M4 版本实现采用 `semver 1.0.28` 作为唯一 Version/precedence/build metadata model，不使用其 Cargo `VersionReq`/`Comparator`；`alcomd-vpm` 私有最小 range AST/parser 只实现冻结的 VPM vectors。批准 `zip 8.6.0`（defaults off，仅 `deflate-flate2-zlib-rs`）、`sha2 0.11.0`（defaults off）和 `unicode-normalization 0.1.25`（defaults off，仅 `std`）用于 bounded archive、SHA-256 和 path collision；不引入第二套版本模型、完整 VPM framework 或通用 parser/workflow abstraction |
