# M7 ALCOMD3 v3 -> ALCOMD v4 布局映射

状态：visual realignment proposal；所有 `pending` 项在项目所有者批准前不得作为 production 设计结论。

分类：`same`、`md3_translation`、`adaptive_translation`、`intentional_deviation`、`missing`、
`incorrect_current_design`。

| v3 surface | v3 layout | v4 route | v4 intended MD3 layout | difference | reason | approval status |
|---|---|---|---|---|---|---|
| Main shell | 约 260 px persistent left sidebar + rounded right content canvas；无长期 top product bar | shell | wide 保持 left navigation + single content canvas；narrow 转 modal drawer；品牌/连接状态紧凑进入侧栏或内容 header | `incorrect_current_design` | 当前 top app bar + 11 项平铺 rail 改变第一眼结构 | pending |
| Landing | Projects 是主要工作入口，无独立 Home | `/`、`/projects` | 默认工作入口恢复为 Projects；Home/status 可由 brand/secondary destination 到达 | `intentional_deviation` | v4 有 daemon/RPC status，但不应替代项目工作入口 | pending |
| Projects | header: title/refresh/search/list-grid/create；body 单一 list/grid | `/projects` | 同结构，用 MD3 buttons/icon buttons/text field/segmented treatment；保留 typed RPC data | `md3_translation` | 功能相同，恢复动作位置和信息密度 | pending |
| Project manage | back/name/path + Unity selector/Open Unity/overflow；package table 为主 | `/projects/:id` 及 packages/unity/backups routes | 单一 Project workspace；header 保留 back/name/Unity/open/overflow；Packages/Unity/Backups 作为 secondary tabs/sections | `adaptive_translation` | v4 有稳定子路由和更多 durable read model | pending |
| Package management | Project context 中的 dense package table、search/filter、version/action | `/projects/:id/packages` | Project workspace 默认/主要 tab，table/list 保持主导；Plan review 使用 daemon ChangeSet | `md3_translation` | M4 Plan/Apply 改变 authority，不改变 workflow shape | pending |
| Packages & Templates group | 一级分组，Repositories/User Packages/Templates 三段 selector | `/repositories`、`/templates` | 一级“Packages & Templates” destination，页内 tabs 至少连接 Repositories/Templates；未实现能力不显示假 tab | `incorrect_current_design` | 当前把 Repositories/Templates 拆成平级一级入口且没有 group | pending |
| User Packages | Packages group 内独立 tab/list | no complete route | capability 不存在时不发布；未来以兼容 tab 加入 | `missing` | M7 不能虚构尚未实现的完整 user-package registry | not applicable until capability exists |
| Repositories | page tabs + Add Repository at upper-right + dense list/table | `/repositories`、`/repositories/:id` | 保留 group tabs、header add/refresh、table/list 与 detail drill-in | `incorrect_current_design` | 当前 generic cards + page-bottom action section 弱化主要动作 | pending |
| Templates | Packages group tab + template list/cards + contextual create | `/templates`、`/templates/:id` | 保留 group tabs；list/detail/context actions；Plan/Apply 保持当前 RPC | `incorrect_current_design` | 当前独立一级 generic grid/detail/action section | pending |
| Unity in project | Project header selector/Open Unity，迁移 dialog | `/projects/:id/unity` | header 仍显示 selected editor/open；详细 registry/writer evidence 可在 project Unity tab | `adaptive_translation` | v4 M5 有 writer-state 与 installation identity | pending |
| Unity registry | Settings cards | `/unity` | 可保留专用 registry page，但在 primary nav 中归入 secondary/utility group；Project workflow 不绕到 registry 才能 launch | `intentional_deviation` | v4 有完整 installation registry use case | pending |
| Backup create/restore | Projects/Project context action + modal/progress；settings 保存 format/path | `/projects/:id/backups`、`/backups/:id` | Project Backups tab 展示 durable records；create/restore 主动作留在 Project/Projects header 或 overflow | `intentional_deviation` | v4 M5 有 durable metadata、Plan/Operation/recovery | pending |
| Long-running work | context modal/progress host，无 history page | `/operations`、`/operations/:id` | 原业务上下文继续展示 active Operation；独立 Operations 作为历史/恢复 utility destination | `intentional_deviation` | M2 durable OperationId 是 v4 新能力 | pending |
| Extensions | bottom utility entry；MCP/Discord 可直接出现在 sidebar | `/extensions`、detail、Portable UI | Extensions 位于 utility section；detail/permissions/UI 使用 host MD3 components；未来 first-party entry 可按合同贡献 | `adaptive_translation` | v4 统一 first/third contract 和 Portable UI | pending |
| Settings | 单一 header + vertical scroll card groups；Theme 快捷动作 | `/settings` | 恢复 grouped scrolling settings；appearance/theme 是同一 Config v1 section；save/discard 保留 authoritative RPC | `incorrect_current_design` | 当前通用大标题 + 少量原生 selects，组织与 v3 差异大 | pending |
| Log | Activity/Technical tabs + search + filters | `/activity`、`/diagnostics` | utility group内可见的 Activity/Diagnostics destinations；可用 secondary tabs 保留共同 observability context | `intentional_deviation` | A-026 分离 permission/redaction，不能合并 authority | pending |
| About/licenses | Settings cards/License child；version/update在侧栏底部 | `/about` | About 可作为 Settings 内 secondary route；版本、Schema、licenses 用 definition/list/card | `intentional_deviation` | v4 需要明确产品/contract/notice disclosure | pending |
| Narrow window | compact icon sidebar | shell | modal navigation drawer + retained page toolbar/content ordering | `adaptive_translation` | MD3 adaptive layout 与 320 CSS px accessibility | pending |
| Dialogs | centered modal with headline/content/footer | all mutations | `@alcomd/ui` Material Dialog；daemon Plan/risk/result contract不变 | `md3_translation` | 只替换组件与视觉层级 | pending |
| Progress | modal/task host, cancel/minimize | Operation follow | Material progress + durable Operation summary；close 不等于 cancel | `adaptive_translation` | v4 Operation 是 authority | pending |

## 允许的设计变化

无需逐项偏离审批的 MD3 translation：颜色、字体、角半径、elevation、细节 spacing、图标、component visual treatment、
motion，以及 narrow/compact 的合理 adaptive composition。

以下变化必须继续出现在上表并由项目所有者明确批准：primary navigation model、major page grouping/hierarchy、主要内容和
动作位置、list/detail 关系、Settings 组织、package/repository/project workflow shape。

## 业务边界

本映射不改变现有 RPC、Plan/Apply、Operation、Settings Config v1、Activity、Diagnostics 或 Portable UI protocol。实现时优先
复用 `19267230507071dc61ba306b98c8cfdd113e9ea2` 已完成的 typed hooks/client/state/flow logic，只重组 shell、page
composition、component rendering 和 visual hierarchy。
