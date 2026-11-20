# M11 普通 ZIP、首次发布基线和 v3 迁移

所有者批准按当前功能定义 v4 首次发布基线，并明确要求移除未发布的格式和设计。正式安装、更新及发行归 M12。

## 普通 ZIP 和绝对路径

- `backups.create` 协商 `backups.create.v2`，根据项目 ID、revision、已有内容选项和幂等键提交任务。`destinationPath` 可指定绝对 ZIP 文件名，省略时采用 Core 中设置的目录与命名策略。
- `backups.restore` 协商 `backups.restore.v2`，接收 Core 主机上的 `archivePath` 和幂等键。可传 `projectName` 自定义默认项目根中的目录名，或传 `destinationPath` 指定尚不存在的绝对项目目录。默认建议名来自 ZIP 文件名。两方法返回 `operationId`、`replayed`，文件或已登记项目在任务成功后可用。
- 解压前检查 ZIP 根部的 Unity 工程结构、条目路径、类型与配额。源文件、目标父目录和发布位置受身份复验、同卷暂存和独占发布约束。无效 ZIP 统一按当前根结构判断。项目已经存在时保留原目录。
- MCP 项目登记、创建和复制接受绝对路径。备份使用 `alcomd_create_backup` 和 `alcomd_restore_backup`，恢复继续要求确认。现有身份、权限和撤权检查继续执行。路径始终指 Core 所在主机，任务响应与普通错误保持脱敏。

GUI、CLI 和 MCP 的入口、对应 Core 用例及自动化测试见[绝对路径对照表](../testing/M11-absolute-paths.md)。

## 首次发布数据基线

- State46 由一份初始 DDL 直接创建。已存在的数据库按当前标识、版本和结构识别，旧库与未知内容在启用 WAL 或恢复之前拒绝。
- 生命周期和数据边界见 [State46](../../specs/storage/state-v46.md) 与 [Config13](../../specs/config/settings-v13.md)。
- Config13 使用当前完整 JSON/TOML 合同中的生效字段。设置按现行 TOML 文档读取和写入。Core 在打开数据库后执行持久任务恢复，打开任何一侧前同时预检数据库与配置。
- 已撤销的备份登记、归档清单、恢复计划、MCP 逐路径审批、开发数据升级和旧模板文件自动清理从生产路径移除。保留通用任务、幂等回执、项目登记及恢复所需的文件系统日志。
- 公共变更影响测试期 v4 客户端和数据。协议 major 仍为 1，ZIP 和 MCP 工作流通过更新后的 capability 协商。

## 首发包版本偏好

所有者批准 Config13 的包设置增加 `preferPrerelease`（默认关闭），`showPrerelease` 新配置默认开启。关闭显示在同一次更新中清除偏好。所有者批准 MCP 安装工具和通用安装、升级意图省略版本选择器，缺省使用 Core 推荐；显式 Unity 兼容性例外仍要求精确版本。候选摘要新增 `recommended`，保留 latest 系列字段含义。设置和摘要响应增加必需字段，对严格校验旧响应结构的测试期客户端属于破坏性变化。仓库管理清单仍完整，项目可选目录与 Apply 继续服从来源可见性。

## 验收状态

本阶段实施中。以下结果来自 2026-11-21 的 Windows 工作区。

- MCP 绝对路径真实 IPC 通过 1 项。原生复制与重启恢复通过 8 项。MCP HTTP 与 OAuth 端到端通过 2 项，覆盖配置目录之外的 ZIP、同键恢复和撤权。[三端对照表](../testing/M11-absolute-paths.md)
- State46 结构合同通过 5 项，Store 单元通过 90 项。
- Windows 外部 Node 开发包和 Rust SDK 通过真实 IPC 复验。普通 ZIP 的真实 IPC 套件通过 6 项，包含两套子进程阶段终止矩阵。配置拒绝、重开与备份目录检查通过 3 项。
- 前端检查及构建通过。735 项 Chromium 单轮通过 734 项，项目网格几何有一项超时。该项在相同源码上按原断言单独复测通过。
- 本机测试验证普通 ZIP 的内容选择、取消、重启恢复和目标保护。整仓 Rust 门禁以最终复验结果为准。

签名候选需要三平台 Hosted CI 和 CodeQL 的同一 SHA。旧钥签名、真实数据删除及远端写入各有授权边界。真实 3.4.0 样本、各平台部署交接和持久阶段故障注入仍待验收。
