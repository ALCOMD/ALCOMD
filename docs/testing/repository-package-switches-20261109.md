# 存储库内软件包开关验证

日期：2026-11-09。基于 main `1369a950b525985c5fb85b92cd775fc35a97d2f2` 的未提交工作树。所有者明确批准 [M7 方案](../exec-plans/M7-repository-package-switches.md)及 Config13 合同后实施；所有者随后授权本项本地提交；未推送或替换运行程序，没有更改真实项目和现有运行数据。

## 实际结果

- 仓库浏览列表与包详情提供同一设置开关，使用包名称。关闭以仓库＋包ID为范围，覆盖全部版本，包括以后出现的版本；同仓库其他包、其他仓库同名包和本地用户包独立。
- Core 的 Store 查询在返回普通版本数据和应用记录上限前过滤，使用不相关的复合键集合查询，避免逐版本重复展开完整关闭清单。VPM 候选与 Application 冻结来源复验补充防御。版本、提供方、依赖求解、批量、显式来源、旧 Plan 和 Apply 使用同一设置权威。
- 已安装包及版本继续来自项目；关闭其唯一来源后提供方为空，沿用未知来源展示，本地卸载仍可执行。只有明确的仓库管理查询保留关闭项，便于重新开启。
- Config13 默认空清单，Config1–12 原设置与 revision 保留。新字段在旧格式中拒绝，Config13 缺字段也拒绝；持久输入须规范化。请求可无序，Core 排序；空组、重复、非法身份、超过256组或4096个包ID拒绝。
- 完整 Config13 文件上限1MiB；排除新字段的原设置保持16KiB基线预算，旧格式完整文件仍限16KiB。文件读取超限保留 Corrupt 错误，显式超限保存拒绝且不改已接受文件。没有新增 SQL Schema、RPC 方法、权限或生产依赖。
- GUI 使用既有设置 CAS，失败保留已确认状态并可重读；整库关闭时仍可调整包偏好。列表/详情/分页/返回/跨资源导航状态一致，加载表头与实际列共用定义。100ms、区域加载、后台原子替换和意图预取规则保留。

## 通过的验证

日志及截图在仓库 `target/` 下，属于可丢弃证据，不提交构建产物。

| 验证 | 结果与证据 |
|---|---|
| Application、Protocol、Store、VPM 的 lib/tests | 520项通过，3项既有忽略；`config13-rust-final.log`。包括 Schema/Config迁移、大小预算、持久化、请求/文件规范化、原子保存、配置权限/CAS和超过100001条关闭版本在记录上限前被过滤。 |
| 合成项目与原生 RPC 集成 | 27项通过；`config13-integrations.log`，四个测试目标为 `m7_candidate_snapshot_race`、`m7_official_rpc`、`m7_repository_auto_refresh`、`m7_vpm_semantics`。覆盖跨来源隔离、全版本关闭、未知来源安装事实、卸载、依赖/批量/重装拒绝、显式来源/旧计划/Apply拒绝与重开、raw管理保留、配置往返及旧配置重启迁移。 |
| 最后文件读取错误分类调整 | 38项 Config 回归通过；`config13-store-final.log`。包含旧格式超16KiB和新格式超1MiB读取均 Corrupt，原文件保持；与前述520项重叠，不重复计数。 |
| 严格 Clippy | Application/Protocol/Store/VPM/Testing/daemon 的 all-targets 通过，`config13-clippy2.log`；最后 Store 调整的 all-targets 再验通过，`config13-store-clippy.log`。 |
| 前端检查与构建 | 全工作区 `npm run check` 和 `npm run build` 通过，`config13-frontend-final.log`、`config13-build-final.log`。1200条消息、Material控件边界和35份图标边界通过；保留既有大于500kB的构建提示。 |
| 浏览器回归 | 92项不同用例最终通过，覆盖 `m7-repository-list`、`m7-package-workspace`、`m7-utility-pages`、`m7-navigation-loading`；`config13-browser-complete.log` 中91项通过，剩余截图用例改为整页截图后在 `config13-visual.log` 单项通过。 |
| 外观 | 已查看 `config13-visual/m7-repository-list-package-097e8-ation-and-resource-sections-chromium/package-switch-list.png` 和 `package-switch-detail.png`：浅色常规窗口，关闭态列表和详情勾选、包名称、刷新与固定操作区正常。 |

最终 fmt、xtask 与差异检查结果在 `config13-final-gates.log`；最终文件摘要在 `config13-inputs.json`。

## 失败轮次与范围

- 首轮编译/前端检查发现新增必填字段遗漏的既有测试初始化器，以及活动 Config12 合同断言；已同步。历史 Config1–12 的 JSON Schema 和原始输入保持，不以新格式冒充旧格式。旧字段16KiB限制、未知字段、错误与权限断言保留。
- 浏览器首轮3项失败来自新测试使用了错误的导航角色与 `mode` 参数（夹具使用 `state`）；纠正后仓库套件30项通过。扩展轮次1项既有断言仍预期单列表头，已增加新启用列断言。
- 加入外观取证后，原交互已通过的用例在 Material Dialog 元素截图处超时，trace停在截图 API；没有减少交互断言或放宽超时。改用 `page.screenshot` 后完整用例通过，两张截图已实际查看。
- fmt 首次最终检查与最后读取错误分类修改重叠，报告未格式化的新断言；随后格式化并重新执行最终检查。

不是 H7/整个M7最终签收，没有运行整个工作区测试、三平台 Hosted CI/CodeQL或替换原生GUI/Core。原生RPC只使用自身临时数据，在未发现用户 Core 进程时运行；不把合成测试当作真实项目修改验收。运行版本更新时 Core 与 GUI 需配套；旧 Core 不支持 Config13，回退须使用升级前配置备份。
