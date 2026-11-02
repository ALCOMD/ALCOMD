# M7 H5 Utilities / Observability

状态：**STAGE_ACCEPTED_WITH_FOLLOWUPS**，2026-11-02。所有者已实际查看并签收 H5，认可移除紧凑模式和统一默认间距，
保留一般文案、布局及过度设计的非阻塞体验待办。H2-A/H2-B/H3/H4 原签收保持，M7 进行中；H6 已批准，H7 与推送未授权。
下文历史运行、候选交付时的未确认项分别保留，阶段性签收不等于体验完善或遗漏范围全部通过。

## 当前候选：彻底移除紧凑模式（6739705）

所有者临时插入删除 GUI 紧凑模式，并明确批准公开字段删除及 Config4。源码候选为
`6739705de9711b18e9e398270e0254e6b8719c6b`：删除开关、词条、专用 CSS、渲染与保存分支及公开 density 类型/字段，
统一原默认间距。局部行操作的 compact 排布与普通间距 token 不属于模式，保持已认可布局。
Config1–3 按历史合同校验后迁移，仅丢弃 density，保留其他偏好与 revision；Config4 文件/完整响应/更新拒绝旧字段，
没有旧 RPC 别名或第二套当前模型。当前规范为 settings-v4；State16、权限和依赖不变。

本轮证据目录 `target/m7-h5-no-compact-20261102/`：Store 19/0、application 17/0、协议合同 7/0、hello 1/0、映射 1/0；
真实隔离 Core RPC 1/0（先核对全终态并暂停自己记录的原 Core，使用一次可丢弃 daemon 测试，不连接原数据）。
GUI/UI check 1028 条三语言通过，保存队列 15 场景、关联浏览器 76/0（2.5m）及保存刷新补充 1/0（8.6s）通过，
184 项 GUI 输入前后不变。原保存/CAS/未知结果/主题冲突/焦点断言改用仍存在的 motion/packages 偏好，不删除保护。
四 crate Clippy 首轮仅新 daemon 测试的字段后赋值写法失败（`config4-clippy-before.log`）；改成直接初始化，
仅重验 daemon all-targets Clippy、映射 1/0 和 IPC test target Clippy，均通过；fmt 通过，无 allow 或断言删除。
536 项完整输入清单中的该测试初始化差异单独保留；最终构建前后 536 项完全一致。
Core release 与 GUI release no-bundle 各一次通过（各 Rust release 1m41s），GUI 构建包含 check/Vite；未跑完整 Workspace/浏览器/远端 CI。

原开发目录 `target/m7-icon-acceptance-20260906/data` 已恢复新 Core **143516** 和 GUI **141068**（窗口 **41227382**，可响应）。
实际 `config/settings.toml` 迁移前后逐值比较，仅 schema 3→4 和删除 density，revision **166**、主题及其他偏好全部保留。
五组公开状态按该获批差异核对一致，其余四组原样不变；hello 报告 Config4/State16。
Core SHA-256 `b90ec8b93d043ca8ef6077d9f0bfdec40a5db642a5c91f820cf17a0d665d4687`；
GUI SHA-256 `23d6d77170b071d7060e77cacd0de5b47cd607049f0875eeef1b0e4abbe9b1bc`。
完整状态、文件前后摘要及实际进程见 `migration-verification.json`、`verification.json`、`core-runtime.json`、`gui-runtime.json`。
当前窗口已打开；复开使用本轮 `target/m7-h5-no-compact-20261102/H5-Window.ps1 -Action Open`，已真实执行，核对匹配 Core/GUI 摘要。
没有操作用户 Unity/未知进程或恢复桌面控制；交付后所有者已明确签收 H5。新增代码仍无对应远端验证，不以旧 CI 替代，不推送。

## 首个 H5 候选复用与改动（c768289，保留历史）

已有 grouped Settings/全部 Config3 字段、普通设置串行 CAS、主题预览与三方合并、Log 分段/搜索/已加载项筛选/
keyset 分页、About/许可证/footer 入口继续复用，不重建页面或新增管理入口。

- 设置保留安全错误码/诊断编号，复用共享提醒；普通设置、主题和 Hub 的未知写入不误报确定失败，Retry 仍先读取权威值。
  Auto detect 无覆盖路径时的只读失败、已保存 Hub 但安装发现失败分别显示。主题短成功使用浮动反馈。
- 隐藏仓库按完整意图的 256 项上限阻止新增，仍允许取消已有隐藏；Core 最终校验不变。仓库选项分页失败保留行及原 cursor。
- Log 详情复用 OperationFollow 读取同一任务至终态，关闭后丢弃迟到响应；依赖任务查询的动作检查已有 operations capability。
  状态检查受理后跟踪同一 ID，终态前不重复启动，结束后刷新诊断。诊断正文使用共享稳定错误映射，保留 code/diagnosticId。
- 已有扩展安装/卸载及 state.check 活动摘要补有限翻译；About 分开显示本地 GUI 构建版本和 Core 版本，Core 断线不隐藏本地信息。
  footer 版本不等待 Tasks。GUI 版本来自经 metadata 检查与产品/Tauri 对齐的构建元数据。

不新增公共 RPC、State Schema、权限、依赖、保存队列或任务/通知框架。不改 Core、Portable UI 协议或 Runtime。
没有新增路径格式化实现；只读文件路径继续统一 `displayPath`，实际路径、可编辑值及 RPC 参数保持原值。

## 首个 H5 候选证据与边界（c768289）

GUI 源码候选：`c7682899a27c938e588720e1dce7ef4113882a17`，标准目录/main 的真实 Windows 用户环境本地提交。
Core 源码/二进制保持 `e2b7ddd` / `7d62a6c8aed0eb6bba3ebc6b33f037d7fd18aebaf1186c109487c310d5da7a18`。
证据目录：`target/m7-h5-utilities-20261102/`。GUI check 通过（TypeScript、Material、1030 条三语言消息）。
九个受影响浏览器模块首轮 **101/2（2.8 分钟）**，184 项 GUI/SDK/UI/build/test 输入前后完全一致。
一处为 Hub 更正重试时新加的清空使在途错误区域提前消失，已移除该清空，保留完成时成功清除/失败替换。
另一处为导航测试仍要求 H4 已默认折叠的原始错误码；改为先核对可读原因，再展开详情检查原码，断言不删除。
仅复验 Settings 新用例、Hub、导航三个关联模块，独立 **56/0（1.4 分钟）**，184 项输入前后未变。
两轮间只有 `SettingsWorkspace.tsx` 的清空逻辑与导航测试断言变化，其余输入一致；不重跑其余六模块或拼成完整套件通过。
两轮日志、输入摘要及首轮失败 trace/截图均保留；截图是浏览器测试证据，不是原生桌面截图。
稳定候选的一次 Tauri release no-bundle 构建通过，包含 GUI check（1030 条消息）及 Vite 构建，GUI Rust release 耗时 1m33s。
GUI SHA-256 为 `c815871679676fedc35b2ea4c7f3de0b658613897bcd58ed52541bb1a67f6484`。
构建后 184 项输入与定向复验完全一致，输入清单 SHA-256 为 `ff9f2d2f7eb5c1efdc716ff28f9f44949e044b446c14ac2d226b101803d87d86`。
既有 Vite 大 chunk 提醒保留，没有改阈值或为此重做打包。Core 未重建。
本轮不重复 Rust 全量、浏览器全套或三平台 CI；H2-B/H3/H4 及 H5 新代码仍无对应远端验证。
H4 首轮 78/1、定向修复 6/0 继续独立保留，不拼接成完整套件通过。

H4 签收后隔离任务全终态，已实际关闭隔离 Core/GUI，并恢复原目录的 Core 136212、GUI 132312；五组完整公开状态一致，
详见 [H4 恢复记录](m7-h4-extensions-20261102.md)。H5 复核该原 Core 身份及只读 RPC：Activity 有记录，Diagnostics 当前为空。
当前没有往原环境安装样例扩展或写入测试数据；浏览器失败/长文本/分页样例仅在内存客户端中运行。
桌面控制保持停用，不伪造原生截图或把进程窗口存在写成原生交互通过。

新 release GUI 已用原 profile 实际打开：PID **144972**、窗口 **2431108**、标题 ALCOMD3、可响应；Core **136212** 未停止，
仍使用 `target/m7-icon-acceptance-20260906/data`。只在任务全终态及身份核对后关闭记录的旧 GUI，未操作 Unity/未知进程。
构建前 `before-build/` 与开窗后 `original-after/` 五组完整公开结果一致（`build-to-window-comparison.json` 五项 true）。

较早 `original-before/` 至 `before-build/` 的 Unity 记录有真实差异：15:01:28.832 的公开 event 38 为 installation.removed，
15:01:33.546 的 event 39 为 installation.discovered；旧 ID `fcb37996-8fda-459e-885f-16271e568135` 变为
`7a33e215-81a1-4eb4-892f-64d32110a123`，版本、路径与文件身份相同。该变化已存在于构建前，其他四组未变；
`five-group-comparison.json` 保留 editors=false 的原始结果，不改旧快照、不回写数据，也不把它写成 H5 五组全程不变。
H4 14:39 实际恢复的五组一致证据仍有效。H5 只读脚本未执行移除/发现写请求，不推断这两条 event 的操作者。

实际入口为 `C:\Users\M2922\MyProjects\Rust\ALCOMD\target\release\alcomd-gui.exe`，当前窗口保持打开。
已验证的本地 `target/m7-h5-utilities-20261102/H5-Window.ps1 -Action Open` 会核对 Core 身份和 candidate-artifact 摘要，
保留原 profile；仅在窗口关闭且同一 Core 仍运行时使用，不会启动或切换 Core。该入口本轮已真实执行，未使用桌面输入接口。
汇总为 `verification.json`、`candidate-artifact.json` 和 `gui-runtime.json`。本地文档收尾只做 metadata/diff，不重跑测试或构建。

## H5 人工查看范围

1. **Settings**：查看既有分组，修改一项普通偏好；打开 Theme 预览并取消或保存，观察反馈不挤动工具栏。Unity 检测不重验。
2. **Log → Activity / Diagnostics**：搜索、切换分段和筛选；有任务记录时点“查看任务”，核对详情能关闭并返回原列表。当前诊断为空属真实状态。
3. **侧栏底部 About**：核对“界面版本”和 Core 版本、状态及许可证；不要求断开真实 Core 来制造失败。

错误、分页、未知写、断线和迟到响应由定向浏览器回归提供证据，不要求所有者手工证明权限系统或重复已签收页面。
该清单交付后，所有者已确认 H5 阶段性人工验收通过；H2-B/H3/H4/H5 非阻塞文案、布局及过度设计待办保留，不借 H6 全面重做。
