# 删除源目录与载荷均缺失时的终态修正

2026-11-12，接续[实施计划](../exec-plans/M7-project-delete-missing-target.md)。所有者在本次更新后指出同一删除任务仍恢复中，取消返回operation_not_cancellable。

## 原因与修正

原任务停在quarantine intent之后，Core尚未记录移动身份；原目录和owned payload现在均不存在。旧分支将此情况统一保留为recovery_required，而取消边界又已过，形成无法继续、无法结束的任务。

可信adapter仍先验证wrapper/owner。仅在未观测quarantine identity、两个路径均由symlink_metadata明确返回NotFound时，以既有project_delete_source_missing终结failed，填写completedAt，保留项目登记，释放活动任务；不声称完成了删除。内部强类型结果区分SourceUnmoved、SourceAndPayloadMissing和Uncertain，未新增RPC数据、错误码、Schema、权限或依赖。空准备目录清理尽力执行，清理失败不将不可继续任务续挂。

已移动身份、尚存payload、重解析对象或访问错误不进入该终态例外，forward recovery和取消边界保留。终态后重建原路径不会触发旧删除。真实任务仅由Core原恢复入口处理，不手工改数据库。

## 验证

22项不同Rust用例通过：完整删除套件14项（含4项新增），P7契约2项，Core空闲退出6项。删除套件中1项ignored是kill/restart矩阵显式启动的子进程fixture，未缺失矩阵覆盖。日志位于 `target/m7-delete-missing-target-20261112/` 的 `delete-regressions.log`、`p7-contract.log`、`idle-regressions.log`。

新回归覆盖新/旧缺失任务的failed/completedAt、全局无活动任务、项目登记保留、没有虚假directory_deleted事件、日志failed及同路径重建后不重试；已有移动证据但载荷丢失仍恢复，真实移动尚未checkpoint但payload存在仍完整forward recovery。原占用、空准备目录清理、替换路径、取消和每个持久checkpoint的kill/restart回归保持。

首轮4项中3项通过、1项在测试临时目录保护处失败：Windows extended path与普通路径前缀不同。改为两端canonicalize后检查绝对路径及fixture边界，保持递归操作保护，最终4项及完整套件通过；原日志 `target/delete-missing-tests.log`，复验 `target/delete-missing-tests-final.log`。

受影响Core/application/store/vpm/testing的all-targets/test-kill-gates严格Clippy、fmt、xtask及差异检查通过。Clippy日志 `target/delete-missing-clippy.log` 保留编译器增量缓存硬链接失败后复制的环境提示，退出码0，无代码lint放宽。未运行全工作区测试、三平台CI/CodeQL；无GUI代码变更，复用现有SourceMissing展示和终态呈现，不扩大前端验证。

## 纠正交付

接续所有者刚才的“提交并更新”及本次仍卡住的反馈，已正常关闭GUI，仅在确认唯一活动任务是原已停止恢复后停止准确Core，离线备份原程序、34份数据和336份profile并逐文件校验。更新准备的两次拒绝分别来自辅助脚本读取errorCode字段方式及CIM时间精度；未错误停止Core，改用实际DTO字段和同一进程API的精确启动时间复核，日志保留。

源码修正提交后构建独立Core，真实恢复、终态和自动退出验证另记；不推送，不扩大M7/H7验收。

## 真实任务与运行结果

功能提交为 `87edbfbeb12b0025055af0b0e3a9119220f31fd0`，独立Core构建通过后仅替换Core，GUI沿用上一轮已验证的同一文件。新Core经正常恢复入口将原任务 `da9ef751-4c88-466f-b3a0-0b956c733202` 记为failed、project_delete_source_missing、revision9，completedAt已写入，活动任务数0。Core清理了自有空wrapper；源目录/载荷仍不存在，未手工删除项目内容、修改数据库或虚报删除成功。

真实关闭本次GUI后，仅监视进程、没有RPC轮询/手工终止Core，观测Core在1314ms内自动结束。原数据入口再次启动后，失败任务的revision和completedAt保持，未重新执行；Core PID **126408**、GUI PID **114740**，ready/State32/Config13、0活动任务，窗口响应及实际GUI连接正常。未获取到已退出进程的退出码，不将此观察表述为退出码0；正常结果另由6项隔离生命周期回归覆盖。

- Core SHA-256：`F5468E4ECB89D9659C2F6BB057D9A6363315E7AD4C931D17942A6887246B0490`。
- GUI SHA-256保持：`E5157666C099571E1EA9FA3D2722D6BB644DC0D7913FBBA018AF6F9C5DF040F1`。
- 原4项目/30仓库/5模板/1Unity登记/0扩展的身份及配置、持久设置/revision7、12个项目标志检查位保持；模板定义完整保持，原10个Unity/Hub进程保持，沿用原WebView profile。
- 原 `target/m7-core-idle-update-20261112/ALCOMD.lnk` 仍指向同一程序和原数据/profile入口，适用于本次Core，未增加系统关联或修改永久环境。

证据见本项目标目录的 `verification-result.json`、`state-final.json`、`live-idle-result.json`、`state-after-reopen.json`、`reopened/open-result.json`、`environment-verification.json` 及 `final-filesystem-state.json`。原生验证限于实际进程、响应窗口、IPC和profile，未使用原生页面点击/内容验收。功能提交后的交付文档单独保存，未推送，M7/H7保持。此前记录中的“原任务仍恢复中”为旧版观察，本次已结束。
