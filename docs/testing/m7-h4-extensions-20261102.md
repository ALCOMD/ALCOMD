# M7 H4 Extensions / Portable UI 本地候选

状态：**STAGE_ACCEPTED_WITH_FOLLOWUPS**，2026-11-02。所有者已实际查看并阶段性签收 H4，认可共享错误可读性整改，
保留一般文案、布局及过度设计的非阻塞体验待办。H2-A/H2-B/H3 签收保持，M7 进行中；H5 已批准，H6/H7 与推送未授权。
下文各轮运行保留其当时状态，当前签收、环境恢复与未覆盖范围见末节，不把历史待验收状态当作当前状态。

## 范围与候选

标准目录/main，当前 GUI 为 `52ed23bc41f01bfcfa0a7a3fc18cb5db56aaa766`，Core 保持 `e2b7dddf824bc4fb064b7cc682e3a25a40ab4ffa`。
公开只读合同为 `0f8c9f4`；前轮 GUI `88c3f69304cc3afe77b45d474010c81ebd0152f0` 的 155 项输入/36 项证据仅对应前轮，
不替代本次共享错误显示整改。各轮 Core 与 GUI 的独立输入/构建摘要保留。
`9d8fc3d` 记录 H3 签收，`a913201` 补齐 H3 已审阅的
Windows version.dll 私有模块精确 unsafe allowlist；原 deny/安全注释门禁保持，没有新增 unsafe 或校验放宽。

- 保留已有扩展管理和 17-node renderer，补列表分页、安装预期缺席/冻结申请审阅、同一 Operation、启停/隔离反馈与卸载结果。
- 获批只读合同增加 Plan 权限申请及 `extensions.permissions.list`，沿用 `extensions.read`/所有权；Core 一致读事务和
  keyset cursor 绑定扩展、grantRevision、包和冻结安装证据，变化拒绝混读。State Schema 仍为 16，无新权限/依赖/运行时。
- 权限弹窗分开显示申请与实际授权、友好名称和具体项目范围，set/revoke 后重读，失效/迟到读取不能覆盖新结果；未知写入不自动重试。
- Portable UI 的打开、刷新、操作、关闭和草稿按当前 client/session 隔离；断线重协商，不重放动作。复用 Material/Reminder/进度，
  修复共享 Switch 在受控恢复时勾选与 host 不一致，保留真实 Material 控件及焦点。路径标签复用 `displayPath`，实际参数不变。
- `e2b7ddd` 修复真实 Core 暴露的 required 缺权仍启用，以及历史包声明误允许授权；现有 Store 事务在写入前校验当前冻结声明，
  读取、启用/恢复、运行前检查及 setGrant 共用证据，拒绝不改状态/隔离/revision/实例/授权/幂等，不新增合同或机制。

## 本地技术验证

证据目录 `target/m7-h4-extensions-20261102/`，日志、输入摘要和各轮失败保留，不拼接通过项。

| 检查 | 实际结果与证据 |
| --- | --- |
| 最终 M6 Store | **13/0**；包括原权限读取、分页/冻结、lease/隔离/数据与四项新增 required/历史包授权回归；`core-required-candidate.log`、`core-required-verification.json` |
| 真实 dispatcher 定向 | 2/0；`core-rpc-permissions.log`，该测试未启动 IPC/Host |
| 协议 / M6 契约最终 | 1/0、5/0；`core-protocol-permission-metadata.log`、`core-m6-permission-metadata.log` |
| Rust/SDK | 初次七 crate Clippy/SDK check 保留；Store 变化后再次通过 Store/应用 all-targets Clippy `-D warnings`、两文件 rustfmt/diff；`core-required-verification.json` 绑定新增两项输入，其他输入沿用 `core-verification.json` |
| GUI | check/Material/933 条三语言消息通过；`gui-check-controls.log` |
| 稳定定向浏览器 | **36/0，1.3 分钟，1 worker，0 retries**；`browser-final.log`，前后 155 项输入相同，`gui-code-verification.json` 绑定 GUI SHA |
| 仓库门禁 | xtask、metadata、diff 通过；公共 method 与矩阵 93/93 对齐 |
| 当前构建 | 初次 Core/一次 Tauri release no-bundle 通过；实际缺陷修复后仅再构建 Core（1m26s），`core-required-release.log`、`required-fix-release.json`；GUI 输入未变，不重建 |

浏览器覆盖安装/卸载与未知结果、分页/重试、实际权限读取/刷新/变更、旧响应隔离、全部 17 node、dirty form、
失效 session、重连及相关 H3/共享 Material 回归；不是全浏览器套件。未重跑完整 Rust Workspace、旧 292 项 Chromium 或三平台 CI。
Vite 保留原有 chunk size warning，构建 exit 0；没有为消除 warning 重构打包。

| 历史运行 | 结果及处理 |
| --- | --- |
| 首轮浏览器 | 20/15；保留 `browser-directed.log/results`。定位受控 Switch、隐藏弹窗重复任务及 fixture/locator 问题 |
| 第二轮浏览器 | 26/10；保留 `browser-candidate.log/results`。修正 fixture 生命周期、公开语义 locator 与 Escape 焦点操作 |
| 修复定向 | 11/0；`browser-repair.log/results`，不加到后续完整定向计数中 |
| 首轮 M6 契约 | 4/1；production scanner 误扫新增 browser tests，改为仅排除测试目录，生产断言保留，随后独立 5/0 |
| 首次 xtask | 缺少 H3 私有版本适配器的精确 allowlist；审阅后补该文件，`xtask-final.log` 通过，未放宽其他模块 |
| 初次 GUI check | 四处类型/fixture 问题已修；初始日志保留，最终 933 条消息检查独立通过 |
| required 缺权反例 | 修前 1/2；`core-required-before.log`，后台/项目必需权限错误启用，UI-only 用例通过；修复后中间 12/0 独立保留 |
| 历史包误授权反例 | 修前 0/1；`core-required-current-package-before.log`，旧包声明错误允许新包不申请的授权；最终独立 13/0，不拼接中间轮 |

## 构建与真实隔离 Core/Host

| 产物 | SHA-256 |
| --- | --- |
| 最终 Core `target/release/alcomd.exe` | `7d62a6c8aed0eb6bba3ebc6b33f037d7fd18aebaf1186c109487c310d5da7a18` |
| 当前 GUI `target/release/alcomd-gui.exe` | `1233815a7bd79b06a2a5ec7bcd7f5111cfa76e9786a672b62ec081fa4942c76e` |
| 复用既有 Host `target/release/alcomd-extension-host.exe` | `ceecfc1882511e3634d98c32de94bdc316c8cb31aeb271d36002b767bb297a82` |
| 初次 Core，已被功能修复替换 | `f6b0e52fd4557db1477adb212fad569d1af9cfa9b0781e5f0a1c61f39951215d` |

Host 本轮未重建；以下先记录初次 Core 的真实运行结果及反例。初次隔离 Core PID 137392，数据只在 `target/m7-h4-extensions-20261102/data-native`。
每次公开 RPC 请求均核对唯一 Core 的 PID/创建时间/可执行文件摘要/命令行/数据目录；GUI 未访问数据库。
测试包来自仓库既有完整 WASM fixture，以公开测试密钥签名，仅经 Core 正式公开 Plan/Apply 安装，没有用户扩展/真实工程写入。

- `com.example.h4.portable` archive SHA `f6d8ffca7c9fec2623143832f68134f004dcf53f6e3ee94410c3df45f378dea3`，
  package digest `ba0e16cce021508d8fa5eaf1586571d99a930230c9495c7a15e0ec31fc320efe`。安装任务
  `78089660-90cf-41c8-93db-ac903e65dff3` succeeded，默认停用、grants 为空；申请 `projects.read` 不等于已授权。
  只为旧隔离恢复样例登记 ProjectId `f2bb8e2a-2a8a-4dea-ab59-7670e89b8454`，授权后准确返回该 Project 范围，撤销后为空，grantRevision 1→2→3。
- 同一认证连接内 Portable UI open→dispatch→refresh→close 成功，snapshotRevision 1→2→3、closed=true。
  `native-rpc/portable-ui-same-connection.json`。真实 WASM 样例只有 page/button 两个 node；17 node 和 dirty-form 是浏览器覆盖，不冒充该真实样例内容。
- `com.example.h4.quarantine` 为故意 trap 的既有 fixture。安装任务 `44735877-d74d-4746-b9e5-bf3c6a9d382c` succeeded，
  未授 `background.run` 时 desired enabled 但 runtime stopped；声明没有自动变成授权。显式授权后观察 crashed→quarantined/stopped。
  **这次未授权 enable 返回成功违反既有“required 未授权不能 enable”合同，是实际功能缺陷，已由 `e2b7ddd` 修复；初轮反例保留，不当作正常启用验证或非阻塞文案问题。**
  显式恢复返回 **`extension_crashed`** 且仍隔离，属于失败结果保持，不能称作恢复健康；随后显式停用。
  retain-data 卸载任务 `32875285-32e5-47d6-85a3-d6b0bb203cec` succeeded。delete-data 未做真实写入，GUI 审阅由浏览器覆盖。
- `native-rpc/` 保留所有请求结果。最初逐次断开连接的 helper 在 dispatch 得到 session_not_found；按既有连接所有权改为同连接验证，
  未改 Runtime。helper 初次误写 `extensions.permissions.setGrant` 得到 method_not_found，重读确认无授权副作用后改用既有
  `extensions.setGrant`；生产 client 原本名称正确。首次 hello 的 PowerShell void 输出污染也独立保留，helper 修正后重读。
- 样例仅经 CLI 登记与 Core 只读查询，四文件前后 SHA 相同；未启动、迁移或关闭任何 Unity。测试包/数据和失败证据全部保留。

### 最终 Core 的定向复验

最终 Core PID 127568 使用新的 `data-native-required-fix`，摘要/提交绑定 `required-fix-release.json`。
结果在 `native-rpc-required-fix/` 与 `required-fix-native-verification.json`：

- 两个相同签名 fixture 经新 Core 安装成功，required 后台包任务 `bcb90fd3-5ed3-4c31-a420-93254e04ed47`、
  UI-only 包任务 `742f2579-5f69-4a6a-9fce-4537a632b283` 均 succeeded。
- required 未授予的 enable 返回 `permission_denied`，前后完整公开安装记录相同，grants 仍空、grantRevision 仍为 1。
  未声明的 background 授权也被拒绝，无自动授权。
- required=[]、仅 optional projects.read 的 UI-only 包仍可启用；同一认证连接的 open/dispatch/refresh/close 全部成功，revision 1→2→3。
- 明确授予当前声明的 background 后，之前被拒请求的同一 key 可继续调用，执行到故意 trap 并返回 `extension_crashed`，
  随后观察 quarantined/stopped；不是 idempotency 冲突或误报健康。两个样例均已显式停用，Host 已收束。
- 历史包声明不再授权的卸载/重装链由最终 13/0 Store 回归覆盖；没有冒称这条链已在原生 GUI 操作。

## 前轮原环境恢复、窗口与缺口（历史证据）

暂停前五组完整公开状态已核对且任务全终态。为释放构建中的旧 exe，先把原 Core 安全切到原摘要的保留副本 PID 131660；
首次隔离结束曾恢复到 Core 130424；发现功能缺陷后用保留副本 116656 维持原数据运行并仅重建 Core。
最终隔离结束实际恢复到修复后的 release Core PID **135272**，原数据目录 `target/m7-icon-acceptance-20260906/data`。
Projects 2、Editors 1、Operations 2（全终态）、Settings revision 155、Status ready 与暂停前完整结果一致。
`original-restored-runtime.json`、`required-fix-native-restore-*/five-group-comparison.json` 及初次独立恢复目录保存实际过程与结果，不是守候进程承诺。

前轮 release GUI PID **128448**、窗口 **727218**、标题 ALCOMD3，当时打开且可响应，使用独立 WebView profile。
隔离 GUI 108656、两轮 Core 137392/127568 已停止，无残留 Host；此前本轮验收 GUI 135172 已在 Core 构建前按身份核对关闭。
没有处理用户 Unity 或未知进程。
原数据在本轮前没有扩展，没有把测试包安装到原数据中。前轮只打开原窗口，尚未具备完整 H4 人工验收条件；本轮准备见下节。

桌面工具收到物理 Esc 后保持中断，**本轮新增 GUI 原生交互及整改后原生截图未完成**；未切换桌面入口或重置工具绕过。
构建成功、进程可响应、真实 Core/Host smoke 和浏览器通过分别记录，均不替代 native GUI 验收。
整改前完整窗口截图位于
`C:/Users/M2922/.codex/visualizations/2026/08/17/01a00f12-6751-7d50-9346-7b58e94f2eea/h4-extensions/`，
文件为 `before-extensions.jpg`、`before-install-dialog.jpg`；没有冒称当前整改后图。

本轮用下面的隔离样例、实际入口与四项清单交付，不要求所有者往日常数据安装测试包。

## 本轮可操作隔离候选（待人工验收）

该样例准备轮保持 `e2b7ddd` 与当时 GUI 输入/构建，不构建、不重跑测试套件、不调用 Computer Use 或其他桌面输入接口；
随后发生的共享错误显示整改及新 GUI 见末节，不能继续把旧构建称作最新。
复用原签名包和 `data-native`，只补正常样例的实际准备：正式 Core retain-data 卸载→重装→启用→停用→卸载全部成功。
任务 `15ae6e46-7023-46c3-ad0c-39fb80de0207`、`f250b565-c83f-46cd-bb28-cdce8bd9a07a`、
`e7f2208e-97ca-4bc5-8e91-cd10d3d11576` 为该准备轮，原始结果在 `native-rpc-review/`；不加到旧 13/0 或 36/0 计数。

正常样例统一为 `com.example.h4.portable` / `1.0.0`：required 无、optional `projects.read`，不申请后台运行，UI 为现有 Fixture UI。
包 SHA、签名、发布者及 Core 接受证据沿用前文，并再次核对文件摘要。正式发布者批准与包校验没有跳过。
当前扩展列表为空是**有意留出安装起点**，不是只有一个空窗口：下列可安装包及一个已登记的隔离项目均已就绪。

样例包（复制整行到 Extension package）：

```text
C:\Users\M2922\MyProjects\Rust\ALCOMD\target\m7-h4-extensions-20261102\native-fixture\com.example.h4.portable.alcomdext
```

项目选择使用 `H3-Restored-20261101`（路径位于 `target/m7-h3-contextual-20261101/native/restores/`），
ProjectId 为 `f2bb8e2a-2a8a-4dea-ab59-7670e89b8454`。仅该可丢弃项目登记在本隔离环境；四个文件 SHA 未变，不访问用户实际工程。

### 已验证入口和当前窗口

以下固定本地入口复用已有 Core/GUI 启动方法、独立 WebView profile、完整公开分页读取和身份围栏。
命令显式使用已验证的 PowerShell **7.6.5** 宿主；不依赖系统 Windows PowerShell 5.1。
已实际执行 Start→Restore，恢复时 Projects/Editors/Operations/Settings/Status 五组与暂停前一致，随后再 Start 交付；
不是把新脚本的语法检查描述成启动验证。入口与依赖 SHA、往返结果见 `manual-review-preparation.json`、`review-entry-verified-cycle.json`。

```powershell
# 当前已经启动；只有关闭并恢复后需要再次打开时才运行 Start。
& 'C:\Users\M2922\.cache\codex-runtimes\codex-primary-runtime\dependencies\native\powershell\pwsh.exe' -File 'C:\Users\M2922\MyProjects\Rust\ALCOMD\target\m7-h4-extensions-20261102\H4-Review.ps1' -Action Start

# 人工查看结束、等待安装/卸载任务终态后恢复原环境。
& 'C:\Users\M2922\.cache\codex-runtimes\codex-primary-runtime\dependencies\native\powershell\pwsh.exe' -File 'C:\Users\M2922\MyProjects\Rust\ALCOMD\target\m7-h4-extensions-20261102\H4-Review.ps1' -Action Restore
```

该次交付时隔离 Core PID **138156**、GUI PID **136888**、窗口 **1841230**，标题 ALCOMD3、可响应；后续新 GUI 窗口见末节。
从左侧 Extensions 进入；没有自动导航、点击、键盘输入或截图。**原环境当前暂停等待本次人工查看结束**，不是已经恢复；
Restore 会确认隔离任务全终态、仅关闭记录的本轮 GUI/Core、保留未知进程，实际启动原数据 Core/GUI 并核对五组状态。
最近一次完整往返已恢复到原 Core 123196 / GUI 133036，五组全同；本次再次暂停前也已保存新完整快照。
不停止用户 Unity。若还有活动任务或归属不明的进程，入口拒绝继续，不能只把“等待”写成恢复成功。

### 四项人工清单

按钮以下用英文标签定位；其他语言对应同一操作。只查看 H4，不重验已签收页面。

| 从哪里点击 | 预期看到什么 |
| --- | --- |
| **1. Extensions → Install extension**，粘贴上述包路径，明确勾选 Approve this publisher for this extension → Create install plan | 审阅为 `com.example.h4.portable`、`1.0.0`、本地测试发布者；必需权限无，可选 Read projects，申请不等于已授权。点 Apply reviewed plan 一次，任务成功后出现扩展。可先给路径追加 `.missing` 检查真实失败，再改回正确路径继续：错误留在弹窗，不报成功，重试不沿用旧提示。 |
| **2. 样例卡片 → Manage extension**，Enable → Disable，再 Enable | 状态与操作结果一致，Open Portable UI 随启用可用。完成第 3、4 项后，Uninstall extension → Create uninstall plan，先 Cancel/Discard 检查未移除，再重新审阅并 Apply。默认不勾选删除扩展数据，成功后回列表；无需删除项目。 |
| **3. Manage extension → Manage permissions** | 先看到 optional Read projects 与 No permissions granted；选择隔离项目 → Grant，已允许区出现该项目路径。Refresh/关闭再打开仍一致；Revoke 后条目消失，再授予可恢复。无需手工证明整个权限系统或检查 revision/JSON。 |
| **4. Enable 后 → Open Portable UI**（列表也可点 Open） | 同一应用窗口显示 Fixture UI 和 Refresh 按钮；点击按钮/页头刷新能工作。切换到 Resources 再回 Extensions → Open，应重新打开，旧状态不残留。关闭扩展页用侧栏切换；最后按第 2 项卸载。 |

暂未覆盖的真实桌面状态：这个既有正常 fixture 只有 page/button，不含可编辑表单，不能据此验收 dirty-form；
会话超时、外部断线/撤销的迟到反馈未在当前桌面强制造出，也没有新截图。相关浏览器/Store/真实 Core 证据保持各自范围。
故意 trap 的隔离/恢复失败已有前文 Core 证据，不要求为了本次四项清单再安装第二个异常扩展，未宣称恢复健康或人工通过。

### 两项权限缺陷与范围限制的复用证据

最终独立 **13/0** 绑定 `e2b7dddf824bc4fb064b7cc682e3a25a40ab4ffa`。实现/测试 SHA 分别为
`968532113e6be6dd6f637f78b463ff34be32400a8fe3c3e7778ec703cf304ab5`、
`eaa4109d9fe2be024fcbf2a64dba66af3bcb0979e03f62ee25d6eea637b50fa8`，本轮未变、未重跑。
下表行号均位于 `crates/alcomd-store/tests/m6_runtime.rs`，方法名称与逐项结果见 `core-required-candidate.log`。

| 要证明的结果 | 已有真实证据 |
| --- | --- |
| required 缺权不能误报启用成功 | `m6_runtime.rs:204` 的 required background/恢复回归：PermissionDenied、四表完整不变、不耗 key、授权后同 key 可用。真实新 Core `native-rpc-required-fix/required-enable-refused.json` 及前后记录证明结果和权威状态一致、grants 空、grantRevision 1。 |
| 历史声明不能给当前包授权 | `m6_runtime.rs:411` 的旧包→retain 卸载→新包重装回归：旧 background 权限拒绝、四表不变，同 key 可授当前声明的项目权限。真实新 Core `undeclared-grant-refused.json` 另证当前未声明权限拒绝；没有把它冒充真实历史重装链。 |
| 授权、撤销与项目限制持续生效 | `m6_runtime.rs:559` 证明 grant/revoke 与准确 ProjectId；`:1127` 证明获授项目可读、范围外 ScopeDenied、撤销后旧 lease InstanceStale；`:352` 证明项目 required 必须有明确范围。旧真实项目 grant→revoke 的 1→2→3 仍绑定旧 F6B0… Core，不写为本轮新跑。 |
| 撤销/包变化不能混读分页 | `m6_runtime.rs:618` 的完整有序分页与旧 cursor RevisionConflict，包含在最终 13/0。不是 GUI 假客户端错误。 |

H3 阶段签收保持，H4 继续等待项目所有者实际查看确认，M7 进行中。

## 远端验证

H2-B 新增代码、H3 与本 H4 均**尚无对应远端 CI/CodeQL**，本轮未推送或触发 workflow。
`0ef5b088dfdb7a8eb2b7a88ebef94ae436c90f50` 的三平台 CI `41287607183` 与 CodeQL `41287607108`
成功继续作为该旧 SHA 的历史证据，不代表本轮新增代码远端通过。

## 共享错误提示整改（52ed23b，签收前历史记录）

所有者截图中发布者确认框未勾选，Core 正确返回 `extension_publisher_confirmation_required`；界面却恒定显示
“请求失败/未能完成请求”加原始 code，未说明原因。源码确认这不是单点：共享 RequestReminder、页面初读/刷新、
分页、模板任务/创建以及资源逐项错误都有同类显示或丢失原因。

- 共享有限映射覆盖当前扩展码及常用通用/资源/项目错误，按稳定码给出中英日可读说明；不猜未知原因，不显示原始 RPC message/data。
- 当前这项显示“请确认扩展发布者”，明确说明只有信任此扩展时才勾选“为此扩展批准该发布者”，再创建计划。
  不自动勾选、不把未确认等同签名无效，不绕过 Core，不取消后续安装审阅。
- code 与 diagnosticId 默认在“详情”，保持可排障；错误切换重置展开状态，原因与当前上下文/未知写入说明同时保留。
  公共加载/分页保留诊断信息、已有数据及原重试规则；行内提示只换文字，不重设计页面。诊断页面的 code 仍是明确的诊断元数据。

证据目录为 `target/m7-h4-readable-errors-20261102/`：GUI check **1021** 条消息、Material/类型检查通过。
七个受影响 browser 模块首轮 **78/1（3.2 分钟）**，前后 **173** 项输入不变。唯一失败是新详情按钮被测试点击后，
旧断言仍要求开关保有焦点；把 checked/focus 断言放在展开详情前，全部断言保留。生产文件未改，仅复验该项与五项新增
错误说明/三语言/未知错误不泄露原文/发布者确认回归，独立 **6/0（13 秒）**，173 项输入不变。不相加、不称作独立 79/0。
`verification.json`、两轮日志/输入摘要及旧失败 trace 保留。

一次更新后的 Tauri release no-bundle 构建通过（2m02s），Core 未重建；新 GUI SHA 为
`985e11edb32b867c10afeda0d697ff9b5c564ddb9ebbe862684a166a88222b11`，原 GUI 已保留为 `previous-gui.exe`。
当前隔离 Core 仍 **138156**，GUI 已替换为 **128624** / 窗口 **4068254**，标题 ALCOMD3、可响应；
更新前任务全终态，只有记录的本轮 GUI 被关闭，没有操作用户 Unity、原 Core 或未知进程。新 GUI 仍使用同一隔离数据。
`H4-Review.ps1` 仅同步新 GUI 摘要和源码标识，恢复逻辑不变；新入口语法/进程身份已核对，未为文案更新再跑完整往返。
早前 `manual-review-preparation.json` 的入口摘要/窗口属于前轮，新摘要与当前记录见 `candidate-runtime.json` / `review-session.json`。

没有恢复 Computer Use、没有新增原生截图或声称完成原生体验验收。当前扩展仍未安装，所有者可用同一签名包从安装审阅继续。
原环境仍暂时暂停，验收结束继续由既有 Restore 实际恢复并核对五组状态；H3 原签收保持，H4/M7 未签收完成。
当前并非所有业务码已逐项人工覆盖：未知码仍给通用失败与可展开标识，包链接失败的既有隐藏反馈承载未重设计。
正式 MCP/Discord 业务仍留给 M8/M9，本次没有新增产品功能、RPC、Schema、权限、依赖或推送。

## 阶段性人工签收与实际恢复（2026-11-02）

项目所有者已检查扩展功能并明确 **H4 阶段性人工验收通过，保留非阻塞体验问题**。
保留安装、启停、卸载、权限管理与同窗口 Portable UI 成果；共享错误可读性整改获认可，后续复用该组件。
一般文案、布局及过度设计只在现有 status 待办短记，后续优先删减、合并、改写，不继续扩大本轮整改。
样例未覆盖的表单、会话超时、外部断线和正式 MCP/Discord 业务仍未验证；不要求所有者证明整个权限系统安全。
首轮浏览器 **78/1** 和定向复验 **6/0** 原样独立保留，没有完整套件通过结论。

验收后于本地 14:39 实际执行既有 `H4-Review.ps1 -Action Restore`：先取得隔离完整公开快照，确认 Operations 全终态，
只关闭记录的本轮隔离 GUI/Core。实际恢复原目录 `target/m7-icon-acceptance-20260906/data` 的 Core **136212** 和 GUI **132312**；
通过进程可执行文件、命令行、创建时间及摘要核对身份。Core 仍为 `7d62a6…da7a18`，GUI 为 `985e11…222b11`。
Projects、Unity 安装、Operations、Settings、Status 五组完整结果均与本次暂停前一致：Projects 2、Editors 1、Operations 2 全终态、
Settings revision 155、Status ready。隔离数据保留作证据，不继续占用端点；没有安装测试扩展到原环境，未处理 Unity 或未知进程。

恢复证据：`target/m7-h4-extensions-20261102/manual-review-restore-20261102T0639271018499Z/` 的原/隔离快照与
`five-group-comparison.json`（五项均 true），以及 `review-session.json` 的 `phase=restored`、实际 Core/GUI 身份。
调用日志为 `target/m7-h4-readable-errors-20261102/h4-accepted-restoration.log`。
没有恢复桌面控制或伪造整改后原生截图；所有者签收与自动化覆盖分开记录。

本次收尾只做文档适用的 metadata/diff 检查，不重复构建、完整测试或 CI。H2-B/H3/H4 新代码仍无对应远端结果。
H5 已获准直接实施；H6/H7、推送及 M7 整体完成均未授权。
