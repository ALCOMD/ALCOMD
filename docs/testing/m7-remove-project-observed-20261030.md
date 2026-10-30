# M7 H2-A：移除项目观察时间

所有者明确批准彻底删除项目 `observedAtMs`。标准目录 `main`、HEAD `7d938cf2845cd982c25b12f106a8dac33981c8e7` 上实施，保留既有最后修改时间、三语言与 Noto WIP，未提交／推送。Visual Gate 2 仍 PENDING。

## 当前有效合同

项目 `ProjectObservation` 不再产生或保存通用采集时间；VPM reader 不接收时钟参数。`ProjectSnapshot`、RPC adapter、GUI 类型和项目 fixture 移除 `observedAtMs`，没有旧名别名、fallback、双模型或新 method/capability/permission。公共字段删除是所有者明确批准的发布前破坏性变化，沿用既有 DTO 未知字段策略，未额外收紧所有响应。

`ProjectRecord.last_modified_at_ms`／`ProjectSnapshot.lastModifiedAtMs` 及上轮冻结的注册、Unity 启动接受、包与迁移提交时间边界保留。普通刷新直接比较完整项目观察内容；不变内容不推进 revision、最后修改时间或候选证据。删除仅为观察时间清零而存在的比较 helper 与恢复测试抹平逻辑，不增加另一套时间模型。

State 15→16 同一事务执行：

- 物理删除 `projects.observed_at_ms`。
- 清理 projects snapshot、注册／刷新／收藏永久幂等响应、copy/delete/migration Plan 项目快照与对应 Plan 响应、copy journal 发布项目中的旧 snake_case 字段。精确 JSON 路径见 `specs/storage/state-v16-remove-project-observed.contract.json`。
- 事务内暂撤后按原定义恢复 copy/delete immutable 与 copy journal append-only 触发器；不取消原保护。
- Unity migration 的旧 authority hash 覆盖过该字段，因此旧未 Apply Plan 设为过期，并同步其幂等 Plan 响应；必须重新规划。不重签已批准哈希。已 Apply 的操作保留原哈希、操作绑定及恢复证据。
- copy/delete hash 不依赖完整 ProjectRecord，仅清理其项目快照，不改变原 authority hash。旧候选 fingerprint/cursor 因有效模型变化，走现有 `evidence_stale`，要求重读。

Unity installation 扫描时间、copy/delete writer 的公共 `observedAtMs`、内部 `checked_at_ms` 和迁移专有 `reobserved_at_ms` 均属独立证据，保留；迁移重检时间在目标版本／marker 验证成功边界采集。历史 SQL 及固定旧版本迁移测试保留，以证明真实旧库可升级，不代表运行时继续支持旧项目字段。

不修改 resolver、来源优先级、Plan/Apply 验证规则或恢复权限；没有新增依赖、unsafe、平台 API、配置项或 GUI 布局／图标／CSS 改动。

## 验证记录

项目浏览器专项 **10/10**，退出码 0；覆盖旧字段缺失、无变化刷新、最后修改时间排序、列表／网格、三语言和启动完成后的 Core 重读。frontend check/build 退出码 0，保留既有大 chunk 构建告警。fmt、全 Workspace Clippy、baseline freeze 检查通过。

最终 `cargo test --workspace --no-fail-fast -- --test-threads=1` 退出码 **0**：**427 passed / 0 failed / 8 ignored**，76 组结果。8 个 ignored 均为既有 kill/restart 父测试调用的 subprocess 入口，未新增跳过。包含补强后的真实历史 snake_case JSON 和升级后原始 JSON 物理缺字段断言；不依赖 serde 忽略未知字段来证明清理。

覆盖真实完整 Schema 15→16、安装／writer／迁移证据保留、旧 Plan 过期不重签、不可变触发器原文及拒改保护、事务末端失败回滚、v10 永久幂等响应升级重放、无变化刷新、候选／迁移 authority hash 与真实磁盘重检。初轮存储专项 50/50，最终 Workspace 覆盖完整最终源码，未出现失败重试。

`cargo fmt --all -- --check`、最终 `cargo clippy --workspace --all-targets -- -D warnings`、`cargo xtask check`、metadata、baseline freeze、两份存储合同与 RPC JSON 解析及 diff check 均通过。Cargo.lock／package-lock.json 无变动。

所有本轮日志保存在 `target/m7-remove-project-observed-20261030/`，不覆盖上轮最后修改时间验证记录。没有取得新的真实 Tauri 人工视觉或屏幕阅读器证据；不宣布 H2-A／M7 完成。


## 恢复的隔离 dev

仅暂停／恢复已核实属于本轮的 dev 树和 daemon，未关闭 Unity／v3 或归属不明进程。必要 daemon／CLI 构建为 dev（unoptimized + debuginfo），继续使用原获准 `target/m7-icon-acceptance-20260906/data`。最新 daemon PID 50364，CLI 只读确认 ready 和两个原有项目仍为 revision 1，项目响应无 `observedAtMs`，旧记录最后修改时间仍未知。

另以验证工具只读检查隔离数据库（不是 GUI 产品路径）：`PRAGMA user_version = 16`，projects 无观察时间列，项目 snapshot 无旧 key，原两个项目保留；升级由唯一状态持有者 daemon 执行，验证工具未写库。摘要及检查结果见 `native-session.json`、`native-status.json`、`native-projects.json` 与 `native-storage-check.json`。Tauri dev 已恢复，GUI PID 57152、窗口响应正常，前端来自当前工作区 Vite 1420；不将窗口就绪当成桌面验收。

- daemon SHA-256：`35D3AF599BEB733AFC5A5B1C6AED12B93A8EBCA397A83A9D19633F71432E9899`。
- CLI SHA-256：`83FE54937AF13BC1B86AC9025736B7BCA243C9F3EC093A6EF84C98F40E38953F`。
- GUI SHA-256：`443EA449E59C36FC32D1C6C1B4C8BDB2A6134538BE2966FE35F51371D0586697`。
