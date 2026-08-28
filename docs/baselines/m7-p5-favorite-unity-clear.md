# M7 P5-B Project Favorite / Unity Preference Clear 审计

状态：contract-first Stop A 已按项目所有者修订批准并完成 production implementation、本地完整验收、三平台 Hosted CI 与 CodeQL；
P6/H2 尚未开始。
本文保留 v3 行为证据与进入 P5-B 时的 v4 缺口，并记录已经激活的 State v11、RPC、typed client与 official GUI合同。

## v3 Project Favorite 精确行为

只读参考源码位于 `../ALCOMD3-v3-readonly`，冻结提交为
`4aa98ae4f18d42c10137278997180dbede991e88`。本审计只记录行为，没有复制、移植或改写其实现。

- `vrc-get-vpm/src/environment/project_management.rs` 的 `UserProject` 把 `Favorite` 作为项目 LiteDB document 中的持久
  `boolean`；新建/注册记录默认 `false`，读取缺失或非法值也按 `false` 处理。
- `vrc-get-gui/src/commands/environment/projects.rs::environment_set_favorite_project` 按规范化项目路径查找记录、修改
  `Favorite` 并保存数据库；v3 activity log 记录 `project.set_favorite` 和目标状态。
- list row 与 grid card 都显示 star toggle；`-projects-list-card.tsx::sortSearchProjects` 先执行用户选择的 name/type/
  Unity/added/last-modified 排序，再做稳定 favorite-first partition。因此 Favorite 总是优先于所选排序；没有 project
  favorite-only filter。
- 进程重启后值由 LiteDB 恢复。注销项目会删除整条 project document；同一 filesystem path 重新注册会创建新 document，
  因而恢复默认 `false`。dedup 只用于合并同时存在的重复记录，并以任一重复项为 favorite；它不是注销后的 tombstone。
- Create、Restore、Copy 注册都调用创建新 `UserProject` 的路径，初始 Favorite 为 `false`。
- v3 没有 Project revision；Favorite 属于持久 Project metadata，而不是从项目目录重新观察出的文件内容。v3 activity log
  有记录，但没有可供 v4 复用的 durable aggregate revision/Event 合同。

## P5-B 前 v4 Project 能力

- `ProjectRecord` 只有 observation、revision 与 registered timestamp；`ProjectSnapshot` 和 `projects` table 均无 Favorite。
- `snapshot_json` 是可刷新、由磁盘重新观察得到的 project read model，不适合承载用户 Favorite；把 Favorite 放进去会让 refresh
  authority 混淆并可能覆盖用户选择。
- active `projects.list` cursor 只由 registration ordering 构成。P5-B proposal 不改变 registry page ordering 或 cursor；official
  GUI 在已加载结果上做稳定 favorite-first presentation，继续保留用户所选 secondary sort。没有 favorite-only filter。

## Favorite 最小合同与实现

- State v11 给 `projects` 增加 `favorite INTEGER NOT NULL DEFAULT 0 CHECK (favorite IN (0,1))`；v10 保持冻结。
- registered `ProjectSnapshot` 兼容增加可选 `favorite:boolean`，实现后 registered project 必须返回它，未注册 inspect snapshot
  省略它。客户端继续忽略未知可选字段。
- 新增 `projects.setFavorite(projectId,favorite,expectedRevision,idempotencyKey)`，复用 `projects.manage`；不新增 Permission、
  capability、Operation 或 ResourceKey。
- 实际状态变化在一个 transaction 中校验 Project revision、更新 Favorite、将 Project revision 加一、写 durable
  `project.favorite_changed` Event 并保存 idempotency response。相同 key/相同 fingerprint 返回 replay；相同 key/不同 fingerprint
  返回 `idempotency_conflict`。
- 新 key 设置为当前值是 semantic no-op：不增加 revision、不写 Event，仍保存确定的 completed idempotency response。
- Favorite 不改变 Core list ordering/cursor，因此 mutation 不使已签发 cursor 失效；official GUI 对当前完整加载集合稳定地把 favorites
  排到前面。不存在 favorite-only filter。
- unregister 删除 Favorite；同一 path identity 重新注册得到新 ProjectId 且默认 `false`。Create/Copy/Restore 新项目也默认
  `false`。

## v3 Clear Unity Preference 精确行为

- v3 将显式 editor path 与 custom Unity arguments 分别保存在 project document 的 `unity_path` 与 `custom_unity_args`。
- `project_set_unity_path(path, null)` 只移除 `unity_path` 并保存；不会清除 custom arguments。菜单只在存在 override 时显示
  “forget Unity path”。
- 下一次 launch 立即恢复 automatic selection：按项目 Unity version 找 installation。一个匹配项直接使用；多个匹配项要求用户
  选择，并可重新勾选“keep using”；零匹配项返回缺失提示。保存的 path 已不再匹配时，一个候选会清掉 stale override 并使用它，
  多个候选会重新询问。
- v3 对已经没有 override 的 clear 仍可安全移除缺失字段并保存，但正常 GUI 不显示该 action。v3 没有 revision；命令 activity
  记录更新。missing installation 不让 stale path 继续成为 launch authority。

## P5-B 前 v4 Unity 能力与缺口

- active `unity.projectEditor.set` 要求 non-null `installationId`，并把 installation 与 arguments 存在同一
  `project_editor_preferences` row；`unity.projectEditor.get` 和 `unity.launch` 都要求该 row 存在。
- table 的 `installation_id` 是 `NOT NULL` 且 `ON DELETE RESTRICT`。现有 set/get 无法表达 automatic selection，也无法在清除
  installation override 时保留 arguments。把既有 required string 偷改成 nullable 是破坏性合同，不接受。

## Unity 最小合同与实现

- 复用现有 `unity.read` / `unity.manage` / `unity.launch` Permission 和 capability；不新增 Permission。
- 新增 `unity.projectEditor.selection.get`，返回 tagged selection：`automatic` 或 `explicit{installationId}`，并返回 arguments、
  preference revision 与 timestamp。缺失 row 是 canonical automatic + empty arguments + revision `0`。
- 新增 `unity.projectEditor.clear(projectId,expectedRevision,idempotencyKey)`。它只把 selection 改为 automatic，保留 arguments；
  不把 `installationId:null` 塞入既有 `unity.projectEditor.set`。
- State v11 重建现有 table 为 `selection_mode` + nullable `installation_id` + existing arguments/revision/timestamp；约束只允许
  `explicit + non-null` 或 `automatic + null`。既有 row 迁移为 explicit；不新增第二张 Unity preference table。
- clear 的 `expectedRevision=0` 只匹配缺失/default automatic；正值必须精确匹配现有 preference revision。真实 explicit ->
  automatic 变化增加 preference aggregate revision并写 `unity.project-editor.selection_cleared` Event；不增加 Project revision。
  已经 automatic 的新-key clear 是 no-op，不增加 revision/Event，但保存 completed idempotency response。
- launch 在 automatic 模式每次根据当前 registry 与 project Unity version 重新解析：零匹配返回既有 installation-not-found 语义，
  一个匹配直接使用，多个匹配返回新稳定 `unity_editor_selection_required`，由 GUI 要求用户显式选择后调用既有 set。不会静默
  选择 nondeterministic candidate。
- clear 后下一次 launch 立即使用上述 resolution；arguments 原样保留。missing/stale explicit installation 继续 fail closed；
  不回退到环境变量或不受管理的 executable path。

## Active production evidence

经批准后保留为 implemented evidence 的 proposal schema 位于 `specs/rpc/m7-project-preferences.proposal.schema.json`，active State合同位于
`specs/storage/state-v11.md` 和 `state-v11-migration.proposal.contract.json`，machine vectors 位于
`crates/alcomd-testing/fixtures/m7/project-preferences-contract-vectors.json`。active implementation包括：

- `0011_project_preferences.sql` 单事务 migration与 State v11广告；
- active protocol/dispatcher/store/application/typed client/Tauri/GUI；
- Favorite完整分页 stable-first presentation、list/grid真实 toggle；
- automatic/explicit selection、legacy get/set、clear与 automatic launch 0/1/multiple分支；
- Rust migration/store/RPC/compatibility tests与 deterministic Playwright tests。

P6-P8仍未完成，因此 `projects.management` 与 visible-action completeness继续 `in_progress`/`planned`。
