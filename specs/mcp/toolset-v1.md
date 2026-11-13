# ALCOMD MCP Toolset v1

状态：2026-11-13 已批准首个六工具切片，实施中；其余工具仍为方向基线，未开放。
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

本切片仅六个获批工具：

```text
alcomd_list_projects
alcomd_get_project
alcomd_plan_package_uninstall
alcomd_apply_package_changes
alcomd_get_operation
alcomd_cancel_operation
```

项目查询只展示当前授权的登记快照，Plan重读当前文件。Apply要求显式confirm=true，只应用本Principal的原remove Plan，接受Operation不代表业务成功。来源委托、安装/重装、备份工具及管理扩展不在本切片。

## 长任务

核心权威状态为 ALCOMD Operation；本切片任务查询/取消只支持packages.apply。4.0.0不采用或广告MCP Tasks。协议请求取消/STDIO EOF只结束当前请求，不能默认取消已经接受的Operation；只有显式cancel_operation请求Core合作式业务取消。
