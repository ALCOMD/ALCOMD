# ALCOMD MCP Toolset v1

状态：Draft
候选 MCP 规范基线：`2026-07-28`

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

候选工具：

```text
alcomd_list_projects
alcomd_get_project
alcomd_search_packages
alcomd_plan_package_install
alcomd_apply_package_changes
alcomd_create_backup
alcomd_get_operation
alcomd_cancel_operation
```

M-1 必须根据 v3 工具集和完整用例补齐。

## 长任务

核心权威状态为 ALCOMD Operation。是否实现 MCP Tasks 扩展由独立 ADR 决定。无论选择何种 MCP 表现，取消都必须映射到同一个 Operation。
