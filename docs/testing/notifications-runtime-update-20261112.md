# 收藏/通知分次提交与本机更新（2026-11-12）

所有者明确要求“分次提交推送并更新”。在main上将收藏实心修正独立提交为`7d353b34`，通知功能独立提交为`a1f88a9e`。确认origin/main无分叉后，正常推送两项及其9个既有前置提交（原远端978b1de7）；没有改写历史、强推、发布、打标签或改GitHub设置。

## 远端检查发现与修正

首次推送的Ubuntu job 131059573696在Check阶段报`apps/alcomd-gui/src-tauri/src/lib.rs:291` unused_mut。已直接读取该已结束job日志，确认Unity Hub选择器只在Windows/macOS分支修改builder，Linux不修改。改为不可变绑定及平台条件重绑定，保留路径提示、exe/app过滤和系统选择器行为；独立提交并正常推送为`1f48e668`。没有放宽-D warnings。格式检查、GUI all-targets严格Clippy及5项既有picker_location回归通过。

修正候选的三平台CI任务43183267728及CodeQL任务43183267622仍在运行，不能宣称通过；1d1e5db完整本地收尾结果仅是本次增量前基线。M7/H7仍保持技术收尾，未推进M8。

## 构建与切换

运行代码候选为`1f48e668ee3af174ee60d576ab6c8c26276784aa`。Core release构建和Tauri no-bundle嵌入式GUI构建通过；修正后的Core复建复用未改变的Core缓存，GUI按修正候选重新构建。现有前端类型/控件/1260条语言/36份图标边界通过，收藏与通知的分轮192项浏览器回归见[功能验证](in-app-notifications-20261112.md)。后续交付文档提交不改变运行代码。

切换前重新查询ready及0活动任务，并核对旧Core PID84408和GUI PID123988的路径、精确启动时间。通过正常关闭窗口结束GUI，Core自动退出，GUI退出后等待Core耗时7156ms；未手工杀进程或RPC轮询保活。原WebView进程释放后，备份两个旧程序、34份数据和336份profile，逐文件SHA-256验证备份与原件一致。随后仅以已构建发布程序替换原target/debug/alcomd.exe及alcomd-gui.exe，核对安装/启动哈希一致。

沿原开发入口重开，同一原数据目录`target/m7-package-navigation-100ms-20261108/native-reopen/data`及同级原webview-profile继续使用；原快捷方式仍有效，没有启动默认用户数据目录或改变永久环境/系统关联。

## 实际运行与保全

- Core PID **101844**，GUI PID **131692**；ready、GUI实际连接且窗口响应，State32/Config13、活动任务0。
- 原4项目、30仓库、5模板、1Unity登记、0扩展的身份/配置保持；模板定义完整保持。持久设置文件SHA及完整RPC设置/revision7保持，12个项目标志检查位保持。
- 原删除任务仍failed/project_delete_source_missing、revision9，原completedAt不变，没有重删或改库。
- 原4个Unity编辑器及6个Hub进程的PID、路径和启动时间均保持，原WebView profile实际打开；没有关闭Unity/Hub。
- Core SHA-256：`E77CD21BE84A98753EBC83A96151A8B99CC4AFD6B0741560BB920D759C1E7A9E`。
- GUI SHA-256：`F2449B8ADB0B010B16A33DA3F2FD3C09B0FD259310037A5A0870E36435F37027`。

证据位于`target/m7-notifications-delivery-20261112/`：原/新状态、精确进程身份、backup-manifest、offline-before/after、close-result、installed-programs、reopened/open-result、verification-result、environment-verification及构建/CI日志。旧程序和原数据/profile备份位于其backup子目录，未删除。

本轮原生验证限于真实进程、正常关闭/自动退出、窗口响应、IPC、程序摘要及数据/profile保全；未自动点击真实收藏/刷新操作或进行原生读屏人工验收。未做数据库迁移、手工修改真实任务或包，也未以此交付记录替代新统一候选的完整阶段门禁。
