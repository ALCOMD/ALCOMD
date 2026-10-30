# 项目状态

2026-10-31 当前代码检查点 `7357d8054e425400ad2befe5dd93132d6c073689`：Core／公开合同 `626ec35`、共享 UI `fbb4213`、GUI／回归 `7357d80`；本轮由项目所有者明确授权分组提交并推送，验收文档单独归档。Hub 自动查找与打开／下载位于卡片右上角，路径仍可直接编辑；Core 复核 revision／有效目标后启动，仅确认无配置且未找到时由 GUI 打开固定官网页，草稿及未确认保存阻止启动，未知结果不自动重试。完整 Workspace 474 passed／0 failed／8 既有 subprocess-helper ignored，Clippy／fmt／xtask／metadata、活动 Schema 12/12 及 check/build（844 词条）通过；浏览器首轮 43/45，纠正两个 Material 宿主 tooltip 测试定位后针对 3/3 通过，不拼作单轮 45/45。匹配源码的 debug daemon／dev 已恢复，Core ready 并公布新能力，配置摘要保持。构建与当前功能证据见 [Hub 验收记录](testing/m7-unity-hub-location-20261031.md)。真实 Hub／下载网页与屏幕阅读器仍待人工验收，窗口记忆开关及其他未闭合项不因本次提交关闭；Visual Gate 2 PENDING，H2-A 保持开放，不进入后续阶段。

2026-10-31 当前 Hub 路径允许直接编辑：文本为本地草稿，Enter 或离开整个操作组经原串行 CAS/Core 校验保存，Esc 撤销；IME、未修改值、同组按钮间 Tab 不提交。失败保留原文/错误，旧失败文本重复失焦不重试；自动查找/选择/Retry 不先提交未完成文本，普通保存忙时可排队，Core 归一化成功后同步。共用 TextField 补标准焦点/键盘事件，Hub 按钮复用 Material softDisabled 保焦点防重复，沿用既有 8px 内容间距，无新增 CSS/后端/DTO/Schema/依赖/native Rust。最终 check/build、33/33 前端回归及既有 HEX/HCT 单例 1/1 通过；首轮 30/32 的焦点问题和非 composed 测试事件已定位修正，日志独立保留。当前 dev 按原命令打开供人工检查；未提交/推送，Visual Gate 2 PENDING，桌面与屏幕阅读器证据不以浏览器测试代替。见 [Hub 验收记录](testing/m7-unity-hub-location-20261031.md)。

2026-10-31 当前 Hub 修正：所有者确认去掉 Windows 内部前缀，并增加“自动查找”按钮。平台仅在解析回同一 canonical 文件时使用普通盘符/UNC 格式，GUI 不裁剪路径；本机只读检测得到 `C:\Program Files\Unity Hub\Unity Hub.exe`。自动查找复用 hubPath:null/既有串行 CAS；已在自动模式只重读 Core，同 revision 更新证据，不写配置或刷新 Editor。专项平台 7/7、真实 RPC 1/1、Workspace Clippy/fmt、前端 check/build、队列 15/15、功能浏览器 22/22 通过，中文“自动查找”最终字面修正另有 2/2 本地化回归；不将不同轮次拼成一个数字。此前完整 Workspace 457/0/8 是初版检查点，本次窄修正未重复整轮。匹配源码的 debug daemon/窗口已恢复，Core ready、隔离配置摘要不变；未提交/推送，Visual Gate 2 PENDING，真实 Tauri picker/点击动作及屏幕阅读器仍待人工验收。证据见 [Hub 验收记录](testing/m7-unity-hub-location-20261031.md)。

2026-10-31 Unity Hub 位置选择：所有者已批准精确合同并完成源码接线。复用 settings.get/update、原权限、Config3 和串行 CAS，增加显式 hubPath 偏好及不持久化的 unityHub 路径状态；Core 校验、store 原子保存、GUI 选择后立即保存，取消不写，旧连接/重复点击/未知提交结果有回归覆盖。完整 Workspace 最终 457 passed / 0 failed / 8 既有 subprocess-helper ignored，含新增真实 RPC 场景；首轮仅测试末尾旧 revision/语言预期遗漏，保留失败记录，修正精确断言及逐次 revision 校验后整轮复跑通过。此前库级 230/230、protocol contract 30/30、活动 Schema 15/15、autosave 10 场景、浏览器 17/17、check/build、Workspace Clippy/fmt/xtask/metadata 通过。依用户明确授权临停并恢复本工作区 dev 与隔离 daemon，均为匹配当前源码的 debug 构建；Core ready、原两个样例和配置摘要保持，已安装 v3 未动。真实 Tauri picker/保存重开及屏幕阅读器证据仍未取得，不把窗口恢复当作桌面验收通过。当前实现及注册表/Hub Editor 发现边界见 [验收记录](testing/m7-unity-hub-location-20261031.md)。现有 WIP 完整保留，未提交/推送，Visual Gate 2 PENDING。

2026-10-31 主题弹窗左右不均定位为公共内容宽 800px、主题表单又固定 752px，造成右侧多 48px。表单改为填满公共内容盒，沿用 Material 默认左右内边距，公共 Dialog 与卡片样式保持。真实几何断言修复前明暗两项均复现 48px 差异，修复后与 footer／键盘／三语组 6/6；check/build、diff check 通过，当前 dev 热更新，中文及明暗截图已核对。未提交／推送，Visual Gate 2 仍 PENDING，见 [弹窗整体留白](testing/m7-theme-dialog-20261030.md)。

2026-10-31 项目网格多余区域背景定位为共用 projects-content 的 surface 底色：列表由内部 viewport 覆盖，网格则透出该底色。直接删除容器背景声明，两种模式外层均透明，保留卡片及表格表面、圆角和滚动。check/build、diff check 通过；首轮操作／持久化 2 项通过，网格多尺寸用例触及原 30 秒时限，保持实现、断言和时限不变单独复核 1/1，通过记录与首轮超时分别保留，不写同轮 3/3。明暗全窗口截图已核对，证据 target/m7-grid-transparent-20261031/，当前 dev 热更新，未提交／推送，Visual Gate 2 仍 PENDING。

2026-10-31 项目视图按钮改为显示当前状态：列表显示列表图标和“列表视图”，网格显示网格图标和“网格视图”；点击仍切换，原持久化及宽度预留保持。对应调用方／中日定位同步；工具栏、视图刷新／重新打开、网格操作、排序及最后修改等回归 29/29，check/build 与 diff check 通过，证据在 target/m7-current-view-label-20261031/。当前 dev 热更新，未提交／推送，Visual Gate 2 仍 PENDING。

2026-10-31 主题内自定义卡片没有库默认内边距，按所有者要求由同一 theme-panel 规则将四周从 16px 统一为 24px。圆角及控件库内间距保持；check/build、明暗／三语几何回归 6/6、diff check 通过，当前 dev 热更新。未提交／推送，Visual Gate 2 仍 PENDING，见 [主题卡片](testing/m7-theme-dialog-20261030.md)。

2026-10-31 主题文案按所有者要求清理：移除源色多余说明；恢复按钮统一 Restore defaults／恢复默认／既定値に戻す；HCT 常驻数字移除，复用 Material Slider 内置 labeled 浮动提示，由库处理焦点／键盘／拖动显隐。仅清理对应 caption／output CSS，三字段草稿及 Save／Cancel 不变。check/build、815 条词条校验、合同解析及 diff check 通过；初轮主题组 24/27 的断言空白／旧译文、后续键盘改变颜色前提及选择行号问题均保留证据，修正后定向复核 4/4，不宣称同轮 27/27。中文静态及操作中浮层的浏览器截图已核对，证据见 [主题文案与数值](testing/m7-theme-dialog-20261030.md)。当前 dev 热更新，未提交／推送，Visual Gate 2 仍 PENDING。

2026-10-30 主题入口按所有者要求改为与新建项目同级的公共 Filled 主按钮，加入调色盘图标；共享按钮间距与 icon slot 复用，既有弹窗及保存行为保留。仅新增现有冻结 Material Symbols 上游的 palette 24px 资产并登记摘要／原路径，无依赖或页面 CSS 覆盖。check/build、31 项图标资产校验、主题展开／三语回归 6/6 和 diff check 通过，明暗浏览器完整窗口截图已目视核对。当前 dev 热更新，未提交／推送，无新增真实桌面签收，Visual Gate 2 仍 PENDING；详见 [主题入口](testing/m7-theme-dialog-20261030.md)。

2026-10-30 界面动画按所有者要求将跟随系统（System）排在第一项，其后为启用动画、关闭动画；Core 与共享 UI 原默认值已为 system，保持已有用户保存值，不重置配置。动画回归 9/9、check/build、合同解析及 diff check 通过，当前 dev 提供新模块。未提交／推送，Visual Gate 2 仍 PENDING；下述 Core 验证已在获准临时停启后补齐，原 GUI 窗口保留。

2026-10-30 按所有者授权完成三项 Core 架构修正：非 nullable 设置 PATCH 显式 null 整次拒绝，仅 sourceColor:null 可清除；application 经窄 ConfigStore port 调用 alcomd-store 文件适配器，权限／CAS／取消锁／候选缓存仍在用例层；候选与 Unity migration 项目权威改显式字段白名单，冻结字节与 hash 保持。Application／Store／VPM 完整 lib 回归 159/159、Protocol 合同 29/29、候选竞态 3/3，以及全 Workspace Clippy、fmt、xtask、metadata、baseline、diff check 通过。最终完整 Workspace 443 passed／0 failed／8 个既有 subprocess 入口 ignored（退出码 0），真实设置 RPC 的非法 null／文件不变／清除回归通过。经所有者明确批准，仅临停已确认的原隔离 daemon 并按原目录恢复为 PID 60832；当前工作树 debug daemon 构建通过，CLI ready／项目读取及摘要核对正常，原 GUI PID 56208 继续响应，配置哈希不变。初轮编译辅助断言、沙箱连接和 AlreadyRunning 证据保留，不混作最终功能结果。修正、失败及限制见 [Core 架构修正](testing/m7-core-architecture-fixes-20261030.md)。原 GUI 窗口与既有 WIP 保留，未提交／推送，Visual Gate 2 仍 PENDING。

2026-10-30 文案统一：任务入口、页面标题、返回及活动提示改为 Tasks／任务／タスク；语言、动画及主题模式的系统选项统一复用 System／跟随系统／システム設定，删除 Task Center、Follow system 和 System language 重复词条。三语路由、动画及主题分段等现有回归 20/20，check/build、816 条词条校验及 diff check 通过。仅文案及调用方同步，无 CSS、图标或公共合同变化；当前 dev 提供新模块，已有 WIP 保留，未提交／推送，Visual Gate 2 仍 PENDING。详见 [三语言当前文案](testing/m7-localization-20261030.md)。

2026-10-30 按所有者要求清理无阅读价值的瞬时请求提示：主题 Save、筛选、分页、Plan／Apply、启动、复制／删除等请求期间保持原动作文字，不新增状态行；禁用／重入锁、aria-busy、错误和真实长任务 Operation 进度保留。首次读取复用现有 160ms 延迟，创建／恢复不再闪加载或误报空列表，候选及版本菜单分页保留已有结果。五项短请求专项 5/5、候选／批量 17/17、最终工具栏 1/1；首轮主题 21/21、分页 3/3、创建／恢复 3/3，check/build、合同解析、diff check 通过。新测试的 Material 空壳文本断言、旧危险色常量和 writer 机器值展示断言已按实际语义修正，各轮失败分别保留，未宣称同轮全套通过。详见 [瞬时状态清理](testing/m7-quiet-feedback-20261030.md)。现有 dev 已提供新模块，GUI／隔离 daemon 保留；已有 WIP 及两项列表长文本行高缺口不变，未提交／推送，无新增桌面或屏幕阅读器签收，Visual Gate 2 仍 PENDING。

2026-10-30 普通设置按所有者要求改为修改即保存，移除整个保存／放弃／已保存栏；主题等已有独立 Save／Cancel 的编辑器仍明确确认。现有 settings.get/update、权限与 CAS 保持，当前 App 连接内串行窄 PATCH，连续改动合并最新意图，跨页面保留队列；实际失败显示 Retry，重读后避免未知提交的重复写，不自动重放到新连接。协调测试六场景、即时保存专项 6/6、主题／动画／三语组 36/36、check/build／xtask／metadata／diff check 通过。扩展组 38/41，旧主题下拉定位与初始焦点等待修正后的复核 2/2；剩余两项列表长文本行高断言仍失败，未放宽标准或在本轮扩改列表，完整 GUI suite 未通过。详情见 [普通设置即时保存](testing/m7-settings-autosave-20261030.md)。当前 dev 热更新，未提交／推送，无新增桌面签收，Visual Gate 2 仍 PENDING。

2026-10-30 公共 SegmentedButtons 自动预留最长标签及选中勾选所需空间，统一禁止文字换行。复用真实字体和 Material 标签 token 的不可见尺寸层参与 CSS intrinsic/grid 计算，不改可见控件字体／内边距，不写死中文标签或页面按钮宽度；父网格尊重子组件的最小内容宽度。修复前中文标签确实断成两行，修复后三语逐项切换均保持单行、边界及组宽高稳定；主题相关六项通过，资源／日志导航与中文截图复核 3/3，check/build 通过。验证中修正了默认英语无变化仍点击 Save 和旧独立 Theme 分区两项过时测试假设，保留失败日志。当前 dev 热更新，已有 WIP 保留，未提交／推送，Visual Gate 2 仍 PENDING。

2026-10-30 主题保存抖动已定位为临时“正在保存主题…”独占行使居中弹窗向上移动 32px。移除该行及其闲置翻译，当前保存按钮始终显示原动作名称，不显示瞬时状态文案，form 标记 aria-busy 并保留忙时禁用；失败仍保留明确错误，成功重试时不先折叠错误区再关闭。复现几何断言修复前失败，修复后保存期间实际 dialog 与 footer 的位置／尺寸保持稳定；保存、防重入、取消、失败及冲突重试共 6/6，check/build、diff check 通过。当前 dev 热更新，未提交／推送，无新增真实桌面签收，Visual Gate 2 仍 PENDING。

2026-10-30 主题弹窗只借鉴参考图的展开信息布局：左侧源色／纵向 HCT，右侧“明亮／跟随系统／暗黑”文字分段（选中带勾）／九方案双列单选。控件使用既有 Material Web 原生外观，撤除仿图的 tonal 胶囊覆盖、HEX 胶囊字段、自定义色带和弹窗底色；保留已确认的全局 28px 卡片、Noto 字体、动态主题与明确 Save／Cancel，不声称这些产品定制均为 MD3 默认值。卡片复用现有低容器表面色，标准滑块完整轨道可见。文字分段／布局／单选及保存取消回归 5/5、前端构建通过；保留真实 dialog 边界与完整 footer 验证，不重跑 Rust Workspace。当前行为和证据见 [主题弹窗](testing/m7-theme-dialog-20261030.md)。当前 Tauri dev 热更新，后端保持；未提交／推送，Visual Gate 2 保持 PENDING。

2026-10-30 动画偏好按所有者要求改为“启用动画／关闭动画／跟随系统”，默认跟随系统。沿用 settings.get/update、原权限与当前未发布 Config3，motion 增加 full，保留 reduced/system 原值；显式 Save settings 生效，Discard／失败不改变已接受偏好。公共 UI 统一系统动态偏好、页面及 Material Shadow DOM、菜单／弹窗 quick 与滚动条淡出；原切页动画继续撤除。动画＋主题浏览器 29/29、设置后续 2/2、Application 定向 2/2、Protocol 合同 28/28、真实 RPC 1/1、renderer 3/3 通过；check/build、Workspace Clippy、fmt、xtask、metadata／baseline check 通过，未重跑完整 Rust Workspace。首次 strict-null 与旧排版断言失败及最终修复如实保留。必要重建 debug daemon／CLI，使用同一隔离样例恢复 Tauri dev。详情与当前构建证据见 [动画三态](testing/m7-motion-20261030.md)。已有 WIP 保留，未提交／推送，无新桌面／屏幕阅读器签收，Visual Gate 2 仍 PENDING。

最后更新：2026-10-30

## 当前阶段

2026-10-30 主题交互：按所有者最新修正，外观标题栏右侧保留“主题”按钮，语言选择器有明确可见标签并右对齐，完整主题控件进入 Material 弹窗；实时预览，Save 才一次持久化三字段，Cancel／Escape／关闭恢复已接受主题，移除自动保存与后台队列。保留默认 #6CB6FF／Vibrant／系统模式、九方案、HEX／拾色器／HCT、连接 revision 门控及冲突三方合并；保存忙状态阻止重复提交和关闭，其他设置草稿独立。完整弹窗浏览器检查点 30/30；外观排版后续相关回归 6/6，前端 build（含 check）和 diff check 通过，已目视核对中文暗色浏览器样例；此前 xtask／metadata 检查保留；本轮无 Rust／RPC／Config／依赖变更，后端沿用已验证 Config 3（此前完整 Workspace 432 passed／0 failed／8 既有 subprocess ignored），不重复记为本轮执行。当前隔离 Tauri dev 接收前端 HMR，默认 1400×800；原 daemon／原生构建保持，真实桌面和屏幕阅读器新证据未取得。配色与后端记录见 [主题逻辑](testing/m7-theme-controls-20261030.md)，最新行为与证据见 [主题弹窗](testing/m7-theme-dialog-20261030.md)。保留既有 WIP，未提交／推送，Visual Gate 2 仍 PENDING。

2026-10-30 项目观察时间移除：按所有者明确批准，当前 `ProjectObservation`／`ProjectSnapshot`／GUI 不再定义或输出项目 `observedAtMs`；State v15→v16 物理删除项目列并定点清理持久 JSON 和永久幂等响应。删除时间清零比较及项目采集时钟参数；最后修改时间保留。旧未 Apply 的 Unity migration Plan 明确过期并同步重放响应，保留原哈希及已执行恢复证据；旧候选 cursor 走既有 stale 处理。Unity 安装、writer 检查及迁移专有重检时间保留，没有额外兼容别名或新 method/capability/permission。最终完整 Workspace 427 passed／0 failed／8 个既有 subprocess 入口 ignored（退出码 0），浏览器专项 10/10，check/build、最终 Clippy、fmt、xtask、metadata、baseline freeze 和合同解析通过。debug daemon／CLI 已重建并恢复原隔离样例；真实只读 RPC 无旧字段，隔离库只读验证 State 16、物理列与 JSON key 已删除，两个旧项目及 revision 保留。最新 Tauri dev PID 57152 响应正常，匹配 daemon PID 50364，构建均为 dev/debug；未取得人工视觉新证据。合同及本轮证据见 [项目观察时间移除](testing/m7-remove-project-observed-20261030.md)。已有最后修改时间／翻译／字体成果保留，未提交／推送，HEAD 仍 `7d938cf`，Visual Gate 2 与屏幕阅读器仍 PENDING。

2026-10-30 项目最后修改时间：按所有者明确指令，Core 独立维护 `ProjectRecord.last_modified_at_ms`，现有 RPC v1 兼容增加可选 `ProjectSnapshot.lastModifiedAtMs`，State v14→v15 仅增加 nullable 项目时间列，无新 method／capability／permission。首次注册及新创建／复制／恢复的目标初始化时间；成功包 Apply、Unity 迁移项目提交、通过前置校验后首次接受的 Unity 启动更新时间（spawn 失败仍保留接受时间）。普通刷新、收藏／参数、Plan／取消、幂等重放／状态轮询不更新时间；旧记录无可靠历史值则未知，不以其他时间补造。列表、网格和排序均使用新字段，收藏优先／稳定决胜／未知在后，启动结束后重读 Core。时间不独立推进项目 revision，候选及 migration authority hash 排除新元数据；resolver／Plan／Apply 其他语义不变。

最终完整 Rust Workspace 命令退出码 0：425 项通过，8 个既有 subprocess fixture 入口默认 ignored；fmt、全 Workspace Clippy、xtask、metadata、baseline freeze、前端 check/build 和 diff check 通过。浏览器综合首轮 32/33，新修改时间专项 10/10；失败为网格 More 菜单 Escape 焦点恢复，保持代码／断言不变单独复测 1/1，通过记录与首轮失败均保留，不记作同轮 33/33 或真实桌面验收。已构建本轮 debug/dev daemon、CLI 并恢复隔离 Tauri dev：daemon PID 45028，GUI PID 43932，窗口响应正常，使用原 `target/m7-icon-acceptance-20260906/data`；旧样例字段未知，符合无回填合同。完整规则、失败修正、构建摘要及证据见 [项目最后修改时间](testing/m7-project-last-modified-20261030.md)。先前三语言／Noto WIP 保留，本轮未提交／推送，HEAD 仍 `7d938cf`。本轮全 Workspace 结果不沿用历史环境阻塞结论；Visual Gate 2 和屏幕阅读器验收仍 PENDING，不关闭 H2-A。

2026-10-30 字体：按所有者要求，正文及 Material 字体统一引用公共主题的本机 Noto Sans 栈，代码／等宽文本引用 Noto Sans Mono 栈；中文和日文分别使用 SC／JP 的地区字体族与桌面 CJK 名称，缺失时自然回退系统 sans-serif／monospace。没有随包字体、@font-face、在线加载或新依赖，移除原 Inter／微软雅黑／Cascadia 的重复指定；既有字号、字重、行高及布局不改。check/build、diff check 和 5 项现有浏览器回归通过（退出码 0），覆盖三语言显示、共享 Material 控件及按钮留白／宽度；产物无 woff/woff2/ttf/otf 文件，保留既有大 chunk 警告。证据在 `target/m7-noto-fonts-20261030/`，现有 dev 已 HMR，未新增真实桌面字体／屏幕阅读器验收。保留先前三语言 WIP；字体增量未提交／推送，Visual Gate 2 仍 PENDING。

2026-10-30 三语言：按所有者明确范围先完成现有 `en-US`／`zh-CN`／`ja-JP`，既有术语与对应文案对照冻结 v3，v4 新增文案另补。官方 GUI 的侧栏、页面、表头、控件、弹窗、候选不可用原因、Review、状态／错误和无障碍名称接入统一 locale context 与 817 条双语消息；时间跟随 locale 且保留 UTC 边界，设置成功保存后才切换，扩展继续收到现有 locale。项目／包／来源名称、路径、版本、机器标识和扩展提供内容保持原值。公共词条归一，check 新增缺项／冲突／占位符与显式 JSX 文案检查，无新增依赖或 RPC/Config/State/权限变化，未改 Rust、CSS、图标或 Plan/Apply。

最终受影响组 17/17 PASS（退出码 0），覆盖中日设置保存、跨页、现有九路由及切回英语、创建／Review 取消、外部文本保留、语言匹配与全部词条插值、日志及创建／恢复。另候选／批量 17 项和 Projects preferences 18 项通过。保留此前整合 40/44、44/48 失败记录，不写成全套同轮通过；旧菜单入口／控件定位与日期、诊断本地化断言已对应修正，英语状态显示保持既有格式。check/build 与 diff check 通过，保留大 chunk 警告。现有 1420 dev 已热更新，Tauri／隔离 daemon 保留；未取得新的真实桌面／屏幕阅读器验收，Visual Gate 2 仍 PENDING。本轮未提交／推送，修复前 HEAD `7d938cf`。详细边界、失败原因及截图位置见 [三语言验证](testing/m7-localization-20261030.md)。

2026-10-30 Open Unity 行为修正：Projects 列表／网格此前将精确安装数量不为 1 的点击直接导航到 Project Unity，未在原处完成启动选择。现在列表、网格及 Project workspace 共用 `useProjectUnityLaunch`，点击时重新读取现有 launch-options：唯一精确匹配直接调用既有 launch；多个匹配原地选择本次安装；没有匹配原地提示，仅显式 Migrate Project 操作进入迁移页。选择项显示版本、架构及路径，不保存默认安装；重复点击保护、取消／Escape 焦点恢复和明确错误状态由共享流程持有。对照冻结 v3 的点击启动及多安装选择语义，保留已批准的 v4 exact-version／one-shot 合同，不引入 v3 缓存 Editor 或跨版本回退。未改 Rust、RPC、权限、配置、迁移或 Plan/Apply。

本次 frontend check/build 与 diff check 通过，保留既有大 chunk 警告。最终受影响组 7/8，通过列表／网格 0/1/many、重复点击、选择重置、焦点恢复、错误、workspace 和迁移后重读；唯一失败为既有网格 More 菜单 Escape 焦点检查，保持实现与断言不变单独复测 1/1，通过记录见 `target/m7-open-unity-in-place-20261030/grid-focus-check/.last-run.json`。不记作全套同轮通过。初轮测试 ID 误写及浏览器 target crash、取消焦点缺陷的失败证据保留在同目录；取消焦点缺陷已修复。fixture 启动状态由不属于真实合同的 `spawned` 校正为 `opening`，RPC 参数与调用次数另外断言，不以提示文案冒充真实启动。

已重新运行 `npm run gui:dev`，当前真实 Tauri PID 31300 响应正常，profile 为 dev（unoptimized + debuginfo）；开发前端来自当前工作区，GUI 路径 `target/debug/alcomd-gui.exe`，SHA-256 `5BD568B882E60B8016DEE227A0614B16CFB72A6EC2BE16DD9F5249AA78BBB93B`。匹配的本工作区 daemon PID 42628 使用原获准的 `target/m7-icon-acceptance-20260906/data`，未改 Rust，复用 daemon 摘要 `982D77AAA55B85ED030760290257DCBC21E80F03661C3A492BC1D5F3D5CFE95C`；CLI 只读确认 ready、隔离项目 revision 1 和 Unity 2022.3.22f1。其安装列表及精确匹配集合均为空，没有真实 Unity 启动证据；未注册安装或操作真实工程。启动日志与最终 check/build 在 `target/m7-open-unity-dev-20261030/`。本次修改已获所有者授权整理为本地提交，未推送，修复前基线为 `b03aa62`；真实交互及屏幕阅读器仍待验收，Visual Gate 2 保持 PENDING。

2026-10-30 公共表头字体补充：普通 DesktopList 表头原为 12px／normal 行高，可排序表头则使用 Material 文字按钮的 14px／20px；连同 Projects 的非排序 Actions 列也存在混用。现在字体族、字号、字重和行高由 DesktopList 同一组 header token 持有，普通表头和排序按钮显式引用，默认沿用 MD3 Label Large 的 14px／20px／500；通用按钮字号、是否显示箭头及升降序状态均不改变表头字体。保留排序高亮、箭头、表头内边距、圆角、粘性列和复选框原有触控高度，没有页面单独覆盖。

新增回归在修复前复现普通表头 12px 与排序标签 14px 不一致；主题回归进一步复现通用文字按钮字号改为 28px 时排序表头也被带大，均退出码 1。中间四项回归 2/4：两个逐页用例误将包表格复选框原有 48px 表头要求为 40px；修正测试为同行高度一致且至少 40px，保留字体等值要求，没有为测试缩小控件触控区。最终专项 4/4 PASS（退出码 0），覆盖 12 个列表入口在 1280／1440px 的字体、长内容／控件／圆角边界，仓库浏览弹窗，以及通用按钮 28px 时普通／排序表头仍为 14px 和升降序切换。既有 Projects 排序／工具栏回归此前亦通过。check/build、diff check 通过，保留大 chunk 警告；证据在 target/m7-list-header-consistency-20261030/，仓库弹窗完整窗口截图已目视核对。1420 dev 已收到 HMR，各轮本任务 4173 Vite 测试服务已清理；没有原生桌面或屏幕阅读器新证据，未重建原生程序、操作真实工程或重复 Rust Workspace。所有者已授权提交此增量，并将其与前五笔本地提交一起正常推送；该授权不代表视觉验收通过，Visual Gate 2 仍 PENDING。

2026-10-27：Dependabot #22–27 的本地安全修复仅涉及开发／构建依赖。`package-lock.json` 将 `source-map-js 1.2.1 → 1.2.2`、`vite 7.3.6 → 7.3.7`，只更新 version/resolved/integrity。现有依赖范围允许这些补丁；Vite 同步修复内嵌 launch-editor，不新增依赖、override、安装脚本许可、配置或产品代码。对应 GHSA 为 `68fv-2mgg-jv7q`、`698v-4r3x-3vcv`、`587h-7wmj-h5pm`、`r825-w8mm-5pxw`、`9jrq-w75r-8gcw`、`rq7h-c2jc-7f22`。Node engines、依赖集合、MIT／BSD-3-Clause 许可保持。未改变 RPC、权限、State 或 Plan/Apply。

`npm ci --no-audit --no-fund`、`npm run check`、`npm run build`、diff check 通过；`npm audit --json` 为 0 漏洞，实际依赖树确认修复版本且不进入 omit-dev 图（空集合的 npm ls 返回 1）。source-map 临时回归 12/12：升级前确认 PostCSS 对象／字符串／两种 inline 编码接受越界偏移、深层 SourceNode lookup 在 2 秒预算下超时；升级后拒绝直接／累计越界及非法类型，正常 CSS 和映射控制保留。Vite 临时回归 11/11：公共 HTML 转换及 import-analysis 的升级前缺陷已不能复现，正常 HTML 保留；编辑器参数在禁止 fs/child_process 的 VM 中验证百分号／换行、Windows 路径规范化与模拟 WSL flag 前缀，未启动真实编辑器或 shell。既有 GUI 冒烟 3/3（退出码 0），覆盖键盘、22 个路由与 Material 控件。临时样例／输出由安全技能管理，不纳入产品。构建保留大 chunk 警告；npm ci 提示现有 esbuild postinstall 未获本机 allowScripts 许可，未改许可，构建正常。WSL／非 Windows 实机完整 exploit 未验证；未操作真实工程、重建桌面程序或重复 Rust Workspace。所有者已授权分批提交和推送；远端告警关闭以默认分支重新扫描结果为准，Visual Gate 2 保持 PENDING。

2026-10-22：公共列表由 `DesktopList` 统一持有列定义、表头、行、滚动和排序。默认列禁止文字软换行及 flex 操作控件折行，包含嵌套内容；主副文本等显式分行保留。`UtilityTable` 不再隐式放开所有列换行，全部生产调用方使用组件默认值；标准按钮宽度受单元格限制，长标签沿用 Material 自身省略行为。已审计 Projects、Project Packages、Repositories、Repository Packages（独立页和浏览弹窗）、User Packages、Templates、Unity、Settings 内 Unity installations、Backups、Task Center、Activity 和 Diagnostics。包页保留同列主操作及逐行更多菜单、单一工具栏内 Update All；Activity / Diagnostics 保留固定列布局、完整 UTC 时间和列内摘要截断。

圆角按所有者 2026-10-30 的确认统一：主画布、内容容器、卡片、操作／筛选条、反馈和 Portable UI 卡片共用 `--alcomd-shape-container`，映射 Material extra-large 的 28px，移除原 canvas/card 两档 token 及其引用。Settings 保存条和分组、Projects 网格卡片／次工具栏、Logs 筛选条、Extensions、风险／操作反馈、空／错误状态及自有筛选弹层均遵循此值；Material 按钮、输入框、导航、弹窗及滚动条保留各自控件形状。纯布局包装、透明分隔信息行及分段面板内部接缝保持平直，未改成卡片。公共列表滚动容器继续负责内容裁切；列表项仍无双色条纹／分割线、统一底色，行间为真实 2px 空隙，行内连接处为 8px 微圆角，首项上圆角及尾项下圆角为 28px，表头视为首个列表项。粘性操作列沿用同一行底色及 hover 状态。

28px 增量的前端 check/build 与 diff check 通过；调整前先核对共享 Material 和多个业务模态对话框，2/2 通过，原有实际绘制表面均为 28px，没有另设不同的对话框圆角。最终浏览器专项 **7/7 PASS，退出码 0**，覆盖 1280px／1440px 全部列表几何、亮／暗主题的画布／设置保存条／分组／Projects／Extensions／Logs／About／Portable UI 卡片、自有筛选浮层 28px、模态对话框 28px 及取消／焦点恢复，并更新完整窗口 fixture 截图。项目行更多菜单的 12px 属于现有菜单控件规则，不当作卡片或模态对话框统一；其他 Material 控件形状同样保留。结果在 `target/m7-shapes28-20261030/`，截图沿用 `target/m7-v3-recheck-20260919/browser/`。此前 20px 候选的失败诊断保留在 `target/m7-shapes-20261030/`，不作为当前圆角规范。正在运行的 `npm run gui:dev` 已收到 HMR，真实窗口 PID 96964 响应正常；热更新和 browser fixture 不代表人工视觉或屏幕阅读器验收通过。未改依赖、Rust、图标、布局或进行窄窗口适配，未重新构建桌面产物，Visual Gate 2 保持 PENDING。

项目所有者认可列表卡片式样式；该认可仅覆盖列表样式，不代表完整 H2-A 交互或 Visual Gate 2 通过。28px 统一已整理为独立提交。

2026-10-30 选择器样式按所有者要求统一：公共 React `Select` 固定使用 Material Filled Select，共享主题配置胶囊全圆角、填充底色及无底部指示线。选择框只显示一行选中值，不绘制小字标签，原 label 在没有显式 aria-label 时提供无障碍名称；shared field 上下各 8px，使选择框正文高 40px，辅助说明独立保留。所有业务调用及 Portable UI 保持原值、disabled、required 和事件；Projects 保留外置排序标签，重复形状／底色／留白规则收回公共主题；各栏宽度与字号按下方公共筛选栏规则统一。原描边 Select 的宽度／字体规则及浏览器定位同步更新，不改变下拉菜单形状、滚动、选择逻辑、RPC 或依赖。`npm run check`、`npm run build` 和 diff check 通过；最终无小字版本的浏览器专项 7/7 PASS，退出码 0，覆盖实际背景胶囊圆角、小字隐藏、40px 高度、combobox 名称、设置保存、Portable UI、Projects 排序、Settings 几何、完整窗口截图和 Select／dialog 自绘滚动条。结果在 `target/m7-filled-select-20261030/no-label/`，完整窗口 fixture 截图沿用既有目录；已目视核对 Logs 的无小字单行效果。测试完成后已单独清理确认归属的 4173 测试 Vite，1420 dev 与 Tauri 窗口保持。前端已 HMR，真实 dev GUI PID 96964 响应正常；没有重新构建桌面程序或执行原生交互／屏幕阅读器验收，Visual Gate 2 仍为 PENDING。选择器样式已整理为独立提交。

2026-10-30 公共筛选栏：Projects 网格排序栏、Activity 和 Diagnostics 共用 `@alcomd/ui` 的 `.alcomd-filter-bar`，移除各页重复容器、选择器和结果文字规则。统一 surface-container 背景、28px 外圆角、56px 最小高度、上下 8px／左右 12px 留白、8px 间距、14px 文字；选择器共用 200px 宽／40px 高，结果说明靠右且同字号，复选框标签不再额外撑高。显式覆盖 Material Select 的 210px 默认最小宽度，使指定宽度真正生效。保留原排序、筛选、详情勾选、结果范围说明和无障碍名称；包筛选弹层仍为原有浮层，不改变其结构。check/build 和 diff check 通过；首轮专项 5/6，通过 1280px／1440px 全列表几何、Projects、Logs 和完整窗口截图，另一个用例发现上述默认最小宽度问题；公共规则修正后受影响专项 3/3 PASS（退出码 0），直接比较亮／暗主题下三个栏的实际背景、圆角、尺寸、间距及字体，并复核 Projects 交互和完整窗口截图。证据在 `target/m7-filter-bars-20261030/`，已目视核对 Activity 全窗口。测试服务结束后仅清理确认归属的 4173 Vite，真实 dev GUI PID 96964 响应正常，1420 已收到 HMR。公共筛选栏已整理为独立提交；无新增依赖或业务合同变更，Visual Gate 2 仍为 PENDING。

2026-10-30 Projects 视图模式持久化：表格／网格不再仅保留于组件内存，进入页面同步读取 `localStorage` 的 `alcomd.gui.projects.view`，点击切换时保存 `list|grid`。这是 AGENTS 允许的可丢弃 GUI 视图状态，不存项目、权威设置或业务快照；缺失／未知值默认表格，存储读取或写入不可用时仍可在当前页面正常切换，不增加依赖、RPC 或 State。既有圆角／筛选栏回归不再假定重新进入 Projects 必然回到表格，而是通过现有 UI 确认网格。前端 check/build、diff check 通过；首轮专项 4/6，既有 Projects 工具栏、Favorite 两项及跨页面形状／筛选栏通过，新增两项直接 reload 被 fixture 的 history URL 替换导向未注入 client 的真实入口而失败。仅修正测试刷新入口、保持真实 reload 后，两项持久化专项 2/2 PASS（退出码 0），覆盖页面导航返回、刷新、新页面重开、表格／网格双向保存、无效值及不可用存储。证据在 `target/m7-project-view-20261030/`；没有以浏览器 fixture 冒充原生应用重启证据。真实 dev 已收到 HMR，GUI PID 96964 响应正常；未重新构建桌面产物，Visual Gate 2 保持 PENDING。

2026-10-30 Projects 网格候选按所有者参考图继续修正，Visual Gate 2 仍待验收：所有者已明确问题是操作缺失或按钮无法使用；前一轮大块卡片和纵向主操作区不作为认可布局。当前采用紧凑卡片：16px 名称、14px 路径、类型图标／版本同一行、Added／Last observed 同一行，底部 Open Unity／Manage／Backups 三按钮同排，更多与收藏位于右上角。沿用已确认的 28px 圆角、主题色和已有图标；卡片主按钮采用参考图的文字形式，表格中的播放图标保持。没有按图片引入新背景色、翻译文案或时间语义；observed 不冒充 last modified。名称、路径和日期有界并保留完整 title，卡片按内容高度排列。网格通过 ProjectRowActions 的 card 上下文复用表格同一套权限、exact-Unity、错误、复制 Plan/Apply 和移除确认；已补回原来缺失的 Open Unity、目录打开、Copy Project 和 Remove Project。没有新增 RPC、业务判断、依赖或操作旁路，也不为显示卡片创建 Plan／Operation。冻结 v3 仅用于功能审计，未复制源码。

参考图版本的 check/build、diff check 与最终专项 8/8 PASS（退出码 0），证据在 target/m7-project-grid-reference-20261030/。覆盖默认桌面 1180×760、1280px 两列和 1440px 三列、稀疏／七卡片、亮暗主题、单行操作、长文本边界、滚动、Manage 键盘激活与 Backups 跳转、启动、目录打开、Copy Review → Back → Cancel、移除取消、菜单 Escape 焦点、权限不足、缺 Editor 导向既有 Unity 页面、视图持久化及 Favorite。所有动作均为 browser fixture，没有启动真实 Unity、复制或删除真实工程；1180px 证据只针对卡片操作，不扩展为全窗口工具栏适配验收。此前几何候选 7/7 与操作补全的首轮 8/10／后续单项结果保留为历史证据，不能代替当前参考图结果；基线拉伸截图仍在 target/m7-project-grid-20261030/baseline/。当前完整窗口截图在参考图结果目录，已目视对照 1440px 和默认桌面布局。computer-use 捕获此前返回 Computer Use app approval timed out，原生桌面证据未取得，未重复申请或绕过授权，不当作功能失败或通过。当前 1420 dev 已收到 HMR，GUI PID 3360 响应正常，沿用既有隔离 daemon／样例；未重新构建桌面程序，未进行窄窗口适配；所有者已授权本轮本地提交，未推送，Visual Gate 2 继续等待人工验收。

2026-10-30 Projects 新增入口按所有者参考恢复为分裂按钮：主按钮 Create project 直接打开既有创建流程，右侧独立下拉按钮包含 Add existing project 和 Restore from backup；不再把注册、恢复、创建分别铺在工具栏。通用 SplitButton 独立为与 DesktopList 同层的 SplitButton.tsx，形状、间隙和紧凑 40px 菜单由 @alcomd/ui 主题持有，使用已有 20px 下拉图标及 Material Button／Menu／MenuItem；未新增依赖。主按钮和菜单项分别按原 capability 禁用，下拉仍能展示不可用项的原因。已有目录选择／注册确认、Template／Backup Plan Apply Operation 保持；菜单关闭后才执行选中动作，取消添加／恢复回到下拉按钮，取消创建回到主按钮，支持 ArrowDown 展开与 Escape 关闭并恢复焦点。不改变权限、RPC、Plan/Apply、已确认图标和其他页面布局，冻结 v3 只读审计确认相同分组语义，未复制源码。

该增量 check/build、diff check 通过；首批业务／键盘／权限回归 6/6，通过注册确认、焦点、错误／空集合、200% 既有布局和表格工具栏；第二批 3/6，列表 1280／1440 及网格通过，三项遇到 Material Shadow DOM slotted 文本读取和 Filled Select 的相同 aria-label 在 combobox／隐藏菜单重复定位。只改测试为 accessible name／明确 combobox，复核 3/3，正常 Create 和 Restore 的既有 Plan Apply Operation 流程保持；最后紧凑菜单行高与完整窗口专项 1/1。没有把这些批次合并写成完整 GUI suite 通过。证据在 target/m7-project-split-button-20261030/，最新截图为 compact 下对应用例的 projects-creation-menu.png，已目视核对 1180×760 的完整窗口、连续胶囊形和两个菜单项。1420 dev 已收到 HMR，GUI PID 3360 响应正常；没有重新申请 computer-use 或把 browser fixture 当作原生桌面验收，没有操作真实工程。已本地提交，未推送，Visual Gate 2 保持 PENDING。

2026-10-30 组合按钮逐页补充：只读检索冻结 v3 的全部下拉触发器及拼接容器，确认四组：Projects 创建、Templates 创建、Repositories 添加、Project management 的 Open Unity。当前 Projects 保留上一增量；Templates 改为 Create template 主按钮＋Import template 下拉，空目录同样能创建；项目工作区改为 Open Unity＋维护菜单，包含启动参数、目录、复制、备份和移除，备份入口不再单独铺在工具栏。三组共用 SplitButton／主题，模板派生与启动参数编辑从现有 v4 实现抽取共用弹窗，沿用原 RPC、revision、Plan Review／Apply／Operation、错误与权限条件；表格／网格卡片入口保持。仓库 import/export 在现有公共合同中缺失，已与既有审计互证，本轮保留可用 Add repository，不伪造禁用菜单、不擅自增加 RPC；v3 条件性遗留 Unity path 清除不引回已拒绝的缓存 Editor 模型。详见 [逐页组合按钮审计](testing/m7-v3-recheck-implementation-20260919.md#2026-10-30-composite-button-audit)。

该增量 browser 首轮 13/17：模板导入取消焦点未返回下拉按钮是真实缺陷，已修正；两项量到了 Material 内部 button 而非拼接 host，一项旧菜单定位因新增菜单不再唯一，均修正为精确定位，保留原验收要求。复验 6/6（含 12 个共享列表入口在 1280／1440px 的边界），稳定展开截图与流程复核 2/2，退出码均 0；首轮退出码 1。覆盖创建／导入草稿、菜单 ArrowDown／Escape、焦点返回、空集合／权限、真实既有模板 Plan Review 和显式 Apply fixture、Unity 0/1/多安装分支、启动参数保存／清除／取消、备份导航及永久删除 Review。证据在 target/m7-composite-buttons-20261030/，最终完整窗口截图在 stable/ 并已目视核对；不合并宣称完整 GUI suite 全部通过。最终 check/build、diff check 通过，保留既有大 chunk 警告；没有改 Rust、依赖或公共合同，没有操作真实工程，未重复 Rust Workspace。原 1420 dev 已收到 HMR；未重新构建桌面产物，未重复申请或绕过 computer-use，屏幕阅读器证据未取得。整理起点为 b41afe2，已有及本次成果已分别提交，未推送，Visual Gate 2 保持 PENDING。

组合按钮微圆角补充：已核对安装的 Material Web 2.5.0 没有现成 Split Button；三处统一使用独立 SplitButton.tsx，不在 Material.tsx 保留旧导出别名。两半内侧复用列表项 8px 圆角 token，之间复用列表 2px 间隙 token，移除分隔边线；外侧使用按钮高度的一半（当前 20px），保留胶囊外形。此前 target/m7-split-button-shape-20261030/ 的 3/3 只证明 computed style 和交互，不能证明实际微圆角绘制：所有者指出内侧仍像直角，核对发现 9999px 外侧圆角触发 CSS 统一缩放，8px 内侧被压到接近零；host 和背景层的 computed style 仍报告 8px。新增截图像素检查在修复前确实失败（退出码 1，角落仍为按钮紫色），外侧改为高度相关圆角后，两个内侧切角像素与间隙背景一致，按钮本体保持填色。最终 check/build、diff check 和三处专项 3/3 PASS（退出码 0），保留菜单／弹窗／焦点流程；修复前后局部与完整窗口 browser 截图在 target/m7-split-painted-corners-20261030/，已目视核对实际绘制。修正仅在公共主题，调用方无页面补丁。1420 dev 已收到 HMR，已清理本轮 4173 测试服务；未重建原生程序，未取得新增桌面或屏幕阅读器证据，已本地提交，未推送，Visual Gate 2 仍 PENDING。

2026-10-30 按所有者要求统一同类型按钮 MD3 内边距：共享 @alcomd/ui theme 中，有底色／描边按钮纯文字左右 24px，图标侧 16px／对侧 24px；文字按钮纯文字左右 12px，图标侧 12px／对侧 16px。移除表头排序和网格卡片中的间距覆盖，Material 边界检查阻止 GUI CSS／TSX 重新定义这类 token；分裂按钮固定尺寸箭头区仍由公共组件持有，主按钮使用普通填充按钮规则。保留 widthLabels 的状态宽度稳定及可访问名称，不再要求不同 variant 等宽。网格最小卡片宽度由 320px 调为 376px，容纳采用标准内边距的三个完整操作，按钮同排、图标、外形及顺序不变；1440px 下两列，1600px 下三列。未通过缩短标签、挤压按钮内边距或减少操作维持旧列数，未增加窄窗口适配。现行规则和历史结果见 [公共按钮间距与状态宽度](testing/m7-button-width-20260919.md)。

该增量首轮 browser 7/8（退出码 1），1440px 网格暴露标准按钮超出卡片内边界；修正最小卡片宽度后几何专项 1/1，覆盖 1180／1280／1440／1600px、亮暗主题、长内容、滚动、完整操作及统一 24px。启动／目录／复制 Review／取消／移除以及权限／缺 Editor 的网格专项 2/2，退出码均 0。首轮其余七项通过按钮实际内边距、图标分支、状态文案稳定、三处分裂按钮的绘制／交互、12 个列表入口在 1280／1440px 的边界及 utility 弹窗草稿／焦点，不汇总宣称完整 suite 全通过。最终 check/build 和 diff check 通过，保留既有大 chunk 警告；证据在 target/m7-md3-button-padding-20261030/，最终网格完整窗口已目视核对。1420 dev 已收到 HMR，清理了各轮本任务 4173 测试服务；未改 Rust／依赖／公共合同，未操作真实工程或重建原生程序，未新增桌面／屏幕阅读器证据。现有 WIP 保留，已本地提交，未推送，Visual Gate 2 继续 PENDING。

本轮已按所有者要求分次本地提交：aa88d01 为独立 SplitButton 和实际微圆角主题，acd1061 为三处组合入口／共享弹窗及浏览器回归，b3542d0 为 MD3 同类型间距／页面覆盖移除／网格最小宽度／边界检查及对应回归说明；本次审计及状态记录单独提交。拆分未改动已验证的产品／测试文件内容，逐笔暂存差异检查通过；未为提交重复运行已经通过的测试，没有推送、改写历史或提升 Visual Gate 2。

卡片式列表完整 GUI browser suite（单 worker）首轮 96/97：一项在 `browserContext.newPage` 阶段 `Target crashed`，未进入用例；代码及断言不变单独复跑 1/1 通过。最终首尾圆角增量的逐页/截图专项 3/3、GUI 与共享 UI check、frontend build、debug Tauri no-bundle、diff check 通过。12 个列表入口在 1280px / 1440px 下统一验证长内容、按钮边界、真实行间空隙、无条纹/边线、首尾及内部圆角、圆角命中裁切，另有非列表层级回归。该列表样式候选的 debug GUI 摘要为 `A8EBCFC7C90ECD2C9615D6B1108C0B5EF37435048B08E309E926B05C746C0222`，其历史验证结果不因后续标题修正而扩大。

当前标题规则：一级页面标题对应侧栏页面名称，由 `navigation-labels.ts` 同时供侧栏及页面引用。Resources 的 Repositories / User Packages / Templates 分段均显示 Resources；Logs 的 Activity / Diagnostics 分段均显示 Logs；Settings 所属 Unity installations 页面显示 Settings。分段按钮和表格继续表达具体内容，详情页及项目工作区保留上下文标题。所有官方页面通过公共 `PageHeader` 渲染 `PageTitle`，统一采用 Projects 的标题栏规则：最小高度 56px、左右内边距 8px、控件间距 8px、垂直居中；标题及副标题外边距为左右各 4px，不收缩、不软换行，字号 24px / 字重 500 / 行高 1.2 / primary 色。Projects 标题位置及工具栏操作顺序保持，普通页面标题与其对齐；详情页通过 leading / subtitle 插槽保留返回按钮、项目路径和操作区，路径与长项目名在可用宽度内省略。Extension UI 去除额外画布内边距，标题栏使用同一规则，扩展内容归属及身份说明保留在标题栏下方。删除各页重复的标题栏几何规则及已移除包装层的样式，保留既有路由焦点 ID / ref、导航、操作及列表规则，不修改业务合同。

公共标题栏增量的 `npm run check`、`npm run build` 与 diff check 通过；accessibility / utility / Material foundation / extension workspace 浏览器回归 **41/41 PASS，退出码 0**。22 个路由除字体外直接比较标题边距、收缩及换行规则、标题栏相对主画布的位置与尺寸、padding/gap/垂直对齐，侧栏一级页面另与 Projects 比较标题实际坐标；保留详情页 Back / subtitle 语义、路由焦点、弹窗取消、扩展生命周期及 1280px / 1440px 列表回归。完整窗口 fixture 截图在 `target/m7-v3-recheck-20260919/browser/`，已目视核对 Settings、Extension UI 和 Project Packages。本轮沿用已安装 Chromium，测试完成后清理已核实归属的残留 Vite 服务，收尾产生服务终止日志但 Playwright 退出码为 0。所有者要求打开桌面检查后，`npm run gui:build -- --debug --no-bundle` 成功，产物为 dev profile（unoptimized + debuginfo），`target/debug/alcomd-gui.exe` SHA-256 为 `6585481E4D12F4842B9EAB468B61241F790DC4F4E1709E4BEACC1EF89CCBC6E5`，已启动 PID 199572（主窗口响应正常）。复用已核实 executable/data-dir 的隔离 daemon PID 194596，摘要与既有记录一致，CLI 只读状态为 ready / 4.0.0-alpha.0；未重建 daemon/CLI，未操作真实工程。所有者已确认本轮公共标题栏及对称 4px 边距的局部效果，并授权分批提交和推送；该确认不代表完整 H2-A 或 Visual Gate 2 通过，屏幕阅读器证据仍未取得，Visual Gate 2 保持 PENDING。后续标题及副标题左右各 4px 的窄增量通过前端 check/build、debug Tauri no-bundle 与 diff check，未重复浏览器 suite；上述 41/41 是公共标题栏统一时的证据。此前标题增量的 46/46、debug Tauri no-bundle 为历史证据，仅比较文字计算样式与可见性，没有验证标题栏几何，不能代替本轮统一结果；构建保留既有大 chunk 与增量缓存 hard-link 改用 copy 的警告。

标题文字专项 11/11（退出码 0）：在既有路由回归中直接比较一级页主标题与侧栏当前选中项的可见文字，并验证资源/日志分段切换、模板/日志返回、焦点及瞬时灰层回归。专项首轮因测试在 Material host 上定位委托的 `aria-current` 而失败；改为页面实际绘制选中状态的 `[selected]` 定位后通过，未修改产品选中行为或验收要求。前述 46/46 是标题样式与可见性增量的历史证据，不代表当时已验证正确的标题语义。本轮 `npm run check`、`npm run build`、debug Tauri no-bundle 与 diff check 通过，完整窗口 fixture 截图沿用上述目录；真实桌面文字仍待所有者复看。

切页瞬时灰层修正：App 的能力快照生命周期不再依赖 `route.kind`，普通切页复用当前连接已确认的能力；首次获取、client 更换及明确重连仍清空并重新获取，缺失能力仍禁用。修复前在既有 harness 中将状态响应延迟 120ms，鼠标/键盘切页记录到 3 次能力检查加载层挂载，并发现标题焦点丢失。修复后相同瞬时挂载监测为零，目标标题焦点恢复；没有通过隐藏灰色背景、关闭动画或保留其他页面数据掩盖状态问题。修复前 trace/context 在 `target/m7-route-switch-20261022/before/`。相关 accessibility / Material foundation / utility 回归 37/37（退出码 0），`npm run check`、`npm run build`、debug Tauri no-bundle 通过。此证据验证已定位的挂载链，真实 Tauri 偶发帧是否完全消失仍待所有者复验。

按钮内容按冻结 v3 的 Projects/package/repository/user-package/template/log/extension 实际入口核对：官方原生刷新入口统一使用公共 `RefreshButton` 和既有刷新图标，覆盖页面工具栏、Settings 的 Unity 区域、Extension UI 宿主和行内刷新；保留可访问名称、提示、busy/disabled 与原请求保护。仓库行 Browse packages 改用既有包图标，仓库及用户包移除入口改用既有关闭图标，原确认弹窗和操作语义不变。新增、导入、保存、确认及菜单文字按动作保留；扩展提供的 Portable UI 声明按钮不扩展图标合同。没有新增图标资产、CSS、RPC 或依赖。

标题/刷新/资源行按钮专项 16/16（退出码 0），覆盖 22 个入口的侧栏标题对应和原生工具栏刷新图标、12 个列表入口的桌面几何、刷新请求合并/过时响应/错误、用户包移除确认及焦点等既有行为。`npm run check`、`npm run build`、debug Tauri no-bundle 与 diff check 通过。完整窗口 fixture 截图已更新至 `target/m7-v3-recheck-20260919/browser/`；该专项不代表完整 GUI suite、屏幕阅读器或真实 Tauri 交互通过。没有修改 Rust，未重复完整 Rust Workspace 测试；既有环境限制仍保留，未推送也未运行当前 Hosted CI。

自绘滚动条使用 `overlayscrollbars = 2.17.0`（MIT，无额外 runtime dependencies），通过共享 `@alcomd/ui` adapter/theme 和 GUI `ScrollArea` 接入公共列表、工作区、Projects 网格、侧栏及筛选弹层；Material dialog/menu（含 Select 内部菜单）通过 Lit 公开生命周期接入。冻结 v3 使用 Radix React ScrollArea；本轮选择可绑定现有 viewport 的 OverlayScrollbars，不复制 v3 源码，也不引入 React adapter 或第二套库。真实轨道/滑块由库生成 DOM，原生滚动条只隐藏，不保留原生换肤实现；沿用 10px 透明轨道、细圆角滑块及共享主题/高对比度颜色。滚轮、触控、键盘及内容滚动保持浏览器原生语义。React 首次测量在子元素更新后的帧执行，并用库的 initialize 属性避免原生滚动条闪现；关闭的 Material 弹层不建实例，关闭/卸载时销毁观察器和生成节点。滑块层级高于列表粘性列；公共工作区原有“唯一内容列表”高度规则排除生成的滚动控件。库不支持 textarea 内部编辑滚动，该部分保持原生编辑行为，明确不计作自绘覆盖。没有修改业务合同、列定义、已认可圆角或图标、Rust 或原生平台 API。依赖比较/风险记于当前 ExecPlan，完整 MIT 文本记于 `THIRD_PARTY_NOTICES.md`，lockfile 仅新增该 exact package。

自绘重做完整 browser 首轮 99/101：257 包加载和模板强制点击禁用项的后续重开失败。修正关闭菜单的无效初始化开销、首次尺寸测量时机后，大包量仍保留原 5 秒标题断言；受控跳过库初始化的诊断对照已执行并自动恢复源码，没有交付旁路或测试开关。模板事件记录证实 `force` 在展开动画期间绕过 actionability、命中当时菜单外部而触发关闭；仅等待展开完成再执行原鼠标/键盘禁用和重开断言，产品 Menu 状态/动作时序不改，临时诊断代码已移除。最终受影响回归 **12/12 PASS**（退出码 0），覆盖两个失败、模板流程、重复菜单激活、焦点、12 个列表在 1280px / 1440px 的既有几何及三个自绘专项；不把它写成最终完整 101/101。专项验证真实滑块 DOM、原生 gutter 为零、纵横滚轮、拖拽命中、内容收缩后隐藏、卸载清理、Material 弹窗/Select、主题/高对比度和 textarea 编辑保持；初轮发现并修正滑块被粘性列遮挡，测试按库实际属性及标准/兼容隐藏路径核对。无头测试取消 `--hide-scrollbars`，截图不禁用库用来定位滑块的滚动动画。前期另有一次测试进程在进入用例前异常退出 `3221226505`，不计作功能失败或通过。完整窗口 fixture 截图为 `target/m7-v3-recheck-20260919/browser/custom-scrollbars.png`，首轮两项失败证据保留于 `target/m7-overlayscrollbars-20261022/before/`，均不冒充 Tauri 操作证据。最终 `npm run check`、frontend build、xtask 与 diff check 通过；没有改 Rust，未重复完整 Rust Workspace 或宣称三平台运行通过。

滑块自动隐藏由共享 adapter 的 `autoHide: "scroll"`、`autoHideDelay: 600` 与共享 theme 共同实现：实际滚动或悬停滚动条轨道时唤出，内容区移动指针不触发显示；停留在轨道/滑块及拖拽期间保持可见。沿用库计时：最后一次滚动后约 100ms 收尾，再等待 `autoHideDelay` 的 600ms，随后用 150ms 淡出；离开轨道不重新计时，若隐藏计时已结束则立即开始 150ms 淡出，否则沿用剩余时间。项目所有者选择保留这组内置行为，显隐和计时由库负责，不保留自定义计时 controller。该滚动条配置的专项复跑 3/3、`npm run check`、`npm run build` 与 diff check 通过；本轮切页增量的最新构建以本节下方为准。theme 只为库标记为可滚动轴且处于自动隐藏状态的轨道保留 `visibility: visible`，透明度仍由库控制，借其已有 pointerenter/leave 和拖拽状态恢复显示；不增加隐藏计时器、全窗鼠标监听或每页补丁，无溢出轴保留库原隐藏/不可交互规则。此前 `leave` / `move` / 仅 `scroll` 的结果均为历史技术证据，不能代替当前语义验收。新增用例在修复前复现隐藏轨道无法唤出的失败；共享 theme 修正后专项 **3/3 PASS**（退出码 0），真实鼠标在两轴隐藏轨道上悬停可显示，且滚动位置仍为 `[0, 0]`，离开轨道后淡出。保留内容区连续移动 6 组采样始终透明、滚动显示、轨道悬停/拖拽保持、纵横滚动、卸载及 Material dialog/Select 回归。测试按实际绘制透明度判断显隐，并独立验证鼠标命中，避免把保留命中的透明节点误当成绘制可见。修复前证据为 `target/m7-scroll-edge-20261022/before/`；前一轮内容区移动错误的证据保留于 `target/m7-scroll-trigger-20261022/before/`。该增量的 `npm run check`、frontend build、debug Tauri no-bundle 与 diff check 通过。库策略已核对已安装 2.17.0 类型定义及 [官方选项文档](https://github.com/KingSora/OverlayScrollbars#scrollbarsautohide)。

所有者要求先撤掉新增动画、解决闪烁后再考虑动效。当前已删除 `PageTransition` 及输入方式监听，撤掉侧栏背景、筛选弹层与操作反馈新增的过渡/入场规则；保留 Material 与 OverlayScrollbars 原有行为及应用 reduced-motion 设置，不以隐藏动画开关代替删除实现。

当前切页通过共享 `PageRead` 协调首读：普通只读工作区在新页数据就绪后一次替换当前 DOM，等待时保留旧页面原有绘制，正文 `inert` 阻止旧页操作，侧栏仍能选择其他目的页；只有超过 160ms 才显示不占布局的顶部轻量进度条。标题焦点在新页真正提交后恢复，迟到响应不能覆盖最新目的页。每次访问与每次重连使用独立、仅内存的读取作用域，最多三个固定槽位，不跨访问复用结果、不建立持久缓存或目录 revision。Projects/ResourcePage/UtilityWorkspace 共用主首读；Settings 的设置快照、Unity 安装列表与仓库来源首读并行准备、共同就绪后显示，分别保留失败状态，避免先空白再分批填充。刷新/来源变更继续走原读取与失效逻辑，错误/空/断开状态保留；弹窗内读取保持局部加载，扩展 Session 动作仍在原事件/effect 中执行，不通过预读创建 Session、Plan 或 Operation。实现使用已安装 React 的 [useDeferredValue](https://react.dev/reference/react/useDeferredValue) / [Suspense](https://react.dev/reference/react/Suspense) 协调提交，不包含视觉动画。无新增依赖、RPC、Rust、权限、持久配置或图标修改，已认可列表、工具栏、圆角和滚动条保持不变。

动效及相关 browser 回归首轮 18/19：唯一步骤点击了弹层圆角裁切外的位置；改用弹层内部位置后，与当时滚动条增量合计 **7/7 PASS**，并非完整 GUI suite 通过。更早专项还纠正了空 User Package fixture、Material slot 定位及弹层既有视口边界限制的测试假设；初次 check 的 Playwright fixture 类型错误已修正，未修改生产语义或放宽验收。此前通过的 18 项包括普通切页无能力加载灰层、22 个入口、键盘/弹窗/Portable UI 焦点及草稿、12 个列表在 1280/1440px 的几何、形状层级和筛选。动效测试在浏览器内暂停真实动画以确定性验证连续切换取消，不向产品加入测试开关。`npm run check`、frontend build、debug Tauri no-bundle 及 diff check 通过；未修改 Rust，未重复完整 Rust Workspace。失败诊断证据在 `target/m7-motion-20261022/before/`。真实桌面动效观感和无障碍证据仍待所有者验收，Visual Gate 2 保持 PENDING。

原轻淡入及随后完整页面淡入的结果保留为历史技术证据：后者的 browser 回归曾 **16/16 PASS**（退出码 0），视频在 `target/m7-motion-visible-20261022/unpaused-page-transition.webm`，初轮 fixture 失败证据在同目录 `before/`。所有者随后确认动画可感知，但仍有加载闪烁和灰色加载；这 16 项通过不证明观感通过。

正文过渡与共享轻量加载候选的 **19/19 + 3/3 PASS** 保留为历史技术证据，两次退出码均为 0；视频/拼图在 `target/m7-motion-loading-20261022/`。这些检查验证了不透明度和加载提示，但没有证明旧页面卸载后的空白间隔被消除。所有者继续报告明显闪烁，并明确指出任意页面切到 Settings；该候选不作为视觉验收通过依据。此前前端 check/build 和 debug Tauri 构建通过，测试结束后的 Vite 服务需清理才能结束收尾；服务终止日志保留。本轮撤除动效后的读取交接验证以当前结果为准，Visual Gate 2 保持 PENDING。

撤除动效后的 Settings 专项首轮 **8/8 PASS**，逐帧检查 Projects/Resources/Logs → Settings、设置快照/Unity/仓库三类响应分批放行、迟到响应及真实错误；确认旧页或完整 Settings 持续绘制，没有中间空白/部分 Settings 帧，透明度始终为 1，未调用新增动画。全套初轮 19 项通过、3 项失败、88 项未执行：同页键盘焦点与两项日志弹窗读取作用域错误；弹窗边界修正后相关组 12/13，余下同页焦点在 Material 键盘处理结束后的帧恢复。第二次全套 **108/110**：两项独立 UtilityWorkspace 用例暴露组件在无导航 Provider 时不能初读；修正共享 hook，使普通独立使用与共享 Material.Dialog 内都采用局部读取，保留公共组件独立使用、排队刷新及迟到响应语义，原测试断言未改。最终受影响组（上述两项与九项 Settings/导航/错误/筛选检查）**11/11 PASS，退出码 0**；不将分组复测描述为全套 110/110 同轮通过。失败诊断在 `target/m7-navigation-settings-20261022/before/` 与 `standalone-before/`；最终完整窗口视频与已目视核对的拼图在同一主目录。均为浏览器 fixture 证据，不冒充真实 Tauri 操作或屏幕阅读器验收。各轮用例完成后均需清理已确认属于本轮的 Vite 测试服务才能结束收尾，服务终止日志保留；失败轮退出码为 1，通过轮为 0。未执行真实工程 Apply/迁移/复制/删除，未重复完整 Rust Workspace，Visual Gate 2 保持 PENDING。

最终 `npm run check`、frontend build、debug Tauri no-bundle 与 diff check 通过。当前 `target/debug/alcomd-gui.exe` 为 dev profile（unoptimized + debuginfo），SHA-256 `3CBE955A158F67127FDE6183191E4FFA4BA4E3B881BAB49C018A2B4141D0A10E`，已重开（PID 198176），使用原 `target/m7-icon-acceptance-20260906/data` 隔离环境。复用本轮已启动的 daemon（PID 194596，二进制 SHA-256 `982D77AAA55B85ED030760290257DCBC21E80F03661C3A492BC1D5F3D5CFE95C`），再次核对 executable/data-dir 并通过既有 CLI `system status` 确认为 ready / 4.0.0-alpha.0；本轮未改 Rust、未重建 daemon/CLI，只关闭本轮已核实路径的旧验收 GUI，未操作真实用户工程。窗口启动不代表标题、按钮、切页稳定性、自绘滚动条的真实桌面交互或人工视觉验收通过，屏幕阅读器证据仍未取得。构建保留既有大 chunk 警告及本机增量缓存 hard-link 不可用而改用 copy 的警告，**Visual Gate 2 PENDING**。

2026-09-22：所有者明确选定 MD3 Segmented buttons。Resources 与 Logs/Settings 同类切换入口已复用现有 Material Web 2.5.0 labs 的 outlined segmented button/set，使用单选、文字式无勾选图标配置，保留禁用条件与路由权威状态；移除旧普通按钮拼接底板。原按钮宽度 WIP 保留，无新增依赖或 RPC。组件/页面回归 17/17，键盘、禁用跳过及拒绝导航恢复专项 1/1；专项首轮因 Shadow DOM slotted 子元素定位失败，修正测试定位后通过。check/build 与 diff check 通过，构建保留既有大 chunk 警告。浏览器完整窗口证据在 `target/m7-v3-recheck-20260919/browser/`；未取得新的真实 Tauri 或屏幕阅读器证据。所有者随后授权将按钮宽度与分段按钮改动分别提交，并将本地 main 累积提交正常推送至 origin/main；该授权不代表视觉验收通过。**Visual Gate 2 PENDING**。

本节是唯一当前阶段记录；下文“已完成”、旧 ExecPlan 日志、testing 报告及归档分支均是历史证据，不构成另一条活动开发线。

- **唯一工作目录**：`C:\Users\M2922\MyProjects\Rust\ALCOMD`；活动分支 `main`。两个额外工作树已经移除，后续不自行创建 worktree。
- **当前阶段**：M7 H2-A Project Workspace / Packages，`IN_PROGRESS / VISUAL_GATE_2`；尚未通过完整人工视觉验收。
- **当前代码来源**：`main` 的公共列表及 GUI 修正为 `ce99bee`，跨页面列表与圆角回归为 `aac9c30`；公共标题样式与对应回归为 `131c84f`，连接能力快照生命周期与瞬时灰层回归为 `b69e9b5`，记录提交为 `0ddf7e7`。侧栏/标题名称统一及对应回归为 `385ed18`，公共刷新及资源行按钮内容修正和对应回归为 `d00da14`，验收及状态记录为 `50a067e`。OverlayScrollbars 共享自绘组件、Material 生命周期及自动隐藏、exact 依赖与许可记录为 `579a502`，专项回归为 `83e4360`，记录为 `06a2eff`。滚动条内容区移动不唤出、轨道悬停唤出及对应回归为 `00d9256`，共享切页/筛选/反馈动效与对应回归为 `7744d03`；验收及状态记录为 `561295e`。新增动效撤除、共享首读交接与弹窗读取边界、Settings 首批内容共同就绪的实现为 `bc1726d`，逐帧浏览器回归及既有加载监测更新为 `46ec6c6`；本次验收与状态记录独立本地提交；此前整合和功能 checkpoint 保留为历史证据。
- **旧修改保留**：原标准目录 9 个未提交文件完整保存在 `b4547d5`，作为历史恢复快照，不复活已被替代的 Automatic Editor 产品模型。
- **下一步**：当前列表样式已获所有者认可，仍需完成 H2-A 真实桌面交互和完整视觉验收；项目所有者批准的其他既有页面基础可用性修正属于当前 H2-A，不代表 H2-B/H3-H7 全量获批或任一 Visual Gate 通过。
- **边界**：候选查询沿用已批准合同；本次其他页面修正复用现有读写接口与 Material 组件；滚动条仅新增本轮获准的 OverlayScrollbars exact 依赖，不新增 State/Config migration 或平台 API，不改变 resolver/Plan/Apply，保留已确认 Projects/package workspace 的图标与布局，不做窄窗口适配，不自动 push。
- **远端**：公共列表、标题样式/名称、切页、刷新及资源行按钮修正和对应回归、验收记录均已整理为本地提交，未推送；共享滚动条实现与回归已分别整理为本地提交，验收状态记录独立提交，均未推送；本轮动效与滚动条行为修正及对应回归已分别整理为本地提交，验收状态记录独立提交，均未推送。新增动效撤除与首读交接实现、浏览器回归已分别整理为 `bc1726d`、`46ec6c6` 两笔本地提交，本次验收与状态记录独立提交，均未推送。没有 push、force-push 或历史重写；此前 CI 只证明其对应 SHA，不证明本轮本地修改。

整合审计、恢复位置和本轮验证结果见 [工作目录整合记录](testing/workspace-consolidation-20260917.md)。

2026-09-19 当前补充：项目所有者已明确拒绝上一版其他页面重做，指出其没有按 v3 的弹窗与页面结构执行。
上一版 78/79、局部 26/26 和 17 张截图仅保留为历史技术证据，**不能作为设计符合性或验收通过证据**。
本轮重新读取冻结 v3 并逐流程核对：Settings 按支持组重建、资源/模板/扩展/备份及启动参数改为对应模态流程，
扩展恢复卡片/启用/Open，日志增加有界搜索筛选与真实 Operation 详情。已认可 Projects/package workspace 保留。
完整差异、现有合同缺口、当前验证与浏览器证据见 [v3 重新核对与修正](testing/m7-v3-recheck-implementation-20260919.md)。
当前 GUI 修正已按所有者要求整理为本地提交：`c67dca1`（公共组件与独立回归）、`ce443ec`（页面/交互与页面回归）；本记录与审计资料单独提交。仍待视觉验收，未推送。完整 browser 首轮 85/91；修正后受影响组 43/44，最后一个旧文本断言修正复测 1/1；额外宽弹窗几何修复后 utility 6/6。最终 frontend check/build、xtask 与 diff check 通过。真实 Tauri 与屏幕阅读器证据未取得；**Visual Gate 2 PENDING**。

2026-09-19 按钮补充：在 `a531e56` 上统一共享按钮各 variant 的水平留白，修复选中切换时宽度变化；状态文案按钮按已知最长文案预留宽度。组件/页面浏览器回归 17/17、分页 3/3，frontend check/build 通过。该修正按所有者要求独立提交，并纳入本次获准推送；真实桌面与屏幕阅读器验收未增加。见 [按钮宽度修正记录](testing/m7-button-width-20260919.md)。Visual Gate 2 仍为 **PENDING**。

标准目录的 check/build/xtask/metadata/diff 检查通过；浏览器首轮 47/48（navigation 超时），
保持代码及断言不变、按先前单 worker 配置复跑为 48/48。首轮原因未确认，不隐去失败记录。

2026-09-17 GUI 补充（批准候选查询前的历史记录）：已实现 Installed 列精确仓库版本/来源选择、prerelease 标记传递、
单 Bulk Plan 批量移除，并修复搜索隐藏选中行时批量重装只提交可见子集的问题。
用户包缺分类时保留窄手动降级入口，修复其已选来源丢失；不伪造分类。
完整 52 项 browser tests、frontend check/build、最终 debug Tauri no-bundle、xtask 与 metadata 检查通过。
**旧 Latest/Update 的字符串排序与比较尚未修正，不构成条件更新验收证据。**
本轮未改生产 Rust/公共合同/依赖/图标/布局 CSS，未提交或推送，Visual Gate 2 仍开放。
详细证据、失败记录与桌面验证边界见 `testing/m7-h2-a-inline-version-20260917.md`。
computer-use 读取 v3 / 启动最终 v4 GUI 均返回应用授权超时；新真实桌面交互未执行。
已核对真实隔离 daemon/CLI 和 5 个源样例文件摘要，停止本轮测试 daemon，保留测试资料。

2026-09-17 Stop A 合同提交记录（历史，后已获实施批准）：当时仅文档、提案 Schema/vector 和合同校验；没有新查询生产实现。
P6/P8 原 PASS 保留为历史 checkpoint，当前 completeness fixture 的 `currentH2AAssessment`
明确为 IN_PROGRESS，不能由历史 PASS 或 52/52 推导整个包管理页完成。
Material host disabled/click guard 不证明屏幕阅读器内部菜单项禁用播报；该证据仍缺失。
桌面授权超时仅表示证据未取得；后续允许所有者手动启动指定构建、执行检查并提供截图，
无需强制 computer-use，Visual Gate 2 仍 PENDING。本轮不再尝试桌面授权。
合同与验证审阅记录见 `testing/m7-package-candidates-stop-a-review.md`。

2026-09-17 批准后实现：`packages.queryProjectCandidates` / `packages.candidates.v1`
已接入 Core/Application/Store/RPC/SDK/Tauri。行内 Repository/User Package 版本、Core Latest/Stable、
条件更新、Update All 与完整选择集批量动作已实现，过渡手填入口已统一移除。
当前浏览器全套 **69/69 PASS**，Rust fmt/全 workspace Clippy 通过；真实候选 RPC、Config
竞态/取消、DTO、Core 排序与既有包 Plan/Apply 回归通过。全 workspace 测试仍有既有 Project Copy
用例因真实 Unity 导致 `running_suspected` 的环境限制，未关闭用户进程、未放宽 writer gate。
39 向量逐项映射测试（38 项实现测试引用、1 项屏幕阅读器证据限制），结构检查不冒充语义执行。
`currentH2AAssessment` 保留 IN_PROGRESS：真实桌面和屏幕阅读器证据仍开放，**Visual Gate 2 PENDING**。
完整验证、构建、失败经过与验收说明见 `testing/m7-package-candidates-implementation-20260917.md`。

### 已有验收基线与 H2-A 局部证据（历史）

`M6 已正式完成；M7 Portable UI B0-D、Project / Package Functional Closure P0-P8 与 Project Unity Version / exact Open Unity U1-U7 technical closure 已通过。Unity production sealed HEAD e9349fd0f4cc9c18d9174dfc5826d48654aeb6f5 对应 Hosted CI run 33352406804 与 CodeQL run 33352406214 均通过；更新后的 P8 M7-owned visible-action completeness 为 PASS，global release completeness 仍为 BLOCKED_BY_M11。H2-A 已恢复为 ACTIVE，并停在 Project Workspace / Packages Visual Gate 2 候选制作；24b03916bd6d958c921b4039bd62bec00afe25d4 保留为 pre-Unity-model local checkpoint，不是 rejected，也不是当前候选。M7 仍为 IN_PROGRESS，projects.management、packages.vpm 与 repositories.management 保持 in_progress，M8/M9 未开始。`

H2-A 最新本地补充：shared icon integration 与 reconnect 修正已通过 44 项 GUI browser tests 和
Tauri no-bundle build；真实 Windows GUI 的 package / Copy review 已完成局部检查。
项目所有者明确不要求窄窗口适配；撤销未经批准的 Open Unity 图标/位置替换，保留原播放图标。
项目所有者已接受 18px button icon；临时双图标比较已撤下，完整 44 项 browser tests 和 debug
no-bundle build 再次通过。隔离测试项目的真实 GUI Copy 已成功，源文件未变且副本 5 文件摘要一致；
副本包管理页与缺失精确 Editor 提示已检查。未执行真实 Unity launch 或 migration。
Visual Gate 2 不关闭，也不据此重排操作菜单或隐藏数据列。
证据与未验证边界见 `docs/testing/m7-icon-reconnect-local-acceptance.md`；本轮无新 Hosted CI 或人工验收结论。

后续 v3 源码对照已落实包管理工具栏与筛选入口的一轮补齐，完整 browser suite 为 48/48 PASS，
frontend check/build、debug no-bundle build、xtask 与 metadata 检查通过。新筛选的真实桌面交互仍待验证；
版本选择、条件更新、完整批量操作仍未完成，不提升 feature 状态。
详见 `docs/testing/m7-h2-a-package-workspace-continuity.md`。

## 已完成

- 稳定命名与产品身份配置。
- Rust Workspace 骨架。
- Tauri 2 + React + Vite GUI 壳。
- CLI、MCP、API、Extension Host、Bootstrap 与迁移程序占位。
- MCP 管理与 Discord 第一方扩展清单。
- 架构、ADR、契约和 Codex 工作规则骨架。
- CI、Dependabot、初始化和检查脚本骨架。
- 旧内部标识扫描 `cargo xtask check`。
- 已接受 ALCOMD3 3.4.0 作为唯一直接迁移入口；更早 v3.x 必须先升级到 3.4.0。
- `source-lock.toml` 已锁定 v3 审计源、v3.4.0 迁移入口版本、GitHub Release 安装与
  updater/签名资产、updater 公钥指纹、vrc-get 功能行为基线和 MCP `2026-07-28` 规范；
  `freeze-baselines.ps1` 可通过冻结时分支上下文、不可变 commit API 证据和 Release API
  确定性生成并校验该文件，且不会因上游分支或标签后来前进而使固定 commit 失效。
- v3.4.0 Release 当前 `immutable = false`；锁文件以 release/asset ID、大小和 SHA-256 建立可检测篡改的快照，任何远端资产变化都会使 `freeze-baselines.ps1 -Check` 失败。
- `docs/baselines/vrc-get.md` 已恢复为独立的功能、安全与行为基线；明确 vrc-get 是必须覆盖的验收来源，但不是代码上游。
- ALCOMD3 v4 已明确为 ALCOMD 产品家族中的独立新项目：继承 v3 的用户品牌与功能定位，但不复制、移植或改写 v3、vrc-get 或 vrc-get-vpm 源码，VPM 独立实现。
- v4 自有代码、SDK、规范、文档、脚本与第一方扩展统一采用 `AGPL-3.0-only`；完整许可证与第三方边界已经落盘。
- 已完成冻结 v3.4.0 与 vrc-get commit 的静态细粒度审计，覆盖项目、包/仓库、Unity、模板、
  备份、GUI、CLI、MCP、Discord、更新器、安装器、错误和高风险行为；结论分别记录在
  `docs/baselines/alcomd3-v3-audit.md` 与 `docs/baselines/vrc-get-audit.md`。
- `feature-parity.toml` 已提升到 user-entry 清单，区分“基线审计状态”和“v4 实现状态”；所有
  release blocker 已关联机器可读测试计划，但需要真实 Fixture 的计划仍为 blocked。
- `migrations/v3/artifacts.toml` 已按 R0/R1/R2/C1/N 阶段记录源码确认的路径/身份模板、所有权、
  操作与 residue test；没有真实安装快照的可删除实例全部保持 `confirmed = false`。
- MCP 核心规范已锁到官方 commit、最终 Schema blob/SHA-256 和固定 conformance npm tarball；
  A-021 已决定 4.0.0 使用 OperationId 与显式输入/审批工具，不采用 Tasks；A-023 已冻结无
  session 权限名与 HTTP/STDIO Principal 隔离方向。
- 更新 API 与 v3.4.0 实际接受的 bridge 输入、Minisign 身份、文件名、下载上限和失败语义已
  冻结；M-1 不实现或发布 bridge。
- A-024 与 ADR 0015 已关闭 O-008：ALCOMD 是多组件 Rust 本地应用平台，只有
  `alcomd-gui` 是 Tauri 子应用；Windows 使用单一 Inno Setup EXE 的两种互斥安装模式，
  macOS 使用 DMG，Linux 使用 AppImage 与 DEB。三个平台、四种主要格式均为
  4.0.0 blocker。
- 完整产品发行、三平台 CLI 集成、updater/bootstrap 边界与未来 `cargo xtask dist` 已进入
  `docs/exec-plans/M12-full-product-distribution.md`；本阶段未实现安装器、签名或发行资产。
- A-025 已关闭 O-003：Windows 正式测试 Win10 22H2/Win11 并采用 WebView2 Evergreen 在线/
  离线部署；Linux 在 Ubuntu 22.04 构建且发行 ELF 最高所需符号不超过 `GLIBC_2.35`；macOS
  arm64 deployment target 为 11.0。当前 Authenticode 与 Apple Developer ID/notarization
  不是 blocker，但应用层更新/bridge/扩展签名和摘要验证仍为强制要求。
- A-026 已批准 MCP v4 工具命名基线、33 项功能覆盖而非一对一工具、Plan/Apply、
  `OperationId`、`diagnostics.read` 与稳定结构化错误方向；未知内部错误固定使用
  `internal_error + diagnostic_id`。
- 项目所有者已确认冻结 vrc-get 只作为实现与风险参考，不继续对其执行攻击性黑盒或网络
  安全验证；路径、ZIP、事务、来源确定性和凭据边界进入 ALCOMD 自身实现的验收测试。
- 项目所有者已批准 M-1 完成；`feature-parity.toml` 的 `verified` 只表示对应基线、范围和验收
  合同已冻结，所有 `implementation_status` 仍独立反映真实实现状态。
- M0 ExecPlan 已细化为固定工具链、`npm ci`、Cargo lock、三平台 `--no-bundle` build 和独立
  扩展 test gate。
- M0 已补齐纯占位 `alcomd-updater` app 边界；它只报告 scaffold 状态，不包含下载、签名、
  替换或回滚实现。
- `alcomd.product.toml` 现在覆盖 updater 身份；xtask 与元数据验证器校验 Cargo Workspace、
  Tauri、npm、第一方扩展和派生身份，并固定本地 crate 依赖方向。
- PowerShell/Bash setup/check/test 已对齐：Node 安装使用 `npm ci`，Cargo 构建/Clippy/test 使用
  `--locked`，完整 Workspace 不再排除 GUI，独立 Discord backend 增加 test gate，命令执行
  前后校验根 `Cargo.lock`、根 `package-lock.json` 与 Discord backend `Cargo.lock` 摘要。
- 跨平台 CI 已按批准方案配置并实际通过：`windows-2025`、`ubuntu-22.04`、Apple Silicon
  `macos-15` hosted job；所有 job 固定 Rust 1.97.1、Node.js 24、Python 3.11 与 action commit。
- Linux job 安装含 `libgtk-3-dev` 的 Tauri 前提并执行 GLIBC 符号上限检查；macOS job 检查
  arm64 与实际最低部署版本。GitHub Actions run `32056593208` 在提交
  `8a6f2968bdf4212a7a98f0ea55d93cc291883e87` 上通过：Linux 实测最高 `GLIBC_2.34`，九个
  macOS Mach-O 均为 arm64 / minos 11.0，三平台三锁文件 gate 均通过。
- Windows 本机已通过 setup、完整 check/test、Git Bash 语法检查和 Tauri
  `build --no-bundle`；该结果只证明 GUI 子应用可构建，不是完整产品发行验证。
- M0 最终提交 `8112415f1dae0dc6f521d5cc3a2c980baac3b408` 已通过 GitHub Actions run
  `32066209115`：Windows Server 2025、Ubuntu 22.04 与 macOS 15 arm64 三个 hosted job
  全部成功；项目所有者已确认 M0 最终验收通过。
- M1 的 RPC v1 帧、envelope、稳定错误、`system.hello`/`system.status` Schema 与兼容规则已经
  contract-first 冻结并通过 Rust/Schema 合同测试；响应不虚构后续 data/config/extension 版本。
- 已实现真实的最小只读垂直切片：`alcomd-platform` 提供每用户安全端点与生命周期实例锁，
  daemon 只经 `alcomd-application` 返回 ready 状态，`alcomd-client` 完成握手和类型化查询，
  `alcomd-cli system status` 支持 human/JSON 与 `--no-start-daemon`。
- Windows 本机已动态通过当前用户 SID Named Pipe/DACL、单实例、并发 client、帧错误、CLI
  human/JSON 与两个 CLI 并发按需启动测试；最终只产生一个权威 daemon，测试进程已清理。
- Unix 实现使用获批的 `rustix 1.1.4` safe API 完成有效 UID、逐组件 no-follow、0700/0600、
  fd-based 类型/所有权校验、非阻塞独占 flock 与 stale socket 恢复；Linux 与 macOS target
  编译检查及各自 hosted 环境中的真实运行测试均已通过。
- Windows FFI 仅存在于私有 `windows_security.rs`，所有 unsafe block/impl 有 SAFETY 说明；
  xtask 禁止其他文件使用 unsafe 或新增 allowance。Cargo.lock 只新增获批的 `rustix 1.1.4` 和
  `linux-raw-sys 0.12.1`，复用现有 bitflags/errno/libc/windows-sys。
- M1 实现提交 `e509554af6cb1029f4a023e26013b495c0a56ffe` 已通过 GitHub Actions run
  `32124358425`：Windows Server 2025、Ubuntu 22.04 与 macOS 15 arm64 三个 hosted job 全部
  成功；Linux 实测最高 `GLIBC_2.34`，九个 macOS Mach-O 均为 arm64 / minos 11.0，三平台
  三锁文件与最终 diff 门禁均通过。Windows hosted 结果仍不代表 Win10/Win11 客户端发行验收。
- 项目所有者已确认 M1 最终提交 `7ed70626a0176f855a8e9efdc6d35d317f51ca78` 与 GitHub Actions
  run `32126344788` 人工验收通过；HEAD、`origin/main` 与 CI head SHA 一致，M1 正式完成。
- M2 已按冻结合同实现 `state.db` Schema v1、单连接 SQLite worker、`state.check`、Operation/
  Event/Revision/永久幂等、两个 Resource Key、恢复 journal、五个兼容 RPC 方法与三项 capability；
  daemon 在 store 初始化和恢复完成后才 bind，启动失败保持 fail closed。
- Windows 正式数据目录通过获批的私有 `windows_known_folder.rs` 使用
  `SHGetKnownFolderPath(FOLDERID_LocalAppData)`；COM 初始化和返回内存由局部 RAII 平衡，xtask
  将 unsafe 硬限制在两个已批准 Windows 文件。没有新增 crate 或额外 windows-sys 版本。
- M2 聚焦验证已覆盖 Schema/migration 回滚、幂等与 Revision 冲突、Principal owner 隔离、
  Event/Operation 分页、取消竞态、100 并发命令、journal 不一致安全失败、真实 IPC 垂直切片及
  子进程强制终止/重启恢复。完整本地 `check.ps1`、`test.ps1`、冻结基线、跨目标 platform
  compile 与差异门禁均已通过。
- M2 最终提交 `9076574ef0f4d3de8690865dfb18aa5856d7ad64` 对应 GitHub Actions run
  `32144082427`：Windows Server 2025、Ubuntu 22.04 与 macOS 15 arm64 三个 hosted job
  全部成功；Ubuntu 实测最高 `GLIBC_2.34`，macOS 预期产物均为 arm64 / minos 11.0。
- 首轮 Windows hosted 测试发现 SQLite worker shutdown 与测试目录清理之间的生命周期竞态；
  最终提交已改为在最后一个 state store handle 释放时确定性关闭并回收 worker，随后重新通过
  Windows 完整测试。项目所有者已确认 M2 人工验收通过，M2 正式完成。
- M3 已冻结 ADR 0016、RPC/storage Schema v2、项目/仓库权限与稳定错误；实现 exact/parent 项目
  发现、VPM/UPM/ProjectVersion bounded 读取、local/anonymous HTTP(S) repository 读取、平台对象
  identity、Schema v2 registry、revision/Event/永久幂等、304/no-op 与 last-known-good 语义。
- M3 已接通 daemon/application/store/vpm/protocol/client/CLI 的真实只读垂直切片；synthetic IPC
  测试确认读取前后项目与 repository 源文件字节不变。完整 v3 parity、repository import/deep-link、
  credential、SemVer/resolver、package 下载/安装与项目写入仍未实现。
- M3 直接 HTTP 依赖保持精确 `reqwest 0.13.4`、`default-features = false`、仅 `rustls`；feature
  graph 证明确认 quinn/HTTP3、system-proxy、cookie 和压缩未激活。Cargo.lock 中可存在获批的
  optional/target-only 锁定项，但不将它们描述为 ALCOMD 当前运行时组件。
- M3 Windows 本机 `check.ps1`、`test.ps1`、Tauri no-bundle、冻结基线与锁文件门禁已经通过；
  最终提交 `2082b5596d246975ca7a48dab20826899103e03d` 对应 GitHub Actions run
  `32174028968` 的 Windows Server 2025、Ubuntu 22.04 与 macOS 15 arm64 job 全部成功，且
  CI head SHA、`HEAD` 与 `origin/main` 一致。项目所有者已完成人工验收，M3 正式完成。
- M4 已按 contract-first 合同实现 State Schema v3、确定性 VPM range/resolver、不可变 Plan/
  ChangeSet、stale/source revalidation、SHA-256 content cache、bounded hostile ZIP extraction、项目级
  Resource Lock、install/remove 文件事务、持久 filesystem journal、阶段 progress 与重启恢复。
- `semver 1.0.28` 是唯一 Version model；私有最小 range AST 只覆盖冻结 vectors，并使用
  `Version::cmp_precedence`，不使用 Cargo `VersionReq`/`Comparator`。`zip 8.6.0`、`sha2 0.11.0` 与
  `unicode-normalization 0.1.25` 保持获批的精确最小 features。
- 本机离线测试已证明真实 RPC `Plan -> Apply -> Operation` 安装/卸载、强制终止子进程后的同一
  Operation 重启恢复、append-only journal 合法重复阶段、脱敏持久 progress，以及同项目串行/
  不同项目并行的锁语义。恢复测试发现并移除了错误的 phase/state 唯一约束；journal 仍以 step 为
  主键且禁止 update/delete。
- 补充真实进程测试保留 `archive_ready`，并覆盖 `prepared`、旧包已 rename 到 backup、新包已
  publish、VPM manifest 已 atomic replace、`filesystem_committed` 后五个关键 checkpoint。每例均
  在 durable test evidence 后强制终止 daemon，重启后复用原 OperationId/Plan/幂等键且不重新 Plan；
  最终 package tree 与 manifest 收敛完整新状态，原 Apply 重放返回同一 OperationId。test gate 不
  增加公开 RPC/Schema 或生产 failpoint framework。
- `feature-parity.toml` 引用的四个 M4 test ID 均已存在并绑定真实 evidence；metadata gate 永久拒绝
  空、重复或不存在的 feature test reference，并要求 `implemented` M4 test 的 evidence path 存在。
  integrity/cache 与 concurrency 描述已收敛到实际离线/本地测试证据，不把已排除的攻击性网络、
  凭据传播或公网故障测试描述为完成。
- M4 保持 SHA-256-required remote archive 安全子集和 `Packages/manifest.json` byte-for-byte 不变；
  hashless VPM、legacy cleanup、credential、local user package、完整 GUI/MCP 与 M5 CLI 体验仍未完成，
  因此完整 `packages.vpm`、`packages.transaction-safety` 与 `packages.security` 只标记 `in_progress`。
- M4 最终代码候选 `cd125da1ff4609dd34bf893ad193e7034fd91674` 对应 GitHub Actions run
  `32274892596` 的 Windows Server 2025、Ubuntu 22.04 与 macOS 15 arm64 job 全部成功；Ubuntu
  实测最高 `GLIBC_2.34`，九个 macOS 预期产物均为 arm64 / minos 11.0，三平台锁文件与最终 diff
  门禁均通过。首轮 CI 暴露的 macOS directory fsync `EINVAL` 平台差异和 Windows 测试临时路径
  时间戳碰撞已在最终代码候选中修复；Ubuntu 前两次重跑停滞于系统软件源，第三次未复现且完整通过，
  未修改 baseline、CI 或安全门禁以规避外部故障。
- M4 最终补充验收提交 `20a86d674b480981d269088cf0615ffdcd9b8e70` 对应 GitHub Actions run
  `32289522274`：Windows Server 2025、Ubuntu 22.04 与 macOS 15 arm64 全部成功；Ubuntu 实测最高
  `GLIBC_2.34`，macOS 所有预期产物均为 arm64 / minos 11.0。真实 filesystem kill/restart matrix
  覆盖 `archive_ready`、`prepared`、旧包 backup rename、新包 publish、VPM manifest atomic replace
  与 `filesystem_committed`，均复用原 OperationId、Plan 和幂等键恢复且不重新 Plan。metadata
  test-reference gate 已永久加入。项目所有者已确认最终人工验收通过，M4 正式完成。
- M5 CLI contract v1 已冻结 human/JSON/NDJSON、stdout/stderr、`0/1/2/3/130` 退出码、alias、
  `--yes`/`--dry-run`/`--no-wait`、TTY/EOF、Ctrl-C detach、Operation 分离与静态 completion；命令
  catalog 只有后端 capability 真实实现后才允许发布，CLI 继续只能经 `alcomd-client`/RPC。
- M5 Unity contract 已冻结 installation registry、project editor preference、bounded argv、launch/
  observation、四态 writer evidence、hard/advisory gate、RPC capability/error 与最小权限。State Schema
  v4 已接入 v3→v4 自动 migration，daemon/client 已广告并实现十个 Unity method 与三项 capability。
- 项目所有者已批准 `sysinfo 0.39.6`（defaults off，仅 `system`，官方 MSRV Rust 1.95）。依赖只进入
  `alcomd-platform`，最小 refresh 显式 `without_tasks()` 且只请求 exe/cmd；没有启用 rayon、增加
  ALCOMD unsafe 文件或平台 API。fake provider、真实短生命周期子进程和并行观察测试已接入。
- Unity 最小生产切片已实现 synthetic Editor 校验、手工/known-root registry、project Editor preference、
  PID+start-time writer evidence、无 shell 独立 argv 启动、永久幂等 launch/status 与真实 daemon RPC
  往返。完整 Hub 格式、真实 Unity/Hub parity、迁移与 foreground 仍未完成。
- Unity production slice 已作为独立提交 `8b63c6923b178a6ebb12bd5964412b2db7268e04` 保存。M5
  Template contract-first 已冻结原生 `.alcomdtemplate` v1、独立 archive quota、三个 native builtin
  inventory/AGPL provenance、Schema v4 registry compatibility、RPC/permission/error、已发布 CLI、synthetic
  Fixture 和 contract/security/migration snapshot。项目所有者已批准 Schema v5 的窄 immutable
  `template_plans` authority 与三个精确 Template Operation kind，并批准闭环通过后直接实施 production。
  Template capability、registry/import/export/derive/create-project、M4 frozen staging package adapter 与
  RPC/CLI 已真实接入。create-project 的六点真实 daemon kill/restart matrix 已复用原
  OperationId/Plan/idempotency 收敛成功；v3 `.alcomtemplate` parity 继续 blocked 到 M11。
- M5 Backup Create contract-first 与获批 production slice 已完成：`ALCOMD Backup Archive v1`、严格 `backup.json`、ZIP64
  Stored/Deflate、64 GiB/500,000 entries/32 GiB single file/128 GiB uncompressed/depth 128/path 1,024
  bytes/ratio 10,000:1 配额，以及 Logs/Obj/Temp、任意 `.git`、根 `Library*` 与唯一直接文件例外的精确
  排除表。locked VPM 排除不猜测、不联网，manifest 与 unknown/unlocked/embedded child 保留。
- State Schema v6 只增加严格 Operation kind `backups.create`，原子重建并保留 operation journal、
  idempotency、package plan/filesystem journal 与 template plan 外键依赖；现有 `backups` 表保持不变。
  Backup RPC/permission/error/recovery phase、dispatcher、official client、CLI `backup list/get/create` 与
  Operation worker 已真实接入。六点真实子进程 kill/restart matrix 复用原 OperationId、预分配 BackupId
  与幂等键并只产生一个最终 archive；`backups.m5-create` 已 implemented。
- M5 Backup Restore 已实现只恢复到不存在的全新 Unity Project、durable immutable
  Plan/Apply、预分配 ProjectId、`ProjectCreate(parent identity, leaf)` 锁、完整 Backup Archive v1 hostile
  validation、同卷 sibling staging/atomic publish 与 publish 后 forward recovery。State Schema v7 只增加
  `backups.restore`、窄 `backup_restore_plans` 和 Restore 专用 append-only filesystem journal；Template
  checkpoint 仍保持 Template-specific。`backups.restore.v1`、permissions/errors、dispatcher、worker、
  client 与 CLI `backup restore` 已发布；真实 kill/restart matrix、artifact/target race、同目标并发及
  publish 后外部修改 fail-closed 已通过，`backups.m5-restore` 为 implemented。完整 backups
  feature 保持 in progress，`backups.v3-parity` 继续 blocked 到 M11。
- CLI 高影响确认现在由窄本地 `CliError::ConfirmationRequired` 分类；非 TTY/EOF/拒绝以退出码 1 和
  `confirmation_required` 返回，不再被错误映射为 `daemon_unavailable`，且没有改动 RPC/daemon。
- M5 CLI 已完成与真实后端一致的 Operation/Project/Repository/Package/Unity/Template/Backup 命令树、
  alias、human/JSON/NDJSON envelope、stdout/stderr、usage/domain/interrupt 退出码、non-TTY/EOF、
  `--yes`/`--dry-run`/`--no-wait`、Operation follow/detach、broken-pipe 与不连接 daemon 的静态
  completion。`cli.command-contract`、`cli.non-tty` 与 `cli.help-output-exit` engineering tests 已实现；
  `cli.complete` 聚合 feature 仍因 M5 外 roadmap 命令保持 `in_progress`，不虚构为完整产品 CLI。
- M5 Restore/CLI 生产提交为 `50d76ba301ff9190f0d01d3c80f229e0a4c654d9`。本地 fmt、clippy、完整
  Workspace tests、xtask、metadata、baseline freeze、diff、`scripts/check.ps1`、`scripts/test.ps1` 与
  Tauri no-bundle 已通过；三份锁文件未变化，未新增 production dependency、ALCOMD unsafe 或平台 API。
- M5 最终候选 `48b188e3bf3f90c10b5cb3257365fa2ff8259faf` 对应 GitHub Actions run
  `32668802957`：Windows Server 2025、Ubuntu 22.04 与 macOS 15 arm64 全部成功；Ubuntu 实测最高
  `GLIBC_2.34`，九个 macOS 预期产物均为 arm64 / minos 11.0。项目所有者已确认最终人工验收通过，
  M5 正式完成；四项 v3 differential parity 仍 blocked，Windows 10/11 完整客户端发行验证仍 deferred
  到 M12。
- M6 ExecPlan 已按项目所有者附条件审阅冻结：每个 enabled ExtensionId 独占一个 Host OS process、无 ambient
  WASI authority、WIT type/function shape 默认 breaking、lease/grant revision linearization、desired/runtime
  state 分离、bounded key/value、scoped Project summary 第一能力、publisher trust 分层与 exact runtime limits。
- M6 Stop A contract-first 工件已形成 `.alcomdext`/Manifest/signature/archive profile、WIT ABI v1/compatibility
  fixture、permissions/scope/lease/revocation、State Schema v8 migration contract、RPC/error/hello optional contract、
  UI Bridge headless envelope、threat model、synthetic vectors 和永久 contract test。
- 项目所有者已批准 `wasmtime 48.0.0` LTS（defaults off，仅 async/component-model/cranelift/runtime/std）与
  `ed25519-dalek 3.0.0`（defaults off）；第一条 slice 不依赖 `wasmtime-wasi`。实际依赖已按批准配置落盘：Wasmtime
  只进入 `alcomd-extension-host` graph，Ed25519 只进入 `.alcomdext` 验签路径，没有新增 ALCOMD unsafe 或平台 API。
  State v8、版本化 RPC、Plan/Apply/recovery、permission/scope/lease/data、one-host-per-ExtensionId Component Host、
  UI Bridge headless envelope、crash/quarantine 与即时 revoke 已完成最小 production wiring。
- M6 真实测试已覆盖 Component/Host/daemon 生命周期、first-party/third-party 同权路径、scoped Project summary、
  bounded extension data、trap/fuel limit/quarantine、revoke-in-flight，以及 `archive_verified`、
  `staging_complete`、`package_published`、`state_committed`、`package_moved_to_backup` kill/restart matrix。
  最终 SHA `b666e8fcac6fd4153750c401b76f2f61f6d2a34a`（production commit
  `9fcd7c31dc1c5707f9490555e2b298d0fa28ca01`）对应 CI run `32724915827`：Windows Server 2025 成功，
  Ubuntu 22.04 成功且最高 `GLIBC_2.34`，macOS 15 arm64 成功且九个预期产物均为 arm64 / minos 11.0。
  项目所有者已确认最终人工验收通过，M6 正式完成；尚未开始 M7。
- M7 旧 WebView Stop A 曾形成 route/settings/typed adapter 与 iframe/child container proposal，但没有修改 production
  RPC、State Schema、Permission、Tauri capability、dependency 或 UI Bridge public contract。CI run `32752875840`
  的三平台 sandboxed iframe 均创建主 WebView但 extension document 超时且 custom handler 未触发，结论为
  `rejected_for_m7_v1`；这不是 private IPC security 成功或失败证明。
- Windows 本地 Tauri `2.11.5` / WebView2 `151.0.4129.101` managed child diagnostic 中，`Window::add_child`
  成功，但已知 App 页面没有 navigation/page-load/title callback，`Webview::url()` 返回
  `runtime error: failed to receive message from webview`，eval 没有 callback。分类为
  `child_webview_navigation_unavailable`，managed child 结论为 `rejected_for_m7_v1`；没有继续 custom protocol、
  Ubuntu/macOS child、WebviewWindow、direct Wry 或平台 API 研究。
- M7 基础 Extension UI 已在公开发布前重置为 GUI-neutral Portable UI：Extension Backend 经统一 Host、application
  和 RPC 暴露 semantic document，官方 React/Material Design 3 GUI 与第三方 GUI 使用各自 renderer。ADR 0024 只
  接受产品方向。Phase 0 commit `12089661f7c694f059aaf172344809ff814716f6` 已获人工验收；obsolete WebView probe
  已在独立 cleanup commit `c05c5d2c712dd7a791401efc22b8dd19b88023a6` 退役，历史 rejected evidence 与普通
  Tauri build/no-bundle gate 保留。
- M7 Portable UI Stop A与执行语义已通过最终人工审批；review closure把合同收敛为每extension至多一个隐式Portable UI，
  Manifest仅声明protocol，ABI v1强制guest-ui export，WIT只向guest暴露guest-session-id/locale/action，RPC冻结64项
  exact replay，State v9只增nullable ui_protocol。Slice B0 已完成 active Manifest/package/WIT direct replacement、production
  migration、typed Plan/record authority、client permission name与四个 RPC DTO foundation；旧 Web UI production contract已删除。
  Slice B1 已完成 memory-only Session/InvocationContext、guest-ui Host binding、interactive lifecycle、dual authority、render
  purity、current-only replay、validator、invalidation 与 dataSchema 9/capability advertising。真实 daemon/RPC/Host 测试覆盖
  UI-only/background 生命周期、render write denial、action write、cross-connection、disconnect、disable stale 与 malformed guest
  quarantine。Slice C official React/MD3 renderer 已通过共享 Fixture/authority 边界定向测试；Slice D 已完成独立
  non-Tauri headless consumer、hostile document/action、dual-authority/Host binding、replay/race/limits与payload redaction
  本地证据。完整本地 gate 已通过；完整 a11y、最终三平台验收与人工验收尚未完成，M7仍为进行中。
- Portable UI production candidate `aa1323430252ed21995284a7b36dd36e45a15e0a` 对应 GitHub Actions run
  `32877438910`：Windows Server 2025、Ubuntu 22.04 与 macOS 15 arm64 全部成功，Ubuntu 最高 `GLIBC_2.34`，
  macOS 九个预期产物均为 arm64 / minos 11.0。该结果只验收 B0-D，不关闭 M7。
- M7 completion audit 发现 Portable UI architecture reset 期间误删了 official GUI Core page/flow scope。项目所有者已批准
  恢复 M1-M7 当前 Core capability 的 Home、Project/Package/Repository/Template/Unity/Backup/Operation/Extension、Settings、
  Activity、Diagnostics 与 About surfaces，并批准 Config Schema v1、四个 RPC 与四个 permission。完整 coverage matrix 位于
  `docs/exec-plans/M7-gui-rpc-coverage.md`。
- GUI browser-level accessibility automation 唯一获批新增 `@playwright/test 1.62.1` devDependency 与匹配 Chromium；它不进入
  production bundle/runtime，不复活旧 WebView probe，也不替代 M12 的真实平台 screen-reader 验收。
- M7 Official GUI Completion E1-F3 已完成 M1-M7 GUI-relevant Core 页面与流程、typed Tauri/client/RPC adapter、
  Plan/Apply/Operation、Config Schema v1 durable settings、safe Activity/Diagnostics projection 和统一 loading/refreshing/empty/
  error/disconnected 状态。新增公共合同严格限于获批的四个 RPC 与四个 permission；没有 State DB 新表、generic RPC
  passthrough、新 Cargo dependency、unsafe 或平台 API。
- G1 的 Playwright 1.62.1 / Chromium revision 1234 browser suite 本地 12/12 通过，覆盖键盘与焦点、dialog/form/live region、
  320 CSS px、deterministic 200% layout、reduced motion、light/dark targeted contrast、17 个 Portable UI node 与 Core
  success/stale/failed/cancelled flows。完整本地 Rust/npm/Tauri/metadata/baseline/lock/authority gates 已通过，production bundle
  与 production npm graph 均不含 Playwright。这些继续是 functional/accessibility infrastructure evidence，不表示 visual design
  已通过。
- 项目所有者在正式 checklist 前实际查看 `19267230507071dc61ba306b98c8cfdd113e9ea2`，将其记录为
  `functional_implementation_candidate but rejected_for_visual_and_information_architecture_acceptance`；该候选不得 push。
  Core/RPC、Settings、Activity、Diagnostics、
  Portable UI、Plan/Apply 与 Playwright/a11y 证据不回滚。
- A-033 已冻结 official GUI 默认继承 v3 用户模型与可识别结构；daemon/RPC/Operation/Extension Host/Portable UI/permission/
  State 等内部边界不得自动成为新用户领域。A/B/C 已降为 historical exploration；minimum-change IA proposal 与H0/H1
  production已获批准，H1后停止Visual Gate 1。MD3只现代化component/interaction/visual language。
- 首次 H1 本地提交采用了 global top app bar、admin-style sidebar grouping、centered max-width page、hero typography 与低密度
  empty/form cards；项目所有者将其标记为 `rejected_for_visual_direction`。现改为 reference-driven implementation：先以 v3
  发布图和冻结 GUI 源码完成直接视觉拆解，再只重建 wide Projects prototype；功能/RPC/state/hooks/flows 证据不回滚。
- reference-driven wide Projects prototype 已由项目所有者在真实 release GUI 中通过 Visual Gate 1；检查点
  `7bb325b78a74518881fb65e03de7f79902e72cd3`。sidebar/main canvas、Projects toolbar、桌面密度及真实 Material Web
  Button/TextField/Select 的 interaction、ripple/state layer 与 keyboard focus 均取得人工证据，但不冒充整个 M7 visual 完成。
- M7 Project Actions Functional Closure Stop A 已完成 P0-P4 本地候选：exact `open 5.4.2` 与
  `tauri-plugin-dialog 2.7.2`、closed ProjectId opener/native directory picker、active `projects.copy.v1`、State Schema v10、
  durable JSONL inventory、两遍 SHA-256 source consistency、sibling staging/forward recovery，以及 CLI/Projects list/grid/
  workspace Plan/Apply flow。真实子进程 kill/restart matrix 覆盖 inventory_ready、staging、staging_complete、publish_intent、
  target_published、project_registry_commit_intent、state_committed 与 cleanup_complete；重启复用原 PlanId、OperationId、
  idempotency 和预分配 target ProjectId。P5-P8 与 H2 visual 继续暂停，聚合 `projects.management`/`packages.vpm` 不提升。
- P0-P4 本地验收已通过 Rust fmt/strict Clippy/完整 locked Workspace tests、npm check/build、19 项 browser tests、xtask、
  metadata、baseline freeze、diff check 与 Tauri release no-bundle build；同 target 并发只发布一次，Plan expiry 与四态 writer
  gate 也有直接回归证据。最终 checkpoint `287790b3e3ef1855a6b7cea92c476b46873d2ca9` 的 Hosted CI run
  `33175602728`、CodeQL run `33175602645` 与项目所有者人工验收均已通过。
- P5-A Projects Create / Restore 已复用现有 Template / Backup typed RPC、Plan / Apply / Operation、native directory chooser 和
  Projects registry 形成最小 GUI 垂直切片；成功身份来自 Core Operation/Apply 结果，完成后刷新列表并进入新 Project workspace。
  Create/Restore 的选择、取消、chooser、名称、Plan review、Apply、Operation、成功/失败、空备份和新注册项目可见性已有 3 项
  browser tests；完整 22 项 Playwright suite、Rust fmt/strict Clippy/完整 locked Workspace tests、npm check/build、Tauri release
  no-bundle、xtask、metadata、baseline freeze 与 diff check 均已本地通过。未新增 RPC、State、Permission、dependency、unsafe
  或平台 API。commit `12638e7bdc5ba10ea9e9438b8dbc209a3993cafc` 的 Hosted CI run `33189137494` 在 Windows、
  Ubuntu、macOS 全部成功，CodeQL run `33189136465` 全部成功；Ubuntu最高 `GLIBC_2.34`，macOS 9 个预期产物均为
  arm64 / minos 11.0。聚合 feature 状态不提升。
- P5-B Favorite/Clear Unity Preference 已完成精确 v3/current-v4 审计、修订合同、State v11 migration、active RPC/typed client/
  Tauri/official GUI 与本地完整验收。Favorite复用 `projects.manage`和 `projects.registry.v1`，实现 exact Project CAS、no-op/
  replay/Event、refresh保留、完整分页 stable favorite-first 与 list/grid toggle；Unity复用 `unity.read/manage`，实现 automatic/
  explicit selection、保留 arguments的 clear、legacy get/set compatibility、0/1/multiple automatic launch与 v1/v2 idempotency。
  active daemon广告 `dataSchema: 11`；无新增 dependency、Permission、capability、table、Operation、ResourceKey、unsafe或平台 API。
  最终候选 `1fc139b8fd47572d52e8cf468a247195e61091f4` 的 Hosted CI run `33209167919` 已在 Windows Server 2025、
  Ubuntu 22.04 与 macOS 15 arm64 全部成功，CodeQL run `33209165247` 四个分析 job 全部成功；Ubuntu最高
  `GLIBC_2.34`，macOS 9 个预期产物均为 arm64 / minos 11.0，三平台 Official GUI 26 项 Playwright suite 均通过。
  聚合 feature不提升。
- P6-A Package workspace 已复用 existing repository typed client 完成完整分页读取、顺序逐仓库 refresh、partial failure、
  `revision_conflict` 单次 fresh retry、最终 catalog reload 与 GUI-only All/Remote/Local source filter。browser regression 覆盖零/单/多
  仓库、部分失败、不虚假成功及 source kind presentation；无新增 RPC、Capability、Permission、State、Config、dependency、unsafe
  或平台 API。测试热修复 commit `bb934ef94ad57f26cb40a6d718fc46181a4915f4` 对应 Hosted CI run
  `33264281642`：Windows Server 2025、Ubuntu 22.04 与 macOS 15 arm64 全部成功；CodeQL run `33264281469`
  的四类分析全部成功。热修复只修改 test daemon harness，production 5 秒启动窗口与 IPC 未变；该远端检查点取得时，
  P6-B/P6-C 尚只完成获附修正批准的 contract-first Stop A。
- P6-B/P6-C 已实现 Config Schema 2、State Schema 12、package metadata links、`packages.plan.v2`、Reinstall、
  atomic Bulk、User Package loose-directory registry/deterministic owned cache、v1/v2 resolver隔离与official GUI用户入口。
  Targeted Rust suites、TypeScript、Vite build与32项Chromium Playwright均已通过；owned cache缺失明确验证不回读mutable source。
  最终候选 `c7477089571246376ab4f90d106b860d9a8e98cd` 的 Hosted CI run `33277180223` 已在 Windows Server 2025、
  Ubuntu 22.04 与 macOS 15 arm64 全部成功，CodeQL run `33277179920` 四类分析全部成功；Ubuntu最高 `GLIBC_2.34`，
  macOS 9 个预期产物均为 arm64 / minos 11.0。P6-A/P6-B/P6-C 均为 PASS；聚合 `packages.vpm`/
  `repositories.management`继续保持`in_progress`；该P6切片没有新增production dependency、Permission、unsafe或平台API。
- P7 Delete Project Directory 已按获批合同形成并通过远端验收：active `projects.delete.v1`、独立
  `projects.delete`、State v13、ProjectId-only Plan/Apply、sibling quarantine永久删除、mount-safe cleanup、forward recovery、
  CLI/GUI入口及 accepted 到 cleanup_complete 的真实 kill/restart矩阵均已接线。未新增dependency、unsafe或Windows API；
  sealed HEAD `2ee11066c07a0994f3aebe6a9ce3f84ab2c8acd9` 已通过 Hosted CI run `33298022030` 的 Windows、
  Ubuntu、macOS 与 CodeQL run `33298021806`；P7 remote checkpoint 为 PASS。`projects.management`仍保持`in_progress`。
- P8 使用唯一 `visible-action-completeness-v1.json` 清单核对 92 个 action：88 个 M7-owned action 中 33 个为
  `implemented`、55 个只按可观察状态或未协商 capability 条件禁用，永久 fake/placeholder 为 0；4 个 M11-owned
  VCC/legacy/differential parity action 继续作为全局 blocker。GUI 已消费 `system.status.capabilities`，缺失 capability 时不下发
  对应受保护 RPC；typed client/closed adapter、Delete/Remove 双动作及键盘确认边界均通过本地 Rust、34 项 Playwright、npm
  build 与 Tauri release no-bundle 验收。P8 historical technical checkpoint 已在 sealed functional HEAD `d314b155…`
  通过。新的 Unity product model 已移除 Automatic/Preferred/Clear Editor Preference，并加入独立 Launch Config、Project
  Unity migration selector、exact Open Unity 0/1/>1、one-shot selection、Plan/Apply/Operation 与 migrate-and-open；更新后的
  P8 verdict 为 `M7-owned PASS`，global 继续 `BLOCKED_BY_M11`。
- Project Unity Version / exact Open Unity U1-U7 production closure 已通过项目所有者技术验收：sealed HEAD
  `e9349fd0f4cc9c18d9174dfc5826d48654aeb6f5` 的 Hosted CI run `33352406804` 在 Windows Server 2025、
  Ubuntu 22.04 与 macOS 15 arm64 全部成功，CodeQL run `33352406214` 成功。State v14、Launch Config、
  one-shot exact launch、migration Plan/Apply/Operation、private preparation/recovery 与 official GUI flow 均已接线；
  H2-A 的 Unity-model pause 已解除并恢复 Visual Gate 2，M7 仍未完成。

## 后续里程碑尚未完成

- v3.4.0 完整安装后快照、脱敏迁移 Fixture 和 GUI 冻结截图/流程已由项目所有者明确后移到
  M11；M-1 仅保留 VM 操作报告，不把它升级为实例证据或删除授权。
- MCP 33 个 v3 用例的 M-1 工具合同基线已形成并获 A-026 批准；正式 Schema、快照、兼容
  别名策略和协议实现留在对应后续里程碑。
- VPM、项目、模板与备份。
- M7 Official GUI H0-H7 realignment、四个分阶段 Visual Gate、一次交互桌面最终视觉/键盘签收、最终候选三平台 Hosted CI 与
  最终人工验收；既有 functional/browser evidence 和 B0-D Hosted-green candidate 均不冒充 visual/M7 acceptance。
- MCP 实现。
- Discord IPC。
- v3 迁移与 Bootstrap。
- 安装器、签名与发布。

## 当前阻塞与缺口

- Windows 10 22H2 与 Windows 11 仍是正式目标支持平台，但真实客户端运行验证尚未完成，
  不得记为通过。项目所有者已将仅重复编译的 M0 self-hosted job 取消，并把验证 deferred 到
  M12：届时必须安装并启动完整产品，覆盖 WebView2 渲染、托盘、注册表、用户数据路径、
  更新器、安装器、升级与卸载。
- GitHub 已宣布 `ubuntu-22.04` hosted runner 从 2026-09-17 开始弃用并于 2027-04-17 退役；
  当前 M0 仍使用该构建基线，未来替代不能直接用 Ubuntu 24.04 冒充 Ubuntu 22.04 /
  `GLIBC_2.35` 等价验证。
- M7 WebView-based Extension UI direction 已在 production 前拒绝，不再把 container/physical mapping、custom
  scheme/CSP 或三平台 WebView isolation matrix 作为产品 blocker。Portable UI production 已获批准且 B0/B1 已完成；
  Slice C official renderer、Slice D headless/security/fault conformance 与 Hosted CI 已通过；restored Official GUI Core
  surfaces/flows、Settings/Activity/Diagnostics contracts 与 Chromium browser-level accessibility 已完成本地实现和验收。
  当前 H2-A Projects 列表与导航的本地视觉迭代已获项目所有者局部认可；Unity production closure 通过后，
  H2-A 已恢复为 `ACTIVE / VISUAL_GATE_2`。`24b03916…` 只保留为 pre-Unity-model local checkpoint，
  不是 rejected，也不是当前候选。Project Copy P0-P4 remote checkpoint、P5-A Create/Restore、P5-B historical
  preference closure、P6 package closure、P7 Delete Project Directory 与更新后的 P8 M7-owned completeness
  均已通过对应验收；完整 Visual Gate 2、
  后续 H2-H7、最终候选三平台 Hosted CI 与项目所有者最终人工验收仍是实际缺口。

- 真实安装快照和迁移 Fixture 尚未建立；因此 artifact 模板继续保持 `confirmed = false`，
  迁移删除、GUI/模板/备份/Unity 差异测试仍 blocked。项目所有者已决定不在 M-1 继续投入
  合成状态与安装后采集，完整证据后移到 M11；这不再阻塞 M-1 的其余静态审计收尾。
- v4 bridge 尚无版本/tag/资产、handoff、journal、恢复源、Health Check、Commit marker、DEB
  交接和 rollout 设计。M-1 已冻结输入与失败契约；实现/发布属于 M11，不能在本阶段伪造。
- A-017 至 A-026 已批准发布平台范围、Local API/SDK 后置、第一方扩展默认状态、GUI 等价
  边界、MCP Operation 映射、Extension ABI v1、MCP 管理权限、全产品打包模型与平台技术
  基线和 MCP 工具/诊断/错误方向。O-008 已被 A-024/ADR 0015 替代，O-003 已由 A-025 关闭。
- Tasks SEP 已 Final，但扩展 artifact 仍带 Draft/experimental 标记，且所审错误码与最终 core
  Schema 冲突；在固定兼容版本前保持阻塞。
- `specs/extensions/permissions-v1.md` 已在 M6 Stop A 应用 A-023 并明确拒绝 `mcp.sessions.read`；
  `specs/mcp/toolset-v1.md` 的 A-021/A-023 正式 Schema/快照更新仍属于 M8，不能由 M6 提前发布。
- `alcomd-mcp`、GUI 与其他后续入口仍是 scaffold；M1 只实现 daemon/client/CLI 的最小 status
  切片，不得将其描述为完整 RPC、CLI、Operation 或业务功能。
- `projects.v3-parity` 继续保持 `blocked`，直到 M11 建立脱敏真实 v3 Fixture；synthetic/public
  Fixture 只能证明独立实现的格式与工程行为，不能冒充真实 v3 differential evidence。
- 真实 credential enrollment/revocation 仍未完成；完整 `projects.management`、
  `repositories.management`、`daemon.single-writer`、`rpc.local` 与 `cli.complete` 仍只实现了各自
  的阶段性切片，必须保持真实的 `in_progress`/`planned` 状态。
- `templates.v3-parity`、`backups.v3-parity` 与 `unity.v3-vrc-parity` 继续 blocked；M5 Unity engineering
  测试只证明 synthetic Editor、进程观察和 RPC/migration 行为，不证明真实 Unity/Hub、模板、备份
  或 v3 differential parity。
- M4 已批准并精确锁定 `semver 1.0.28`、`zip 8.6.0`、`sha2 0.11.0` 与
  `unicode-normalization 0.1.25`；normalization 许可证记录保持
  `(MIT OR Apache-2.0) AND Unicode-3.0`。`semver` 只提供 Version/precedence，冻结 VPM range 语义由
  `alcomd-vpm` 私有最小 parser/matcher 实现；`nodejs-semver` 与 `node-semver` 未进入生产依赖。

## 下一停止点

M0、M1、M2、M3、M4、M5 与 M6 均已完成并通过最终人工验收。M7 Portable UI B0-D 已通过 Hosted CI；Official GUI
functional candidate `192672…` 保留技术证据但已被拒绝 visual/IA acceptance；首次 H1 也已被标记为
`rejected_for_visual_direction`。替代的 v3-reference wide Projects shell 已在 `7bb325b…` 通过 Visual Gate 1。当前只完成
Project / Package Functional Closure P0-P4 remote checkpoint、P5-A Create/Restore 与 P5-B Favorite/Clear Unity Preference
均已远端通过。P6-A/P6-B/P6-C 也已通过同一最终候选的三平台 Hosted CI 与 CodeQL；P6 remote checkpoint 为 PASS。
P7 Delete Project Directory 已通过 sealed HEAD 的三平台 Hosted CI 与 CodeQL，remote checkpoint 为 PASS。
当前唯一获准工作是 H2-A Project Workspace / Packages Visual Gate 2；Unity technical closure 已在 sealed HEAD
`e9349fd0f4cc9c18d9174dfc5826d48654aeb6f5` 通过。P8 M7-owned completeness 为 `PASS`。M11 VCC
Import/Migrate、legacy entry 与真实 v3 differential parity 继续是 `BLOCKED_BY_M11`，不计入 M7-owned completeness 分母。
H2-B、H3-H7、M8、M9 与 M11 production 不得开始；H2-A 形成新的 real Tauri Visual Gate 2 候选后必须停止等待人工视觉验收。
M4 完整
VPM 产品功能以外的未完成范围继续按 feature/test 元数据推进，不因里程碑验收而虚构为 implemented。
`projects.v3-parity` 与真实 credential
revocation 仍未完成；Windows 10/11 完整客户端
安装、启动、WebView2、更新与卸载验证继续 deferred 到 M12，不得把 Windows Server hosted 结果
描述为客户端发行证据。
