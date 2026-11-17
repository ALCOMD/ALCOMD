# 发布者确认彻底删除（2026-11-16）

代码候选 `de8ead33f789c1264df574c69ecae0fb5bf155bd`，标准目录/main、真实Windows用户本地提交。所有者已批准行为变更、公共字段删除及State39两列删除清单。随后明确批准完整四笔链并已普通快进推送至记录提交 `e99cd2c9d9254025eb8926c23ea2976f599e99fd`，同SHA自动CI43816630421终态failure（Ubuntu709/1，Windows与macOS各710/0）、CodeQL43816630482四语言success；不是全部通过。M8仍IN_PROGRESS，不进入M9。

当前远端已推进至6bbf876，最新结果见文末；下述e99cd2c与“未推送”均保留各轮历史，不代表当前远端状态。

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

此前远端2a9d1233的CI43803776166三平台失败、CodeQL43803775667四语言成功保留为历史。CI读状态冲突在b9f19d5修复，本轮真实Host模块包含原失败用例并整轮通过；该修复及State39现已按所有者四笔授权推送。本轮e99cd2c的实际终态单独列于下节，不用原CodeQL结果替代，也不重跑旧workflow。

原生GUI人工查看仍待验证；本批跨平台与远端实际结果见下节，不记整体通过；Provider成功取消运行中任务的独立原生场景、OAuth目标客户端人工接入及其他既有M8后续继续保留。本次只完成取消发布者确认，不宣布M8完成。


## e99cd2c推送与浏览器同步修复

所有者批准的四笔链为b9f19d5 → 9c6f329 → de8ead3 → e99cd2c，基线2a9d123；推送前核对main、干净工作树、精确链和远端祖先关系，真实Windows用户普通快进并用ls-remote确认`e99cd2c9d9254025eb8926c23ea2976f599e99fd`。未夹带额外提交、强推、发布、打标签或手动触发/重跑workflow；推送前xtask/diff通过，未重跑既有完整本地验证。

本轮自动CI43816630421、CodeQL43816630482均为上述SHA、attempt1。Ubuntu完整Chromium为709通过/1失败（17.6分钟），Windows为710通过/0失败（18.3分钟），macOS为710通过/0失败（23.2分钟）；各轮分别保留，不能拼成全平台通过。Windows/macOS整个CI job success，Ubuntu仅Chromium步骤失败；三平台Check、CLI自动启动、release/Tauri构建、锁文件/diff均通过，前轮真实Host生命周期的revision_conflict不再阻塞。

Ubuntu唯一失败位于`m7-project-backup-defaults.spec.ts`英文六种压缩选项用例：选level7后value仍为fast，原5秒断言超时。源码核对发现测试点击前未等待Material select完成展开/焦点初始化，点击后也未等待关闭；已按既有motion用例的公开opened/closed事件同步，不改实际点击、六种标签、值、RPC载荷和精确一次写入断言，不加重试、force或放宽超时。修复仅测试代码，提交`882eb3d99474158a7440dc8050ff3a5c8f64e276`，尚未推送。

本机未复现原远端失败：原英文用例在6倍CPU节流下单轮重复4次为4/0；不能据此证明具体远端事件时序。修复后备份默认值+motion两个模块单轮19/0、30.8秒，英文同条件4/0、46.8秒，前端check通过。CPU诊断钩子已移除，记录分别为select-before-cpu6.log、select-module-fixed.log、select-after-cpu6.log及select-frontend-check.log。修复是有依据的同步候选，远端是否收敛仍待新授权推送验证；不把本地19/0或其他平台通过用于抹掉Ubuntu709/1。

原始远端状态、各平台日志及本地定向输出保存于`target/m8-push-e99cd2c-20261116/`。未重复Rust全量、Chromium全套、release或三平台运行，原运行环境/数据与桌面控制未操作。


### 远端最终结果（同e99cd2c、attempt1）

| 检查 | 真实结果 |
| --- | --- |
| [CI43816630421](https://github.com/ALCOMD/ALCOMD/actions/runs/43816630421) / Ubuntu | failure，Chromium709/1；唯一失败见上文 |
| 同CI / Windows | success，Chromium710/0 |
| 同CI / macOS | success，Chromium710/0 |
| [CodeQL43816630482](https://github.com/ALCOMD/ALCOMD/actions/runs/43816630482) | actions、Rust、JavaScript/TypeScript、Python全部success |

运行已全部终止，ci-final-metadata.json、codeql-final-metadata.json及三平台job.log保留真实终态。监视中一次只读查询EOF后重新读取状态，未重试任何workflow。修复后的本地状态检查为locked metadata、cargo xtask check和diff通过；不重复完整测试或构建。882eb3d与本轮文档记录超出四笔推送授权，均未推送，修复远端验证仍缺失；后续需另获普通快进授权，不能宣称本轮CI通过或M8完成。


## 3752c08两笔推送与远端终态

所有者批准882eb3d、3752c08两笔后，已核对main、干净工作树、真实Windows用户cqmhv-dt\m2922、最新origin/main=e99cd2c及准确两笔链/祖先关系，普通快进推送至`3752c0870574536524d966c975e5c831a656f9ed`并用ls-remote确认。未夹带新提交，未强推、标签、发布或手动触发/重跑workflow，也未重复既有完整本地测试。

以下均为本次候选SHA、attempt1的真实终态：

| 检查 | 结果 |
| --- | --- |
| [CI43837500362](https://github.com/ALCOMD/ALCOMD/actions/runs/43837500362) / Ubuntu | success，完整Chromium710/0，12.3分钟 |
| 同CI / Windows | success，完整Chromium710/0，18.4分钟 |
| 同CI / macOS | failure，完整Chromium709/1，26.1分钟 |
| [CodeQL43837499791](https://github.com/ALCOMD/ALCOMD/actions/runs/43837499791) | actions、Rust、JavaScript/TypeScript、Python全部success |

三平台Check、CLI自动启动、release/Tauri构建、锁文件/diff均success。前轮备份压缩选择失败用例在本轮三平台均通过，但保留e99cd2c原Ubuntu709/1。macOS本轮唯一失败为`m7-package-workspace.spec.ts:301`的User Packages来源选择：期待displayText=Local avatar tools，5秒内实际仍为Multiple sources；不把本轮两个710/0与旧macOS710/0拼成三平台通过。原始终态JSON、ci-final-metadata.json、codeql-final-metadata.json和三平台job.log保存在target/m8-push-3752c08-20261116；只读查询一次临时失败后继续读取，未重试workflow。

### 来源选择的本地定向修复

测试点击来源后立即点击选项，未同步Material select的展开/焦点初始化与关闭；同模块的筛选测试已有opened/closed同步。按该模式窄修`56eed3fd9d4cdbd0077d86b3c8bde6cc170277b4`，仅给这一个失败用例增加生命周期等待和句柄清理。保留实际来源/选项点击、显示文本、原Reinstall及批量Plan审阅断言，未改生产组件、超时或重试配置，未跳过真实交互。

| 本地执行 | 真实结果 |
| --- | --- |
| 原例首个CPU诊断调用 | npm.cmd转义grep后No tests found，未执行；source-before-cpu6.log保留 |
| 修正调用后的原例CPU6倍、四次 | 4/0，33.2秒，4 workers；source-before-cpu6-execution.log，未复现远端失败 |
| 修复后完整m7-package-workspace模块 | 单轮29/0，1.5分钟，1 worker；source-module-fixed.log |
| 修复后该例CPU6倍、四次 | 4/0，1.4分钟，1 worker；source-after-cpu6.log |
| 前端check | 通过，Material/1286条三语言边界通过；source-frontend-check.log |
| locked metadata、cargo xtask check、diff | 通过；无完整Rust/Chromium/release重跑 |

本机没有捕获远端具体事件顺序，生命周期缺口修复属于待远端确认的候选，不把本地通过写成远端失败已消失。临时CPU钩子已恢复并扫描确认不存在；56eed3f及本次结果文档只本地提交，超出已执行的两笔授权，不自动推送。原Core/GUI、数据、Unity与桌面控制未操作；既有原生人工与其他M8未覆盖项不因此变成通过，M8继续IN_PROGRESS。


## e371d27两笔推送与事件证据初始化

所有者批准56eed3f、e371d27两笔后，核对真实Windows用户cqmhv-dt\m2922、干净main、最新origin/main=3752c08、准确两笔链/祖先关系，普通快进至`e371d27f961404a6606700ed87e4c01a8cfb6c6c`并通过ls-remote确认。未夹带、强推、标签/发布或手动dispatch/rerun，复用已通过本地证据，不因推送重复完整测试。

下表为本候选同SHA、attempt1的独立完整运行，不与历史局部通过拼接：

| 检查 | 终态 |
| --- | --- |
| [CI43851129715](https://github.com/ALCOMD/ALCOMD/actions/runs/43851129715) / Ubuntu | success，完整Chromium710/0，17.5分钟 |
| 同CI / macOS | success，完整Chromium710/0，26.5分钟 |
| 同CI / Windows | failure，完整Chromium709/1，19.0分钟 |
| [CodeQL43851129372](https://github.com/ALCOMD/ALCOMD/actions/runs/43851129372) | actions、Rust、JavaScript/TypeScript、Python全部success |

三平台Check、CLI自动启动、release/Tauri构建、锁文件/diff均success。前两轮压缩/来源选择的失败用例在本轮三平台均通过，旧e99cd2c及3752c08失败记录保留。Windows本轮唯一失败为m7-event-limits.spec.ts:63的5000行缓存测试：open助手在window.eventLimits未定义时读取coordinator，抛TypeError，未进入该例缓存断言。此为实际失败，不是超时取消，不记整体CI通过。完整各平台job.log、ci-final-metadata.json、codeql-final-metadata.json及逐步状态JSON保留在target/m8-push-e371d27-20261116；一次只读查询临时失败后继续读取，未重跑workflow。

### 根因与最小回归

UtilityWorkspaceEvidence.tsx先render标题，再void import("./event-limits-evidence")；标题可见不保证动态模块已赋值window.eventLimits。旧助手的expect.poll回调直接解引用，异常终止轮询。窄修`6ffde60599087e895cdaec86f5c2bfac9957e416`仅对尚未定义的全局返回未就绪，仍以原精确ready断言作为初始化门槛，随后才暂停原模拟时钟；实际coordinator错误不会被捕获或吞掉。

新增回归仅拦截测试证据JS模块，在助手实际首次读取eventLimits后才释放，真实运行原证据模块及coordinator；不修改生产代码或harness，不增加旁路。旧初始化逻辑下新例单轮1失败，复现相同coordinator未定义异常（event-startup-before.log和before.ts）；修复后整个模块单轮10/0、15.7秒、1 worker，包含9个原缓存/限流/任务/断线用例和新初始化用例（event-startup-module-fixed.log）。原5000行、8MiB、并发/查询上限及任务状态断言全部保留，原超时和零重试配置不变。

前端check（Material及1286条三语言）、locked metadata、cargo xtask check、diff通过；event-startup-frontend-check.log及local-final-metadata.json保留。未重跑完整Rust、浏览器全套或release。6ffde60与本轮文档记录仅本地、无对应远端验证，超出本次已执行两笔授权，不自动推送。原Core/GUI、数据、Unity及桌面控制未操作；其他既有M8人工未覆盖项保持，不宣布M8完成。


## dd5aa9b两笔推送与仓库刷新焦点

所有者批准6ffde60、dd5aa9b后，真实Windows用户核对干净main、最新origin/main=e371d27、精确两笔链及祖先关系，普通快进至`dd5aa9bae0a4b04ff4f3dd51b1433a240f0be7cf`并以ls-remote确认。未夹带、强推、标签/发布或手动dispatch/rerun，不因推送重复完整本地测试。

| 检查（均同dd5aa9b、attempt1） | 终态 |
| --- | --- |
| [CI43865931992](https://github.com/ALCOMD/ALCOMD/actions/runs/43865931992) / Ubuntu | success，完整Chromium711/0，14.8分钟 |
| 同CI / Windows | success，完整Chromium711/0，13.9分钟 |
| 同CI / macOS | failure，完整Chromium710/1，20.4分钟 |
| [CodeQL43865930193](https://github.com/ALCOMD/ALCOMD/actions/runs/43865930193) | actions、Rust、JavaScript/TypeScript、Python全部success |

三平台Check、CLI自动启动、release/Tauri、锁文件/diff均success，事件初始化的新回归及此前压缩/来源选择用例本轮三平台均通过。macOS唯一失败为m7-repository-list.spec.ts:220返回包列表后的Avatar tools焦点断言，实际inactive。旧运行原结果分别保留，不拼接为整体通过；完整三平台job.log、终态metadata/逐步JSON保存于target/m8-push-dd5aa9b-20261116。

### 实际产品竞态与本地修复

RepositoryRefresh完成后异步重读目录，PagedItems按新first-page引用更换PageSession；CatalogContents的局部origins/returnTo也被重建。若用户在刷新完成前已从详情返回并恢复焦点，后续会话替换会移除已聚焦按钮并丢掉恢复意图。本轮沿真实UI与现有hold-resource-refresh模拟边界控制时序，没有新增生产测试旁路。

旧生产源码+新增两例在本地单轮1失败/1通过、24.9秒，复现与远端一致的inactive；失败场景先确认返回按钮已聚焦，再释放刷新并验证旧DOM确实断开，随后断言新按钮仍应聚焦。对照场景用户把焦点移到现有Close按钮，要求刷新不抢焦点。证据为repository-focus-before.log、repository-focus-before.tsx、repository-focus-regression.spec.ts及repository-focus-before-trace.zip、repository-focus-before-test-failed-1.png、repository-focus-before-error-context.md；这是隔离headless页面，不能称真实桌面截图。

修复代码`d667a012e601b729d220b4527cdc6948f7a0e1a0`仅把焦点意图/元素引用置于既有Catalog会话外，在旧列表移除前捕获实际焦点、等待新Material更新完成后恢复，清理不存在目标；主动移到其他控件不恢复到包名。原焦点断言、分页/版本/原Plan审阅、超时与零重试保持；未更改通用分页/Material、布局、RPC、权限、State/Config、Core或依赖。前端指导只用于本次焦点管理核对，未扩展为界面重设计。

修复后整个仓库模块单轮35/0、2.8分钟、1 worker（repository-focus-module-fixed.log），包含两新增受控例及原CI失败例；前端check/build通过，372模块、1286条三语言边界通过（repository-focus-frontend-build.log）。JS index-CFilgX92.js的SHA256为90adae98a0ef0cc6e3a3c68535647d2fd18748392759ce62e276c3c9b7c75443，CSS index-8F20k4yu.css为480fdffa0cfec078febc523085c35610c913cd63195fb771c44a5339a9d4725b；输入/产物汇总与代码SHA绑定于repository-focus-inputs-and-build.json。locked metadata、cargo xtask check及diff通过，未重复完整Rust/Chromium/Tauri release；构建的既有大chunk提示保留，不放宽门禁或改依赖。

本地d667a01及结果文档超出本次已执行的两笔推送授权，尚未推送，新增代码无对应远端/原生桌面复验，不使用dd5aa9b旧结果替代。未操作原Core/GUI、数据或Unity，未恢复桌面控制；既有未覆盖项与M8 IN_PROGRESS保持，不进入M9。


## e10bce7远端收集与权限读回（2026-11-17）

所有者批准d667a01、e10bce7后，真实Windows用户核对干净main、最新origin/main=dd5aa9b、精确两笔链与祖先关系，普通快进至`e10bce7b67e77ef76e2f7db9a7dce2b41f22c4fe`并以ls-remote确认。未夹带、强推、标签/发布或手动dispatch/rerun，未因推送重复完整本地测试。

| 本轮检查（同e10bce7、attempt1） | 实际终态 |
| --- | --- |
| [CI43885504578](https://github.com/ALCOMD/ALCOMD/actions/runs/43885504578) / Ubuntu | success，完整Chromium713/0，17.3分钟 |
| 同CI / macOS | failure，完整Chromium712/1，26.5分钟 |
| 同CI / Windows | failure，完整Chromium710/3，15.3分钟 |
| [CodeQL43885504395](https://github.com/ALCOMD/ALCOMD/actions/runs/43885504395) | Python、JavaScript/TypeScript、Rust、Actions全部success |

三平台Check、CLI自动启动、release/Tauri、锁文件/diff均success，仓库焦点原例与两新增回归本轮三平台通过。Mac唯一失败为m8-management-permissions.spec.ts:35未出现预设撤销失败提示。Windows三项为该同例、m7-h4-management.spec.ts:241项目读取连续开关未撤销、m7-motion.spec.ts:46系统减少动画后quick仍false。不得用其他轮次部分通过替代本轮failure。全量job.log、最终API metadata/jobs、逐步JSON保存在target/m8-push-e10bce7-20261116。状态查询端点多次EOF后官方直接API仍能读取原run，核验同SHA/attempt后继续收集；未重启CI。

### 权限快照与原生乐观状态

受控挂起写成功后的权限读取，并观测DOM：旧ExtensionPermissions换代在effect执行前暴露旧授权可编辑窗口，新回归单例1失败（permissions-reload-before.log）。修复f1f1027819b78558d9ddc8e82863a1ccf99a8473只接受当前owner/generation的读结果作为可显示、可写快照，保持Core检查、所有分页与revision规则、未知写结果冻结和失败后恢复权威状态。

原开关用例本机CPU6倍四次4/0未复现；加入请求诊断的实施轮次2通过/2失败，失败时只有预期两次setGrant而没有revoke（permissions-request-diagnostic.log），说明最后点击未提交撤销，不能归咎于错误翻译或Core错误码丢失。Material的原生checked会先乐观变化，成功操作用例现控制已有模拟写请求及Core读回，验证读取前没有可写旧开关，再保留原checked/错误/权限/精确请求与expectedGrantRevision断言。此辅助只在浏览器模拟客户端中挂起/释放现有请求，不增加生产测试旁路。

| 本地记录 | 真实结果与范围 |
| --- | --- |
| permissions-before-cpu6.log | 原例CPU6倍四次4/0，51.9秒；不是复现或修复证明 |
| permissions-reload-before.log | 受控旧行为1失败，确认旧快照可写窗口 |
| permissions-modules-fixed.log | 初版归属修复27/1，原连续操作问题仍失败 |
| permissions-request-diagnostic.log | 2/2，请求序列缺少撤销；不记整轮通过 |
| permissions-modules-readback-fixed.log | 27/1，辅助先挂起读导致下一动作等待；测试设置问题 |
| permissions-modules-owned-read-final.log | 27/1，辅助一处set/revoke方法参数错误；测试设置问题 |
| permissions-modules-verified.log | 修正后相关三模块单轮28/0，47.7秒 |
| motion-before-cpu6.log | 原动画例CPU6倍四次4/0，1.5分钟；未复现Windows原失败 |
| motion-input-witness-module.log | 增加输入见证后的完整motion模块9/0，21.8秒 |
| candidate-targeted-regression.log | 最终四相关模块同输入单轮37/0，1.1分钟，1 worker |

### 动画仍待定位

088710612a7bb37fdaac7a759c3b626a87bdc586未修改生产动画逻辑，只验证原生媒体查询状态/原生change事件已送达，再保留原quick传播断言，并在失败时记录7个非秘密状态字段。原远端失败缺少这些输入证据，本机未复现，尚不能区分浏览器仿真投递与应用传播；这是待验证的诊断改进，**不标记原失败已修复/豁免**。未放宽现有超时、添加重试或替换真实输入，未增加依赖或改workflow。

前端check/build通过（permissions-frontend-build.log），Material/1286条三语言、372模块通过；原有大chunk提示保留。JS index-C_kTntgB.js SHA256为829aca43ad66d692fe9c0fafa6ae1003680fcf9b33170a286ce759a2414b25df；CSS index-8F20k4yu.css为480fdffa0cfec078febc523085c35610c913cd63195fb771c44a5339a9d4725b。源码/输入/产物摘要和两笔代码SHA保存于permissions-motion-inputs-and-build.json，locked metadata/xtask/diff通过。未重跑完整Rust、浏览器全套或Tauri release。

新权限修复与动画诊断及结果文档仅本地，无对应远端或原生桌面复验；远端当前仍e10bce7失败。下一批如获新推送授权，应确认权限修复并收集动画输入证据，不能先写成全绿。原Core/GUI、数据与Unity未操作，未恢复桌面控制；其余既有未覆盖项保持，M8仍IN_PROGRESS，不进入M9。


## 6bbf876推送与权限读回布局（2026-11-17）

已按所有者明确批准普通快进推送f1f1027、0887106、6bbf876，候选`6bbf876fb5b2c413eaa50fba0f7fde1dcc3258e2`。真实Windows用户、干净main、精确完整链与远端祖先关系均已核对，ls-remote确认；无夹带、强推、发布/标签或手动workflow操作。以下均同SHA、attempt1，旧轮次不拼接。

| 检查 | 本轮实际终态 |
| --- | --- |
| [CI43915767271](https://github.com/ALCOMD/ALCOMD/actions/runs/43915767271) / Windows | success；完整Chromium714/0、18.0分钟 |
| 同CI / Ubuntu | failure；完整Chromium713/1、15.6分钟 |
| 同CI / macOS | cancelled；未获取托管runner、零步骤，测试与构建未执行 |
| [CodeQL43915766843](https://github.com/ALCOMD/ALCOMD/actions/runs/43915766843) | Python、JavaScript/TypeScript、Rust、Actions全部success |

Windows和Ubuntu的Check、CLI自动启动、release/Tauri、锁文件/diff均success。macOS官方注解为托管runner多次未获取，并明确arm64容量限制；runner字段为空、steps为空，非测试失败，不能记通过。原注解/macOS job元数据、全部终态metadata/jobs及两平台完整日志保存在target/m8-push-6bbf876-20261117。查询/日志下载短暂EOF仅重试只读获取，未重跑检查。Windows动画原用例本轮通过，原未知根因仍不写成已定位；Ubuntu唯一失败为m7-h4-management.spec.ts:240等待撤销请求（permission-switch-actions.ts:8）得到0而非1。

### 连续点击与布局根因

原速本机复现6通过/2失败，请求只有setGrant没有revokeGrant；输入捕获显示第二次pointerdown时开关启用且已选中，但没有后续pointerup/change。失败trace第二次点击位置y458，首次y359.5。成功写入后的新归属读回会移除整组行并显示加载，导致弹窗收缩后再重新定位，点击序列可能落到已移动的区域。CPU6倍四次4/0未复现，不能当作修复证据。原日志、trace.zip和headless失败截图分别保存，不冒充原生桌面截图。

225726bc4ebb13156688d6b58588f19cadc29d1a保留同Core/扩展的禁用权限行，读取完成前不可写；不同身份不沿用旧内容，读取能力变化撤销旧归属。保留未知写结果冻结、失败提示、分页/revision和原精确RPC断言；成功时保持操作位置，读取失败仍明确清除不可用快照。测试辅助明确验证相同行数全部禁用，新例另验读回前后位置及下一次点击仅发出一次撤销。不关闭事件、不强制点击、不放宽超时、不自动重试、不改公共合同或共享Material。

| 本地文件 | 结果（分别记录） |
| --- | --- |
| permission-click-diagnostic.log | CPU6倍原例4/0，42.8秒，未复现 |
| permission-click-native-speed.log | 原速6通过/2失败，35.4秒，捕获无第二次change/RPC |
| permission-layout-before.log | 旧代码受控布局单例1失败，等待3个开关实际为0 |
| permission-layout-modules-fixed.log | 修复后三相关模块单轮29/0，45.5秒 |
| permission-click-fixed-native-speed.log | 原失败例原速8/0，21.8秒 |

前端check/build通过（permission-layout-frontend-build.log，372模块、1286条三语言、3.79秒），既有大chunk提示保留；xtask/diff通过，locked metadata复用未变依赖输入的有效结果。最终源文件仅在测试/构建后澄清一行注释，无执行逻辑变化。JS index-CuCXm1Lf.js SHA256为8e8752d38bdd077adc3e88ef2488dc358e2aeeb06ff5edcbf0bd32dc6f7c19f1，CSS480fdffa0cfec078febc523085c35610c913cd63195fb771c44a5339a9d4725b；完整代码SHA/输入/产物摘要见permission-layout-inputs-and-build.json。不把29/0与8/0拼为完整套件通过，未重复完整Rust/Chromium/Tauri release。

当前远端仍6bbf876，CI失败且macOS未覆盖；225726b及本地记录未推送、无对应远端/原生桌面验证，新增推送需另获明确授权。原Core/GUI、数据、Unity和桌面控制未操作；既有人工未覆盖项保留，M8仍IN_PROGRESS，不进入M9。


## 717d3e0 两笔推送终态与布局测试基准（2026-11-17）

225726b、717d3e0按明确授权由真实Windows用户普通快进推送；远端为717d3e085b6ce29649633e24c08e732c2e172855。[CI44019052791](https://github.com/ALCOMD/ALCOMD/actions/runs/44019052791)同SHA/attempt1三平台均failure，完整Chromium各714通过/1失败；三平台Check、CLI自动启动、release/Tauri、锁文件/diff均通过。[CodeQL44019052564](https://github.com/ALCOMD/ALCOMD/actions/runs/44019052564)四语言全部success。未dispatch/rerun，不拼平台或旧轮次。

三平台唯一失败均为新增m8-management-permissions布局用例初始y基准，Ubuntu12.7177px、Windows17.819px、macOS17.643px；原阈值1px保持。测量原在打开点击之前，而点击本身等待弹窗动画稳定。cd9d009只将基准移到点击完成、写入已挂起之后，未改生产代码、超时、输入或断言。用真实Element.animate的24px受控动画复现旧逻辑1失败，修复后同诊断1/0、无注入模块6/0。此为测试基准错误，不称CI服务故障；本机受控证据不冒称捕获远端帧。原日志及trace见target/m8-push-717d3e0-20261117。

cd9d009与其后获批Core观测实现均未推送，无对应远端证据。观测新权限浏览器单轮6/1的空白页Target crashed另行保留，根因未定，不替换本轮Hosted结果。M8仍进行中。


本地后续补充：Core观测96c5322的同提交组合2/2及两个旧guest资源限制失败另存，诊断单项1/0、组合4/0不冒称根因修复。浏览器空白页失败精确行号单项复验1/0（11.7秒），原模块6/1及一次正则未命中仍保留。诊断代码2c4f2a1未推送，均不能替代717的Hosted/CodeQL结果。
