# M7 Project / Package Functional Closure：contract-first Stop A

状态：Stop A 合同修正与 P0-P4 production implementation 已通过本地、三平台 Hosted CI、CodeQL 和项目所有者验收；
P5-A Create / Restore existing-Core GUI wiring 已形成仅使用既有 typed client、Plan / Apply / Operation 合同的本地候选并
通过完整本地 gate，等待该候选自身 Hosted CI。H2 visual WIP 继续暂停，P5-B 只允许 contract-first Stop A，P6-P8、M8/M9 未开始。

## 目标与边界

本 Stop A 把 v3 已有、但当前 M7 official GUI 尚未形成真实用户入口的 Project / Package 行为变成可审阅的最小合同。
P0-P4 已按批准范围落盘 dependency foundation、closed GUI affordance、active Copy RPC/State v10、filesystem
Operation/recovery 与 client/CLI/GUI flow。下列内容仍明确不在本轮：Favorite/Remove Directory/Package 新合同、H2 视觉推进、
M8/M9、任何新 unsafe 或额外平台 API。

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
Open Directory、native Register chooser 与 Copy 标为真实 implemented；完整 visible-action gate 仍因 P5-P8 已知缺口保持未通过。

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
  metadata、baseline freeze 与 diff check 均已本地通过；Hosted CI 证据待本候选提交后取得。
