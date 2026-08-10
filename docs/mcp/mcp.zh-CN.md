# ALCOMD3 MCP 说明

语言: [English](../mcp.md) | [日本語](mcp.ja.md) | 简体中文 | [繁體中文](mcp.zh-TW.md)

本文档说明 ALCOMD3 的 MCP 接入方式、可用工具、生命周期行为和排障方法。

ALCOMD3 使用 RMCP 3.1.2 实现 MCP `2026-07-28`，同时兼容 `2025-11-25`
客户端的普通工具调用。MCP server 已成为 GUI 进程的一部分：MCP 扩展启用时，GUI
以 `alcomd3-mcp` implementation 名称在 `127.0.0.1` 暴露一个本地 Streamable HTTP
endpoint，不再存在 helper 进程、私有 IPC listener 或 endpoint metadata 文件。

## 快速开始

1. 启动 ALCOMD3，在“扩展”页确认 MCP 扩展已启用，再打开侧边栏中的 MCP 页面。
2. 启用 MCP。
3. 默认使用页面显示的 MCP Endpoint 和授权令牌手动配置客户端。Windows 上的 Codex、
   Claude Code 和 Cursor 用户也可以选择对应的可选快速配置按钮。
4. 手动配置时，将 URL 添加为 Streamable HTTP server，并用
   `Authorization: Bearer <令牌>` 发送授权令牌。
5. 保持 ALCOMD3 中的 MCP 为启用状态，然后运行工具调用。

请直接使用 GUI 显示的 endpoint，不要自行猜测端口。配置示例和生命周期详情请参阅
[启用和客户端配置](#启用和客户端配置)。

## 当前边界

- MCP 功能默认停用，需要在 GUI 中手动启用后才允许新的工具调用读取或写入 ALCOMD3 数据。
- MCP 扩展启用时，GUI 会运行本机 Streamable HTTP endpoint；在 MCP 页面启用/停用
  MCP 只控制新的工具数据访问，不关闭 endpoint。
- 从“扩展”页关闭 MCP 扩展会撤销 MCP 访问许可、停止 endpoint、从侧边栏移除
  MCP，并取消仍由 GUI 管理的 MCP 项目任务。重新启用扩展会立即完成开关操作，并在
  后台恢复 endpoint；MCP 访问仍保持停用，直到用户再次在 MCP 页面主动启用。
- 当前提供项目、环境级模板、仓库、软件包和环境设置只读工具，以及有限写工具：新建项目、
  创建/编辑/删除环境级模板、为派生模板单独设置或移除一项直接 VPM 依赖或 UnityPackage
  附件引用、添加已有项目、添加或删除用户 VPM 仓库、备份已登记项目、复制已登记项目、从 zip 备份恢复项目、
  为已登记项目安装/卸载/重装单个软件包。不提供仓库重排、项目删除等其他写操作。
- MCP 扩展启用时，GUI 负责启动和管理本地 Streamable HTTP server；关闭 GUI 或关闭
  MCP 扩展时都会停止该 server。
- 使用 MCP 工具期间必须保持 ALCOMD3 运行；关闭 GUI 也会关闭公开的 loopback endpoint。
- MCP 停用时，新的 tool call 返回结构化 `mcp_disabled` 错误，不关闭 endpoint、不 panic，
  并在 MCP tool result 上标记 `isError: true`。已启动项目长任务的 `tasks/get`、
  `tasks/cancel` 是收尾例外，可继续查询结果或取消该任务。
- 内置 server 对 tool call 做本地限流和并发保护；超过限制时返回结构化
  `rate_limited` 错误，并在 MCP tool result 上标记 `isError: true`。
- GUI MCP 页面会在已知 tool call 执行时高亮对应工具，并在完成或失败后短暂保留高亮，
  便于观察很快完成的调用。
- GUI MCP 页面按只读、写入和日志用途分组显示工具，并保留工具的精确 MCP 名称；鼠标悬停在工具名上会显示本地化的可读名称。
- GUI MCP 页面显示的是最近活动过的客户端，不是实时连接列表；超过一段时间没有活动的记录
  会自动隐藏。
- 日志扩展启用时，MCP tool call 会写入 GUI 的本地活动记录。记录包含来源、工具名、
  request id、客户端摘要、开始/完成/失败/取消状态和经过安全处理的目标/详情，便于用户在
  GUI 的“活动记录”页回溯 Agent 做了什么。
- GUI 项目管理页和 MCP 包工具共用后端的 GUI-visible package catalog。预发布、yanked、
  隐藏仓库、隐藏本地用户包、同名包跨来源合并、默认/用户仓库优先级和 Unity 兼容性判断由后端统一执行。
- 每个公开 MCP tool 都必须映射到 GUI 已有 capability，并通过 `vrc-get-gui/src/backend/`
  中的共享后端服务进入业务逻辑。MCP dispatch 只负责启用状态 gate、参数解析、任务封装、
  错误映射和活动记录，不应新增 GUI 不具备的业务能力。
- Streamable HTTP 请求必须携带为本机 ALCOMD3 安装生成的 bearer token。
- HTTP server 校验 `Host` 和 `Origin`，严格绑定 `127.0.0.1`，不监听局域网或公网地址。

活动记录不会保存原始 MCP params、token-like 字段、HTTP header 值、带 query 的 URL 或 URL userinfo 凭据。
本地文件系统路径会保留完整值，用于排查 Unity、VPM 和中文路径等问题；MCP access 仍需先在 GUI 中启用。

## 架构

```text
MCP Host / Client
        |
        | Streamable HTTP + bearer token
        v
ALCOMD3 GUI process
        |
        +-- embedded alcomd3-mcp HTTP/RMCP service
        +-- direct in-process GUI business dispatch
        +-- shared Tasks, cancellation, locks, and activity state
```

所有已认证请求都在 GUI 进程内处理。工具 handler 直接调用 GUI 使用的同一套后端服务，
不再经过 TCP bridge 或 JSON 行序列化。所有已认证本机客户端共享同一个 bearer principal
和任务命名空间；客户端名称和版本只用于最近活动展示与日志。

## 启用和客户端配置

1. 启动 ALCOMD3。
2. 在“扩展”页确认 MCP 扩展已启用，再打开侧边栏中的 MCP 页面。
3. 点击启用，允许 MCP 工具读取 ALCOMD3 数据。
4. 复制页面中的 MCP Endpoint 和授权令牌。
5. 在支持 Streamable HTTP MCP server 的客户端中添加 endpoint URL，并将令牌配置为
   bearer `Authorization` header。

通用配置形态如下，具体字段名以 MCP 客户端为准：

```json
{
    "mcpServers": {
        "alcomd3": {
            "url": "http://127.0.0.1:51739/mcp",
            "headers": {
                "Authorization": "Bearer <ALCOMD3 页面显示的令牌>"
            }
        }
    }
}
```

手动配置仍是默认方式，不会修改操作系统或 AI 客户端配置。

### Windows 上可选的 AI 客户端快速配置

Windows 的 MCP 页面为 Codex、Claude Code 和 Cursor 提供各自独立的快速配置按钮。
只有用户明确点击某个按钮后，才会修改对应客户端。每个按钮都会将当前令牌写入当前
Windows 用户的 `ALCOMD3_MCP_BEARER_TOKEN` 环境变量，并仅添加或更新所选客户端的
ALCOMD3 MCP 配置项：

- Codex：使用 `$CODEX_HOME/config.toml`；未设置 `CODEX_HOME` 时使用
  `~/.codex/config.toml`：

```toml
[mcp_servers.alcomd3]
url = "http://127.0.0.1:51739/mcp"
bearer_token_env_var = "ALCOMD3_MCP_BEARER_TOKEN"
```

- Claude Code：使用 `$CLAUDE_CONFIG_DIR/.claude.json`；未设置
  `CLAUDE_CONFIG_DIR` 时使用 `~/.claude.json`：

```json
{
    "mcpServers": {
        "alcomd3": {
            "type": "http",
            "url": "http://127.0.0.1:51739/mcp",
            "headers": {
                "Authorization": "Bearer ${ALCOMD3_MCP_BEARER_TOKEN}"
            }
        }
    }
}
```

- Cursor：使用 `~/.cursor/mcp.json`：

```json
{
    "mcpServers": {
        "alcomd3": {
            "type": "http",
            "url": "http://127.0.0.1:51739/mcp",
            "headers": {
                "Authorization": "Bearer ${env:ALCOMD3_MCP_BEARER_TOKEN}"
            }
        }
    }
}
```

所选客户端的其他设置和 MCP server 会原样保留。如果环境变量或该客户端的 `alcomd3`
配置项已经存在不同值，ALCOMD3 会先请求确认，不会静默覆盖。完成快速配置后请完全退出
并重新启动所选客户端，使其继承新的用户环境变量并重新加载 MCP 配置。

不同客户端的字段名可能不同。请始终从 GUI 复制当前 URL 与令牌。默认端口为 `51739`；
高级用户可在启动 ALCOMD3 前修改 `gui-config.json` 中的 `mcpHttpPort`。

endpoint 仅在 ALCOMD3 运行且 MCP 扩展启用时可用。如果 MCP 页面中的访问许可已停用，
新的工具调用返回 `mcp_disabled`，启用 MCP 后重试即可。关闭 MCP 扩展会停止 endpoint，
并取消仍由 GUI 管理的 MCP 项目任务。

外部 HTTP 端口和 bearer token 以 `mcpHttpPort`、`mcpHttpToken` 保存在
`gui-config.json`。请将 token 视为本机密钥，不要放入日志、截图或共享配置。替换 token
后，已有客户端配置会失效。

## 内置运行时和持久配置

所有受支持安装包和归档都只通过 GUI 可执行程序提供 MCP，不再包含需要定位或启动的
`alcomd3-mcp` helper。`cargo xtask build-alcom` 只构建 GUI，由 GUI 进程持有 HTTP
listener 并执行全部工具。

公开端口和 bearer token 以 `mcpHttpPort`、`mcpHttpToken` 保存在
`gui-config.json`。运行时不再读取、写入或迁移 `mcp/endpoint.json`；
`ALCOMD3_MCP_ENDPOINT_FILE` 和内部 listener 覆盖项已删除，客户端配置仍可使用
`ALCOMD3_MCP_BEARER_TOKEN`。

修改端口或轮换令牌会有序停止并重新绑定内置 transport。transport 重启期间，共享的
协议任务状态仍保留在 GUI 中。新端口绑定失败不会影响 GUI 其他功能；MCP 页面会显示
server 未运行，并由技术日志记录失败。

## 可用工具

ALCOMD3 当前公开 33 个工具。主指南集中说明使用流程和安全边界；
[完整工具参考](tools.zh-CN.md)逐项列出每个输入、输出字段，说明它是否必填或按条件出现、
省略时的默认值，以及字段的实际含义。

| 领域 | 读取工具 | 写入工具 |
| --- | --- | --- |
| 项目 | 项目列表和详情 | 创建、登记、备份、复制和恢复 |
| 模板 | 模板列表和详情 | 创建、编辑、设置/移除 VPM 依赖和 UnityPackage 引用，以及删除 |
| 存储库 | 存储库列表 | 添加和删除远程用户存储库 |
| 软件包 | 软件包列表和详情 | 安装、卸载和重装项目软件包 |
| 环境 | Unity 安装、启动参数和默认路径 | 无 |
| 日志 | 搜索、详情、上下文和聚合 | 无 |

工具参考还记录分页默认值、允许的枚举、MCP Task 支持、共享返回类型、错误结构，
以及两个按设计直接返回详情对象而不含 `ok` 的详情工具。

### 日志查询工具

日志工具按用途分为“活动记录”和“技术日志”两套，避免 Agent 为了排查一个问题把全部日志拉入上下文。

- 活动记录是用户可读、结构化、已脱敏的操作历史。`alcomd3_search_activity_logs`
  默认 `visibility` 为 `important`，会返回写操作、失败、取消和重要 MCP/System 行为等关键活动；
  需要辅助记录时显式传入 `secondary`、`technical` 或 `all`。
- 活动日志搜索结果只返回摘要字段，包括 id、时间、来源、类型、状态、操作、对象、耗时和错误摘要。
  需要详情时再调用 `alcomd3_get_activity_log_entry`，需要上下文时调用
  `alcomd3_get_activity_log_context`。
- 技术日志是排错入口，默认只查当前进程内存里的 `error` 和 `warn`。需要读取近期文件时显式传入
  `"scope": "recent_files"`；需要 Info/Debug/Trace 时显式传入 `levels`。
- 技术日志工具不会返回无限制原文。搜索只返回 `messagePreview`，详情按 `max_message_chars`
  截断，并会脱敏 token、secret、authorization、API key、`sk-` 开头的值，以及 URL userinfo、query 和 fragment。
- 日志工具本身也会被记录为 MCP read activity。成功读取日志属于 Secondary，失败仍会作为失败活动默认可见。

### 项目长任务

ALCOMD3 使用 RMCP 3.1.2 提供的实验性 `io.modelcontextprotocol/tasks` 扩展。
不同客户端的支持程度可能不同，该扩展也可能在后续版本中演进。

`alcomd3_create_project`、`alcomd3_backup_project`、`alcomd3_copy_project`、
`alcomd3_restore_project_from_backup`、`alcomd3_install_project_package`、
`alcomd3_uninstall_project_package` 和 `alcomd3_reinstall_project_package` 会在客户端声明
`io.modelcontextprotocol/tasks` capability 时使用 task-aware 调用：

- `tools/call` 会立即返回带 `taskId` 的 task handle。
- `tasks/get` 返回 `working`、`input_required`、`completed`、`failed` 或 `cancelled`，
  并在详细任务状态中包含完成结果或失败信息。
- `tasks/update` 向运行中的任务提供其请求的响应。
- `tasks/cancel` 会协作式取消底层 GUI 操作，并释放对应资源锁。
- `alcomd3_create_project` 在项目正式登记前收到取消或包解析/应用失败时，会清理 MCP 创建出的未登记项目目录。
- 如果任务运行期间用户停用 MCP，新的工具调用和新的项目任务启动仍会返回
  `mcp_disabled`；已获得 `taskId` 的任务仍可由已认证请求使用 `tasks/get` 查询或
  `tasks/cancel` 取消。
- 关闭整个 MCP 扩展或退出 GUI 会取消未完成任务并清理其协议状态。

该扩展有意不提供旧 core Tasks 的 `tasks/list` 和 `tasks/result`；完成输出直接从
`tasks/get` 读取。同步 `tools/call` 带 `_meta.progressToken` 时仍会收到标准
`notifications/progress`；task-aware 调用也会随后端进度更新可读状态信息。

未声明 Tasks capability 的客户端继续获得原有普通同步 `tools/call` 行为和结果形状。

### 路径限制

`alcomd3_get_project_details`、`alcomd3_backup_project`、`alcomd3_copy_project`
以及项目包安装/卸载/重装工具的源项目路径只允许使用 ALCOMD3 数据库中已登记的项目路径。MCP client 不能通过这些工具
读取或复制任意本地路径。

`alcomd3_get_environment_settings` 会返回 ALCOMD3 已保存的本机路径，例如 Unity 可执行文件、
默认项目目录和备份目录。该工具不启动 Unity、不调用 Unity Hub 刷新、不扫描额外磁盘路径。

`alcomd3_backup_project` 的 `backup_name` 只允许是单一合法文件名，不能是路径，并且不包含自动追加的
`.zip` 扩展名。归档始终写入 GUI 配置的备份目录，且不会覆盖现有归档。

`alcomd3_copy_project` 的 `new_project_path` 必须是绝对路径、尚不存在的目录路径，且不能位于
源项目目录内部；工具会创建该目录，复制项目文件后登记新项目，失败时会清理新建目录。
`alcomd3_restore_project_from_backup` 的 `backup_path` 必须是绝对路径，并且只从 zip 备份恢复到
GUI 配置的默认项目目录。`project_name` 只允许是单个合法文件夹名，不能包含路径分隔符、
根路径或 `..`。
`alcomd3_create_project` 的 `project_name` 使用同样的单文件夹名限制；显式传入的 `base_path`
必须是绝对路径。未传 `base_path` 时使用 GUI 默认项目路径。`alcomd3_add_existing_project`
的 `project_path` 必须是绝对路径，并且必须能按 Unity 项目加载。

### 软件包可见性和写入限制

`alcomd3_list_packages` 和 `alcomd3_list_repository_packages` 使用与 GUI 软件包页相同的包状态加载路径，不调用强制刷新路径。
返回结果会遵循 GUI 中的预发布、隐藏仓库、隐藏本地用户包和 yanked 过滤规则。MCP tool call
不做服务端搜索。添加仓库必须显式调用 `alcomd3_add_repository`；列表工具不会隐式添加仓库或重构仓库刷新策略。

仓库参数的用途彼此分离：软件包读取和安装工具统一使用 `alcomd3_list_repositories` 返回的 `id` 选择仓库；用户仓库的添加和删除使用已存 URL，因此删除输入与添加输入直接对应，并且不会作用于内置默认仓库。重复检查仍同时覆盖已存 URL 和仓库发布者声明的 ID。GUI 的添加、删除和重排也使用同一个基于 URL 的共享后端。不支持本地仓库：加载设置时会丢弃无 URL 的用户仓库条目，也不提供本地仓库创建路径。

GUI 项目管理页的软件包表由后端合并同名包生成。MCP 的包列表、包详情和项目包安装选择使用同一套后端规则：

- 关闭“显示预发布软件包”后，GUI 和 MCP 的 GUI-visible 结果都不会包含预发布版本；MCP `latest_gui_visible`
  也无法选择预发布版本。底层缓存仍可保存预发布数据，重新开启后才会进入可见结果。
- yanked 包不会进入可见候选。已安装包如果当前版本 yanked，会在项目包行中保留 yanked 标记。
- 隐藏仓库和隐藏本地用户包只影响可见候选；隐藏来源仍可作为“存在来源”信息显示，但不参与最新版本选择。
- 同名包跨来源在项目管理页合并成一行，默认仓库、本地用户包、用户仓库和未登记仓库按后端顺序合并。
- 项目包安装只会从 GUI-visible 且与项目 Unity 版本兼容的候选中选择版本。

`alcomd3_install_project_package`、`alcomd3_uninstall_project_package` 和
`alcomd3_reinstall_project_package` 会先生成 pending project changes。若结果包含依赖冲突或 legacy
文件/文件夹删除，且未传入 `"allow_conflicts": true`，工具会返回
`project_package_conflicts`，并在 `error.data.changes` 中附带变更摘要；此时不会应用到项目。
确认后重试并设置 `"allow_conflicts": true` 才会继续 apply。

包列表工具只返回适合发现和筛选的摘要字段：`name`、`displayName`、`version` 和 `source`。
列表中的 `totalCount` 和分页字段按聚合后的摘要条目计算，不是仓库原始版本清单的长度。
需要读取描述、关键词、依赖、legacy 包、文档 URL、变更日志 URL 或 Unity 版本要求时，应先从列表中选出候选包，
再调用 `alcomd3_get_package_details` 获取详细元数据。

包列表工具默认 `offset` 为 `0`、`limit` 为 `200`；`limit` 最大为 `1000`，超过时会被限制到最大值。
分页响应包含 `totalCount`、`offset`、`limit`、`returnedCount`、`hasMore` 和 `nextOffset`。
需要读取完整列表时，应在 `hasMore` 为 `true` 时使用 `nextOffset` 继续请求下一页。
包相关工具不再返回 `count` 字段。

## 生命周期和客户端行为

GUI 加载本地配置后会绑定一个内置 `alcomd3-mcp` Streamable HTTP server。
MCP `2026-07-28` 请求无 session，并且每次请求都必须携带标准协议 metadata；普通
`2025-11-25` 客户端继续使用 legacy session。两条路径共享 GUI 状态、限流器、任务管理器、
资源锁和活动记录器。

ALCOMD3 的生命周期边界：

- GUI 退出或 MCP 扩展关闭时会停止 HTTP listener，等待服务任务结束并取消未完成操作。
- endpoint URL 与 bearer token 对当前本机安装保持稳定；GUI 重启后客户端无需修改配置即可重连。
- GUI 中的客户端区域按客户端名称和版本归并“最近活动”，不是实时 session 列表；工具高亮
  表示当前正在处理的调用。
- GUI 不可用时，本机 endpoint 也不可用，因为不存在继续运行的独立 MCP 进程。
- GUI 可用但 MCP 停用期间，新的 tool call 返回结构化 `mcp_disabled` 错误；已启动
  项目长任务仍可通过已认证的 `tasks/get` 查询或 `tasks/cancel` 取消。
- GUI 重新启动后会再次绑定已配置的 loopback 端口，客户端后续请求可以重连；工具是否
  返回数据仍取决于 GUI 中的 MCP 启用开关。
- 如果配置端口已被占用，MCP 页面会显示 server 未运行，技术日志会记录启动错误。

## 错误和排障

### `mcp_disabled`

MCP 页面处于停用状态。endpoint 仍可能显示运行中，这是正常状态；启用 MCP 后重新调用
工具即可返回数据。已经启动的项目长任务是例外，客户端仍可使用 `tasks/get`、
`tasks/cancel` 查询结果或取消任务。

### `rate_limited`

内置 server 在短时间内收到过多 tool call，或已有 64 个 tool call 正在执行。每分钟
最多启动 600 次 tool call，达到限制后请稍后重试。

### The MCP endpoint is unavailable

常见原因：

- ALCOMD3 GUI 未运行。
- 客户端 URL 与 GUI 当前显示的 MCP Endpoint 不一致。
- 配置的本机端口已被占用。

处理方式：

1. 启动 ALCOMD3。
2. 在 MCP 页面确认 endpoint running。
3. 重新复制 MCP Endpoint 和授权令牌，更新客户端配置。
4. 重启 MCP 客户端。

Windows 上使用受支持的客户端时，可以再次点击对应的快速配置按钮，按提示确认替换，
然后完全退出并重新启动该客户端。

### HTTP `401 Unauthorized`

bearer token 缺失或与 ALCOMD3 显示的令牌不一致。请更新客户端的 `Authorization` header。

### HTTP `403 Forbidden`

请求携带了不允许的浏览器 `Origin`。ALCOMD3 只接受原生 MCP 客户端和同一 loopback
origin，防止 DNS rebinding 和跨站请求访问本机 server。

### Protocol negotiation errors

使用 MCP `2026-07-28` 时，每次请求都需携带标准 `MCP-Protocol-Version`、`Mcp-Method`
和 `_meta`；或者先初始化 `2025-11-25` legacy session，再执行普通工具调用。server 不
公布其他协议版本。

## 开发 smoke test

在仓库根目录构建包含内置 MCP 服务的 GUI：

```powershell
cargo build -p vrc-get-gui
```

运行 HTTP 生命周期和安全 smoke tests：

```powershell
cargo test -p vrc-get-gui mcp::
```

预期结果：

- `initialize` 成功。
- 携带标准 header 和请求 metadata 时，`2026-07-28` 的 `server/discover` 与普通
  无 session 请求成功。
- `2025-11-25` legacy session 可以初始化并执行普通工具调用。
- `tools/list` 返回当前可用的 MCP 工具。
- `tools/call` 返回 `ok: false` 的可读错误，并在 MCP tool result 上标记
  `isError: true`。
- 缺失或错误 bearer token 返回 HTTP `401`。
- 不允许的 Origin 返回 HTTP `403`。

## 相关源码

- 内置 HTTP/RMCP 服务与工具：`vrc-get-gui/src/mcp/server.rs`
- MCP 生命周期、直接分发、操作与共享状态：`vrc-get-gui/src/mcp/mod.rs`
- 内部 MCP 数据类型：`vrc-get-gui/src/mcp/types.rs`
- GUI 共享后端服务和 MCP capability 矩阵：`vrc-get-gui/src/backend/`
- GUI Tauri commands：`vrc-get-gui/src/commands/mcp.rs`
- GUI MCP 页面：`vrc-get-gui/app/_main/mcp/index.tsx`
- 打包逻辑：`xtask/src/build_alcom.rs`、`xtask/src/bundle_alcom*`

## 参考

- RMCP 3.1.2: <https://github.com/modelcontextprotocol/rust-sdk/releases/tag/rmcp-v3.1.2>
- MCP Specification `2026-07-28`: <https://modelcontextprotocol.io/specification/2026-07-28>
- MCP Specification `2025-11-25`: <https://modelcontextprotocol.io/specification/2025-11-25>
- 实验性 Tasks 扩展：<https://github.com/modelcontextprotocol/ext-tasks>
