# VPM 管理行为复查与运行更新

日期：2026-11-09。所有者要求“更新。并且再次检查VPM管理行为是否符合官方标准以及v3行为”。本轮更新已验证产物，随后只读审计及独立夹具核对，不修改新的生产行为、不执行用户项目包操作、不推进M11或宣称H7签收。

结论：当前常规管理路径有实现及定向证据，但**仍有四处可复现的v3行为缺口，不能宣称全面对齐**。官方的包格式/操作文档、VCC界面政策和v3自身行为分别核对，不将其中一项等同于全部VPM规范。

## 运行更新

更新前，运行源码、dist和候选程序与上轮`target/m7-package-removal-20261109/inputs.json`逐项一致。确认0活动任务，备份旧程序和完整数据后替换并重开。

- Core PID279216，SHA256 `E1590ED9C3A9C0ACA4782147928586E3E1FEADAC1758B2333FE77AA77E6DAE65`。
- GUI PID271640，SHA256 `508C082BC83ED1397986450703EF6A8DC6BDCF5929F9F14B7EF1BDF872C90F6E`，窗口ALCOMD3响应正常。
- Core ready / State31 / Config12，2项目、2仓库；保留原数据目录和WebView profile，项目ID/路径、仓库ID和设置摘要均保持。新程序包括卸载确认修正、提前表头及加载条裁切修正。
- 证据、前后状态和备份：`target/m7-package-removal-update-20261109/`。没有恢复旧数据库或修改Unity项目包；既有后台仓库维护仍可更新检查时间等运行记录，不声称数据库字节不变。未操作原生页面按钮，不将窗口响应代替包操作人工验收。

## 对照依据与范围

官方一手资料于本轮在线核对：

- [VPM包格式与版本范围](https://vcc.docs.vrchat.com/vpm/packages/)：清单、传递依赖、预发布、旧资产声明及摘要。
- [VPM CLI](https://vcc.docs.vrchat.com/vpm/cli/)：指定版本安装、按清单恢复、移除及显式强制移除。GUI的冲突审阅后Apply对应明确授权的方向，不把它描述成CLI参数完全相同。
- [Resolver](https://vcc.docs.vrchat.com/vpm/resolver/)：恢复缺失/错误版本、按锁定清单恢复及VCC的Resolver按钮政策。
- [仓库格式](https://vcc.docs.vrchat.com/vpm/repos/)、[社区仓库](https://vcc.docs.vrchat.com/guides/community-repositories/)：JSON索引、按包/版本组织和来源。
- [官方文档链接的版本库](https://github.com/adamreeve/semver.net)：SemVer/npm范围、预发布与构建号优先级。复用本项目已有范围向量，不把旧SDK版本对照网页误作范围规范。
- [源代码管理说明](https://vcc.docs.vrchat.com/vpm/source-control/)：包恢复、Resolver保留及VCC的gitignore副作用。

v3只读参考：`../ALCOMD3-v3-readonly/vrc-get-vpm/src/unity_project/{add_package,reinstall,resolve,remove_package,pending_project_changes}.rs`，以及`vrc-get-gui/app/_main/projects/manage/-package-list-card.tsx`和`-use-package-change.tsx`。仅审计行为，不复制、移植或包装源码。

| 范围 | 本轮结论 |
| --- | --- |
| 包/仓库JSON、name/version身份、vpmDependencies、Unity最低版本 | 对照既有解析实现和向量；支持常规官方形状。作者邮箱按此前获批合同允许省略，比文档列出的字段要求宽松，不称为严格逐字段等同 |
| 普通SemVer范围、项目直接依赖与锁定版本 | 区分普通裸版本exact、项目直接依赖minimum和locked exact；已有范围、预发布及构建号向量证据保持。未重新执行官方VPM二进制的完整差分 |
| 可见性、预发布、来源和版本显示 | 检查Core提前过滤、安装事实保留、无来源显示未知、单版本菜单、已安装强调及来源消歧；沿用所有者已确认的显示合同 |
| 普通安装/升级/降级 | Core保持未选择安装包、求解所需依赖，读锁定图与包元数据；UPM清单保持。版本/Unity冲突政策仍有下述获批差异 |
| 删除直接/传递包、依赖冲突审阅 | 上轮Avatars/Base修正已部署；明确目标实际移除、保留依赖方、确认前不写项目。但清理范围及缺失目录仍有下述问题 |
| 重装指定版本/指定包 | 固定版本及确认流程存在；无关包来源会阻挡单包重装，见F3 |
| Resolve恢复缺失目录/错误版本 | `build_resolution_plan_with_ranges`实际检查目录/版本；正向夹具对缺失目录产生Install，而非只比较锁定版本。完整Plan仍会准备归档及旧资产证据 |
| legacyFiles/Folders/Packages | 普通包事务已有路径、GUID移动、meta、替代包和恢复实现及隔离事务证据；不等于M11旧工程整体迁移完成 |
| 刷新、缓存与只读查询 | 保留用户确认的逐仓库自动维护、按包读取、显式刷新及100ms展示；不是VPM文件格式的一部分 |
| Plan/Apply、恢复及来源权限 | 保持v4冻结证据、过期拒绝、幂等和整体事务；比v3逐包失败处理更严格，不能描述为完全相同 |

上述复用验证绑定上轮同一运行输入：380项不同Rust测试、48个不同浏览器用例的实际范围见[上轮记录](package-removal-confirmation-20261109.md)。本轮没有重复执行完整工作区或这些套件；本轮新增的是下面五项独立核对。

## 已确认的四处缺口

### F1：未托管的项目内包不参与依赖保护

场景：locked中B依赖C，另一个手动放入`Packages/`、未列入locked的本地包也依赖C。卸载B时，v4将B和C都列为移除，冲突列表为空。该本地包清单变化也不改变包计划指纹。

证据：`plan.rs:526`只用direct键作根；`plan.rs:168`仅散列locked包元数据；`plan.rs:566`冲突只从这些保留的锁定边产生。v3的`pending_project_changes.rs:464`把unlocked包依赖作为保留根，Resolve也处理此类依赖。

影响：本地包所需的依赖可能随另一包卸载被移走，且不在冲突提示里。它与“已锁定包来源未知”和“用户包源登记快照”不同。优先补齐有界的项目内包元数据读取、依赖保留/冲突及对应冻结证据，不需要遍历payload。

### F2：卸载会清理与目标无关的孤立锁定包

场景：A/B是根包，C是历史遗留孤立锁定包，C与B没有依赖关系。请求卸载B，v4返回B+C。v3仅对本次移除/替换包的旧依赖闭包做无用包清理，保留这种无关C（`pending_project_changes.rs:423`）。

影响：移除范围超出所选包的相关依赖。v4当前`build_removal_plan`对全部locked做不可达清理；既有循环/共享依赖单测明确把orphan也移除写为预期，所以测试通过只证明实现自洽，不能证明v3一致。上轮获批方案保留了该旧规则，本次审计进一步确认它仍不等同v3。建议将自动清理限制在本次受影响的旧依赖闭包。

### F3：单包重装被无关包来源缺失阻挡

场景：A/B均安装，仓库中仅能找到A，要求只重装A。v4返回PackageNotFound，因为`engine.rs:240`为全部locked构造来源请求。v3的`reinstall.rs:44`只对指定重装包查找固定版本。

影响：无关包B的缺失/关闭来源或歧义影响本可完成的A重装。此前普通安装已加入未选择包保留规则，单包重装仍未补齐。建议只为选中重装目标准备来源/归档，保留其他安装包及原锁定图；不把此规则放宽到重装全部。

### F4：计划前目录已缺失的包不能正常卸载清单记录

场景：locked有B，但B目录在生成计划前就不存在。Core接受卸载并生成计划，Apply最终Failed，错误为`project_changed_during_apply`。`engine.rs:1722`对非Install遇到不存在目录一律拒绝，未区分计划前缺失与确认后变化。

v3在`pending_project_changes.rs:860`将移除目录时的NotFound作为已移除处理，继续更新清单。建议允许冻结为“原本缺失”的删除清单项；计划后目录消失/出现仍应按原证据拒绝，不以此放松竞态校验。

## 可复现证据与正向对照

`target/m7-vpm-management-audit-20261109/{probe.rs,run-probe.py,probe-final.log}`使用当前生产库，独立新建合成项目/数据库/缓存。未连接用户Core、未读写真实Unity项目、未更改生产源码或既有测试。

最终探针输出：

```text
unrelated-orphan removed=["com.example.child", "com.example.remove"]
unlocked-dependent removed=["com.example.child", "com.example.remove"] conflicts=[] ignoresLocalMetadataChange=true
selected-reinstall with unrelated B source missing=Err(PackageNotFound)
resolve-missing mutations=[("com.example.keep", Install)]
remove-missing operation=Failed error=Some("project_changed_during_apply")
```

F1/F2直接核对真实draft，F3调用实际Engine准备入口，F4经过隔离M4Application/Store/Plan/Apply与任务终态。Resolve正向对照只调用真实resolver及draft构造，未计为完整下载/Apply通过。

辅助程序首轮混用Tokio构建变体导致无reactor；修正为Core相同变体后，最初Resolve正向验证进入了既有归档准备分支，合成.invalid地址连接失败。最终改为纯draft正向检查，完整探针通过。保留`probe.log`和`probe-verified.log`；辅助验证失败不作产品缺陷。所有写入只在独立合成夹具中。

## 保留差异与未覆盖项

- 继续采用此前获批的Core权威、Plan/Apply、原子事务、来源歧义显式选择、按包准备、自动刷新、隐藏来源政策及本地用户源快照。安装/升级/降级和混合意图仍拒绝依赖版本冲突，只有已批准的手动Unity根目标和纯卸载冲突有窄例外；v3能展示更多冲突后继续，两者仍非全部相同。
- 官方VCC隐藏Resolver移除按钮；只读v3及当前v4都显示一般已安装包的移除入口。这是VCC界面政策与v3的差别，不直接认定为VPM格式违规。官方文档描述的自动创建Packages gitignore副作用，在v3/v4普通包操作源码中均未见；v4内置模板保留已有文件。这些也不能被概括成与VCC所有行为相同。
- 安装确认前仍准备归档以冻结旧资产证据，Apply复用缓存。这是已有获批v4事务设计，会让冷缓存网络延迟体现在确认前，非本轮新增，也不等于v3纯清单准备成本。
- 官方资料没有覆盖每个工具实现细节，不能把“文档未禁止”当作官方行为已验证。本轮不是官方VPM库全量运行差分、真实旧工程迁移、Unity编辑器验证或三平台验收。M11及H7边界保持。
- 本审计结束时四个新发现尚未修复；所有者随后明确授权修正，现已实施、验证并更新Core/GUI，见[四处缺口修正记录](vpm-four-gaps-20261109.md)。本页保留修复前反例及审计范围，当前行为以修正记录为准。
