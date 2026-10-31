# 项目状态

最后更新：2026-11-01

## 当前阶段

**M7 IN_PROGRESS / H2-B IMPLEMENTING / H2-A ACCEPTED / Visual Gate 2 PASS**。项目所有者于 2026-10-31 签收本轮 H2-A；
M0-M6 已正式通过人工验收，M7 尚未完成。
本文件是唯一当前阶段入口；ExecPlan 定义实施合同，testing 文档保存各自 SHA/场景的证据。

- 唯一工作目录：`C:\Users\M2922\MyProjects\Rust\ALCOMD`，活动分支 `main`；不另建工作树。
- 验证按风险分级：日常定向测试及必要检查，模块完成后覆盖相关回归，稳定候选再集中执行完整
  门禁。三平台 Hosted CI/CodeQL 用于重要技术检查点、阶段收尾及最终验收候选；不逐小修改或
  提交重复运行。已运行 CI 不因此取消，推送仍按原授权；最终结果必须对应实际候选 SHA。
  具体安排见 [M7 验证计划](exec-plans/M7-official-gui-extension-ui.md#验证与-release-blockers)。
- H2-A 验收检查点：`23f545245f613f47f8def1e16418c4d12303404e`，包含本轮架构、P6 wire、
  测试门禁、真实桌面交互修正及 Visual Gate 2 签收记录。其前置检查点 `4eae1d4` 包含
  Core `626ec35`、共享 UI `fbb4213`、GUI `7357d80` 和对应验收记录。
- 本轮开始时本机 HEAD 与远端 `main` 均为 `736f55a9eea504442fa8d95e6c26cfc5559a40b7`；暂存区与
  tracked 工作树在本轮开始时为空，原有 `output/playwright/` 两张截图保留。
- 项目所有者已批准依次完成技术收尾、H2-B 实施、真实桌面候选。技术收尾已完成跨平台测试与
  Wasmtime/rustls/ZIP 官方安全公告的逐项评估及获准补丁；跨不兼容版本等仍按独立审批边界执行。
  技术提交独立于 H2-B。所有者已先后明确批准技术 SHA `73f465b` 与后续
  `02b6f0c3e81b8eedb711123ef9de2edc228792ab` 在保留各自本机 GUI 失败记录的前提下快进推送，
  以同一最终 SHA 三平台完整 required jobs 与 CodeQL 全绿验证本轮技术收尾；适用漏洞已修复且
  其他公告完成适用性判断后，H2-B 自动获准实施。
- 本轮已选定并锁定 Wasmtime 48.0.8、rustls 0.23.46、rustls-webpki 0.103.16；项目所有者另行
  明确批准 ZIP 精确 9.1.3、必要传递闭包与 API 适配，保留既有归档格式、安全和事务。当前
  RustSec 版本匹配为 6 项未启用的 QUIC/wasm32 证书包链，维护/unsound/yanked 另逐项记录。
  [技术收尾证据](testing/m7-technical-closure-20261031.md)保存完整公告、锁图与每轮实际结果；
  归档安全与双向格式回归、完整 Rust 491/0/8、前端检查/构建、冻结基线、并发 CLI 启动、
  release bins 和 Tauri no-bundle 已通过。首个技术提交的完整 Chromium 为 237/10，10 项在文档或
  必需静态 JS 加载时遇到 ERR_NO_BUFFER_SPACE、业务未到达，该失败及其额外推送批准保持。
  原样例 Core/开发窗口已恢复，Projects/Editors/Operations/Settings 与任务前一致，revision 155。
  获批技术 SHA 已在 `origin/main` 核实；
  [CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/41204615683) 四语言成功。
  [三平台 CI](https://github.com/ALCOMD/ALCOMD/actions/runs/41204615658) 已结束：Windows 完整成功，
  browser 247/0；macOS 的 Hub 8 项通过，后续候选 RPC fixture 超出 103 字节 socket 限制失败；
  Ubuntu browser 245/2，固定宽 Select 的字体换行与表格测试缺少剩余空间已由诊断复现。
  macOS 现沿用已有短临时根规则，保留 UUID 与原断言，定向 1/1、完整本地 491/0/8 通过；
  Select 仅改已有内容宽度规则，表格保留全部原断言，相关定向 3/3。后续候选根 npm check/build、
  release no-bundle 通过；独立完整 Chromium 为 **246/2（15.5 分钟）**、150 个输入未变：一项
  跨越 30 秒 test budget，另一项标题检查失败后才有完整页面帧，批量业务未到达，不能计为通过。
  未改系统 TCP、浏览器 flags、超时、重试或包管理实现。后续技术 SHA `02b6f0c` 已经按明确批准
  快进推送，原样例再次读回一致；[该 SHA 三平台 CI](https://github.com/ALCOMD/ALCOMD/actions/runs/41209763063)
  已结束：Windows 与 macOS 完整成功、browser 均为 248/0；Ubuntu 为 247/1，唯一失败是
  257 目标用例的 5 秒包管理标题检查。[CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/41209762434)
  已四语言成功。大列表后续候选将未打开的行菜单延至首次使用，并在内容获得一次绘制机会后
  初始化滚动容器。相关包管理、滚动条及导航 63/0，前端 check/build、release no-bundle、xtask
  与 metadata 通过；独立完整本地 Chromium 为 **247/2（18.4 分钟）**、150 个输入未变，
  257 目标与新增菜单回归通过。两项失败分别为必需 JS 的 ERR_NO_BUFFER_SPACE 和四宽度网格
  累计 30 秒预算耗尽，未覆盖部分保持未通过。所有者已另行明确批准精确技术 SHA
  `1932140a815dd12712cd00108d70acebe51a3560` 保留本机失败后普通快进推送，已核实该 SHA
  位于 `origin/main`。[CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/41221092450) 四语言已全部
  成功；[该 SHA CI](https://github.com/ALCOMD/ALCOMD/actions/runs/41221092743) 已结束：Ubuntu 与
  macOS 完整成功、browser 均为 249/0；Windows browser 为 248/1，唯一失败是正常动画分支
  的一次性滚动条样式读取为 0 秒。六组合本地观测符合原阈值，没有远端 trace 可确定唯一原因；
  后续测试候选在默认断言时限内等待实际样式并增加 property 校验，保留数值阈值与全部业务检查。
  动画专项 9/0、前端 check/build 与 metadata 通过；独立完整 Chromium 为 **248/1（12.6 分钟）**，
  150 个输入未变。唯一失败是必需静态 JS 的 ERR_ADDRESS_IN_USE，Hub 页面根为空，输入/保存/
  焦点业务均未到达。未重跑或改 Hub；原样例五项 RPC 再次完全一致、revision 155。
  本机完整门禁仍失败；所有者已明确批准精确 SHA `97c761f76613ea68ebc7a93e4695940dabb969dd`
  保留该记录后普通快进推送，已核实位于 `origin/main`。
  [CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/41228566315) 四语言已全部成功；
  [该 SHA CI](https://github.com/ALCOMD/ALCOMD/actions/runs/41228566567) 三平台完整 required jobs
  已全部成功，browser 各 249/0，Windows/Ubuntu 为 4.8 分钟、macOS 为 9.3 分钟。
  同一最终技术 SHA 的集中检查点已通过，前述本机失败记录保持；H2-B 按既有授权开始实施。
  GitHub 仍 open 的 11 条告警与任务开始时集合相同，当前锁定版本已不匹配其影响范围；
  告警状态未人工修改，完整 RustSec 及维护判断仍见技术记录。
- [H2-B 既有方案](exec-plans/M7-official-gui-extension-ui.md)现为
  **APPROVED / IN_PROGRESS**。模板分页、三类 review、页面任务持有与成功刷新、用户包详情已接线。
  分页/创建 14/0、资源与相关来源回归 18/0、普通模板动作 6/0 分别通过；扩展任务组为 30/4，
  含一项导航 ERR_NO_BUFFER_SPACE 及三项后续修正的测试定位；其后任务专项为 18/1，唯一失败为
  必需 JS 的 ERR_NO_BUFFER_SPACE、业务未到达，不能拼接为完整通过。当前准备稳定
  候选的集中验证与隔离 native 闭环，详见 [H2-B 开发验证](testing/m7-h2-b-20261101.md)。H2-B 只允许本地候选
  提交，不自动推送；真实桌面材料完成后停止等待项目所有者验收。
- 已验收范围：Project/Packages 和已获准的其他既有页面基础交互修正、统一公共组件及验收。
  H2-B 按上述前提顺序推进；H3-H7、M8、M9、M11 未获准开始；本次签收不代表 M7 完成。
- 当前格式：State v16、Config3、RPC v1、Extension ABI v1。按 A-034 保留 Schema 识别，
  未发布开发库不承诺长期兼容；不自动清空现有数据库、Plan、Operation 或恢复证据，
  首次发布前的初始 Schema 整理是独立审计任务，当前版本与迁移保持。
- 目录整合及旧修改恢复方式见 [工作目录整合记录](testing/workspace-consolidation-20260917.md)；
  `codex/archive/*` 仅用于恢复历史，不是活动开发线。

## 当前有效实现

- 包管理：Core `packages.queryProjectCandidates` 提供来源保真、SemVer/Unity/歧义、
  snapshot/paging/quota 证据；GUI 行内版本、条件更新、Update All 和完整选择集批量操作复用
  单次既有 Plan、显式 Apply 与一个 Operation。经项目所有者批准，P6 source/reinstall/bulk
  的实际 wire 已修正为冻结 camelCase，候选使用独立 CandidateSource 保持冻结 snake_case；
  真实 Tauri 六种 Plan review 和公开 RPC 8 项复核通过。普通 resolver 和来源优先规则保持。
- 公共 GUI：真实 Material Web、DesktopList、PageHeader、SplitButton、Dialog、ScrollArea 和
  分段控件统一持有组件规则；默认桌面窗口 1400×800，容器圆角 28px，不增加窄窗口适配。
  Open Unity 继续使用原 leading play 与 18px button icon。异步包 Plan 保留稳定原控件焦点，
  MenuItem 支持公开 Enter/Space 选择；Select 保存后恢复可用且选中标签随语言同步。
- 主题/语言：共享动态明暗九方案、默认 `#6CB6FF`/Vibrant，主题弹窗实时预览并明确 Save/Cancel；
  字体仅使用本机 Noto/CJK fallback；三语言为英语、简中、日语，静态词条与 React 文本插值。
  动画偏好 system/full/reduced；普通设置修改即经串行 CAS 保存，不再有全局 Save/Discard。
- Core 设置：application 持有权限/CAS/校验/候选缓存和取消一致性；Store 的 ConfigStore
  适配器持有文件解析、16 KiB 界限、原子替换与恢复。Config1/2 是已有迁移输入，不是当前写入格式。
- 项目时间：lastModifiedAtMs 维护批准的注册/接受启动/成功变更边界，未知不补造；
  项目 observedAtMs 已从活动模型和 State16 移除，安装扫描/writer/迁移专用证据保留。
- Unity：精确版本 0/1/多安装启动与迁移保持；Hub 路径选择、编辑、自动查找及打开/下载已接线。
  打开经窄权限和 local-owner 校验，Core 重验 revision/路径，平台有界回收自己创建的 Child；
  未确认草稿不隐式保存并启动，未知结果不自动重试。

当前合同见 [M7 ExecPlan](exec-plans/M7-official-gui-extension-ui.md)、
[候选合同](exec-plans/M7-package-candidate-evidence-stop-a.md)、
[Config3](../specs/config/settings-v3.md) 和 [Hub 合同](../specs/config/unity-hub-location.proposal.contract.json)。

## 架构审查修复与验证

本轮修复五项已审阅问题：

1. UtilityWorkspace 的离线 Retry/Refresh 使用共享 reconnect，恢复页面数据和连接能力。
2. Available/Missing source 只使用完整 Core providers/肯定证据，可选元数据首页不再决定状态。
3. hello.dataSchema 由 daemon 提供真实存储版本，新增 dispatcher 和实际 SQLite user_version
   一致性回归，不让 protocol 依赖 Store。
4. 公共列表正文统一稳定 typography 行盒，处理 Noto/CJK 混排，不加页面特例或放宽原断言。
5. 当前架构/RPC/ExecPlan/状态原条款直接同步有效格式、依赖、范围及 Schema 策略。

P6 修复后的 fmt、Workspace Clippy、xtask、metadata、前端 check/build、生产
daemon/CLI/Extension Host build 和 Tauri **release no-bundle** 均通过。完整 Workspace 为
**483 passed / 0 failed / 8 既有 helper ignored**。新增 P6 44 项 shape 与候选两种来源的
嵌套 Serde 回归通过，候选冻结 Schema/vectors 的 hash 不变。
P6 wire 修复轮完整 GUI 为 **238 passed / 0 failed**（9.0 分钟），单 worker、零重试，
不代替下述 H2-A 最终结果；此前逐轮失败与 trace 保留在对应 testing 报告。

真实 IPC 测试现先等待自建 daemon 初始化并绑定成功，服务 task 结束后仍由测试 handle
保留端点所有权；35 个启动点已接线，四项所有权回归通过。Copy 用例改为独立 writer RPC
证据 oracle，保留全部 Apply/内容/注册/replay 验证，并新增 Apply 重验 confirmed 无副作用
回归。生产 Copy 仍只对 confirmed 硬拒绝，suspected/unknown 为风险证据，未停止用户 Unity。

GUI fixture 预构建三个现有入口，保留 development React/StrictMode、现有工具链和全部
断言。真实浏览器 HTTP 响应从 148/116/126 降到 7/5/5，独立 HTTP connectionId 从
6/6/6 到 6/4/4，HMR WS 从各 1 到 0；请求数未当作 TCP 数量。本轮时段未查询到新增
System/Tcpip 4227 事件，不据此承诺 Windows 全局端点限制已被消除。

本轮真实桌面使用新的临时 Core、独立 WebView2 profile 和纯 JSON/文本包，已完成真实
GUI Apply 1.1.0、Running→Succeeded、自动刷新，并在 Activity 精确核对同一 OperationId。
另由正式 CLI 将项目降为 1.0.0 / revision 3，原 GUI Plan 明确拒绝；拒绝前后完整项目、
Operation 列表及文件 hash 相同。Settings 真实断线/Retry/重连恢复数据和能力、分组键盘
导航、双向弹窗 Tab/Escape/返回焦点、来源/预发布筛选、主题/语言和 Core/GUI 重启持久化
均有记录。Hub 原生选择、同组草稿/Escape 和实际 Open 已检查；下载和无现成 fixture 的
native Portable UI 按清单记未执行。

最终 release GUI SHA-256 为 `0AA8ADA4BABA8192D6953FBEA6E93CE106942B4030B02F132E9776EBF435BCD2`。
前端 check/build、release no-bundle、根 npm check、xtask 和 metadata 已通过。焦点修正轮
完整 GUI 为 245/0；最终候选首轮为 246/1，唯一失败为必需 JS 资源的
`ERR_ADDRESS_IN_USE`，同刻 Windows Tcpip 4227。保持输入、断言、超时和零重试后，独立
完整重跑为 **247 passed / 0 failed（9.4 分钟）**，前后代码/测试 hash 一致，该时段无新增
4227。各轮和定向结果独立保存，不声称系统全局端点限制已消除。

临时 GUI/Core/服务器已关闭；临时偏好值恢复初始，原 daemon/Tauri dev 按原命令恢复。
两个原样例 Project、Editor 空列表、Operation、全部设置和 revision 155 与测试前相同，
开发 GUI 有窗口且可响应。每张 native 截图标记实际构建 hash；项目所有者已签收本轮
Visual Gate 2，没有 M7 或 Hosted CI 通过结论，原未跟踪 `output/` 截图保留。
详见 [Visual Gate 2 签收记录](testing/m7-visual-gate-2-20261031.md)、
[P6 与前一轮门禁](testing/m7-test-gate-closure-20261031.md) 和
[架构审查原轮次](testing/m7-architecture-review-fixes-20261031.md)。

## 已有验收证据

| 检查点 | 已证明的范围 | 证据 |
| --- | --- | --- |
| M0-M6 | 对应里程碑及项目所有者人工验收完成；M6 final `b666e8f` / CI `32724915827` | 各里程碑 ExecPlan |
| M7 Portable UI B0-D | `aa13234` / CI `32877438910` 三平台技术通过 | M7 ExecPlan |
| Project/Package P0-P8、exact Unity | 原技术检查点保留；Unity `e9349fd` / CI `33352406804`、CodeQL `33352406214` | Project/Package、Unity ExecPlan |
| `4eae1d4` 前的本地 Hub/设置检查点 | 完整 Workspace 474 passed / 0 failed / 8 既有 subprocess-helper ignored；Clippy/fmt/xtask/metadata/check/build 与活动 Schema 12/12 通过。Hub browser 首轮 43/45、定位修正后专项 3/3，不能合称一次完整 GUI 通过 | [Hub 记录](testing/m7-unity-hub-location-20261031.md) |

不同 SHA/轮次的日志不能合并为当前候选通过。三平台技术结果不等于 Windows 10/11 完整客户端发行验收。

## 剩余边界与下一停止点

- P6 修复的契约、自动化与真实 Plan 证据保持；本轮 H2-A 与 Visual Gate 2 已获项目所有者签收。
  技术收尾已达成同一 SHA 的完整远端检查点，当前实施既有 H2-B；下载、native
  Portable UI 等旧未覆盖范围保持，不由 browser fixture 或窗口存在替代真实动作。
- 窗口记忆、仓库 credential enrollment/revocation、完整用户入口缺口保持真实状态；聚合
  projects.management/packages.vpm/repositories.management 等继续 in_progress，不批量提升。
- projects/templates/backups 的 v3 parity 和 unity.v3-vrc-parity 仍等待 M11 真实脱敏 Fixture；
  synthetic/public 工程测试不能冒充真实 differential parity。
- M12 继续承担 Windows 10/11 安装、运行、WebView2、更新/卸载、三平台真实辅助技术与完整发行验收。
- MCP、Discord 产品逻辑、Local API、v3 migration、bootstrap/updater/installer/dist 属于后续范围。
  已有开放依赖告警仍需独立适用性评估，不因这轮 GUI/握手修复关闭。
