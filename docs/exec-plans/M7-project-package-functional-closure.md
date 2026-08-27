# M7 Project / Package Functional Closure：contract-first Stop A

状态：Stop A 已由项目所有者附合同修正批准；允许依次实施 P0-P4，完成完整本地 gate 后停止。H2 visual WIP 已以独立、
未验收 checkpoint 保存且继续暂停，M8/M9 未开始。

## 目标与边界

本 Stop A 把 v3 已有、但当前 M7 official GUI 尚未形成真实用户入口的 Project / Package 行为变成可审阅的最小合同。
本轮只允许 proposal、dependency/platform probe、Schema/migration proposal、fixture、contract test 和 milestone ownership。
下列内容明确不在本轮：生产 Rust/TypeScript、active RPC、production migration、Cargo/npm manifest、lockfile、unsafe、平台
API、H2 视觉推进、M8/M9。

当前两个可见永久假入口是 `Open Project Directory` 与 `Copy Project`，但这不等于全部功能缺口只有两个。
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

## Copy Project public proposal

proposal 文件为 `specs/rpc/m7-project-copy.proposal.schema.json`；它不是 active RPC publication。

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

## State Schema v10 proposal

`specs/storage/state-v10.md` 与 `state-v10-migration.proposal.contract.json` 已获 production 批准；在完整 Copy wiring 落盘前仍不
存在 active `0010` SQL，daemon 继续广告 `dataSchema: 9`。完整接线后才广告 `10`。

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
v3 release-blocker 不能通过删除按钮或改成 `unavailable-non-blocker` 隐藏；metadata 必须保持 in_progress/blocked。Stop A test
只验证 inventory 与 gate proposal 自洽，并明确记录当前两个 known fake entries；production gate 要等 P1/P2 activation 后才能改为
implemented，当前不能虚构通过。

## 验收与停止

Stop A 必须执行 fmt、clippy、Workspace tests、xtask、metadata、baseline freeze 与 diff check，并证明 active RPC、production
Rust/TypeScript、manifests、locks、unsafe、platform API 和现有 H2 WIP 未变。完成后停止，等待 dependency、Copy/State v10 与
后续 slices 的人工审批。
