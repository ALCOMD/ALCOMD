# 项目状态

最后更新：2026-11-03

## 当前阶段

**M7 IN_PROGRESS / H7 MANUAL_PENDING / H6 STAGE_ACCEPTED_WITH_FOLLOWUPS / H5 STAGE_ACCEPTED_WITH_FOLLOWUPS / H4 STAGE_ACCEPTED_WITH_FOLLOWUPS / H3 STAGE_ACCEPTED_WITH_FOLLOWUPS / H2-B STAGE_ACCEPTED_WITH_FOLLOWUPS / H2-A ACCEPTED / Visual Gate 2 PASS**。项目所有者于 2026-10-31 签收本轮 H2-A；
M0-M6 已正式通过人工验收，M7 尚未完成。
本文件是唯一当前阶段入口；ExecPlan 定义实施合同，testing 文档保存各自 SHA/场景的证据。

项目所有者于 2026-11-02 确认 **H6 阶段性人工验收通过，保留非阻塞体验问题**，认可窄窗、键盘、主题及主要辅助功能，并批准既定 H7 最终整体验收。
H2–H6 原签收保持；H7 只处理实际影响使用的问题，整理精简最终清单并在稳定代码上集中完整本地验证，不逐页重验或重新开发。
统一默认间距，不恢复紧凑模式及 density 公共字段。真实读屏器、跨平台辅助技术仍属 M12。
所有者随后批准推送两笔窄修/记录提交，既有远端技术基线 `c273c76` 已取得同 SHA 三平台 CI 与四语言 CodeQL 全部成功；后续目录设置增量见下，不借用旧 CI 作为新代码通过。H7 人工确认及剩余事项处理仍待完成，M7 进行中。

### H7 最终体验细调（2026-11-03）

所有者要求在现有成果上逐项细调，保留 H2–H6 签收及下方 H7 技术基线，不进入 M8，不重新验收历史功能。

通知通道补正代码 **`92aaad7`**：操作反馈由统一入口发布/清除，既有弹窗中的Unity查找等操作失败默认进入右下出口；宿主随原生模态弹窗切换，详情/重试可点击，关闭后清理来源，直接启动失败不再打开纯错误弹窗。保留原错误与诊断编号，不另写日志或扩展诊断合同。
集中定向49/2；关闭Unity错误提醒后的焦点返回及新增用例的路由预期已修正，补验11/0，覆盖Unity查找/启动、单次发布和嵌套弹窗；不合称全套通过。GUI check及release no-bundle构建通过，未重跑完整Rust、浏览器或远端门禁。
所有者单独批准本次GUI构建与重启后，已沿用原profile打开GUI **109092**（`8CEDF3E6…0105039`），Core **130792**保持；五组公开状态和配置摘要与更新前一致。构建包含当时工作区其他设置/菜单改动，其验证范围沿用各自记录；完成后SettingsWorkspace又有并行修改，未纳入本次产物与验证。本批通知源文件保持，原生交互与最终签收仍待所有者，桌面控制继续停用。代码/文档分批本地提交，未推送。
所有者明确确认默认项目保存目录、默认备份保存目录是严重影响使用的核心功能遗漏，并批准 Config5 及相应公开设置/持久定位扩展。
功能代码 **`1a31e2a82459eb57a7263f454fdad6e8bf07f585`** 已补齐设置页、项目表单默认目录及备份实际保存位置；已有备份与已接受任务保留各自定位，不自动搬迁。
本轮本地定向通过：协议/Schema、设置权限/CAS/迁移与重开、平台不覆盖发布、实际配置到旧/新备份恢复、
真实隔离设置及备份 RPC/kill-restart，八个受影响 crate Clippy、fmt、xtask、metadata；GUI 保存队列 20 场景，
最终集中浏览器 26/0、关联流程 12/0、320/640px 2/0分别保留。失败与后续窄修分别记录，不拼为全套通过。
一次 Core/CLI release 与 GUI release no-bundle 构建通过。已恢复原数据目录和原 WebView profile，
目录设置交付时 Core **130768** / GUI **132996**（窗口 **101191372**，可响应）；项目、Editor 登记、Operation、Status 与切换前完全一致，
Settings 仅增加 Config5 的目录字段；实际 TOML 只有 schema 4→5，revision **166** 及其他全部字段保持。
本轮没有操作用户 Unity 或真实工程，没有新增原生自动交互/截图；桌面控制继续停用，原生观感/目录选择由所有者查看。
没有执行完整 Workspace、完整 Chromium 或新远端 CI。详见 [目录设置补齐](testing/m7-storage-locations-20261103.md)。本地提交，不推送，不进入 M8。

所有者确认分段导航在过窄窗口可放不下，包括此前 320px 及本次 640px 英文设置导航。当前源码已移除分段滚动容器、焦点自动滚动、专用溢出 CSS 及补偿留白；保留 Material 组件、等宽和选择语义。
无图标分段内部左右各 24px，移除勾号选项和隐藏宽度测量层。按正常桌面尺寸验证，不再要求过窄窗口的全部分段可见；其他已有窄屏操作/表格检查保留。不新增替代机制，不改数据合同或依赖。
此前分段滚动简化的选择/语言/缩放定向 11/0、响应式模块 13/0 及 GUI/UI check、metadata/diff 结果保持原候选绑定，不代替本次 24px 调整验证。未重跑完整浏览器、Rust 或远端检查。
随后在 1280/800px 的十个主页面核对同类滚动，修复 About 刷新条的 4px 伪溢出，以及长错误文案挤压刷新按钮产生的额外溢出；恢复共享刷新条默认高度、容纳完整点击区并保持按钮尺寸，清理失效滚动引用和样式。真实超宽表格及长内容滚动保留。
About 修前 0/1；最终 Chromium/同版 Edge 各 3/0，覆盖正常刷新、长失败反馈、离线版本和许可证信息；相关分段回归 6/0。GUI check 通过，先前失败及中止构建保留本地记录，不拼接为完整套件通过。
分段与 About 两批整改已集中完成匹配 GUI release no-bundle 构建，并在交付时核对原 Core、五组公开状态及配置摘要不变；具体结果继续绑定该批源码。

设置页进一步按所有者截图采用共享分段导航，当前源码为“常规 / Unity / 外观 / 通知”，常规包含保存位置和软件包选项。“系统信息”区块及重复入口已按要求删除；关于和诊断仍从现有侧栏访问。分组只切换显示，保留原组件和自动保存状态，不增加持久偏好或滚动兜底。
“通知”按所有者要求留空，不增加说明或开关；已有应用内成功/错误提醒保留，系统通知及通知偏好尚未接入。
主题直接展开到外观页，删除主题弹窗、HCT 三个调节条及其专用代码/样式；按所有者确认改为模式、主题色、配色方案三条共享设置行，模式与九方案均用现有下拉框，删除两列结构并让操作紧接内容。保留颜色选择、HEX、九配色、实时预览和独立保存；取消还原最新已接受主题。模式选项顺序按所有者要求统一为“跟随系统 / 浅色 / 深色”。
此前分段间距/模式下拉的 GUI/UI check、metadata/diff 通过；关联定向 **53/1**（3 项未执行），失败为新增连续选择用例未等下拉关闭完成，改等公开 opened/closed 后，失败/未执行项及获准调整的 640px 步骤补查 **5/0**。原 53 项的产品代码未变，按范围复用；首次 640px 旧导航断言失败及后续失败保留，不合称全套零失败。此前各候选证据保持原绑定；未重跑完整浏览器、Rust 或远端 CI。不改公共合同、权限、持久数据或依赖。
上一批匹配 GUI release no-bundle 构建通过，用原 profile 打开 GUI **149844**，原 Core **130768** 持续运行；五组公开状态及配置摘要与更新前一致。本次选项顺序调整的 GUI check、三语与键盘定向 **7/0** 通过，暂不重建桌面程序，随下一批集中更新。当前入口见 [人工查看清单](testing/m7-official-gui-manual-checklist.md#当前查看入口)，原生观感仍待所有者确认，M7 继续进行中。

Unity 设置截图只读对照确认：全局默认启动参数仍是缺口，现有项目级参数不能替代；Core 的 Editor 发现只覆盖默认安装根，尚未覆盖 v3 的 Hub 配置来源。刷新扫描和添加能力已有，但在管理弹窗中；列表刷新只重读登记记录。
所有者已批准全局默认参数，并明确要求依据 Unity 官方文档与正确启动方式实现。Config6、字段级权限、Core 继承/读取投影与 argv 传递、GUI 无损逐项编辑已接入；定向测试发现 State16 禁止空数组后，另获批准 State17 最小迁移，保存显式空项目覆盖。范围与依据见 [M7 实施合同](exec-plans/M7-official-gui-extension-ui.md#unity-设置截图对照与补齐2026-11-03)。Hub 发现缺口仍另行处理，不复制旧源码/策略开关，不启动真实项目。

本批 Application **52/0**、Protocol **52/0**、Store **123/0**（含 State17 迁移 3 项）通过；随后私有启动快照长度修正的 Store M5 模块 **4/0** 再验，其他 Store 结果按未受影响范围复用。
隔离 RPC/合同 **25/0**（含 Config6 实际文件与 Core 重启保持）、端点所有权 **4/0**；六个受影响 crate Clippy、fmt、xtask、metadata 均通过。
GUI check/build、保存队列 **24** 场景通过；首轮相关浏览器 **65/6**，六项均为新增区预期或 Material 插槽定位的测试问题，保留产品状态/错误断言后，完整受影响两模块 **14/0**。其余产品代码未变，按范围复用；不合称完整浏览器零失败。
一次 Core/CLI release 与 GUI release no-bundle 构建通过，包含此前模式排序。原数据/profile 已恢复为 Core **125676** / GUI **144536**（窗口 **33295358**，可响应）。
五组公开状态保持，仅 Settings 增加 Config6 字段；hello 实报 State17/Config6，TOML 仅 schema5→6，实际切换前 revision **168** 和其他全部字段保持（166 是先前目录交付采样，未覆盖用户后续修改）。
Core/GUI 摘要分别为 `62608EE9…A3C85D91` / `B47F4CD0…318479C`，复用既有打开入口。没有关闭用户 Unity、操作真实项目、恢复桌面控制或新增原生自动交互/截图。
本批仅定向验证和本地提交，没有完整 Workspace、完整浏览器或新三平台 CI/CodeQL；原生观感仍待所有者查看，M7 不自行完成。

所有者随后要求窗口状态记忆及默认关闭开关，并批准 Config7 最小设置合同。设置→外观包含“窗口”区；
Core 仅持有 `window.rememberState`，普通逻辑宽高及最大化状态为可丢弃 GUI 缓存。关闭后清缓存、不记录，下次使用默认窗口；开启后只在启动恢复一次。
原生薄适配器限制自身主窗口，按当前显示器工作区/DPI裁剪并居中；最小化/全屏不覆盖普通尺寸，恢复后采用窗口管理器实际结果。
没有新增依赖、权限名或数据库迁移。默认关闭、保存失败/重试、三语与分组定向浏览器 **25/0**，保存队列 **27** 场景、缓存逻辑 **11** 场景、原生纯模型 **6/0**；
Application **53/0**、Protocol **53/0**、Store 配置 **25/0**、隔离 RPC/合同 **8/0**（开关实际置 true 并重启保持），六 crate Clippy 及最终原生模块补查、fmt/xtask/metadata 通过。
静态审阅修正窗口事件目标匹配及恢复保护收束后，仅补验受影响原生模块；其他已过结果按未变代码复用。原生模型/浏览器均不代表真实拖动、最大化、关闭重开交互已验收。
一次 Core/CLI release、GUI release no-bundle 构建与端点所有权 **4/0** 通过；原数据/profile实际恢复为 Core **130792** / GUI **149024**（窗口 **161288362**，可响应）。
五组公开状态保持，hello State17/Config7；TOML仅schema6→7并增加默认false的window表，实际revision **168** 和其他全部设置不变。
Core/GUI摘要 `163458B5…E6AC1F46` / `CC6FEEC4…80EA2C2B`。原Unity未操作，桌面控制未启用；仅本地提交，无完整Workspace/全套浏览器或新远端CI。
后续细调分两笔本地代码提交：`76e404b` 将全局成功、设置保存失败及窗口记忆失败交给唯一 ReminderProvider 右下出口，删除独立右上定位；错误原因、详情、重试及持续显示保留，成功仍短暂显示且隔离旧页面迟到结果。
`249e6d7` 将窗口记忆开关移至外观页，普通外观选项后、主题区前；默认值和行为不变。GUI check、相关定向浏览器 **20/0** 与一次 GUI release no-bundle 构建通过，覆盖统一位置、错误持续显示、重试、页面切换及分组。
上一批 GUI **127300**（窗口 **55906386**，可响应），摘要 `C8D8B6AE…F9424825`，使用原 profile；Core **130792** 持续运行。五组公开状态及配置摘要更新前后均一致，revision **168** 保持。没有重跑完整 Rust、完整浏览器或远端 CI；原生观感仍待所有者查看。
`3a84f39` 完成其他页面提醒归一：项目、包链接/筛选与仓库刷新结果、用户包、模板收藏、Unity、扩展启停、主题及工具页主动刷新反馈复用右下共享出口；字段/弹窗校验和任务进度保持上下文。路径重试保留焦点边界，主题错误可收起但未知结果仍核对；同时修正刷新错误串入删除反馈、首个删除终态漏报。
GUI check及相关定向50/0通过；后续终态/错误归属及样式窄修16/0、成功删除1/0补验，其他结果按未影响范围复用，不合称完整套件通过。早期夹具/遮挡失败材料保留本地，未重跑Rust/全套浏览器/远端CI。
一次GUI release no-bundle构建通过；该批GUI **156292**（窗口 **119871002**，可响应），摘要 `3133DD66…D2F3DCDB`，沿用原profile；Core **130792**持续运行，五组公开状态及配置摘要不变，Config7/revision **168**、窗口记忆false保持。原Unity未操作，桌面控制未启用；原生观感待所有者查看。
普通文字、数字和多行输入框按所有者截图在共享 Material 主题中由默认4px统一为12px圆角（`c74016a`）；搜索框、下拉框既有胶囊覆盖保持。UI/GUI check及共享控件定向1/0通过，补验实际Shadow DOM圆角12px和原下拉9999px亦1/0通过。已构建匹配GUI release no-bundle并复开原profile：GUI **92684**（窗口 **2626520**，可响应），摘要 `DECEA49D…2FAAEDEA`；Core **130792**持续运行，五组公开状态及配置摘要一致，revision **168**保持。无业务、尺寸或间距变化，未操作Unity/启用桌面控制，原生观感仍待所有者查看。


展开的下拉选择菜单及普通操作菜单已在 `eb471da` 统一为共享12px圆角，删除项目/分体菜单局部重复覆盖；选择框本体保持胶囊形。UI/GUI check与三类真实菜单渲染/键盘/焦点定向3/0通过，GUI release no-bundle构建通过。
该批GUI **139420**（窗口 **22286504**，可响应），摘要 `36961A25…A40864ED`，沿用原profile；Core **130792**持续运行，五组公开状态及配置摘要一致，revision **168**保持。构建包含当时工作区并行的弹窗整改，不将圆角定向结果作为那批改动的验收；其他工作树修改保留且未纳入本次代码提交。

软件包行操作菜单在 `0f204e3` 复用与分体按钮相同的小型菜单样式：普通项高40px、字号14px、圆角12px，保留禁用状态、分隔线与原操作。UI/GUI check及菜单/移除审查定向3/0通过；GUI release no-bundle构建并复开原profile，当前GUI **140868**（窗口 **29364994**），摘要 `8DF2947C…3181A174`。Core **130792**及五组公开状态/配置摘要保持，revision **168**不变。构建包含并行工作树改动，未将本批验证扩展为无关改动验收；提交仅含本批样式和断言。

操作菜单与选择器在 **`cf135fa`** 明确区分：项目、模板、软件包维护/行操作和分体按钮统一40px项高/14px字号，来源、版本及普通Select保留56px选择列表；两类展开表面均为12px圆角。共享组件强制声明用途，删除项目局部密度覆盖，保留权限、删除确认及单次操作。UI/GUI check和受影响菜单/焦点/模板行为定向浏览器 **9/0** 通过；早期新增用例定位失败材料保留，不冒称完整门禁。
当前GUI **109092** 已包含本批菜单输入；核对源码时间、内嵌资源名、JS分类与CSS规则后复用刚完成的GUI构建及原profile，不重复构建或重启。原Core与数据保持，原生观感仍待所有者；本批仅本地提交、无推送。

### H7 已有远端技术基线与验收状态

本轮增量前的最终技术候选 **`c273c76976c711590f6ce0f131bab572019caa50`**：
[CI 41477415678](https://github.com/ALCOMD/ALCOMD/actions/runs/41477415678) **attempt 1 / success**，Windows Server 2025、Ubuntu 22.04、macOS 15 arm64 完整 required jobs 全部成功；
三平台各自完整 Chromium **397/0**（Windows 11.2m、Ubuntu 8.9m、macOS 15.2m）。
[CodeQL 41477413640](https://github.com/ALCOMD/ALCOMD/actions/runs/41477413640) **attempt 1 / success**，四语言全部成功。
只收集获批 push 自动产生的运行，没有手动触发或重跑 workflow。原 CI `9e9fe22` 的 395/1 仍保留，不回填或拼接。
原滚轮测试技术阻塞已由修复候选的同 SHA 完整结果收束；既有完整本地验证继续绑定 `a6c9bd5` 和准确输入，
受影响模块 13/0 绑定 `46eb6df`，两者不冒称重新执行在后续文档提交上；没有重复完整本地 Rust、Chromium 或 release 构建。

**剩余条件**：上述技术候选的测试阻塞已收束；新确认的默认保存目录缺口按本轮细调范围单独处理。
仍需所有者 H7 两项最终确认，并明确下述非阻塞/未覆盖事项的保留安排，才能记录 M7 完成。
一般提示、布局和过度设计继续作为既有非阻塞体验待办；真实误报成功、危险误导或数据缺陷不属暂缓范围。
Portable UI 原生表单/超时/外部断线未覆盖，具体补验阶段未另定；正式 MCP/Discord 业务属 M8/M9，真实 v3 对照属 M11，
发行客户端及真实读屏/辅助技术属 M12。窗口大小/最大化记忆已按上方增量实现，窗口位置记忆未纳入当前范围。
该基线收尾时 Core **131912** 与 GUI **157348** 保留，产物摘要未变，进程可响应；当时五组公开状态与切换前/前次快照一致，Config4/revision166 保持。当前运行环境见上方目录设置增量，不改写历史采样。
没有关闭用户 Unity、改动真实工程或恢复 Computer Use；没有新增原生自动交互/截图。
既有技术基线详见 [最终技术结论与剩余事项](testing/m7-h7-final-20261102.md#当前候选技术结论2026-11-03)；当前按本轮反馈逐项细调，H7 最终人工确认仍待所有者，不进入 M8、不自动推送。

### H7 原远端失败与本地修复（历史证据）

远端候选 **`9e9fe2215abe0c9db6ea33df492af5fe158442b2`** 已推送，CI **41460223799 / attempt 1 / failure**：
Windows Server 2025 与 macOS 15 arm64 全部通过（各 Chromium **396/0**）；Ubuntu 22.04 的 Rust、合同、构建及平台检查均通过，
仅 Chromium **395/1**，失败在 `m7-h6-responsive` 的 640px Settings 滚轮断言。CodeQL **41460221628 / attempt 1 / success**，四语言全部通过。
没有重跑或另外触发 workflow，不把部分平台通过合为三平台通过。

本地窄修 **`46eb6dfa8ad1f8514cd5056dac1f22faa436bc7d`** 只改一个测试文件：外层视口中心可能命中嵌套水平滚动条，
改为先验证指针属于预期视口内容再发真实滚轮；位移、几何和 trial-click 断言完整保留，新增子视口/document 不滚动保护。
确定性新回归修前 **0/1**、修后 **1/0**，最终整个受影响响应式模块 **13/0（1.3m，1 worker、0 retries）**；
GUI check 通过，589 项输入在模块前后不变（`fc282b…42236`），相对原完整验证只有该测试文件变化，生产代码、构建输入及二进制均未变。
原远端未上传 artifacts，因此精确命中点无法取回；本地复现证明测试缺陷，与原远端表现一致；后续 Ubuntu 独立完整通过见当前结论。

以上为修复推送前的证据；当时原 CI 失败、修复未取得远端验证，不能以本地 13/0 关闭。所有者随后明确批准推送 `46eb6df` 和文档 `c273c76`，
其技术阻塞已由上方同 SHA 完整结果收束；一般体验及未覆盖边界不因 CI 成功自动关闭。
原 Core **131912** 持续运行；旧验收 GUI 已关闭，本次按既有入口复开为 GUI **157348**（窗口 **9966488**、可响应），
同一产物/原 profile，五组公开状态与上次完整验证结束时一致，Config4/revision166 保持。未操作用户 Unity，也未恢复 Computer Use。
详见 [远端结果与窄修证据](testing/m7-h7-final-20261102.md#远端结果与窄测试修复)。

### H7 首轮完整本地验证（保留准确输入）

冻结验证 SHA **`a6c9bd5568f8bffb26fe531a58fe4286c7779f6f`**，产品代码与 H6 的 `a4f12a1` 相同；当时本地审计没有新增需修复的产品功能阻塞。
完整 Workspace **526/0，8 个既有 subprocess helper ignored**；独立完整 Chromium **396/0（14.9m，1 worker、0 retries）**。
xtask/fmt/Workspace Clippy、GUI Cargo check、独立 Discord backend 检查（0 项测试）、全部前端 check/build、metadata/lockfile、
并发自动启动、全 Workspace release bins 与 Tauri release no-bundle 均通过。589 项代码/合同/测试/构建输入全程不变，
清单 SHA-256 `1fa264…01037`；108 项已跟踪文档在验证期间未变，之后仅更新状态/验收文档，不重跑完整门禁。

首次全量测试因磁盘不足在链接阶段失败（未取得测试通过），定位后只清理可重建 incremental 缓存，并保留测试断言/溢出检查，
继续未完成门禁；首次 Chromium 在记录 354 项通过后执行会话消失，没有最终结果，记中断、不记全套通过。
上述失败/中断及 H6 **383/12 → 53/1 → 1/0** 均原样保留，不用本轮独立通过回填历史。
原环境已实际恢复，最新 Core **131912** / GUI **153004**（窗口 **4132060**、可响应）；
构建/恢复及最终浏览器结束后的 projects/editors/operations/settings/status 五组完整结果均与暂停前一致，Config4/revision166 保持。
当前 Core 摘要 `1faff4…8f1ff`，GUI `6ec81c…8bc83`，均来自本次冻结源码；窗口保持打开。
没有恢复 Computer Use，没有新增原生自动交互/截图，不操作用户 Unity 或实际工程。最终人工清单已合并为两项，不重验 H2–H6。
完整结果、实际启动入口及未覆盖项见 [H7 本地候选](testing/m7-h7-final-20261102.md) 与
[最终人工清单](testing/m7-official-gui-manual-checklist.md)。以上为推送前记录；后续 `9e9fe22` 的真实远端结果和当前窗口见上，不回写初轮证据。H7 尚未人工签收，M7 不标记完成。

### H6 已签收记录（各轮独立证据）

修正窄窗侧栏/工具栏和设置/扩展局部重排、Portable UI 正文滚动、主题分段键盘聚焦可见性与 Skip 锚点焦点被抢回。
共享 SplitButton 下拉按钮的真实点击区域曾越出外观并抢占主按钮右缘，已删除冗余 spacing 覆盖，保持原尺寸及布局。
复用 Tauri 原生缩放设置，不新增 Core 合同、Schema 或业务权限。路径继续统一 `displayPath`，默认间距和 Config4 保持。
定向检查独立 **27/11**、**8/4**、**13/1**，最后只补 320px 受影响用例 **1/0**；各轮失败 trace、截图、日志及 186 项输入摘要保留。
一次集中浏览器回归 **383/12**（395 项，16.3m），10 项旧断言、1 项模板测试菜单闭合时序、1 项真实日语主题列宽缺陷；
修正后只补失败及受影响路径 **53/1**（2.4m），原 12 项均通过；新增日语窄窗测试的 host 焦点定位改用公开按钮后，仅该项 **1/0**。
不将这些结果拼成一次全套通过。原生缩放/读屏不以等效浏览器布局冒充通过。
本地代码候选 **`a4f12a19d5fca0692053bb281fefab472fd60b65`**；GUI/UI check、metadata/diff 和一次 Tauri release no-bundle 构建通过。
最终 186 项输入与构建前后相同（清单摘要 `94d8c3…6316e4`）；GUI 摘要 `1d6ae9…839b47`，Core 保持 `6739705` / `b90ec8…5d4687`。
新 GUI **139052**（窗口 **8393506**，可响应）已使用原 profile 打开，原 Core **143516** 和原数据目录持续运行。
五组完整公开状态在构建前后全部一致，Config4/revision166 及配置文件摘要不变；没有暂停原 Core 或遗留隔离环境。
桌面控制仍停用；所有者随后已实际查看并阶段签收 H6，新增代码仍无对应远端 CI/CodeQL，未推送。
原集中轮 12 项与后续同名通过逐项核对见 [H6 验收记录](testing/m7-h6-accessibility-20261102.md)，不改写原失败轮。

### H5 已签收记录（各轮独立证据）

H5 签收候选为 **`6739705de9711b18e9e398270e0254e6b8719c6b` / STAGE_ACCEPTED_WITH_FOLLOWUPS**。所有者认可彻底删除 GUI 紧凑模式：
开关、专用样式、渲染/保存分支和公开 `appearance.density` 均已删除，统一原默认间距；当前唯一配置为 Config4。
Config1–3 仅校验后迁移，丢弃旧 density，保留其他设置和 revision，Config4 拒绝该旧字段；State16、权限不变。
配置 Store **19/0**、应用 **17/0**、协议合同 **7/0**、hello **1/0**、映射 **1/0**、真实隔离 Core RPC **1/0**；
GUI/UI check（1028 条三语言）、保存队列 **15** 场景、关联浏览器 **76/0** 与补充保存刷新 **1/0** 通过。
Clippy 首轮仅新测试初始化写法失败，最小修复及关联复验通过，失败日志保留；未重复完整 Rust/浏览器或 CI。
一次必要 Core release 和匹配 GUI release 均通过（各 Rust release 1m41s），536 项代码/合同/构建输入在最终构建前后不变。
原开发目录已实际恢复为新 Core **143516**（`b90ec8…5d4687`）和 GUI **141068**（窗口 **41227382**，`23d6d7…e9b1bc`）。
实际配置迁移严格只有 schema 3→4 和删除 density；revision **166**、主题及其他偏好完全保留，其余四组公开状态不变。
桌面控制保持停用，没有原生自动点击/截图；所有者已实际签收 H5，新增代码仍无对应远端结果。H6 交接只读核对 Config4/revision166，
公开外观只含 mode/motion/scheme/sourceColor，原 Core/GUI 身份及摘要相符；既有迁移保留偏好的证据有效，不重复 Rust/迁移测试。

首个 H5 候选 `c7682899a27c938e588720e1dce7ef4113882a17` 的下述证据作为历史保留：补设置安全错误与未知写反馈、
仓库隐藏 256 项边界、日志同任务持续详情/权限接线、诊断原因和既有摘要翻译，以及独立 GUI 版本/断线 About。
不新增公共合同、权限、依赖或机制。GUI check **1030** 条三语言通过；九个关联浏览器模块首轮 **101/2**，
修复 Hub 在途反馈清空及旧 raw-code 断言后，仅三个受影响模块复验 **56/0**。两轮各 184 项输入未变，独立记录，不拼成完整套件通过。
一次 Tauri release no-bundle 构建通过（GUI Rust release 1m33s），GUI 摘要 `c81587…7f6484`；Core 仍为 `e2b7ddd` / `7d62a6…da7a18`。
该轮已打开原环境 GUI **144972**（窗口 **2431108**、可响应），原 Core **136212** 当时持续运行；该次构建前后五组公开状态一致。
H4 恢复时的五组一致证据保持。H5 较早快照至构建前发生过 Unity 记录移除→重发现（15:01，版本/路径/文件身份不变、新记录 ID）；
该差异保留为真实历史，不回写或强造一致。没有操作用户 Unity、未知进程或恢复桌面控制。
上述首轮交付时等待人工确认；当前 H5 已阶段性签收。新增代码仍无远端 CI/CodeQL，旧轮失败及定向复验分开保留。
记录与历史查看清单见 [H5 验收记录](testing/m7-h5-utilities-20261102.md)。

### H4 已签收记录（各轮历史证据）

H4 签收 GUI 候选为 `52ed23bc41f01bfcfa0a7a3fc18cb5db56aaa766`，Core 保持 `e2b7ddd`。保留扩展列表分页、
安装/卸载审阅与同一 Operation 跟踪、启停和隔离恢复反馈、真实权限/项目范围，以及同窗口 Portable UI 的
会话与迟到请求处理。获批的权限只读合同在 `0f8c9f4` 实现；不改 State Schema、Portable UI 协议或 Runtime，
不新增导航、权限、依赖或自动授权。路径标签继续复用 `displayPath`，RPC/文件参数保留原值。
GUI 接线提交为 `88c3f69`；真实 Core 复验暴露的必需权限启用误报成功、旧包声明误授权，已由 `e2b7ddd` 在现有
Store 事务内修复，启用/恢复、授权和读取统一使用当前包的冻结声明，不改变公共合同。
人工截图随后暴露共享错误提示只显示通用说明和原始错误码；`52ed23b` 为扩展、公共加载/分页/任务及 Resources
补稳定错误码对应的可读原因和行动，错误码/诊断编号默认收进详情，保留未知写入与权限/信任边界，不展示 RPC 原始 message/data。
新 GUI check（1021 条三语言）与一次 release no-bundle 构建通过。关联定向首轮 **78/1**、173 项输入未变；
唯一失败为测试在点击“详情”后才断言开关焦点，调整顺序后仅复验相关 **6/0**，生产输入未再改，不拼成一次 79/0。
旧 GUI 的 36/0 继续绑定其旧输入；Core/Store 13/0 仍对应未变的 `e2b7ddd`，不重跑 Rust/浏览器全套或 CI。

GUI 独立定向浏览器 **36/0**（1 worker、0 retries），155 项输入不变，沿用 `88c3f69` 的 GUI 构建；最终 Store **13/0**
包含权限读取及新增拒绝/幂等回归。dispatcher **2/0**、协议 **1/0**、M6 契约 **5/0**、SDK/GUI check 绑定各自未变输入；
初次七 crate Clippy 保留，Store 修复后再次通过 Store/应用 Clippy、静态 xtask 与必要 Core release 构建。
早期浏览器 **20/15**、**26/10** 和修复定向 **11/0** 分别保留，不拼接。未重跑完整 Workspace、浏览器全套或三平台 CI。
真实隔离 Core/Host 已完成签名包 Plan/Apply、默认停用且无授权、项目授权/撤销、Portable UI 同连接打开/操作/刷新/关闭、
崩溃隔离与保留数据卸载。故意 trap 的扩展显式恢复仍返回 `extension_crashed`，不能描述为恢复健康。

前轮原环境实际恢复及五组一致证据保留。本轮已实际验证 `H4-Review.ps1 Start/Restore` 往返，恢复时五组公开结果
与本次暂停前完整快照一致（Projects 2、Editors 1、Operations 2 全终态、settings revision **155**、Status ready）。
随后按所有者要求打开过可操作的隔离验收环境：Core **138156**、更新后 GUI **128624**（窗口 **4068254**），
复用 Core `7d62a6…da7a18` / 新 GUI `985e11…222b11`，隔离数据为 `target/m7-h4-extensions-20261102/data-native`。
正常签名样例 `com.example.h4.portable` 已验证保留数据卸载/重装/启停，交付时留作待安装；一个可丢弃项目已登记。
验收后已执行 Restore：隔离任务全终态，关闭记录的隔离 GUI/Core，实际恢复原数据目录的 Core **136212**、GUI **132312**。
五组完整公开状态与本次切换前一致（Projects 2、Editors 1、Operations 2 全终态、Settings revision 155、Status ready），
证据为 `target/m7-h4-extensions-20261102/manual-review-restore-20261102T0639271018499Z/five-group-comparison.json`。
原环境未安装测试扩展；没有处理用户 Unity/未知进程。桌面控制保持停用，没有新增自动化原生截图。
所有者实际查看并认可 H4 功能与错误可读性；测试样例未覆盖的表单、会话超时、外部断线及正式 MCP/Discord 业务仍未验证。
H2-B/H3/H4 新增代码均无对应远端 CI/CodeQL；旧 `0ef5b08` 的成功仍只对应旧输入。未推送。详见
[H4 候选、隔离证据与查看清单](testing/m7-h4-extensions-20261102.md)。
启动/恢复入口及历史清单见该记录；本次验收收尾已实际执行更新后 GUI 的恢复入口并核对原进程身份，不再停在守候状态。

### H3 已签收记录（各轮历史证据）

项目所有者于 2026-11-02 确认 **H3 阶段性人工验收通过，保留已记录的非阻塞体验问题**，并明确批准直接进入
既有 H4 Extensions / Portable UI；不重复验收已检查页面，不进入 H5-H7，不自动推送。H3 代码候选为
`fec8117b289fb696cd193c3a52d8afad99e10d50`；已确认 Unity 正常自动检测并显示、刷新仍为同一条、路径正常。
该签收不表示体验完善、整个 Visual Gate 3 或 M7 完成；原有 H2-A/H2-B 签收保持。
本次新反馈是安装路径直接显示 Windows 内部 `\\?\` 前缀。现将项目列表已有的路径显示规则收束为共享
`displayPath`，覆盖安装列表/完整路径 tooltip、启动选择及相关项目、来源和目标审阅；只格式化盘符与 UNC
标签，特殊命名空间保持原样，RPC、编辑状态、文件操作与安装身份继续使用原值，URL 不参与路径格式化。

本次独立定向浏览器 **10/0**（1 worker、0 retries），GUI check（919 条翻译）及 GUI release 构建通过。
两轮早期失败 **8/2**、**9/1** 分别保留，不拼接通过项。未重跑 Rust、浏览器全套、Core 构建或三平台 CI。
真实窗口已核对 **设置 → 已安装的 Unity → 刷新**：`2022.3.22f1` 仍为一条，路径显示
`C:\Program Files\Unity\Hub\Editor\2022.3.22f1\Editor\Unity.exe`；完整窗口整改前后截图已保存。
前后完整 `unity.installations.list` 结果相同，Core 内部路径与 installationId/revision 未变。
该路径整改轮的 release GUI PID **132564**（窗口 **7868742**，SHA-256 `cc4f…9808a`）当时打开且可响应；
Core 为 `efdb133` 的原进程 **96504**、产物 `ef371…1b5ca`，连接原
`target/m7-icon-acceptance-20260906/data`，该轮未暂停/重启 Core，没有处理用户 Unity 或不明进程。

此前 Windows 检测修复 `efdb133a82f6ead7b0436919dddd7cab939e94c4` 的 platform **8/0**、daemon **3/0**、
exact version **1/0**、RPC **1/0**、浏览器 **6/0**、Clippy 与 Core/GUI 构建继续绑定该轮输入。
根因为 Package Manager 清单没有 Editor `version`，旧发现链静默丢弃候选；现读取 Editor 自身版本资源并保留
canonical/exact 校验。真实自动发现及重复刷新、两个样例只读 exact candidates 已验证；当时 Core 更新前后
五组公开状态一致，登记后仅 Editor 从 0 到 1，其他四组保持、settings revision **155**。
当时 Esc 后未取得安装页原生证据的历史记录保留；本次所有者反馈及上述真实窗口核对分别补齐当前检测与显示证据。
没有手动登记或启动/迁移实际工程。所有者随后明确确认“路径显示正常了”，`fec8117` 的路径显示整改项
人工确认通过；Unity 检测、显示与重复刷新不再待确认。随后 H3 阶段性签收以本页开头记录为准，其他 H3 页面不重验。

上轮 `ab5d183` 的 **34/0** 与主体 `ef47c25` 的备份、恢复、结果 ProjectId 和反馈原生证据保持各自原提交/构建范围，
不替代本轮代码的验证。范围、逐轮失败、产物摘要和当前窗口见 [H3 候选与桌面证据](testing/m7-h3-contextual-20261101.md)。
H3 新增代码仍没有对应远端 CI/CodeQL，旧 `0ef5b08` 的通过结果不替代；M7 进行中，未推送。
H4 开始时的交接只读核对已完成：当时原 Core **96504** 使用原数据目录，验收 GUI **132564** 有窗口且可响应；
Projects/Editors/Operations/Settings/Status 完整公开读取为 2/1/2/1/1，任务全部终态、settings revision **155**。
交接时未发现隔离 Core 占用，无需暂停或恢复，也未处理用户 Unity/不明进程；后续 H4 构建与隔离切换、实际恢复见本页当前记录。
H4 先复核既有安装/启停/卸载、权限/隔离及同窗口 Portable UI，只补缺失接线和生命周期，复用 Material、Reminder、
Plan/Apply/Operation 与 `displayPath`。不重写 Runtime/Portable UI 协议、不提前实现 M8/M9。

此前 Resources 体验整改与仓库列表交换已在标准目录/main 本地实现，提交为 `348dbef`、
`ceef00f`；`2a163b0` 仅修正测试类型检查，生产输入与 `ceef00f` 一致。尚未推送。
共享 Reminder 将短成功反馈移出正文流，错误/风险/部分结果保留在字段或对应弹窗；新增列表
预览、选择、显式追加与 Core 全量导出，包含 query 保真、过期预览及新建文件保护。
H2-B 本地技术候选 `2a163b0` 的定向浏览器 **47/0**、相关 Rust/协议/Clippy/metadata 与 release
构建已通过；没有重跑完整 Rust Workspace、292 项浏览器或三平台 CI。项目所有者于 2026-11-01
确认 H2-B **阶段性人工验收通过，保留体验改进项**，允许结束本轮体验整改。范围为 Resources
的仓库（含列表导入/导出）、用户包、模板与相关键盘/弹窗体验；不表示体验完善、所有问题解决、
整个 Visual Gate 3 或 M7 完成。Agent 此前因 Computer Use 中断未取得整改后原生截图的记录保留，
不追写为已执行；本次签收依据项目所有者的人工确认，H2-A 原签收保持。
原开发环境在 H2-B 签收后曾实际恢复：当时 Core PID 118832、开发 GUI PID 121112 有窗口且可响应，
Projects/Editors/Operations/Settings/Status 五组公开结果与暂停前完全一致，settings revision 155。
当时隔离任务完整分页确认 11 项全终态，H2-B 隔离 GUI/Core/两个样例服务均已停止；H3 本轮最新恢复见上。
远端仍为 `0ef5b08`，原同 SHA 三平台 CI/CodeQL 成功保持有效；本轮新增代码尚未取得远端验证，
不借用旧结果替代，且本次没有新增推送授权。

- 唯一工作目录：`C:\Users\M2922\MyProjects\Rust\ALCOMD`，活动分支 `main`；不另建工作树。
- 验证按风险分级：日常定向测试及必要检查，模块完成后覆盖相关回归，稳定候选再集中执行完整
  门禁。三平台 Hosted CI/CodeQL 用于重要技术检查点、阶段收尾及最终验收候选；不逐小修改或
  提交重复运行。已运行 CI 不因此取消，推送仍按原授权；最终结果必须对应实际候选 SHA。
  具体安排见 [M7 验证计划](exec-plans/M7-official-gui-extension-ui.md#验证与-release-blockers)。
- H2-A 验收检查点：`23f545245f613f47f8def1e16418c4d12303404e`，包含本轮架构、P6 wire、
  测试门禁、真实桌面交互修正及 Visual Gate 2 签收记录。其前置检查点 `4eae1d4` 包含
  Core `626ec35`、共享 UI `fbb4213`、GUI `7357d80` 和对应验收记录。
- 本轮开始时本机 HEAD 与远端 `main` 均为 `736f55a9eea504442fa8d95e6c26cfc5559a40b7`；暂存区与
  tracked 工作树在本轮开始时为空，原有 `output/playwright/` 两张截图保留。
- 项目所有者已批准依次完成技术收尾、H2-B 实施、真实桌面候选。技术收尾已完成跨平台测试与
  Wasmtime/rustls/ZIP 官方安全公告的逐项评估及获准补丁；跨不兼容版本等仍按独立审批边界执行。
  技术提交独立于 H2-B。所有者已先后明确批准技术 SHA `73f465b` 与后续
  `02b6f0c3e81b8eedb711123ef9de2edc228792ab` 在保留各自本机 GUI 失败记录的前提下快进推送，
  以同一最终 SHA 三平台完整 required jobs 与 CodeQL 全绿验证本轮技术收尾；适用漏洞已修复且
  其他公告完成适用性判断后，H2-B 自动获准实施。
- 本轮已选定并锁定 Wasmtime 48.0.8、rustls 0.23.46、rustls-webpki 0.103.16；项目所有者另行
  明确批准 ZIP 精确 9.1.3、必要传递闭包与 API 适配，保留既有归档格式、安全和事务。当前
  RustSec 版本匹配为 6 项未启用的 QUIC/wasm32 证书包链，维护/unsound/yanked 另逐项记录。
  [技术收尾证据](testing/m7-technical-closure-20261031.md)保存完整公告、锁图与每轮实际结果；
  归档安全与双向格式回归、完整 Rust 491/0/8、前端检查/构建、冻结基线、并发 CLI 启动、
  release bins 和 Tauri no-bundle 已通过。首个技术提交的完整 Chromium 为 237/10，10 项在文档或
  必需静态 JS 加载时遇到 ERR_NO_BUFFER_SPACE、业务未到达，该失败及其额外推送批准保持。
  原样例 Core/开发窗口已恢复，Projects/Editors/Operations/Settings 与任务前一致，revision 155。
  获批技术 SHA 已在 `origin/main` 核实；
  [CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/41204615683) 四语言成功。
  [三平台 CI](https://github.com/ALCOMD/ALCOMD/actions/runs/41204615658) 已结束：Windows 完整成功，
  browser 247/0；macOS 的 Hub 8 项通过，后续候选 RPC fixture 超出 103 字节 socket 限制失败；
  Ubuntu browser 245/2，固定宽 Select 的字体换行与表格测试缺少剩余空间已由诊断复现。
  macOS 现沿用已有短临时根规则，保留 UUID 与原断言，定向 1/1、完整本地 491/0/8 通过；
  Select 仅改已有内容宽度规则，表格保留全部原断言，相关定向 3/3。后续候选根 npm check/build、
  release no-bundle 通过；独立完整 Chromium 为 **246/2（15.5 分钟）**、150 个输入未变：一项
  跨越 30 秒 test budget，另一项标题检查失败后才有完整页面帧，批量业务未到达，不能计为通过。
  未改系统 TCP、浏览器 flags、超时、重试或包管理实现。后续技术 SHA `02b6f0c` 已经按明确批准
  快进推送，原样例再次读回一致；[该 SHA 三平台 CI](https://github.com/ALCOMD/ALCOMD/actions/runs/41209763063)
  已结束：Windows 与 macOS 完整成功、browser 均为 248/0；Ubuntu 为 247/1，唯一失败是
  257 目标用例的 5 秒包管理标题检查。[CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/41209762434)
  已四语言成功。大列表后续候选将未打开的行菜单延至首次使用，并在内容获得一次绘制机会后
  初始化滚动容器。相关包管理、滚动条及导航 63/0，前端 check/build、release no-bundle、xtask
  与 metadata 通过；独立完整本地 Chromium 为 **247/2（18.4 分钟）**、150 个输入未变，
  257 目标与新增菜单回归通过。两项失败分别为必需 JS 的 ERR_NO_BUFFER_SPACE 和四宽度网格
  累计 30 秒预算耗尽，未覆盖部分保持未通过。所有者已另行明确批准精确技术 SHA
  `1932140a815dd12712cd00108d70acebe51a3560` 保留本机失败后普通快进推送，已核实该 SHA
  位于 `origin/main`。[CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/41221092450) 四语言已全部
  成功；[该 SHA CI](https://github.com/ALCOMD/ALCOMD/actions/runs/41221092743) 已结束：Ubuntu 与
  macOS 完整成功、browser 均为 249/0；Windows browser 为 248/1，唯一失败是正常动画分支
  的一次性滚动条样式读取为 0 秒。六组合本地观测符合原阈值，没有远端 trace 可确定唯一原因；
  后续测试候选在默认断言时限内等待实际样式并增加 property 校验，保留数值阈值与全部业务检查。
  动画专项 9/0、前端 check/build 与 metadata 通过；独立完整 Chromium 为 **248/1（12.6 分钟）**，
  150 个输入未变。唯一失败是必需静态 JS 的 ERR_ADDRESS_IN_USE，Hub 页面根为空，输入/保存/
  焦点业务均未到达。未重跑或改 Hub；原样例五项 RPC 再次完全一致、revision 155。
  本机完整门禁仍失败；所有者已明确批准精确 SHA `97c761f76613ea68ebc7a93e4695940dabb969dd`
  保留该记录后普通快进推送，已核实位于 `origin/main`。
  [CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/41228566315) 四语言已全部成功；
  [该 SHA CI](https://github.com/ALCOMD/ALCOMD/actions/runs/41228566567) 三平台完整 required jobs
  已全部成功，browser 各 249/0，Windows/Ubuntu 为 4.8 分钟、macOS 为 9.3 分钟。
  同一最终技术 SHA 的集中检查点已通过，前述本机失败记录保持；H2-B 按既有授权开始实施。
  GitHub 仍 open 的 11 条告警与任务开始时集合相同，当前锁定版本已不匹配其影响范围；
  告警状态未人工修改，完整 RustSec 及维护判断仍见技术记录。
- H2-B 整改起因（历史反馈）：既有流程可用，但提示放置挤动/破坏 Resources 布局，且仓库列表
  导入/导出缺失，当时体验未通过。真实窗口已复现成功与失败提示挤入仓库行操作区，
  完整窗口截图保存在 h2b-resources-remediation；先修 Resources 及直接共享提醒组件，保持侧栏、
  画布、图标和桌面密度，再补仓库列表能力。新公共文件 RPC/capability 的修订窄合同和共享提醒框
  已获明确批准，先同步 ExecPlan/Schema 后实施；参数化 URL 完整保留、显式确认导出，不改 State Schema。
  已按获批合同完成并取得本次阶段签收；其余提示细节作为下方非阻塞待办，不在本轮继续扩修。
- [H2-B 既有方案](exec-plans/M7-official-gui-extension-ui.md)此前已形成远端候选与桌面材料。
  `0ef5b08` 的三平台 CI/CodeQL 技术检查通过保持有效，不能证明本次发现的遗漏功能已实现。
  模板分页、
  三类 review、页面任务持有与成功刷新、用户包详情已接线。首个候选 `6988bc6` 的完整
  Chromium 292/0 保持独立；该轮 native stale Apply 暴露 Core 接受后才重验、留下失败
  Operation 和目标文件，不能称作该候选 native 通过。最终代码候选
  `cbb4c4198a21519efb24d8f0a04a509664e83b50` 在原 Store 接受事务内拒绝已知登记漂移，
  保留最终重验和原幂等重放；完整 Rust **492/0/8**、根 check/build、冻结基线、release bins、
  Tauri release no-bundle 通过。两轮独立完整 Chromium 为 **290/2（12.1 分钟）**和
  **283/9（12.5 分钟）**，各轮前后 651 项输入未变；全部失败有必需 JS/HTML 的
  ERR_ADDRESS_IN_USE 或 ERR_NO_BUFFER_SPACE、业务未到达的 trace，仍按整轮失败记录。
  上述两轮未改断言、超时、重试、浏览器 flags 或系统 TCP。随后资源加载专项捕获 WinSock
  10048；仅回移 Playwright 官方端口随机化修正的完整门禁为 **291/1（10.8 分钟）**，残余
  失败另由带 NetLog 的有界采样复现，不能称作修复通过。fixture 随后改由服务端在响应完成后
  关闭 HTTP 连接，客户端 TIME_WAIT 明显减少；583 个上下文/4,081 个 200 响应未发生
  fixture 请求失败、同端口重绑通过后，独立完整 Chromium 为 **292/0（11.2 分钟）**，
  652 项输入前后相同。测试启动配置完整保留原 16 项浏览器默认禁用列表，仅增加上游一项；
  原断言、30 秒超时、单 worker、零重试、系统配置和生产代码保持。专项基于 `173c956`
  工作区，四个测试文件与三份相关文档已提交为
  `0ef5b088dfdb7a8eb2b7a88ebef94ae436c90f50` 并普通快进推送 `origin/main`，完整输入摘要见验证记录。
  各失败轮独立保留，不宣称消除 Windows 全局端点限制或确定所有错误的唯一系统原因。
  当前 release GUI/Core 的隔离 native 已复核 stale 零接受副作用、两入口创建及文件/ProjectId、
  import/override/export/derive、两类仓库、用户包详情/显式刷新/移除、收藏及偏好重启持久化、
  断线 Retry、代表性深浅色/键盘焦点和用户模板移除。builtin GUI 删除禁用、Plan 替换拒绝；
  先前 CLI confirmed remove 被自动审批拒绝的历史记录保留，没有绕过；项目所有者随后明确授权
  在既有可丢弃隔离环境补齐负向测试。正式 release CLI 带 --yes 对内置 Blank/rev1 的唯一请求
  返回 template_builtin_immutable、exit 1；包含目标模板的七组公开状态、63 个文件的路径/大小/hash
  前后完全一致，无新 Operation，见 builtin-rejection-verification.json。
  收尾定向 native 使用摘要未变的同一 release GUI/Core；新 Operation `1904b87d` 返回
  ProjectId `d138da13-ad72-4de8-b438-d99b70c43f66`，点击“查看已创建项目”后真实窗口 URL、
  公开 projects.get 与目标目录完全一致。资源分段 Tab/Shift+Tab、Enter/Space 已复核；
  Left/Right 保持焦点与路由，符合现有分组按钮合同，不新增方向键导航模式。
  前次定向收尾中，原样例五项完整公开结果与暂停前相同、revision 155，原开发窗口已恢复并实际查看；
  旧失败任务、残留五文件及其余外部样例保持。当前远端 SHA `0ef5b08` 的
  [CI 41287607183](https://github.com/ALCOMD/ALCOMD/actions/runs/41287607183) attempt 1 三平台
  完整 jobs 全部成功，Chromium 各 **292/0**（Ubuntu 7.5、Windows 6.8、macOS 11.4 分钟）；
  [CodeQL 41287607108](https://github.com/ALCOMD/ALCOMD/actions/runs/41287607108) attempt 1
  四语言全部成功。只收集已有运行，没有重跑 workflow 或为状态整理重复完整本地门禁。
  逐项矩阵、证据复用绑定、候选入口、代表性截图与 Resources 人工检查清单见
  [H2-B 开发验证](testing/m7-h2-b-20261101.md)。最终验收准备复用此前实际成功的启动步骤，
  前次 release GUI PID 104572、Core PID 77432 使用既有 H2-B 隔离目录与 profile，曾保持打开于
  Resources / Repositories，包含两个合成仓库、用户包 1.2.0/rev1 和原五个模板。
  原开发环境再次按授权暂停，暂停前五组完整公开结果已保存，无活动任务；未停止 Unity/未知进程。
  所有者反馈后，旧恢复守候及本轮开发窗口经身份核对已停止，曾暂停隔离 Core 完成定向 IPC
  与新构建。本次阶段签收后按既有授权关闭本轮窗口，守候已实际完成原环境恢复，随后独立核对
  五组公开状态与原 GUI 响应；见 acceptance-restoration-verification.json。负向请求证据对应
  未改变的 builtin guard 输入，保持有效且不重复执行。
  本轮新增技术/桌面结果与最终保持打开的进程身份见 testing 文档，不复跑旧完整门禁或三平台 CI。
  H2-B 按本消息记阶段性人工验收通过并保留体验待办，不扩大为 Visual Gate 3 或 M7 整体验收。
- 已验收范围：Project/Packages 和已获准的其他既有页面基础交互修正、统一公共组件及验收。
  H2-B 本轮体验整改已结束；项目所有者已明确批准现有 H3 范围直接实施，H4-H7 与推送仍未授权，M8、M9、M11 不在本次范围。
- 当前源码格式：State v16、Config5、RPC v1、Extension ABI v1；各旧验证/运行产物仍按其实际 Config4 记录。按 A-034 保留 Schema 识别，
  未发布开发库不承诺长期兼容；不自动清空现有数据库、Plan、Operation 或恢复证据，
  首次发布前的初始 Schema 整理是独立审计任务，当前版本与迁移保持。
- 目录整合及旧修改恢复方式见 [工作目录整合记录](testing/workspace-consolidation-20260917.md)；
  `codex/archive/*` 仅用于恢复历史，不是活动开发线。

## 当前有效实现

- 包管理：Core `packages.queryProjectCandidates` 提供来源保真、SemVer/Unity/歧义、
  snapshot/paging/quota 证据；GUI 行内版本、条件更新、Update All 和完整选择集批量操作复用
  单次既有 Plan、显式 Apply 与一个 Operation。经项目所有者批准，P6 source/reinstall/bulk
  的实际 wire 已修正为冻结 camelCase，候选使用独立 CandidateSource 保持冻结 snake_case；
  真实 Tauri 六种 Plan review 和公开 RPC 8 项复核通过。普通 resolver 和来源优先规则保持。
- 公共 GUI：真实 Material Web、DesktopList、PageHeader、SplitButton、Dialog、ScrollArea 和
  分段控件统一持有组件规则；默认桌面窗口 1400×800，容器圆角 28px，不增加窄窗口适配。
  Open Unity 继续使用原 leading play 与 18px button icon。异步包 Plan 保留稳定原控件焦点，
  MenuItem 支持公开 Enter/Space 选择；Select 保存后恢复可用且选中标签随语言同步。
- 主题/语言：共享动态明暗九方案、默认 `#6CB6FF`/Vibrant，外观页直接展开主题，移除 HCT 滑条，实时预览并明确 Save/Cancel；中文模式为“浅色 / 深色”；
  字体仅使用本机 Noto/CJK fallback；三语言为英语、简中、日语，静态词条与 React 文本插值。
  动画偏好 system/full/reduced；普通设置修改即经串行 CAS 保存，不再有全局 Save/Discard。
- Core 设置：application 持有权限/CAS/校验/候选缓存和取消一致性；Store 的 ConfigStore
  适配器持有文件解析、16 KiB 界限、原子替换与恢复。Config1/2 是已有迁移输入，不是当前写入格式。
- 项目时间：lastModifiedAtMs 维护批准的注册/接受启动/成功变更边界，未知不补造；
  项目 observedAtMs 已从活动模型和 State16 移除，安装扫描/writer/迁移专用证据保留。
- Unity：精确版本 0/1/多安装启动与迁移保持；Hub 路径选择、编辑、自动查找及打开/下载已接线。
  打开经窄权限和 local-owner 校验，Core 重验 revision/路径，平台有界回收自己创建的 Child；
  未确认草稿不隐式保存并启动，未知结果不自动重试。

当前合同见 [M7 ExecPlan](exec-plans/M7-official-gui-extension-ui.md)、
[候选合同](exec-plans/M7-package-candidate-evidence-stop-a.md)、
[Config5](../specs/config/settings-v5.md) 和 [Hub 合同](../specs/config/unity-hub-location.proposal.contract.json)。

## 架构审查修复与验证

本轮修复五项已审阅问题：

1. UtilityWorkspace 的离线 Retry/Refresh 使用共享 reconnect，恢复页面数据和连接能力。
2. Available/Missing source 只使用完整 Core providers/肯定证据，可选元数据首页不再决定状态。
3. hello.dataSchema 由 daemon 提供真实存储版本，新增 dispatcher 和实际 SQLite user_version
   一致性回归，不让 protocol 依赖 Store。
4. 公共列表正文统一稳定 typography 行盒，处理 Noto/CJK 混排，不加页面特例或放宽原断言。
5. 当前架构/RPC/ExecPlan/状态原条款直接同步有效格式、依赖、范围及 Schema 策略。

P6 修复后的 fmt、Workspace Clippy、xtask、metadata、前端 check/build、生产
daemon/CLI/Extension Host build 和 Tauri **release no-bundle** 均通过。完整 Workspace 为
**483 passed / 0 failed / 8 既有 helper ignored**。新增 P6 44 项 shape 与候选两种来源的
嵌套 Serde 回归通过，候选冻结 Schema/vectors 的 hash 不变。
P6 wire 修复轮完整 GUI 为 **238 passed / 0 failed**（9.0 分钟），单 worker、零重试，
不代替下述 H2-A 最终结果；此前逐轮失败与 trace 保留在对应 testing 报告。

真实 IPC 测试现先等待自建 daemon 初始化并绑定成功，服务 task 结束后仍由测试 handle
保留端点所有权；35 个启动点已接线，四项所有权回归通过。Copy 用例改为独立 writer RPC
证据 oracle，保留全部 Apply/内容/注册/replay 验证，并新增 Apply 重验 confirmed 无副作用
回归。生产 Copy 仍只对 confirmed 硬拒绝，suspected/unknown 为风险证据，未停止用户 Unity。

GUI fixture 预构建三个现有入口，保留 development React/StrictMode、现有工具链和全部
断言。真实浏览器 HTTP 响应从 148/116/126 降到 7/5/5，独立 HTTP connectionId 从
6/6/6 到 6/4/4，HMR WS 从各 1 到 0；请求数未当作 TCP 数量。本轮时段未查询到新增
System/Tcpip 4227 事件，不据此承诺 Windows 全局端点限制已被消除。

本轮真实桌面使用新的临时 Core、独立 WebView2 profile 和纯 JSON/文本包，已完成真实
GUI Apply 1.1.0、Running→Succeeded、自动刷新，并在 Activity 精确核对同一 OperationId。
另由正式 CLI 将项目降为 1.0.0 / revision 3，原 GUI Plan 明确拒绝；拒绝前后完整项目、
Operation 列表及文件 hash 相同。Settings 真实断线/Retry/重连恢复数据和能力、分组键盘
导航、双向弹窗 Tab/Escape/返回焦点、来源/预发布筛选、主题/语言和 Core/GUI 重启持久化
均有记录。Hub 原生选择、同组草稿/Escape 和实际 Open 已检查；下载和无现成 fixture 的
native Portable UI 按清单记未执行。

最终 release GUI SHA-256 为 `0AA8ADA4BABA8192D6953FBEA6E93CE106942B4030B02F132E9776EBF435BCD2`。
前端 check/build、release no-bundle、根 npm check、xtask 和 metadata 已通过。焦点修正轮
完整 GUI 为 245/0；最终候选首轮为 246/1，唯一失败为必需 JS 资源的
`ERR_ADDRESS_IN_USE`，同刻 Windows Tcpip 4227。保持输入、断言、超时和零重试后，独立
完整重跑为 **247 passed / 0 failed（9.4 分钟）**，前后代码/测试 hash 一致，该时段无新增
4227。各轮和定向结果独立保存，不声称系统全局端点限制已消除。

临时 GUI/Core/服务器已关闭；临时偏好值恢复初始，原 daemon/Tauri dev 按原命令恢复。
两个原样例 Project、Editor 空列表、Operation、全部设置和 revision 155 与测试前相同，
开发 GUI 有窗口且可响应。每张 native 截图标记实际构建 hash；项目所有者已签收本轮
Visual Gate 2，没有 M7 或 Hosted CI 通过结论，原未跟踪 `output/` 截图保留。
详见 [Visual Gate 2 签收记录](testing/m7-visual-gate-2-20261031.md)、
[P6 与前一轮门禁](testing/m7-test-gate-closure-20261031.md) 和
[架构审查原轮次](testing/m7-architecture-review-fixes-20261031.md)。

## 已有验收证据

| 检查点 | 已证明的范围 | 证据 |
| --- | --- | --- |
| M0-M6 | 对应里程碑及项目所有者人工验收完成；M6 final `b666e8f` / CI `32724915827` | 各里程碑 ExecPlan |
| M7 Portable UI B0-D | `aa13234` / CI `32877438910` 三平台技术通过 | M7 ExecPlan |
| Project/Package P0-P8、exact Unity | 原技术检查点保留；Unity `e9349fd` / CI `33352406804`、CodeQL `33352406214` | Project/Package、Unity ExecPlan |
| `4eae1d4` 前的本地 Hub/设置检查点 | 完整 Workspace 474 passed / 0 failed / 8 既有 subprocess-helper ignored；Clippy/fmt/xtask/metadata/check/build 与活动 Schema 12/12 通过。Hub browser 首轮 43/45、定位修正后专项 3/3，不能合称一次完整 GUI 通过 | [Hub 记录](testing/m7-unity-hub-location-20261031.md) |

不同 SHA/轮次的日志不能合并为当前候选通过。三平台技术结果不等于 Windows 10/11 完整客户端发行验收。

## 剩余边界与下一停止点

- P6 修复的契约、自动化与真实 Plan 证据保持；本轮 H2-A 与 Visual Gate 2 已获项目所有者签收。
  技术收尾已达成同一 SHA 的完整远端检查点；H2-B 远端候选 `0ef5b08` 的三平台 CI/CodeQL
  已全部成功；本地后续代码 `2a163b0` 已获 H2-B 阶段性人工签收，仍保留体验改进项及新增代码
  远端验证缺口。下载、native Portable UI 等旧未覆盖范围保持，不由本次签收批量关闭。
- 已执行获批 G3 的完成度核对，三个仓库列表 RPC 的矩阵 **92/92** 是当时结果；H4 新增获批权限读取后，
  当前公开 method 与矩阵 **93/93** 对齐。方法归属和 typed 接线不代表全部动作已独立人工验收。
  H3 签收及 Unity 检测、刷新、路径确认保持。H4 已由所有者阶段性签收，保留样例未覆盖项与一般体验待办；
  原环境已实际恢复并核对五组预期一致。H5/H6 已阶段性签收；H7 原 CI Ubuntu 395/1 保留，窄测试修复本地 13/0，最终 `c273c76` 同 SHA 三平台 CI/CodeQL 全绿，等待最终人工确认与剩余事项处理，不自动推送。
- 窗口大小/最大化状态记忆已按所有者后续要求纳入本轮，默认关闭，具体结果见上方最新增量；窗口位置/全屏偏好不在该合同范围。原生恢复交互仍需所有者查看。
  仓库 credential enrollment/revocation 属 ADR 0016/H2-B 已排除的后续 Core/公共合同能力，当前无相应 RPC。
  初次设置、导入/迁移、更新及完整 v3 入口对齐仍按既有后续范围处理；聚合 projects.management/packages.vpm/repositories.management 等保持 in_progress，不批量提升。
- projects/templates/backups 的 v3 parity 和 unity.v3-vrc-parity 仍等待 M11 真实脱敏 Fixture；
  synthetic/public 工程测试不能冒充真实 differential parity。
- M12 继续承担 Windows 10/11 安装、运行、WebView2、更新/卸载、三平台真实辅助技术与完整发行验收。
- MCP、Discord 产品逻辑、Local API、v3 migration、bootstrap/updater/installer/dist 属于后续范围。
  已有开放依赖告警仍需独立适用性评估，不因这轮 GUI/握手修复关闭。

### 非阻塞体验待办（H2-B/H3/H4/H5/H6 阶段签收保留）

| 页面 | 触发条件 | 实际问题 |
| --- | --- | --- |
| Resources / 添加仓库 | URL 或本地清单检查失败 | 同一字段提示同时要求检查网址或清单路径，未贴合当前输入种类。 |
| Resources / 导入列表 | 检查、确认追加及部分结果 | 流程说明、汇总和逐条状态叠加；“失败”未清楚区分检查与登记阶段，信息层次偏重。 |
| Resources / 导出列表 | 打开预览并选择新文件 | 地址分享、新建文件、非完整备份等说明分散且重复，确认文字和保存引导挤占主要操作关注点。 |
| Resources / 用户包与模板 | 刷新、收藏、导出及重试 | 泛化的反馈文案和技术说明仍偏多，部分结果信息位置需要简化。 |
| 任务详情 | 任务已终态 | 仍显示技术阶段字符串及禁用的任务控制容器，信息可继续合并、删减。 |
| 备份详情 / 恢复审阅 | 显示目标与 VPM 依赖风险 | 目标可换行，但摘要、Core 说明和风险文字仍有重复，后续优先改写与合并。 |
| 设置 / Unity 安装管理 | 尚未选择移除目标时 | 添加、发现与禁用的确认移除同框，操作层次偏重；后续集中精简，不在检测修复中重做。 |
| Extensions / 安装与权限 | 审阅安装、修改授权及操作结果 | 一般提示文案、说明层次和信息位置仍可删减合并，部分操作偏重；共享可读错误整改已获认可。 |
| Extensions / Portable UI | 打开、切换及关闭扩展页面 | 普通状态与会话说明仍可简化；表单、超时、外部断线及正式 MCP/Discord 业务另列未覆盖，不视为已通过。 |
| Settings / Log / About | 保存、查看详情和辅助状态 | 一般文案、提示位置及说明层次仍可合并简化；H5 阶段签收保留，不因 H6 全部转为阻塞。 |
| Logs / Diagnostics | 收到 `package_archive_invalid` 诊断 | 保留真实失败码，但可读原因仍为泛化的“暂时无法识别”；后续补准确简短原因，不代表归档校验失效。 |

后续集中优先删除、合并、改写冗余提示，减少无必要步骤与机制，不增加提示解释过度设计。
真实错误、操作结果与必要的破坏性确认保持准确；误导危险操作、误报成功、数据损坏或其他实际
功能缺陷不属暂缓范围，发现后及时处理。
