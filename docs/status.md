# 项目状态

最后更新：2026-11-22

## 当前阶段

M11 正在实施[三端调用、客户端管理与扩展入口](exec-plans/M11-client-parity.md)，从当前生产注册和[三端操作矩阵](exec-plans/M11-client-operation-matrix.md)核对用户入口。设置页「客户端」的安装发现及扩展 CLI 声明属于新增能力。

项目和软件包分域候选的首发清单涵盖 131 个 RPC、45 项能力和 76 个内建 MCP 工具，原生 SDK 提供 116 个方法。项目登记、创建和复制预演已共用原生方法，各端入口见[三端操作矩阵](exec-plans/M11-client-operation-matrix.md)。手动创建的模板从选定的内置或用户模板继承内容，成品独立使用。v3 派生模板迁移只生成 `2022.3.22f1` 的版本。[首发合同及验收范围](exec-plans/M11-first-release-contracts.md)

**M11 实施中。** 普通 ZIP 和 Core 主机绝对路径已接通。首发 State1 和 Config1 已通过 Windows 隔离测试，涵盖正常重开、结构不符时保全文件以及设置恢复。Windows 已通过绝对路径 IPC、MCP HTTP/OAuth、Core 重开后的配置和项目对账，以及普通 ZIP 的定向验证。隔离签名 bridge 已执行暂存和回滚，真实 3.4.0 样本和跨平台验收待完成。[M11 实施计划](exec-plans/M11-v3-migration-and-zip.md)

软件包默认显示预发布版，安装和升级优先兼容稳定版。CLI 和 MCP 安装、升级均可省略版本。Windows 已通过真实 Core 的候选与设置重启测试、MCP 安装升级、27 项 VPM 回归和 74 项软件包浏览器测试。[配置规范](../specs/config/settings-v1.md)

M11 仓库调序让列表顺序决定同版本包的仓库来源优先级。仓库移动、专用格式 v1 与验证记录见[仓库顺序与来源优先级](exec-plans/M11-repository-order.md)。

Unity Hub 使用系统登记发现和打开，Editor 优先读取官方安装清单，再通过系统登记补齐。Windows 已通过真实 MSIX 启动请求和隔离 Core 的 Editor 发现验证。macOS 和 Linux 已通过交叉编译，真实桌面流程待验收。[实施与验收记录](exec-plans/unity-application-discovery.md)

**M10 已完成。** 签名候选 `dab76fcd` 的三平台 Hosted CI 和四项 CodeQL 均通过，Chromium 每平台 738 项通过。Rust 与 TypeScript SDK 共覆盖 94 个公开方法，含官方 GUI 的全部 93 个 Core 方法。三个平台的 npm 开发包已完成外部安装和真实 IPC 验收。[M10 验收记录](exec-plans/M10-native-sdk-and-rpc.md#2026-11-20-验收结果)

**M9 已完成。** 第一方 Discord 扩展已通过公开 Extension API 接通 Core 和 Host，并沿用 v3 的 Discord 应用身份。主仓候选 `feca8df3` 的三平台 Hosted CI 和四语言 CodeQL 全部通过，Chromium 各 738 项通过。独立扩展候选 `0ff04bc` 已通过三平台构建及 Actions、Rust 和 Python CodeQL。Windows 已完成真实 Unity 与 Discord 的 IPC 验收，Ubuntu 和 macOS 的真实桌面验收按计划延期。[M9 收尾记录](exec-plans/M9-discord-first-party-extension.md#2026-11-20-m9-收尾)

M8 冻结候选为 `622cb62a`，Windows、Ubuntu 和 CodeQL 已通过。macOS 未获运行器，按所有者决定延期补验。其他遗留验收项见 [M8 收尾记录](exec-plans/M8-mcp-protocol-and-management.md#2026-11-20-m8-收尾)。

## 按任务查合同

| 任务 | 实施合同 |
| --- | --- |
| GUI、仓库、模板、操作日志、共享组件、可访问性、Portable UI | [M7 ExecPlan 任务索引](exec-plans/M7-official-gui-extension-ui.md) |
| 项目动作、包管理 | [项目与包功能收口](exec-plans/M7-project-package-functional-closure.md)、[包候选合同](exec-plans/M7-package-candidate-evidence-stop-a.md)、[VPM 事务](exec-plans/M4-package-transactions.md) |
| Unity 项目版本迁移 | [Unity 版本迁移合同](exec-plans/M7-unity-project-version-stop-a.md) |
| GUI 与公开 RPC 用例对照 | [M7 GUI RPC 覆盖](exec-plans/M7-gui-rpc-coverage.md) |
| Discord 第一方扩展（M9 已完成） | [M9 实施顺序、公共合同与验收记录](exec-plans/M9-discord-first-party-extension.md) |

## 剩余边界与下一停止点

- H7沿用所有者整体认可及条件收尾授权；新增修正按实际门禁和运行交付验收，不以浏览器fixture冒充原生人工操作。Portable UI原生表单/超时/外部断线等旧未覆盖项保持，见[桌面验收清单](testing/m7-official-gui-manual-checklist.md)。
- 最终功能证据绑定`c46f7544`；此前候选的失败、分轮结果和历史三平台基线分别保留，不替代本轮完整通过。收尾文档为后续独立提交，不声称其新CI已完成。
- 14项原有`#[ignore]`未全部直接单跑；子进程入口部分由父回归调用，联网VRCFury、Windows单实例/注册表、显式公开仓库快照及专用HTTP/代理隔离项不计作本轮直接通过。
- v3 真实工程差分夹具仍属 M11。原生安装、更新、卸载和完整发行验收仍属 M12。后续里程碑按各自授权推进。
- M7 已正式完成，M8 已按所有者要求收尾，M9 和 M10 已验收完成。Discord 启用状态迁移和随包装配分别归入 M11 和 M12。原生 Core 安装登记由 M12 安装器维护。[M10 验收记录](exec-plans/M10-native-sdk-and-rpc.md#2026-11-20-验收结果)

[M7 状态与验收历史](testing/m7-status-history-through-20261107.md) 保存原状态页的候选、签收、遗留事项、失败轮次及证据链接。
