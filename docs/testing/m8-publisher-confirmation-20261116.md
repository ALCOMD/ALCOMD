# 发布者确认彻底删除（2026-11-16）

代码候选 `de8ead33f789c1264df574c69ecae0fb5bf155bd`，标准目录/main、真实Windows用户本地提交。所有者已批准行为变更、公共字段删除及State39两列删除清单。随后明确批准完整四笔链并已普通快进推送至记录提交 `e99cd2c9d9254025eb8926c23ea2976f599e99fd`，同SHA自动CI43816630421终态failure（Ubuntu709/1，Windows与macOS各710/0）、CodeQL43816630482四语言success；不是全部通过。M8仍IN_PROGRESS，不进入M9。

当前远端已推进至e371d27，最新结果见文末；下述e99cd2c与“未推送”均保留各轮历史，不代表当前远端状态。

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
