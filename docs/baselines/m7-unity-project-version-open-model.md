# M7 Unity 项目版本与 Open Unity 行为审计

状态：Stop A contract-only 审计与 owner seal 已完成；本文件不修改 active RPC、State Schema、Permission 或 production
implementation。最终合同通过远端 checkpoint 后方可进入已批准的 production implementation。

## 审计基线

- v3 只读源码：`ALCOMD3-v3-readonly` commit `4aa98ae4`。
- v4 sealed functional baseline：`d314b155374d31a8d7c0449c62efdea5dd745e72`。
- H2-A 暂停候选：`24b03916bd6d958c921b4039bd62bec00afe25d4`，由本地保护分支
  `h2-a-vg2-paused-unity-model` 保留；该候选没有被拒绝。

本审计只读取 v3 源码，不复制、移植、包装或改写 v3 实现。

主要源码证据：

| 基线 | 文件/authority |
| --- | --- |
| v3 | `vrc-get-gui/app/_main/projects/manage/index.tsx`、`-unity-migration.tsx` |
| v3 | `vrc-get-gui/lib/open-unity.tsx`、`lib/version.ts`、`components/unity-selector-dialog.tsx` |
| v3 | `vrc-get-gui/src/commands/project.rs`、`vrc-get-vpm/src/unity_project/migrate_unity_2022.rs` |
| v4 | `crates/alcomd-application/src/m5.rs`、`crates/alcomd-store/src/m5.rs`、migration `0011` |
| v4 | `crates/alcomd-protocol/src/lib.rs`、`apps/alcomd/src/m5_rpc.rs`、`apps/alcomd/src/m5_platform.rs` |
| v4 | `specs/rpc/m5-unity.schema.json` 与相关 client/CLI/GUI/tests |

## v3 项目 Unity 版本选择器

v3 的项目管理页把 `ProjectVersion.txt` 观察到的 Unity 版本作为当前值。下拉候选来自已发现的 Unity installation，按版本
字符串去重并降序排列；VRC 支持版本优先显示。选择当前版本不产生动作，选择其他版本进入 migration flow，而不是保存
“以后用哪个 Editor 打开”的偏好。

版本变化被分为：

- `changeChina`；
- `upgradePatchOrMinor`；
- `upgradeMajor`；
- `downgradePatchOrMinor`；
- `downgradeMajor`。

v3 比较器忽略 China suffix 的 precedence，并把同一 base version 的 China/non-China 切换归为 `changeChina`。目标安装先按
完整字符串匹配；某些流程在国际版缺失时会尝试 `c1`。后者不是新的 v4 exact-match 合同。

## v3 migration flow

- migration 前检查目标项目是否已有 Unity writer。
- 一个目标 installation 直接使用；多个 installation 只在本次 migration 中询问，不持久保存选择。
- VRC 支持的 patch/minor upgrade 可以确认后原地执行。
- VRC 支持的 major upgrade 提供 backup、copy、in-place 三种组合。
- 其他 upgrade/downgrade/China transition 使用明确的高影响确认。
- copy 生成独立 sibling project，backup 使用既有 backup flow；二者成功后再执行 migration，不与 migration 组成单一
  ACID transaction。
- v3 通过 Unity 启动参数 `-quit -batchmode -ignorecompilererrors -logFile - -projectPath <project>` 执行 migration，
  显示有界输出；exit code 0 即报告成功并刷新查询。

推荐版本提示是 v3 的 presentation/policy：VRC 2019 项目提示推荐的 2022，VRC 2022 非推荐 patch 提示推荐 patch，China
variant 提示 international variant。目标缺失时，只有特定推荐目标会显示对应 Hub affordance。新 v4 proposal 不把
“recommended”转化为 Open Unity fallback，也不把它当作 target installation authority。

v3 没有在成功前重新观察 `ProjectVersion.txt` 并证明它已经变为目标版本。v4 必须修正这个弱点：外部进程 exit code 0
只是证据之一，不是 migration 成功事实。

## v3 VRChat 2019 -> 2022 预处理

`projectMigrateProjectTo2022` 不只是解析并应用普通 VPM package plan。它还：

1. 从 `Packages/manifest.json` 删除旧 XR UPM dependencies；
2. 将 VRChat base/avatars/worlds/VPM resolver 等已安装 package 提升到与推荐 Unity 2022 兼容的版本；
3. 然后才调用目标 Unity。

当前 M4/P6 public package contract 不直接表达上述预处理。owner seal 已采用 migration-private
`vrchat-2019-to-2022-v1` preparation profile：复用现有 resolver/cache/integrity/package transaction primitives，并把两个固定
XR UPM dependency、四个固定 VRChat package ID 与有界 legacy/unlocked cleanup 收敛在唯一
`projects.unity-migration` Operation 内，不扩展 P6 public contract。

### Exact mutation inventory

`migrate_unity_2022.rs` 与它实际调用的 `add_package_request`/`apply_pending_changes` 形成以下完整 mutation inventory：

1. source Unity major 必须为 2019；未发现 VPM VRCSDK 只记录 warning，不直接修改项目。
2. 从 UPM `Packages/manifest.json.dependencies` 删除
   `com.unity.xr.oculus.standalone` 与 `com.unity.xr.openvr.standalone`。
3. 在四个固定 package ID 中，仅对当前 locked 的 package 处理：`com.vrchat.base`、`com.vrchat.avatars`、
   `com.vrchat.worlds`、`com.vrchat.core.vpm-resolver`。
4. 对上述已安装 package 选择与 VRChat recommended Unity 2022 compatible 的 latest non-prerelease candidate；缺少任一必要
   candidate 时失败。`InstallToDependencies` 会把需要升级的 package 写入 direct dependencies，但不会把已锁定的更新版本
   降级到较旧 candidate。
5. resolver 补齐 transitive dependencies、替换旧 package tree、移除不再使用的 locked transitive packages，并更新
   `Packages/vpm-manifest.json` 的 `dependencies`/`locked`。
6. 如果新 package metadata 声明 `legacyPackages`，移除对应已锁定旧 package。
7. 对与待安装 package ID 冲突的 unlocked package directory 执行移除。
8. 对新 package metadata 新增、旧安装版本未声明的 `legacyFiles`/`legacyFolders`，先限制在 `Assets`/`Packages` 下并按
   path/GUID 查找，再在 package transaction 成功后删除。
9. package extraction/install/manifest write 失败时尝试回滚 package tree；legacy asset removal 位于成功后的 cleanup，v3 不提供
   M2/M4 级 durable crash journal。

新的 v4 proposal 只复用行为需求，不复制 v3 的 partial-failure 或非 durable cleanup 实现。

### v4 A/B/C feasibility mapping

| v3 requirement | 分类 | v4 proposal |
| --- | --- | --- |
| target-compatible latest stable resolve、transitive graph、unused locked removal | A | 复用现有 resolver，以 Plan target canonical major/minor 作为 internal compatibility input |
| pinned download/cache/SHA-256、package replace/remove、VPM manifest transaction | A | 复用 M4/P6 engine 与 recovery；不增加 public RPC |
| 删除两个固定 XR UPM dependencies | B | Unity-migration-private bounded preparation profile |
| 只处理四个固定已安装 SDK/resolver package | B | private profile 生成 existing resolver requests；不建立通用 migration DSL |
| `legacyPackages`、unlocked package conflict | B | 仅消费本次 pinned candidates 的 bounded metadata，validated quarantine 后随同一 journal推进 |
| `legacyFiles`/`legacyFolders` path/GUID cleanup | B | private bounded metadata parser + 既有 path/link/identity/quarantine 原语；不暴露 P6 surface |
| 新 generic package semantic | C | 未发现 |

结论：当前不需要修改 `packages.plan.v2` 或 P6 public contract。上述 A/B 复用规则已获 owner seal；若 production 审计证明
B 类无法在既有 M4 transaction/journal primitives 内安全表达，必须停止并报告
`UNITY_MIGRATION_SECONDARY_CONTRACT_NEEDS_OWNER`。

## v3 Open Unity

Open Unity 与 migration 的核心区别是：它只需要为当前项目版本选择一个可执行 installation，不改变项目版本。

- 按项目版本字符串寻找 installation。
- 零个匹配时提供安装/替代提示；v3 可建议 China/international alternative。
- 一个匹配时直接启动。
- 多个匹配时显示 chooser。
- v3 chooser 可以“keep using”并按项目路径持久化 installation preference。
- v3 custom launch arguments 与 installation preference 是独立用户能力。

新的 v4 模型保留 custom arguments，但拒绝 alternative-version fallback 和持久 installation preference。

## 当前 v4 model

当前 State v11 的 `project_editor_preferences` 同时存储：

- `automatic` / `explicit` selection mode；
- optional installation ID；
- project launch arguments；
- preference revision 与 timestamp。

当前 RPC/DTO/CLI/GUI 暴露 `ProjectEditorPreference`、`ProjectEditorSelection`、
`ProjectEditorSelectionState`、`unity.projectEditor.get/set/selection.get/clear` 以及 Automatic/Explicit/Forget UI。当前
automatic launch 以 Unity `major.minor` compatibility 选候选，而不是项目版本 exact match；一个候选启动，多个候选返回
`unity_editor_selection_required`。explicit installation 也只要求 major.minor compatible。

这与新产品模型冲突：用户看到的项目 Unity Version 应是项目事实，不应与“偏好的 Editor installation”并列成为第二个版本
权威。

## v4 已存在且可复用的 primitives

- Project observation 已读取 `m_EditorVersion` 与 optional revision marker。
- Unity installation registry 已保存 installation ID、version、revision、filesystem identity 与验证状态。
- writer-state 已有 `running_confirmed`、`running_suspected`、`not_observed`、`unknown` 四态。
- launch adapter 已使用独立 argv 调用验证后的 executable，并禁止 caller 提供 `-projectPath`。
- M2 Operation/Event/revision/idempotency、M4 filesystem journal、M5 backup/copy、P7 project locks 可复用。
- `Project(ProjectId)` 已是资源锁，不需要新通用 lock kind。
- M4/P6 `packages.plan.v2`/Apply、package cache/integrity 与 recovery 可在 secondary contract 获批后被组合，但当前不能改写
  UPM manifest；State v13 继续是完整历史基线，不回改旧 migration。

## Launch arguments disposition

Launch arguments 是已冻结的 M5 用户能力，不是 accidental preference：feature parity、M5 ExecPlan、CLI/RPC tests 都覆盖
bounded per-project argv；参数最多 64 项、单项最多 4096 字符，不经 shell，且不能覆盖 `-projectPath`。

因此 Stop A 提议把 arguments 从 installation preference 中分离为 `project_unity_launch_config`，而不是删除。上层仍只传
bounded arguments，不传路径、executable 或完整 command line；普通日志、Event、activity 与 diagnostics 不保存 argv。

## 审计结论

1. Project observation 是唯一用户级 Unity version authority。
2. Project header selector 表示 version migration，不是 Editor preference。
3. Automatic/Explicit installation preference 应在 State v14 移除。
4. Open Unity 使用 canonical exact version：0 个不启动、1 个直接启动、多个 one-shot chooser。
5. one-shot installation ID 只绑定当前 launch/idempotency，不持久保存。
6. migration success 必须由 target Unity exit 与 project reobservation 共同证明。
7. Backup/Copy 是 migration 前置 composition，不是一个跨外部进程的 ACID transaction。
8. VRChat 2019 -> 2022 使用 owner-sealed、migration-private `vrchat-2019-to-2022-v1` preparation profile；不得调用
   public package Apply 形成 child workflow，也不得扩展为通用 migration DSL。
