# M7 P5-B Project Favorite / Unity Preference Clear 审计

本文记录 v3 行为和当前首发合同，项目验收状态见[项目状态](../status.md)。

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

## Favorite 最小合同与实现

- State1 的 `projects` 保存 `favorite INTEGER NOT NULL DEFAULT 0 CHECK (favorite IN (0,1))`。
- 已注册项目的 `ProjectSnapshot` 返回 `favorite` 布尔值，未注册的 inspect 快照省略该字段。
- 新增 `projects.setFavorite(projectId,favorite,expectedRevision,idempotencyKey)`，复用 `projects.manage`；不新增 Permission、
  capability、Operation 或 ResourceKey。
- 实际状态变化在一个 transaction 中校验 Project revision、更新 Favorite、将 Project revision 加一、写 durable
  `project.favorite_changed` Event 并保存 idempotency response。相同 key/相同 fingerprint 返回 replay；相同 key/不同 fingerprint
  返回 `idempotency_conflict`。
- 新 key 设置为当前值是 semantic no-op：不增加 revision、不写 Event，仍保存确定的 completed idempotency response。
- `projects.list` 带视图参数时，Core 在分页前优先排列收藏项目，再应用所选排序。GUI 沿用 Core 游标续页。
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

## 首发 Unity 启动配置

项目使用其实际 Unity 版本选择兼容安装，具体安装可以在本次启动时指定。用户可以通过 `unity.projectLaunchConfig.get/set/clear` 读取、修改和清除项目启动参数。[Unity 项目版本](../exec-plans/M7-unity-project-version-stop-a.md)

## 现行合同

[项目与 Unity RPC](../../specs/rpc/alcomd-rpc-v1.md) · [State1](../../specs/storage/state-v1.md)
