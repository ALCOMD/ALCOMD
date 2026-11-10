# MA 降级依赖条件校验修复（2026-11-10）

## 依据与变化

只读查看两个真实降级任务（65f2cf2e-66de-47f6-b9d4-e86f87482f3e、09058de7-0ea7-4145-b19f-48fe7ff84215）：计划、确认、下载、解压均通过，随后因 PlanStale/manifest_fingerprint_changed 失败。事务标记停在首次替换 intent，备份目录为空、包仍在 staging，项目 MA 保持 1.18.7，目标包为 1.18.6。日志摘要中 packages_replaced 的 succeeded 行对应写入前的 journal intent，不表示已完成替换。

归档的 NDMF 条件为 >=1.14.7 <2.0.0-a，来源解析器会将比较条件规范化；归档验收先前用原始 JSON 比较，导致等价的顺序变化也拒绝。本轮两边复用既有 VPM 依赖解析器再比较，不增加新的兼容路径、不关闭校验，不改公共协议或依赖。

## 验证

- 新增隔离 MA 1.18.7 → 1.18.6、NDMF 1.14.8 的完整 Plan/Apply 回归。修复前稳定复现 failed/plan_stale/PackagesReplaced（target/m7-ma-downgrade-baseline.log），修复后成功，只有 MA 一个替换，NDMF 锁版本和 package.json 字节保持。
- m7_vpm_semantics 全文件 22 项通过（target/m7-ma-downgrade-integration.log）。扩展既有归档校验回归：真实依赖变化、无效范围、null 结构、Unity 要求和旧资源变化在写项目之前拒绝；相应 24 项旧资源/归档测试全部通过（target/m7-ma-downgrade-regression.log）。
- VPM 单元测试 182 项、包 RPC 6 项、手动 Unity 兼容性 2 项通过；连同语义 22 项和旧资源/归档 24 项，共 236 项不同 Rust 用例通过，另有 4 项既有忽略。日志为 target/m7-ma-downgrade-vpm.log、m7-ma-downgrade-integration.log、m7-ma-downgrade-regression.log。
- VPM all-targets 及两份集成测试严格 Clippy、fmt、xtask 和差异检查通过。Core 独立构建通过；GUI 复用上一轮相同二进制，8 份相关界面源码/测试输入摘要保持，沿用其已通过的 81 项浏览器验证，不宣称本轮重新执行。输入与构建摘要在 target/m7-ma-downgrade-update-20261110/。

## 交付范围

仅 Core 比较和相关 Rust 回归/记录变化，GUI 源码保持上一轮验证输入；不重跑无改动的前端。未代替用户执行真实项目降级、安装、卸载或重试，没有提交或推送。离线备份原程序、28 份数据文件和 324 份 WebView 配置并逐文件核对后，已更新打开 Core PID31256、GUI PID27580；Core 摘要 29C4C8DAA7D271689A394E80291E7C2A670CBFAA899C15B93026BA10D87DF2D9，GUI 摘要保持。ready/State31/Config13、0 活动任务，3 项目/30 仓库/5 模板和完整设置/revision5、原 profile、v3/Unity 保持。GUI 打开后项目/模板/设置保持，仓库仅 12 个正常后台刷新时间推进，其余字段完全相同；初次严格比较对此拒绝，差异核对后单独记录而未回退数据。9 份真实项目清单摘要保持，MA 仍为 1.18.7。记录与可回退备份在 target/m7-ma-downgrade-update-20261110/。未进行原生包按钮操作或全工作区三平台门禁，M7/H7 状态保持。
