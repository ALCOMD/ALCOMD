# M7 H2-A：主题弹窗与显式保存

当前普通设置已按所有者要求修改即保存，页面总 Save／Discard／已保存栏移除；本文的独立
主题 Save／Cancel 继续保持。普通设置与主题使用同一串行 CAS 协调，普通已保存值不被主题
覆盖，失败及跨页行为见 m7-settings-autosave-20261030.md。旧普通草稿的检查点只作历史结果，
当前不再保留普通设置草稿确认入口。

## 弹窗整体左右留白 — 2026-10-31

宽弹窗的公共内容盒是 800px content-box，Material 自己另提供左右各 24px；主题表单曾再次
固定为 752px，留下额外 48px 右侧空白。现在主题表单 width:100%，填满公共内容盒，不再
重复扣除内边距或设置一套页面宽度；公共 Dialog 及 Material 默认 padding 保持。

新增实际 dialog 与左右列的几何比较：修复前明暗两项均以差值 48px 失败；同一断言修复后
通过（左右差不超过 1px），连同 footer 边界、键盘及三语布局共 6/6。check/build、diff check
通过，明暗／中文完整窗口已目视核对，证据 target/m7-theme-dialog-insets-20261031/。
既定卡片 24px 内边距、28px 圆角、列间距及保存行为保持；当前 dev 热更新，未提交／推送，
未新增真实桌面或屏幕阅读器签收，Visual Gate 2 仍 PENDING。

## 主题卡片内边距 — 2026-10-31

这些 theme-panel 是自定义容器，没有 Material 卡片组件可继承的默认内边距；按所有者要求，
主题内三块卡片由同一规则统一四周 24px。只改原 padding:16px，不调整已确认的 28px 圆角、
分栏及按钮／滑块库内间距。check/build、明暗与三语几何回归 6/6、diff check 通过；证据在
 target/m7-theme-card-padding-20261031/，当前 dev 热更新，未提交／推送，Visual Gate 2 仍 PENDING。

## 主题文案与数值展示 — 2026-10-31

按所有者修正，源色仅保留输入框的可访问标签，移除“选择整个应用使用的强调色”及闲置词条。
恢复按钮三语使用 Restore defaults／恢复默认／既定値に戻す，仍只修改主题三字段草稿，Save
才持久化，Cancel 保持原值。HCT 删除常驻 output，由公共 Slider 的 labeled 选项启用库内置
浮动提示；显隐、焦点、键盘和拖动由 Material 自身处理，无自定义数字浮层或计时逻辑。
CSS 仅删除旧 caption flex 和 output 数字规则，保留标签原字号及滑块布局。

check/build、815 条词条校验、合同 JSON 解析和 diff check 通过。初轮主题组 24/27，失败为
新数值断言未处理库包装空白及两项旧中日按钮文案期望。后续数值提示复核曾因新加的方向键
实际改变颜色而污染原红色回退前提；将键盘步骤移到原无效 HEX 校验之后，保留全部原颜色、
无写入和回退断言。最终定向复核 **4/4**（verified.log，退出码 0），覆盖键盘浮层显隐与实际
输入值一致、HEX／拾色器／HCT 联动、英中日重置标签及设置可访问名称／保存链路；不是同轮
整组 27/27。固定行号选择曾漏选动态三语用例，最终按实际测试声明行选择，4 项运行记录可核对。

完整中文静态弹窗与键盘操作浮层截图已目视核对，位于 target/m7-theme-copy-20261031/。
它们是 browser fixture 证据，不冒充原生桌面或屏幕阅读器签收。当前 dev 热更新，未重启
原生程序／daemon，未提交／推送，Visual Gate 2 保持 PENDING。

## 主题入口

按所有者要求，Appearance 标题右侧的 Theme 入口使用与 Create project 相同的公共 Filled
主按钮，并在 icon slot 放调色盘图标。沿用共享按钮的高度、图标尺寸及内边距，没有增加
页面 CSS 覆盖；按钮名称、开启编辑器、焦点和 Save／Cancel 不变，保留此前其他 WIP。
新增 palette 24px SVG 来自现有冻结上游提交，Rounded／400／grade 0／fill 0，未修改几何；
manifest 记录原路径和 SHA-256，既有 Apache-2.0 许可清单继续覆盖，不新增依赖或字体。

`target/m7-theme-entry-icon-20261030/`：check/build、31 项精确图标资产校验和 diff check
通过，既有主题展开／键盘／三语尺寸回归 **6/6**（退出码 0）。明暗完整窗口截图已目视核对，
图标及主按钮清晰可见；它们是浏览器 fixture 证据，不替代真实 Tauri 或屏幕阅读器签收。
当前 1420 dev 提供新模块，未重建原生程序／重启 daemon，未提交／推送，Visual Gate 2 仍 PENDING。

## 公共分段组件的尺寸预留

Material Web 2.5.0 的实验分段组使用等分 grid，而未选中项的勾选区宽度为零；中文按字换行
使“跟随系统”在选中后变成两行。修复前浏览器真实 Range 返回两行，回归失败。
公共 SegmentedButtons 现在由不可交互、aria-hidden 的标签尺寸层参与 intrinsic grid 计算，
按最长标签在所有选中态所需宽度提前预留，保留库的标准可见控件及文字单行。尺寸层复用
同一字体／字号／字重 token，12px 两端、18px 默认图标与 8px 间隙对应当前已安装 Material
版本；不按语言或页面写宽度，字体及标签变化由 CSS 自动重算。升级 Material 时应复核这些
库尺寸及三语向量。主题右列只改为尊重子组件 min-content，不增加页面专用按钮尺寸。

英／中／日均逐项选中，检查实际文本 Range 为单行、文字不超出段、组宽高不随选项变化、
分段不超出父列；加上明暗完整窗口、键盘与勾选共六项通过。资源与日志共用导航及中文
最终截图复核 **3/3**。默认英语未变时不应点击禁用 Save，以及 Theme 已移到 Appearance
按钮、不再是独立分区，两项测试假设已按既有有效行为修正，未修改产品来满足旧断言。
check/build、diff check 通过，日志在 target/m7-segment-sizing-20261030/；中文完整窗口
chinese-system-selected.png 已目视核对。仅前端共享组件／布局约束及文档，无新依赖、
RPC 或持久字段。当前 dev 热更新，Visual Gate 2 与屏幕阅读器验收仍未取得新增签收。

## 保存期间的几何稳定性

移除保存期间临时插入的“正在保存主题…”段落。该段落与 form 间距改变内容高度，
导致居中弹窗在保存开始时向上移动 **32px**。当前 Save 保持原动作名称，不切换为 Saving…，
也不预留瞬时文案；禁用按钮及 form aria-busy 表达请求中，不新增可见提示行。
错误仍明确呈现；成功重试保留错误区域直到弹窗关闭，避免先缩回再播放关闭动画。
已完成的提交通过 finished／dirty 门控禁止再次保存。

修复前几何断言实际失败（dialog y 变化 32px），修复后 **6/6**：dialog／footer 的
x、y、宽、高在保存期间变化均不超过 1px，原单次提交／忙时禁用、防重复、Cancel／Escape、
失败重试、失败取消、CAS 冲突行为保持。首次测试辅助代码误等待了滚动条无限观察动画，
改为结束有限开场动画后测量；该超时与真实 32px 复现分别保留，不混作功能结果。
日志在 target/m7-theme-save-stability-20261030/，check/build 和 diff check 通过。
只修改前端与 presentation 描述，不改后端或持久合同；当前 dev 接收热更新，
未提交／推送，Visual Gate 2 仍 PENDING，未新增真实桌面或屏幕阅读器证据。

当前短请求审计及最新回归见 [瞬时状态清理](m7-quiet-feedback-20261030.md)；上文 6/6 保留为历史检查点。

## 当前展开布局

参考图只定义左右展开的信息布局，控件不按图复刻。左侧为标准描边 HEX 字段与纵向
Material HCT 滑块，右侧为“明亮／跟随系统／暗黑”标准描边文字分段和九方案 Material Radio 双列组。
主题分段显示 Material 自带的选中勾选标记，不使用太阳／系统／月亮图标；公共组件通过
showCheckmark 显式选择是否显示，其他已有分段组保持原行为。
文字分段专项与布局／单选／保存取消合计 **5/5**，构建通过；日志和完整窗口／弹窗截图在
target/m7-theme-segments-20261030/。已目视确认选中项同时显示勾选和文字，没有模式图标。
已移除仿图的 tonal 胶囊覆盖、HEX 胶囊字段、色带和弹窗底色覆盖，使用已安装 Material Web
自身的边框、选中色、滑块轨道和焦点样式。卡片复用现有低容器表面色，让完整轨道可见。
全局 28px 卡片、Noto 字体、种子色为所有者已确认的产品定制，不宣称是 MD3 默认值。
Source color 保留可访问标签，移除多余 supporting text；Restore defaults／Cancel／Save 的三字段确认合同不变。

当前 Material 外观修正的布局／键盘／原设置 **4/4**、前端 check/build 通过，
日志和明暗完整窗口／弹窗截图在 target/m7-theme-md3-20261030/，文件名为
theme-light-window.png、theme-dark-window.png、theme-light-dialog.png、theme-dark-dialog.png。
断言保持九项可见、单选／重复点击、零写草稿、完整 footer、真实保存／取消与可访问说明；
没有增加依赖或 RPC／Config 改动。原 Tauri dev 接收前端热更新，无新增桌面签收。

### 历史展开布局检查点

最新验证：主题／动画组合有 **30 项通过、2 项布局测试定位失败**，后二项将 slotted 内容
误从 Shadow DOM 内的 dialog 查找；改为 host 查询内容、真实 dialog 截图。随后最终布局／
键盘／既有设置组合 **4/4**，退出码 0。截图复核发现 Material 默认 560px 高度会裁切底部
按钮，已让主题弹窗在视口减 48px 内按内容增高，并用实际 dialog 边界验证整个 footer；
最终明暗完整窗口均已目视核对。早期测试还遇到 ElementInternals 状态不被 Playwright
DOM checked/role helper 读取，使用组件真实状态与键盘行为断言，没有添加伪 ARIA 来通过测试。
外部 aria-describedby 无法穿透文本框的 Shadow DOM，已保留支持的 supportingText，
原 accessible-description 断言保持并通过。最初两轮失败、无绘制尺寸 host 截图等待与
修复后的日志均在 target/m7-theme-expanded-20261030/，不将这些批次写成完整 GUI suite PASS。

历史完整窗口／弹窗截图保留于该目录；当前外观复核见根目录 design-qa.md。
只修改前端与对应文档／机器合同的 presentation，不改 RPC、Config 或生产依赖；
当前 Tauri dev 接收热更新，不重跑 Rust Workspace、不重建未改后端。
没有新增真实桌面或屏幕阅读器证据，Visual Gate 2 仍 PENDING，未提交／推送。

## 显式保存检查点

项目所有者修正保存交互：外观中只放一个“主题”按钮，点击后打开弹窗；调色实时预览，明确 Save 才持久化，Cancel 恢复已保存主题。本轮在标准 main、HEAD 7d938cf 上保留既有全部 WIP 实施，未提交／推送，Visual Gate 2 仍 PENDING。

## 有效行为

- 移除设置页独立 Theme section 和完整内联控件，将入口放到 Appearance 标题栏右侧，保持与标题同行；语言选择器增加独立可见标签并右对齐，避免“跟随系统”被误认为主题模式；复用宽 Material UtilityActionDialog，九方案／HEX／拾色器／HCT 控件和默认色保持。
- 打开与编辑、切模式／方案、HCT、拾色器、重置都不调用 settings.update。重置只修改草稿；保存仅一次更新三个主题字段。弹窗 form 阻止事件向外观主表单传播，不会顺带保存其他设置。
- 移除旧自动保存、同步意图合并和后台待写队列。Save 使用即时 busyRef 防重入，保存时编辑／Cancel／Escape／背景关闭均受阻；成功后关闭并在入口显示 Theme saved，焦点回到入口。
- Cancel／Escape／关闭／确认外部导航丢弃预览，恢复 App 当前已接受权威，不进行新写入。失败保留可编辑草稿与明确错误；可 Cancel 或显式 Retry，Retry 前读取当前权威并做三方合并，保留其他客户端未被本地修改的字段。
- App 已有连接／revision 门控继续防止迟到 shell 读覆盖预览／新结果，主设置的 locale、packages、density、motion 草稿不被主题保存或取消带走。

没有修改 Rust、RPC、Config/State Schema、权限或依赖。仍是 Config 3 与同一个 settings.get/update，原先 432 passed／8 既有 subprocess ignored 是后端历史验证，本轮不冒充重新运行完整 Workspace。

## 本轮验证

npm run check／build、cargo xtask check、metadata validation 与 diff check 通过（退出码 0），保留既有大 chunk 提示。浏览器组合 **30/30** 通过，退出码 **0**，包括 20 项主题与既有设置／多语言回归。断言入口位于 Appearance、设置页不占用主题面板空间、18种 scheme/mode 组合、编辑零写、一次确认仅一次三字段请求、自然双击与额外 submit 防重入、保存忙期间禁止关闭、Cancel／Escape／重开和焦点、失败重试／失败后取消、CAS 三方合并、其他草稿、迟到读取、外部导航与三语标签。

日志保存在 target/m7-theme-dialog-20261030/。继续使用当前隔离 Tauri dev（前端 HMR），原 daemon 与原生程序未改变，没有对真实项目执行 Apply／迁移／复制／删除。真实屏幕阅读器、原生颜色对话框操作与 Visual Gate 2 仍待项目所有者验收；窗口记忆开关尚未实施。


只读确认现有 GUI PID 29388 响应正常，daemon PID 48368 保持原隔离目录；通过 Tauri 实际配置的 `http://localhost:1420` 获取模块，确认新入口与显式 Save 已提供、自动队列不存在。没有重建或替换未修改的 Rust 程序。


## 外观区排版复核

根据所有者当前截图修正入口实际布局：Theme 不再独占设置行，位于 Appearance 标题栏右侧；Language 有清晰可见标签，选择器靠右。保存确认状态在入口前显示，入口右边缘不随提示出现而移动。公共 settings-row 统一标签／控件的水平排列，不改变动画与紧凑模式行为、弹窗保存合同、圆角或图标。

本次相关回归 **6/6**，退出码 0；新增几何断言验证标题和 Theme 入口确实同行、语言标签与选择器同行且互不覆盖、保存状态不挤动入口，连同取消／焦点、三语与其他草稿保护。npm build（含前端 check）通过，保留大 chunk 提示，日志在 target/m7-appearance-review-20261030/。修正过一次验证日志路径错误，该次没有启动测试，只有实际完成的一轮记为通过。

使用 Playwright CLI 在独立浏览器隔离 fixture 中完成中文暗色 1400×800 目视核对，并检查 Theme 按钮实际打开弹窗；截图为 output/playwright/appearance-current.png 与 theme-dialog-current.png。它们是浏览器渲染证据，不冒充真实 Tauri 视觉签收。当前 Tauri dev 使用同一源码热更新；独立截图浏览器已关闭，不影响用户窗口。
