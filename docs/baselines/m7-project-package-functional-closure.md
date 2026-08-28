# M7 Project / Package Functional Closure matrix

状态：P0-P4 remote checkpoint 与 P5-A Create/Restore production wiring 已通过；P5-B Favorite/Clear Unity Preference
contract-first Stop A proposal 已形成，尚未获生产实现批准。v3 只作行为参考；没有复制、移植或改写其源码。P6-P8 仍未完成，
因此聚合 feature 继续 `in_progress`。

| 用户入口 | 当前 Core 是否足够 | 最小合同 / 实现归属 | Permission 复用 | State / Config 影响 | dependency / platform | slice |
|---|---|---|---|---|---|---|
| Open Project Directory | `projects.get` + `ProjectSnapshot.rootPath` 足够 | closed Tauri `ProjectId` adapter；无 public RPC | `projects.read`（existing client call） | 无 | approved `open 5.4.2`，defaults off | P1 / S |
| Add/Register chooser | register RPC 足够；native chooser 缺失 | closed folder picker 后调用 existing `projects.register` | `projects.manage` | 无 | approved dialog 2.7.2，defaults off + gtk3 | P1 / S |
| Create | `templates.planCreateProject/applyCreateProject` 足够；built-in template 可表达 blank/create | Projects toolbar + native folder picker + existing Plan/Apply/Operation + Core ProjectId navigation | existing template/project/package read/manage matrix | 无 | compact host-owned dialog | P5-A / implemented + remote green |
| Restore | `backups.planRestore/applyRestore` 足够 | Projects toolbar + managed Backup selector + native folder picker + existing Plan/Apply/Operation + Core ProjectId navigation | existing backup/project permissions | 无 | compact host-owned dialog | P5-A / implemented + remote green |
| Copy Project | P2-P4 已实现 | active `projects.copy.v1`, `projects.planCopy/applyCopy`, Operation `projects.copy` | `projects.read + projects.create` | active State v10 两表 | approved dialog；copy engine无新 dependency/platform unsafe | P2-P4 / implemented candidate |
| Favorite | 不足；Project DTO/registry 没有 favorite | Stop A proposed `projects.setFavorite(projectId,favorite,expectedRevision,idempotencyKey)`；registered Project DTO兼容可选 `favorite` | `projects.manage` | proposed State v11 `projects.favorite`；不进入 observation JSON | 无 | P5-B proposal / M |
| Clear/Forget Unity preference | 不足；`unity.projectEditor.set` 要求 non-null installationId，且 installation/arguments 共存一行 | Stop A proposed `unity.projectEditor.selection.get` + `unity.projectEditor.clear`；tagged automatic/explicit selection；既有 set 不改 nullable | `unity.read` / `unity.manage` | proposed State v11重建 existing preference table；clear保留 arguments；无新表 | 无 | P5-B proposal / M |
| Open Unity / Backup | 已有真实 RPC/Operation | 保持 existing wiring；不是新合同 | existing | 无 | existing | 已实现证据保持 |
| Remove registry row | `projects.unregister` 足够 | GUI confirmation + existing RPC；不删除目录 | `projects.manage` | existing | 无 | P3 / S |
| Remove directory | 不足且是高影响删除 | 独立 Plan/Apply/Operation、writer gate、Project lock、trash/delete/recovery contract，另行人工审批 | 待定；不得从 `projects.manage` 隐式获得任意删除 | 后续 schema | 可能需平台 trash/删除评估 | P7 / XL |
| Package workspace refresh | 单 repo `repositories.refresh` 足够 | GUI 先 list，再逐 registered repo refresh，逐项显示 partial failure，最后 reload；不声称 atomic refresh-all | `repositories.manage/read` | 无 | existing HTTP/local | P4 / M |
| Package source filter | registered local/remote repository DTO 足够做展示过滤 | GUI-only filter，source identity来自 daemon；GUI不做 resolver | read only | 可保持 view state；无 authority state | 无 | P4 / S |
| Show prerelease | resolver已有 `includePrerelease`，read DTO只有 version string | proposed optional `RepositoryPackageVersion.prerelease: bool`，避免前端建立第二套 SemVer；偏好进入 Config Schema v2 | settings read/manage | Config v2，不是 State v10 | 无 | P4 / M |
| Hidden repositories | current Config v1 无字段 | Config v2 proposed `hiddenRepositoryIds`（bounded opaque IDs）；只影响展示/候选可见性，不能篡改 pinned Plan | settings read/manage | Config v2 | 无 | P4 / M |
| Hide local user packages | true local user package model不存在 | 先冻结 local package enrollment/source identity/hash/visibility；不得把 local repository JSON冒充 loose user package | 待审批 | 可能 registry + Config v2 | directory/path security另审 | P6 / L |
| Reinstall one/all | existing Plan action没有 reinstall；install same version可成为 no-op | proposed `packages.planReinstall`，输入 bounded package IDs（one/all由显式集合表达），仍由 `packages.applyPlan` Apply | existing packages read/manage + projects/repositories read | package plan action需后续 migration，**不并入 v10** | 无新依赖 | P5 / L |
| Bulk selected install/upgrade/remove/reinstall | ChangeSet可多 mutation，但现有 request不能表达显式混合集合 | proposed `packages.planBulk`，bounded typed intents，单 durable Plan/Apply；禁止客户端拼接多个 Plan | existing package permissions | package plan action/request evidence需后续 migration | 无 | P5 / XL |
| Hidden package section | Core 能返回 yanked/source，但 visibility policy不完整 | 由 Config v2 + daemon-provided prerelease/source class驱动；yanked保持不可新选 | settings/read | Config v2 | 无 | P4/P6 |
| Docs / changelog | 当前 normalized read DTO 丢弃对应 manifest字段 | proposed bounded optional link descriptors；GUI closed command只接受 `(repositoryId,packageId,version,linkKind)`，daemon re-fetch current link，拒绝非 http/https/userinfo | repositories/packages read | cache row/DTO兼容增加，是否迁移另审 | opener `open_url` 不能接收 frontend URL；依赖同 opener候选 | P6 / L |
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
7. State v10 保持 Copy-only，避免一次 proposal 同时改变 Copy、Favorite 与 Package Plan authority。
8. v3 Favorite 是持久 Project metadata，始终作为 selected sort 的第一排序键；v4 proposal 不改变 Core pagination cursor，
   official GUI只对完整加载集合做稳定 favorite-first presentation，也不新增 favorite-only filter。
9. v3 Clear Unity path 保留 custom arguments并恢复 automatic selection。当前 v4 set/get 无法兼容表达；删除 preference row会丢失
   arguments且让 launch直接失败，因此 P5-B proposal需要显式 tagged read + clear method与 State v11 table rebuild。

## Release / milestone ownership

- 除明确属于 M11 的 migration/differential parity 外，已知 Project/Package 缺口均属于 M7 functional closure；未完成前
  `projects.management` / `packages.vpm` 不得提升为 implemented。
- P7 Remove Directory 是 M7 内独立 high-impact approval slice；没有批准前保留缺口，不显示永久 disabled production fake action。
- VCC import/migrate 与真实 v3 differential evidence属于 M11，保持 blocked。
- Win10/Win11完整客户端安装/运行/WebView2/更新/卸载仍属于 M12。
