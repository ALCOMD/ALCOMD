# M7 软件包展示补齐

状态：所有者于 2026-11-08 批准候选 v2 与 State30；本地实现及定向验收完成。公开读合同为有意的不兼容变更，不保留 v1 并行实现。未提交、推送或部署，H7 人工签收保持。

所有者于 2026-11-08 要求补齐已核实的 v3 展示差异，并区分已安装/未安装包。实现独立编写；v3 仅用于行为审计，不复制、移植或改写其源码。前一批关闭来源/版本可见性修改保留。

## 已授权且不扩展合同的部分

| Before | After | Why |
| --- | --- | --- |
| 已安装/未安装名称使用相同强调度 | 已安装名称使用 on-surface / 600；未安装使用 on-surface-variant / 400；保留控件的正常透明度与交互 | 区分安装状态，同时不让可安装的包看起来被禁用 |
| 来源触发框默认显示“自动选择来源” | 一个来源显示来源名，多个显示“多个来源”并提供完整名称 tooltip；菜单仍可自动/显式选择 | 分开来源事实和选择操作，保留 v4 的显式来源能力 |
| 最新列使用全局 latest，更新使用 projectLatest | 最新列与更新均使用 Core 的 projectLatest；另显示 Core 已判定的不兼容版本 | 显示与实际更新目标一致，GUI 不解析 SemVer/Unity |

本部分仅复用 Material option 的公开 `displayText` 属性、现有表格和主题 token，无新增依赖、图标或动画。验证范围含共享控件、软件包排序/筛选/Plan 流程，以及单一/多来源、安装样式和兼容目标的定向截图。

实际结果：GUI TypeScript/Material/本地化检查、全工作区前端构建和 83 项 Chromium 回归通过，零自动重试/跳过；截图已查看。没有新增 Core/Rust/公开合同修改。本批结果与前一批 Core 可见性验证分别记录于[验证记录](../testing/package-presentation-20261108.md)。

## 已批准的公开候选合同

将既有候选能力从 `packages.candidates.v1` 替换为 `packages.candidates.v2`，方法仍为 `packages.queryProjectCandidates`。不并行维护两个实现，不新增权限；继续要求现有四项读权限和精确 Principal/Project/source scope。由于现行 DTO 拒绝未知字段，此扩展按有意的不兼容读合同处理，Core、SDK、GUI 必须配套更新；旧客户端应通过能力协商发现不支持，不能继续按 v1 解析 v2 数据。

现有字段保留，新增字段草案：

```ts
type PresentationState = "complete" | "partial" | "unobserved";
// CandidateSummary
description?: string;
installedYanked?: boolean; // 缺失表示未知，不将缺失当 false
presentationState: PresentationState;
// CandidateProvider
displayName?: string; // 用户覆盖名
name?: string;        // 声明名或用户包名称
builtin?: "official" | "curated";
sourceKind: "remote" | "local" | "user-package";
displayOrder: number; // Core 计算的稳定展示序号，不决定 resolver 来源
// ProjectCandidatePage
presentationComplete: boolean;
```

- Core 从项目内已安装包清单优先取得显示名、说明、关键词/别名、文档/更新链接、撤回状态及 legacyPackages；版本与锁定事实不能被展示字段覆盖。清单缺失、版本不匹配或不可读时标为未知/部分，不伪装成未安装。
- 仓库候选仍提供未安装包信息。关键词/别名仅用于 Core 搜索，无需下发给 GUI；description 用于说明 tooltip。字符串、数组、读取字节与总响应沿用现有有界规范，并为新增元数据设定显式总量上限。
- 来源标签和排序直接从可见 provider 返回。普通行不再读取完整注册列表来认定来源；完整清单由筛选/管理控件持有。没有可见 provider 显示未知来源，关闭来源与缺少来源保持等价，项目内已安装信息独立保留。
- 已撤回版本从可选版本中移除；已安装撤回状态独立显示。可见来源判断与是否可安装分离，不能把 Unity 不兼容直接当作没有来源。
- Core 根据已安装 SDK、可见候选依赖关系排除另一种 SDK 及其未安装依赖者；已安装冲突项仍保留。已安装包声明的 legacyPackages 仅隐藏未安装的被替代包，证据未知时不猜测删除。
- GUI 按 Core 的 Unity 分类分组显示版本，保留现有权限/歧义/精确来源/Plan 资格；不将同号但来源不同的条目合并为一个无来源选择。

## 已批准的 State30

当前 State29 未持久保存已安装包展示信息，仓库索引也未完整保存关键词/别名和 legacyPackages。新增只用于展示观测的结构：

1. `project_package_presentations`：以 projectId/packageId 标识已安装清单的规范化显示元数据、观测版本、受控链接、撤回状态、legacyPackages 及清单指纹。
2. `project_presentation_observations`：记录项目展示观测 revision、对应项目 revision 和 complete/partial/unobserved 状态。候选 fingerprint 纳入它，避免复用陈旧展示缓存。
3. `repository_package_versions.presentation_json` 可空列：保存规范化关键词/别名和 legacyPackages；现有 description、dependencies_json 和链接列复用。用户包复用现有 manifest_json，经同一规范化入口读取。

State29 → State30 只增加结构，保留旧记录、revision、事件、已创建 Plan 和冻结的 Apply/recovery authority；不删除历史数据。旧展示观测明确为 unobserved，不以空数组伪装已采集。Core 通过现有项目检查/刷新与仓库刷新用例补采，更新与对应观测事务绑定。候选查询继续只读取已注册的规范化快照，不在每次分页扫描磁盘或联网。展示观测不得进入 resolver/Plan/Apply 的冻结授权投影；需要回归现有不可变字节、权限、修订、游标及恢复测试。

## 验收与停止点

- 现有 Core 可见性与安装事实断言保留；增加来源关闭/缺失时本地安装信息仍保留、关键词检索、撤回版本、SDK/legacy 包取舍与实际字段状态的回归。
- 全工作区前端检查/构建、相关浏览器、受影响 Rust/Store/协议/SDK 测试、fmt、严格 Clippy、仓库合同检查及迁移/冻结 authority 回归必须通过。原生与 Hosted 结果按实际执行记录，不代替 H7 人工签收。
- 所有者已批准本合同与 State30。2026-11-08 后续明确要求“提交并更新”，授权提交及更新本机运行中的 Core/GUI；远端推送仍未获授权。

## 实施依据

VPM 清单按 [VRChat 官方包规范](https://vcc.docs.vrchat.com/vpm/packages/)读取；扩展别名及撤回标记采用 [vrc-get 发布文档](https://vrc-get.anatawa12.com/en/cli/releases/)公开记录的 `vrc-get.aliases` 与 `vrc-get.yanked` 语义（布尔值或撤回原因）。实现独立编写，未使用参考仓库源码。未增加生产依赖。


## 完成与验证

Core、SDK、协议与 GUI 已配套完成。项目检查/发布的展示信息独立存储；普通候选查询只读规范化快照。GUI 提供旧项目观测补采入口，失败保留安装事实与重试机会。SDK 依赖闭包以索引和队列遍历，避免逐行扫描整个目录；本地安装元数据也预先按 ID 索引。

受影响 Rust 测试共 461 通过，3 项既有 VPM 显式忽略；包含 436 项应用层/Store/VPM 测试与 25 项候选/权限/快照/项目动作/Plan 合同。Core、协议、客户端与测试全部受影响目标的严格 Clippy、原生 GUI Rust all-targets 编译检查、fmt、xtask、元数据校验及 26 项 DTO 正反例形状校验通过。前端全工作区检查/构建通过。浏览器覆盖 86 个独立用例：85 项套件通过，最后补采入口变更后 7 项展示用例通过，未变的 79 项复用前轮；零自动重试/跳过。验证、早期失败和范围边界见[记录](../testing/package-presentation-20261108.md)。

此结果是本地工作树验收，不是新发行或 H7 总签收。原生运行、真实数据库迁移及三平台 Hosted CI/CodeQL 尚未执行。
