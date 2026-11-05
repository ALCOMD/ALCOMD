# M7：官方 GUI 与 Portable Extension UI

状态：**M7 IN_PROGRESS / H7 MANUAL_PENDING / H6 STAGE_ACCEPTED_WITH_FOLLOWUPS / H5 STAGE_ACCEPTED_WITH_FOLLOWUPS / H4 STAGE_ACCEPTED_WITH_FOLLOWUPS / H3 STAGE_ACCEPTED_WITH_FOLLOWUPS / H2-B STAGE_ACCEPTED_WITH_FOLLOWUPS / H2-A ACCEPTED / Visual Gate 2 PASS**。唯一工作目录为标准 `ALCOMD`，
活动分支为 `main`；当前阶段与验证结果统一见 `docs/status.md`。已通过的 M6、Portable UI B0-D、
Project/Package P0-P8 和 exact Unity 技术检查点保持其原始 SHA/证据，不替代本轮 GUI 人工验收。

## 实现依据与跨模块整改计划（2026-11-05，已批准实施）

所有者要求先继续检查其他模块，使用官方证据与vrc-get辅助判断，完整梳理后提出统一计划。
所有者随后明确批准完整实施本计划；下列公开合同修订和生产依赖边属于本次授权。已按下方实际候选完成本轮实现和定向验证，不代表M7签收。
问题/来源/隔离复现和未覆盖范围统一见
[跨模块依据复核](../testing/m7-implementation-basis-audit-20261105.md)，不另立阶段入口。

### 范围、判断规则与成本

- 仍只收束M7现有产品链路，保留H2–H6签收、已有修改、数据、profile和验收材料；不进入M8–M12，不推送。
- 外部规范先确定字段所属格式及版本，再确定业务上下文。分别处理UPM项目清单、VPM项目清单、
  包清单、仓库索引；最低Unity兼容范围、项目实际Editor版本和启动exact匹配也分开。
- vrc-get冻结`14d73d018bdc90c5b005064eb10f8e9714fa8409`仅作可复查行为/输入对照；
  不复制、包装、Fork或改写其源码，不继承其内部架构或将全部差异判为ALCOMD错误。
- 官方文档、真实样本、参考实现和ALCOMD策略冲突时先记录差异；不靠修改测试预期或新特例消除冲突。
  已批准的SHA-256要求、权限/owner隔离、Plan/Apply、无覆盖恢复和业务扫描保持。
- 优先内部修正，预期不新增数据库结构、权限名、通用框架或整套前端状态模型。
  本次已批准：普通范围与项目直接依赖的上下文语义修正；创建请求必需unityVersion；store复用锁定toml 1.1.4；非Windows有界-version探测；补齐unityRelease。

### 批次0：校正依据与验收输入

覆盖A01–A10。建立轻量“格式/字段或动作→版本化来源→上下文→样本→预期→实现→测试”映射，
原位修订错误的current Config引用、能力声明、fixture形状和依赖评估结论。
测试先覆盖官方真实输入和相邻模块消费结果，不先做全量构建。

范围夹具明确区分普通VersionRange与项目直接依赖。保留反例：同一个`1.2.3`在不同上下文可能不同；
不能将上一轮普通版本库对照直接推广为所有入口都exact。每个差分来源固定版本/提交，记录实际比较范围。

### 批次1：清单读取与写出完整性

覆盖A01–A04，是后续模板/迁移验证的基础。

1. 修正legacy对象与数组的区别，保留路径/GUID含义。正常读取、用户包与已有迁移配置使用一致格式。
   能读取不等于批准删除；需要清理但当前不支持的场景应提供准确的unsupported原因，不新增盲删路径。
2. 修正整仓resolver解析失败的反馈。可以保留批准的完整性门槛，但必须说明失败原因/适用范围，
   不再把合法字段误报损坏或无问题地返回全部不可用；若采用逐条解析，仍不得冒称catalog完整。
3. 补齐批准范围需要的合法partial/caret/tilde/comparator组合，明确非法与暂不支持输入。
   普通包依赖/锁依赖采用普通范围；项目直接依赖采用具名上下文；GUI不自行求解。
4. 写出保留未修改条目的依赖与必要元数据，改变条目的锁定依赖来自冻结的权威求解结果。
   同步Plan字节/hash、Apply materialization、模板staging及已有迁移写出，避免Plan与Apply格式不同。

验收：固定官方索引的合法版本不再因字段形状被拒绝；legacy受限原因准确；跨上下文范围向量正确；
安装/升级/移除/resolve/重装的单一Plan/Apply链路及重放/恢复保持，删除B不丢未修改A的锁信息。
当前已签发计划的hash不能静默重签；必要时按既有stale机制拒绝，不修改历史任务。

### 批次2：Editor身份与创建后可启动

覆盖A05、A06、A09。

- 为macOS/Linux建立真实Editor元数据样本，不再把包管理器catalog顶层version当作Editor版本。
  先核实各平台bundle/executable信息或隔离版本查询的可行性、身份绑定、超时和失败行为；
  不凭路径名猜版本，不执行真实项目。vrc-get的`-version`路径仅为辅助候选，不直接搬用。
- 内置模板的主次兼容版本线不再冒充项目实际版本。创建时明确选择完整版本，
  固定最终ProjectVersion内容与摘要，发布后核对同一观察版本；是否安装由后续启动选项独立报告。
  现有创建请求增加必需unityVersion完整版本字段，CLI增加必需--unity-version；允许未安装版本，指定版本不冒称安装存在；
  不硬编码一个补丁号修复所有模板，也不放宽全部exact启动校验。自定义模板的真实版本也须一致处理。
- `unityRelease`先统一适用范围及版本排序依据，再同时影响Core计划与候选证据；
  本轮必须补齐unityRelease，不再以原major/minor子集宣称完整兼容。

验收：空白/Avatar/World模板的创建结果均能进入现有启动流程；自定义模板、未安装/多Editor歧义、
Editor身份变化、旧Plan、版本格式边界都有明确结果。平台验证使用隔离数据和项目；
真实Unity进程操作或额外环境切换只在已授权的具体范围内安排，不借检查任务启动Unity。

### 批次3：配置语法与持久化

覆盖A07、A08、A10，可与不依赖它的格式修正分别形成候选。

- 首选现有workspace TOML库的有界解析再执行closed业务字段校验；评估新增store生产依赖边、
  锁文件和维护成本，使用已获批的store→toml生产依赖边。保留16KiB、UTF-8、重复/未知字段、枚举、revision/CAS和Config1–10迁移约束。
  另核定标准TOML数值与现有revision边界，不能因换parser悄悄缩小公开合同。
- 同义注释/引号/转义与业务非法内容分开测试；序列化结果同时通过独立标准解析与自身读取。
  不删除原设置或损坏证据来“恢复正常”。
- 将配置文件同步、rename、目录同步和备份清理明确为可恢复步骤，复用现有平台原语。
  对平台不支持的情况明确保证边界，不把错误吞成已落盘。分别验证各步骤失败及恢复窗口。

验收：三种已复现合法配置变体与baseline值相等，非法业务值仍拒绝；旧配置revision/值保持；
发布后未知结果仍按既有重读核对处理。进程kill测试与真实掉电证据分别记录，不互相代替。

### 批次4：跨模块候选回归与交付

- 日常只运行改动模块的定向检查；稳定候选集中执行适用完整Rust门禁、前端check/build与相关浏览器回归。
  一次聚合检查已覆盖的内容不在每个提交后重复跑。
- 同一候选补齐创建→登记→启动选项、包操作→锁信息→模板/备份/迁移读取、设置保存→重启读取等边界用例。
  继续覆盖通知稳态零周期业务读取、ACK/重连、旧响应/再次失效、任务全活跃内容/历史分页、终态副作用去重。
- RPC/SDK/原生桥接/浏览器harness同步验证；自有协议不冒充JSON-RPC2.0，WIT exact绑定不被改造成ambient WASI。
  对本轮未发现错误的模块只做受影响回归，不重写它们来“统一实现”。
- 按目的分批本地提交（实施时沿用授权），每批只报告实际变化、实际验证和待查看项；
  不推送，不自动部署/重启应用，不把旧CI/定向结果当新候选完整通过。
- 交付绑定实际候选与工作区边界，列出已消除/仍受限/未实证项及来源，H7/M7最终确认仍由所有者完成。

### 本次执行定稿

创建Plan沿现有数据库要求保持version=1，以私有creationBasisVersion=2标识本轮完整版本冻结证据；无此标记的未接受计划stale，已接受历史计划按原冻结证据恢复，不新增数据库结构。


- 本批保持Config10、不增加SQL迁移（起始State18；并行日志修改另行记录，不由本计划授权）。store新增既有workspace依赖边toml 1.1.4与内部alcomd-platform；前者替换手写语法解析，后者复用目录同步。无新外部解析库；替代方案手写语法无法覆盖标准TOML，重复平台同步实现会扩大维护成本。toml跟随现有锁与升级检查，平台不支持及真实掉电保证单独列证据。
- 必需创建输入unityVersion贯通RPC/CLI/SDK/GUI；TemplateRecord增加可选unity主次版本线以筛选已安装候选，创建Plan公开所选unityVersion及ProjectVersion摘要。新未接受Plan必须有证据，已接受历史Plan保留原冻结形状及恢复路径。这是未发布RPCmajor1的有意破坏性输入变更，不保留省略unityVersion的旧创建入口。
- PackageChangeSet formatVersion=3标识本轮范围与完整写出依据；1/2未接受计划返回plan_stale，不重签。已接受1/2仅按原冻结摘要恢复旧输出（包括历史直接依赖字符串/对象的准确字节），不借此给新请求提供旧格式兼容入口。既有JSON列/权限/数据库结构不变。
- legacy路径/GUID来自固定来源清单：仓库manifest按既有fingerprint校验后保存为有界、内容寻址的Core缓存证据，用户包沿原owned manifest。归档身份正确不等于index遗留信息相同；不能用缺失的归档字段推断index没有清理义务。Apply只读已冻结证据，缺失或不可排除义务时明确失败，不临时网络请求或静默当作无义务。

| 格式/字段 | 来源与上下文 | 实施规则 |
| --- | --- | --- |
| UPM项目dependencies | Unity 2022.3项目清单 | 字符串；不套VPM版本对象 |
| VPM项目dependencies | VRChat官方模板、固定vrc-get项目DependencyRange辅助 | version对象；完整裸版本在此上下文按minimum |
| VPM locked.dependencies / 包vpmDependencies / 普通调用range | VRChat包规范引用的固定SemanticVersioning.NET；vrc-get普通VersionRange辅助 | 普通完整裸版本exact，partial/caret/tilde/comparator/hyphen独立向量 |
| legacyFolders / legacyFiles / legacyPackages | VRChat包规范与固定官方索引 | 前两项路径→GUID对象，后一项数组；元数据不授予删除资格 |
| unity / unityRelease | Unity 2022.3包清单与assembly version expressions | 同一数值最低版本判断；缺unity不施加门槛，无法解释为unknown |
| Editor观察 / 创建ProjectVersion | Unity 2022.3 -version；ALCOMD显式完整版本策略 | Windows PE，其余显式有界探测；创建选择不冒充安装或启动验证 |


- 创建请求不接受省略unityVersion的旧公开形状；Core校验完整版本与模板兼容性，将最终ProjectVersion内容/摘要固定进Plan。GUI版本选择器包含Core预设、兼容的已安装版本和“自定义”，仅选自定义时显示完整版本输入框；仅一个兼容安装时预填，多项不擅选，无安装默认自定义。手动输入匹配已安装版本也不切换选择模式；项目页和模板页共用同一组件，字段顺序为模板→Unity版本→项目名称→保存位置。

- Windows保留PE读取；macOS/Linux仅在登记/重新扫描时执行-version，独立空工作目录、直接argv，并发2、总超时10秒、stdout/stderr各64KiB；执行前后核对身份，超时只终止并回收自己子进程。普通列表不探测，失败保留已有记录。
- ALCOMD原生Template Bundle v1的unity主次对按同一版本线校验（ALCOMD策略，不冒称VRChat外部模板格式的最低版本规则）；patch由所选完整版本与包最低要求决定，不擅自支持未来主次版本。
- unityRelease进入resolver/candidate共享最低版本判断；缺unity不施加门槛，无法可靠解释则unknown，Plan不据此宣称兼容，启动exact规则保持。数值渠道支持a/b/f/p，experimental及区域/自定义后缀保持unknown；依据[Unity 2022.3版本表达式](https://docs.unity3d.com/2022.3/Documentation/Manual/ScriptCompilationAssemblyDefinitionFiles.html#unity-version-numbers)。
- TOML依赖沿用workspace当前锁定1.1.4，不增加第二个parser；替代手写语法减少维护分歧，仍执行各历史版本closed校验、16KiB限制及u64 revision/CAS。维护成本为既有库升级与新增store依赖边，替代方案继续手写已因可复现语法缺口拒绝。
- 本轮按批次本地提交；不推送、不部署或重启用户环境，不操作真实项目和Unity。旧Plan不重签，未接受旧语义计划走stale，已接受任务恢复证据保持。

### 指定创建版本补齐（2026-11-06，所有者已批准）

v3功能审计确认：内置Avatar/World模板的三项版本与安装登记无关，并按选择使用对应模板内容。v4沿用独立编写的模板脚手架，按所选版本冻结ProjectVersion和SDK依赖，不复制旧版源码或模板字节。

`TemplateRecord.unityVersionPresets`是Core派生的必需只读完整版本列表，最多64项，空数组表示无预设。VRChat内置Avatar/World依次提供2022.3.22f1、2022.3.6f1、2019.4.31f1，其他模板只提供自身声明主次线内的预设。GUI与兼容的安装版本去重合并，保留自定义及原默认选择逻辑；预设不表示已安装Editor。RPC major 1响应扩展，不加缺字段回退，不改变创建请求、权限、State表、Plan恢复或依赖。

2019.4.31f1只作为经Core识别的内置Avatar/World额外版本；导入/派生bundle不继承。内置SDK Avatars/Worlds及传递依赖Base限定不高于3.4.2，仍经现有包兼容求解并冻结确切来源/哈希，缺少兼容SDK明确返回template_dependency_unsatisfied。[VRChat官方当前版本](https://creators.vrchat.com/sdk/upgrade/current-unity-version/)为2022.3.22f1，[官方入门文档](https://creators.vrchat.com/getting-started/)将2019.4.31f1用于SDK3.4.2及以前；已核对[官方包索引](https://vrchat.github.io/packages/index.json)仍有三项SDK3.4.2及SHA-256。2022.3继续依据包元数据及Unity最低版本求解，启动仍按真实安装和完整版本核对。

### 本轮实现与实际验证（A01–A10）

A01–A10已按批准范围实现并分目的本地提交。最终代码候选为`3a3b22e771e2419c1d7c018b77931251bd770085`；包/兼容核心为`e9cb24a`，完整创建版本为`17f0fea`，配置为`b89dc89`及测试桩收束`9691f74`，Editor探测为`073d87b`，范围为`2029aba`。`8a84371`补齐备份VPM对象夹具；`74ed2ff`纠正旧事件测试预期，`3a3b22e`补齐对话框内显式重试。无新SQL迁移、配置字段、权限或外部版本库。

并行日志/活动重建工作树保留且未混入本批；最终验证使用标准目录`target`内由提交生成的源码归档，不是额外Git worktree，不在快照修改代码。所有输入逐Git对象核验（包括既有PowerShell CRLF属性），SDK/UI业务包解析限定到快照自身。Rust、Schema、依赖及SDK输入与已验证`8a84371c891bc94527eed2157db01e8318e4e90e`完全相同，后续仅两个GUI文件改变，因此复用其适用Rust证据；最终GUI重新check/build与浏览器。输入证明及原始输出位于`target/m7-basis-implementation-20261105/`，关键映射为`final-candidate-input.json`和`final-reference-scope.json`，不以共享工作树的旧计数替代纯候选结果。

| 问题 | 已实现和对应证据 | 保留限制 |
| --- | --- | --- |
| A01–A03 清单、范围与来源 | 区分UPM/VPM；legacy路径→GUID对象与packages数组；逐版本保留合法元数据和问题反馈；普通范围与直接依赖独立。纯候选读取固定官方索引303/303 resolver-ready、303展示行、0问题；固定SemanticVersioning.NET对照4180输入及canonical回读均0差异，最终parser字节一致。 | catalog完整性门槛保持。读取legacy不授予删除资格；实际或无法排除的清理义务继续明确unsupported。 |
| A04 锁及冻结输出 | 未改条目保留依赖和元数据，改变条目来自冻结边；删除直接依赖保留必要传递依赖，重装不生成自依赖。真实包链5/0包含Plan/Apply/hash/重放、index权威、路径/GUID、Plan后新增目标及同Plan选中legacyPackages的拒绝。 | format3使旧未接受计划stale；已接受1/2保持原冻结摘要恢复，不能重签或删除证据。本轮未扩展遗留删除。 |
| A05 Editor探测 | Windows PE保持；macOS/Linux显式登记/扫描才执行受限-version，列表不执行。Windows平台Unity/Hub24/0，Core输出/身份4/0；Linux/macOS平台all-targets编译和Linux严格Clippy通过。 | 未执行macOS/Linux真实Editor安装探测，编译不替代实证；Windows测试仅用自有假进程。 |
| A06 完整创建版本 | RPC/CLI/SDK/GUI/桥接必需unityVersion；可选择安装或填写未安装版本。模板版本线、完整ProjectVersion字节/hash冻结，发布后观察核对。模板集成4/0，包含创建→发布→登记→启动选项、版本门槛、旧计划；私有Windows真实帧1/0及CLI5/0通过。 | 创建不冒充Editor已安装或已启动；启动保持exact。私有帧用生产dispatcher和独立Store，不等于完整daemon认证/生产端点验收。 |
| A07–A08 配置与恢复 | toml 1.1.4类型化解析、各Config closed校验、16KiB、u64 revision/CAS、Config1–10迁移保持；配置单元33/0。文件/rename/目录同步和恢复覆盖，终止测试1/0覆盖9个自有写进程边界。 | 进程终止不是物理掉电；真实断电、非Windows文件系统持久性未实证。不重置或删除原数据。 |
| A09 unityRelease | Store到候选/求解共享数值最低版本判断；覆盖相等、低于、渠道、数值revision、缺unity与非法release的一致结果。VPM纯候选146/0。 | experimental、区域/自定义后缀保持unknown，不放宽启动exact。 |
| A10 文档与能力 | 原位修正历史Config当前链接，撤回普通范围bare-minimum错误依据，区分官方格式和ALCOMD模板版本线、摘要/权限策略。 | 历史记录保留其真实版本及适用范围，不作为本轮通过。 |

集中验证：workspace格式检查、all-targets严格Clippy和`xtask check`通过。纯候选App72/0、Domain6/0、Store56/0、VPM146/0、协议单元16/0、Schema43/0、协议默认值2/0；七个集成目标为m4_contract5/0、m5_contract7/0、m5_template4/0、m7_legacy_packages5/0、backup/restore合同各4/0、迁移worker7/0。配置终止特性单独验证，不称作掉电证据。

最终前端`npm run check`、`npm run build`通过，保留现有Vite chunk大小警告；七个相关浏览器模块一次完整61/0。覆盖完整版本输入/未安装/多Editor、模板分页与草稿、终态回调、ACK/断线/重同步、旧响应/删除后迟到、进度合并界限和健康稳态无周期读取。纯候选此前56/5→60/1的失败原始证据保留：旧预期错误要求自动重试、禁止终态失效读取或已接受未知响应任务的全局发现；修订为显式重试/单次失效/不重复Apply。剩余实际缺陷是对话框内没有可操作的重试按钮，已复用共享事件入口修正，并保持对话框打开验证，未靠恢复轮询或删除断言消除失败。备份旧字符串夹具和误含并行活动测试桩也已在独立提交中纠正。

未执行完整`cargo test --workspace`、固定Windows用户端点IPC、整套浏览器、三平台CI/CodeQL或原生GUI人工验收。用户Core168708持续占用该端点，本轮不停止/替换它，不为门禁绕过授权；窄私有管道IPC不冒称这些路径已通过。包/扩展等所有历史端到端流程、并行State19与旧profile结合后的恢复、macOS/Linux真实Editor及物理掉电仍未覆盖。已有H2–H6签收保留，M7 IN_PROGRESS / H7 MANUAL_PENDING。原数据/profile/截图/材料、Core和Unity保持，未部署、重启或推送。

### 统一完整门禁、真实IPC与环境更新（2026-11-05）

按所有者“分次提交”归档为`4d5e7f4`（缓存栈）、`4251875`（菜单关闭）、`7856ace`（共享日志底座）、`b9af3abe7b036cc9066ea9a157abc304e4aec7eb`（Core/SDK/GUI配套集成）。实际代码提交697项输入与下述验证源逐项一致，仅有Git换行规范化及3份既有回车字节差异；未改变语义或重复测试/构建。提交前后持续出现的项目创建界面修改保留在CoreActions.tsx、ProjectCreationDialogs.tsx、locales/forms.ts、use-create-unity-version.tsx及m7-project-create-restore.spec.ts；重叠文件仅暂存已验证内容，未覆盖工作树，不把这些后续改动计入本候选。截图和临时浏览器输出不入库；本地归档与M7/H7人工验收分开。

所有者追加批准完成完整 Rust/真实 IPC 后构建并更新 Core/GUI，允许短暂停启，沿用原数据和 profile，不动 Unity/真实项目。经明确授权与“日志调整”会话协调，由本会话独占固定 Windows 端点及构建/停启；没有另建 worktree、改写历史或推送。

- **候选与门禁**：HEAD `afe42ef7b25fc517b9af38a85aa88532c6c7dce5` 加既有日志工作树及本轮窄修；697项源码输入 SHA-256 `20e04b3613570a226ba1b971806b772f9edceb19773ba6b5c1c435fb3023f4db`。在标准目录使用独立 Cargo 输出缓存，避免先前源码归档共用旧 target 产生的过期产物。`cargo fmt --all -- --check`、workspace/all-targets Clippy `-D warnings`、`cargo test --locked --workspace --no-fail-fast -- --test-threads=1`、`cargo xtask check` 均通过；85个测试目标合计737通过、0失败，8个ignored条目是由上层强杀/恢复测试显式调用的子进程夹具。不是拼接旧 SHA 的结果。
- **失败及修复**：首次完整测试发现3个失败目标。用户包 Plan 调用栈在默认 Windows worker 栈上溢出；崩溃夹具已登记用户包但没有Plan/任务，尚未执行删除缓存或Apply。将 `verify_object` 跨 await 的64KiB数组改为同长度堆缓冲，校验/配额不变；现成库类型量测的 `PackageCache::get` future 从66800降至2704字节，原测试在默认线程栈下及完整回归均通过。候选测试仅排除自有 `data/logs/core`（包含独占writer.lock和异步日志），继续完整核对DB/WAL、缓存、配置、项目、Events、Operations与网络零副作用。未登记UUID的实际既有错误是 `project_not_registered`，修正测试及其失败活动预期，保留原错误/活动一致性断言。定向8/0后集中重跑完整门禁；未扩大线程栈、放宽业务校验或吞读取错误。
- **前端与产物**：根workspace `npm run check` 通过；Tauri `build --debug --no-bundle` 完成GUI check/Vite build及嵌入前端的原生GUI构建，既有chunk大小警告保留。Core、CLI和extension-host使用同一源码配套构建。最终构建与部署前后输入指纹一致；本轮未重跑整套浏览器或release/三平台CI。
- **收尾检查**：更新后额外运行Python元数据检查，发现其严格workspace清单遗漏已批准的`alcomd-logging`，仅补齐该成员后通过，未知成员仍拒绝。最终输入SHA-256 `5e2ce8c404876fe7d380a6caf2bc137f111584d56d7f8e4436ff614db3ade276`；与上述门禁/构建输入逐项比较，唯一差异为`scripts/validate-metadata.py`，所有Rust/前端编译输入及产物不变，因此只重跑受影响元数据/文档检查，不重复IPC、完整Rust或构建。未将验证器修改描述为重新构建。
- **真实IPC与恢复**：先复制离线原数据，关闭副本自动发现，使用新Core、认证的真实用户named pipe与每连接服务端PID核验。State18→19后31张既有表所有行保持，9组公开查询保持（副本仅autoDiscover=false；Template可选unity声明是已批准增量）。再用全新隔离数据/目标验证必需完整unityVersion、无效版本拒绝、不可变Plan字节/hash、Apply→事件终态→项目登记→launchOptions无已安装Editor，未启动Unity；同实例安全游标续传、幂等Apply及单次终态消费通过。日志实时/历史/物理JSONL的来源/会话/段/offset及正文一致；重启Core后旧事件游标明确resyncRequired，新日志session、稳定sourceId及旧会话记录可读。原生脚本首轮将.NET异步void完成值混入返回对象而误报握手，保留失败帧并仅修正验证器，产品协议未改。
- **实际更新**：原数据、profile及旧二进制离线备份并逐字节核验；替换前再次保存原数据。现 `target/debug/alcomd.exe` Core **162992**（SHA-256 `296AEA74…26A26`）、`alcomd-gui.exe` GUI **135892**（`E3677D27…BDE8C`）及匹配CLI/host已更新。原 `target/m7-icon-acceptance-20260906/data`、`target/m7-h4-extensions-20261102/original-gui-profile` 沿用，profile启动前逐文件一致，实际WebView进程仍使用该profile，窗口有句柄且可响应。GUI打开后再次完整分页对照9组状态（3项目、1安装、15个已成功任务、2备份、3模板，其他资源为空）、Config10/revision189及配置字节保持。Core日志ready且所有故障/丢失计数为0；GUI继承原RUST_LOG=warn，正常启动INFO被过滤，未制造错误填日志。Unity/Hub六进程身份和9份项目清单/版本文件摘要保持，无隔离Core遗留。

上述迁移实证覆盖本机全部15个已终态任务；不据此承诺旧State18活跃任务接续，亦不回填旧活动。原事件/任务没有删除。原生GUI仅核对启动、profile和响应，不冒称操作/视觉验收；本候选整套浏览器、真实macOS/Linux Editor、物理掉电及三平台CI/CodeQL仍未执行。H2–H6签收保留，M7 IN_PROGRESS / H7 MANUAL_PENDING，最终确认由所有者完成。

原始日志、源码清单、失败轮次、新旧库future量测、迁移逐行比较、IPC帧、物理记录及二进制hash统一保留在标准目录 `target/m7-full-gates-update-20261105/run-20261105T093004Z/`；关键结果为 `rust-test-counts.json`、`migration-row-preservation.json`、`native-pair-result.json`、`deployment-result.json`、`updated-after-gui-comparison.json` 和 `protected-final.json`。本节补齐此前独立批次声明未执行的完整Rust/固定用户端点IPC和原环境更新，不改写那些历史结果。

## 当前有效合同

### 模板系统清理与重建（公共合同与删除清单已获所有者批准）

分两批验收。第一批停用旧模板系统，保留资源页中的模板空页，并提供独立的新建项目。第二批启用统一的模板声明和本地管理。旧模板格式和接口在第一批切换后失效。开发期模板数据直接废弃。以下 Schema 编号以当前工作树的 State21 为基线。

#### 第一批：State22 与独立新建项目

- 新增 `projects.create.v1` 和 `projects.create`。只允许已认证的本机所有者，检查现有 `projects.create` 权限。请求为 `{name, targetParent, unityVersion, idempotencyKey}`，返回 `{operationId, projectId, replayed}`。`name` 使用现有跨平台单段项目目录校验，`unityVersion` 为完整规范 Unity Editor 版本，允许尚未安装的版本。幂等作用域与现有规则一致，规范请求相同则返回原任务，不同则返回 `idempotency_conflict`。
- 新任务 kind 为 `projects.create`。接受事务分配项目 ID 并记录请求和恢复信息。Core 在同一父目录暂存最小 Unity 工程，发布前核对父目录身份，使用平台独占重命名，目标存在时保留原目录。发布后同事务登记项目、终结任务并写入 `project.registered` 和任务变化事件。恢复只处理能核实归属的暂存或已发布目录。GUI 保留默认位置、已安装版本和自定义版本，接受后跟踪原任务，成功后打开项目详情。CLI 的 `project create` 调用同一入口。
- 删除 `templates.*` 方法、`templates.read.v1`、`templates.manage.v1`、`templates.create-project.v1` 及 `templates.read`、`templates.manage` 权限名称，连同旧模板命令、SDK 与前端适配、Schema、包解析/生成、注册、恢复和模板变更通知。资源页的模板标签继续打开空页。其他使用 `projects.create` 的入口保留原有授权要求。
- State22 删除 `templates`、`template_plans` 及其索引和触发器。从共享表清除 `templates.import`、`templates.derive`、`templates.create-project` 三种旧任务及其 journal，清除这些任务的幂等记录及 `templates.applyPlan`、`templates.setFavorite`、`templates.remove` 方法的幂等记录。清除 `aggregate_kind='template'` 事件和已确认旧任务 ID 对应的 operation 事件。清除 `operation_log_entries` 中关联这些旧任务 ID 或结构化 action 以 `templates.` 开头的记录，覆盖 `templates.favorite` 等独立于任务的日志。保持保留行、外键约束及事件和日志的自增水位。更新 operation kind 和 event aggregate kind 的 Schema 约束。
- 迁移开始时若仍有未终结旧模板任务，先在升级旧库之前拒绝打开，确保旧 Core 还能正常完成或取消任务。已创建的项目、项目事件、非模板任务、设置和共享包缓存不在删除清单内。Core 首次成功升级 State4–21 后检查数据根直属的 `template-store` 与 `template-builtin-staging`。只有路径、文件系统身份和旧产物命名均可核实时才清理。无法核实的内容留在原处，Core 启动输出说明失败原因。真实 profile 切换另按运行环境授权执行。

#### 第二批：State23 与统一模板声明

- 新增 `projectTemplates.v1` 及 `projectTemplates.list`、`projectTemplates.save`、`projectTemplates.remove`，初期限已认证本机所有者。`list` 按 ID 稳定分页，沿用现有有界列表参数和游标约定。记录返回 `{declaration, revision, locked}`。`save` 接收 `{declaration, expectedRevision?}`，省略修订表示创建，提供修订表示更新。创建同 ID 同内容返回已有记录，同 ID 异内容报告冲突。更新及 `{id, expectedRevision}` 删除均先验证当前修订与锁定状态。相同内容不产生新修订或变化事件。
- 声明为 `{id, name, unity, dependencies, constraints}`。ID 必填，点分隔的非空小写 ASCII 段，每段以字母或数字开头及结尾，允许内部连字符，最多 128 字节。名称可重名。Unity 条件由允许的 major/minor 系列和精确版本预设取并集，两者为空时接受合法完整版本。依赖为 `{packageId, versionRange}`，条件约束为 `{unityVersion, packageId, versionRange}`，只限制已被求解器纳入的包，不追加直接依赖。声明统一校验并生成确定性 UTF-8 JSON，内部记录 UUID、修订、锁定标记和时间戳在 JSON 之外。
- State23 新增模板目录和目录修订分配状态。记录保存 owner、公开模板 ID、内部 UUID、规范声明 JSON、正整数修订和锁定标记，按 owner 和公开 ID 唯一。修订在同一事务分配，跨删除和重建不重用。预置 Avatars 和 Worlds 使用与用户声明完全相同的结构和校验，分别使用 `com.vrclearn.alcomd.templates.vrchat.avatars` 和 `com.vrclearn.alcomd.templates.vrchat.worlds`。预设依次为 `2022.3.22f1`、`2022.3.6f1`、`2019.4.31f1`，2019 对应 SDK 与 Base 上限 `3.4.2` 由条件约束表达。内置锁仅影响编辑及删除。普通表单可生成与内置逐字节一致的声明，同名不同 ID 可分别保存。
- `projects.create` 请求增加可选 `template: {id, revision}`。Core 接受任务时在事务内校验修订并冻结声明，旧修订或已删除记录提示重新选择。使用模板依赖的创建还检查现有 `packages.read`、`repositories.read` 和 `packages.manage` 权限，通过同一求解器锁定来源、版本和内容后再发布。GUI 默认不使用模板，无独立 Blank。模板创建、更新和删除同事务写入 `project-template.changed`，沿用当前事件入口，事件 aggregate ID 用内部 UUID，GUI 按模板目录失效处理。

#### 验收与交付边界

第一批在隔离 profile 验证旧入口消失、空页零模板请求、State21 定向清理与中断拒绝、外键和水位保留、GUI/CLI 无模板创建、目录冲突、幂等及发布恢复，并回归登记、复制、备份还原。第二批验证权限、同名不同 ID、同 ID 的重用和冲突、删除后重建的旧修订、内置不可编辑删除、普通表单与内置字节一致、六组内置模板与 Unity 版本组合、依赖约束及事件更新。同步协议规范、Schema、SDK 和契约向量，分别记录验证。H2–H6 历史材料保留，M7/H7 最终确认由所有者完成。

### 页面提示边界

页面只渲染自身内容。成功、失败、后台同步及任务进度均不在已加载的页面、列表行或页头追加提示。必要的确认、校验和失败细节可保留在已经打开的操作框内；没有合适位置则省略提示，沿用原操作控件或任务中心。首读失败时的替代内容保留重试。禁止为被删除的提示新增容器、浮层或弹窗。

### 单页业务操作日志（2026-11-06，所有者批准实施）

所有者在完成旧系统撤销后要求重新制作一个日志页，并确认记录范围为“业务操作及Core关键步骤，保留历史”。新功能只有一个连续滚动列表，没有活动/技术日志切换、分页按钮、加载更多按钮或浮动提示。本节替代下节“不提供替代日志”的限制；旧系统撤销结果和数据保留边界继续有效，不恢复旧实现或旧格式兼容。

**界面**：生产`/logs`由共享Core客户端读取，沿用PageHeader、DesktopList、Material按钮、共享路径显示及三语；显示时间、操作者、操作与步骤、目标及结果，完整路径换行，实际失败详情可展开。展示组件只消费记录。浏览器专用样例与真实数据验证分开；初次读取失败可以重试，已有记录时保留列表。

**已批准的最小新增合同**：State21的`operation_log_entries`、只读`logs.read`及`logs.read.v1`能力、现有events.batch的`operation-log`失效范围；不新增权限、配置、生产依赖或第二套任务状态机。所有者随后明确要求标注操作者：持久`actor`为实际发起入口，如GUI、CLI、MCP，来自现有握手客户端元数据，不用于授权；Core自主触发时标Core，未标识具体客户端的本机IPC连接标Local IPC。后台执行与恢复继承任务发起入口；取消请求自身标记提交取消的入口。不标注未建模的“用户”“AI”身份。MCP及Local API自身的业务接线仍属后续里程碑。

1. **记录及存储**：Core唯一写入，新增一张追加表。列为`sequence INTEGER PRIMARY KEY AUTOINCREMENT`、`entry_id TEXT UNIQUE`、`owner TEXT`、`execution_id TEXT`、`actor TEXT`、可空`operation_id TEXT`、`occurred_at_ms INTEGER`、稳定`action TEXT`、稳定`step TEXT`、`outcome TEXT`（started/succeeded/failed/cancelled/waiting）、`target_json TEXT`及可空`details TEXT`。索引`(owner, sequence)`及按执行/任务关联所需索引；目标快照和正文均不可因目标改名/删除被改写，不设级联删除。任务继续由Operation决定状态；每条日志只描述一个已发生的事实，不更新成另一套活动状态。
2. **字段边界**：公开条目使用camelCase对应字段，`target`仅含kind、可选id、name、path；Core使用闭合action/step映射，不持久化翻译整句。`details`保留实际执行处的必要结果或原始错误原因链，完整正常路径不脱敏，不打印整个请求、argv、配置或带凭据URL。单条完整JSON编码上限512KiB，details上限256KiB、单路径上限128KiB UTF-8；超限明确报告，禁止静默截断路径或伪造成功。普通业务RPC的错误字段边界保持。
3. **实际发生处记录**：进入明确的业务执行、检查/生成计划、接受执行、实际公开阶段、提交结果、实际失败/取消/恢复时记录。请求鉴权、协议解码、普通查询、内部预读、切页、搜索不记；GUI/Core RPC编码层不补写。Plan是同一业务流程的真实步骤，计划流程使用Core生成的planId作为executionId，Apply继续该关联；只预览未执行也如实停在已完成的计划步骤，不伪装成执行成功。Operation接受后其operationId持续关联同一executionId；内部逐文件/字节进度不逐个生成行。
4. **提交与失败**：数据库业务接受/状态/终态和对应日志在同一事务；回滚不留下“已提交/成功”，幂等重放和重复公开阶段不新增相同事实。真实重试新建executionId，可再次记录失败。外部I/O前后步骤在实际application/adapter边界记录，不能把进程启动成功称为项目加载完成。已认证且已授权的语义明确执行在检查失败后可独立保存失败事实，业务对象/任务/原幂等状态保持不变；此日志是拒绝请求零业务副作用的显式例外。无效/未授权请求不产生业务日志。数据库不可用时不能保证历史写入，保留原失败并明确呈现存储失败，不以GUI缓存补造记录；不承诺文件系统、进程与数据库原子提交。
5. **读取**：`logs.read`仅已认证local-owner且有既有`operations.read`权限；按owner读取，不接受路径参数。请求`beforeSequence?`/`afterSequence?`互斥、`limit?`默认100最多200；不传位置读取最近记录，before向历史读取，after从最早尚未接纳记录向新记录读取。响应`entries`始终sequence升序、`highWatermark`来自同次读取快照、`hasMore`和有剩余时的`nextSequence`；旧向为本批最小sequence，新向为最大sequence。sequence在JSON安全整数范围；完整响应不超过现有4MiB，按编码大小停止在完整条目边界并提供续读位置，不丢弃超帧剩余内容。空日志与读取失败严格区分。
6. **更新和历史**：Store实际提交后比较已提交日志水位，再通过现有共享事件订阅发`invalidatedScopes:["operation-log"]`；只带失效，不复制日志正文。复用同一连接、ACK、owner隔离和故障恢复，注册监听后读初始日志；读取期间再变脏补读，健康空闲无周期读取。实例变化/断线后按最后已接纳日志sequence追齐；日志本身不因Core实例更换改变身份。日志失效不能导致资源/任务/计数等无关查询，也不为每行再写持久Events。只读取公开事件的客户端不会因此被授予日志读取权限。
7. **无分页界面**：记录按最新在前展示，向上/下滚动自动读取相邻范围。内部有界传输不作为用户分页。显示窗口最多500行或8MiB；离页只保留读取锚点，返回从锚点重新读取。新记录不会把正在阅读历史的人拉回顶部，断线后按Core权威记录继续读取。持久历史不自动清理，也不转换已撤销旧日志/活动。

覆盖映射（实施和验收按此检查，不以RPC调用数冒充业务覆盖）：

| 来源 | 实际记录处及关键步骤 |
| --- | --- |
| 项目登记/移除/收藏/发现 | registry/project-discovery用例及Store提交；发现扫描只记实际发现结果，空扫描不刷屏 |
| 软件包解析/计划/应用 | M4用例、真实adapter阶段、接受/提交/失败/恢复；读候选/查列表不记 |
| 仓库、用户软件包、模板 | 获取/导入/校验/生成计划/应用/移除及真实结果；自动读取列表不记 |
| 备份、还原、项目复制/删除/Unity版本迁移 | 各执行用例及现有公开阶段；目标路径快照、提交或失败原因 |
| Unity安装管理/查找、Hub/项目启动 | 实际扫描、登记、检查和进程派发结果；其他Unity进程存在不生成假操作结果 |
| 扩展安装/授权/启停/卸载 | M6实际执行/生命周期边界及Store提交，不记录空状态读取 |
| 明确设置修改、取消及重启恢复 | application实际保存/取消/恢复边界；只记变化项名称，不记秘密或整份设置；重复无变化及窗口视图缓存不记 |

实施：批准新增合同后同步Schema/强类型/客户端，完成Store与真实业务边界，接入共享事件和真实状态驱动夹具，再接产品导航；在隔离目录验证完整业务链路后才构建/更新原环境。保留所有旧删除工作树和历史材料，不重新引入提醒、recordGui、RPC结果摘要或任务终态伪日志。定向检查先行，稳定候选集中回归，M7/H7仍由所有者签收。

### 日志、活动记录与提示系统撤销（2026-11-05，所有者明确要求）

所有者当时撤销日志重建，要求彻底删除日志、活动记录及提示系统。本节保留已完成删除及获批State20的边界；后续新增功能仅以上节单页业务操作日志为准，不恢复下方历史日志/提醒授权。

- 删除日志与活动页面、导航、筛选、视图缓存、订阅/读取桥接、GUI异常日志上报、共享Reminder组件及专用样式，不以改名保留旧系统。
- 删除Core文件日志管线、日志读取/健康RPC、独立活动RPC/DTO/事件/权限、生产日志依赖及业务结果反向补记。普通业务错误、权限校验、破坏性确认、任务与恢复、资源变化通知保留。
- 清理SDK、Schema、方法覆盖和测试夹具；只删除被撤销功能的断言，其他业务失败、重试和完成行为继续验证。
- 原文件日志、原数据和历史验收材料保留，不清理Git历史，不推送，不操作Unity或真实项目。
- 存储删除边界（所有者已明确批准）：以State20在同一事务移除独立activities表、五个触发器（activities_created、activities_changed、operation_activity_created、operation_activity_required、operation_activity_changed），以及仅该功能产生的activity.changed事件；其他业务表/Operation/恢复记录/事件的内容和sequence保持。移除State19活动创建实现，空库不再建立该功能。实际原库升级前保留可核验完整副本；本删除清单已获人工审批，原库升级前仍须核验备份，不将其他持久数据纳入删除。
- 先完成源码和合同删除、隔离存储/业务回归，再决定是否更新运行环境；不得用旧候选门禁冒称清理后通过。M7/H7仍待所有者最终确认。

本轮实际交付：上述源码/合同删除完成，新增State20仅实施已批准删除清单。副本和原库真实可执行文件验证均保留31张表中其他业务原行、事件身份/水位及4份历史日志；仅删除1条活动和2条activity.changed。原数据/profile已完整备份并逐文件核验SHA-256。新版Core171196/GUI86372沿原位置运行，9组分页业务快照、配置字节与Unity进程一致，窗口可响应；无桌面控制、提交或推送。

验证绑定HEAD ce59b7f加当前工作树，构建前后生产输入一致（SHA-256：1e5d881929541f8d4ff960137d1551cd40a99e7731ee449bbb6fc8faf096628b）；完整Rust706/0，8项ignored为上层驱动的子进程夹具，fmt/严格workspace Clippy/xtask、前端workspace构建、原生debug构建、定向浏览器77/0及40/0通过。覆盖State20回滚/空事件表水位、撤销接口拒绝、真实IPC、事件竞态/背压/稳态无轮询、错误/重试/未知写入结果、包/备份/模板/项目/扩展完成行为。完整浏览器试跑存在旧Theme显式保存/取消等与既有自动保存不符的断言，未宣称最终完整GUI回归或原生人工签收。证据：target/m7-remove-observability-20261105/run-20261105T145154Z/、target/remove-observability-*.log；既有历史失败轮次保留。


- 所有者于2026-11-05明确批准完整移除手动启动状态查询：删除菜单/弹窗、GUI桥接、Core RPC/application用例和专用Store读取port、Rust SDK方法、CLI命令及专用DTO/Schema。正常打开Unity、安装选择、启动幂等记录及接受/失败反馈保持，不删除共享记录或迁移既有数据。本次为首次公开发布前已批准的接口破坏性删除，RPC major仍为1；原方法按标准未知方法处理，不提供兼容别名、替代入口或轮询，权限名称、数据库、配置与依赖不变。
- GUI 继承 A-033 的 v3 用户模型，使用真实 Material Web 控件和共享组件。Projects、包管理及
  已获准的其他既有页面基础可用性修正已在 H2-A 验收；H2–H6 原签收保持，当前只推进已批准 H7 最终整体验收。
  Open Unity 保留单个 leading play 图标和 Material 按钮的 18px 图标尺寸。Projects 的视图入口
  显示当前模式并保留视图偏好；内容包装透明，卡片和表格使用自己的表面。
- `packages.queryProjectCandidates` / `packages.candidates.v1` 按已批准的
  `M7-package-candidate-evidence-stop-a.md` 提供完整、来源保真的候选证据。GUI 不解析
  SemVer/Unity，不以可选元数据首页推断来源缺失或可操作性；行内版本、条件更新与批量操作仍
  复用原有单次 Plan、显式 Apply 和一个 Operation，不更改普通 resolver/来源/恢复策略。
- 当前源码基线为 **State v20**，按本节批准撤销独立活动，保留原任务及其他业务事件；按 A-035 允许项目显式空参数覆盖。v15 引入独立的 `lastModifiedAtMs`；v16 移除项目观察时间，
  保留安装扫描、writer 检查及迁移专有恢复证据。候选/迁移的权威投影显式白名单排除展示时间，
  保持原冻结字节与 hash；旧未 Apply 的 Unity 迁移计划按合同过期，不重签原审批。
- 按 A-034 保留 Schema 识别机制，hello 报告成功初始化后的真实存储版本。未发布开发库不
  承诺长期兼容，仅为明确需要保留的数据维护迁移；当前不降回 v1、不删除现有迁移；已批准的 State17 窄增量见 A-035。
  发布前初始 Schema 整理是独立审计任务，不自动清空数据库、Plan、Operation 或恢复证据。
  本轮 hello 修复只纠正已有 `dataSchema` 事实值与内部构造参数，不改 wire DTO/Schema 形状、
  RPC major 或 capability，属于非破坏性修正；由 dispatcher、兼容形状和实际 SQLite 回归验证。
- 当前设置合同为 **Config10**，含默认目录、项目自动发现、备份默认选项、Unity 全局参数及默认关闭的窗口记忆开关；保留已批准的紧凑模式及 `appearance.density` 删除。
  application 持有权限、CAS、字段校验、候选缓存和取消一致性；
  `ConfigStore` 窄 port 的文件适配器位于 store，负责 strict 16 KiB TOML、Config1–9→10 和
  原子替换/恢复。普通设置修改即保存，以当前连接内的串行窄 PATCH 协调；失败保留意图、
  显式 Retry 先重读，未知已提交不重复写，换连接停止旧队列，不保留全局 Save/Discard 栏。
- 主题编辑器直接展开于外观页，与其他偏好共用串行自动保存队列；sourceColor/mode/scheme 只提交实际改动字段，保持实时显示、未知结果重读去重和显式失败重试。
  默认种子 `#6CB6FF`、Vibrant、跟随系统及九方案不变，模式下拉顺序为“跟随系统 / 浅色 / 深色”。共享 Material HEX/拾色器/
  Select 使用标准控件及现有设置区块间距；模式、主题色、配色方案按共享设置行纵向排列，模式与九方案均用下拉框，不保留主题专用分段和两列布局。
  删除主题弹窗、HCT 三个调节条及独立 Save/Cancel 和草稿离开确认。HEX 仅接受完整有效颜色，未完成输入失焦回到当前有效值；保存中可继续编辑，后续改动合并排队。
  短请求保持动作名称，不插入瞬时状态行。
- 字体只引用本机 Noto Sans/Mono 及中日 CJK fallback，不打包或联网加载字体。三语言为
  en-US/zh-CN/ja-JP，使用静态词条和 React 文本插值；外部内容不翻译、不解释 HTML。
  动画三态为 system/full/reduced，默认 system；共享适配器处理 CSS/Shadow DOM/弹层及滚动条，
  不恢复已撤除的切页动画。
  日志页沿用“活动记录 / 技术日志”的用户术语；技术日志的en-US名称为Technical Logs、ja-JP为技術ログ，搜索、筛选、表格及空状态同步。“诊断ID”继续表示错误排障编号。
  所有页面移除通用刷新及专用承载，不保留标题刷新槽或扩展宿主刷新确认分支；仓库获取、软件包重新读取、业务完成后的数据更新及错误重试保留。
  所有者已否定为删除按钮而增加定时业务读取，并于2026-11-04批准下方完整变化通知合同；本轮按提交后通知改造项目、任务、活动及任务详情，移除3秒/750ms轮询及旧追赶逻辑，不恢复通用刷新作为替代。
  已有分页、去重、筛选及阅读位置保护保留；以本轮真实代码和定向证据确认事件覆盖、恢复及背压，不把旧轮询候选的通过记录当作新链路验证。
  日志筛选复用标准 Select 动画，移除页面的强制 quick 覆盖；筛选值经已有关闭后选择流程提交，避免连续切换与前次收起动画竞争。Select 只依据统一动画偏好跳过动画，不再暴露页面专用 quick 开关。
- 原生默认4px容器圆角的Field、TextField、Select及Menu，在共享主题中将组件默认形状统一映射到Material medium（12px），包含filled/outlined本体和选择菜单；使用点直接继承。保留extra-small=4px、small=8px及其他形状等级，不通过修改圆角等级改变组件默认。搜索框和既有共享选择器保持胶囊形状，不改变控件尺寸和间距。
  提醒框（含浮动出口）及仓库列表导入/导出预览框共用容器28px；代码底色及跳转正文链接使用small（8px），状态标签、扩展数量徽标和拾色器使用full。删除日志搜索框的旧outlined专用覆盖，继承共享搜索控件；不将表格、字段、菜单或滚动条的既有层级改成容器圆角。
- 操作统一使用共享 Menu（40px项高/14px字号）；来源、版本与设置项统一使用真正的共享 Select（56px选项/16px字号及同一胶囊本体），删除按钮自建的选择菜单及其类型分支。版本首次展开才按原64条页及既有限额自动续读，关闭停止续页、同快照重开续载；候选身份/校验/权限及原Plan→Apply保持，选择关闭后才打开审查，取消保留已安装值。固定宽度选择器截断长标签并保留箭头，展开列表在视口内使用原生菜单滚动，不影响按内容测量的Unity选择器。
- 公共 DesktopList 统一列、排序、截断和稳定正文行盒；外轮廓直接引用现有Material large（16px），表头、末行及视口共用，删除自定义20px形状变量且不修改圆角等级定义；行内仍8px，外层布局不重复按28px裁切。行距仅保留在行间，表格首尾不留额外行距；边界裁切不形成滚动容器，保留横向列宽及吸顶。
  卡片内表格按所有者确认采用表面色分层：SettingsSection内的表头和行使用surface-container-high，视口与吸顶表头底板使用卡片的surface-container-low，使间隙透出卡片底色。保留原列宽、内边距、圆角、行距和滚动，不增加组件选项；独立表格不变。平直行/分隔线嵌入方案已弃用，移除其源码备份、补丁及候选程序；历史截图和验证记录保留。
  软件包管理工具栏与项目网格排序栏共用alcomd-filter-bar，独立显示在表格上方；外部仅保留透明布局，去除包住工具栏和表格的背景、圆角及阴影，工具栏与表格保留12px正常间距，不设置内联提示或状态承载。无更新、普通加载、候选不确定、目录不完整和批量限制不主动弹提醒，也不为这些状态新增诊断日志。候选读取失败/过期在现有版本信息列显示，重读复用工具栏按钮；仓库刷新只保留按钮忙碌状态及原有操作结果反馈，相关权限、候选校验和操作禁用保持。
  PageHeader、SplitButton、Dialog、ScrollArea 和 SegmentedButtons 统一持有布局/生命周期。无图标分段按钮内部左右各 24px，采用 Material 自身等宽布局，
  移除勾号选项及隐藏宽度测量层，不写死文案宽度；所有者允许过窄窗口放不下，包括本次 640px 英文设置导航，不增加滚动兜底。滚动条只在实际滚动或轨道悬停时唤出，沿用库计时和销毁语义；textarea
  内部编辑滚动保持原生。离线页面恢复同时重新协商当前连接能力，不依赖切页刷新能力。
- 异步 Plan 的触发控件通过稳定本地 origin ref 返回焦点；共享 Dialog 等待公开 closed、
  业务控件恢复可用及 Material updateComplete，不因修复焦点而放宽 disabled/eligibility。
  MenuItem 统一消费公开 click/Enter/Space 选择并只在 closed 后派发一次；Escape 不派发。
  Select 由真实 busy 状态控制禁用，选中标签按公开生命周期同步，不增加输入/设置写入。
- 按最新整体批准的“日志系统重建”移除全部浮动提示和日志触发提示；保留字段校验、就地失败、显式重试、确认、导航与完成回调。错误详情及诊断编号继续可读，未知结果仍先核对，不因删除提示掩盖失败或重复执行。Core/GUI不再拥有本轮删除的日志生产、读取或补记管线。此前ReminderProvider和按源弹出合同仅保留为历史。
- Unity Hub 位置复用 settings.get/update、settings.read/manage 与 Config5 `unity.hubPath`；
  有界只读发现和路径规范化由 platform 提供，不加注册表/PATH/Hub CLI 扫描。路径草稿 Enter
  或离开操作组保存，Esc 撤销，IME/同组焦点切换/无修改不写；选择和自动查找共享串行 CAS。
- `unity.hub.open` / `unity.hub.open.v1` 要求 settings.read 和独立 unity.hub.open，
  初期仅认证 local-owner；application 在协调锁内重验 revision/有效路径，独立 Launcher
  预建有界 worker 后 spawn 并回收自己的 Child，最多四个，不 kill 用户进程或自动重试未知结果。
  仅无显式偏好且确实未找到时返回 download_required，由闭合无参数 Tauri adapter 打开固定
  官方下载页。Auto detect/Open 位于卡片右上角、Select 留在编辑行；未确认草稿阻止启动。
  No shell 指应用不构造、调用或通过 shell 执行命令，不拼接/解释 shell 参数；保留标准库与
  OS 的 ENOEXEC/解释器回退语义。spawn 成功仅为 OS 接受，不代表 Hub 已正常运行、厂商
  身份、退出成功或窗口就绪；同步拒绝、后续退出与资源回收分别验证。
- 正式合同与证据分别见 `specs/config/settings-v10.*`（Config1–9 仅为迁移历史）、
  `specs/config/theme-controls.proposal.contract.json`、
  `specs/config/unity-hub-location.proposal.contract.json`、
  `specs/storage/state-v15-project-last-modified.contract.json`、
  `specs/storage/state-v16-remove-project-observed.contract.json`、
  `specs/storage/state-v17-empty-unity-override.md` / `.contract.json` 和对应 testing 文档。
  名为 proposal 的已批准合同仍由这些既有文件直接维护，不建立第二份有效合同。

本轮 Schema 策略与架构审查修复的逐轮证据见
[修复验证记录](../testing/m7-architecture-review-fixes-20261031.md)；当前阶段仍统一见 `docs/status.md`。
测试门禁使用 ready 且由测试 handle 持有端点的 daemon，Windows 占用时在创建客户端前失败；
生产初始化顺序和每用户唯一端点保持。Chromium suite 将三个现有 fixture 静态预构建，保留
development React/StrictMode、原断言、零重试与 30 秒门槛；不改变生产 Vite 配置、依赖版本
或系统 TCP 参数。H2-B 资源加载专项仅在测试浏览器回移 Playwright 官方的
`TcpPortRandomizationWin` 修正，完整保留原 16 项默认禁用列表及其他参数，并用版本 guard
要求升级时复核。fixture 以 `Connection: close` 在响应完成后关闭连接，减少 context 销毁
留下的客户端 TIME_WAIT；所有资源继续通过同一 Vite fixture 的真实 HTTP 加载。专项的
原失败轮、连接对照与该候选独立完整 292/0 结果见
[H2-B 验证记录](../testing/m7-h2-b-20261101.md#chromium-资源加载专项)。前置完整结果和真实桌面范围见
[测试门禁与桌面验证](../testing/m7-test-gate-closure-20261031.md)。
H2-A 本轮真实 Apply、失效 Plan、焦点、筛选、偏好重启及签收图集见
[Visual Gate 2 材料](../testing/m7-visual-gate-2-20261031.md)。

项目所有者于 2026-10-31 明确批准 P6 wire 修复：P6 source selector、reinstall selection 和
bulk intent 的 Rust enum variant fields 使用已冻结的 camelCase；候选查询使用独立公开
source DTO，保持已冻结的 snake_case Schema/vectors 不变。该修复会替换当前错误接受的
P6 snake_case 字段，属于实际 wire 的破坏性纠正；按此次未发布合同审批直接修复，RPC v1、
State16、Config3、权限和依赖保持。不添加旧字段别名、双格式支持或 GUI 回退。

## 组织身份统一（2026-11-05，所有者已批准）

所有者明确要求将个人发布身份统一为组织：发布者及自有作者/版权声明为`VRCLearn`，
Bundle/Tauri/Linux desktop ID为`com.vrclearn.alcomd`，声明的Windows AUMID为`vrclearn.ALCOMD`。
本次批准直接修订尚未发布的v4永久身份，原A-001/A-003及ADR0001原位同步。

- 产品身份仍以`alcomd.product.toml`为唯一来源，同步Tauri、第一方扩展派生ID/发布者、
  Discord占位backend、GUI夹具/路由、当前规范、校验和禁止旧身份回退规则。
- Windows pipe/mutex命名空间同步为`vrclearn.ALCOMD`；这是未发布端点及第一方扩展身份的
  破坏性替换，不增加旧值别名/双身份支持，不改RPC形状、权限、State18或Config10。
  更新运行环境时必须成套停止旧Core/客户端后替换；数据目录仍为`ALCOMD`，禁止新旧Core
  对同一数据目录并行运行。本轮只修改及验证源码，不部署或停止当前桌面程序。
- 产品名/二进制/URI及数据目录保持；所有者进一步明确当前网站/服务基址为
  `https://alcomd.vrclearn.com/`，v4自有Schema的`$id`/`$ref`及引用同步新域名。
  v3迁移识别值、冻结请求源、历史证据和真实Windows用户名保持，旧入口的实际衔接在M11验证。
- Windows AUMID按下方原生接入合同应用于GUI启动，值从产品配置派生；
  Inno AppId仍按M12独立确定，本轮不创建安装器身份或安装快捷方式。
- 验证覆盖metadata/xtask/fmt、受影响Rust检查和扩展/GUI定向回归，不新增依赖，不执行真实项目写入。

已执行的各项结果、浏览器旧夹具失败及并行修改边界见
[组织身份验证](../testing/organization-identity-20261105.md)，不据此提升 M7/H7 人工签收状态。

### Windows AUMID 原生接入（2026-11-05，所有者要求补齐并分次本地提交）

- GUI 在创建 Tauri Builder/窗口之前调用 Windows Shell 的进程级 AUMID 设置，设置失败即停止启动，
  不带着未知身份继续显示窗口。值由构建脚本从`alcomd.product.toml`派生，文件变化触发重新生成。
- FFI 放在`alcomd-platform`独立的 Windows 私有模块，使用现有已锁定的`windows-sys 0.61.2`
  和已启用的Shell/COM feature。仅此模块允许有文档的unsafe，不放宽GUI或其他模块的门禁。
  输入先验证空串/内嵌NUL/空白/UTF-16长度，指针由在途UTF-16缓冲区持有，HRESULT显式检查。
- GUI 直接复用内部`alcomd-platform`薄系统适配器；构建脚本增加既有workspace `toml`作为构建依赖。
  替代方案为GUI内重复引入FFI或手写TOML解析，均不采用。没有新增第三方包或版本升级；维护成本
  限于两条既有依赖边及一个受控Windows FFI模块，非Windows启动保持原路径。
- Windows测试在独立测试进程中设置并通过`GetCurrentProcessExplicitAppUserModelID`读回确认；
  同时验证非法输入不改变当前身份。系统返回缓冲区用`CoTaskMemFree`释放，不启动GUI/Core或访问用户数据。
- 第一笔提交组织身份与域名统一，第二笔提交原生接入、依赖边、边界校验及最终验证记录；只本地提交，
  不推送、不替换当前桌面程序。安装器快捷方式使用同AUMID仍属于尚未实现的M12，不虚报完成。

## 任务与页面变化通知整改（2026-11-04，已实现本地候选，人工确认待定）

所有者已批准M7变化通知计划及完整补充，并授权直接实现、定向验证和分批本地提交；本节替换此前研究提案，不再增加纯文档审批。目标固定为Core提交变化→发布通知→GUI按需更新，不重新设计应用，不进入M8–M12。

### 公共合同与范围

- 新增`events.subscribe`、`events.live.v1`；认证/协商后先返回带请求id的就绪响应，再发送独立无id的`events.batch`通知。`events.closed`为明确终止原因的控制通知。Notification Envelope及Schema独立于普通Request/Response；不放宽响应id，不以id:null冒充通知。
- 就绪包含subscriptionId、instanceId、baselineCursor和resyncRequired；批次包含订阅/实例身份、batchId、现有Event、有界后续游标。游标另含resourcesRevision，批次含固定invalidatedScopes（仅extensions）：既有M6同步启停/权限/runtime缺少持久资源事件且State18不允许扩展聚合，因此仅在实际提交后发送失效标记，不伪造Event、不改数据库。仅失效批次必须推进资源版本，保持持久sequence；两种内容都空的批次非法。沿用events.read、认证和owner过滤，初期只接已认证local-owner；事件不授予资源读取权限。
- `operations.list`增加activeCount及默认false的activeOnly；count是当前调用者全部非终态数，与同次列表读取一致，不受分页/展示过滤影响。activeOnly复用现有分页，用于加载全部已有活跃任务，不扫描全部历史寻找活跃项。
- 同步协议规范、Schema、向量、Rust客户端、packages/alcomd-sdk、GuiRpcClient、薄Tauri Channel及窄ACK/取消适配、浏览器harness和相关夹具。不新增数据库结构、配置字段、权限名称、生产依赖或通用消息框架；现有events.list保留回放用途，不建旧轮询兼容路径。
- 兼容性：RPC major仍为1；订阅通过新capability显式协商，既有普通响应信封不变。activeOnly默认false，activeCount为列表响应新增字段；本仓严格Schema和客户端同时更新，不增加旧字段别名、双模型或轮询兼容层。

### Core提交、传输与恢复

- 任务创建、公开状态/阶段及完成与Event同事务提交；只有公开变化才递增revision。补齐包、备份恢复及项目操作的阶段事件/重复静默，不将逐文件/字节或每次内部journal变成公开事件；回滚、重复阶段、no-op和幂等重放无新变化。
- 补齐既有任务恢复状态的revision/事件，重复恢复不制造新变化，Unity迁移继续由其专用恢复入口负责。项目最近活动时间及仓库仅检查时间实际推进分别发布project.activity-recorded/repository.observed；沿用当前资源revision，不改变Plan语义。已接受的Unity打开请求进入活动记录，仓库仅观察时间与任务进度继续排除。
- 唯一Store工作线程在实际写入边界确认持久Events水位，更新保留最新水位的信号并唤醒application订阅者；不依赖调用方接收响应。先注册监听再检查/等待，覆盖恰在检查和等待间提交的受控竞态。不周期查询DB；写线程不等网络、前端ACK或页面查询；释放数据库读取后才等待发送。
- 独立订阅连接，不占普通命令连接。默认100/max1000事件，完整编码帧≤现有4MiB；按编码实际大小分批，游标只推进到已包含事件，单条超限明确终止，不截断。每订阅最多一批待发送，其余在持久Events。
- 分开记录已发送、已接收、前端已接纳游标；仅以安全接纳位置重连。同实例合法游标补发，允许重复；owner序号间隔不代表丢失。首次或WebView丢失状态必须新基线；实例变化/合法游标不可恢复返回resyncRequired=true及新B，同时继续B后的通知。固定资源版本不保存内存历史，故恢复游标resourcesRevision不等于当前版本时也必须重新同步。旧代际/批次/查询不得覆盖新状态。
- 未知帧/非法数据/权限等永久错误停止自动重连；传输失败仅作250ms起至30s封顶退避。连接/就绪5s，发送/ACK10s超时取消旧任务并恢复；健康空闲连接无周期读超时、空批次或业务心跳。
- 原生桥接最多一批未确认；ACK绑定consumer/订阅、代际和batchId，只表示入口已去重并登记dirty，不等查询/渲染/回调。ACK丢失、窗口关闭、重载、超时可取消并重同步，旧ACK/取消不影响新连接。

### GUI失效、合并与视图完整性

- 单一应用级入口，事件失效、查询取权威状态，不在GUI实现Core状态机。每资源一在途查询；读取间再次变脏须补读。revision/读取代际/连接代际防倒退；删除代际防旧响应插回。失败保留dirty、内容和Retry，不周期重试；新变化/显式重试/连接恢复可再读。
- 初始先订阅再读取快照，变化登记后ACK；默认不为所有读取增加事件水位。仅具体用例需要时增加同一读取快照的水位，不先查数据再取最新sequence，不承诺原子全局快照。
- 合并跨批次生效：100ms窗口、同资源普通读取最短250ms、最大合并等待500ms；总并发4，普通最多3，终态/新任务优先且不等普通窗口，队列老化避免饥饿。界限不承诺Core故障/在途等待下的硬实时完成。512资源键超限提升为类别失效，不无限保存ID或事件副本。
- 任务初始及resync读取近期历史首屏并分页加载全部activeOnly，游标彼此独立；按ID/revision合并保留已加载历史。count正确不替代内容完整；创建/状态改变合并更新count，progress不重查count。
- 活动新增补齐与历史分页分别维护位置；operation.progress不产生大量独立活动记录或提醒，不强制阅读历史者回顶部。离页仅缓存项目/任务/活动3视图，总≤5000条或8MiB，超限淘汰数据快照但留筛选/锚点/历史边界；返回完整增量补齐或按锚点/边界重载，不以首屏冒充恢复、不静默留分页断层。
- 终态副作用仅绑定本会话明确跟踪意图，消费一次；历史观察/补发/重连不重建成功提示、导航或写入资格。命令结果与事件幂等合并，但不吞后续真实变化。原有有效设置结果、操作完成回调、日志通道和提醒规则保留。

| 变化源 | 已提交事件/既有通知 | 受影响查询 |
| --- | --- | --- |
| 任务创建/状态/恢复/终态 | operation事件 | 对应任务、合并count及活动；失败补诊断历史 |
| 公开进度 | operation.progress | 正在观察的对应任务，不重查count或全部页面 |
| 项目登记/移除/收藏/内容/发现/最近使用 | project事件 | 项目列表/对应项目 |
| 项目启动参数及编辑器偏好 | project-unity-launch-config/project-editor-preference事件 | 项目Unity及软件包工作区 |
| 仓库/用户包/模板/安装登记 | repository/user-package/template/unity-installation事件 | 对应资源查询 |
| 扩展启停/卸载开始/权限/runtime | Store提交后的固定extensions失效标记 | 扩展列表/详情/权限查询；续租与其他owner不发标记 |
| 备份等操作完成 | 任务终态及权威结果 | 对应类别及已有完成回调 |
| 技术日志/设置保存 | 专用日志通道/已接受结果及回调 | 既有对应展示 |

数据库通知不发现尚未被Core观察的外部文件变化；现有获批发现扫描、仓库获取及内容观察继续，结果接入通知，不顺手重设计。

### 实施与验证

先合同/向量→Core源事件/水位→订阅/桥接ACK取消→共享GUI及同抽象内存夹具→证明链路后删除3秒/750ms轮询及旧追赶逻辑→稳定候选集中验证。夹具先改权威状态再发事件，可控重复/延迟/断线/resync/ACK停滞；不直接改DOM成功，不在测试恢复轮询。

定向覆盖稳态零周期查询；回滚/no-op/重放及调用方断开；检查/等待竞态；订阅/快照和在途变脏；重复/旧响应/删除迟到；游标/实例/WebView丢失；慢消费/ACK/队列/关闭；5秒1000条同任务progress事件触发查询≤22次且不查count、终态不等普通窗口；跨客户端、超一页/旧活跃任务；历史/筛选/阅读位置；权限/owner/资源范围；普通命令不受订阅阻塞；包/备份/项目/扩展完成失败及StrictMode无重复连接/回调。

日常定向检查，稳定候选集中完整Rust门禁和适用前端/浏览器回归，不按提交重复。只在标准main开发，保留全部既有修改/证据；不操作真实Unity/项目、不恢复桌面控制、不推送，不自行签收M7/H7。交付绑定实际候选及真实竞态/背压/查询计数和未覆盖范围。

### Unity 启动只判断目标项目（2026-11-05）

2026-11-06 所有者确认 v3 能看到其他入口启动的 Unity，并能把已就绪的编辑器置于前台。v4 将此能力作为 Windows 本地桌面操作。Core 通过现有项目读取合同授权目标，启动判断和写者保护继续走原用例。桌面平台按项目目录身份、Editor 进程和主窗口确认按钮状态，Windows 目录通知、窗口事件及进程退出触发更新。共享适配一次等待一份最新状态，健康空闲时无周期读取。点击置前时再次通过 Core 核对目标项目及占用，再核对当前 HWND 的进程归属。Windows 拒绝或窗口消失时在操作对话框反馈。其他平台维持已有打开入口。本轮使用已有 `windows-sys` 增开窗口及可访问性 API 特性，无公共 RPC、数据结构或权限变更。目录启动提示有界，覆盖当前最多 20 个登记目标，其余项目通过窗口事件和初始快照更新。

所有者明确：目的是防止重复打开同一项目，其他 Unity 或无法关联项目的进程不得形成全局启动禁令。本轮据此纠正 M5 行为，不增加再次确认或绕过开关。

2026-11-06 所有者报告打开项目仅出现Hub、v3可以打开。只读核对登记为实际2022.3.22f1 Editor；Editor.log记录程序和-projectPath均为`\\?\`路径，切换到Test-Avatar后立即exit0。许可更新错误后仍有entitlements resolved，不能据此认定许可是根因。[官方启动参数](https://docs.unity3d.com/2022.3/Documentation/Manual/EditorCommandLineArguments.html)使用直接Editor和-projectPath；本次候选仅在执行边界复用已有Hub的DOS/UNC适配，重新解析仍为同一路径时才转换，保留数据库/RPC/锁身份，普通启动和迁移共用。先红后绿，平台53/0、严格Clippy/fmt及自有假Editor真实argv两例通过，覆盖中文/空格/引号及shell字符、不同对象和verbatim语义保留。所有者选择自行实测；尚未证明本次Hub现象的现场因果，也未启动真实Unity、修改真实项目或替换Core171196，不混入并行State21候选。补丁/结果位于`target/m7-launch-hub-20261106/`，不宣称原生问题已解决。

Windows 路径在外部调用处统一使用平台适配。Unity 普通启动、版本迁移、Hub 以及系统目录打开和选择器使用普通 DOS 或 UNC 拼写前，须确认该拼写仍解析到同一规范路径。长路径、特殊命名空间和普通规则可能重新解释的名称保留原拼写。GUI 路径标签及 Core 或选择器提供的初始字段值使用共享展示规则，未编辑提交仍带原值。项目身份、文件写入、Plan 和 RPC 继续使用各自的原始路径。[Rust 规范化行为](https://doc.rust-lang.org/std/fs/fn.canonicalize.html) · [Windows 路径命名](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file) · [Windows 路径长度](https://learn.microsoft.com/en-us/windows/win32/fileio/maximum-file-path-limitation)

定向验证覆盖 Windows 平台 54 项、严格 Clippy、假 Editor 的两条真实 Windows argv 链路、Tauri 检查、前端检查和 53 条浏览器用例。浏览器旧刷新入口断言已改为从管理安装触发查找。Cargo 合同测试入口受并行 Core 改动的 `store` 移动后借用错误阻断，对应静态合同用例独立编译执行通过。真实 Unity、系统文件选择器与 SMB 共享留待原生实测。

- 明确匹配其他项目目录身份的进程不污染目标项目状态；同目标项目且已登记 Editor 为 confirmed，同目标但 executable 未核实为 suspected；无法读取/归属的观察保留 unknown。启动只拦目标项目 confirmed/suspected，unknown 不作为全局启动禁令。
- Windows 用有界版本资源和前后文件身份核对排除同名非 Editor（实际复现为 Hub 的 unity.exe serve）；缺失/损坏/变化的身份不伪装为已确认非 Editor。没有新增 unsafe、依赖、持久数据、配置、权限或 RPC 字段，四态 Schema 保持，协议语义原位更新。
- 所有者后续明确批准将项目范围规则用于删除及 Unity 版本迁移：confirmed/suspected 仍拒绝，无法归属的 unknown 不再全局阻断。目标目录身份、内容变化、权限及文件系统边界检查保留；删除在隔离前再次观察占用，若出现目标项目写者则保留目录/登记并清理自有预备目录。迁移已观察到自身子进程退出时继续目标复验；中断且缺少退出证据仍需恢复核实，不伪报成功或重启 Unity。复制、创建备份、派生模板的既有规则保持。
- 代码`4423bd5`最终定向删除4/0、迁移7/0，包含隔离目录真实删除、执行前目标占用、真实迁移引擎的退出证据/目标身份复验；全部使用临时目录、假启动器，不操作真实项目或 Unity。未新增锁文件存在性拦截、RPC字段、依赖、数据库或配置。四态协议兼容2/0、受影响Clippy及fmt通过。本轮只修改源码，未重建或重启当前 dev Core；受影响检查见本轮交付，M7/H7人工状态保持。
- 定向 Application 7/0、Store 启动集成 5/0（含七类进程关联/探测失败场景、假启动器次数）、Platform 9/0、Core adapter 4/0，以及受影响四 crate Clippy、fmt、GUI check 通过。最初同名辅助程序回归先红后绿。未重复完整 Rust/浏览器或远端门禁。
- 构建包含保留的并行产品身份修改，不将定向结果冒称独立提交的完整门禁。修复已加载为 dev Core122652 / GUI144432，原数据/profile 及六组公开状态、配置摘要、revision189 保持；真实只读 RPC 中三个未打开项目由 suspected 变为 not_observed，现有 Unity/Hub 同名工具保持运行。没有实际触发 Unity 启动、项目删除或迁移。证据位于 target/m7-unity-writer-20261105/runtime-20261104T204334Z，M7/H7 人工状态保持。

### 本轮实现与实际验证

- 代码候选`89ec908`，其中Core/协议/SDK/原生桥接为`27f8fef`；文档提交不改变产品代码。无数据库、配置、权限或生产依赖增量；RPC Event Schema补齐既有State18实际允许的八类聚合，不将extensions伪装成持久聚合。
- 已删除项目/任务/活动3秒周期读取、OperationFollow的750ms，以及项目复制300ms/删除250ms追踪循环；改用提交通知、按需权威读取与已接受命令结果。保留业务扫描/获取、日志专用通道、显式重试和必要的事件合并/故障退避/超时，不设稳态轮询兜底。
- 稳定Rust检查：workspace fmt/Clippy、xtask通过；2026-11-04在真实Windows用户下以隔离临时目录运行`cargo test --locked --workspace -- --test-threads=1`，**646/0、8个既有helper ignored**，82组结果、进程exit 0，包含全部既有RPC和新`m7_events_live`。串行测试函数避免每用户端点争用，Tokio用例内部并发保持，不删除或放宽断言。日志见`target/m7-live-ipc-desktop-20261104/attempt-20261103T213813Z/workspace-tests.log`；原分层日志`target/m7-live-stable-*.log`保留。
- 可控证据覆盖检查/等待间提交、调用方已断开、回滚/重复阶段/恢复静默、owner隔离、完整4MiB分批和超限拒绝、实际填满内存传输的慢订阅者不阻塞其他命令；原生ACK绑定、超时与取消由库测试覆盖。浏览器以同GuiRpcClient抽象的权威内存流验证接纳/恢复，不冒充真实Tauri IPC。
- 最终通知/限额/生命周期及H3补验20/0（进程exit 0）；5秒1000条进度含受控迟到读取时查询≤22且进度期间不读count，终态不等普通窗口。证明总并发4/普通3、512键提升、5000条/8MiB淘汰、60秒空闲零业务读取；三份延时列表同时提升优先级先复现7次而非6次，回调身份修复后每键仅一次补读。结果见`target/m7-events-gui-final-reentry.json`，原红轮保留。
- 相关业务合集79/81；两项旧H3夹具时序/浮层选择器修正后已在最终20项内2/2补过，不合称同轮81/0。其余覆盖包/备份/扩展完成失败、跨夹具客户端修改、旧活跃任务/分页、删除迟到，以及缓存淘汰后活动35条历史、筛选与行锚点恢复（Y误差≤1px）。原结果为`target/m7-events-gui-final-related.json`；最终57项打包GUI源码映射与工作文件一致，TypeScript通过。
- 全workspace前端check及既有隔离GUI生产build通过；本轮最终GUI release no-bundle再次包含GUI check及生产build，Core/CLI release配套成功。共享release缓存含旧构建快照的workspace产物，最终改用标准工作目录源码及项目内独立Cargo缓存重编workspace，未改产品API迎合旧缓存；首轮缓存失配及中间前端类型检查失败日志均保留。最终685项构建输入前后一致；与完整Rust回归相比只有保留的四项GUI/浏览器文件改变，Rust/合同输入无变化。构建见`target/m7-live-ipc-desktop-20261104/build-20261103T222038Z`及`test-final-build-input-comparison.json`。上述浏览器结果保留其原候选范围，本轮并行GUI增量只补GUI check/build，不冒称已重跑浏览器。
- 真实命名管道用例证明独立命令/订阅连接、任务创建至终态的有序revision、按已接纳游标重复补发、Core重启重新同步及协议拒绝；未读取的订阅保持时另一订阅及命令可继续。此处不将有限事件量宣称为实际填满Windows管道，也不将同用户参数拒绝宣称为跨Windows用户验证。更新后的真实release另以只读`events.subscribe`取得带ID就绪响应，保持订阅时独立连接`system.status`成功，随后释放订阅。
- 已用配套新版原位打开Core **142980** / GUI **107944**；Core SHA-256 `48C30E64…67652E69`、GUI `3CD5AD48…64CC3D0`，窗口可响应，实际WebView profile路径保持。部署后仍有并行GUI源码继续变化，未计入本次运行产物，见`target/m7-live-ipc-desktop-20261104/post-update-source-comparison.json`。原程序、数据和profile已离线备份；六组完整公开读取与配置文件摘要一致，Config10/State18及revision **189**保持；仅允许并验证本轮批准的capability和activeCount增量。证据见`target/m7-live-ipc-desktop-20261104/update-20261103T222621Z/{desktop-verification,restoration-verification,release-subscription-probe}.json`。未触发真实业务写入、Unity或项目操作，不将窗口响应/IPC冒充原生交互验收。

隔离4175服务已关闭并验证不可连接；本轮仅按所有者授权暂停/更新/重启ALCOMD，Unity及真实项目未操作，桌面控制保持停用。原生交互、新三平台CI/CodeQL与M7/H7人工签收未执行，不借用旧技术基线；已有H2–H6签收和验收材料保持。

## 目标与完成定义

M7 在 M1-M6 已完成的 daemon/RPC/application/Extension Host 基础上交付官方 React/Material Design 3 客户端，并建立
GUI-neutral Portable Extension UI v1：

```text
Extension Component
    -> alcomd-extension-host
    -> alcomd-application authority
    -> ALCOMD RPC v1 Portable UI
        -> official React/MD3 renderer
        -> non-Tauri headless conformance consumer
        -> future third-party GUI renderer
```

官方 GUI 与 extension renderer 都只经 typed client/RPC/application 访问权威状态；不直接读 state.db、项目、repository、
package cache、extension data 或 Host protocol。first-party extension 与 third-party extension 使用相同 package、WIT、
permission、Host、session、RPC 与 renderer contract。

M7 完成需要：

1. 保持已经通过 Hosted CI 的 Portable UI B0-D contract、Core/Host、official renderer、headless conformance 与
   security/fault evidence；
2. official GUI 完整覆盖 M1-M7 已真实发布的 GUI-relevant application/RPC use case；
3. 统一实现 navigation、route、responsive shell、typed adapter、页面状态、Plan review、Apply、Operation follow 与恢复；
4. 实现获批的 Config3、settings/activity/diagnostics、候选读取与 Unity Hub 窄合同，并保持各自权限和版本边界；
5. Chromium browser-level DOM/component accessibility automation，以及 A-033 所要求的 v3 user-model continuity、Material component
   behavior、分阶段截图和一次真实交互桌面的视觉/流程/键盘焦点签收完成；
6. 本地完整 gate 和最终提交自身的 Windows Server 2025、Ubuntu 22.04、macOS 15 arm64 Hosted CI 通过；
7. 项目所有者完成人工验收。

`gui.v3-entry-parity` 不属于 M7 completion blocker：它保持 blocked 并由 M11 在真实脱敏 v3 Fixture 到位后完成。
Narrator、VoiceOver 与 Linux screen-reader smoke 由 M12 在真实目标客户端/辅助技术 runtime 上完成，M7 不把它们标记为通过。

## 已完成前置证据

- M6 final `b666e8fcac6fd4153750c401b76f2f61f6d2a34a` / CI `32724915827` 已通过最终人工验收；M6 backend runtime
  acceptance 保持有效。
- M7 WebView evidence 只证明 sandboxed iframe 与 Tauri managed child 不适合作为 v1 产品方向，保留在
  `specs/gui/m7-stop-a.md`，不属于 security/compatibility success。
- Phase 0 architecture reset commit `12089661f7c694f059aaf172344809ff814716f6` 独立保留。
- obsolete test-only WebView harness cleanup commit `c05c5d2c712dd7a791401efc22b8dd19b88023a6` 独立保留；普通 Tauri
  build/no-bundle gate 未删除。

## Stop A frozen contract

以下合同已获项目所有者批准。Slice B0 已原子替换 active Manifest/package/WIT，加入 State v9 migration、typed
`ui_protocol`、`extensions.ui.use` 与 RPC DTO foundation；Slice B1 已完成 Session runtime 与 capability advertising；
renderer 已在 Slice C 完成；B0/B1 状态没有被用于冒充 renderer evidence。

### Manifest、package 与单一 Portable UI

active Manifest v1 已直接改写为：

```toml
[entrypoints]
component = "component/extension.wasm"

[ui]
protocol = "portable-v1"
```

`background_component` 直接改名为 `component`，不保留 alias；删除 `ui_entry`。package allowed roots 只剩
`alcomd-extension.toml`、`META-INF/`、`component/`，删除 `ui/` 与 UI quota，保留全部 ZIP/signature/digest 安全规则。

v1 每个 extension 最多公开一个隐式 Portable UI；其逻辑身份 `main` 不保存或传输。MCP management 与 Discord Presence
synthetic fixture 都可在一个 page 内使用 section/list/form 表达。Manifest、State、WIT、RPC与Snapshot均没有页面数组、
ID参数、dynamic discovery或兼容槽位。extension record optional addition固定为：

```json
{"ui":{"protocol":"portable-v1"}}
```

### Capability 与 consumer completeness

客户端在 hello 请求 `extensions.ui.portable.v1`；daemon 完整实现后才回显。声明支持的 consumer 必须实现全部 v1
node/action。没有 hostId、official GUI identity、framework/renderer name、feature subset/intersection 或 GUI-specific tree。
不支持的 GUI 继续提供 extension management，并把功能页标为 unsupported。unknown v1 node fail closed；新增 node 使用
新的 capability/version。

### Active WIT/ABI v1

active WIT 位于 `specs/extensions/wit/extension-v1/`，production bindgen 与真实 Extension Host 使用
`alcomd:extension/extension-v1@1.0.0`，import host-projects/host-data，required export guest-lifecycle/guest-ui。所有 ABI v1
Component 都必须实现 guest-ui；Manifest 无 `[ui]` 时 daemon 不调用，reference fixture 提供 empty stub，Host instantiate
仍验证完整 world。guest-ui exact sync functions：

```text
open(guest-session-id, locale) -> UiDocument
refresh(guest-session-id) -> UiDocument
dispatch(guest-session-id, UiAction) -> UiDocument
close(guest-session-id)
```

Host embedding 继续 async；没有启用 component-model-async、WASI future/stream、wasmtime-wasi、guest thread 或新依赖。
旧 proposal directory 已删除；没有保留 parallel world，没有创建 ABI v2，`extensionApi.major` 仍为 1。

### Lifecycle 与 Host protocol

activation kind 固定 `background|interactive-ui`；stop reason增加 `interactive-ui-idle`，并保留 disabled、permission-revoked、
lease-expired、daemon-shutdown、uninstalling。`extensions.ui.open`不隐式enable；disabled/quarantined/no-ui分别返回
`extension_not_enabled`/`extension_quarantined`/`extension_ui_not_available`。检查client、package/digest、
grant/lifecycle/quarantine后创建interactive-ui lease，按需启动并仅在本次Host activation尚未发生时activate。
多个session共用activation，background-running Host不重复activate。最后session关闭且无background lease时调用
`deactivate(interactive-ui-idle)`并在5,000 ms内停Host；关闭route不修改desired state。
`background.run`不自动加入Manifest required permissions。扩展若主动列为required且未授权，enable失败；列为optional且
未授权时UI仍可用，但最后一个interactive session关闭后Host停止。Portable UI不产生隐式background lease。

daemon 为每个 invoke-export签发 `InvocationContextId`，绑定 invocation kind、Extension lease/grant/generation、deadline/
cancel，以及 interactive-ui 的 Client Principal/connection/UiSession/snapshot。capability-call必须回显。grant/lease/
session/deadline/generation正常竞态返回stable stale/permission/cancel并取消invocation，不计Host crash。unknown/cross
extension/session/invocation、completed reuse或authority修改/伪造才是Host protocol violation并终止Host。export返回后
context立即失效；requestId/callId仍只做correlation。

### Document、tree、form 与 action

Guest 只返回完整 UiDocument；daemon验证并包装从 1 单调递增的 UiSnapshot revision。flat tree必须恰好一个 page root、
parent-before-child、无 cycle/unreachable node、连续唯一 sibling order、exact parent/child matrix、unique
node/field/action/option ID、depth 8。ID grammar固定 `^[a-z][a-z0-9._-]{0,63}$`，form nodeId即formId。固定 node：

- layout：page、section、stack、group、form、list、list-item；
- display：text、status、key-value、progress、divider；
- input：button、switch、text-field、integer-field、select。

tone 只有 neutral/info/success/warning/danger；progress 为 indeterminate 或 0-10000 basis points；integer 为 JSON-safe
signed integer。禁止 HTML/CSS/JS/Markdown HTML/canvas/image/icon URL/link/URI/font/color/animation/absolute/custom ARIA/
DOM/component/expression/iframe/WebView/navigation。

switch/select/text/integer拥有且只拥有一个form ancestor，禁止nested form；v1 action只有 `activate` 与 `submit-form`。
submit按node order只携带完整editable field set；disabled/read-only值来自Snapshot，client提交它们即invalid。daemon验证
form/action归属、type/required/length/range/option/completeness/size。field可带valid或invalid+最多512 UTF-8 bytes纯文本；
renderer设置invalid与host-generated `aria-describedby`，该文本不是security confirmation。

### Exact limits

机器权威值在 `specs/extensions/portable-ui-limits-v1.json`：session per extension/client/daemon 为 8/16/128；
Snapshot/action 262,144/65,536 bytes；nodes 256；depth 8；ID 1-64 lowercase ASCII；single/total text 4,096/65,536
UTF-8 bytes；validation 512 bytes；form 64 fields；select 64 options、label 256 bytes；Host/session concurrency 1；token bucket 60/minute、
burst 10；idle/absolute 300,000/3,600,000 ms；request cache 64；Host idle stop 5,000 ms。

plain text拒绝 NUL、C0/C1/DEL与冻结 bidi controls。text node允许 LF；text-field只有 multiline 时允许 LF；其他字段
单行，CR/TAB始终拒绝。locale只接受 2-15 ASCII bytes 的 canonical language[-Script][-Region] core BCP47 子集。

### Session、replay 与 failure

memory-only session绑定 Client Principal/connection、Extension Principal/instance/package、grant/lifecycle/
snapshot revisions、locale/deadlines、next sequence和64-entry replay cache。disconnect/restart/disable/revoke/uninstall/
package or generation change/Host crash/quarantine/timeout/protocol violation立即关闭。session不写 DB/Event/data/package/log，
不 crash-recover，不创建 Operation。

dispatch每session内存保存最近64项sequence/requestId/expected revision/canonical action fingerprint/resulting Snapshot。
新连续请求调用guest一次；exact replay不调用guest并返回原resulting Snapshot、`replayed=true`；相同ID/sequence但
revision/fingerprint冲突或gap/out-of-order统一返回 `extension_ui_action_invalid`。Client错误不计Host crash；Guest invalid
document关闭session、终止Host并计入既有crash/quarantine。Portable UI无server push；draft只在GUI memory并绑定
session/revision/form，dirty时禁止自动refresh/merge，主动刷新/导航需host-owned discard confirmation。

### Authorization 与 RPC

client candidate permission 是 `extensions.ui.use` scoped exact ExtensionId；没有 extension `ui.contribute`。list/get UI
declaration用 extensions.read；open/refresh/dispatch用 extensions.read + extensions.ui.use + exact scope；close返回
`{closed:boolean}`并 best effort，不泄露 owner。

UI invocation 内 Host business capability取 Client Principal与 Extension Principal同一 resource scope交集；host-data
仍由 extension self namespace authority控制，client只需 UI permission。high-impact Plan/credential/Operation继续 host-owned。

RPC compatible addition只有 capability 与 open/refresh/dispatch/close。DTO closed/bounded，open只接收extensionId/locale，
不接收页面ID、host/framework/platform。固定route为 `/extensions/:extensionId/ui`。stable error分类由
`m7-portable-ui.schema.json`冻结，删除 `extension_ui_surface_not_found`，不创建宽泛
`extension_ui_failed`，也不公开 Host PID/pipe/lease/path/identity/context。

### Portable UI 存储与 renderer

M6 的 v8 后，Portable UI 在 v9 为 extensions 和 immutable extension_plans 各增 nullable `ui_protocol`，合法值 NULL 或
`portable-v1`。不保存页面常量，不建session/Snapshot/replay/action/draft/renderer/browser/cache/workflow table。v8->v9
单事务 migration 已保留 rows/FK/revision/Event sequence，existing 默认 NULL，future schema fail closed；B1 完整接线后
当前 hello 广告实际 State v16 与 `extensions.ui.portable.v1`；v9 是该字段的历史引入版本。

官方 React/MD3 renderer与non-Tauri headless consumer使用同一 public DTO/fixtures。官方 contract冻结 host-owned
name/ExtensionId/publisher-trust/version/desired/runtime/quarantine/extension-provided chrome、exhaustive match、
keyboard/focus/ARIA、200%/320px、reduced motion、无 extension CSS/DOM/Tauri。headless consumer不依赖GUI/Tauri并输出
确定性 semantic summary。B0/B1 不实现任何 renderer；Slice C 已实现 official renderer，Slice D 已实现独立 headless
consumer 并用相同 public DTO/Fixture 验证全部 node/action 与 fail-closed 行为。

## Contract artifacts

- ADR refinement：`docs/adr/0024-portable-extension-ui.md`；
- Portable UI contract/schema/limits：`specs/extensions/portable-ui-v1.*`、`portable-ui-limits-v1.*`；
- active Manifest/package/ABI/permission/Host protocol：`specs/extensions/`、
  `specs/extensions/host-protocol-invocation-context-v1.md`；
- active WIT：`specs/extensions/wit/extension-v1/`；
- RPC contract：`specs/rpc/m7-portable-ui.schema.json`、`specs/rpc/alcomd-rpc-v1.md`；
- State v9/migration：`specs/storage/state-v9*`、`crates/alcomd-store/migrations/0009_portable_extension_ui.sql`；
- threat model/consumer contract：`specs/security/extension-portable-ui-threat-model.md`、
  `specs/gui/portable-ui-renderer-v1.md`；
- MCP/Discord/adversarial/headless fixtures：`crates/alcomd-testing/fixtures/m7/`；
- contract tests：`crates/alcomd-testing/tests/m7_contract.rs`。

## 已批准 production slices

1. Slice B0（已完成）：原子替换 active Manifest/package/WIT/ABI/permission/State contract。
2. Slice B1（已完成）：接入 Host/Core/RPC memory-only session、InvocationContext、interactive lifecycle、render purity、
   current-only replay、validation、capability advertising 与 invalidation。
3. Slice C（已完成）：官方 shell/typed adapter/React/MD3 renderer、forms、host-owned chrome 与
   loading/error/disconnected/reconnect，仍只调用 public client/RPC。
4. Slice D（本地实现与定向验证已完成）：non-Tauri consumer、first/third parity、malformed/revoke/crash/reconnect、
   InvocationContext binding、dual-authority route、payload redaction 与 limits；最终三平台/a11y验收仍待 push 后执行。

## Restored Official GUI Completion

Portable UI architecture reset 只替换旧 Web UI/iframe/isolated WebView 方向，不删除 official GUI 对当前 Core use case 的覆盖。
completion audit 后恢复以下批准切片：

1. Slice E1：Core read surfaces、stable route identity、wide rail/drawer、narrow modal drawer 与统一 data-route state；
2. Slice E2：当前既有 mutation、typed Plan review、explicit Apply、Operation progress/cancel/reconnect；
3. Slice F1：当前 Config3、`settings.toml` 单 writer durability、ConfigStore port、settings permission/RPC；
4. Slice F2：Event/Operation 的 safe Activity projection 与 redacted Diagnostics projection；
5. Slice F3：Settings、Activity、Diagnostics 与 About GUI；
6. Slice G1：Chromium browser-level DOM/component accessibility automation；
7. Slice G2：一次真实交互桌面的 manual screenshot/flow/keyboard-focus evidence preparation；
8. Slice G3：metadata/completion audit、完整本地 gate 与最终本地候选。

### Information architecture

Official GUI 的宏观 information architecture 受 A-033 约束。冻结参考与映射分别位于：

- `docs/gui/alcomd3-v3-layout-baseline.md`；
- `docs/gui/m7-layout-mapping.md`；
- `docs/gui/m7-current-layout-gap.md`；
- `docs/gui/m7-material-component-audit.md`；
- `docs/gui/m7-user-model-continuity.md`；
- `docs/gui/m7-information-architecture-candidates.md`（historical exploration only）。

Official GUI 默认继承 v3 用户模型：Projects、package-centric Project workflow、Packages & Templates、Extensions、Settings、
Log 与 footer/progress utilities。v4 的首要目标是在同一用户结构中提供 MD3 现代化和更可靠、安全、可观察的实现，而不是为
daemon、RPC、Operation、Extension Host、Portable UI、permission/scope 或 State Schema 建立新产品领域。

只有 v3 无对应用户用例、用户需要独立工作区、无法自然容纳于既有概念且独立入口明确提升可用性时，才考虑新增 user-level
surface。Candidate A/B/C 因仍从“如何给 v4 domain 找位置”出发，现只保留为 historical exploration。

以下是当前仍有效的 implementation route/capability inventory，不是 proposed sidebar hierarchy；route 可以保留作 deep link、
secondary surface 或 context workspace：

```text
Home
Projects
  -> Project Detail
       -> Packages
       -> Unity
       -> Backups
Repositories
Templates
Operations
Extensions
  -> Extension Detail
       -> Permissions
       -> Portable UI
Settings
Activity
Diagnostics
About
```

route identity 使用稳定 ASCII ID 和 opaque resource ID，不使用翻译文案或完整路径作为 key。不存在 Core/RPC capability 的
业务不显示假按钮或 disabled fake page。M8 MCP、M9 Discord、migration/import、updater/distribution、Local API、Custom
Web UI 与 full external-client credential pairing/revocation 不进入 M7。

当前 minimum-change proposal 将 Projects 恢复为 home-equivalent，将 Repositories/Templates移回Packages & Templates，将
project Unity/Backups保留在Project context，将global Unity放入Settings，将Operations表达为context progress与secondary Task
Center，将Activity/Diagnostics放入同一Log utility，并把About/version放入footer/Settings。Extensions是v3既有用户概念，保持
其utility身份和大致归属；permission/quarantine/Portable UI进入Extension detail。Portable UI v1不提供sidebar/menu/toolbar/
arbitrary navigation contribution。该proposal已获项目所有者批准。

### Visual architecture and component policy

`19267230507071dc61ba306b98c8cfdd113e9ea2` 的功能接线保持有效，但状态固定为：

```text
functional_implementation_candidate
but
rejected_for_visual_and_information_architecture_acceptance
```

源码审计确认 `@material/web 2.5.0` production import 为 0、rendered `md-*` element 为 0；Core 与 Portable UI 使用 native
button/input/select/textarea/progress 和 custom CSS。该事实分类为
`material_web_dependency_present_but_component_system_not_adopted`，主题分类为
`material_theme_not_actually_wired`。

Material Web 成为 interactive component foundation。只要 2.5.0 提供对应组件，Core 与 Portable UI 都必须经
`@alcomd/ui` 的窄 typed wrapper 使用真实 Material behavior；不得自行模拟 ripple/state layer。应用 shell、data table、page
grid、split pane 等 Material Web 未提供的结构，使用 semantic HTML + 最小 `@alcomd/ui` layout primitive + 同一 MD3 tokens。
`apps/alcomd-gui` 原则上不散布 direct Material imports。

React 19 custom-element properties/events/boolean/form/ref/typing/validation 必须在 H0 用真实 2.5.0 component test 冻结；优先使用
React 19 原生支持，不增加第三方 adapter。新增 dependency 仍是人工停止点。

### RPC coverage authority

`docs/exec-plans/M7-gui-rpc-coverage.md` 是 M1-M7 public RPC 到 official GUI 的枚举矩阵。每个 GUI-relevant method 必须有
page、user action、read/write、confirmation、Operation、state handling 与 test owner；transport/internal method 显式标记
`not_applicable`。矩阵由 protocol constants 与 official client 自动枚举结果核对，不凭记忆删减。

### Typed GUI adapter

所有业务保持 `React -> typed Tauri adapter -> alcomd-client -> RPC -> alcomd-application`。允许使用 app-private closed
`gui_query`/`gui_command` discriminated variants 收敛 adapter，但 Rust/TypeScript 必须 exhaustive match，unknown variant
fail closed，且不能成为 arbitrary method/JSON passthrough。GUI 不直接连接 daemon socket、SQLite、project filesystem、
repository HTTP、package cache 或 Extension Host。

### Common states and mutation semantics

每个 data route 实现 `initial-loading`、保留 last-known-good 的 `refreshing`、`empty`、`error` 与 `disconnected`；mutation
route 另实现 `confirmation-required`、`operation-running`、`success`、`failed`、`cancelled`。permission denial 不是 empty；
`internal_error` 只展示 stable code 与 `diagnosticId`。

高影响写入保持 `Plan -> review daemon ChangeSet/safe risk summary -> explicit Apply -> OperationId -> progress -> terminal
result`。stale/revision conflict 回到明确 review，不静默重新 Plan；route/window close 不等于取消 Operation。

### Settings Config9 与存储权威

当前活动格式为 `specs/config/settings-v8.md` / `.schema.json`。closed 字段为：
appearance mode/sourceColor/scheme/motion、locale、packages 三项偏好、unity.hubPath/defaultLaunchArguments、paths 两目录覆盖、window.rememberState、projects.autoDiscover 及 backups 三项默认；
精确枚举、边界和 null 语义以现有机器合同为准。所有非 nullable PATCH 字段显式 null 整次拒绝，
sourceColor:null、hubPath:null 和两个目录的 null 分别清除显式覆盖；有效目录由 Core 只读解析后投影。

settings.get/update 要求原 settings.read/manage，返回 Config9、revision、完整归一化设置及
派生 unityHub/有效目录；revision checked monotonic u64，文件 strict UTF-8/unknown-field fail closed，
最多 16 KiB，确定性序列化、可恢复原子替换。派生文件状态不推进 revision，保存后状态读取
失败不把已提交设置伪报为失败。application 只经 ConfigStore port 保存归一化快照，Store
负责文件格式和恢复，不增加配置数据库表、通用 key/value、GUI 直写或第二套权威。

主题确认、普通即时设置和 Hub 配置共用 UI 串行 CAS；状态读取、能力协商和故障恢复仍通过
typed client/RPC。当前 hello.configSchema=9。Config1–8 只作为保留原数据的迁移输入，不是当前写入或
返回格式；旧 density 校验后丢弃，保留原 revision 与其他设置，Config9 不接受该字段。自动发现字段及生效时的项目目录修改附加现有 projects.manage；Unity全局参数修改附加unity.manage。
开发兼容策略按 A-034，发布前另行审计这些历史输入的保留需求。

### 日志、活动记录与提示已撤销

按当前有效合同彻底移除日志、独立活动及浮动提示。Operation与业务Event保留原用途；页面内错误、确认、重试及完成行为继续验证。下方旧日志重建方案及技术记录只保留为撤销前历史，不授予恢复实现的范围。

### Accessibility and manual ownership

M7 automation 使用唯一获批 devDependency `@playwright/test = 1.62.1` 与其匹配 Chromium revision，覆盖 semantic HTML、
landmark/heading/name/label/validation/live region、drawer/dialog focus trap/restore、route focus、keyboard-only flow、Tab/arrow/Escape、
visible focus、reduced motion、light/dark contrast、200% deterministic layout evidence、320 CSS px、error/disabled/progress 与全部
17 Portable UI node 的真实 DOM semantics。自动化不复活 iframe/child WebView probe，也不宣称真实平台辅助技术认证。

M7 manual owner 是一次真实交互桌面的 visual screenshot、flow、keyboard/focus smoke 与项目所有者签收。M11 owner 是真实 v3
entry/flow/migration-state/screenshot differential parity。M12 owner 是 Windows Narrator、macOS VoiceOver、Linux screen reader
及真实平台 client accessibility runtime validation。

visual work 不再等到全部页面完成后一次性查看。H1-H5 期间设置四个阻塞 gate：

1. Visual Gate 1：main shell/navigation；
2. Visual Gate 2：Projects/packages/repositories；
3. Visual Gate 3：remaining core pages；
4. Visual Gate 4：Portable UI/settings/diagnostics。

每个 gate 必须启动真实 GUI、采集仓库外 bounded screenshot 并取得项目所有者签收后才能进入下一大块。最终 H7 还要检查
Material hover/pressed/focus/ripple/disabled behavior，不以自动测试或截图替代真实交互。

不得同时保留旧 Web UI和Portable UI parser/world/renderer，不建立通用 UI/workflow engine。

## 已批准 production 修改范围

```text
apps/alcomd/
apps/alcomd-extension-host/
apps/alcomd-gui/
crates/alcomd-application/
crates/alcomd-domain/
crates/alcomd-platform/
crates/alcomd-vpm/
crates/alcomd-client/
crates/alcomd-extensions/
crates/alcomd-protocol/
crates/alcomd-store/
crates/alcomd-testing/
packages/alcomd-sdk/
packages/alcomd-ui/
.github/workflows/ci.yml
docs/adr/ docs/exec-plans/ docs/status.md docs/testing/
feature-parity.toml
specs/config/ specs/extensions/ specs/gui/ specs/rpc/ specs/security/ specs/storage/
```

当前获批依赖/资产范围：

- 唯一前端浏览器测试依赖 `@playwright/test = 1.62.1`，不进入 production 图。
- `@alcomd/ui` 直接 production 依赖为原有 `@material/web = 2.5.0`、配色引擎
  `@material/material-color-utilities = 0.4.0`（Apache-2.0）和
  `overlayscrollbars = 2.17.0`（MIT）；两项新增库均无额外 runtime dependency/native addon。
- Material Symbols 为固定 Google upstream commit 的 only-used vendored SVG；新增 palette
  只使用同一 upstream/样式并登记 path/hash，不存在 Material Symbols npm 包或第二图标库。
- npm lock 的已批准开发安全补丁为 Vite 7.3.7 和 source-map-js 1.2.2，不加新库/override。
  许可和维护风险见 THIRD_PARTY_NOTICES.md、当前合同及依赖评估记录。

上述限定支持相应 `packages/alcomd-ui/package.json`、GUI check script 与根 package-lock
改动；不构成后续依赖升级许可。候选读取允许窄 VPM primitive、domain/Store read snapshot
及 typed DTO 接线；已批准的 Hub 发现/启动在现有 platform adapter 内使用既有 std API。
不新增 production crate、Cargo 依赖、unsafe 文件或未批准 native API；不扩大 Tauri
capability、不启用 unstable、不创建 iframe/child WebView/WebviewWindow 或 M8/M9 接线。

## 明确排除

- M8 MCP backend/management product logic、M9 Discord IPC/Presence product logic；fixtures只证明UI表达能力；
- custom Web UI、iframe/child WebView/WebviewWindow、asset/origin/CSP/Tauri capability；
- Local API、migration/import/bootstrap/updater/installer/signing/dist、update channel 与 full external-client credential pairing；
- WASI 0.3、component-model-async、wasmtime-wasi、guest thread，以及下述清单外的新 dependency；
- M8/M9 production wiring，以及 Portable UI 合同外的新 public RPC/Permission/error/node/action/surface。

## 验证与 release blockers

验证按风险和开发阶段安排，不以小修改或独立提交为完整门禁的触发点：

| 时机 | 验证安排 |
| --- | --- |
| 日常修改 | 受影响模块的定向测试及必要的 fmt/Clippy、前端 check；仅文档修改执行 metadata、引用与 diff check |
| 功能模块完成 | 集中覆盖该模块及相关回归路径；H2-B 按模板分页/创建、Plan review/终态刷新、资源生命周期分组，涉及共享组件时覆盖 H2-A 对应的焦点、权限、重连和包管理路径 |
| 稳定候选与阶段收尾 | 合并相关修改后集中运行适用完整门禁；跨平台、安全、依赖和重要架构变更覆盖相应平台与安全回归，避免反复编译同一批依赖 |

高风险问题或回归失败及时执行必要的额外测试，不机械等待模块或阶段结束。最终候选基于其实际
SHA 运行 format/Clippy/Workspace tests、xtask、metadata、baseline freeze、npm check/build、
完整 Playwright suite、Tauri release no-bundle 及 diff check；不逐项或逐提交重复完整测试与构建。
三平台 Hosted CI 和 CodeQL 集中用于获准的重要技术检查点、阶段收尾及最终验收候选；
在已有授权范围内批量提交、集中推送，不新增推送授权，不仅因本次流程调整取消运行中的 CI。
跨平台与依赖安全技术检查点已在 `97c761f` 取得同一 SHA 三平台完整 required jobs 和 CodeQL 成功。
H2-B `0ef5b08` 已按项目所有者本轮明确授权推送，当前三平台 CI/CodeQL 成功；停止在 Resources
人工验收。未经新的推送授权不推送额外提交，不因状态文档整理重跑既有完整验证。

本地完整验证优先复用 `scripts/check.ps1` / `scripts/check.sh`，其已包含 fmt、Clippy、
Workspace tests、xtask、前端 check/build 与 metadata；不在其后逐项重跑，也不叠加另一个
包含相同完整检查的聚合入口。CLI auto-start、冻结基线、浏览器及 release 构建等未覆盖部分
按现有独立入口补齐，保留各自实际结果与锁文件校验。

依赖变化仅允许上一节精确清单，三份 lockfile 不得产生无法解释的增量；production Vite 图
不含 Playwright 或完整 icon corpus，Cargo/unsafe/native API 边界保持。各轮失败、定向重试和
完整 suite 分别记录，不拼接不同提交的通过结果，不将未执行标记为 PASS；保留全部测试、
断言和安全门禁。运行中的真实环境未经授权不为测试而停止。
Schema 的机器形状检查与 daemon 实际版本一致性、页面完整来源证据、离线恢复和公共列表
混排行盒须有执行回归，不能以静态门禁通过替代。

后续 first-party private node/page/command/permission、partial v1 renderer、GUI-to-Host direct channel、renderer作为business
authority均是M7 blocker。
native interactive control 在 Material Web 已有对应组件时继续以 custom CSS 模拟、Core/Portable UI 使用不同 component
foundation、违反已批准 minimum-change user model/A-033 continuity 或包含 `UNJUSTIFIED_DEVIATION`、跳过任一 Visual Gate，
同样是 M7 blocker。
M11真实v3 fixture缺失继续阻塞GUI differential parity，但不阻塞 `gui.m7-core-surfaces`。M12继续承担Win10/Win11安装/启动/
WebView2/update/uninstall，以及 Narrator/VoiceOver/Linux screen-reader 和真实平台 accessibility runtime validation。

## Visual realignment slices

2026-09-19 owner-authorized supplement: before Visual Gate 2 acceptance, correct basic layout and usability
of the other existing GUI pages, referencing frozen v3 design to the same degree as Projects. This explicitly
allows resource tables/segmented navigation, grouped Settings with its Unity entry, Logs Activity/Diagnostics,
existing task/extension/about/backup/detail presentation and normal return paths. Reuse current Material
components and RPC contracts; do not copy v3 source. Preserve Projects/package workspace and confirmed icons,
avoid narrow-window adaptation, new dependencies, contracts, migrations and production platform APIs. This is
not blanket approval of the remaining H2/H3-H7 roadmap or visual signoff. Browser fixtures may establish frontend
evidence; Computer Use interruption or missing Tauri evidence must remain separate. Continue straightforward
fixes and defer uncertain contract-dependent work without blocking unrelated corrections.

1. **H0 Material foundation（已批准）**：`@alcomd/ui` 封装审计中真实需要的 Material Web components；接通真实 MD3 color/type/shape/
   elevation/state tokens；验证 React 19 integration、interaction/ripple、component accessibility 和Core/Portable共用层。
2. **H1 v3-reference Projects shell/navigation（Visual Gate 1 已通过）**：首次实现因 global top app bar、admin-style sidebar grouping、
   centered max-width page、hero typography、giant empty/form cards 与过低桌面密度被标记为
   `rejected_for_visual_direction`。重新执行必须先完成 `docs/gui/alcomd3-v3-layout-baseline.md` 的直接视觉拆解，再只实现
   wide desktop 的 recognizable sidebar/content canvas、Projects page toolbar、sort/filter 与 dense content。narrow adaptive
   不能反向决定本 gate 的宽屏结构；完成 first runnable prototype 后立即停止 Visual Gate 1。
3. **H2 Projects/Packages & Templates（H2-A 与上述既有页面修正已验收）**：完成 package-centric workspace、
   候选读取/行内版本/批量、统一公共组件及已批准的资源/模板/设置/日志/扩展/备份基础交互。
   只实现本文件列明的窄公开合同。项目所有者于 2026-10-31 签收 Visual Gate 2；H2-B 已获下述技术收尾通过后的条件批准，其他产品范围仍未批准。
4. **H3 contextual v4 enhancements（阶段性人工验收通过，保留非阻塞体验问题）**：把 global/project Unity、Backups 与 Operations 增强嵌入既有 Settings/Project/progress workflow，不改
   Plan/Apply/Operation；完成 Visual Gate 3 的第一部分。项目所有者于 2026-11-02 明确签收本轮，并确认 Unity 自动检测、同记录刷新和路径显示；不表示体验完善或整个 Visual Gate 3/M7 完成。项目上下文、共享提醒和任务机制保持，不重复验收已检查页面。
5. **H4 Extensions/Portable UI（阶段性人工验收通过，保留非阻塞体验问题）**：保留Extensions既有用户概念和utility归属；permission/quarantine/Portable UI位于detail，host
   chrome与renderer全部使用共同Material components；不改 Portable UI protocol。所有者于 2026-11-02 认可安装/启停/卸载、权限管理、同窗口功能页与共享错误可读性；保留表单、超时、外部断线和 M8/M9 正式业务未覆盖项，不表示整个 Visual Gate 4/M7 完成。
6. **H5 utilities/observability（阶段性人工验收通过，保留非阻塞体验问题）**：保留 grouped Settings、Log内Activity/Diagnostics与footer About/version，
   只补真实缺失功能及必要整改，保留 A-026 权限分离，复用共享 Material、错误显示与只读 `displayPath`；当前 Config4 窄变更见下方批准记录。
   所有者于 2026-11-02 明确签收，认可删除紧凑模式；保留一般文案、布局与过度设计待办，不表示体验完善或 M7 完成。
7. **H6 responsive/a11y/Playwright regression（阶段性人工验收通过，保留非阻塞体验问题）**：覆盖已批准 minimum-change user model 的 structural continuity、grouping/workspace/
   action placement、Material element presence和observable interaction、theme propagation、wide/narrow/200%/reduced motion；
   不测试Material shadow DOM私有细节。所有者于 2026-11-02 确认窄窗、键盘、主题与主要辅助功能符合预期；默认间距保持，不恢复 density，不建立第二套组件。
8. **H7 manual visual signoff（同 SHA 技术门禁已通过，等待人工验收及剩余事项处理）**：整理精简最终清单，核对历史失败的后续修复并集中稳定候选完整本地验证；只修实际功能、操作、重要布局与回归，不重验已签收页面。不以推送或技术结果代替人工签收。

H0-H7 只重组 shell、composition、component rendering 和 visual hierarchy；不得重新设计 RPC、Plan/Apply、Operation、Settings、
Activity、Diagnostics 或 Portable UI authority。

### H5 已批准切片（2026-11-02）

H4 阶段签收及实际环境恢复后直接推进 H5，不另设规划停止点。先核对 Settings/Config3、Activity/Diagnostics、
About/version 已有接线、反馈、分页与权限，只补真实缺口；正常状态简洁，错误/风险准确保留，短暂反馈不撑开布局。
只读路径统一 `displayPath`，不修改实际路径或 RPC/文件参数；公共合同确有缺口才提交最小方案并只暂停相关部分。
日常定向检查，稳定候选集中必要验证与真实界面入口；不重复全量 Rust/Chromium/release/三平台 CI，不以旧结果替代新代码。
已有 H2-B/H3/H4 非阻塞待办保留，不批量升为 H5 阻塞。不进入 H6/H7，不推送，不宣布 M7 完成。

复用审计已确认全部 Config3 字段、普通设置串行 CAS/未知结果核对、主题预览/三方合并、日志分段/搜索/已加载项筛选/
keyset 分页及关于/许可证入口已有实现。只修设置丢失安全错误原因、隐藏仓库 256 项界限、日志任务详情不刷新、
缺少 operations capability 时的操作接线、诊断原因与现有摘要翻译，以及 GUI/Core 版本区分和断线时本地信息可见性。
复用现有 OperationFollow 与 Reminder；GUI 构建版本取已由 metadata 检查对齐产品/Tauri 的 package 元数据。
不新增公共 RPC、State Schema、权限、依赖或通知/保存/任务机制。真实窗口只经已有自身进程启动方式准备，桌面控制保持停用。

移除紧凑模式前的 H5 候选 `c768289` 已完成 GUI check（1030 条消息）与一次 release no-bundle 构建；该轮 Core 为 `e2b7ddd`。
关联浏览器首轮 101/2、定位修复后的三个模块 56/0 分别保留，184 项输入逐轮未变，不拼成全套通过。
原 Core 持续运行，新 GUI 已实际打开；构建前后五组公开状态一致，期间更早的 Unity 记录变化单独保留。
交付 Settings、Log、About 三项真实窗口清单并等待 H5 人工确认；未执行完整 Rust/浏览器套件或远端验证，不进入 H6/H7、不推送。
证据与未覆盖项见 [H5 候选记录](../testing/m7-h5-utilities-20261102.md)。

#### H5 已批准差异：移除紧凑模式与 Config4（2026-11-02）

项目所有者已批准完整删除 GUI 紧凑模式及公开 `settings.get/update` 的 `appearance.density`，
当前设置合同升级为 Config Schema 4；不保留隐藏开关、旧字段别名或两套当前模型。Config4 文件、完整响应及
partial update 均不含 density，显式提供该字段作为未知字段拒绝；现有 settings.read/manage、串行 CAS、
16 KiB 文件上限、原子发布及恢复纪律保持。无需新 RPC、capability、权限、State Schema 或生产依赖。

Config1–3 仅作为既有启动迁移输入：按各自历史合同校验后丢弃旧 density，保留其余有效设置和原 revision；
v1/v2 仍仅补各自原先缺少的 packages/scheme 默认值。迁移在 ready 前完成，不算用户更新，不推进 revision；
Config4 当前写入与公开响应只使用新的唯一模型。当前规范及 Schema 原位转向 settings-v4，旧版本明确标为迁移历史。
验证只覆盖字段拒绝、旧格式迁移/其余设置及 revision 保留、GUI 紧凑入口与样式移除及相关回归；不重复无关全量或远端检查。

当前候选为 `6739705`：Store 19/0、application 17/0、协议合同 7/0、hello/映射/真实隔离 RPC 各 1/0，
GUI/UI check、保存队列 15 场景、浏览器 76/0 及补充 1/0 已通过；Clippy 初轮测试初始化失败与窄修复验分别保留。
一次 Core/GUI 必要构建后已实际恢复原环境，Config4 迁移只删除 density，revision 166 和其他偏好不变；新窗口保持打开。
以上为 `6739705` 交付时的历史状态；所有者随后已阶段性签收 H5 并批准 H6，当前授权见下节。
旧轮测试不冒充本轮全套，新增代码无远端验证；H7 与推送仍未授权。

## 技术收尾与 H2-B 顺序授权（2026-10-31）

### 当前 H6 授权（2026-11-02）

H5 已阶段性人工签收，紧凑模式移除与 Config4 方向获认可；H2–H4 原签收保持，不重验已检查流程。
直接推进既定 H6：先核对已有实现/测试，只修桌面宽度变化、较窄窗口、200% 可用性、键盘焦点与弹窗、
明暗主题和减少动画的实际缺口，复用共享 Material/ScrollArea/Reminder，不重做 GUI 或新增适配体系。
旧测试若只因可读错误/已删除密度/既有名称变化失配，应保留安全与交互断言并改为当前可观察合同，不能删断言掩盖产品缺陷。
定向诊断与修复后集中必要浏览器回归，准确区分等效缩放与真实平台/辅助技术结果；未覆盖项留待明确验收。
只读核对 Config4 现状复用迁移证据，不重跑 Rust/迁移/三平台 CI。H6 候选交付精简结果与人工入口，不进入 H7、不推送、不宣布 M7 完成。

本地安装的 Tauri 2.12.5 配置/类型确认缩放快捷键默认关闭。H6 复用其 `zoomHotkeysEnabled`，
仅为本地主窗口增加 Tauri 已有的 `core:webview:allow-set-webview-zoom` UI 权限以支持 macOS/Linux 的既有缩放适配；
Windows 使用 WebView2 原生缩放。不新增 ALCOMD RPC/权限或产品设置。自动化以 720×450 CSS + DPR2 核对
1440×900 物理窗口的 200% 等效布局，不把改根字号或浏览器结果冒充真实平台快捷键/读屏验收。

#### H6 本地候选与实际交付（2026-11-02）

代码候选 `a4f12a19d5fca0692053bb281fefab472fd60b65` 修正窄窗工具/表单/扩展卡片、Portable 正文滚动、
主题分段聚焦、Skip 锚点和 SplitButton 真实点击区域。宽屏保持原布局，日语主题保留 intrinsic 宽度，
沿用 860px 断点在窄窗单列；默认间距和 Config4 保持，不改 Core/RPC/State/扩展合同。
独立定向 27/11、8/4、13/1、1/0；一次集中 Chromium **383/12**（395 项，16.3m）。
旧断言、模板公开闭合时序与真实日语列宽分别修正后，失败及受影响路径 **53/1**，剩余新测试定位窄修后仅该项 **1/0**。
不拼成全套零失败；各轮 186 项输入及失败证据保留，最终输入与一次 GUI release no-bundle 构建一致。
GUI/UI check、metadata/diff 通过；不重跑 Rust 全量、Core release 或三平台 CI，新增代码仍无远端验证。

原 Core **143516** 持续使用原目录；仅关闭已记录旧 GUI，新 GUI **139052**（窗口 **8393506**，可响应）已打开。
构建前后项目、Editor、Operation、settings、status 五组完整公开状态全部一致，Config4/revision166 及文件摘要不变。
没有恢复桌面控制或伪造原生截图，原生缩放/实际读屏和跨平台辅助技术仍未覆盖。
以上为 H6 候选交付时的历史状态，所有者随后已阶段性签收 H6 并批准 H7；
各轮结果及 12 项对应关系见 [H6 验收记录](../testing/m7-h6-accessibility-20261102.md)。H2–H6 原签收保持，推送仍未授权，M7 进行中。

### H7 最终体验细调与默认保存目录缺口（2026-11-03）

本轮在既有成果上按所有者反馈逐项修正，保留 H2–H6 签收及既有 H7 技术证据，不进入 M8，不重验所有历史功能。
日常定向验证；相关整改形成稳定候选后集中必要回归，不因每条反馈重复完整门禁或发布构建。允许本地提交，不自动推送。

已确认 v3 `gui.settings` 基线包含项目/备份路径，当前 Config4、Settings DTO 和设置页均未实现。
新建及恢复项目的父目录从空值开始；Core 的 `BackupEngine` 固定绑定 `data_root/backups`，
既有 `backup-v1:<id>` 定位和任务恢复也依赖该根。因此只加 GUI 输入框不能补齐实际行为。

所有者已于本轮明确批准以下核心功能补齐范围：在设置中增加默认项目目录、默认备份目录，经既有 `settings.get/update`、
`settings.read/manage` 和串行 CAS 保存，扩展为 Config5；默认值遵循架构中的用户 Documents/ALCOMD/Projects、Backups。
项目目录用于新建、复制、恢复表单的初始目标，保留当次覆盖；备份目录实际控制后续新备份的保存位置。
每个已接受备份任务须固定其目的地，历史备份继续可读取/恢复；不自动搬迁旧项目、旧备份或重定向已有任务。
现有备份定位缺少独立目录，必须同步补足持久定位与恢复证据；真实既有数据需要的旧定位读取支持有保留价值，成本为两类定位的验证及回归。
不新增业务权限、生产依赖或第二套设置存储；若实施需要额外数据库结构或其他合同变更，先说明具体差异。
本次发布前扩展会增加严格 settings 消费者必读的 paths/directories，须同步协议、Schema、当前消费者与快照；RPC v1、权限、State16不变。
平台通过既有 Known Folder 查询 Documents，产品目录由 `alcomd.product.toml` 派生；不覆盖发布使用 Windows MoveFileExW(flags=0) 与 Unix 既有 rustix NOREPLACE。
Windows 的窄 FFI 保留说明与安全注释，不增加生产依赖；定向回归验证目标冲突不覆盖与旧/新目录恢复。
本地实现、相关定向检查及一次匹配 Core/CLI/GUI 构建已完成；该批原数据目录/profile 已实际恢复为 Core130768/GUI132996。
Config4→5 只改变 schema，revision166 和其余设置不变；四组业务状态完全一致。独立结果与原失败见
[目录设置补齐](../testing/m7-storage-locations-20261103.md)，不以旧 CI 或浏览器证据替代当前原生/跨平台验收。
本轮本地提交，不推送；H7 最终签收仍待所有者，M7 进行中。

所有者随后明确 320px 是刻意极窄状态，允许分段导航放不下；不再以该尺寸下的完整可见/横向滚动作为分段控件要求。
移除资源/设置分组/日志/主题的分段滚动容器、焦点自动滚动和对应 CSS，删除为该容器补的留白，沿用 Material 选择及稳定宽度行为。
回归保留正常桌面的初次展示、切换、键盘、语言和缩放；仅调整已取消的极窄分段要求，其余窄屏操作/表格覆盖保留。
分段关联 11/0、调整后的完整响应式模块 13/0，GUI/UI check 与 metadata/diff 通过；首轮过时的 320px 页头防溢出断言失败保留在本地测试产物中。
该批只作定向前端检查；随后在正常 1280/800px 的十个主页面继续核对，发现 About 刷新条点击区越界及长错误挤压按钮。
共享刷新条恢复默认高度、容纳点击区并固定按钮尺寸，删除已失效的滚动引用/样式；必要的表格和长内容滚动保留。
About 确定性修前 0/1；最终 Chromium/同版 Edge 各 3/0，相关分段 6/0，GUI check 通过；原失败/中止构建留在本地，不将定向合并为完整套件。
两批 GUI 调整已集中完成匹配 release no-bundle 构建，用原 profile 打开 GUI136124；原 Core130768 持续运行，五组公开状态和配置摘要不变。
原生交互/观感仍待所有者查看，桌面控制继续停用。不改公共合同、持久数据或依赖，不重复完整门禁。

设置页按所有者截图改用同一共享分段组件，当前源码分为“常规 / Unity / 外观 / 通知”，默认常规以“项目”和“备份”两个独立区块分别呈现默认项目保存位置和默认备份保存位置，复用原目录编辑及串行自动保存；重复的软件包区块及其仓库读取、专用文案与样式已按所有者要求移除，包筛选统一由项目软件包管理页提供，既有权威偏好继续保留。“系统信息”区块及其重复入口、专用文案按所有者要求删除，关于/诊断保留既有侧栏入口。
分组只控制显示，原设置组件保持挂载，保留路径草稿、失焦提交、串行自动保存；主题现与普通偏好共用队列，分组选择不写入设置，不增加滚动适配、公共合同或持久字段。
最终三组合并定向 21/0（1 worker、0 retries），覆盖分组、三语、键盘、自动保存、目录/软件包及正常桌面布局；未变路径按范围复用本轮此前定向结果。GUI check、metadata/diff 通过。旧用例只改为真实分组入口，保留原数据、权限和失败断言；不重跑完整 Rust/浏览器或远端门禁。
三组版本的匹配 GUI 构建及原 profile 启动完成，GUI143680/Core130768，五组公开状态和配置摘要保持；测试固定截图路径改用各自运行输出目录，误落外部目录的本批七张夹具图已核对摘要后收回工作区，历史图保留。
随后按所有者要求追加“通知”分段，内容区留空，不添加说明、开关或后台功能。已有应用内提醒保留，系统通知及其偏好尚未接入；GUI check 和该增量定向 6/0 通过。
主题随后直接展开到外观页，删除主题弹窗、HCT 三个调节条及无引用的换算函数、译词和样式，保留颜色选择/HEX/九配色、预览及显式保存。Save 后保持可编辑，Cancel 还原最新已接受主题；分组切换保留草稿，离开路由仍有未保存确认。中文模式统一“浅色 / 跟随系统 / 深色”。
内嵌首版 GUI/UI check、metadata/diff 及定向 **60/0**（1 worker、0 retries）通过，并用匹配 GUI132412/Core130768 核对五组公开状态和配置不变；该证据绑定首版，不替代所有者体验确认。
所有者随后确认改为逐行布局：复用共享设置行，模式/主题色/方案纵向排列，九方案使用现有 Select，删除两列布局和专用样式；操作紧接最后一行，保存和预览逻辑不变。GUI check、metadata/diff 与定向 **35/0** 通过，覆盖真实下拉键盘、九方案预览及保存/取消/失败/冲突、三语和布局。匹配 GUI 构建通过，原 profile 复开 GUI147632，原 Core130768、五组公开状态及配置保持。不改公共合同、权限、持久数据或依赖，不重复完整门禁；最终人工确认仍待所有者。
所有者进一步要求无图标分段内部左右各 24px，主题模式改为现有 Select；删除 showCheckmark 选项和隐藏宽度测量层，使用 Material 自身等宽布局。所有者确认窄窗允许放不下，包括本次 640px 英文设置导航；仅调整该完整显示断言，正文/路径/按钮可达性及其他窄窗覆盖保留。GUI/UI check、metadata/diff 通过；分批相关结果为 53/1（3 项未执行）及补查 5/0，连续下拉用例改等公开关闭生命周期，产品代码未再改动，原失败保留；详见当前 status，不冒称完整门禁。匹配 GUI 构建及原 profile 复开完成，GUI149844/Core130768，五组公开状态及配置不变，原生观感仍待所有者。

#### 设置卡片恢复默认（2026-11-03）

所有者要求设置卡片标题同行提供“恢复默认”，且每次都先确认，确认后只恢复本卡片。所有者最新明确Unity Hub路径和Unity安装列表两处作为特例，不提供恢复默认，保留原操作。其余六卡片：项目/备份路径恢复nullable系统默认，项目自动添加true；备份fast/保留软件包true/Library false；外观locale与motion恢复system；窗口rememberState/fullscreen均false；主题mode=system/sourceColor=null/scheme=vibrant；全局启动参数恢复null。不新增设置字段/权限/Schema。复用串行CAS，确认期间同字段被外部修改须拒绝，未知结果先重读，不重复写；取消不提交，目录本地草稿仅在确认成功后丢弃，主题按下述统一自动保存。旧主题footer及参数编辑弹窗内重复的恢复入口移至卡片标题，不保留两套入口。

按最新例外取消安装列表重置的提案，不新增RPC或Core登记写入逻辑。只在隔离夹具中验证偏好恢复，不对真实设置执行重置；配套桌面仅构建并打开以供所有者查看。

所有者补充：当前卡片已是默认值时禁用恢复按钮；未保存的目录草稿也参与判断，仍可确认丢弃；主题使用通用队列的当前意图判断。启动参数null与显式空数组保持区别；目录按nullable覆盖判断，不能将显示路径相同误作默认。确认成功并禁用按钮后，焦点返回本卡片标题；取消时仍返回启用的恢复按钮。

#### 主题统一自动保存（2026-11-03）

所有者要求移除主题底部取消/保存特例。主题mode/sourceColor/scheme接入现有SettingsAutosave.change，仅发送实际变更字段；复用普通偏好的串行CAS、连续改动合并、失败保留意图、显式Retry先重读及未知已提交去重。输入完整合法HEX或选择颜色/模式/方案即即时显示并自动保存，非法/部分HEX不写入，失焦还原当前有效颜色。保存中允许继续编辑、切页后队列继续；没有独立草稿会话、主题成功通知或离开确认。失败采用统一设置错误与Retry，不新增主题特例。标题恢复默认仍需确认，仅恢复主题三字段，默认时禁用，成功后返回标题焦点。不改RPC、Config9、State18、权限、默认值或依赖。

#### 项目与备份板块及默认行为（2026-11-03，已批准）

所有者要求“项目”包含默认路径及默认开启的自动发现/添加项目；“备份”包含默认位置、默认压缩等级、默认开启的保留软件包和默认关闭的保留 Library 缓存。板块标题及既有目录项可直接调整；新增行为不得用占位控件冒充完成。

只读审计确认原 Config7 无上述四项偏好；项目没有默认目录扫描，`projects.list` 是纯读取；备份已有 store/fast/maximum 三档压缩，但原 GUI 默认排除 VPM 包，Library 内容选择固定在已冻结 profile。项目所有者已明确选择“批准，按该合同实施”，以下为本轮有效实施合同（A-037）：

- **Config8**：沿用 `paths.projectDirectory` / `paths.backupDirectory`，增加 `projects.autoDiscover: boolean = true`，以及 `backups.compressionMode: store|fast|maximum = fast`、`backups.includeVpmPackages: boolean = true`、`backups.includeLibrary: boolean = false`。四项均为严格非 nullable 字段，PATCH 可独立省略，闭合空容器/非法类型/未知字段整次拒绝。Config1–7 有效迁移保留 revision 及全部既有设置，只补四项默认值；不重置路径、主题或窗口偏好。继续原串行 CAS、16 KiB 文件上限和原子替换。
- **权限及自动发现**：设置仍需 `settings.manage`；修改自动发现字段，或在开启自动发现时更改项目默认路径，额外要求现有 `projects.manage`。Core/application 内部任务在启动、开启或路径变更后调度，并在 Core 运行期间每30秒合并重扫；不阻塞 ready，不改 `projects.list` 的只读语义，不新增公开方法或权限名。只枚举默认根目录的直接子目录，不递归、不跟随链接/reparse；复用 `ExactRoot` 检查和 Store 文件身份查重。单轮最多4096个目录项、单个扫描任务在途，重复项目不新增记录/事件；无效项目、不可达根目录或扫描上限不删除已登记项目，也不写用户工程。关闭或切换配置后不再接受旧一轮的后续登记。默认目录中的项目若只从列表移除、文件仍在，之后扫描会重新加入；本轮不新增持久排除清单或数据库迁移。
- **备份默认值与单次覆盖**：压缩继续现有不压缩/快速/最大三档，未另指定的默认等级沿用快速。新备份表单读取权威默认值，允许当次修改；软件包选项使用正向“保留”，映射原 `excludeVpmPackages` 的反值。CLI/其他入口未显式指定时，也由同一 application 用例解析当前默认值；已经显式指定的值优先。接受任务时固定有效目的地及各选项，后续设置更新不得改变已接受操作或同幂等键重放。
- **兼容性与视图跟进**：本轮是已批准的发布前设置模型增加与备份可选参数增加；RPC major保持1，严格设置消费者须同步完整Config8模型，既有显式备份请求和旧已接受任务继续有效。GUI只在自动发现开启且项目页可见时，每3秒单在途重读既有projects.list，静默保留列表及焦点、切页取消跟进；不扫盘、不登记项目、不把读取变成写操作。
- **`backups.create` 与内容选择**：新增可省略的 `includeLibrary` 布尔选项；压缩及排除包字段允许省略以使用权威默认值。显式值仍严格校验，null/非法值拒绝。Library=true 包含标准项目根目录 `Library` 的缓存内容，其他临时目录、链接、路径穿越、配额、哈希、取消及一致性门禁继续保持；false 沿用原 Library 缓存排除策略及既有 `LastSceneManagerSetup.txt` 保留规则。同步修订现有 Backup Archive v1 内容选择 profile，保持归档 manifest、已存 BackupRecord 和恢复格式形状，不新增数据库列或另立归档格式。
- **必要的原任务保留**：原已接受任务没有 Library 字段时按原 false 语义解码；false 选择保留既有幂等 fingerprint，true 必须进入 fingerprint，防止不同内容选择错误重放。该保留用于保护当前数据库中已有任务/恢复证据，不新增旧字段别名、平行解析器或常态旧产品依赖。新增设置、可省略请求字段及此最小兼容处理均已获本次批准。
- **验证**：Config8 默认、严格形状/迁移、字段级权限与串行保存；临时目录下自动发现开关、路径变更、身份去重、上限及链接拒绝；备份三档压缩、包/Library组合、实际归档/恢复内容、同幂等键冲突及接受后设置变化；定向真实隔离 RPC/重启与 GUI 三语/单次覆盖。只用可丢弃合成 fixture，不为测试运行自动发现或备份用户真实工程。不新增生产依赖，不进入 M8/M11，不提交、推送或启动真实数据环境。

本增量已实施，最终本地完整Workspace598/0（8既有helper ignored）、严格Clippy/fmt/xtask/metadata及前端check/build通过，最终定向Chromium8/0。自动扫描复用发布锁并增加启动恢复调度前的RAII屏障；GUI读取代际保护显式刷新。回归发现的RPC派发栈溢出通过将大型future放到堆上修正，全部原测试保留。所有者另行明确批准为IPC验证临时关闭并原样恢复既有GUI/Core；只用隔离夹具，最终原程序/数据/profile及五组状态、配置hash均已核对恢复，不将本增量指向真实数据。完整证据及未覆盖范围见[验证记录](../testing/m7-project-backup-defaults-20261103.md)，不提升M7/H7人工状态。

#### 备份压缩等级细化（2026-11-03，所有者已选择六档）

所有者反馈现有三档过少。审计确认当前仅覆盖ZIP Stored、Deflate level1和level9；数据库backups表也将compression_mode闭合限制为store/fast/maximum，且有恢复计划外键引用，因此不能只增加GUI选项。

所有者明确以NanaZip截图选择六档：0-仅存储、1-极速压缩、3-快速压缩、5-标准压缩、7-最大压缩、9-极限压缩；以此替换此前未批准的0–9逐级提案。GUI设置默认值及单次备份统一使用上述六档，默认保持1。RPC/CLI/持久枚举保留store/fast/maximum分别表示0/1/9，新增level3、level5、level7三个有效值，不增加同义别名。显式非法值和null仍拒绝，已接受任务固定等级且各等级进入幂等指纹。

Config9仅扩展该枚举；有效Config1–8迁移保留revision及全部偏好，现有fast默认含义不变。State18仅在原子迁移中扩展backups表该CHECK，保留所有行、列、索引、约束、备份ID及恢复计划外键，其他状态不动。同步扩展Archive v1压缩枚举及Schema/profile；保留旧归档和旧已接受任务原有含义、指纹及恢复行为。旧版本程序不承诺读取新增等级的归档，所有严格消费者同步更新，不增加依赖、权限或公开方法。

验证覆盖全部六档的真实归档/恢复、24组等级×包/Library内容组合、非法值、旧配置/数据库/归档保留、接受后设置变化及重放；设置和备份弹窗一致、单次覆盖不回写默认值。所有者在已说明Schema迁移及保留成本后，以截图确定最终档位，按该合同实现并按本轮桌面查看流程集中构建更新，不执行真实工程备份或清理已有数据，不提交或推送。

#### Unity 设置截图对照与补齐（2026-11-03）

只读对照 v3 设置、启动流程及当前 v4 后确认：全局默认 Unity 启动参数尚缺；已有项目参数不能替代它。
原 `m7-v3-settings-audit-20260919.md` 已记录此缺口，未找到将其正式取消或划入后续里程碑的决定。
当前安装列表的刷新仅重读 registry，真正 discovery/register 位于管理弹窗；这些能力已有，并非尚未实现。
v3 旧 Hub 开关切换读取 Hub 配置与调用 Hub CLI 两种 Editor 发现策略；v4 当前只扫描平台默认安装根，
尚未覆盖 Hub 自定义安装根及已定位 Editor。此发现缺口单独补齐，不直接复制旧开关或旧实现。

**所有者已批准全局参数最小合同，并要求依据 Unity 官方规则实现：**

- Config5→6，`settings.unity.defaultLaunchArguments: string[] | null`；TOML `unity.default_launch_arguments` 可省略。
  null 使用产品内置默认（当前为空数组），`[]` 是显式空参数；settings PATCH 省略该字段表示不变。
- 沿用 settings.get/update、settings.read/manage；修改此字段同时要求已有 unity.manage，不新增权限名或 RPC method。
  `unity.projectLaunchConfig.get` 在现有 config 外增加 `defaultArguments: string[]`、`effectiveArguments: string[]`，
  由 application 投影；config.arguments 仍表示项目覆盖，既有项目读取/设置/清除/启动权限不扩大。
- 实际启动优先级：存在项目覆盖（含空数组）→全局覆盖→内置默认，整组替换而非拼接；清除项目覆盖恢复继承。
  GUI 的默认参数组与项目参数编辑器应准确区分继承、显式空数组、取消和恢复默认，不沿用 v3 的 null/取消混淆。
- 复用现有参数数量/长度/项目路径选择器禁用规则，拒绝 NUL，保留 ConfigStore 16 KiB 文件总上限和串行 CAS。
  参数通过 argv 数组传递；不使用 shell，不把完整参数写入 Activity、普通错误或诊断摘要。
- Config5 及既有迁移输入增加 null 默认，保留 revision、目录、Hub、主题及其他设置。
  定向测试发现 State16 只允许非空数组；所有者另行批准 State17 最小迁移，仅允许该表保存 0–64 项，原样复制旧记录，
  其他列、外键、STRICT、大小/修订约束保持，不改项目文件。此前不需 State 变更的实现假设据实际约束纠正。
  接受启动时固定有效参数快照；全局变更不得改变已接受启动，同幂等键受影响的设置变更沿用明确冲突而非静默改参重启。
- 新增全局参数设置 UI、Core 用例/读取投影、协议/Schema/消费者/快照及定向回归按一个独立功能提交；不新增依赖，
  不执行真实用户项目启动、迁移或 Hub CLI。Editor 发现完整性与现有入口简化作为另一项整改，不捆绑审批。

Unity 官方 2022.3 [参数说明](https://docs.unity3d.com/2022.3/Documentation/Manual/EditorCommandLineArguments.html)
及 Rust 官方 [Command::args](https://doc.rust-lang.org/std/process/struct.Command.html#method.args)
确认 flag/value 分别传入、含空格路径保持单个 argv，不做 shell 引号解析。`-openfile` 另选项目入口、
`-createProject` 创建项目，故和 `-projectPath` 一并拒绝，保持打开所选项目的语义；不增加跨版本 flag allowlist。
Core 共享已初始化的设置快照，启动时固定有效数组；继承非空参数使用有界私有幂等标识及响应快照绑定准确 argv，
既有无附加参数及项目覆盖请求保留旧指纹，不改写历史记录。完整参数不进入 Activity/普通错误。
GUI 逐项编辑参数，保留空值、空格、引号与反斜线，使用共享 Material/提醒/错误组件；保存失败、未知结果和竞争写入不冒充成功。

本批定向结果：Application52/0、Protocol52/0、Store123/0，私有快照长度修正后 M5 模块4/0；隔离RPC/合同25/0、端点所有权4/0；
GUI check/build、保存队列24场景、浏览器65/6与完整受影响两模块补查14/0，测试预期/插槽定位修正没有改产品实现或删除断言。
六crate Clippy、fmt/xtask/metadata及一次匹配Core/CLI/GUI构建通过；原数据/profile已恢复，Core125676/GUI144536，
公开状态保持，Config6迁移仅改TOML版本，保留实际切换前revision168。各轮准确范围见status，不冒称完整门禁或M7签收。

#### 持久全屏模式（2026-11-03，所有者已批准，A-039）

所有者要求在“外观→窗口”增加全屏，并明确批准跨启动保留：Config10增加window.fullscreen非nullable bool，默认false；有效Config1–9严格校验后只补false，保留revision及原设置/数据。window PATCH可独立设置rememberState/fullscreen，空容器、null/非法类型/未知字段整次拒绝；沿用settings.read/manage及串行CAS，无新RPC method、State18迁移、权限、依赖或任务变更。

GUI Checkbox和F11共用同一设置意图队列；已接受偏好经现有主窗口私有薄适配器应用，不以可丢弃缓存存储全屏偏好。首次普通尺寸恢复后再应用全屏；全屏切换不再次恢复旧几何，过渡尺寸和全屏尺寸不写入普通窗口缓存。窗口卡片恢复默认先确认，将两项同时置false并退出全屏；默认时禁用。原生应用失败沿用安全提醒并提供不重复写设置的原生Retry。默认关闭不会主动更改其他窗口设置。

本批必要验证包含Config9迁移保留、严格读写/窄PATCH、串行/丢响应/冲突、GUI默认/持久/F11/恢复/三语、原生几何模型与桥接协调；测试真实IPC时暂时关闭原Core/GUI，结束后以配套新版、原数据/profile打开。真实原生全屏交互和三平台表现不以浏览器/模型测试冒充人工验收。

#### 窗口状态记忆：最小设置合同（2026-11-03，所有者已批准）

所有者要求实现窗口状态记忆，在设置中提供默认关闭的开关。范围为普通窗口大小与最大化状态，
不增加窗口位置、最小化启动、全屏偏好或通用窗口管理器。按所有者后续纠正，窗口开关放在外观页，使用现有设置行与 Checkbox。

- Config6→7：新增 `settings.window.rememberState: boolean`，默认 false；TOML `[window] remember_state = false`。
  `settings.get/update` 沿用 settings.read/manage；PATCH window 为闭合非空对象，只接受该 bool。
  Config1–6 迁移补 false，保留原 revision 和其他设置，不改 State17、权限名称或 RPC method。
- 开关是 Core 权威设置，继续经现有串行 CAS/错误反馈。窗口几何仅是可丢弃 GUI 视图缓存，
  与现有项目视图一样可使用 localStorage；不把业务数据或权威开关放在缓存中，不新增 GUI 文件写入。
- 默认关闭时不恢复、不记录几何并清除旧缓存；启用后记住普通窗口逻辑宽高和最大化状态。
  下次启动恢复一次，关闭/重新打开设置或切换页面不重新套用旧窗口大小。
  缓存丢失/损坏/不可用时使用现有默认窗口；恢复尺寸按当前显示器可用区域限制，忽略最小化临时尺寸。
- 使用已安装 Tauri 2.11.5 的窗口/显示器 API 和闭合的本地主窗口薄适配器；不新增依赖、任意路径或任意窗口操作接口。
  native 只处理自身窗口 UI，不访问 Core 数据库或项目。必要恢复错误沿用共享提醒，不能因读写视图缓存失败阻止应用启动/关闭。
- 定向验证覆盖默认关闭、开关保存/失败、缓存与恢复一次、普通/最大化/最小化尺寸及显示器/DPI边界；
  桌面控制保持停用，浏览器或纯逻辑检查不冒称原生调整窗口已验收。本批单独本地提交，不推送。

窗口设置已接入现有保存队列；几何缓存只受已接受的 Core 值驱动。新主窗口适配器使用 Tauri 已有 API，
事件目标匹配 JS Window 监听，恢复 API 请求后读取实际状态解除中间事件保护，避免依赖精确尺寸事件而长期冻结。
本批浏览器25/0、保存队列27场景、缓存11场景、原生纯模型6/0；Application53/0、Protocol53/0、Store配置25/0、
隔离RPC/合同8/0，六crate Clippy与原生最终补查、fmt/xtask/metadata通过。实际原生调整/关闭重开仍待所有者查看。
匹配Core/CLI/GUI构建及所有权4/0完成，原环境恢复Core130792/GUI149024，公开状态保持，Config7仅补false且revision168不变。
后续按所有者要求，窗口开关移至外观页（`249e6d7`）；全局成功、设置保存失败和窗口失败共用 ReminderProvider 右下出口（`76e404b`），删除独立右上定位，保留持久错误、详情、重试及迟到成功隔离。
GUI check、受影响定向浏览器20/0及一次GUI release no-bundle构建通过。该批GUI127300（`C8D8B6AE…F9424825`）使用原profile，Core130792持续运行；五组公开状态和配置摘要均保持，不增加Rust/完整浏览器/远端CI重复验证。

#### 日志系统重建（2026-11-05，所有者整体批准，源码候选）

本节取代下方日志/提醒窄修合同。按未发布、空环境首次启动实现；不设计旧数据转换、旧格式兼容、存量任务等待或旧系统接续。开发机原数据、截图与验收材料保留。M7/H7 未签收，不扩展 M8–M12。

- 移除全部浮动提示、日志触发提示、RPC 响应补写、任务终态摘要、recordGui 及独立造记录缓存；保留就地失败、重试、确认、导航与完成回调。普通缓存仅保存真实记录。
- 技术日志采用 tracing / tracing-subscriber 标准 JSON Lines；默认应用 INFO、第三方 WARN，RUST_LOG 显式启用 DEBUG/TRACE。保留 timestamp/level/target/fields/spans，来源、会话和诊断/任务关联不另造 formatter。
- 错误保留路径与 source，在责任边界显式展开不重复的原因链并记录一次；逐层传播、RPC/GUI/终态订阅不补写。真实重试可新记；不承诺文件/数据库原子或恰好一次。普通 RPC/Activity 仍保持闭合安全字段；自动插桩跳过请求/配置/argv/env/凭据，网络错误先去认证 URL，正常完整路径保留。
- 新增 tracing-appender = '=0.2.5'（default-features=false，MIT，MSRV1.63）；现有 tracing-subscriber 0.3.23 增加 json。实际锁文件新增 tracing-appender0.2.5、symlink0.1.0、tracing-serde0.2.0 与本地 alcomd-logging；复用现有外部包，未升级其版本。替代方案为重建私有队列；选择官方有界线程与 guard，项目仅维护窄大小轮转 writer 与健康观测。工程 MSRV1.97.1 保持。
- 完整记录最大512KiB，队列64条（正文最多32MiB），非阻塞丢弃；每稳定来源跨会话最多5份且每份5MiB。独占来源租约，段ID不复用，不删除有效写入者文件。整行写完且可读后才推进位置，入队不通知内容；不静默截断路径/JSON。
- 独立内存健康含 sourceId/sessionId/revision/state、queueDropped/oversizeRejected/writeFailed/rotationFailed/flushFailed/hasGap/lastFailure。拒收/磁盘故障直接唤醒健康读取，不依赖成功日志行，不递归记录，不依赖stderr作为唯一出口。
- 正常退出停止业务、最后记录、关闭生产入口，再在关闭线程释放长期持有的 WorkerGuard；核对排空/flush，日志关闭总预算2秒；不承诺强杀/断电/超时零损失。
- Core读自身受限日志；GUI原生层写/读/等待自己的受控目录，Core离线仍可用。前端异常仅窄字段上报一次，不接收业务成功、Core失败、RPC响应或任意对象。
- 记录身份为来源/会话/段ID/字节位置；首次B建立后接续历史与实时。源端筛选覆盖保留文件，默认100/最多200条、每次扫描1MiB、完整帧≤4MiB；无匹配也推进。半行/损坏/清理/游标过期/读失败明确报告。缓存≤2000条或8MiB，无稳态周期读取。
- diagnostics.log.read 改为基线/向前/真实变化等待（无空闲超时空页），history 改为有界筛选分页，增加 health；沿用 diagnostics.read/local-owner。删除 diagnostics.list 和 notify/提示字段。Rust/SDK/GUI/浏览器夹具同步，无旧格式兼容。
- 独立活动保存ID/owner/revision/action/status/固定目标快照/闭合参数/起止时间/任务与诊断关联。已核对编号，State19 增加表和索引，并扩展 Events 的 activity aggregate CHECK（保留原事件、序列与索引）；不转换旧活动。Operation继续权威；接受/状态/取消/中断/恢复/终态与活动及activity.changed同事务；进度/重复状态/幂等重放不增加活动revision。
- 同步业务与活动同事务；已授权明确执行的预检失败独立记录失败活动，业务对象/任务/幂等零副作用允许该历史例外，数据库不可用如实报告。查询/预读/无效协议/未授权无活动；Plan/预览不独立记，Apply/直接执行一次活动。
- activity.list 使用新模型，增加按ID读取；固定按(startedAtMs,activityId)排序，目标删除不级联、不改摘要，本轮不自动删除。恢复属于新系统自身能力，不是开发数据兼容。

执行顺序：合同→独立清理提示→责任边界→writer/健康→同源读取→活动事务/恢复→页面/夹具→验证闭环后删除旧实现，不长期双写。故障只在隔离目录可控注入。定向日常检查，稳定候选集中完整门禁；不操作真实Unity/项目、不推送。

本轮是发布前破坏性替换：RPC major 仍为1，严格客户端必须同步更新 SDK；不保留旧 DTO/notify/diagnostics.list 别名。`events.live.v1` 和资源读取权限保持。
搜索按解码后的字段名/值（含正文、target、span）大小写不敏感匹配，最多4096 UTF-8字节；不匹配 JSON 转义字符串、不截短查询。读取问题最多32条，更多合并为 `additional_issues`，健康最后故障最多4096字符。
空闲 follow 无周期超时；请求写入及首字节到达后的完整帧传输各最多5秒，失败丢弃连接。取消/窗口销毁/页面重载取消旧读取，GUI 离线日志不依赖 Core。

字段职责示例（示意，不是实际操作证据；完整位置身份位于读取包装）：

```json
{"timestamp":"2026-11-05T04:00:00Z","level":"INFO","target":"alcomd_vpm::repository","fields":{"message":"repository fetch completed","package_count":42},"spans":[{"name":"operation","operation_id":"33333333-3333-4333-8333-333333333333"}]}
{"timestamp":"2026-11-05T04:01:00Z","level":"ERROR","target":"alcomd_vpm::project","fields":{"message":"failed to inspect Unity project","path":"C:\\ALCOMD-Test\\项目A\\Packages\\vpm-manifest.json","error":"invalid VPM manifest\ncaused by: expected a version object","diagnostic_id":"44444444-4444-4444-8444-444444444444"},"spans":[{"name":"request","request_method":"projects.register"}]}
{"activityId":"55555555-5555-4555-8555-555555555555","revision":3,"action":"backups.create","status":"succeeded","startedAtMs":1793851200000,"finishedAtMs":1793851202000,"subject":{"kind":"project","id":"66666666-6666-4666-8666-666666666666","name":"项目A"},"parameters":{"compression":"level3","includeLibrary":false},"operationId":"33333333-3333-4333-8333-333333333333","result":{"code":"backups.create.succeeded"}}
```

技术记录保留技术正文，不本地化；活动只持久稳定 action/result 和闭合参数，由页面翻译。数据库事务不涵盖日志文件；关联日志缺失/已轮转时显示不可用，不合成替代记录。
State19 的任务触发器保证接受、状态/终态及恢复与活动和独立 `activity.changed` 一起提交；活动目标/参数由相应用例在同一接受事务固定。公开进度不改活动 revision。原业务表、任务、事件和恢复数据不清理，不回填旧活动。

本轮实现覆盖：M2状态检查；项目/仓库与文件观察；包、模板、备份/恢复、复制/删除、Unity迁移、用户包、扩展及Portable UI实际执行失败；配置/Hub与GUI原生连接、窗口、前端真实异常。技术错误由持有原始错误的请求或任务边界记录，不在通用响应、活动或终态订阅补写；项目发现以scan为唯一后台记录边界。未执行完整原生产品流程，不以源链覆盖表冒称每种OS故障均已注入。

本候选验证（各轮原始输出在 `target/m7-logging-rebuild-20261105/`，工作树包含并行改动）：

| 范围 | 实际结果 |
| --- | --- |
| Rust静态门禁 | workspace all-targets严格Clippy通过；后续受影响crate再次通过；xtask通过。最终全仓fmt及diff检查通过。 |
| 单元/二进制 | 排除会激活默认Windows端点夹具的testing/CLI/host后21个目标447/0；CLI5/0、host2/0另跑。收尾改动后Core34/0、App72/0、Store69/0、GUI17/0独立复跑通过，不合称一次完整workspace通过。 |
| 存储/合同 | Store11个集成目标分批补齐（96个用例分别保留）；Schema43/0、模拟迁移worker7/0。原空override快照先后因版本18断言和新增空activities表失败，随后保留全部旧行对比并显式断言不回填；项目/用户包事件测试保留原业务事件与幂等断言，另核对独立activity.changed。 |
| 文件/IPC | 日志底座15/0覆盖非阻塞满队列、入队非可读、写入/轮转/flush故障、健康独立唤醒、检查等待竞态、跨会话五段、损坏/超大行跨扫描边界、游标/筛选。私有Windows管道真实帧验证1/0包含hello/read/history/health/取消/空闲无空页，不使用用户Core端点。 |
| 路径/责任边界 | 真实坏清单及缺Editor/缺发现目录均保留中文完整路径/原因；一次失败一条、已有DID传播/真实重试新DID、扩展伪造DID拒绝。GUI17个单元包含离线真实异常写入读取、连接原因与不补记Core业务失败。 |
| 前端 | SDK/GUI check通过，1122三语消息；最终Vite build通过（保留既有chunk大小提示）。模型7/0；长中文及emoji异常入口最终4/0，按128/4096/16384/2048 UTF-8字节限额，不切破字符。 |
| 浏览器轮次 | 135轮134过/1失败；126轮60过/10失败/56未执行；后续68轮65/3、26轮24/2、21轮20/1、13轮12/1，最后长查询边界1/0，前端UTF8异常4/0。旧浮动断言、真实就地遮挡/代际/菜单问题与夹具/选择器分别修复后专项补验；不拼成全套通过。 |
| 未执行/环境 | 完整cargo test --workspace、现有固定Windows端点集成、整个浏览器套件、三平台CI/CodeQL和原生人工观察未执行。未操作真实Unity/项目、未迁移原数据库、未重启或替换运行Core168708，无GUI进程；原数据/profile/材料保留，未提交日志候选或推送，M7/H7仍由所有者签收。 |

#### 以下为已撤回的旧日志候选证据（不再定义当前实现）

原“日志触发提醒”“日志正文修正”“历史会话日志查看”描述下方历史候选，不能用其既有通过结论证明本次重建。此前以GUI提示补记宣称统一的结论已撤回。

#### 日志触发提醒（2026-11-03，历史合同，已由重建取代）

所有者于2026-11-05进一步明确：浮动提示必须有对应技术日志，技术日志不一定弹出。v3只作行为审计：全局Logger及命令错误显式写日志后进入缓冲/前端，gui_toast只控制是否提醒；不复制或移植其源码。以下保留既有公开读取合同，并补齐Core采集和GUI会话日志入口。
本切片前Core只有tracing进程输出；`diagnostics.list`是失败任务投影，`events.list`不提供技术日志订阅。本切片以同一Core日志源补齐安全读取，不靠查询失败任务或GUI读取Core输出文件替代。

- 新增单一RPC `diagnostics.log.read`及capability `diagnostics.log.v1`，沿用`diagnostics.read`且本切片只允许认证local-owner；无新增权限名、数据库/Config迁移或生产依赖。所有读取经application用例层，Tauri只作传输适配。
- Core复用tracing收集真实正文及经审查技术字段，保留1024条内存缓冲；公开记录增加已批准的message/target，实际规则见下方“日志正文修正”。默认采集Debug及以上，Trace不进入默认Core日志；secret、argv/env、请求参数和任意结果值不进入日志。
- 请求为闭合对象`{cursor?: {instanceId, sequence}, limit?: 1..200, waitMs?: 0..20000}`，默认limit100、waitMs0。响应为`{items, nextCursor, reset, gap}`；记录按sequence递增，明确标记Core重启、过期游标和缓冲截断，不伪造连续。首次返回当前有限历史及尾部游标，GUI只建立基线、不补弹既有记录。
- GUI通过专用只读RPC连接执行有界长读，不占用普通操作连接；断开后同实例从已消费游标续读，退出/换连接时取消旧任务。日志页与提醒消费同一安全记录源；既有失败任务历史保留，不冒称已补齐完整技术日志检索、导出或持久历史。
- 日志Error默认`notify=true`，Warn默认false，允许源显式`gui_toast`覆盖；info/debug/trace保持静默。仅新增且notify=true的记录进入现有唯一右下出口；以实例/序号避免重复消费。同步RPC上下文默认`gui_toast=false`，事件显式标记优先，后台任务不继承请求静默；已由页面处理的错误保留其详情/重试。
- Core统一记录已认证业务RPC的安全结果：失败为Error并与响应共用已有diagnosticId，删除统一成功Trace；日志读取、事件传输及握手不反向制造日志。任务终态通过既有提交后事件订阅采集，进度不生成日志；不依赖页面等待或增加轮询。
- GUI复用有界会话日志缓存，所有浮动反馈先登记日志再展示，标明GUI来源；Core错误与界面上下文按diagnosticId合并，关闭只收起提示。界面上下文displayMessage/title为本地元数据，不替代Core正文，不进入持久业务模型；Core与GUI记录共用1024条视图上限，重新加载不承诺保留会话记录。
- 不扩展通知设置、系统通知或后续里程碑；文件日志仅按下方已批准的固定轮转规则实施。验证覆盖缓存/游标/脱敏/权限、长读取消与连接隔离、首次不补弹/新增只弹一次/级别覆盖、日志先于提示、错误关联去重及现有模态交互；不重复全量门禁，真实环境切换沿用既有授权边界并集中准备稳定候选。

所有者已明确批准上述最小日志合同；后续“提示只来自日志”继续保持；日志正文/来源字段依照下方专项批准同步，不新增RPC方法、权限、数据库、配置或依赖。权威公开Schema为`specs/rpc/m7-diagnostic-log.schema.json`。

2026-11-05本轮代码为`959a86c`/`3571e39`；活动补链为`1b14336`。Core单元32/0、Store单元51/0、受影响Clippy/fmt、GUI check/build和日志模型通过。定向浏览器首轮21/0，日志基线去重后专项13/0分别保留；不冒称完整Rust/GUI门禁。真实IPC验证错误与日志同ID、读取不递归、3条原被过滤的打开活动恢复。已更新dev Core106648/GUI133500并核对原profile、六组公开状态与配置摘要保持，Unity未动。构建输入与部署证据见`target/m7-log-origin-20261105/runtime-20261104T213920Z/`；会话日志不具备跨重启文件历史，原生观感与M7/H7签收仍由所有者确认，未推送。

#### 日志正文修正（2026-11-05，所有者已批准）

本轮根因已核实：Core采集器主动丢弃tracing正文和技术字段，公开记录只有code，GUI再拼通用摘要；随后统一RPC成功Trace进一步制造噪声。v3实际行为是同一完整LogEntry经全局Logger进入文件、内存及前端，不采用代码冒充正文；只参考行为，不复制源码。

最小公开合同修正：在既有`diagnostics.log.read`条目增加必填`message`（非空、最多4096个UTF-8字节）及`target`（来源模块、最多128个ASCII字符）；同名字段原样同步Rust DTO、SDK、Schema和夹具。原游标、级别、code、diagnosticId、notify、local-owner及权限保持，不新增RPC/权限/数据库/配置/依赖。未发布客户端同步替换，不保留无正文记录的兼容层。例：列表显示`Unity Editor executable is unavailable`；日志内保留path/reason等经审查技术详情，code/错误编号在详情查看，不以`diagnostic.warning`冒充日志正文。secret、凭据、请求参数、argv/env和任意RPC结果禁止进入日志；本机路径允许用于排障。

采集沿用tracing及现有会话日志入口：移除通用RPC成功记录；错误保留实际安全正文与同一diagnosticId；任务终态记录具体任务种类和结果，进度不刷日志。读取、文件和提醒消费同一已接纳条目，关闭不删除记录，日志读取不产生新日志。Core在自身数据目录的logs下写UTF-8技术日志文件，固定5MiB/文件、最多5份，不改业务持久模型；文件失败明确保留内存记录并报告，不能递归写同一失败。GUI仍有明确来源的本地会话记录；不宣称GUI离线记录已写入Core文件。既有长读/重连、首次不补弹、去重及唯一提醒出口保持，不新增业务轮询。

验证聚焦真实正文、中文/换行、边界与secret排除、文件读写失败和轮转、错误/提示同条记录、普通读取无日志噪声、关闭/重连不重复、原生IPC实际正文及原环境保持。不重验全部历史功能，不扩展通知设置或M8。所有者已明确批准本节最小合同，正文、文件和客户端按配套版本实施；不再增加文档审批。

代码`9b12d93`已实施。Core36/0、采集4/0、Application日志8/0、协议专项1/0、受影响严格Clippy/fmt、xtask、SDK/GUI check/build和模型通过；最终浏览器22/0，既有22/0轮次单独保留。真实IPC证明正文/来源/编号跨管道保留并与文件一致，普通读取零新增日志、默认Trace为0。Core120792/GUI127336已用原数据/profile恢复，六组公开读取和配置摘要保持、Unity未动。文件格式为UTF-8 JSON行；文件写入队列1024，启动记录在取得唯一Core后接入文件，失败/溢出明确报告且不递归。RPC不新增旧文件检索。构建及环境证据在`target/m7-log-origin-20261105/runtime-20261104T221606Z/`；保留并行修改，后续Unity守卫等Rust增量不属于该Core产物，不借用本批结果。未推送，M7/H7签收保持待所有者。

#### 历史会话日志查看（2026-11-05，所有者已批准）

所有者要求技术日志页能查看文件中保留的旧会话。最小新增只读RPC为`diagnostics.log.history`，沿用`diagnostics.log.v1`、`diagnostics.read`及认证local-owner，不新增权限、数据库、配置或依赖。请求为闭合`{cursor?: string, limit?: 1..200}`（默认100）；响应为`{items, nextCursor?, skippedCount}`，items沿用现有完整日志条目并保留原instanceId/sequence，按文件记录从新到旧分页。cursor为最长128字符的不透明记录位置，过期明确返回`log_history_cursor_expired`；文件无法读取返回`log_history_unavailable`。最多读取既有五份5MiB日志；异常/不完整记录计入skippedCount并由界面明确说明，不能伪装为完整空历史，不读取任意用户路径。

Core在独立阻塞读取任务中检查文件，历史读串行且不持有数据库事务；与轮转协调取得文件快照，读取期间不占用写入队列或等待网络。GUI进入技术日志页即加载已保存历史，沿用表格、筛选和“加载更多”，按原日志身份与实时记录去重；历史内容仅供查看，不接入提醒消费队列。读取失败保留当前内容和显式重试。每轮历史遍历最多覆盖保留文件总量25MiB，离页释放历史视图数据；不轮询文件、不加入通知设置或会话管理面板。Rust客户端、SDK、GUI抽象和浏览器内存夹具同步。

定向验证覆盖跨会话、跨文件分页与轮转、实时/文件重复、旧错误不补弹、损坏/不完整记录提示、过期游标、权限/任意路径拒绝、普通操作不被文件读取阻塞，以及更新后真实IPC和原环境保持。其余采集与文件格式沿用上一节，不重做日志系统或历史业务功能。

所有者已明确批准上述最小读取合同，直接完成RPC/SDK/Tauri及界面接入。历史查询以独立连接及5秒读取期限执行；后台最多一个文件读取任务，完整解析前释放轮转目录锁，不占用业务连接或数据库事务。GUI历史缓存仅向更早位置遍历，最多保留既有文件总量25MiB，离页释放，不周期重读。

同时已按截图查明另一独立缺陷：Reminder通过recordGui把前端业务成功提示反向补成GUI会话日志；“Unity启动状态：工程打开”来自此链路。本轮历史接入不构成该缺陷已修复的证据；此前“所有提示已由真实日志驱动”的结论撤回，不能用换文案或增加一层补记宣称完成。

### 当前 H7 授权与收束（2026-11-02）

直接执行既有 H7，不另设规划审批。先核对现有实现与证据，仅修真实使用缺陷；普通文案、一般视觉和过度设计待办继续非阻塞。
原完整 Chromium 383/12 与后续 53/1、最后新增日语用例 1/0 独立保留，逐项对应不等于合并通过。
稳定候选集中执行适用的完整 Rust、前端、Chromium 与构建门禁，绑定实际执行 SHA 和输入摘要；聚合脚本重叠项不重复执行。
Windows 完整 IPC 测试必须使用隔离数据；沿用已批准流程，在原任务全终态、身份核对后暂停记录中的原 GUI/Core，终态后实际恢复并比较五组完整公开状态。
真实读屏器、跨平台辅助技术和客户端发行环境验证保持 M12 归属，Portable UI 未覆盖原生状态单独记录，不虚报或扩大至 M8/M9 业务。
最终人工清单原位整理在 `docs/testing/m7-official-gui-manual-checklist.md`，合并历史重复项，保留 H2–H6 签收引用；
桌面控制仍遵守此前停止边界，没有新增原生交互或截图时如实标记。形成准确本地候选后交所有者检查，再单独确认远端安排；不自动推送、不自行宣布 M7 完成。

H7 本地候选已形成，冻结执行 SHA `a6c9bd5568f8bffb26fe531a58fe4286c7779f6f`，产品代码与 `a4f12a1` 相同，无额外产品修复。
完整 Workspace **526/0，8 个既有 helper ignored**，独立完整 Chromium **396/0（14.9m，1 worker、0 retries）**，
fmt/Clippy/xtask、GUI/独立 backend 检查、全部前端 check/build、metadata/lockfile、并发启动、release bins 与 Tauri release 均通过。
首次磁盘不足链接失败和 Chromium 354 项后会话中断分别保留；前者定位容量后续跑未完成门禁，后者另取一轮完整结果，不拼接。
589 项非文档输入及验证期间 108 项文档均未变，之后只做文档收尾。原环境真实恢复为 Core131912/GUI153004，最终五组公开状态一致，Config4/revision166 保留。
实际窗口保持打开；Computer Use 未恢复、无新增原生截图。窗口记忆、仓库凭据及 Portable UI/平台未覆盖边界均在
[H7 候选记录](../testing/m7-h7-final-20261102.md) 列明；两项人工清单等待所有者，远端 CI/CodeQL 等待独立推送安排，M7 进行中。

所有者随后批准推送 `9e9fe2215abe0c9db6ea33df492af5fe158442b2`。CI **41460223799 / attempt 1** 已完成：Windows、macOS 全部通过、Chromium 各 396/0，
Ubuntu 仅 Chromium 395/1，其他 Rust/合同/构建/平台检查通过；CodeQL **41460221628 / attempt 1** 四语言成功。没有另触发或重跑 workflow。
唯一失败为 640px Settings 的外层滚轮测试：固定中心可命中嵌套水平条。本地确定性回归修前 0/1、修后 1/0，窄修提交 **`46eb6dfa8ad1f8514cd5056dac1f22faa436bc7d`**
只改测试命中点与单轮截图输出，保留全部原断言；最终受影响模块 13/0，生产及构建输入/产物未变，不重复全量本地门禁。
原远端未上传 artifacts，精确命中仍为推断。以上为修复推送前状态，原 CI 失败与各轮证据分别保留，不能用本地 13/0 代替远端通过。
原 Core 131912 持续运行，现有产物复开 GUI 157348（窗口 9966488、可响应）；五组公开状态与上次验证结束一致，Config4/revision166 保持。
一般体验待办不升级为阻塞，窗口记忆未定归属、Portable UI 原生状态与 M11/M12 未覆盖边界继续原样保留。

2026-11-03，所有者明确批准 `46eb6df` 和文档 `c273c76` 的普通快进推送，最终技术候选为 **`c273c76976c711590f6ce0f131bab572019caa50`**。
该 SHA 的 **CI 41477415678 / attempt 1 / success** 三平台完整 required jobs 全部通过，完整 Chromium 各 **397/0**
（Windows 11.2m、Ubuntu 8.9m、macOS 15.2m）；**CodeQL 41477413640 / attempt 1 / success** 四语言全部通过。
只收集本次获批 push 自动产生的运行，没有手动触发或重跑。旧 395/1、修前 0/1 和各轮定向结果保留，不拼接或回写。
原测试技术阻塞已收束；准确本地证据继续引用 `a6c9bd5` 的完整门禁和 `46eb6df` 的 13/0，没有重复完整本地验证或构建。
原 Core/GUI 保留，最终五组公开状态一致、Config4/revision166 不变；无新增原生自动交互/截图，Computer Use 保持停用。
最终技术结论与分类见 [H7 验收记录](../testing/m7-h7-final-20261102.md#当前候选技术结论2026-11-03)。
当前只等待所有者 H7 两项最终确认及剩余事项处理；窗口记忆未实现且归属未定、Portable UI 原生补验阶段未另定，不擅自划入 M12。
M7 继续进行中，不进入 M8，不自动推送纯状态文档提交，也不为它再制造一轮全量验证。

项目所有者批准先处理当前跨平台测试/启动缺陷与依赖安全问题，再实施既有 H2-B，最后形成
真实桌面候选并停止人工验收；不重开 H2-A/Visual Gate 2，不重规划 M7 或重做 Projects/Packages。
开始时现场核对唯一工作区、实际 Git 状态与当前 HEAD 的 CI/CodeQL；`736f55a`、
CI `41191370985` 和 CodeQL `41191370892` 仅为审阅基线，不要求回退或重复修复。

- Hub 先区分 OS 同步 spawn 错误、OS 已接受和 child 随后退出，证明容量、owned child 回收与
  接收方取消/断线边界；按证据选择 fixture-only 或窄生产修复，保持 shell/参数、权限和进程边界。
- 安全评估基于实际 Cargo.lock、features、平台与调用链，逐项核对上游和 RustSec。先在既有
  依赖评估写明选定版本与理由，再升级当前兼容维护线内必要补丁及精确约束/锁文件；传递变化
  逐项解释。保持 Wasmtime Host-only、ABI/WIT、TLS 校验、ZIP 防御与 Plan/Apply 语义。
  跨不兼容版本、新直接生产依赖、feature 扩大、unsafe 或 native API 变更必须先停受影响项审批。
- 范围内 CI 后续失败可诊断并窄修；不得跳平台/测试、放宽断言、单纯延长超时、无诊断重跑或
  干扰用户进程来取得绿色。逐轮保留失败与修复证据。
- 技术收尾单独本地提交，通过适用本地门禁后明确允许普通 fast-forward 推送 `origin/main`，
  仅覆盖上述修复、获准安全补丁与对应证据。不得强推、改历史、改仓库设置或绕过工具审批；
  工具若仍要求具体 SHA 额外授权，报告 SHA 与范围后停止该推送。
- 同一最终技术 SHA 必须取得 Windows、Ubuntu、macOS 完整 required jobs 及 CodeQL 成功，
  且适用安全问题已修、其他公告有可检查结论。未满足时不宣称 PASS，也不提前改 H2-B 产品。
- 本轮已完成所选 Wasmtime/rustls/webpki/ZIP 维护及 ZIP 类型防御，完整 Rust 491/0/8、前端、
  冻结基线、CLI 并发启动与 release/no-bundle 通过。首个技术提交 Chromium 237/10 的每项均在
  文档或必需静态 JS 请求遇到 ERR_NO_BUFFER_SPACE、业务未到达；该门禁仍失败，不能拼接
  前轮或定向结果作为通过。项目所有者已明确批准仅 SHA
  `73f465b4cd0ab28b90794fcaac94e21db5be0e72` 在保留本机失败记录时普通快进推送，并以同 SHA
  三平台完整 required jobs 与 CodeQL 全绿作为本轮技术收尾通过条件。该 SHA 已核实位于
  `origin/main`，CodeQL 四语言已成功；Windows 完整成功、browser 247/0，macOS Hub 已通过但后续
  候选 RPC fixture 长路径失败，Ubuntu browser 245/2。后续修复沿用既有 macOS 短临时根、修正
  Select 内容宽度及表格测试的剩余空间前提，保留全部原断言；完整本地 Rust 491/0/8、布局专项
  3/3、根前端检查/构建与 release no-bundle 通过。后续候选完整 Chromium 为 246/2，分别为
  30 秒预算与标题等待失败，150 个输入未变，不能拼为 PASS。项目所有者另行明确批准后续精确
  SHA `02b6f0c3e81b8eedb711123ef9de2edc228792ab` 在保留失败记录时普通快进推送，该 SHA 已在
  `origin/main` 核实；CI `41209763063` 的 Windows/macOS 完整成功、browser 各 248/0，Ubuntu
  为 247/1，唯一失败是 257 目标的 5 秒标题检查。CodeQL `41209762434` 四语言成功；
  大列表候选仅按需创建行菜单、给内容一次绘制机会后初始化滚动容器。相关包管理/滚动条/导航
  63/0、前端 check/build、release no-bundle、xtask 与 metadata 通过；独立完整本地 Chromium
  247/2（18.4 分钟）、150 个输入未变。两项失败为必需 JS 加载与四宽度网格累计预算，未通过
  范围如实保留。所有者另行明确批准精确 SHA `1932140a815dd12712cd00108d70acebe51a3560`
  保留失败后普通快进推送，已在 `origin/main` 核实；CodeQL `41221092450` 四语言成功，
  CI `41221092743` 的 Ubuntu/macOS 完整成功、browser 各 249/0；Windows browser 248/1，
  唯一失败是正常动画分支的一次性滚动条样式读取。后续测试候选在默认断言时限内等待实际样式，
  增加 property 校验并保留原阈值；动画专项 9/0、前端与 metadata 通过，独立完整本地 Chromium
  248/1（12.6 分钟），150 个输入未变。唯一失败为必需 JS 的 ERR_ADDRESS_IN_USE，Hub 业务
  未到达；未重跑、改 Hub 或扩大预算。所有者已明确批准精确 SHA
  `97c761f76613ea68ebc7a93e4695940dabb969dd` 保留本机失败后普通快进推送，已核实位于
  `origin/main`；CodeQL `41228566315` 四语言成功，CI `41228566567` 三平台完整 required jobs
  全部成功、browser 各 249/0。同一最终技术 SHA 的集中收尾检查点已通过，H2-B 按既有授权
  开始实施；前述本机失败记录保持。详细证据见
  `../testing/m7-technical-closure-20261031.md`。
- 满足前提后 H2-B 自动获准开始，无需再询问。H2-B 允许本地候选提交，禁止自动推送；
  当前 release GUI、匹配 daemon 和隔离样例必须走实际写入闭环，最后交截图/清单等待人工验收。
- 项目所有者于 2026-11-01 明确要求将本轮 Chromium 门禁修复与相关文档用真实 Windows
  用户环境提交并推送；允许将包含既有 H2-B 基线的当前候选普通快进更新到 `origin/main`。
  该显式授权只覆盖本轮候选，H2-B 与 M7 人工验收仍待定。

## H2-B 资源与模板范围及验收方案（条件批准）

状态：**阶段性人工验收通过，保留体验改进项；本轮体验整改结束**，2026-11-01。
实施前提与推送边界以上节为准；H2-A 检查点为
`23f545245f613f47f8def1e16418c4d12303404e`，Visual Gate 2 已签收。
本节直接细化原 H2 的剩余范围，不新建产品里程碑，不重复实现已存在的入口。
当前阶段仍以 `../status.md` 为唯一入口；技术收尾已在 `97c761f` 完成同 SHA 三平台 CI/CodeQL 检查点。
H2-B 前次运行时代码候选为 `cbb4c4198a21519efb24d8f0a04a509664e83b50`，完整 Rust 492/0/8、
前端/冻结基线/release 构建通过。原两轮完整 Chromium 290/2、283/9 与仅上游缓解的 291/1
独立保留；fixture 连接关闭修正经有界对照后，工作区独立完整 Chromium 为 292/0，11.2 分钟、
652 项输入前后相同，不拼接不同轮次。专项验证基于 `173c956`，测试基础设施修复与相关
文档已提交为 `0ef5b088dfdb7a8eb2b7a88ebef94ae436c90f50` 并普通快进推送 `origin/main`。
隔离 native 已核对 stale 零接受副作用、资源/模板写入、两入口创建、持久化、
Retry 与原环境恢复。builtin confirmed CLI remove 先前自动审批拒绝的历史保持；项目所有者随后
明确授权在既有可丢弃隔离环境补齐：正式 CLI 带 --yes 的唯一请求返回 template_builtin_immutable，
exit 1，七组公开状态与 63 个文件的路径/大小/hash 完全一致，无新增 Operation。
收尾仅补新建项目结果链接与既有键盘合同：真实窗口 `/projects/d138da13-ad72-4de8-b438-d99b70c43f66`
与 Operation 返回 ProjectId、公开查询一致；Tab/Shift+Tab、Enter/Space 符合分组按钮合同，
方向键未改变焦点或路由，无须增加新的交互模式。前次收尾已读回五组原样例状态一致并恢复开发窗口。
完整矩阵与截图身份见 `../testing/m7-h2-b-20261101.md`。同一远端 SHA `0ef5b08` 的
CI `41287607183` attempt 1 三平台完整成功、Chromium 各 292/0；CodeQL `41287607108`
attempt 1 四语言成功。只收集已有运行；运行时、测试与构建输入未改，不重复完整本地门禁。
后续实际候选为 `2a163b0`（生产输入 `ceef00f`），已沿用此前成功方式启动匹配 release GUI/Core。
项目所有者本次确认 Resources 仓库/列表交换、用户包、模板和键盘/弹窗体验的阶段性人工验收
通过，允许结束本轮整改；文案不准确、冗余说明、信息位置和过度设计保留为现有 status 待办。
不宣称体验完善、所有问题解决或整个 Visual Gate 3/M7 完成，H2-A 原签收保持。
隔离 11 个任务已完整分页确认终态后，按授权关闭本轮窗口并实际恢复原开发 Core/GUI；五组
公开结果与暂停前一致、settings revision 155。新代码的远端验证仍未取得，不借旧 SHA 替代。
本次仅允许本地文档提交，不新增推送授权；不将此前截图/工具缺口追写为已执行。

### 本轮人工反馈整改与仓库列表窄合同

项目所有者实际查看后确认：流程可用，但 Resources 提示挤动布局、提醒位置混乱，体验未通过；
仓库列表导入/导出是冻结 v3 用户能力的实际缺口，本轮补齐，不另立 M7 规划或进入后续阶段。
先在真实 release 窗口保存完整正常/成功/失败状态截图，再修共享提醒框与调用位置。
短暂成功使用不占布局的轻提示；字段错误靠近字段，操作错误留在弹窗/对应上下文，风险与部分
失败持久显示；进度仍由既有机制负责。关闭、取消、重试清理旧状态，长内容可换行/展开/滚动。
共享组件影响其他页面时补关联定向回归，不重做已签收的 H2-A。

实施前复用审计：冻结 v3 `4aa98ae` 的仓库列表规范为 UTF-8 文本、每行 URL、`#` 注释、空白忽略，
支持 http(s) 与 `vcc://vpm/addRepo?url=...&headers[]=...`。当时 Core/client 已有 inspect、分页
list、register、refresh、unregister，尚无列表文件读写合同。GUI 已依赖 dialog 插件，已有目录
选择与 Hub 文件打开，可复用为薄打开/保存适配器，不在 Tauri 或 GUI 中实现仓库登记权威。

公共差异（**项目所有者已批准以下修订合同，已在本地实现并取得本轮阶段性人工签收**）：

| RPC | 输入 / 输出 | 权限及边界 |
| --- | --- | --- |
| repositories.inspectListFile | `{path}` → 有界条目 `{line, source?, status, reason?}` 与忽略行数 | local-owner + 既有 repositories.read；只读 UTF-8，最多 1 MiB / 4096 条，超限整体拒绝，不写 registry，不自动下载 |
| repositories.previewListExport | `{}` → 全量登记快照的可导出数量、排除项及 fingerprint | 既有 repositories.read；Core 从一致只读快照取得全量，不受 GUI 当前页限制；返回排除原因，不返回认证秘密 |
| repositories.exportListFile | `{targetPath, expectedFingerprint}` → 实际导出数量及摘要 | 认证 local-owner + 既有 repositories.read；重验预览内容与范围的全量快照，变化则在创建文件前拒绝；仅创建全新文件，拒绝 Core 数据/已登记项目内目标，取消选择不调用 |

新增 capability `repositories.list-files.v1`，沿用既有权限；补 RPC Schema/SDK/兼容快照，
不改 State Schema，不引入新生产依赖、批量事务或通用工作流引擎。错误用有界稳定 code，
不将原始无效行、headers 或 URL 凭据写入日志/普通错误。

最小用户行为与文件边界：

- 工具栏 Add repository 的 split menu 提供添加、导入列表、导出列表，不新增导航。
- 复用 v3 普通 UTF-8 URL 列表，支持 CRLF/LF、可选 UTF-8 BOM、注释及不带 headers 的 vcc 添加链接；
  不是 repository manifest、package 导入或旧数据库迁移。带 headers/userinfo 的条目明确列为不支持，
  不降级为匿名条目，不显示其秘密；其他 scheme、无效行都给行号和原因，不静默忽略。
- 导入先预览并选择，再调用既有 repositories.inspect 检查可访问性和现有来源规则；重复 URL/ID、
  无效或不可访问项留在结果中。显式确认后按所选项逐条调用既有 register，默认追加，不清空或替换。
  不是原子事务；取消停止调度新条目，在途请求的实际完成仍计入，分别展示成功/失败/未处理/未知。
- 导出预览和实际文件覆盖 Core 全量登记快照，不能只导出 GUI 已加载的一页。保留 Core 已验证的
  完整 http(s) URL，包括 query 的字节顺序与内容，不删除、重排或因存在 query 而排除。
  预览明确提示文件包含完整地址，地址自身可能携带凭据、分享前需检查，显式确认后才导出。
  不读取凭据库、不附带 headers/密码/token，不承诺 URL 已被认证为无秘密，不建立按参数名猜测
  所有秘密的系统。当前无法表达的认证来源和本地来源明确排除，逐项说明，不称作完整备份。
- 当前 Core 没有内置仓库记录类型或预置仓库注入；导出当前实际登记的远程来源，不按 URL 猜测
  builtin，也不凭空加入 v3 官方/精选默认仓库。本地来源不伪装为 URL。
- 原生打开/保存只选择路径，Core 负责格式、全量查询和文件写入。保存只创建新文件，已有文件
  明确报冲突并允许另选路径，保存弹窗不承诺覆盖。失败不留下可被当作成功的半成品，且不能清理
  非本次创建的文件。普通成功轻提示，排除项/部分失败持续保留供用户处理。

实现为 application 的 RepositoryListApplication、Store 的一致只读投影与独立 VPM 文件适配器；
通过现有 RPC/client/SDK 接入 GUI。当前 dialog 插件不能关闭系统保存框的覆盖确认，保存界面因此
使用新文件名字段和已有原生目录选择器，Tauri 仅组合用户选定路径；打开复用原生文件选择器。
保存不显示系统的替换文件承诺，最终目标、冲突及路径保护仍由 Core 判断。列表和错误正文滚动，
操作按钮使用 Material Dialog 公开 actions slot。未增加生产依赖或持久任务。

该修订合同已明确获准，先同步本节及 RPC Schema，再实施，不再追加纯文档审批。
独立共享提醒框组件同样已批准；各信息性质使用合适承载，避免一律浮动或重复反馈。

### 目标与现状证据

建议 H2-B 收口 Resources 内 Repositories、User Packages、Templates 三个既有分区，以及它们
连接到 Projects 的模板创建流程。保持现有导航、1400×800 桌面基线、已签收密度与共享 Material
组件，不重做 Projects/Packages，也不以资源子系统划分新的一级工作区。

| 现状 | 已核对证据 | H2-B 处理 |
| --- | --- | --- |
| 仓库 inspect/register/refresh/unregister、包预览与分页已接线 | `CorePages.tsx` 的 RepositoriesPage/RepositoryPackageList；`CoreActions.tsx` 的 RegisterRepositoryPanel/RepositoryActions | 复用现有功能，补齐异常、并发 revision 与真实桌面闭环证据 |
| 用户包登记、刷新、取消登记及行内详情已接线 | `CorePages.tsx` 的 UserPackageEnroll/UserPackageManager/UserPackageDetails；`rpc.ts` 的既有 userPackageGet | 详情读取当前 Core 记录，关闭隔离旧响应；资源与相关包来源 18/0；当前 release 的真实 chooser、详情、显式刷新、取消/确认移除与重启持久化已记录 |
| 模板导入、派生、收藏、导出、移除、创建项目都已有入口 | `CoreActions.tsx` 的 TemplateImportPanel/TemplateDeriveDialog/TemplateActions；`ProjectCreationDialogs.tsx` | 修正当前流程缺口，不把这些功能重新列为“从零实现” |
| 模板列表与创建选择器已接通后页 | `rpc.ts` 保持 limit=100 并透传 cursor；TemplatesPage 复用 PagedItems，CreateProjectDialog 保留后页失败时的数据/选择并隔离关闭后的响应 | 101 项、创建导航、失败重试、旧响应隔离和延迟 Retry 的模块回归 14/0；证据见 `../testing/m7-h2-b-20261101.md`，不替代最终候选或 native 验收 |
| 模板任务由当前页面持有并在成功终态刷新 | 可控延迟已复现关闭失去跟踪/初读错误停止等缺口；复用 OperationFollow，行 controller 位于 PagedItems 外 | 保留多个显式接受 ID；成功才刷新，旧任务不覆盖新草稿；完整整轮与定向结果分别见 testing 记录 |
| 三类模板 review 已使用现有白名单 evidence | `TemplatePlanReview.tsx` 校验顶层字段并绑定读取/请求上下文；两个创建入口共享规则 | 缺失、错误类型、身份/digest 不匹配不开放 Apply；不显示 filesystem identity，不扩充公共字段 |
| 浏览器已补真实 wire、分页、任务交错及资源生命周期 | 新模板/资源/shared-follow specs 和原 harness；规范 UUID、顶层 Plan、受控 Running/terminal 和有状态 CAS fixture | 保留全部既有断言；模块 14/0、资源 18/0、普通模板 6/0 分别记录；34 项扩展组的 30/4 不合并为完整通过 |
| H2-A 真实桌面材料没有这些资源写入闭环 | `../testing/m7-visual-gate-2-20261031.md` 的实际范围 | 新建 H2-B 独立证据，不用 H2-A 的 GUI Apply 或 CLI backend 测试冒充 |

上述 GUI 缺口以浏览器受控场景复现和验证，尚不作为 native 故障或真实写入通过证据。
v3 行为仅使用冻结审计和布局对照，不复制源码。

### 建议实施范围与顺序

1. **模板读取与页面状态**。只为 existing `templates.list` 接通 cursor，使用既有 PagedItems
   或等价的现有分页交互，覆盖 Templates 列表和 Projects 的模板选择器；保留 Core 顺序与
   opaque cursor，不解析游标、不提高 limit 规避分页，也不静默把未加载项当作不存在。
   失败保留已取得的数据与选择，明确提供再次读取；切页/关闭后不接受旧请求结果。
   TemplateDeriveDialog 已有项目分页，验证它能选择后页项目即可，不另建项目列表。
2. **模板确认、任务与结果**。复用当前 import/derive/create Plan/Apply、OperationFollow 和
   shared Dialog。review 至少展示 action、TemplateId；import/override 显示 expectedRevision
   与是否替换；derive 显示源 ProjectId/revision；create 显示 templateRevision、targetLeaf
   与 targetMustBeAbsent。都从该 Plan 现有 evidence 读取，不显示 filesystem identity 的原始字节。
   名称/版本只可使用与 Plan digest/identity 对齐的读取快照作为上下文，不能伪造为 Plan 返回字段。
   TS 对已知 evidence 做窄类型校验，不增加任意 JSON viewer；关键证据缺失时给有界错误并阻止 Apply。
   Apply 后跟随同一个 OperationId，成功终态再刷新实际列表/项目；失败、取消、断线不能报成功。
   stale/conflict 明确显示，不自动重新 Plan 或重发 Apply。两个创建入口保持同一 Core 用例与结果身份，
   用户可定位 Core 注册的新项目，不建立第二套项目创建/Operation engine。
3. **资源动作闭环**。仓库继续使用既有 URL/local manifest 输入、inspect→确认→register、
   refresh、unregister 与包预览分页；用户包继续使用既有目录选择器、enroll、refresh 和确认后 remove。
   用户包详情在同一 Resources 工作区的行内弹窗内读取 get，显示名称、packageId/version 和来源目录，
   revision/hash 只作次要详情；不暴露 object-store/cache 路径或改成新一级导航。
   取消选择/确认不写入；并发变化要求显式重读，删除仅取消登记，不删除源文件或已安装包。
4. **验收与材料**。完成下面的自动化及真实桌面矩阵，沿用共享 loading/last-known-good/error/
   reconnect/capability、focus/menu/select 规则。只有因范围内失败而需要的共享组件窄修正可进入本 slice，
   并回归已签收的 Project/Package 路径；不借此重构全部页面。

资源 URI、RPC capability、权限和存储版本保持。相关 method 分组为：仓库既有七项
inspect/list/get/packages/register/refresh/unregister，用户包既有五项 list/get/enroll/refresh/remove，
模板既有十二项 list/get/inspectBundle/planImport/applyImport/planDerive/applyDerive/export/
setFavorite/remove/planCreateProject/applyCreateProject。Operation 查询/取消仍用原合同；
低影响 command 不创造 GUI Plan 或新 Operation。

GUI production 改动为 `apps/alcomd-gui/src/CorePages.tsx`、`CoreActions.tsx`、
`ProjectCreationDialogs.tsx`、`TemplatePlanReview.tsx`、`rpc.ts`、必要的本地 `core-models.ts` 与三语言词条；
tests/browser 的现有 harness/spec 同步。`UtilityWorkspace.tsx`、`Material.tsx` 等共享层只允许
为上述可复现问题作必要修正。已有 Tauri templates adapter 已接收完整参数，无需新增平台 command。
本轮同时收口真实桌面已复现的模板 Apply 接受边界：`crates/alcomd-store/src/m5_template.rs`
在原有接受事务内重验 import/derive/create-project 的登记 revision、digest 和对象存在性，
先处理已接受请求的幂等重放，再拒绝已知失效且尚未接受的 Plan；最终提交继续复用同一重验。
`crates/alcomd-testing/tests/m5_template.rs` 证明拒绝不产生 Operation/Event/目标目录，不消耗
幂等键，显式新 Plan 仍可创建，已完成任务在模板 revision 改变后仍按原 ID 重放。
这项修复不改变冻结字段、错误码或恢复格式，不删除旧失败任务、目标目录或恢复证据。
本方案不提出新依赖、public DTO/wire、RPC、权限、Schema、migration 或 native API。

### 自动化验收矩阵

| 场景 | 必须证明的结果 |
| --- | --- |
| 模板分页与完整选择 | 至少 101 项、两页不同数据；后页模板可见且可用于创建。请求沿用真实 cursor/limit；翻页不创建 Plan。后页失败保留前页及选择，重读不重复项；空列表与读取失败分开 |
| 模板派生项目分页 | 从源项目后页选择，Plan 使用该 ProjectId 与重读的 revision；关闭/重新打开不继承前次草稿或迟到响应 |
| 三类 Plan review | import/override、derive、create 各使用 Core 实际投影的 evidence 形状；展示与返回对象一致。非法/缺失关键证据不开放 Apply，不读未经批准的字段；不加大公共合同 |
| 模板任务终态 | 可控 Running→Succeeded 后才核对新增/替换对象与版本；失败/取消不显示成功；同一 OperationId、一次显式 Apply。接受阶段的早读不能覆盖终态重读 |
| stale 与未知结果 | Plan 后推进模板或项目 revision；原 Apply 明确拒绝，前后无额外 Operation/文件变化。断线重连跟随原任务，未知提交结果不自动重试 |
| 模板普通动作与边界 | favorite 双向持久化；真实 export 返回成功并有外部 artifact；remove 用户模板及 builtin remove/override 拒绝。导入相同 digest、不同 digest 和明确 override 分别按已有合同处理 |
| 仓库生命周期 | inspect 不登记，确认后 register；重复/无效源有稳定错误。refresh 失败保留 last-known-good，成功后展示当前包；预览后页可达。取消 unregister 无写入，确认只取消 registry |
| 用户包生命周期 | 选择取消不 enroll；有效目录登记、get 当前详情、源变更后显式 refresh；revision conflict 不覆写；remove 只取消登记，源文件不变。缺 capability 不调用业务 RPC |
| 页面与交互 | 各分区 loading/empty/refreshing/error/disconnected；Retry 恢复数据和能力。键盘分段、弹窗 Tab/Shift+Tab/Escape/返回原控件、三语言/明暗及既有 reduced-motion 行为保持 |

现有 browser helper 不得用提前 succeeded、固定一项返回或 synthetic 错误码跳过真实竞态。
同一测试场景的请求序列和数据结果均须可检查；使用公开 Material 生命周期，不访问私有 shadow DOM。
测试先补足能证明缺口的失败用例再修实现，不放宽 30 秒、单 worker、零重试或原断言。

开发中按本文件“验证与 release blockers”的风险分级安排定向及模块回归；相关修改稳定后，
在实际本地候选 SHA 上集中完成主 ExecPlan 的完整本地门禁：fmt、Clippy、Workspace tests、
xtask、metadata、冻结基线、npm check/build、完整 Chromium、Tauri release no-bundle 与 diff check。
该清单不按单个修改或提交重复触发，H2-B 也不自动触发 Hosted CI/CodeQL 或推送。
日志逐轮保存；旧 SHA 的通过、定向复核与完整运行不得拼接，未执行项保持未执行。
仅修改本方案或验证安排时运行文档检查，不重复执行或宣称取得 H2-B 的 Rust/browser/native 验收结果。

### 真实桌面验收方案与停止点

真实验收使用嵌入 production 前端的 release GUI、独立临时 Core 数据目录及 WebView2 profile，
在正式应用用例层内操作。准备只含 JSON、文本和最小项目结构的合成 source、两个有效的同 ID
不同 digest 用户模板 bundle、一个 local 和一个仅监听 loopback 的 anonymous remote 仓库。
它们必须与用户样例、正式仓库、Unity/Hub 安装隔离；模板只用 native `.alcomdtemplate`。
外部输入可在登记前构造，登记后只经正式 CLI/GUI/RPC 修改；不直接写数据库、cache 或已注册项目。

真实 GUI 至少走完：

1. 仓库 preview/确认添加→包预览→refresh 成功与失败的 last-known-good→取消/确认移除；
2. 用户包 chooser 取消与选中→详情→外部合成 source 改动后 refresh→取消/确认 remove，
   用只读 hash 核对 source 未被删除或改写；
3. 用户模板 import→Running/terminal→列表出现，favorite→刷新与重启仍保持；
4. 用户模板 export 到全新路径→Core inspect 验证，直接 export 的 bundle bytes/digest 与已验证对象一致；
5. 同 ID 不同 digest 的导入先拒绝未确认覆盖，再显式 override；builtin 的覆盖/移除仍拒绝；
6. 从合成项目 derive→成功后列表更新，从 Projects 及模板行分别创建两个新项目，
   跟随各自 Operation 并核对 Core 注册的 ProjectId、目标文件与排除项；不启动 Unity；
7. GUI review 后由另一正式客户端推进临时 revision，原 Apply 拒绝；保留前后 Project/Template、
   Operation 列表及文件摘要，证明没有额外副作用；
8. 代表性的深浅色、键盘/focus、断线/Retry 与重启持久化。native 未覆盖的场景逐项记未执行，
   browser 多页/错误注入不得冒充 native 动作。

共享 Windows endpoint 必须先核对归属与可用性，不能误连原样例 Core；占用/初始化失败即停止。
需要暂停既有样例进程时按已有授权核实身份及无活动 Operation，并记录、恢复完整原状态；
不停止用户 Unity/Hub。临时运行时最后关闭，原 Projects/Editors/Operations/Settings 与 revision
前后比对；截图仓库外保存，逐张记实际 GUI hash/构建，技术日志留在 ignored target。

完成后停止在 **H2-B 候选与真实桌面签收**，交付修改摘要、逐项矩阵、日志/输入 hash、
截图和未覆盖项，等待项目所有者验收。这些是 Visual Gate 3 remaining core pages 的资源/模板证据，
不自动签收整个 Visual Gate 3，不重开已通过的 Visual Gate 2，也不自动开始 H3-H7 或 Hosted CI/push。

### 排除项与必须停止审批的条件

- 仓库列表 import/export 已由本轮人工反馈明确纳入 H2-B，新增窄公共合同按上节审批。
  仓库凭据 enrollment/revocation、header/proxy、deep link handler、reorder/cleanup/cache
  等其他缺口保持原边界，不静默加入，也不标记为已完成。
- v3 `.alcomtemplate`/tar.gz、v3 模板编辑/依赖与 UnityPackage parity、真实脱敏 differential fixture
  保持 M11 归属；不在正常 runtime 加回旧解析器。H2-B 只验收现有 native Template Bundle v1。
- 不新增 template 文件打开/保存选择器。现有导入/导出路径输入仍可完成流程；新增 native picker、
  文件关联或系统 opener 是独立方案与审批点，不因已依赖 dialog plugin 就自动获批。
- 不新增 source renderer、workflow engine、通用数据表框架；不扩大窄窗口/窗口记忆、H3 Unity/Backups/
  Operations 增强、H4 Portable UI、H5 utilities、M8/M9、安装更新或三平台辅助技术范围。
- 若现有 evidence/Operation 结果不足以安全完成上述确认或定位，或实现需要 public DTO/Schema、RPC、
  capability/permission、持久格式、依赖或 native API 变更，先停止对应步骤并提交具体补充方案；
  不能根据表单内容伪造 Core 证据、直接读项目文件、加别名/双格式或放宽权限来绕过。
- 聚合 feature 仍保持实际 in_progress；M11/M12 未完成项与 Hub 下载/native Portable UI 的旧未覆盖
  范围不被本次提案或资源验收提升为通过。提交、推送、Hosted CI 和发布仍按各自授权执行。

### H3 已批准切片：已有能力复核与直接接线

项目所有者在 H2-B 阶段签收后明确批准现有 H3 直接实施；上节 H2-B 的范围排除不再阻止本切片。
交接已完成：原开发 Core/GUI 已实际恢复，Projects/Editors/Operations/Settings/Status 五组完整公开结果一致；
仓库列表三个 RPC 已补入既有 GUI 矩阵，公开方法 92/92 对齐。无需另设纯规划或交接人工停止点。

| 已有流程 | 本轮只补的缺口 |
| --- | --- |
| Settings 已有 Unity Hub、安装列表、登记/发现/移除；项目已有精确启动、启动参数、Unity 迁移 Plan/Apply | 接入共享成功轻提示、上下文错误与请求生命周期；本页接受启动后从现有更多菜单按需读取/刷新该启动记录，校验 ID/项目/安装身份，清理迟到结果；不重做启动或 Hub |
| 项目已有备份创建、备份详情与恢复入口 | 创建接受后将同一 Operation 留在项目备份上下文，终态刷新历史；接通现有备份 cursor；详情复用 Projects 恢复弹窗与所选备份，移除重复恢复面板 |
| 已有 OperationFollow、任务中心、任务详情、协作取消 | 任务列表接通现有 cursor；详情复用同一跟踪组件持续读取、明确失败/取消状态及 Core 结果入口；取消错误留在确认上下文 |
| Projects 已有托管备份恢复 Plan/Apply | 复用已有 OperationFollow，已知 ID 读取失败持续跟踪，不能再次 Apply；未知写入结果保留未知、禁止重复提交；补全备份选择分页 |

不新增公共 RPC、Schema、权限、依赖、持久任务或通用工作流；GUI 不读写数据库或项目文件。
先运行受影响 GUI 检查与定向回归，稳定候选再集中验证和真实隔离桌面查看。主体 `ef47c25` 已取得
30/0 与真实隔离备份/恢复/反馈证据；启动状态补齐后的最终代码 `ab5d183` 已取得独立 **34/0**、
GUI check（918 条翻译）与更新后的 release GUI。备份/任务等 16 个相关函数不变的比对证据保留，
原生截图仍绑定实际 `ef47c25` 构建；Esc 后新增启动状态弹窗未做原生操作/截图。原环境已实际恢复，
五组完整公开状态一致、revision 155。详见 [H3 候选与桌面证据](../testing/m7-h3-contextual-20261101.md)。H2-B 新增代码及 H3
仍没有对应远端 CI/CodeQL，旧 `0ef5b08` 的通过只对应旧输入。H3 的停止点是本地候选、证据、真实窗口
和简短清单交由项目所有者验收；当时 H4-H7、推送与 M7 整体完成均未授权。当前 H4 的后续批准见下节，H5-H7 与推送仍未授权。

2026-11-02 所有者暂不签收 H3，要求优先修复当前 Windows Editor 检测。现场原开发数据的登记为空，
但默认 Hub 安装根存在 `2022.3.22f1`；真实 Package Manager 清单不含 Editor 顶层版本，旧验证却依赖人造
`{"version":...}` 形状并静默丢弃失败。此轮只修现有验证/发现链与直接误报：Windows 从 Editor 本身的
ProductName/ProductVersion 资源读取，复用现有 `windows-sys` FileSystem 功能，不执行 Editor 获取版本，
不以目录名推断、不新增依赖或公开合同。窄 version.dll FFI 只在 platform 私有模块局部允许 unsafe，
每处说明指针、缓冲区、对齐与生命周期，输出仍经 Core canonical version 校验。发现失败明确返回既有错误，
技术日志保留完整路径和原因，GUI 不把版本无法验证说成未安装；先补真实元数据形状的失败回归。
Hub 自定义根尚未覆盖的能力不作为本机故障原因，也不借此扩大实现。既有 exact launch、Plan/Apply、
无持久 Editor 偏好与不干扰用户 Unity 的边界不变，其他非阻塞体验只保留现有待办。

该修复已本地提交为 `efdb133`：platform 8/0、daemon 3/0、exact version 1/0、RPC 1/0、Unity 浏览器
6/0 与受影响 Clippy、GUI check、Core/GUI release 构建通过。真实原环境的正式 CLI 自动发现及再次刷新
均得到同一 verified `2022.3.22f1`，两个样例的 exact candidates 匹配；没有手动登记、启动或迁移实际工程。
原数据 Core 已恢复，更新前后五组公开状态一致；发现后 Editor 登记按预期从 0 到 1，其他四组保持、revision 155。
physical Escape 后未再进行桌面输入/原生截图，新 GUI 已启动可响应，真实安装页显示仍待所有者确认。
本轮停止点只核对这一项，不重新验收所有 H3 页面，不以 CLI 成功替代原生 GUI 验证或 H3 签收。

所有者随后确认 Unity 已正常检测/显示、刷新保持同一条，检测功能项通过；H3 整体仍未签收。
新反馈是 Windows `\\?\` 内部路径前缀直接显示。项目列表已有私有格式化函数，其余只读路径未复用。
本轮把它收束为 GUI 的单一 `displayPath`，接入安装列表/完整路径 tooltip、启动选择与已有项目/来源/目标审阅。
仅格式化已知盘符和 UNC 展示，保留 Volume GUID/设备等特殊命名空间；RPC、编辑状态、文件操作与 Core 身份
继续用原值，URL 不按文件路径处理。不修改 Core、Schema、权限、导航或页面布局，只补路径和参数分离的定向回归。

路径整改已本地提交为 `fec8117`：独立定向浏览器 10/0（1 worker、0 retries）、GUI check 及 GUI release 通过，
历史 8/2、9/1 保留。真实窗口已保存前后完整截图，刷新后仍为同一条 `2022.3.22f1`，显示普通盘符路径；
前后完整安装 RPC 结果一致，Core 原进程/数据未变，没有操作用户 Unity。GUI 新产物为 `cc4f…9808a`，
Core 继续使用 `efdb133` 的 `ef371…1b5ca`，不重复 Rust/浏览器全套/三平台 CI。
所有者随后明确确认“路径显示正常了”，并进一步给出 H3 阶段性人工签收，保留现有非阻塞体验问题；不重复验收。

### H4 已批准切片：复用审计与实施边界

所有者在上述 H3 签收后明确批准 H4 直接实施，不另设纯规划停止点。交接时原开发 Core/验收 GUI 在原数据环境，
五组公开状态已完整核对，任务全终态，无隔离占用需要恢复。现有扩展卡片、安装/卸载 Plan、启停、权限变更与
17-node Portable UI renderer 均已有接线，继续保留；只补管理流程、分页、反馈/进度和异步 session 生命周期。
安装输入的手填 registry revision 说明与 Core 不符：现有安装只接受新 ExtensionId，`expectedRevision=0` 表示
预期缺席，已有安装由 Core 拒绝；按该既有语义修正 GUI，不新增 registry 读接口或绕过版本校验。
权限/作用范围与安装审阅如发现公开只读信息不足，提交具体最小合同审批，仅暂停该部分；其余流程继续。
Portable UI 的文档、草稿、action 与 session authority 保持原合同；不增加页面发现、额外窗口或第二套任务机制。
稳定候选集中做受影响浏览器/GUI 检查与必要构建，真实窗口交付实际范围、未覆盖项与简短清单；H4 人工签收由所有者确认。

#### H4 权限只读缺口：已批准的最小公共合同

现有 `extensions.get` 只有 grantRevision，不返回权限声明或已授权 scope；公开接口只有 set/revoke，GUI 无法
如实展示“已授予哪些项目”。安装 Plan 内已有冻结的 required/optional permissions，但 RPC 投影未返回，无法审阅。
已按人工批准增加以下只读信息，复用既有 Core 表与权限权威，不改 State Schema、Portable UI 协议或运行时：

- `ExtensionPlan` 兼容增加 `requiredPermissions` / `optionalPermissions` 数组，直接投影冻结 Plan evidence；每组沿用
  Manifest 的最多 64 项、每项 128 字符约束，不以 GUI 表单/猜测代替。新 GUI 缺少这些证据时明确提示审阅不可用。
- 新增 `extensions.permissions.list({extensionId,cursor?,limit?})` 与 capability `extensions.permissions.read.v1`，
  需要现有 `extensions.read`、当前 Principal 对该 ExtensionId 的所有权；不新增 permission 或扩大 grant 能力。
  返回 `{extensionId,grantRevision,requiredPermissions,optionalPermissions,grants,nextCursor?}`；grants 只列当前有效的
  `{permission,resourceKind,resourceId}`，不返回历史撤销、凭据、Host 身份或数据。默认 100、最大 1000 项，按三字段
  keyset 分页；游标绑定 ExtensionId、grantRevision 及已安装包快照，读取同一事务内的声明和有效 grants，授权或包变化用既有
  `revision_conflict` 拒绝后续页，要求重读，不能拼接不同 revision 的授权集合。
- GUI 在既有详情/权限弹窗显示声明、实际权限和具体项目范围；set/revoke 继续使用返回的 grantRevision 显式提交，
  不自动授权、不伪造空集合、不提供通配范围。本提案不增加 marketplace、升级、外部服务权限或 M8/M9 产品功能。

所有者已明确批准直接实施本合同，先同步 Schema/测试，不再停一轮纯文档审批。声明来自已冻结 Plan evidence，
不在审阅时从可变来源重读；实际 grants 只来自 Core 当前权威记录。GUI 用清楚名称和实际范围区分“申请什么”与
“已允许什么”，不堆原始 JSON/revision/权限字符串，不增加持久查询会话或缓存；失效读取不能覆盖新状态。
定向覆盖无/部分授权、项目 scope、未授权访问、分页中撤销或包变化、Plan 声明不漂移，以及 GUI 读取失败/刷新/变更后展示。
新增代码没有 Hosted CI/CodeQL 结果，不复用旧 SHA 作为本轮通过；不推送、不进入 H5-H7。

#### H4 本地候选与实际证据

Core/公开合同提交 `0f8c9f4`、GUI 提交 `88c3f69` 已完成范围内接线，最终代码候选为 `e2b7ddd`。安装只审阅冻结权限申请，权限管理读取
当前 grants 并展示具体项目，撤销/包变化使分页快照失效；写入仍用既有接口及调用时校验。列表分页保留已加载项，
安装/卸载跟踪同一 Operation；隔离恢复显式执行，不误报成功。共享 Material Switch 修复受控状态恢复时原生勾选不同步，
关联焦点和 H3 路径回归通过；Portable UI 保留原协议，按 client/session 隔离异步结果与草稿，不重放未知写入。

GUI 独立浏览器 36/0，155 项输入不变；相关 dispatcher/协议/契约、七 crate Clippy、SDK/GUI check、
xtask、初次 Core release 与一次 Tauri release no-bundle 通过。旧失败与修复定向运行独立保留。
真实隔离 Core/Host 验证了签名安装、授权/撤销、UI 同连接操作、崩溃隔离及 retain-data 卸载，样例四文件未变。
初次跨连接 UI 操作的 session_not_found、证据 helper 误写旧授权方法名的 method_not_found 均保留，修正测试入口后
单独验证；生产 GUI/client 原有方法名没有该错误。故意 trap 的恢复仍为 extension_crashed，保持隔离，未宣称健康。

真实 Core 反例暴露 required 未授权仍成功写 enabled；另一定向反例证明 setGrant 能借用历史包已不再有效的声明。
两者均属功能正确性缺陷，`e2b7ddd` 在原 Store 事务复用当前安装包的冻结证据，required 缺失在写入前拒绝，
授权也只认当前声明，不改 Runtime/公共合同。修前 1/2、0/1 和中间 12/0 独立保留，最终 M6 Store **13/0**、
Store/应用 Clippy、静态 xtask 通过；因实际生产输入变化仅再构建 Core。新隔离 Core 验证 permission_denied、
完整公开记录不变、拒绝不消耗 key、未声明授权拒绝、UI-only 无自动授权及同连接 UI 正常。
GUI/客户端/协议输入不变，复用原 GUI 构建和 36/0，不重复 Tauri/浏览器或完整 Workspace。

前轮原环境已恢复到当时 Core/GUI，五组完整公开状态一致、revision 155；当轮隔离进程均已收束，无用户 Unity/不明进程被处理。
Esc 中断后未完成新增原生 GUI 交互及整改后截图，不将浏览器/Core 结果冒充 native PASS。当前状态为本地候选待人工验收，
H2-A/H2-B/H3 签收保持，M7 继续进行中。完整构建摘要、历史失败、实际窗口及四项清单见
[H4 本地验证记录](../testing/m7-h4-extensions-20261102.md)。最终文档整理只运行 metadata/diff，不重新构建或全量测试。

所有者随后要求补齐实际可操作的隔离样例，并明确保持 Esc 后桌面控制停用。本轮复用同一签名正常包、已有隔离项目和
`data-native`；经正式 Core 完成正常包 retain-data 卸载/重装/启停准备，最终留在未安装状态供人工从安装审阅开始。
固定本地 `H4-Review.ps1` 已实际 Start→Restore 核对五组一致，再 Start 交付隔离 GUI；不调用桌面输入接口，
不向原环境装样例、不改产品代码或协议、不重新构建/运行测试套件。原环境当前为人工查看临时暂停，验收结束必须
实际 Restore 并核对，不能继续沿用前轮“原窗口已打开”冒充隔离候选。入口、短清单、现有权限证据与未制造状态见同一 testing 记录。

随后截图中的 `extension_publisher_confirmation_required` 暴露共享错误缺少用户说明，所有者要求处理多处原始返回显示。
`52ed23b` 只修 GUI 提示：有限稳定码映射、中英日文字、默认折叠技术标识及必要公共/资源接线；原始 message/data 不进入正文，
取消、重试、进度、未知结果和信任/签名/权限逻辑保持。关联首轮 78/1 后，唯一测试焦点顺序问题定向 6/0，逐轮保留；
check 和必要 GUI 构建通过，Core/协议/依赖未变，不重跑 Rust、浏览器全套或远端 CI。当前隔离窗口已更新，原生交互仍待所有者查看。

## Progress log

- 2026-08-24：M6完成；M7最初研究WebView-based UI。
- 2026-08-24至2026-08-25：iframe CI `32752875840` 与Windows managed-child diagnostic形成rejected-design evidence，
  没有production security/compatibility success。
- 2026-08-25：项目所有者批准GUI-neutral Portable UI architecture reset；commit
  `12089661f7c694f059aaf172344809ff814716f6`。
- 2026-08-25：独立cleanup commit `c05c5d2c712dd7a791401efc22b8dd19b88023a6`删除obsolete test-only harness与CI
  probe，保留历史evidence与普通Tauri gates。
- 2026-08-25：形成本Portable UI Stop A proposal、synthetic fixtures与contract tests；active/production未修改。
- 2026-08-25：项目所有者认可Stop A总体架构方向，并要求完成单一隐式页面、required guest-ui、exact replay/form/tree、
  InvocationContext分类、State v9最小列与EOF格式的窄review closure；production仍未批准。
- 2026-08-25：项目所有者最终批准 Portable UI Stop A production implementation；独立执行语义补充 commit
  `41f049365894f347d59b44eb7a41ac41c95e64df` 冻结 InvocationContext、render purity、close/replay/race/locale 与脱敏边界。
- 2026-08-25：Slice B0 完成 active Manifest/package profile/WIT ABI v1 direct replacement、State v9、immutable typed
  `ui_protocol`、`extensions.ui.use` 与四个 RPC DTO foundation；backend fixture 使用同一 mandatory guest-ui world，未保留
  Web UI compatibility path。
- 2026-08-26：Slice B1 完成 daemon-memory-only `UiSessionCoordinator`、InvocationContext exact echo/capability matrix、
  real guest-ui Host binding、interactive lifecycle、dual authority、render purity、current-only replay、monotonic snapshot、
  disconnect/disable/digest/grant/generation invalidation 与 schema/capability advertising。真实 daemon/RPC/Host 测试证明
  UI-only open/close、background Host 保留、render write 不落库、action write 落库、cross-connection denial、disconnect cleanup、
  malformed document terminate/crash/quarantine；未开始 official renderer。
- 2026-08-26：Slice C 完成 official main-shell route、五个窄 typed Tauri-to-client adapter、全部 17 node/2 action 的
  exhaustive React renderer、memory-only revision-bound form draft、host-owned identity/error/discard confirmation、
  light/dark/system/source-color/density/locale、320 CSS px/reduced-motion/focus/ARIA 边界。共享 MCP/Discord Fixture 在 Node 24
  真实 consumer test 中通过；未增加 dependency、lockfile、unsafe、platform API、Tauri capability 或 private Host channel。
- 2026-08-26：Slice D 完成独立 non-Tauri headless consumer与共享 Fixture semantic conformance、hostile document/action
  matrix、session exact cap/rate/timeout/u64 overflow、Host binding forge/stale rejection、dual-authority route永久门禁和敏感
  payload无日志sink/close后不可访问证据；复用真实 daemon/RPC/Host lifecycle、Store scope/revoke及first/third parity测试，
  未增加 dependency、lockfile、unsafe、platform API或公共合同。
- 2026-08-26：最终完整本地 fmt/clippy/workspace tests、xtask、metadata、baseline freeze、npm check/build、Tauri release
  no-bundle 与 diff/lock/authority gate 全部通过，形成等待项目所有者明确 push 批准的本地技术候选；尚无 Hosted CI 或
  最终人工验收证据。
- 2026-08-26：Portable UI candidate `aa1323430252ed21995284a7b36dd36e45a15e0a` 正常 push 后由 Hosted CI
  `32877438910` 在 Windows Server 2025、Ubuntu 22.04 与 macOS 15 arm64 全部验证成功；completion audit 未将其冒充完整
  M7 acceptance。
- 2026-08-26：项目所有者确认 architecture reset 误删 official GUI scope，批准恢复 E1-G3、四个新 RPC、四个 permission、
  Config Schema v1，并将 v3 differential parity 与三平台 screen-reader smoke 分别归属 M11 与 M12。
- 2026-08-26：唯一新增前端测试依赖 `@playwright/test = 1.62.1` 获批；Chromium-only browser automation 不进入 production
  bundle/runtime，也不作为 WebView2/WebKitGTK/WKWebView compatibility certification。
- 2026-08-26：Official GUI Completion E1-F3 完成 M1-M7 GUI-relevant Core routes、typed Tauri/client/RPC adapter、
  Plan/Apply/Operation flows、Config Schema v1 durable settings、safe Activity/Diagnostics projection 与统一页面状态；只新增已批准
  的四个 RPC 和四个 permission，没有 State DB 新表、generic RPC passthrough、新 Cargo dependency、unsafe 或平台 API。
- 2026-08-26：G1 Chromium automation 使用 `@playwright/test 1.62.1` 匹配的 Chromium revision `1234`，12 项 browser tests
  全部通过，覆盖 keyboard/focus/dialog/form/live region、320 CSS px、deterministic 200% layout、reduced motion、light/dark targeted
  contrast、全部 17 个 Portable UI node、dirty discard 及 Core success/stale/failed/cancelled flows。该证据只证明 Web frontend
  semantics，不替代 M12 的真实 WebView/辅助技术验证。
- 2026-08-26：G3 本地 fmt/clippy/locked Workspace tests、xtask、metadata、baseline freeze、npm ci/check/build、Playwright、
  Tauri release no-bundle 与 diff/lock/authority gates 通过；production Vite bundle 不含 Playwright，production npm graph 不含
  Playwright。当时 G2 checklist 已准备但尚未执行，最终三平台 Hosted CI 与项目所有者人工验收仍待 push 后取得。
- 2026-08-26：项目所有者实际查看 functional candidate `19267230507071dc61ba306b98c8cfdd113e9ea2` 后，在正式
  checklist 开始前拒绝 visual/layout acceptance；候选未 push。功能、RPC、Config/Activity/Diagnostics、Portable UI、
  Plan/Apply与自动化证据保留，official GUI shell/rendering进入realignment。
- 2026-08-26：只读审计冻结 v3 final source/reference 的 macro layout baseline、v3->v4 mapping、当前逐页 gap、Material Web
  2.5.0 control/theme/integration inventory；新增 A-033 约束 A-020。H0-H7 仅完成规划，production未开始。
- 2026-08-26：项目所有者纠正“参照 v3”被机械解释为固定导航树的问题。A-033 修订为 recognizable structural/product
  continuity；新增 divergence budget 与 Candidate A（project-first）、B（three workspace hubs）、C（bounded Overview）IA
  synthesis。Material Web事实结论、`192672…`功能证据与M7边界不变；尚未选择production target，H0/H1未开始。
- 2026-08-26：项目所有者进一步冻结“v4默认继承v3用户模型”的前提。A/B/C降为historical exploration；建立v3/v4
  user-facing capability map，将M1-M7绝大多数变化归为same/enhanced existing capability，仅把durable task recovery/history与
  extension permission/quarantine视为嵌入既有区域的有限新用户价值。形成单一minimum-change proposal；production未开始。
- 2026-08-26：项目所有者批准minimum-change IA baseline与H0/H1 production，并冻结Project tree只表达语义归属、Portable UI
  无navigation contribution、Task Center为secondary utility、Update仅在真实capability存在时显示。H1后停止Visual Gate 1。
- 2026-08-26：H0以`@alcomd/ui`集中注册Material Web 2.5.0、共享MD3 tokens和窄React 19 facade完成；H1将一级用户区域
  收敛为Projects、Packages & Templates、Extensions、Settings、Log及footer utilities，默认进入Projects并保留全部既有route。
  npm check/build和Playwright 15项通过；等待完整repository/Tauri gate与真实GUI Visual Gate 1签收。
- 2026-08-26：项目所有者在真实 GUI 中拒绝首次 H1 的视觉方向，分类为 `rejected_for_visual_direction`；技术 gate 不冒充
  visual acceptance。停止对该 shell 的补丁式修改，改为以 v3 发布图和冻结 GUI 源码为直接 reference，先形成 Projects 的
  relative geometry、其他主要页面、共同 component/layout vocabulary 与 MD3 translation，再仅重建 wide Projects prototype。
- 2026-08-26：项目所有者以真实 v3 Projects reference 和真实 release GUI 对照，人工确认新的 wide desktop shell、sidebar/
  main canvas、Projects toolbar、桌面密度及 Material Web Button/TextField/Select 的 interaction、ripple/state layer 与 keyboard
  focus；Visual Gate 1 为 `PASS`。独立检查点 `7bb325b78a74518881fb65e03de7f79902e72cd3`，该结论不代表整个 M7 visual
  acceptance 完成。获批下一步仅为 H2-A Project workspace / Packages。
- 2026-08-26：项目所有者批准唯一 production UI asset dependency `@material-symbols/svg-400 = 0.47.0`，冻结
  Material Symbols Rounded/weight 400、default fill 0、selected navigation fill 1、`@alcomd/ui` ownership、显式
  `?url` import 与 CSS mask。动态字符串 registry、remote font/CDN、SVGR 和第二套通用 icon language 保持禁止。
- 2026-08-26：icon foundation 以独立提交 `ea3312b1c10be2f021a5c516cec61449778be761` 落盘；lock closure 只有
  一个零传递 package record。H2-A Project package workspace 已完成本地候选并只增加当前可见 action 所需的十个
  named Rounded glyph。production bundle 精确包含二十个 Material Symbols data URL（十个 navigation regular/filled 与
  十个 H2 action），不含完整 icon corpus、Outlined/Sharp、远程字体或 emitted SVG。npm check/build、Playwright 17项、
  Tauri release no-bundle、fmt/clippy/Workspace tests、xtask、metadata、baseline freeze 与 diff gate 已通过；Visual Gate 2
  尚未人工验收。
- 2026-08-26：项目所有者查看 `1eddee436fcd0343e57d85f300312dab1c97e120` real release GUI 后拒绝首轮
  Visual Gate 2：Material text button 使 sidebar icon/label group 居中，Projects toolbar 的 refresh、view switch 与排序方向仍以
  纯文字表达，偏离 v3 reference 和已冻结 icon contract。修正只改为 left-aligned semantic navigation + MD3 state layer，
  并为实际 toolbar action 接入 named Material Symbols；不改变页面、RPC、状态或业务流程。
- 2026-08-26：首轮 Visual Gate 2 修正候选已完成本地技术验证：npm check/build、Playwright 18 项、Tauri release
  no-bundle、xtask、metadata、baseline freeze 与 diff gate 均通过；shipping bundle 包含 30 个实际使用的 SVG data URL，
  不含 Google Fonts 请求或第三方 package path。Visual Gate 2 仍等待项目所有者复看 real release GUI，不因自动化通过而关闭。
- 2026-08-26：项目所有者复看 `6dd463852227f565b055fdd8ada21575ee9ba6a8` 后确认左对齐问题已修复，但拒绝继续用
  native control + custom CSS 模拟 Material Web component。正式执行规则修正为：只要 `@material/web 2.5.0` 存在适用组件，
  official GUI 和 Portable UI 必须经 `@alcomd/ui` 的窄 facade 渲染真实 `md-*` element；native button/input/select/
  textarea/progress/dialog 均由 source gate 禁止。2.5.0 没有完整 Navigation Drawer/Rail，且 `md-list-item` 不能提供本导航
  所需的 button/`aria-current` 合同，因此 primary navigation 是唯一窄例外：保留 semantic native button，并组合真实
  `md-ripple`/`md-focus-ring`，不再用伪元素或 JavaScript 模拟 Material state layer。该例外不得扩展到其他控件。
- 2026-08-27：项目所有者要求导航实现以 Material Web 官方页面源码而非截图近似为准。只读核对官方仓库
  `cac97678831d48d4eb4a606ca50f92673a1dc20c` 后，确认 catalog drawer 的真实 composition 是 custom `nav-drawer`
  shell + `md-list` + `md-list-item`，并以 12px list/item spacing、28px shape 和 `surface-container-highest` selected
  container 实现。Primary Navigation 因此移除最后一个 native button exception，改由 `@alcomd/ui` 集中注册并由 GUI 窄
  facade 渲染真实 Material List/ListItem；semantic aside/nav shell保留，未采用labs drawer、未新增dependency，也未改变
  route/RPC/state/permission。Material 2.5.0 的 delegated ARIA行为由integration test固定，不以shadow DOM私有结构作为合同。
- 2026-08-27：项目所有者复看后指出连续 navigation items 仍过于松散。进一步核对 Material 3 Navigation Drawer
  component contract，确认 item shape top/bottom inset 为 0dp；Material Web catalog site 的 12px block margin 是文档站局部
  layout，并非必须继承的MD3导航间距。ALCOMD移除连续item之间的额外margin，保留56px component height、28px shape、
  12px drawer inline inset、Material state layer以及独立区域之间的语义分隔。
- 2026-08-27：项目所有者最终选择 v3 final 的实际尺寸作为导航密度基线，并明确包含总宽、高度和间距。只读审计
  `SideBar.tsx` 冻结：260px sidebar、12px padding、4px item gap、48px item height、16px item inline padding、
  20px icon、16px icon-label gap、full-pill shape、14px/20px weight-500 label和
  `surface-container-highest` selected/hover container。v4只覆盖Material List tokens与host spacing，仍使用真实
  `md-list`/`md-list-item`、Material Symbols和既有route/RPC边界，不复制v3 React/Tailwind实现。
- 2026-08-27：项目所有者纠正 icon infrastructure：`@material-symbols/svg-400`/`svg-500` static SVG 固定
  `opsz=48`，均标记 `rejected_as_final_icon_source` 并从 manifest/lock 移除。最终只从 Google 官方
  `google/material-design-icons@e083cc60a0828fdd3b404cea0cb8a5b900e9c23e` vendoring 当前实际使用的 Rounded / weight
  400 / grade 0 / fill 0 / opsz 20 或 24 SVG；`@alcomd/ui` 按 exact optical size 提供闭合 named export，mask 保留
  官方 geometry/currentColor 并删除 120% 裁边及单图标补丁。selected navigation 继续复用 fill-0 symbol，只由
  Material selected container/state 表达选择。
- 2026-08-27：项目所有者在真实 release GUI 中观察后判定 primary navigation 的真实 20px/opsz20 symbols 视觉过小。
  primary navigation 因此整体使用真实 24px/opsz24 SVG，并把 host icon-label gap 从 16px 调整为 12px，使标签起点、
  48px item height 与其他 v3-derived shell geometry 保持不变；没有裁边、scale、weight override 或 per-icon 特例。
- 2026-08-27：项目所有者批准 Projects 列表移除低价值 package count，并以 v3 的添加日期体验恢复真实注册时间。
  RPC v1 只兼容增加可选 `ProjectSnapshot.registeredAtMs`：注册项目返回 store 中既有的 registration timestamp，
  未注册 `projects.inspect` snapshot 省略；不新增 method、capability、permission 或 State Schema，GUI 不得以
  `observedAtMs` 伪造添加日期。
- 2026-08-28：项目所有者在真实 release GUI 中逐项复看 Projects 列表后认可当前局部实现。列表模式只通过表头排序，
  Project 列保留可见最小宽度并在宽屏伸展，其余可预测列与 action region 按最长冻结文案锁定宽度；Added 使用
  `YYYY-MM-DD`，Last observed 使用相对时间。表头与单元格统一左对齐、单行截断，Type 的 24px symbol 与文本基线对齐；
  Open Unity、Manage、Backups 与 Material overflow menu 均保留固定 action footprint。Primary navigation 不提供收起状态，
  窄宽度直接裁切而不产生额外滚动条。本次局部认可不关闭完整 Visual Gate 2，也不批准其他 H2、H3-H7。
- 2026-08-28：Project Actions Functional Parity Audit 经项目所有者接受。确认当前 permanent-disabled visible fake
  entries 只有 Open Project Directory 与 Copy Project，但 Add/Register chooser、Create、Restore、Favorite、Unity
  preference clear、package reinstall/bulk/source/local/prerelease/hidden/docs/changelog、Remove Directory 与 M11 VCC migration
  等真实缺口不能被 fake-entry grep 覆盖。H2 visual work暂停，形成独立 Project/Package Functional Closure Stop A proposal、
  State v10 Copy-only migration contract、fixtures/tests、dependency evaluation 与 milestone ownership；active RPC、production
  migration/implementation和依赖均未改变。
- 2026-08-28：项目所有者附合同修正批准 Functional Closure P0-P4 production。exact opener/dialog dependency、closed
  ProjectId opener/native picker、active Project Copy RPC/State v10、durable inventory/two-pass content verification、forward
  recovery、CLI 与 list/grid/workspace GUI flow 已形成 local candidate；每个 durable Copy phase 的真实 kill/restart matrix 已通过。
  H2 visual 与 P5-P8 继续暂停，完整聚合 feature 状态未提升。
- 2026-08-30：P6 remote checkpoint 已由项目所有者验收为 PASS，sealed HEAD
  `0f83536bdeb74116e4f8d533c9fc3bf5bea529fb`。随后只创建 P7 Delete Project Directory proposal-only Stop A：完成 v3
  exact behavior、现有 v4 primitive、Windows/Linux/macOS delete primitive 与 Rust `remove_dir_all` 审计，提出 ProjectId-only
  Plan/Apply、独立 `projects.delete`、sibling quarantine permanent-delete、State v13 durable evidence 与 recovery/path vectors；
  active RPC、State、Permission、production source、dependency、unsafe 与平台 API 均未改变。
- 2026-08-31：Visual Gate 2 期间重新冻结 Project Unity Version / Open Unity 产品模型。v3 exact behavior 与当前 v4
  Automatic/Explicit preference model 已完成源码审计；Stop A 提出 canonical exact version、0/1/>1 Open Unity、one-shot
  installation chooser、独立 migration Plan/Apply/Operation、State v14 preference removal/launch-config split 与 crash recovery。
  VRChat 2019 -> 2022 当时识别为 secondary preparation 缺口，未扩展 P6。P8 historical PASS
  保留，但当前 completeness 为 `REOPENED_BY_PRODUCT_MODEL_CHANGE`；H2-A 暂停而非拒绝。
- 2026-08-31：Unity model proposal chain `a3faa7b…`、`baa4768…` 的最终 owner review 为
  `PASS WITH IMPLEMENTATION AMENDMENTS`；State v14、exact launch、migration Plan/Operation、private preparation 与窄平台进程
  合同已 seal。contract-only remote checkpoint 全绿后 production 自动获准开始；H2 visual 仍暂停。final amendment 补齐
  Launch Config/launchOptions/required one-shot launch、single-authority migration Plan、downgrade fail-closed、
  `vrchat-2019-to-2022-v1` bounded private preparation、final phases 与窄 UnityMigrationProcess platform contract；
  H2/P8/global 状态不变。
- 2026-08-31：Unity production sealed HEAD `e9349fd0f4cc9c18d9174dfc5826d48654aeb6f5` 已通过
  Hosted CI run `33352406804` 与 CodeQL run `33352406214`，更新后的 P8 M7-owned completeness 为 PASS；
  global 仍为 `BLOCKED_BY_M11`。H2-A 从该 sealed HEAD 恢复 Visual Gate 2；`24b03916…` 只保留为
  pre-Unity-model local checkpoint。

- 2026-09-06：H2-A local candidate 的 shared icon renderer 改为官方支持的 `md-icon` + SVG，
  继续使用 pinned Google Rounded 400 资源；移除 mask 与 wrapper 的 button icon size 特例。
  真实 Windows GUI 操作发现并修复同路由 reconnect 后 capability snapshot 未刷新的问题；
  final browser suite 44/44 与 Tauri no-bundle build 通过。真实 package review / Copy review 的
  布局和 Escape 取消已检查，未执行复制或包移除。窄窗口 sticky action 遮盖数据列仍是未通过项，
  不以自动测试通过替代视觉验收；详见 `docs/testing/m7-icon-reconnect-local-acceptance.md`。

- 2026-09-06：项目所有者纠正 icon integration 范围：不要求窄窗口适配，不授权更换按钮图标。
  撤销 Open Unity 的 `open_in_new`/trailing 替换，恢复原 `play_arrow`/leading；仅保留 `md-icon`
  与官方 SVG 接入修正，不引入响应式菜单或其他布局改变。

- 2026-09-06：项目所有者接受普通 Material Web button 的 18px icon；移除临时双图标比较，
  保持单个原始 leading play。最终前端 check、44/44 browser tests 与 debug no-bundle build 通过。
  computer-use 在隔离 synthetic fixture 中完成真实 Copy 的 review/Apply/成功反馈/列表刷新与
  副本 Manage；源文件未变，副本 5 文件 SHA-256 全部一致。缺失精确 Editor 提示与 Escape 通过。
  本轮没有真实 Unity launch、migration、Hosted CI 或 Visual Gate 2 人工通过结论。

- 2026-09-06：按项目所有者要求继续对照 v3 实际源码，补齐 H2-A 包管理工具栏与筛选入口：
  shared Material search、icon Refresh、maintenance overflow、repository/local/prerelease filters，
  复用既有 settings revision/RPC。48/48 browser tests、frontend check/build、debug no-bundle build、
  xtask 与 metadata 检查通过。桌面新筛选交互尚未完成验收（电脑控制检测到用户输入，已暂停点击）；
  不把浏览器测试冒充 real Tauri evidence。版本选择器、条件更新与批量入口仍是未完成项，详见
  `docs/testing/m7-h2-a-package-workspace-continuity.md`。Visual Gate 2 保持开放。

- 2026-09-17：按项目所有者要求收敛为标准目录单工作树。原 main 的 9 个未提交文件保存为历史快照
  `b4547d5`；整合提交 `9d0ddbb` 保留双方历史且采用 `9307572` 精确代码树，不复活 Automatic Editor。
  两个额外 worktree 已移除，本地材料保存在标准目录恢复归档，旧分支统一归为 `codex/archive/*`。
  本次只做工作空间与记录整合，没有 GUI 新功能、合同调整、push 或视觉验收结论。

## 下一停止点

P6 实际 wire 字段修复与候选 DTO 分离已按项目所有者批准落实，其完整 Rust 483/0/8、公开
RPC 8/8 和六种 Plan review 证据保持。本轮 H2-A 已形成真实桌面签收候选，完成 GUI
Apply/同一 Operation、失效 Plan 拒绝、焦点、筛选、语言/主题持久化及环境恢复。
最终完整 Chromium 在同一输入下独立通过 247/0，零重试；首轮 246/1 网络加载失败及各构建
截图身份独立记录于 `docs/testing/m7-visual-gate-2-20261031.md`，不拼接定向结果。
本地门禁与材料已收口，项目所有者于 2026-10-31 以“可以，接下来做什么”确认签收 Visual Gate 2。
H2-A 验收检查点为 `23f545245f613f47f8def1e16418c4d12303404e`。
技术收尾已在 `97c761f` 取得同一 SHA 三平台完整 CI/CodeQL 成功，H2-B 已按既有授权实施。
前次 H2-B 运行时代码候选为 `cbb4c41`：完整 Rust/构建通过。资源加载专项只改测试基础设施，
独立完整 Chromium 已通过 292/0，修复与相关文档已用真实 Windows 用户环境提交并普通快进
推送为 `0ef5b088dfdb7a8eb2b7a88ebef94ae436c90f50`；原失败轮与网络/root/NetLog 证据保持。
该 SHA 的 CI `41287607183` 三平台及 CodeQL `41287607108` 四语言均在 attempt 1 完整成功。
隔离桌面的结果链接精确导航、既有键盘合同、构建/截图 hash、未覆盖项与原环境恢复已记录。
只补明确缺口并收集既有运行，没有重复完整本地测试或触发 workflow。先前被审批拒绝的 builtin
confirmed CLI remove 已在新明确授权后通过正式 CLI 补齐，Core 拒绝及零副作用证据独立保存，
历史未执行记录保留。项目所有者随后实际查看并拒绝 H2-B 体验签收，指出提示布局和列表交换缺口。
本轮按上述批准合同整改，本地提示提交为 `348dbef`、列表功能为 `ceef00f`；`2a163b0` 只修测试
类型检查，不改变运行时代码。旧 release 构建已独立保留，新候选已在同一隔离数据环境启动。
该候选集中定向浏览器 47/0，相关 Rust/协议/Clippy 及 release 构建通过；未复跑旧完整门禁。
此前因 Computer Use 中断未取得 Agent 的新原生交互/整改后截图证据，历史缺口保留；本次
项目所有者明确确认“阶段性人工验收通过，保留体验改进项”，允许结束 H2-B 本轮体验整改。
未将该签收提升为体验完善、整个 Visual Gate 3 或 M7 完成。按授权实际恢复原 Core/开发窗口，
五组公开状态与暂停前一致、revision 155，不再仅记录守候等待。
获批 G3 的本轮完成度核对已完成，现有 RPC 覆盖矩阵与公开方法 92/92 对齐；方法归属不冒充独立 GUI 动作全部完成。
项目所有者随后明确批准既有 H3，上轮形成 `ab5d183` 本地代码候选：同 SHA 定向回归 34/0、GUI check
与 release 构建通过，主体备份/恢复的原生证据仍绑定实际 `ef47c25` 构建，新增启动状态弹窗未完成原生查看。
原开发环境再次实际恢复，五组完整公开状态一致、revision 155。`efdb133` 修复 Windows Editor 版本读取后，
项目所有者已确认检测、显示及重复刷新；该轮 Esc 后未取得原生查看的历史记录保留。
随后路径显示反馈已由 `fec8117` 共享格式化窄修，10/0 与当前原生安装页/刷新、前后截图和 RPC 不变均已记录。
所有者已确认本次路径显示正常，Unity 检测、显示和重复刷新确认保持，不重复要求查看已通过项。
所有者已进一步确认 H3 阶段性人工验收通过，保留非阻塞体验问题；原环境/验收窗口核对完成，直接推进获批 H4。
H4 已形成本地代码候选 `e2b7ddd`，两处权限权威缺陷已窄修；最终 Store 13/0 和隔离 Core 复验通过。
GUI 的 155 项输入仍与 `88c3f69` 一致，复用该构建和 36/0；原环境已实际恢复。
新增原生交互/整改后截图因 Esc 尚未覆盖，下一停止点为窗口、清单与 H4 人工确认，不重验 H2/H3，不宣布 M7 完成。
以上为 H4 候选交付时的历史状态。所有者随后已于 2026-11-02 阶段性签收 H4，并批准直接实施 H5。
隔离任务已全终态，原 Core/GUI 已实际恢复；五组公开状态与切换前完整快照一致，详见现有 status/testing 文档。
H2-A、H2-B、H3 签收保持；H2-B 新增代码、H3 与 H4 仍无对应远端验证，不用旧 CI 替代。
上述“只推进 H5、不进入 H6/H7”为当时交接边界；此后 H5、H6 已阶段性签收，H7 已获批准并形成本地候选。
最终 `c273c76` 已获批推送且同 SHA 三平台 CI/CodeQL 全绿；当前停止点只等待 H7 最终人工确认及剩余事项处理，详见本计划“当前 H7 授权与收束”。不进入 M8/M9/M11，不擅自推送或宣布 M7 完成。
候选读取、exact Unity、Hub 和 State16 的既有合同保持；Config3 已由获批 Config4 迁移替代，删除 density、保留其他偏好及 revision。
完整 GUI/Rust/三平台技术门禁与人工视觉结果分别记录，不把旧 SHA 或定向通过冒充当前候选全绿。
