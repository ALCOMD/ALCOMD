# M7 H3 本地候选与桌面证据（2026-11-01—02）

当前阶段以 [status](../status.md) 为准。H3 已获项目所有者明确批准；本记录只覆盖已有 Unity、
Backups、Operations 在 Settings、项目及进度流程中的接线。**H3 人工验收待确认，M7 仍进行中**。
H2-A 签收及 H2-B“阶段性人工验收通过，保留体验改进项”保持有效，H4-H7 与推送未授权。
项目所有者随后反馈 Unity 检测失败、暂不签收 H3；当前修复代码为 `efdb133`，结果与唯一待确认项见文末。

## 上轮候选与适用范围

此前主体提交为 `ef47c257edd37aa0912e302c9980060080314639`，补齐 Unity 启动状态后的该轮最终代码为
`ab5d183635ccda9b4d9a4246315ed837f09b15fc`。真实 Windows 用户 `CQMHV-DT\M2922`
在标准目录/main 本地提交，未推送。只改 GUI/测试；没有新增公共 RPC、Schema、权限、依赖或持久任务。

- Settings 已有 Unity 安装管理、Hub，项目已有精确启动/启动参数/迁移；本轮只统一相关反馈和请求生命周期。
- 本页接受一次启动后，项目现有更多菜单可按需读取/刷新该启动记录；不自动重发启动，不另建常驻卡片、后台轮询或跨页启动历史。结果校验 launchId、projectId、installationId，旧请求不能重开弹窗，关闭后返回菜单按钮。
- 备份创建接受后关闭输入弹窗，在项目备份页跟踪同一 Operation，终态刷新历史，结果入口使用 Core 的 ID。
- 备份历史、任务列表及恢复选择接通现有分页。备份详情复用 Projects 恢复弹窗，移除重复恢复面板。
- 已接受恢复任务的读取失败继续跟踪同一 ID；未知创建/恢复/迁移写入不自动重发，也不再 Apply 同一计划。
- 任务详情复用 OperationFollow 持续读状态；取消失败留在确认上下文，重试使用最新 revision，取消请求不代表已取消或回滚。
- 修复恢复列表重试触发弹窗关闭的问题；已显示的弹窗在重试加载时保持打开。

## 本地技术验证

`target/m7-h3-contextual-20261101/` 保存逐轮日志、退出码、运行时及截图摘要。

| `ab5d183` 检查（保留原证据） | 实际结果 |
| --- | --- |
| GUI check | TypeScript、Material 边界、三语言 **918** 条检查通过；release 前置检查由既有构建命令执行 |
| 最终同 SHA 受影响回归 | `browser-ab5d183-final.log`：**34 passed / 0 failed**，1 worker、0 retries、2.8 分钟；包含 H3 14 项、任务跟踪、创建/恢复、Unity 与直接共享进度/确认回归 |
| 追加 Unity 接线定向检查 | `unity-status-final.log`：15/0；随后已纳入上述独立 34/0，不把两个数量相加 |
| release GUI | 主体 `gui-release.log`：1m54s；新增启动状态后 `gui-release-final.log`：**3m27s**。两次对应不同运行时代码输入，没有为文档/提交重复构建 |
| metadata / diff | 检查通过；最终纯文档更新只补适用检查 |
| Core / native adapter | 相对 `ceef00f` 的 crates、Cargo、产品身份、Schema、SDK/UI 包、迁移与 GUI Rust 输入均无差异；复用原有有效证据和 release Core，无需重跑 Rust 全量 |

| 实际产物 | SHA-256 |
| --- | --- |
| 已保留至 `target/m7-h3-unity-discovery-20261102/previous-runtime/alcomd-gui.exe`，代码 `ab5d183` | `2b39ab2a94508456bb3f66a4c2f2055ca7bab3dab76d199e9898ffb7e4c16cc4` |
| 原生备份/恢复所用 `target/m7-h3-contextual-20261101/native/gui-runtime/alcomd-gui.exe`，代码 `ef47c25` | `c170b0d39d5d025d2125ef40c4b058b5543f5f34d196875676b6dcd5f9467ceb` |
| 上轮 Core 已保留至 `target/m7-h3-unity-discovery-20261102/previous-runtime/alcomd.exe` | `ea22d776c6416d487149e438b2eb1b2857ea8df8185367a3466bec587cb1b136` |
| 复用 `target/release/alcomd-cli.exe` | `2d718f27bca110cf578aaf94ca49be68c4fda15f794427e06abf89b494641cc1` |

未运行完整 Rust Workspace、完整浏览器套件或三平台 CI。远端仍只有 `0ef5b08` 的 CI `41287607183`
与 CodeQL `41287607108` 同 SHA 通过；H2-B 后续代码及本 H3 候选没有对应远端结果。

历史失败单独保留：早期翻译检查两处标题字面量失败；sandbox 浏览器缓存路径没有 Chromium，13 项未进入业务；
真实用户首轮 7/6，新测试的 Material 弹窗作用域和页脚读计数不正确；随后 H3 7/3，修正 fixture revision
及 StrictMode 下的失败注入。首个 30 项候选为 29/1，发现真实恢复重试误关闭；修复后在 `ef47c25` 独立取得 **30/0**，
不拼接失败轮次中的通过项。启动状态接线随后取得 15/0，最终 `ab5d183` 再集中取得上述独立 **34/0**。

## 真实 Windows 桌面

下面的原生写入验证使用 **`ef47c25` / GUI `c170…67ceb`**、匹配 release Core 与既有可丢弃 H2-B 数据。
`ab5d183` 的备份/任务/Unity 登记/提醒框相关 **16 个函数体保持不变**，证据为 `reused-flow-inputs.json`；
这些截图继续绑定实际构建，不称为整个新 GUI 或新增启动状态弹窗已完成原生验证。六个旧项目的 ID、完整路径和 revision 与前轮
证据逐一相同。首次路径预检因 Windows `\\?\` 前缀停止，未启动 GUI 或执行测试写入；核对规范路径和旧证据后继续。

- 原生创建备份成功：Operation `57bee214-c34c-49f7-9d57-0f06e956de89`，BackupId `12723e26-0211-4635-bf38-4d71859a5666`。
  弹窗关闭、项目上下文进度、终态刷新和任务详情已实际查看。
- 恢复审阅拒绝 `CON`，弹窗保留 `invalid_request`；完整 Operation 结果不变、目标父目录仍为空。
  改名后旧错误清理，长目标路径换行、必要的单独解析 VPM 依赖提示保持可见。
- 显式应用一次计划后，恢复 Operation `e564c598-c005-4886-9c27-242954541265` 成功；窗口实际进入
  `f09873ed-06c4-4bc7-81a6-a6eed02281c0`，与 Core 结果及登记的隔离目标完全一致。没有自动执行依赖解析。
- Settings 管理安装的“查找并刷新”实际执行，成功反馈在右下轻提示中，不撑开弹窗控件；关闭后无旧错误。

未在原生窗口重新执行 Unity 启动/迁移或制造长任务取消竞争；这些前端分支由本轮定向回归及已有证据覆盖。
超过一页的备份/任务选择及网络/未知结果注入是浏览器证据，不冒充原生多页/故障注入。
`unity.launchStatus` 已接入项目更多菜单的显式读取/刷新，浏览器覆盖同记录、失败、身份不符、迟到与焦点。
新增启动状态弹窗在 Esc 后未做原生操作/截图。`unity.installations.get` 保留 typed adapter，Settings 当前直接消费完整列表快照，不另建安装详情页面。

原开发环境已实际恢复：Core PID **108860**、开发 GUI PID **119908**，窗口可响应；五组完整公开状态
Projects/Editors/Operations/Settings/Status 与暂停前一致，settings revision **155**。隔离 **13 项任务全终态、0 活动**，
GUI/Core 已停止，没有处理用户 Unity 或不明进程。恢复通过实际执行与状态核对确认，不以守候进程代替。

上轮 release GUI PID **104516**、窗口 **13506050** 曾以正常进程方式启动，句柄非零、Responding=true，
连接恢复的原开发样例。Esc 后没有再执行桌面输入或视觉检查，不能据此声称新弹窗已实际看过。
启动时曾保留开发窗口与先前 `ef47c25` 查看窗口（PID 90372、窗口 333950）；后者曾只读停在
**Project Copy → 备份**。该轮交付前的进程核对中两者已不在列表，当时仅原 Core 与该轮 release GUI 仍在运行且可响应；
没有据此再次启动多余窗口。查看窗口使用恢复的原开发 Core，原生写入验证则使用 release Core/隔离数据，二者明确区分。
最终查看窗口启动后再次完整读取五组状态仍一致，见 `final-original-state-verification.json`。

截图位于仓库外 `C:/Users/M2922/.codex/visualizations/2026/08/17/01a00f12-6751-7d50-9346-7b58e94f2eea/h3-contextual/`，
摘要见 `screenshots.json`。修改前/`owner-review-ready.png` 为同一原开发 Project Copy 与主题，后者对应
`ef47c25` 的查看窗口；隔离流程截图使用该隔离样例自己的主题。最终新 GUI 没有另补截图。
代表画面为 `before-project-backups.png`、`owner-review-ready.png`、`after-backup-result.png`、
`after-restore-error.png`、`after-restore-review.png`、`after-restored-project.png`、`after-unity-success-reminder.png`。

## 上轮所有者清单（本轮不要求重验）

1. **项目 → 备份**：创建入口、压缩/排除选项；关闭或取消输入不产生任务。实际创建后的进度与历史体验见隔离截图。
2. **备份详情 → 恢复**：带入选中的备份，目标与风险可读，错误留在上下文，改正后重试；实际隔离恢复结果见对应截图与 ID。
3. **任务 → 详情**：状态、结果入口和终态取消不可用；有活动任务时，确认取消应继续显示 Core 实际状态。
4. **设置 → 已安装的 Unity → 管理安装**：反馈位置、连续操作、关闭/重开与键盘焦点；本页有已接受的启动时，项目“更多 → Unity 启动状态”应只读取/刷新同一记录。无须重验 H2-A 或所有 GUI 页面。

当前只交付 H3 本地候选，等待所有者验收；未标记 H3 ACCEPTED、整个 Visual Gate 3 或 M7 完成。

## 2026-11-02：Windows Unity 检测整改

本轮只修人工反馈的主要阻塞；提示冗余、任务终态控制与恢复摘要等非阻塞体验继续记现有待办。
代码 `efdb133a82f6ead7b0436919dddd7cab939e94c4` 已由真实 Windows 用户在标准目录/main 本地提交，未推送。
没有新增公共 RPC、State Schema、权限或依赖。证据目录为 `target/m7-h3-unity-discovery-20261102/`。

### 故障分类与根因

初始 GUI `2b39…16cc4`、debug Core `b659…10f7` 连接的是原开发
`target/m7-icon-acceptance-20260906/data`，不是缺少登记的 H2-B 隔离环境。公开列表确为 0、Hub 定位 available；
实际默认根 `C:\Program Files\Unity\Hub\Editor\2022.3.22f1\Editor\Unity.exe` 存在。
该二进制 ProductName=`Unity`、ProductVersion=`2022.3.22f1_887be4894c44`；项目所需为 `2022.3.22f1`。
问题不是 GUI 筛选丢行或没有 exact version，而是版本验证在登记前拒绝了已安装 Editor。

真实 `Data/Resources/PackageManager/Editor/manifest.json` 顶层只有 `schemaVersion/packages/metadataPackageName`，
不含 Editor `version`。旧 daemon 却依赖该字段，测试也只提供人造 `{"version":"2022.3.22f1"}`；
随后 `.filter_map(... .ok())` 把错误静默丢掉。`root-cause-before.json` 保留真实文件 hash/字段摘要和构建/数据身份。
GUI “查找并刷新”的首轮输入被用户输入中断，未将其写成成功完成的原生操作。

Windows 现从 Editor 自身的 ProductName/ProductVersion 资源只读取版本，按声明 Translation 验证身份和一致性，
对缓冲、指针范围、UTF-16 与长度有界检查；不执行 Editor，不信任目录名或包版本。
窄 FFI 依据 [Microsoft 版本资源 API](https://learn.microsoft.com/en-us/windows/win32/api/winver/nf-winver-verqueryvaluew)，
复用已存在的 FileSystem 功能，每处 unsafe 有安全说明。Core 复用同一 canonical version 校验，exact launch 规则不变。
存在候选无法验证时，这轮发现明确失败、保留原登记；不静默吞掉错误后报告成功。私有日志记录路径/具体原因，
GUI 用既有错误合同保留版本验证/安装不可用的上下文错误；空表仅描述“尚未添加 Unity 安装”。

### 定向验证与构建

| 范围 | 当前结果与日志 |
| --- | --- |
| 根因修复前回归 | `root-cause-before-fix.log`：1 failed，实际 `VersionUnverified`，证明真实形状清单触发旧错误；保留失败 |
| Windows 版本资源/发现 | `platform-final.log`：8/0；身份/UTF-16/有界读取、真实 VERSIONINFO、注入安装根的候选与拒绝不被吞掉 |
| daemon 元数据适配 | `daemon-final.log`：3/0；包目录/包版本不可替代 Editor 版本，错误产品及无效版本拒绝 |
| Core exact version | `exact-version-directed.log`：1/0；已有 canonical/exact/China variant 门禁保持 |
| 受影响 RPC | `rpc-directed.log`：1/0；fixture 使用自有测试程序副本的原生 VERSIONINFO，不依赖真实 Editor 执行 |
| Unity GUI | `browser-directed.log`：6/0，1 worker、0 retries；发现→列表、刷新、验证失败/显式重试、原有 exact launch/迁移组合 |
| Clippy / GUI check | `clippy-final.log` 通过；TypeScript/Material/三语言 919 条通过。首轮 Clippy 两处取模风格失败保留在 `clippy-directed.log`，修正后集中检查 |
| 必要构建 | `core-release.log` 3m06s、`gui-release.log` 2m32s，通过；只构建变更的 Core/GUI，无 Workspace 全量、浏览器全套或三平台 CI |

同一运行时代码输入的 GUI 回归与以上各 Rust 组独立记录，不把旧 34/0 或失败轮的通过项拼进本轮。
Windows 专用测试 helper 只给自有测试程序新副本添加资源，不复制真实 Unity 二进制，不新增依赖或生产兼容分支。
macOS/Linux 未在本轮运行；非 Windows 原有元数据读取未在本轮修复或证明可用。Hub 自定义安装根不作为本机根因，未扩大扫描范围。

| 当前产物（`efdb133`） | SHA-256 |
| --- | --- |
| `target/release/alcomd.exe` | `ef371a609eaa428e44a97881c108ed4b3656dd5740d34f8d2602269ca921b5ca` |
| `target/release/alcomd-gui.exe` | `1bf3716ded0b97408d848b2c48bcde5f1b251c99cca4277662193bea903a7fc8` |

### 当前真实环境与唯一待确认项

关闭/暂停前核对旧 GUI/Core 的路径、创建时间、命令行与 hash，完整遍历任务确认全终态。
仅更新这轮自有 GUI/Core，RPC 用例用完后在 finally 中实际恢复原数据；更新前后五组公开状态全一致。
当前 release Core PID **96504** 使用原数据目录，settings revision **155**；没有停止用户 Unity 或不明进程。

通过正式 CLI `unity refresh` 调用同一 Core 自动发现/登记用例，得到安装 **`fcb37996-8fda-459e-885f-16271e568135`**，
来源 `known_install_root`，版本 **`2022.3.22f1`**；不是手动登记。再次刷新仍为同一条、revision 1。
两个样例项目的只读 `launch-options` 均返回该 exact installation，Project Copy 保持 revision 5，另一个项目保持 revision 1。
两者的 `unityRevision` 均为空，本轮仅证明 canonical version 完全匹配，不宣称已经验证相同 Editor build hash。
没有启动或迁移实际工程。登记从 0 到 1 是本轮授权验证的预期结果，保留该有效记录；其余 Projects/Operations/Settings/Status
公开结果保持一致，见 `actual-discovery-verification.json`。不宣称整个数据库零变化。

新 GUI PID **104600**、窗口 **530566** 已启动且 Responding=true，匹配上述 Core/代码，保持打开。
physical Escape 后停止桌面输入，没有安装页修复后原生截图/点击证据；只完成进程启动与公开结果核对。
修复前完整窗口截图为仓库外
`C:/Users/M2922/.codex/visualizations/2026/08/17/01a00f12-6751-7d50-9346-7b58e94f2eea/h3-unity-discovery/before-empty-installations.jpg`；
没有把接口/浏览器结果称为原生 GUI 通过。

本轮仅请所有者确认：**设置 → 已安装的 Unity** 能显示 `2022.3.22f1` 与上述真实路径，刷新后仍为同一条；
若使用“管理安装 → 查找并刷新”，应成功保留该记录。其余 H3 页面不重验。
H3 继续未签收，M7 进行中。H2-A、H2-B 原阶段签收有效，旧 `0ef5b08` 远端成功不覆盖本轮代码；未推送或进入 H4-H7。
