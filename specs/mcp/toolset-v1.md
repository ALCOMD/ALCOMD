# ALCOMD MCP Toolset v1

状态：首个六工具切片已验收。回环 HTTP Bearer、预登记 HTTPS/OAuth 和包目录增量各有独立合同及验证记录，按 [M8 当前状态](../../docs/status.md) 查对应候选与覆盖范围。
冻结 MCP 规范基线：`2026-07-28`

正式输入输出、错误枚举及annotations见[包卸载工具Schema](package-removal-v1.schema.json)，完整授权与执行边界见[M8 ExecPlan](../../docs/exec-plans/M8-mcp-protocol-and-management.md#首个真实业务工具合同项目包卸载闭环2026-11-13已批准实施中)。没有v3别名、通用RPC透传或无凭据业务调用。

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
重新读取第一页。绑定还覆盖token上限、resource和时间/ProjectId位置，不扩大权限。
非OAuth继续省略该字段；没有新的工具、State、Config、权限、session或缓存。

## 长任务

核心权威状态为 ALCOMD Operation。`alcomd_get_operation` 需要 `operations.read`，`alcomd_cancel_operation` 需要 `operations.cancel`。两项按真实调用者读取或取消自身任务，使用 Core 的生命周期和修订规则。包变更和状态检查有明确结果字段，其余任务提供状态、修订、时间和稳定错误字段。4.0.0 不采用或广告 MCP Tasks。协议请求取消及 STDIO EOF 只结束当前请求，只有显式 `alcomd_cancel_operation` 请求 Core 合作式业务取消。

## 包目录、安装与重装

获批的增量工具见[工具 Schema](package-catalog-plan-v1.schema.json)与 [Core RPC Schema](../rpc/m8-mcp-packages-v1.schema.json)。目录按当前 `repositories.read`、`packages.read` 展示可访问的包和来源，可选项目上下文另需 `projects.read`。`alcomd_search_packages` 可浏览全局包或按仓库筛选，`alcomd_get_package` 可查版本详情，`alcomd_list_package_sources` 提供选源所需的稳定 ID 和可用状态。受保护来源当前无需向客户端透露设置或凭据。

`alcomd_plan_package_install` 可按最新兼容版本或精确版本生成真实 Plan，`alcomd_plan_package_reinstall` 保持已安装的锁定版本。两项均需项目、仓库、包读取能力。`alcomd_apply_package_changes` 使用相同读取能力、`packages.manage`、原 Plan、显式确认和既有幂等规则。来源和版本均由 Core 冻结、重新核对，输出使用稳定来源 ID 和名称。

MCP、GUI 与 CLI 复用 Core 的已登记来源、可见性、解析与归档校验。获得所需业务权限的 MCP 客户端可使用已登记的公开或认证仓库、带查询参数的来源、本地仓库和用户包。Core 在原有范围内处理仓库凭据与下载头，MCP 响应只给安全的来源标识和普通文档链接。MCP 目录在协商 `mcp.packages.catalog-plan.v1` 后按实时权限展示这些工具。

## 客户端接入速查

为每个客户端单独创建身份，打开实际需要的能力，再由本机所有者用 `alcomd-cli mcp client issue-credential` 签发 Bearer 或 STDIO 凭据。凭据交给客户端的受控秘密入口。每次轮换立即使旧凭据失效。

| 传输 | 入口 |
| --- | --- |
| 回环 HTTP Bearer | 运行 `alcomd-mcp serve --port 0`，将报告的 `/mcp` 地址和该客户端独立 Bearer 配置到支持冻结协议的客户端。 |
| STDIO | 将 `alcomd-mcp stdio` 配置为启动命令，并通过子进程环境提供 `ALCOMD_MCP_CLIENT_CREDENTIAL`。 |
| 回环 HTTPS/OAuth | 预登记客户端和精确回调地址，运行 `alcomd-mcp serve-oauth --port <port> --tls-cert <cert> --tls-key <key>`，客户端使用 `https://localhost:<port>/mcp`。证书须由该客户端信任，OAuth 请求的能力还受注册上限和当前授权约束。 |

已验证的 Codex CLI 0.153.4 在隔离配置启用 `mcp_2026_07_28` 后可用 HTTP Bearer 和预登记 OAuth。该版本的 STDIO 实测仍发送旧握手，需使用现代 HTTP 入口。配置自动写入、其他客户端兼容性及真实通知消费继续按独立验证记录交付。[接入证据](../../docs/testing/m8-management-manual-20261115.md)
