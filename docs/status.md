# 项目状态

最后更新：2026-11-07

## 当前阶段

**M7 官方 GUI 与 Portable Extension UI 进行中，H7 等待项目所有者最终人工确认。** M0–M6 已验收。H2-A 和 Visual Gate 2 已签收，H2-B、H3–H6 已阶段性签收并保留各自的后续事项。

- `main` 源码使用 [State29](../specs/storage/state-v29.md) 与 [Config12](../specs/config/settings-v12.md)。State29 增加仓库显示名。仓库操作资格、项目列表、包批量操作、Unity 版本选项和操作日志筛选由 Core 提供。GUI 使用同一套用例结果。
- 本机 Core 最近一次有记录的部署基于 `0a73acca`，报告 State29 / Config12 和 `ready`，已核对九组业务读取。证据位于 `target/m7-core-boundary-update-20261107/`。
- 随后的 GUI 单独更新基于 `0a73acca` 加文件选择器工作树改动，记录了窗口响应、GUI profile 和同一 Core 进程。证据位于 `target/m7-shared-file-picker-update-20261107/`。CLI 仍匹配 `7fe5f157` 部署记录中的产物摘要，见 `target/m7-repository-core-update-20261107/`。各自的证据只覆盖对应构建。
- 本机最近一次 GUI 部署使用 `34cac0a2` 加当时文件选择器工作树修改，证据位于 `target/m7-file-picker-integration-update-20261107/`。本轮验证后恢复原 Core/GUI 二进制、数据目录和 profile；九组业务读取一致，Unity 进程未变，没有另行部署。
- 本轮源码整合候选为 `213d0d2`：独立文件/文件夹选择内容接入仓库列表导入、模板导入、添加已有项目和用户软件包登记；调用方负责确认、预览及弹窗生命周期。另同步旧浏览器预期、State29 精确断言，以及 Config12 和已有平台功能的元数据校验。只有标准 `main` 工作树，本地归档分支均为其祖先，无 stash；截图及生成材料保留在本地。
- 该候选的完整 Rust fmt、严格 workspace Clippy、workspace 测试及仓库检查通过，测试汇总 772 通过、13 项既有显式忽略。前端全工作区检查/构建和 Chromium **514/514** 通过，零重试、零跳过；元数据及 7 组内存反例通过。指纹已核对各验证范围的输入未变；早期失败轮继续保留在 `target/m7-main-integration-20261107T040906Z/`。原生系统选择窗口仍待人工核对。
- `e5ede89f` 的 [CodeQL 42340358565](https://github.com/ALCOMD/ALCOMD/actions/runs/42340358565) 四语言通过，[CI 42340358982](https://github.com/ALCOMD/ALCOMD/actions/runs/42340358982) 三平台未通过：Unix 测试模块位置和 Windows 专用 HTTP 测试钩子的编译范围触发严格 Clippy；Windows 两项设置测试争用用户 IPC 端点。`de3c06a`、`41a1653` 修正这三处测试边界，保留原断言及生产端点合同。默认并行 workspace 回归 772 通过、13 项既有忽略，两线程设置 IPC 额外三轮共 6 项通过，fmt 和受影响严格 Clippy 通过。原 Core、数据和 profile 保留，GUI 维持原先关闭；Unity 原有编辑器新增导入子进程，清单不完全相同，未操作这些进程。证据位于 `target/m7-ci-e5ede89f-20261107/`；后续 Hosted 结果按实际推送 SHA 从 [Actions](https://github.com/ALCOMD/ALCOMD/actions?query=branch%3Amain) 核对。
- `b1ad980c` 的 [CodeQL 42372423392](https://github.com/ALCOMD/ALCOMD/actions/runs/42372423392) 四语言通过，[CI 42372423496](https://github.com/ALCOMD/ALCOMD/actions/runs/42372423496) 继续发现测试生命周期问题：Unix 四项版本探测夹具并行争用两个生产探测名额；Windows 私有管道创建测试在终态后的归档清理释放前删除目录。整改仅为共享探测夹具互斥，以及私有测试运行时释放后再删除目录；生产并发上限、超时和业务断言保持。定向 fmt、Windows Core/平台严格 Clippy、Linux/macOS 平台跨目标严格 Clippy 通过；私有管道创建测试连续 8 轮通过。跨目标检查不算 Unix 原生执行，后续 Hosted 结果以实际修复候选为准；证据仍存上述目录。
- `af1bf478` 的 [CodeQL 42374353138](https://github.com/ALCOMD/ALCOMD/actions/runs/42374353138) 四语言通过，[CI 42374353476](https://github.com/ALCOMD/ALCOMD/actions/runs/42374353476) 三平台 Check 和仓库契约测试通过；Unix 发布与 GUI 构建通过，浏览器为 Ubuntu 512/514、macOS 510/514。`d295d1a`、`2400ce0` 保留原断言，修正正常菜单关闭后的继续操作、弹窗缩放未稳定时采样，以及异步保存尚未接受就断言原生 checkbox 立即变更的测试时序；设置夹具可暂停/释放下一次提交。两项菜单各重复 8 轮通过；设置两项修复前 20 次中 9 次失败、修复后 20 次全通过；四个受影响文件共 44 项通过。证据位于 `target/m7-ci-af1bf478-20261107/`。另已复现依赖库在关闭动画中途重开时可出现 open 但不可见，此批只校正正常关闭场景，不宣称修复该交互限制；[Material 事件合同](https://material-web.dev/components/menu/) 明确区分 closing 与 closed。后续完整结果仍按实际候选核对。

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
