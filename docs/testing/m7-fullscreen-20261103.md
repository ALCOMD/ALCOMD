# M7 持久全屏模式

所有者要求在“外观→窗口”增加全屏，随后明确批准跨启动保留的Config10最小合同（A-039）。M7/H7人工状态保持。

## 最终范围

Config10增加window.fullscreen，默认false，完整读取必须包含该字段；window的两个bool可以独立PATCH，空对象、null、未知字段与非法类型整次拒绝。有效Config1–9先严格校验，再保留revision和所有已有设置补false。State18、既有任务/归档、RPC方法、权限和依赖保持。

GUI提供三语复选框，F11也经同一设置队列保存。Core接受后调用现有主窗口私有薄适配器应用全屏，下次启动保持。首次恢复普通尺寸后再应用全屏；全屏切换不再次恢复旧缓存，过渡/全屏尺寸不记录为普通尺寸。窗口卡片恢复默认先确认、只将rememberState/fullscreen均置false，默认时按钮禁用。原生应用失败提供安全提醒及只重试窗口应用的Retry，不额外提交设置。

## 实际验证

- Protocol schema契约41/0、Application M7设置28/0、Store配置29/0；五个受影响crate（protocol/application/store/alcomd/GUI）严格Clippy通过。fmt、xtask、metadata、diff及根前端check通过。
- 设置队列43场景、GUI窗口桥接14场景、原生窗口纯模型6/0通过，保留既有缓存/恢复/停止连接边界并补全屏与记忆关闭独立、切换不重复恢复缓存、原生失败重试。
- 浏览器首轮26/1：旧项目目录键盘用例在确认Dialog关闭动画完成前重新抢焦点；补等待真实焦点返回，保留路径草稿/Shift+Tab及零写入断言。修正后整组27/0（1 worker、0 retries），覆盖全屏默认/持久/F11/恢复/丢响应与三语，以及设置自动保存、卡片恢复、分组回归。输出target/fullscreen-browser及-r2。
- 原Core占端点时，IPC保护返回AlreadyRunning，未放宽互斥。安全关闭原Core后的首轮隔离IPC为1/1：新增全屏重启用例通过；旧回归的持久文件逐字预期仍写Config9，与正确迁移不符。程序更新在构建前停止，按保护路径恢复原二进制、原数据/profile为Core135756/GUI162464，六组公开状态完全一致，真实配置仍Config9。
- 仅修正旧测试的canonical Config10预期，保留revision14及全部旧字段逐字检查，并补实际Config9输入重启后迁移回canonical10的断言；no-run编译通过。再次安全关闭后隔离IPC **2/0**，覆盖全屏true跨重启保留、窄PATCH保留rememberState、非法请求原子拒绝与Config9迁移不增加revision。记录分别为target/fullscreen-desktop及-r2。
- 第二次更新Core/CLI release通过（3分19秒），GUI检查遇到同工作区并行刷新布局改动遗留的三处showRefreshBar参数，构建停止并恢复原二进制为Core143116/GUI163872，六组状态仍一致，真实配置保持9。仅清理三处已废弃调用参数，保留并行布局，GUI check通过；本轮全屏验证不替代并行刷新/活动记录改动的专项验收。原日志保留。
- 第三次更新target/fullscreen-desktop-r3的IPC仍2/0；Cargo因target缺少CACHEDIR.TAG拒绝clean，未绕过目录保护、未删除文件，按保护恢复Core164220/GUI93752，六组状态一致。针对含并行布局的GUI重新集中回归27/0，结果保存于target/fullscreen-browser-r3。
- 第四次更新target/fullscreen-desktop-r4核对Rust/manifest/lock输入与IPC2/0时完全一致，不重复IPC。只备份并移除两个经路径、包名和长度校验的16字节可执行缓存标记以强制重建，不清理target目录，并保留新Core/CLI staged副本，避免回退后的旧二进制被复用。并行页面修改保留，其专项验收范围沿用对应记录。

本轮按受影响模块和契约执行验证，未重跑完整Workspace/远端门禁，不拼接历史605/0或主题58/0作为本轮完整结果。浏览器和IPC使用隔离夹具；真实原生全屏交互及三平台表现仍待人工查看，不以模型或浏览器测试冒充。

## 桌面交付

第四次更新的Core/CLI release（2分03秒）及GUI release no-bundle（3分01秒）通过，已用原数据目录/profile打开Core **173444** / GUI **168544**，GUI窗口可响应。Core SHA-256为`681F15E1689D5FAC6BDF7FA4B3537002667709BF967CB1DB216B94F402B1EA42`，GUI为`F6CD9D8DB9A29F6FCFA016C550D9EF1CE1674BA93DB948EADDF89E173130CEF0`。握手实际为Config10/State18。

真实Config9→10仅补fullscreen=false，更新前后revision **177**及所有原设置保持；Project、Editor、Operation、Backup和Status五组公开状态完全相同。初次字符串比较因window对象的JSON键顺序误报settings不同；保留原comparison.json，另以递归规范JSON比较全部字段、类型、数组顺序和值，六组均通过，记录semantic-comparison.json与comparison-note.txt，没有修改真实数据去匹配预期。

构建期间同工作区并行活动记录/刷新改动继续，原501项全工作区输入检查如实记录6项变化，没有将该检查改成通过；原生GUI构建前206项输入快照保留。另核对本轮全屏及契约的290项输入（含全部Rust/manifest/lock及specs和全屏GUI实现/用例）均无变化。scope-input-verification.json记录本轮范围通过与全工作区未冻结的区别，不把本批视作M7最终候选门禁；并行页面的当前源码不保证全部已进入本次产物。

当前运行身份同步至target/m7-storage-locations/candidate-runtime.json；原程序/配置、各轮失败、IPC及构建日志分别保留。没有对真实设置测试开启全屏、恢复默认或执行工程操作，没有提交或推送；所有者可在已打开程序中查看原生效果。

## 快捷键文案补充

所有者随后要求标签加“（F11）”，现为中文“全屏模式（F11）”、英文“Full screen mode (F11)”、日文“全画面モード（F11）”；同步现有浏览器定位。GUI check与三语边界通过。仅改标签，不改快捷键或设置行为，按项目集中构建约定未为此单条文案重复构建桌面程序；上一节产物不包含该文案增量。
