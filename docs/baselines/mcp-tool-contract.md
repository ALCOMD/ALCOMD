# MCP 工具用例合同基线

状态：M-1 合同基线已由 A-026 批准；正式 Schema 与实现留在后续里程碑

本页把冻结 v3.4.0 的 33 个 MCP 用户工具逐项映射到 v4 用户能力。它是实现与 Schema 的
验收输入，不是已经发布的 MCP ABI。M-1 不修改 `specs/` 或生产代码；A-026 已批准本页公开
工具名作为命名基线、新权限 `diagnostics.read` 和结构化错误方向，后续里程碑必须据此生成
正式 Schema、快照与兼容测试。

证据：

- `docs/baselines/alcomd3-v3-audit.md::MCP`
- 冻结 v3 源码中的 `docs/mcp/tools.zh-CN.md`、`src/mcp/tools.rs`、`src/mcp/mod.rs`
- `docs/baselines/mcp.md`
- A-021、A-023 与 ADR 0006

## 不继承的 v3 协议细节

- 不保留 `alcomd3_*` 名称、嵌入 GUI 的生命周期、固定共享 token 或旧 Tasks 映射。
- 已登记项目、模板、仓库、包、备份与 Operation 优先使用核心稳定 ID；只在登记外部路径、
  创建目标或导入备份时接受路径，并由核心做规范化、授权根与逃逸检查。
- 工具只能调用公开 ALCOMD RPC，再进入 application use case。MCP 适配器不读数据库、日志
  文件、仓库缓存或 Unity 项目。
- `clientInfo`、工具名、资源 ID、路径或 OperationId 都不能证明 Principal 身份。

## 通用工具结果

只读成功结果为 `resultType = complete`，`structuredContent` 至少包含：

```text
data
revision?       # 被读取资源或集合的 revision
nextCursor?     # 分页时使用；禁止 offset 作为跨 revision 快照保证
```

列表按同一 revision 稳定排序。权限过滤结果的 `cacheScope` 默认为 `private`；TTL 到期、对应
资源事件或权限撤销会使缓存失效。

短写成功结果返回更新后的资源 ID 与 revision。所有可重试写入都要求 `idempotency_key`；修改
既有资源还要求 `expected_revision`。长写成功结果只返回持久化的：

```text
operation_id
state
resource
created_at
```

返回前 Operation 必须可由 `alcomd_get_operation` 读取。MCP 请求断开不得默认取消它。

输入不符合工具 JSON Schema 时使用 JSON-RPC `-32602`。工具已进入业务用例后的失败返回
`resultType = complete`、`isError = true` 和稳定结构：

```text
error.code
error.message       # 可操作、已脱敏
error.retryable
error.details?      # 不含 token、Authorization 或完整私密路径
error.operation_id?
error.diagnostic_id? # internal_error 必有；用于受控关联脱敏诊断
```

通用领域错误为 RPC v1 已列出的 `authorization_required`、`permission_denied`、
`invalid_request`、`resource_conflict`、`revision_conflict`、`operation_not_found`、
`operation_requires_input`、`operation_cancelled` 与 `internal_error`。`internal_error` 必须返回
不含敏感信息的 `diagnostic_id`，不得把内部错误字符串当作公开消息。下表的“错误族”引用本页
后续定义的候选细分错误；A-026 允许正式 Schema 合并语义重复项，但不允许退化为任意字符串。

## 33 个冻结用例的 v4 映射

“同步”表示一个短 Query/Command；“Operation”表示立即返回 `OperationId`；“Plan/Apply”表示
计划工具不写项目，应用工具重新验证 revision、权限与资源锁后创建 Operation。

### 项目、备份与恢复（7）

| # | v3 用例 | v4 工具 | 关键输入/结果 | 最小权限与 scope | 执行 | 错误族 |
|---:|---|---|---|---|---|---|
| 1 | `list_projects` | `alcomd_list_projects` | `cursor?`, `limit?` → project summaries、revision | `projects.read`；仅返回 Principal 可读项目 | 同步 | `E-PROJECT` |
| 2 | `get_project_details` | `alcomd_get_project` | `project_id` → Unity、类型、解析状态、已安装包 | `projects.read`；目标 project | 同步 | `E-PROJECT` |
| 3 | `create_project` | `alcomd_create_project` | name、受控 base location、template/unity selector、idempotency key → Operation | `projects.manage`、`templates.read`、`unity.read`；目标根 | Operation | `E-PROJECT`, `E-TEMPLATE`, `E-UNITY` |
| 4 | `add_existing_project` | `alcomd_register_project` | 绝对 project path、idempotency key → project ID/revision | `projects.manage`；授权导入根 | 同步 | `E-PROJECT`, `E-PATH` |
| 5 | `backup_project` | `alcomd_create_backup` | project ID、可选名称、exclude VPM、idempotency key → Operation | `projects.read`, `backups.manage`；目标 project | Operation | `E-PROJECT`, `E-BACKUP` |
| 6 | `copy_project` | `alcomd_copy_project` | source project ID、受控目标、idempotency key → Operation | `projects.read`, `projects.manage`；源项目与目标根 | Operation | `E-PROJECT`, `E-PATH` |
| 7 | `restore_project_from_backup` | `alcomd_plan_backup_restore` + `alcomd_apply_backup_restore` | backup ID/受控导入文件、目标预览 → Plan；plan ID、revision、idempotency key → Operation | plan: `backups.read`, `projects.read`；apply: `backups.manage`, `projects.manage`；备份与目标根 | Plan/Apply | `E-BACKUP`, `E-PROJECT`, `E-PATH`, `E-PLAN` |

### 模板（9）

| # | v3 用例 | v4 工具 | 关键输入/结果 | 最小权限与 scope | 执行 | 错误族 |
|---:|---|---|---|---|---|---|
| 8 | `list_templates` | `alcomd_list_templates` | cursor/limit → 摘要、能力标记、revision | `templates.read` | 同步 | `E-TEMPLATE` |
| 9 | `get_template` | `alcomd_get_template` | template ID → 可读定义，不返回存储路径 | `templates.read`；目标 template | 同步 | `E-TEMPLATE` |
| 10 | `create_template` | `alcomd_create_template` | display name、base ID、Unity range、VPM ranges、附件引用、idempotency key → template/revision | `templates.manage`, `packages.read`, `unity.read`；附件授权根 | 同步 | `E-TEMPLATE`, `E-PATH` |
| 11 | `edit_template` | `alcomd_replace_template` | template ID、完整定义、expected revision、idempotency key → template/revision | `templates.manage`；目标 template | 同步 | `E-TEMPLATE` |
| 12 | `set_template_package` | `alcomd_set_template_package` | template ID、package ID、range、revision、idempotency key → template/revision | `templates.manage`, `packages.read`；目标 template | 同步 | `E-TEMPLATE`, `E-PACKAGE` |
| 13 | `remove_template_package` | `alcomd_remove_template_package` | template ID、package ID、revision、idempotency key → template/revision | `templates.manage`；目标 template | 同步 | `E-TEMPLATE`, `E-PACKAGE` |
| 14 | `set_template_unitypackage` | `alcomd_set_template_unitypackage` | template ID、授权附件引用、revision、idempotency key → template/revision | `templates.manage`；template 与附件根 | 同步 | `E-TEMPLATE`, `E-PATH` |
| 15 | `remove_template_unitypackage` | `alcomd_remove_template_unitypackage` | template ID、附件引用 ID、revision、idempotency key → template/revision | `templates.manage`；目标 template | 同步 | `E-TEMPLATE` |
| 16 | `remove_template` | `alcomd_remove_template` | template ID、revision、idempotency key → removed summary | `templates.manage`；目标 template | 同步 | `E-TEMPLATE` |

### 仓库、包目录与环境（7）

| # | v3 用例 | v4 工具 | 关键输入/结果 | 最小权限与 scope | 执行 | 错误族 |
|---:|---|---|---|---|---|---|
| 17 | `list_repositories` | `alcomd_list_repositories` | cursor/limit → 仓库摘要、可见性、revision；不返回 header 值 | `repositories.read` | 同步 | `E-REPOSITORY` |
| 18 | `add_repository` | `alcomd_add_repository` | HTTPS URL、write-only headers/credential reference、idempotency key → Operation | `repositories.manage`；目标 origin | Operation | `E-REPOSITORY`, `E-CREDENTIAL` |
| 19 | `remove_repository` | `alcomd_remove_repository` | repository ID、revision、idempotency key → removed summary | `repositories.manage`；user repository | 同步 | `E-REPOSITORY` |
| 20 | `list_packages` | `alcomd_search_packages` | query/filter/cursor/limit → 可见 package summaries、revision | `packages.read` | 同步 | `E-PACKAGE` |
| 21 | `list_repository_packages` | `alcomd_list_repository_packages` | repository ID、filter/cursor/limit → package summaries | `repositories.read`, `packages.read`；目标 repository | 同步 | `E-REPOSITORY`, `E-PACKAGE` |
| 22 | `get_package_details` | `alcomd_get_package` | package ID、version/source selector → metadata/source | `packages.read`；可见 source | 同步 | `E-PACKAGE` |
| 23 | `get_environment_settings` | `alcomd_get_environment_settings` | `{}` → 已配置 Unity、默认项目/备份位置摘要；不返回 secret | `settings.read`, `unity.read` | 同步 | `E-UNITY` |

`add_repository.headers` 只允许在受认证调用中作为 write-only secret 输入，必须进入 OS credential
store 或仅用于一次请求；不得进入普通配置、缓存、活动、技术日志、错误或导出结果。

### 活动与技术诊断（7）

| # | v3 用例 | v4 工具 | 关键输入/结果 | 最小权限与 scope | 执行 | 错误族 |
|---:|---|---|---|---|---|---|
| 24 | `search_activity_logs` | `alcomd_search_activity` | filter/cursor/limit/include details → 脱敏摘要 | `activity.read`；仅可见资源与 Principal 范围 | 同步 | `E-ACTIVITY` |
| 25 | `get_activity_log_entry` | `alcomd_get_activity` | activity ID、include details → 脱敏详情 | `activity.read`；目标 activity scope | 同步 | `E-ACTIVITY` |
| 26 | `summarize_activity_logs` | `alcomd_summarize_activity` | filter/group/cursor/limit → 聚合 | `activity.read`；仅可见资源范围 | 同步 | `E-ACTIVITY` |
| 27 | `get_activity_log_context` | `alcomd_get_activity_context` | activity ID、before/after/include details → 邻接记录 | `activity.read`；逐条重新过滤 | 同步 | `E-ACTIVITY` |
| 28 | `search_technical_logs` | `alcomd_search_diagnostics` | filter/cursor/limit/preview cap → 已脱敏预览 | 候选 `diagnostics.read`；禁止扩展为 activity 权限 | 同步 | `E-DIAGNOSTIC` |
| 29 | `get_technical_log_entry` | `alcomd_get_diagnostic` | diagnostic ID/message cap → 已脱敏截断详情 | 候选 `diagnostics.read`；目标 diagnostic | 同步 | `E-DIAGNOSTIC` |
| 30 | `summarize_technical_logs` | `alcomd_summarize_diagnostics` | filter/group/cursor/limit → 聚合 | 候选 `diagnostics.read` | 同步 | `E-DIAGNOSTIC` |

技术诊断不得暴露 token、Authorization、credential、完整私密路径或未经脱敏的第三方响应。
`diagnostics.read` 默认不授予外部客户端，且默认结果仍必须脱敏；它不授权原始技术日志、完整
堆栈、完整本地路径、进程信息或凭据。未来确需原始诊断导出时必须使用独立权限和显式审计。
`activity.read` 不得隐式包含这些技术诊断。

### 项目包变更（3）

| # | v3 用例 | v4 工具 | 关键输入/结果 | 最小权限与 scope | 执行 | 错误族 |
|---:|---|---|---|---|---|---|
| 31 | `install_project_package` | `alcomd_plan_package_install` + `alcomd_apply_package_changes` | project/package/version/source selector → Plan；plan ID、project revision、idempotency key → Operation | plan: `projects.read`, `packages.read`；apply: `packages.manage`；目标 project | Plan/Apply | `E-PROJECT`, `E-PACKAGE`, `E-PLAN` |
| 32 | `uninstall_project_package` | `alcomd_plan_package_uninstall` + `alcomd_apply_package_changes` | project/package → Plan；plan ID、project revision、idempotency key → Operation | plan: `projects.read`, `packages.read`；apply: `packages.manage`；目标 project | Plan/Apply | `E-PROJECT`, `E-PACKAGE`, `E-PLAN` |
| 33 | `reinstall_project_package` | `alcomd_plan_package_reinstall` + `alcomd_apply_package_changes` | project/package/source policy → Plan；plan ID、project revision、idempotency key → Operation | plan: `projects.read`, `packages.read`；apply: `packages.manage`；目标 project | Plan/Apply | `E-PROJECT`, `E-PACKAGE`, `E-PLAN` |

Plan 必须展示直接/传递安装与移除、版本、来源、冲突、Unity 不兼容、unlocked 包、legacy
文件/目录和缺失依赖。Apply 禁止 `allow_conflicts` 式布尔绕过；需要同意的内容进入持久
`waiting_for_input`，由显式 Operation 响应工具处理。Plan 过期、项目 revision 变化、权限撤销
或来源内容变化都必须拒绝应用并要求重新计划。

## A-021 Operation 工具

这些工具是 v4 Operation 合同所需的附加工具，不计入冻结 v3 的 33 项：

| 工具 | 输入/结果 | 权限与安全边界 |
|---|---|---|
| `alcomd_get_operation` | operation ID → owner、resource、state、progress、pending input、result/error | `operations.read` + Operation owner/资源 scope；持有 ID 不授权 |
| `alcomd_submit_operation_input` | operation ID、input request ID、typed values、operation revision、idempotency key → accepted revision | 原业务写权限 + owner/scope；只接受当前 outstanding keys |
| `alcomd_approve_operation` | operation ID、approval ID、revision、idempotency key → accepted revision | 原业务写权限 + owner/scope；审批摘要必须与等待项一致 |
| `alcomd_reject_operation` | operation ID、approval ID、reason?、revision、idempotency key → accepted revision | 原业务写权限 + owner/scope；拒绝不等于 transport cancel |
| `alcomd_resume_operation` | interrupted/waiting operation ID、revision、idempotency key → Operation ref | 原业务写权限 + owner/scope；核心重新校验资源与权限 |
| `alcomd_cancel_operation` | operation ID、revision、idempotency key → cancel accepted | `operations.cancel` + owner/scope；合作式，不承诺最终为 cancelled |

普通 HTTP SSE 关闭或 STDIO `notifications/cancelled` 只结束当前 MCP 请求。响应丢失时必须以
原幂等键找回同一个 Operation；适配器不得创建第二个 Operation 或默认取消第一个。

## 候选错误族

| 错误族 | 候选稳定错误码 |
|---|---|
| `E-PROJECT` | `project_not_found`, `project_invalid`, `project_already_registered`, `project_in_use` |
| `E-TEMPLATE` | `template_not_found`, `template_not_editable`, `template_not_removable`, `template_cycle`, `template_dependency_invalid` |
| `E-UNITY` | `unity_not_found`, `unity_incompatible`, `unity_version_invalid` |
| `E-BACKUP` | `backup_not_found`, `backup_invalid`, `backup_target_exists` |
| `E-PATH` | `path_invalid`, `path_out_of_scope`, `path_escape`, `path_type_unsupported` |
| `E-REPOSITORY` | `repository_not_found`, `repository_duplicate`, `repository_invalid`, `repository_unavailable`, `repository_protected` |
| `E-CREDENTIAL` | `credential_required`, `credential_unavailable`, `credential_rejected` |
| `E-PACKAGE` | `package_not_found`, `package_version_not_found`, `package_source_ambiguous`, `package_incompatible`, `package_dependencies_missing`, `package_conflict` |
| `E-ACTIVITY` | `activity_not_found`, `activity_filter_invalid` |
| `E-DIAGNOSTIC` | `diagnostic_not_found`, `diagnostic_filter_invalid` |
| `E-PLAN` | `plan_not_found`, `plan_expired`, `plan_stale`, `plan_requires_approval`, `plan_already_applied` |

所有 `not_found` 必须在未授权与真实不存在之间避免形成跨 scope 枚举信道。网络、文件系统、
解析器或内部 crate 错误不得直接变成公开错误码或原始字符串。

## 差异测试合同

`mcp.tool-parity` 必须以 33 个表格行号作为稳定 case ID，并对 STDIO 与 HTTP 运行同一语义
fixture。每个 case 至少验证：

1. `tools/list` 中工具名、input/output Schema 与 destructive/read-only hint。
2. 最小权限成功；无权限、错误资源 scope、撤权后重试均失败。
3. 输入边界、稳定排序/分页、脱敏和错误码快照。
4. 写入的 revision、幂等重放与并发冲突。
5. Operation 创建后立即可读、客户端/适配器/daemon 重启恢复及 owner 隔离。
6. MCP 适配器不直接写项目、数据库、配置、日志或 credential store。

另外：

- `mcp.operation-input` 覆盖 submit/approve/reject/resume 的缺键、重复、过期、越权和竞态。
- `mcp.request-operation-cancel` 覆盖提交前、Operation 创建中、响应丢失后和显式 cancel 竞态。
- `mcp.principal-operation-isolation` 使用两个 HTTP Bearer Principal 与两个项目交错至少 100 次，
  验证后端 RPC 连接、缓存、审计和 Operation owner 不串线。
- 包 Apply、备份恢复和路径输入复用项目事务、archive/path-safety、资源锁与故障注入测试。

## A-026 已批准方向

A-026 已批准：

1. 上表的 v4 工具名称作为正式 Schema 命名基线；33 项保持功能覆盖但可映射为多步
   Plan/Apply。正式 Schema 冻结后，重命名必须版本化或提供兼容别名。
2. 新权限 `diagnostics.read`；其他权限复用现有通用权限，Operation 响应继续要求原业务写权限
   与 owner/scope，不新增万能审批权限。
3. 候选错误族作为稳定公开错误码集合的设计输入；后续 Schema 可以合并语义重复项，但所有
   公开错误必须保留机器可读 code，未知内部错误使用 `internal_error + diagnostic_id`。
