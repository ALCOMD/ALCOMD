# ALCOMD3 v3 GUI 宏观布局基线

状态：M7 visual realignment 的只读设计输入；不是源码上游，也不是 M11 的完整 differential parity 证据。本文记录 v3
具体事实以识别产品连续性，不把其导航树、像素尺寸或每个历史页面冻结为 v4 目标。

## 证据边界

- 冻结参考仓库：`../ALCOMD3-v3-readonly`；读取到的 `main` 提交为
  `4aa98ae4f18d42c10137278997180dbede991e88`。
- 主要源码证据：`vrc-get-gui/app/_main/route.tsx`、`components/SideBar.tsx`、
  `components/sidebar-extension-definitions.tsx`、`components/layout.tsx` 与各 route 文件。
- 可视证据：仓库已有 `docs/release-assets/ALCOMD3-BOOTH-1.png` 和
  `docs/release-assets/ALCOMD3-BOOTH-2.png`。这些截图来自较早可见版本，只用于交叉确认持久侧栏、右侧内容画布、页内工具栏、
  列表/表格、分段导航与对话框的宏观关系；最终入口名称和顺序以冻结源码为准。
- 本次未启动 v3 GUI，未联网，未修改 v3 repository，也未采集或伪造新截图。
- 本文只研究 visible information architecture、layout、navigation、control placement 和 user flow；不得据此复制、移植、
  包装或改写 v3/vrc-get 源码。

M11 仍负责真实脱敏 Fixture、迁移与 release-grade exhaustive differential parity。本文不能解除
`gui.v3-entry-parity = blocked`。

## User-model interpretation

本文中的具体入口和页面用于证明 v3 用户模型，而不是要求 v4 逐项复刻。该用户模型的主要区域是：

- Projects 及其 package-centric Project workspace；
- Packages & Templates 资源区域及其 Repositories、User Packages、Templates secondary views；
- Extensions management 与 extension-specific entries；
- Settings / UI Theme utility；
- Log / Activity / Technical observability utility；
- context progress、footer version/update 与 Settings 内的 licenses/system information。

特别是，Extensions 不是 v4 新增用户概念。v4 的 Extension Host、permission/scope、quarantine 与 Portable UI 是既有
Extensions 用户任务的增强实现，默认应进入 Extensions detail，而不是成为重新分类 Extensions 的理由。

v4 默认继承该用户模型。只有 v3 没有对应用户用例、用户确实需要独立工作区、无法自然容纳于既有概念且独立入口可明确提升
可用性时，才考虑新增 user-level surface。

## A. 主窗口

### Window structure

- 应用主体是左右两区：左侧持久导航，右侧单一主内容画布。
- 常规宽度下左栏固定约 260 px；compact 模式收窄为图标导航。右侧内容区占据其余空间。
- 主内容区本身是带圆角的 surface，内部由页面纵向布局、顶部工具栏和可滚动/可伸缩主体组成。
- v3 没有另加一条长期占高的产品级 top app bar；Windows/macOS/Linux 原生标题栏之外，页面标题和动作位于右侧内容区的
  页内工具栏。

### Title/header area

- 主要页面共享 `HNavBar`：标题或分段导航在左，搜索和上下文操作填充中部/右侧，主要创建/添加操作通常靠右。
- 工具栏下面是页面的单一主内容卡片、列表、表格或滚动设置卡片组。
- 项目详情继续使用同一顶部工具栏：返回、项目名称/安全路径摘要、Unity 版本选择、打开 Unity 与更多动作集中在页面首部。

### Settings/access placement

- Settings 是左侧一级入口。
- UI Theme 是左侧可直接打开的动作，而不是全局 top bar 的独立 Settings 快捷按钮。
- Extensions 固定靠近侧栏底部；版本/更新入口位于底部状态区。运行时可见的 MCP、Discord 等 extension entry 仍处于同一
  侧栏体系。

## B. 导航

冻结源码的默认侧栏顺序事实为：

1. Projects；
2. Packages & Templates；
3. MCP；
4. UI Theme；
5. Settings；
6. Log；
7. Discord。

另有：

- Extensions management 固定在主体入口之后、版本区之前；
- version/update 与 hostname warning 位于底部；
- extension entry 可显示、隐藏和排序，但 Projects、Packages、Settings 是不可隐藏的结构锚点；
- 当前选中项以整行 pill/container 标识；compact 时保留图标与 tooltip。

### 层级

- v3 没有独立 Home 页面；Projects 是启动后的主要工作入口和 home-equivalent。
- Packages & Templates 是一个一级分组，页内三段为 VPM Repositories、User Packages、Templates。
- Unity 主要存在于 Project Manage 顶部工具栏和 Settings 的 Hub/Editor 管理，不是独立一级页面。
- Backup create/restore 是 Projects/Project Manage 的动作与进度流程；backup path/format 属于 Settings，不是独立一级页。
- Operations 没有独立历史页；长任务以 modal/progress host 与可最小化任务入口呈现。
- Activity 与 Technical Logs 位于同一 Log 页面内的分段视图。
- Licenses、system information 和 about-like information 属于 Settings 及其 Licenses 子页。

## C. 主要页面

### Home

v3 没有独立 Home route。应用完成 setup 后以 Projects 作为默认主要工作区。版本/更新状态在侧栏底部，系统信息在
Settings。这证明 Projects 的 home-equivalent 角色是重要连续性参考，但不预先禁止 v4 提供有明确用户价值、克制且不泄漏
domain namespace 的 Overview。是否保留该 surface、它是否承担 landing，必须在 IA 候选中与 Projects landing 明确比较。

### Projects

- Header：Projects 标题、刷新、搜索、list/grid 切换、右侧 Create Project split action。
- Body：单一列表或网格卡片区；empty state 在同一主体内提供 create/import/restore 入口。
- Row/card：项目摘要与 Manage、Backup 等常用动作；完整私密路径不是导航 identity。
- Restore from backup 从 Projects 创建动作附近进入，而不是要求用户先进入独立 Backup catalog。

### Project manage / Packages

- Header：返回、项目名称、保存位置摘要；右侧 Unity version selector、Open Unity split action 和 overflow actions。
- Body：package management table/list 是详情主内容，包含搜索/filter、已安装/可用版本与 source/actions。
- Apply：package change 先在确认 dialog 中集中显示安装、升级、降级、删除、冲突和风险，再进入进度状态。
- Backup、copy、remove 等项目级动作放在 Project context 的 overflow/menu 中。

### Packages / Repositories / Templates

- 一级导航只有 Packages & Templates。
- 页内顶部使用三段 selector：VPM Repositories、User Packages、Templates。
- Repositories：header 左侧是分段 selector，右侧 Add Repository split action；body 是密集表格/列表，操作与排序紧邻 row。
- User Packages：保留同一分段 selector 和页内工具栏，body 是用户 package 列表。
- Templates：保留同一分段 selector，body 是模板列表/卡片；创建项目等动作围绕选中模板展开。

### Unity

v3 没有独立 Unity 一级页：

- Project Manage header 选择项目使用的 Unity version 并提供 Open Unity；
- Settings 管理 Unity Hub path、已发现/手工配置的 Editor；
- 版本迁移使用 dialog/confirmation/progress，仍从具体 Project context 进入。

### Backups

v3 没有独立 Backup 一级页：

- Project card/Project Manage 发起 backup；Projects header 发起 restore；
- dialog 收集名称、格式、exclude VPM packages 等输入并展示 progress/cancel/minimize；
- 设置页保存 backup path 与 format；
- 正在执行的 backup/restore 可在全局 progress host 恢复查看。

### Operations / equivalent v3 UX

v3 没有 durable Operation catalog。等价 UX 是 modal progress、可最小化的 project progress task 和完成/失败 toast。v4 的
durable OperationId 是架构新增事实；它可以由业务上下文、全局 status/history utility、次级 surface 或经论证的独立区域承载。
是否需要一级入口取决于用户心智模型和使用频率，不能由 Operation RPC/domain 边界自动决定；动作入口仍应留在原业务上下文。

### Extensions / v4 equivalent

- Extensions management 靠近侧栏底部；页面负责 extension 安装、启用、可见性和侧栏排序。
- MCP 与 Discord 可作为侧栏 extension entry 直接到达。
- v4 Portable UI 必须继续由 host shell 提供统一 chrome；不能通过扩展自定义 DOM/CSS 改写应用导航。

### Settings

- Header 仅显示 Settings；主体是一个纵向滚动的卡片组。
- 卡片按 Unity Hub/Editor、项目/backup 路径与格式、外观/compact/animation、locale、update、deep link、legacy import、
  licenses/contributors/system information 等主题组织。
- Theme 有侧栏快捷入口，但设置本身仍归属于 Settings 组织，不形成另一套产品 authority。

### Logs / Diagnostics / About

- Log header：Log 标题、Activity/Technical 分段 selector、搜索与右侧过滤/打开目录动作。
- Activity 视图有 source/status/kind 与 secondary/details filters；Technical 视图有 log level 和 auto-scroll。
- v3 没有独立 redacted Diagnostics 页面；v4 的 Activity/Diagnostics 权限分离是 A-026 带来的结构变化。
- Licenses 与 system information 位于 Settings，版本/更新位于侧栏底部；v3 没有独立 About 一级页。

## D. 通用交互模式

| 意图 | v3 可见模式 |
|---|---|
| add/create | 页首工具栏右端的 filled/split action；复杂输入进入 dialog |
| remove/destructive | row overflow 或 context menu 发起，dialog 明确确认 |
| refresh | 页标题或 section 附近的 icon action，保留原内容区 |
| search/filter | 页首工具栏中部搜索，必要时第二行 filter toolbar |
| install/update | 项目 package table 中选择版本/动作，集中 review 后执行 |
| confirmation | modal dialog，headline/content/footer actions，危险动作明确区分 |
| progress | modal/progress host，可取消或最小化；业务入口仍可恢复正在执行的任务 |
| error/empty | 保留在页面主体或 dialog 上下文，不把 permission/error 冒充空列表 |
| settings | 纵向滚动卡片组；相关字段与解释在同一卡片 |

## E. 用于 v4 MD3 转译的视觉结构

- 识别性来自“sidebar + main content”的整体 shell、有限层级的用户语义分组、页内工具栏、桌面友好信息密度与
  list/detail/workspace 空间关系，不是来自某个具体颜色值、260 px 尺寸或固定入口清单。
- 选中导航、分段 selector、filled/tonal action、rounded surface 与 modal 是可转译到 MD3 的结构锚点。
- v4 可以改变 color、typography、corner、elevation、spacing、icon、motion 和 adaptive behavior；不复制 v3 CSS/Tailwind/Radix
  实现。
- v4 可以在有明确用户价值时重组个别页面或 secondary surface，但默认先把新可靠性/安全机制嵌入既有用户 workflow。只有通过
  user-level independent-workspace test 才考虑新增 surface；Core/domain/API 边界不能逐项变成一级导航。
- 数据密集页面优先延续 title/search/actions/table 的位置习惯与桌面密度；不要求每个按钮逐像素同位，也不能仅把每一项改成
  大卡片后声称是视觉现代化。
- narrow 模式可把持久侧栏转为 modal drawer；wide 模式应保留 v3 可识别的左侧导航和右侧业务画布关系，但无需复制固定
  宽度、旧组件或每个历史页面。

## F. Reference-driven visual decomposition

本节是 2026-08-26 Visual Gate 1 重置后的直接视觉实现输入。它不从“现代桌面应用”或 Web dashboard 惯例推导界面，
只记录发布图中实际可见的关系以及冻结源码明确表达的几何。发布图是合成宣传图，Projects 主体的一部分被 Theme dialog
遮挡；被遮挡部分只采用冻结源码可验证的结构，不猜测不可见像素。

### F.1 Projects wide desktop

证据：`../ALCOMD3-v3-readonly/docs/release-assets/ALCOMD3-BOOTH-1.png` 中两张 Projects 窗口，以及
`app/_main/route.tsx`、`components/SideBar.tsx`、`components/layout.tsx`、`app/_main/projects/index.tsx`、
`-projects-list-card.tsx`、`-project-row.tsx`、`-projects-grid-card.tsx` 和 `-project-grid-item.tsx`。

1. **Window composition**：原生标题栏下面立即进入二栏应用主体；没有第二条产品级 top app bar。左栏是背景上的持久导航，
   右栏是一个接近窗口全高的大圆角内容画布。
2. **Sidebar geometry**：冻结源码的常规宽度是 260 px。按发布图中的常规桌面窗口估算，侧栏约占可用宽度的 18%–21%，
   主画布约占 79%–82%。侧栏不是窄 rail，也不是带独立卡片边界的 admin navigation。
3. **Sidebar icon + label**：每项单行水平排列，20 px 图标在前、标签在后，二者约 16 px 间隔；常规项高 48 px，横向
   padding 16 px。所有主入口使用同一视觉重量，不加 `WORKSPACE` / `SYSTEM` 分组标题。
4. **Selected navigation shape**：选中状态覆盖整行，使用 full pill/container；不是左侧细线，也不是独立卡片。未选中项保持透明，
   hover 与 selected 共享同一 surface-container 家族。
5. **Main content outer margin**：主画布相对窗口顶部、右侧和底部约 12 px，左侧紧邻 sidebar 的内容边界；视觉上形成一块完整的
   大工作区，而不是背景中央的 max-width card。
6. **Main rounded surface**：主画布圆角为约 28 px，内部 padding 常规为 24 px；画布占满余下高度。页面内容与画布边缘之间的
   24 px inset 明显大于页面内部连续控件间的 8–12 px gap。
7. **Page toolbar**：顶部 `HNavBar` 是页面内工具栏，约一行 40 px 控件加上下 8 px padding，即约 56 px；它与下方主体之间
   只有约 12 px 间隔。工具栏自身横向充满画布，不再套一层居中的页面容器。
8. **Search placement**：标题和刷新之后立即是可伸展搜索框；搜索承担中段弹性空间，而不是另起一行或放入筛选卡片。
9. **Refresh placement**：刷新是紧邻 Projects 标题的 icon action；加载时原地旋转，不改变工具栏结构。
10. **View toggle**：搜索之后是带 list/grid 图标和文字的轻量 action；它与搜索、标题属于同一 toolbar leading group。
11. **Primary action**：最右侧是 Create Project filled split action；主按钮直接创建，窄下拉段承载 Add existing / Restore 等邻近入口。
    它是工具栏右端的唯一主要视觉强调。
12. **Sort/filter row**：list mode 直接把排序放进 sticky table header；grid mode 才在内容前放一个约 40–48 px 的紧凑
    secondary toolbar（label、160 px select、方向 icon）。不存在占据大块高度的筛选面板。
13. **Project item geometry**：list 是 full-width dense table；每个 cell 约 10 px 内边距，名称单元允许两行信息，动作集中在最右。
    grid card 使用 1/2/3 列容器查询，常规双列阈值约 40.75 rem、三列约 67.5 rem，每卡约 20–22 rem 的可读宽度，卡间距
    约 12 px。
14. **Metadata hierarchy**：名称是主文本；路径是 14 px、约半透明的次文本；类型以 20 px 图标加标签呈现；Unity version、创建时间、
    最近修改时间是同层紧凑 metadata。完整路径是 secondary detail，不成为巨型 identity heading。
15. **Row/card actions**：Open Unity 是强调动作，Manage/Migrate 与 Backup 紧邻，overflow 承载 open folder/copy/remove；favorite 是
    最左或卡片右上角的轻量 icon state。常用动作不被搬到独立详情 dashboard。
16. **Scroll behavior**：sidebar 可独立滚动；主 canvas 固定；toolbar 不滚；项目表/网格在剩余高度内滚动。list header sticky，
    水平滚动只在表格不足以容纳列时出现。
17. **Information density**：toolbar、secondary toolbar、row/card 之间保持 8–12 px 节奏；宽屏首屏应出现多行项目或多张卡片，
    不以 hero、营销留白或巨型 empty card消耗工作区。
18. **Typography hierarchy**：页面标题约 24 px、normal；项目名称为普通正文强调；metadata 约 12–14 px；没有 eyebrow，也没有
    40–56 px display headline。字体是 Noto Sans/system sans。
19. **Utility/footer placement**：Extensions 位于主导航之后、底部 version/update 之前；hostname warning 等状态也在 sidebar footer。
    版本、更新和 utilities 不形成横跨窗口的 global web header。

Empty state 仍必须保留完整 shell、toolbar 和内容滚动骨架。v3 源码的 empty card 居中且有 create/import 动作，但 v4 Visual Gate 1
只保留紧凑 contextual empty message；Register/New Project 必须由 toolbar action 打开 dialog 或专用 flow，不在页面常驻表单。

### F.2 Other v3 page families

| 页面 | frame / toolbar | content organization | action placement / density |
|---|---|---|---|
| Project management / package workspace | 同一 canvas 和 `HNavBar`；左侧 back + project name/path，右侧 Unity selector、Open Unity split action、overflow | 单一 package table/list 占满剩余高度；可在表格前出现窄建议条 | refresh/search/filter 在 package header；Apply review、migration、backup 等进入 dialog/progress host |
| Resources | sidebar 一级入口 `Packages & Templates`；canvas 顶部是 Repositories/User Packages/Templates 三段 selector | repository 为 dense table；packages 为 list；templates 为 list/card | Add Repository 等 primary action 位于 toolbar 右端；排序和 row actions 贴近数据 |
| MCP | 与其他页面相同的标题 toolbar | 主体是按状态/工具组织的紧凑 section 与 tool grid，不改变 shell | connect/config/action 位于相应 section；技术详情不提升为全局 dashboard |
| Theme | sidebar utility action打开 modal panel，而非独立全局 header page | dialog 内左右两列：color/sliders 与 mode/scheme，窄屏再单列 | reset 在 dialog title row，Close 在 footer；外层 Projects 页面保持可见 |
| Settings | 简单 Settings toolbar | canvas 内纵向滚动 settings cards，按 Unity、paths、appearance、locale、update、legacy、licenses/system 分组 | action/switch/select 与说明文字留在同一 section，页面不使用 hero |
| Log | Log 标题 + Activity/Technical segmented selector + search/filter/open-directory | dense activity cards/list 或 technical log stream，主体自行滚动 | filter 与 auto-scroll 属于 toolbar/secondary controls；details 就地展开 |
| Discord | 标准标题 toolbar | 紧凑 status/config cards；宽屏可用信息/预览双列，但仍在同一 canvas | connect/enable/test 等动作留在对应 section，不形成独立 admin landing |
| Extensions | 标准 Extensions toolbar | sidebar-order card + installed/available sections；extension items为密集 list/card grid | visibility/reorder/enable/action 与 extension item 同位；Portable UI 是 detail，不拥有应用 chrome |

这些页面共享的事实不是“每页都做卡片”，而是 `page toolbar -> 一块主要工作内容 -> 必要时局部 secondary controls/dialog`。
卡片只表达确有边界的设置组、extension item 或项目卡，不把每个标题和字段再次套入嵌套 surface。

## G. v3 component/layout vocabulary

以下名称只用于 M7 implementation vocabulary，不构成公共 UI framework 或 Extension UI 合同：

| vocabulary | v3 可见职责 | 不得演变为 |
|---|---|---|
| `AppShell` | 原生标题栏下的 sidebar + content canvas | 全局 Web header + dashboard body |
| `Sidebar` | 常规 260 px、可 compact 的持久一级导航 | admin 分组树 |
| `SidebarItem` | 48 px icon + label row，selected full pill | route/domain 自动生成器 |
| `SidebarUtilityItem` | Extensions、version/update、warning 等底部项 | 第二套 top-bar utility nav |
| `ContentCanvas` | 填满剩余窗口的大圆角 surface | centered max-width page card |
| `PageToolbar` | 页面标题、search/context actions、primary action | hero/header banner |
| `SearchField` | toolbar 中可伸展搜索 | 独立搜索页面 |
| `ToolbarAction` | refresh/view/overflow 等局部动作 | 通用 command bus |
| `SecondaryToolbar` | grid sort、table filter 等紧凑第二行控件 | 大型 filter panel |
| `DenseGrid` | 1/2/3 列项目或 tool items | marketing card gallery |
| `DenseList` | 可滚动的短行列表 | 无限嵌套 feed |
| `ProjectCard` | project identity、metadata、常用 actions | project dashboard |
| `DataTable` | sticky sortable header + dense rows | 自有通用 data-grid framework |
| `SettingsSection` | 同主题字段、说明和 action 的局部 surface | 每字段一张卡 |
| `LogView` | segmented activity/technical stream | 独立 analytics dashboard |
| `ContextActions` | row/card/project toolbar 的主次动作与 overflow | 全局 action palette |
| `ProgressDialog` | review/progress/cancel/minimize 的 modal host | 第二套 Operation engine |

## H. MD3 translation contract for Visual Gate 1

| v3 element | v4 MD3 expression | preserved relationship |
|---|---|---|
| selected sidebar row | selected state/container + Material state layer | full-row pill、icon-label spacing、48 px rhythm |
| content canvas | `surface-container` family + large shape | fills remaining window、12 px outer margin、24 px inset |
| page title | MD3 title typography | 24 px class hierarchy；no eyebrow/display hero |
| Create Project | Material Web filled Button | toolbar far right、split/adjacent menu semantics |
| search | Material Web TextField | between refresh and view control、flex-grow |
| grid sorting | Material Web Select + icon action | compact secondary toolbar immediately above grid |
| project row/card | MD3 surface/color/state/shape | dense metadata and action hierarchy unchanged |
| create/register flow | Material Web Dialog and fields | invoked from toolbar; never persistent page form |
| refresh/view actions | Material Web Button/IconButton state/ripple/focus | remain in page toolbar, no global header |

Visual Gate 1 的 first runnable slice 只实现 wide desktop 的 `AppShell + Sidebar + ContentCanvas + Projects PageToolbar +
SecondaryToolbar + Dense Project Content`。其他页面分析只冻结后续共同空间语言，不授权同时修改它们。至少 Button、TextField、
Select 必须经 `@alcomd/ui` 渲染真实 Material Web 2.5.0 元素，并以可观察的 focus/state/disabled/ripple 证据验收。
