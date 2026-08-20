# M5：完整 CLI 合同与本地项目工作流

状态：进行中；Unity 最小生产切片已独立提交；Template production/RPC/CLI 已完成本地验收，停止在 Backup Create contract 审批点

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
6. State Schema v4 的 Unity/registry 与 Schema v5 的窄 Template Plan authority；不为未来 GUI/MCP/扩展预建字段。
7. public/synthetic engineering Fixture、CLI subprocess golden、跨平台 process/filesystem 测试和三个
   hosted 平台验收；真实 v3 differential 项保持 blocked。

## 完成定义

- CLI 的每个已发布命令均只调用 `alcomd-client`，且能由 RPC 监控测试证明没有直接打开 state.db、
  repository/package cache 或 Unity 项目文件。
- human/JSON/NDJSON、stdout/stderr、退出码和 alias 形成版本化快照；非 TTY 与关闭 stdin 永不等待，
  `--yes` 不绕过 stale revision、权限、Unity writer 或 Plan revalidation。
- Project/Repository/Package、Template、Backup、Unity 每个 M5 命令均有真实 daemon integration test；
  不发布只返回 scaffold/unsupported 的假命令。
- Unity `running_confirmed` 会触发明确 hard gate；证据缺失不被描述为“证明 Unity 未运行”。外部
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
crates/alcomd-store/                 # 获批的 Schema v4/v5 migration、registry/Plan/journal
crates/alcomd-platform/              # 三平台发现、进程、路径/权限与启动原语
crates/alcomd-vpm/                   # 复用既有 bounded archive/project transaction adapter
crates/alcomd-testing/               # synthetic/public Fixture、CLI/daemon/kill tests
specs/rpc/                           # M5 RPC/error/output Schema 与兼容快照
specs/cli/                           # CLI v1 进程合同、machine Schema 与 command catalog
specs/storage/                       # State Schema v4/v5 migration
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

M5 只支持全新、不存在目标的 Plan/Apply restore；不支持已有目标、覆盖、merge 或先删除再恢复。
复用同一 filesystem journal 和真实 kill/restart gate，完成后才接 CLI confirmation/`--yes`。不得将
restore 实现为普通 unzip。

### Slice 5：CLI surface 收敛

在真实后端存在后补齐 project/repository/package/template/backup/unity 命令、completion 和所有
output/exit/TTY golden。没有后端的命令不进入 help；聚合 feature 状态按真实覆盖更新。

## 完整 CLI contract v1

### 命令面

权威 Slice 0/1 名称与 alias snapshot 是 `specs/cli/command-catalog-v1.json`。它只冻结 system、
operation、现有 project/repository、M4 package shortcut、Unity 与 completion。template/backup 命令
必须等各自 contract slice 冻结后兼容增加；hashless/legacy/credential/project migration 等被排除能力
不能通过同名命令暗中实现。任何命令只有 backend capability 真实存在后才可进入 help。

### 输出与退出

- human 为默认；`--json` 与 `--ndjson` 互斥。JSON/NDJSON envelope 和字段是稳定机器合同。
- human 最终结果写 stdout；progress/warning/diagnostic/fatal error 写 stderr。
- JSON 成功 stdout 恰好一个 document 且 stderr 无普通日志；失败 stdout 为空，stderr 恰好一个稳定
  error document。
- NDJSON stdout 每行是带 type 的独立 JSON record；Operation follow 可输出 operation/progress/event，
  stream 开始后终态 error 是最后一条 stdout record。diagnostic/log 仍写 stderr。
- 退出码固定为 `0` success、`1` command/domain/Operation failure、`2` usage、`3` explicit partial
  success、`130` interrupted/detached。机器分类继续使用稳定 error.code。
- `--quiet` 只抑制非错误 human progress/diagnostic，不抑制最终结果或 fatal error。
- `--no-start-daemon` 沿用 M1；broken pipe 必须有界退出且不 panic。

### 确认、TTY、EOF 与 Operation

- 高影响 shortcut 必须先产生并展示 immutable Plan，再确认精确 `planId/fingerprint` 后 Apply。
  `--yes` 只代替确认，不跳过 permission、expectedRevision、source pin、writer gate 或 stale Plan。
- non-TTY 永不读取 prompt；closed stdin/EOF 立即返回 `confirmation_required`。JSON/NDJSON 总是
  non-interactive。
- mutation 默认等待 Operation 终态；`--no-wait` 接受后立即返回 OperationId。
- Ctrl+C 在确认/Operation 创建前以 130 退出且不产生 mutation；OperationId 已产生后只 detach、输出
  ID 并返回 130。业务取消只能显式 `operation cancel`。
- `--dry-run` 只创建/返回 durable immutable Plan，绝不 Apply 或产生外部 filesystem/business mutation。
- shell completion 只由 clap command tree 静态生成；不读取项目/repository 动态名称，不触发 daemon。

## Unity / Unity Hub integration

### 发现与身份

- source 固定为 `manual`、`hub_config`、`known_install_root`、`unity_cli_hint`；同一 Editor 通过平台
  file identity 去重，path string、显示名和版本不是权威身份。Unity CLI 只提供 advisory hint，Hub
  CLI 不作为权威 source 或必需依赖。
- Hub/config 读取必须 bounded、只读、no-follow 并保留 last-known-good registry；unknown/malformed
  source 返回结构化诊断，不删除手工记录。
- version 必须来自验证后的 Editor bundle/executable metadata；architecture 固定枚举
  `x86_64`/`arm64`/`universal`/`unknown`，unknown 不可偷偷当兼容。
- 项目首选 Editor 是显式 ProjectId -> EditorId 关系；项目 `ProjectVersion.txt` 继续是兼容校验输入。
  launch arguments 持久化为最多 64 项、单项最多 4,096 bytes 的 string list，不存 shell command
  line，不经 shell 解析，并拒绝 `-projectPath` 及等价重复 selector。
- Unity 官方当前文档将独立 Unity CLI 标为 experimental，并将 Hub CLI 标为 deprecated；M5 不把
  两者当权威发现依赖。它们未来最多是经审批的可选 import source。

参考官方材料：

- <https://docs.unity.com/en-us/hub/cli-overview>
- <https://docs.unity.com/en-us/unity-cli/use-unity-cli>
- <https://docs.unity3d.com/cn/current/Manual/EditorCommandLineArguments.html>

### 启动与进程状态

- 直接执行已验证 Editor executable，并以独立 argv 传 `-projectPath <root>`；不得经 shell 拼接。
- launch 在 Project lock 下复验 project/editor identity、project revision 与 version compatibility，成功
  spawn 后只记录 opaque launch ID、`opening` 与 project/editor IDs；spawn accepted 不等于完全打开。
  后续 observation 才可变为 `open`/`failed`，daemon 生命周期不拥有 Unity 生命期。
- writer state 固定为 `running_confirmed`、`running_suspected`、`not_observed`、`unknown` 并携带 bounded
  evidence。PID 必须与 executable identity/start evidence 联合验证；无法安全读取时返回 unknown。
- 前台激活没有可靠的三平台统一语义，不是 M5 基础完成 blocker。只有 Windows/macOS/Linux 各有
  可测试且不依赖桌面自动化框架的实现时才兼容增加 `unity activate`；否则保持未发布。

### 外部 Unity writer 安全边界

- package mutation、从项目生成 template 与 backup create 对 `running_confirmed` hard reject
  `unity_project_running`。`running_suspected`（包括不足以关联进程的 lock evidence）与 `unknown` 只产生
  advisory，并继续 live fingerprint/changed-during-operation 检查；不存在通用 `--force`。
- Unity launch 对 confirmed 拒绝第二实例；suspected/unknown 返回 `unity_launch_state_uncertain`；只有
  not_observed 允许启动。not_observed 不能描述成 definitely not running。
- 每个 destructive operation 仍须在 Plan、首次 mutation 和 commit checkpoint 重验 project
  fingerprint；变化返回 `project_changed`/`plan_stale` 并进入已有 rollback/recovery。
- ALCOMD 无法阻止用户或外部 Unity 在检查后开始写入。文档与错误必须明确这一局限，不能宣称任意
  Unity writer 与 ALCOMD transaction 被全局协调。

## Templates

- v4 原生格式固定为 `ALCOMD Template Bundle v1` / `.alcomdtemplate`，不是 v3 `.alcomtemplate`。
  v3 v1/v2 import 与 differential parity 在 M11 前 blocked；M5 不引入 tar/gzip parser。
- ZIP 根只允许 `template.json`、`payload/` 与 manifest 声明的 `resources/`；Stored/Deflate、UTF-8/NFC、
  collision/link/special-file/path 与 streaming 检查复用 M4 engine。独立 template quota 固定为 2 GiB
  compressed、100,000 entries、单 entry 2 GiB、total 8 GiB、depth 64、path 1,024 UTF-8 bytes、
  ratio 1,000:1，见 `specs/templates/template-bundle-v1.md`。
- registry `source_kind` 继续只有 `builtin | user`。imported/derived/authored 是 bounded provenance；user
  object locator 是 `sha256:<digest>`，builtin locator 是 `builtin:<id>@<version>`，RPC 不暴露实际路径。
- v1 builtin inventory 固定 Blank、VRChat Avatars、VRChat Worlds 三个 stable TemplateId。scaffold 为
  ALCOMD 独立创作、AGPL-3.0-only，不嵌入 v3/vrc-get/Unity/VRChat SDK bytes；SDK 仅声明 VPM dependency。
- import 先 inspect/preflight/digest/conflict，再 immutable Plan/Apply。user 同 ID 同 digest no-op，不同
  digest conflict；builtin immutable。export create-new 且从已验证 object 复制，格式只承诺 semantic
  deterministic；remove 只解除 user registry binding，不在 M5 做 object GC。
- derive 是 self-contained 新 TemplateId/bundle，不建立 inheritance DAG。遍历 policy 逐项冻结在 bundle
  规范；writer confirmed hard reject，suspected/unknown advisory，但前后 fingerprint 变化必定失败。
- create-project Plan 固定 template/source/package/resource/parent identity 与 target leaf，且 target 必须
  不存在。Resource Key 是 `ProjectCreate(parent_identity,target_leaf)`；Apply 返回 OperationId，复用 M4
  archive/package/filesystem journal，不 nested package Operation、不重新 resolve、不创建半项目。

## Backups

### Create

- 输入为已注册 ProjectId 和 expectedRevision；读取前后 fingerprint 必须一致。Unity
  `running_confirmed` 时拒绝一致性备份；suspected/unknown 产生 advisory 并依赖前后 fingerprint。
- 流式写 ALCOMD backup root 内 operation-owned partial，使用现有 ZIP writer/profile、SHA-256、entry/
  total/path quota，flush/fsync 后原子 publish；取消/失败删除或保留 journal-owned partial。
- compression profile 只暴露 Frozen 枚举（例如 stored/fast/maximum），不把第三方 codec 参数变成
  公共合同。
- `exclude VPM packages` 的精确语义是人工审批点：必须冻结被排除目录、保留的 manifests、restore
  时 package resolve 前提和 offline/credential 失败行为，不能仅按目录名猜测。

### Restore

- restore 始终先 `backups.planRestore`：固定 archive digest、source project fingerprint、target identity、
  ChangeSet、expected target state 与冲突；Apply 返回 OperationId。
- target 必须全新且不存在，并通过路径/owner/no-follow/同卷校验；M5 不支持已有 target、覆盖、merge
  或删除后恢复。restore 获取 `ProjectRestore(target_identity)`，且要求显式确认或 `--yes`。
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

## State Schema v4/v5 与 RPC v1

M5 的 durable registry 需求使用已批准的 Schema v4：

- `unity_installations`：InstallationId、validated executable path、platform file identity、Unity version、
  architecture/unknown、source kind、revision 与 observed/updated timestamp。
- `project_editor_preferences`：ProjectId、InstallationId、bounded argv array、revision。
- `templates`：TemplateId、builtin/user source、versioned bounded manifest、payload locator/digest、favorite、
  revision。完整 template bundle/operation 合同留在 Slice 2。
- `backups`：BackupId、可选历史 source ProjectId、archive locator、file identity/digest/size、format version、
  createdAt、compression 与 exclude-VPM flag。它不是复杂 aggregate/Event 状态机。

Template immutable Plan 使用获批的 Schema v5 窄表 `template_plans`，只允许 import/derive/
create-project 和一次性 `unapplied -> applied + OperationId` transition；M4 `package_plans` 不泛化。
operations.kind 只精确增加 `templates.import`、`templates.derive`、`templates.create-project`，不预建
export/Backup/generic workflow kind。migration 覆盖 v1->...->v5、v4->v5、rollback、future schema
fail closed，并保留 M2-M4 Operation/journal/idempotency/Event sequence、package Plan 与 Unity registry。

不新增 CLI 设置表、通用 workflow 表、Hub mirror 数据库、process history 日志或未来 GUI state。

Slice 1 RPC v1 兼容增加已冻结为 `unity.read.v1`、`unity.manage.v1`、`unity.launch.v1`，准确 method、
DTO、collection limit 与 enum 见 `specs/rpc/m5-unity.schema.json`。template/backup capability 必须等各自
contract slice 再冻结。新增 method/capability/可选字段不提升 RPC major，删除/改义才提升 major。

既有权限名优先直接精化：

- template query 使用 `templates.read`；create-project 使用 `templates.read + projects.create`。
- backup query 使用 `backups.read`；create 使用 `backups.manage + target project read scope`；restore 使用
  `backups.read + backups.manage + projects.create`。
- Unity query 使用 `unity.read`；registry/refresh/project preference 使用新增 `unity.manage`；launch/status
  使用独立 `unity.launch`。
- `projects.create` 只授权在显式、不存在 destination 创建一个全新 Project，不允许覆盖、删除、merge
  或修改既有 Project；不得扩大 M3 `projects.manage`。
- 所有 M5 外部文件写仍只对 `builtin:local-owner` 开放；真实外部 credential/revocation 未完成前不
  宣称第三方写入口可用。

Slice 0/1 已冻结 `confirmation_required` 以及 Unity installation/version/architecture/running/launch/
selector 错误，见 `rpc-error.schema.json`。template/backup 错误在对应 contract slice 冻结。未知错误
继续 `internal_error + diagnosticId`。

## Fixture 与 parity

M5 engineering test ID 写入 `docs/testing/test-plan.toml`；只有合同测试绑定真实 evidence 并标记
implemented，动态/生产测试继续 planned：

- `cli.command-contract`、`cli.help-output-exit`、`cli.non-tty`：由 planned 变为 implemented 仅在完整
  subprocess/golden/EOF 覆盖后。
- `m5.cli-unity-contract` 绑定 CLI/Unity Schema、Schema v4 migration 与权限合同。
- `unity.m5-registry-launch` 与 `unity.m5-writer-gate` 使用 synthetic layout/fake process provider。
- `templates.m5-bundle` 与 `templates.m5-registry` 只在 Schema/quota/security/migration snapshot 有真实
  evidence 后标 implemented；import/export/derive/create-project/fault engineering tests 在生产实现前保持
  planned。`backups.m5-create`、`backups.m5-restore` 继续 planned。

四项 v3 differential test 保持 blocked 到 M11。公开 Unity 文档、synthetic Hub files 和 fake Editor
只能证明工程合同；Hosted CI 不要求安装真实 Unity，未取得真实 evidence 时不得宣称 differential
parity 或客户端兼容已验证。

## 生产依赖与平台 API 审批

当前计划不批准任何新 production dependency。实现前逐项提交精确版本/features/license/MSRV/维护
状态/替代方案/Cargo.lock diff：

| 需求 | 首选路径 | 潜在新增项 / 审批边界 |
|---|---|---|
| shell completion | 成熟 clap 生态生成器 | 候选 `clap_complete`；版本必须与已锁 clap 兼容，只进入 CLI |
| process discovery | std/Tokio 无法可靠跨平台枚举 PID/exe/start time | 已批准并接入 `sysinfo = 0.39.6`（defaults off，仅 `system`），只存在于 `alcomd-platform`；MSRV 1.95，详见 `M5-process-discovery-evaluation.md` |
| archive/compression | 复用 `zip 8.6.0` + 现有 feature | 默认不新增 crate/codec，不启用 zip defaults |
| digest/fingerprint | 复用 `sha2 0.11.0` | 不新增算法或 crypto framework |
| filesystem/ownership | 复用 std/Tokio、rustix、现有 windows-sys adapter | 新 Win32 API、rustix feature 或 unsafe 文件必须单独审批 |
| window activation | 初始不实现 | Windows UI API、macOS AppKit/Accessibility、Linux desktop protocol 均不得 blanket approve |

`semver`、`reqwest`、`unicode-normalization` 只在其现有职责需要时复用；CLI/template/backup 不因方便
直接依赖它们。不得启用 Tokio `full`，不得引入通用 desktop automation、ORM、HTTP framework、
workflow engine、第二 ZIP stack 或第二 process supervisor。

### M5 内部停止点

- **A（已通过）**：冻结 Slice 0 CLI 与 Slice 1 Unity 的 ADR、Schema、RPC、错误、权限、State v4 结构
  migration 和合同测试；`sysinfo 0.39.6` 精确依赖已经批准。
- **B（已通过）**：Unity production slice 已作为提交
  `8b63c6923b178a6ebb12bd5964412b2db7268e04` 保存。Template bundle、库存/许可证、权限、RPC 与
  create-project transaction contract 已获批并冻结；项目所有者已进一步批准 Schema v5 closure 与
  Template production。严格按 parser/inventory/object/registry/import-export/derive/create-project 推进。
- **C**：Template 垂直切片通过后停止；审批 Backup create archive profile、exclude-VPM 精确语义。
- **D**：Backup create 垂直切片通过后停止；审批全新目标 Backup restore Plan/Apply 与 recovery 合同。
- **E**：Backup restore 与完整 CLI surface 收敛后，运行 M5 全量本地/Hosted 验收并停止在 M6 前。

不得因 Schema v5 migration 文件存在就越过后端真实性门禁；capability 和 method 只有
对应 adapter/use case 真正实现并通过当前 slice 验收后才能接线或广告。

## 单元、集成、故障与跨平台验收

### 单元/合同

- clap command tree、alias/help、全局参数互斥、exit-code/error mapping、human/JSON/NDJSON golden。
- injected IO/terminal state 覆盖 TTY/non-TTY、EOF、unknown input、拒绝/同意、`--yes` 和 Ctrl+C。
- Unity version/architecture/identity/config bounded parser；argv 不经 shell。
- template/backup manifest、path profile、include/exclude、collision/quota 和 canonical digest。
- Schema v4/v5 migration、RPC backward compatibility、permission/resource scope、unknown optional fields。

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

项目所有者已经批准 Slice 0/1 的 CLI、Unity、RPC/错误/权限与 State Schema v4 结构合同；这些合同
由 A-030/A-031、`specs/cli/`、`specs/rpc/m5-unity.schema.json`、`specs/storage/state-v4.md` 和两个
ADR 冻结。process discovery production dependency 已按精确配置批准；后续 slice 仍分别审批：

1. `M5-process-discovery-evaluation.md` 中 `sysinfo = 0.39.6` 已关闭；任何 feature 扩大仍须重新审批。
2. v4 template contract-first、production registry/import/export/derive/create-project 与窄 M4 staging
   package adapter 已批准并实现；下一停止点是 Backup Create contract，不得提前实现 Backup。
3. backup archive profile 与 `exclude VPM packages` 精确语义。
4. M5 仅限全新目标的 restore Plan/Apply/recovery 合同；覆盖已有目标不在 M5。
5. 此后每个新 production crate、windows-sys/rustix feature、unsafe 文件或窗口/进程平台 API。

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
- 2026-08-20：项目所有者批准 M5 总体方向并进入 contract-first。已冻结 Slice 0 CLI v1 与 Slice 1
  Unity RPC/capability/error/permission、State Schema v4 结构 migration 和 writer gate ADR；Schema v4
  仍未接入 daemon，Unity method 仍未广告，未开始生产实现。
- 2026-08-20：完成 `sysinfo 0.39.6`（defaults off，仅 `system`）隔离 feature/Cargo.lock 评估，未修改
  Workspace manifest/lockfile。按内部停止点 A 等待 process discovery production dependency 审批。
- 2026-08-21：完成 Template registry/import/export/derive/create-project 的 production、RPC 与 CLI
  垂直切片。create-project 复用一个 daemon-owned ResourceLockCoordinator 与 M4 PackageCache，冻结
  ChangeSet 在取得 ProjectCreate lock 前完成 cache 验证，UPM manifest byte-for-byte 保持不变。
  真实 daemon kill/restart matrix 覆盖 prepared、staging complete、target publish intent、target
  published、project registry commit intent 与 state committed；全部复用原 OperationId、Plan、
  idempotency 与预分配 ProjectId，且修复了 M2 通用恢复器误接管 Template Operation 的缺陷。
- 2026-08-20：项目所有者批准 `sysinfo 0.39.6`，并纠正其官方 MSRV 为 Rust 1.95。依赖仅接入
  `alcomd-platform`；完成最小 process evidence、Unity executable registry/writer gate/launch adapter、
  State Schema v4 自动 migration、RPC v1 capability/method 和 client 接线，进入 Slice 1 本地验收。
- 2026-08-20：Slice 1 本地验收通过：统一 `scripts/check.ps1`、全 Workspace test/clippy、Discord、
  TypeScript/Vite、Tauri release `--no-bundle`、Schema v4 migration、真实 daemon RPC、fake-provider 四态、
  Windows 短生命周期子进程、冻结基线、metadata 与 diff 门禁均成功。停在内部停止点 B；Linux/macOS
  真实子进程结果须由后续 hosted CI 取得，不得据 Windows 本机结果宣称三平台已通过。
- 2026-08-21：Unity production slice 以独立提交 `8b63c6923b178a6ebb12bd5964412b2db7268e04`
  保存。随后按项目所有者批准冻结 ADR 0020、`.alcomdtemplate` Bundle/manifest/quota、三个 native
  builtin inventory/AGPL provenance、Template RPC/permission/error、planned CLI、Schema v4 compatibility、
  synthetic Fixture 与 contract/security/migration snapshot；未开始任何 Template production adapter。
- 2026-08-21：项目所有者批准 Template contract-first 总体审核与 production slice。先增加
  `0005_template_plans.sql`：窄 immutable Plan authority、三个精确 Template Operation kind、Schema v5
  hello compatibility，并以 migration/rollback/foreign-key/state-preservation 测试闭环；随后无需再次
  停止，按 parser -> inventory -> object store -> registry -> import/export -> derive -> create-project 实施。
