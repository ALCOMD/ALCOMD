# M11 三端调用、客户端管理与扩展入口

实施基线为 `94b3413e`。Core 的业务用例以[首发合同清单](../../specs/first-release-contracts.json)为准。当前交付状态见[操作矩阵](M11-client-operation-matrix.md)。每项已有入口也要比较实际请求。静态调用位置只证明可达；只有在真实入口检查目标、字段、确认、写入次数与终态后才能标为已验证。

## 分域交付与核定

| 交付 | 差异与验收 |
|---|---|
| 1. 入口和合同核对 · [PR #11](https://github.com/ALCOMD/ALCOMD/pull/11) | 记录基线的 93 个原生操作、29 个连接服务方法和桌面动作。矩阵逐方法列出入口、缺口、例外和证据，并关联公共合同差异与删除清单。 |
| 2. 设置 → 客户端 · [PR #12](https://github.com/ALCOMD/ALCOMD/pull/12) | Core 发现已安装客户端，按程序归集活动实例。第三方 GUI 可直接连接。GUI 设置页集中管理原生连接及外部 AI 客户端的登记、授权与撤销。 |
| 3. 扩展 · [PR #14](https://github.com/ALCOMD/ALCOMD/pull/14) | 扩展可分别声明 GUI、CLI 和 MCP 入口，启停决定入口的可见性。Core 在调用时检查实际身份、业务权限和资源归属。后端可独立运行。 |
| 4. 项目、包、模板、备份 · [PR #15](https://github.com/ALCOMD/ALCOMD/pull/15) | 三端复用项目登记、创建和复制预演。软件包请求保留版本和来源的省略或显式选择，应用前可读取原计划。模板与备份操作沿用 Core 主机路径。 |
| 5. 仓库、设置、事件、Unity · [PR #16](https://github.com/ALCOMD/ALCOMD/pull/16) | 仓库按原预览登记，设置只提交改动字段，事件支持按原游标续读，Unity 管理和诊断使用具名入口。Core 为设置更新、仓库文件导出和 Hub 启动保存幂等回执。 |

各端遵守相同 Core 业务权限。客户端清单和握手中的显示信息用于识别程序；安装登记不产生授权。扩展授予的权限与调用者的身份均须真实有效。公共合同始终使用首发编号 1，直接更新生产者与消费者。正式验收要求同一候选的真实 Core/SDK/CLI/MCP 验证、格式、严格 Clippy、workspace 测试、xtask、前端检查与构建、浏览器回归和三平台 Hosted CI/CodeQL 全部通过。

## 实施状态

所有者已核定 State1 的两项原位修订。`project_unity_migration_plans.project_id` 和 `target_installation_id` 各自保存预演当时的标识，采用首发计划表的规范 UUID 检查。移除项目或 Editor 登记后，原调用者仍能读取冻结快照。Apply 会重新读取当前登记；任一记录缺失时返回对应业务错误，且不会创建任务或启动 Editor。此次变更保持 Schema 编号 1，使用旧表结构的开发数据库需要另建。验收覆盖[两种登记移除后的原计划和任务](../../crates/alcomd-testing/tests/m7_unity_migration_worker.rs)。

项目登记、创建和复制预演统一使用 `projects.register`、`projects.create` 和 `projects.planCopy`。三端向 Core 传递默认目录或绝对路径，登记重试返回第一次登记时的项目快照。此项首发合同修订会中断依赖专用 MCP 原生方法或旧目标结构的开发客户端，需更新到当前 SDK。

项目菜单提供检查、刷新及按 ID 查阅原计划。软件包工作区支持混合意图、版本范围、来源和预发布策略；CLI 和 MCP 可查询原生候选视图及本地用户包。模板与备份选择的文件仍以 Core 主机上的路径提交。三端入口及调用差异见[操作矩阵](M11-client-operation-matrix.md)，具体请求见[GUI 场景](../../apps/alcomd-gui/tests/browser/project-catalog-parity.spec.ts)和[MCP 场景](../../apps/alcomd-mcp/tests/support/project_catalog_parity.rs)。

所有者已核定[原生客户端合同差异](M11-native-client-contract-review.md)和[扩展入口合同差异及删除路径](M11-extension-entry-contract-review.md)，并核定使用已有 Core 用例补齐具名 MCP 工具。矩阵中“已提供”记录静态可达性，“已验证”须链接验收证据。每个服务操作也逐方法登记；系统文件选择、扩展可移植界面、OAuth 协议参与方等具体差异分别登记到对应行。未验收条目保留实际状态。

仓库、设置、事件和 Unity 操作采用[操作矩阵](M11-client-operation-matrix.md)中的原生方法。登记仓库须先检查来源，再确认同一调用者的预览 ID。设置更新、启动 Hub 和导出仓库清单须携幂等键。中断后 Core 根据首次受理的记录恢复结果。首发合同的这次修订要求开发客户端更新上述请求，已有开发数据库须符合包含这些任务类型的 State1 结构。[MCP 工具](../../specs/mcp/operations-v1.schema.json) · [持久化与恢复](../../specs/storage/state-v1.md)
