# ADR 0022：Backup Restore 只创建全新项目

状态：Accepted（2026-08-24，contract-first；Restore 生产实现尚未获批）

## 决定

M5 Backup Restore 只接受已经注册且每次重新验证通过的 `ALCOMD Backup Archive v1`，并把它恢复为
一个全新、不存在的 Unity Project directory。M5 不支持 in-place、overwrite、merge、delete-then-restore、
已有 Unity Project 目标、v3 legacy backup 或 arbitrary ZIP。v3/legacy compatibility 继续 blocked 到 M11。

Restore 必须经 `backups.planRestore` / `backups.applyRestore`。Plan 是 durable immutable authority，创建时
预分配 `ProjectId`，固定 Backup metadata、archive 文件身份/SHA-256/大小/格式、`backup.json` fingerprint、
VPM 排除摘要、target parent identity/path、normalized leaf、目标必须不存在、预期 Unity Project 摘要与 Plan
fingerprint。Apply、幂等 replay 与 recovery 始终复用同一 PlanId、OperationId、ProjectId 和 BackupId，不
重新 Plan，也不生成第二个 ProjectId。

Plan 和 Apply 都重新验证 parent 与 target；Plan 还必须重新打开归档并验证文件身份、大小、SHA-256、ZIP
profile、严格 `backup.json` 和 `formatVersion=1`。Apply 再次验证全部 frozen evidence。archive bytes 永远
视为不可信，完整复用 Backup Archive v1 的 Stored/Deflate、ZIP64、path/collision/link/special-file 与
64 GiB/500,000 entries/32 GiB single file/128 GiB total/depth 128/path 1,024 UTF-8 bytes/ratio 10,000:1
限制。ZIP 根只能是 `backup.json` 与 `project/`，最终 target 不保留 `project/` wrapper。

## 新项目事务与恢复

Restore 与 Template create-project 竞争同一目标时复用
`ResourceKey::ProjectCreate(parent_identity, normalized_leaf)`；不新增 `ProjectRestore`。staging 是 target
parent 同卷、operation-owned sibling，publish 使用 atomic directory rename。固定顺序为：

```text
revalidate Plan / Backup artifact
-> ProjectCreate lock
-> create safe sibling staging
-> extract project/ into staging
-> validate complete Unity Project
-> durable publish intent
-> atomic staging -> target
-> sync parent
-> durable target_published
-> register preallocated ProjectId
-> state_committed
-> succeeded
```

Template create-project 当前把持久 checkpoint 写为 `operation_journal.kind='templates.create-project'`，其
所有权和恢复语义明确是 Template-specific，不能静默改义。因此 Schema v7 选择窄的
`backup_restore_filesystem_journal`。它只记录 Restore 的 append-only intent/completed evidence；复用既有
安全 archive/path engine、ProjectCreate lock 和 new-project staging/publish primitive，不建立第二套完整
filesystem engine，也不复用 `package_filesystem_journal`。

阶段固定为 `accepted`、`archive_verified`、`extracting`、`staging_complete`、`publish_intent`、
`target_published`、`project_registry_commit_intent`、`state_committed`。`publish_intent` 前取消可停止并清理
operation-owned staging；之后进入 forward-finalize/recovery decision，不能因 cancel 删除可能已经用户可见
的 target。

publish 前 crash 可丢弃 staging，并以同一 authority 重新 extract。durable `target_published` 后默认 forward
recovery：只有 target identity、project fingerprint、Plan evidence 与预分配 ProjectId 全部一致时才能完成
registry/DB finalize；若外部已经修改 target，返回 `backup_restore_recovery_required`，不得 succeeded、删除
target 或重新解压覆盖。target 已 publish 但 DB commit 失败时同样优先 forward-finalize。

## VPM、RPC 与实施边界

`excludeVpmPackages=true` 只恢复 archive 中实际存在的文件并保留两个 manifest，不联网、不 refresh、不下载、
不 resolve、不调用 staging package materializer；result 明确 `packagesRequireResolve=true` 并返回 bounded
`excludedPackages` summary。为 false 时也只恢复 archive，不根据本机 repository/package 状态做 normalization。

RPC v1 兼容冻结 capability `backups.restore.v1` 与两个方法。Plan 需要 `backups.read + projects.create`；Apply
需要 `backups.read + backups.manage + projects.create`。外部 filesystem write 仍只允许
`builtin:local-owner`。公共 DTO 不返回 archive/staging 路径、DB locator 或 journal detail。

本 ADR 只批准合同、Schema v7 migration、synthetic hostile fixture 与合同/迁移/安全测试。它不批准 Restore
dispatcher/client/CLI publication、application use case、worker、提取/publish/registry adapter、新 production
dependency、unsafe 或平台 API。
