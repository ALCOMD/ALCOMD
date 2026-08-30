# M7 Project / Package Functional Closure：contract-first Stop A

状态：Stop A 合同修正与 P0-P4 production implementation 已通过本地、三平台 Hosted CI、CodeQL 和项目所有者验收；
P5-A Create / Restore existing-Core GUI wiring 与 P5-B Favorite / Clear Unity Preference 均已通过本地完整验收、三平台
Hosted CI 与 CodeQL。P6-A、P6-B 与 P6-C 已完成获批合同和 production implementation，并通过适用的本地门禁、
同一最终候选的三平台 Hosted CI 与 CodeQL；P6 remote checkpoint 为 PASS。
P7 Delete Project Directory 已按项目所有者批准及实施修正完成 active RPC/Permission、State v13、mount-safe filesystem
primitive、Plan/Apply/Operation/recovery、CLI/GUI 与真实 fault tests，并在 sealed HEAD
`2ee11066c07a0994f3aebe6a9ce3f84ab2c8acd9` 通过 Hosted CI run `33298022030` 的 Windows、Ubuntu、macOS
及 CodeQL run `33298021806`。P7 remote checkpoint 为 PASS。P8 正在执行 M7-owned 可见 action 完整性审计；H2 visual WIP
继续暂停，M8/M9 未开始。

## 目标与边界

本 Stop A 把 v3 已有、但当前 M7 official GUI 尚未形成真实用户入口的 Project / Package 行为变成可审阅的最小合同。
P0-P4 已按批准范围落盘 dependency foundation、closed GUI affordance、active Copy RPC/State v10、filesystem
Operation/recovery 与 client/CLI/GUI flow；P5-B 又完成 Favorite 与 Unity selection/clear 垂直切片。P7 当前只冻结
Delete Project Directory 的 proposal/test vectors；H2 视觉推进、M8/M9、任何 production wiring、新 unsafe 或额外平台 API
仍明确不在本轮。

此前两个可见永久假入口 `Open Project Directory` 与 `Copy Project` 已成为真实入口，但这不等于全部功能缺口已经关闭。
`projects.management` 与 `packages.vpm` 必须继续保持 `in_progress`。

## Open Project Directory

最终架构提案固定为 official-GUI-local platform affordance：

```text
React(ProjectId only)
    -> closed typed Tauri command
    -> private Rust GUI adapter
    -> alcomd-client
    -> existing projects.get
    -> registered ProjectSnapshot.rootPath validation
    -> narrow platform opener
```

- Tauri command 只接受 `ProjectId`，不得接受 path、URL、executable、shell command 或 extension value。
- Rust adapter 每次重新调用 `projects.get`，复验项目仍注册、`rootPath` 存在且是目录，然后才调用 opener。
- 不新增 public RPC、DTO、State Schema、Permission 或 Extension/Portable UI capability。
- command busy 时入口是 current-state conditional disabled；不得永久 disabled。
- app-private error code 只允许 `project_not_registered`、`project_directory_missing`、
  `project_directory_not_directory`、`project_directory_open_failed`、`internal_error`。响应不得包含完整路径、shell、OS Debug。
- `tauri-plugin-opener` 已拒绝。批准的窄依赖为
  `open = { version = "=5.4.2", default-features = false }`；永久禁止 `insecure` 与
  `shellexecute-on-windows`，只允许 `open::that(validated_registered_project_root)`。

## Native directory chooser

当前 production GUI 没有 `gui_select_path`、dialog plugin、npm dialog binding 或 dialog capability；旧 Stop A fixture 中出现的
名称只是未落盘提案。Add/Register、Create、Restore 和 Copy target parent 都需要 host-owned native directory selection。

最小提案是注册 Rust-side `tauri-plugin-dialog = "=2.7.2"`，只由 closed typed Tauri command 调用 folder picker：

```text
gui_select_directory() -> { outcome: "selected", path: string } | { outcome: "cancelled" }
```

React 不提供初始任意路径，不接收 File handle，不直接调用 plugin command。production capability 不加入 `dialog:*`，不安装
frontend npm binding。plugin registration 是 Rust API 工作所必需，但 guest WebView 没有对应 capability。错误只返回 private
`directory_selection_failed` 或 `internal_error`，取消不是错误。该 exact dependency/feature 已获批准。

## Copy Project public contract

机器可读文件为 `specs/rpc/m7-project-copy.proposal.schema.json`；文件名保留审计沿革，但 publication 已切换为 implemented，
active RPC v1 规范、protocol、dispatcher 与 client 已接线。

- capability：`projects.copy.v1`
- methods：`projects.planCopy`、`projects.applyCopy`
- Operation kind：`projects.copy`
- Plan/Apply permission：`projects.read + projects.create`
- 外部 filesystem write：仅 `builtin:local-owner`
- 不新增 Permission 或 ResourceKey

### Bounded Plan

`projects.planCopy` 是 bounded synchronous preflight，不递归 source、不读取文件内容、不生成完整 inventory/tree fingerprint。
它固定：PlanId、owner Principal、source ProjectId/revision/canonical root/opaque filesystem identity、project kind、Unity
version/revision、writer evidence、target parent canonical path/opaque identity、normalized target leaf、
`targetMustNotExist=true`、预分配 target ProjectId、copy profile ID/version、exact include/exclude、quota、safe exclusion
summary、Plan fingerprint、idempotency evidence、createdAt 与 expiresAt。

现有 package/template/backup Plan 没有可复用的 TTL/expiry 惯例。Copy 的路径/identity/writer preflight 会随外部文件系统变化，
因此本 proposal 冻结一个新的窄值：

```text
planExpiryMs = 900000
expiresAtMs = createdAtMs + 900000
expired when nowMs >= expiresAtMs
```

过期返回 `project_copy_plan_stale`，safe subreason 为 `expired`。Plan row 保持 durable，不自动删除；永久 idempotency replay
仍返回原 Plan，调用方要重新 Plan 必须使用新 key。这个值必须在 production activation 前取得项目所有者批准，不能写成
implementation-defined。

### Apply inventory、staging 与 final revalidation

`projects.applyCopy` 重新验证 owner、permission、expiry、revision、source/target identity、writer evidence、Plan fingerprint 与
idempotency 后立即返回 durable OperationId；不得重新 Plan。

Operation phase 固定为：

```text
accepted
-> inventory_ready
-> staging
-> staging_complete
-> publish_intent
-> target_published
-> project_registry_commit_intent
-> state_committed
-> cleanup_complete
```

`inventory_ready` 在 worker 内执行 bounded traversal，并把完整 inventory 写入 Operation-owned private recovery/staging area 中的
versioned deterministic JSON Lines manifest。它不是 Project content、public API 或 State DB table，也不得写入
`evidence_json`。header 至少包含 formatVersion、OperationId、PlanId、source ProjectId/root identity、copy profile version、entry
count、total regular-file bytes、createdAt；每个 entry 至少包含 normalized relative path、entry kind、size、source filesystem
identity evidence 与适用的 executable bit。manifest 必须 bounded、crash durable，在 journal 进入 `inventory_ready` 前 flush/sync；
journal 只记录其 private locator、SHA-256 与 byte length。它不得通过 RPC 返回或写入日志，并在 `cleanup_complete` 后删除。

staging 读取每个 regular file 时同步写入 payload 并计算 SHA-256，把 staging/source-read digest 写入 inventory evidence。
`staging_complete` 后、`publish_intent` 前，对 Source 执行第二次 bounded verification pass，精确复验 normalized entry set、entry
type、filesystem identity、size、content SHA-256 与 executable bit。最终 Source digest 必须与 staging inventory digest 一致；任何
差异返回 `project_copy_source_changed`，清理 owned staging，不 publish。Plan 从不声称 full inventory 已冻结。

staging 固定采用同一 target parent 下的 sibling shell：

```text
.alcomd-copy-<opaque-operation-id>.staging/
    owner/recovery metadata
    inventory.jsonl
    payload/
```

publish 只把 `payload` 原子 rename/move 到 target leaf；shell 中的 ownership/recovery metadata 保留至 `state_committed` 后清理，
最终 Project root 不永久留下 ALCOMD marker。

### Copy profile v1

包含 ordinary project contents、`Library`/`Library*`、`Packages`、locked VPM packages、
`Packages/manifest.json`、`Packages/vpm-manifest.json` 与 ordinary hidden entries。排除 root `Logs`、`Obj`、`Temp`，以及
任意深度、大小写不敏感、任意 entry type 的 `.git`。

拒绝 symlink、junction、reparse point、hard-linked regular file、special file、non-UTF-8 name、absolute/device/UNC escape、
traversal、Unicode/case/file-directory collision、target inside source、source inside target、overwrite 与 merge。不联网、不
refresh repository、不 resolve package、不重写 manifest。

必须保留 regular-file bytes、directory hierarchy 和 Unix executable bits；后者只使用 safe
`std::os::unix::fs::PermissionsExt`。不承诺 timestamp、owner、ACL、xattr、creation time、平台 metadata 或完整 Unix mode。

Quota 固定为 500,000 entries、32 GiB single regular file、128 GiB total regular bytes、depth 128、normalized path
1,024 UTF-8 bytes。超限返回 `project_copy_limit_exceeded`，不得公开部分 target。

### Writer gate、locks、cancel 与 recovery

- `running_confirmed`：`unity_project_running` hard reject。
- `running_suspected` / `unknown`：advisory，允许 Operation，但强制 initial/final fingerprint。
- `not_observed`：允许，但不表示 Unity 确定未运行。所有状态都执行 final revalidation。
- 一次性取得 `Project(source ProjectId)` 与
  `ProjectCreate(target parent identity, normalized target leaf)`；去重后按现有
  `ResourceKey::canonical_bytes` 排序。相同 target 只有一个 Apply 继续。
- `accepted`、inventory、staging、`staging_complete` 与写入 `publish_intent` 前可取消；写入 intent 后只允许 forward
  recovery，最终为 succeeded、failed 或 recovery_required，不能 cancelled。
- 专用 `project_copy_filesystem_journal` 是 append-only，不复用 package/template/backup journal。
- `target_published` 后复验 owner marker、target identity/fingerprint 与 preallocated ProjectId。外部修改时保留 target，返回
  `project_copy_recovery_required`，不覆盖、不删除、不虚假 succeeded。
- recovery 复用 original PlanId、OperationId、idempotency、ProjectId、profile 与 inventory evidence。

publish 前不得创建 Project registry row。target 完整发布并复验后，在短 SQLite transaction 中提交 Project registry、
revision、Event、idempotency result、Operation 与 journal state。Projects list/grid 发起后留在列表并刷新；workspace 发起后
进入新 Project workspace；两者调用相同 Core Plan/Apply。

## State Schema v10 production

`specs/storage/state-v10.md`、`state-v10-migration.proposal.contract.json` 与 active
`crates/alcomd-store/migrations/0010_project_copy.sql` 已落盘；完整 Copy wiring 后 daemon 广告 `dataSchema: 10`。

v10 只为 Copy 增加 `project_copy_plans`、`project_copy_filesystem_journal` 与 `operations.kind='projects.copy'`。它复用
Project registry、Revision、Event、idempotency 与 Operation；Plan authority immutable/durable，journal append-only，带 recovery
index、source Project FK/identity、owner/idempotency binding，future schema fail closed。不把 Favorite、Package bulk 或配置偏好
顺手塞入 v10。

## Stable public errors

proposal 冻结：`project_copy_plan_not_found`、`project_copy_plan_stale`、`project_copy_target_exists`、
`project_copy_target_unsafe`、`project_copy_source_unsafe`、`project_copy_source_changed`、
`project_copy_limit_exceeded`、`project_copy_recovery_required`。继续复用 `project_not_registered`、
`revision_conflict`、`unity_project_running`、`idempotency_conflict`、`permission_denied` 与
`internal_error + diagnosticId`。禁止 `project_copy_failed` 一类宽泛错误。

## GUI Copy flow

```text
Project context -> Copy Project -> host-owned dialog(parent + name)
-> projects.planCopy -> Plan review(source/target/profile/writer/quota risk)
-> explicit Apply -> durable Operation progress -> completion
```

GUI 不计算 collision、identity、inventory、exclusion authority、copy size 或 fingerprint。

## 其他缺口与顺序

完整 ownership/contract matrix 位于 `docs/baselines/m7-project-package-functional-closure.md`。M7 ownership 与顺序冻结为：

1. P0：`open` + `tauri-plugin-dialog` dependency foundation；
2. P1：Open Directory + native chooser + Add/Register；
3. P2-P4：Copy active contract/State v10、filesystem Operation/recovery、client/CLI/GUI；
4. P5：Create/Restore/Favorite/Clear Unity preference closure proposal；
5. P6：package refresh/filter/prerelease/hidden/docs/changelog/Reinstall/Bulk/User Packages closure；
6. P7：Remove Directory 独立 high-impact Stop A；
7. P8：visible-action completeness gate；
8. M11：仅 VCC Import/Migrate、legacy migration 与真实 differential parity。

## Visible action completeness gate

fixture `visible-action-completeness-v1.json` 冻结每个用户可见业务 action 的状态分类：

- `implemented`：真实调用 typed client/RPC 或批准的 closed local adapter；
- `conditional-disabled`：只因可观察 current state 暂不可用；
- `unavailable-non-blocker`：backend capability 明确不存在，且不是承诺的 release blocker。

禁止 permanent-disabled fake action、empty handler、no-op success、local fake mutation 与 placeholder production button。已承诺的
v3 release-blocker 不能通过删除按钮或改成 `unavailable-non-blocker` 隐藏；metadata 必须保持 in_progress/blocked。P1/P4 已把
Open Directory、native Register chooser 与 Copy 标为真实 implemented；P5/P6 已完成真实入口，P7 Delete Project Directory
也已成为真实入口。完整 visible-action gate 仍因 M11 VCC Import/Migrate 缺口保持未通过。

## 验收与停止

P0-P4 必须执行 fmt、clippy、Workspace tests、npm check/build/browser tests、xtask、metadata、baseline freeze 与 diff check，
并证明没有第三个 production dependency、新 unsafe、新平台 API、额外 public RPC/Permission/State table 或 H2 视觉推进。
完成并提交后停止，等待项目所有者决定进入 P5 还是返回视觉 Gate。

## P0-P4 implementation progress

- P0：exact `open 5.4.2` 与 `tauri-plugin-dialog 2.7.2` 已以独立 dependency commit 落盘；没有 opener plugin、frontend dialog
  binding/capability、`open` insecure feature、新 ALCOMD unsafe 或直接平台 API。
- P1：Open Directory 只接收 ProjectId 并逐次 `projects.get` 复验；Add/Register 与 Copy 共用 Rust-side native directory picker。
- P2：`projects.copy.v1`、`projects.planCopy/applyCopy`、State v10 两表 migration、protocol/dispatcher/store/client 已接线。
- P3：private durable JSONL inventory、两遍 content SHA-256、sibling staging/atomic publish、forward recovery 与每个 durable phase
  的真实子进程 kill/restart matrix 已落盘；M2 generic recovery 已显式把 `projects.copy` 交给专属恢复器。
- P4：CLI Plan/Apply、Projects list/grid 与 Project workspace Copy flow 已接线；list/grid 完成后刷新，workspace 完成后进入新
  Project workspace。
- 2026-08-28 本地验收：`cargo fmt --all -- --check`、严格 Clippy、完整 locked Workspace tests、npm check/build、19 项
  Playwright browser tests、`cargo xtask check`、metadata、baseline freeze、`git diff --check` 与 Tauri release
  `--no-bundle` 全部通过。Copy 额外证明 15 分钟 expiry 边界、四种 writer state、同 target 并发单次发布、pre-intent
  cancel、post-intent 不可取消、外部 target 修改进入 recovery-required，以及全部八个 durable checkpoint 的真实
  kill/restart。没有新增第三个 production dependency、ALCOMD unsafe、平台 API、Permission、ResourceKey 或 v10 第三张表。
- 2026-08-29 P0-P4 remote checkpoint：commit `287790b3e3ef1855a6b7cea92c476b46873d2ca9` 的 Hosted CI run
  `33175602728` 在 Windows Server 2025、Ubuntu 22.04 与 macOS 15 arm64 全部成功，CodeQL run `33175602645`
  成功；项目所有者已正式通过该 checkpoint。
- P5-A Create / Restore 直接复用既有 `templates.list/get/planCreateProject/applyCreateProject`、
  `backups.list/get/planRestore/applyRestore`、native directory chooser、Operation 与 Projects registry。Projects toolbar
  提供 Create / Restore 入口；对话框覆盖来源选择、目标目录、名称、Plan 审阅、Apply、进度/取消、结构化失败、空备份、
  完成后刷新和进入 Core 返回 ProjectId 的 workspace。未新增 `projects.create`/`projects.restore`、RPC、State、Permission、
  dependency、unsafe 或平台 API；归档、冲突、writer gate 与恢复规则仍只由 Core 执行。新增 3 项 browser flow tests，完整
  22 项 Playwright suite、Rust fmt/strict Clippy/完整 locked Workspace tests、npm check/build、Tauri release no-bundle、xtask、
  metadata、baseline freeze 与 diff check 均已本地通过。commit `12638e7bdc5ba10ea9e9438b8dbc209a3993cafc` 的 Hosted CI
  run `33189137494` 已在 Windows Server 2025、Ubuntu 22.04 与 macOS 15 arm64 全部成功，CodeQL run `33189136465`
  四个分析 job 全部成功；Ubuntu最高 `GLIBC_2.34`，macOS 9 个预期产物均为 arm64 / minos 11.0。
- P5-B 精确审计确认：v3 Favorite 是持久 Project metadata，新项目默认 false、restart保留、注销后同路径重新注册不保留，
  且在所有 selected sort 之前做 favorite-first stable partition；没有 favorite-only filter。active State v11 已实现
  `projects.favorite`、兼容可选 registered DTO字段及 `projects.setFavorite`，复用 `projects.manage` 与既有
  `projects.registry.v1` capability；Project revision/Event/idempotency、refresh保留、完整分页后稳定 favorite-first 及
  list/grid toggle 均已有生产和自动化证据。
- v3 Clear Unity path只移除 explicit path、保留 custom arguments并恢复 automatic version matching。State v11 已以 existing
  table rebuild 实现 tagged `unity.projectEditor.selection.get`、窄 `unity.projectEditor.clear`、legacy get/set compatibility、
  automatic 0/1/multiple resolution 与独立 launch fingerprint v2；explicit fingerprint v1保持不变。`0011` migration、
  Store/Application/RPC/typed client/Tauri/official GUI、迁移与浏览器回归已完成，本地完整门禁通过；没有新 Permission、
  capability、table、dependency、unsafe、平台 API 或 Operation kind。
- P5-B 最终远端候选 `1fc139b8fd47572d52e8cf468a247195e61091f4` 的 Hosted CI run `33209167919` 已在
  Windows Server 2025、Ubuntu 22.04 与 macOS 15 arm64 全部成功，CodeQL run `33209165247` 的 Python、
  JavaScript/TypeScript、Rust 与 Actions 分析全部成功；Ubuntu 实测最高 `GLIBC_2.34`，macOS 9 个预期产物均为
  arm64 / minos 11.0。该候选包含仅使用 Material Web 官方 spacing token 的跨平台 Project action density 修复；
  三个平台的 26 项 Official GUI Playwright suite 均通过。
- P6-A production 只复用 `repositories.list`、`repositories.refresh`、`repositories.get`、
  `repositories.packages` 与现有 typed client：完整分页读取 registered repositories，按顺序 refresh，逐项记录 success/failure，
  `revision_conflict` 重新读取当前 revision 后最多重试一次，并在 partial failure 后仍 reload package data。Source filter 只消费
  daemon `RepositorySource.kind` 并提供 All/Remote/Local，不持久化且不参与 resolver/Plan/Apply/source pin。新增 browser test 覆盖
  zero/one/multiple、partial failure、single retry、final reload 和三种 source presentation；没有新增 RPC、Capability、Permission、
  State、Config、dependency、unsafe 或平台 API。最终 hotfix commit `bb934ef94ad57f26cb40a6d718fc46181a4915f4`
  只修改 `crates/alcomd-testing/tests/m7_project_copy_rpc.rs`，使 test daemon guard 的 shutdown/drop 顺序和 client
  connect timeout 与 production 5 秒启动窗口不再相互竞速；production timeout、IPC、RPC 与 daemon 均未修改。
  Hosted CI run `33264281642` 的 Windows Server 2025、Ubuntu 22.04、macOS 15 arm64 全部成功，CodeQL run
  `33264281469` 的 Actions、Rust、JavaScript/TypeScript 与 Python 分析全部成功。

## P6-B Stop A：package visibility / metadata

本节记录已获批准并已实施的 contract-first 基线；Config Schema 2、State Schema 12、兼容 RPC/DTO 与 production wiring
均严格按本节及 machine-readable proposal 落盘。

### Core evidence 与 prerelease DTO

- `alcomd-vpm` 已用唯一获批的 `semver 1.0.28` 解析 resolver-ready package；Core request 已有
  `includePrerelease`。当前 public `RepositoryPackageVersion` 只有原始 `version`，React 无权自行用 split/regex
  分类 prerelease。
- 提案对 read DTO 兼容增加 `prerelease?: boolean`。严格 `semver::Version` 解析成功时，Core 根据
  `Version.pre` 是否为空返回 `true`/`false`；legacy/raw/unparseable version 返回字段 absent，语义固定为
  `classification_unavailable`，不得当作 `false`。
- 未知分类不得进入新的默认可选候选；若它已经被项目 exact lock 安装，仍显示该 installed row，并标记
  classification/source unavailable。旧 durable snapshot 不会因为缺字段被误归类为 stable。

### Config Schema 2

规范 TOML 提案为：

```toml
schema = 2
revision = 1
locale = "system"

[appearance]
mode = "system"
source_color = "#6750A4" # 仅非空时出现
density = "default"
motion = "system"

[packages]
show_prerelease = false
hidden_repository_ids = []
hide_local_user_packages = false
```

公开 `settings.get` normalized JSON 在原 `settings` 对象中兼容增加：

```json
{
    "packages": {
        "showPrerelease": false,
        "hiddenRepositoryIds": [],
        "hideLocalUserPackages": false
    }
}
```

`settings.update.update.packages` 是 closed partial object；`showPrerelease`、`hiddenRepositoryIds` 与
`hideLocalUserPackages` 均可独立省略，
省略保持当前值。`hiddenRepositoryIds` 是整个集合的 CAS replacement，不提供 add/remove patch DSL。

- `showPrerelease` 默认 `false`，与 v3 serde default 和 v4 当前安全行为一致。
- `hideLocalUserPackages` 默认 `false`，且只控制 official GUI 的 User Package source/section presentation。
- `hiddenRepositoryIds` 最多 256 个 canonical lowercase UUID；空数组清空。输入重复、非 canonical UUID 或超限
  均拒绝，daemon 不静默去重。规范写出按 UUID UTF-8 byte order 升序。
- 未注册/stale ID 保留但 inactive；unregister 不改设置。相同 source 后续重新注册取得新 RepositoryId，因此默认
  可见，旧 stale ID 不自动绑定新 authority。
- v1 -> v2 仅在 daemon startup、对外 ready 前执行：验证完整 v1 后补 `packages` 默认值，保留现有 revision，
  使用既有 same-directory synced temp/backup/recovery 原子替换。此迁移不是用户 mutation，不增加 revision；之后每次
  成功 `settings.update` 仍把 revision 精确加一。
- migration 失败保留或恢复原合法 v1，daemon 对 settings RPC fail closed；不得用默认值覆盖。v2 未知字段/section、
  错误类型、重复 ID 与 future schema 均 fail closed。`settings.get/update` 方法、permission 和 CAS 错误不变；hello
  只在 v2 durable/queryable 后把兼容可选 `configSchema` 从 1 广告为 2。

### Hidden presentation policy

v3 审计证明 hidden 不是一个布尔业务属性：`gui_hidden_repositories`、`hide_local_user_packages`、prerelease、yanked、
unavailable source、Unity incompatible 与折叠 section 是不同原因。P6-B 因此冻结分离原因：

- `hidden_repository`：repository 在 Config v2 列表；official GUI 隐藏该 repository/source，并不改变 Core catalog。
- `prerelease_preference`：`showPrerelease=false` 时从默认可选候选中排除。
- `yanked`：保持现有 Core policy，绝不因“显示隐藏项”而可被新选择；installed exact row 仅显示警告。
- `source_unavailable` / `classification_unavailable`：不可新选择；installed exact row 保留并显示原因。
- `unity_incompatible`：仍是独立 compatibility reason，不与 hidden 合并。
- User Package：P6-C 的独立 source kind/section，不冒充 local repository 或 hidden repository。
- folded/collapsed section：纯瞬时 UI 状态，不进入 Config v2。

隐藏只影响 official GUI presentation 和用户主动选择 source 的 UI，不改变 Core resolver catalog、dependency resolution、
repository refresh、已创建 Plan、pinned source、Apply 或 recovery。用户选择可见 repository 后，GUI把该 RepositoryId 作为
existing source selector显式传入；不得通过隐藏配置隐式改变authority。`hideLocalUserPackages` 同样不注销source、不删除cache、
不使Plan失效。public DTO 应返回可组合 reason，不增加 `hidden: bool`。

### Docs / Changelog 与 official GUI closed opener

v3 package manifest 的真实字段是 `documentationUrl` 和 `changelogUrl`；P6-B 不把 homepage/repository/任意 manifest
字段扩成 generic links。Core parse 时复用已有 `reqwest::Url`：原始 UTF-8 最多 2,048 bytes，必须是 absolute
`http`/`https`、有 host、无 username/password userinfo；malformed/unsupported 值作为脱敏 read issue 丢弃。query 与
fragment 可以保留。提案兼容增加：

```json
{
    "links": {
        "documentation": {"url": "https://example.invalid/docs"},
        "changelog": {"url": "https://example.invalid/changelog"}
    }
}
```

`links` 及两个 member 均 optional；State v12 为 repository package row 增加 nullable bounded
`documentation_url` / `changelog_url`，以便 restart/HTTP 304 后仍保留已验证 authority。旧 row 为 NULL。

Official GUI 新 closed Tauri command 只接收 `(repositoryId, packageId, version, linkKind)`，其中 `linkKind` 仅
`documentation|changelog`。Rust adapter 每次通过 typed client 重新读取 current repository package metadata，按完整
三元组找到当前 descriptor，再用现有 `open::that` 打开。React 不传 URL；不存在 generic `openUrl(string)`，也不增加
Extension capability。第三方 GUI 可消费 Core 已验证的 public descriptor并自行选择 opener。

P6-B 不需要新 public RPC method、capability、permission、production dependency、unsafe 或平台 API；只提议 existing read
DTO/settings DTO 的兼容字段、Config Schema 2 以及 State v12 的两个 nullable cache columns。

## P6-C Stop A：package mutation / User Packages

本节记录已获批准并已实施的 contract-first 基线；method、migration、resolver、cache 与 official GUI 均已按冻结边界接线。

### Reinstall

当前 `packages.planInstall` 经过 `build_resolution_plan` 后，目标 exact version 与 installed lock 相同时可以得到空
ChangeSet，因此不能冒充 reinstall；remove+install 或多个 Plan 也不具备单事务语义。

提案新增 compatible RPC `packages.planReinstall`：

```json
{
    "projectId": "uuid",
    "expectedRevision": 1,
    "selection": {"kind": "packages", "packageIds": ["com.example.package"]}
}
```

或 `{"selection":{"kind":"all"}}`。

- explicit list 为 1..256 个现有 package ID，必须 unique；Core 以 package ID UTF-8 byte order 规范排序后参与 fingerprint。
  空数组不表示 all。`all` 由 Core 从 authoritative vpm lock 扩展为全部 locked direct+transitive package，排序后仍不得
  超过 256，超限失败。
- 允许显式 repair direct 或 transitive package；任一 ID 未安装/未 locked 时整个 Plan 返回
  `package_not_installed`。`all` 在没有 installed package 时返回空的成功 Plan，不制造 Apply Operation。
- Reinstall 固定 current locked exact version，但当前 installed state 不包含 original source/digest provenance，不能声称恢复
  原来源。Core只寻找 exact version candidate；有多个authority且没有selector时返回 `package_source_ambiguous`，显式
  repository/user-package selector必须确实提供该exact version。成功生成 `replace` mutation，不升级/降级、不改变
  dependency graph或VPM manifest语义，并优先逐字节保留当前`vpm-manifest.json`。
- Plan 阶段沿用 `projects.read + repositories.read + packages.read`，Apply仍使用现有
  `packages.applyPlan` + `packages.manage`、一个 durable Operation 与同一 filesystem recovery。
- 新方法要求 `packages.plan.v2`；`packages.plan.v1` 完整冻结且继续只使用repository catalog。`packages.apply.v1`与
  `packages.applyPlan`不变。State v12 的 `package_plans.action` 增加`reinstall`；ChangeSet/source set/fingerprint
  保持durable。selection超限复用`plan_too_large`和safe subreason=`selection_limit`。

### Bulk

提案新增 `packages.planBulk`，要求 `packages.plan.v2`，输入 1..256 个 closed typed intents：

```json
{
    "projectId": "uuid",
    "expectedRevision": 1,
    "intents": [
        {"kind": "install", "packageId": "a", "versionRange": "^1.0.0", "repositoryId": "uuid", "includePrerelease": false},
        {"kind": "upgrade", "packageId": "b", "versionRange": "^2.0.0", "includePrerelease": false},
        {"kind": "remove", "packageId": "c"},
        {"kind": "reinstall", "packageId": "d"}
    ]
}
```

只支持当前 install/upgrade/remove/reinstall intent；不加入表达式语言、nested batch、resolve/downgrade alias。每个 package
最多出现一次，重复或互斥 intent 返回 `package_intent_conflict`。Core 验证并按 `(packageId, kind, normalized fields)`
规范排序，将 normalized intent array、project revision与resolver inputs纳入 Plan request fingerprint；caller 顺序不影响结果。

Core 在一个 resolver invocation 中处理完整 intent set。依赖/版本/source冲突使整个 Plan失败，不产出部分 Plan。成功只产生
一个 immutable durable PackagePlan（action=`bulk`），随后复用一次 `packages.applyPlan`、一个 `packages.apply`
Operation、现有 Resource Lock、transaction与recovery；React不得拼接多个 Plan/Apply。

### User Packages / loose local packages

v3 `settings.json.userPackageFolders` 与 `folder/package.json` 证明该能力是 loose local package directory enrollment，
不是 local VPM repository，也不接受 archive。P6-C 提案只支持 directory：

- RPC compatible additions：`packages.userPackages.list/get/enroll/refresh/remove`，capability
  `packages.user-packages.v1`。list/get使用 `packages.read`；enroll/refresh/remove使用 `packages.manage` 与既有
  `builtin:local-owner` filesystem authority。不提议新 Permission。
- enroll input 是 daemon重新验证的 absolute UTF-8 directory path（official GUI picker仅提供候选）。root及每个 component
  不得是 symlink/reparse/junction；tree只允许 directory/regular file，拒绝 non-UTF-8、escape、collision、special file及
  multi-hard-link regular file。复用现有 file identity/path normalization和package archive validator。exact quotas为65,536 entries、
  1 GiB single regular file、4 GiB total、depth 64、normalized path 1,024 UTF-8 bytes、final archive不超过1 GiB。
- authoritative manifest 只来自 root `package.json`，使用现有 strict VPM manifest + `semver::Version` parser。enrollment
  identity 是 daemon分配 `UserPackageId`；root opaque filesystem identity阻止路径字符串重绑。每个 owner的
  `packageId` 必须唯一；重复identity返回`user_package_already_enrolled`。同一UserPackageId refresh可改变version但不得改变
  packageId，否则`user_package_source_changed`。
- enroll/refresh 完整验证后，把 normalized tree打包为 deterministic、immutable ALCOMD-owned cache archive，记录 SHA-256
  archive digest、manifest fingerprint与normalized tree fingerprint。source directory从不成为Apply时的可变输入；resolver
  pin为 `(UserPackageId, sourceRevision, packageId, version, archiveSha256)`，M4下载/完整性/ZIP/path/transaction/recovery
  继续处理该本地cache object。这样离线Apply可用且不建立第二套package filesystem engine。
- refresh只针对同一 UserPackageId/path/root identity；重读发生变化的tree后以CAS发布新 sourceRevision和新immutable cache
  snapshot。路径丢失、identity改变或安全验证失败时，source标为`unavailable`，不静默重绑。已创建Plan继续引用原cache
  snapshot；新Plan不得选择unavailable source。
- remove只注销 enrollment并增加 revision/Event；绝不删除用户目录。已创建Plan/运行中Operation与其pinned cache仍可恢复，
  后续GC只能在没有durable reference时按既有cache policy处理。

### State Schema 12 proposal

State v11保持sealed。State v12只做三组有独立理由的变化：

1. rebuild `package_plans`，保留全部列/trigger/index/data，仅把 action CHECK扩为
   `install|remove|upgrade|downgrade|resolve|reinstall|bulk`；
2. `repository_package_versions` 增加 nullable `documentation_url`、`changelog_url`，每项 1..2,048 UTF-8 bytes；
3. 新建专用 `user_package_sources`，不是generic source registry：

| column | contract |
|---|---|
| `user_package_id` | canonical UUID primary key |
| `owner_principal_id` | 1..128；参与所有查询与mutation |
| `source_root_path` | absolute normalized UTF-8，1..32,768；不得进入普通日志/Event |
| `source_identity_key` | opaque BLOB，1..1,024；不暴露平台类型 |
| `package_id` | current package ID grammar，1..128 |
| `version` | strict canonical SemVer，1..1,024 |
| `manifest_json` | normalized bounded manifest authority；不要求upstream url/zipSHA256 |
| `manifest_fingerprint`, `content_fingerprint` | exact 32-byte SHA-256 BLOB |
| `archive_sha256` | 64 lower-hex；指向ALCOMD-owned immutable cache object |
| `revision` | positive monotonic integer |
| `created_at_ms`, `updated_at_ms` | bounded nonnegative integer |

约束为 `UNIQUE(owner_principal_id, source_identity_key)` 和
`UNIQUE(owner_principal_id, package_id)`；refresh只允许相同root object/packageId的checked mutation，remove通过RPC显式
删除registry row但不得级联删除被Plan引用的cache evidence。迁移复制现有rows后重建trigger/index，transaction失败完整回滚，
future schema fail closed。没有generic workflow/state table。

### Public errors、dependencies 与 test vectors

批准新增稳定错误：`package_not_installed`、`package_intent_conflict`、`user_package_not_found`、
`user_package_already_enrolled`、`user_package_source_unavailable`、`user_package_source_unsafe`、
`user_package_source_changed`、`user_package_manifest_invalid`、`user_package_limit_exceeded`。selection超限复用
`plan_too_large`/`selection_limit`；manifest/range/hash/cache/archive/path错误继续复用现有精确package error，不增加宽泛
`user_package_failed`。

提案不需要新production dependency、unsafe或平台API：SemVer、SHA-256、ZIP、path/file identity、cache与local opener均复用
现有边界。production前永久contract/test vectors至少覆盖：same-version install no-op证明；reinstall one/direct/transitive/all/
empty-all/missing/limit/order/fingerprint/pinned-source/offline cache；bulk四kind、duplicate/conflict、resolver atomic failure、single
Plan/Operation/idempotency/order；State 11->12 copy/rollback/future fail-closed；User Package ordinary enroll、same identity、duplicate
id/version、rename/delete/recreate、symlink/reparse/hardlink/non-UTF8/collision/quota、refresh CAS、unavailable/remove、cache corruption、
offline Apply、existing Plan after refresh/remove，以及三平台filesystem identity行为。

## P6 approved production boundary

项目所有者已批准按本节及machine-readable proposal实施Config Schema 2、State Schema 12、`packages.plan.v2`、Reinstall、
Bulk、User Package registry/cache/resolver与official GUI closure；不新增Permission、dependency、unsafe/platform API、
ResourceKey或Operation kind。该批准不包含P7；P8、H2、M8、M9与M11仍未开始。

## P6-B/P6-C production progress

- Config Schema 2 已实现 v1 原 revision 迁移、CAS 更新、默认关闭 prerelease/local hiding、canonical hidden RepositoryId 集合、
  256/257 边界与 16 KiB 上限；official GUI 只把这些设置用于 presentation，Core resolver authority 不变。
- State Schema 12 已原子重建 `package_plans` action CHECK、持久化 nullable documentation/changelog URL，并加入唯一的
  `user_package_sources` business table；迁移链、回滚与 future-schema fail-closed 测试已接入。
- `packages.plan.v2`、Reinstall、Bulk 与 User Package RPC/client/CLI/official GUI 已接线。Reinstall 固定 locked exact version并生成
  Replace；Bulk 在一个 deterministic resolver invocation 中生成一个 durable Plan，并继续只产生一个 `packages.apply` Operation。
- User Package enrollment/refresh 会对 loose directory 做 bounded fail-closed scan并生成 deterministic ALCOMD-owned cache ZIP；
  Plan 只固定 UserPackageId/revision/identity/manifest/archive digest。源目录在 Plan 后变化或 registry remove 不改变 Plan；owned
  cache 丢失时返回 `offline_cache_miss`，不得回读 mutable source。
- Windows 本地已通过 nested/root reparse、hard-link与deterministic archive测试；Rust P6 targeted suites、TypeScript check、Vite
  production build与32项Chromium Playwright均通过。
- P6 最终候选 `c7477089571246376ab4f90d106b860d9a8e98cd` 的 Hosted CI run `33277180223` 已在
  Windows Server 2025、Ubuntu 22.04 与 macOS 15 arm64 全部成功，CodeQL run `33277179920` 的四类分析全部成功；
  Ubuntu最高 `GLIBC_2.34`，macOS 9 个预期产物均为 arm64 / minos 11.0，三平台 Official GUI 32 项 Playwright suite
  均通过。最终 test-only hotfix 为慢速 Windows runner 提供 120 秒有界 subprocess startup deadline 与提前退出诊断，
  没有修改 production recovery、IPC 或 socket 合同。P6-A、P6-B、P6-C 均为 PASS。
- 未新增 production dependency、Permission、ResourceKey、Operation kind、unsafe 文件或平台 API；P7当时尚未开始，
  P8/H2/M8/M9/M11继续停止。

## P7：Delete Project Directory production candidate

P7 的精确审计、策略比较、合同与测试矩阵位于：

- `docs/baselines/m7-p7-project-directory-delete.md`；
- `specs/rpc/m7-project-delete.proposal.schema.json`；
- `specs/storage/state-v13-project-delete.proposal.contract.json`；
- `specs/security/m7-project-delete-path-vectors.json`。

该切片已按项目所有者批准合同进入 production，并保持以下冻结边界：

- v3 的 `Remove Project` 是单一对话框，提供 `Remove from the List` 与 `Remove the Directory` 两个结果；目录删除先调用
  OS Trash，再移除 registry。v4 保持现有 `projects.unregister` 独立，不改变它的仅移出列表语义。
- 提案新增 capability `projects.delete.v1`、`projects.planDeleteDirectory` / `projects.applyDeleteDirectory`、Operation
  `projects.delete-directory` 与 deny-by-default `projects.delete`。调用只接受 `ProjectId`，filesystem writer 仅允许
  `builtin:local-owner`；该 Permission 在 4.0.0 不授予 extension。
- 选择 sibling quarantine + permanent delete，不使用 OS Trash，也不直接就地递归删除。Apply 必须重新验证 project revision、
  root/parent identity、leaf、ProjectVersion marker、protected roots 与 fresh writer evidence；只有 `not_observed` 可继续，且不
  声称 Unity 确定未运行。
- `quarantine_intent` 是 cancel 与 forward-only recovery 边界。root rename 到 sibling owned quarantine 并复验 identity 后，
  recovery 永远不再触碰 original path；这样原路径被外部重建时也必须存活。
- State v13 只新增 `project_delete_plans` 与 append-only `project_delete_filesystem_journal`，并精确重建此前
  仍以 `projects(project_id)` 为外键的 durable history 表，使 Plan/Journal/Operation/幂等证据可在 registry row 删除后继续存在；
  不新增 generic deleter、workflow、tombstone framework 或 recursive inventory 表。
- Windows继续使用pinned Rust std的no-follow删除；Linux使用已批准的safe rustix `openat2`/`NO_XDEV`，macOS使用fd-relative
  traversal和`st_dev + f_mntonname` mount token。guard不可用或遇到mount boundary均fail closed；没有新增dependency、unsafe
  或Windows API。
- 单一成功 Event 是 `project.directory_deleted`；不得同时写 `project.unregistered`。GUI 必须清楚区分“Remove from list”和
  “Delete Project Directory”，展示永久删除、无 Trash、无自动备份、完整目标路径与 writer 状态，并要求输入 exact project
  leaf 解锁 Apply；输入确认不是后端 authority。

P7 已实现 active RPC/Permission、State v13 migration、Linux `openat2` `NO_XDEV`/macOS mount token/Windows std no-follow、
sibling quarantine、durable forward recovery、CLI/GUI入口及真实 subprocess kill/restart矩阵。sealed HEAD
`2ee11066c07a0994f3aebe6a9ce3f84ab2c8acd9` 已通过 Hosted CI run `33298022030` 的 Windows、Ubuntu、macOS
以及 CodeQL run `33298021806`，P7 remote checkpoint 为 PASS。P8 只审计并关闭 M7-owned 可见 action；M11 的
VCC Import/Migrate、legacy entry 与真实 v3 differential parity 继续作为全局 release blocker，不计入 M7-owned 分母。

## P8 visible-action completeness

唯一机器可读 inventory 是
`crates/alcomd-testing/fixtures/m7/visible-action-completeness-v1.json`。它逐项区分 owner milestone、当前分类和全局
release-blocker 状态，不再把 M11 缺口混入 M7-owned 分母。P8 gate 固定输出：

- `m7OwnedCompleteness = PASS`：M7 official GUI 当前可见 Project、Repository、Package、User Package、Template、Unity、
  Backup、Operation、Extension、Settings、Activity 与 Diagnostics action 均有真实 typed client/closed adapter 实现，或按实际
  daemon capability/current state 明确禁用；
- `globalReleaseCompleteness = BLOCKED_BY_M11`：VCC Import、VCC Migrate、legacy migration entry 与真实 v3 differential
  parity 继续是 release blocker，且没有 fake/placeholder GUI 入口；
- capability 来源是 `system.status.capabilities`。GUI 不把 client 请求集合当成已授权事实；缺失 `projects.delete.v1`、
  `projects.copy.v1`、`packages.plan.v2`、`packages.user-packages.v1` 或其他 M7 capability 时不得发起对应 RPC；
- GUI 仍只使用 `GuiRpcClient` 的 closed typed method，以及 native directory picker、registered Project directory opener 和经过
  daemon 重新验证的 package link opener。不存在 generic RPC method/string invoke、generic path/URL opener 或 direct state/filesystem mutation；
- Delete Project Directory 与 Remove from list 保持两个独立 action；exact typed confirmation 可在 Apply 前 Escape，Apply 后由
  durable Operation/recovery 权威推进，不提供前端伪取消。

聚合 `projects.management`、`packages.vpm` 与 `repositories.management` 继续保持 `in_progress`：这些 feature 的冻结
user-entry 仍包含 M11 differential parity、M8 MCP 或其他后续里程碑范围。P8 PASS 只表示 M7-owned official GUI action
完整性，不把完整产品/发行状态提升为 implemented，也不恢复 H2 visual。

2026-08-30 本地 P8 候选已核对唯一清单中的 90 个 action：86 个 M7-owned action 为 33 个 `implemented` 与 53 个
`conditional-disabled`，4 个 M11-owned action 保持 `blocked-future-milestone`，永久 fake 数为 0。GUI 现在从
`system.status.capabilities` 消费实际协商结果，页面和 action 在 capability 缺失时不调用受保护 method；已有公开 M6 capability
常量经 typed SDK 复用，未增加 RPC、Permission、State、dependency、unsafe 或平台 API。Rust 完整 locked Workspace tests、
strict Clippy、34 项 Playwright、npm check/build 与 Tauri release no-bundle 已通过；最终提交仍需取得三平台 Hosted CI 与 CodeQL。
