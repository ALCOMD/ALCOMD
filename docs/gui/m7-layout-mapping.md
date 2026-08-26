# M7 ALCOMD3 v3 -> ALCOMD v4 user-model mapping

状态：项目所有者已批准的 minimum-change IA mapping；授权 H0/H1，H1 后停止在 Visual Gate 1。

权威综合说明位于 `docs/gui/m7-user-model-continuity.md`。此前 Candidate A/B/C 已降为 historical exploration，不再是下一轮
production 候选。

## Mapping rule

ALCOMD v4 GUI 默认继承 ALCOMD3 v3 用户模型。映射顺序固定为：

1. 识别 v3 中用户实际进入的区域和完成的任务；
2. 判断 v4 是相同能力、既有能力增强，还是真正的新用户能力；
3. 相同/增强能力优先回到原用户概念和 workflow；
4. genuinely-new 能力先尝试 context、secondary surface 或 utility；
5. 只有通过四项独立工作区测试，才考虑新增 primary surface。

crate、RPC、State、permission 或 subsystem 边界不能跳过该顺序。

## Minimum-change mapping

| v3 user area | v4 capability | Classification | Minimum-change placement | Current `192672…` correction |
|---|---|---|---|---|
| Projects landing | Projects | `SAME` + `ENHANCED_EXISTING` | 保持 landing/home-equivalent、toolbar、list/grid、create/import/restore | 删除独立 Home domain dashboard；status进入shell/footer |
| Project Manage | Packages | `ENHANCED_EXISTING` | package table仍是Project主要工作面；Plan/Apply/Operation嵌入原流程 | 从generic detail/subnav恢复package-centric workspace |
| Project header/context | project Unity | `ENHANCED_EXISTING` | selector、writer state、Open Unity留在Project context；不冻结为同级tab | 不把project Unity提升为global nav |
| Projects/Project actions | Backups | `ENHANCED_EXISTING` | create/restore从context发起；history/detail使用context action、secondary view、dialog或project sub-surface | 不从树形文档或Backup RPC推导同级tab/global domain |
| Packages & Templates | Repositories | `ENHANCED_EXISTING` | 同一资源区域的Repositories secondary view | 从平级primary移回资源分组 |
| Packages & Templates | User Packages | `SAME` target；implementation incomplete | 真实能力完成后恢复同组secondary view | 当前不显示fake/disabled entry |
| Packages & Templates | Templates | `ENHANCED_EXISTING` | 同一资源区域的Templates secondary view；create-project回到Projects | 从平级primary移回资源分组 |
| Extensions utility | Extensions | `ENHANCED_EXISTING` | 保持既有用户概念和大致utility归属；list/detail内增加安全控制 | 不归入新的Platform/Resources分类 |
| Extension-specific UI | Portable UI | `INTERNAL_ONLY` presentation | Extension detail内渲染extension功能 | 不显示Portable UI产品入口 |
| Settings | global Unity installations | `ENHANCED_EXISTING` | Settings内Hub/Editor/installation section | 移除Unity平级primary entry |
| Settings/UI Theme | Settings/appearance | `SAME` + `ENHANCED_EXISTING` | grouped Settings；可保留轻量appearance shortcut | 移除长期top Settings shortcut的竞争hierarchy |
| Log > Activity | Activity | `ENHANCED_EXISTING` | Log/Activity utility内secondary view | 与Diagnostics合并用户容器，不合并RPC/permission |
| Log > Technical | Diagnostics | `ENHANCED_EXISTING` | 同一utility内Diagnostics secondary view | 移除独立Diagnostics primary entry |
| Progress/task/toast | Operations | `ENHANCED_EXISTING` + bounded `GENUINELY_NEW` history | context progress + footer active-task + secondary Task Center/history | 移除Operations primary-domain地位 |
| Settings/licenses + footer version | About/licenses | `SAME` | footer/version action或Settings secondary page；Updater真实存在后才显示Update | 移除About primary entry与fake update UI |

## Structural continuity anchors

- sidebar + single main content canvas；
- 与 v3 同数量级和深度的主要用户区域；
- Projects、Packages & Templates、Extensions、Settings、Log 的用户概念连续性；
- package-centric Project workspace 与 context action；
- toolbar + dense list/table/compact row；
- utility/footer 中的 progress、version、About；
- list/detail/workspace 与 modal review/progress 的空间关系。

这些是产品识别基线，不要求旧 CSS、固定像素、每个按钮、历史缺陷或旧实现。

## Necessary v4 additions inside existing areas

| Enhancement | Existing user area |
|---|---|
| Plan review / Apply / stale handling | 原 package/template/backup/extension mutation workflow |
| durable progress / cancel / recovery | 原 context progress + secondary Task Center |
| extension permission/scope/quarantine | Extensions detail |
| Portable UI rendering | extension-specific content |
| structured redacted diagnostics | Log/Diagnostics secondary view |
| Unity writer evidence | Project Unity context |
| authoritative revisioned settings | Settings |

## Divergence budget

| Class | Treatment under the minimum-change proposal |
|---|---|
| `STRUCTURAL_CONTINUITY` | 默认；保留 v3 user model、shell、grouping、density与workflow关系 |
| `MD3_MODERNIZATION` | 允许；改变component/visual/interaction language，不替代IA |
| `V4_NECESSARY_ADDITION` | 先嵌入既有区域；必须说明真实新用户价值 |
| `UX_CORRECTION` | 允许窄修正；不得用来重建整个产品分类 |
| `ADAPTIVE_CHANGE` | wide/narrow/zoom/input所需；保持任务和语义顺序 |
| `UNJUSTIFIED_DEVIATION` | 禁止；包括把module/RPC/State边界映射为导航 |

## Boundary

现有 route identity 与 deep link 可以保留，即使 route 不出现在 sidebar。后续复用
`19267230507071dc61ba306b98c8cfdd113e9ea2` 的 typed client/hooks/state/workflow、Settings、Activity、Diagnostics、
Portable UI 与 Playwright infrastructure，只重构获批的 presentation、composition、component layer 与 visual hierarchy。

本映射不修改 RPC、Plan/Apply、Operation、Settings Config v1、Activity/Diagnostics permission、Portable UI protocol 或
application authority。
