# 四处 VPM 行为缺口修正

日期：2026-11-09。所有者明确授权修复[前轮审计](vpm-management-audit-20261109.md)的四项缺口，验证后更新 Core/GUI；合同见[卸载修正及后续授权](../exec-plans/M7-package-removal-confirmation.md)。在 main 工作区实施，保留既有修改；所有者随后授权本地提交，按软件包Core/合同、GUI请求隔离、软件包确认及加载展示分组，不推送，不扩大 M7/H7/M11 签收。

## 有效行为

| 缺口 | 修正后行为及证据 |
| --- | --- |
| F1 未锁定本地包 | 仅枚举 Packages 直属目录，读取有界 package.json，未锁定包的 vpmDependencies 参与保留、显式卸载冲突和安装/升级/降级保护。Resolve 与批量安装也纳入本地包依赖；依赖不会因此变为项目直接声明。本地包目录或清单变化使旧计划失效，payload 编辑不影响计划且不读取其内容。 |
| F2 无关孤立包 | 自动移除限定为所选包旧依赖闭包中不再使用的 locked 包。原有无关孤立包及其目录/锁定元数据保留；共享依赖、循环和显式冲突确认仍有覆盖。 |
| F3 单包重装 | 仅为选中目标查找固定版本来源、准备归档并替换，原 VPM 清单字节及未选中包保持。无关包缺失来源不阻挡；重装全部继续要求每个目标来源，不放宽来源/摘要校验。 |
| F4 原本缺失目录 | 初始目录缺失作为内部 absentRemovalPackages 证据冻结，Apply 可删除该锁定记录。计划后新增/消失的目录拒绝执行；发布及重复回滚不触碰后来出现的目录。 |

本地包目录上限沿用 4096，单份清单沿用既有限额、累计读取上限 16 MiB。拒绝链接/reparse，原包内容读取限制不放宽。普通包元数据指纹域更新为 v3，旧未执行计划需重新生成；Unity 迁移原内容指纹域和内容校验保持。这里的 v3 是指纹域修订号，不是引用参考仓库代码。

缺失目录证据仅在内部 formatVersion 4 ChangeSet 中持久化并参与摘要；公开 RPC DTO 不携带该字段，Core 投影测试覆盖其不外泄。没有新增数据库列、迁移、权限、公共 RPC 字段或生产依赖。按 v3 的行为自行实现，参考仓库只读，未复制或移植源码。

## 验证

证据目录：`target/m7-vpm-four-gaps-20261109/`。隔离合成项目使用真实 reader、store、resolver、Plan/Apply 和任务终态，不连接用户 Core，不写用户 Unity 项目。

- VPM 库最终 175 通过、3 项既有忽略：`vpm-last.log`。包含本地包对升级/降级的保护、Windows 独占锁住 payload 时仍可读取元数据，以及冻结缺失目录与重复回滚。
- Core 库 43 通过、1 项既有忽略；应用层 77 通过：`libraries.log`。内部证据到公开 DTO 的边界纳入覆盖。
- M4 合同 6、真实包事务 18、手动 Unity 例外 2、P6 形状/闭合 3 通过：`transactions-last.log`。新增六项包事务覆盖四个反例、目录及本地元数据过期、实际 Apply、Resolve 和批量入口；其余旧断言保留。
- Unity 迁移工作器 7 通过：`migration-boundary.log`，覆盖持久阶段恢复、恢复失败边界及目标身份/写入者校验。
- 暂停原 Core 后原生 RPC 6 通过、1 个子进程入口标记忽略（由父测试调用）：`target/m7-vpm-four-gaps-update-20261109/native-rpc.log`。含安装/卸载往返、手动 Unity 例外、源缓存保护、归档阶段恢复及既有破坏性事务 kill/restart 矩阵，未运行到用户项目。
- 受影响 VPM/application/store/Core/testing 全 targets 严格 Clippy 通过：`clippy-last.log`。前端工作区检查和 GUI 生产构建通过：`frontend-check.log`、`frontend-build.log`，保留既有 bundle 大小提示。
- 最终 Rust fmt、xtask、元数据和 diff 检查通过：`fmt-check-last.log`、`xtask-last.log`、`metadata-final.log`、`diff-check.log`。独立 Core 最终构建和启用 `tauri/custom-protocol` 的 GUI 构建通过：`core-build-final.log`、`gui-build.log`。
- 本轮 GUI 源码未新增修改；前轮记录的 17 项 GUI/SDK/产物输入摘要均一致，复用[既有 48 个不同浏览器用例及中文确认框证据](package-removal-confirmation-20261109.md)，不计作本轮重新执行。`inputs.json` 记录完整本轮输入。

上述共 337 项不同 Rust 测试通过；另有 4 项既有忽略未直接运行，1 个忽略的子进程入口由父用例实际调用。最终输入核对覆盖 706 个源码、合同和前端产物文件，基于工作区 HEAD `4c3a46c3` 加保留及本轮未提交差异；不将其表述为该 SHA 的干净候选。

## 更新运行环境

守卫脚本及实证：`target/m7-vpm-four-gaps-update-20261109/`。精确核对原 Core/GUI 路径、命令、profile、零活动操作，备份关闭后的完整数据及旧二进制；原生 RPC 在原 Core 停止后运行且通过，才替换候选程序。最终 Core、GUI SHA-256 与 `inputs.json` 产物摘要一致。

- Core PID **260200**，SHA-256 `645C84D0CE260F88A2B448E49C060C615F2C41633A0733FD02F17F8A643F787D`；GUI PID **272600**，SHA-256 `6D8BA24CA97A3D5E8AB641D5A97FA31C12DF068B5612758A672F46FBC37DF6EA`。
- Core ready，State31/Config12，0 活动任务；原 2 项目及路径、2 仓库与设置摘要保持；候选 v2 读取通过。
- 原数据 `target/m7-package-navigation-100ms-20261108/native-reopen/data`、原 `webview-profile` 保持，GUI 单窗口可响应，WebView 子进程使用原 profile。备份位于本轮更新目录的 `backup/`。
- 没有卸载用户 SDK 或写用户 Unity 项目，未执行本轮原生界面点击和截图，不将窗口响应当作真实包操作或 H7 人工验收。

## 失败轮次和范围

首轮事务 15 通过、1 失败（`transactions.log`）：既有发布失败/回滚测试在 Plan 后创建无清单的目录，新指纹规则提前拒绝为 PlanStale，使测试不能再到达原发布冲突。将该夹具冲突目录放到 Plan 前创建，保留全部发布失败、资产恢复和哨兵断言；新增测试另外覆盖 Plan 后目录出现/消失的拒绝。最终整套通过，未放松产品校验。

初次 Core/GUI 独立构建通过，随后补齐批量入口后重建最终 Core，早期 Core 产物不作为部署依据。显式 rustc 输出文件名产生的两条提示保留，不作为严格 Clippy 的替代。

额外原生 RPC 首轮 6 项被运行中的用户 Core 持有的 Windows 每用户单实例锁拒绝，均为 AlreadyRunning；没有执行测试事务，见 `boundary.log`。更新脚本在完整备份并暂停原实例之后、替换二进制之前单独执行该套件；失败则恢复旧程序。没有修改单实例规则或测试断言来绕过环境。

不宣称全部 VPM/v3 行为相同。未执行完整 workspace、Hosted CI、Linux/macOS 原生、官方 VPM 二进制全量差分或真实用户包卸载；没有为新增缺失目录证据另建进程 kill 矩阵，使用实际 Apply 和重复回滚回归。原有来源歧义、冻结 Plan/Apply、原子事务、手动 Unity 窄例外、100ms 导航和失败呈现政策保持。
