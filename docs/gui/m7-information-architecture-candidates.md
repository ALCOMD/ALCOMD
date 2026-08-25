# M7 Official GUI information architecture candidates

状态：人工选择输入，全部 `not approved`。本文不授权 H0/H1 production，不改变 route、RPC、Schema、Permission、State 或
Portable UI contract。

## Synthesis constraints

三个候选共同满足：

- wide desktop 保持 ALCOMD3 可识别的 `sidebar + main content` composition；
- primary navigation 体现用户心智模型，不枚举 Rust module、RPC namespace 或 authority boundary；
- Project Packages、project Unity action 和 Backups 优先留在 Project workspace；
- Repositories、Templates 和真实存在时的 User Packages 归入资源型用户语义；
- dense list/table/compact row 是 Projects、Packages、Repositories、Operations、Extensions、Activity 的默认桌面表达；
- page toolbar 承载 title、search/filter、view switch 和 primary action；复杂/high-impact action进入review dialog；
- Material Design 3 负责 component、interaction、tokens、state layer、ripple、elevation、motion 与 adaptive behavior，不替代 IA；
- global route 可以存在而不等于 sidebar top-level entry；deep link、notification、operation follow 或上下文动作可打开子页面；
- `19267230507071dc61ba306b98c8cfdd113e9ea2` 的 typed RPC、hooks/state、workflow、Settings、Activity、Diagnostics、
  Portable UI和Playwright基础设施应尽量复用。

wireframe 顶部的 `ALCOMD3` 行表示 native window title/chrome，不是另加的长期 product app bar；产品内 navigation 与 page
toolbar 从其下方开始。

## Divergence budget

| 分类 | 含义 | 审批规则 |
|---|---|---|
| `STRUCTURAL_CONTINUITY` | 延续 shell、导航深度/分组、信息密度、workspace/action空间关系和产品辨识度 | 必须存在 |
| `MD3_MODERNIZATION` | 组件、颜色、字体、shape、elevation、state layer、ripple、motion与细节spacing现代化 | 允许，需component evidence |
| `V4_NECESSARY_ADDITION` | v3不存在但v4真实能力需要的入口/状态，例如 durable Operation、Portable UI、redacted Diagnostics | 必须说明用户价值和placement |
| `UX_CORRECTION` | 修正v3已知的不易发现、层级混乱或accessibility问题 | 必须说明问题，不以个人偏好替代证据 |
| `ADAPTIVE_CHANGE` | wide/narrow/compact因MD3 adaptive与可访问性改变composition | 允许，但用户语义顺序保持 |
| `UNJUSTIFIED_DEVIATION` | 仅因Core/domain/API独立、实现方便或“重新设计”而新增/拆分结构 | 不得进入推荐方案 |

三个候选均不包含已知 `UNJUSTIFIED_DEVIATION`。若后续实现新增这种变化，应立即停止并回到 IA 审批。

## Candidate A — Project-first classic continuity（Codex recommendation）

### Design summary

Projects 继续承担 home-equivalent；Resources 维持 v3 的包/资源用户语义；Extensions、Activity & Diagnostics、Settings 放入清晰但
不喧宾夺主的 utility group。Operations 不占永久一级导航，而以 sidebar footer 的 active-status/history entry 和业务上下文
progress共同表达。

### Wide desktop wireframe

```text
┌────────────────────────────────────────────────────────────────────────────┐
│ ALCOMD3                                                                    │
├──────────────────┬─────────────────────────────────────────────────────────┤
│ WORK             │ Project name      Unity 2022.3   [Open Unity] [⋮]       │
│ ● Projects       ├─────────────────────────────────────────────────────────┤
│   Resources      │ [Packages] [Unity] [Backups]                            │
│                  │ Search / filter                     Primary action      │
│ TOOLS            ├─────────────────────────────────────────────────────────┤
│   Extensions     │ dense table / list / project workspace                 │
│   Activity       │                                                         │
│   Settings       │                                                         │
│                  │                                                         │
│──────────────────│                                                         │
│ ◷ 2 operations  │                                                         │
│ v4.0 · About     │                                                         │
└──────────────────┴─────────────────────────────────────────────────────────┘
```

### Navigation hierarchy

```text
Projects
  -> Project workspace
       -> Packages
       -> Unity
       -> Backups
Resources
  -> Repositories
  -> Templates
  -> User Packages (only when a real capability exists)
Extensions
  -> Extension detail
       -> Permissions
       -> Portable UI
Activity
  -> Activity
  -> Diagnostics
Settings
  -> General / Appearance
  -> Unity installations
  -> other approved settings
Footer utility
  -> active Operations / history
  -> About / licenses / version
```

“Activity”可在最终文案中命名为“Activity & Diagnostics”或“Logs”；它是一个用户可理解的observability surface，内部
Activity/Diagnostics仍调用不同权限和RPC。

### Main content and page toolbar

右侧始终是一个 task-focused workspace：page toolbar 在顶部放置 title、search/filter、view switch 与当前任务的 primary
action；dense list/table 或 Project workspace 占据主体。Project workspace 自己承载 Packages/Unity/Backups secondary navigation，
Project identity、selected Unity、Open Unity 与 overflow action 保留在同一 header hierarchy。Settings 使用 grouped scrolling
sections，Activity/Diagnostics 使用同一 observability container 的 secondary navigation。

### Feature placement

| Capability | User placement |
|---|---|
| Projects | 默认一级入口与home-equivalent；header提供search/list-grid/create/refresh |
| Project Packages | Project workspace默认tab；dense package table与Plan review |
| Repositories | Resources内tab/subpage；header Add/Refresh，dense table |
| Templates | Resources内tab/subpage；模板列表/详情/context create |
| User Packages | 仅能力真实存在时加入Resources；当前不显示假tab |
| Unity | project-specific selector/open/writer state在Project；global installation management在Settings > Unity |
| Backups | Project workspace tab + Project/Projects context create/restore action |
| Operations | sidebar footer active-status/history utility；业务页面保留Operation follow；完整history可打开子页 |
| Extensions | TOOLS utility一级入口；detail/permissions/Portable UI嵌套 |
| Portable Extension UI | Extension detail子页，host chrome与Core共享Material components |
| Settings | TOOLS utility一级入口，纵向grouped sections |
| Activity | Activity surface默认tab，dense chronological list |
| Diagnostics | Activity surface次级tab，权限/脱敏仍独立 |
| About/licenses | sidebar footer中的version/About action；可进入Settings-style子页 |

### Narrow/adaptive behavior

- sidebar 变为 modal navigation drawer；group label、entry order、operation footer语义不变；
- project toolbar换行或把次要动作收进overflow，但Project name、selected Unity、primary action保持可到达；
- Project/Resources secondary tabs允许水平滚动或紧凑select fallback，不能提升为更多一级drawer entries；
- dense table在窄屏使用priority columns + row detail expansion，不把每行无条件变成巨大card；
- drawer关闭后焦点回到menu button，route进入后焦点到page heading。

### Comparison

**ALCOMD3 v3**

- 延续：Projects landing、资源分组、左侧主导航/右侧workspace、页首toolbar、project-context action、settings/logs/footer utility、
  桌面信息密度。
- 改变：把v4 Extensions管理明确为utility entry；Operation成为footer/history；Activity/Diagnostics替代v3 Activity/Technical
  Logs；global Unity installation继续在Settings但使用v4 read model。
- 连续性：用户仍从Projects和Resources进入核心工作，并在相同空间位置寻找search/create/open/backup/log/settings。

**当前 192672 GUI**

- 修正：删除长期top app bar和11项flat domain navigation；合并Repositories/Templates；移动Unity/Backups到Project；合并
  Activity/Diagnostics；About/Operations移到footer utility；恢复dense toolbar/list。
- 保留：现有route identity、Home/status数据可作为footer/status或secondary surface；全部functional wiring与状态逻辑保留。
- 移动/合并：`/unity`、`/operations`、`/about` route可继续存在供deep link，但不自动占sidebar top level。

**v4 architecture**

- Durable Operation由全局status/history和context follow容纳；不泄漏为永久domain nav。
- Extension/Portable UI有清晰入口，但仍是工具管理语义，不获得私有authority。
- Activity和Diagnostics呈现合并，permission/RPC保持分离。

### Divergence classification

| Change | Class |
|---|---|
| sidebar + content canvas、Projects/Resources工作模型 | `STRUCTURAL_CONTINUITY` |
| Material components/tokens/ripple/adaptive surfaces | `MD3_MODERNIZATION` |
| durable Operations footer/history | `V4_NECESSARY_ADDITION` |
| Activity + Diagnostics shared surface | `V4_NECESSARY_ADDITION` + `UX_CORRECTION` |
| global Unity in Settings、project Unity in workspace | `STRUCTURAL_CONTINUITY` + `V4_NECESSARY_ADDITION` |
| narrow modal drawer / priority columns | `ADAPTIVE_CHANGE` |

### Strengths

- 产品连续性最强，一级入口少且语义稳定；
- Project/Resources两个最常用工作区最直接；
- 能容纳v4 durable state而不把domain model泄漏到sidebar；
- 从当前GUI迁移主要是composition/component重构，业务逻辑复用率高。

### Weaknesses

- Extensions和Operations的v4平台价值较不显眼；
- Activity名称需要在Logs/History/Activity & Diagnostics之间做文案验证；
- footer Operations必须有清晰badge/live-state，否则历史入口可能不易发现。

### Migration cost from current GUI

中等：重写shell/nav/page toolbar composition，合并资源和observability页面容器；保留大多数route、typed client、query/action
components与tests fixture，逐步把native controls换成`@alcomd/ui`。

## Candidate B — Three workspace hubs

### Design summary

侧栏只表达三个长期工作区：Projects、Resources、Platform。Platform容纳global Unity installations、Extensions和Operations。
Activity与Settings保持utility entry，About/version在footer。该方案比A更强调v4“本地应用平台”，但增加一层workspace hub。

### Wide desktop wireframe

```text
┌────────────────────────────────────────────────────────────────────────────┐
│ ALCOMD3                                                                    │
├──────────────────┬─────────────────────────────────────────────────────────┤
│ WORKSPACES       │ Platform                                                │
│ ● Projects       │ [Unity] [Extensions] [Operations]                       │
│   Resources      ├─────────────────────────────────────────────────────────┤
│   Platform       │ Search / filter                         Context action   │
│                  ├─────────────────────────────────────────────────────────┤
│ UTILITIES        │ dense installation / extension / operation list         │
│   Activity       │                                                         │
│   Settings       │                                                         │
│                  │                                                         │
│──────────────────│                                                         │
│ v4.0 · About     │                                                         │
└──────────────────┴─────────────────────────────────────────────────────────┘
```

### Navigation hierarchy

```text
Projects
  -> Project workspace: Packages / Unity / Backups
Resources
  -> Repositories / Templates / User Packages when available
Platform
  -> Unity installations
  -> Extensions -> detail -> Portable UI
  -> Operations
Activity
  -> Activity / Diagnostics
Settings
Footer
  -> About / licenses / version
```

### Main content and page toolbar

进入 Projects 或 Resources 后，右侧仍使用 v3-continuous page toolbar + dense workspace；进入 Project 后使用 Packages/Unity/
Backups secondary navigation。进入 Platform 后，toolbar 显示 Platform title、Unity/Extensions/Operations secondary navigation、
search/filter 与当前子页 action，主体按子页显示 dense registry/list，而不是三个 domain summary card。Settings 与 Activity/
Diagnostics 保持各自的 utility workspace。

### Feature placement

| Capability | User placement |
|---|---|
| Projects | 默认一级入口与 home-equivalent；toolbar提供search/list-grid/create/refresh |
| Project Packages | Project workspace默认tab；dense package table与Plan review |
| Repositories | Resources > Repositories；header Add/Refresh与dense table |
| Templates | Resources > Templates；list/detail与context create |
| User Packages | 仅真实能力存在时成为Resources子页；当前不显示假入口 |
| Unity | project action留在Project；global installations在Platform > Unity |
| Backups | Project workspace tab + Project/Projects context create/restore |
| Operations | Platform > Operations；active operation仍在业务上下文显示 |
| Extensions | Platform > Extensions及其detail子页 |
| Portable Extension UI | Extension detail子页，host chrome与Core共享Material components |
| Settings | 独立utility，不承载Unity installation registry |
| Activity | Activity utility surface默认tab，dense chronological list |
| Diagnostics | 同一surface的次级tab，权限/脱敏仍独立 |
| About/licenses | footer utility |

### Narrow/adaptive behavior

- drawer只显示三个workspace与两个utility，进入workspace后用顶部tabs或次级drawer选择subpage；
- narrow下Platform的Unity/Extensions/Operations不能同时成为一级drawer item；
- project/resource table策略与Candidate A相同。

### Comparison

**ALCOMD3 v3**

- 延续：侧栏数量级、Projects/Resources、右侧workspace、toolbar和dense data；Settings/Logs仍为utility。
- 改变：新增Platform hub，Unity installation从Settings移入Platform，Extensions/Operations在同一v4 hub。
- 连续性：外壳与核心Projects/Resources仍熟悉，但“Platform”是v3没有的新用户概念。

**当前 192672 GUI**

- 修正：11项flat nav收敛为3+2；Project/Resources恢复上下文；Activity/Diagnostics合并；About移到footer。
- 保留：当前独立Unity、Extensions、Operations页面可以较直接地组合进Platform tabs，迁移成本低于A。
- 删除/移动：Home不再一级；Backups不再独立；Repositories/Templates合并。

**v4 architecture**

- 为global registries、extension runtime和durable operation提供清晰容器；
- 风险是“Platform”可能接近内部架构语言，必须通过用户研究证明用户理解，而不能因为这些模块都属于v4就合并。

### Divergence classification

| Change | Class |
|---|---|
| shell、Projects/Resources、dense workspace | `STRUCTURAL_CONTINUITY` |
| Platform hub | `V4_NECESSARY_ADDITION`，approval required |
| Unity从Settings迁到Platform | `UX_CORRECTION`或`V4_NECESSARY_ADDITION`，需证据 |
| Material/adaptive behavior | `MD3_MODERNIZATION` + `ADAPTIVE_CHANGE` |

### Strengths

- 一级sidebar最简洁；
- 当前Unity/Extensions/Operations页面较容易复用；
- v4新能力有一致容器，未来扩展不必继续增加一级入口。

### Weaknesses

- Platform语义可能对只想管理VRChat/Unity项目的用户过于抽象；
- Unity和Extensions/Operations之间的用户任务关联弱；
- 次级层级更深，可能降低常用Extensions/Unity管理可发现性。

### Migration cost from current GUI

中等偏低：保留更多现有独立页面，主要新增workspace容器、二级tabs和shell重组；仍需完成Material component migration与
dense layout realignment。

## Candidate C — Overview-assisted next generation

### Design summary

保留一个克制的Overview作为v4新增产品入口，展示daemon状态、recent projects和active operations；Projects、Resources、
Extensions是三个工作入口。Activity和Settings为utility，About/version与operation status位于footer。Overview不是大卡片dashboard，
也不复制所有domain summary。

### Wide desktop wireframe

```text
┌────────────────────────────────────────────────────────────────────────────┐
│ ALCOMD3                                                                    │
├──────────────────┬─────────────────────────────────────────────────────────┤
│ ● Overview       │ Overview                       connection: ready         │
│   Projects       ├─────────────────────────────────────────────────────────┤
│   Resources      │ Recent projects        Active operations                │
│   Extensions     │ compact rows           compact rows                     │
│                  │                                                         │
│ UTILITIES        │ Attention / recent activity (bounded, redacted)          │
│   Activity       │                                                         │
│   Settings       │                                                         │
│                  │                                                         │
│──────────────────│                                                         │
│ ◷ 2 operations  │                                                         │
│ v4.0 · About     │                                                         │
└──────────────────┴─────────────────────────────────────────────────────────┘
```

### Navigation hierarchy

```text
Overview
Projects -> Project workspace: Packages / Unity / Backups
Resources -> Repositories / Templates / User Packages when available
Extensions -> detail -> Permissions / Portable UI
Activity -> Activity / Diagnostics
Settings -> Appearance / Unity installations / approved settings
Footer -> active Operations / history; About / licenses / version
```

### Main content and page toolbar

Overview 使用 compact rows 展示 bounded recent/active/attention，不复制管理页面；其余 Projects、Resources 与 Project workspace
沿用 Candidate A 的 dense toolbar/list/tab 结构。Extensions 右侧 workspace 使用 list -> detail -> Permissions/Portable UI 的嵌套关系。
Settings 与 Activity/Diagnostics 仍是 utility workspace，title/search/filter/context action 均留在各自 page toolbar。

### Feature placement

| Capability | User placement |
|---|---|
| Projects | 一级入口；recent projects也可从Overview进入 |
| Project Packages | Project workspace默认tab；dense package table与Plan review |
| Repositories | Resources > Repositories；header Add/Refresh与dense table |
| Templates | Resources > Templates；list/detail与context create |
| User Packages | 仅真实能力存在时成为Resources子页；当前不显示假入口 |
| Unity | project-specific action留在Project；global installations在Settings > Unity；Overview只显示需处理的安全摘要 |
| Backups | Project workspace tab + Project/Projects context create/restore |
| Operations | active summary在Overview与footer，完整history为utility route |
| Extensions | Extensions一级入口与嵌套detail |
| Portable Extension UI | Extension detail子页，host chrome与Core共享Material components |
| Settings | utility一级入口 |
| Activity | 共同utility surface默认tab；Overview只显示bounded attention items |
| Diagnostics | 同一surface的次级tab，权限/脱敏仍独立 |
| About/licenses | footer |

### Narrow/adaptive behavior

- modal drawer保留Overview/Projects/Resources/Extensions和utility grouping；
- Overview从双栏compact rows变为单列，不变成巨大summary cards；
- active operation使用固定但不遮挡内容的status entry，不复制完整progress dialog；
- Project/Resources的nested结构与Candidate A相同。

### Comparison

**ALCOMD3 v3**

- 延续：sidebar/content、Projects/Resources、dense rows、page toolbar、context workspace与footer utility。
- 改变：增加Overview和Extensions一级入口；Activity/Diagnostics合并；global Unity仍在Settings。
- 连续性：主要工作区仍熟悉，但启动第一眼从Projects变为Overview，是三个候选中最大的新结构。

**当前 192672 GUI**

- 修正：Home不再是只显示四张大status cards的domain dashboard，而是受限的recent/active/attention入口；11项nav收敛；
  Repositories/Templates合并；Unity/Backups移入context；Activity/Diagnostics合并；Operations/About移到footer。
- 保留：Home route、status query、Extensions一级route与current recent/status composition可部分重用。
- 删除：top Settings shortcut、flat Unity/Operations/Diagnostics/About entries和大面积generic cards。

**v4 architecture**

- Overview合理容纳daemon connection、active Operation和recent Project，但必须只呈现真实、安全、bounded数据；
- Extensions成为一级是产品战略选择，不是因为extension domain独立；需项目所有者确认其使用频率和重要性。

### Divergence classification

| Change | Class |
|---|---|
| shell、Projects/Resources/context workflows | `STRUCTURAL_CONTINUITY` |
| Overview | `V4_NECESSARY_ADDITION`，approval required |
| Extensions一级入口 | `V4_NECESSARY_ADDITION`，approval required |
| compact recent/active layout | `UX_CORRECTION` |
| Material/narrow behavior | `MD3_MODERNIZATION` + `ADAPTIVE_CHANGE` |

### Strengths

- daemon连接、active Operation和recent Projects有自然入口；
- 比当前GUI明显更克制，同时保留一部分现有Home/Extensions composition；
- 对未来真正的平台级能力有更高可发现性。

### Weaknesses

- 与v3以Projects为home-equivalent的连续性最弱；
- Overview很容易再次退化成domain dashboard、大卡片和低信息密度；
- Extensions一级入口是否符合大多数用户心智模型尚无证据。

### Migration cost from current GUI

中等偏低：可保留Home/Extensions route和status logic，但必须重构Overview内容密度、shell、资源分组、Project context与Material
component layer。

## Direct comparison of the required questions

| Question | Candidate A | Candidate B | Candidate C |
|---|---|---|---|
| A. Projects home-equivalent | yes | yes | no，Overview landing |
| B. Repository/Template/User Package | Resources group | Resources hub | Resources group |
| C. Unity global vs project | Settings vs Project | Platform vs Project | Settings vs Project |
| D. Backup ownership | Project workspace | Project workspace | Project workspace |
| E. Operation placement | footer/status/history utility | Platform subpage + context | Overview/footer/history utility |
| F. Extensions placement | utility entry | Platform subpage | primary entry |
| G. Activity + Diagnostics | one utility surface, separate tabs/contracts | same | same |
| H. About placement | footer utility | footer utility | footer utility |

## Codex recommendation（not approved）

推荐 **Candidate A** 作为项目所有者下一轮 refinement 的起点，理由：

1. 它最直接保留v3的产品识别性、导航数量级、Projects/Resources心智模型、信息密度和context action习惯；
2. 它容纳Operation、Portable UI、Diagnostics等v4真实新增能力，但没有把这些authority namespace全部提升为sidebar entry；
3. 它对`192672…`的功能代码复用仍然充分，主要成本集中在presentation/composition/component layer；
4. 相比B，它不要求用户先理解抽象的Platform hub；相比C，它避免Overview再次演变成与v3无关的domain dashboard。

建议从B吸收的要点：为global Unity/Extensions/Operations保持可复用的secondary route和清晰deep link。建议从C吸收的要点：
在sidebar footer或Projects header提供connection/active-operation摘要，但不必建立永久Overview landing。

本推荐不是批准。项目所有者可以选择A/B/C、组合其明确部分，或要求第四个候选；在选择前不得开始H0/H1 production。
