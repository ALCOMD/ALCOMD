# 软件包展示与状态样式验证

## 第一批：原有合同内界面

2026-11-08，范围为 [M7 软件包展示补齐](../exec-plans/M7-package-presentation-completion.md) 的现有合同内界面部分。前一批 Core 来源/版本可见性工作树保留，其 Rust 与合同验证见[可见性记录](package-candidate-visibility-20261108.md)。本批没有修改 Core、生产公开 DTO、能力名称或数据库迁移。

- 已安装名称：Material on-surface、600 字重；未安装：on-surface-variant、400 字重。行与按钮透明度均为 1，安装/更新按钮仍可操作。安装状态继续由原有版本字段及 Core/项目观察决定，不用颜色推断业务状态。
- 来源触发框显示来源名称或“多个来源”，完整来源名称在 tooltip 中保留；自动/显式来源菜单和原选择值不变。复用 Material Web SelectOption 的公开 `displayText`，与菜单文字分离，不操作其私有选中缓存。
- 最新列采用 Core `projectLatest`，与更新目标一致；Core 判定的不兼容版本单独说明。GUI 不比较 SemVer/Unity，不放宽资格或替换来源。
- `DesktopListRow.className` 为可选本地呈现属性，其他列表行为不变。现有行距、控件、排序、动画及主题保持；未新增依赖或图标。

GUI TypeScript、Material 边界、本地化检查、根 `npm run build`、`git diff --check` 通过。`m7-package-presentation`、`m7-package-candidates`、`m7-package-workspace`、`m7-package-order`、`m7-material-foundation` 共 **83/83 Chromium 通过，零自动重试、零跳过**。新增三项覆盖状态样式/控件透明度、来源事实与选择、兼容最新版本和实际 Plan 请求一致；已有回归覆盖来源关闭、设置 revision 隔离、版本分页/排序、键盘选择及 Plan/Apply。

截图已查看。结果和截图位于 `.local/package-presentation-20261108/`；主截图为 `browser/m7-package-presentation-in-3cad0-is-without-dimming-controls-chromium/installed-and-available.png`。这只是浏览器夹具证据，没有部署或操作运行中的 Core/GUI，也没有对真实项目或 Unity 执行操作。

该批结果只覆盖原有合同内界面，不能代替下列批准后的完整实施验证。M7/H7 人工签收保持。


## 第二批：获批候选 v2 / State30

所有者明确批准 [ExecPlan](../exec-plans/M7-package-presentation-completion.md) 后配套实现 Core、协议、SDK 和 GUI。`packages.candidates.v2` 替换 v1；严格 DTO 的必需字段扩展是有意的不兼容读合同，没有并行旧实现或新增权限。State30 为新增展示观测，不修改冻结项目 JSON、Plan/Apply/recovery 投影。

- 项目检查器有界读取锁定版本对应的本地 package.json，核对 ID/版本、阻止链接越界、约束字节与字符串；保存名称、说明、搜索词、合法 HTTP(S) 链接、撤回及 legacyPackages。缺失/不匹配为 partial；旧数据库为 unobserved，不伪造已采集。
- 登记、刷新、包操作、创建、复制、恢复及 Unity 迁移的项目发布事务写入独立展示观测。收藏只同步对应项目 revision。候选指纹单独纳入展示观测，分页不读文件或联网。
- 仓库展示元数据保存至 nullable presentation_json，用户包复用同一规范化函数。普通行名称与排序使用可见 provider，完整来源清单只在筛选器/管理页读取。关闭来源与缺失来源保持相同候选证据；本地安装信息不依赖 provider。
- Core 关键词/别名检索、撤回版本移除、已安装撤回标记、单一 SDK 对另一 SDK 的未安装依赖闭包隐藏，以及已安装现代包的未安装 legacyPackages 隐藏均有回归。未知证据不猜测隐藏；安装冲突仍保留。
- GUI 分组版本使用 Core Unity 分类。已安装/未安装名称层级、说明 tooltip、已安装撤回提示和正确来源标签保留。信息不完整时可调用既有项目刷新补采，成功后重读候选，失败保留列表并允许重试。此入口复用现有 projects.registry 能力和 Core projects.manage 校验；不自动写项目或授予权限。

原始日志、各轮失败及截图保留在 `.local/package-presentation-20261108/`，包括 v2-browser、v2-browser-final、v2-browser-acceptance 和 presentation-final。首轮浏览器 78/84 通过，六项失败涉及旧撤回版本的分页数量、分组文字定位、三个用户来源夹具标签；同步后第二轮 83/84，剩余为 Material host 文本包含空白的定位，改为核对精确 trim 后的公共 option 标签/disabled 属性。之后 85/85 通过。补采入口完成后展示用例 7/7 通过，其余 79 项复用没有改变的范围，共覆盖 86 个独立用例；零自动重试/跳过。截图已查看。

早期严格检查发现瞬时展示载荷扩大了既有恢复枚举，改为 Box 持有以保持内存布局；保留枚举与恢复语义。旧 State19/State16 迁移对比明确增加两张新表为空的断言，其余业务记录继续逐项比较，没有移除原断言。旧撤回候选断言按批准合同改为“候选列表为空”，仍保留 AllYanked 解释及 legacy 资格断言。早期测试编译和严格检查日志保留，最终门禁结果另见下方。

没有部署、提交、推送，未迁移真实数据库或重启运行中的 Core/GUI。浏览器夹具与静态编译不代替原生桌面和 H7 人工签收；跨平台 Hosted CI/CodeQL 未执行。


## 最终门禁

| 范围 | 实际结果 |
| --- | --- |
| 应用层 / Store / VPM | 436 项通过；VPM 3 项既有显式忽略。最终库回归为 application 74、Store 90、VPM 167；其余 105 项 Store 集成回归复用相同生产输入范围 |
| 候选 RPC、快照竞争、v2 DTO、项目动作、M4 合同 | 25 项通过；旧游标/权限/冻结 authority 断言保留 |
| 严格 Clippy | application、store、vpm、protocol、client、alcomd、testing 的 all-targets 通过；最终日志 clippy-verified.log |
| 原生 GUI 桥接 | cargo check -p alcomd-gui --all-targets --locked -j 2 通过；gui-rust-check.log |
| 前端 | 全工作区 npm run check / npm run build 通过；最终构建 frontend-acceptance-build.log。沿用既有 Vite 分块大小提示，没有修改门禁 |
| 浏览器 | 85 项整组通过；最后补采入口修改后 7 项展示用例通过，其余 79 项复用；共 86 个独立用例，零重试/跳过 |
| 公开合同 | 26 项正反例 Schema 检查通过；39 个冻结语义向量/81 个测试引用完整。形状脚本不等于执行全部语义测试，实际执行范围以上表为准 |
| 仓库 | cargo fmt --all -- --check、cargo xtask check、validate-metadata.py、git diff --check 通过 |

后续库重跑与严格检查期间只修正测试夹具参数顺序及已分配 Box 的原位赋值，没有放宽任何业务断言。最终库回归 331 项全通过，Store 集成范围复用先前已通过的 105 项。最终工作树文件摘要另存本地 verification-v2.json，避免把先前界面批次或历史 SHA 的结果当作本轮结果。


## 后续提交与本机更新

所有者随后明确要求“提交并更新”，实现已提交为 `e0073759` 并部署 Core/GUI，复用原数据与 profile。State30 迁移和候选 v2 实际只读核验通过，GUI 已打开且响应；备份、产物摘要与执行边界见[更新记录](package-presentation-update-20261108.md)。上文未部署的描述仅对应实施验证批次。
