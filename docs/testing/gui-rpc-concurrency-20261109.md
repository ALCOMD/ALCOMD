# GUI 耗时请求与跨页读取隔离

2026-11-09，M7 范围内的后续修正。保留此前尚未提交的包计划输入修正，本记录只覆盖 GUI 请求隔离、客户端连接复用及切页初始读重试。未提交、推送或替换运行程序。

## 原因与实现

GUI 原生桥接的 84 个 typed adapter 原先共用 `Mutex<Option<AlcomdClient>>`，持锁直到 RPC 返回；Core 对单连接串行调度。因此一个包计划等待即可阻挡项目、设置、任务读取及取消请求。Core 已支持不同连接并发，包计划读取/求解期间不持跨页面的项目写锁；无需改写公开协议。

原生桥接现使用按需建立且有界的独立连接池：页面读 4、耗时操作 4、状态/任务控制 2。仓库 inspect/register 及 Portable UI 各保留一条专用连接，保持预览复用、会话序号及连接绑定。现有事件订阅、批量仓库注册/取消和 Unity 窗口登记的独立连接保持。池的上限不含这些既有通道。满载时只在相应池内等待，Core 的项目锁、权限、revision、Plan/Apply 与业务资格保持。

连接租约结束自动归还槽位；客户端中断在途帧或传输失败后不再复用旧流，完整成功/业务错误可复用。连接/传输失败使所有池的旧代际连接失效，下一次使用重连，不自动重发写操作。SDK 只增加 `is_usable()` 状态查询，无公共 RPC/Schema/权限或生产依赖变化。

生产模式验证又复现初始读取挂起后的缓存重试问题：重试绕过 `use(promise)` 直接消费 settled map，后续 `useRef` 被当作尚未存在的 update hook。React 报 [467](https://react.dev/errors/467)，外层以 [520](https://react.dev/errors/520) 同步恢复整根渲染。每次页面访问现记录已交给 React 的初始读，挂起重试继续消费同一 Promise；首次渲染前已完成的预取仍同步读取。没有新增页面提示/弹窗，100ms 边界和原加载条保持。

## 最终验证

Rust/native 证据：`target/m7-gui-rpc-concurrency-20261109/`。

- GUI 原生库 **18/18**，客户端库 **11/11**：`native-final.log`。包含任务池满载、仓库预览及扩展会话占用时读/控制仍就绪；第二连接可继续；排队取消归还容量；全池重连代际；真实私有 IPC 帧的成功/业务错误复用及在途取消拒绝复用。
- GUI/client 全 targets 严格 Clippy、Rust fmt 和 xtask 通过：`clippy-final.log`、`fmt.log`、`xtask.log`，最终文档门禁见 `xtask-final.log`。
- GUI 前端检查及生产构建通过：`frontend-check.log`、`frontend-build.log`。保留既有 bundle 大小提示。
- 独立 GUI locked 原生构建通过：`gui-build-verified.log`，产物 `alcomd-gui-independent.exe`。两条显式输出文件名的 rustc 提示保留。源码、dist 及产物摘要见 `inputs.json`。

浏览器使用独立 Edge CLI 会话及生产模式确定性夹具。证据：`output/playwright/rpc-concurrency-20261109/`。

- `repeated.log`：正常、4 倍 CPU 限速各 3 轮；计划 Promise 始终挂起时设置内容、任务空状态及项目列表均加载成功。释放旧计划后路径保持项目页，零迟到弹窗、零 Apply、零 React 错误，始终只有一次 install 计划请求。
- `loading-budget.log`：正常/4 倍 CPU 下已预取软件包页各连续 6 帧均显示已安装版本 `1.2.3`；另暂停目标设置页读取，确认原 100ms 机制显示目标页顶部加载条、旧项目页隐藏，释放同一请求后正常加载。
- 浏览器夹具绕过原生 transport，只证明 React 切页、迟到结果及加载边界；原生队列隔离和私有 IPC 帧安全由上述 Rust 回归证明，不将两者拼称真实桌面端到端通过。

## 失败轮次与限制

- 首轮误将空任务列表当作 Tasks 表格等待，30 秒断言超时；修正为实际空状态后继续验证，保留 `finish.log`。未修改产品数据或伪造表格。
- 原初严格切页验证发现 React 错误；`verified.log`、`diagnose.log` 保留 520/467 及底层 hook 栈。该轮其他页面已加载、无迟到弹窗，但不能记为零错误通过。修正 `PageRead` 后同一严格流程及 6 轮正常/限速复验均通过。
- 默认沙箱的 CLI 缓存缺少工具，改用真实 Windows 用户的独立 CLI 会话；不是产品错误。浏览器唯一资源错误是夹具缺失 favicon 的 404，未忽略 React/pageerror。
- 未进行真实项目安装/卸载、未启动或停止 Unity，未替换实际 Core/GUI。浏览器验证会话已关闭。未执行完整 workspace、Hosted CI、Linux/macOS 原生或全量浏览器套件，不扩大为 H7 人工签收。

## 后续运行更新

所有者随后明确要求更新Core/GUI并重新打开。独立构建输入摘要核对一致后，已备份旧程序和完整数据，更新为Core PID288700、GUI PID283656，ready/State31/Config12。原数据、WebView profile、2项目、2仓库及设置摘要保持；运行证据见`target/m7-core-gui-runtime-update-20261109/runtime-after.json`。UIA确认实际项目列表后，所有者Esc中断桌面操作，未继续原生管理页或安装/卸载验证。该部署不包含其后软件包加载样式的修改。
