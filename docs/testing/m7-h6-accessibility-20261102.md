# M7 H6 Responsive / Accessibility / Playwright Regression

状态：**MANUAL_PENDING**，2026-11-02。H5 已阶段性人工签收，紧凑模式删除方向获认可；H2–H4 原签收保持。
本轮只推进获批 H6，保留既有非阻塞文案、布局及过度设计待办，不进入 H7、不推送、不宣布 M7 完成。

## 实际缺口与修正

先复用已有 Material Dialog/Menu/Select 生命周期、路由焦点、主题、减少动画、Reminder 和 ScrollArea，
未重建组件体系。只读浏览器诊断覆盖 30 个布局样本，保存测量、输入摘要和完整 viewport 图片；这是诊断，
不计为正式测试通过。发现并修正：

- 320px 时固定侧栏使正文仅剩 48px，Resources 工具栏在中等宽度也会裁切操作。
  不足 1100px 时复用同一侧栏为图标栏，保留可访问名称、标题和选中状态；标题/工具按可用宽度换行。
  表格继续使用原有局部横向滚动，不删列或另建移动页面；宽屏侧栏、默认间距与扩展两列布局保持。
- Extensions 固定两列在窄窗溢出；Settings 路径行、选择器和长标签不能适当换行。
  窄窗局部重排，移除选择器的最小宽度限制，保留复选框/开关/单选框尺寸，让标签换行。
- 主题弹窗的两列在 320px 重叠；分段按钮真实 Tab 到“深色”后仍被裁切。
  窄窗改为一列，复用局部滚动；共享分段组件在 focusin 时仅将公开按钮滚入最近可见位置，清理监听器。
  宽屏保留整组日语标签的 intrinsic 最小宽度；沿用 860px 断点切为单列，避免 621–800px 时把颜色字段挤窄。
- Portable UI 没有可滚动正文，短窗中实际滚轮无法到达最后操作。
  复用 ScrollArea，将现有宿主标题留在正文外，正文、反馈和扩展内容可局部滚动。
- “跳到内容”的原生锚点触发同路径 popstate 后，路由逻辑会抢回标题焦点。
  main 可编程聚焦；同路径锚点保留原生焦点，不触发路由离开或脏表单确认。真正跨路由的保护保持。
- SplitButton 将公开 spacing token 与实际 padding 同时清零，导致 40px 下拉按钮的真实点击区域达 64px，
  抢占主按钮右缘。删除两项多余 token 覆盖，保持 40px 外观及位置；回归真实点击主按钮右缘，不能打开菜单。
- 删除两个不存在的 aria-describedby 引用，保留 Material 字段已有 supportingText。

Tauri 2.12.5 默认不启用页面缩放快捷键，本轮复用 `zoomHotkeysEnabled` 及主窗口限定的既有 Tauri UI zoom 权限；
Windows 走 WebView2 原生缩放。无新增 ALCOMD RPC、State/Config Schema、权限、依赖或扩展协议。
只读路径继续使用 `displayPath`，不更改真实路径、可编辑值或 RPC 参数。

## Config4 与证据边界

H5 签收候选 `6739705de9711b18e9e398270e0254e6b8719c6b` 的真实迁移证据继续有效：
`target/m7-h5-no-compact-20261102/migration-verification.json` 证明 schema 3→4、仅删除 density，
revision 166 及其他设置逐值保留，五组预期比较均 true。H6 交接和构建前只读 `settings.get` 仍为 Config4/revision166，
公开 appearance 只含 mode/motion/scheme/sourceColor。没有恢复紧凑模式开关、样式分支或公开字段，未重复迁移/Rust 测试。

证据目录：`target/m7-h6-accessibility-20261102/`。每轮捕获 GUI/SDK/UI/build/test 的 186 项文件摘要，
各轮内前后相同；轮次之间的修正不能混成原套件通过。

| 独立轮次 | 实际结果 | 处理与边界 |
| --- | --- | --- |
| 首轮四模块定向 | **27/11**，2.3m | 保留失败 trace、截图、日志。若干新测试错误地断言原始 aria 属性或在 opened/初始标题聚焦前操作；另有真实主题分段焦点裁切。 |
| 新增响应式/键盘复验 | **8/4**，56.8s | 主题与 Portable 滚动通过；暴露 Skip 锚点重聚焦和 SplitButton 点击区域溢出，按实际链路修正。 |
| 关联定向 | **13/1**，1.0m | 包括全部路由及 200% 等效主要页面；剩余 320px Settings 选择器溢出，诊断同时发现控件被 flex 压窄。 |
| 仅 320px 受影响用例 | **1/0**，13.8s | 保留全部几何、真实滚轮、按钮边缘点击及控件尺寸断言，覆盖四个主要页面；不重跑上一轮其他用例。 |
| 一次集中 Chromium 回归 | **383/12**，16.3m | 全部 395 项已运行；186 项输入前后完全一致。此轮失败保留，不标记全套通过。 |
| 失败及受影响路径复验 | **53/1**，2.4m | 原集中轮 12 项均通过；新增日语 621px 用例错误地检查外层 host 焦点，公开按钮已聚焦。未放宽几何或尺寸断言。 |
| 仅日语 621px 用例 | **1/0**，7.7s | 改用公开 button 角色检查真实焦点，其他输入不变；不重跑 54 项或 395 项。 |

集中轮失败分类：10 项为 H3/H5 可读错误/当前弹窗及日期范围的旧断言；1 项模板测试在强制点击禁用项使菜单闭合后，
继续假定菜单仍开着并在 closed 前重开；1 项为新增主题滚动容器改变宽屏日语的列宽贡献，属于实际布局缺陷。
模板对照使用公开 opened/closed，确认 End 跳过禁用移除项、Export/Derive 正常，且没有 Plan/Apply/导出/删除写入。
旧断言仅按当前可观察合同修正，保留修改时间权威、排序/Unknown、权限、一次/零请求和副作用检查。
随后只复验失败用例及受影响主题/响应式/键盘路径，未重复 395 项。

最终源码候选为 **`a4f12a19d5fca0692053bb281fefab472fd60b65`**，真实 Windows 用户 `CQMHV-DT\M2922` 在唯一标准目录/main 本地提交。
集中轮至 53/1 轮只变更 `styles.css` 的 Theme 宽度/860px 单列规则和七个测试文件；其余运行时与构建输入完全相同。
53/1 至最后 1/0 只改新测试的公开角色定位，运行时代码未变。差异见 `consolidated-to-affected-inputs.json`。
最终 186 项输入清单为 `candidate-inputs.json`，文件 SHA-256 为
`94d8c342fe32094813acd75e16dfc0d0770c54d33bc2efe5ad6fe2e5b56316e4`。
这证明证据对应的范围，不表示最终候选已取得一轮全套零失败结果。

## 实际构建与原环境

稳定候选只进行一次 Tauri release no-bundle 构建，GUI Rust release **1m23s**，包含 GUI check 和 Vite；
现有大 chunk 提醒保留，不修改警戒阈值。未重复完整 Rust、Core release 或远端 CI。
`build-inputs-after.json` 的 186 项输入与最终定向验证、`candidate-inputs.json` 完全相同。
GUI SHA-256：`1d6ae9d360f764b5f13413449ab1992ff925b8af890656f5ba7738dd82839b47`。
Core 来源保持 `6739705`，摘要 `b90ec8b93d043ca8ef6077d9f0bfdec40a5db642a5c91f820cf17a0d665d4687`。

本轮不暂停 Core。核对任务全终态及 PID/路径/创建时间/命令行/摘要后，只关闭旧 GUI 141068；
新 GUI **139052** 使用原 profile 实际打开，窗口 **8393506**、标题 ALCOMD3、可响应。
原 Core **143516** 持续使用 `target/m7-icon-acceptance-20260906/data`，没有安装测试扩展或操作用户 Unity。
`before-build/` 至 `original-after/` 的 projects/editors/operations/settings/status 五组完整公开状态全部一致，
`five-group-comparison.json` 五项 true；Config4/revision166 及其他设置保留，实际配置文件 SHA-256
仍为 `4fd8b2504e4f1102df33ce0fe1613bf339e2e463bd75d4683bbce1bab9a64dd1`。
进程、构建和文件证据见 `candidate-artifact.json`、`core-runtime.json`、`gui-runtime.json`、`window.json`。

当前窗口保持打开。实际程序：
`C:\Users\M2922\MyProjects\Rust\ALCOMD\target\release\alcomd-gui.exe`。
窗口关闭后可在 PowerShell 复开：

```powershell
& "C:\Users\M2922\MyProjects\Rust\ALCOMD\target\m7-h6-accessibility-20261102\H6-Window.ps1" -Action Open
```

该入口沿用并核对既有原环境流程，**本轮已真实执行 Open**，核对 Core/GUI 摘要并复用原 profile；
有 GUI 或 Core 身份变化时会保留现场并拒绝重开，不自动启动/切换 Core。窗口存在仅证明候选可供查看，不代表原生交互已验收。

首个 GUI check 因新测试引入 Node 类型失败，删除不必要的 Node 导入后修正，失败日志保留；随后 GUI check
（1028 条三语言）通过。UI check（31 个固定图标资产）通过。已有错误反馈测试改为先检查可读原因、展开详情再检查原码，
不删除 CAS、重试、权限或副作用断言；fixture 的 Unity/backup 错误码纠正为 Core 当前合同。

H4 首轮 **78/1** 与定向 **6/0**、首个 H5 **101/2** 与定向 **56/0**、Config4 **76/0** 和补充 **1/0** 均独立保留，
不把本轮覆盖或旧提交的远端结果回填为历史套件通过。H2-B、H3、H4、H5 及 H6 新增代码仍无对应 Hosted CI/CodeQL。

## 未覆盖项与人工查看范围

浏览器以真实 DOM/Material、键盘和滚轮验证，但数据来自内存 fixture，不是实际 Core 权限或文件副作用证明。
720×450 CSS viewport + DPR2 是 1440×900 物理窗口在 200% 下的等效重排；没有修改根字号，也未自动执行原生缩放快捷键。
图片均为浏览器 viewport 证据，**不是原生桌面截图**。Computer Use 保持停用，不绕过 Esc 停止或工具审批。
Windows 原生缩放/实际读屏器、macOS/Linux 辅助技术不写成通过；正式 MCP/Discord 业务及 H4 样例未覆盖的状态保持原边界。
原环境没有测试扩展，扩展卡片/Portable UI 窄窗仅有浏览器 fixture 证据，本轮不往原环境安装样例。

H6 只需查看以下变化，不重验 H2–H5 已签收流程：

1. 调窄窗口，查看 Projects / Resources / Settings 的主要操作；窄窗图标栏有名称，工具不裁切，表格只在自身滚动。
2. Settings → Theme，在 200% 缩放下查看并用 Tab、Shift+Tab 操作；最后的取消/保存可到达，Esc 关闭后焦点返回，结束后复位缩放。
3. 用 Tab 到“跳到内容”并 Enter；再开关一个弹窗，确认焦点清晰、关闭后回到原操作；切换明暗和减少动画偏好，确认普通页与弹窗一致。

阶段人工确认仍由项目所有者作出，不将窗口存在或浏览器回归等同于 H6 ACCEPTED。
