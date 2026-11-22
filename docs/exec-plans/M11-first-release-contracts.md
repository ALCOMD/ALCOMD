# M11 首发合同归一

所有者已逐项核定首批公共合同与 100 个文件路径，另核定 Unity Hub 能力、两份测试 guest 源码、三份旧 ADR、备份 CLI 清单、三个重复 MCP RPC 的归一和冻结计划的读取。CLI 的强类型游标参数复用工作区 serde，以 DeserializeOwned 共用解析和参数错误；逐类型重复实现为备选。沿用锁文件内现有版本和许可证。

本地核定基线 `8eff6433`，包含 `dab76fcd` 之后的十笔前置提交。[PR #10](https://github.com/ALCOMD/ALCOMD/pull/10) 以 `main` 为目标，正文单列随本次修正合入的前置提交。三平台 Hosted CI 和 CodeQL 验证同一候选提交。本计划记录当前实施差异。逐项审批时的文件清单保存在首个签名提交，删除范围以本次 PR 的实际差异为准。所有实现在主工作树进行。

分页游标沿用 HMAC-SHA256 和现有密钥、消息编码。应用层采用 [RustCrypto hmac 0.13.0](https://docs.rs/hmac/0.13.0/hmac/)（MIT 或 Apache-2.0），复用已有的 sha2 和 digest。它替代项目自行维护的 HMAC 实现，避免引入原生编译依赖。锁定版本，升级时重新验证 RFC 4231 长短密钥向量及真实游标续页。

执行规则适用于本里程碑及后续首发前的破坏性变更。

- 自有合同编号固定为 1。当前定义覆盖生产者、消费者、规范、夹具与打包产物。
- 手动创建模板时必须提供基底模板 ID 和修订，可以选择内置模板或用户模板。创建任务冻结基底内容，独立成品和导出文件只描述自己的身份、工程文件和依赖。
- 读取时按当前结构和业务规则校验。任何来源的合规数据都可直接读取。验收覆盖现行必填字段、非法值以及失败时的数据保全，只有违反当前合同的样本断言拒绝。禁止按数据年代或废弃格式另设读写路径。
- GUI、CLI 与 MCP 的同一业务操作具有相同权限，沿用同一 Core 用例、默认参数与计划。Core 检查真实身份、资源归属及撤销。扩展遵守授权和沙箱。
- 本次对开发期间的自有格式是破坏性变更。旧工作区内符合当前定义的记录可以直接使用。结构不符时报告校验错误，独立的真实 v3 导入仍使用获准的迁移入口。

## 公共合同差异

| 首发入口 | 保留的语义 | 同步与验收 |
|---|---|---|
| `packages.plan.v1` | 统一单一 Plan，精确 Unity 例外、用户包、批量和 Apply 复验 | Core、客户端、GUI、SDK、真实 Plan/Apply |
| `packages.candidates.v1` | 完整候选证据、来源、recommended、快照 | Core、GUI、SDK、候选 RPC |
| `projectTemplates.v1` | 当前单文件模板和项目创建 | 模板目录、创建、导入和导出 |
| `backups.create.v1` | 普通 ZIP，接受目的文件 | GUI/CLI/MCP/SDK、ZIP IPC |
| `backups.restore.v1` | 普通 ZIP，接受源文件与恢复目录 | GUI/CLI/MCP/SDK、恢复重开 |
| `mcp.clients.v1` | 连接认证、签发与撤销 | Core、GUI 设置、CLI、MCP、撤销测试 |
| `mcp.oauth.v1` | OAuth 连接 scope、真实 Principal | OAuth 端到端及生命周期测试 |
| `mcp.project-workflows.v1` | 登记、创建、复制、备份恢复 | 工作流工具、授权、路径、幂等复验 |
| `unity.hub.open.v1` | 以 expectedTargetToken 复验 Hub，返回 launch_requested；所有者补充核定 | Core、GUI、SDK、Schema 和握手 |
| 管理 WIT 1.0.0 | 当前管理接口、Core 主体与扩展各自授权 | 重建 host、guest、runtime/transport 的 deps 和 Wasm 夹具 |
| State1 | 当前完整单一 DDL、事务和恢复 | 独立目录新建、重开、结构不符零写入 |
| Config1 | 当前完整 TOML；prefer_prerelease 为必填持久字段；窄更新仍允许省略 | 设置、bootstrap、独立迁移器、SDK |
| system.hello | 必填 dataSchema、configSchema 及 extensionApi；Core 报告首发存储、配置、ABI 主版本和基础 WIT world | 双 SDK、握手、类型和快照 |
| 仓库交换 | 缺省为原生；显式 legacy 表示 VCC TXT | 导入导出、管理界面和 RPC |
| 批量安装与升级 | 默认只给动作和包 ID；显式目标、来源及仅稳定版作为可选约束 | 候选摘要、批量请求、依赖求解、MCP Schema |
| 包来源证据 | 仓库与用户包使用显式 kind；保留两类完整证据 | RPC、领域类型、SDK、Apply |
| 内部指纹 | 使用当前计算输入，保留用途区分，将开发迭代标签归一 | 候选分页、计划过期及磁盘重开 |

权限名称沿用权限目录，现有同一业务操作按 Core 用例统一授权；保持扩展受限。不增加新权限名称。产品版本、真实 v3 迁移源、第三方格式和业务 revision 独立于合同编号。

## 原生公开方法核对索引

以生产注册和握手为最终核对来源。以下是当前 SDK 方法目录（连接服务与 MCP 适配方法见操作矩阵）。

- backups：`backups.create`, `backups.restore`
- events：`events.list`, `events.subscribe`
- extensions：`extensions.applyInstall`, `extensions.applyUninstall`, `extensions.disable`, `extensions.enable`, `extensions.get`, `extensions.inspectSource`, `extensions.list`, `extensions.permissions.list`, `extensions.planInstall`, `extensions.planInstallFromSource`, `extensions.planUninstall`, `extensions.revokeGrant`, `extensions.setGrant`, `extensions.ui.close`, `extensions.ui.dispatch`, `extensions.ui.open`, `extensions.ui.refresh`
- logs：`logs.read`
- operations：`operations.cancel`, `operations.get`, `operations.list`
- packages：`packages.applyPlan`, `packages.planBulk`, `packages.planDowngrade`, `packages.planInstall`, `packages.planReinstall`, `packages.planRemove`, `packages.planResolve`, `packages.planUpgrade`, `packages.projectInventory`, `packages.queryProjectCandidates`, `packages.userPackages.enroll`, `packages.userPackages.get`, `packages.userPackages.list`, `packages.userPackages.refresh`, `packages.userPackages.remove`
- plans：`plans.get`
- projectTemplates：`projectTemplates.edit`, `projectTemplates.export`, `projectTemplates.get`, `projectTemplates.list`, `projectTemplates.remove`, `projectTemplates.save`
- projects：`projects.applyCopy`, `projects.applyDeleteDirectory`, `projects.applyUnityMigration`, `projects.create`, `projects.get`, `projects.inspect`, `projects.list`, `projects.planCopy`, `projects.planDeleteDirectory`, `projects.planUnityMigration`, `projects.refresh`, `projects.register`, `projects.setFavorite`, `projects.unregister`
- repositories：`repositories.cancelListRegistration`, `repositories.exportListFile`, `repositories.get`, `repositories.importListFile`, `repositories.inspect`, `repositories.inspectListFile`, `repositories.list`, `repositories.packages`, `repositories.previewListExport`, `repositories.refresh`, `repositories.refreshAll`, `repositories.register`, `repositories.setCredentials`, `repositories.move`, `repositories.setDisplayName`, `repositories.unregister`
- settings：`settings.get`, `settings.update`
- state：`state.check`
- system：`system.status`
- unity：`unity.hub.open`, `unity.installations.get`, `unity.installations.list`, `unity.installations.refresh`, `unity.installations.register`, `unity.installations.remove`, `unity.launch`, `unity.launchOptions`, `unity.projectLaunchConfig.clear`, `unity.projectLaunchConfig.get`, `unity.projectLaunchConfig.set`, `unity.writerState`

## 补充核定

MCP 的项目复制 Apply、备份恢复和仓库刷新使用同一原生业务入口。确认、连接认证、目录身份及幂等回执继续在相应边界核验。批准的逐项删除清单随首个签名提交保存，工作树差异展示实际删除范围。

已核定删除仅描述替换前格式的三份 ADR。单文件模板、普通 ZIP 及恢复目录的有效定义以当前规范为准。

备份 CLI 清单与 ZIP 参数使用首发编号 1，所有者已补充核定此文件更名。

各端的业务操作与待补入口见[三端操作矩阵](M11-client-operation-matrix.md)。

所有者核定只读 `plans.get`。它按种类读取当前调用者已冻结的包、复制、目录删除或 Unity 迁移计划；交互式 CLI 展示原计划以后才发送 Apply。

仓库刷新沿用统一的仓库快照返回值。其包清单另行分页，幂等回执只保存可还原该次公开快照的元数据。仓库来源 URL 属于可展示信息。凭据与下载认证资料遵守各自的保管边界。

所有者核定 `system.hello` 固定报告 `extensionApi`，供外部 SDK 查询当前 Core 的 ABI 主版本与基础 WIT world。两者与公开 ABI 规范一致，宿主另行校验实际加载的组件接口。

v3 派生模板按 `2022.3.22f1` 转换。迁移器检查基底链的版本约束，选取该版本对应的实际工程内容，生成独立模板。合法但只支持其他版本的派生模板留在源目录，盘点和导出报告跳过数量。v3 直接归档的工程保留其实际版本。v4 的内置目录仍提供 Blank 及 Avatars、Worlds 各自的 `2019.4.31f1` 和 `2022.3.22f1` 模板。

迁移器和 VPM 共用域层的版本范围匹配，直接依赖沿用工作区已锁定的 `semver`。这样独立迁移器无需链接仓库网络访问和包解析服务。共享语义的维护边界由现有 VPM 范围向量以及迁移范围交集测试覆盖。

共享项目、Editor、仓库和用户包使用本机资源所有者。业务事件发给相应资源的订阅者，操作审计和幂等回执记录实际调用者。MCP 身份在读写事务内复验；设置写入和 Hub 启动在取得设置锁并校验目标之后、执行操作之前复验原凭据及所需权限。

v3 全量导入分别保存项目的登记时间与最后活动时间，并在重开时对账。选择性资源导入通过真实 Core 核对已有用户包的来源身份，匹配时保留原登记并继续。迁移器的测试依赖复用工作区 Core，以隔离数据库覆盖重复来源、来源变更和撤销。
