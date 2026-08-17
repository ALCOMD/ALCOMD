# 冻结版本 vrc-get 细粒度行为审计

状态：静态参考审计完成；不对冻结实现继续执行攻击性黑盒

审计提交：`14d73d018bdc90c5b005064eb10f8e9714fa8409`

最后更新：2026-08-16

## 审计边界

本审计只提取用户能力、公开格式兼容语义、安全边界和可复现测试输入。缓存布局、内部容器、
错误文案、临时目录、GUI command 拆分和自更新器实现均不构成 v4 实现约束。ALCOMD 不复制、
包装、移植或改写 vrc-get / vrc-get-vpm 源码。

## CLI 用户入口

| Feature ID | 命令/行为 | 主要证据 |
|---|---|---|
| `cli.package.install` | 安装指定包/版本，支持 prerelease、project、offline、no-update、yes；省略包等同 resolve | `vrc-get/src/commands.rs`：`Install::run` |
| `cli.package.resolve` | 按 locked/dependency 状态补齐或重装依赖 | `vrc-get/src/commands.rs`：`Resolve::run` |
| `cli.package.remove` | 移除一个或多个包并显示冲突、legacy 与 unused 变化 | `vrc-get/src/commands.rs`：`Remove::run` |
| `cli.package.reinstall` | 按 locked 精确版本重新获取并安装 | `vrc-get/src/commands.rs`：`Reinstall::run` |
| `cli.package.outdated` | 查询 locked 包可升级版本，受已装包依赖和 prerelease 约束 | `vrc-get/src/commands.rs`：`Outdated::run` |
| `cli.package.upgrade` | 单包或全部 locked 包升级，不新增 direct dependency | `vrc-get/src/commands.rs`：`Upgrade::run` |
| `cli.package.downgrade` | 指定 locked 包与目标版本降级 | `vrc-get/src/commands.rs`：`Downgrade::run` |
| `cli.package.search` | 多查询 AND，搜索名称、显示名和描述 | `vrc-get/src/commands.rs`：`Search::run` |
| `cli.repo.list` | 列出用户仓库 | `vrc-get/src/commands.rs`：`RepoList::run` |
| `cli.repo.add` | 添加远程 URL 或本地路径，支持名称和请求 header | `vrc-get/src/commands.rs`：`RepoAdd::run` |
| `cli.repo.remove` | 按 id/url/name/path 查找并删除仓库 | `vrc-get/src/commands.rs`：`RepoRemove::run` |
| `cli.repo.refresh` | 刷新全部仓库缓存，单仓库错误可能只记录日志 | `vrc-get/src/commands.rs`：`Update::run` |
| `cli.repo.cleanup` | 删除 Repos 直接子级中未引用的 JSON cache | `vrc-get/src/environment.rs`：`cleanup_repos_folder` |
| `cli.repo.packages` | 按 URL、仓库名或 id 查看包与版本 | `vrc-get/src/commands.rs`：`RepoPackages::run` |
| `cli.repo.import` | 逐行导入 http(s) 或 `vcc://vpm/addRepo` | `vrc-get/src/commands.rs`：`RepoImport::run`；`vrc-get-vpm/src/repositories_file.rs` |
| `cli.repo.export` | 导出远程仓库；带 header 时生成 vcc URI | `vrc-get/src/commands.rs`：`RepoExport::run` |
| `cli.user-package.*` | 列出、添加和移除本地 loose package | `vrc-get/src/commands.rs`：`UserPackageList/Add/Remove::run` |
| `cli.info.project` | human 或 JSON v1 项目信息 | `vrc-get/src/info.rs`：`Project::run` |
| `cli.info.package` | JSON v1 包版本/yanked 信息 | `vrc-get/src/info.rs`：`Package::run` |
| `cli.migrate.unity2022` | 交互式原地迁移并调用 Unity | `vrc-get/src/commands/migrate.rs`：`Unity2022::run` |
| `cli.migrate.vpm` | 交互式原地 VPM 迁移 | `vrc-get/src/commands/migrate.rs`：`Vpm::run` |
| `cli.cache.clear` | 删除包 ZIP 与 sidecar cache | `vrc-get/src/commands.rs`：`CacheClear::run` |
| `cli.completion` | 显式或环境推断 shell completion | `vrc-get/src/commands.rs`：`Completion::run` |
| `cli.vcc.experimental` | feature-gated VCC project/Unity 命令 | `vrc-get/src/commands/vcc.rs` |

ALCOMD CLI 不继承当前确认机制和输出缺口：必须统一支持 `--json`、`--ndjson`、`--quiet`、
`--dry-run` 与非交互模式；stdout 只输出结果，日志/进度进入 stderr，稳定退出码形成公共契约。

## 项目与 VPM 行为

| Feature ID | 已观察行为 | 主要证据 |
|---|---|---|
| `projects.discovery` | 从当前目录向父级查找 VPM manifest，其次 UPM manifest；显式 project 路径不向上搜索 | `vrc-get-vpm/src/io/tokio.rs`：`DefaultProjectIo::find_project_parent` |
| `projects.load-manifests` | 并发加载 VPM/UPM、枚举 Packages、区分 locked/unlocked、读取 Unity 版本 | `vrc-get-vpm/src/unity_project.rs`：`UnityProject::load` |
| `projects.detect-type` | 按 locked SDK、UPM 与 legacy DLL 识别 Worlds/Avatars/Legacy/Unknown | `vrc-get-vpm/src/unity_project/project_type.rs` |
| `unity.version-detection` | 从 `ProjectVersion.txt` 解析版本与可选 revision | `vrc-get-vpm/src/unity_project.rs`：`read_unity_version` |
| `packages.manifest.compat` | strict/loose 两档解析 package manifest 与 VPM 可选字段 | `vrc-get-vpm/src/package_manifest/mod.rs` |
| `repositories.format` | 解析 repository 元数据和 `packages[name].versions[version]`，坏单包可隔离跳过 | `vrc-get-vpm/src/repository/remote.rs` |
| `versions.vpm-ranges` | 支持 exact、bare、caret、tilde、hyphen、OR、prerelease 与范围交集语义 | `vrc-get-vpm/src/version/` |
| `packages.version-selection` | yanked、prerelease、Unity compatibility 和特定 VRChat SDK 策略影响候选 | `vrc-get-vpm/src/version_selector.rs`；`vrc-get-vpm/src/lib.rs` |
| `repositories.sources-cache` | 默认/用户仓库、cache、ETag 200/304、失败保留旧 cache | `vrc-get-vpm/src/environment/repo_holder.rs` |
| `repositories.identity-dedup` | 按 URL/id 去重，设置顺序保留首项 | `vrc-get-vpm/src/settings.rs` |
| `repositories.precedence` | 跨仓库按版本选择；同版本来源没有已确认的确定性优先级 | `vrc-get-vpm/src/environment/package_collection.rs` |
| `packages.resolve` | 合并 direct、locked、unlocked 依赖并生成传递依赖、冲突、legacy 与 missing 结果 | `vrc-get-vpm/src/unity_project/package_resolution.rs` |
| `packages.plan` | 先收集 Install/Remove、manifest、legacy 和冲突变化，再 apply | `vrc-get-vpm/src/unity_project/pending_project_changes.rs` |
| `packages.apply-transaction` | staging 解压、rename 包目录、分别保存 VPM/UPM manifest、最后清理 legacy | `vrc-get-vpm/src/unity_project.rs`：`apply_pending_changes` |
| `packages.download-cache` | ZIP+SHA sidecar cache，命中时重算摘要，offline 仅使用有效 cache | `vrc-get-vpm/src/environment/package_installer.rs` |
| `packages.archive-safety` | ZIP entry 必须 UTF-8，拒绝绝对路径与 parent traversal | `vrc-get-vpm/src/utils/extract_zip.rs` |
| `packages.legacy-cleanup-safety` | legacy 路径必须位于 Assets/Packages 且 component 正常 | `vrc-get-vpm/src/unity_project/find_legacy_assets.rs` |
| `projects.manifest-write` | 单文件临时写、flush/sync/rename，保留未知 UPM 顶层字段 | `vrc-get-vpm/src/utils/save_json.rs`；`vrc-get-vpm/src/io/tokio.rs` |

公开兼容范围是 repository/package/VPM/UPM/Unity 可观察格式与生态行为，不包括
`VRChatCreatorCompanion` 数据路径、`Repos/vrc-get-*.zip` 布局、HashMap 顺序或 Temp 目录实现。

## GUI 能力补充

- 设置：首次设置流程、语言、主题、动画、紧凑模式、项目视图/排序、日志级别、Unity Hub、
  默认 Unity 参数、项目/备份路径、备份格式、排除 VPM 包、prerelease、频道和协议关联。
  证据：`vrc-get-gui/src/commands/environment/config.rs`、`settings.rs`。
- 项目：列表同步、添加、移除、删除目录、收藏、创建、复制和迁移副本。
  证据：`vrc-get-gui/src/commands/environment/projects.rs`。
- 包：详情、plan/apply、安装/升级/降级/移除/resolve 与结构化 missing-dependency 错误。
  证据：`vrc-get-gui/src/commands/project.rs`、`state/changes.rs`。
- Unity：检测、Hub 同步、选择、启动、进程状态、项目级路径/参数和两类迁移。
- 备份：创建 ZIP、压缩级别、排除 VPM 包与进度；冻结 handlers 中未找到 restore 入口，
  因此 `backups.restore` 不能仅凭该基线标记为存在。
- 仓库与本地包：列表、刷新、显隐、排序、预览、导入/导出、cache clear、deep link。
- 模板：内建/用户模板、保存、移除、导入冲突覆盖和导出。
- 日志/关于/更新：日志页、licenses、版本、受限外链和按平台签名更新。

## 不得继承的缺陷与负向要求

| 优先级 | 发现 | v4 验收要求 |
|---|---|---|
| P0 | 包名直接参与 cache、staging 与 Packages 路径，可能形成 traversal；尚待恶意 Fixture 确认 | 包 ID 语法验证，最终路径必须保持在受控根目录内 |
| P0 | 没有项目级跨进程锁和持久 recovery journal | 同项目写串行、不同项目可并行，崩溃后恢复或回滚 |
| P1 | VPM/UPM 两份 manifest 分别原子写但不是统一事务 | 包目录与两份 manifest 必须作为同一可恢复事务 |
| P1 | 缺失或格式错误的 `zipSHA256` 会静默降级为本地 sidecar | 声明哈希格式错误或与实际不符必须拒绝 |
| P1 | repository/package headers 明文持久化，并可能由 export 输出到 stdout | 凭据进入 OS store，日志和导出脱敏，跨 origin redirect 不泄露 |
| P1 | 非 TTY stdin EOF 后确认循环可能永久等待 | 非 TTY 不进入交互；明确失败或要求 `--yes` |
| P1 | 查询加载项目时会修复并写回旧 manifest 值 | Query 不得写项目；修复必须显式 Plan/Apply |
| P2 | 本地 loose package 遇 symlink/特殊文件可能 panic | 明确拒绝、结构化错误、不得 panic 或逃逸 |
| P2 | 同版本跨仓库来源可能受无序容器影响 | 冻结确定性来源优先级并做重复运行测试 |
| P2 | legacy 删除错误只记录日志，可能部分成功且不可回滚 | ChangeSet 明示删除，失败进入事务恢复或结构化 partial result |

## ALCOMD 实现必须覆盖的验收测试

以下项目是从参考实现提取的 ALCOMD 防御性测试输入，不要求对冻结 vrc-get 本身重新执行
攻击性黑盒、网络安全或故障注入。测试对象是后续 ALCOMD 独立实现：

1. CLI help/alias/feature 快照、stdout/stderr/JSON/退出码和关闭 stdin 的超时测试。
2. repository BOM/null/坏单包/key mismatch/ETag/部分失败/重复/等版本来源 Fixture。
3. 公开版本范围跨实现 Fixture。
4. 项目发现、manifest、Unity 版本、locked/unlocked 与只读无写入测试。
5. resolver golden、下载/哈希/cache/offline、恶意 ZIP 和包 ID Fixture。
6. 每个 extract/rename/manifest save/legacy delete 的故障注入及进程终止恢复。
7. 同项目读写/写写、不同项目写、刷新/install 和 clear/download 并发。
8. symlink、junction、循环和特殊文件；三平台 Unity/路径/协议/更新差异。
9. 错签名、错误资产、redirect、安装路径欺骗、跨设备和提权 helper 摘要错误。

## 静态风险参考边界

- 包名路径穿越、同版本来源顺序、ZIP symlink 属性、Windows rename 覆盖语义与跨 origin
  Authorization header 只作为源码观察到的风险输入，不声称已经在冻结二进制中动态复现。
  ALCOMD 不继承这些不确定行为，必须自行定义安全、确定且可测试的结果。
- `RemovePackageErr::ConflictsWith` 在冻结源码未找到可观察构造路径，不能只凭枚举标记为功能。
- 冻结仓库现有测试不足以证明事务、下载、ZIP、并发和崩溃安全；ALCOMD 必须建立自己的测试。
