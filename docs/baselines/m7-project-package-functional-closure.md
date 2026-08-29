# M7 Project / Package Functional Closure matrix

状态：P0-P4 remote checkpoint、P5-A Create/Restore production wiring 与 P5-B Favorite/Clear Unity Preference
均已通过本地与远端验收。P6-A Package refresh/source filter 已通过本地完整验收、三平台 Hosted CI 与 CodeQL。
v3 只作行为参考；没有复制、移植或改写其源码。P6-B/P6-C Stop A 已按合同修正获生产批准；P7-P8 仍未完成，
因此聚合 feature 继续 `in_progress`。

| 用户入口 | 当前 Core 是否足够 | 最小合同 / 实现归属 | Permission 复用 | State / Config 影响 | dependency / platform | slice |
|---|---|---|---|---|---|---|
| Open Project Directory | `projects.get` + `ProjectSnapshot.rootPath` 足够 | closed Tauri `ProjectId` adapter；无 public RPC | `projects.read`（existing client call） | 无 | approved `open 5.4.2`，defaults off | P1 / S |
| Add/Register chooser | register RPC 足够；native chooser 缺失 | closed folder picker 后调用 existing `projects.register` | `projects.manage` | 无 | approved dialog 2.7.2，defaults off + gtk3 | P1 / S |
| Create | `templates.planCreateProject/applyCreateProject` 足够；built-in template 可表达 blank/create | Projects toolbar + native folder picker + existing Plan/Apply/Operation + Core ProjectId navigation | existing template/project/package read/manage matrix | 无 | compact host-owned dialog | P5-A / implemented + remote green |
| Restore | `backups.planRestore/applyRestore` 足够 | Projects toolbar + managed Backup selector + native folder picker + existing Plan/Apply/Operation + Core ProjectId navigation | existing backup/project permissions | 无 | compact host-owned dialog | P5-A / implemented + remote green |
| Copy Project | P2-P4 已实现 | active `projects.copy.v1`, `projects.planCopy/applyCopy`, Operation `projects.copy` | `projects.read + projects.create` | active State v10 两表 | approved dialog；copy engine无新 dependency/platform unsafe | P2-P4 / implemented candidate |
| Favorite | active DTO/registry/GUI 已实现 | `projects.setFavorite(projectId,favorite,expectedRevision,idempotencyKey)`；registered Project DTO兼容可选 `favorite` | `projects.manage` | active State v11 `projects.favorite`；不进入 observation JSON | 无 | P5-B / implemented + remote green |
| Clear/Forget Unity preference | active automatic/explicit state 与 legacy view 已实现 | `unity.projectEditor.selection.get` + `unity.projectEditor.clear`；tagged automatic/explicit selection；既有 set 保持 non-null | `unity.read` / `unity.manage` | active State v11重建 existing preference table；clear保留 arguments；无新表 | 无 | P5-B / implemented + remote green |
| Open Unity / Backup | 已有真实 RPC/Operation | 保持 existing wiring；不是新合同 | existing | 无 | existing | 已实现证据保持 |
| Remove registry row | `projects.unregister` 足够 | GUI confirmation + existing RPC；不删除目录 | `projects.manage` | existing | 无 | P3 / S |
| Remove directory | 不足且是高影响删除 | 独立 Plan/Apply/Operation、writer gate、Project lock、trash/delete/recovery contract，另行人工审批 | 待定；不得从 `projects.manage` 隐式获得任意删除 | 后续 schema | 可能需平台 trash/删除评估 | P7 / XL |
| Package workspace refresh | 单 repo `repositories.refresh` 足够 | GUI 完整分页 list，顺序逐 registered repo refresh，逐项记录 partial failure，`revision_conflict` 仅 fresh retry 一次，最后 reload；不声称 atomic refresh-all | `repositories.manage/read` | 无 | existing HTTP/local | P6-A / implemented + remote green |
| Package source filter | registered local/remote repository DTO 足够做展示过滤 | GUI-only All/Remote/Local filter，source identity只来自 daemon `RepositorySource.kind`；GUI不做 resolver | read only | 不持久化；无 authority state | 无 | P6-A / implemented + remote green |
| Show prerelease | resolver已有 `includePrerelease`，read DTO只有 version string | proposed optional `RepositoryPackageVersion.prerelease: bool`；strict Core parse返回bool，legacy/unparseable absent且不可当stable；默认false | settings read/manage | Config v2，不是State | 无 | P6-B Stop A |
| Hidden repositories | current Config v1 无字段 | Config v2 max 256 canonical unique `hiddenRepositoryIds`；stale ID保留，只影响official GUI展示/source chooser，不修改resolver authority | settings read/manage | Config v2 | 无 | P6-B approved |
| User Packages | v3 是 `userPackageFolders` loose directory，不是local repository | approved directory-only enrollment、opaque file identity、deterministic owned cache archive、v2 source pin、refresh/remove CAS；不接受archive input | packages.read/manage + local-owner | State v12专用 `user_package_sources` | 复用现有semver/sha2/zip/platform；无新依赖 | P6-C approved |
| Reinstall one/all | existing Plan action没有 reinstall；install same version可成为 no-op | approved `packages.planReinstall`，plan.v2、`packages|all` tagged selection、max 256，locked exact version重新解析source并force replace | existing packages read/manage + projects/repositories read | State v12 action=`reinstall` | 无新依赖 | P6-C approved |
| Bulk selected install/upgrade/remove/reinstall | ChangeSet可多 mutation，但现有 request不能表达显式混合集合 | approved `packages.planBulk`，plan.v2、max 256 unique typed intents，单 durable Plan/Apply；禁止客户端拼接多个 Plan | existing package permissions | State v12 action=`bulk` | 无 | P6-C approved |
| Hidden package section | Core 能返回 yanked/source，但 visibility policy不完整 | 由 Config v2 + daemon-provided prerelease/source class驱动；yanked保持不可新选 | settings/read | Config v2 | 无 | P4/P6 |
| Docs / changelog | 当前 normalized read DTO 丢弃真实 `documentationUrl`/`changelogUrl` | proposed 2,048-byte validated optional descriptors；GUI closed command只接受 `(repositoryId,packageId,version,linkKind)`，daemon re-fetch current link，拒绝非 http/https/userinfo | repositories/packages read | State v12 nullable URL columns | 复用reqwest URL与open；无新依赖 | P6-B Stop A |
| VCC Import / Migrate | 不足 | M11 migration-only implementation +真实脱敏 Fixture | M11决定 | M11 | M11 | M11 / blocked |

## 关键判断

1. Create、Restore、逐 repository refresh 不需要新 Core 方法；缺口是 GUI flow/native chooser。
2. `unity.projectEditor.set` 当前 Schema 要求 `installationId: string`，把它改成 nullable会改变既有方法输入合同；新增窄 clear method
   更直接。
3. `PackageChangeSet` 虽支持多个 mutation，但现有 Plan request 无法表达 reinstall 或显式 mixed bulk，不能由 GUI 串行多个
   Plan/Apply 冒充单一事务。
4. prerelease 不应由 TypeScript 解析 version string；SemVer authority继续留在 Core，DTO只兼容增加布尔分类。
5. v3 的 “hidden” 包含 hidden repository、local user package visibility和折叠后的不可用来源，不是一个可由 CSS 隐藏代替的
   单一概念。
6. docs/changelog URL 来自不可信 repository manifest；React 不得把任意 URL传给 generic opener。
7. State v10 保持 Copy-only历史基线；P5-B只由独立 State v11承载 Favorite与 Unity selection，不改变 Package Plan authority。
8. v3 Favorite 是持久 Project metadata，始终作为 selected sort 的第一排序键；v4 active实现不改变 Core pagination cursor，
   official GUI只对完整加载集合做稳定 favorite-first presentation，也不新增 favorite-only filter。
9. v3 Clear Unity path 保留 custom arguments并恢复 automatic selection。当前 v4 set/get 无法兼容表达；删除 preference row会丢失
   arguments且让 launch直接失败，因此 P5-B active实现使用显式 tagged read + clear method与 State v11 table rebuild。

## Release / milestone ownership

- 除明确属于 M11 的 migration/differential parity 外，已知 Project/Package 缺口均属于 M7 functional closure；未完成前
  `projects.management` / `packages.vpm` 不得提升为 implemented。
- P7 Remove Directory 是 M7 内独立 high-impact approval slice；没有批准前保留缺口，不显示永久 disabled production fake action。
- VCC import/migrate 与真实 v3 differential evidence属于 M11，保持 blocked。
- Win10/Win11完整客户端安装/运行/WebView2/更新/卸载仍属于 M12。
