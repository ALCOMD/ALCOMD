# 发布者确认彻底删除（2026-11-16）

代码候选 `de8ead33f789c1264df574c69ecae0fb5bf155bd`，标准目录/main、真实Windows用户本地提交。所有者已批准行为变更、公共字段删除及State39两列删除清单。未推送；M8仍IN_PROGRESS，不进入M9。

## 当前行为

用户选择有效扩展包后按原安装流程安装，不再提供或要求发布者确认。GUI复选框、列表/详情/审阅中的信任状态、弃用翻译/映射、安装RPC的publisherApproval及枚举、ExtensionRecord/ExtensionPlan/SDK的trustDecision及运行时类型、确认专用错误均已删除。不是自动勾选或偷偷发送approve_for_extension；旧参数按严格解析拒绝。

保留正常安装审阅、原Plan/Apply/Operation、严格Ed25519签名、内容/来源身份校验、权限申请与实际授权的区分、默认零授权、隔离及恢复。有效签名不表示发布者可信。无法由调用者冒充受控的first_party_packaged来源；留存数据仍绑定ExtensionId和发布者指纹，另一发布者不能继承。现有extension_package_untrusted仅用于原受控来源限制，界面提示已改为“不支持此安装来源”，不会再要求勾选信任框。

State39新增0039，单事务仅删除extensions和extension_plans的trust_decision两列，并重建extension_plans_immutable，除对应字段比较外逐字保留原条件。其余值、主键/外键/索引/触发器、权限、数据、MCP接受记录及事件序列保留。原0001–0038迁移与Cargo.lock无修改，没有新依赖、表、Config、权限或身份模型。历史SQL/迁移夹具/原始审计内容保留真实旧值，不构成当前可用信任模型。

## 定向结果

证据目录 `target/m8-publisher-confirmation-20261116/`；各行对应各自运行，不相加为完整Workspace通过。

| 范围 | 实际结果 |
| --- | --- |
| 新迁移 | State39四项4/0，`state39-first.log`：两列精确删除、全旧值及对象保全、其余计划不可变约束、每处DROP后的失败回滚及真实初始化/重开 |
| Store全库 | 单轮141/0，`store-lib-first.log`；历史初始化用例只套用两列精确删除差异，历史迁移之间的原始保全/序列/回滚断言不变 |
| 扩展Store | 单轮13/0，`store-extension-runtime.log`，含必需权限拒绝、零授权启用、历史包声明拒绝、撤销/lease/隔离与保留数据 |
| 协议 | 单轮24/0，`protocol-final.log`，新安装输入不含批准字段、旧参数拒绝、输出Schema无信任字段 |
| 相关合同 | M6 5/0、M7 7/0、M7当前版本引用9/0、管理扩展2/0、Provider2/0，`contracts-final.log`；M7不重开，旧WIT和两份组件字节及provenance保持 |
| 真实Core/Host | m6_lifecycle整轮7/0及1项原有ignored，60.10秒，`native-extension-lifecycle-second.log`。有效包无需批准即可安装；重新封装有效ZIP/CRC但伪造Ed25519签名的包被Core返回extension_signature_invalid，扩展列表无副作用；原权限、撤销、恢复、崩溃隔离与管理组件用例通过 |
| 真实Provider | 具名1/0，70.69秒，`provider-final.log`：正常签名/无批准安装、原真实双客户端/STDIO/HTTP/OAuth/Plan/Apply/写响应丢失与取消恢复继续通过；未重跑另六项业务用例，不称新一轮7/0 |
| GUI | 相关Chromium单轮26/0、零重试，`browser-first.log`，含三语言签名错误/重试、没有确认框、无自动Apply、原审阅/权限/未知结果和两主题窄窗。其后只删除无当前调用者的旧信任翻译/映射；最终frontend-cleanup-check.log/frontend-build.log通过，1286条三语言消息校验通过 |
| 严格检查/构建 | 受影响八包all-targets严格Clippy、fmt、locked metadata、xtask与diff通过；Core/MCP/Host正常debug构建通过，`affected-clippy-final.log`、`fmt-final.log`、`xtask-final.log`、`production-build.log` |

真实Host模块初次编译失败：新负向测试误按旧ZIP API处理entry.name()；核对当前9.1.3返回Result后显式检查UTF-8成员名，随后完整模块7/0。初次日志`native-extension-lifecycle.log`保留，不记为测试执行通过。没有删除/放宽签名断言，也没有重跑全量Rust、Chromium、Tauri/release或三平台CI。

834份候选输入汇总SHA256 `789f14406861072b41c4f8c870e7a5a472e24ac059bf3e5e280fe7e3e20ec1d5`，逐文件见final-evidence.json；构建程序摘要另存production-artifacts.json。上述测试输入与该提交对应；GUI的最后变化仅为未使用映射/译文清理，检查/构建已补，不声称提交后重新跑了所有测试。

## 环境、远端与剩余边界

原Core/GUI及review在测试前已关闭，测试后无本轮Core/GUI/Host遗留；没有守候待恢复进程，没有部署新版原生GUI、升级原库或恢复桌面控制。原data36份、review data10份/WebView profile303份的文件集合及摘要全部不变，见final-preservation-second.log。

首次最终保全脚本在原Unity PID40632已不再存在时报告进程集合不同（final-preservation.log），不能记作原进程保持。再次核对分别记录：数据全部保持、无自有运行时遗留、原Unity已不再观测到，原因未确定。本轮没有执行Unity启动/关闭，也不自动恢复用户程序；未把该外部状态变化伪写成产品测试失败或通过。

远端仍为2a9d1233：CI43803776166三平台失败、CodeQL43803775667四语言成功。CI读状态冲突已在本地b9f19d5修复，本轮真实Host模块包含原失败用例并整轮通过；b9f19d5及本次State39均尚无对应远端验证，不用原CodeQL结果替代。未额外推送或触发workflow。

原生GUI人工查看、跨平台执行及本批远端结果仍待验证；Provider成功取消运行中任务的独立原生场景、OAuth目标客户端人工接入及其他既有M8后续继续保留。本次只完成取消发布者确认，不宣布M8完成。
