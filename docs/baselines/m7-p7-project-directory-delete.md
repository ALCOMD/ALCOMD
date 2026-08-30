# M7 P7 Project Directory Delete：contract-first Stop A

状态：`proposal-only`、`owner-approval-required`。本文件只冻结审计结论与候选合同；P7 production、
State v13 migration、active RPC/Permission、Operation worker、CLI/GUI handler 均未开始。

## v3 exact behavior

行为基线固定为只读仓库 `../ALCOMD3-v3-readonly` 的 commit
`4aa98ae4f18d42c10137278997180dbede991e88`。v3 只作行为参考，不复制、移植或改写其实现。

| 问题 | 冻结事实 | 源码证据 |
|---|---|---|
| UI label | 列表/网格菜单入口是 `Remove Project`；同一个 dialog 内有 `Remove from the List` 与 `Remove the Directory` 两个按钮。 | `vrc-get-gui/locales/en.json5:221-222,304-310`；`components/RemoveProjectDialog.tsx:80-126` |
| 两个入口 | 不是两个顶层菜单项，而是一个 `Remove Project` 菜单项打开 dialog 后提供两个明确动作。 | `app/_main/projects/-project-row.tsx:244-270`；`-project-grid-item.tsx:90-121` |
| 自动 unregister | directory move-to-trash 成功后才删除 registry record并保存；因此成功会同时 unregister。 | `src/commands/environment/projects.rs:1188-1205` |
| permanent / Trash | 使用 `trash 5.2.6` 的 `trash::delete`，即 OS Trash / Recycle Bin，不是 v3 自己的永久递归删除。 | `Cargo.lock:6101-6117`；`src/utils.rs:484-489`；`projects.rs:1155-1173,1192` |
| confirmation | 有一个 dialog。正文只说明将移除名为 leaf 的 project 并询问是否确定；风险主要由两个按钮文案区分。 | `RemoveProjectDialog.tsx:82-103,105-126` |
| 风险信息 | 初始 dialog 不说明 Trash、可恢复性、自动 unregister、无自动 Backup、完整路径、文件数量或 Unity writer 状态。directory 失败后才显示“关闭 Unity 和 Unity Hub 后重试”。 | `locales/en.json5:304-310`；`RemoveProjectDialog.tsx:86-102` |
| 二次确认 | 无 typed-name、checkbox 或第二个 dialog；一次 dialog 内点击 destructive button 即调用后端。 | `RemoveProjectDialog.tsx:105-126` |
| Unity running | 没有 writer preflight。按钮只因 directory missing 或 mutation pending 被禁用；打开的 Unity/Hub 通过 Trash 调用失败后映射成提示。 | `RemoveProjectDialog.tsx:112-125`；`projects.rs:1192-1197` |
| directory missing | GUI 的 directory action 在 `is_exists=false` 时禁用，unregister 仍可用。若绕过 GUI 调用 directory=true，Trash 失败且 registry 保留。 | `RemoveProjectDialog.tsx:112-125`；`projects.rs:1188-1203` |
| deletion failure | Trash 返回错误时函数在 registry removal 前返回，registry 保持。 | `projects.rs:1192-1203` |
| partial deletion | v3 没有 journal、identity-bound recovery 或 rollback；若第三方 Trash 实现已产生部分副作用后报错，v3 只保留 registry 并返回错误。 | `projects.rs:1188-1203`；`utils.rs:484-489` |
| automatic Backup | 无。该 command 没有调用 backup 路径。 | `projects.rs:1143-1211` |
| unregistered path | 不允许；后端先用 caller path 查找已注册 project，未找到即失败。 | `projects.rs:1181-1186` |
| symlink/junction | v3 command 没有自己的 root/nested symlink、junction、reparse、hardlink 或特殊文件检查，直接把已注册 path 交给 `trash::delete`。 | `projects.rs:1184-1197`；`utils.rs:484-489` |

因此 v4 保留 v3 的两个用户语义，但不复刻 v3 的路径入参、无 writer gate、第三方 Trash 黑盒或无恢复行为：

- `projects.unregister` 永久保持 **Remove from list only**；
- P7 建立独立 **Delete Project Directory** Plan/Apply workflow。

## 当前 v4 可复用能力

| 能力 | 可复用结论 | 证据 / 缺口 |
|---|---|---|
| `projects.get` | 复用 ProjectId -> owner-scoped authoritative record。 | `crates/alcomd-application/src/lib.rs:1169-1176` |
| `projects.unregister` | 只复用“不删 filesystem”的既有语义与永久幂等模式；P7 不调用它拼装删除。 | `crates/alcomd-application/src/lib.rs:1248-1259`；`crates/alcomd-store/src/m3.rs:522-578` |
| Project snapshot / identity | durable `ProjectRecord` 有 revision、canonical root 与 opaque `path_identity_key`；public `ProjectSnapshot` 不暴露 identity，符合 daemon-owned authority。 | `crates/alcomd-application/src/lib.rs:126-151`；`crates/alcomd-protocol/src/lib.rs:1070-1090` |
| Project revision | 复用 exact `Revision` CAS；Plan/Apply 都要重新验证。 | 同上；`crates/alcomd-store/src/m3.rs:543-547` |
| Unity writer | 已有 confirmed/suspected/not_observed/unknown 四态及不含 PID/argv/path 的 safe evidence。 | `crates/alcomd-application/src/m5.rs:106-132,683-744` |
| Resource locks | `Project(ProjectId)` 能串行 registry/project mutation；`ProjectCreate(parent identity hash, leaf)` 能串行同 child path 的 absent/present 变化。两者按 canonical bytes 排序，足够表达 P7。 | `crates/alcomd-domain/src/lib.rs:634-699` |
| Project Copy | 复用 immutable 15-minute Plan、永久 idempotency、两把锁、append-only workflow journal、intent 后 forward recovery 和 private locator 模式；不复用 Copy inventory/profile。 | `specs/storage/state-v10.md`；`crates/alcomd-vpm/src/project_copy.rs` |
| Backup / Restore | 复用 sibling staging、atomic publish、phase journal、kill/restart 与 identity revalidation的设计模式；不建立第二套通用 workflow engine。 | `specs/storage/state-v6.md`、`state-v7.md`；`crates/alcomd-vpm/src/backup.rs` |
| directory identity | Windows 为 volume serial + 128-bit file ID；Unix 为 device + inode；`resolve_directory_identity` 返回 canonical directory + opaque identity。 | `crates/alcomd-platform/src/windows_file_identity.rs:19-75`；`unix.rs:17-23`；`lib.rs:40-63` |
| link count | Windows 现有 `file_link_count` 可读 hard-link count，但 P7 删除单个 pathname不需要据此拒绝 hardlink；不得把该 helper扩成删除 API。 | `windows_file_identity.rs:77-100` |
| path / link validation | 现有 VPM/Copy/Backup 有 bounded normalization 与 link/reparse检测，可复用规则和测试思路；private profile-specific实现不能被误称为现成 generic deleter。 | `crates/alcomd-vpm/src/plan.rs`、`project_copy.rs`、`backup.rs` |
| approved platform surface | 当前无 Trash API、generic delete adapter 或额外 Windows delete API。`rustix` 已存在于 Unix platform crate，但其既有批准范围不自动授权 P7 recursive cleanup。 | `crates/alcomd-platform/Cargo.toml`；`src/lib.rs` |

## Rust 1.97.1 filesystem audit

- [`std::fs::rename`](https://doc.rust-lang.org/1.97.1/std/fs/fn.rename.html) 在 Unix 对应 `rename`，在 Windows
  对应 `MoveFileExW`/`SetFileInformationByHandle`；跨 mount/filesystem 失败。同 parent 的 sibling quarantine 因而不会要求
  cross-volume move。destination 必须由 ALCOMD 创建的 opaque namespace约束，不能依赖“先 exists 再 rename”的排他性。
- [`std::fs::remove_dir_all`](https://doc.rust-lang.org/std/fs/fn.remove_dir_all.html) 不跟随 symbolic link，并明确说明在
  Windows、Linux、macOS 的实现对 symlink TOCTOU 有防护；Windows 1.97.1 source 使用 parent-relative open、
  `FILE_OPEN_REPARSE_POINT` 与 `OBJ_DONT_REPARSE`（`rust-lang/rust` tag `1.97.1` 的
  `library/std/src/sys/fs/windows/remove_dir_all.rs`）。
- 同一文档也明确：目标必须存在、失败可能已经删除部分内容、并发写入可产生 `DirectoryNotEmpty`。因此 cleanup 必须由 journal
  forward recover，不能把一次错误当作“没有副作用”。
- **缺口**：std 没有承诺不跨 Unix mount/bind-mount boundary。`st_dev` 能识别普通 cross-device mount，却不能充分识别同设备
  bind mount。由此推断，单独调用 std API 不能证明“只删 Project tree object graph”这一冻结安全合同。

P7 production 因此有一个明确人工停止点：Windows 可优先使用 pinned Rust std 的 no-follow删除；Unix 必须先批准一个
Project-Delete-private、fd-relative、no-follow且不跨 mount boundary 的精确实现方案。可以评估扩大现有 safe `rustix 1.1.4`
用法，但若无法覆盖 Linux/macOS mount boundary，不得退回 libc FFI、新 unsafe 或“pre-scan 后 remove_dir_all”。本 Stop A 不作
该生产改动。

## 删除策略决策

| 候选 | 结论 | 原因 |
|---|---|---|
| A. OS Trash / Recycle Bin | reject for P7 v1 | Windows 要 IFileOperation/SHFileOperation 或新 crate，macOS 要 Foundation trash API，Linux 要 freedesktop Trash 语义；当前无统一 headless authority、identity/recovery、partial-failure合同，并会新增 dependency/platform surface。v3事实保留为行为参考，不作为 v4恢复模型。 |
| B. ALCOMD-owned sibling quarantine -> permanent cleanup | **recommended** | 同 parent rename使 original path 原子消失；Plan/Operation/journal可绑定原 root与quarantine identity；crash后只 forward cleanup；后来重建的 original path与旧 payload分离。 |
| C. in-place recursive delete | reject | 第一次 destructive entry后 original path仍混合存在；partial failure、restart、registry ordering和新同名路径难以区分，恢复证据最弱。 |

P7 v1 deletion mode固定：`sibling-quarantine-permanent-v1`。它不是 Trash，不自动创建 Backup，也不承诺恢复按钮。

## Public RPC、Permission 与 Principal

候选 compatible RPC v1 additions：

```text
capability: projects.delete.v1
projects.planDeleteDirectory
projects.applyDeleteDirectory
Operation kind: projects.delete-directory
```

- Plan request exact fields：`projectId`、`expectedRevision`、`idempotencyKey`。
- Apply request exact fields：`planId`、`expectedRevision`、`idempotencyKey`。
- caller 不得提交 path、directory、URL、shell、recursive/followLinks/force/exclusion/ignore 参数。
- Plan permission：`projects.read + projects.delete`；Apply permission：`projects.delete`。
- 新 `projects.delete` 不由 `projects.manage` 隐式包含；外部 filesystem mutation还必须满足
  `PrincipalId == builtin:local-owner`。
- 4.0.0 不允许把 `projects.delete` grant给 Extension Principal；Manifest permission catalog、WIT、Host capability 与 Portable UI
  均不新增删除入口。public local RPC capability不等于 Extension capability。

## Bounded Plan

Plan TTL复用 Copy 的 `900000 ms`（15分钟）。两者都冻结易受外部 filesystem/writer变化影响的 identity preflight；另设值没有
实际收益。`now >= expiresAtMs` 即 stale；永久 idempotency replay仍返回原 Plan，重新 Plan必须使用新 key。

Plan exact authority fields：

- PlanId、owner Principal、ProjectId、Project revision；
- authoritative canonical root、opaque root filesystem identity；
- canonical parent、opaque parent identity及其 ResourceKey SHA-256；
- normalized leaf；
- bounded `ProjectSettings/ProjectVersion.txt` marker fingerprint与parsed Unity version/revision；
- Unity writer evidence；
- deletion mode `sibling-quarantine-permanent-v1`；
- safety profile `{id: alcomd-project-delete, version: 1}`；
- protected-root profile version；
- plan fingerprint、plan idempotency evidence、createdAtMs、expiresAtMs。

Plan只做 bounded exact-root/marker/identity/protected-root/writer检查，不递归遍历、不读取目录内容、不生成tree hash或inventory，
也不估算总文件数/bytes。

## Exact root safety

Plan与Apply都从 ProjectId重新读取 current owner-scoped registry row，并按下列顺序 fail closed：

1. row存在且 revision exact；
2. stored root path存在；对该 pathname的 `symlink_metadata` 证明它是ordinary directory且root本身不是symlink/junction/reparse；
3. canonical root identity等于registry/Plan identity；
4. canonical parent仍是ordinary directory且identity等于Plan；
5. canonical root的直接 leaf等于Plan normalized leaf；
6. existing exact-root reader仍能安全读取并解析bounded `ProjectSettings/ProjectVersion.txt`，其marker fingerprint等于Plan；
7. protected-root profile仍通过；
8. fresh writer observation为`not_observed`。

missing绝不视为成功；用户应使用既有 Remove from list。

Protected Root v1 对每个当前实际路径执行canonicalization、opaque identity与component-aware ancestry比较；任何解析/identity失败即
`project_delete_source_unsafe`。规则为：

- filesystem root：拒绝 candidate等于它（它没有合法direct parent/leaf）；
- user home：拒绝 candidate等于home或是home的ancestor；home下普通Project不因位于home内而被全部拒绝；
- ALCOMD data root、runtime root、package cache root、当前可执行文件的安装parent tree：candidate与任一critical root只要
  相等、是其ancestor或是其descendant（任意overlap）就拒绝；
- data root已包含的template/backup/extension/config/cache child仍显式受同一overlap规则保护；
- Project root若是任一关键目录ancestor，即使registry里已有row也拒绝。

## Writer、locks 与 link policy

Writer v1 exact policy：

| state | behavior |
|---|---|
| `running_confirmed` | hard reject，复用 `unity_project_running` |
| `running_suspected` | hard reject，`project_delete_source_unsafe` + safe reason `writer_state_suspected` |
| `unknown` | hard reject，`project_delete_source_unsafe` + safe reason `writer_state_unknown` |
| `not_observed` | 允许继续，但GUI不得表述为“Unity definitely not running” |

Apply一次性取得并持有：

```text
Project(ProjectId)
ProjectCreate(sha256(parent identity), normalized leaf)
```

去重后按 `ResourceKey::canonical_bytes` 排序，直到 `cleanup_complete` 或安全的 `recovery_required` evidence落盘。无需新增
ResourceKey。它串行 concurrent unregister、Copy source、Package/Backup、以及同parent/leaf的Create/Copy；不能阻止外部进程，
所以仍要每阶段identity revalidation。

Link / entry policy与Copy不同：

- root symlink、root junction、root reparse：reject；不决定“删link还是target”。
- nested symlink：允许删除link pathname本身，禁止跟随target；target必须在测试中存活。
- nested Windows junction/name-surrogate/other reparse：只允许由no-follow recursive primitive删除reparse entry本身；target必须存活。
- regular hardlink：允许删除quarantine内的pathname；其他link保留。link count不作为拒绝条件。
- FIFO/socket/device/other non-directory special file：只允许unlink directory entry，不打开、不读写device；外部object不受影响。
- Unix nested mount/bind mount：reject且不得跨入；在可证明的production primitive获批前是P7 implementation blocker。

任何“先scan无link，然后path-based递归删”的实现均违反合同。

## Quarantine 与 recovery

每个Operation在Project parent中创建唯一 wrapper：

```text
.alcomd-delete-<operation-id>.quarantine/
    owner.json
    payload/
```

wrapper由daemon以exclusive create建立、写入bounded versioned owner marker并sync；marker绑定owner、OperationId、PlanId、ProjectId、
root/parent identity与profile，不包含caller authority。随后只把原root同parent rename为`payload`。rename前destination必须不存在；
rename后必须验证payload identity等于Plan root identity并sync parent/wrapper。任何mismatch进入recovery_required。

rename成功后绝不再按original path执行delete/overwrite/merge/rename。外部重建original path时，ALCOMD忽略它；测试必须证明新目录
及其sentinel存活。cleanup每次只在wrapper/marker/payload identity全部匹配journal后处理payload；unknown wrapper entry、identity
mismatch或owner marker mismatch均保留evidence并进入recovery_required。成功后先证明payload absent，再删除owned marker和empty
wrapper。不得用`remove_dir_all(wrapper)`吞掉未知注入内容。

### Durable phases

| phase | filesystem / registry事实 | cancel | crash recovery |
|---|---|---|---|
| `accepted` completed | Plan绑定Operation；root与registry仍在 | yes | reacquire locks并重新preflight |
| `preflight_complete` completed | fresh identity/marker/writer/protected-root均通过；wrapper owner evidence durable | yes | 重做所有动态检查 |
| `quarantine_intent` intent | registry仍在；即将执行first destructive rename | **no，forward-only boundary** | 根据original/payload identity判定未rename或已rename；ambiguous fail closed |
| `root_quarantined` completed | original path absent；payload identity=root identity；registry尚在 | no | 只按quarantine identity继续 |
| `registry_commit_intent` intent | filesystem已quarantine；准备短DB transaction | no | 可幂等重放state commit |
| `state_committed` completed | Project registry row absent；单一delete Event、Operation/idempotency/journal已提交；payload可能仍在 | no | 只继续cleanup |
| `deleting` intent/append-only progress | payload正在no-follow cleanup；不宣称百分比 | no | 复验identity后继续 |
| `cleanup_complete` completed | payload、marker、wrapper均absent；Operation succeeded | terminal | 无 |
| `recovery_required` completed evidence | identity/mount/unknown-entry或persistent cleanup错误；quarantine保留 | no | 同Plan/Operation/idempotency重试；不得新Plan或触碰original path |

Public progress固定为phase-only；允许journal追加bounded attempt/error-class counters，但不保存entry list、完整路径、目录内容、总inventory
或虚假百分比。

Pre-quarantine错误/取消会删除仅在identity匹配时可证明owned且为空的wrapper，原Project与registry保持，Operation为failed/cancelled。
Post-intent永不回滚到original path；暂时cleanup失败使Operation保持非成功的`interrupted/recovering`语义并暴露
`project_delete_recovery_required`，Apply永久重放必须返回并恢复同一OperationId。未完成cleanup不得返回succeeded。

## Registry、State v13、Event 与 idempotency

安全顺序固定：fresh validate -> durable quarantine intent -> atomic sibling rename -> quarantine identity verify -> durable registry
commit intent -> one short SQLite transaction删除exact registry row并提交Event/Operation/idempotency/journal -> cleanup quarantine -> success。

State v13 proposal只新增业务表：

- `project_delete_plans`；
- `project_delete_filesystem_journal`；

并把Operation kind扩为`projects.delete-directory`。不创建generic deletion/job/tombstone/inventory table。

现有多个永久Plan/journal把ProjectId以FK指向可删除的`projects` registry row。物理删除row时既不能CASCADE擦掉历史authority，也不能
被RESTRICT永久阻止。因此v13 migration还必须**约束性重建**当前Schema 12中以下exact durable tables/columns：

- `package_plans.project_id`；
- `package_filesystem_journal.project_id`；
- `project_copy_plans.source_project_id`；
- `project_copy_filesystem_journal.source_project_id`。

这些字段保留为canonical UUID scalar；插入时仍由application/trigger证明current registry存在，但history不再以current registry row
作为lifetime owner。migration必须逐列保留所有row、trigger、index、Operation/idempotency关系并通过`foreign_key_check`。
`project_editor_preferences`是明确随registry消失的current preference，保留`ON DELETE CASCADE`。这是P7物理删除所必需的FK修正，
不增加第三张业务表。production migration开始前仍须重新扫描Schema，若届时出现新的Project FK必须停下更新合同，不能静默纳入。

新Delete Plan/Journal从一开始就把ProjectId保存为checked scalar，不引用`projects(project_id)`：

- Plan immutable，唯一transition为unapplied -> applied with exact OperationId，DELETE forbidden；
- journal `(operation_id, step)` append-only，UPDATE/DELETE forbidden；
- journal记录OperationId、PlanId、ProjectId、root/parent/quarantine opaque identity、private quarantine locator、phase/state、bounded
  safe evidence与timestamps；locator不进入RPC/Event/activity/log；
- existing idempotency record在Plan/Apply接受transaction中永久绑定method/key/fingerprint/result。registry删除后，同key/same
  fingerprint仍返回原Plan或Operation；different fingerprint仍`idempotency_conflict`。

Delete成功只写一个Event：`project.directory_deleted`。不再双写`project.unregistered`，因为那会把一次用户动作伪装成两个动作。
Event resource identity使用ProjectId和next revision，safe payload可含profile/mode但不得含完整root/quarantine path。

## Stable errors

新增且仅新增：

- `project_delete_plan_not_found`；
- `project_delete_plan_stale`；
- `project_delete_source_missing`；
- `project_delete_source_unsafe`；
- `project_delete_source_changed`；
- `project_delete_recovery_required`。

复用`project_not_registered`、`revision_conflict`、`unity_project_running`、`idempotency_conflict`、`permission_denied`、
`internal_error + diagnosticId`。禁止宽泛`project_delete_failed`。exact mapping：missing -> source_missing；root/parent/leaf/marker/
identity replaced -> source_changed；protected root/root link/mount/writer uncertain -> source_unsafe；post-quarantine mismatch/cleanup failure ->
recovery_required。

## GUI、CLI/API 与 MCP

GUI保留两个独立动作：`Remove from list` 与 `Delete Project Directory…`。Delete先Plan，再显示host-owned high-risk review：Project
display name、canonical local root、永久删除本地文件、不会进入Trash、不会自动Backup、与Remove from list的区别、writer safe
summary。用户必须输入exact displayed project leaf/name后才能按destructive Apply；该输入只解锁GUI，不进入RPC authority。

CLI在P7 production获批后提议发布独立`alcomd-cli project delete-directory --project-id <uuid>`，不复用`project unregister`
的`remove` alias。默认human/TTY显示Plan并确认；non-TTY/EOF需`--yes`，`--dry-run`只Plan，`--no-wait`返回OperationId；所有模式
仍经typed client/RPC且不接受path。public RPC即当前API publication；Local API未实现，不在P7增加。MCP不自动新增destructive tool。

## Permanent test contract

机器向量位于`specs/security/m7-project-delete-path-vectors.json`。production至少覆盖：normal delete；unregister-only不删盘；
arbitrary path无法表达；stale revision；missing/replaced root；parent/leaf/marker改变；root/home/data/runtime/cache/executable tree保护；
writer四态；root/nested symlink；Windows junction/reparse；hardlink；special file；Unix mount/bind mount；cancel boundary两侧；每个durable
phase真实kill/restart；quarantine identity/owner mismatch；original path重建及sentinel存活；partial cleanup/retry；permanent idempotency
replay/conflict；concurrent unregister/Copy/Create；registry与Event exact-once；success无owned quarantine；recovery_required保留evidence；
Windows/Linux/macOS真实filesystem测试。

Synthetic engineering evidence不能冒充M11 v3 differential parity。P7只恢复用户可见Delete Directory行为；不进入P8/H2/M8/M9/M11。

## Owner approval points before production

1. 批准本文件的RPC/Permission/Plan/Quarantine/State v13/Event/GUI/CLI合同；
2. 批准v13对existing durable Project FK的精确constraint rebuild；
3. 批准Unix Project-Delete-private mount-safe cleanup方案，或提供另一个能证明同等边界的safe实现；
4. production仍不得新增crate、unsafe或平台API；若第3项确实需要，必须另行提交exact依赖/API/unsafe评估。
