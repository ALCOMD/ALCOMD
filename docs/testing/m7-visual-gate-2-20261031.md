# M7 H2-A Visual Gate 2：真实桌面验收与签收材料

状态：**H2-A ACCEPTED / Visual Gate 2 PASS；项目所有者于 2026-10-31 签收**。
本报告只覆盖已批准的 H2-A 与既有页面基础交互，不开始 H2-B、H3-H7 或后续里程碑。

## 候选与证据

- 代码基线：`4eae1d4619b4534cf6bd4916e9c282b87be0eec4` 加当前 `main` 未提交修改。
- 使用嵌入 production 前端的 Tauri **release no-bundle**，不是 development server。
- 环境：Windows 11 Pro 10.0.26300 x64；显示设备报告 96 DPI；内容基线 1400×800，
  主窗口截图 1402×832（含系统标题栏）。不以此代表 Windows 10/11 发行验收。
- 原始记录、过程快照和输入摘要：`target/m7-visual-gate-2-20261031/`。
- 截图与可离线打开的 `Visual-Gate-2.html` 位于仓库外
  `C:/Users/M2922/.codex/visualizations/2026/08/17/01a00f12-6751-7d50-9346-7b58e94f2eea/m7-visual-gate-2-20261031/`。
  `native-evidence.json` 为每张截图记录实际 GUI SHA-256；旧构建的失败证据不作为最终通过图。

最终 GUI SHA-256 为
`0AA8ADA4BABA8192D6953FBEA6E93CE106942B4030B02F132E9776EBF435BCD2`，与隔离 runtime
副本逐字节一致。`final-candidate-inputs-before.json` 保存完整代码/测试输入，
`gui-final-process.json` 保存该构建进程、独立 profile 和启动身份。浅色/深色最终截图分别为
`68-final-release-projects-light.jpg`、`59-final-release-projects-dark.jpg`；深色包工作区与
批量 Plan 为 `62-final-package-workspace-dark.jpg`、`65-final-dark-bulk-review.jpg`。

本次 native 证据跨三个明确记录的 release 构建：原 P6 候选用于断线与初始状态；
`CBC82A37…` 完成焦点/Menu 与真实 Apply；最终 `0AA8ADA4…` 复核筛选/语言、重启偏好、
深浅色及批量返回焦点。每张图分别标记 hash，不把不同构建的截图冒充一次最终构建全量签收。

离线 HTML 的 38 张图片、全部本地引用和截图锚点均已检查。内置浏览器的 URL 策略禁止
`file://`；未绕过该限制，也未将早期 HTTP 草稿预览描述为定稿渲染通过。交付为可离线
打开的 HTML 与同目录原始图片。

## 真实环境隔离

按已有授权暂停原样例 daemon 与 Tauri dev，保留其完整 Project、Editor、Operation、Settings
基线。所有新项目、仓库、包、Plan、Operation 与设置变化均属于新的临时 Core 数据目录。
没有直接写 daemon 数据库、cache 或已注册项目；项目和包变化只经正式 CLI/GUI/RPC。

真实 GUI 使用 release 二进制的逐字节副本和进程级独立 WebView2 profile。隔离 runtime
目录不放置 sibling `alcomd.exe`，只为离线测试阻止 GUI 自动启动默认数据 Core；这不证明
完整安装布局的按需启动。包下载服务器只监听 loopback，两个归档仅含 `package.json` 和
`README.txt`；按块延时仅用于观察真实 Running 进度，不改变产品实现或测试超时。

## 已核实的真实流程

| 场景 | 结果与边界 |
| --- | --- |
| 默认入口与项目视图 | release 从 Projects 启动；同一合成项目的 list/grid、无匹配搜索及清空恢复通过。 |
| 键盘导航 | 保留主导航/实用工具分组；Resources、Settings、Logs、Tasks、Extensions、About 的键盘进入和可见标题焦点已有截图。 |
| 断线与恢复 | Settings 保留 last-known-good 并显示 `daemon_unavailable`；离线 Retry 后 Tasks 禁用；恢复同一临时 Core 后 Retry 恢复页面数据和能力，没有默认 Core。 |
| 候选不足 | 缺少 resolver-ready 元数据时有界说明原因，Install 拒绝鼠标且被 Tab 跳过；补齐外部合成仓库并公开 refresh 后恢复候选。 |
| Plan 焦点 | 新 release 通过双向 Tab trap、Escape、主 Install 与版本 anchor 返回焦点，以及返回后 Enter 重开；版本菜单经 Tab 聚焦、Enter 真实生成 Plan。 |
| GUI Apply | 显式 Apply 安装 `com.example.visual-gate` 1.1.0；观察 Running 与 Succeeded，列表自动刷新，项目 revision 1→2。 |
| 同一 Operation | GUI 安装为 `6574c48b-434c-4f71-b693-33066522df74`；公开 RPC 的 Running/terminal 与 Activity 详情精确一致，终态 `state_committed`。 |
| 失效 Plan | GUI 冻结 1.1.0→1.0.0 Plan 后，另一正式 CLI 操作将项目推进到 revision 3 / 1.0.0；原 GUI Apply 明确提示项目已变化。拒绝前后完整项目、Operation 列表和文件 hash 相同，没有自动重建或额外变更。 |
| 安装结果 | 两次真实安装后的 package 文件集、内容 SHA-256 与对应归档完全一致；验证只读取已注册项目。 |
| 包搜索/筛选 | 无匹配时明确显示空状态；Update All 基于完整候选，不受搜索隐藏行影响。最终构建预发布保存后 Status/Source 均恢复可用；Remote 过滤为空、All 恢复本地包，未额外写设置。 |
| 普通设置 | Language 选择 English 即保存 `en-US`；GUI animation 选择 Disable animations 即保存 `reduced`，没有全局 Save/Discard。最终构建切简中时，动画选中标签立即翻译，值不变。 |
| 主题/重启 | Dark 预览立即生效而 Core 仍为 system；Cancel 恢复原主题且 revision 不变；Save 后保存 dark，保留语言/动画。关闭 GUI 后重启 Core，完整设置及 revision 6 不变；最终 GUI 重启亦保持。 |
| Hub 选择/草稿 | 原生 exe chooser 选择既有 Hub 后从 discovered/null 变为 configured；同组 Tab 保留草稿且 Open 禁用，回输入框 Escape 恢复接受值。完整设置及 revision 在 Tab/Escape 前后相同。 |
| Hub Open | 最终 GUI 明确点击 Open 后返回已有 `Unity Hub 3.22.5` 窗口，未出现 GUI 失败反馈；不读取 Hub 内容、操作账号或安装。窗口可见不证明 Hub 全部就绪。 |
| Activity / Diagnostics | Activity 仅显示 bounded summary、event sequence 和 opaque id；GUI 任务详情没有路径、参数、SQL、stack 或 token。Diagnostics 当前为空，这不单独证明 hostile payload 覆盖。 |
| 状态完整性 | GUI 发起 `state.check`，观察 Running→Succeeded；唯一对应 `bc439561-d19e-4098-8f3b-c4f504e7b990`，Core integrity/foreignKeys 均为 ok。 |

## 交互修正

包 Plan 的异步 source/Plan 准备会正确禁用或暂时移除触发控件，使默认 dialog restoration
失去可靠的原控件。现在动作携带稳定的本地 origin ref；共享 Dialog 等待真实 `closed`、
catalog ready 和 Material 公开 `updateComplete`，再复验 connected/disabled 后返回焦点。
成功接受 Apply 后不会延迟抢焦。版本/overflow 返回既有 anchor。

Material 菜单键盘选择发布公开 `close-menu`，并阻止原生 Enter/Space click。共享 MenuItem
统一在 `closed` 后派发一次，保留 click 去重与 Escape/disabled/removed/closed-menu 防护。
没有改动 capability、eligibility、完整候选、失效证据或权限规则。

真实 prerelease 保存另复现 Status/Source 永久 disabled：Material 的 fieldset 回调把禁用
反射到控件自身，而两处 Select 没有受控的 false 更新。修正只让这两处显式复用同一 `busy`，
保留 fieldset、CAS 和所有业务 guard。共享 Select 的选中标签变化则在公开 `updateComplete`
后调用公开 `select(value)`；不写 SSR 专用 `displayText`、不重建节点、不读取私有 shadow DOM。
独立红测后，合并定向 **19/19**；验证同节点双向翻译、值/焦点/disabled 不变，input/change
为零，Config 只有显式语言写入。最终 native 亦验证选中标签同步、保存后恢复和真实来源筛选。

## 技术验证

焦点/Menu 修正后的一轮完整 Chromium 为 **245 passed / 0 failed（9.8 分钟）**，单 worker、
零重试、原 30 秒超时。对应 release 构建通过，源码/测试输入在该轮前后 hash 相同；该构建
SHA-256 为 `CBC82A37C4B7BC28317735315B29C4CD34C203AF8C2C1B35FE878D1821CD97EB`。
这轮结果不代替随后筛选修正的最终完整回归。

最终 `0AA8ADA4…` 构建、其前端 check/build、根 `npm run check`、`cargo xtask check` 和 metadata
检查均通过。完整 Chromium 首轮为 **246 passed / 1 failed（9.8 分钟）**，失败归档为
`gui-final-failures/`，不是业务断言或组件异常：必需的 `PagedItems-*.js` 请求报
`net::ERR_ADDRESS_IN_USE`，其他六个资源为 200；React 尚未初始化、没有 pageerror。
Windows 同刻记录 Tcpip **4227 / RecordId 546584**（17:02:06.3277663 +08:00）。
代码、测试、超时和断言保持，关闭临时进程后在同一输入下独立重跑完整套件，结果为
**247 passed / 0 failed（9.4 分钟）**，单 worker、零重试。
`gui-final.log/.exit` 与后续 `gui-final-verified.log/.exit` 分别保存，不拼接为全绿。
`final-candidate-inputs-after.json` 确认完整代码/测试输入在运行前后不变；P6 已记录的 21 项
Rust/Cargo/WIT 输入与 Core/CLI 二进制保持。重跑起止 17:22:40—17:33:27（+08:00）未查询到
新增 Tcpip 4227，见 `tcp-events-final-verified-run.json`；不据此声称全局端点限制已消除。

此前 P6 修正后的完整 Rust **483 passed / 0 failed / 8 既有 helper ignored**、Clippy/fmt、
xtask 和公开 RPC 复核保留在 `m7-test-gate-closure-20261031.md`；本轮 frontend 修正没有修改
Rust、RPC、State、Config、权限、依赖或冻结候选合同。最终仍核对输入一致性，不重复累加定向计数。

## 环境恢复

临时 fixture 偏好通过公开 `settings.update`/CAS 恢复，先验证唯一合成 ProjectId、路径和
当前 revision；最终完整偏好值与初始相同，临时 revision 为 13。为了最终浅色图短暂保存
Light 后已恢复 System；当时 Windows 自身已为 Dark（只读确认 AppsUseLightTheme=0），
没有修改系统设置。项目最终 revision 3 / 1.0.0，仅三项测试 Operation，均已成功。

验收 GUI 先关闭，随后核实身份并停止临时 Core、归档和图集服务器。原 daemon 和 Tauri dev
按原命令恢复；`runtime-final-comparison.json` 确认两个原样例 Project、Editor 空列表、
Operation、全部设置和 revision **155** 与测试前相同，恢复的开发 GUI 有窗口且 Responding。
临时 fixture、Plan 和日志保留在 ignored target 作证据；没有清空或覆盖原数据库。

## 未覆盖项与签收

- 本机已安装 Hub；不改变安装以制造 `download_required`，真实下载分支记为未执行。
- 当前无已安装 Portable UI fixture；不为本轮验收新增扩展，已有独立契约/browser/runtime
  证据不冒充本次 native Portable UI 验收。
- Native 验证只声明实际执行的键盘路径；Enter/Space 的精确一次派发与越权防护另由定向和
  完整 Chromium 验证。没有 Narrator、三平台真实辅助技术或 M12 发行结论。
- 不执行真实 Unity 启动/迁移、生产仓库变更、提交、push、Hosted CI、发布或部署。
- Visual Gate 2 已获项目所有者签收；上述未覆盖项保持，M7 与聚合 feature 状态继续 in_progress。

Owner acceptance：**ACCEPTED，2026-10-31**。项目所有者在审阅本轮交付后回复“可以，接下来做什么”。
签收范围为本报告记录的 H2-A 与既有页面修正，不代表 M7 完成或下一 slice 获批。
