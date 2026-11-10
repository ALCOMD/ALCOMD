# 多来源版本灰色修复（2026-11-10）

## 原因与实现

当前项目修订 8、0 活动任务，候选目录完整。只读查询中 NDMF 前 100 条候选覆盖 33 个重复版本组，各项为 eligible、无阻止原因；CustomLocalization4EditorExtension 有 8 个双来源版本组。GUI 在版本分组后要求 entries.length===1，导致默认未选源时除已安装项外全部禁用。它没有检查依赖图；依赖可行性属于后续 Core Plan。

修正仅涉及 GUI：同版本仍显示一行，使用 Core 资格允许版本意图；默认多来源不将界面用于展示的候选代表 source 传给计划，由既有 Core 自动选源并在计划内冻结。显式来源和单来源传递保持。候选被阻止、目录不完整、快照无效、未知分类等条件继续生效；真实歧义与依赖冲突由 Plan 拒绝并在窗口反馈。无公共接口、权限、Schema 或 Core 生产实现变化。

## 验证

- 新增真实 Core/Store/VPM 隔离回归：两个仓库都提供同版本 NDMF，默认精确版本降级按原优先级固定正确来源、完成 Apply 且 MA 保持；降到低于 MA 约束时 Plan 返回 PackageDependencyConflict，项目不变。m7_vpm_semantics 全文件 23 项通过（target/m7-version-source-semantics.log）；新增集成测试严格 Clippy 通过（target/m7-version-source-clippy.log）。
- 三项新增浏览器回归覆盖分页重复版本可选、默认不指定来源及显式选源、Core 阻止版本保持禁用、依赖冲突在 Plan 显示。首轮测试修正可空首项 TypeScript 提示，以及 Material 内部 li 无 disabled 属性时改按既有 md-select-option.disabled 验证；功能断言保留。三份完整套件首轮 83 项通过，1 项旧分页测试明确要求重复来源选项 disabled=true，正是本轮修正的限制；更新为 false，同时保留分页/指纹、显式选源和未点击不建计划的断言，新用例另证默认不伪造来源及 Core 阻止不放行。最终三份完整套件 84 项通过（target/m7-version-source-browser-closure.log）。GUI 检查/生产及嵌入式构建、fmt、xtask 与差异检查通过。

## 交付

保留此前所有修复，未代替用户操作真实项目的软件包，未提交或推送。已逐文件备份旧 GUI、325 份 WebView 配置和设置后更新打开 GUI PID23972；Core PID31256、启动身份及摘要保持。3 项目/30 仓库/5 模板和完整设置/revision5、原 profile、v3/Unity 保持，0 活动任务；13 份源码/测试输入与验证候选一致，9 份真实项目清单摘要保持。GUI 摘要 9B8C59844952504AA0FA7FF3E550A1281EDA7960E1AB871A54BE7A4D4A3EEF7B，窗口响应且使用原 profile。证据与回退备份在 target/m7-version-source-update-20261110/。未执行原生按钮点击验收、H7 人工或三平台全工作区门禁。

额外只读核对实际项目：Mobile Expression Converter 1.4.0 要求 NDMF >=1.14.8，MA 1.18.7 要求 >=1.14.7。真实依赖限制仍由 Plan 判断；本轮删除的是 GUI 的重复来源误禁用，未绕过这些约束。
