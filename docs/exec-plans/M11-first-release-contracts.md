# M11 首发合同归一

所有者已逐项核定本批公共合同与末尾 100 个文件路径。CLI 的强类型游标参数复用工作区 serde，以 DeserializeOwned 共用解析和参数错误；逐类型重复实现为备选。沿用锁文件内现有版本和许可证。

本地核定基线 `8eff6433`。本清单记录实施差异，收尾时归并到有效规范并删除仅描述旧定义的清单。所有实现和验证在主工作树进行。

## 公共合同差异

| 首发入口 | 保留的语义 | 同步与验收 |
|---|---|---|
| `packages.plan.v1` | 统一单一 Plan，精确 Unity 例外、用户包、批量和 Apply 复验 | Core、客户端、GUI、SDK、真实 Plan/Apply |
| `packages.candidates.v1` | 完整候选证据、来源、recommended、快照 | Core、GUI、SDK、候选 RPC |
| `projectTemplates.v1` | 当前单文件模板和项目创建 | 模板目录、创建、导入和导出 |
| `backups.create.v1` | 普通 ZIP，接受目的文件 | GUI/CLI/MCP/SDK、ZIP IPC |
| `backups.restore.v1` | 普通 ZIP，接受源文件与恢复目录 | GUI/CLI/MCP/SDK、恢复重开 |
| `mcp.clients.v1` | 连接认证、签发与撤销 | Core、MCP、管理扩展、撤销测试 |
| `mcp.oauth.v1` | OAuth 连接 scope、真实 Principal | OAuth 端到端及生命周期测试 |
| `mcp.project-workflows.v1` | 登记、创建、复制、备份恢复 | 工作流工具、授权、路径、幂等复验 |

| 管理 WIT 1.0.0 | 当前管理接口、Core 主体与扩展各自授权 | 重建 host、guest、runtime/transport 的 deps 和 Wasm 夹具 |
| State1 | 当前完整单一 DDL、事务和恢复 | 独立目录新建、重开、结构不符零写入 |
| Config1 | 当前完整 TOML；prefer_prerelease 为必填持久字段；窄更新仍允许省略 | 设置、bootstrap、独立迁移器、SDK |
| system.hello | 必填 dataSchema 和 configSchema；首发 Core 均报告 1 | 双 SDK、握手、类型和快照 |
| 仓库交换 | 缺省为原生；显式 legacy 表示 VCC TXT | 导入导出、管理界面和 RPC |
| 批量安装与升级 | 默认只给动作和包 ID；显式目标、来源及仅稳定版作为可选约束 | 候选摘要、批量请求、依赖求解、MCP Schema |
| 包来源证据 | 仓库与用户包使用显式 kind；保留两类完整证据 | RPC、领域类型、SDK、Apply |
| 内部指纹 | 使用当前计算输入，保留用途区分，将开发迭代标签归一 | 候选分页、计划过期及磁盘重开 |

权限名称沿用权限目录，现有同一业务操作按 Core 用例统一授权；保持扩展受限。不增加新权限名称。产品版本、真实 v3 迁移源、第三方格式和业务 revision 独立于合同编号。

## 原生公开方法核对索引

以生产注册和握手为最终核对来源。以下是当前 SDK 方法目录（另须对照受限管理和 MCP 专用 RPC）。

- backups：`backups.create`, `backups.restore`
- events：`events.list`, `events.subscribe`
- extensions：`extensions.applyInstall`, `extensions.applyUninstall`, `extensions.disable`, `extensions.enable`, `extensions.get`, `extensions.inspectSource`, `extensions.list`, `extensions.permissions.list`, `extensions.planInstall`, `extensions.planInstallFromSource`, `extensions.planUninstall`, `extensions.revokeGrant`, `extensions.setGrant`, `extensions.ui.close`, `extensions.ui.dispatch`, `extensions.ui.open`, `extensions.ui.refresh`
- logs：`logs.read`
- operations：`operations.cancel`, `operations.get`, `operations.list`
- packages：`packages.applyPlan`, `packages.planBulk`, `packages.planDowngrade`, `packages.planInstall`, `packages.planReinstall`, `packages.planRemove`, `packages.planResolve`, `packages.planUpgrade`, `packages.projectInventory`, `packages.queryProjectCandidates`, `packages.userPackages.enroll`, `packages.userPackages.get`, `packages.userPackages.list`, `packages.userPackages.refresh`, `packages.userPackages.remove`
- projectTemplates：`projectTemplates.edit`, `projectTemplates.export`, `projectTemplates.get`, `projectTemplates.list`, `projectTemplates.remove`, `projectTemplates.save`
- projects：`projects.applyCopy`, `projects.applyDeleteDirectory`, `projects.applyUnityMigration`, `projects.create`, `projects.get`, `projects.inspect`, `projects.list`, `projects.planCopy`, `projects.planDeleteDirectory`, `projects.planUnityMigration`, `projects.refresh`, `projects.register`, `projects.setFavorite`, `projects.unregister`
- repositories：`repositories.cancelListRegistration`, `repositories.exportListFile`, `repositories.get`, `repositories.importListFile`, `repositories.inspect`, `repositories.inspectListFile`, `repositories.list`, `repositories.packages`, `repositories.previewListExport`, `repositories.refresh`, `repositories.refreshAll`, `repositories.register`, `repositories.setCredentials`, `repositories.move`, `repositories.setDisplayName`, `repositories.unregister`
- settings：`settings.get`, `settings.update`
- state：`state.check`
- system：`system.status`
- unity：`unity.hub.open`, `unity.installations.get`, `unity.installations.list`, `unity.installations.refresh`, `unity.installations.register`, `unity.installations.remove`, `unity.launch`, `unity.launchOptions`, `unity.projectLaunchConfig.clear`, `unity.projectLaunchConfig.get`, `unity.projectLaunchConfig.set`, `unity.writerState`

## 文件清理审批批次

以下为现有路径逐项清单。首发目标路径若与旧文件同名，以最新完整定义覆盖。真实 v3 输入不在删除范围。混合规范中的有效要求原位保留。每个额外删除批次先核对所有消费者、测试和引用，再列明路径。此表不表示文件已删除。

- `crates/alcomd-testing/fixtures/m8/mcp-management-v1.provenance.json`
- `crates/alcomd-testing/fixtures/m8/mcp-management-v1.wasm`
- `specs/config/settings-v1.md`
- `specs/config/settings-v1.schema.json`
- `specs/config/settings-v10.contract-vectors.json`
- `specs/config/settings-v10.md`
- `specs/config/settings-v10.schema.json`
- `specs/config/settings-v11.contract-vectors.json`
- `specs/config/settings-v11.md`
- `specs/config/settings-v11.schema.json`
- `specs/config/settings-v12.contract-vectors.json`
- `specs/config/settings-v12.md`
- `specs/config/settings-v12.schema.json`
- `specs/config/settings-v2.md`
- `specs/config/settings-v2.proposal.md`
- `specs/config/settings-v2.proposal.schema.json`
- `specs/config/settings-v2.schema.json`
- `specs/config/settings-v3.md`
- `specs/config/settings-v3.schema.json`
- `specs/config/settings-v4.contract-vectors.json`
- `specs/config/settings-v4.md`
- `specs/config/settings-v4.schema.json`
- `specs/config/settings-v5.contract-vectors.json`
- `specs/config/settings-v5.md`
- `specs/config/settings-v5.schema.json`
- `specs/config/settings-v6.contract-vectors.json`
- `specs/config/settings-v6.md`
- `specs/config/settings-v6.schema.json`
- `specs/config/settings-v7.contract-vectors.json`
- `specs/config/settings-v7.md`
- `specs/config/settings-v7.schema.json`
- `specs/config/settings-v8.contract-vectors.json`
- `specs/config/settings-v8.md`
- `specs/config/settings-v8.schema.json`
- `specs/config/settings-v9.contract-vectors.json`
- `specs/config/settings-v9.md`
- `specs/config/settings-v9.schema.json`
- `specs/extensions/mcp-management-v1.md`
- `specs/extensions/mcp-management-v1.schema.json`
- `specs/extensions/wit/mcp-management-v1/deps/extension/guest-lifecycle.wit`
- `specs/extensions/wit/mcp-management-v1/deps/extension/guest-ui.wit`
- `specs/extensions/wit/mcp-management-v1/deps/extension/host-data.wit`
- `specs/extensions/wit/mcp-management-v1/deps/extension/host-projects.wit`
- `specs/extensions/wit/mcp-management-v1/deps/extension/types.wit`
- `specs/extensions/wit/mcp-management-v1/deps/extension/world.wit`
- `specs/extensions/wit/mcp-management-v1/management.wit`
- `specs/rpc/m8-mcp-clients-v1.md`
- `specs/rpc/m8-mcp-clients-v1.schema.json`
- `specs/rpc/m8-mcp-oauth-v1.md`
- `specs/rpc/m8-mcp-oauth-v1.schema.json`
- `specs/storage/state-v1.md`
- `specs/storage/state-v10-migration.proposal.contract.json`
- `specs/storage/state-v10.md`
- `specs/storage/state-v11-migration.proposal.contract.json`
- `specs/storage/state-v11.md`
- `specs/storage/state-v12-migration.proposal.contract.json`
- `specs/storage/state-v12.md`
- `specs/storage/state-v12.proposal.md`
- `specs/storage/state-v13-project-delete.proposal.contract.json`
- `specs/storage/state-v13.md`
- `specs/storage/state-v14-unity-project-version.proposal.contract.json`
- `specs/storage/state-v15-project-last-modified.contract.json`
- `specs/storage/state-v16-remove-project-observed.contract.json`
- `specs/storage/state-v17-empty-unity-override.contract.json`
- `specs/storage/state-v17-empty-unity-override.md`
- `specs/storage/state-v18-backup-compression.md`
- `specs/storage/state-v2.md`
- `specs/storage/state-v22.md`
- `specs/storage/state-v23.md`
- `specs/storage/state-v24.md`
- `specs/storage/state-v25.md`
- `specs/storage/state-v26.md`
- `specs/storage/state-v27.md`
- `specs/storage/state-v28.md`
- `specs/storage/state-v29.md`
- `specs/storage/state-v3.md`
- `specs/storage/state-v30.md`
- `specs/storage/state-v31.md`
- `specs/storage/state-v32.md`
- `specs/storage/state-v33.md`
- `specs/storage/state-v34.md`
- `specs/storage/state-v35.md`
- `specs/storage/state-v36.md`
- `specs/storage/state-v37.md`
- `specs/storage/state-v38.md`
- `specs/storage/state-v39.md`
- `specs/storage/state-v4.md`
- `specs/storage/state-v40.md`
- `specs/storage/state-v41.md`
- `specs/storage/state-v42.md`
- `specs/storage/state-v43.md`
- `specs/storage/state-v44.md`
- `specs/storage/state-v45.md`
- `specs/storage/state-v5.md`
- `specs/storage/state-v6.md`
- `specs/storage/state-v7.md`
- `specs/storage/state-v8-migration.contract.json`
- `specs/storage/state-v8.md`
- `specs/storage/state-v9-migration.contract.json`
- `specs/storage/state-v9.md`
