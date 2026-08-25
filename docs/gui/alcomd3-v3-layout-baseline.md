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
- v4 可以为真实用户价值重组个别页面，合并或拆分 secondary surface，并新增 v3 不存在的能力；Core/domain/API 边界不能
  因此逐项变成一级导航。
- 数据密集页面优先延续 title/search/actions/table 的位置习惯与桌面密度；不要求每个按钮逐像素同位，也不能仅把每一项改成
  大卡片后声称是视觉现代化。
- narrow 模式可把持久侧栏转为 modal drawer；wide 模式应保留 v3 可识别的左侧导航和右侧业务画布关系，但无需复制固定
  宽度、旧组件或每个历史页面。
