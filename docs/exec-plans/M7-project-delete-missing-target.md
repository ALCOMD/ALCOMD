# M7 删除目标与载荷均缺失时结束任务

2026-11-12 所有者反馈刚更新后的删除任务仍卡在恢复中，取消返回operation_not_cancellable。本项接续原删除修复与“提交并更新”的纠正交付，处理实际无法继续执行的删除，不通过GUI隐藏任务、手工改库或伪报成功。

## 实施合同

- 在可信文件系统适配器中，验证Core持有的删除wrapper及owner，只有尚无quarantine identity、原目录及payload均经symlink_metadata明确NotFound，才报告既有project_delete_source_missing并明确失败。
- 失败记录completedAt、释放活动任务计数，保留项目登记；不推断原目录由谁移除、不寻找/删除别处目录、不自动重试后来重建的同路径对象。空的Core准备目录尽力清理，清理故障只补诊断。
- 已确认移动、payload存在、身份不符、重解析对象或访问错误仍保留原forward recovery与取消边界；不把source不存在与source仍原位混为一个证明。
- 使用内部强类型文件系统结果区分未移动、源/载荷均缺失与不确定，无公共RPC/错误码/Schema/权限/依赖变化。复用现有SourceMissing展示及任务终态事件。
- 补旧恢复任务与新任务缺失路径、已移动证据、无checkpoint的真实移动回归，保留已有删除/占用/取消/kill-restart门禁。修正后备份并更新Core，通过原恢复流程结束真实任务，保留GUI和数据配置；不推送。

源码与22项定向Rust回归、受影响严格Clippy、fmt/xtask/diff已通过，含4项新增。验证、失败轮次及纠正交付范围见[记录](../testing/project-delete-missing-target-20261112.md)。旧实例已离线备份，运行交付另记，M7/H7保持。

随后提交87edbfbe并更新Core，原真实任务由Core恢复入口结束为failed/source_missing，completedAt存在、活动数0；项目登记及原数据配置保持。实测关闭GUI后Core自动退出，并重新打开确认终态不再重试；沿用原GUI和profile。交付见同一验证记录，未推送，M7/H7保持。
