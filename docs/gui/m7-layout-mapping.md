# M7 ALCOMD3 v3 -> ALCOMD v4 布局映射

状态：information architecture synthesis；尚未选择 production target。本文把 v3 的可识别产品连续性映射到 v4 用户能力，
不把 v3 的具体导航树或当前 route inventory 当成预先批准的 sidebar。

候选方案、wide wireframe、adaptive behavior 与逐项比较位于
`docs/gui/m7-information-architecture-candidates.md`。项目所有者可以选择一个候选、明确组合其部分，或要求新的候选；在此之前
H0/H1 production 不得开始。

## Mapping principles

### Continuity anchors

- sidebar + main content 的可识别 shell composition；
- 与 v3 同数量级、有限深度并按用户任务分组的 navigation；
- Projects、资源和 project workspace 的主要空间关系；
- desktop management tool 所需的 dense list/table/compact row 与 page toolbar；
- search/create/refresh/context action、list/detail、Settings/observability/footer utility 的位置习惯；
- ALCOMD3 产品身份，而不是 generic domain dashboard。

### Permitted reinterpretation

- individual page 可因真实用户价值合并、拆分或重组；
- v4 新能力可进入既有分组、secondary surface、project context、utility 或经论证的新一级区域；
- Material Design 3 可现代化 component、interaction、token、surface、motion 和 adaptive composition；
- narrow 模式可以使用 drawer、priority columns、overflow 与紧凑 secondary navigation；
- 旧 CSS、旧组件、固定像素、每个按钮和不合理历史 UX 都不是兼容合同。

Core/domain/API/RPC namespace 只决定 authority 和实现边界，不自动决定 GUI 一级导航。

## Capability synthesis matrix

| Capability / concern | v3 continuity reference | Current `192672…` evidence or gap | IA question and allowed placement | Decision state |
|---|---|---|---|---|
| Main shell | left sidebar + right business canvas；page toolbar；desktop density | top brand/settings bar + 11-item flat rail 改变第一眼结构 | 所有候选保留 recognizable sidebar/content composition；具体宽度、header chrome 与 group label 可现代化 | shared candidate constraint |
| Landing | Projects 是 home-equivalent | Home/status 成为 generic default dashboard | A/B 使用 Projects landing；C 允许 bounded Overview。必须比较用户价值，不由 `system.status` route 自动决定 | owner selection pending |
| Projects | title/search/refresh/list-grid/create；single list/grid | generic cards 与下方 action section，typed data/action wiring 可复用 | 保持一级工作入口与 dense list/grid；是否同时存在 Overview 不改变 Projects 的核心地位 | shared candidate constraint |
| Project workspace | header 集中 project/Unity/open/overflow；packages 是主要工作面 | Packages/Unity/Backups 被实现为 generic subnav/actions | 使用一个 Project workspace；Packages、project Unity、Backups 作为 contextual tabs/sections/actions，不提升为三个全局入口 | shared candidate constraint |
| Repositories / Templates / User Packages | Packages & Templates 是资源型大类，内部三段 | Repositories/Templates 各自成为一级 route；User Packages 完整能力尚不存在 | A/B/C 均使用 Resources 类高层语义；真实能力存在时才显示 User Packages，不显示 fake tab | naming/refinement pending |
| Unity | project action在 Project；installation management 在 Settings | global `/unity` 和 project route 都已接线 | project-specific Unity 留在 Project；global registry 可在 Settings（A/C）或经论证的 Platform hub（B） | owner selection pending |
| Backups | create/restore 从 Projects/Project context 发起；settings 保存偏好 | durable Project Backups/detail route 已存在 | durable records 可作为 Project workspace tab/detail；create/restore 仍从项目上下文进入 | shared candidate constraint |
| Operations | context progress/task；无 catalog | durable list/detail 是真实 v4 能力 | 可由 footer/status/history utility（A/C）或 Platform secondary surface（B）承载；active follow 仍留在业务上下文 | owner selection pending |
| Extensions / Portable UI | Extensions 靠近 footer；extension entries 位于同一 sidebar 体系 | 独立一级 list/detail/UI route；host-owned chrome 和协议接线有效 | 可为 utility entry（A）、Platform child（B）或产品级一级入口（C）；不能因 Extension Host 是独立 subsystem 自动升级 | owner selection pending |
| Activity / Diagnostics | Log 内 Activity/Technical 分段 | 两个平级一级 generic page；permission/RPC 分离正确 | 可共享 Logs/Activity 类用户 surface 和 secondary tabs，同时保持 `activity.read`/`diagnostics.read` authority 分离 | wording/refinement pending |
| Settings | grouped scrolling settings；appearance 属于同一组织 | top shortcut + sparse generic sections；Config v1 接线有效 | 保持 utility placement 与 grouped sections；global Unity placement依候选决定 | owner selection pending |
| About / licenses / version | licenses/system info 在 Settings；version/update 在 footer | About 成为平级大页面 | footer/version utility 可进入独立 deep-link page 或 Settings-style secondary surface；无需一级 nav | shared candidate constraint |
| Narrow/adaptive | compact sidebar | drawer/focus/320 px logic 已存在 | group/order/semantic hierarchy不变；drawer、compact tabs、priority columns 是 adaptive translation | shared candidate constraint |
| Dialog / progress | modal review/progress/context task | semantic/state tests有效，Material component foundation缺失 | 使用共享 `@alcomd/ui` Material Dialog/Progress；Plan/Apply/Operation authority不变 | H0 pending |

现有 route identity 与 deep link 可以保留，即使 route 不出现在 sidebar；“存在 route”不等于“批准为一级产品区域”。

## Divergence budget

| Class | Meaning | Approval treatment |
|---|---|---|
| `STRUCTURAL_CONTINUITY` | 延续可识别 shell、用户分组、密度、workspace/list/detail/action 关系 | candidate 必须具备 |
| `MD3_MODERNIZATION` | component、interaction、token、color、type、shape、elevation、state layer、motion | 在 Material policy 内允许 |
| `V4_NECESSARY_ADDITION` | 真实新增能力需要新的可见 surface 或状态表达 | 记录用户价值与 placement，owner 选择 |
| `UX_CORRECTION` | 修正 v3 已知不合理 UX，而不改变 authority | 记录理由与 workflow 影响 |
| `ADAPTIVE_CHANGE` | narrow/zoom/input modality 所需的结构转换 | 必须保持任务、顺序和可访问性 |
| `UNJUSTIFIED_DEVIATION` | 仅因 module/RPC/route 存在、generic dashboard潮流或实现方便而改变产品 IA | 不得进入推荐或 production |

不要求所有内容都像 v3；但任何推荐方案中不得存在 `UNJUSTIFIED_DEVIATION`。

## Business and implementation boundary

本映射不改变 RPC、Plan/Apply、Operation、Settings Config v1、Activity、Diagnostics、Portable UI protocol 或 application
authority。后续应复用 `19267230507071dc61ba306b98c8cfdd113e9ea2` 已完成的 typed hooks/client/state/workflow 和
Playwright infrastructure，只重构经批准的 presentation、composition、component layer 与 visual hierarchy。
