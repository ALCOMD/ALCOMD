# 项目状态

最后更新：2026-11-08

## 当前阶段

**M7 官方 GUI 与 Portable Extension UI 进行中，H7 等待项目所有者最终人工确认。** M0–M6 已验收。H2-A 和 Visual Gate 2 已签收，H2-B、H3–H6 已阶段性签收并保留各自的后续事项。


- 2026-11-08 按所有者最新要求完成撤除额外项目展示数据库副本与补采流程，落实[正常读取合同](exec-plans/M7-package-presentation-completion.md)：普通候选请求由 Core 直接读取项目文件，分页仅暂存有界内存结果；GUI 不再先调用 projects.refresh 填库。受影响 Rust 460 项通过、3 项既有忽略，82 项浏览器回归、严格 Clippy、fmt、公开形状、元数据、xtask、前端检查/构建及原生 Core/GUI 构建通过。现有数据仅删除两张展示表，其余 34 张表记录、设置与模板文件一致。Core/GUI 已配套更新并打开，ready / State31 / Config12，原 profile 保留。本轮已按所有者要求本地提交，未推送，H7 保持；详见[验证与更新记录](testing/package-direct-read-20261108.md)。

- 2026-11-08 中间页面读取修正（已由上项取代）：移除新增的信息不完整区域及独立按钮后，曾通过 projects.refresh 自动核对并补充 State30 展示副本。该流程已按所有者最新要求撤除，仅保留[当时验证记录](testing/package-presentation-refresh-flow-20261108.md)作为历史证据。

- 2026-11-08 软件包展示补齐完成本地实施：[候选 v2 与 State30](exec-plans/M7-package-presentation-completion.md)已获所有者批准并配套更新 Core、协议、SDK 和 GUI。补齐本地安装元数据、说明/搜索词、撤回、SDK/legacy 显示规则；已安装/未安装名称层级不同，可见 provider 直接提供来源标签/顺序，版本按 Core Unity 分类分组。旧观测有明确状态与手动补采入口。受影响 Rust 461 通过、3 项既有忽略；严格 Clippy、GUI Rust all-targets、fmt、xtask、公开形状与元数据检查、全工作区前端检查/构建通过；浏览器 86 个独立用例覆盖与验证复用范围见[记录](testing/package-presentation-20261108.md)。源码已提交为 `e0073759`，并按所有者后续要求更新本机 Core/GUI：State30 / Config12、候选 v2、ready，GUI 已打开且响应；原数据 33 张表的旧列记录与设置完整保留，沿用原 WebView profile。备份和核对见[更新记录](testing/package-presentation-update-20261108.md)。未推送，H7 人工状态保持。

- 2026-11-08 按所有者澄清修正项目候选可见性：Core 在形成软件包/provider/版本候选前排除关闭来源和关闭的预发布版本，不再下发禁用的预发布条目。安装/声明事实保留；无可见 provider 时显示“未知来源”，与本地缺少存储库的来源证据一致。另隔离设置 revision 改变前的旧候选。前端检查/构建、62 项定向浏览器、Core VPM 完整回归 163 通过/3 既有忽略、应用层 4 项、RPC 合同 5 项、fmt 与受影响严格 Clippy 通过；早期失败与运行环境边界见[验证记录](testing/package-candidate-visibility-20261108.md)。未提交、推送或部署运行中的 Core/GUI，M7/H7 状态保持。

- 2026-11-08 文件选择器重置基线：按所有者授权移除旧选择器专用提交，混合提交只保留共享弹窗动画；旧 `FilePicker` / `FilePickerControl` 及 CSS 全部不存在。相关入口直接拼接路径输入框和原生选择按钮，GUI 类型、Material 与本地化检查通过。下一步仅重建并接入明确指定的四处。历史重写后的 SHA 与既有 CI 证据 SHA 不同，历史结果不作为新候选通过；当前 Core/GUI 均未重启。
- 随后按已确认外观重建唯一 `FilePicker`，仅接入仓库列表导入、模板导入、添加已有项目和添加用户软件包；其余入口使用普通路径输入框和选择按钮。全工作区前端检查/构建及定向浏览器回归通过；早期旧焦点断言与截图宿主错误、复验范围及原生限制见[重置与重建记录](testing/file-picker-reset-rebuild-20261108.md)。软件包排序、100ms 等待与顶部加载条保留；未部署或主动重启原生 GUI/Core，未操作真实项目。

- `main` 当前工作树使用 [State31](../specs/storage/state-v31.md) 与 [Config12](../specs/config/settings-v12.md)。State31 清理撤除的项目展示副本，保留仓库规范化索引与 State29 仓库显示名；内部数据格式编号不代表产品发布版本。仓库操作资格、项目列表、包批量操作、Unity 版本选项和操作日志筛选由 Core 提供。GUI 使用同一套用例结果。
- 此前 Core 部署基于 `0a73acca`，报告 State29 / Config12 和 `ready`，已核对九组业务读取。历史证据位于 `target/m7-core-boundary-update-20261107/`；当前新路径运行环境见下方记录。
- 随后的 GUI 单独更新基于 `0a73acca` 加文件选择器工作树改动，记录了窗口响应、GUI profile 和同一 Core 进程。证据位于 `target/m7-shared-file-picker-update-20261107/`。CLI 仍匹配 `7fe5f157` 部署记录中的产物摘要，见 `target/m7-repository-core-update-20261107/`。各自的证据只覆盖对应构建。
- 此前一次 GUI 部署使用 `34cac0a2` 加当时文件选择器工作树修改，证据位于 `target/m7-file-picker-integration-update-20261107/`。当轮验证后恢复原 Core/GUI 二进制、数据目录和 profile；九组业务读取一致，Unity 进程未变，没有另行部署。
- 2026-11-08 当前工作树将所有页面的切页首读等待统一限制为 **100ms**，超时显示目标页加载状态，不再无限停留于旧页。项目页先显示标题/工具栏，再独立加载列表或卡片；项目管理页的元数据与软件包首屏共用剩余预算，超时只让包列表继续读取同一个请求。旧响应隔离、分页/修订/权限及操作资格保持。前端全工作区检查与构建通过；浏览器扩展轮覆盖 150 项，其中 149 通过、1 项旧等待时序断言失败，修正测试后导航/管理页 **25/25** 通过；项目初始读隔离收束后导航/项目修改时间 **24/24** 通过，未变范围复用前轮结果，均无自动重试或跳过。证据位于 `target/m7-package-navigation-100ms-20261108/`。本轮未替换或重启原生 GUI/Core，未操作 Unity；Rust/远端 CI 未重跑，M7/H7 状态保持。
- 上一次 GUI 部署使用 `aced843c` 加软件包排序及“立即显示管理页基础内容、仅软件包列表延后加载”工作树修改，原 profile 保留，前端检查/构建、相关测试 **92/92** 及 Tauri debug no-bundle 构建通过。当时记录 GUI PID260640、原 Core PID218092；证据位于 `target/m7-package-list-loading-20261108/`，更早整页加载方案位于 `target/m7-package-navigation-20261108/`。该原生构建不包含当前全局 100ms 修改，浏览器结果不代替本机窗口内部人工验收。
- 所有者随后要求重建并打开当前 GUI：Tauri debug no-bundle 构建通过，复用前端的五份源码及 JS/CSS 摘要与已验证输入一致。新窗口 PID255916 已打开且响应，产物 SHA256 `62DAF418ED7506339CF757AC09CA0BD33B62F93D6BC5B7FE8D1D7C27B97BAD55`。重开前未发现 GUI/Core 进程；此前数据目录 `target/m7-icon-acceptance-20260906/data` 和 WebView profile 均不存在。窗口使用本轮独立临时 profile，VCC 注册关闭；尚未启动 Core、恢复备份或替换数据，数据来源等待所有者指定，不能计作实际项目页面交互通过。启动证据位于 `target/m7-package-navigation-100ms-20261108/native-reopen/`。
- 所有者已允许新路径，当前 Core locked debug 构建通过，PID263632 使用 `target/m7-package-navigation-100ms-20261108/native-reopen/data` 初始化新数据并报告 `ready`，自动发现 1 个项目，未恢复旧备份。GUI PID255916 按要求保持运行，可用现有“重新连接并重试”入口连接；桌面控制未获准操作该 GUI，未验证原生按钮后的结果。Core 及 GUI 身份、只读 RPC 证据仍在上述 `native-reopen/`。
- 当前源码新增整页加载条：4px Material 不定进度条位于主内容画布顶部，沿用主题和减少动画设置；局部列表/弹窗加载边界保持。前端检查/构建通过；67 项范围首轮 64 通过、3 项新增测试的宿主定位错误，修正测试后导航/管理页 **28/28** 通过，未变产品源码和其余 39 项复用首轮结果，零自动重试/跳过。证据位于 `target/m7-page-loading-bar-20261108/`。现有原生 GUI 尚未重建进该加载条，M7/H7 人工签收保持待定。
- 本轮源码整合候选为 `213d0d2`：独立文件/文件夹选择内容接入仓库列表导入、模板导入、添加已有项目和用户软件包登记；调用方负责确认、预览及弹窗生命周期。另同步旧浏览器预期、State29 精确断言，以及 Config12 和已有平台功能的元数据校验。只有标准 `main` 工作树，本地归档分支均为其祖先，无 stash；截图及生成材料保留在本地。
- 该候选的完整 Rust fmt、严格 workspace Clippy、workspace 测试及仓库检查通过，测试汇总 772 通过、13 项既有显式忽略。前端全工作区检查/构建和 Chromium **514/514** 通过，零重试、零跳过；元数据及 7 组内存反例通过。指纹已核对各验证范围的输入未变；早期失败轮继续保留在 `target/m7-main-integration-20261107T040906Z/`。原生系统选择窗口仍待人工核对。
- `e5ede89f` 的 [CodeQL 42340358565](https://github.com/ALCOMD/ALCOMD/actions/runs/42340358565) 四语言通过，[CI 42340358982](https://github.com/ALCOMD/ALCOMD/actions/runs/42340358982) 三平台未通过：Unix 测试模块位置和 Windows 专用 HTTP 测试钩子的编译范围触发严格 Clippy；Windows 两项设置测试争用用户 IPC 端点。`de3c06a`、`41a1653` 修正这三处测试边界，保留原断言及生产端点合同。默认并行 workspace 回归 772 通过、13 项既有忽略，两线程设置 IPC 额外三轮共 6 项通过，fmt 和受影响严格 Clippy 通过。原 Core、数据和 profile 保留，GUI 维持原先关闭；Unity 原有编辑器新增导入子进程，清单不完全相同，未操作这些进程。证据位于 `target/m7-ci-e5ede89f-20261107/`；后续 Hosted 结果按实际推送 SHA 从 [Actions](https://github.com/ALCOMD/ALCOMD/actions?query=branch%3Amain) 核对。
- `b1ad980c` 的 [CodeQL 42372423392](https://github.com/ALCOMD/ALCOMD/actions/runs/42372423392) 四语言通过，[CI 42372423496](https://github.com/ALCOMD/ALCOMD/actions/runs/42372423496) 继续发现测试生命周期问题：Unix 四项版本探测夹具并行争用两个生产探测名额；Windows 私有管道创建测试在终态后的归档清理释放前删除目录。整改仅为共享探测夹具互斥，以及私有测试运行时释放后再删除目录；生产并发上限、超时和业务断言保持。定向 fmt、Windows Core/平台严格 Clippy、Linux/macOS 平台跨目标严格 Clippy 通过；私有管道创建测试连续 8 轮通过。跨目标检查不算 Unix 原生执行，后续 Hosted 结果以实际修复候选为准；证据仍存上述目录。
- `af1bf478` 的 [CodeQL 42374353138](https://github.com/ALCOMD/ALCOMD/actions/runs/42374353138) 四语言通过，[CI 42374353476](https://github.com/ALCOMD/ALCOMD/actions/runs/42374353476) 三平台 Check 和仓库契约测试通过；Unix 发布与 GUI 构建通过，浏览器为 Ubuntu 512/514、macOS 510/514。`d295d1a`、`2400ce0` 保留原断言，修正正常菜单关闭后的继续操作、弹窗缩放未稳定时采样，以及异步保存尚未接受就断言原生 checkbox 立即变更的测试时序；设置夹具可暂停/释放下一次提交。两项菜单各重复 8 轮通过；设置两项修复前 20 次中 9 次失败、修复后 20 次全通过；四个受影响文件共 44 项通过。证据位于 `target/m7-ci-af1bf478-20261107/`。
- 同轮 Windows 发布与 GUI 构建也通过，浏览器 339 项通过后触及 60 分钟 job 上限，被 GitHub 中断（2 项 interrupted、173 项未运行）；不是完整浏览器通过。仅将 Windows 作业上限改为 90 分钟，保留全部步骤、单用例超时、断言和其余平台上限。
- `7720f77a` 的 [CI 42379056402](https://github.com/ALCOMD/ALCOMD/actions/runs/42379056402) 三平台全部通过，各平台 Chromium 均为 514/514；[CodeQL 42379055861](https://github.com/ALCOMD/ALCOMD/actions/runs/42379055861) 四语言全部通过。结果绑定该候选，证据位于 `target/m7-ci-7720f77a-20261107/`，此前失败与中断记录保留。
- Material Web 菜单快速重开行为已由项目所有者评估：当前遵循[官方公开 API 与用法](https://material-web.dev/components/menu/)，保持现状，不增加规避逻辑，不列为待修、M7 整改或验收阻塞项。正常菜单功能、焦点和滚动条的正式测试保留，不另增快速重开专项 CI。

## 按任务查合同

| 任务 | 实施合同 |
| --- | --- |
| GUI、仓库、模板、操作日志、共享组件、可访问性、Portable UI | [M7 ExecPlan 任务索引](exec-plans/M7-official-gui-extension-ui.md) |
| 项目动作、包管理 | [项目与包功能收口](exec-plans/M7-project-package-functional-closure.md)、[包候选合同](exec-plans/M7-package-candidate-evidence-stop-a.md)、[VPM 事务](exec-plans/M4-package-transactions.md) |
| Unity 项目版本迁移 | [Unity 版本迁移合同](exec-plans/M7-unity-project-version-stop-a.md) |
| GUI 与公开 RPC 用例对照 | [M7 GUI RPC 覆盖](exec-plans/M7-gui-rpc-coverage.md) |

## 剩余边界与下一停止点

- H7 最终签收由项目所有者完成。模板的原生文件对话框、Unity 编辑器内导入及剩余真实桌面交互仍需核对。按 [桌面验收清单](testing/m7-official-gui-manual-checklist.md) 区分已签收和待验证项。
- `c273c76` 的三平台 CI/CodeQL 和各批本地验证分别绑定当时的候选。最终验收需要实际候选自己的适用验证证据。
- 本轮没有单独执行 13 项 `#[ignore]`：子进程入口由相应父用例使用；联网 VRCFury、Windows 单实例/注册表、专用 HTTP/代理隔离用例不计作本轮直接通过。macOS/Linux 原生实证和本次推送后的 Hosted CI/CodeQL 结果另行记录。
- v3 真实工程差分夹具仍属 M11。原生安装、更新、卸载和完整发行验收仍属 M12。后续里程碑按各自授权推进。

[M7 状态与验收历史](testing/m7-status-history-through-20261107.md) 保存原状态页的候选、签收、遗留事项、失败轮次及证据链接。
