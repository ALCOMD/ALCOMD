# SDK 卸载确认修正验证

日期：2026-11-09。基于main未提交工作树，按所有者“批准，按方案修复”落实[窄合同](../exec-plans/M7-package-removal-confirmation.md)。此前性能、请求隔离及加载样式修改保留，未提交或推送。

## 改动及行为

Core以锁定安装记录识别卸载目标，直接包和传递包均产生实际remove mutation。遍历本地锁定图时不重新保留明确目标，也不继续遍历目标的依赖；共享依赖保持，不可达的锁定包按原规则清理。保留依赖方的版本、元数据和依赖声明不变，已有缺失依赖不修复。

仅单包/纯移除批量计划允许把本次造成的缺失依赖作为冲突确认。Core把确定排序、去重、有界的removalConflicts纳入formatVersion 4及不可变计划摘要，公开DTO/Schema/SDK与GUI同步。非卸载计划不接受此授权；混合批量仍拒绝移除其余意图所需依赖。无冲突时字段省略，原计划JSON不追加空字段；无Schema迁移或新权限。

GUI在原确认框显示实际移除项和依赖冲突，保留原失败呈现。纯批量卸载的资格改为已安装状态，不要求直接依赖或完整仓库索引；GUI跳过其仓库候选预检，保留Core修订/快照/权限复验。取消恢复原按钮焦点，仓库不完整不阻挡恢复焦点。100ms导航和后台显示逻辑保持。

## 验证

Rust证据：`target/m7-package-removal-20261109/`。

| 范围 | 结果 | 日志 |
| --- | --- | --- |
| VPM库最终范围，含卸载图、排序/循环/共享依赖、混合意图拒绝及事务/恢复 | 173通过，3项既有忽略 | `vpm-boundary-final.log` |
| Core库，含内部证据到公开RPC DTO投影 | 43通过，1项既有联网忽略 | `libraries.log` |
| 应用层，含无来源传递包的批量卸载资格 | 77通过 | `libraries.log` |
| 协议库及公开Schema/DTO | 17+47通过 | `libraries.log`、`protocol-schema.log` |
| M4/P6合同、真实reader/store/Plan/Apply及手动Unity例外 | 6+12+3+2通过 | `transactions.log` |

合计380项不同Rust测试通过，4项既有忽略未单独运行。SDK事务用例分别覆盖Avatars/Base的单包和纯批量卸载、来源全部注销、计划持久化往返、Plan零项目写入、元数据变化拒绝旧计划、实际目录/清单删除、保留依赖方及同一幂等Apply返回同一Operation。仅使用独立合成项目，没有卸载用户SDK。

受影响Core/application/protocol/VPM/testing全部targets严格Clippy通过；新增最后一项VPM边界测试后另跑VPM严格Clippy。Rust fmt、xtask、元数据、所有前端工作区检查及GUI生产构建通过。日志：`clippy.log`、`clippy-vpm-final.log`、`fmt-check.log`、`xtask.log`、`metadata.log`、`frontend-final.log`、`frontend-build.log`。生产构建保留既有bundle大小提示。

浏览器最终覆盖48个不同用例：`browser-final.log`为47通过/1项筛选时序失败；等待Status条件被Core夹具接受后再继续搜索，保留联合条件/expectedSnapshot断言，整个工作区28/28通过（`browser-workspace-final.log`）。导航及v3展示的20项未受该测试同步修改影响，复用前轮结果。没有自动重试或跳过，不描述为最后一次整轮48/48。

生产模式中文Edge核对两份SDK卸载确认内容及截图，均显示实际移除和依赖警示；取消零Apply，无pageerror。证据：`output/playwright/package-removal-20261109/review-verified.log`及`com.vrchat.{avatars,base}.png`。这是夹具截图，不代替真实原生端到端。

## 实际项目只读核对与产物

只读探针对Test-Avatar仅调用inspect和draft计算，不保存Plan、不Apply、不联网。Avatars从原0项变为1项移除，列出5个保留依赖方；Base从PackageNotFound变为1项移除，列出2个依赖方。各3轮约12–29ms，0来源；不作为冷启动或GUI/RPC耗时承诺。证据：`actual-avatars.log`、`actual-base.log`、`measure.rs`。

独立Core构建及启用`tauri/custom-protocol`的嵌入式GUI构建通过；产物`alcomd-removal.exe`、`alcomd-gui-removal.exe`，源码/产物摘要在`inputs.json`。运行中的`target/debug/alcomd.exe`及GUI摘要仍为部署时的80AD1871…与8590618D…，未替换或重启，原数据/profile保持。

## 失败轮次与限制

- 初次all-targets检查因新测试同时借用同一Vec而失败，修正测试借用后通过；日志保留在`target/m7-package-loading-style-20261109/removal-check.log`。未放宽生产校验。
- 初次前端检查发现bulkReady提取后TypeScript不再自动缩窄candidates，补显式存在性检查后通过，见`frontend-check.log`。
- 首轮浏览器45通过/3个新增断言误以Material内部dialog角色节点为内容宿主而失败，改为宿主定位后3项均通过；下一轮1项旧筛选测试在选择状态尚未接受时继续输入，加入条件接受等待后整个文件通过。失败与后轮日志保留。
- 初次独立GUI构建误指定不存在的根custom-protocol feature，失败未替换任何程序；最终采用Tauri依赖feature。显式rustc输出文件名的两条提示保留。只读辅助探针初次误混用不同serde构建变体进行跨crate序列化，改为输出Debug证据，最终编译/计算通过，没有修改产品依赖。
- 生产截图最初选择display:contents宿主无绘制盒，改为实际dialog绘制节点；另一次夹具裸路由reload没有入口HTML而失败，改从正式browser-harness入口重开。仅修正验证脚本，最终两份截图和零Apply核对通过。
- 未运行完整workspace、Hosted CI、Linux/macOS原生或实际用户项目卸载；未执行本轮原生界面交互。现有恢复测试与持久JSON往返通过，未新增独立进程kill矩阵。不扩大为H7人工签收。
