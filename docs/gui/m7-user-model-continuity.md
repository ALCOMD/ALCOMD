# M7 user-model continuity and minimum-change IA proposal

状态：项目所有者已批准为 M7 official GUI product IA baseline，并批准 H0/H1 production；H1 后必须停止在 Visual Gate 1。

本文以用户实际看到、理解和完成的任务为轴，而不是以 Rust crate、RPC namespace、State table 或内部 authority 为轴。默认
产品模型是：

```text
ALCOMD3 v3 user structure
    + Material Design 3 modernization
    + safer, more reliable and more observable v4 implementation
```

只有同时证明以下四点，才考虑新增独立用户级 surface：

1. v3 没有对应用户用例；
2. 用户需要把它理解为独立工作区；
3. 它无法自然容纳进既有 v3 用户概念；
4. 独立入口能够明确提升可用性。

Core module、RPC namespace、State table 或独立 subsystem 均不是证据。

## Classification

- `SAME`：用户目标与主要 workflow 不变，只替换实现。
- `ENHANCED_EXISTING`：用户仍在完成 v3 已有任务，v4 增加安全、可靠性、可观察性或更完整的控制。
- `GENUINELY_NEW`：v3 没有对应用户任务；这仍不自动意味着一级导航。
- `INTERNAL_ONLY`：实现或合同概念，不应成为独立 GUI surface。

## v3 user-facing capability map

| User area / placement | User-facing entry | Page-internal capability | Workflow and context |
|---|---|---|---|
| Primary work area | Projects；启动后的 home-equivalent | project list/grid、search、refresh、create/import/restore、Manage、Backup | 用户先找到 Project，再进入管理、打开 Unity、备份或移除等 project-context workflow |
| Project workspace | 从 Projects 进入 Project Manage | project identity、Unity version selector、Open Unity、package table/search/filter/version/action、overflow actions | Packages 是主要工作面；Unity、Backup、copy/remove 等围绕当前 Project，不是独立产品领域 |
| Primary resource area | Packages & Templates | VPM Repositories、User Packages、Templates 三个 secondary views；Add Repository、row actions、template-based create | 用户理解为项目所需的包来源、用户包和模板资源，而不是三个互不相关的产品模块 |
| Existing extension concept | Extensions management 靠近 sidebar 底部；MCP/Discord 等 extension entry 可出现在 sidebar | install/enable/disable、visible/order；extension-specific UI | Extensions 已是 v3 用户概念；具体 extension 功能仍在同一 shell 内到达 |
| Settings utility | Settings；UI Theme 是快捷 utility | Unity Hub/Editor、project/backup path、appearance、locale、update、deep link、legacy、licenses/system information | 全局配置集中在 grouped scrolling settings；Unity installation 是设置/环境配置 |
| Logs/observability utility | Log | Activity / Technical secondary views、search、source/status/kind/level filters、open log directory | 用户在一个日志类区域查看活动与技术问题，不理解为两个业务领域 |
| Global progress utility | modal/progress host、可最小化 task、toast | progress、cancel/minimize、completion/error | 长任务属于发起它的 workflow，同时有轻量全局恢复入口；没有独立 Operation 产品领域 |
| Footer utility | version/update、hostname warning；About-like information 在 Settings/licenses | version、update、license/system information | 产品元信息不占主要业务导航 |

## v4 user-facing capability map

| Capability | v3 correspondence | Classification | v4 user value | Intended user placement |
|---|---|---|---|---|
| Projects | Projects | `SAME` + `ENHANCED_EXISTING` | stable identity、structured missing/inaccessible errors、revision-aware reads | Projects primary area；保持 list/grid、toolbar 与 Project drill-in |
| Project Packages | Project Manage package table | `ENHANCED_EXISTING` | deterministic resolve、Plan review、Apply、integrity、recovery | Project workspace 的主要 Packages view；内部机制不改变用户归属 |
| Repositories | Packages & Templates > VPM Repositories | `ENHANCED_EXISTING` | normalized source、deterministic precedence、refresh/cache evidence、structured errors | Packages & Templates 内的 Repositories secondary view |
| Templates | Packages & Templates > Templates | `ENHANCED_EXISTING` | registry、import/export/derive、safe create-project、recovery | Packages & Templates 内的 Templates secondary view；create-project回到 Projects workflow |
| User Packages | Packages & Templates > User Packages | `SAME` target，当前完整 v4 capability未完成 | 保留 roadmap 的用户概念，不发布 fake/disabled surface | 只有真实能力完成时才恢复同组 secondary view |
| Unity | Project header + Settings Hub/Editor | `ENHANCED_EXISTING` | verified installation registry、selected editor、writer evidence、安全 launch | project selector/open/status 留在 Project；global installations 留在 Settings |
| Backups | Project/Projects action + Settings | `ENHANCED_EXISTING` | durable create/restore、Plan、progress、recovery、metadata | create/restore 仍从 Project/Projects 发起；history/detail 是 Project-context secondary surface |
| Operations | progress host/task/toast | `ENHANCED_EXISTING`；durable cross-restart history 是有限 `GENUINELY_NEW` 子能力 | unified progress、cancel、restart recovery、history | 发起页面内 progress + sidebar/footer task status；需要时打开 secondary Task Center/history，不是 primary domain |
| Extensions | Extensions management + extension entries | `ENHANCED_EXISTING` | signed package、permission/scope、enable/disable/uninstall、crash/quarantine | 保留 Extensions 既有 utility identity/大致归属；增强进入 list/detail 内部 |
| Portable UI | extension-specific UI | `INTERNAL_ONLY` presentation contract | 同一 extension page 可被 official/third-party GUI安全渲染 | Extension detail 内的功能内容，不显示“Portable UI”产品入口 |
| Settings | Settings + UI Theme | `SAME` + `ENHANCED_EXISTING` | authoritative Config、revision、typed validation、统一 MD3 theme | grouped Settings utility；可保留轻量 appearance快捷动作 |
| Activity | Log > Activity | `ENHANCED_EXISTING` | bounded durable safe projection、pagination、stable filters | Log/Activity utility 的 Activity secondary view |
| Diagnostics | Log > Technical | `ENHANCED_EXISTING` | structured redacted diagnostics、diagnostic ID、独立 permission | 同一 Log/Activity utility 的 Diagnostics secondary view；backend authority保持分离 |
| About / licenses | Settings/licenses + footer version | `SAME` | product/version、licenses、third-party notices、safe system summary | footer/version action 或 Settings secondary page，不占 primary navigation |

### Same capability list

- Projects 的发现、列表、进入管理与创建入口；
- Packages & Templates 作为资源型用户区域；
- Repositories、User Packages、Templates 的用户概念；
- Extensions 作为既有管理概念及 extension-specific UI；
- Settings、appearance、全局 Unity 配置；
- Log/Activity/Technical 的观测概念；
- About、licenses、version/update 的 utility 概念。

### Enhanced existing capability list

- package install/remove/upgrade 通过 Plan -> review -> Apply -> durable progress/recovery 实现；
- Repository 使用 normalized source、deterministic precedence 与安全 refresh/cache；
- Template 增加安全 import/export/derive/create-project 与恢复；
- Unity 增加 verified installation、writer evidence 与安全 launch；
- Backup 增加 durable archive metadata、Plan/Apply、atomic restore 与 crash recovery；
- Extension 增加签名、permission/scope、runtime state、crash/quarantine 与同权 Portable UI；
- Settings 增加 daemon-authoritative revisioned config；
- Activity/Technical Logs 增加 bounded、structured、redacted read model；
- progress/task 增加跨连接、跨重启 Operation recovery 与 history。

### Genuinely new user capability list

M1-M7 没有形成新的一级产品领域。有限的 genuinely-new 用户价值只有：

1. **Durable task recovery/history**：用户可在离开原页面或 daemon restart 后重新找到任务状态；放入全局 task status/history
   secondary surface。
2. **Extension permission/scope review and quarantine recovery**：用户可理解扩展为何被限制或停止，并管理授权；放入既有
   Extensions detail。

它们都能自然嵌入 v3 用户概念，因此不满足新增 primary workspace 的四项条件。

## Internal concepts that do not become GUI surfaces

| Internal concept | User-facing expression |
|---|---|
| daemon / RPC / State Schema | connection/error/retry状态；不显示 subsystem page |
| OperationId | workflow progress、task status/history、cancel/recovery |
| Plan / Apply | high-impact action 的 review/confirm/progress flow |
| Revision / expectedRevision / idempotency | stale/conflict/retry语义，不显示 implementation dashboard |
| Resource Lock / recovery journal | busy/recovering/result状态 |
| Principal / Permission / Scope / Lease | Extension或未来外部客户端的授权详情与denial说明 |
| Extension Host / WASI Component Model / WIT | Extension runtime status/support information，只在诊断需要时给安全摘要 |
| Portable UI | Extension detail内容的安全呈现机制 |
| Quarantine | Extension detail中的安全状态、原因和恢复动作 |
| InvocationContext | action执行上下文，不成为用户页面 |
| data/config/extension API version | compatibility/support信息，通常属于diagnostic/about，不成为导航 |

## Current `192672…` IA leakage

当前 functional candidate 的 route 与 backend boundaries 可以保留，但以下 sidebar/page composition 错误需要纠正：

1. 移除把 Home/status 当成新产品领域的默认大卡片 dashboard；Projects 恢复 home-equivalent，connection状态进入 shell/footer。
2. Repositories 与 Templates 从两个平级 primary entries 合并回 Packages & Templates 用户区域；完整 User Packages 未实现前不显示。
3. Unity 一级入口移回两个既有 context：Project header/workspace 与 Settings installation section。
4. Operations 一级入口降为 workflow progress + sidebar/footer task status/history secondary surface。
5. Extensions 保留既有用户概念和靠近 utility/footer 的大致归属；permission/quarantine/Portable UI进入 Extension detail，
   不归入新的 Resources/Platform hub。
6. Activity 与 Diagnostics 从两个平级 primary entries 合并为同一 Log/Activity utility 的 secondary views；RPC/permission不合并。
7. About 从平级 primary entry 移回 footer/version action 或 Settings secondary page。
8. Project detail 重新以 package workspace为中心；Unity/open/backup/copy/remove回到header、tabs或overflow的project context。
9. generic large cards、巨幅标题、过量留白和page-bottom primary actions改为desktop toolbar、dense list/table/compact row。
10. 顶部长期 product/settings bar不再与sidebar争夺全局 hierarchy；native title chrome之外，page title/action属于主内容toolbar。

这些是 presentation/composition 纠正；typed RPC、hooks/state、workflow、Settings、Activity、Diagnostics、Portable UI 与
Playwright infrastructure 继续复用。

## Minimum necessary IA change proposal

本 proposal 是从 v3 用户结构出发的唯一 minimum-change 方向；尚未获得 production approval，也不冻结最终文案、像素或每个按钮。

### Wide structure

```text
┌────────────────────────────────────────────────────────────────────────────┐
│ native window title: ALCOMD3                                               │
├──────────────────┬─────────────────────────────────────────────────────────┤
│ PRIMARY          │ Page / Project toolbar                                  │
│ ● Projects       │ title · search/filter · context · primary action        │
│   Packages &     ├─────────────────────────────────────────────────────────┤
│   Templates      │ optional secondary navigation                           │
│                  │                                                         │
│ UTILITIES        │                                                         │
│   Settings       │                                                         │
│   Log            │                                                         │
│   Extensions     │                                                         │
│──────────────────│                                                         │
│ ◷ active tasks   │                                                         │
│ v4.0 · About     │                                                         │
└──────────────────┴─────────────────────────────────────────────────────────┘
```

Portable UI v1 不提供 sidebar、menu、toolbar 或 arbitrary navigation contribution。M8/M9 的 MCP/Discord production wiring
不属于 M7；未来若要改变 navigation contribution contract，必须另行审批。

### Navigation and workspace hierarchy

```text
Projects (landing/home-equivalent)
  -> Project workspace
       -> Packages (primary/default workspace)
       -> project context: Unity selection/Open, Backup/history, Copy, Remove

Packages & Templates
  -> VPM Repositories
  -> User Packages (only when real capability is complete)
  -> Templates

Settings
  -> Appearance/config
  -> Unity Hub/Editor installations
  -> approved global settings
  -> Licenses/system information when appropriate

Log
  -> Activity
  -> Diagnostics

Extensions
  -> Extension detail
       -> lifecycle/status
       -> permission/scope
       -> quarantine/recovery
       -> extension-provided Portable UI content

Footer utility
  -> active task status / Task Center history
  -> version / About / licenses
```

该树只冻结用户语义归属，不冻结三个同级 Project tabs。Unity 与 Backup 的 exact presentation 可在后续页面 realignment 中使用
page toolbar action、context action、secondary view、dialog 或 project sub-surface；不得从树形文档机械生成 subnavigation。
Task Center/history 是低权重 secondary utility，不与 Projects 或 Packages & Templates 等同。Update 只有真实 updater/application
capability存在后才允许显示；M7不得提供fake、placeholder或disabled future update UI。

### v3 structure retained

- sidebar + single main content canvas；
- Projects landing 和 Project-first workflow；
- Packages & Templates 资源分组；
- Extensions 作为既有用户概念及靠近 utility/footer 的大致归属；
- Settings、Log、version/About 的 utility hierarchy；
- Project header、package-centric workspace、context actions；
- toolbar + dense list/table/compact row 的桌面信息密度；
- list/detail/workspace 与 modal review/progress 的空间关系。

### Necessary v4 adjustments

- package/backup/template/extension high-impact mutation 在原 workflow 内增加 Plan review 和 durable Operation progress；
- sidebar/footer 增加紧凑 active-task indicator，可打开 secondary Task Center/history；
- Extension detail 增加 permission/scope、runtime/quarantine 与 Portable UI content；
- Log 内用 Activity/Diagnostics secondary views表达独立安全 read models；
- Project/Backup/Template detail可承载 durable record与recovery evidence，但不升级为新一级领域；
- narrow/zoom下 sidebar转drawer、secondary views转compact tabs/select、table使用priority columns/detail expansion；
- 所有交互控件和主题迁移到共享 `@alcomd/ui` / Material Web foundation。

### Explicitly rejected new surfaces

本 proposal 不新增以下独立产品区域：daemon、RPC、Plan、Apply、Operation domain、Extension Host、WASI/WIT、Principal、Permission、
Scope、Portable UI、Quarantine、InvocationContext、State Schema、独立 Unity domain、独立 Backup domain、独立 Diagnostics domain。

Task Center、Extension permissions/quarantine、Diagnostics 只是在既有用户模型中的 secondary surface。

## Material and authority boundary

以下事实和方向不变：

```text
material_web_dependency_present_but_component_system_not_adopted
material_theme_not_actually_wired

@material/web
    -> @alcomd/ui
        -> official GUI
        -> Portable UI renderer
```

Material Design 3 现代化 component、color、typography、shape、elevation、spacing、state layer/ripple、dialog、toolbar、motion 与
adaptive behavior；它不替代用户 IA。Semantic HTML、shell、layout、navigation composition 与 data table 可由 ALCOMD 实现。

GUI 继续只经 typed client/RPC/application 使用 Core。本文不修改 route contract、RPC、State Schema、Permission、Extension API
或 Portable UI protocol。

## Approval stop

本 minimum-change proposal 已获项目所有者批准。H0/H1 可以实施；H1 后必须停止在 Visual Gate 1。此前 Candidate A/B/C 只
保留为历史探索，不再作为 production 候选。不得开始 H2-H7、M8 或 M9。
