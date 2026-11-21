# M7 Project / Package Functional Closure matrix

状态：P0-P4 remote checkpoint、P5-A Create/Restore production wiring 与 P5-B Favorite/Clear Unity Preference
均已通过本地与远端验收。P6-A/P6-B/P6-C 已通过本地完整验收、三平台 Hosted CI 与 CodeQL。
v3 只作行为参考；没有复制、移植或改写其源码。P7 Delete Project Directory 已在 sealed HEAD
`2ee11066c07a0994f3aebe6a9ce3f84ab2c8acd9` 通过三平台 Hosted CI run `33298022030` 与 CodeQL run
`33298021806`，remote checkpoint 为 PASS。P8 的 M7-owned visible-action gate 可独立 PASS；M11 VCC Import/Migrate、
legacy entry 与真实 v3 differential parity 仍是全局 release blocker。聚合 feature 按完整冻结 user-entry 保持 `in_progress`。

| 用户入口 | 当前 Core 是否足够 | 最小合同 / 实现归属 | Permission 复用 | State / Config 影响 | dependency / platform | slice |
|---|---|---|---|---|---|---|
| Open Project Directory | `projects.get` + `ProjectSnapshot.rootPath` 足够 | closed Tauri `ProjectId` adapter；无 public RPC | `projects.read`（existing client call） | 无 | approved `open 5.4.2`，defaults off | P1 / S |
| Add/Register chooser | register RPC 足够；native chooser 缺失 | closed folder picker 后调用 existing `projects.register` | `projects.manage` | 无 | approved dialog 2.7.2，defaults off + gtk3 | P1 / S |
| Create | `templates.planCreateProject/applyCreateProject` 足够；built-in template 可表达 blank/create | Projects toolbar + native folder picker + existing Plan/Apply/Operation + Core ProjectId navigation | existing template/project/package read/manage matrix | 无 | compact host-owned dialog | P5-A / implemented + remote green |
| Restore | `backups.planRestore/applyRestore` 足够 | Projects toolbar + managed Backup selector + native folder picker + existing Plan/Apply/Operation + Core ProjectId navigation | existing backup/project permissions | 无 | compact host-owned dialog | P5-A / implemented + remote green |
| Copy Project | P2-P4 已实现 | active `projects.copy.v1`, `projects.planCopy/applyCopy`, Operation `projects.copy` | `projects.read + projects.create` | 不可变计划和文件系统日志 | approved dialog；copy engine无新 dependency/platform unsafe | P2-P4 / implemented candidate |
| Favorite | active DTO/registry/GUI 已实现 | `projects.setFavorite(projectId,favorite,expectedRevision,idempotencyKey)`；registered Project DTO兼容可选 `favorite` | `projects.manage` | `projects.favorite`；不进入 observation JSON | 无 | P5-B / implemented + remote green |
| 清除项目启动配置 | 项目单独保存启动参数 | `unity.projectLaunchConfig.clear` | `unity.manage` | 项目启动参数 | 无 | 已接入 |
| Open Unity / Backup | 已有真实 RPC/Operation | 保持 existing wiring；不是新合同 | existing | 无 | existing | 已实现证据保持 |
| Remove registry row | `projects.unregister` 足够 | GUI confirmation + existing RPC；不删除目录 | `projects.manage` | existing | 无 | P3 / S |
| Delete Project Directory | 已完成并通过远端验收 | active `projects.delete.v1`、ProjectId-only Plan/Apply、sibling quarantine permanent-delete、writer/protected-root/path gate 与 forward recovery | `projects.delete`；authenticated interactive caller；4.0.0 不授予 extension | 项目删除计划、日志及外键 | 无新增依赖；三平台 mount-safe cleanup 已验证 | P7 / implemented + remote green |
| Package workspace refresh | 单 repo `repositories.refresh` 足够 | GUI 完整分页 list，顺序逐 registered repo refresh，逐项记录 partial failure，`revision_conflict` 仅 fresh retry 一次，最后 reload；不声称 atomic refresh-all | `repositories.manage/read` | 无 | existing HTTP/local | P6-A / implemented + remote green |
| Package source filter | registered local/remote repository DTO 足够做展示过滤 | GUI-only All/Remote/Local filter，source identity只来自 daemon `RepositorySource.kind`；GUI不做 resolver | read only | 不持久化；无 authority state | 无 | P6-A / implemented + remote green |
| 显示预发布版本 | Core 按设置过滤候选 | 默认显示，优先兼容稳定版 | settings.read/manage | Config1 | 无 | 已接入 |
| 隐藏仓库来源 | Core 校验来源身份 | 候选与求解读取排除关闭的来源，仓库管理清单显示实际收录内容 | settings.read/manage | Config1 | 无 | 已接入 |
| User Packages | v3 是 `userPackageFolders` loose directory，不是local repository | implemented directory-only enrollment、opaque file identity、deterministic owned cache archive、带 `kind` 的来源证据、refresh/remove CAS；不接受archive input | packages.read/manage + local-owner | State1 的 `user_package_sources` | 复用现有semver/sha2/zip/platform；无新依赖 | P6-C / implemented + remote green |
| Reinstall one/all | existing Plan action没有 reinstall；install same version保持no-op | implemented `packages.planReinstall`，`packages.plan.v1`、`packages|all` tagged selection、max 256，locked exact version重新解析source并force replace | existing packages read/manage + projects/repositories read | Plan action=`reinstall` | 无新依赖 | P6-C / implemented + remote green |
| Bulk selected install/upgrade/remove/reinstall | ChangeSet可多 mutation，new request表达显式混合集合 | implemented `packages.planBulk`，`packages.plan.v1`、max 256 unique typed intents，单 durable Plan/Apply；禁止客户端拼接多个 Plan | existing package permissions | Plan action=`bulk` | 无 | P6-C / implemented + remote green |
| 关闭的软件包 | Core 按仓库和包 ID 过滤来源 | GUI 呈现 Core 返回的候选；撤回版本不参与自动安装 | settings.read | Config1 | 无 | 已接入 |
| Docs / changelog | normalized read DTO已保留验证后的真实 `documentationUrl`/`changelogUrl` | implemented 2,048-byte validated optional descriptors；GUI closed command只接受 `(repositoryId,packageId,version,linkKind)`，daemon re-fetch current link，拒绝非 http/https/userinfo | repositories/packages read | State1 nullable URL columns | 复用reqwest URL与open；无新依赖 | P6-B / implemented + remote green |
| VCC Import / Migrate | 不足 | M11 migration-only implementation +真实脱敏 Fixture | M11决定 | M11 | M11 | M11 / blocked |

## 关键判断

1. Create、Restore、逐 repository refresh 不需要新 Core 方法；缺口是 GUI flow/native chooser。
2. `unity.launchOptions` 返回当前项目可用的 Unity 安装；项目启动参数通过 `unity.projectLaunchConfig.get/set/clear` 管理。
3. `packages.planReinstall` 和 `packages.planBulk` 覆盖重装和混合批量意图。GUI 确认并应用同一份 Core 计划。
4. prerelease 不应由 TypeScript 解析 version string；SemVer authority继续留在 Core，DTO 提供 Core 计算的布尔分类。
5. v3 的 “hidden” 包含 hidden repository、local user package visibility和折叠后的不可用来源，不是一个可由 CSS 隐藏代替的
   单一概念。
6. docs/changelog URL 来自不可信 repository manifest；React 不得把任意 URL传给 generic opener。
8. 项目收藏由 Core 持久化。请求 `projects.list` 视图时，Core 在分页前将收藏项目排在前面，再应用用户选择的排序。
9. v3 清除 Unity 路径时保留参数。当前 Core 依据项目版本解析已验证的安装；项目启动参数单独持久化。

## Release / milestone ownership

- 除明确属于 M11 的 migration/differential parity 外，已知 Project/Package 缺口均属于 M7 functional closure；未完成前
  `projects.management` / `packages.vpm` 不得提升为 implemented。
- P7 Remove Directory 是 M7 内已完成的 high-impact slice；选择永久 sibling quarantine，而不是 OS Trash 或直接就地
  `remove_dir_all`，完整合同与远端证据见 `docs/baselines/m7-p7-project-directory-delete.md`。
- VCC import/migrate 与真实 v3 differential evidence属于 M11，保持 blocked。
- Win10/Win11完整客户端安装/运行/WebView2/更新/卸载仍属于 M12。
