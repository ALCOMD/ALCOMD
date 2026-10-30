# M7 H2-A：官方 GUI 三语言

项目所有者要求沿用冻结 v3 翻译，并明确本轮先完成已有英语、简体中文和日语。当前阶段仍为 H2-A，Visual Gate 2 **PENDING**。本轮在标准目录 `main`、`7d938cf` 基线上实施，未提交／推送。

## 当前任务名称与系统选项

任务导航、页面标题、详情返回及活动数量提示统一为 Tasks／任务／タスク；复用已有 Tasks
词条，移除 Task Center。语言、动画和主题模式共用 System／跟随系统／システム設定，
删除 Follow system／System language 重复词条，不改 system 持久值或系统语言匹配行为。
仅修改文案、调用方及对应旧回归期望，不改图标、CSS、RPC 或设置结构。

本轮现有三语路由／切回英语、插值与词条、动画、主题三语尺寸回归 **20/20**（退出码 0），
check/build、diff check 通过；816 条双语消息校验通过，保留既有大 chunk 警告。
证据在 `target/m7-label-consistency-20261030/`。1420 dev 模块 HTTP 200，已提供 Tasks 和
统一 System 调用，无旧标签。未提交／推送，未新增真实桌面／屏幕阅读器签收，
Visual Gate 2 仍 PENDING；前次两项列表长文本行高缺口不属于本次文字修正。

## 实现及边界

项目时间的用户语义与翻译覆盖分别验收：`ProjectSnapshot.lastModifiedAtMs` 由独立 Core 记录提供，列表、卡片与排序均使用该字段。项目通用检查时间已按所有者指令移除，不提供其他时间替代最后修改。v3 对应规则、最后修改时间权威边界及验证见 [项目最后修改时间](m7-project-last-modified-20261030.md)，当前 State 16 的字段删除与独立验证见 [项目观察时间移除](m7-remove-project-observed-20261030.md)。本文的三语言专项不替代功能验证，也不据此宣称整个 Project 用户模型或 Visual Gate 2 已完成。

- 复用现有 `settings.get/update`、三种语言枚举及 daemon 持久化；保存成功后切换，放弃草稿不改变应用语言。跟随系统支持语言及区域标签，未支持语言采用英语。HTML `lang` 与实际 locale 同步，包括设置初读失败时的系统语言。
- 使用独立编写的 React locale context、显式 `t` 调用、静态词条和 React 节点插值。无新依赖、HTML 解释、DOM 文本替换、GUI 直写配置或运行时 v3 访问。共享组件只翻译自己持有的默认文案，不自动翻译调用方／扩展提供的任意文字。
- 词条按 shell、页面、动作、表单组织，当前共 **816** 条源消息，各有中日文。重复公共文案统一在共享词条中，不以资源覆盖顺序选择译文。`check-locales.mjs` 纳入 `npm run check`，检查缺项、重复冲突、占位符、显式 JSX 文案及已知枚举标签映射。
- 只读核对 `../ALCOMD3-v3-readonly/vrc-get-gui/locales/{en,zh_hans,ja}.json5`，选择对应用户文案，未复制其翻译实现或整体导入旧词库。初始资源有 116 条源文案可按英文直接对应，其他对应术语经语义核对；v4 独有候选证据、Plan Review、权限与错误另行补译。日语 Resources 沿用 v3 的「パッケージ&テンプレート」，中文 Packages 为「软件包」，Open Unity 为「打开 Unity」／「Unityを開く」。
- 覆盖主导航、主／详情页面、公共控件默认提示、包版本／来源入口与候选原因、单包／批量确认、Unity 启动、设置、创建／恢复、任务、扩展宿主界面及日志。日期与相对时间跟随 locale；技术日志时间继续明确 UTC。恢复选择器日期由原 ISO 显示改为当前语言日期，英语为 `11/14/2023`。
- 项目名、软件包名、来源名、路径、版本、ID、机器错误码和扩展内容保持原值。活动记录的已知操作／状态显示可本地化，原 summary code 仍可查看；未知值沿用既有技术展示。诊断只本地化当前 daemon 唯一固定安全摘要，未知摘要不猜测、不替换。
- 不改变 RPC、权限、Config/State Schema、resolver、来源规则、Plan/Apply 或操作状态。无 Rust、依赖锁文件、图标、CSS 或窄窗口适配修改。

## 验证

最终 `npm run check`、`npm run build`、词条边界与 `git diff --check` 通过。构建保留既有大 chunk 警告；三语言资源使主 JS 增大，未借此扩展到构建拆包重构。

浏览器证据分组记录，不合称某次全套全部通过：

1. 新三语言页面专项修正测试作用域后 **4/4**，退出码 0。首轮 1/4 的失败为隐藏对话框重复内容及 slotted textbox 定位，未修改生产验收条件。
2. 第一轮整合 **40/44**，退出码 1；Projects preferences 整个文件的 18 项通过。失败包括旧 Backups 独立按钮断言、英语状态大小写和日志展示变化。保留既有英语状态／摘要格式并增加本地化；旧 Backups 断言改为当前已批准的更多菜单入口。
3. 第二组 **44/48**，退出码 1；候选／批量 17 项全部通过，包括来源、完整选择集、配额、竞态、稳定版及单 Bulk Plan。剩余失败为搜索控件定位／菜单关闭边界、两项新增用例仍期待英文固定诊断摘要，以及恢复日期仍期待旧 ISO 格式。搜索改用精确 searchbox，并断言焦点恢复及输入值；诊断与日期断言按实际本地化行为修正，没有改生产搜索或操作行为。
4. 最终受影响组 **17/17 PASS**，退出码 0：中日设置保存、侧栏／标题／控件、创建取消、候选与 Plan Review 取消；两种语言各覆盖现有九个实用路由并切回英语；系统语言匹配、全部词条占位符及外部值插值；日志筛选／真实 fixture Operation Review、终态、项目包搜索及创建／恢复。没有把分组复测写为完整 GUI suite 同轮通过。

`m7-localization.spec.ts` 验证真实设置 UI 保存后换语言；`m7-localization-routes.spec.ts` 通过显式 history/popstate fixture helper 保留同一 App/client，覆盖 Resources 三子页、Unity、Extensions、Activity、Diagnostics、Tasks、About，并验证三次设置 revision 写入后切回英语。fixture 使用内存设置，不据此宣称跨进程／跨重启持久化通过；daemon 的现有持久化实现未改变。

日志、退出码与失败 trace 保存在 `target/m7-localization-20261030/{browser,recheck,integration,final,review-final}/` 及同目录日志。最终中日 Settings／包 Review 全窗口截图在 `review-final/`，已目视检查中文 Review 和日文 Settings。各轮用例结束后仅清理确认命令行归属的 4173 Vite 才能完成本机收尾，保留其服务终止日志；测试 runner 的退出码按上文记录。

现有 `npm run gui:dev`（1420）已 HMR，原 Tauri PID 31300 和隔离 daemon PID 42628 保留，仍使用 `target/m7-icon-acceptance-20260906/data`。只更新开发前端，未重建原生二进制或操作真实工程。浏览器 fixture 不等于真实桌面操作；未取得新的屏幕阅读器播报、三语人工视觉验收或真实 Unity 启动证据。未重复 Rust Workspace；无 Rust／公共合同变化。Visual Gate 2 继续 **PENDING**。
