# M7 当前 Official GUI 布局差距

审计基线：本地候选 `19267230507071dc61ba306b98c8cfdd113e9ea2`。

结论：该候选的 Core/RPC、Settings Config v1、Activity、Diagnostics、Portable UI、Plan/Apply、Playwright 和 accessibility
基础设施继续作为有效功能证据；但 official GUI visual architecture、component system 与 information architecture 未通过项目
所有者视觉验收。状态为：

```text
functional_implementation_candidate
but
rejected_for_visual_and_information_architecture_acceptance
```

该候选不得 push，也不能进入原 manual checklist。`gui.m7-core-surfaces` 的 functional wiring 不降为 not started；后续选定 IA 后
应复用 typed RPC、hooks/state、workflows、Settings、Activity、Diagnostics、Portable UI 与 Playwright infrastructure，并只重构
presentation/composition/component layer。

本 gap 分析不把“与 v3 不同”自动判为错误。v4 新能力、次级 route 和 UX correction 可以存在；真正的问题是当前候选把
Core/domain/API 的分界近似逐项投影成一级产品区域，削弱了用户语义分组、工作流上下文、桌面信息密度和 ALCOMD3 识别性。

## 逐面比较

| Surface | 结果 | 当前候选 | 需要纠正或决定的差距 |
|---|---|---|---|
| Main shell | `STRUCTURAL_GAP` | 顶部 brand/settings app bar + 左侧 11 项文字 rail + 通用内容区 | 第一眼不像 v3 的 sidebar + single business canvas；需要恢复可识别 composition，但不要求固定 260 px 或旧 chrome |
| Navigation | `STRUCTURAL_GAP` | Home、Projects、Repositories、Templates、Unity、Operations、Extensions、Activity、Diagnostics、Settings、About 全部平级 | route/domain inventory 泄漏为 sidebar；需要按 Projects、Resources、Project workflow、utility 等用户语义重新分组 |
| Landing | `IA_DECISION_REQUIRED` | 独立 status/dashboard 成为默认页 | v3 的 Projects home-equivalent 是强参考；也可评估有真实价值的 bounded Overview，不能因 `system.status` 存在就默认采用 |
| Projects | `STRUCTURAL_GAP` | generic page title、card grid 和页面后部 action section | 缺少 toolbar、search/list-grid/create/refresh 的位置习惯和 desktop-friendly list/grid density |
| Project workspace | `STRUCTURAL_GAP` | generic detail grid + Packages/Unity/Backups 三个原生按钮 subnav + 分散 action sections | 需要恢复 project header、package-primary workspace、Unity/open/backup contextual action 的整体关系；具体 tab 可重新设计 |
| Resources | `STRUCTURAL_GAP` | Repositories 与 Templates 各自成为一级 generic page；User Packages 尚无完整能力 | 应比较统一 Resources 类大组；现有 routes/detail wiring 可保留，未实现 User Packages 不显示假入口 |
| Unity | `PLACEMENT_DECISION_REQUIRED` | global registry 是一级页，project Unity 是 subroute | v4 registry 是真实能力；需区分 global installation management 与 project-specific action，并选择 Settings、Platform hub 或其他有用户依据的位置 |
| Backups | `CONTEXT_GAP` | Project Backups route 与独立 Backup detail | durable record 合理；create/restore、history/detail 应仍以 Project workflow 为主，而不是因 Backup domain 独立就成为全局主区域 |
| Operations | `PLACEMENT_DECISION_REQUIRED` | 独立 durable list/detail | durable Operation 是必要新增；待比较 footer/status/history utility、secondary hub 或一级入口，context follow 必须保留 |
| Extensions | `PLACEMENT_DECISION_REQUIRED` | 独立一级 generic list/detail/Portable UI route | v4 management/Portable UI 是真实能力；待比较 utility、Platform child 或一级入口，不能由 Extension Host 边界直接决定 |
| Settings | `STRUCTURAL_GAP` | 顶部全局 shortcut；页面是少量 native select + save/discard | Config v1 接线有效；需要 grouped utility surface 与一致 action hierarchy，不要求逐项复刻 v3 设置卡片 |
| Activity / Diagnostics | `GROUPING_DECISION_REQUIRED` | 两个平级一级 generic page | permission/RPC 分离正确；用户呈现可为同一 Logs/Activity surface 的 secondary views，不能合并 authority |
| About / licenses | `UTILITY_GAP` | 独立一级大幅 About page | version/About/licenses 更符合 footer/utility/deep-link placement，不需要因 route 独立而占一级入口 |
| Dialogs | `MATERIAL_FOUNDATION_GAP` | custom React + native `<dialog>` + custom CSS | 交互测试存在，但没有使用 Material Web Dialog/component behavior |
| Portable UI | `MATERIAL_FOUNDATION_GAP` | protocol/render semantics 正确；interactive controls 是 native HTML approximation | Core 与 extension renderer 没有共享 Material component foundation |
| Loading/error/progress | `MD3_REFINEMENT` | 统一 route state 和 Operation follow 已存在 | 功能关系可保留，需迁移到共同 MD3 component/state tokens |
| Narrow/accessibility | `ADAPTIVE_REFINEMENT` | modal drawer/focus trap/320 px automation 已存在 | adaptive基础可复用；选定的 hierarchy/grouping 在 drawer 中必须保持，不机械复制 wide 尺寸 |

## 主要偏离清单

以下是当前候选尚未解决的结构问题，不是对所有 v4 新 surface 的否定：

1. 独立 top app bar 与全局 Settings shortcut 改变产品第一眼 composition；
2. 11 个完全平级的 primary destinations 把 route/domain boundary 冒充用户 IA；
3. Home/Overview 与 Projects 的 landing 角色没有经过用户价值比较；
4. Repositories、Templates 与未来真实 User Packages 缺少资源型高层语义；
5. Project workspace 从 package-centric toolbar/list 变为 generic detail/subnav/action sections；
6. global Unity 与 project Unity、durable Backup/Operation 没有按使用上下文分层；
7. Extensions、observability、Settings、About/version 的 primary/utility hierarchy 缺少明确产品理由；
8. 数据密集 list/table 被大面积 generic cards、巨幅标题和留白替代；
9. 主要动作从 page toolbar/context row 移到页面后部；
10. Core 与 Portable UI 都以 native controls + custom CSS 模拟 Material，而非共享 Material Web component layer。

现有 `/unity`、`/operations`、`/extensions`、`/activity`、`/diagnostics`、`/about` 等 route 可保留作 deep link 或 secondary
surface；是否出现在 sidebar、在哪个 grouping 中出现，是独立 IA 决策。

## 可直接保留的实现证据

- typed Tauri adapter、`alcomd-client`、RPC 与 application authority；
- route identity、deep link 能力和 opaque resource ID；
- Core read/mutation wiring、Plan review、Apply、Operation follow/cancel/reconnect；
- Settings Config v1、Activity 与 redacted Diagnostics contracts；
- Portable UI protocol/session/action/host-owned chrome；
- dirty/discard、loading/refreshing/empty/error/disconnected state logic；
- Playwright 1.62.1、keyboard/focus/ARIA/contrast/320 px/reduced-motion infrastructure。

这些证据只允许在 visual/IA realignment 中复用，不得借页面重组重新设计业务 authority。
