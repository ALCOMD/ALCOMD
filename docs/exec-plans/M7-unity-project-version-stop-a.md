# M7 Unity Project Version / Open Unity Model：contract-first Stop A

状态：checkpoint `a3faa7b362dbcaa1fccbcd4f84bcc55989a4a68b` 的 owner review 为
`PASS WITH REQUIRED CONTRACT AMENDMENTS`；本文件已形成 proposal-only amendment candidate，等待再次审阅。M7 production
implementation 未获批准，H2-A 为 `PAUSED_FOR_UNITY_MODEL_CLOSURE`。

## 目标

用一个直接的产品模型替换当前持久化 Editor preference：项目观察到的 Unity version 是唯一用户级版本事实；版本选择器
发起 project migration；Open Unity 只为当前 exact version 选择一次 installation。Stop A 只冻结拟议合同和测试向量，不修改
active RPC、State Schema、Permission、production code 或 H2 visual candidate。

## 产品模型

最终项目 header 语义为：

```text
←  Project Name        Unity [current project version ▾]  [Open Unity]  [⋮]
```

- current version 始终显示，即使对应 Editor 未安装。
- selector 候选仅来自 verified installation，按 canonical version 去重。
- 选择 current version 是 no-op，不创建 Plan/Operation/Event。
- 选择其他 version 是 migration request，不保存 preferred Editor。
- current 未安装时显示 `Current · Not installed`；这只描述本机 installation 状态，不否定项目声明的版本。
- Open Unity 只启动 current exact version；缺失时提供 `Migrate Project` 与 `Cancel`。
- 同一 exact version 有多个 installation 时，每次 launch 都询问，选择只对本次 launch 有效。

## Canonical Unity version v1

core/application 是 canonicalization 与 equality authority，TypeScript 只显示安全 DTO。

- canonical grammar：`major.minor.patch` + `a|b|f|p|x` + positive increment + optional `c` + positive
  China increment，例如 `2022.3.22f1`、`2022.3.22f1c1`。
- 数字不保留 leading zero；ASCII channel 统一 lowercase。
- installation target 必须可 canonicalize；无法验证时返回 `unity_version_unverified` 并 fail closed。
- exact equality 比较 major/minor/patch/channel/channel increment/China increment；revision hash 不参与 equality。
- SemVer/lexicographic/TypeScript locale compare 不得替代该合同。
- precedence 只用于 migration classification：`a < b < f < p`，同 channel 比 increment；`x` 或无法安全分类的
  transition 为 `unsupported_or_unsafe`。

`2022.3.22f1` 不等于 `2022.3.22f1c1`；不同 revision metadata 但 canonical version 相同仍属于同一个 version，
installation identity/revision 另行绑定。

## 删除 Automatic / Explicit model

State v14 production 获批后必须一次性删除：

- `ProjectEditorPreference`；
- `ProjectEditorSelection` 与 Automatic/Explicit；
- `ProjectEditorSelectionState`；
- per-project preferred installation ID；
- `unity.projectEditor.get/set/selection.get/clear`；
- `unity.project-editor.updated` 与 `unity.project-editor.selection_cleared` 的新写入；
- 对应 capability advertisement、client/CLI/GUI、store、idempotency、fixtures、tests 与文档。

4.0.0 尚未公开，不创建 compatibility alias 或 dual-write。历史 Event 可保留为 inert append-only evidence，但不再产生。

## Launch Config exact public contract

Launch arguments 独立保留为 project launch config，不增加 capability：

| method | request | result | permission |
| --- | --- | --- | --- |
| `unity.projectLaunchConfig.get` | ProjectId | config state | `unity.read` |
| `unity.projectLaunchConfig.set` | ProjectId、bounded arguments、expected config revision、idempotency key | config state、`changed`、`replayed` | `unity.manage` |
| `unity.projectLaunchConfig.clear` | ProjectId、expected config revision、idempotency key | default config state、`changed`、`replayed` | `unity.manage` |

config state 固定为 ProjectId、arguments、revision、updatedAtMs。missing state 返回 `arguments=[]`、`revision=0`、
`updatedAtMs=0`；stored revision 从 1 开始。same-value set 和 clear-missing 都是 no-op，不 bump revision、不写 Event，结果
`changed=false`。真实 set/clear change 精确将 config revision `+1` 并写一个 `unity.project_launch_config_changed` Event；Event
只含 ProjectId、revision、change kind，不含 raw argv、path、installation identity。

set/clear 使用永久 idempotency；replay 返回原结果。bounds、禁止 `-projectPath` 及其等价 authoritative selector、无 shell、
隐私规则保持 M5 合同。config 不包含 selection mode 或 installation ID。Project Unity Migration 绝不读取 launch config，固定
使用 host-owned argv profile。

## State v11 preference -> v14 launch config migration

State v1-v13 历史 migration 不改写。0014 对每个旧 `project_editor_preferences` row：

- `arguments_json` canonical 等于默认空数组 `[]`：不创建 launch-config row；
- 非空且合法：迁移 arguments，并原样保留 positive revision 与 updated timestamp；
- malformed/out-of-bounds：整个 0014 transaction fail closed，不静默丢弃；
- automatic/explicit 一视同仁，selection mode 与 installation ID 永久丢弃。

完成迁移后删除旧表；0011/0013 保持历史不变。

## launchOptions exact-match resolver

`unity.launchOptions` 是 `unity.launch.v1` 上的 read-only compatible addition，不新增 capability。request 只有 ProjectId 与
positive expected Project revision。response 固定包含 ProjectId、当前 Project revision、Core canonicalized
`projectUnityVersion` 与 `exactMatchingInstallations[]`。

permission 固定为 `projects.read + unity.read + unity.launch`：它同时读取 Project version、复用现有 installation safe DTO，
并准备一次 launch；capability 仍只有 `unity.launch.v1`。

candidates 复用现有 `UnityInstallation` safe DTO，因此 executable display path/source/architecture 等没有形成新的 `unity.read`
泄漏；所有候选都已由 Core canonical exact compare。排序固定为 canonical version bytes、source-kind closed order、
InstallationId bytes。GUI 可按 byte-equal canonical string 做 presentation grouping，但不得 parse version、判断 compatibility 或
upgrade/downgrade。

## Open Unity exact-match contract

pre-4.0.0 的 `unity.launch` request 直接改写为 required ProjectId、required one-shot InstallationId、positive expected Project
revision 与 idempotency key。Core 不再隐式选择 Editor。

| exact verified candidate 数量 | authority 行为 | GUI |
| --- | --- | --- |
| 0 | `launchOptions` 返回空 candidates，不调用 launch | `Migrate Project` / `Cancel` |
| 1 | caller 把唯一 InstallationId 传给 launch | 直接启动 |
| >1 | caller 必须选择一个 InstallationId | one-shot chooser；不显示 keep/default |

daemon 复验 Project 仍注册、Project revision、Project version 可 canonicalize、installation 存在、verified、filesystem
identity 有效，且 installation canonical version exact 等于 Project canonical version。不得 closest/major-minor/patch fallback，
不得 international/China alternative，不得保存选择。

launch idempotency fingerprint 绑定 project ID/revision/root identity/current canonical version、resolved installation
ID/revision/identity、launch-config revision 与 arguments digest。GUI/RPC 永不传 path、executable 或 argv。

## Migration public contract proposal

- capability：`projects.unity-migration.v1`；
- methods：`projects.planUnityMigration`、`projects.applyUnityMigration`；
- Operation kind：`projects.unity-migration`；
- new permission：`projects.unity-migrate`；
- external filesystem writer：仅 `builtin:local-owner`；Extension、Portable UI 与第三方 Principal 不可获得该 writer。

Plan params 只含 ProjectId、positive expected Project revision、target verified InstallationId、idempotency key。GUI 不同时提交
target version；Core 从 InstallationId 推导 canonical target version，并把它只作为 Plan output/evidence 固定保存。选择 current
version 返回 `no_change` 且不创建 durable Plan。其他成功结果返回 immutable Plan；Apply 只接受 PlanId 与 apply idempotency key，
不重新 Plan。

版本 selector 按 canonical version 去重；若用户选择的 target version 对应多个 verified installations，GUI 在 Plan 前显示一次性
chooser，并把所选 InstallationId 交给 Plan。该选择不写数据库/config/Project state，也没有 Remember/Clear action。

### Immutable Plan authority

Plan 固定保存：

- ProjectId、expected/current Project revision；
- source canonical Unity version；
- project root filesystem identity；
- `ProjectVersion.txt` bounded SHA-256 marker fingerprint；
- target canonical Unity version；
- verified InstallationId、installation revision 与 filesystem identity；
- writer evidence/revision；
- migration classification；
- plan idempotency authority；
- created/expires timestamps，TTL 900000ms。

TTL 固定 900000ms（15 minutes）。expired Apply 返回 `project_unity_migration_plan_stale`，safe reason=`expired`。同一个 Plan
idempotency key 永久 replay 原 Plan，即使它已经 expired；重新 Plan 必须使用新 key。

Plan/response 不保存或公开 project path、Editor executable、argv、PID、stdout/stderr 或 journal locator。

### Classification

- `patch_or_minor_upgrade`；
- `major_upgrade`；
- `patch_or_minor_downgrade`；
- `major_downgrade`；
- `china_variant_change`；
- `unsupported_or_unsafe`。

每个 classification result 同时包含 Core-authoritative `supportedForApply`，GUI 不从 enum 推导安全性：

| classification | 4.0.0 supportedForApply |
| --- | --- |
| `patch_or_minor_upgrade` | `true`，仍受 exact target/writer/Plan gates约束 |
| `major_upgrade` | `true`，但 VRChat 2019→2022 在 preparation owner seal 前为 `false` |
| `patch_or_minor_downgrade` | `false` |
| `major_downgrade` | `false` |
| `china_variant_change` | `true`，仅限相同 base release、exact verified target |
| `unsupported_or_unsafe` | `false` |

VRChat unsupported Unity versions 一律 `supportedForApply=false`。同版本由 `no_change` 处理。Apply 对 false 返回
`project_unity_migration_unsupported`，不得因 v3 曾显示按钮而放宽。

### Target and writer gates

target 必须已安装、verified 且 canonical exact。Plan 与 Apply 都复验 writer state；只有 `not_observed` 允许继续，
`running_confirmed`、`running_suspected` 与 `unknown` 均 hard reject。Apply 还复验 project revision/root identity/marker
fingerprint、target installation revision/identity 与 Plan TTL。

### Permission and locks

Plan 需要 `projects.read`、`unity.read` 与 `projects.unity-migrate`；Apply 需要 `projects.unity-migrate`。该新 permission
不隐含 `projects.manage`、`unity.manage`、`packages.manage` 或 arbitrary filesystem authority。

持有 `Project(ProjectId)` 整个 destructive/external-writer window；复用 Operation/idempotency authority。除非未来 secondary
package contract 获批，不增加 Repository/Catalog/PackageCache lock，不增加 ResourceKey。

## Backup / Copy composition

- Backup + migrate：先完成既有 backup Operation，再创建 migration Plan/Operation。
- Copy + migrate：先完成 project copy Operation，取得新 ProjectId，再对新项目 Plan migration。
- backup/copy 成功而 migration 失败时保留成功产物；不得声称跨两个 Operation 为 ACID。
- GUI 组合步骤各自显示 OperationId、错误与 retry；migration retry 复用原 Plan/Operation/idempotency，不重新 Plan。

## VRChat 2019 -> 2022

`UNITY_MIGRATION_REQUIRES_SECONDARY_PACKAGE_CONTRACT` 仍是 production blocker，但二次审计没有发现 C 类 generic semantic。
exact A/B/C mapping 位于 baseline audit：现有 engine 能做 resolve/cache/package tree/VPM manifest；migration-private bounded
preparation 需要删除两个固定 XR UPM dependencies、处理四个固定 SDK/resolver package，并对这些 pinned candidates 的
legacyPackages、unlocked conflict、legacyFiles/legacyFolders 做有界 quarantine/cleanup。因此不扩展 P6 public RPC。

private preparation 必须与同一个 Operation/journal 绑定：`preparation_intent` 前生成并 fsync operation-owned recovery artifact；
所有 UPM/VPM/package/legacy mutation 使用 identity revalidation、staging/quarantine、atomic publish 与 bounded evidence；
`preparation_complete` 前 crash/cancel 只能在验证外部未修改后回滚至 source snapshot。launch intent 后禁止自动 rollback，artifact
保留到 exact reobservation/cleanup；外部 modification 或 rollback evidence 不完整进入 recovery required。不得新增
`migration_package_plans`、第三张 business table或 generic workflow。

若 production 设计证明任一 B 类无法复用既有 M4 transaction/journal/path primitives，停止并报告
`UNITY_MIGRATION_SECONDARY_CONTRACT_NEEDS_OWNER`。

## Narrow UnityMigrationProcess platform proposal

Migration 必须启动 verified target Unity 或未来单独批准的 audited mechanism；禁止只改 `ProjectVersion.txt`。

future owner approval 后，`alcomd-platform/src/unity.rs` 可增加唯一 private/narrow safe surface：

```text
spawn_unity_editor_migration(
    validated_executable,
    authoritative_project_root
) -> UnityMigrationProcess

UnityMigrationProcess::wait(self) -> UnityMigrationExit
UnityMigrationExit = success | non_zero
```

它只供 Project Unity Migration 使用，不公开 kill/signal/arbitrary command/shell/arbitrary executable/arbitrary argv/PID authority。
实现只允许 safe Rust std 与现有 bounded blocking execution pattern；不新增 crate、windows-sys feature、raw libc、unsafe 或
generic process supervisor。

固定 invocation profile：

```text
<verified-unity-executable> -quit -batchmode -projectPath <validated-project-root>
```

`-ignorecompilererrors` 和 `-logFile -` 是 v3 historical behavior，不进入默认 v4 proposal。调用仍使用独立 argv、无 shell；
完整 path/argv/output 不进入 public DTO、Event、activity、普通日志或持久 diagnostics。实现优先复用 std、现有 executable
validator 与 process evidence，不需要新 dependency、unsafe 或 platform API。

## Provisional Operation and recovery

以下 phase 在 secondary preparation 获 owner seal 前仍是 provisional，不能作为 production implementation approval：

durable phase contract：

| phase | durable fact | cancel | restart behavior |
| --- | --- | --- | --- |
| `accepted` | Operation/Plan binding 与 apply idempotency 已持久化 | 允许 | 重新做完整 preflight，不改变 Plan |
| `preflight_complete` | revision/identity/marker/installation/writer 全部按 Plan 复验 | 允许 | 再次复验；不得信任旧 writer observation |
| `preparation_intent` | operation-owned rollback/quarantine artifact 已 fsync，首个 preparation mutation 即将发生 | 只可请求 verified rollback | 复验 artifact/project；安全时 rollback，否则 recovery_required |
| `preparation_complete` | UPM/VPM/package/legacy preparation 已验证，rollback evidence 仍保留 | 只可在 launch intent 前 verified rollback | 不重复 preparation；复验后推进 launch intent或rollback |
| `launch_intent` | 即将调用 exact target，且不能再证明从未 spawn | 禁止 | 先观察 process/writer/project；不得自动 respawn |
| `unity_started` | private PID/start-time evidence 表明 target 曾启动 | 禁止 | 观察而不接管/kill；无法判断进入 recovery_required |
| `unity_exited` | 已取得 bounded exit evidence | 禁止普通 cancel | 重新观察 project，不因 exit 0 直接成功 |
| `project_reobserved` | 同一 root identity 与 marker/current version 已重读 | 不适用 | exact target 才允许 state commit，否则 recovery_required |
| `state_committed` | Project revision 与唯一 Event 已原子提交 | 不适用 | 不重复 Event；继续 bounded cleanup |
| `cleanup_complete` | operation terminal success 已可证明 | 不适用 | 返回 durable terminal result |
| `recovery_required` | outcome 无法安全自动判定/推进 | 不适用 | 不 respawn、不伪成功，等待未来明确恢复入口 |

cancel 只允许在 durable `launch_intent` 前；之后不 kill Unity。`launch_intent` durable 后，crash recovery 不得在缺乏
proof 时自动 respawn，避免双启动外部 writer。private journal 可以保存 bounded PID/start-time/process evidence，但不得公开或长期
作为 business identity。

durable `launch_intent` 必须先于 spawn；spawn 返回成功后尽快写 `unity_started`。如果 crash 落在二者之间，必须观察
Project/process/writer，绝不简单 respawn。daemon restart 时若 evidence 表明 Unity仍在运行，Operation保持非终态并做 bounded
reobservation；无法重新获得原 Child exit status 时不伪造 exit code，最终仍由 Project exact reobservation 决定。

success 必须同时满足：target process exit 0、同一 project root identity 重新观察成功、canonical current version exact 等于
target。exit non-zero 但 Project 已 exact target 时不得直接回退，进入 recovery judgment；exit 0 但 Project 仍为 source version
是 failed；unreadable/partial/identity uncertain 是 recovery required。只有 `state_committed` 才更新 Project revision并写 Event。

Event：`project.unity_version_migrated`，安全字段只含 ProjectId、from/to canonical version、classification、OperationId 与
revision；不含 path、PID、argv、executable、stdout/stderr 或 installation locator。

## Stable errors proposal

- `project_not_registered`；
- `unity_editor_selection_required`；
- `unity_installation_not_found`；
- `unity_version_mismatch`；
- `project_unity_migration_plan_not_found`；
- `project_unity_migration_plan_stale`（safe reason 包含 `expired`）；
- `project_unity_migration_unsupported`；
- `project_unity_migration_source_changed`；
- `project_unity_migration_recovery_required`；
- `unity_project_running`；
- `revision_conflict` / `idempotency_conflict` / `permission_denied` / `internal_error`。

## State Schema v14 proposal

State v1-v13 migration history不改写。v14 当前仍是 proposal，secondary preparation journal evidence 经 owner seal 前不得宣称
final。最终目标保持：

1. 新建 `project_unity_launch_config`，只迁移非空 `arguments_json`、revision 与 timestamp；不迁移 selection mode 或
   installation ID；空参数不创建 row。
2. 删除 `project_editor_preferences`。
3. 删除 removed projectEditor methods 的未完成/可重放 idempotency records；保留合法历史 launch/Operation/Event evidence。
4. 新建 `project_unity_migration_plans` 与 `project_unity_migration_journal` 两张 migration business tables。
5. 不增加第三张 migration table、generic workflow/process/filesystem framework 或 State v15+。

## Security vectors

永久 proposal vectors 至少覆盖：canonical exact/China/revision equality、selector dedupe/current no-op、0/1/>1 launch、
one-shot non-persistence、symlink/identity swap、marker race、installation replacement、writer four-state gate、stale/expired Plan、
permission denial、backup/copy composition、launch-intent crash、uncertain process no-respawn、exit-zero-without-version-change、
partial marker、reobservation exact success、argv/output redaction 与 VRChat secondary-contract block。

## CLI / GUI disposition

- 删除 Automatic/Explicit/Forget Unity selection entry。
- 删除/替换 CLI `project-get-editor`、`project-set-editor`、selection clear 等 installation-preference surface。
- custom arguments 改为 launch-config get/set/clear，不与 version selector 混合。
- CLI/第三方 GUI先调用 `unity.launchOptions`；interactive 多候选可询问，non-interactive 多候选必须显式提供 InstallationId
  或返回 `unity_editor_selection_required`。
- selector dialog 文案必须说 `Migrate project to …`，Open Unity chooser 必须说 `Open once with …`。
- missing exact dialog 只提供 `Migrate Project` 与 `Cancel`，不暗示 closest version 安全可用。它进入与 header selector 完全
  相同的 migration flow；migration 成功并重新观察 exact target 后，再回到普通 exact-match Open Unity 并启动。

## P8 / H2 reconciliation

- P8 historical technical checkpoint 保留 `PASS`，不改写既有证据。
- 因用户可见 Unity product model 已改变，M7-owned visible-action completeness 当前为
  `REOPENED_BY_PRODUCT_MODEL_CHANGE`。
- global release completeness 继续 `BLOCKED_BY_M11`。
- H2-A 状态为 `PAUSED_FOR_UNITY_MODEL_CLOSURE`；`24b03916…` visual candidate 不是 rejected。
- production model 完成后从新的 main 重新对齐 H2 candidate，再恢复 Visual Gate 2。

## Amendment progress

- 2026-08-31：按 owner review 补齐 non-empty-only v11→v14 argument migration、Launch Config get/set/clear no-op/revision/
  Event、`unity.launchOptions`、required one-shot InstallationId launch、Core canonical authority 与 Plan single authority。
- downgrade 在 4.0.0 全部 fail closed；classification 另带 `supportedForApply`。VRChat unsupported versions 与
  2019→2022 preparation 未 seal 前均不可 Apply。
- v3 2019→2022 的 exact mutations 已逐项映射为 A/B/C；当前 C 为空，因此不扩 P6 public contract。B 类只允许
  migration-private bounded preparation，并必须纳入同一 journal 的 preparation intent/complete/recovery evidence。
- 增加 proposal-only `UnityMigrationProcess` safe surface；没有 production wiring、dependency、unsafe 或 platform API 变化。
- State v14 继续标为 proposal，等待 secondary preparation/phase owner seal。

## Stop conditions

出现新 production dependency、新 unsafe、新 platform API、State v15+、第三张 migration business table、generic workflow
engine、generic process supervisor、generic filesystem writer、P6 semantic expansion、M11 work 或 H3-H7 时停止并请求审批。

## Stop A completion

本 proposal 通过本地文档/schema/metadata/baseline gates并形成独立 local commit 后停止，等待项目所有者审阅。不得 push，
不得 production implement，不得恢复 Visual Gate 2，不得进入 H2-B/H3-H7/M8/M9/M11。
