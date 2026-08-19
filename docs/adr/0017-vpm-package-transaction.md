# ADR 0017：M4 VPM Package Plan/Apply 与可恢复文件事务

状态：Accepted（2026-08-19，contract-first；生产实现尚未获批）

## 决策

M4 只实现首个 package transaction 垂直切片：确定性 `Plan`、显式 `Apply`、持久
`OperationId`、内容寻址 cache、受限 ZIP extraction、项目文件事务与 crash recovery。首个动态
实现只允许 install/remove；upgrade/downgrade/resolve 复用同一个 model 与 `ChangeSet`，不得建立
通用 package manager 或 workflow engine。

合同、State Schema v3 与 migration 可以先落盘并接受兼容测试；在生产依赖方案获得独立人工批准前，
daemon/store/VPM 生产路径仍停留在 M3/Schema v2，不广告 M4 capability，也不执行 Schema v3 migration。

## Package、版本与来源

- Remote VPM package manifest 必须验证 `name`、`displayName`、`version`、`url`、
  `author.name` 与 `author.email`；repository map key 必须分别等于 manifest 的 name/version。
- `legacyFolders`、`legacyFiles`、`legacyPackages` 只解析 presence。M4 不清理 legacy；需要清理的
  计划返回 `package_legacy_cleanup_required`。
- VPM range 对齐 SemanticVersioning.NET/npm 风格的 exact、bare、comparator、x/`*`、tilde、caret、
  hyphen、OR、prerelease、build metadata 与约束交集。prerelease 默认排除，只有请求显式
  `includePrerelease` 或 comparator 本身含 prerelease 时允许。
- `unity` 是最低 Unity major.minor；`unityRelease` 是可选补充。缺失 `unity` 表示无限制，不把该字段
  当 SemVer range。任何 VRChat 特例必须另有具名、版本化 ADR；M4 当前没有这类隐藏策略。
- yanked 版本不参与新选择，也不能新 install/upgrade/downgrade；已安装的 exact yanked 版本在
  未改变时可以保留。M4 不提供 yanked override。
- repository `priority` 数值越小优先级越高。显式 source pin 高于 priority；同一业务 priority 的
  冲突不能用 UUID、canonical source ID、HashMap 顺序或响应时序偷偷决胜，必须返回
  `package_source_ambiguous`。
- Plan 固定 repository ID/revision、source identity、manifest fingerprint、package version、artifact
  URL 与 SHA-256。Apply 不 fallback、不重新求解、不改变版本或来源。

## Repository snapshot 与 Plan

Schema v3 把 M3 raw repository snapshot 扩展为 resolver-ready metadata。由 v2 升级的既有 row
全部是 `resolver_ready = 0`；只有显式 `repositories.refresh` 经 M4 parser 完整成功后才可置为 ready。
Plan 本身不访问网络、不刷新 repository，只使用 last-known-good resolver-ready snapshot；否则返回
`repository_refresh_required`。

Plan 是 durable、immutable record，状态只有 `unapplied` 与 `applied`，无 TTL、expiry 或自动清理。
一个 Plan 最多绑定一个成功接受的 Apply Operation。`ChangeSet` 最多 1,024 个 package mutation 与
4,096 条 dependency edge，且仍受 RPC 单帧 4 MiB 上限；超限返回 `plan_too_large`，不得截断。

RPC v1 兼容增加以下方法与 capability：

```text
packages.planInstall
packages.planRemove
packages.planUpgrade
packages.planDowngrade
packages.planResolve
packages.applyPlan

packages.plan.v1
packages.apply.v1
```

Plan 不修改项目、不下载、不 refresh。Apply 参数固定为 `planId`、`expectedRevision`、
`idempotencyKey`，接受后返回 durable OperationId。任何 project/source/package 前提变化返回
`plan_stale` 与稳定 subreason；绝不静默产生不同 ChangeSet。

## 下载、cache 与 archive

- M4 remote archive 必须声明 64 个十六进制字符的 SHA-256；缺失或畸形返回
  `package_hash_required`。这是 M4 安全子集，不代表完整 VPM hashless 兼容已经完成。
- 复用 M3 精确的 `reqwest 0.13.4`、仅 rustls、`no_proxy()`、无 cookie/header/credential；只允许
  无 userinfo 的 HTTP(S)，拒绝 HTTPS 降级，最多 5 次 redirect，connect timeout 10 秒，单次下载
  总 timeout 10 分钟。
- streamed body 与 `Content-Length` 的硬上限均为 1 GiB；后者只作提前拒绝。cache object key 为
  `sha256:<64-lower-hex>`，路径为版本化 `sha256/<prefix>/<digest>.zip`。`.part` 使用 create-new，
  hash、flush/fsync 后 atomic publish；每次命中都重算 hash。
- cache 单 object 最大 1 GiB，总硬上限 16 GiB；M4 不自动 LRU。超限返回
  `package_cache_quota_exceeded`。offline 坏对象返回 `package_cache_corrupt`，不得使用。
- ZIP 仅支持 Stored/Deflate，不支持 encryption 或其他 compression。写 staging 前必须完整预检
  central directory，随后流式解压时再次计数。限制为 65,536 entries、单 entry 1 GiB、总解压
  4 GiB、深度 64、normalized UTF-8 path 1,024 bytes、expansion ratio 1,000:1。
- 不允许 lossy filename decoding；拒绝 absolute、`..`、空 segment、NUL/control、drive/UNC/device/
  ADS、link/special file、duplicate/file-directory collision、平台 case collision、Unicode normalization
  collision，以及 Windows reserved/trailing-dot-space。不得调用绕过这些检查的 convenience extract。

## 项目文件事务与恢复

M4 只允许写 `Packages/<validated-id>/`、`Packages/vpm-manifest.json` 和本次事务拥有的
`Library/ALCOMD/transactions/<OperationId>/` staging/backup。`Packages/manifest.json` 必须保持
byte-for-byte 不变；若公开 Fixture 证明必须修改，需另行人工批准。

staging 必须位于同一 project/volume。进入每个路径组件时重新验证类型、owner 与 no-follow/reparse
约束，最终路径不得逃逸 root。正式 commit 顺序固定为：

1. archive verified；central directory preflight 与 extraction 完成；新 VPM manifest 已生成；
2. project/source/package fingerprint 重验；journal intent durable；
3. old package directory rename 到 backup；new staging rename 到 `Packages/<id>`；fsync；
4. atomic replace `vpm-manifest.json`；fsync；验证最终 tree/manifest；
5. durable `filesystem_committed`；
6. 单个短 SQLite transaction 提交 Project revision、Event、Operation、idempotency result 与
   `state_committed`，然后才可报告 succeeded。

不得先删除旧目录。对每个 destructive phase 都先 DB commit intent，再执行 filesystem mutation，
fsync/记录 evidence，最后 DB commit completed。冻结 phase 为：`accepted`、`archive_ready`、
`extracted`、`prepared`、`packages_replaced`、`vpm_manifest_committed`、`filesystem_committed`、
`state_committed`、`rolling_back`、`rolled_back`、`recovery_required`。恢复只依据 intent、project-local
marker、old/new digest 与 project identity；证据缺失或冲突时 fail closed，不猜测。

cancel 在首次 destructive mutation 前可以清 staging 并收敛 cancelled；之后只能完整 rollback 或
完成 commit，且永不删除 recovery evidence。外部 writer 不受 ALCOMD `Project(project_id)` 锁约束，
因此每个关键阶段必须重验；变化返回 `project_changed_during_apply` 或 recovery-required，不覆盖新内容。

## Lock、权限与错误

增加 `PackageCache(sha256)` Resource Key，复用 Project/Repository/Operation。download/cache/extract
不持有 Project lock；cache lock 必须先释放，再取得 Project/source lock 并重验。不同 project 可并行，
同 project 写入串行；不增加全局 Catalog 锁。

所有 plan 方法需要 `projects.read + repositories.read + packages.read` 以及目标 project/repository
scope。Apply 还需要 `packages.manage` 与目标 project write scope，并复验 Plan owner。Plan 的内部
持久化不要求 `packages.manage`。M4 只允许 builtin local owner 完成写入；真实外部 credential、
enrollment 与 revocation 仍未完成。

除既有错误外，M4 冻结 package/plan/cache/archive/transaction 的稳定字符串错误；未知内部错误仍为
`internal_error + diagnosticId`，不得泄漏完整路径、URL credential、raw manifest/archive 或 SQL。

## 后果与排除

M4 的 SHA-256 必需策略、无 legacy cleanup、anonymous source 与 install/remove 首切片都是明确限制。
完整 `packages.vpm`、GUI、MCP、Extension Runtime、Unity/Hub、模板、备份、v3 migration、installer、
updater 与发行仍不在 M4。public/synthetic Fixture 不得冒充 M11 的真实 v3 differential evidence，
`projects.v3-parity` 继续 blocked。
