# M4：VPM Package Plan/Apply 与可恢复项目事务

状态：草案，等待项目所有者审批；未冻结 M4 公共合同，未开始生产实现

## 目标

在已经通过人工验收的 M2 Operation/Revision/Event/Resource Lock/DB recovery 和 M3
Project/Repository normalized read model 上，完成第一个会真实修改 Unity 项目文件的 VPM package
transaction 垂直切片：

```text
RPC package Plan methods
        │
        ▼
deterministic resolver -> durable Plan + ChangeSet
        │
        ▼
packages.applyPlan -> OperationId
        │
        ├─ bounded download / content-addressed cache
        ├─ hostile ZIP validation / staging extraction
        ├─ Project(project_id) serialized filesystem transaction
        ├─ manifest commit / rollback
        └─ durable journal / restart recovery
                │
                ▼
        deterministic final project state
```

M4 优先证明一个真实 package install/remove transaction 能够完整经过
`Plan -> Apply -> Operation -> crash recovery` 并得到确定结果。upgrade、downgrade 与 resolve 复用同一
package model 和 ChangeSet，不建立通用 workflow engine，也不一次实现未来全部 VPM 用户入口。

## 前置条件

- M3 最终提交为 `2082b5596d246975ca7a48dab20826899103e03d`；GitHub Actions run
  `32174028968` 的 Windows Server 2025、Ubuntu 22.04 与 macOS 15 arm64 全部成功，项目所有者
  已确认人工验收通过。
- M1 RPC v1 framing/hello/error 与 M2 Operation/Revision/Event/idempotency 合同只能兼容增加。
- M3 Project/Repository ID、normalized snapshot、source identity 和只读 parser 继续是 M4 输入；
  M4 不另建第二套项目发现或 repository registry。
- `alcomd` 仍是唯一数据库和 Unity 项目写入者；CLI、GUI、MCP、扩展及其他 adapter 不得绕过
  `alcomd-application`。
- `projects.v3-parity` 需要的 M11 脱敏真实 Fixture 仍不存在，必须保持 `blocked`。
- 真实 repository credential enrollment/revocation 尚未完成；M4 只能使用已经批准的 local 与
  anonymous HTTP(S) source，不接受自定义 header、token 或 URL userinfo。

## 最小交付物

1. 独立实现并由公开 Fixture 冻结的 VPM package manifest、SemVer/range 与 Unity compatibility
   模型。
2. 确定性的 repository precedence、同版本 source tie-break 与完整 source fingerprint。
3. install/remove/upgrade/downgrade/resolve 的 Plan 与不可变 ChangeSet；Apply 不得重新规划。
4. package archive 的 bounded download、强完整性校验、content-addressed cache 与 offline 读取。
5. hostile ZIP 验证、根内 staging extraction、数量/大小/膨胀限制和跨平台 path collision 检查。
6. `Packages/`、`Packages/vpm-manifest.json` 及必要时 `Packages/manifest.json` 的可恢复项目事务。
7. 在 M2 journal 上增加明确 filesystem transaction phase、补偿证据和 daemon restart recovery。
8. Project/PackageCache/必要 Catalog resource lock、Operation progress/cancellation 与稳定错误。
9. RPC v1 的兼容新增 method/capability/DTO/Schema；仅提供证明切片所需的最小 CLI 驱动入口。
10. synthetic/public compatibility Fixture、事务故障注入、并发与三平台 hosted 验收。

## 完成定义

- 同一个 catalog、项目 snapshot 和请求在任意运行中产生 byte-stable、顺序稳定的 ChangeSet；同
  package/version 存在多个 source 时不会依赖 HashMap 或文件枚举顺序。
- Plan 固定 package ID/version、source identity、archive URL、声明哈希、dependency graph、项目
  revision 与 catalog/source fingerprint。Apply 发现任一前提变化时返回稳定 stale 错误，不下载或
  静默采用新候选。
- install、remove、upgrade、downgrade 与 resolve 都通过同一 resolver/ChangeSet；至少 install 与
  remove 完成真实 RPC -> Operation -> project mutation -> restart recovery 垂直验证。
- remote package 缺少合法、已支持的强 hash 时 fail closed；下载、cache hit 与 offline hit 都在
  使用前重新验证内容，不信任 sidecar 或文件名。
- archive 中的路径穿越、绝对路径、链接、设备路径、重复/大小写碰撞、超限 entry 和 zip bomb 在
  写入项目树前被拒绝；任何失败都不能写出已验证 staging/project/cache root。
- project commit 的每个外部可观察步骤均有 durable phase 与恢复证据；在 download、extract、旧包
  移出、新包替换、manifest 提交及 DB finalize 处终止进程，重启后都收敛到旧状态或完整新状态，
  不得把混合状态误报为 succeeded。
- 同一项目写操作串行，不同项目可并行；cache publication 不因并发产生半文件或用错 archive。
- 所有普通错误使用稳定机器可读 code，日志/Event/Operation result 不包含 credential、完整私密
  路径、原始 manifest、archive 内容或 parser debug。
- 本地完整门禁及 Windows Server 2025、Ubuntu 22.04、macOS 15 arm64 hosted CI 全部通过；
  工作树干净，并停止在 M5 之前等待人工验收。

## 明确不属于 M4

- GUI、MCP、Local API、SDK 与 Extension Runtime/WASI。
- Unity/Unity Hub 发现、启动、editor 管理与项目 migration。
- 模板、项目创建/复制/删除、备份/恢复与 local user package 管理。
- v3 migration、bootstrap/bridge、installer、updater、签名、发行和零残留。
- M5 及后续完整 CLI/JSON/NDJSON/progress/completion/交互确认用户体验。
- repository import/export/deep-link、credential、自定义 header、proxy 与 authenticated package URL。
- legacy file/folder/package 清理、任意 GUID asset 删除及其不可逆补偿；M4 parser 可识别并报告这些
  字段，但首个切片若需要执行它们必须返回稳定 unsupported 错误。
- package 签名、透明日志、P2P、增量 archive、后台自动更新或通用 download manager。
- 通用 workflow DSL、可插拔 resolver、分布式锁、任务队列或跨机器事务。

## 允许修改范围

M4 只有在合同、Schema、ADR、migration 和生产依赖分别获批后，才允许最小修改：

```text
apps/alcomd/                         # M4 RPC adapter、Operation worker 与启动恢复接线
apps/alcomd-cli/                     # 仅测试垂直切片所需的最小 Plan/Apply 驱动入口
crates/alcomd-domain/                # package/plan/change/resource key 的纯类型与不变量
crates/alcomd-application/           # resolver/Plan/Apply 用例、ports、权限与锁协调
crates/alcomd-protocol/              # 仅获批的 RPC v1 兼容 DTO
crates/alcomd-client/                # 仅对应类型化调用
crates/alcomd-store/                 # Schema v3、Plan/transaction journal/Operation 持久化
crates/alcomd-platform/              # 仅缺失且获批的 fsync/atomic replace 平台原语
crates/alcomd-vpm/                   # manifest/range/resolver/download/archive/project transaction adapter
crates/alcomd-testing/               # public/synthetic Fixture、fault gate 与进程测试
specs/rpc/                           # M4 Schema、method/capability/error 合同
specs/storage/                       # state Schema v3 与 migration 合同
specs/extensions/permissions-v1.md   # 仅获批 packages.read/manage 精确定义
docs/adr/                            # M4 package transaction ADR
docs/exec-plans/M4-package-transactions.md
docs/status.md
docs/testing/test-plan.toml
feature-parity.toml
Cargo.toml
Cargo.lock
scripts/                             # 仅 M4 metadata/fixture/跨平台验收门禁
xtask/                               # 仅依赖方向、unsafe 与 M4 契约硬门禁
.github/workflows/ci.yml             # 仅三平台 M4 验收命令
```

不因本计划预先批准任何具体 Schema、RPC 名称、生产依赖或新的 unsafe。若标准库/现有安全平台 API
不足，必须单独提交最小 API/unsafe 边界审批，不能把通用 filesystem abstraction 塞进
`alcomd-platform`。

## 依赖方向与职责

- package/range/source/ChangeSet 纯类型进入 `alcomd-domain`，不得依赖 SQLite、HTTP、ZIP 或 OS。
- `alcomd-application` 拥有 Plan/Apply 编排、权限、幂等、revision 与 Resource Lock；通过窄 port
  访问 catalog、archive cache、project filesystem 和 store。
- `alcomd-vpm` 实现公开 VPM 格式、resolver、download/archive 和 Unity package layout port；不得
  直接绕过 application 执行业务命令或拥有权威 Operation 状态。
- `alcomd-store` 只负责短 SQLite transaction、Plan/Operation/journal 持久化；网络、解压、fsync、
  等锁和项目 I/O 不得发生在 SQLite transaction 内。
- `apps/alcomd` 只组合 adapter、调度 worker 和启动恢复；RPC dispatcher 不复制 resolver 或事务
  规则。
- CLI 只调用公开 RPC，不能直接解析/写 manifest、读取 cache 或访问 state.db。
- M4 不新建 `alcomd-package-engine`、`alcomd-workflow` 等 crate；真正出现不可分离职责且经审批后
  才重新评估。

## VPM package model

### Package manifest 与身份

合同阶段必须冻结 bounded `package.json` 读取模型：

- `name` 与 `version` 必需；map key、manifest `name/version` 必须一致，不能像参考实现一样静默
  接受错配。
- `url`、强 hash、`vpmDependencies`、`unity`、`yanked` 与 source metadata 进入 normalized model；
  display/description/文档等不影响 resolver 的字段保持 bounded 可选值。
- 未知字段可保留兼容读取但不进入安全决定；重复 key、null、BOM、非法 optional 字段的严格/宽松
  策略必须由公开 Fixture 冻结，不复刻某个参考解析器的偶然容错。
- PackageId 在任何 path join 前完成独立合法性校验。最终规则必须覆盖空值、分隔符、`.`/`..`、
  drive/UNC/device/ADS、控制字符、NUL、尾随点/空格与平台 case collision。
- domain identity 至少包含 `(package_id, semantic_version, source_identity, archive_digest)`；显示名称、
  URL 字符串或 repository 遍历位置不是权威身份。

### SemVer、range 与 dependency

- 版本和 range 语义以公开 VPM/SemVer.NET 兼容 Fixture 冻结，覆盖 exact、bare、caret、tilde、
  hyphen、OR、prerelease、build metadata、交集与无解；不得复制 vrc-get parser。
- Plan 区分 direct dependency、transitive dependency 与 locked installed package；dependency graph 的
  输出顺序按 PackageId/version/source identity 固定。
- resolver 必须一次收集全部约束并返回稳定的 missing/conflict 集合，不能因遍历顺序选择不同
  错误或 candidate。
- prerelease 默认排除，只有明确请求或现有 exact lock 允许时进入 candidate；最终规则在 RPC
  Schema 中显式表示，不能依赖隐藏全局设置。
- yanked candidate 默认不参与新选择；是否允许 exact 已锁版本继续使用、修复或显式降级，必须在
  M4 合同审批时单独冻结。

### Unity compatibility

- 使用 M3 ProjectVersion normalized Unity version；缺失或 malformed 时 Plan fail closed，不猜测。
- package `unity` range 的 major/minor/patch、prerelease 与缺省语义由 public Fixture 冻结。
- VRChat SDK/resolver 的历史 Unity 特例属于产品策略，不自动归入公开 VPM 格式；若 M4 必须兼容，
  应作为具名、版本化 policy 写入 ADR 和测试，而非藏在通用 comparator 中。
- incompatible candidate 被过滤；如果所有 candidate 不兼容，返回独立结构化错误及安全的 package
  identity，不泄露项目完整路径。

### Source precedence 与同版本确定性

- repository 注册顺序、显式用户优先级和 source identity 的关系必须在合同阶段冻结；不能依赖
  HashMap iteration、filesystem enumeration、响应到达顺序或 UUID 随机值。
- 草案建议优先级为：请求显式 pin 的已注册 source，其次持久化 repository priority，最后
  canonical source identity byte order。是否给 M3 Schema 增加 priority 是公共 Schema 决策点。
- 同 package/version 的不同 source 若 digest 不同，不得合并为同一 candidate；无显式 precedence
  时应返回 `package_source_ambiguous`，而不是任意选一个。
- Plan 一旦生成必须保存精确 source identity、repository revision、manifest fingerprint、artifact
  URL 与 digest；Apply 只能使用这些值。

## Plan / Apply 合同

### Plan

- M4 候选方法为 `packages.planInstall`、`packages.planRemove`、`packages.planUpgrade`、
  `packages.planDowngrade` 与 `packages.planResolve`；最终命名、是否收敛为一个带 action enum 的
  method，以及 capability 名称必须经 Schema 审批。
- Plan 是确定性计算结果，不写 Unity 项目。为支持 Apply/重启/授权复验，获批后以不可变 durable
  record 保存，返回 `planId`、项目 revision、catalog/source fingerprints、expiry 与 ChangeSet。
- ChangeSet 至少包含：安装/替换/移除 package、版本/source/hash、direct dependency mutation、
  locked package mutation、manifest field mutation、冲突/unsupported metadata 和预期最终摘要。
- reinstall、legacy cleanup、local user package 不在首个切片；不能通过 install action 偷渡。
- 相同输入与相同 snapshots 产生相同 canonical fingerprint；Plan record 不保存 token、完整私密
  path、raw repository/package JSON 或 archive bytes。

### Apply

- Apply 候选方法 `packages.applyPlan` 必须携带 `planId`、project `expectedRevision` 和永久
  `idempotencyKey`，成功接受后返回同一个 durable `OperationId`。
- Apply 在任何项目写入前重新验证 Principal/permission、Plan owner/scope、Plan 未使用/未过期、
  project identity/revision、repository/source revision、manifest fingerprint、URL 与 digest。
- 任一前提变化返回 `plan_stale` 或更具体的稳定 reason，不得在 Apply 内重新 resolve、替换 source、
  升降版本或产生新 ChangeSet。客户端必须显式重新 Plan。
- 同一 Plan 只能绑定一个 successful Apply Operation；相同幂等 scope/fingerprint 重放原
  OperationId，不同 fingerprint 返回 `idempotency_conflict`。
- Apply 完成后保存实际 final project fingerprint 与 ChangeSet result；如果 final tree 不匹配，不得
  标记 succeeded。

## Package download 与 cache

- 复用 M3 受限 `reqwest` client 的 anonymous/no-proxy/no-cookie/no-credential 基线；package URL
  只允许无 userinfo 的 HTTP/HTTPS。HTTPS -> HTTP redirect 拒绝，redirect 次数、跨 origin 规则、
  connect/total timeout 与 body 上限必须在 M4 合同中重新冻结为 package payload 值。
- remote package 必须声明格式正确的强 digest；首个切片建议只接受 SHA-256 64 hex。缺失、畸形、
  不支持算法与 hash mismatch 都 fail closed，不静默退化为“下载后自算即可”。
- 读取 `Content-Length` 只能提前拒绝，不能替代 streamed byte counter。达到上限、超时、中断或
  checksum 失败时删除/隔离 ALCOMD 自有 partial，不发布 cache object。
- cache key 使用算法 + digest 的 versioned content-addressed key；package ID、URL、显示名不能单独
  决定路径。key 到 path 的编码固定并有 snapshot test，最终 resolved path 必须位于 cache root。
- 下载到 cache root 内唯一 `.part` staging，flush/fsync 后再以原子 publish 变为 immutable object；
  并发同 digest 只允许一个有效对象，其他 writer 验证后复用或安全丢弃自己的 partial。
- 每次 cache hit/offline hit 都重新验证文件类型、大小和 digest；corrupt object 不得使用。online
  可移除 ALCOMD 自有坏对象后重新下载，offline 返回 `package_cache_corrupt`/`offline_cache_miss`。
- cache index/metadata 只能是可重建辅助信息，不取代 archive digest；cache clear 与全量管理留给
  后续完整 CLI，但并发/恢复测试要覆盖对象被清理或替换的竞态。
- archive 最大下载字节数、cache 总量/清理策略和 partial retention 是合同审批点；不能为方便测试
  设置无上限，也不在 M4 预建通用 LRU 服务。

## Archive 安全与 extraction staging

- ZIP entry 名先按获批编码规则解析为逻辑 `/` 分隔相对路径；拒绝 absolute/root/prefix、`..`、
  Windows drive/UNC/device/ADS、NUL/control、空 segment 及最终越出 staging root 的路径。
- 不创建或跟随 symlink、hardlink、junction/reparse point、FIFO/device 或其他 special file；entry
  metadata 声明链接即拒绝。每个 staging 父目录以 no-follow/类型检查方式创建或打开。
- 在真正写 entry 前完成全 archive 目录清单预检：duplicate、file-vs-directory conflict、Unicode
  normalization 风险和平台 case-insensitive collision 都返回稳定错误。Windows 还需覆盖 reserved
  names、尾随点/空格和大小写折叠。
- 明确限制 compressed archive bytes、entry 数、单 entry uncompressed bytes、总 uncompressed
  bytes、目录深度、路径长度与 expansion ratio；精确值在合同审批时冻结，并在流式 extraction 中
  再次计数，不能只信 central directory。
- extraction 只能写入本次 Operation 独占 staging；create-new 防覆盖。CRC/EOF/size/hash 任一失败
  清理或保留为 recovery-owned staging，但绝不 publish 到 `Packages/`。
- 完成 extraction 后校验根 package layout、`package.json` identity/version 与 Plan 一致；archive
  不能通过内部 manifest 改写 Plan 固定的 source/hash/version。

## Project filesystem transaction

### 写入集合

- package directory 只允许位于已验证 project root 的 `Packages/<validated-package-id>`。
- `Packages/vpm-manifest.json` 是首个切片的权威 VPM direct/locked mutation 输入；解析和写回必须
  preserve 不相关字段，并使用冻结的 canonical/minimal-diff 策略。
- 是否必须同步修改 `Packages/manifest.json` 由公开 VPM/Unity Fixture 冻结。若该 operation 不需要，
  文件必须 byte-for-byte 不变；若需要，必须加入同一个 ChangeSet/journal，不能作为 best effort。
- M4 不删除 legacy assets、`.meta`、任意 GUID 路径或未列入 ChangeSet 的 package directory。

### Staging、commit 与 rollback

- project staging 必须位于与目标 `Packages/` 相同 filesystem/volume 的已验证项目内临时区域，保证
  rename 语义可用；具体目录名、所有权、symlink/reparse 拒绝和 cleanup 由 ADR 冻结。
- 在首次破坏性 rename 前，所有 archive 已下载、校验、预检、解压；新 manifests 已完整生成到
  temp file，ChangeSet 与现状再次匹配，journal 已持久化并 fsync。
- 旧 package directory 先原子 rename 到 transaction backup；新 directory 从 staging 原子 rename
  到目标。不得 recursive copy 覆盖 live tree，也不得先删除旧目录。
- manifest temp 必须 flush + fsync 后 atomic replace；每次 replace 后同步必要父目录 metadata。
  Windows/macOS/Linux 的 replace-existing 与 directory fsync 差异须由平台测试验证，不能只凭 API
  名称宣称原子。
- 双 manifest 不具备单一 filesystem 原子性。若两者都需改，固定 commit 顺序、每份 old/new digest、
  已提交 marker 与反向补偿；任意时点恢复都能判断继续提交还是恢复两份旧 manifest。
- 文件树验证通过后才在短 SQLite transaction 中提交 Project revision、Operation/Event、journal
  final state 与 durable idempotency result。DB commit 前不得向客户端报告 succeeded。
- 正常错误先尝试恢复完整旧状态；rollback 本身失败时 Operation 进入明确的 recovering/interrupted
  路径，保留 journal/backup，不得清除证据或声称项目可用。

## Filesystem recovery journal

M2 `operation_journal` 只证明 DB-only `state.check` 可恢复，不能直接作为 M4 文件事务安全证明。
M4 合同阶段必须冻结 Schema v3 和 filesystem journal：

- durable record 保存 operation/plan/project IDs、versioned operation kind、ChangeSet fingerprint、
  project/source revisions、各路径的 opaque project-relative identity、old/new digest、phase 与取消意图。
- 不在 DB、Event、日志或普通错误保存完整私密路径、raw manifest、credential 或 archive 内容。
- project-local staging/backup 保存完成恢复所需的文件实体；state.db 保存权威阶段与关联摘要。两者
  都必须在进入下一 destructive phase 前 durable。
- 最小 phase 为：`accepted`、`archive_ready`、`extracted`、`prepared`、`packages_replaced`、
  `manifests_committing`、`filesystem_committed`、`state_committed`，以及显式 rollback/recovery phase。
  最终名称和转换表由 Schema/ADR 冻结，不能复用任意字符串。
- daemon ready 前扫描非终态 package Operation，重新验证 project identity、staging/backup 类型与
  digest，取得同一 Resource Lock 后从 journal 继续或回滚；未知 kind/phase、缺失证据或不一致一律
  fail closed 并返回 diagnostic ID。
- crash matrix 至少覆盖：下载 partial、cache publish、extract entry 中途、journal fsync 前后、旧包
  rename、新包 publish、第一/第二 manifest replace、filesystem commit marker、SQLite final commit、
  response 发送前后。
- recovery 复用原 OperationId、Plan 和幂等 reservation，不创建第二 Operation、不重新 Plan、不从
  当前 catalog 猜测目标状态。
- cancellation 在 destructive phase 前可直接收敛 cancelled；进入项目 mutation 后只能在安全
  checkpoint 执行完整 rollback 或完成不可分割 commit，再报告最终状态，不能留下半提交。

## Resource Lock 与并发

- 复用 M2 coordinator，兼容增加 `PackageCache(digest-or-key)` 或经审批的最小 cache key；已有
  `Project(project_id)` 与 `Repository(repository_id)` 不改变语义。
- Plan 对 catalog 使用 bounded snapshot/revision，不持有锁跨网络或用户等待。Apply 在下载前后与
  项目 commit 前重验已固定 source fingerprint。
- cache lock 只覆盖同 content key 的 lookup/download/publish 临界区；content-addressed object
  发布并验证后释放，不能为整个项目 transaction 持有全局 cache 锁。
- Project lock 从 Apply 最终重验起覆盖 filesystem commit、recovery 与 SQLite finalization；同项目
  install/install、install/remove 和 recovery/write 串行，不同 ProjectId 可并行。
- 若单阶段确实需要多个锁，先去重并使用 `ResourceKey::canonical_bytes` 一次性按序获取；合同测试
  固定 Repository/Catalog、PackageCache、Project、Operation 的 canonical ordering。不得先持有较后
  key 再等待较前 key。
- SQLite transaction 内不等待网络、解压、fsync、Resource Lock、cancel 或其他 Operation。
- repository refresh 与 Apply 的并发通过 revision/fingerprint 产生 `plan_stale`，不能让 Apply 读到
  一半新一半旧的 catalog。

## RPC、Operation、权限与错误

### Operation 与 progress

- Plan 默认是 bounded 同步 RPC；Apply 和任何实际 project mutation 必须返回 OperationId。若 Plan
  后续实测不能在帧/时间界限内完成，需要异步化时必须另行审批，不能在实现中静默改变合同。
- Apply Operation 使用 M2 状态机：queued -> planning/revalidation -> running -> terminal；restart
  使用 interrupted/recovering。`waiting_for_input` 不在首个 M4 切片使用，审批由显式 Plan/Apply
  分离表达。
- progress 是 bounded、阶段化、可脱敏的 Operation 可选字段/Event；不发送完整 path/URL、文件名
  清单或每个 ZIP entry 高频事件。确切 DTO 与事件节流在 RPC Schema 中冻结。
- `operations.cancel` 只记录合作式取消意图；request disconnect 不取消已经返回的 OperationId。
  取消确认不等于最终 cancelled，最终可能 succeeded/failed/cancelled。

### 权限、revision 与幂等

- Plan 至少需要 `projects.read + packages.read + repositories.read` 及目标资源 scope；Apply 需要
  `packages.manage` 与目标 Project scope，并再次校验 Plan owner。是否让 Plan 也要求
  `packages.manage` 是人工审批点。
- `packages.read/manage` 已存在于权限名称基线，但 M4 必须精确定义其 project/source scope；不能
  把 `builtin:local-owner` 当作未来外部客户端 credential 方案。
- Apply 的 expectedRevision 同时约束 project registry aggregate 与外部 project snapshot；外部文件
  被 Unity/用户修改但 DB revision 未变时，也必须由 digest/fingerprint 检出并返回 stale/conflict。
- 永久 idempotency scope 继续是 `(PrincipalId, method, idempotencyKey)`；fingerprint 包含 planId、
  project/source revisions 与 action，不包含 credential/完整 path。

### RPC v1 兼容增加

- 候选 capability 为 `packages.plan.v1` 与 `packages.apply.v1`；最终 method/capability/DTO/error Schema
  经人工审批后，作为 RPC major 1 的兼容新增。
- 旧客户端继续忽略未知 capability/可选字段。M4 不删除或改变 M1-M3 字段和方法语义。
- Plan/Apply public DTO 必须受 4 MiB frame、集合数量、字符串长度和 ChangeSet 项数上限约束；大型
  catalog/diagnostic 不塞进普通响应。
- 至少冻结：`package_not_found`、`package_version_invalid`、`package_range_invalid`、
  `package_dependency_missing`、`package_dependency_conflict`、`package_unity_incompatible`、
  `package_source_ambiguous`、`package_source_changed`、`package_hash_required`、
  `package_integrity_mismatch`、`package_download_too_large`、`package_cache_corrupt`、
  `offline_cache_miss`、`package_archive_invalid`、`package_archive_limit_exceeded`、
  `package_path_invalid`、`package_path_collision`、`plan_not_found`、`plan_stale`、
  `plan_expired`、`plan_already_applied`、`project_transaction_recovery_required`；语义重叠项可在合同
  审批中合并，但不得退化为任意字符串。
- 未知错误继续是 `internal_error + diagnosticId`。普通 error.data 只含安全枚举、资源 opaque ID、
  expected/actual revision 等，不含 raw archive/manifest、SQL、OS debug、完整路径或 credential。

## Fixture、parity 与测试策略

### 公开/synthetic compatibility

- `packages.vpm-format`：公开 VPM repository/package/vpm-manifest/UPM/ProjectVersion Fixture，覆盖
  strict/loose 字段、ID/key mismatch、SemVer/range、Unity compatibility、yanked 与 source identity。
- `packages.version-range-differential`：与公开 SemVer.NET/VPM vectors 对照 exact、bare、caret、
  tilde、hyphen、OR、prerelease、build metadata、intersection 与 ordering。
- `packages.plan-apply`：direct/transitive、multi-range conflict、install/remove/upgrade/downgrade/resolve、
  deterministic source precedence、stale revision/source/hash、no silent replan。
- 所有 remote 测试使用进程内 local mock server，不依赖公网、真实 credential 或第三方仓库可用性。

### 安全与故障

- `packages.integrity-cache`：合法/缺失/畸形/错误 hash、cache/metadata 篡改、offline、HTTP error/
  timeout/partial/oversize、并发 publish 与 restart partial cleanup。
- `packages.archive-security`：ZipSlip、absolute/drive/UNC/device/ADS、separator 混用、symlink/hardlink/
  junction metadata、duplicate/case/Unicode collision、CRC/EOF、非预期编码、深度/路径/entry/总量/
  expansion quotas。
- `packages.path-id-adversarial`：恶意 package ID、repository key/manifest mismatch、project/cache root
  symlink/reparse、最终 resolved path 根外写断言。
- `packages.transaction-faults`：在每个 journal/fsync/rename/manifest/DB/response 边界注入 I/O error，
  并以 kill/TerminateProcess 后重启验证旧或新 deterministic state、无虚假 succeeded。
- `packages.concurrency-locking`：同项目 install/install、install/remove、refresh/apply、read/write，
  不同项目并行，同 digest cache 并发与 recovery/write。

### v3 parity 边界

- public/synthetic Fixture 可证明公开格式、resolver 与事务安全，但不能证明 v3 用户数据差异兼容。
- `projects.v3-parity` 及需要 v3.4.0 安装快照的 package parity 继续 `blocked` 到 M11；不得因 M4
  synthetic test 通过改成 implemented。
- v3/vrc-get 只作为功能行为、公开格式、风险和测试设计参考；不复制、移植、包装或改写其源码。
  可以按许可证使用成熟第三方通用库，不为“独立实现”重复造轮子。

## 生产依赖审批

本草案不批准任何新 production crate。contract-first 阶段必须先评估现有依赖与标准库，再为每个
确有需要的候选提交：crate 名称、精确版本、features/default-features、许可证、维护状态、
Rust 1.97.1/MSRV、直接用途、替代方案、安全记录、binary/compile 成本与完整 Cargo.lock diff。

至少需要分别决策：

1. **SemVer/range**：标准 `semver` 是否足以表达公开 VPM/SemVer.NET range；若不足，只选择一个
   维护良好的 range parser，不同时引入多个互相转换的 version model。
2. **ZIP/archive**：候选库必须支持流式 bounded read、entry metadata、CRC/error、禁用未需压缩
   算法和跨平台 path 检查；不能直接调用 convenience `extract` 绕过本计划安全策略。
3. **hash**：只引入实际使用的 SHA-256 实现和最小 feature；不得因未来签名需求提前加入 crypto
   framework。
4. **temp/atomic file**：优先使用现有 platform adapter 与 std/Tokio；只有确实提供安全 create-new、
   persist/replace 语义且三平台验证后才考虑 crate，不预建通用文件事务库。

禁止提前采用完整 VPM framework、通用 package manager/resolver、ORM、HTTP framework 或 workflow
engine。若候选依赖需要新的 native library、build script、unsafe、第二 TLS stack、proxy/credential
feature 或未获批的大量传递闭包，必须停止审批。`reqwest 0.13.4` 继续保持 M3 已批准的精确
feature，不为 package 下载顺手启用 HTTP/3、system-proxy、cookie、compression 或 blocking。

## Contract-first 实施顺序

1. 冻结 M4 ADR：package/source/precedence、Plan/Apply、cache/archive quotas、filesystem commit、
   recovery、lock/cancel 与 unsupported metadata。
2. 冻结 RPC Schema/error/capability/permission 和 State Schema v3/migration；先通过 JSON/SQL snapshot
   与破坏性兼容测试。
3. 提交生产依赖精确方案与 Cargo.lock diff，取得人工批准；不得先写依赖实现再补审批。
4. 用 public/synthetic Fixture 实现纯 manifest/version/range/resolver，先证明 ChangeSet 确定性。
5. 实现 Plan persistence/stale revalidation，不接项目写入；覆盖 owner/revision/idempotency。
6. 实现 bounded download/content cache 与 hostile archive staging，所有安全负向测试通过后才接项目。
7. 实现 install/remove project transaction、filesystem journal 与进程重启 recovery；再复用 ChangeSet
   完成 upgrade/downgrade/resolve 的最小合同。
8. 接入 RPC/client/最小 CLI 测试驱动，运行并发、故障注入和三平台完整验收。

每一步都不得以放宽 hash、路径、quota、fsync、rollback、权限或 stale Plan 校验来让测试通过。

## Release blocker 与风险

- **来源不确定**：同版本不同 source 未冻结 precedence/digest 时必须阻塞，不得任意选取。
- **Apply 漂移**：任何隐式 re-resolve 或 source fallback 都破坏审批和幂等合同。
- **下载完整性**：缺失/畸形 hash、partial publish 与 offline 坏 cache 不能被当作可用 package。
- **archive 逃逸/耗尽**：根外写、链接、case collision 或无配额 extraction 是 M4 blocker。
- **双 manifest 半提交**：两份文件与 package tree 不是单原子操作，必须由 journal/补偿和 kill test
  证明恢复；只测试普通错误返回不够。
- **DB 与 filesystem 分裂**：M2 DB journal 不覆盖外部 rename/fsync，未知状态必须 fail closed。
- **取消误语义**：进入 destructive phase 后不能立即丢弃 worker；必须安全完成或回滚。
- **并发锁放大**：全局 cache/catalog 长锁会使不同项目不能并行；锁粒度和顺序必须有测试。
- **权限缺口**：真实 credential revocation 未完成，M4 不开放 authenticated source 或外部写客户端。
- **parity 证据缺口**：M11 前不能把 synthetic/public Fixture 描述为真实 v3 differential evidence。
- **平台证据误用**：Windows Server hosted 只证明 M4 构建/测试；真实 Win10/11 安装、WebView2、
  updater 与卸载仍 deferred 到 M12。
- **过度设计**：首个切片不需要 workflow engine、通用 package framework、后台 scheduler 或全部 CLI。

## 需要人工审批的决策点

开始任何 M4 生产实现前，项目所有者至少需要批准：

1. package manifest strict/loose 规则、PackageId、SemVer/range、Unity compatibility、yanked policy。
2. repository precedence、同版本 digest conflict、source fingerprint 与是否增加 repository priority。
3. Plan durability/expiry、ChangeSet DTO、stale reason、Apply method/capability 与 no-replan 合同。
4. package URL/redirect/timeout/size、强 hash 算法、cache key/layout、offline/corruption 语义。
5. ZIP encoding/path/link/collision 与 entry/size/depth/ratio 的精确 quotas。
6. project staging 路径、Packages/manifest mutation、fsync/replace、双 manifest 顺序与 rollback。
7. State Schema v3、filesystem journal phase、recovery/unknown evidence/cancellation 转换表。
8. ResourceKey 增加、lock scope/ordering，以及同/不同项目并发合同。
9. RPC DTO/稳定错误/Operation progress、`packages.read/manage` scope 和 builtin local-owner 授权。
10. 每个新 production crate 的精确版本/features/license/MSRV/Cargo.lock diff。
11. 若需要新的平台 API 或 unsafe，其精确文件/API/资源生命周期与 xtask 门禁。

同一决定获批后，只要实现严格遵循冻结合同，不需要重复审批；任何偏离、新 production crate、扩大
unsafe/平台 API、公共 RPC/DB/permission 变化或进入 M5 都必须再次停止。

## 与 M2、M3、M11、M12 的关系

- M2 提供 Operation/Event/Revision/idempotency/Resource Lock 与 DB journal 基础；M4 兼容扩展而不
  改写既有状态机，但必须新增独立 filesystem transaction 证据。
- M3 提供 Project/Repository identity、revision、normalized catalog 与受限 anonymous HTTP；M4
  不复制 reader，也不把 M3 raw package identity 冒充已解析 resolver model。
- M11 提供真实 v3.4.0 脱敏 Fixture 与 migration/differential evidence；之前相关 parity 保持 blocked。
- M12 负责完整产品安装、数据布局升级/卸载、真实 Win10/Win11 客户端、installer/updater/dist；M4
  不提前承担这些发行面。

## M4 完成后的停止条件

1. 所有获批 ADR、RPC/State Schema、migration、dependency snapshot 与兼容测试通过。
2. install/remove 的真实 Plan -> Apply -> Operation -> filesystem mutation -> kill/restart recovery
   垂直切片通过；upgrade/downgrade/resolve 满足获批最小 ChangeSet 合同。
3. `packages.vpm-format`、`packages.version-range-differential`、`packages.plan-apply`、
   `packages.integrity-cache`、`packages.archive-security`、`packages.path-id-adversarial`、
   `packages.transaction-faults` 与 `packages.concurrency-locking` 达到批准状态。
4. `feature-parity.toml` 只把实际完成的 M4 子集标记 implemented；完整 GUI/CLI/MCP、credential、
   projects/repositories management 与 v3 parity 保持真实未完成。
5. 本地全部验收和三个 hosted 平台 CI 对最终提交成功；报告 Linux GLIBC、macOS arm64/minos、
   Windows 结果、依赖/锁文件证据与工作树状态。
6. 提交并推送最终候选，确认 `HEAD`、`origin/main` 与 CI head SHA 一致，然后停止在 M5 之前等待
   项目所有者人工验收；不得自动开始 M5。

## 进度日志

- 2026-08-19：M3 最终提交 `2082b5596d246975ca7a48dab20826899103e03d` 与 GitHub Actions
  run `32174028968` 已由项目所有者人工验收；M3 正式完成。
- 2026-08-19：创建本 M4 ExecPlan 草案，只规划 package Plan/Apply、download/cache、archive、项目
  filesystem transaction 与 recovery 垂直切片；未修改 RPC/State Schema、permission、migration、
  Cargo 依赖或生产代码，等待项目所有者审批。
