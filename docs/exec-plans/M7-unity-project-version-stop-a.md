# M7 Unity Project Version / Open Unity Model：contract-first Stop A

状态：proposal-only，等待项目所有者合同审阅；M7 production implementation 未开始，H2-A 为
`PAUSED_FOR_UNITY_MODEL_CLOSURE`。

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

## Launch configuration

Launch arguments 独立保留为 project launch config：

- proposal methods：`unity.projectLaunchConfig.get` / `unity.projectLaunchConfig.set`；
- capability：分别复用 `unity.read.v1` / `unity.manage.v1`；
- permission：分别复用 `unity.read` / `unity.manage`；
- bounds、禁止 `-projectPath`、无 shell、隐私规则保持 M5 合同；
- config 不包含 selection mode 或 installation ID。

## Open Unity exact-match contract

现有 `unity.launch` 只作兼容字段增加：optional `installationId`，含义严格为本次 one-shot choice。

| exact verified candidate 数量 | authority 行为 | GUI |
| --- | --- | --- |
| 0 | 不 spawn，返回 `unity_exact_installation_not_found` | `Migrate Project` / `Cancel` |
| 1 | daemon 选定唯一 candidate 并启动 | 直接启动 |
| >1 | 返回 `unity_editor_selection_required` | one-shot chooser；不显示 keep/default |

caller 提供 `installationId` 时，daemon 复验 installation 存在、verified、canonical exact version、revision 与 filesystem
identity。不得 closest/major-minor/patch fallback，不得 international/China alternative，不得保存选择。

launch idempotency fingerprint 绑定 project ID/revision/root identity/current canonical version、resolved installation
ID/revision/identity、launch-config revision 与 arguments digest。GUI/RPC 永不传 path、executable 或 argv。

## Migration public contract proposal

- capability：`projects.unity-migration.v1`；
- methods：`projects.planUnityMigration`、`projects.applyUnityMigration`；
- Operation kind：`projects.unity-migration`；
- new permission：`projects.unity-migrate`；
- external filesystem writer：仅 `builtin:local-owner`；Extension、Portable UI 与第三方 Principal 不可获得该 writer。

Plan params：ProjectId、target canonical Unity version、verified target InstallationId、positive expected Project revision、
idempotency key。选择 current version 返回 `no_change` 且不创建 durable Plan。其他成功结果返回 immutable Plan；Apply 只接受
PlanId 与 apply idempotency key，不重新 Plan。

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

Plan/response 不保存或公开 project path、Editor executable、argv、PID、stdout/stderr 或 journal locator。

### Classification

- `patch_or_minor_upgrade`；
- `major_upgrade`；
- `patch_or_minor_downgrade`；
- `major_downgrade`；
- `china_variant_change`；
- `unsupported_or_unsafe`。

同版本由 `no_change` 处理。classification 只描述风险和确认要求，不允许 Apply 静默重新选择 target。`unsupported_or_unsafe`
不能 Apply。

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

现有 package Plan/Apply 无法删除旧 UPM dependencies，且会用 migration 前项目版本解析兼容性。结论是
`UNITY_MIGRATION_REQUIRES_SECONDARY_PACKAGE_CONTRACT`。该 transition 在本 Stop A 为
`unsupported_or_unsafe`/blocked；不扩展 P6，不新增第三个 migration business table，不编辑 `Packages/manifest.json`。

## External Unity process authority

Migration 必须启动 verified target Unity 或未来单独批准的 audited mechanism；禁止只改 `ProjectVersion.txt`。

最小 invocation profile proposal：

```text
<verified-unity-executable> -quit -batchmode -projectPath <validated-project-root>
```

`-ignorecompilererrors` 和 `-logFile -` 是 v3 historical behavior，不进入默认 v4 proposal。调用仍使用独立 argv、无 shell；
完整 path/argv/output 不进入 public DTO、Event、activity、普通日志或持久 diagnostics。实现优先复用 std、现有 executable
validator 与 process evidence，不需要新 dependency、unsafe 或 platform API。

## Operation and recovery

durable phase contract：

| phase | durable fact | cancel | restart behavior |
| --- | --- | --- | --- |
| `accepted` | Operation/Plan binding 与 apply idempotency 已持久化 | 允许 | 重新做完整 preflight，不改变 Plan |
| `preflight_complete` | revision/identity/marker/installation/writer 全部按 Plan 复验 | 允许 | 再次复验；不得信任旧 writer observation |
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

success 必须同时满足：target process exit 0、同一 project root identity 重新观察成功、canonical current version exact 等于
target。exit 0 但 marker 未变、process outcome uncertain、identity/revision 变化或 partial write 都进入 `recovery_required`，不得
虚假 succeeded。只有 `state_committed` 才更新 Project revision 并写一个 Event。

Event：`project.unity_version_migrated`，安全字段只含 ProjectId、from/to canonical version、classification、OperationId 与
revision；不含 path、PID、argv、executable、stdout/stderr 或 installation locator。

## Stable errors proposal

- `unity_exact_installation_not_found`；
- `unity_editor_selection_required`；
- `unity_installation_not_found` / `unity_installation_invalid`；
- `unity_version_unverified` / `unity_version_mismatch`；
- `unity_migration_unsupported`；
- `unity_migration_plan_expired` / `unity_migration_plan_stale`；
- `unity_migration_recovery_required`；
- `unity_project_running` / `unity_launch_state_uncertain`；
- `revision_conflict` / `idempotency_conflict` / `permission_denied` / `internal_error`。

## State Schema v14 proposal

State v1-v13 migration history不改写。v14：

1. 新建 `project_unity_launch_config`，迁移所有现有 `arguments_json`、revision 与 timestamp；不迁移 selection mode 或
   installation ID。
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
- custom arguments 改为 launch-config get/set，不与 version selector 混合。
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

## Stop conditions

出现新 production dependency、新 unsafe、新 platform API、State v15+、第三张 migration business table、generic workflow
engine、generic process supervisor、generic filesystem writer、P6 semantic expansion、M11 work 或 H3-H7 时停止并请求审批。

## Stop A completion

本 proposal 通过本地文档/schema/metadata/baseline gates并形成独立 local commit 后停止，等待项目所有者审阅。不得 push，
不得 production implement，不得恢复 Visual Gate 2，不得进入 H2-B/H3-H7/M8/M9/M11。
