# M7 H2-A：项目最后修改时间

所有者明确要求按冻结 v3 的语义补回真实最后修改时间，并同步列表、网格及排序。标准目录 `main`、`7d938cf` 基线上实施；保留既有三语言与 Noto WIP，未提交／推送。Visual Gate 2 **PENDING**。

## 权威语义

v3 的 `LastModified` 是 registry 维护的最近使用／成功变更时间，不是文件树 mtime，也不是最近扫描时间。只读参考 `vrc-get-gui/src/commands/project.rs`、`commands/environment/projects.rs`、`backend/projects.rs` 和 `vrc-get-vpm/src/environment/project_management.rs`，未复制实现或引入运行时依赖。精确合同在当前 M7 ExecPlan、`specs/rpc/alcomd-rpc-v1.md`、M3 RPC Schema 及 `specs/storage/state-v15-project-last-modified.contract.json`。

| 边界 | 更新时间 |
|---|---|
| 首次成功注册项目 | 与首次注册时间同时初始化 |
| 创建／复制／恢复新目标 | 目标首次注册事务初始化，源项目不变 |
| 成功软件包 Apply | 与项目提交同一事务更新 |
| Unity 迁移 | 项目提交事务更新，收尾／恢复调度不另行更新 |
| Unity 启动 | 通过既有前置校验后，首次接受启动的事务更新，发生在 spawn 前；接受后的 spawn 失败仍保留该时间 |
| 检查／刷新，包括观察到外部内容改变 | 不更新，只更新既有观察／revision 语义 |
| 收藏、启动参数、仓库刷新、备份、导出 | 不更新 |
| 候选读取、Plan、Review、取消、前置拒绝 | 不更新 |
| 永久幂等重放、启动状态轮询、恢复收尾 | 不更新；已注册的恢复目标保留首次提交时间 |

采用 `MAX(existing, boundaryTime)` 防时钟回退／迟到事务覆盖。时间本身不推进项目 revision；候选快照与 Unity migration authority hash 在真实生产序列化边界排除新字段，其余校验与指纹内容保持。已有 writer gate、resolver、来源优先级及 Plan/Apply 不变。

## 数据与界面

- `ProjectRecord.last_modified_at_ms: Option<u64>` 由 Core 独立维护。项目观察模型已按后续明确指令移除通用采集时间，当前存储升至 State 16；删除范围和新验证见 `m7-remove-project-observed-20261030.md`。旧 durable Plan／永久幂等 JSON 缺字段读取为未知，None 省略序列化；不加旧方法别名或另一套时间模型。
- 现有 `ProjectSnapshot.lastModifiedAtMs` 为可选 Unix 毫秒值。注册／列表／get／refresh 等统一转换；未注册 inspect 及未知值省略，无新 method、capability 或 permission。
- Schema 14→15 只为 `projects` 增加 nullable INTEGER 列，原数据及其他表不变。NULL 表示未知，0 是合法时间，不回填观察时间、数据库更新时间或注册日期；负值及越界拒绝，升级事务可回滚。
- 列表表头、网格日期及排序统一采用最后修改时间，三语言按 v3 对应词条。收藏优先；其内未知日期始终在已知之后，时间相同按 ProjectId／路径稳定决胜。GUI 不以本机时钟、检查时间或注册日期补值。
- 真正 launch RPC 成功或失败完成后，经显式 `onLaunchSettled` 回调重读 Core。launch-options 失败不调用该回调，无自动 mutation 重试，原错误反馈保留。
- 无新依赖、unsafe、平台 API、配置项、图标、布局、CSS 或窄窗口适配。

## 验证

最终 `cargo test --workspace --no-fail-fast -- --test-threads=1` 退出码 **0**，汇总 **425 passed / 0 failed / 8 ignored**。8 项是原有 kill/restart 矩阵调用的 subprocess entrypoint，保留原标记，不称为新增测试全部单独执行。覆盖真实 M3 注册／读取／刷新、M4 包提交与失败、M5 启动与创建／恢复、Copy／Migration 的提交和恢复，以及协议和存储合同。

本次新增／扩展重点：

- 不同注册／观察／数据库时间及收藏、内容变化刷新，不得污染修改时间。
- launch 接受即更新、spawn 未接受标记、失败／轮询／重放不再 touch、回退时钟保留较新值、事务失败不消耗幂等键。
- 升级旧库保持未知与全部旧字段／表，0／最大值／非法值、DDL 失败回滚。
- candidate 返回真实新 metadata，但 fingerprint／revision／观察内容不变；migration 实际 authority hash 对不同或未知修改时间不变，仍对项目 revision 敏感。
- 包 Plan／失败不更新时间，成功 Apply 与完成时间对应；创建／复制／恢复只初始化目标，重放与恢复保持已提交时间。

`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo xtask check`、`cargo metadata --no-deps --format-version 1`、`scripts/freeze-baselines.ps1 -Check`、frontend check/build 与 diff check 均通过。构建保留原大 chunk 警告；Cargo.lock／package-lock.json 无新增依赖。

失败证据保留：首次沙箱 Clippy 因 Cargo 缓存目录拒绝访问，真实用户环境重试后新增测试有一处 moved-value，修正 clone 后通过；初轮 Workspace 新断言误将既有 migration commit 的观察时间 31 预期为 30，修正测试并保留原生产行为。随后旧可见入口源码断言不识别三语言 `t(...)`，更新正／反断言的匹配语法，不改完成度判据或入口功能。最终全覆盖 Workspace 通过，历史检查点和验收要求没有改写。

浏览器综合 **32/33**，其中新最后修改时间 **10/10**；覆盖检查时间刻意逆序且每次刷新改变、未知／相同时间、收藏置顶、升降序、列表／网格一致、三语言、启动成功／失败后的 Core 重读和错误保留。唯一失败为网格更多菜单 Escape 焦点恢复，保持代码与断言不变单独复测 **1/1**，退出码 0。不记作同轮 33/33，也不据此证明真实桌面菜单或无障碍已通过。各轮用例完成后清理确认属于本轮的 4173 Vite 才能结束本机收尾，服务终止日志保留。

所有输出在 `target/m7-last-modified-20261030/`：`workspace-complete.log/.exit`、`clippy-sealed.*`、`fmt-final.*`、`xtask.*`、`metadata.*`、`baseline-freeze.*`、`check-build.exit`、`browser/` 与 `grid-focus-check/`。初轮及中间失败日志同目录保留，不聚合为额外通过次数。

## 恢复的本地 dev

重建必要 daemon／CLI（dev profile，unoptimized + debuginfo），通过原获准 `target/m7-icon-acceptance-20260906/data` 启动并恢复 `npm run gui:dev`。只暂停和恢复确认归属的本轮测试进程，原生 RPC 测试期间保持每用户端点隔离；未关闭用户 Unity／v3 或未知进程。

- daemon PID 45028，SHA-256 `699521469BE2B2EBAAD2750415D62DAC6613C0A31F4B5ECCA53011E647EA2E7C`。
- CLI SHA-256 `6F39A2B5EB5EB3CCAC65B91B3AA7ED8F340984641B1745706626C581E8778E92`。
- GUI PID 43932，SHA-256 `1277A3D29F259A92322527AA357D5B095381E283A673FEDFEBA1F1000EF0D19C`，窗口响应正常；前端来自当前工作区 Vite 1420。

启动／构建日志及 `native-session.json` 在同目录。CLI 只读确认 ready、原隔离项目仍为 revision 1，旧记录省略 lastModifiedAtMs，符合无回填规则。没有对真实用户工程执行包 Apply／迁移／复制／删除，也没有执行真实 Unity 启动。本轮全 Workspace 命令通过，不沿用旧轮次的本机环境阻塞结论；人工视觉和屏幕阅读器仍未取得新证据，H2-A／Visual Gate 2 不关闭。
