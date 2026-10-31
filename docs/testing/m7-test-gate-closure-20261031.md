# M7 测试门禁与 H2-A 桌面验证

日期：2026-10-31。基于 `4eae1d4619b4534cf6bd4916e9c282b87be0eec4` 的未提交工作树。
本报告保留 P6 修复轮次的验证证据；当前阶段与后续 Visual Gate 2 签收以 `docs/status.md` 为准。
本轮未提交、推送、创建 worktree 或修改真实更新/签名配置。

## 测试实现

- 真实 IPC 测试先等待自建 daemon 完成原有初始化、恢复和绑定，再创建客户端；占用或初始化
  失败立即返回。全部 35 个启动点（包括现有 ignored subprocess helper）使用同一 test-only
  入口。生产初始化顺序不变，不引入第二个 Windows 用户端点或旁路协议。
- `TestDaemon` 在服务 task 正常结束或 panic 后仍持有 listener，直到测试 handle 释放，避免
  测试随后连接到其他实例。新增四项回归验证占用时不开库、初始化失败无端点、成功启动与正常
  退出后的所有权、panic 后的所有权。实现只在既有 `test-kill-gates` feature 下编译。
- Copy 集成测试独立读取 `unity.writerState`，验证 typed evidence，再与 Copy Plan 对照。
  原完整 Apply 成功、文件内容/排除项、注册、来源时间与幂等 replay 断言保留。生产合同仍是
  confirmed 硬拒绝，suspected/unknown 进入风险证据；没有停止用户的 Unity/Hub 进程。
  新增 Plan 后出现 confirmed writer 时 Apply 拒绝且没有 target/Operation 副作用的回归。
- Browser suite 预构建三个既有 HTML fixture 后静态 preview，保留 development React、
  StrictMode、sourcemap、原有单 worker、零重试、30 秒超时及全部断言。输出只在
  `target/gui-browser-fixture/`，清理前检查规范化边界和 symlink/junction。
  七并发候选探针经 harness 静态导入的同一个 `queryCandidates` 调用，仍要求 maximum=2，
  且没有生成 Plan。普通 Vite 配置、依赖版本、生产 manifest 和三个锁文件保持。

项目所有者随后批准了下述 P6 实际 wire 字段纠正。没有新增 RPC 方法、capability、permission、
数据库版本、生产依赖或平台 API；现有测试启动、Copy 与静态 fixture 修复保持。

## 已批准并实施：P6 wire 字段修复

项目所有者于 2026-10-31 明确批准按已冻结 P6 camelCase 合同修正 Rust DTO，并分离候选
source DTO，保持候选查询的 snake_case 合同不变。最终实现如下：

| P6 合同对象 | 当前有效字段 | 原因及修正 |
| --- | --- | --- |
| source selector | `repositoryId` / `userPackageId` | `PackageSourceSelector` 的 variant fields 使用 camelCase |
| reinstall selection | `packageIds` | `PackageReinstallSelection` 的 variant fields 使用 camelCase |
| bulk intent | `packageId` / `versionRange` / `includePrerelease` | `PackageBulkIntent` 的 variant fields 使用 camelCase |

上述 enum 的 `kind` 值仍为 `repository`、`user_package` 等原值。候选查询的四处 source
引用使用独立公开 `CandidateSource`，仍只接受并输出 `repository_id` / `user_package_id`。
内部领域来源模型、现有 DTO→领域转换和 Plan/Apply 业务保持；实际 typed caller 编译验证无需
额外适配。GUI、SDK 与已批准 P6 Schema 本来就使用 camelCase，无需加入页面特例转换。

这是替换当前错误接受的 P6 snake_case 字段，属于实际 wire 的破坏性纠正，按未发布合同的
本次人工批准直接实施。不保留旧字段别名、双格式或回退；RPC v1、State16、Config3、权限和
依赖保持。P6 规范中的 bulk source 示例同步改为已有嵌套 selector，未另建第二份有效合同。

候选 Schema 与 vectors 的 SHA-256 分别仍为
`7955445716A6ABC8DD36AD098C0B1614E3DE369683FF6F922F72DF15A8620C77` 和
`B3AD5DEB89A638EEAE5A9F480549EF64A95CD715CEC7BF10AB13E56CF9F33187`。

新增 44 项 P6 wire shape（15 正、29 负），覆盖六类 DTO 的嵌套字段、旧拼写、混合拼写与未知
字段。独立 JSON Schema 验证输入 shape，真实 Serde 验证接收/拒绝及序列化快照；快照保留
既有 Option 的 null 与默认空数组输出，没有把输入省略规则改写为新的输出合同。候选测试
覆盖两种 source 的四处嵌套位置，确认与 P6 的拼写独立。

修复前同一回归真实失败于 `unknown field repositoryId`，日志保留在
`target/m7-p6-wire-fix-20261031/p6-contract-before.log`；修复后两组纯契约测试为 3/3、5/5。
已有 unit variant `selection:{kind:"all"}` 的额外字段处理仍有 Serde/Schema 差异；本次没有
修改该独立行为，不把上述 44 项表述为全部 P6 Schema 硬化已完成。

浏览器回归另补齐三处真实生命周期等待：Resources 在表格与选中 segment 就绪后才测量宽度；
Motion Select 等待公开 `opened`/`closed`；项目菜单等待打开完成后再 Escape，并在关闭后
验证原焦点。原 trace 分别证明路由仍 busy 时测得零宽度，以及 Escape 在打开点击约 43ms
后发生、菜单尚未移入焦点。只修改测试，保留原业务/焦点/精确几何断言、30 秒和零重试。

## 浏览器网络证据

本节测量和事件快照保存在修复前的 `target/m7-test-gate-closure-20261031/`，没有重新测量为
P6 候选的网络结果。

原失败 trace 的页面导航开始于 `2026-10-30T21:24:11.767Z`，Windows System/Tcpip 4227
事件 `546485` 为 `21:24:11.7687049Z`，指向传出 TCP 本地端点的复用限制，页面尚未加载。
事件保存在 `prior-tcp-event.json`。这不是应用包筛选断言失败，也不能由事后的低连接数推算
当时的全部 TCP 状态。

同一锁定 Chromium、同一浏览器进程、每个入口独立 fresh context 的一次测量：

| 入口 | 原 development HTTP 响应 / 独立 HTTP connectionId / WS | 静态 development HTTP 响应 / 独立 HTTP connectionId / WS |
| --- | --- | --- |
| browser-harness | 148 / 6 / 1 | 7 / 6 / 0 |
| paged-items | 116 / 6 / 1 | 5 / 4 / 0 |
| utility-workspace | 126 / 6 / 1 | 5 / 4 / 0 |

六次导航均无 HTTP 错误、loading failure 或 page error。HTTP 响应数不能当作 TCP 连接数；
表中的 connectionId 来自 CDP responseReceived，WS 单独计数。该测量证明减少了 fixture
请求和部分 HTTP 连接，不能单独证明 Windows 全局端点限制被消除。没有调整系统 TCP 参数、
浏览器 feature flags、工具链版本、测试隔离、重试、超时或验收条件。

独立启动的 fixture 构建为 9.79 秒，外部首次 HTTP 200 为 13.071 秒，低于原 30 秒门槛。
三个 HTML 和引用的八个不同资产 URL 为 200，源模块、`/@vite/client` 和根路径为 404；
sourcemap 确认 React/ReactDOM/jsx-dev-runtime 使用 development 实现。
从完整 GUI 启动至最终恢复核验的 `2026-10-30T22:08:26.8888774Z`—
`22:31:45.0258220Z`，未查询到新增 System/Tcpip 4227 事件，见 `tcp-events-current-run.json`。

## 验证结果

当前 P6 修复后的原始日志和输入摘要位于 `target/m7-p6-wire-fix-20261031/`。

| 检查 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | PASS；`fmt.exit` |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | PASS；`clippy.log/.exit` |
| P6 与候选纯 Serde 契约 | **8 passed / 0 failed**；`p6-contract-final.log`，已包含在完整 Rust 结果中 |
| 独立 JSON Schema | P6 **44/44**；冻结候选 **21 shape** 和 39 semantic 引用覆盖检查通过；后者不冒充 semantic 执行 |
| `cargo test --locked --workspace --no-fail-fast` | **483 passed / 0 failed / 8 既有 helper ignored**；`workspace.log/.exit`、`workspace-counts.json` |
| `cargo xtask check` | PASS；`xtask.log/.exit` |
| metadata、冻结基线检查 | metadata PASS；候选冻结 hash 不变；原静态 fixture 轮次 `baselines.exit` 为 0，基线文件保持 |
| `npm run check`、`npm run build` | PASS；当前 Tauri beforeBuild 日志包含前端 check/build；最终测试等待修正另有两份 targeted-check 日志 |
| production daemon/CLI/Extension Host build | PASS；`native-core-build.log/.exit` |
| 完整 Chromium suite | **238 passed / 0 failed，9.0 分钟**；`gui-full-verified.log/.exit`，单 worker、零重试 |
| Tauri release no-bundle | PASS；`tauri-release-build.log/.exit`，嵌入 production frontend |
| 真实公开 RPC wire 复核 | **8/8**；`fixed-wire-probe.json` |

完整 Rust 结果来自本次同一轮 workspace run；定向测试没有重复加到 483 中。P6 修复后的
首轮 GUI 为 **236/2**（`gui-full.log`），第二轮为 **237/1**（`gui-full-final.log`）；失败
trace 分别保存在 `gui-first-failures/` 与 `gui-second-failures/`。各轮独立记录，不把定向
2/2、1/1 拼接为全套通过。最终 238/0 来自一轮独立完整运行，运行期间源码与测试输入 hash
保持，见 `final-suite-inputs.json`；最终输入与二进制摘要另保存为 `source-inputs.json`、
`binary-inputs.json`。

修复前静态 fixture 轮次保留在 `target/m7-test-gate-closure-20261031/`：完整 Rust
481/0/8、GUI 238/0（12.6 分钟）、启动所有权 4/4、Copy 8/0/1 及对应构建日志。它们不
代替本次 P6 候选的结果。更早架构审查的 475/1、GUI 237/1 继续保留在其原始报告。

## 运行环境与桌面证据

按项目所有者此前的明确授权，暂停原工作区 Tauri dev 与指定样例 daemon 后执行真实 IPC
测试；未停止用户的 Unity Hub helper 或已安装的 v3 程序。核心生产二进制构建后已按原命令、
原 `target/m7-icon-acceptance-20260906/data` 恢复 daemon。正式 CLI 只读确认 ready，全部
两个既有样例 Project 的完整列表与 Editor 空列表均与测试前一致，见当前证据目录的
`runtime-roots-before.json`、`restored-daemon.json` 和 `runtime-final-comparison.json`。

真实桌面使用另一个全新 `desktop-fixture/data`，不复用两个原样例的 registry。未注册的合成
项目和两个版本的归档在本轮 target 目录内准备；归档只有 `package.json` 和 `README.txt`，
没有可执行代码。之后仅通过正式 CLI/RPC 注册项目/仓库和刷新；下载源仅监听 loopback。
没有直接写 daemon 数据库、package cache 或已注册项目。

P6 修复前真实 release GUI 已复现：候选列表正常，Install 在进入确认前失败；同一 CLI
dry-run 成功。公开 RPC 原复现 `p6-wire-probe.json` 证明 nested source、reinstall selection
和 bulk intent 的 camelCase 被拒绝。该历史证据留在原目录，不保留为当前有效实现说明。

修复后在另一个全新 `target/m7-p6-wire-fix-20261031/desktop-fixture/data` 使用当前 release
GUI 验证六种 Plan review：单包安装 1.1.0、单包重装 1.0.0→1.0.0、批量安装、Update All
批量升级 1.0.0→1.1.0、批量重装、批量移除。每项均显示正确摘要并用 Escape 关闭，未在
GUI 点击 Apply。为取得已安装状态，只通过正式 CLI 在临时项目安装合成 1.0.0；归档仍仅
含 JSON 与文本。最终项目 revision 2、已装 1.0.0，只有该 seed 的一个 succeeded Operation，
后续 review 与 RPC 探针未执行 Apply；见 `native-package-checks.json` 和最终 project/operation
快照。弹窗 Tab 从 Apply 回到 Cancel 的焦点环可见；UIA 只报告外层 pane，完整返回焦点与
键盘输入验收尚未确证。

公开 RPC 复核 8/8：P6 upgrade/bulk/reinstall 的 camelCase 成功，旧 snake_case 分别
返回 `invalid_request`；候选 versions 请求仍接受 snake_case 并返回两个同格式来源记录，
对应 camelCase 请求被拒绝。见 `probe-fixed-wire.ps1`、`fixed-wire-probe.json`。

真实 Settings 另验证：深色预览期间 Core 仍为 system，Cancel 恢复浅色；Save 后 Core 为
dark，语言选择 English 即保存。重启 release GUI 后深色与英语仍保留，Core revision 3。
Hub 原生文件选择器显示正确 exe filter 和既有路径；首次索引点击后的实际返回无法归因为
取消，故不作为取消证据。之后 Auto detect 恢复 discovered/null 偏好至 revision 5，再次
打开选择器并按 Escape，全部设置与 revision 5 不变。Open 经一次窗口绑定恢复后的点击
显示既有 `Unity Hub 3.22.5` 窗口，未操作其登录、安装或项目；窗口可见不证明 Hub 全部就绪。
见 `native-settings-hub-checks.json` 和各份 `desktop-settings-*.json`。

本机已存在 Hub，未隐藏/卸载它以制造 download_required，真实下载分支未验。当前默认 GUI
client 在失联时可能启动默认数据 daemon，因此未在该真实 release 会话停掉 Core 来制造离线，
避免触及隔离样例外的数据；browser 的共享 reconnect 回归已执行，不能代替此项 native 验收。
完整键盘/返回焦点、GUI Apply/Operation、完整视觉签收仍待完成；本轮没有 Hosted CI 或 M12
客户端发行结论。

截图位于仓库外的
`C:/Users/M2922/.codex/visualizations/2026/08/17/01a00f12-6751-7d50-9346-7b58e94f2eea/`，
文件前缀为 `m7-p6-wire-`，包含 light Projects、六类 review、dialog focus、dark preview、
settings restart 和 Hub chooser。修复前 `m7-gate-closure-*.png` 只保留失败历史。

当前 release GUI、临时 daemon 和归档服务器均已在核实身份及无活动 Operation 后关闭，
见当前目录 `desktop-runtime-stopped.json`。临时 fixture/Plan/日志保留在 ignored target 作证据。
原样例 daemon 与原 Tauri dev 已按原命令恢复；`runtime-final-comparison.json` 确认两个
Project 完整列表、Editor 空列表、全部设置及 revision 155 与测试前一致，开发 GUI 可响应。
