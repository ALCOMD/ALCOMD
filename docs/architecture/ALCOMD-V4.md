# ALCOMD3 v4.0.0 完整架构、扩展与无残留迁移方案

状态：**Accepted as product direction，implementation pending**

## 1. 定位

ALCOMD is a Rust-based local application platform with a Tauri-powered official GUI.

ALCOMD 是一个基于 Rust 的本地应用平台，其官方 GUI 使用 Tauri 构建。它不是以 GUI 为中心的
单体程序，也不是一个由 Tauri 定义产品边界的应用。

ALCOMD3 v4 在品牌和功能定位上继承 ALCOMD3 v3，但拥有独立 Git 历史与全新代码库。
它不是 v3 的增量修改或派生代码库。v3 只作为迁移与功能审计的只读输入；vrc-get
不是代码上游。v4 不复制、移植、Fork、包装或改写两者源码，VPM 能力依据公开格式、
生态兼容需求和 ALCOMD 自有规范独立实现。

ALCOMD v4 自有代码、SDK、规范、文档、脚本与第一方扩展统一采用
`AGPL-3.0-only`；第三方依赖和资产继续使用各自许可证。

`alcomd` 是唯一核心进程。`alcomd-gui`、`alcomd-cli`、`alcomd-mcp`、Local API、第三方 GUI、第三方应用和扩展都通过统一协议调用同一套应用用例。

```text
用户与外部入口
├─ alcomd-gui
├─ alcomd-cli
├─ alcomd-mcp
├─ alcomd-api
├─ 第三方 GUI
└─ 其他应用
        │
        ▼
ALCOMD RPC v1
        │
        ▼
alcomd
├─ Commands / Queries / Events
├─ Operations / Approvals / Resource Locks
├─ Projects / Packages / Repositories / Templates
├─ Unity / Backups / Extensions / Updates
├─ Settings / Activity / Access
└─ Storage / Platform / VPM ports
        │
        ├─ state.db
        ├─ settings.toml
        ├─ object store
        ├─ recovery journal
        └─ OS credential store
```

## 2. 永久身份

| 层级 | 值 | 是否允许随品牌变化 |
|---|---|---:|
| 当前用户品牌 | `ALCOMD3` | 是 |
| 产品家族 | `ALCOMD` | 否 |
| 技术名称根 | `alcomd` | 否 |
| Bundle / Tauri identifier | `com.cqmhv.alcomd` | 否 |
| Windows AUMID | `CQMHV.ALCOMD` | 否 |
| URI Scheme | `alcomd://` | 否 |
| 数据目录 | `ALCOMD` | 否 |

用户品牌未来变化时，只修改展示名称、图标与文案。程序名、Bundle ID、安装身份、数据路径、RPC、扩展 API、SDK 与第三方授权不得改变。

## 3. 程序组成

ALCOMD 是一个 Rust Workspace 和多组件本地应用平台。`alcomd-gui` 是唯一使用 Tauri 构建的
官方 GUI 子应用；Tauri 不定义完整产品的组件、安装、更新或迁移生命周期。

| 程序 | 职责 |
|---|---|
| `alcomd` | 每用户唯一核心、唯一状态持有者和唯一写入者 |
| `alcomd-gui` | 官方 GUI、Portable Extension UI renderer、审批与操作中心 |
| `alcomd-cli` | 完整可自动化命令行客户端 |
| `alcomd-mcp` | 独立 MCP 协议适配器 |
| `alcomd-api` | 可选 Loopback HTTP 网关 |
| `alcomd-extension-host` | 第一方与第三方后台扩展沙箱宿主 |
| `alcomd-bootstrap` | 安装、更新、迁移、替换与卸载协调器 |
| `alcomd-updater` | 下载并验证完整产品更新，调用 bootstrap 完成原子替换 |
| `alcomd-migrate-v3` | 只在 v3 升级期间释放的一次性迁移程序 |

`alcomd-migrate-v3` 不进入普通 v4 安装结果。

安装器部署完整 ALCOMD 产品，而不是孤立的 `alcomd-gui`。正式发行由未来的
`cargo xtask dist --target <target>` 收集、验证并打包全部 release-blocker 组件，并按发布策略
执行可选平台代码签名和强制的 ALCOMD 应用层签名/摘要生成。

## 4. 进程模型

- `alcomd` 每个操作系统用户只运行一个实例。
- 用户安装和全局安装都使用用户级核心。
- 第一个客户端可以按需启动核心。
- 没有客户端、任务、API 监听或后台扩展租约时，核心可以退出。
- 不默认注册为 Windows 系统服务。
- 普通项目操作不以管理员身份运行。
- 更新或机器级安装操作由外部 `alcomd-bootstrap` 单独提权。

建议 IPC：

```text
Windows: \\.\pipe\alcomd.rpc.<user-sid>
Unix:   $XDG_RUNTIME_DIR/alcomd/rpc.sock
```

## 5. 唯一写入者

只有 `alcomd` 可以：

- 写入 SQLite。
- 修改 Unity 项目和 `vpm-manifest.json`。
- 安装、移除、升级或降级包。
- 修改仓库、模板和备份状态。
- 修改扩展安装与权限。
- 修改外部客户端授权。
- 写入权威设置。
- 执行应用更新。

GUI、CLI、MCP、API 和扩展只能提交命令或查询。

## 6. 分层与依赖方向

```text
Adapters / Apps
        │
        ▼
alcomd-application
        │
        ▼
alcomd-domain

alcomd-store / alcomd-platform / alcomd-vpm / alcomd-extensions
        └─ 实现 application 定义的 ports
```

禁止：

```text
GUI -> SQLite / VPM implementation / project files
CLI -> SQLite / VPM implementation / project files
MCP -> SQLite / VPM implementation / project files
Extension -> SQLite / project files / private Tauri commands
Domain -> Tauri / SQLite / HTTP / MCP / OS APIs
```

## 7. Command、Query、Event 与 Operation

- `Command`：可能修改状态。
- `Query`：只读。
- `Event`：资源与操作状态变化。
- `Operation`：可观察、可取消、可恢复的长任务。

操作状态：

```text
queued
planning
waiting_for_input
running
cancelling
succeeded
failed
cancelled
interrupted
recovering
```

长操作立即返回 `OperationId`。客户端关闭不会默认取消操作。

## 8. Plan / Apply

高影响操作必须先计划后应用：

```text
Plan
├─ 读取最新状态
├─ 解析依赖与冲突
├─ 检查权限与兼容性
└─ 生成可审查 ChangeSet

Apply
├─ 重新验证计划
├─ 获取资源锁
├─ 执行事务
├─ 校验结果
└─ 发布事件
```

适用范围包括包变更、项目迁移、备份恢复、扩展安装、权限变更、外部配置修改和应用更新。

## 9. 资源锁

建议资源：

```text
GlobalUpdate
Environment
RepositoryCatalog
PackageCache
Project(project_id)
ProjectBackup(project_id)
ProjectRestore(target_path)
Template(template_id)
Extension(extension_id)
ExternalConfig(provider_id)
```

同一项目写操作串行，不同项目可并行。扩展不得实现绕过核心的锁。

## 10. 包事务

```text
1. 读取项目当前状态
2. 解析依赖并生成 ChangeSet
3. 检查 Unity 与包兼容性
4. 获取项目写锁
5. 下载到 staging
6. 验证声明哈希与实际哈希
7. 拒绝路径穿越、绝对路径和符号链接逃逸
8. 解压到 staging
9. 写入恢复日志
10. 原子应用文件变更
11. 更新 vpm-manifest.json
12. 校验最终状态
13. 提交事务
14. 广播事件
```

中断后必须恢复或回滚，不允许半安装状态。

## 11. ALCOMD RPC v1

官方组件使用 Named Pipe 或 Unix Domain Socket。消息采用 JSON-RPC 风格语义、长度前缀帧、双向事件、请求 ID、Trace ID、幂等键、Schema 与能力协商。

连接首先调用 `system.hello`：

```json
{
    "rpcVersion": 1,
    "client": {
        "name": "alcomd-gui",
        "version": "4.0.0",
        "instanceId": "..."
    },
    "capabilities": [
        "events",
        "operations",
        "interactive-approval"
    ]
}
```

核心返回：

```json
{
    "rpcVersion": 1,
    "daemonVersion": "4.0.0-alpha.0",
    "capabilities": []
}
```

M2 在 state store 成功初始化并完成恢复后，兼容增加可选 `dataSchema: 1`；它不是 RPC
capability，也不属于 M1 客户端必读字段。`configSchema` 与 `extensionApi` 仍未实现，未来只有
对应子系统真实存在后才可按照 RPC v1 的兼容增加规则作为可选字段加入。

公共 DTO 与内部领域对象必须分离。破坏性变化提升对应协议大版本，而不是仅提升应用版本。

## 12. 第三方应用通信

4.0.0 优先提供本地 RPC 与 TypeScript/Rust SDK。可选 Loopback API、Python SDK 和 .NET SDK
后置到 4.0.0 之后；初始公共合同必须保留以后兼容增加这些入口的空间，但不得为未实现入口
扩大当前发行或安全范围。

首次配对：

```text
请求客户端身份与权限
    -> 用户通过 GUI 或 CLI 审批
    -> 核心签发独立凭据
    -> 用户可查看和撤销
```

每个客户端独立身份，不共享万能 token。

后续可选 `alcomd-api`：

- 默认关闭。
- 只监听 `127.0.0.1` 与 `::1`。
- 动态端口。
- 校验 Host 与 Origin。
- 防止 DNS Rebinding。
- 独立客户端授权。
- 所有调用审计。
- token 进入操作系统凭据库。

## 13. 官方 GUI

`alcomd-gui` 是 ALCOMD 中唯一使用 Tauri 构建的官方 GUI 子应用。Tauri 负责窗口、WebView 和
GUI 可执行文件；`tauri build` 或 `npm run gui:build` 只验证/生成 GUI，不是完整产品发行命令。

技术栈：

```text
Tauri 2
React
Vite
TypeScript
Material Web
Material Design 3
```

GUI 内置核心页面：

```text
项目
包
仓库
模板
Unity
备份
操作中心
扩展管理
外部访问管理
通用设置
活动记录
技术日志
```

GUI 迁移要求用户入口、用例、数据结果、错误、进度和可访问性等价，不要求像素级复刻。
导航或视觉重构必须有冻结流程对照与人工截图签收，不能用外观变化掩盖功能或状态缺失。

Tauri command 只能用于窗口、文件选择器、系统通知和 typed RPC 桥接。Portable Extension UI 仍经同一
ALCOMD RPC/application 边界，不使用 extension private command。Tauri command 不得实现包、项目、MCP、Discord
或迁移业务。

Tauri Bundler 可在某个平台作为 `cargo xtask dist` 的受控底层工具，但不得决定产品组成、
安装布局、更新事务、迁移流程或最终发行资产集合。这不是禁用 Tauri Bundler；它可以参与
适合的平台打包步骤，只是不能成为整个 ALCOMD 产品的安装框架或生命周期权威。

权威设置通过 RPC 存储；localStorage 仅保存可丢失的临时视图状态。

## 14. CLI

`alcomd-cli` 必须覆盖全部核心用例，并支持：

```text
--json
--ndjson
--quiet
--yes
--dry-run
--wait
--detach
--no-progress
--no-start-daemon
```

stdout 只输出结果，stderr 输出日志和进度。非 TTY 环境不得突然进入交互模式。

## 15. 扩展平台

第一方和第三方扩展必须使用同一套：

- `.alcomdext`
- Extension Manifest
- Extension API
- Portable Extension UI contract
- 权限
- 数据命名空间
- WASM/WASI 后台宿主
- 沙箱
- 安装、更新与崩溃隔离

区别只允许是发行者、签名、默认安装策略与官方支持等级。

ALCOMD Extension 是 Core Extension，不是 `alcomd-gui` plugin。Extension Runtime compatibility 与
Extension UI compatibility 相互独立。Extension ABI v1 使用 WASI 0.2 Component Model 和版本化 WIT；后台运行在
`alcomd-extension-host`。运行时采用实现时合适的 Wasmtime
LTS 并固定其主版本线，兼容的安全与关键正确性补丁必须升级。WASI 0.3 不阻塞 4.0.0，后续
只通过兼容层或 Extension ABI v2 评估，不直接破坏 ABI v1。禁止原生 DLL、`.so` 与 `.dylib`
扩展。

ADR 0024 接受基础 Extension UI 的 GUI-neutral Portable UI Surface 方向。扩展只描述 bounded semantic UI tree、state 与 typed
action，不携带 HTML、CSS、JavaScript 或 GUI framework code。GUI Host 通过自己的原生组件渲染：

```text
.alcomdext
    -> Extension Host
    -> Portable UI Surface
    -> alcomd application
    -> ALCOMD RPC v1
        -> alcomd-gui React / Material Design 3 renderer
        -> third-party GUI renderer
        -> headless conformance client
```

`alcomd-gui` 不拥有 Extension Runtime，Portable UI 不依赖 Tauri，第三方 GUI 不需要加载 React、HTML 或扩展网页。
GUI 不支持某个 Surface 时只影响扩展功能页面，不影响安装、启用、后台运行、权限、数据或生命周期。官方 GUI 的
扩展页面位于主窗口内容区并统一使用 React/Material Design 3、主题和可访问性体系；不打开额外窗口，不创建
iframe、child WebView 或 WebviewWindow，也不向扩展授予 Tauri capability。Custom Web UI 和 GUI-specific
surface 不属于 4.0.0；M8/M9 第一方扩展与第三方扩展使用同一 Portable UI 合同。

系统能力必须是窄接口，例如：

```text
host.network.request
host.external-config.plan
host.external-config.apply
host.discord-presence.connect
host.discord-presence.update
host.discord-presence.clear
host.notification.send
```

任何新能力都必须有公开权限、Schema、威胁模型与审计。

## 16. MCP

MCP 分为：

1. 独立协议适配器 `alcomd-mcp`。
2. MCP 管理第一方扩展。

`alcomd-mcp` 必须在 GUI 未运行时工作，并支持 STDIO 与当前规范要求的 HTTP 传输。

MCP 协议基线已冻结为 `2026-07-28`。实施时必须遵守其无会话、按请求携带版本与能力、`server/discover` 和订阅流设计，不得照搬旧版初始化握手或 `Mcp-Session-Id`。

ALCOMD 长任务通过核心 `OperationId` 表达。4.0.0 不采用或广告 MCP Tasks；公开工具提供
query、input、approve、reject、resume 与 cancel。未来只能以兼容增加方式评估 Tasks，不得把
旧实验性 Tasks API 当成稳定核心协议。

MCP 管理扩展 ID：

```text
com.cqmhv.alcomd.extension.mcp
```

它负责客户端、请求、连接、订阅流、权限、审批、配置计划、操作进度与诊断可视化。禁用它
不得影响 MCP 协议服务。HTTP Principal 按每请求 Bearer 身份隔离；STDIO 使用启动 Principal；
自报 clientInfo 不参与安全决定。管理权限使用 `mcp.requests.read`、`mcp.connections.read` 与
`mcp.subscription-streams.read`，不得重新引入协议级 `mcp.sessions.read`。

MCP 管理扩展默认安装并启用。

## 17. Discord

Discord 功能全部作为第一方扩展：

```text
com.cqmhv.alcomd.extension.discord
```

扩展包含设置/预览 UI 与 WASM 后台。它通过公开权限 `integrations.discord.presence` 调用 Extension Host 的窄 Discord Presence 能力。

禁止：

- 读取 Discord token。
- 读取 Discord 用户数据库。
- 模拟用户操作。
- 控制其他应用的 Presence。
- 任意访问本地命名管道。

禁用或卸载扩展必须立即清除 Presence 并停止 Discord 通信。GUI 关闭后，扩展可在受控后台租约下继续运行。
Discord 扩展默认安装但新用户默认禁用；v3 升级用户迁移原有启用状态。

## 18. 数据规范

Windows 用户数据：

```text
%LOCALAPPDATA%\ALCOMD\
├─ config\settings.toml
├─ data\state.db
├─ data\objects\
├─ data\extensions\
├─ data\recovery\
├─ cache\
├─ logs\
└─ runtime\
```

安装目录：

```text
当前用户  %LOCALAPPDATA%\Programs\ALCOMD\
所有用户  %ProgramFiles%\ALCOMD\

ALCOMD\
├─ bin\
│  └─ alcomd-cli.exe
└─ runtime\
   ├─ alcomd.exe
   ├─ alcomd-gui.exe
   ├─ alcomd-mcp.exe
   ├─ alcomd-extension-host.exe
   ├─ alcomd-bootstrap.exe
   ├─ alcomd-updater.exe
   └─ 其他内部组件
```

Windows 仅将 `ALCOMD\bin` 写入对应 scope 的 PATH。不得把完整安装目录加入 PATH，也不得把
核心、GUI、MCP、扩展宿主、bootstrap 或 updater 暴露为普通终端命令。更新不得重复添加 PATH；
scope 转换和卸载只移除安装器创建的精确条目。

默认用户目录：

```text
Documents\ALCOMD\Projects
Documents\ALCOMD\Backups
```

macOS 与 Linux 分别遵循 Application Support 和 XDG 规范，但稳定目录名仍为 `ALCOMD` /
`alcomd`。macOS DMG 中的 `ALCOMD.app` 包含完整产品，CLI 集成只能由用户在 GUI 中主动安装，
不得在拖入 Applications 时修改 shell 配置。Linux DEB 把公开 CLI 安装到
`/usr/bin/alcomd-cli`，其他程序进入包管理器目录；AppImage 的可选 CLI 集成可创建
`~/.local/bin/alcomd-cli`，但不能假设 AppImage 本体路径固定。

`settings.toml` 保存可理解的公开设置。`state.db` 是内部权威状态，不是第三方 API。凭据进入 OS Credential Store / Keychain / Secret Service。

版本独立：

```text
应用版本
RPC API
数据 Schema
配置 Schema
扩展 Manifest
扩展 API
导出格式
MCP 工具集
```

## 19. VCC 与 VPM

v4 正常运行时不读写 VCC `settings.json`，不与 VCC 双向同步，不使用 VCC LiteDB 作为内部数据库。

继续支持：

- VPM repository JSON。
- VPM package manifest。
- `Packages/vpm-manifest.json`。
- VPM 依赖解析。
- 用户 VPM 包。
- Unity 项目结构。
- 可选 `vcc://` 生态导入输入。

`vcc://` 进入后立即转换为内部请求，不能污染内部模型。

## 20. v3 到 v4 迁移

迁移原则：

```text
失败：v3 完整可恢复，v4 staging 删除。
成功：v4 完整可用，已确认属于 v3 的资源全部删除。
```

迁移入口固定为已于 2026-08-15 发布的 ALCOMD3 3.4.0：

- 3.4.0 是 v3 迁移入口版本（v3 migration entry release），也是 v4 迁移链唯一接受的直接来源。
- 3.4.0 不负责执行完整 v4 替换迁移；它只发现、验证并启动签名的 ALCOMD v4 bridge
  installer。
- 更早的公开 v3.x 必须先通过原有更新链升级到 3.4.0，不得直接进入 v4 迁移。
- 3.4.0 已将更新 JSON 源切换到 `https://alcomd.cqmhv.com/api/v1/updates/stable.json` 与 `https://alcomd.cqmhv.com/api/v1/updates/beta.json`。
- ALCOMD v4 bridge installer 安装完整 v4 产品，包含或调用 `alcomd-bootstrap`；bootstrap 再
  协调临时 `alcomd-migrate-v3`、健康检查、回滚、v3 清理与 Commit。

上述已上线路径和频道映射作为 M-1 基线；v4 迁移桥接安装器的 JSON Schema、版本推进、签名验证和错误行为在 M-1 冻结。

Windows Inno Setup 脚本只负责产品部署、系统集成和调用 bootstrap，不解析 v3 数据库、不修改
Unity 项目，也不实现业务迁移。

阶段：

```text
Inventory
Freeze
Export
Build Native State
Move Owned User Folders
Install v4 Identity
Install First-party Extensions
Health Check
Commit
Cleanup
Residue Audit
```

只有 Health Check 全部通过后才能进入不可回滚 Commit。

需要迁移：

- 项目、仓库、模板、设置和活动记录。
- 默认项目与备份目录。
- MCP 客户端配置的精确匹配条目。
- MCP GUI 偏好到 MCP 扩展命名空间。
- Discord 设置到 Discord 扩展命名空间。
- 扩展启用状态。

需要重建或作废：

- 仓库缓存与缩略图。
- 无法验证哈希的包缓存。
- MCP/API token。
- 更新 staging。
- 技术日志与临时文件。

不得删除：

- 用户仍在使用的 VCC。
- 无法证明属于 ALCOMD3 的独立应用。
- 用户自定义项目和备份。
- 其他应用的配置。

零残留标准：

```text
迁移后的系统状态
-
（全新安装相同 v4 版本的系统状态 + 相同用户数据）
=
空集
```

成功后不保留永久迁移标记、旧配置副本、旧数据库、旧 token、旧路径回退或旧 identifier。

## 21. 功能完整性

`feature-parity.toml` 是发布合同。每个功能必须记录：

- 来源。
- release class。
- GUI/CLI/RPC/MCP/API/Extension 覆盖。
- 测试。
- 当前状态。
- 证据。

功能与验收来源：

1. ALCOMD3 v3 最终冻结提交。
2. v3 README、维护文档、MCP 文档与 Changelog。
3. 冻结版本 vrc-get 的功能、安全行为、CLI 与错误处理。
4. 公开 VPM 格式与生态兼容性要求。
5. 真实 UI 行为和安装快照。
6. 用户明确要求保留的能力和 ALCOMD v4 产品计划。

这些来源仅定义预期行为，不授权复制或改写其实现源码。

## 22. 更新与发行

完整 ALCOMD 产品是原子发行单元：

```text
alcomd
alcomd-gui
alcomd-cli
alcomd-mcp
alcomd-extension-host
alcomd-bootstrap
alcomd-updater
release-blocker 第一方扩展
```

组件不能分别更新到不兼容版本。正式更新由 `alcomd-updater` 下载并验证 ALCOMD 应用层签名
与摘要覆盖的完整产品包，
`alcomd-bootstrap` 协调停止、替换、健康检查和回滚；失败时恢复上一整套程序。Tauri Updater
不是完整产品的权威更新系统。

4.0.0 的三个发布平台和四种主要用户发行格式全部是 Release Blocker：

1. Windows x86_64：`ALCOMD_4.0.0_windows_x86_64_setup.exe`，单文件 Inno Setup。
2. macOS arm64：`ALCOMD_4.0.0_macos_aarch64.dmg`。
3. Linux x86_64：`ALCOMD_4.0.0_linux_x86_64.AppImage`。
4. Linux amd64：`ALCOMD_4.0.0_linux_amd64.deb`。

Windows 当前用户与所有用户安装是同一 EXE 的两种模式，不是两个发行资产。签名文件、更新
压缩包与 update manifest 是辅助发行产物，不增加平台/主格式类别。RPM、Flatpak、Snap 和
macOS PKG 不属于 4.0.0 普通用户发行 blocker。

正式流水线目标入口：

```text
cargo xtask dist --target <target>
```

它必须构建 Rust 组件与 GUI 前端，用 Tauri 编译 `alcomd-gui`，构建并按 Extension 合同签名
第一方扩展，将所有 release-blocker 组件收集到统一 staging，验证版本、权限和许可证，再调用
Inno Setup、DMG、AppImage 或 DEB 工具链，生成应用层签名/摘要与 update manifest，并在启用
平台代码签名时执行对应步骤。单一 Tauri bundle 不得被当作完整产品发行物。

平台技术基线由 A-025 冻结：

- Windows x86_64 正式测试 Windows 10 22H2 与 Windows 11；不增加主动拒绝更旧 Windows 的
  应用级版本判断。WebView2 使用 Evergreen Runtime，安装/更新检测缺失后支持在线
  Bootstrapper，并为离线部署保留 Standalone Installer 路径。
- Linux x86_64 在 Ubuntu 22.04 构建，所有发行二进制的最高所需 glibc 符号版本不得超过
  `GLIBC_2.35`。这是 ABI 上限，不是“只允许 glibc 2.35 或更旧”的运行条件。
- macOS arm64 使用 `MACOSX_DEPLOYMENT_TARGET=11.0`，不再增加应用级系统版本门槛。
- 当前 Windows Authenticode 与 Apple Developer ID/notarization 不阻塞 4.0.0；未签名、未公证
  发行必须在真实系统验证下载、安装/挂载、首次启动、警告说明和更新。应用层更新签名、
  bridge 信任链、扩展签名和包完整性验证仍为强制要求。

扩展独立版本化，通过 Extension API 范围决定兼容性。

Windows Inno Setup 的新 AppId 在发行实现前单独冻结，不能复用 v3 身份，也不能随未来品牌变化。

## 23. 安全与测试门槛

必须覆盖：

- 功能对照。
- 迁移 Fixture。
- 崩溃与断电注入。
- 零残留快照比较。
- 多客户端并发。
- ZIP 路径穿越和符号链接逃逸。
- 包哈希与更新签名。
- Loopback API 的 Origin / Host / DNS Rebinding。
- 扩展越权、崩溃和资源配额。
- MCP 无 GUI 运行。
- 禁用 MCP UI 不影响协议。
- 禁用 Discord 后清除 Presence。
- RPC、MCP、CLI 和 Extension Schema 契约快照。

未通过功能对齐、迁移、零残留、并发、安全和扩展隔离测试时，不得向 v3 用户发布 4.0.0。

## 24. 实施顺序

```text
M-1 只读审计与基线冻结
M0  Workspace、身份、CI 与空骨架
M1  alcomd + RPC 握手 + CLI system status
M2  SQLite、Operations、Events、Locks、Recovery
M3  项目与仓库只读垂直切片
M4  VPM Plan / Apply 与包事务
M5  完整 CLI 与核心项目能力
M6  扩展运行时与公开 API
M7  官方 GUI 与 Portable Extension UI
M8  MCP 协议与 MCP 管理扩展
M9  Discord 第一方扩展
M10 TypeScript/Rust SDK 与公共合同硬化
M11 v3 迁移、Bootstrap、Updater 与 bridge installer 协调
M12 `cargo xtask dist`、全产品安装器、功能对齐、安全、零残留和发行硬化
Post-v4 可选 Loopback API、Python SDK 与 .NET SDK
```

每个里程碑必须独立验收并在完成后停止。

## 25. 最终规则

1. `ALCOMD3` 是当前用户品牌，`ALCOMD` 是永久产品家族，`alcomd` 是永久技术根。
2. 系统身份永久使用 `com.cqmhv.alcomd`。
3. 持久目录使用 `ALCOMD`，不使用 `ALCOMD3`。
4. 只有 `alcomd` 可以写数据库和项目。
5. 所有入口使用同一应用用例层。
6. MCP 协议独立于 GUI。
7. MCP 管理 GUI 是第一方扩展。
8. Discord 全部功能是第一方扩展。
9. 第一方扩展不得拥有隐藏捷径。
10. v3/VCC 兼容代码不得进入正常运行时。
11. 迁移必须先验证 v4，再删除 v3。
12. 功能完整性必须由清单和自动化测试证明。
13. 未来品牌更名不得要求数据迁移或第三方重新接入。
14. v4 自有内容统一采用 `AGPL-3.0-only`。
15. v3 与 vrc-get 不是 v4 的代码上游，VPM 必须独立实现。
16. ALCOMD 是多组件 Rust 本地应用平台，只有 `alcomd-gui` 是 Tauri 应用。
17. 正式发行必须打包完整产品；Tauri/Tauri Bundler 只能是受控子工具。
18. Extension Runtime compatibility 与 Portable Extension UI compatibility 相互独立；官方 GUI 只是第一个 renderer。
