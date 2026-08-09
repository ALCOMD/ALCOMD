# ALCOMD3 MCP 工具参考

[English](tools.md) | [繁體中文](tools.zh-TW.md) | [日本語](tools.ja.md)

本文档以当前公开的 33 个 MCP 工具实现为准，适合在编写 Agent、排查调用或检查兼容性时查阅。
连接、鉴权、生命周期和客户端配置请参阅 [MCP 主指南](mcp.zh-CN.md)。

## 如何阅读

- 输入字段使用 `snake_case`，输出字段通常使用 `camelCase`；请以每个工具的表格为准。
- “必填”为“是”的字段必须传入；“否”表示可以省略。省略后的默认行为写在字段说明中。
- 无输入字段的工具仍应传入空对象 `{}`。
- `string \| null` 表示字段始终存在，但当前值可能为空；“出现条件”则说明字段是否只在特定情况下出现。
- 运行时的 `tools/list` 会提供每个工具的 `inputSchema`。目前
  `alcomd3_list_repositories` 还提供严格的 `outputSchema`。
- 工具返回的是 MCP `structuredContent`。大多数成功结果包含 `ok: true`，但
  `alcomd3_get_activity_log_entry` 和 `alcomd3_get_technical_log_entry`
  直接返回详情对象，不包含 `ok`；下文逐项注明。

业务错误统一返回：

```json
{
    "ok": false,
    "error": {
        "code": "error_code",
        "message": "Actionable message",
        "data": {}
    }
}
```

其中 `error.data` 仅在错误需要附带结构化上下文时出现，MCP 外层结果同时带有
`isError: true`。参数结构错误、业务错误和协议错误的区别请参阅
[MCP Tools 规范](https://modelcontextprotocol.io/specification/2025-11-25/server/tools#error-handling)。

## 快速索引

| 分类 | 工具 | 行为 | 用途 |
| --- | --- | --- | --- |
| 项目 | `alcomd3_list_projects` | 只读 | 列出已登记项目。 |
| 模板 | `alcomd3_list_templates` | 只读 | 列出可用的环境级模板。 |
| 模板 | `alcomd3_get_template` | 只读 | 读取一个环境级模板。 |
| 模板 | `alcomd3_create_template` | 写入 | 创建派生模板。 |
| 模板 | `alcomd3_edit_template` | 破坏性写入 | 通过整体替换定义来编辑派生模板。 |
| 模板 | `alcomd3_set_template_package` | 幂等写入 | 设置一个直接 VPM 依赖。 |
| 模板 | `alcomd3_remove_template_package` | 破坏性写入 | 移除一个直接 VPM 依赖。 |
| 模板 | `alcomd3_set_template_unitypackage` | 幂等写入 | 设置一个 UnityPackage 附件引用。 |
| 模板 | `alcomd3_remove_template_unitypackage` | 破坏性写入 | 移除一个 UnityPackage 附件引用。 |
| 模板 | `alcomd3_remove_template` | 破坏性写入 | 将可删除模板移入回收站。 |
| 项目 | `alcomd3_get_project_details` | 只读 | 读取已登记项目详情。 |
| 仓库 | `alcomd3_list_repositories` | 只读 | 列出远程仓库和包显示设置。 |
| 仓库 | `alcomd3_add_repository` | 外部网络写入 | 添加远程 VPM 仓库。 |
| 仓库 | `alcomd3_remove_repository` | 破坏性写入 | 按 URL 删除用户仓库。 |
| 软件包 | `alcomd3_get_package_details` | 只读 | 读取可见软件包详细元数据。 |
| 软件包 | `alcomd3_list_packages` | 只读 | 分页列出所有 GUI 可见软件包摘要。 |
| 软件包 | `alcomd3_list_repository_packages` | 只读 | 分页列出一个仓库的软件包摘要。 |
| 环境 | `alcomd3_get_environment_settings` | 只读 | 读取 Unity 安装和默认路径设置。 |
| 活动记录 | `alcomd3_search_activity_logs` | 只读 | 筛选并分页读取活动摘要。 |
| 活动记录 | `alcomd3_get_activity_log_entry` | 只读 | 读取一条完整活动记录。 |
| 活动记录 | `alcomd3_summarize_activity_logs` | 只读 | 聚合活动记录。 |
| 活动记录 | `alcomd3_get_activity_log_context` | 只读 | 读取一条活动前后的上下文。 |
| 技术日志 | `alcomd3_search_technical_logs` | 只读 | 筛选并分页读取技术日志预览。 |
| 技术日志 | `alcomd3_get_technical_log_entry` | 只读 | 读取一条技术日志详情。 |
| 技术日志 | `alcomd3_summarize_technical_logs` | 只读 | 聚合技术日志。 |
| 项目 | `alcomd3_create_project` | 长任务写入 | 创建并登记 Unity 项目。 |
| 项目 | `alcomd3_add_existing_project` | 写入 | 登记已有 Unity 项目。 |
| 项目 | `alcomd3_backup_project` | 长任务写入 | 创建项目 zip 备份。 |
| 项目 | `alcomd3_copy_project` | 长任务写入 | 复制并登记项目。 |
| 项目 | `alcomd3_restore_project_from_backup` | 长任务写入 | 从 zip 恢复并登记项目。 |
| 项目包 | `alcomd3_install_project_package` | 长任务写入 | 安装一个 VPM 包。 |
| 项目包 | `alcomd3_uninstall_project_package` | 破坏性长任务 | 卸载一个已安装包。 |
| 项目包 | `alcomd3_reinstall_project_package` | 长任务写入 | 重装一个已安装包。 |

“长任务”表示工具声明 `execution.taskSupport: "optional"`：支持 MCP Tasks 的客户端可以异步轮询，
不支持 Tasks 的客户端仍可用普通同步 `tools/call`。完整行为见
[项目长任务](mcp.zh-CN.md#项目长任务)。

## 项目和模板

### `alcomd3_list_projects`

列出 ALCOMD3 数据库中登记的项目，不扫描未登记目录。

**输入：** 无字段，传 `{}`。

**成功输出：**

| 字段 | 类型 | 出现条件 | 含义 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功时为 `true`。 |
| `projects` | [`ProjectSummary[]`](#projectsummary) | 必有 | 已登记项目摘要；无法形成有效路径摘要的记录会被跳过。 |

### `alcomd3_list_templates`

列出当前可用于新建项目的环境级模板。这些模板不是某个已登记项目拥有的模板数据。模板源文件路径不会返回。

**输入：** 无字段，传 `{}`。

**成功输出：**

| 字段 | 类型 | 出现条件 | 含义 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功时为 `true`。 |
| `templates` | [`TemplateSummary[]`](#templatesummary) | 必有 | 模板摘要和能力标记。 |

### `alcomd3_get_template`

按稳定模板 ID 读取一个环境级模板；此调用不会检查已登记项目。ID 仅用于选择读取对象，不会使只读调用变成写入。

**输入：**

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `template_id` | `string` | 是 | `alcomd3_list_templates` 返回的模板 `id`；去除首尾空白后不能为空。 |

**成功输出：**

| 字段 | 类型 | 出现条件 | 含义 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功时为 `true`。 |
| `template` | [`TemplateDetails`](#templatedetails) | 必有 | 模板摘要和可读取定义；不包含模板存储路径。 |

### `alcomd3_create_template`

创建一个派生模板。后端生成并持久化模板 ID。

**输入：**

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `display_name` | `string` | 是 | 用户可见模板名称。 |
| `base_template_id` | `string` | 是 | 必须指向 `usableAsBase: true` 的现有模板。 |
| `unity_version_range` | `string` | 是 | 可解析的 Unity 版本范围。 |
| `vpm_dependencies` | `object<string, string>` | 是 | VPM 包名到版本范围的完整映射。 |
| `unitypackage_paths` | `string[]` | 是 | 已存在的绝对 `.unitypackage` 普通文件路径。可传空数组。 |

**成功输出：**

| 字段 | 类型 | 出现条件 | 含义 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功时为 `true`。 |
| `template` | [`TemplateDetails`](#templatedetails) | 必有 | 新建模板的持久化定义和生成 ID。 |

附件只被引用，不会复制。自引用、基模板依赖环、无效包名、无效版本范围和无效附件路径会被拒绝。

### `alcomd3_edit_template`

整体替换一个派生模板的可编辑定义；模板 ID 和存储位置保持不变。

**输入：**

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `template_id` | `string` | 是 | 要编辑的派生模板 ID。 |
| `display_name` | `string` | 是 | 替换后的显示名称。 |
| `base_template_id` | `string` | 是 | 替换后的基模板 ID。 |
| `unity_version_range` | `string` | 是 | 替换后的 Unity 版本范围。 |
| `vpm_dependencies` | `object<string, string>` | 是 | 替换后的完整 VPM 依赖映射。 |
| `unitypackage_paths` | `string[]` | 是 | 替换后的完整附件路径列表。 |

**成功输出：**

| 字段 | 类型 | 出现条件 | 含义 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功时为 `true`。 |
| `template` | [`TemplateDetails`](#templatedetails) | 必有 | 编辑后的完整定义。 |

内置模板和项目归档模板不能字段级编辑。本工具标记为 destructive，因为采用整体替换语义。

### `alcomd3_set_template_package`

为派生模板设置一个直接 VPM 依赖。它只保存包名和版本范围声明，不选择仓库、不解析依赖，也不安装文件。

**输入：**

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `template_id` | `string` | 是 | 可编辑的派生模板 ID。 |
| `package_name` | `string` | 是 | 合法的完整 VPM 包名。 |
| `version_range` | `string` | 是 | 要新增或替换的可解析 VPM 版本范围。 |

**成功输出：** `ok: true`，并在 `template` 中返回完整的最新 [`TemplateDetails`](#templatedetails)。重复设置相同包名和范围不会写入。

### `alcomd3_remove_template_package`

从派生模板移除一个直接 VPM 依赖声明，不会修改任何已有项目。

**输入：**

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `template_id` | `string` | 是 | 可编辑的派生模板 ID。 |
| `package_name` | `string` | 是 | 要移除的现有直接依赖。 |

**成功输出：** `ok: true`，并在 `template` 中返回完整的最新 [`TemplateDetails`](#templatedetails)。依赖不存在时返回 `template_package_not_found`。

### `alcomd3_set_template_unitypackage`

为派生模板设置一个 UnityPackage 附件引用。

**输入：**

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `template_id` | `string` | 是 | 可编辑的派生模板 ID。 |
| `unitypackage_path` | `string` | 是 | 已存在的绝对 `.unitypackage` 普通文件路径。 |

路径会被规范化，文件只会被引用而不会复制。重复设置同一规范路径不会写入。

**成功输出：** `ok: true`，并在 `template` 中返回完整的最新 [`TemplateDetails`](#templatedetails)。

### `alcomd3_remove_template_unitypackage`

从派生模板移除一个 UnityPackage 附件引用。路径应复制自 `alcomd3_get_template`；引用的文件不会被删除，也不要求仍然存在。

**输入：**

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `template_id` | `string` | 是 | 可编辑的派生模板 ID。 |
| `unitypackage_path` | `string` | 是 | 模板定义中已有的附件路径。 |

**成功输出：** `ok: true`，并在 `template` 中返回完整的最新 [`TemplateDetails`](#templatedetails)。引用不存在时返回 `template_unitypackage_not_found`。

### `alcomd3_remove_template`

把一个可删除模板移入系统回收站。内置模板不可删除，附件文件不会被删除。

**输入：**

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `template_id` | `string` | 是 | 要删除的模板 ID。 |

**成功输出：**

| 字段 | 类型 | 出现条件 | 含义 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功时为 `true`。 |
| `template` | [`RemovedTemplate`](#removedtemplate) | 必有 | 被移除模板的标识、名称和类型。 |

### `alcomd3_get_project_details`

读取一个已登记项目的 Unity 信息和已安装包。不能借此读取任意未登记目录。

**输入：**

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `project_path` | `string` | 是 | 必须精确匹配 ALCOMD3 数据库中的已登记项目路径。 |

**成功输出：**

| 字段 | 类型 | 出现条件 | 含义 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功时为 `true`。 |
| `project` | [`ProjectDetails`](#projectdetails) | 必有 | Unity 版本、解析状态和已安装包。 |

### `alcomd3_create_project`

创建 Unity 项目、解析项目包并登记到 ALCOMD3。支持可选 MCP Task。

**输入：**

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `project_name` | `string` | 是 | 单个合法目录名，不能是路径、根目录或 `..`。 |
| `base_path` | `string` | 否 | 绝对父目录；省略时使用 GUI 默认项目目录。 |
| `template_id` | `string` | 否 | 模板 ID；省略时遵循 GUI 当前模板选择规则。 |
| `unity_version` | `string` | 否 | Unity 版本；省略时遵循 GUI 当前模板选择规则。 |

**成功输出：**

| 字段 | 类型 | 出现条件 | 含义 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功时为 `true`。 |
| `projectPath` | `string` | 必有 | 新项目的绝对路径。 |
| `templateId` | `string` | 必有 | 实际使用的模板 ID。 |
| `unityVersion` | `string` | 必有 | 实际选择的 Unity 版本。 |

### `alcomd3_add_existing_project`

把已有 Unity 项目登记到 ALCOMD3，不复制项目内容。

**输入：**

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `project_path` | `string` | 是 | 指向有效 Unity 项目目录的绝对路径。 |

**成功输出：**

| 字段 | 类型 | 出现条件 | 含义 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功时为 `true`。 |
| `projectPath` | `string` | 必有 | 实际登记的项目路径。 |

### `alcomd3_backup_project`

为已登记项目创建 zip 备份。支持可选 MCP Task。

**输入：**

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `project_path` | `string` | 是 | 已登记项目路径。 |
| `backup_name` | `string` | 否 | 不含 `.zip` 的单一合法文件名；省略时自动生成。不能传路径。 |
| `exclude_vpm_packages` | `boolean` | 否 | 为 `true` 时排除已安装 VPM 包内容；默认 `false`。 |

**成功输出：**

| 字段 | 类型 | 出现条件 | 含义 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功时为 `true`。 |
| `backupPath` | `string` | 必有 | 创建出的 zip 绝对路径。 |

备份总是写入 GUI 配置的备份目录，不覆盖已有文件。

### `alcomd3_copy_project`

复制一个已登记项目，并登记复制后的项目。支持可选 MCP Task。

**输入：**

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `source_project_path` | `string` | 是 | 已登记源项目路径。 |
| `new_project_path` | `string` | 是 | 尚不存在的绝对目标目录，且不能位于源项目内。 |

**成功输出：**

| 字段 | 类型 | 出现条件 | 含义 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功时为 `true`。 |
| `projectPath` | `string` | 必有 | 复制并登记后的项目路径。 |

### `alcomd3_restore_project_from_backup`

从 zip 备份恢复项目到 GUI 默认项目目录并登记。支持可选 MCP Task。

**输入：**

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `backup_path` | `string` | 是 | ALCOMD3 zip 备份的绝对文件路径。 |
| `project_name` | `string` | 否 | 恢复后的单一合法目录名；省略时使用备份文件名。 |

**成功输出：**

| 字段 | 类型 | 出现条件 | 含义 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功时为 `true`。 |
| `projectPath` | `string` | 必有 | 恢复并登记后的项目路径。 |

## 仓库、软件包和环境

### `alcomd3_list_repositories`

列出所有受支持的远程仓库以及影响包可见性的全局设置。本地仓库不受支持。

**输入：** 无字段，传 `{}`。

**成功输出：**

| 字段 | 类型 | 出现条件 | 含义 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功时为 `true`。 |
| `repositories` | [`RepositorySummary[]`](#repositorysummary) | 必有 | 官方、Curated 和用户远程仓库的唯一规范数组。 |
| `packageVisibility` | `object` | 必有 | 全局软件包显示设置。 |
| `packageVisibility.hideLocalUserPackages` | `boolean` | 必有 | 是否隐藏本地用户包。 |
| `packageVisibility.showPrereleasePackages` | `boolean` | 必有 | 是否显示预发布包。 |

包读取工具使用返回的 `id`；删除用户仓库使用返回的 `url`。

### `alcomd3_add_repository`

下载、校验并添加一个远程 VPM 仓库，然后清理包缓存。此工具会访问仓库 URL。

**输入：**

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `repository_url` | `string` | 是 | 有效的远程 VPM 仓库 URL，也是后续删除使用的身份。 |
| `headers` | `object<string, string>` | 否 | 下载仓库时附带的 HTTP header 映射；默认空对象。 |

**成功输出：**

| 字段 | 类型 | 出现条件 | 含义 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功时为 `true`。 |
| `repository` | [`RepositoryMutationSummary`](#repositorymutationsummary) | 必有 | 实际加入的用户仓库摘要。 |

已存 URL 或发布者声明 ID 重复都会被拒绝。活动记录只保存脱敏 URL 和 header 数量，不保存 header 值。

### `alcomd3_remove_repository`

按已存 URL 精确删除一个用户仓库并清理包缓存。默认仓库不能删除。

**输入：**

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `repository_url` | `string` | 是 | `alcomd3_list_repositories` 返回的用户仓库 `url`。只接受 URL，不接受 ID。 |

**成功输出：**

| 字段 | 类型 | 出现条件 | 含义 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功时为 `true`。 |
| `repository` | [`RepositoryMutationSummary`](#repositorymutationsummary) | 必有 | 被删除仓库的摘要。 |

### `alcomd3_list_packages`

分页列出与 GUI 包列表相同的可见软件包摘要。工具不提供服务端文本搜索。

**输入：**

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `offset` | `integer >= 0` | 否 | 起始条目偏移；默认 `0`。 |
| `limit` | `integer >= 0` | 否 | 请求页大小；默认 `200`，实际限制在 `1..=1000`。 |

**成功输出：**

| 字段 | 类型 | 出现条件 | 含义 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功时为 `true`。 |
| `totalCount` | `integer` | 必有 | 过滤并按来源聚合后的摘要总数。 |
| `offset` | `integer` | 必有 | 本次使用的偏移。 |
| `limit` | `integer` | 必有 | 本次实际页大小。 |
| `returnedCount` | `integer` | 必有 | 本页返回数量。 |
| `hasMore` | `boolean` | 必有 | 是否还有下一页。 |
| `nextOffset` | `integer \| null` | 必有 | 下一页偏移；无下一页时为 `null`。 |
| `packages` | [`PackageSummary[]`](#packagesummary) | 必有 | 当前页的软件包摘要。 |

### `alcomd3_list_repository_packages`

分页列出一个远程仓库中的 GUI 可见软件包摘要。

**输入：**

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `repository_id` | `string` | 是 | `alcomd3_list_repositories` 返回的仓库 `id`。只接受 ID，不接受 URL。 |
| `offset` | `integer >= 0` | 否 | 起始偏移；默认 `0`。 |
| `limit` | `integer >= 0` | 否 | 页大小；默认 `200`，实际限制在 `1..=1000`。 |

**成功输出：** 与 `alcomd3_list_packages` 的分页字段相同，并额外包含：

| 字段 | 类型 | 出现条件 | 含义 |
| --- | --- | --- | --- |
| `repository` | [`PackageRepositorySummary`](#packagerepositorysummary) | 必有 | 被读取仓库的摘要。 |
| `packages` | [`PackageSummary[]`](#packagesummary) | 必有 | 仅来自指定仓库的当前页摘要。 |

### `alcomd3_get_package_details`

读取一个 GUI 可见包的详细元数据。省略筛选字段时可能返回多个来源或版本。

**输入：**

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `package_name` | `string` | 是 | 完整 VPM 包标识；去除首尾空白后不能为空。 |
| `version` | `string` | 否 | 精确版本字符串。 |
| `repository_id` | `string` | 否 | 将结果限制到指定远程仓库；使用仓库列表返回的 ID。只接受 ID。 |

**成功输出：**

| 字段 | 类型 | 出现条件 | 含义 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功时为 `true`。 |
| `packages` | [`PackageDetails[]`](#packagedetails) | 必有 | 所有匹配且 GUI 可见的包详情，至少一项。 |

### `alcomd3_get_environment_settings`

读取 ALCOMD3 当前保存的 Unity 安装、启动参数和默认路径；不启动 Unity，也不扫描额外磁盘。

**输入：** 无字段，传 `{}`。

**成功输出：**

| 字段 | 类型 | 出现条件 | 含义 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功时为 `true`。 |
| `unityInstallations` | [`UnityInstallation[]`](#unityinstallation) | 必有 | 已登记 Unity 安装。 |
| `unityLaunchArguments` | `object` | 必有 | Unity 启动参数来源和有效值。 |
| `unityLaunchArguments.configured` | `string[] \| null` | 必有 | 用户配置；未配置时为 `null`。 |
| `unityLaunchArguments.builtinDefault` | `string[]` | 必有 | ALCOMD3 内置默认参数。 |
| `unityLaunchArguments.effective` | `string[]` | 必有 | 当前实际生效的参数。 |
| `unityLaunchArguments.usesBuiltinDefault` | `boolean` | 必有 | 是否正在使用内置默认值。 |
| `paths` | `object` | 必有 | 默认目录。 |
| `paths.defaultProjectPath` | `string` | 必有 | 默认项目目录。 |
| `paths.projectBackupPath` | `string` | 必有 | 项目备份目录。 |

## 活动记录

### 活动筛选公共输入

`alcomd3_search_activity_logs` 和 `alcomd3_summarize_activity_logs` 共用以下筛选字段：

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `search` | `string` | 否 | 在 operation、summary、target、tool name 和 client name 中做不区分大小写的包含匹配。 |
| `sources` | `("gui" \| "mcp" \| "deep_link" \| "system")[]` | 否 | 限制活动来源。 |
| `kinds` | `("read" \| "write" \| "passive" \| "open" \| "maintenance")[]` | 否 | 限制活动类型。 |
| `statuses` | `("started" \| "succeeded" \| "failed" \| "cancelled" \| "info")[]` | 否 | 限制活动状态。 |
| `visibility` | `"important" \| "primary" \| "secondary" \| "technical" \| "all"` | 否 | 可见性层级；默认 `important`。 |
| `operations` | `string[]` | 否 | 限制内部 operation 标识。 |
| `tool_names` | `string[]` | 否 | 限制 MCP 工具名。 |
| `request_id` | `string` | 否 | 限制 MCP 请求 ID。 |
| `target` | `string` | 否 | 限制操作对象。 |
| `since` | `RFC3339 string` | 否 | 包含的最早时间。 |
| `until` | `RFC3339 string` | 否 | 包含的最晚时间；不得早于 `since`。 |
| `offset` | `integer >= 0` | 否 | 分页偏移；默认 `0`。 |
| `limit` | `integer >= 0` | 否 | 页大小；默认 `50`，实际限制在 `1..=200`。 |
| `order` | `"newest" \| "oldest"` | 否 | 时间顺序；默认 `newest`。 |

### `alcomd3_search_activity_logs`

按公共筛选条件分页读取用户可读活动摘要。

**输入：** [活动筛选公共输入](#活动筛选公共输入)中的任意字段；全部可选，可传 `{}`。

**成功输出：**

| 字段 | 类型 | 出现条件 | 含义 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功时为 `true`。 |
| `totalCount` | `integer` | 必有 | 筛选后的活动总数。 |
| `offset`、`limit`、`returnedCount`、`hasMore`、`nextOffset` | 分页字段 | 必有 | 语义与软件包分页相同。 |
| `entries` | [`ActivityEntrySummary[]`](#activityentrysummary) | 必有 | 当前页活动摘要。 |

### `alcomd3_get_activity_log_entry`

按搜索或汇总结果中的 ID 读取一条完整活动记录。

**输入：**

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `id` | `string` | 是 | 活动记录 ID。 |
| `include_details` | `boolean` | 否 | 是否返回 `details`；默认 `true`。为 `false` 时返回空数组。 |

**成功输出：** 直接返回 [`ActivityEntry`](#activityentry)，不包含 `ok` 包装字段。

### `alcomd3_summarize_activity_logs`

按字段聚合筛选后的活动，用于先定位问题范围。

**输入：** 公共筛选字段，另加：

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `group_by` | `"source" \| "kind" \| "status" \| "operation" \| "tool_name" \| "client_name" \| "day" \| "hour"` | 否 | 聚合维度；默认 `source`。 |

**成功输出：**

| 字段 | 类型 | 出现条件 | 含义 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功时为 `true`。 |
| `groupBy` | `string` | 必有 | 实际聚合维度。 |
| `totalCount` | `integer` | 必有 | 筛选后的活动总数。 |
| `totalGroupCount` | `integer` | 必有 | 分页前的分组总数。 |
| `offset`、`limit`、`returnedCount`、`hasMore`、`nextOffset` | 分页字段 | 必有 | 对分组列表分页。 |
| `groups` | [`ActivitySummaryGroup[]`](#activitysummarygroup) | 必有 | 当前页聚合结果。 |

### `alcomd3_get_activity_log_context`

读取指定活动及其相邻记录，不需要拉取全部日志。

**输入：**

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `id` | `string` | 是 | 中心活动记录 ID。 |
| `before` | `integer >= 0` | 否 | 前置记录数量；默认 `5`，最大 `50`。 |
| `after` | `integer >= 0` | 否 | 后置记录数量；默认 `5`，最大 `50`。 |
| `include_details` | `boolean` | 否 | 是否在三组记录中包含详情；默认 `false`。 |

**成功输出：**

| 字段 | 类型 | 出现条件 | 含义 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功时为 `true`。 |
| `entry` | [`ActivityEntry`](#activityentry) | 必有 | 中心记录。 |
| `before` | [`ActivityEntry[]`](#activityentry) | 必有 | 中心记录之前的活动。 |
| `after` | [`ActivityEntry[]`](#activityentry) | 必有 | 中心记录之后的活动。 |

## 技术日志

### 技术日志筛选公共输入

`alcomd3_search_technical_logs` 和 `alcomd3_summarize_technical_logs` 共用：

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `search` | `string` | 否 | 在 target 和 message 中做不区分大小写的包含匹配。 |
| `levels` | `("error" \| "warn" \| "info" \| "debug" \| "trace")[]` | 否 | 日志级别；默认 `error` 和 `warn`。 |
| `targets` | `string[]` | 否 | target 的不区分大小写包含匹配。 |
| `scope` | `"memory" \| "recent_files"` | 否 | 当前进程内存或近期日志文件；默认 `memory`。 |
| `since` | `RFC3339 string` | 否 | 包含的最早时间。 |
| `until` | `RFC3339 string` | 否 | 包含的最晚时间；不得早于 `since`。 |
| `offset` | `integer >= 0` | 否 | 分页偏移；默认 `0`。 |
| `limit` | `integer >= 0` | 否 | 页大小；默认 `50`，实际限制在 `1..=100`。 |
| `max_message_chars` | `integer >= 0` | 否 | 搜索预览最多字符数；默认且最大为 `300`。汇总结果不含消息文本。 |

### `alcomd3_search_technical_logs`

分页读取已脱敏、有限长度的技术日志预览。

**输入：** [技术日志筛选公共输入](#技术日志筛选公共输入)中的任意字段；全部可选。

**成功输出：**

| 字段 | 类型 | 出现条件 | 含义 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功时为 `true`。 |
| `totalCount` | `integer` | 必有 | 筛选后的日志总数。 |
| `offset`、`limit`、`returnedCount`、`hasMore`、`nextOffset` | 分页字段 | 必有 | 当前页状态。 |
| `entries` | [`TechnicalLogEntrySummary[]`](#technicallogentrysummary) | 必有 | 当前页技术日志预览。 |

### `alcomd3_get_technical_log_entry`

读取搜索结果中的一条日志，消息会先脱敏再截断。

**输入：**

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `id` | `string` | 是 | 搜索结果返回的技术日志 ID。 |
| `max_message_chars` | `integer >= 0` | 否 | 消息最多字符数；默认且最大为 `4000`。 |

**成功输出：** 直接返回 [`TechnicalLogEntryDetails`](#technicallogentrydetails)，不包含 `ok` 包装字段。

### `alcomd3_summarize_technical_logs`

聚合筛选后的技术日志。

**输入：** 技术日志公共筛选字段，另加：

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `group_by` | `"level" \| "target" \| "file" \| "hour"` | 否 | 聚合维度；默认 `level`。 |

**成功输出：**

| 字段 | 类型 | 出现条件 | 含义 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功时为 `true`。 |
| `groupBy` | `string` | 必有 | 实际聚合维度。 |
| `totalCount` | `integer` | 必有 | 筛选后的日志总数。 |
| `totalGroupCount` | `integer` | 必有 | 分页前的分组总数。 |
| `offset`、`limit`、`returnedCount`、`hasMore`、`nextOffset` | 分页字段 | 必有 | 对分组分页。 |
| `groups` | [`TechnicalLogSummaryGroup[]`](#technicallogsummarygroup) | 必有 | 当前页聚合结果。 |

## 项目软件包写入

### `alcomd3_install_project_package`

从 GUI 可见且与项目 Unity 版本兼容的候选中安装一个包。支持可选 MCP Task。

**输入：**

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `project_path` | `string` | 是 | 已登记项目路径。 |
| `package_name` | `string` | 是 | 合法的完整 VPM 包标识。 |
| `version_selector` | `object` | 是 | `{"type":"latest_gui_visible"}`，或 `{"type":"exact","version":"x.y.z"}`。精确版本仍须 GUI 可见且兼容。 |
| `source` | `object` | 否 | 可选远程仓库选择器。 |
| `source.repository_id` | `string` | 提供 `source` 时必填 | `alcomd3_list_repositories` 返回的仓库 `id`；不接受 URL。 |
| `allow_conflicts` | `boolean` | 否 | 是否允许依赖冲突或 legacy 文件/目录删除；默认 `false`。 |

**成功输出：** [`ProjectPackageChangeResult`](#projectpackagechangeresult)。

### `alcomd3_uninstall_project_package`

卸载一个已安装包。支持可选 MCP Task，并标记为 destructive。

**输入：**

| 字段 | 类型 | 必填 | 含义 |
| --- | --- | --- | --- |
| `project_path` | `string` | 是 | 已登记项目路径。 |
| `package_name` | `string` | 是 | 当前项目中已安装的合法 VPM 包标识。 |
| `allow_conflicts` | `boolean` | 否 | 是否允许冲突或 legacy 删除；默认 `false`。 |

**成功输出：** [`ProjectPackageChangeResult`](#projectpackagechangeresult)。

### `alcomd3_reinstall_project_package`

重新安装一个已安装包。支持可选 MCP Task。

**输入：** 与 `alcomd3_uninstall_project_package` 相同。

**成功输出：** [`ProjectPackageChangeResult`](#projectpackagechangeresult)。

三个工具都会先生成 pending changes。若需要明确授权但 `allow_conflicts` 为 `false`，返回
`project_package_conflicts`，并在 `error.data.changes` 中提供同一 [`PendingChanges`](#pendingchanges)
结构；此时不会修改项目。

## 共享输出类型

### `ProjectSummary`

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `name` | `string \| null` | 项目显示名称。 |
| `path` | `string` | 登记路径。 |
| `projectType` | `string` | 后端识别的项目类型。 |
| `unity` | `string \| null` | Unity 版本。 |
| `unityRevision` | `string \| null` | Unity revision。 |
| `lastModified` | `integer \| null` | 最后修改 Unix 毫秒时间。 |
| `createdAt` | `integer \| null` | 创建 Unix 毫秒时间。 |
| `favorite` | `boolean` | 是否收藏。 |
| `exists` | `boolean` | 登记目录当前是否存在。 |

### `TemplateSummary`

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `displayName` | `string` | 模板显示名称。 |
| `id` | `string` | 稳定管理 ID。 |
| `unityVersions` | `string[]` | 可供项目创建选择的 Unity 版本。 |
| `updateDate` | `string \| null` | 模板更新时间。 |
| `hasUnityPackages` | `boolean` | 是否引用 Unity package。 |
| `hasProjectArchive` | `boolean` | 是否包含项目归档。 |
| `available` | `boolean` | 当前模板是否可用。 |
| `kind` | `"builtIn" \| "derived" \| "projectArchive"` | 模板类型。 |
| `editable` | `boolean` | 是否可字段级编辑。 |
| `removable` | `boolean` | 是否可删除。 |
| `usableAsBase` | `boolean` | 是否可作为派生模板的基模板。 |

### `TemplateDetails`

包含全部 [`TemplateSummary`](#templatesummary) 字段，另有：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `baseTemplateId` | `string \| null` | 派生模板的基模板 ID。 |
| `unityVersionRange` | `string \| null` | 派生模板的 Unity 版本范围。 |
| `vpmDependencies` | `object<string, string>` | VPM 包名到版本范围。 |
| `unityPackagePaths` | `string[]` | 被引用的绝对附件路径。 |

### `RemovedTemplate`

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `id` | `string` | 被删除模板 ID。 |
| `displayName` | `string` | 被删除模板名称。 |
| `kind` | `"builtIn" \| "derived" \| "projectArchive"` | 被删除模板类型。 |

### `ProjectDetails`

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `path` | `string` | 项目路径。 |
| `unity.major` | `integer` | Unity 主版本。 |
| `unity.minor` | `integer` | Unity 次版本。 |
| `unity.version` | `string` | 完整 Unity 版本。 |
| `unity.revision` | `string \| null` | Unity revision。 |
| `shouldResolve` | `boolean` | 项目是否需要重新解析包。 |
| `installedPackages` | `object[]` | 已安装包列表。 |
| `installedPackages[].id` | `string` | 项目依赖项中的包 ID。 |
| `installedPackages[].package` | [`PackageDetails`](#packagedetails) | 已安装 manifest 摘要，不含 `source`。 |

### `RepositorySummary`

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `id` | `string` | 包读取工具使用的仓库身份；无声明 ID 时回退到 URL。 |
| `url` | `string` | 远程仓库 URL；用户仓库删除时使用此值。 |
| `name` | `string` | VPM 仓库声明的原名。 |
| `displayName` | `string` | 非空显示名称；初始值为 `name`，用户可以修改。 |
| `kind` | `"officialDefault" \| "curatedDefault" \| "user"` | 唯一仓库分类字段。 |
| `hidden` | `boolean` | 当前是否被 GUI 包可见性设置隐藏。 |

### `RepositoryMutationSummary`

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `id` | `string \| null` | 仓库声明 ID；缺失时可能为 `null`。 |
| `url` | `string` | 已添加或删除的 URL。 |
| `name` | `string \| null` | VPM 仓库声明的原名。 |
| `displayName` | `string` | 操作时的非空显示名称。 |
| `kind` | `"user"` | 固定为用户仓库。 |

### `PackageRepositorySummary`

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `id` | `string \| null` | 缓存仓库 ID。 |
| `url` | `string \| null` | 缓存仓库 URL。 |
| `name` | `string` | VPM 仓库声明的原名。 |
| `displayName` | `string` | 仓库的非空显示名称。 |
| `kind` | `"officialDefault" \| "curatedDefault" \| "user"` | 仓库分类。 |

### `PackageSummary`

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `name` | `string` | 完整 VPM 包标识。 |
| `displayName` | `string \| null` | 显示名称。 |
| `version` | `string` | 版本。 |
| `source` | [`PackageSource`](#packagesource) | 包来源。 |

### `PackageSource`

远程来源：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `type` | `"remote"` | 表示包来自远程仓库。 |
| `kind` | `"officialDefault" \| "curatedDefault" \| "userRepository"` | 远程仓库分类。 |
| `id` | `string \| null` | 仓库声明 ID。 |
| `name` | `string` | VPM 仓库声明的原名。 |
| `displayName` | `string` | 仓库的非空显示名称。 |
| `url` | `string \| null` | 仓库 URL。 |

本地用户包来源：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `type` | `"localUser"` | 表示包来自已登记的本地用户包目录。 |
| `kind` | `"localUser"` | 固定的本地用户包分类。 |
| `isLocalUserPackage` | `true` | 明确标记这是本地用户包。 |

### `PackageDetails`

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `name` | `string` | 完整包标识。 |
| `displayName` | `string \| null` | 显示名称。 |
| `description` | `string \| null` | 包描述。 |
| `version` | `string` | 版本。 |
| `unity` | `object \| null` | Unity 要求，非空时含 `major`、`minor`。 |
| `keywords` | `string[]` | 关键词。 |
| `aliases` | `string[]` | 包别名。 |
| `vpmDependencies` | `string[]` | 依赖包标识列表，不包含版本范围。 |
| `legacyPackages` | `string[]` | 被取代的 legacy 包。 |
| `changelogUrl` | `string \| null` | 变更日志 URL。 |
| `documentationUrl` | `string \| null` | 文档 URL。 |
| `isYanked` | `boolean` | 当前版本是否撤回。 |
| `source` | [`PackageSource`](#packagesource) | 包详情工具返回时必有；项目已安装包摘要中不出现。 |

### `ActivityEntrySummary`

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `id` | `string` | 活动记录 ID，可用于详情和上下文工具。 |
| `startedAt` | `RFC3339 string` | 活动开始时间。 |
| `finishedAt` | `RFC3339 string \| null` | 活动完成时间；尚未完成时为 `null`。 |
| `source` | `string` | 活动来源，例如 `Gui`、`Mcp`、`DeepLink` 或 `System`。 |
| `kind` | `string` | 行为类型，例如 `Read`、`Write` 或 `Maintenance`。 |
| `status` | `string` | 当前或最终状态，例如 `Started`、`Succeeded`、`Failed`。 |
| `importance` | `string` | 可见性级别：`Primary`、`Secondary` 或 `Technical`。 |
| `operation` | `string` | 稳定的内部操作标识。 |
| `summary` | `string` | 面向用户的简短活动说明。 |
| `target` | `string \| null` | 被操作的资源或路径。 |
| `durationMs` | `integer \| null` | 已完成活动的耗时毫秒数。 |
| `requestId` | `string \| null` | 关联的 MCP 请求 ID。 |
| `toolName` | `string \| null` | 关联的 MCP 工具名。 |
| `clientName` | `string \| null` | 发起调用的 MCP 客户端名称。 |
| `detailCount` | `integer` | 完整记录中键值详情的数量。 |
| `hasError` | `boolean` | 完整记录是否包含错误文本。 |
| `errorSummary` | `string \| null` | 已脱敏、截断的错误摘要。 |

### `ActivityEntry`

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `id` | `string` | 活动记录 ID。 |
| `source` | `string` | 活动来源；当前输出使用 `Gui`、`Mcp`、`DeepLink`、`System`。 |
| `kind` | `string` | 行为类型；当前输出使用 `Read`、`Write`、`Passive`、`Open`、`Maintenance`。 |
| `status` | `string` | 活动状态；当前输出使用 `Started`、`Succeeded`、`Failed`、`Cancelled`、`Info`。 |
| `importance` | `string` | 可见性级别：`Primary`、`Secondary` 或 `Technical`。 |
| `operation` | `string` | 稳定的内部操作标识。 |
| `summary` | `string` | 面向用户的活动说明。 |
| `target` | `string \| null` | 操作对象。 |
| `details` | [`ActivityDetail[]`](#activitydetail) | 已脱敏的结构化详情。传 `include_details: false` 时为空数组。 |
| `requestId` | `string \| null` | 关联的 MCP 请求 ID。 |
| `toolName` | `string \| null` | 关联的 MCP 工具名。 |
| `clientName` | `string \| null` | MCP 客户端名称。 |
| `startedAt` | `RFC3339 string` | 活动开始时间。 |
| `finishedAt` | `RFC3339 string \| null` | 活动完成时间。 |
| `durationMs` | `integer \| null` | 活动耗时毫秒数。 |
| `error` | `string \| null` | 已脱敏的完整错误文本。 |

输入筛选枚举使用小写 `snake_case`；上表列出的输出枚举使用当前实现值。

### `ActivityDetail`

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `key` | `string` | 详情键。 |
| `value` | `string` | 已脱敏的详情值。 |

### `ActivitySummaryGroup`

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `key` | `string` | 分组键。 |
| `count` | `integer` | 记录数。 |
| `failedCount` | `integer` | 失败数。 |
| `cancelledCount` | `integer` | 取消数。 |
| `latestEntryId` | `string \| null` | 组内最新记录 ID。 |
| `latestStartedAt` | `string \| null` | 组内最新开始时间。 |

### `TechnicalLogEntrySummary`

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `id` | `string` | 技术日志 ID，可用于详情工具。 |
| `time` | `RFC3339 string` | 日志时间。 |
| `level` | `string` | 当前输出使用 `Error`、`Warn`、`Info`、`Debug` 或 `Trace`。 |
| `target` | `string` | 产生日志的 Rust target。 |
| `messagePreview` | `string` | 已脱敏并按搜索上限截断的消息预览。 |
| `truncated` | `boolean` | 消息是否被截断。 |
| `source` | `"memory" \| "file"` | 日志来自当前进程内存还是近期日志文件。 |
| `fileName` | `string \| null` | 来源日志文件名；内存日志为 `null`。 |
| `lineNumber` | `integer \| null` | 来源日志文件行号；内存日志为 `null`。 |

### `TechnicalLogEntryDetails`

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `id` | `string` | 技术日志 ID。 |
| `time` | `RFC3339 string` | 日志时间。 |
| `level` | `string` | `Error`、`Warn`、`Info`、`Debug` 或 `Trace`。 |
| `target` | `string` | 产生日志的 Rust target。 |
| `message` | `string` | 已脱敏并按详情请求上限截断的消息。 |
| `truncated` | `boolean` | 消息是否被截断。 |
| `source` | `"memory" \| "file"` | 日志来源。 |
| `fileName` | `string \| null` | 来源文件名；内存日志为 `null`。 |
| `lineNumber` | `integer \| null` | 来源文件行号；内存日志为 `null`。 |

### `TechnicalLogSummaryGroup`

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `key` | `string` | 分组键。 |
| `count` | `integer` | 日志数。 |
| `errorCount` | `integer` | Error 数量。 |
| `warnCount` | `integer` | Warn 数量。 |
| `latestEntryId` | `string \| null` | 组内最新日志 ID。 |
| `latestTime` | `string \| null` | 组内最新时间。 |

### `ProjectPackageChangeResult`

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `ok` | `boolean` | 成功时为 `true`。 |
| `operation` | `"install" \| "uninstall" \| "reinstall"` | 实际操作。 |
| `projectPath` | `string` | 被修改项目路径。 |
| `packageName` | `string` | 目标包标识。 |
| `changes` | [`PendingChanges`](#pendingchanges) | 已应用变更摘要。 |

### `PendingChanges`

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `changes_version` | `integer` | 后端变更快照版本。 |
| `package_changes` | `[string, PackageChange][]` | 包名与安装/移除变更的元组列表。 |
| `remove_legacy_files` | `string[]` | 将删除的 legacy 文件。 |
| `remove_legacy_folders` | `string[]` | 将删除的 legacy 目录。 |
| `conflicts` | `[string, ConflictInfo][]` | 包名与冲突详情的元组列表。 |

`PackageChange` 是 `{ "InstallNew": PackageInfo }` 或
`{ "Remove": "Requested" \| "Legacy" \| "Unused" }`。`Remove` 值分别表示用户请求删除、
被其他包取代或已不再被依赖。

### `PackageInfo`

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `name` | `string` | 要安装的完整包标识。 |
| `display_name` | `string \| null` | 包显示名称。 |
| `description` | `string \| null` | 包描述。 |
| `keywords` | `string[]` | 合并后的包别名和关键词。 |
| `version` | `object` | 结构化 SemVer，包含 `major`、`minor`、`patch`、`pre`、`build`。 |
| `unity` | `[integer, integer] \| null` | 所需 Unity 主版本和次版本。 |
| `changelog_url` | `string \| null` | 变更日志 URL。 |
| `documentation_url` | `string \| null` | 文档 URL。 |
| `vpm_dependencies` | `string[]` | 依赖包标识。 |
| `legacy_packages` | `string[]` | 被取代的 legacy 包。 |
| `is_yanked` | `boolean` | 该版本是否已撤回。 |

### `ConflictInfo`

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `packages` | `string[]` | 与目标变更冲突的包标识。 |
| `unity_conflict` | `boolean` | 是否存在 Unity 版本冲突。 |
| `unlocked_names` | `string[]` | 应用变更时需要解锁的包标识。 |

### `UnityInstallation`

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `path` | `string` | Unity 可执行文件路径。 |
| `version` | `string` | 已登记的完整 Unity 版本。 |
| `loadedFromHub` | `boolean` | 此安装是否由 Unity Hub 记录加载。 |
