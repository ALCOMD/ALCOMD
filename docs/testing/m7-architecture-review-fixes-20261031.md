# M7 架构审查修复验证

日期：2026-10-31。基于 `4eae1d4619b4534cf6bd4916e9c282b87be0eec4` 的本地工作树，
仅处理已批准的 Schema 策略与五项审查问题。本报告形成时为
**M7 IN_PROGRESS / H2-A / Visual Gate 2 PENDING**；该轮未提交或推送，当前阶段以 `docs/status.md` 为准。

## 最终实现

1. UtilityWorkspace 的离线 Retry/Refresh 复用已有 reconnect 协调入口，同时重读页面和连接能力。
   新增从离线 Settings 恢复 Hub/Unity 控件、再切回 Projects 恢复操作的回归。
2. 包列表 Available 使用完整 Core providers；Missing source 要求 providers 为空且 Core 给出
   肯定 `none`。可选 metadata 首页、读取失败或未知候选不再决定来源缺失。新增七项回归覆盖
   metadata 后页/失败、ambiguous/unknown/none、真实缺失和候选读取失败，并检查没有遍历
   metadata 后页、预取版本或创建 Plan。
3. `HelloResult::m7_official_gui` 接收 ready daemon 提供的存储版本；daemon 组合处传入
   `alcomd_store::CURRENT_DATA_SCHEMA`，protocol 不依赖 Store。新增实际 dispatcher 序列化/
   DTO 解码回归，以及初始化隔离数据库后读取真实 `PRAGMA user_version` 的一致性回归。
4. 公共 DesktopList 单元格持有正文 line-height，稳定 Noto/CJK 混排行盒。既有 1280/1440px
   全列表长文本断言保持原来的 `rowHeightChanges <= 1px`、nowrap、裁切与溢出条件。
5. 架构、A-034、RPC 正文、M7 ExecPlan、RPC 矩阵和当前状态直接同步现行有效合同。
   89 个实际 RPC 方法与矩阵 89 行集合完全一致，无缺失、额外或重复。

A-034 保留 State v16 和所有现有迁移/数据，不新增版本或迁移，不清空数据库。未发布开发库
不承诺长期兼容，仅为明确需要保留的数据维护迁移；发布前初始 Schema 整理另行审计。
正式发布后严格维护升级和恢复链。数据库策略不自动授权公共 RPC、Extension API 或权限变更；
已获批准的 State16 发布前字段移除继续按其现行机器合同执行，不增加别名或回退。

没有新增生产依赖、公共方法、capability、permission 或 wire 字段；hello 只是纠正已有
`dataSchema` 字段的事实值，现行 Schema 已接受 v16，兼容形状/快照测试继续保持。

## 验证

原始日志位于 `target/m7-architecture-review-20261031/`；当前生产/测试修改的输入摘要见
`source-inputs.json`，恢复后的进程、响应窗口和四个本地二进制摘要见 `runtime-after.json`。

| 检查 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | PASS |
| Protocol + Store 全部测试 | 148 passed / 0 failed / 0 ignored，含 Schema/快照/迁移/实际 SQLite 版本回归；`rust-isolated.log/.exit` |
| 无 IPC 的 daemon dispatcher 定向回归 | 1 passed / 0 failed |
| `cargo test --locked --workspace --no-fail-fast` | 475 passed / 1 failed / 8 既有 subprocess-helper ignored；`workspace.log/.exit` |
| `cargo xtask check`、metadata、冻结基线检查 | PASS |
| `npm run check`、前端 production build | PASS |
| Tauri debug no-bundle、daemon/CLI/Extension Host build | PASS；`tauri-build.log/.exit`、`native-core-build.log/.exit` |
| 修正后的 Select 生命周期 helper 单例 | 1 passed；`gui-helper-confirmation.log/.exit` |
| 最终完整 GUI 回归 | 237 passed / 1 failed（导航 `ERR_ADDRESS_IN_USE`）；`gui-full-final.log/.exit` |
| 导航失败用例原样单独复核 | 1 passed / 0 failed；`gui-navigation-recheck.log/.exit` |
| 恢复后的真实 IPC hello | State16 / Config3 / Extension ABI1，`restored-hello.json` |

Workspace 唯一失败为既有
`m7_project_copy_rpc::project_copy_plan_apply_registers_an_exact_independent_copy`：预期
`not_observed`，实际 `running_suspected`。当时存在
`C:\Program Files\Unity Hub\resources\unity.exe` 进程，产生无法读取的进程证据。
Copy 的既有合同只对 `running_confirmed` 硬拒绝；`running_suspected`/`unknown` 进入风险证据，
不是该 Copy 的生产拒绝条件。这一轮失败源于测试把宿主环境固定为 `not_observed`，发生在
Apply 之前；未停止该用户进程，原始完整 Workspace 仍按这一轮的失败结果记录。
后续测试修正及独立完整门禁结果见 `m7-test-gate-closure-20261031.md`。

首轮完整 GUI 有六项失败，证据保存在 `gui-first-run/`。连续打开 Select 的两项语言测试和
一项新增包筛选测试没有等待关闭动画；测试 helper 现在等待 Material 公开 `opened/closed`
事件。另一个设置分组断言仍缺少已批准的 Unity Hub Path 区块，已更新为完整精确顺序。
搜索输入和 257 项加载首轮失败后仍保留原断言。各轮结果独立记录，不拼接为同轮全绿。

最终完整 GUI 的唯一失败发生在 `page.goto`，浏览器返回 `net::ERR_ADDRESS_IN_USE`，尚未执行
该用例的包筛选断言；trace、截图和上下文保存在 `gui-final-failure/`。源码、断言、超时和
运行参数不变的单项复核通过。完整套件仍按 237/1 记录，不合成为 238 passed。
两种窗口宽度的全部公共列表混排行盒、离线恢复、原搜索/语言/分组失败场景都在最终完整
套件中通过。`source-inputs.json` 中的 11 个生产/测试输入在整轮执行期间均未变化。

## 运行环境与隔离

既有 Windows 集成测试使用当前用户的默认 IPC。首次定向 `m5_unity_rpc` 在样例 daemon 仍
运行时误连该实例，新增了一条临时 Project 和一条临时 Editor 注册，并设置后清空了测试
启动参数；writer gate 阻止了启动。已通过公开 CLI 撤销本轮创建的两条注册，原有两个样例
Project 的路径/revision 保持，Editor 列表恢复为空；审计和幂等历史保留，未直接修改数据库。

受影响且已撤销的记录：Project `6822fe68-eff3-4eb9-be83-656223d5a753`，
Editor `5935cdf0-8c6f-404c-bd31-56e2a87665a4`。两者均指向该测试创建的临时目录，
不属于用户真实工程。

项目所有者随后明确批准暂停并恢复工作区 Tauri dev 与指定样例 daemon。停止前核对完整
进程身份/父子关系及现有 Operation 已结束；完整 Workspace 回归在默认端点空闲时执行。
之后按原命令恢复 daemon 的
`target/m7-icon-acceptance-20260906/data` 与
`npm.cmd run tauri dev --workspace @alcomd/gui`。
恢复证据见 `runtime-before.json`、`restored-daemon.json`、`restored-dev.json` 和对应日志。

本轮没有执行 Hub/网页打开、真实系统选择器或完整视觉/辅助技术验收，没有 Hosted CI 结果。
浏览器 fixture 和运行环境恢复不替代 Visual Gate 2 人工签收；依赖告警的适用性评估属于独立任务。
