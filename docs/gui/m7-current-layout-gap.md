# M7 当前 Official GUI 布局差距

审计基线：本地候选 `19267230507071dc61ba306b98c8cfdd113e9ea2`。

结论：该候选的 Core/RPC、Settings Config v1、Activity、Diagnostics、Portable UI、Plan/Apply、Playwright 和 accessibility
基础设施继续作为有效功能证据；但 official GUI visual architecture、component system 与 v3 layout fidelity 未通过项目所有者
视觉验收。状态为：

```text
technically_valid
but
rejected_for_m7_visual_design_acceptance
```

该候选不得 push，也不能进入原 manual checklist。`gui.m7-core-surfaces` 的 functional wiring 不降为 not started；visual
acceptance 由新的 planned gates 单独追踪。

## 逐面比较

| Surface | 结果 | 当前候选 | 与 v3 基线的主要差距 |
|---|---|---|---|
| Main shell | `MAJOR_LAYOUT_DEVIATION` | 顶部 brand/settings app bar + 左侧 11 项文字 rail + 通用内容区 | v3 是无长期 top bar 的持久侧栏 + 单一圆角内容画布；第一眼结构、内容起点与 utility placement 均变化 |
| Navigation | `MAJOR_LAYOUT_DEVIATION` | Home、Projects、Repositories、Templates、Unity、Operations、Extensions、Activity、Diagnostics、Settings、About 全部平级 | v3 以 Projects、Packages & Templates、Settings、Log 为结构锚点，Extensions/version 为底部 utility；当前分组消失 |
| Home | `MAJOR_LAYOUT_DEVIATION` | 独立 status/dashboard 成为默认页 | v3 Projects 是 home-equivalent；保留 Home 需要显式偏离批准 |
| Projects | `MAJOR_LAYOUT_DEVIATION` | generic page title、card grid 和页面内 action section | 缺少 title/refresh/search/list-grid/create 的统一顶部工具栏和 v3 识别性列表密度 |
| Project detail | `MAJOR_LAYOUT_DEVIATION` | generic detail grid + Packages/Unity/Backups 三个原生按钮 subnav + 分散 action sections | v3 header 集中 back/name/path/Unity/Open/overflow，package table 是主要工作面 |
| Packages | `MISSING` | 没有 top-level Packages & Templates group；package 只在 Project subroute | v3 一级资源分组与三段 selector 不存在 |
| Repositories | `MAJOR_LAYOUT_DEVIATION` | 独立一级页，generic resource cards/detail，add/refresh/remove 在下方 action section | v3 group tabs、header 右侧 Add Repository 和 dense row actions 被打散 |
| Templates | `MAJOR_LAYOUT_DEVIATION` | 独立一级页，generic cards/detail/workflow forms | v3 在 Packages group 内；create/import/derive 等上下文和主要动作位置不同 |
| Unity | `MAJOR_LAYOUT_DEVIATION` | 独立一级 registry + Project Unity subroute | v3 以 Project header/Settings 为主；v4 新 route 有架构理由但尚未完成布局偏离批准 |
| Backups | `MAJOR_LAYOUT_DEVIATION` | Project Backups route 与独立 Backup detail | v3 以 Project/Projects actions、dialog/progress 和 Settings 为主；durable record 是有理由但待批的偏离 |
| Operations | `NOT_APPLICABLE` | 独立 durable list/detail | v3 无对应 page；v4 M2 OperationId 提供合理新增，但 utility placement 和 context follow 仍待批准 |
| Extensions | `MAJOR_LAYOUT_DEVIATION` | 独立一级 generic list/detail/Portable UI route | v3 是底部 utility + extension sidebar entries；v4 host chrome正确但 grouping/visual composition 未对齐 |
| Settings | `MAJOR_LAYOUT_DEVIATION` | 顶部全局 Settings shortcut；页面是少量 native select + save/discard | v3 单一滚动卡片组和 theme/settings组织未保留；当前 Config v1 功能接线仍有效 |
| Activity/Diagnostics | `MAJOR_LAYOUT_DEVIATION` | 两个平级一级页，各自 generic cards | v3 是 Log 内 Activity/Technical 分段、search/filter；v4权限分离合理但呈现分组尚待批准 |
| About | `MAJOR_LAYOUT_DEVIATION` | 独立一级大幅 About page | v3 licenses/system information 位于 Settings，version/update 位于 sidebar footer |
| Dialogs | `MAJOR_LAYOUT_DEVIATION` | custom React + native `<dialog>` + custom CSS | 交互测试存在，但没有使用 Material Web Dialog/component behavior |
| Portable UI | `MAJOR_LAYOUT_DEVIATION` | protocol/render semantics 正确；interactive controls 是 native HTML approximation | Core 与 extension renderer 没有共享 Material component foundation |
| Loading/error/progress | `MINOR_MD3_DIFFERENCE` | 统一 route state 和 Operation follow 已存在 | 功能关系可保留，需迁移到共同 MD3 component/state tokens |
| Narrow/accessibility | `MINOR_MD3_DIFFERENCE` | modal drawer/focus trap/320 px automation 已存在 | adaptive基础可复用，但 shell视觉结构和Material组件行为需重验 |

## 主要偏离清单

以下偏离均未在本轮获得 production approval：

1. 独立 top app bar 与全局 Settings shortcut；
2. 11 个完全平级的 primary destinations；
3. Home 取代 Projects 成为主要 landing；
4. Repositories 与 Templates 脱离 Packages & Templates group；
5. Project workspace 从 package-centric toolbar/table 变为 generic detail/subnav/action sections；
6. Unity、Backups、Operations 的 v4 新 surface 没有与 v3 context workflow 建立可识别映射；
7. Settings、Activity/Diagnostics、About 的 utility grouping 被拆散；
8. 数据密集 list/table 被大面积 generic cards 替代；
9. 主要动作从 page header/context row 移到页面后部；
10. Core 与 Portable UI 都以 native controls + custom CSS 模拟 Material，而非共享 Material Web component layer。

## 可直接保留的实现证据

- typed Tauri adapter、`alcomd-client`、RPC 与 application authority；
- route identity 和 opaque resource ID；
- Core read/mutation wiring、Plan review、Apply、Operation follow/cancel/reconnect；
- Settings Config v1、Activity 与 redacted Diagnostics contracts；
- Portable UI protocol/session/action/host-owned chrome；
- dirty/discard、loading/refreshing/empty/error/disconnected state logic；
- Playwright 1.62.1、keyboard/focus/ARIA/contrast/320 px/reduced-motion infrastructure。

这些证据只允许在 visual realignment 中复用，不得借页面重组重新设计业务 authority。
