# M7 Material Web component audit

审计基线：`19267230507071dc61ba306b98c8cfdd113e9ea2`。下列 inventory 保留该候选的历史事实；H0 已在本地提交
`35c7110` 完成审计结论的修复，尚待 Visual Gate 1。

## 结论

历史结论：

```text
material_web_dependency_present_but_component_system_not_adopted
material_theme_not_actually_wired
```

H0 当前结果：`@material/web = 2.5.0` 的 direct ownership 已迁移到 `@alcomd/ui`，没有新增 package；Material registration
集中在该 package，GUI 通过窄 React 19 facade 使用真实 Button/IconButton/TextField/Select/Switch/Checkbox/Dialog/Progress。
随后完成的 component-boundary 修正已将 Core 与 Portable UI 中 Material Web 有对应能力的 production control 全部迁移为真实
`md-*` element，并将静态 source gate 纳入 GUI `check`。Playwright 已验证 keyboard/pointer/focus/disabled、property/event、
dialog、validation/supporting text 和共享 theme token 行为。`material-color-utilities` 未进入依赖图；当前三档冻结 source color
直接映射到正式 `--md-sys-color-*` token，没有实现近似 HCT 算法。

## Historical static production JSX control inventory

计数范围仅包括 `App.tsx`、`CoreActions.tsx`、`CorePages.tsx`、`PortableUiRenderer.tsx` 中的静态 JSX source sites；
map/render 后的实际实例数可能更大。例如 primary navigation 的一个 source site 会产生 11 个 runtime item。

| Semantic control | Static sites | Historical implementation | Notes |
|---|---:|---|---|
| button | 59 | native HTML + custom CSS/custom React composition | App 8、CoreActions 32、CorePages 17、Portable UI 2；包含 filled/tonal/danger/text-link/navigation |
| icon button | 2 user-visible | native button + text glyph | navigation hamburger 和 close；scrim 另用 native button但不是可见 icon action |
| text field | 20 | native input + custom labels/CSS | CoreActions 19、Portable UI 1；不含 number/checkbox |
| textarea | 2 | native textarea | CoreActions 1、Portable UI 1 |
| integer/number field | 3 | native input type=number | CoreActions 2、Portable UI 1 |
| select | 12 | native select | CoreActions 6、CorePages Settings 5、Portable UI 1 |
| switch | 1 | native checkbox styled/semantically consumed as Portable UI switch | Core official settings当前没有 switch |
| checkbox | 5 additional | native checkbox | CoreActions；与 Portable switch 合计 6 个 checkbox source sites |
| radio | 0 | none | 当前 production 未渲染 |
| dialog | 1 host site / 4 custom wrappers | custom React around native dialog | Confirm/Plan/TemplatePlan/ExtensionPlan 共用 ModalDialog，手写 focus/showModal handling |
| menu | 0 | none | context/overflow menus 尚未实现为语义 menu |
| tabs | 0 | native buttons in custom subnav | Project Packages/Unity/Backups 不是 Material Tabs |
| progress | 2 | native progress | OperationFollow 与 Portable UI |
| list | multiple dynamic surfaces | custom React + semantic list/table/card where present | Resource grid、DataTable、Portable UI list；没有 Material List |
| navigation item | 1 map site / 11 runtime items | native buttons in custom aside/nav | Project secondary navigation另有3个native buttons |

以上表格只记录审计发现时的迁移输入，不描述当前 production 状态。当前 production Material module import 只位于
`packages/alcomd-ui/src/index.ts`；`apps/alcomd-gui` 没有 direct
`@material/web/*` import。

当前实现来源汇总：Button/IconButton/TextField/Select/Switch/Checkbox/Dialog/Progress 的叶节点全部是真实 Material Web element；
Primary Navigation 的 action items 使用真实 `md-list` / `md-list-item`；route/card/data table/page grid 继续使用 semantic HTML。
Material Web 2.5.0 不提供完整 Drawer/Rail，因此 `aside` / `nav` shell 仍是 semantic layout，但不再包含 native interactive
exception。`aria-current` 保留在 host contract，2.5.0 同时把当前项映射为 `aria-selected`；ripple、focus ring、keyboard activation
与 state layer 由 `md-list-item` 提供。

## `@alcomd/ui` 当前能力

H0 后 `packages/alcomd-ui` 提供：

- product/technical name constants；
- 三档 spacing constants；
- appearance type、defaults和`applyAppearance()`；
- Material Web 2.5.0 的集中 registration 与窄 element name contract；
- shared `theme.css` 的真实 MD3 color/type/shape token source；
- GUI 本地窄 React 19 facade 使用的 Button、IconButton、TextField、Select、Switch、Checkbox、Dialog、Progress、List/ListItem。

它仍不提供：

- navigation/page/grid/split-pane layout primitives；
- 任意 source color 的动态 HCT palette generation；
- 未有 H0/H1 真实需求的 Menu/Tabs/Radio。

React integration 保持在 GUI 的本地窄 facade，因此 `@alcomd/ui` 不需要新增 React dependency；Core 与 Portable UI 共用该
facade和同一 token source，没有建立第二套 Material implementation。

## Component coverage matrix

| Semantic control | Current implementation | Material Web 2.5.0 available | `@alcomd/ui` wrapper needed | Policy | Exception/rationale |
|---|---|---|---|---|---|
| filled/tonal/outlined/text button | real `md-*-button` | yes, button variants | yes | `USE_MATERIAL_WEB` | danger基于Material text button + error tokens，不手写另一按钮 |
| icon button | real `md-icon-button` | yes, icon button variants | yes | `USE_MATERIAL_WEB` | icon source由host选择，interaction来自Material |
| text/search/password field | real `md-outlined-text-field` | yes | yes | `USE_MATERIAL_WEB` | search semantics在wrapper中保持label/role |
| textarea | real `md-outlined-text-field type=textarea` | yes | yes | `USE_MATERIAL_WEB` | rows/validation由integration test覆盖 |
| integer field | real `md-outlined-text-field type=number` | yes | yes | `USE_MATERIAL_WEB` | JSON-safe bounds仍由现有业务逻辑验证 |
| select | real `md-outlined-select` | yes | yes | `USE_MATERIAL_WEB` | single select；options由host生成 |
| switch | real `md-switch` | yes | yes | `USE_MATERIAL_WEB` | Portable UI payload/protocol不变 |
| checkbox | real `md-checkbox` | yes | yes | `USE_MATERIAL_WEB` | label保持host semantic composition |
| radio/radio group | none current | yes | only when a reviewed surface needs it | `USE_MATERIAL_WEB` when needed | 不为未来需求提前封装 |
| dialog | real `md-dialog` | yes | yes | `USE_MATERIAL_WEB` | focus/cancel/return focus以observable tests验证 |
| menu | none current | yes | yes when H1/H2 restores overflow actions | `USE_MATERIAL_WEB` | anchor/keyboard/typeahead需wrapper测试 |
| tabs | none current | yes | yes when a reviewed tab surface exists | `USE_MATERIAL_WEB` when needed | 不为不存在的 tab user entry 预建wrapper |
| linear progress | real `md-linear-progress` | yes | yes | `USE_MATERIAL_WEB` | determinate/indeterminate均已支持 |
| list/list item | real `md-list` / `md-list-item` for primary navigation；semantic data/list surfaces elsewhere | yes | yes for action/navigation lists that fit Material List | `USE_MATERIAL_WEB` where semantic fit | dense data tables不强行转换成md-list |
| primary navigation shell | semantic aside/nav containing real Material list items | no complete application shell | layout primitive + Material List facade | `SEMANTIC_HTML_OR_HOST_COMPONENT` shell / `USE_MATERIAL_WEB` actions | 2.5.0缺少Drawer/Rail；官方 catalog site同样以custom drawer shell组合`md-list`/`md-list-item` |
| data table | semantic table/custom | no production-ready Material Web table | table primitive/styles | `SEMANTIC_HTML_OR_HOST_COMPONENT` | 保留真实 table semantics与keyboard/accessibility |
| page/card/grid/split pane | custom semantic layout | no complete equivalent | minimal layout primitives | `SEMANTIC_HTML_OR_HOST_COMPONENT` | 只共享tokens/structure，不伪造md component |
| headings/text/section/article/main/dl | semantic HTML | not applicable | no | `SEMANTIC_HTML_OR_HOST_COMPONENT` | 非交互语义HTML应继续使用 |

## Material interaction policy

只要 Material Web 提供对应 interactive component，official GUI 与 Portable UI 不得用 native HTML + custom CSS 重新实现视觉近似版。
Material component必须保留它提供的 hover state layer、pressed state、focus state、ripple 和 disabled behavior；不得增加 CSS
radial-gradient 或 JavaScript ripple imitation。

依赖方向冻结为 proposal：

```text
@material/web
    -> @alcomd/ui
        -> apps/alcomd-gui
        -> PortableUiRenderer
```

`apps/alcomd-gui` 原则上不散布 direct `@material/web/*` import。Material registration、React integration、theme tokens 和
ALCOMD variants 集中在 `@alcomd/ui`。Core 与 Portable UI 使用同一层；Portable UI protocol/node/action/surface/RPC/permission
不变。

### Information architecture non-conclusion

本审计只证明 interactive component foundation 与 theme wiring 尚未采用 Material Web；它不选择 GUI information architecture，
也不把 Material component catalog、Rust module、RPC namespace 或 route inventory解释为一级导航清单。Semantic HTML、application
shell、layout、data table 与 navigation composition 仍可由 ALCOMD 实现，并由同一 MD3 tokens 驱动。最终结构必须同时满足
A-033 的 v3 user-model continuity、真实 v4 用户任务和项目所有者批准的 minimum-change proposal；Material Design 3 是组件、
交互与视觉语言，不是 IA 替代品。

## Theme audit

`applyAppearance()` 继续只设置：

- `data-appearance`；
- `data-density`；
- `data-source-color`。

H0 的 `theme.css` 已让 Material components 与 semantic layout 同时消费正式 `--md-sys-color-*`、type和shape token；旧变量
只是 semantic layout 的短 alias。当前未引入动态 palette generator，也未实现近似 Material color algorithm。Material Web
已有对应能力的 production control 已完成迁移；后续新增 interactive control 由 source gate 强制遵循同一边界。

## React 19 + Material Web 2.5.0 integration audit

当前 lock 中 React/ReactDOM 为 `19.2.8`，Material Web 为 `2.5.0`。本地 package declarations 已确认：

- text field 支持 text/textarea/number、value、readOnly、disabled、constraint validation 和 composed input/change；
- select 支持 value、validation、input/change 与 opening/opened/closing/closed events；
- switch/checkbox/radio 是 form-associated custom elements；
- dialog 提供 open/show/close、cancel、focus trap 与生命周期 events；
- buttons、checkbox、list item、switch 等内部包含 Material ripple/focus/state-layer实现；
- tabs/menu/progress/list APIs存在，但 application shell和data table不在其完整组件范围。

H0 contract tests 必须逐项证明：

1. React 19 对 custom element boolean/string/number property 的传递符合 2.5.0 declaration；
2. standard与custom events被 typed wrapper正确订阅/清理，不能假设所有事件都等价于React synthetic event；
3. controlled value/selected/open 与 ref 生命周期稳定；
4. TypeScript JSX typing由 `@alcomd/ui` 内部解决，不把Material element类型泄漏到业务页面；
5. form participation、FormData、reset、required、reportValidity和errorText在真实 Chromium/WebView path 可观察；
6. disabled/readOnly/error/validation和label/description语义保持；
7. dialog/menu/select的focus、Escape、anchor、return-focus与reduced-motion行为稳定；
8. SSR不适用：official GUI是Tauri client-only，但这不能用来跳过custom element registration顺序测试。

优先使用 React 19 原生 custom element support 与项目自有窄 wrapper；不增加第三方 React Material adapter。若实际实现需要新
dependency，必须先停止并单独审批。

## Current evidence and permanent gate

- component presence：验证公开 `@alcomd/ui` wrapper实际渲染相应 `md-*` custom element；
- behavior：验证 observable click/input/change/pressed/focus/ripple/state/result，不测试Material Web shadow DOM私有结构；
- theme propagation：同一 source color/mode/density/motion驱动Core、Portable UI和layout primitive；
- exceptions：每个继续使用native interactive element的场景必须在coverage matrix中有明确rationale；当前production TSX无例外；
- zero duplicate foundation：Core与Portable UI不得出现两套button/form/dialog实现。
- source gate：`apps/alcomd-gui/scripts/check-material-controls.mjs` 扫描production TSX；发现native
  button/input/select/textarea/progress/dialog即失败。

### Official catalog navigation source audit

2026-08-27 对 Material Web 官方仓库 `cac97678831d48d4eb4a606ca50f92673a1dc20c` 做只读源码核对。官方
`catalog/site/_includes/default.html` 的 drawer navigation 由 `nav-drawer`、`md-list.nav` 与 `md-list-item` 组成；
`catalog/site/css/global.css` 为 list 设置透明 container、12px inline margin，并以 28px radius 和
`surface-container-highest` 表达 selected container。它额外设置的 12px item block margin 属于 catalog site 自身排版，
不是 Navigation Drawer 的通用 item spacing。Material 3 Navigation Drawer 的官方实现合同使用 0dp top/bottom shape inset；
ALCOMD 因此保留官方 component composition 与 inline inset，但不在连续 navigation items 之间增加外部 gap。没有照截图猜测控件，
也没有导入 labs navigation drawer 或新增依赖。

项目所有者随后选择 v3 final navigation density 作为 ALCOMD 的产品连续性基线。只读源码
`../ALCOMD3-v3-readonly/vrc-get-gui/components/SideBar.tsx` 的精确值为：260px sidebar、12px outer padding、
4px item gap、48px item height、16px item inline padding、20px icon、16px icon/label gap和full-pill shape。v4保留真实
`md-list`/`md-list-item`与Material state layer，只将公开视觉tokens覆盖为这些已验证值；不复制v3组件源码。
