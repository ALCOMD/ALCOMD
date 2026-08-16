# ALCOMD3 v4.0.0 完整架构、扩展与无残留迁移方案

状态：**Accepted as product direction，implementation pending**

## 1. 定位

ALCOMD3 v4.0.0 是一套 Rust 本地应用平台，而不是以 GUI 为中心的单体程序。

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

| 程序 | 职责 |
|---|---|
| `alcomd` | 每用户唯一核心、唯一状态持有者和唯一写入者 |
| `alcomd-gui` | 官方 GUI、扩展 UI 容器、审批与操作中心 |
| `alcomd-cli` | 完整可自动化命令行客户端 |
| `alcomd-mcp` | 独立 MCP 协议适配器 |
| `alcomd-api` | 可选 Loopback HTTP 网关 |
| `alcomd-extension-host` | 第一方与第三方后台扩展沙箱宿主 |
| `alcomd-bootstrap` | 安装、更新、迁移、替换与卸载协调器 |
| `alcomd-migrate-v3` | 只在 v3 升级期间释放的一次性迁移程序 |

`alcomd-migrate-v3` 不进入普通 v4 安装结果。

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
    "daemonVersion": "4.0.0",
    "dataSchema": 1,
    "configSchema": 1,
    "extensionApi": 1
}
```

公共 DTO 与内部领域对象必须分离。破坏性变化提升对应协议大版本，而不是仅提升应用版本。

## 12. 第三方应用通信

原生桌面应用优先使用本地 RPC 与官方 SDK。

首次配对：

```text
请求客户端身份与权限
    -> 用户通过 GUI 或 CLI 审批
    -> 核心签发独立凭据
    -> 用户可查看和撤销
```

每个客户端独立身份，不共享万能 token。

可选 `alcomd-api`：

- 默认关闭。
- 只监听 `127.0.0.1` 与 `::1`。
- 动态端口。
- 校验 Host 与 Origin。
- 防止 DNS Rebinding。
- 独立客户端授权。
- 所有调用审计。
- token 进入操作系统凭据库。

## 13. 官方 GUI

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

Tauri command 只能用于窗口、文件选择器、系统通知、RPC 桥接和扩展容器。不得实现包、项目、MCP、Discord 或迁移业务。

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
- UI Bridge
- 权限
- 数据命名空间
- WASM/WASI 后台宿主
- 沙箱
- 安装、更新与崩溃隔离

区别只允许是发行者、签名、默认安装策略与官方支持等级。

扩展 UI 运行在 sandboxed iframe 或隔离 WebView。后台运行在 `alcomd-extension-host` 的 WASM/WASI 环境。禁止原生 DLL、`.so` 与 `.dylib` 扩展。

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

ALCOMD 长任务通过核心 `OperationId` 表达。是否采用 MCP Tasks 扩展必须单独 ADR 决定，不得把旧实验性 Tasks API 当成稳定核心协议。

MCP 管理扩展 ID：

```text
com.cqmhv.alcomd.extension.mcp
```

它负责客户端、会话/请求、权限、审批、配置计划、操作进度与诊断可视化。禁用它不得影响 MCP 协议服务。

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
%LOCALAPPDATA%\Programs\ALCOMD
%ProgramFiles%\ALCOMD
```

默认用户目录：

```text
Documents\ALCOMD\Projects
Documents\ALCOMD\Backups
```

macOS 与 Linux 分别遵循 Application Support 和 XDG 规范，但稳定目录名仍为 `ALCOMD` / `alcomd`。

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
- 更早的公开 v3.x 必须先通过原有更新链升级到 3.4.0，不得直接进入 v4 迁移。
- 3.4.0 已将更新 JSON 源切换到 `https://alcomd.cqmhv.com/api/v1/updates/stable.json` 与 `https://alcomd.cqmhv.com/api/v1/updates/beta.json`。
- 3.4.0 从新标准 API 获取并验证 v4 迁移桥接安装器（v4 bridge installer）元数据；该安装器再启动 `alcomd-bootstrap` 和临时 `alcomd-migrate-v3`。

上述已上线路径和频道映射作为 M-1 基线；v4 迁移桥接安装器的 JSON Schema、版本推进、签名验证和错误行为在 M-1 冻结。

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

核心组件是原子发行单元：

```text
alcomd
alcomd-gui
alcomd-cli
alcomd-mcp
alcomd-api
alcomd-extension-host
alcomd-bootstrap
```

不能分别更新到不兼容版本。更新由外部 bootstrap 进行，失败时恢复上一整套程序。

扩展独立版本化，通过 Extension API 范围决定兼容性。

Windows 新 AppId / UpgradeCode 在 v4 初始化后固定，不能复用 v3 身份，也不能随未来品牌变化。

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
M7  官方 GUI 与扩展 UI 容器
M8  MCP 协议与 MCP 管理扩展
M9  Discord 第一方扩展
M10 Local API 与 SDK
M11 v3 迁移、Bootstrap 与更新桥
M12 功能对齐、安全、零残留和发行硬化
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
