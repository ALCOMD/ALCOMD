# M5：完整 CLI 合同与本地项目工作流

状态：草案；M4 已完成人工验收，M5 生产实现尚未获批、尚未开始

## 目标

在 M1-M4 已完成的 RPC、Operation、Event、Revision、Resource Lock、Project/Repository read model
与 package transaction 上，交付一组可由真实本地用户从 CLI 端到端完成的核心工作流：

```text
alcomd-cli
    -> ALCOMD RPC v1
        -> alcomd-application
            -> Project / Repository / Package
            -> Unity discovery / selection / launch
            -> Template registry / project creation
            -> Backup create / restore
                -> existing Operation / lock / archive / filesystem journal primitives
```

M5 首先冻结所有 CLI 命令共用的 human/JSON/NDJSON、stdout/stderr、退出码、交互确认和 Operation
跟随合同，再依次完成 Unity、模板、备份垂直切片。每个切片必须先有 RPC/State/权限/故障恢复合同，
再接生产实现和 CLI。不得同时铺开四套半成品系统，也不得让 CLI 直接访问数据库或项目文件。

## 前置条件

- M4 最终提交 `20a86d674b480981d269088cf0615ffdcd9b8e70` 与 GitHub Actions run
  `32289522274` 已通过 Windows Server 2025、Ubuntu 22.04、macOS 15 arm64 和项目所有者人工验收。
- M1 RPC v1 只能兼容增加；M2 Operation/Event/Revision/idempotency/Resource Lock/recovery 语义继续
  是唯一状态基础。
- M3 Project/Repository identity、只读解析与 anonymous/no-proxy repository 边界继续复用。
- M4 immutable Plan/Apply、SHA-256 cache、bounded archive、Project lock、filesystem journal 与真实
  kill/restart recovery 是项目文件写入的唯一基础；M5 不建立第二套事务框架。
- `projects.v3-parity`、`templates.v3-parity`、`backups.v3-parity`、`unity.v3-vrc-parity` 所需的
  M11 真实脱敏 Fixture 尚不存在，继续保持 `blocked`。
- repository credential enrollment/revocation 尚未实现。M5 默认只处理 local 与 anonymous HTTP(S)
  source；userinfo、自定义 header、token 和认证导入必须 fail closed。

## 最小交付物

1. 版本化 CLI contract：命令/alias、三种输出模式、stdout/stderr、退出码、确认、TTY/EOF、Operation
   跟随/取消和 completion。
2. Project/Repository/Package 已有 RPC 的完整 CLI 表面，以及 M5 实际补齐的本地管理 RPC。
3. 三平台 Unity Hub/Editor 只读发现、手工 Editor 注册、项目 Editor/参数选择、启动与诚实的进程状态。
4. versioned template registry、内建/导入/派生模板和从模板创建项目的 Plan/Apply Operation。
5. backup create 与高影响 restore：bounded archive、Plan/Apply、ProjectRestore lock、staging、journal、
   rollback、取消和真实 kill/restart recovery。
6. 最小 State Schema v4 和 RPC v1 兼容新增；不为未来 GUI/MCP/扩展预建字段。
7. public/synthetic engineering Fixture、CLI subprocess golden、跨平台 process/filesystem 测试和三个
   hosted 平台验收；真实 v3 differential 项保持 blocked。

## 完成定义

- CLI 的每个已发布命令均只调用 `alcomd-client`，且能由 RPC 监控测试证明没有直接打开 state.db、
  repository/package cache 或 Unity 项目文件。
- human/JSON/NDJSON、stdout/stderr、退出码和 alias 形成版本化快照；非 TTY 与关闭 stdin 永不等待，
  `--yes` 不绕过 stale revision、权限、Unity writer 或 Plan revalidation。
- Project/Repository/Package、Template、Backup、Unity 每个 M5 命令均有真实 daemon integration test；
  不发布只返回 scaffold/unsupported 的假命令。
- Unity 正在使用项目的证据会触发明确 hard gate；证据缺失不被描述为“证明 Unity 未运行”。外部
  writer 造成的 fingerprint 变化稳定返回 `project_changed`/`plan_stale`，不静默重试。
- 模板创建和 backup restore 都复用已有 archive/path/Operation/filesystem journal 原语；项目写入
  crash 后只允许完整旧状态或完整新状态，不产生混合状态或虚假 succeeded。
- M5 engineering tests 可以使用 public/synthetic Fixture，但四项 v3 differential parity 继续 blocked
  到 M11；缺少真实 Unity/Hub 的 hosted 环境不得被描述为真实 Editor compatibility 证据。
- 本地完整门禁及 Windows Server 2025、Ubuntu 22.04、macOS 15 arm64 hosted CI 对最终提交通过；
  工作树干净，并停止在 M6 前等待人工验收。
- `packages.vpm`、`packages.transaction-safety`、`packages.security`、`projects.management`、
  `repositories.management` 与 `cli.complete` 只有在各自全部 feature-parity 入口真实完成时才可标记
  `implemented`；M5 里程碑完成本身不能自动提升这些聚合 feature。

## 明确不属于 M5

- Extension Runtime、WASI/WIT、Extension API 或第一方/第三方扩展宿主（M6）。
- 官方 GUI 产品页面、Material Design 3 UI、Tauri command 业务接线或 GUI parity（M7）。
- MCP 协议、MCP 管理扩展、Discord、Local API 或新的 SDK（M8+）。
- v3 数据迁移、bridge/bootstrap/updater、安装器、签名、dist 或零残留（M11-M12）。
- Windows 10/11 完整客户端安装、WebView2、注册表、升级和卸载验证；继续 deferred 到 M12。
- authenticated repository、credential store、header 导入/导出或 credential revocation。
- hashless remote VPM、legacy asset/GUID cleanup、Unity 2022/VPM project migration；没有单独获批合同前
  不借完整 CLI 名义带入。
- 通用 desktop automation、窗口控制框架、workflow engine、第二套 archive/download/filesystem
  transaction framework或通用任务 DSL。
- 依赖 Unity experimental CLI、已弃用 Hub CLI 或私有 Hub 数据库作为权威状态。

## 允许修改范围

contract-first 获批后，M5 只允许按垂直切片修改：

```text
apps/alcomd/                         # RPC adapter、Operation worker、M5 recovery 接线
apps/alcomd-cli/                     # CLI contract 与命令实现；只能依赖 client/protocol
crates/alcomd-domain/                # 纯 Unity/template/backup/plan identity 与不变量
crates/alcomd-application/           # 用例、ports、权限、锁、Plan/Apply 编排
crates/alcomd-protocol/              # 获批的 RPC v1 兼容 DTO
crates/alcomd-client/                # 类型化 RPC 调用与 Operation follow
crates/alcomd-store/                 # 获批的 Schema v4/migration/registry/journal
crates/alcomd-platform/              # 三平台发现、进程、路径/权限与启动原语
crates/alcomd-vpm/                   # 复用既有 bounded archive/project transaction adapter
crates/alcomd-testing/               # synthetic/public Fixture、CLI/daemon/kill tests
specs/rpc/                           # M5 RPC/error/output Schema 与兼容快照
specs/storage/                       # State Schema v4/migration
specs/extensions/permissions-v1.md   # 仅精化既有 M5 权限语义
docs/adr/                            # CLI、Unity writer、template/backup transaction ADR
docs/exec-plans/M5-cli-core-workflows.md
docs/status.md
docs/testing/test-plan.toml
feature-parity.toml
scripts/                             # M5 metadata/contract/platform gate
xtask/                               # 依赖方向、unsafe、Schema/CLI contract 门禁
.github/workflows/ci.yml             # 仅三平台 M5 验收命令
Cargo.toml / Cargo.lock              # 仅单独批准的精确依赖
```

内建模板资产的目录、许可证与发布归属必须先在人工作出的模板库存决策中冻结，不能在计划阶段任意
创建。M5 不修改 GUI、extensions、MCP、SDK、migration 或发行目录。

## 依赖方向与架构约束

- `apps/alcomd-cli -> alcomd-client -> alcomd-protocol`；CLI 不依赖 store/platform/vpm。
- `alcomd-application` 拥有确认后的 use case、Plan/Apply、权限、revision、idempotency 和锁策略；
  平台、archive、filesystem 实现只能作为 port adapter。
- `alcomd-domain` 不依赖 SQLite、ZIP、HTTP、Tauri、CLI、进程枚举或操作系统 API。
- `alcomd-platform` 只负责 Hub/Editor/进程/启动和已有安全 filesystem 原语，不持有 RPC DTO、模板
  manifest、backup policy 或业务规则。
- `alcomd-vpm` 可通过窄内部 adapter 复用已证明的 bounded ZIP/path/extraction 与 project transaction
  原语；不得复制一份 template ZIP 和 backup ZIP 实现。若现有模块边界确实无法复用，应先提交“提取
  单一共享 project-file adapter”的局部设计审批，而不是直接新增第二套实现。
- SQLite transaction 内不得等待进程、压缩、解压、fsync、Operation、Resource Lock 或用户确认。
- GUI/MCP/扩展未来调用与 CLI 使用相同 application 用例；M5 不为它们增加隐藏捷径。

## 实施顺序：顺序化垂直切片

### Slice 0：CLI 合同与只读命令

先冻结 CLI ADR、machine-readable Schema、help/alias snapshot 和错误映射。接通已有 system/project/
repository/package queries、Operation list/get/follow/cancel；此阶段不增加新的项目写入。

### Slice 1：Unity 与 Project writer gate

冻结 Unity installation registry、discovery evidence、项目选择、launch/status 与 `project_in_use` 语义。
先完成手工 Editor 注册和 ALCOMD 启动/跟踪，再增加受限 Hub discovery。writer gate 接入 M4 package
Apply 和后续所有 project mutation 后，才允许进入模板/恢复。

### Slice 2：Template registry 与创建项目

先实现只读内建/用户 template registry 和 import/export，再实现 derive/copy。最后通过 durable Plan 与
Operation 创建完整新项目；dependency resolution 必须在首次目标写入前完成，不能创建半项目后再
临时求解。

### Slice 3：Backup create

实现一致性前后 fingerprint、流式压缩、SHA-256、partial cleanup、Operation progress/cancel 和
ProjectBackup lock。Unity 使用中的项目默认拒绝创建“可恢复一致备份”，不把 best-effort ZIP 称为
一致备份。

### Slice 4：Backup restore

先支持空/新目标 restore，再支持已有目标的高影响 Plan/Apply。复用同一 filesystem journal 和真实
kill/restart gate，完成后才接 CLI confirmation/`--yes`。不得将 restore 实现为普通 unzip。

### Slice 5：CLI surface 收敛

在真实后端存在后补齐 project/repository/package/template/backup/unity 命令、completion 和所有
output/exit/TTY golden。没有后端的命令不进入 help；聚合 feature 状态按真实覆盖更新。

## 完整 CLI contract 草案

### 命令面

最终命名必须在 contract-first 阶段形成 snapshot。当前命名基线：

| 组 | M5 计划入口 |
|---|---|
| `system` | `status` |
| `operation` | `list`、`get`、`follow`、`cancel` |
| `project` | `inspect`、`list`、`get/info`、`add/register`、`refresh`、`remove/unregister`、`create`、`copy`、`delete`、`favorite`、`open` |
| `repository`（alias `repo`） | `inspect`、`list`、`get/info`、`packages`、`add/register`、`refresh`、`remove/unregister`、`hide/show`、`reorder`、`import/export`、`cleanup`、`cache clear` |
| `package` | `search`、`info`、`outdated`、`install`（alias `i`）、`resolve`、`remove`（alias `rm`）、`upgrade`、`downgrade`、`reinstall`、`apply-plan`、`cache clear`、`local list/add/remove` |
| `template` | `list`、`get/info`、`import`、`export`、`derive/copy`、`remove`、`favorite`、`create-project` |
| `backup` | `list`、`get/info`、`create`、`plan-restore`、`restore/apply-plan`、`remove` |
| `unity` | `list`、`refresh`、`register`、`remove`、`project get/set-editor/set-args`、`launch`、`status`；`activate` 仅在有可靠平台实现时加入 |
| `completion` | 显式 shell 参数生成静态 completion，不连接或启动 daemon |

上述表不是对所有命令生产实现的预先批准。hashless/legacy/credential/project migration 等被排除能力
不能通过同名命令暗中实现；如果某入口在 M5 后仍缺后端，`cli.complete` 保持 `in_progress`。

### 输出与退出

contract-first 阶段必须冻结以下候选规则并取得人工批准：

- human 为默认；`--json` 与 `--ndjson` 互斥。human 文案可本地化但关键字段/错误有 golden；JSON/
  NDJSON envelope 和字段是稳定机器合同。
- human 成功结果写 stdout；进度、确认提示和非结果诊断写 stderr。错误时 stdout 为空。
- JSON 成功只写一个 complete result envelope 到 stdout；调用前/transport/domain 错误只写一个 error
  envelope 到 stderr。不得混入日志、颜色、进度或提示。
- NDJSON 对同步命令写单个 result record；对 Operation 写 operation/progress/event/final result 或 final
  error typed records。stream 开始前的 CLI/transport 错误写 stderr；stream 开始后的终态错误作为最后
  一条 stdout record，随后以非零退出。
- 建议退出码：`0` 成功、`1` domain/Operation failure、`2` 参数/usage、`3` daemon/transport/protocol、
  `4` permission/confirmation/safety gate、`130` 用户中断跟随。最终数字必须由 CLI Schema snapshot
  冻结；不得按任意内部错误动态分配。
- `--quiet` 只抑制 human 成功摘要和进度，不抑制错误，也不删除 JSON/NDJSON 最终 record。
- `--no-progress` 只关闭进度显示；`--no-start-daemon` 继续沿用 M1 语义。

### 确认、TTY、EOF 与 Operation

- 高影响 shortcut 必须先产生并展示 immutable Plan，再确认精确 `planId/fingerprint` 后 Apply。
  `--yes` 只代替该确认，不跳过权限、expectedRevision、source pin、Unity writer gate 或 stale Plan。
- 只有 stdin 与用于 prompt 的 stderr 都是 TTY 时才允许交互。prompt 只写 stderr；closed stdin/EOF/
  非 TTY 立即返回 `confirmation_required`，不得循环或等待。
- mutation 默认等待 Operation 到终态；`--detach` 只返回 OperationId。`--wait` 可显式要求默认行为并
  与 `--detach` 互斥；`--no-progress` 不改变等待语义。
- Ctrl+C/客户端断开只停止 follow 并返回 130，不默认取消已经创建的 Operation。业务取消只能通过
  明确 `operation cancel`，其 ack 只表示已接受取消意图。
- `--dry-run` 候选语义是“完成真实只读解析并创建 durable immutable Plan，但不 Apply/不写项目”；
  它可能写入 Plan record，必须在帮助与 JSON 中明确，不得伪称零状态变化。若项目所有者要求完全
  无状态 dry-run，需要新的 RPC 合同并另行审批。
- shell completion 只由 clap command tree 静态生成；不读取项目/repository 动态名称，不触发 daemon。

## Unity / Unity Hub integration

### 发现与身份

- source 分为 `manual`、`hub_config`、`known_install_root`；同一 Editor 通过平台 file identity 去重，
  path string、显示名和版本不是权威身份。
- Hub/config 读取必须 bounded、只读、no-follow 并保留 last-known-good registry；unknown/malformed
  source 返回结构化诊断，不删除手工记录。
- version/revision 必须来自验证后的 Editor bundle/executable metadata；architecture 固定枚举
  `x86_64`/`arm64`/`universal`/`unknown`，unknown 不可偷偷当兼容。
- 项目首选 Editor 是显式 ProjectId -> EditorId 关系；项目 `ProjectVersion.txt` 继续是兼容校验输入。
  launch arguments 持久化为 bounded string list，不存 shell command line，不经 shell 解析。
- Unity 官方当前文档将独立 Unity CLI 标为 experimental，并将 Hub CLI 标为 deprecated；M5 不把
  两者当权威发现依赖。它们未来最多是经审批的可选 import source。

参考官方材料：

- <https://docs.unity.com/en-us/hub/cli-overview>
- <https://docs.unity.com/en-us/unity-cli/use-unity-cli>
- <https://docs.unity3d.com/cn/current/Manual/EditorCommandLineArguments.html>

### 启动与进程状态

- 直接执行已验证 Editor executable，并以独立 argv 传 `-projectPath <root>`；不得经 shell 拼接。
- launch 在 Project lock 下复验 project/editor identity 与 version compatibility，成功 spawn 后记录
  opaque launch ID、PID、start evidence 和 project/editor IDs。daemon 生命周期不拥有 Unity 生命期。
- `running/not_running/unknown` 必须携带 evidence kind。PID 必须与 executable identity/start time
  联合验证，避免 PID reuse；无法安全读取时返回 unknown，不猜测。
- 前台激活没有可靠的三平台统一语义，不是 M5 基础完成 blocker。只有 Windows/macOS/Linux 各有
  可测试且不依赖桌面自动化框架的实现时才兼容增加 `unity activate`；否则保持未发布。

### 外部 Unity writer 安全边界

- ALCOMD 自身跟踪到目标 ProjectId 的 live Unity process 是 hard gate。
- 目标内 Unity lock evidence（例如实际存在的 project lock）作为保守 hard gate；ALCOMD 不自动
  删除或按 mtime 猜测其 stale，也不允许 `--yes` 覆盖。其格式/可靠性必须由支持版本 Fixture 冻结。
- 只能观察到“系统上有 Unity 进程”但不能可靠关联目标时是 warning/advisory，不得误报目标正在运行。
- 没有观察到 process/lock 不证明 Unity 未运行。每个 destructive operation 仍须在 Plan、首次 mutation
  和 commit checkpoint 重验 project fingerprint；变化返回 `project_changed`/`plan_stale` 并进入已有
  rollback/recovery。
- ALCOMD 无法阻止用户或外部 Unity 在检查后开始写入。文档与错误必须明确这一局限，不能宣称任意
  Unity writer 与 ALCOMD transaction 被全局协调。

## Templates

- v4 template bundle 使用 versioned bounded manifest、content digest 和同一 bounded ZIP/path profile；
  v3 import 格式在 M11 Fixture 前保持 blocked，不用 synthetic 格式冒充兼容。
- registry source 为 built-in/imported/derived；保存 opaque TemplateId、版本、display fields、favorite、
  bundle digest 与 normalized dependency/additional-resource descriptors。raw archive 不进入 state.db。
- built-in template 的清单、内容来源和许可证必须单独人工确认；测试 synthetic template 不能自动变成
  产品内建资产。
- import 先完整 preflight/digest，再原子 publish object 和 registry。冲突必须显式 return
  `template_conflict`；override 是 Plan/Apply，不按文件名静默覆盖。
- export 从已验证 object 生成确定性 bundle；不得输出 credential、绝对项目路径或内部 DB metadata。
- derive/copy 对项目做 bounded include/exclude；symlink/reparse/special file fail closed。Library/Temp/log/
  credential 等默认排除策略须写入 manifest contract。
- create-project 在首次写入前解析 template dependency 和额外资源，生成 durable Plan。Apply 使用
  ProjectCreate(target identity) lock、同卷 staging、single journal 和已有 archive/transaction adapter；
  任何 package dependency 无法满足时不创建半项目。

## Backups

### Create

- 输入为已注册 ProjectId 和 expectedRevision；读取前后 fingerprint 必须一致。Unity writer hard gate
  命中时拒绝一致性备份。
- 流式写 ALCOMD backup root 内 operation-owned partial，使用现有 ZIP writer/profile、SHA-256、entry/
  total/path quota，flush/fsync 后原子 publish；取消/失败删除或保留 journal-owned partial。
- compression profile 只暴露 Frozen 枚举（例如 stored/fast/maximum），不把第三方 codec 参数变成
  公共合同。
- `exclude VPM packages` 的精确语义是人工审批点：必须冻结被排除目录、保留的 manifests、restore
  时 package resolve 前提和 offline/credential 失败行为，不能仅按目录名猜测。

### Restore

- restore 始终先 `backups.planRestore`：固定 archive digest、source project fingerprint、target identity、
  ChangeSet、expected target state 与冲突；Apply 返回 OperationId。
- 空/新 target 仍须路径/owner/no-follow/同卷校验。已有 target restore 是高影响写入，必须获取
  `ProjectRestore(target_identity)` 与必要 Project lock，且要求显式确认或 `--yes`。
- archive 先在 target 同卷 sibling staging 完整验证/extract；旧 target 先 rename 到 backup，新 target
  再 publish。复用 M4 intent -> mutation -> evidence -> completed journal 规则，最终 DB commit 前不得
  succeeded。
- crash/取消发生在旧 target rename、新 target publish、manifest validation、filesystem committed、
  state committed 各边界时，重启必须复用原 OperationId/Plan/幂等键恢复。证据完成前不得删除旧 target
  backup/staging。
- restore 目标存在 Unity writer hard-gate 时拒绝。不存在 evidence 仍不保证没有外部 writer，故每个
  checkpoint 继续做 target fingerprint/identity 检查。
- backup 不是普通 unzip；不支持链接、root escape、case/Unicode collision、special file 或未经批准
  的 codec。backup archive profile 可有不同 quota 参数，但必须调用同一安全 engine。

## State Schema v4 与 RPC v1

M5 的 durable registry 需求足以需要 Schema v4；候选最小表/变化为：

- `unity_installations`：EditorId、platform file identity、normalized version/revision/architecture、source、
  last-seen revision；path 只在内部加密/受限 DB 中保存，不进入普通 Event/error。
- `project_unity_preferences`：ProjectId、可选 EditorId、bounded argv list、revision。
- `templates`：TemplateId、source kind、normalized manifest、bundle digest、favorite、revision。
- `backups`：BackupId、可选 source ProjectId、archive digest/size、options、source fingerprint、created time、
  terminal state；in-flight 状态继续由 Operation/filesystem journal 表达。
- 兼容扩展现有 filesystem journal 的 versioned operation kind/evidence；不复制 package journal 表。

不新增 CLI 设置表、通用 workflow 表、Hub mirror 数据库、process history 日志或未来 GUI state。migration
必须覆盖 v1->v2->v3->v4、v3->v4、rollback、未知 future schema fail closed 和现有 M4 journal 保留。

RPC v1 候选兼容增加按领域拆分 capability 和 Schema：Project/Repository/Package query+management、
`templates.manage.v1`、`backups.manage.v1`、`unity.manage.v1`。准确 method、DTO、collection limit、错误码
和 capability 名必须在 contract-first 人工审批后冻结；新增 method/可选字段不提升 RPC major，删除/
改义才提升 major。

既有权限名优先直接精化：

- template query：`templates.read`；import/derive/remove/favorite/Plan/Apply：`templates.manage`，创建项目
  还需要 `projects.manage` 和相关 package read/manage scope。
- backup query：`backups.read`；create/remove/restore：`backups.manage`，restore 还需要目标 project write
  scope；不隐含任意路径写入。
- Unity discovery/query：`unity.read`；manual registry 与 project preference 是否归 `unity.read` 或新增
  manage 权限是人工决策点；launch/activate 使用 `unity.launch`。
- 所有 M5 外部文件写仍只对 `builtin:local-owner` 开放；真实外部 credential/revocation 未完成前不
  宣称第三方写入口可用。

稳定错误至少覆盖 CLI `confirmation_required`/`non_interactive`/`operation_failed`，Unity
`unity_editor_not_found`/`unity_version_mismatch`/`unity_architecture_unsupported`/`unity_project_in_use`/
`unity_process_unknown`，template `template_not_found`/`template_conflict`/`template_archive_invalid`/
`template_dependency_unavailable`，backup `backup_not_found`/`backup_corrupt`/`backup_target_invalid`/
`backup_restore_stale`/`project_changed`/`project_restore_recovery_required`。最终集合需消除语义重复并
更新公共 error Schema；未知错误继续 `internal_error + diagnosticId`。

## Fixture 与 parity

M5 新增的 engineering test ID 在 contract-first 阶段写入 `docs/testing/test-plan.toml`，必须绑定真实
evidence，不能创建空 metadata：

- `cli.command-contract`、`cli.help-output-exit`、`cli.non-tty`：由 planned 变为 implemented 仅在完整
  subprocess/golden/EOF 覆盖后。
- 候选 `projects.m5-management`、`repositories.m5-management`、`packages.m5-cli-workflows`。
- 候选 `unity.m5-engineering`：synthetic Hub/config、fake Editor process、manual registry、launch/status、
  writer gate；不等于 `unity.v3-vrc-parity`。
- 候选 `templates.m5-engineering`：v4 public/synthetic bundle、registry、create/derive/import/export、
  fault matrix；不等于 `templates.v3-parity`。
- 候选 `backups.m5-transaction`：create/restore/target validation/cancel/kill recovery；不等于
  `backups.v3-parity`。

四项 v3 differential test 保持 blocked 到 M11。公开 Unity 文档、synthetic Hub files 和 fake Editor
只能证明工程合同；真实 installed Editor/Hub 验收门槛必须由项目所有者决定是否在 M5 完成前提供，
未取得时 `unity.integration` 保持 `in_progress`。

## 生产依赖与平台 API 审批

当前计划不批准任何新 production dependency。实现前逐项提交精确版本/features/license/MSRV/维护
状态/替代方案/Cargo.lock diff：

| 需求 | 首选路径 | 潜在新增项 / 审批边界 |
|---|---|---|
| shell completion | 成熟 clap 生态生成器 | 候选 `clap_complete`；版本必须与已锁 clap 兼容，只进入 CLI |
| process discovery | std/Tokio 无法可靠跨平台枚举 PID/exe/start time | 候选小型跨平台 process crate（优先评估 `sysinfo` 的最小 feature）；不得用于系统遥测 |
| archive/compression | 复用 `zip 8.6.0` + 现有 feature | 默认不新增 crate/codec，不启用 zip defaults |
| digest/fingerprint | 复用 `sha2 0.11.0` | 不新增算法或 crypto framework |
| filesystem/ownership | 复用 std/Tokio、rustix、现有 windows-sys adapter | 新 Win32 API、rustix feature 或 unsafe 文件必须单独审批 |
| window activation | 初始不实现 | Windows UI API、macOS AppKit/Accessibility、Linux desktop protocol 均不得 blanket approve |

`semver`、`reqwest`、`unicode-normalization` 只在其现有职责需要时复用；CLI/template/backup 不因方便
直接依赖它们。不得启用 Tokio `full`，不得引入通用 desktop automation、ORM、HTTP framework、
workflow engine、第二 ZIP stack 或第二 process supervisor。

## 单元、集成、故障与跨平台验收

### 单元/合同

- clap command tree、alias/help、全局参数互斥、exit-code/error mapping、human/JSON/NDJSON golden。
- injected IO/terminal state 覆盖 TTY/non-TTY、EOF、unknown input、拒绝/同意、`--yes` 和 Ctrl+C。
- Unity version/architecture/identity/config bounded parser；argv 不经 shell。
- template/backup manifest、path profile、include/exclude、collision/quota 和 canonical digest。
- Schema v4/migration、RPC backward compatibility、permission/resource scope、unknown optional fields。

### 集成/故障

- 真实 daemon + CLI subprocess 覆盖每个命令组、daemon auto-start/no-start、stdout/stderr/exit 捕获和
  Operation follow/detach/cancel/replay。
- 真实临时 Unity 项目 + fake Editor executable/process 覆盖 discover/register/select/launch/status、
  lock evidence、外部项目变化和不虚构 running 状态。
- template create 与 backup restore 在每个 destructive journal boundary 强制 kill daemon，重启后复用
  OperationId/Plan/idempotency，断言完整旧/新状态和 evidence 生命周期。
- 同项目 package/template/backup/Unity launch 串行；不同项目并行；ProjectBackup/ProjectRestore 与
  package Apply lock ordering 无死锁。
- 所有 HTTP 测试仅使用本地 mock；M5 不执行攻击性公网、真实 credential 或凭据传播测试。

### 三平台

- Windows Server 2025、Ubuntu 22.04、macOS 15 arm64 均运行 setup/check/test、M0-M5 contract/fault
  tests、release build、Tauri no-bundle、三锁文件和 final diff。
- Ubuntu 继续报告最高 GLIBC 且不高于 `GLIBC_2.35`；macOS 所有预期产物必须 arm64/minos 11.0。
- Windows hosted 只证明编译/工程测试，不得描述为 Win10/Win11 客户端或真实 Unity compatibility。
- synthetic/fake Editor 结果必须与真实 Unity/Hub evidence 分栏报告。若 M5 人工验收要求真实 Editor，
  使用受控测试机并记录版本/架构/项目，不把下载 Unity 加入普通 hosted job。

## Release blocker 与风险

- **CLI 自动化污染**：machine output 混入日志/颜色/提示或 EOF 挂起是 blocker。
- **确认漂移**：确认后重新规划、`--yes` 绕过 stale/safety 或重放创建第二 Operation 是 blocker。
- **Unity 外部 writer**：ALCOMD lock 不能约束 Unity；把“未观察到”当“不存在”会产生数据竞争。
- **进程身份**：PID reuse、不可访问参数或 daemon restart 后错误关联必须返回 unknown，而非猜测。
- **模板供应链**：内建/导入 archive 的来源、许可证、digest 和 path 安全未冻结时不得发布。
- **不一致备份**：Unity/外部 writer 修改中的 best-effort ZIP 不能标记为可恢复一致备份。
- **restore 半提交**：目标目录与 registry/Operation 分裂必须由 journal 与真实 kill test证明恢复。
- **archive 复制**：template/backup 各自实现 unzip/zip policy 会造成安全规则漂移，必须共享 engine。
- **Schema 膨胀**：不得把 Hub cache、process telemetry、CLI view state 或未来 GUI 状态塞入 v4。
- **parity 误报**：synthetic Fixture 不能解除四项 M11 differential blocker。
- **范围过大**：任一 slice 只有合同没有端到端用户入口时不得同时开启下一 slice。

## 人工审批点

开始任何 M5 生产代码前，项目所有者至少审批：

1. CLI 命令/alias、output envelope、stdout/stderr、exit code、TTY/EOF、`--yes`、`--dry-run`、Operation
   wait/detach/cancel 的最终合同。
2. RPC method/capability/error/permission 兼容新增和 State Schema v4/migration。
3. Unity discovery source、process evidence、Project writer hard/advisory gate 与真实 Editor 验收门槛。
4. v4 template bundle Schema、内建模板库存/来源/许可证和 conflict/override 语义。
5. backup archive profile、`exclude VPM packages` 精确语义与覆盖已有目标的 restore Plan。
6. 每个新 production crate、windows-sys/rustix feature、unsafe 文件或窗口/进程平台 API。

审批后仍需在以下情况重新停止：新增未批准 production dependency；扩大 unsafe/平台 API；改变 RPC/
State/permission/CLI public contract；放宽 archive/path/recovery/Unity writer safety；进入 M6 或其他后续
里程碑。

## 与 M0、M4、M11、M12 的关系

- M0 固定工具链和三个 hosted build 继续是每次 M5 提交的基础门禁；Windows hosted 不是客户端证据。
- M4 提供 package Plan/Apply 和唯一 archive/filesystem recovery 基础；M5 复用并扩展 operation kind，
  不降低 SHA-256、ZIP/path、stale Plan 或 kill/restart 规则。
- M11 提供真实 v3.4.0 脱敏 Project/Template/Backup/Unity Fixture 与 migration/differential evidence；
  在此之前相关 parity 保持 blocked。
- M12 负责完整产品、installer/updater/dist 和 Win10/Win11 客户端发行验证。M5 的 Unity/CLI 工程测试
  不替代安装、WebView2、更新、卸载或真实发行资产测试。

## 验证命令

计划实施完成后至少执行：

```text
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo xtask check
<fixed-python> scripts/validate-metadata.py
pwsh -NoProfile -File scripts/freeze-baselines.ps1 -Check
git diff --check
```

并执行 `scripts/setup/check/test`、CLI subprocess/golden、Schema v4 migration、template/backup kill matrix、
Unity process/writer gate、Tauri no-bundle、lockfile/unsafe/dependency feature graph，以及三个 hosted job。

## M5 完成后的停止条件

1. 所有获批 CLI/RPC/State/permission/template/backup/Unity 合同和兼容测试通过。
2. 每个 M5 slice 有真实 daemon + CLI 端到端入口；没有半成品命令或绕过 RPC 的路径。
3. template project create、backup create/restore 和 Unity writer gate 的故障/并发/重启测试通过。
4. machine metadata 只把有真实 evidence 的 engineering test 标记 implemented；四项 v3 parity 保持
   blocked，聚合 feature 状态按真实覆盖更新。
5. 本地完整验收和最终提交的 Windows/Ubuntu/macOS hosted CI 全部成功；报告 GLIBC、macOS target、
   依赖/unsafe/lockfile 和 HEAD/origin/CI SHA。
6. 提交并推送最终候选，工作树干净，然后停止在 M6 前等待项目所有者人工验收；不得自动进入 M6。

## 进度日志

- 2026-08-20：M4 最终提交 `20a86d674b480981d269088cf0615ffdcd9b8e70` 与 GitHub Actions run
  `32289522274` 已由项目所有者人工验收；M4 正式完成。
- 2026-08-20：创建本 M5 ExecPlan 草案。只规划完整 CLI 合同、Unity、本地模板和备份工作流；未修改
  RPC/State Schema、权限、生产依赖、生产代码或 M5 feature/test 状态，等待项目所有者审批。
