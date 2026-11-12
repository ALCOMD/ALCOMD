# Core 生命周期修正提交与运行更新

2026-11-12 所有者明确要求“提交并更新”。Core 生命周期功能已提交到 main：`98e6b9540a00dd904ae25501ac09993309929a8b`。本轮不推送、发布或修改系统关联。

## 构建与备份

12份工作文件与107项 Rust 验证的收尾摘要一致，提交只补充后续授权文字。以干净的98e6b95构建生产前端及独立 Core、启用 `tauri/custom-protocol` 的嵌入式 GUI，结果通过；GUI 同时包含此前已提交的刷新圆圈反馈。复用已有功能回归及严格 Clippy/fmt/xtask，不重复执行整套测试。既有前端大块提示和 rustc 显式输出路径提示保留。

更新前 Core/GUI 均未运行。沿用 `target/m7-package-navigation-100ms-20261108/native-reopen/data` 和同级 `webview-profile`，离线备份旧程序、34份数据文件（含DB/WAL）及329份界面配置，复制结果与源树逐文件SHA-256一致。备份和日志位于 `target/m7-core-idle-update-20261112/backup/` 及同级证据目录，未启动旧 Core 触发原删除恢复。

## 更新结果

- Core PID **126816**，GUI PID **113392**，ready，窗口响应正常；GUI 已建立实际 IPC 连接，WebView 使用原 profile。
- Core SHA-256：`269547BC9A585C3CE15F2C16A90A715804A3B212B257796EBA250DEEFF1472C5`。
- GUI SHA-256：`E5157666C099571E1EA9FA3D2722D6BB644DC0D7913FBBA018AF6F9C5DF040F1`。
- 本次启动应用此前已获批准的 State31→32 仓库元数据迁移；Config13保持。4个项目、30个仓库、5个模板、1个Unity登记、0个扩展保留。
- 项目身份/根路径/类型/Unity版本/收藏、仓库来源及自定义名称/顺序/认证引用、Unity登记身份及路径保持；模板定义和扩展登记完整比较保持。索引、刷新时间等正常观察数据不混入配置比较。
- 持久设置文件逐字节保持，Core设置revision **7**；12个项目标志文件检查位的存在/缺失与摘要保持。原10个Unity/Hub进程身份保持，未关闭Hub或Editor。

证据包括 `candidate.json`、`backup-manifest.json`、`offline-before.json`、`state-final.json`、`offline-after.json`、`verification-result.json`、`environment-verification.json` 和 `first-open/`。原生验证限于进程、响应窗口、实际IPC、profile和只读状态；未作页面内容/点击验收。

## 原删除任务仍需恢复

更新前就有1个非终态任务：`da9ef751-4c88-466f-b3a0-0b956c733202`，删除 `kalmano-backup-2026-11-09T19.31.16`，状态recovering。新版按此前“继续完成删除”的授权尝试恢复，产生新的execute/recovery_required日志，结果仍为waiting / `project_delete_recovery_required`。

只读核对发现原目录现在不存在，暂存wrapper仅有owner.json、没有payload；更新前该项目三个标志文件也已不可读。未据此推断删除的时间/操作者，未手工移除项目登记或伪造任务完成。新检查与日志保存在 `delete-recovery-inspection.json` 和 `offline-after.json`。此任务仍未完成，因此当前Core会继续运行；不能将本次更新描述为“0活动任务”或已经完成该删除。生命周期自动退出已由隔离真实IPC回归验证，未用修改真实任务状态来制造空闲。

## 重开与回退

当前是使用独立数据目录的开发实例。新增本地 `target/m7-core-idle-update-20261112/ALCOMD.lnk`，通过同级 `open-runtime.ps1` 显式启动原目录的Core和原profile的GUI，避免Core自动退出后直接运行GUI误启动默认数据目录。启动时临时IPC连接等待GUI建立自己的实际连接后释放，不常驻保活；已验证首次打开和复用既有进程两条路径，重复入口保持相同Core/GUI PID。未创建桌面/系统快捷方式或改动永久环境设置，非正式安装器功能。

回退须同时恢复备份中的旧程序与升级前数据，不能让旧State31 Core直接打开State32数据库。本轮没有执行回退、真实包变更或手工删除。功能提交后的交付文档独立保存，程序对应的功能候选仍为98e6b95；M7/H7不扩大签收。
