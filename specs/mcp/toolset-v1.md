# ALCOMD MCP Toolset v1

状态：首个六工具切片已验收。回环 HTTP Bearer、预登记 HTTPS/OAuth 和包目录增量各有独立合同及验证记录，按 [M8 当前状态](../../docs/status.md) 查对应候选与覆盖范围。
冻结 MCP 规范基线：`2026-07-28`

正式输入输出、错误枚举及annotations见[包卸载工具Schema](package-removal-v1.schema.json)，完整授权与执行边界见[M8 ExecPlan](../../docs/exec-plans/M8-mcp-protocol-and-management.md#工具与管理界面)。没有v3别名、通用RPC透传或无凭据业务调用。

## 架构

`alcomd-mcp` 只做协议映射：

```text
MCP request
    -> input validation and client identity
    -> ALCOMD RPC
    -> alcomd application use case
    -> result / OperationId
```

## 当前规范约束

实现不得照搬旧版：

- 不依赖协议级 session。
- 不发送或要求 `Mcp-Session-Id`。
- 不使用旧 `initialize` / `notifications/initialized` 生命周期。
- 每个请求按规范携带协议版本与客户端能力。
- 支持 `server/discover`。
- HTTP 订阅与请求内通知按规范处理。

## 命名

工具前缀：

```text
alcomd_
```

不保留 `alcomd3_*` 运行时别名。

首个卸载切片的六个工具：

```text
alcomd_list_projects
alcomd_get_project
alcomd_plan_package_uninstall
alcomd_apply_package_changes
alcomd_get_operation
alcomd_cancel_operation
```

项目查询展示当前授权的登记快照，Plan 重读当前文件。卸载使用本 Principal 的原 remove Plan，Apply 要求显式 `confirm=true`。返回 Operation 表示 Core 已接受计划，业务结果由任务终态给出。包目录、安装及重装的后续接入见下方。

2026-11-15所有者批准OAuth项目游标补充：`alcomd_list_projects`的cursor新增可选
`authorization_binding`（64位小写hex），透传Core `authorizationBinding`，不由适配器签发。
OAuth续页必须原样回传；缺失、篡改、换token或授权快照变化返回既有revision_conflict，
重新读取第一页。绑定覆盖认证的令牌、资源和时间 / ProjectId 位置。
非OAuth继续省略该字段；没有新的工具、State、Config、权限、session或缓存。

## 长任务

核心权威状态为 ALCOMD Operation。`alcomd_get_operation` 需要 `operations.read`，`alcomd_cancel_operation` 需要 `operations.cancel`。两项按真实调用者读取或取消自身任务，使用 Core 的生命周期和修订规则。包变更和状态检查有明确结果字段，其余任务提供状态、修订、时间和稳定错误字段。4.0.0 不采用或广告 MCP Tasks。协议请求取消及 STDIO EOF 只结束当前请求，只有显式 `alcomd_cancel_operation` 请求 Core 合作式业务取消。

`alcomd_list_operations` 使用同一读取权限和任务视图，接受 `limit`、`cursor` 和 `active_only`，返回当前调用者的任务分页及活动数。

## 包目录、安装与重装

获批的增量工具见[工具 Schema](package-catalog-plan-v1.schema.json)与 [Core RPC Schema](../rpc/m8-mcp-packages-v1.schema.json)。目录按当前 `repositories.read`、`packages.read` 展示可访问的包和来源，可选项目上下文另需 `projects.read`。`alcomd_search_packages` 可浏览全局包或按仓库筛选，`alcomd_get_package` 可查版本详情，`alcomd_list_package_sources` 提供选源所需的稳定 ID 和可用状态。受保护来源当前无需向客户端透露设置或凭据。

`alcomd_plan_package_install` 和通用安装、升级意图均建议省略 `version_selector`，由 Core 推荐兼容版本。调用者也可指定精确版本或范围生成真实 Plan，`alcomd_plan_package_reinstall` 保持已安装的锁定版本。两项均需项目、仓库、包读取能力。`alcomd_apply_package_changes` 使用相同读取能力、`packages.manage`、原 Plan、显式确认和既有幂等规则。来源和版本均由 Core 冻结、重新核对，输出使用稳定来源 ID 和名称。

`alcomd_plan_package_changes` 复用相同项目与修订输入，支持安装、升级、降级、卸载、重新解析、选定包或全部重装，以及最多 256 项的批量意图。可在安装或升级中指定版本范围及预发布策略。精确安装、升级或降级可明确选择 Unity 兼容性例外，Core 在 Plan 中列出风险并沿用原确认规则。

MCP、GUI 与 CLI 复用 Core 的已登记来源、可见性、解析与归档校验。获得所需业务权限的 MCP 客户端可使用已登记的公开或认证仓库、带查询参数的来源、本地仓库和用户包。Core 在原有范围内处理仓库凭据与下载头，MCP 响应只给安全的来源标识和普通文档链接。MCP 目录在协商 `mcp.packages.catalog-plan.v1` 后按实时权限展示这些工具。

## 仓库管理

`mcp.repositories.v1` 提供 [MCP 工具](repositories-v1.schema.json) 和 [Core RPC](../rpc/m8-mcp-repositories-v1.schema.json)。`alcomd_list_repositories` 用 `repositories.read` 浏览共用仓库顺序。继续分页时使用返回的完整游标。仓库顺序或成员变化后，Core 对旧游标返回 `revision_conflict`，应从首页重读。`alcomd_inspect_repository` 检查远程地址，同时有 `repositories.manage` 时提供 60 秒有效、绑定到当前认证的预览 ID。`alcomd_register_repository` 要求显式确认原预览，Core 在提交前检查来源证据，变动时需要重新检查。`alcomd_refresh_repository` 和 `alcomd_remove_repository` 使用实际仓库 ID 和修订，移除另需显式确认与内置保护。三项写入均要求 `repositories.manage` 和幂等键，以原参数同键恢复。凭据及本地路径由本机入口管理，Core 在原规则下刷新这些已登记来源，安全响应省略来源地址与凭据。

## 项目和备份

`mcp.project-workflows.v1` 提供项目操作和备份文件操作。路径使用 Core 所在机器的文件系统。工具、参数及结果见[工具 Schema](project-workflows-v1.schema.json)和 [Core RPC Schema](../rpc/mcp-project-workflows-v1.schema.json)。

- `alcomd_register_project` 接收现有项目目录的绝对路径，也可用相对路径在默认项目目录中选择项目。
- `alcomd_create_project` 接收完整的新项目目录路径，也可给出默认项目目录下的目录名。`alcomd_list_project_templates` 提供可用于创建的模板 ID 和修订。含包的模板沿用现有包与仓库能力。
- `alcomd_plan_project_copy` 冻结源项目修订和目标父目录身份。目标可填写完整目录路径或默认项目目录下的目录名。确认后用 `alcomd_apply_project_copy` 提交原 Plan。
- `alcomd_create_backup` 生成 Unity 工程根目录结构的普通 ZIP。可指定完整的 `destination_path`，也可使用默认备份位置。
- `alcomd_restore_backup` 接收 `archive_path` 和显式确认，可恢复默认备份目录之外的文件。可以用 `destination_path` 指定完整的新项目目录，也可以用 `project_name` 在默认项目目录中命名。两项均省略时使用 ZIP 文件名。

Core 在接受任务前核对当前授权、路径身份及目的地可用性。已接受任务通过现有 Operation 查询或取消。写响应丢失时按原参数同键恢复。

已认证的 MCP 客户端与 GUI、CLI 使用相同的项目和备份权限。工具目录反映当前协商的 Core 能力。共享资源归 Core 的本机所有者，Plan、Operation 和幂等记录归发起请求的实际客户端。对外结果只返回稳定 ID、必要的摘要和目标名称。

## 读取设置和 Unity 环境

| 工具 | 同一 Core 入口 | 权限 |
| --- | --- | --- |
| `alcomd_get_settings` | `settings.get`。返回当前设置和 Core 解析的默认项目、备份目录。 | `settings.read` |
| `alcomd_list_unity_installations` | `unity.installations.list`。使用同一安装登记与游标，包含安装路径。 | `unity.read` |
| `alcomd_get_project_launch_config` | `unity.projectLaunchConfig.get`。返回项目覆盖、默认值及实际有效的完整参数。 | `projects.read`、`unity.read` |

三端由 Core 按当前调用者检查相同的读取权限。MCP 管理扩展负责登记与撤销连接，OAuth 使用连接 scope `mcp`。MCP 工具仅将字段名转换为 snake_case，保留 Core 返回的值。 [工具 Schema](environment-v1.schema.json)

项目列表与详情在 `projects.read` 下返回同一 Core 登记路径 `root_path`。MCP 的项目操作继续使用稳定项目 ID。

## 客户端接入速查

为每个客户端单独创建身份，用 `alcomd-cli mcp client issue-credential` 签发 Bearer 或 STDIO 凭据。凭据交给客户端的受控秘密入口。每次轮换立即使旧凭据失效。

| 传输 | 入口 |
| --- | --- |
| 回环 HTTP Bearer | 运行 `alcomd-mcp serve --port 0`，将报告的 `/mcp` 地址和该客户端独立 Bearer 配置到支持冻结协议的客户端。 |
| STDIO | 将 `alcomd-mcp stdio` 配置为启动命令，并通过子进程环境提供 `ALCOMD_MCP_CLIENT_CREDENTIAL`。 |
| 回环 HTTPS/OAuth | 预登记客户端和精确回调地址，运行 `alcomd-mcp serve-oauth --port <port> --tls-cert <cert> --tls-key <key>`，客户端使用 `https://localhost:<port>/mcp`。证书须由该客户端信任，OAuth 使用预登记资源和连接 scope `mcp`。 |

### Codex CLI 0.153.4 的回环 HTTP 接入

1. 在 MCP 管理扩展中创建独立客户端。用 `alcomd-cli mcp client list` 核对 ID 和当前修订，再运行 `alcomd-cli mcp client issue-credential <id> --expected-revision <n> --idempotency-key <key>` 取得一次性凭据。用 `alcomd-mcp serve --port 0` 启动监听，记下实际报告的地址。
2. 运行 `codex mcp get alcomd` 查看目标条目。确认该条目不存在时，核对下面的地址和环境变量名后执行添加。已有同名条目时，先确认要修改的字段，再手动合并该条目，或选择新名称。`codex mcp add` 会直接覆盖同名条目，读取配置出错时应先解决错误。

   ```powershell
   codex mcp add alcomd --url 'http://127.0.0.1:实际端口/mcp' --bearer-token-env-var ALCOMD_CODEX_BEARER
   ```

3. 在专用终端通过不回显输入把该客户端凭据交给 Codex 进程。配置只保存环境变量名。

   ```powershell
   if (Test-Path Env:ALCOMD_CODEX_BEARER) { throw '请在尚未设置此变量的专用终端运行' }
   $env:ALCOMD_CODEX_BEARER = [System.Net.NetworkCredential]::new(
       '', (Read-Host '输入此客户端的 Bearer 凭据' -AsSecureString)
   ).Password
   try {
       codex --enable mcp_2026_07_28
   } finally {
       Remove-Item Env:ALCOMD_CODEX_BEARER
   }
   ```

4. 查看当前可用工具并运行一项已授权的读取。清理时用 `codex mcp remove alcomd` 移除该配置，并在 Core 中撤销专用客户端。配置名若不同，使用实际配置名。

该版本在隔离配置开启 `mcp_2026_07_28` 后已实测 HTTP Bearer 和预登记 OAuth。STDIO 实测仍发送旧握手，不能接入冻结协议。ALCOMD 自动写入客户端配置和真实通知消费仍待验证。 [接入证据](../../docs/testing/m8-management-manual-20261115.md)

### Claude Code 2.1.327 的回环 HTTP 接入

为 Claude Code 创建独立客户端并签发专用 Bearer。运行 `alcomd-mcp serve --port 0`，用它报告的地址替换示例。在专用 PowerShell 终端执行以下命令。

```powershell
@'
{"type":"http","url":"http://127.0.0.1:实际端口/mcp","headers":{"Authorization":"Bearer ${ALCOMD_CLAUDE_BEARER}"}}
'@ | claude mcp add-json alcomd - --scope user
```

同名条目已存在时，此命令拒绝覆盖。核对现有条目后可以使用新名称。运行 `claude mcp get alcomd`，确认状态显示 `Connected`。在同一终端使用 Claude Code 时也需要保留环境变量。

```powershell
if (Test-Path Env:ALCOMD_CLAUDE_BEARER) { throw '请在尚未设置此变量的专用终端运行' }
$env:ALCOMD_CLAUDE_BEARER = [System.Net.NetworkCredential]::new(
    '', (Read-Host '输入此客户端的 Bearer 凭据' -AsSecureString)
).Password
try {
    claude mcp get alcomd
    # 连接通过后，可在此处启动 claude。
} finally {
    Remove-Item Env:ALCOMD_CLAUDE_BEARER
}
```

清理时运行 `claude mcp remove alcomd --scope user`，并在 Core 中撤销该专用客户端。Windows 独立配置已实测添加、拒绝同名覆盖、真实回环 HTTP 连接和移除。Claude Code 的 STDIO、OAuth、模型发起的工具调用及通知消费未在此次检查中执行。 [Claude Code MCP 配置](https://code.claude.com/docs/en/mcp) · [协议支持记录](https://code.claude.com/docs/en/changelog)

### Cursor CLI 2026.11.17-f532c45

Windows x64 具名版本在隔离配置实测 HTTP、STDIO 均发出 `2025-11-25` 的 `initialize`，无法接入当前冻结的 `2026-07-28` 服务。其他版本和 OAuth 未验证。 [实测记录](../../docs/exec-plans/M8-mcp-protocol-and-management.md#2026-11-20-m8-收尾)
## 项目已安装的包和模板编辑

| MCP 工具 | 共享 Core 方法 | 所需 Core 权限 |
| --- | --- | --- |
| `alcomd_list_installed_packages` | `packages.projectInventory` | `projects.read`、`packages.read` |
| `alcomd_get_project_template` | `projectTemplates.get` | `projects.create` |
| `alcomd_edit_project_template` | `projectTemplates.edit` | `projects.manage` |
| `alcomd_create_project_template` | `projectTemplates.save` | `projects.manage`，项目来源还需要 `projects.read` |
| `alcomd_export_project_template` | `projectTemplates.export` | `projects.manage` |
| `alcomd_remove_project_template` | `projectTemplates.remove` | `projects.manage` |

项目包结果区分声明版本、锁定版本和本机观察到的版本。修订或观察结果变化时，
从第一页重新读取。模板详情提供依赖和分页附件名。编辑使用原修订、明确修改内容
和 `confirm: true`，通过既有任务工具查询结果。Core 在接受前冻结附件文件，
写入结果未知时可显式使用原请求和原幂等键恢复。内置模板保持只读。
创建可以使用手工定义、已登记项目或已有模板文件。创建和导出返回同一 Core
任务。移除返回模板 ID 与移除修订，重放原请求返回同一结果，保留后来重建的模板。
工具字段见[项目包 Schema](project-inventory-v1.schema.json)和[模板 Schema](template-edit-v1.schema.json)。

三个原生客户端工具使用 `clients.registry.v1`，参数和返回值见[安装与连接工具定义](client-registry-v1.schema.json)。`alcomd_list_clients` 读取已安装及活跃程序，`alcomd_list_client_instances` 读取指定程序的实例/连接，`alcomd_disconnect_client` 按已确认的 Core 实例、目标及原幂等键断开。AI 凭据仍由 Core 验证和撤销。

桌面工具使用 `desktop.actions.v1`。三项请求分别接收已登记的项目、项目修订或固定 Unity 下载页操作；调用者传入本次操作的幂等键。Core 在其本机桌面执行。相同请求的回执未知时，原参数和原键可用于取回结果。[桌面工具字段](desktop-actions-v1.schema.json)
