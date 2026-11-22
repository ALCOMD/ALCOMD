# 首发 Unity 6 支持

## 交付范围

在首发正式版中加入固定为 `6000.0.67f1` 的 Avatar 和 Worlds 模板。新建窗口仍默认选中空白模板。已有项目按精确 Editor 版本打开。Unity 6 验收用 SDK 为 `3.11.0-beta.1`，候选冻结时复核正式包状态。安装、升级及创建共用 Core 的可见性与稳定版优先设置。当前模板和迁移只接受已验收的公测版本。未来正式 SDK 完成真实编辑器验收后，再同步扩充两个模板及迁移的版本范围。

迁移支持已有 `2022.3.6f1`、`2022.3.22f1` 的 Avatar 和 Worlds。默认先复制，取得副本 ID 后再计划迁移；也可明确选择原地升级。先复验来源、占用、安装和 SDK 可用性，再开始写入。所需包仅有预发布版且显示设置关闭时，提示开启显示。确认页说明源、实际目标、SDK 版本、原地备份及 Unity 6 上传后的限制。复制和迁移分别保留任务、结果及重试入口。

## 已批准的首发合同

沿用首发 v1 的创建、复制、Unity 迁移和计划读取。迁移计划在需要更新 SDK 时返回 `targetSdkVersion`，由原计划读取入口返回相同值；创建与迁移预检使用 `prerelease_packages_hidden` 区别于缺少兼容包。MCP 增加 `alcomd_plan_unity_migration` 和 `alcomd_apply_unity_migration`，复用原有调用者、权限和确认。所有者已核定以上公共差异及 State1 计划表的可空 TEXT 列 `target_sdk_version`，用于重开后读取与复验同一 SDK。

## 样本与验收

以[官方便携 VCC](https://github.com/vrchat-community/creator-companion/releases/tag/2.5.0-beta.3) 与[官方 SDK 包](https://github.com/vrchat/packages/releases/tag/3.11.0-beta.1)作为行为依据。VCC 发行 ZIP 的 SHA-256 为 `a7111cce300e7e435a7ed3fdfe3c3bdcd90d411ac4aa905ea3cb924661f4d1ac`，Base 包为 `e0e4f03c2421d455fc7558adcdb6e6d6bb0659304e8936f42da33dbc1d51058a`。两种官方模板均声明 Base 及对应 SDK，Editor revision 为 `78a1c2bbeb6a`。SDK 向导从 `Library/VRCProjectUpgradeMarker.json` 读取源版本、场景及向导进度。

对两个新模板验收创建、精确打开、依赖解析及本地构建；测试已有项目、预发布设置及稳定 SDK。两种来源补丁 × 两类项目 × 副本或原地，共八组迁移，核对清单、升级标记、首次导入、向导及本地构建。覆盖写入前拒绝、计划过期、复制成功后迁移失败、原地中断、Core 重启和重复 Apply。真实 Editor 结果与程序夹具分开登记。同一候选的三平台 CI、CodeQL 和首发门禁按现行项目规则验收。

## 实测与批准的迁移差异

同一便携 VCC 2.5.0-beta.3 创建的两个 2022 项目均直接引用 `com.unity.ide.vscode@1.2.5`。两个 Unity 6 新建模板均无此依赖。成功迁移的 Avatar 和 Worlds 副本均已去除该项。一个缺少场景 .meta 的 Worlds 副本在启动 Editor 前失败，此时该条目已被移除。vrc-get 的[合并实现](https://github.com/vrc-get/vrc-get/blob/44b9c729977f49ccb11d5787a70bd0bae82df64b/vrc-get-vpm/src/unity_project/migrate_unity.rs)对 Unity 6 单独移除此项。所有者批准仅在 2022 → 6000 时从目标 UPM 清单移除这条直接依赖。

两组迁移后 UPM 同时包括 AI Navigation 2.0.8、Burst 1.8.24、Development 1.0.0、Timeline 1.8.8 和 uGUI 2.0.0。新建模板采用各自的创建清单。SDK 向导标记中的 sceneList 为场景 GUID 列表。Avatar 空工程得到空数组；Worlds 单场景工程得到场景 .meta 中的 GUID。两者 wizardCompleted 初始为 false，upgradedScenes 为 null。

ALCOMD 的两个模板工程均已用真实 6000.0.67f1 批处理生成，随包数据和许可见[工程来源](../../specs/templates/builtin-scaffolds/README.md)。模板创建、包解析与原始工程文件校验已通过定向 Rust 测试。真实 SDK 构建、八组迁移、三端交互与候选门禁分别验收。

State1 原位新增 `target_sdk_version TEXT`，属于首发前存储结构的破坏性修订，按现有 State1 结构校验规则打开。RPC 的可选结果字段及两项 MCP 工具为增量差异。客户端收到 `prerelease_packages_hidden` 时提示开启显示预发布包。

## Windows 验收记录

2026-11-22，隔离 Core 通过正式 CLI 创建并登记两个项目。Unity `6000.0.67f1` 与 SDK `3.11.0-beta.1` 的真实 Avatar 本地构建通过，生成 760242 字节的资源包。Worlds 等待 UdonSharp 编译结束后生成 72519 字节的资源包。Unity 构建日志中出现过一条 `Build Finished, Result: Failure`。另用干净的真实 Editor 打开新建、2022.3.22f1 副本和原地工程的三个产物，均读到预期的场景路径。三份正式 `BuildReport.summary.result` 均为 `Succeeded`，错误和警告数均为 0，复核进程退出码为 0。另用 VCC 的两个已有 Unity 6 样本验证包管理，两个项目均从 Core 的常规打开入口匹配该版本并启动真实 Editor，退出码均为 0。两个项目均能读取包清单、安装 `com.vrchat.core.vpm-resolver@0.1.28` 并升级到 `0.1.29`。

[VCC 2.3.0 发行包](https://github.com/vrchat-community/creator-companion/releases/tag/2.3.0)中的历史 Avatar 与 Worlds 工程记录了完整的 `2022.3.6f1 (b9e6e7e9fa2d)`。官方便携 ZIP 的 SHA-256 为 `71102cdb77399ea57f1e7bf5f5ef5df95b75ffec2ec15e0f77485c6acc857bd0`。两者的 UPM 直接依赖均含 `com.unity.ide.vscode@1.2.5`。原始 VPM 分别声明 Avatars 或 Base 与 Worlds 的 `~3.5.x-` 范围。

Chromium 在 43 项相关界面回归中验证了两个模板的创建及设置重试、缺少或具有多个匹配编辑器的已有工程、默认副本、显式原地，以及迁移失败后在原副本上重新计划。前端类型、控件、语言和生产构建均通过。VPM 库定向测试通过 186 项，另有 3 项按原配置忽略。MCP 库 18 项测试通过。首发清单检查通过 122 个 RPC、43 项能力和 36 个 MCP 工具。

真实 MCP 客户端从分页工具清单发现两个迁移工具。原地预览返回了工程实际路径及 SDK，默认流程返回复制计划。副本创建后，以已完成的复制任务和新项目 ID 得到迁移计划，Apply 成功。重复 Apply 返回同一任务 ID，最终状态为 `succeeded`。SQLite 重开和冻结 SDK、到期计划拒绝、写入半个向导标记后重新执行，以及现有七项迁移任务恢复测试均已通过。

验收时官方最新稳定 SDK 为 [3.10.6](https://github.com/vrchat/packages/releases/tag/3.10.6)。稳定包夹具验证了两个条件：新出现的版本须先进入已验收范围；进入范围后，默认设置优先选择正式版。

八组 Core 迁移均已通过真实 `6000.0.67f1 (78a1c2bbeb6a)` 首次导入。每组复验了 Editor、Base 和对应 SDK、UPM 删除条目、向导初始状态以及实际场景 GUID。四组副本源工程的逐文件哈希均保持一致。

| 源工程 | Avatar 副本 | Avatar 原地 | Worlds 副本 | Worlds 原地 |
| --- | --- | --- | --- | --- |
| VCC 的 `2022.3.22f1` 工程 | 通过 | 通过 | 通过 | 通过 |
| 历史 VCC 的 `2022.3.6f1` 工程 | 通过 | 通过 | 通过 | 通过 |

历史模板的原始清单只声明依赖。验收样本按原始 `~3.5.x-` 范围安装并锁定官方 SDK 3.5.2，三份包均对照官方索引验证 SHA-256。Worlds 原地样本额外放入一个有有效 .meta 的场景，Core 写入的 GUID 与文件一致。还有两次用受控版本标记完成的副本迁移，八组表格只计原始编辑器版本来自 VCC 的项目。

Windows 实测中，SDK 暂存树刷盘使用只读句柄导致 Apply 失败，修复后在已保留副本上恢复。历史 Worlds 的一次 CLI 轮询超时后，原迁移任务成功完成。CLI 现会在临时读取错误后重连并跟踪已接受的任务 ID。矩阵在长时间运行的 Core 进程中完成；随后收紧了 SDK 范围并增加 Editor 启动前的准备产物复核。这批结果记录开发过程中的真实迁移，冻结候选仍需按同一 SHA 验收。

最终隔离 Core/CLI 对 Avatar 和 Worlds 各执行了创建、默认副本和原地迁移三项隐藏预发布包检查，六次均在工程写入前返回 `prerelease_packages_hidden`。重新开启显示并保留稳定版优先后，两个家族均创建成功，迁移计划也选定同一 beta。直接按 ID 对副本预览中的源计划执行 Apply，缺少显式原地选择或有效复制回执时均被拒绝，源 SHA 保持一致。

本地 Rust 工作区完整运行有 1053 项通过、1 项失败、22 项忽略。失败来自扩展宿主的零授权管理页测试，返回 `extension_resource_limit`。原测试二进制单独复跑通过，重新编译后再次触发该限额。该门禁仍未通过。全工作区 Clippy 通过。本次功能涉及的 Rust 文件格式检查通过，全工作区 fmt 检查另报并行变更文件的换行符。仓库合同门禁在源文件快照通过；主工作目录中的扫描还读到了 .local/parity 下其他已登记工作树，产生了 102 项重复路径报错。
八组真实迁移的 SDK 升级向导均已完成，本地构建均已产生资源包。历史 Worlds 的两个项目在隔离验收克隆中使用 SDK 面板对应的 Setup Layers 和 Setup Collision Matrix。副本的正式构建报告为 Succeeded，零错误、零警告，重新打开 72538 字节的 .vrcw 可见预期场景。原地工程的 GUID 测试场景 Probe.unity 在迁移前即缺少 VRChat 场景描述器，向导仍按正确 GUID 写回完成。SDK 拒绝直接构建这个场景。在验收克隆中加入官方 VRCWorld 预制体后，SDK 生成 84690 字节的 .vrcw，报告为 Succeeded，零错误、零警告，重新打开得到 Assets/Probe.unity。产品迁移流程不会向用户场景注入该预制体。

本轮为 Windows 本地功能验收。三平台托管 CI、CodeQL 和同一冻结 SHA 的签收仍待执行。

整合主线 `d91d6920` 后，Windows 用户会话用重新编译的程序单独复跑零授权管理页通过。此前触发限额的测试程序按串行运行整个宿主测试组，12 项通过、3 项按原配置忽略。历史限额失败的原因仍未确定。全仓 fmt 与主工作目录的 xtask 检查均通过；扫描器依据 Git 登记排除 `.local/parity` 下的独立工作树。11 月 22 日复核官方发布清单，Unity 6 验收 SDK 仍为 `3.11.0-beta.1`，最新稳定版为 `3.10.6`。
